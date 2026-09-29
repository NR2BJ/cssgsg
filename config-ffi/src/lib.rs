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

fn give(result: std::thread::Result<Result<String, String>>) -> *const c_char {
    match result.unwrap_or_else(|_| Err("설정 변환 중 패닉".into())) {
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
    .unwrap_or_else(|_| Err("사전 저장 중 패닉".into()));
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
        assert!(json.contains(r#""newline_delay_offset_ms":0"#), "{json}");
        assert!(!json.contains("taps"), "옛 탭 표는 JSON에 내보내지 않는다");
        // 기본 설정 파일은 모두 주석이고, 다시 읽으면 같은 JSON이다.
        let text = toml_from_json(&json).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), Config::default());

        let changed =
            json.replace(r#""hud":true"#, r#""hud":false"#).replace("chamshin-v18", "chamshin-d-v19");
        let text = toml_from_json(&changed).unwrap();
        assert!(
            text.contains("\nhud = false\n") && text.contains("\nko_layout = \"chamshin-d-v19\"\n"),
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
    fn errors_are_reported() {
        assert!(json_from_toml(Some("ko_layout = 3")).is_err());
        assert!(json_from_toml(Some("ko_layout = \"qwerty\"")).unwrap_err().contains("qwerty"));
        assert!(toml_from_json("{").is_err());
        assert!(
            toml_from_json(r#"{"mac":{"newline_delay_offset_ms":80}}"#)
                .unwrap_err()
                .contains("newline_delay_offset_ms")
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
