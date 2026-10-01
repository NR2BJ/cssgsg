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

use cssgsg_core::hanja::Learning;
use cssgsg_core::{Config, Context, Engine, Mode, Output};
use windows::Win32::Foundation::{E_UNEXPECTED, LPARAM, WPARAM};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    SendInput, VIRTUAL_KEY, VK_CAPITAL, VK_PACKET,
};
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED,
    GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
    IEnumTfDisplayAttributeInfo, ITfCategoryMgr, ITfCompartment, ITfCompartmentEventSink,
    ITfCompartmentEventSink_Impl, ITfCompartmentMgr, ITfComposition, ITfCompositionSink,
    ITfCompositionSink_Impl, ITfContext, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfEditSession, ITfKeyEventSink, ITfKeyEventSink_Impl,
    ITfKeystrokeMgr, ITfLangBarItemButton, ITfLangBarItemMgr, ITfSource, ITfTextInputProcessor_Impl,
    ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfThreadMgr, ITfThreadMgrEventSink,
    ITfThreadMgrEventSink_Impl, TF_E_SYNCHRONOUS, TF_ES_ASYNCDONTCARE, TF_ES_READ, TF_ES_READWRITE,
    TF_ES_SYNC, TF_SD_READONLY,
};
use windows::Win32::UI::WindowsAndMessaging::GetMessageTime;
use windows::core::{
    BOOL, ComObject, GUID, HRESULT, IUnknown, IUnknownImpl, Interface, Ref, Result, implement,
};

use crate::edit::{ApplyOps, Attrs, EndComposition, Measure, ReadInputScope, ScopeRead, Slot, UiHook};
use crate::guard::{guarded, poisoned};
use crate::host::{self, HostConverter, HostLink};
use crate::keys::{self, Clock};
use crate::langbar::ModeButton;
use crate::plan::plan;
use crate::ui::Screen;
use crate::{
    GUID_COMPARTMENT_MODE, GUID_DISPLAY_ATTRIBUTE_FOCUSED, GUID_DISPLAY_ATTRIBUTE_INPUT, debug_log, display,
    read_debug_flag,
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
    /// 지난번에 읽은 입력 범위. 조합 중에는 입력칸이 그대로라 다시 읽지 않는다.
    scope: RefCell<ScopeRead>,
    /// 지난 키의 입력칸 종류(실제 쪽에서 문서를 고칠 때 쓴다).
    field: Cell<Field>,
    /// 후보창과 모드 HUD(편집 세션이 자리를 잰 뒤 맞춘다).
    screen: Rc<RefCell<Screen>>,
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
    /// 엔진 호스트 연결(일본어 한자 변환, 설정 파일, 한자 기억). 엔진의 변환기와 같이 쥔다.
    host: Option<HostLink>,
    /// 호스트에서 받아 쓰고 있는 (설정 파일, 한자 기억)의 판. 처음은 0(기본 설정, 빈 기억).
    synced: (u64, u64),
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
            scope: RefCell::new(ScopeRead::NotRun),
            field: Cell::new(Field::Normal),
            screen: Rc::new(RefCell::new(Screen::default())),
        }
    }
}

/// 입력칸 종류([`decide`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Normal,
    /// 입력기가 글자를 바꾸면 안 되는 곳(읽기 전용, 앱이 입력기를 끈 문맥, 웹 페이지 본문, 문맥 없음): 키를 넘긴다.
    /// 언어 전환 탭은 된다(갇히지 않게).
    Closed,
    /// 비밀번호 칸: 모드와 상관없이 Graphite로 바로 확정한다(사용자 결정 "비밀번호도 Graphite").
    Password,
}

/// 입력칸 종류를 TSF에서 읽은 것으로 정한다. 둘째 값은 근거(개발자 기록).
/// - 읽기 전용이면 닫힌 칸. Chromium(윈도우 11)의 페이지 본문 문서는 빈 텍스트 저장소가 읽기 전용이라고 알린다.
/// - 입력 범위가 IS_PASSWORD·IS_NUMERIC_PASSWORD면 비밀번호 칸(Firefox 등).
/// - 입력기 끔(GUID_COMPARTMENT_KEYBOARD_DISABLED)과 빈 문맥(GUID_COMPARTMENT_EMPTYCONTEXT)이 둘 다 켜졌는데 선택 영역이
///   읽히면 비밀번호 칸: Chromium은 비밀번호 칸 문서에 둘을 켜고(`ui/base/ime/win/tsf_bridge.cc` InitializeDisabledContext),
///   입력 범위는 IS_PASSWORD가 아니라 IS_PRIVATE로 준다(`tsf_input_scope.cc`). Firefox가 둘을 켜는 입력칸 아닌 곳
///   (페이지 본문)은 선택 영역을 주지 않는다.
/// - 그 밖에 하나라도 켜졌으면 닫힌 칸.
fn decide(readonly: bool, scope: &ScopeRead, disabled: bool, empty: bool) -> (Field, &'static str) {
    if readonly {
        (Field::Closed, "readonly")
    } else if scope.is_password() {
        (Field::Password, "scope")
    } else if disabled && empty && scope.readable() {
        (Field::Password, "disabled+empty")
    } else if disabled {
        (Field::Closed, "disabled")
    } else if empty {
        (Field::Closed, "empty")
    } else {
        (Field::Normal, "")
    }
}

/// 문서를 언제 고치는지([`TextService_Impl::apply`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum When {
    /// 키를 먹었다: 앱이 편할 때(비동기여도 된다).
    Whenever,
    /// 키를 앱에 넘기기 전: 동기로 먼저 한다. 거절되면 비동기로라도.
    BeforeKey,
    /// 지금 동기로만. 못 하면 false(비밀번호 칸은 유니코드 키 입력으로 넣는다. 비동기로 미루면 두 번 들어갈 수 있다).
    Now,
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

/// 글자를 유니코드 키 입력(VK_PACKET)으로 넣는다. 앱이 문서 고치기를 받지 않을 때만 쓴다: 이미 줄 선 키보다
/// 뒤에 들어간다. 이 키는 입력기도 다시 받는데, [`TextService_Impl::process`]가 VK_PACKET은 넘긴다.
fn inject_text(text: &str) {
    let inputs: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|unit| {
            [KEYBD_EVENT_FLAGS(0), KEYEVENTF_KEYUP].map(|up| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: unit,
                        dwFlags: KEYEVENTF_UNICODE | up,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            })
        })
        .collect();
    if !inputs.is_empty() {
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }
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
        let log = read_debug_flag();
        // 일본어 한자 변환은 사용자당 하나인 엔진 호스트가 한다(Mozc). 연결은 처음 변환할 때(또는 일본어로 바꿀 때) 잇는다.
        let mut engine = Engine::new(Config::windows_default());
        let host = host::link();
        if let Some(link) = &host {
            engine.set_converter(Box::new(HostConverter::new(link.clone())));
        }
        *self.state.try_borrow_mut().map_err(|_| E_UNEXPECTED)? = Some(Active {
            thread_mgr: thread_mgr.clone(),
            client_id,
            engine,
            attrs,
            button: None,
            thread_cookie: None,
            mode_slot: None,
            log,
            host,
            synced: (0, 0),
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
        self.sync_host();
        Ok(())
    }

    /// 엔진 호스트와 설정 파일·한자 기억을 맞춘다(켤 때, 입력칸이 바뀔 때: 설정 앱에서 고치고 돌아오면 바로 쓴다).
    /// 호스트에 닿지 못하면(아직 안 떴다, 띄울 수 없는 앱) 지금 것을 그대로 쓴다.
    fn sync_host(&self) {
        let Some((link, known)) = self
            .state
            .try_borrow()
            .ok()
            .and_then(|s| s.as_ref().and_then(|a| Some((a.host.clone()?, a.synced))))
        else {
            return;
        };
        let Some((config, learning)) = host::sync(&link, known) else { return };
        let Ok(mut state) = self.state.try_borrow_mut() else { return };
        let Some(a) = state.as_mut() else { return };
        if let Some(c) = config {
            match Config::from_toml_windows(&c.text) {
                Ok(parsed) => {
                    a.engine.set_config(parsed);
                    a.synced.0 = c.version;
                    debug_log("settings applied");
                }
                // 호스트는 올바른 설정만 주지만, 판이 다른 입력기·호스트가 섞였을 때를 위해.
                Err(e) => debug_log(&format!("settings: {e}")),
            }
        }
        if let Some(l) = learning {
            a.engine.set_hanja_learning(Learning::from_tsv(&l.text));
            a.synced.1 = l.version;
        }
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
        // 유니코드 키 입력(VK_PACKET)은 글자가 이미 정해져 있다: 화상 키보드, 비밀번호 관리자의 자동 입력, 우리 inject_text.
        if poisoned() || wparam.0 == VK_PACKET.0 as usize {
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
            self.apply(context, &left.out, When::Whenever);
        }
        if let Some(context) = context {
            self.settle_context(context);
        }
        // 문맥이 없으면(입력칸이 없는 창, 입력기를 끈 창: 게임, Win32 비밀번호 칸) 글자를 넣을 곳이 없다: 키를 넘긴다.
        let (field, why) = context
            .map_or((Field::Closed, "no context"), |c| self.classify(c, down && !self.has_composition()));
        self.field.set(field);
        let ctx = Context {
            secure_field: field != Field::Normal,
            secure_latin: field == Field::Password,
            ..Context::default()
        };
        let ev = keys::event(down, wparam, lparam, self.clock.try_borrow_mut().ok()?.seconds(id.time));
        let out = {
            let mut state = self.state.try_borrow_mut().ok()?;
            let a = state.as_mut()?;
            if self.terminated.take() {
                let _ = a.engine.reset();
            }
            let out = a.engine.handle_key(&ev, &ctx);
            if let Ok(mut screen) = self.screen.try_borrow_mut() {
                screen.queue(out.candidates.clone(), out.mode, a.engine.mode() == Mode::Ja);
            }
            if a.log {
                debug_log(&format!(
                    "key {:?} down={} mods={:?} repeat={} field={:?}({}) scope={:?} -> eat={} commit={} preedit={:?} mode={:?}",
                    ev.key,
                    ev.down,
                    ev.mods,
                    ev.repeat,
                    field,
                    why,
                    self.scope.try_borrow().map(|s| s.clone()).unwrap_or_default(),
                    out.consumed,
                    out.commit.chars().count(),
                    out.preedit.as_ref().map(|p| p.text.chars().count()),
                    out.mode
                ));
            }
            // 한자를 골랐으면 호스트에 알린다(기억은 호스트가 모아 저장하고 다른 앱에 나눠 준다).
            let pick = out
                .learning_changed
                .then(|| a.engine.hanja_learning().last_pick().map(|(r, t)| (r.to_string(), t.to_string())))
                .flatten()
                .zip(a.host.clone());
            (out, pick)
        };
        let (out, pick) = out;
        if let Some(((reading, text), link)) = pick {
            host::report_pick(&link, &reading, &text);
        }
        self.after(&out);
        if let Ok(mut seen) = self.seen.try_borrow_mut() {
            *seen = Some(Seen { id, out: out.clone(), applied: false });
        }
        Some((out, false))
    }

    /// 이 입력칸이 어떤 칸인지([`decide`]). 둘째 값은 근거(개발자 기록).
    ///
    /// 입력 범위는 편집 쿠키가 있어야 읽혀서 `read_scope`일 때만(조합이 없는 키 눌림) 동기 읽기 세션으로 읽고,
    /// 아니면 지난 값을 쓴다(조합 중에는 입력칸이 그대로다).
    fn classify(&self, context: &ITfContext, read_scope: bool) -> (Field, &'static str) {
        let readonly = unsafe { context.GetStatus() }.is_ok_and(|s| s.dwDynamicFlags & TF_SD_READONLY != 0);
        if read_scope && !readonly {
            let read = self.read_input_scope(context);
            if let Ok(mut scope) = self.scope.try_borrow_mut() {
                *scope = read;
            }
        }
        let flag = |guid: &GUID| {
            context
                .cast::<ITfCompartmentMgr>()
                .ok()
                .and_then(|m| unsafe { m.GetCompartment(guid) }.ok())
                .and_then(|c| get_i32(&c))
                .is_some_and(|v| v != 0)
        };
        let (disabled, empty) =
            (flag(&GUID_COMPARTMENT_KEYBOARD_DISABLED), flag(&GUID_COMPARTMENT_EMPTYCONTEXT));
        let Ok(scope) = self.scope.try_borrow() else { return (Field::Closed, "busy") };
        // 개인 입력칸(시크릿 창 등)이면 일본어 변환과 한자 기억이 배우지 않는다(호스트 연결에 적어 둔다).
        if let Ok(state) = self.state.try_borrow()
            && let Some(link) = state.as_ref().and_then(|a| a.host.as_ref())
        {
            link.private.set(scope.is_private());
        }
        decide(readonly, &scope, disabled, empty)
    }

    fn read_input_scope(&self, context: &ITfContext) -> ScopeRead {
        let Some(client_id) = self.state.try_borrow().ok().and_then(|s| s.as_ref().map(|a| a.client_id))
        else {
            return ScopeRead::NotRun;
        };
        let session = ReadInputScope::new(context.clone());
        let result = session.result();
        let session: ITfEditSession = session.into();
        match unsafe { context.RequestEditSession(client_id, &session, TF_ES_SYNC | TF_ES_READ) } {
            Ok(hr) if hr.is_ok() => result.take(),
            _ => ScopeRead::NotRun,
        }
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
            // 문서는 그대로인데 화면만 바뀌었다(모드 HUD, 후보 페이지): 실제 쪽이 안 올 수 있으니 지금 맞춘다.
            self.refresh_screen(context);
        }
        (down && out.consumed) || edits
    }

    /// 실제: 문서를 고치고, 먹었는지 돌려준다(뗌은 늘 넘긴다).
    fn key(&self, down: bool, context: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> bool {
        let Some((out, applied)) = self.process(down, context, wparam, lparam) else { return false };
        if !applied {
            match context {
                // 비밀번호 칸: 앱이 문서 고치기를 바로 받지 않으면(빈 문맥 등) 글자를 유니코드 키 입력으로 넣는다.
                Some(context) if self.field.get() == Field::Password => {
                    if !self.apply(context, &out, When::Now) {
                        debug_log("password field: typing the commit as unicode input");
                        inject_text(&out.commit);
                    }
                }
                // 키를 앱에 넘기면 그 키보다 먼저 고쳐야 한다(조합 중 Ctrl, 기호 키).
                Some(context) => {
                    let when = if down && out.consumed { When::Whenever } else { When::BeforeKey };
                    self.apply(context, &out, when);
                }
                None => self.refresh_screen(None),
            }
            self.mark_applied();
        }
        down && out.consumed
    }

    /// 편집 세션이 잰 자리로 후보창·HUD를 맞추는 고리.
    fn ui_hook(&self) -> UiHook {
        let screen = self.screen.clone();
        Rc::new(move |rect| {
            if let Ok(mut s) = screen.try_borrow_mut() {
                s.flush(rect);
            }
        })
    }

    /// 문서는 고치지 않고 화면만 맞춘다. 자리는 읽기 세션으로 재고, 못 재면 마지막 자리(없으면 마우스 옆).
    fn refresh_screen(&self, context: Option<&ITfContext>) {
        if !self.screen.try_borrow().is_ok_and(|s| s.pending()) {
            return;
        }
        let client_id = self.state.try_borrow().ok().and_then(|s| s.as_ref().map(|a| a.client_id));
        if let (Some(context), Some(client_id)) = (context, client_id) {
            let session: ITfEditSession =
                Measure::new(context.clone(), self.slot.clone(), self.ui_hook()).into();
            match unsafe { context.RequestEditSession(client_id, &session, TF_ES_ASYNCDONTCARE | TF_ES_READ) }
            {
                Ok(hr) if hr.is_ok() => return,
                other => debug_log(&format!("measure session: {other:?}")),
            }
        }
        if let Ok(mut s) = self.screen.try_borrow_mut() {
            s.flush(None);
        }
    }

    /// 모드가 바뀌었으면 전역 칸·입력 모드 칸·아이콘에 알린다. 일본어를 나가면 Caps Lock을 끈다.
    fn after(&self, out: &Output) {
        if let Some(mode) = out.mode {
            self.show_mode(mode, true);
        }
        // 이 앱에서 일본어로 바꿨다: 첫 변환을 기다리지 않게 엔진 호스트를 미리 띄우고 잇는다.
        if out.mode == Some(Mode::Ja) {
            let link = self.state.try_borrow().ok().and_then(|s| s.as_ref().and_then(|a| a.host.clone()));
            if let Some(link) = link {
                host::prepare(&link);
            }
        }
        if out.caps_lock_off && keys::caps_on() {
            toggle_caps_lock();
        }
        if out.timer_ms.is_some() {
            debug_log("timer requested (quick tap buffering is not wired on Windows yet)");
        }
    }

    // ---- 문서 고치기 ------------------------------------------------------------------------------

    /// 엔진 출력대로 문서를 고친다. 고쳤거나 고치기로 했으면(비동기) true.
    fn apply(&self, context: &ITfContext, out: &Output, when: When) -> bool {
        let ops = plan(out);
        if ops.is_empty() {
            self.refresh_screen(Some(context));
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
            Some(self.ui_hook()),
        );
        let ran = session.ran();
        let session: ITfEditSession = session.into();
        let request = |flags| unsafe { context.RequestEditSession(client_id, &session, flags) };
        let mut result = request(match when {
            When::Whenever => TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
            When::BeforeKey | When::Now => TF_ES_SYNC | TF_ES_READWRITE,
        });
        if matches!(result, Ok(hr) if hr == TF_E_SYNCHRONOUS) {
            if when == When::Now {
                debug_log("synchronous edit refused");
                return false;
            }
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
        self.drop_engine_composition();
    }

    /// 엔진의 조합을 버리고 후보창을 닫는다(문서의 조합은 이미 끝났다). 엔진을 빌릴 수 없으면(재진입) 다음 키 앞에서.
    fn drop_engine_composition(&self) {
        match self.state.try_borrow_mut() {
            Ok(mut s) => {
                if let Some(a) = s.as_mut() {
                    let _ = a.engine.reset();
                }
            }
            Err(_) => self.terminated.set(true),
        }
        if let Ok(mut screen) = self.screen.try_borrow_mut() {
            screen.close_candidates();
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
                    self.drop_engine_composition();
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

    /// 입력칸이 바뀌었다: 조합을 그 자리에서 끝낸다(맥 deactivate와 같다). 비밀번호 칸인지는 새 칸에서 다시 읽는다.
    fn OnSetFocus(&self, _focus: Ref<ITfDocumentMgr>, _previous: Ref<ITfDocumentMgr>) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                if let Ok(mut scope) = self.scope.try_borrow_mut() {
                    *scope = ScopeRead::NotRun;
                }
                if self.has_composition() {
                    self.end_composition();
                }
                // 설정 앱에서 고치고 돌아왔을 수 있다.
                self.sync_host();
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
    fn fields_follow_what_the_app_reports() {
        use windows::Win32::UI::TextServices::IS_PASSWORD;
        let field = |readonly, scope: &ScopeRead, disabled, empty| decide(readonly, scope, disabled, empty).0;
        let password_scope = ScopeRead::Scopes(vec![IS_PASSWORD.0]);
        // Firefox 비밀번호 칸: 입력기 끔 + 입력 범위 IS_PASSWORD.
        assert_eq!(field(false, &password_scope, true, false), Field::Password);
        // Chromium 비밀번호 칸(Edge 기록): 입력기 끔·빈 문맥을 켜고 입력 범위는 IS_PRIVATE(61), 숫자 칸은 IS_DIGITS(28)도.
        assert_eq!(field(false, &ScopeRead::Scopes(vec![61]), true, true), Field::Password);
        assert_eq!(field(false, &ScopeRead::Scopes(vec![61, 28]), true, true), Field::Password);
        assert_eq!(field(false, &ScopeRead::Empty, true, true), Field::Password);
        // Chromium 페이지 본문(윈도우 11): 빈 텍스트 저장소가 읽기 전용이라고 알린다.
        assert_eq!(field(true, &ScopeRead::NotRun, true, true), Field::Closed);
        // Firefox 페이지 본문(기록): 둘 다 켰고 세션은 돌지만 선택 영역을 주지 않는다. 세션이 돌았는지만 본 판은 이것을
        // 비밀번호 칸으로 봐서 YouTube 단축키를 먹었다.
        assert_eq!(field(false, &ScopeRead::Failed("selection"), true, true), Field::Closed);
        assert_eq!(field(false, &ScopeRead::NotRun, true, true), Field::Closed);
        // 입력기만 끈 칸, 빈 문맥만 켠 칸.
        assert_eq!(field(false, &ScopeRead::Empty, true, false), Field::Closed);
        assert_eq!(field(false, &ScopeRead::Empty, false, true), Field::Closed);
        // 보통 칸(입력 범위 IS_DEFAULT).
        assert_eq!(field(false, &ScopeRead::Scopes(vec![0]), false, false), Field::Normal);
        assert_eq!(field(false, &ScopeRead::Failed("value"), false, false), Field::Normal);
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
