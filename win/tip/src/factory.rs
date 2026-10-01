//! COM 클래스 팩토리: TSF가 스레드마다 텍스트 서비스를 만들 때 쓴다.

use std::ffi::c_void;

use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_POINTER, E_UNEXPECTED};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::core::{BOOL, GUID, IUnknown, Interface, Ref, Result, implement};

use crate::guard::guarded;
use crate::service::TextService;

#[implement(IClassFactory)]
pub struct ClassFactory;

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        guarded(
            || Err(E_UNEXPECTED.into()),
            || unsafe {
                if ppv.is_null() || riid.is_null() {
                    return Err(E_POINTER.into());
                }
                *ppv = std::ptr::null_mut();
                if outer.is_some() {
                    return Err(CLASS_E_NOAGGREGATION.into());
                }
                let service: IUnknown = TextService::new().into();
                service.query(riid, ppv).ok()
            },
        )
    }

    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}
