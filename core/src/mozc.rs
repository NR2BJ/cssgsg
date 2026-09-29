//! Mozc 변환기(일본어 한자). `mozc` 기능을 켜면 mozc/cssgsg C API(정적 라이브러리)를 쓴다.
//!
//! Mozc는 입력기 프로세스 안에서 돈다(mozc_server 없음). 변환할 때마다 코어가 가진 가나 읽기를 통째로 넘기고,
//! 결과 화면(문절·후보)을 JSON으로 받는다. 엔진 준비 10~20ms, 변환 1ms 안쪽(2026-09-29 실측).

use std::ffi::{CStr, CString, c_char};

use serde::Deserialize;

use crate::convert::{ConvCmd, ConvView, Converter};

#[repr(C)]
struct RawMozc {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn cssgsg_mozc_new(data_path: *const c_char, profile_dir: *const c_char) -> *mut RawMozc;
    fn cssgsg_mozc_free(m: *mut RawMozc);
    fn cssgsg_mozc_start(m: *mut RawMozc, reading: *const c_char) -> *const c_char;
    fn cssgsg_mozc_command(m: *mut RawMozc, command: i32, arg: i32) -> *const c_char;
    fn cssgsg_mozc_commit(m: *mut RawMozc) -> *const c_char;
    fn cssgsg_mozc_cancel(m: *mut RawMozc);
    fn cssgsg_mozc_set_learning(m: *mut RawMozc, enabled: i32);
    fn cssgsg_mozc_reload(m: *mut RawMozc);
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
    raw: *mut RawMozc,
}

impl MozcConverter {
    /// `data_path`: mozc.data, `profile_dir`: 학습·사용자 사전 폴더. 실패하면 `None`.
    /// 학습 폴더 설정은 Mozc 프로세스 전체에 하나다(마지막으로 만든 것이 쓴다).
    pub fn new(data_path: &str, profile_dir: &str) -> Option<Self> {
        let data = CString::new(data_path).ok()?;
        let profile = CString::new(profile_dir).ok()?;
        // SAFETY: NUL로 끝나는 문자열 두 개를 넘긴다.
        let raw = unsafe { cssgsg_mozc_new(data.as_ptr(), profile.as_ptr()) };
        (!raw.is_null()).then_some(Self { raw })
    }

    /// 학습(확정한 후보를 다음에 먼저 내기)을 켜고 끈다. 기본은 켬. 끄면 결과가 늘 같다.
    pub fn set_learning(&mut self, enabled: bool) {
        // SAFETY: 살아 있는 인스턴스.
        unsafe { cssgsg_mozc_set_learning(self.raw, enabled as i32) }
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
        Self::view(unsafe { cssgsg_mozc_start(self.raw, reading.as_ptr()) })
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
        Self::view(unsafe { cssgsg_mozc_command(self.raw, code, arg) })
    }

    fn commit(&mut self) -> String {
        // SAFETY: 살아 있는 인스턴스. 돌려준 문자열은 바로 복사한다.
        let text = unsafe { cssgsg_mozc_commit(self.raw) };
        if text.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    }

    fn cancel(&mut self) {
        // SAFETY: 살아 있는 인스턴스.
        unsafe { cssgsg_mozc_cancel(self.raw) }
    }

    fn reload(&mut self) {
        // SAFETY: 살아 있는 인스턴스.
        unsafe { cssgsg_mozc_reload(self.raw) }
    }
}

impl Drop for MozcConverter {
    fn drop(&mut self) {
        // SAFETY: new가 만든 포인터를 한 번만 해제한다.
        unsafe { cssgsg_mozc_free(self.raw) }
    }
}
