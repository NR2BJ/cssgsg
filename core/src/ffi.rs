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
    /// 후보(candidates_changed이고 candidate_count > 0이면 보인다).
    pub candidates: *const *const c_char,
    pub candidate_count: u32,
    /// 선택된 후보, 없으면 -1.
    pub candidate_selected: i32,
    /// 지금 페이지/전체 페이지(1부터), 모르면 0.
    pub candidate_page: u32,
    pub candidate_pages: u32,
}

pub struct CssgsgEngine {
    engine: Engine,
    out: CssgsgOutput,
    commit: CString,
    preedit: CString,
    segments: Vec<CssgsgSegment>,
    candidates: Vec<CString>,
    candidate_ptrs: Vec<*const c_char>,
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
        if let Some(c) = &cands {
            o.candidate_selected = c.selected.map_or(-1, |i| i as i32);
            if let Some((page, pages)) = c.page {
                o.candidate_page = page as u32;
                o.candidate_pages = pages as u32;
            }
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
    let ctx = Context {
        game_mode: ctx.game_mode != 0,
        taps_disabled: ctx.taps_disabled != 0,
        secure_field: ctx.secure_field != 0,
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
            assert_eq!(cssgsg_engine_mode(e), 0);
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
    fn bad_config_returns_null_with_error() {
        let cfg = CString::new("ko_layout = \"nope\"").unwrap();
        let e = unsafe { cssgsg_engine_new(cfg.as_ptr()) };
        assert!(e.is_null());
        assert!(s(cssgsg_last_error()).contains("nope"));
    }
}
