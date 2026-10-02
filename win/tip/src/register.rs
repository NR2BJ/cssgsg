//! 등록·해제(regsvr32, 관리자). 사용자 입력 목록에 넣고 빼는 것(InstallLayoutOrTip)은 사용자 권한에서 따로 한다
//! (tools/win/tip-dev.ps1).
//!
//! 등록하는 것: CLSID의 InprocServer32(DLL 경로, Apartment), en-US 입력 프로필, TSF 카테고리.
//! DLL은 앱 컨테이너 앱(새 메모장, 시작 메뉴 검색)도 읽을 수 있는 곳(Program Files)에 두어야 한다.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, MAX_PATH};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_COMLESS, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr,
    ITfInputProcessorProfileMgr,
};
use windows::core::{Error, GUID, HRESULT, Result};

use crate::{CLSID_TEXT_SERVICE, GUID_PROFILE, LANGID_EN_US, braced, module};

/// 입력 목록에 보이는 이름.
const DESCRIPTION: &str = "cssgsg";

/// - TIP_KEYBOARD: 키보드 입력기.
/// - IMMERSIVESUPPORT: 스토어 앱·시작 메뉴 검색에서도 쓴다.
/// - SYSTRAYSUPPORT: 작업 표시줄 입력 표시기에 모드 아이콘(M3b).
/// - COMLESS: 입력기 안에서 CoCreateInstance를 쓰지 않는다(TF_Create* 사용). 일부 앱(Minecraft)은 이게 있어야 올린다.
/// - DISPLAYATTRIBUTEPROVIDER: 조합 밑줄 모양을 우리가 준다(앱이 텍스트 서비스에 표시 속성을 묻는다).
/// - UIELEMENTENABLED: UILess 모드를 받는다. 후보창을 직접 그리는 게임(오버워치 등)은 TSF를 TF_TMAE_UIELEMENTENABLEDONLY로
///   켜서 이 카테고리의 입력기만 불러들인다. 없으면 게임 안에서 입력기가 아예 안 떠 쿼티만 나온다(2026-10-02, 오버워치).
const CATEGORIES: [GUID; 6] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
];

fn clsid_key() -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{}", braced(&CLSID_TEXT_SERVICE))
}

/// TSF가 프로필·카테고리를 적는 곳. 해제 API는 값만 지우고 빈 키 뼈대를 남겨서 마지막에 통째로 지운다.
fn tsf_key() -> String {
    format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{}", braced(&CLSID_TEXT_SERVICE))
}

fn module_path() -> Result<String> {
    let mut buf = vec![0u16; MAX_PATH as usize];
    loop {
        let n = unsafe { GetModuleFileNameW(Some(module()), &mut buf) } as usize;
        if n == 0 {
            return Err(Error::from_thread());
        }
        if n < buf.len() {
            return Ok(String::from_utf16_lossy(&buf[..n]));
        }
        buf.resize(buf.len() * 2, 0);
    }
}

/// COM을 이 스레드에서 쓸 수 있게 한다(regsvr32는 이미 초기화했다). 이미 다른 모드면 그대로 쓴다.
struct Com(bool);

impl Com {
    fn init() -> Self {
        Com(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn register() -> Result<()> {
    let path = module_path()?;
    let key = windows_registry::LOCAL_MACHINE.create(clsid_key())?;
    key.set_string("", DESCRIPTION)?;
    let inproc = key.create("InprocServer32")?;
    inproc.set_string("", &path)?;
    inproc.set_string("ThreadingModel", "Apartment")?;

    let _com = Com::init();
    let description: Vec<u16> = DESCRIPTION.encode_utf16().collect();
    unsafe {
        let profiles: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        // 아이콘은 아직 없다(M3b에서 DLL 리소스로). 기본 사용 켬: 사용자 목록에 넣으면 바로 고를 수 있다.
        profiles.RegisterProfile(
            &CLSID_TEXT_SERVICE,
            LANGID_EN_US,
            &GUID_PROFILE,
            &description,
            &[],
            0,
            HKL::default(),
            0,
            true,
            0,
        )?;
        let categories: ITfCategoryMgr = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for category in &CATEGORIES {
            categories.RegisterCategory(&CLSID_TEXT_SERVICE, category, &CLSID_TEXT_SERVICE)?;
        }
    }
    Ok(())
}

/// 할 수 있는 것은 모두 하고 첫 오류를 돌려준다.
pub fn unregister() -> Result<()> {
    let mut first: Result<()> = Ok(());
    let mut keep = |r: Result<()>| {
        if first.is_ok() {
            first = r;
        }
    };
    let _com = Com::init();
    unsafe {
        match CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) {
            Ok(categories) => {
                for category in &CATEGORIES {
                    keep(categories.UnregisterCategory(&CLSID_TEXT_SERVICE, category, &CLSID_TEXT_SERVICE));
                }
            }
            Err(e) => keep(Err(e)),
        }
        match CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(profiles) => {
                keep(profiles.UnregisterProfile(&CLSID_TEXT_SERVICE, LANGID_EN_US, &GUID_PROFILE, 0))
            }
            Err(e) => keep(Err(e)),
        }
    }
    for key in [tsf_key(), clsid_key()] {
        match windows_registry::LOCAL_MACHINE.remove_tree(key) {
            Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {}
            r => keep(r),
        }
    }
    first
}
