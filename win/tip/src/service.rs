//! 텍스트 서비스: TSF가 입력기를 쓰는 스레드마다 하나 만들고 Activate/Deactivate를 부른다.
//!
//! 키 하나는 엔진에 한 번만 넣는다. TSF는 시험(OnTestKeyDown/Up: 먹을지 묻기)과 실제(OnKeyDown/Up)를 나눠 부르는데,
//! 앱에 따라 둘 다 오기도, 실제만 오기도 한다(새 메모장, mozc #1415). 그래서 먼저 온 쪽에서 엔진을 돌리고
//! 결과를 메시지(가상 키, lParam, 메시지 시각)로 기억해 둔다. 문서 고치기는 실제 쪽에서 한다. 시험에서 할 일이 있다고
//! 하면 TSF가 실제 쪽을 부른다. 키를 먹지 않는 일(확정 뒤 앱에 넘기기)도 그렇게 실제 쪽에서 먼저 고친 뒤 넘긴다.
//! 키 뗌과 수식키는 먹지 않는다(짝이 안 맞는 뗌을 먹으면 앱에서 키가 눌린 채로 남는다).
//!
//! 모드는 앱 사이에서 하나다(맥은 엔진이 하나). 윈도우는 입력기가 앱 프로세스마다 따로 떠서, 모드를 TSF 전역 칸
//! ([`GUID_COMPARTMENT_MODE`], 값 = 모드 | 직전 비영어 모드 << 4)에 적고 다른 앱의 입력기가 그 알림을 받아 따라간다.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cssgsg_core::{Config, Context, Engine, Mode, Output};
use windows::Win32::Foundation::{E_UNEXPECTED, LPARAM, WPARAM};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CAPITAL,
};
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
    IEnumTfDisplayAttributeInfo, ITfCategoryMgr, ITfCompartment, ITfCompartmentEventSink,
    ITfCompartmentEventSink_Impl, ITfCompartmentMgr, ITfComposition, ITfCompositionSink,
    ITfCompositionSink_Impl, ITfContext, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfEditSession, ITfKeyEventSink, ITfKeyEventSink_Impl,
    ITfKeystrokeMgr, ITfLangBarItemButton, ITfLangBarItemMgr, ITfSource, ITfTextInputProcessor_Impl,
    ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfThreadMgr, ITfThreadMgrEventSink,
    ITfThreadMgrEventSink_Impl, TF_E_SYNCHRONOUS, TF_ES_ASYNCDONTCARE, TF_ES_READWRITE, TF_ES_SYNC,
};
use windows::Win32::UI::WindowsAndMessaging::GetMessageTime;
use windows::core::{
    BOOL, ComObject, GUID, HRESULT, IUnknown, IUnknownImpl, Interface, Ref, Result, implement,
};

use crate::edit::{ApplyOps, Attrs, EndComposition, Slot};
use crate::guard::{guarded, poisoned};
use crate::keys::{self, Clock};
use crate::langbar::ModeButton;
use crate::plan::plan;
use crate::{
    GUID_COMPARTMENT_MODE, GUID_DISPLAY_ATTRIBUTE_FOCUSED, GUID_DISPLAY_ATTRIBUTE_INPUT, debug_enabled,
    debug_log, display,
};

// 입력기 안에서는 CoCreateInstance 대신 이것으로 카테고리 관리자를 얻는다(COMLESS). windows 크레이트 바인딩에 없다.
windows::core::link!("msctf.dll" "system" fn TF_CreateCategoryMgr(ppcat: *mut *mut std::ffi::c_void) -> HRESULT);

fn category_mgr() -> Result<ITfCategoryMgr> {
    let mut raw = std::ptr::null_mut();
    unsafe {
        TF_CreateCategoryMgr(&mut raw).ok()?;
        Ok(ITfCategoryMgr::from_raw(raw))
    }
}

/// INPUTMODE_CONVERSION 칸 값(앱이 지금 입력 모드를 볼 때): 영어는 영숫자, 한국어는 네이티브, 일본어는 네이티브·전각.
const CONVERSION_NATIVE: i32 = 0x1;
const CONVERSION_FULLSHAPE: i32 = 0x8;

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfThreadMgrEventSink,
    ITfCompartmentEventSink,
    Agile = false
)]
pub struct TextService {
    state: RefCell<Option<Active>>,
    slot: Slot,
    seen: RefCell<Option<Seen>>,
    clock: RefCell<Clock>,
    /// 조합이 밖에서 끝났는데 그때 엔진을 빌릴 수 없었다(재진입). 다음 키 앞에서 엔진 조합을 버린다.
    terminated: Cell<bool>,
}

/// Activate부터 Deactivate까지.
struct Active {
    thread_mgr: ITfThreadMgr,
    client_id: u32,
    engine: Engine,
    attrs: Attrs,
    button: Option<ComObject<ModeButton>>,
    thread_cookie: Option<u32>,
    /// 앱 사이에서 같은 모드를 쓰는 전역 칸과 알림 쿠키.
    mode_slot: Option<(ITfCompartment, u32)>,
    /// 개발자 기록(HKCU\Software\cssgsg DebugLog=1): 키 코드와 길이만 남긴다(글자는 남기지 않는다).
    log: bool,
}

/// 지난 키 메시지와 그 결과.
struct Seen {
    id: KeyId,
    out: Output,
    applied: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct KeyId {
    down: bool,
    wparam: usize,
    lparam: isize,
    time: u32,
}

impl TextService {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(None),
            slot: Rc::new(RefCell::new(None)),
            seen: RefCell::new(None),
            clock: RefCell::new(Clock::default()),
            terminated: Cell::new(false),
        }
    }
}

/// 문서를 고칠 일이 있는지(확정·조합).
fn has_edits(out: &Output) -> bool {
    !plan(out).is_empty()
}

fn mode_value(mode: Mode, last_non_en: Mode) -> i32 {
    mode as i32 | (last_non_en as i32) << 4
}

fn conversion_value(mode: Mode) -> i32 {
    match mode {
        Mode::En => 0,
        Mode::Ko => CONVERSION_NATIVE,
        Mode::Ja => CONVERSION_NATIVE | CONVERSION_FULLSHAPE,
    }
}

fn same_object(a: &impl Interface, b: &impl Interface) -> bool {
    match (a.cast::<IUnknown>(), b.cast::<IUnknown>()) {
        (Ok(a), Ok(b)) => a.as_raw() == b.as_raw(),
        _ => false,
    }
}

fn set_i32(compartment: &ITfCompartment, client_id: u32, value: i32) -> Result<()> {
    unsafe { compartment.SetValue(client_id, &VARIANT::from(value)) }
}

fn get_i32(compartment: &ITfCompartment) -> Option<i32> {
    let v = unsafe { compartment.GetValue() }.ok()?;
    i32::try_from(&v).ok()
}

/// Caps Lock 키를 한 번 눌렀다 뗀다(일본어 모드를 나갈 때 가타카나 끄기).
fn toggle_caps_lock() {
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: VK_CAPITAL, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

impl TextService_Impl {
    // ---- 켜기·끄기 -------------------------------------------------------------------------------

    fn activate(&self, thread_mgr: &ITfThreadMgr, client_id: u32) -> Result<()> {
        let attrs = unsafe {
            match category_mgr() {
                Ok(categories) => Attrs {
                    input: categories.RegisterGUID(&GUID_DISPLAY_ATTRIBUTE_INPUT).unwrap_or(0) as i32,
                    focused: categories.RegisterGUID(&GUID_DISPLAY_ATTRIBUTE_FOCUSED).unwrap_or(0) as i32,
                },
                Err(e) => {
                    debug_log(&format!("category manager: {e:?}"));
                    Attrs::default()
                }
            }
        };
        let log = debug_enabled();
        *self.state.try_borrow_mut().map_err(|_| E_UNEXPECTED)? = Some(Active {
            thread_mgr: thread_mgr.clone(),
            client_id,
            engine: Engine::new(Config::default()),
            attrs,
            button: None,
            thread_cookie: None,
            mode_slot: None,
            log,
        });

        let keystrokes: ITfKeystrokeMgr = thread_mgr.cast()?;
        unsafe { keystrokes.AdviseKeyEventSink(client_id, &self.to_interface::<ITfKeyEventSink>(), true)? };

        let me: IUnknown = self.to_interface();
        let thread_cookie = thread_mgr
            .cast::<ITfSource>()
            .and_then(|s| unsafe { s.AdviseSink(&ITfThreadMgrEventSink::IID, &me) })
            .inspect_err(|e| debug_log(&format!("thread manager sink: {e:?}")))
            .ok();

        // 앱 사이에서 같은 모드: 전역 칸에 값이 있으면 따라가고, 없으면 우리 모드를 적는다.
        let mode_slot = unsafe { thread_mgr.GetGlobalCompartment() }
            .and_then(|m| unsafe { m.GetCompartment(&GUID_COMPARTMENT_MODE) })
            .and_then(|c| {
                let cookie =
                    unsafe { c.cast::<ITfSource>()?.AdviseSink(&ITfCompartmentEventSink::IID, &me)? };
                Ok((c, cookie))
            })
            .inspect_err(|e| debug_log(&format!("mode compartment: {e:?}")))
            .ok();

        let button = ComObject::new(ModeButton::new(Mode::Ko));
        let added = thread_mgr
            .cast::<ITfLangBarItemMgr>()
            .and_then(|m| unsafe { m.AddItem(&button.to_interface::<ITfLangBarItemButton>()) })
            .inspect_err(|e| debug_log(&format!("language bar item: {e:?}")))
            .is_ok();

        if let Ok(mut state) = self.state.try_borrow_mut()
            && let Some(a) = state.as_mut()
        {
            a.thread_cookie = thread_cookie;
            a.mode_slot = mode_slot;
            a.button = added.then_some(button);
            if let Ok(m) = thread_mgr.cast::<ITfCompartmentMgr>()
                && let Ok(c) = unsafe { m.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) }
            {
                let _ = set_i32(&c, client_id, 1);
            }
        }
        self.follow_global_mode(true);
        Ok(())
    }

    fn deactivate(&self) {
        self.end_composition();
        let Some(a) = self.state.try_borrow_mut().ok().and_then(|mut s| s.take()) else { return };
        unsafe {
            if let Ok(keystrokes) = a.thread_mgr.cast::<ITfKeystrokeMgr>() {
                let _ = keystrokes.UnadviseKeyEventSink(a.client_id);
            }
            if let (Some(cookie), Ok(source)) = (a.thread_cookie, a.thread_mgr.cast::<ITfSource>()) {
                let _ = source.UnadviseSink(cookie);
            }
            if let Some((c, cookie)) = &a.mode_slot
                && let Ok(source) = c.cast::<ITfSource>()
            {
                let _ = source.UnadviseSink(*cookie);
            }
            if let (Some(button), Ok(items)) = (&a.button, a.thread_mgr.cast::<ITfLangBarItemMgr>()) {
                let _ = items.RemoveItem(&button.to_interface::<ITfLangBarItemButton>());
            }
        }
        *self.seen.borrow_mut() = None;
    }

    // ---- 키 ------------------------------------------------------------------------------------

    /// 키 메시지 하나를 엔진에 (한 번만) 넣고 결과를 돌려준다. 이미 본 메시지면 그때 결과와 적용 여부.
    fn process(
        &self,
        down: bool,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<(Output, bool)> {
        if poisoned() {
            return None;
        }
        let id = KeyId { down, wparam: wparam.0, lparam: lparam.0, time: unsafe { GetMessageTime() } as u32 };
        if let Ok(seen) = self.seen.try_borrow()
            && let Some(s) = seen.as_ref()
            && s.id == id
        {
            return Some((s.out.clone(), s.applied));
        }
        // 앞 메시지의 할 일이 남았다(시험만 하고 실제를 부르지 않은 앱): 지금이라도 적용한다.
        let left = self.seen.try_borrow_mut().ok().and_then(|mut s| s.take()).filter(|s| !s.applied);
        if let (Some(left), Some(context)) = (left, context) {
            debug_log("key test without the key: applying its edits late");
            self.apply(context, &left.out, false);
        }
        if let Some(context) = context {
            self.settle_context(context);
        }
        let ev = keys::event(down, wparam, lparam, self.clock.try_borrow_mut().ok()?.seconds(id.time));
        let out = {
            let mut state = self.state.try_borrow_mut().ok()?;
            let a = state.as_mut()?;
            if self.terminated.take() {
                let _ = a.engine.reset();
            }
            let out = a.engine.handle_key(&ev, &Context::default());
            if a.log {
                debug_log(&format!(
                    "key {:?} down={} mods={:?} repeat={} -> eat={} commit={} preedit={:?} mode={:?}",
                    ev.key,
                    ev.down,
                    ev.mods,
                    ev.repeat,
                    out.consumed,
                    out.commit.chars().count(),
                    out.preedit.as_ref().map(|p| p.text.chars().count()),
                    out.mode
                ));
            }
            out
        };
        self.after(&out);
        if let Ok(mut seen) = self.seen.try_borrow_mut() {
            *seen = Some(Seen { id, out: out.clone(), applied: false });
        }
        Some((out, false))
    }

    fn mark_applied(&self) {
        if let Ok(mut seen) = self.seen.try_borrow_mut()
            && let Some(s) = seen.as_mut()
        {
            s.applied = true;
        }
    }

    /// 시험: 실제 쪽을 불러 달라고 할지(먹거나, 문서를 고칠 일이 있으면).
    fn test(&self, down: bool, context: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> bool {
        let Some((out, applied)) = self.process(down, context, wparam, lparam) else { return false };
        let edits = !applied && has_edits(&out);
        if !edits {
            self.mark_applied();
        }
        (down && out.consumed) || edits
    }

    /// 실제: 문서를 고치고, 먹었는지 돌려준다(뗌은 늘 넘긴다).
    fn key(&self, down: bool, context: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> bool {
        let Some((out, applied)) = self.process(down, context, wparam, lparam) else { return false };
        if !applied {
            if let Some(context) = context {
                // 키를 앱에 넘기면 그 키보다 먼저 고쳐야 한다(조합 중 Ctrl, 기호 키).
                self.apply(context, &out, !(down && out.consumed));
            }
            self.mark_applied();
        }
        down && out.consumed
    }

    /// 모드가 바뀌었으면 전역 칸·입력 모드 칸·아이콘에 알린다. 일본어를 나가면 Caps Lock을 끈다.
    fn after(&self, out: &Output) {
        if let Some(mode) = out.mode {
            self.show_mode(mode, true);
        }
        if out.caps_lock_off && keys::caps_on() {
            toggle_caps_lock();
        }
        if out.timer_ms.is_some() {
            debug_log("timer requested (quick tap buffering is not wired on Windows yet)");
        }
    }

    // ---- 문서 고치기 ------------------------------------------------------------------------------

    /// 엔진 출력대로 문서를 고친다. `before_key`면 키를 앱에 넘기기 전이라 동기로 먼저 한다.
    fn apply(&self, context: &ITfContext, out: &Output, before_key: bool) -> bool {
        let ops = plan(out);
        if ops.is_empty() {
            return true;
        }
        let Some((client_id, attrs)) =
            self.state.try_borrow().ok().and_then(|s| s.as_ref().map(|a| (a.client_id, a.attrs)))
        else {
            return false;
        };
        let session = ApplyOps::new(
            context.clone(),
            ops,
            self.slot.clone(),
            self.to_interface::<ITfCompositionSink>(),
            attrs,
        );
        let ran = session.ran();
        let session: ITfEditSession = session.into();
        let request = |flags| unsafe { context.RequestEditSession(client_id, &session, flags) };
        let mut result = request(if before_key {
            TF_ES_SYNC | TF_ES_READWRITE
        } else {
            TF_ES_ASYNCDONTCARE | TF_ES_READWRITE
        });
        if before_key && matches!(result, Ok(hr) if hr == TF_E_SYNCHRONOUS) {
            debug_log("synchronous edit refused: the commit may land after the key");
            result = request(TF_ES_ASYNCDONTCARE | TF_ES_READWRITE);
        }
        match result {
            Ok(hr) if hr.is_ok() => {
                if !ran.get() {
                    debug_log("edit session deferred (async)");
                }
                true
            }
            Ok(hr) => {
                debug_log(&format!("edit session failed: {hr:?}"));
                false
            }
            Err(e) => {
                debug_log(&format!("RequestEditSession: {e:?}"));
                false
            }
        }
    }

    /// 다른 문맥에 조합이 남아 있으면(포커스가 옮겨 갔다) 그 자리에서 끝낸다.
    fn settle_context(&self, context: &ITfContext) {
        let elsewhere =
            self.slot.try_borrow().ok().and_then(|s| s.as_ref().map(|c| !same_object(&c.context, context)));
        if elsewhere == Some(true) {
            self.end_composition();
        }
    }

    /// 조합을 그 자리에서 끝내고(글자는 남는다) 엔진의 조합을 버린다.
    fn end_composition(&self) {
        if let Some(c) = self.slot.try_borrow_mut().ok().and_then(|mut s| s.take())
            && let Some(client_id) =
                self.state.try_borrow().ok().and_then(|s| s.as_ref().map(|a| a.client_id))
        {
            let session: ITfEditSession = EndComposition::new(c.clone()).into();
            if let Err(e) = unsafe {
                c.context.RequestEditSession(client_id, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)
            } {
                debug_log(&format!("end composition: {e:?}"));
            }
        }
        match self.state.try_borrow_mut() {
            Ok(mut s) => {
                if let Some(a) = s.as_mut() {
                    let _ = a.engine.reset();
                }
            }
            Err(_) => self.terminated.set(true),
        }
    }

    // ---- 모드 ------------------------------------------------------------------------------------

    /// 모드를 아이콘·입력 모드 칸에 보이고, `publish`면 전역 칸에도 적는다(다른 앱이 따라온다).
    fn show_mode(&self, mode: Mode, publish: bool) {
        let Ok(state) = self.state.try_borrow() else { return };
        let Some(a) = state.as_ref() else { return };
        if let Some(button) = &a.button {
            button.set_mode(mode);
        }
        if let Ok(m) = a.thread_mgr.cast::<ITfCompartmentMgr>()
            && let Ok(c) = unsafe { m.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION) }
        {
            let _ = set_i32(&c, a.client_id, conversion_value(mode));
        }
        if publish && let Some((c, _)) = &a.mode_slot {
            let _ = set_i32(c, a.client_id, mode_value(mode, a.engine.last_non_en()));
        }
    }

    /// 전역 칸의 모드를 따라간다. 칸이 비었으면(처음 뜬 입력기) 우리 모드를 적는다.
    fn follow_global_mode(&self, first: bool) {
        let followed = {
            let Ok(mut state) = self.state.try_borrow_mut() else { return };
            let Some(a) = state.as_mut() else { return };
            let value = a.mode_slot.as_ref().and_then(|(c, _)| get_i32(c));
            match value.map(|v| (Mode::from_i32(v & 0xF), Mode::from_i32(v >> 4 & 0xF))) {
                Some((Some(mode), Some(last))) => {
                    if mode == a.engine.mode() && last == a.engine.last_non_en() && !first {
                        return;
                    }
                    let out = a.engine.follow_mode(mode, last);
                    Some((
                        a.engine.mode(),
                        out.preedit.is_some_and(|p| p.text.is_empty()) && self.has_composition(),
                    ))
                }
                _ => {
                    // 처음 뜬 입력기: 우리 모드를 적는다.
                    let m = a.engine.mode();
                    if let Some((c, _)) = &a.mode_slot {
                        let _ = set_i32(c, a.client_id, mode_value(m, a.engine.last_non_en()));
                    }
                    Some((m, false))
                }
            }
        };
        if let Some((mode, dropped)) = followed {
            if dropped {
                // 엔진 조합은 버렸으니 문서의 조합도 그 자리에서 끝낸다.
                self.end_composition();
            }
            self.show_mode(mode, false);
        }
    }

    fn has_composition(&self) -> bool {
        self.slot.try_borrow().map(|s| s.is_some()).unwrap_or(false)
    }
}

// ---- COM ----------------------------------------------------------------------------------------

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        self.ActivateEx(ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                self.deactivate();
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

    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        Ok(guarded(|| false, || self.test(true, pic.ok().ok(), wparam, lparam)).into())
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        Ok(guarded(|| false, || self.key(true, pic.ok().ok(), wparam, lparam)).into())
    }

    fn OnTestKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        Ok(guarded(|| false, || self.test(false, pic.ok().ok(), wparam, lparam)).into())
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        Ok(guarded(|| false, || self.key(false, pic.ok().ok(), wparam, lparam)).into())
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    /// 앱이 조합을 끝냈다(클릭, 포커스 등). 글자는 앱에 그대로 남는다. 우리 칸을 비우고 엔진의 조합도 버린다.
    fn OnCompositionTerminated(&self, _ecwrite: u32, pcomposition: Ref<ITfComposition>) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                let ours = match (self.slot.try_borrow_mut(), pcomposition.ok()) {
                    (Ok(mut slot), Ok(ended))
                        if slot.as_ref().is_some_and(|c| same_object(&c.composition, ended)) =>
                    {
                        slot.take().is_some()
                    }
                    _ => false,
                };
                if ours {
                    match self.state.try_borrow_mut() {
                        Ok(mut s) => {
                            if let Some(a) = s.as_mut() {
                                let _ = a.engine.reset();
                            }
                        }
                        Err(_) => self.terminated.set(true),
                    }
                }
                Ok(())
            },
        )
    }
}

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(display::EnumAttributes::new().into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        if guid.is_null() {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        display::find(unsafe { &*guid })
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    /// 입력칸이 바뀌었다: 조합을 그 자리에서 끝낸다(맥 deactivate와 같다).
    fn OnSetFocus(&self, _focus: Ref<ITfDocumentMgr>, _previous: Ref<ITfDocumentMgr>) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                if self.has_composition() {
                    self.end_composition();
                }
                Ok(())
            },
        )
    }

    fn OnPushContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfCompartmentEventSink_Impl for TextService_Impl {
    /// 다른 앱(또는 이 앱의 다른 스레드)의 입력기가 모드를 바꿨다.
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                if !rguid.is_null() && unsafe { *rguid } == GUID_COMPARTMENT_MODE {
                    self.follow_global_mode(false);
                }
                Ok(())
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{SetKeyboardState, VK_Q, VK_SHIFT};

    fn lparam(scan: u32, up: bool) -> LPARAM {
        LPARAM((1 | (scan << 16) | if up { 3 << 30 } else { 0 }) as isize)
    }

    #[test]
    fn mode_values_round_trip() {
        for (m, l) in [(Mode::En, Mode::Ko), (Mode::En, Mode::Ja), (Mode::Ko, Mode::Ko), (Mode::Ja, Mode::Ja)]
        {
            let v = mode_value(m, l);
            assert_eq!((Mode::from_i32(v & 0xF), Mode::from_i32(v >> 4 & 0xF)), (Some(m), Some(l)));
        }
        assert_eq!(conversion_value(Mode::Ja), 9);
    }

    #[test]
    fn without_activation_keys_pass_through() {
        // 켜지기 전(또는 실패한 켜기)에는 아무 키도 먹지 않는다.
        unsafe { SetKeyboardState(&[0u8; 256]).unwrap() };
        let sink: ITfKeyEventSink = TextService::new().into();
        unsafe {
            let q = (WPARAM(VK_Q.0 as usize), lparam(0x10, false));
            assert!(!sink.OnTestKeyDown(None::<&ITfContext>, q.0, q.1).unwrap().as_bool());
            assert!(!sink.OnKeyDown(None::<&ITfContext>, q.0, q.1).unwrap().as_bool());
            let shift_up = (WPARAM(VK_SHIFT.0 as usize), lparam(0x36, true));
            assert!(!sink.OnKeyUp(None::<&ITfContext>, shift_up.0, shift_up.1).unwrap().as_bool());
        }
    }
}
