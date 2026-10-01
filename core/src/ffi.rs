//! C ABI. 셸(맥 Swift, 나중에 윈도우 TIP)이 부르는 함수들. 헤더는 `core/include/cssgsg.h`.
//!
//! - 문자열은 UTF-8, NUL로 끝난다.
//! - 돌려주는 `CssgsgOutput`과 그 안의 포인터는 같은 엔진의 다음 호출 전까지만 유효하다.
//! - 한 엔진은 한 스레드에서만 쓴다(IMK는 메인 스레드).
//! - 패닉은 여기서 잡는다. 패닉이 나면 엔진 상태를 비우고 "키를 앱에 넘김"을 돌려준다.
//!   입력기가 죽는 것보다 한 글자 흘리는 편이 낫다.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use crate::config::Config;
use crate::engine::{Context, Engine, Mode, Output};
use crate::hanja::Learning;
use crate::key::{Key, KeyEvent, Mods};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CssgsgKeyEvent {
    /// HID 키 코드(`cssgsg_key_from_mac_keycode`로 얻는다).
    pub key: u16,
    pub down: u8,
    pub is_repeat: u8,
    /// `CSSGSG_MOD_*` 비트.
    pub mods: u32,
    /// 초 단위 단조 시각(NSEvent.timestamp).
    pub time: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CssgsgContext {
    pub game_mode: u8,
    pub taps_disabled: u8,
    pub secure_field: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CssgsgSegment {
    /// UTF-16 단위 시작 위치(NSString과 같다).
    pub start: u32,
    /// UTF-16 단위 길이.
    pub len: u32,
    pub focused: u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct CssgsgOutput {
    pub consumed: u8,
    pub preedit_changed: u8,
    pub candidates_changed: u8,
    pub caps_lock_off: u8,
    /// 바뀐 모드(CSSGSG_MODE_*), 안 바뀌었으면 -1.
    pub mode: i32,
    /// 확정할 글자. 없으면 빈 문자열(NULL 아님).
    pub commit: *const c_char,
    /// 조합 중 글자(preedit_changed일 때만 의미가 있다).
    pub preedit: *const c_char,
    /// 커서 위치(UTF-16 단위). 항상 끝이다.
    pub preedit_caret: u32,
    pub segments: *const CssgsgSegment,
    pub segment_count: u32,
    /// 후보: 포커스된 문절의 후보 전체(candidates_changed이고 candidate_count > 0이면 보인다). 페이지는 후보창이 나눈다.
    pub candidates: *const *const c_char,
    pub candidate_count: u32,
    /// 선택된 후보(전체 목록 기준), 없으면 -1.
    pub candidate_selected: i32,
    /// 지금 페이지/전체 페이지(1부터, 지금 목록/격자 모드 기준), 모르면 0.
    pub candidate_page: u32,
    pub candidate_pages: u32,
    /// 격자(펼친) 모드면 1. 목록은 한 페이지 9개, 격자는 5열 × 6행.
    pub candidate_grid: u8,
    /// 한자 학습이 바뀌었다(셸이 저장한다).
    pub learning_changed: u8,
    /// 후보마다 같이 보일 뜻(candidate_count개, 빈 문자열 가능). 뜻이 없는 후보창이면 NULL.
    pub candidate_notes: *const *const c_char,
    /// 0이 아니면 이만큼(밀리초) 뒤에 `cssgsg_engine_timer`를 불러 달라(빠른 탭 전환 보정).
    pub timer_ms: u32,
}

/// 맥 셸 설정(설정 파일의 `[mac]`). 엔진은 쓰지 않는다.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CssgsgMacSettings {
    pub hud: u8,
    /// 1이면 HUD를 마우스 옆에, 0이면 커서 위에.
    pub hud_at_mouse: u8,
    /// 후보창 글자 크기(포인트).
    pub candidate_font_size: u32,
    /// 줄바꿈 넣기(웹 기술로 만든 앱)·⌘ 단축키 다시 보내기(모든 앱) 대기(밀리초).
    pub newline_insert_wait_ms: u32,
    /// Shift+Enter 다시 보내기 대기(밀리초, `cssgsg_engine_newline_key_press_apps`의 앱).
    pub newline_key_press_wait_ms: u32,
}

pub struct CssgsgEngine {
    engine: Engine,
    out: CssgsgOutput,
    commit: CString,
    preedit: CString,
    segments: Vec<CssgsgSegment>,
    candidates: Vec<CString>,
    candidate_ptrs: Vec<*const c_char>,
    notes: Vec<CString>,
    note_ptrs: Vec<*const c_char>,
    learning_tsv: CString,
    newline_key_press_apps: CString,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(msg: &str) {
    LAST_ERROR.with(|e| *e.borrow_mut() = cstring(msg));
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

fn empty_output() -> CssgsgOutput {
    CssgsgOutput {
        consumed: 0,
        preedit_changed: 0,
        candidates_changed: 0,
        caps_lock_off: 0,
        mode: -1,
        commit: ptr::null(),
        preedit: ptr::null(),
        preedit_caret: 0,
        segments: ptr::null(),
        segment_count: 0,
        candidates: ptr::null(),
        candidate_count: 0,
        candidate_selected: -1,
        candidate_page: 0,
        candidate_pages: 0,
        candidate_grid: 0,
        learning_changed: 0,
        candidate_notes: ptr::null(),
        timer_ms: 0,
    }
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

impl CssgsgEngine {
    fn store(&mut self, out: Output) -> *const CssgsgOutput {
        self.commit = cstring(&out.commit);
        let mut o = empty_output();
        o.consumed = out.consumed as u8;
        o.caps_lock_off = out.caps_lock_off as u8;
        o.learning_changed = out.learning_changed as u8;
        o.timer_ms = out.timer_ms.unwrap_or(0);
        o.mode = out.mode.map_or(-1, |m| m as i32);
        o.commit = self.commit.as_ptr();

        o.preedit_changed = out.preedit.is_some() as u8;
        let preedit = out.preedit.unwrap_or_default();
        self.preedit = cstring(&preedit.text);
        o.preedit = self.preedit.as_ptr();
        o.preedit_caret = utf16_len(&preedit.text);
        // 글자 단위 구간 → UTF-16 단위.
        let chars: Vec<char> = preedit.text.chars().collect();
        let u16_at = |i: usize| chars[..i.min(chars.len())].iter().map(|c| c.len_utf16() as u32).sum::<u32>();
        self.segments = preedit
            .segments
            .iter()
            .map(|s| {
                let start = u16_at(s.start);
                CssgsgSegment { start, len: u16_at(s.start + s.len) - start, focused: s.focused as u8 }
            })
            .collect();
        o.segments = self.segments.as_ptr();
        o.segment_count = self.segments.len() as u32;

        o.candidates_changed = out.candidates.is_some() as u8;
        let cands = out.candidates.flatten();
        self.candidates = cands.iter().flat_map(|c| c.items.iter()).map(|s| cstring(s)).collect();
        self.candidate_ptrs = self.candidates.iter().map(|c| c.as_ptr()).collect();
        o.candidates = self.candidate_ptrs.as_ptr();
        o.candidate_count = self.candidate_ptrs.len() as u32;
        self.notes = cands.iter().flat_map(|c| c.notes.iter()).map(|s| cstring(s)).collect();
        self.note_ptrs = self.notes.iter().map(|c| c.as_ptr()).collect();
        if !self.note_ptrs.is_empty() && self.note_ptrs.len() == self.candidate_ptrs.len() {
            o.candidate_notes = self.note_ptrs.as_ptr();
        }
        if let Some(c) = &cands {
            o.candidate_selected = c.selected.map_or(-1, |i| i as i32);
            if let Some((page, pages)) = c.page {
                o.candidate_page = page as u32;
                o.candidate_pages = pages as u32;
            }
            o.candidate_grid = c.grid as u8;
        }

        self.out = o;
        &self.out
    }

    /// 패닉 뒤: 상태를 버리고 키는 앱에 넘긴다.
    fn recover(&mut self) -> *const CssgsgOutput {
        let _ = catch_unwind(AssertUnwindSafe(|| self.engine.reset()));
        self.store(Output::default())
    }
}

/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
unsafe fn run(e: *mut CssgsgEngine, f: impl FnOnce(&mut Engine) -> Output) -> *const CssgsgOutput {
    // SAFETY: 호출자가 보장한다.
    let Some(e) = (unsafe { e.as_mut() }) else { return ptr::null() };
    match catch_unwind(AssertUnwindSafe(|| f(&mut e.engine))) {
        Ok(out) => e.store(out),
        Err(_) => {
            set_error("엔진 패닉: 상태를 비우고 키를 앱에 넘겼다");
            e.recover()
        }
    }
}

/// 엔진을 만든다. `config_toml`이 NULL이면 기본 설정. 설정 오류면 NULL(`cssgsg_last_error` 참고).
///
/// # Safety
/// `config_toml`은 NULL이거나 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_new(config_toml: *const c_char) -> *mut CssgsgEngine {
    let config = if config_toml.is_null() {
        Config::default()
    } else {
        // SAFETY: NUL로 끝나는 문자열이라고 약속한다.
        let src = unsafe { CStr::from_ptr(config_toml) }.to_string_lossy();
        match Config::from_toml(&src) {
            Ok(c) => c,
            Err(err) => {
                set_error(&err);
                return ptr::null_mut();
            }
        }
    };
    let made = catch_unwind(|| Engine::new(config));
    let Ok(engine) = made else {
        set_error("엔진을 만들다 패닉");
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(CssgsgEngine {
        engine,
        out: empty_output(),
        commit: CString::default(),
        preedit: CString::default(),
        segments: Vec::new(),
        candidates: Vec::new(),
        candidate_ptrs: Vec::new(),
        notes: Vec::new(),
        note_ptrs: Vec::new(),
        learning_tsv: CString::default(),
        newline_key_press_apps: CString::default(),
    }))
}

/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
/// 해제한 뒤에는 다시 쓰지 않는다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_free(e: *mut CssgsgEngine) {
    if !e.is_null() {
        // SAFETY: cssgsg_engine_new가 만든 포인터를 한 번만 넘긴다.
        drop(unsafe { Box::from_raw(e) });
    }
}

/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
/// `ev`는 유효한 구조체를, `ctx`는 NULL이거나 유효한 구조체를 가리켜야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_handle_key(
    e: *mut CssgsgEngine,
    ev: *const CssgsgKeyEvent,
    ctx: *const CssgsgContext,
) -> *const CssgsgOutput {
    // SAFETY: 셸이 유효한 구조체 포인터를 넘긴다(ctx는 NULL 허용).
    let Some(ev) = (unsafe { ev.as_ref() }).copied() else { return ptr::null() };
    let ctx = unsafe { ctx.as_ref() }.copied().unwrap_or_default();
    let event = KeyEvent {
        key: Key(ev.key),
        down: ev.down != 0,
        mods: Mods(ev.mods),
        repeat: ev.is_repeat != 0,
        time: ev.time,
    };
    // secure_latin은 맥(이 C ABI를 쓰는 셸)에서는 쓰지 않는다: 비밀번호 칸에 글자를 넣을 수 없다.
    let ctx = Context {
        game_mode: ctx.game_mode != 0,
        taps_disabled: ctx.taps_disabled != 0,
        secure_field: ctx.secure_field != 0,
        secure_latin: false,
    };
    unsafe { run(e, |engine| engine.handle_key(&event, &ctx)) }
}

/// 조합 중인 것을 확정한다(포커스 해제 등).
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_commit(e: *mut CssgsgEngine) -> *const CssgsgOutput {
    unsafe { run(e, |engine| engine.commit_all()) }
}

/// `timer_ms`만큼 기다린 뒤 부른다. `now`는 키 이벤트와 같은 시계(초, NSEvent.timestamp)다.
/// `held`는 지금 실제로 누르고 있는 수식키(CSSGSG_MOD_*, 좌우를 모르면 양쪽 비트).
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_timer(
    e: *mut CssgsgEngine,
    now: f64,
    held: u32,
) -> *const CssgsgOutput {
    unsafe { run(e, |engine| engine.timer(now, Mods(held))) }
}

/// 셸이 엔진에 넘기지 않은 키가 눌렸다(Shift+Enter를 다시 보내기 전에 잡아 둔 키): 진행 중인 수식키 탭을 무효로 한다.
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_cancel_tap(e: *mut CssgsgEngine) {
    // SAFETY: 위 약속대로.
    if let Some(e) = unsafe { e.as_mut() } {
        e.engine.cancel_tap();
    }
}

/// 변환기(Mozc)의 사용자 사전을 다시 읽는다(설정 앱이 고친 뒤).
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_reload_dictionary(e: *mut CssgsgEngine) -> u8 {
    // SAFETY: 위 약속대로.
    let Some(e) = (unsafe { e.as_mut() }) else { return 0 };
    catch_unwind(AssertUnwindSafe(|| e.engine.reload_converter())).is_ok() as u8
}

/// 마우스 클릭: 조합을 확정하고 진행 중인 수식키 탭을 무효로 한다.
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_mouse_down(e: *mut CssgsgEngine) -> *const CssgsgOutput {
    unsafe { run(e, |engine| engine.mouse_down()) }
}

/// 확정하지 않고 비운다(입력기 활성화 때).
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_reset(e: *mut CssgsgEngine) -> *const CssgsgOutput {
    unsafe { run(e, |engine| engine.reset()) }
}

/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_set_mode(e: *mut CssgsgEngine, mode: i32) -> *const CssgsgOutput {
    let Some(mode) = Mode::from_i32(mode) else { return ptr::null() };
    unsafe { run(e, |engine| engine.set_mode(mode)) }
}

/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_mode(e: *const CssgsgEngine) -> i32 {
    // SAFETY: cssgsg_engine_new가 준 포인터.
    unsafe { e.as_ref() }.map_or(-1, |e| e.engine.mode() as i32)
}

/// NULL이면 빈 문자열.
///
/// # Safety
/// `p`는 NULL이거나 NUL로 끝나는 문자열이어야 한다.
unsafe fn str_arg(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: 호출자가 보장한다.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// 설정을 바꾼다(설정 앱이 설정 파일을 고친 뒤). 조합 중인 것·모드·학습은 그대로 둔다.
/// `config_toml`이 NULL이면 기본 설정. 성공하면 1, 설정 오류면 0(그대로 두고 `cssgsg_last_error`).
///
/// # Safety
/// `e`는 `cssgsg_engine_new`가 돌려준 살아 있는 포인터, `config_toml`은 NULL이거나 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_set_config(e: *mut CssgsgEngine, config_toml: *const c_char) -> u8 {
    // SAFETY: 위 약속대로.
    let Some(e) = (unsafe { e.as_mut() }) else { return 0 };
    let config = if config_toml.is_null() {
        Config::default()
    } else {
        match Config::from_toml(&unsafe { str_arg(config_toml) }) {
            Ok(c) => c,
            Err(err) => {
                set_error(&err);
                return 0;
            }
        }
    };
    match catch_unwind(AssertUnwindSafe(|| e.engine.set_config(config))) {
        Ok(()) => 1,
        Err(_) => {
            set_error("설정을 바꾸다 패닉");
            0
        }
    }
}

/// 한자 학습(저장 형식 TSV)을 불러와 지금 것을 바꾼다. 읽은 항목 수를 돌려준다.
///
/// # Safety
/// `e`는 `cssgsg_engine_new`가 돌려준 살아 있는 포인터, `tsv`는 NULL이거나 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_hanja_learning_load(e: *mut CssgsgEngine, tsv: *const c_char) -> u32 {
    // SAFETY: 위 약속대로.
    let Some(e) = (unsafe { e.as_mut() }) else { return 0 };
    let src = unsafe { str_arg(tsv) };
    let learning = Learning::from_tsv(&src);
    let count = learning.len() as u32;
    e.engine.set_hanja_learning(learning);
    count
}

/// 한자 학습을 저장 형식(TSV)으로 돌려준다. 다음 이 함수 호출이나 엔진 해제 전까지 유효하다.
///
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_hanja_learning_save(e: *mut CssgsgEngine) -> *const c_char {
    // SAFETY: 위 약속대로.
    let Some(e) = (unsafe { e.as_mut() }) else { return ptr::null() };
    e.learning_tsv = cstring(&e.engine.hanja_learning().to_tsv());
    e.learning_tsv.as_ptr()
}

/// 이 코어가 읽을 수 있는 Mozc 엔진 C API 판(mozc/cssgsg/cssgsg_mozc.h의 CSSGSG_MOZC_ABI_VERSION).
/// `mozc` 기능 없이 빌드했으면 0. 셸은 이 판의 엔진만 받고 읽는다.
#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_mozc_abi() -> i32 {
    #[cfg(feature = "mozc")]
    {
        crate::mozc::SUPPORTED_ABI
    }
    #[cfg(not(feature = "mozc"))]
    {
        0
    }
}

/// Mozc(일본어 한자 변환)를 켠다: 엔진 라이브러리를 읽고 만든 뒤 낱말 하나를 변환해 본다. 성공하면 1.
/// 실패하면 0이고 변환기는 그대로다. 까닭은 `cssgsg_last_error`(`mozc` 기능 없이 빌드했을 때도).
///
/// # Safety
/// `e`는 `cssgsg_engine_new`가 돌려준 살아 있는 포인터, 세 문자열은 NUL로 끝나야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_use_mozc(
    e: *mut CssgsgEngine,
    library_path: *const c_char,
    data_path: *const c_char,
    profile_dir: *const c_char,
) -> u8 {
    // SAFETY: 위 약속대로 살아 있는 엔진이다.
    let Some(e) = (unsafe { e.as_mut() }) else { return 0 };
    if library_path.is_null() || data_path.is_null() || profile_dir.is_null() {
        set_error("mozc: 경로가 NULL");
        return 0;
    }
    #[cfg(feature = "mozc")]
    {
        // SAFETY: NUL로 끝나는 문자열이다.
        let (library, data, profile) = unsafe {
            (
                CStr::from_ptr(library_path).to_string_lossy(),
                CStr::from_ptr(data_path).to_string_lossy(),
                CStr::from_ptr(profile_dir).to_string_lossy(),
            )
        };
        match crate::mozc::MozcConverter::load(&library, &data, &profile) {
            Ok(converter) => {
                e.engine.set_converter(Box::new(converter));
                1
            }
            Err(err) => {
                set_error(&format!("mozc: {err}"));
                0
            }
        }
    }
    #[cfg(not(feature = "mozc"))]
    {
        let _ = e;
        set_error("mozc 기능 없이 빌드했다");
        0
    }
}

/// 맥 셸 설정을 읽는다. `e`가 NULL이면 기본값.
///
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_mac_settings(e: *const CssgsgEngine) -> CssgsgMacSettings {
    let default = crate::config::MacConfig::default();
    // SAFETY: 위 약속대로 NULL이거나 살아 있는 엔진이다.
    let mac = unsafe { e.as_ref() }.map(|e| &e.engine.config().mac).unwrap_or(&default);
    CssgsgMacSettings {
        hud: mac.hud as u8,
        hud_at_mouse: (mac.hud_position == crate::config::HudPosition::Mouse) as u8,
        candidate_font_size: mac.candidate_font_size,
        newline_insert_wait_ms: mac.newline_insert_wait_ms,
        newline_key_press_wait_ms: mac.newline_key_press_wait_ms,
    }
}

/// Shift+Enter 키를 다시 보낼 앱(설정 파일 `[mac] newline_key_press_apps`)의 번들 ID를 줄바꿈으로 이은 글자열.
/// 빈 목록이면 빈 글자열. 다음 이 함수 호출이나 엔진 해제 전까지 유효하다. `e`가 NULL이면 NULL.
///
/// # Safety
/// `e`는 NULL이거나 `cssgsg_engine_new`가 돌려준, 아직 해제하지 않은 포인터여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_engine_newline_key_press_apps(e: *mut CssgsgEngine) -> *const c_char {
    // SAFETY: 위 약속대로.
    let Some(e) = (unsafe { e.as_mut() }) else { return ptr::null() };
    e.newline_key_press_apps = cstring(&e.engine.config().mac.newline_key_press_apps.join("\n"));
    e.newline_key_press_apps.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_key_from_mac_keycode(code: u16) -> u16 {
    Key::from_mac_keycode(code).0
}

/// 이 스레드의 마지막 오류 메시지(없으면 빈 문자열).
#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(p: *const c_char) -> String {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }

    fn key(mac: u16, t: f64) -> CssgsgKeyEvent {
        CssgsgKeyEvent { key: cssgsg_key_from_mac_keycode(mac), down: 1, is_repeat: 0, mods: 0, time: t }
    }

    #[test]
    fn round_trip_through_c_abi() {
        unsafe {
            let e = cssgsg_engine_new(ptr::null());
            assert!(!e.is_null());
            assert_eq!(cssgsg_engine_mode(e), 1, "한국어로 시작한다");
            assert_eq!((*cssgsg_engine_set_mode(e, 0)).mode, 0);
            let out = &*cssgsg_engine_set_mode(e, 1);
            assert_eq!(out.mode, 1);

            // k(0x28) f(0x03) → 가
            cssgsg_engine_handle_key(e, &key(0x28, 1.0), ptr::null());
            let out = &*cssgsg_engine_handle_key(e, &key(0x03, 1.1), ptr::null());
            assert_eq!(out.consumed, 1);
            assert_eq!(out.preedit_changed, 1);
            assert_eq!(s(out.preedit), "가");
            assert_eq!(out.preedit_caret, 1);
            assert_eq!(s(out.commit), "");

            let out = &*cssgsg_engine_commit(e);
            assert_eq!(s(out.commit), "가");
            cssgsg_engine_free(e);
        }
    }

    #[test]
    fn candidates_and_utf16_segments() {
        unsafe {
            let e = cssgsg_engine_new(ptr::null());
            cssgsg_engine_set_mode(e, 2);
            cssgsg_engine_handle_key(e, &key(0x01, 1.0), ptr::null()); // s → か
            let out = &*cssgsg_engine_handle_key(e, &key(0x31, 1.1), ptr::null()); // Space → 변환
            assert_eq!(out.candidates_changed, 1);
            assert_eq!(out.candidate_count, 2);
            let items: Vec<String> =
                (0..out.candidate_count as usize).map(|i| s(*out.candidates.add(i))).collect();
            assert_eq!(items, vec!["か", "カ"]);
            assert_eq!(out.candidate_selected, 0);
            assert_eq!(out.segment_count, 1);
            let seg = *out.segments;
            assert_eq!((seg.start, seg.len, seg.focused), (0, 1, 1));
            cssgsg_engine_free(e);
        }
    }

    #[test]
    fn hanja_through_c_abi() {
        unsafe {
            let e = cssgsg_engine_new(ptr::null());
            cssgsg_engine_set_mode(e, 1);
            // 국 = k r e (맥 키코드 0x28 0x0F 0x0E)
            for (i, code) in [0x28u16, 0x0F, 0x0E].into_iter().enumerate() {
                cssgsg_engine_handle_key(e, &key(code, 1.0 + i as f64 / 10.0), ptr::null());
            }
            let mut ev = key(0x24, 2.0); // Option+Return: 조합 중인 국을 바꾼다
            ev.mods = crate::key::Mods::ALT_L;
            let out = &*cssgsg_engine_handle_key(e, &ev, ptr::null());
            assert_eq!((out.consumed, out.preedit_changed, out.candidates_changed), (1, 1, 1));
            assert_eq!(s(out.preedit), "國");
            assert!(out.candidate_count > 3 && !out.candidate_notes.is_null());
            assert_eq!(s(*out.candidates.add(1)), "局");
            assert_eq!(s(*out.candidate_notes.add(0)), "나라 국");
            let out = &*cssgsg_engine_commit(e);
            assert_eq!((s(out.commit).as_str(), out.learning_changed), ("國", 1));

            let tsv = s(cssgsg_engine_hanja_learning_save(e));
            assert!(tsv.contains("국\t國\t1\t"), "{tsv}");
            let other = cssgsg_engine_new(ptr::null());
            let tsv = CString::new(tsv).unwrap();
            assert_eq!(cssgsg_engine_hanja_learning_load(other, tsv.as_ptr()), 1);
            assert_eq!(cssgsg_engine_hanja_learning_load(other, ptr::null()), 0);
            // 일본어 후보에는 뜻 배열이 없다
            cssgsg_engine_set_mode(other, 2);
            cssgsg_engine_handle_key(other, &key(0x01, 3.0), ptr::null());
            let out = &*cssgsg_engine_handle_key(other, &key(0x31, 3.1), ptr::null());
            assert!(out.candidate_count > 0 && out.candidate_notes.is_null());
            cssgsg_engine_free(other);
            cssgsg_engine_free(e);
        }
    }

    #[test]
    fn config_changes_while_running() {
        unsafe {
            let e = cssgsg_engine_new(ptr::null());
            cssgsg_engine_set_mode(e, 1);
            let cfg = CString::new("[mac]\nhud = false\ncandidate_font_size = 20").unwrap();
            assert_eq!(cssgsg_engine_set_config(e, cfg.as_ptr()), 1);
            let m = cssgsg_engine_mac_settings(e);
            assert_eq!((m.hud, m.candidate_font_size), (0, 20));
            assert_eq!(cssgsg_engine_mode(e), 1, "모드는 그대로");
            let bad = CString::new("ko_layout = \"nope\"").unwrap();
            assert_eq!(cssgsg_engine_set_config(e, bad.as_ptr()), 0);
            assert!(s(cssgsg_last_error()).contains("nope"));
            assert_eq!(cssgsg_engine_mac_settings(e).hud, 0, "오류면 그대로");
            assert_eq!(cssgsg_engine_set_config(e, ptr::null()), 1);
            assert_eq!(cssgsg_engine_mac_settings(e).hud, 1, "NULL은 기본 설정");
            cssgsg_engine_free(e);
        }
    }

    #[test]
    fn bad_config_returns_null_with_error() {
        let cfg = CString::new("ko_layout = \"nope\"").unwrap();
        let e = unsafe { cssgsg_engine_new(cfg.as_ptr()) };
        assert!(e.is_null());
        assert!(s(cssgsg_last_error()).contains("nope"));
    }
}
