//! 이 로그인 세션의 입력기를 바꾼다(Win+Space로 고르는 것과 같다). 시험 스크립트가 옛 메모장 같은 앱을 띄우기 전에 쓴다.
//!
//!   cargo run -p cssgsg-tip --example use_tip            cssgsg(한국어 프로필)
//!   cargo run -p cssgsg-tip --example use_tip -- ms      MS 한국어 입력기

fn main() {
    #[cfg(windows)]
    {
        let ms = std::env::args().nth(1).is_some_and(|a| a == "ms");
        if let Err(e) = imp::run(ms) {
            eprintln!("{e:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
mod imp {
    use cssgsg_tip::{CLSID_TEXT_SERVICE, GUID_PROFILE, LANGID_KO_KR};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, ITfInputProcessorProfileMgr, TF_IPPMF_FORSESSION,
        TF_PROFILETYPE_INPUTPROCESSOR,
    };
    use windows::core::{GUID, Result};

    /// MS 한국어 입력기(examples/imm_spy와 같다).
    const CLSID_MS_KOREAN: GUID = GUID::from_u128(0xA028AE76_01B1_46C2_99C4_ACD9858AE02F);
    const PROFILE_MS_KOREAN: GUID = GUID::from_u128(0xB5FE1F02_D5F2_4445_9C03_C568F23C99A1);

    pub fn run(ms: bool) -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            let (clsid, profile) =
                if ms { (CLSID_MS_KOREAN, PROFILE_MS_KOREAN) } else { (CLSID_TEXT_SERVICE, GUID_PROFILE) };
            profiles.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                LANGID_KO_KR,
                &clsid,
                &profile,
                HKL::default(),
                TF_IPPMF_FORSESSION,
            )?;
            println!("activated {} for this session", if ms { "Microsoft Korean IME" } else { "cssgsg" });
        }
        Ok(())
    }
}
