//! 텍스트 서비스: TSF가 입력기를 쓰는 스레드마다 하나 만들고 Activate/Deactivate를 부른다.

use std::cell::RefCell;

use cssgsg_core::latin::LatinLayout;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfThreadMgr,
    TF_ES_ASYNCDONTCARE, TF_ES_READWRITE,
};
use windows::core::{BOOL, GUID, IUnknownImpl, Interface, Ref, Result, implement};

use crate::edit::InsertText;
use crate::guard::{guarded, poisoned};
use crate::{debug_log, keys};

/// COM 객체는 TSF 스레드(STA) 하나에서만 쓴다. 그래서 RefCell이고, 다른 스레드로 넘길 수 있다고 알리지 않는다(Agile = false).
#[implement(ITfTextInputProcessorEx, ITfKeyEventSink, Agile = false)]
pub struct TextService {
    state: RefCell<Option<Active>>,
    latin: LatinLayout,
}

/// Activate부터 Deactivate까지 쥐고 있는 것.
struct Active {
    thread_mgr: ITfThreadMgr,
    client_id: u32,
}

impl TextService {
    pub fn new() -> Self {
        Self { state: RefCell::new(None), latin: LatinLayout::graphite() }
    }

    /// 이 키를 먹을지, 먹으면 넣을 글자. 상태를 바꾸지 않는다(OnTestKeyDown·OnKeyDown 어느 쪽에서 몇 번 불려도 같다).
    fn decide(&self, wparam: WPARAM, lparam: LPARAM) -> Option<char> {
        if poisoned() {
            return None;
        }
        let press = keys::read(wparam, lparam);
        if press.command {
            return None;
        }
        self.latin.char_for(press.key, press.shift, press.caps, false)
    }

    fn client_id(&self) -> Option<u32> {
        self.state.try_borrow().ok()?.as_ref().map(|a| a.client_id)
    }
}

impl TextService_Impl {
    fn activate(&self, thread_mgr: &ITfThreadMgr, client_id: u32) -> Result<()> {
        let sink: ITfKeyEventSink = self.to_interface();
        let keystrokes: ITfKeystrokeMgr = thread_mgr.cast()?;
        unsafe { keystrokes.AdviseKeyEventSink(client_id, &sink, true)? };
        *self.state.try_borrow_mut().map_err(|_| windows::Win32::Foundation::E_UNEXPECTED)? =
            Some(Active { thread_mgr: thread_mgr.clone(), client_id });
        Ok(())
    }

    fn insert(&self, context: &ITfContext, text: &str) -> bool {
        let Some(client_id) = self.client_id() else { return false };
        let session: ITfEditSession = InsertText::new(context.clone(), text).into();
        match unsafe {
            context.RequestEditSession(client_id, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)
        } {
            Ok(hr) if hr.is_ok() => true,
            Ok(hr) => {
                debug_log(&format!("edit session refused: {hr:?}"));
                false
            }
            Err(e) => {
                debug_log(&format!("RequestEditSession failed: {e:?}"));
                false
            }
        }
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        self.ActivateEx(ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                let active = self.state.try_borrow_mut().ok().and_then(|mut s| s.take());
                if let Some(a) = active
                    && let Ok(keystrokes) = a.thread_mgr.cast::<ITfKeystrokeMgr>()
                {
                    let _ = unsafe { keystrokes.UnadviseKeyEventSink(a.client_id) };
                }
                Ok(())
            },
        )
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    /// 늘 S_OK를 돌려준다. 실패를 돌려주면 TSF가 이 스레드에서 다시 초기화하지 못할 수 있다(chewing).
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                match ptim.ok() {
                    Ok(thread_mgr) => {
                        if let Err(e) = self.activate(thread_mgr, tid) {
                            debug_log(&format!("activate failed: {e:?}"));
                        }
                    }
                    Err(_) => debug_log("activate without a thread manager"),
                }
                Ok(())
            },
        )
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, _foreground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        Ok(guarded(|| false, || self.decide(wparam, lparam).is_some()).into())
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let eaten = guarded(
            || false,
            || {
                let Some(c) = self.decide(wparam, lparam) else { return false };
                let Ok(context) = pic.ok() else { return false };
                // 넣지 못했으면 키를 앱에 넘긴다(쿼티 글자라도 들어가게).
                self.insert(context, c.encode_utf8(&mut [0; 4]))
            },
        );
        Ok(eaten.into())
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SetKeyboardState, VIRTUAL_KEY, VK_A, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_Q, VK_SHIFT,
    };

    fn lparam(scan: u32) -> LPARAM {
        LPARAM((1 | (scan << 16)) as isize)
    }

    /// 이 스레드의 키 상태를 정한다(GetKeyState가 이것을 읽는다).
    fn hold(keys: &[VIRTUAL_KEY]) {
        let mut state = [0u8; 256];
        for k in keys {
            state[k.0 as usize] = 0x80;
        }
        unsafe { SetKeyboardState(&state).unwrap() };
    }

    #[test]
    fn letter_keys_become_graphite_letters() {
        let service = TextService::new();
        hold(&[]);
        assert_eq!(service.decide(WPARAM(VK_Q.0 as usize), lparam(0x10)), Some('b'));
        assert_eq!(service.decide(WPARAM(VK_A.0 as usize), lparam(0x1E)), Some('n'));
        hold(&[VK_SHIFT, VK_LSHIFT]);
        assert_eq!(service.decide(WPARAM(VK_Q.0 as usize), lparam(0x10)), Some('B'));
        hold(&[]);
    }

    #[test]
    fn shortcuts_and_modifiers_pass_through() {
        let service = TextService::new();
        hold(&[VK_CONTROL, VK_LCONTROL]);
        assert_eq!(service.decide(WPARAM(VK_Q.0 as usize), lparam(0x10)), None);
        hold(&[]);
        assert_eq!(service.decide(WPARAM(VK_SHIFT.0 as usize), lparam(0x2A)), None);
    }

    #[test]
    fn the_com_key_sink_eats_downs_and_never_ups() {
        hold(&[]);
        let sink: ITfKeyEventSink = TextService::new().into();
        unsafe {
            let q = (WPARAM(VK_Q.0 as usize), lparam(0x10));
            assert!(sink.OnTestKeyDown(None::<&ITfContext>, q.0, q.1).unwrap().as_bool());
            assert!(!sink.OnTestKeyUp(None::<&ITfContext>, q.0, q.1).unwrap().as_bool());
            assert!(!sink.OnKeyUp(None::<&ITfContext>, q.0, q.1).unwrap().as_bool());
            // 문맥이 없으면 넣을 곳이 없으니 키를 앱에 넘긴다.
            assert!(!sink.OnKeyDown(None::<&ITfContext>, q.0, q.1).unwrap().as_bool());
        }
    }
}
