//! cssgsg 윈도우 입력기: TSF 텍스트 서비스(TIP) DLL.
//!
//! M3a 뼈대다. COM 클래스 팩토리, ITfTextInputProcessorEx, 키 이벤트 싱크, 프로필 등록·해제까지 있고,
//! 글자 키는 Graphite 글자로 바꿔 바로 확정한다. 조합(밑줄), 세 모드, Shift 톡 전환은 M3b에서 코어 엔진으로 바꾼다.
//!
//! 남의 프로세스(탐색기, 시작 메뉴 검색, 게임) 안에서 도는 코드라서 지킨다.
//! - COM 진입점은 모두 [`guard::guarded`]로 감싼다. 패닉을 COM 경계 밖으로 내보내지 않고(밖으로 풀리면 그 프로세스가 죽는다),
//!   FP 환경(MXCSR)을 표준값으로 맞췄다가 되돌린다(예외를 켜 둔 Delphi 앱 등).
//! - DllMain은 모듈 핸들만 저장한다. 로더 잠금 안에서 다른 일을 하지 않는다.
//! - 키 판정은 상태를 바꾸지 않는다. OnTestKeyDown 없이 OnKeyDown이 바로 오는 앱이 있어서(윈도우 11 메모장, mozc #1415)
//!   두 곳에서 같은 판정을 다시 한다.
//! - 키를 뗄 때(key-up)와 수식키는 먹지 않는다. 짝이 안 맞는 뗌을 먹으면 앱에서 키가 눌린 채로 남는다.
#![cfg(windows)]

mod edit;
mod factory;
mod guard;
mod keys;
mod register;
mod service;

use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

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

static MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// 이 DLL의 모듈 핸들(DllMain이 저장한다).
pub(crate) fn module() -> HMODULE {
    HMODULE(MODULE.load(Ordering::Relaxed))
}

/// GUID를 레지스트리·InstallLayoutOrTip 형식("{XXXXXXXX-…}")으로.
pub fn braced(guid: &GUID) -> String {
    format!("{{{guid:?}}}")
}

/// 디버거 출력(DebugView 등)에 남긴다. 파일에는 쓰지 않는다(앱 컨테이너 프로세스는 쓸 곳이 없다).
pub(crate) fn debug_log(message: &str) {
    let wide: Vec<u16> = format!("[cssgsg] {message}\n").encode_utf16().chain(Some(0)).collect();
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr()))
    };
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
