//! 이 로그인 세션의 입력기를 바꾼다(Win+Space로 고르는 것과 같다). 시험 스크립트가 앱을 띄우기 전에 쓴다.
//!
//!   cargo run -p cssgsg-tip --example use_tip            cssgsg(한국어 프로필)
//!   cargo run -p cssgsg-tip --example use_tip -- ms      MS 한국어 입력기
//!   ... -- mode=ko                                        cssgsg 모드도 맞춘다(앱 사이 전역 칸, ko·en·ja)

fn main() {
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let ms = args.iter().any(|a| a == "ms");
        let mode = args.iter().find_map(|a| a.strip_prefix("mode=")).map(str::to_owned);
        if let Err(e) = imp::run(ms, mode.as_deref()) {
            eprintln!("{e:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
mod imp {
    use cssgsg_tip::{CLSID_TEXT_SERVICE, GUID_COMPARTMENT_MODE, GUID_PROFILE, LANGID_KO_KR};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr, ITfInputProcessorProfileMgr, ITfThreadMgr,
        TF_IPPMF_FORSESSION, TF_PROFILETYPE_INPUTPROCESSOR,
    };
    use windows::core::{GUID, Result};

    /// MS 한국어 입력기(examples/imm_spy와 같다).
    const CLSID_MS_KOREAN: GUID = GUID::from_u128(0xA028AE76_01B1_46C2_99C4_ACD9858AE02F);
    const PROFILE_MS_KOREAN: GUID = GUID::from_u128(0xB5FE1F02_D5F2_4445_9C03_C568F23C99A1);

    pub fn run(ms: bool, mode: Option<&str>) -> Result<()> {
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
            // 전역 칸 값 = 모드 | 직전 비영어 모드 << 4 (win/tip/src/service.rs mode_value, En 0, Ko 1, Ja 2). 영어는 직전 한국어.
            if let Some(mode) = mode {
                let value = match mode {
                    "en" => 1 << 4,
                    "ja" => 2 | 2 << 4,
                    _ => 1 | 1 << 4,
                };
                let threads: ITfThreadMgr =
                    CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
                let client = threads.Activate()?;
                let slot = threads.GetGlobalCompartment()?.GetCompartment(&GUID_COMPARTMENT_MODE)?;
                slot.SetValue(client, &VARIANT::from(value))?;
                threads.Deactivate()?;
                println!("cssgsg mode {mode}");
            }
        }
        Ok(())
    }
}
