//! Mozc 변환기(일본어 한자). `mozc` 기능을 켜면 쓴다.
//!
//! Mozc는 입력기 프로세스 안에서 돈다(mozc_server 없음). 엔진은 libcssgsg_mozc.dylib(mozc/cssgsg C API)이고
//! 실행 중에 읽는다(dlopen). 그래서 입력기를 새로 내지 않아도 더 새 Mozc로 바꿀 수 있다(NRIME 1.0.12와 같다).
//! 어느 엔진(앱에 든 것, 입력기가 받은 더 새것)을 읽을지는 셸이 정하고(mac/cssgsg/Mozc), 여기서는 받은 경로를 읽는다.
//!
//! 변환할 때마다 코어가 가진 가나 읽기를 통째로 넘기고, 결과 화면(문절·후보)을 JSON으로 받는다.
//! 엔진 준비 10~20ms, 변환 1ms 안쪽(2026-09-29 실측).
//! dlopen은 macOS·리눅스의 libSystem/libc에 있어서 따로 의존성 없이 직접 선언한다.

use std::ffi::{CStr, CString, c_char, c_int, c_void};

use serde::Deserialize;

use crate::convert::{ConvCmd, ConvView, Converter};

/// 이 코어가 아는 C API 판(mozc/cssgsg/cssgsg_mozc.h의 CSSGSG_MOZC_ABI_VERSION). 판이 다른 엔진은 읽지 않는다.
pub const SUPPORTED_ABI: i32 = 1;

#[repr(C)]
struct RawMozc {
    _private: [u8; 0],
}

const RTLD_NOW: c_int = 0x2;
#[cfg(target_os = "macos")]
const RTLD_LOCAL: c_int = 0x4;
#[cfg(not(target_os = "macos"))]
const RTLD_LOCAL: c_int = 0;

unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
}

/// 엔진의 C API(cssgsg_mozc.h). 읽은 엔진은 내리지 않는다: C++ 코드를 내리는 것은 안전하지 않다.
/// 실패한 엔진도 올라온 채로 두고 부르지 않을 뿐이다. 그래서 함수 표는 프로그램이 끝날 때까지 산다(&'static).
struct Api {
    new: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut RawMozc,
    free: unsafe extern "C" fn(*mut RawMozc),
    start: unsafe extern "C" fn(*mut RawMozc, *const c_char) -> *const c_char,
    command: unsafe extern "C" fn(*mut RawMozc, i32, i32) -> *const c_char,
    commit: unsafe extern "C" fn(*mut RawMozc) -> *const c_char,
    cancel: unsafe extern "C" fn(*mut RawMozc),
    set_learning: unsafe extern "C" fn(*mut RawMozc, i32),
    reload: unsafe extern "C" fn(*mut RawMozc),
    version: unsafe extern "C" fn() -> *const c_char,
}

fn last_dl_error() -> String {
    // SAFETY: dlerror는 NULL이거나 NUL로 끝나는 문자열을 돌려준다. 곧바로 복사한다.
    let e = unsafe { dlerror() };
    if e.is_null() { "?".into() } else { unsafe { CStr::from_ptr(e) }.to_string_lossy().into_owned() }
}

/// 읽은 라이브러리에서 C 함수 `name`을 찾는다.
///
/// # Safety
/// `handle`은 dlopen이 돌려준 핸들, `T`는 그 함수의 모양(`unsafe extern "C" fn`)이어야 한다.
unsafe fn function<T: Copy>(handle: *mut c_void, name: &str) -> Result<T, String> {
    assert_eq!(size_of::<T>(), size_of::<*mut c_void>());
    let c = CString::new(name).map_err(|_| "이름에 NUL이 있다".to_string())?;
    // SAFETY: 살아 있는 핸들과 NUL로 끝나는 이름.
    let p = unsafe { dlsym(handle, c.as_ptr()) };
    if p.is_null() {
        return Err(format!("C API에 {name}이 없다"));
    }
    // SAFETY: 함수 포인터와 크기가 같고, 모양은 호출한 쪽이 약속한다.
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
}

impl Api {
    /// 엔진 라이브러리를 읽고 C API를 찾는다. 판이 다르면 거절한다.
    fn load(library: &str) -> Result<&'static Api, String> {
        let path = CString::new(library).map_err(|_| "엔진 경로에 NUL이 있다".to_string())?;
        // SAFETY: NUL로 끝나는 경로. 라이브러리의 초기화 코드가 돈다(우리가 빌드한 Mozc).
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        if handle.is_null() {
            return Err(format!("dlopen: {}", last_dl_error()));
        }
        // SAFETY: 판을 알려 주는 함수의 모양은 판이 바뀌어도 그대로다.
        let abi: unsafe extern "C" fn() -> i32 = unsafe { function(handle, "cssgsg_mozc_abi_version")? };
        // SAFETY: 인자 없는 함수.
        let version = unsafe { abi() };
        if version != SUPPORTED_ABI {
            return Err(format!("C API 판이 {version}이다(아는 판은 {SUPPORTED_ABI})"));
        }
        // SAFETY: 판이 같으니 함수 모양은 cssgsg_mozc.h의 선언(= 아래 필드의 모양)과 같다.
        let api = unsafe {
            Api {
                new: function(handle, "cssgsg_mozc_new")?,
                free: function(handle, "cssgsg_mozc_free")?,
                start: function(handle, "cssgsg_mozc_start")?,
                command: function(handle, "cssgsg_mozc_command")?,
                commit: function(handle, "cssgsg_mozc_commit")?,
                cancel: function(handle, "cssgsg_mozc_cancel")?,
                set_learning: function(handle, "cssgsg_mozc_set_learning")?,
                reload: function(handle, "cssgsg_mozc_reload")?,
                version: function(handle, "cssgsg_mozc_version")?,
            }
        };
        Ok(Box::leak(Box::new(api)))
    }
}

/// C API가 돌려주는 화면(mozc/cssgsg/cssgsg_mozc.h).
#[derive(Deserialize)]
struct View {
    segments: Vec<String>,
    focused: usize,
    candidates: Vec<String>,
    selected: Option<usize>,
}

pub struct MozcConverter {
    api: &'static Api,
    raw: *mut RawMozc,
}

impl MozcConverter {
    /// 엔진을 읽고 만든다. `library`: libcssgsg_mozc.dylib, `data`: 같이 빌드한 mozc.data,
    /// `profile_dir`: 학습·사용자 사전 폴더(Mozc 프로세스 전체에 하나다. 마지막으로 만든 것이 쓴다).
    /// 만든 뒤 낱말 하나를 변환해 본다. 엔진이 읽히기만 하고 변환하지 못하면(데이터가 맞지 않는 등) 실패다.
    /// 실패하면 까닭을 돌려준다(셸이 로그에 남기고, 받은 엔진이면 다시 쓰지 않는다).
    pub fn load(library: &str, data: &str, profile_dir: &str) -> Result<Self, String> {
        let api = Api::load(library)?;
        let data = CString::new(data).map_err(|_| "데이터 경로에 NUL이 있다".to_string())?;
        let profile = CString::new(profile_dir).map_err(|_| "학습 폴더 경로에 NUL이 있다".to_string())?;
        // SAFETY: NUL로 끝나는 문자열 두 개.
        let raw = unsafe { (api.new)(data.as_ptr(), profile.as_ptr()) };
        if raw.is_null() {
            return Err("엔진을 만들지 못했다(mozc.data 확인)".into());
        }
        let mut converter = Self { api, raw };
        let probe = converter.start("にほんご");
        converter.cancel();
        match probe {
            Some(v) if !v.candidates.is_empty() => Ok(converter),
            _ => Err("엔진이 변환하지 못했다".into()),
        }
    }

    /// 읽은 엔진의 Mozc 버전(예: "3.34.6239.101").
    pub fn version(&self) -> String {
        // SAFETY: 엔진이 가진 NUL로 끝나는 문자열. 곧바로 복사한다.
        let v = unsafe { (self.api.version)() };
        if v.is_null() { String::new() } else { unsafe { CStr::from_ptr(v) }.to_string_lossy().into_owned() }
    }

    /// 학습(확정한 후보를 다음에 먼저 내기)을 켜고 끈다. 기본은 켬. 끄면 결과가 늘 같다.
    pub fn set_learning(&mut self, enabled: bool) {
        // SAFETY: 살아 있는 인스턴스.
        unsafe { (self.api.set_learning)(self.raw, enabled as i32) }
    }

    fn view(json: *const c_char) -> Option<ConvView> {
        if json.is_null() {
            return None;
        }
        // SAFETY: C API가 돌려준 문자열은 다음 호출 전까지 유효하고 NUL로 끝난다. 곧바로 복사한다.
        let text = unsafe { CStr::from_ptr(json) }.to_string_lossy();
        let v: View = serde_json::from_str(&text).ok()?;
        Some(ConvView {
            segments: v.segments,
            focused: v.focused,
            candidates: v.candidates,
            selected: v.selected,
        })
    }
}

impl Converter for MozcConverter {
    fn start(&mut self, reading: &str) -> Option<ConvView> {
        let reading = CString::new(reading).ok()?;
        // SAFETY: 살아 있는 인스턴스와 NUL로 끝나는 문자열.
        Self::view(unsafe { (self.api.start)(self.raw, reading.as_ptr()) })
    }

    fn command(&mut self, cmd: ConvCmd) -> Option<ConvView> {
        let (code, arg) = match cmd {
            ConvCmd::Next => (0, 0),
            ConvCmd::Prev => (1, 0),
            ConvCmd::FocusLeft => (2, 0),
            ConvCmd::FocusRight => (3, 0),
            ConvCmd::Shrink => (4, 0),
            ConvCmd::Expand => (5, 0),
            ConvCmd::Select(n) => (8, i32::try_from(n).ok()?),
        };
        // SAFETY: 살아 있는 인스턴스.
        Self::view(unsafe { (self.api.command)(self.raw, code, arg) })
    }

    fn commit(&mut self) -> String {
        // SAFETY: 살아 있는 인스턴스. 돌려준 문자열은 바로 복사한다.
        let text = unsafe { (self.api.commit)(self.raw) };
        if text.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    }

    fn cancel(&mut self) {
        // SAFETY: 살아 있는 인스턴스.
        unsafe { (self.api.cancel)(self.raw) }
    }

    fn reload(&mut self) {
        // SAFETY: 살아 있는 인스턴스.
        unsafe { (self.api.reload)(self.raw) }
    }
}

impl Drop for MozcConverter {
    fn drop(&mut self) {
        // SAFETY: load가 만든 포인터를 한 번만 해제한다. 라이브러리는 그대로 둔다.
        unsafe { (self.api.free)(self.raw) }
    }
}
