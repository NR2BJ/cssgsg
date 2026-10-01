//! cssgsg 윈도우 입력기: TSF 텍스트 서비스(TIP) DLL.
//!
//! 키를 코어 엔진(맥과 같은 cssgsg-core)에 넣고, 엔진 출력대로 TSF 조합(밑줄)과 확정을 한다. 한·영·일 세 모드와
//! Shift 톡 전환은 엔진이 하고, 모드는 작업 표시줄 아이콘(G/ㅊ/月)으로 보이며 앱 사이에서 하나로 맞춘다([`service`]).
//!
//! 남의 프로세스(탐색기, 시작 메뉴 검색, 게임) 안에서 도는 코드라서 지킨다.
//! - COM 진입점은 모두 [`guard::guarded`]로 감싼다. 패닉을 COM 경계 밖으로 내보내지 않고(밖으로 풀리면 그 프로세스가 죽는다),
//!   FP 환경(MXCSR)을 표준값으로 맞췄다가 되돌린다(예외를 켜 둔 Delphi 앱 등).
//! - DllMain은 모듈 핸들만 저장한다. 로더 잠금 안에서 다른 일을 하지 않는다.
//! - 키 판정은 상태를 바꾸지 않는다. OnTestKeyDown 없이 OnKeyDown이 바로 오는 앱이 있어서(윈도우 11 메모장, mozc #1415)
//!   두 곳에서 같은 판정을 다시 한다.
//! - 키를 뗄 때(key-up)와 수식키는 먹지 않는다. 짝이 안 맞는 뗌을 먹으면 앱에서 키가 눌린 채로 남는다.
#![cfg(windows)]

mod display;
mod edit;
mod factory;
mod guard;
mod host;
mod keys;
mod langbar;
mod plan;
mod register;
mod service;
mod ui;

use std::ffi::c_void;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, E_POINTER, E_UNEXPECTED, HINSTANCE, HMODULE, S_FALSE, S_OK,
};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface};

/// 텍스트 서비스 COM 클래스. 한 번 정하면 바꾸지 않는다(사용자 입력 목록·레지스트리가 이 값을 기억한다).
pub const CLSID_TEXT_SERVICE: GUID = GUID::from_u128(0xA9227DC2_56BC_4023_AE29_82A9AAA4EE14);
/// 입력 프로필. en-US 하나뿐이고 한·영·일은 입력기 안에서 바꾼다(CONCEPT §10.2).
pub const GUID_PROFILE: GUID = GUID::from_u128(0xDCFBD969_D52F_4AFB_ABC0_271CC58FE18C);
/// 프로필 언어: en-US.
pub const LANGID_EN_US: u16 = 0x0409;
/// 조합 표시 속성: 입력 중(가는 밑줄).
pub const GUID_DISPLAY_ATTRIBUTE_INPUT: GUID = GUID::from_u128(0x5D2BD476_8831_4D21_9207_BE1AE451EC27);
/// 조합 표시 속성: 변환 중 포커스된 문절(굵은 밑줄).
pub const GUID_DISPLAY_ATTRIBUTE_FOCUSED: GUID = GUID::from_u128(0xD9845973_3763_4314_8FDE_BDA44C91F643);
/// TSF 전역 칸: 앱 사이에서 같은 모드(값 = 모드 | 직전 비영어 모드 << 4).
pub const GUID_COMPARTMENT_MODE: GUID = GUID::from_u128(0x557E78D1_7585_4FA7_9F39_3767F2A4A2B7);

static MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// 이 DLL의 모듈 핸들(DllMain이 저장한다).
pub(crate) fn module() -> HMODULE {
    HMODULE(MODULE.load(Ordering::Relaxed))
}

/// GUID를 레지스트리·InstallLayoutOrTip 형식("{XXXXXXXX-…}")으로.
pub fn braced(guid: &GUID) -> String {
    format!("{{{guid:?}}}")
}

/// 개발자 기록을 켰는지(마지막으로 [`read_debug_flag`]가 읽은 값).
static DEBUG: AtomicBool = AtomicBool::new(false);

/// 디버거 출력(DebugView 등)에 남기고, 개발자 기록을 켰으면 %LOCALAPPDATA%\cssgsg\tip-debug.log에도 쓴다
/// (앱 컨테이너 프로세스는 쓸 곳이 없어 건너뛴다). 친 글자는 남기지 않는다(키 코드·길이만).
pub(crate) fn debug_log(message: &str) {
    let wide: Vec<u16> = format!("[cssgsg] {message}\n").encode_utf16().chain(Some(0)).collect();
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr()))
    };
    // 시험은 개발자의 진짜 기록 파일에 쓰지 않는다.
    if cfg!(test) || !DEBUG.load(Ordering::Relaxed) {
        return;
    }
    static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
    let Ok(mut file) = FILE.lock() else { return };
    if file.is_none() {
        let Some(base) = std::env::var_os("LOCALAPPDATA") else { return };
        let dir = std::path::Path::new(&base).join("cssgsg");
        let _ = std::fs::create_dir_all(&dir);
        *file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("tip-debug.log")).ok();
    }
    if let Some(f) = file.as_mut() {
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let _ = writeln!(
            f,
            "{:02}:{:02}:{:02}.{:03} {}[{}] {message}",
            t.wHour,
            t.wMinute,
            t.wSecond,
            t.wMilliseconds,
            exe.unwrap_or_default(),
            std::process::id()
        );
    }
}

/// 개발자 기록 설정(HKCU\Software\cssgsg 의 DebugLog=1)을 읽는다. 입력기가 켜질 때마다 다시 읽어서, 켜고 끄면
/// 앱을 다시 띄우지 않아도 다음 입력칸부터 따른다. 켜면 debug_log가 파일에도 쓰고 텍스트 서비스가 키마다 한 줄씩 남긴다.
pub(crate) fn read_debug_flag() -> bool {
    let on = windows_registry::CURRENT_USER
        .open("Software\\cssgsg")
        .and_then(|k| k.get_u32("DebugLog"))
        .is_ok_and(|v| v != 0);
    DEBUG.store(on, Ordering::Relaxed);
    on
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(module: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        MODULE.store(module.0, Ordering::Relaxed);
    }
    BOOL::from(true)
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    guard::guarded(
        || E_UNEXPECTED,
        || unsafe {
            if rclsid.is_null() || riid.is_null() || ppv.is_null() {
                return E_POINTER;
            }
            *ppv = std::ptr::null_mut();
            if *rclsid != CLSID_TEXT_SERVICE {
                return CLASS_E_CLASSNOTAVAILABLE;
            }
            let factory: IUnknown = factory::ClassFactory.into();
            factory.query(riid, ppv)
        },
    )
}

/// 내리지 않는다: 앱이 쥔 객체·스레드가 남아 있어도 안전하게.
#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

/// regsvr32(관리자)가 부른다: CLSID, 입력 프로필, 카테고리를 등록한다. 사용자 입력 목록에 넣는 것은 따로 한다(InstallLayoutOrTip).
#[unsafe(no_mangle)]
extern "system" fn DllRegisterServer() -> HRESULT {
    guard::guarded(|| E_UNEXPECTED, || register::register().map_or_else(|e| e.code(), |()| S_OK))
}

#[unsafe(no_mangle)]
extern "system" fn DllUnregisterServer() -> HRESULT {
    guard::guarded(|| E_UNEXPECTED, || register::unregister().map_or_else(|e| e.code(), |()| S_OK))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_strings_are_braced_upper_case() {
        assert_eq!(braced(&CLSID_TEXT_SERVICE), "{A9227DC2-56BC-4023-AE29-82A9AAA4EE14}");
        assert_eq!(braced(&GUID_PROFILE), "{DCFBD969-D52F-4AFB-ABC0-271CC58FE18C}");
    }

    #[test]
    fn class_object_only_for_our_clsid() {
        let mut out = std::ptr::null_mut();
        let other = GUID::from_u128(1);
        let hr = DllGetClassObject(&other, &windows::Win32::System::Com::IClassFactory::IID, &mut out);
        assert_eq!(hr, CLASS_E_CLASSNOTAVAILABLE);
        assert!(out.is_null());
        let hr = DllGetClassObject(
            &CLSID_TEXT_SERVICE,
            &windows::Win32::System::Com::IClassFactory::IID,
            &mut out,
        );
        assert_eq!(hr, S_OK);
        assert!(!out.is_null());
        unsafe { drop(windows::Win32::System::Com::IClassFactory::from_raw(out)) };
    }
}
