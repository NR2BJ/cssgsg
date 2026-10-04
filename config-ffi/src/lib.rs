//! 설정 앱용 C ABI: 설정 파일(TOML) ↔ JSON, Mozc 사용자 사전 파일 ↔ JSON. 헤더는 `config-ffi/include/cssgsg_config.h`.
//!
//! 설정 앱은 입력기 코어 정적 라이브러리(Mozc가 묶여 있다)를 링크하지 않고 이것만 링크한다.
//! 설정의 모양과 값 검사는 코어의 [`Config`] 한 곳에만 있다.
//!
//! - 문자열은 UTF-8, NUL로 끝난다.
//! - 돌려주는 문자열은 같은 스레드에서 이 라이브러리를 다시 부르기 전까지만 유효하다.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::catch_unwind;
use std::ptr;

use cssgsg_core::Config;

pub mod userdict;

thread_local! {
    static OUT: RefCell<CString> = RefCell::new(CString::default());
    static ERR: RefCell<CString> = RefCell::new(CString::default());
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// 설정 파일 내용(TOML)을 JSON으로. 적지 않은 설정은 기본값을 채운다. `None`이면 기본 설정.
pub fn json_from_toml(toml: Option<&str>) -> Result<String, String> {
    let config = match toml {
        Some(text) => Config::from_toml(text)?,
        None => Config::default(),
    };
    serde_json::to_string(&config).map_err(|e| e.to_string())
}

/// JSON(설정 앱이 고친 설정)을 설정 파일 내용으로. 적지 않은 설정은 기본값, 값 검사도 한다.
pub fn toml_from_json(json: &str) -> Result<String, String> {
    let config: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
    config.validate()?;
    Ok(config.to_toml())
}

/// 윈도우: 설정 파일 내용을 JSON으로. 윈도우 입력기가 읽는 것과 같다(적지 않은 한자 단축키는 윈도우 기본값).
pub fn json_from_toml_windows(toml: Option<&str>) -> Result<String, String> {
    let config = match toml {
        Some(text) => Config::from_toml_windows(text)?,
        None => Config::windows_default(),
    };
    serde_json::to_string(&config).map_err(|e| e.to_string())
}

/// 윈도우: JSON을 설정 파일 내용으로(윈도우 기본값과 같은 설정은 주석, [`Config::to_toml_windows`]).
/// JSON에 없는 설정은 맥 기본값이 되므로 설정 앱은 읽은 JSON 전체를 고쳐서 준다.
pub fn toml_from_json_windows(json: &str) -> Result<String, String> {
    let config: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
    config.validate()?;
    Ok(config.to_toml_windows())
}

/// 윈도우 스캔 코드(확장 키 비트)의 키 이름. 수식키는 `Err`로 수식키 이름(`shift_left`), 모르는 키는 `Ok(None)`.
pub fn windows_key_name(scan: u16, extended: bool) -> Result<Option<&'static str>, &'static str> {
    let key = cssgsg_core::Key::from_windows_scancode(scan, extended);
    if let Some(modifier) = cssgsg_core::shortcut::modifier_name(key) {
        return Err(modifier);
    }
    Ok(cssgsg_core::shortcut::key_name(key))
}

fn give(result: std::thread::Result<Result<String, String>>) -> *const c_char {
    match result.unwrap_or_else(|_| Err("설정을 변환하다 멈췄습니다(패닉)".into())) {
        Ok(text) => OUT.with(|o| {
            *o.borrow_mut() = cstring(&text);
            o.borrow().as_ptr()
        }),
        Err(err) => {
            ERR.with(|e| *e.borrow_mut() = cstring(&err));
            ptr::null()
        }
    }
}

/// # Safety
/// `p`는 NULL이거나 NUL로 끝나는 문자열이어야 한다.
unsafe fn arg(p: *const c_char) -> Option<String> {
    // SAFETY: 호출자가 보장한다.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// 설정 파일 내용(TOML)을 JSON으로. NULL이면 기본 설정. 설정 오류면 NULL(`cssgsg_config_error`).
///
/// # Safety
/// `toml`은 NULL이거나 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_config_json(toml: *const c_char) -> *const c_char {
    // SAFETY: 위 약속대로.
    let toml = unsafe { arg(toml) };
    give(catch_unwind(|| json_from_toml(toml.as_deref())))
}

/// JSON을 설정 파일 내용(설명이 달린 TOML)으로. 오류면 NULL(`cssgsg_config_error`).
///
/// # Safety
/// `json`은 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_config_toml(json: *const c_char) -> *const c_char {
    // SAFETY: 위 약속대로.
    let json = unsafe { arg(json) }.unwrap_or_default();
    give(catch_unwind(|| toml_from_json(&json)))
}

/// Mozc 사용자 사전 파일(`user_dictionary.db`)을 JSON으로: `{"dictionaries":[{"id","name","entries":[{"key","value","comment","pos","locale"}]}]}`.
/// 파일이 없으면 빈 목록. 오류면 NULL(`cssgsg_config_error`).
///
/// # Safety
/// `path`는 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_userdict_json(path: *const c_char) -> *const c_char {
    // SAFETY: 위 약속대로.
    let path = unsafe { arg(path) }.unwrap_or_default();
    give(catch_unwind(|| {
        let storage = userdict::load(&path)?;
        serde_json::to_string(&storage).map_err(|e| e.to_string())
    }))
}

/// JSON(`{"dictionaries":[...]}`)을 사용자 사전 파일에 쓴다. 같은 id의 사전은 모르는 필드를 지킨다.
/// 성공하면 1, 오류(틀린 항목 등)면 0(`cssgsg_config_error`).
///
/// # Safety
/// `path`와 `json`은 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_userdict_save(path: *const c_char, json: *const c_char) -> u8 {
    // SAFETY: 위 약속대로.
    let (path, json) = unsafe { (arg(path).unwrap_or_default(), arg(json).unwrap_or_default()) };
    let result = catch_unwind(|| {
        let storage: userdict::Storage = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        userdict::save(&path, storage.dictionaries)
    })
    .unwrap_or_else(|_| Err("사전을 저장하다 멈췄습니다(패닉)".into()));
    match result {
        Ok(()) => 1,
        Err(err) => {
            ERR.with(|e| *e.borrow_mut() = cstring(&err));
            0
        }
    }
}

/// 맥 키코드의 설정 파일 키 이름(`enter`, `a`, `f13` …). 수식키나 모르는 키면 NULL. 단축키 녹화에 쓴다.
#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_config_key_name(mac_keycode: u16) -> *const c_char {
    let key = cssgsg_core::Key::from_mac_keycode(mac_keycode);
    match cssgsg_core::shortcut::key_name(key) {
        Some(name) => give(Ok(Ok(name.to_string()))),
        None => ptr::null(),
    }
}

/// 윈도우 설정 앱: 설정 파일 내용(TOML)을 JSON으로. NULL이면 윈도우 기본 설정. 오류면 NULL(`cssgsg_config_error`).
///
/// # Safety
/// `toml`은 NULL이거나 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_config_json_windows(toml: *const c_char) -> *const c_char {
    // SAFETY: 위 약속대로.
    let toml = unsafe { arg(toml) };
    give(catch_unwind(|| json_from_toml_windows(toml.as_deref())))
}

/// 윈도우 설정 앱: JSON을 설정 파일 내용으로. 오류면 NULL(`cssgsg_config_error`).
///
/// # Safety
/// `json`은 NUL로 끝나는 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cssgsg_config_toml_windows(json: *const c_char) -> *const c_char {
    // SAFETY: 위 약속대로.
    let json = unsafe { arg(json) }.unwrap_or_default();
    give(catch_unwind(|| toml_from_json_windows(&json)))
}

/// 윈도우 스캔 코드(`extended`는 확장 키 비트, 0 또는 1)의 설정 파일 키 이름(`enter`, `a`, `f13` …).
/// 수식키면 `mod:` 뒤에 수식키 이름(`mod:shift_left`), 모르는 키면 NULL. 단축키 녹화에 쓴다.
#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_config_key_name_windows(scan: u16, extended: u8) -> *const c_char {
    match windows_key_name(scan, extended != 0) {
        Ok(Some(name)) => give(Ok(Ok(name.to_string()))),
        Ok(None) => ptr::null(),
        Err(modifier) => give(Ok(Ok(format!("mod:{modifier}")))),
    }
}

/// 이 스레드의 마지막 오류(없으면 빈 문자열).
#[unsafe(no_mangle)]
pub extern "C" fn cssgsg_config_error() -> *const c_char {
    ERR.with(|e| e.borrow().as_ptr())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_and_toml_round_trip() {
        let json = json_from_toml(None).unwrap();
        assert!(json.contains(r#""ko_layout":"chamshin-v18""#), "{json}");
        assert!(json.contains(r#""toggle_english":"tap:shift_right""#), "{json}");
        assert!(json.contains(r#""hanja":"alt_left+enter""#), "{json}");
        assert!(json.contains(r#""candidate_font_size":14"#) && !json.contains("delay"), "{json}");
        assert!(!json.contains("taps"), "옛 탭 표는 JSON에 내보내지 않는다");
        // 기본 설정 파일은 모두 주석이고, 다시 읽으면 같은 JSON이다.
        let text = toml_from_json(&json).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), Config::default());

        // 맥 표에는 모드 표시(hud)가 없다(0.7.5에서 뺐다).
        assert!(json.contains(r#""mac":{"candidate_font_size":14"#), "{json}");
        let changed = json
            .replace(r#""mac":{"candidate_font_size":14"#, r#""mac":{"candidate_font_size":20"#)
            .replace("chamshin-v18", "chamshin-d-v19");
        let text = toml_from_json(&changed).unwrap();
        assert!(
            text.contains("\ncandidate_font_size = 20\n") && text.contains("\nko_layout = \"chamshin-d-v19\"\n"),
            "{text}"
        );
        let back: serde_json::Value = serde_json::from_str(&json_from_toml(Some(&text)).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<serde_json::Value>(&changed).unwrap());
    }

    #[test]
    fn key_names_for_the_recorder() {
        let name = |code| {
            let p = cssgsg_config_key_name(code);
            (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_owned())
        };
        assert_eq!(name(0x24).as_deref(), Some("enter"));
        assert_eq!(name(0x31).as_deref(), Some("space"));
        assert_eq!(name(0x00).as_deref(), Some("a"));
        assert_eq!(name(0x69).as_deref(), Some("f13"));
        assert_eq!(name(0x38), None, "수식키");
    }

    #[test]
    fn windows_reads_and_writes_with_its_own_defaults() {
        let json = json_from_toml_windows(None).unwrap();
        assert!(json.contains(r#""hanja":"tap:control_right""#), "{json}");
        assert!(
            json.contains(r#""windows":{"hud":true,"hud_position":"caret","candidate_font_size":15}"#),
            "{json}"
        );
        // 기본 설정은 모두 주석, 맥 표 없음.
        let text = toml_from_json_windows(&json).unwrap();
        assert_eq!(Config::from_toml_windows(&text).unwrap(), Config::windows_default());
        assert!(text.contains("# hanja = \"tap:control_right\"") && !text.contains("[mac]"), "{text}");
        let changed = json.replace(r#""candidate_font_size":15"#, r#""candidate_font_size":20"#);
        let text = toml_from_json_windows(&changed).unwrap();
        assert!(text.contains("\n[windows]\n") && text.contains("\ncandidate_font_size = 20\n"), "{text}");
        let back: serde_json::Value =
            serde_json::from_str(&json_from_toml_windows(Some(&text)).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<serde_json::Value>(&changed).unwrap());
        // 맥 기본값 파일(한자 단축키를 적지 않았다)을 윈도우는 윈도우 기본값으로 읽는다.
        let mac_file = toml_from_json(&json_from_toml(None).unwrap()).unwrap();
        assert!(json_from_toml_windows(Some(&mac_file)).unwrap().contains(r#""hanja":"tap:control_right""#));
    }

    #[test]
    fn windows_scancodes_for_the_recorder() {
        let name = |scan, ext| {
            let p = cssgsg_config_key_name_windows(scan, ext);
            (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_owned())
        };
        assert_eq!(name(0x1C, 0).as_deref(), Some("enter"));
        assert_eq!(name(0x1C, 1).as_deref(), Some("keypad_enter"));
        assert_eq!(name(0x39, 0).as_deref(), Some("space"));
        assert_eq!(name(0x1E, 0).as_deref(), Some("a"));
        assert_eq!(name(0x4B, 1).as_deref(), Some("left"));
        assert_eq!(name(0x2A, 0).as_deref(), Some("mod:shift_left"));
        assert_eq!(name(0x36, 0).as_deref(), Some("mod:shift_right"));
        assert_eq!(name(0x1D, 1).as_deref(), Some("mod:control_right"));
        assert_eq!(name(0x38, 0).as_deref(), Some("mod:alt_left"));
        assert_eq!(name(0x5B, 1).as_deref(), Some("mod:meta_left"));
    }

    #[test]
    fn errors_are_reported() {
        assert!(json_from_toml(Some("ko_layout = 3")).is_err());
        assert!(json_from_toml(Some("ko_layout = \"qwerty\"")).unwrap_err().contains("qwerty"));
        assert!(toml_from_json("{").is_err());
        assert!(
            toml_from_json(r#"{"mac":{"candidate_font_size":40}}"#)
                .unwrap_err()
                .contains("candidate_font_size")
        );
        assert!(toml_from_json(r#"{"shortcuts":{"hanja":"meta_left+enter"}}"#).unwrap_err().contains("⌘"));
        assert!(toml_from_json(r#"{"surprise":1}"#).is_err(), "모르는 키");
        // 적지 않은 것은 기본값
        assert_eq!(toml_from_json("{}").unwrap(), Config::default().to_toml());
    }

    #[test]
    fn c_abi() {
        unsafe {
            let p = cssgsg_config_json(ptr::null());
            let json = CStr::from_ptr(p).to_str().unwrap().to_owned();
            let cjson = CString::new(json).unwrap();
            let p = cssgsg_config_toml(cjson.as_ptr());
            assert!(CStr::from_ptr(p).to_str().unwrap().contains("# ko_layout = \"chamshin-v18\""));
            let bad = CString::new("tap_threshold_ms = 1").unwrap();
            assert!(cssgsg_config_json(bad.as_ptr()).is_null());
            assert!(CStr::from_ptr(cssgsg_config_error()).to_str().unwrap().contains("tap_threshold_ms"));
        }
    }
}
