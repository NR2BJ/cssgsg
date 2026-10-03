//! 게임처럼 TSF를 UILess 전용으로 켜고(TF_TMAE_UIELEMENTENABLEDONLY) cssgsg를 고른 뒤, 입력기 DLL이 이 프로세스에
//! 불려 들어왔는지 본다. 오버워치처럼 게임이 후보창을 직접 그리는 앱은 이렇게 켜서, GUID_TFCAT_TIPCAP_UIELEMENTENABLED로
//! 등록한 입력기만 불러들인다(2026-10-02: 0.2.2까지 cssgsg는 오버워치에서 쿼티만 나왔다, `tasklist /m`에도 없었다).
//!
//!   cargo run -p cssgsg-tip --example uiless_host                UILess 전용(게임처럼)
//!   cargo run -p cssgsg-tip --example uiless_host -- normal      보통 앱처럼(비교용)
//!   cargo run -p cssgsg-tip --example uiless_host -- [normal] tap  켠 뒤 오른쪽 Shift를 톡 친다(입력칸 없는 게임 화면에서
//!                                                                  모드가 안 바뀌는지, 개발자 기록의 key 줄 mode=로 본다)
//!
//! 설치된(등록된) 입력기를 쓴다. 개발자 기록을 켜 두면 입력기가 켜질 때 "activated (flags …)" 줄도 남는다.

fn main() {
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let has = |a: &str| args.iter().any(|x| x == a);
        if let Err(e) = imp::run(!has("normal"), has("tap")) {
            eprintln!("{e:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use cssgsg_tip::{CLSID_TEXT_SERVICE, GUID_PROFILE, LANGID_KO_KR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, SendInput, SetFocus,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr, GUID_TFCAT_TIP_KEYBOARD,
        ITfInputProcessorProfileMgr, ITfKeystrokeMgr, ITfThreadMgrEx, TF_INPUTPROCESSORPROFILE,
        TF_IPPMF_FORPROCESS, TF_PROFILETYPE_INPUTPROCESSOR, TF_TMAE_UIELEMENTENABLEDONLY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
        MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SetForegroundWindow, TranslateMessage, WINDOW_EX_STYLE,
        WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    use windows::core::{Interface, Result, w};

    extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// 메시지를 잠깐 돌린다. TSF를 직접 쓰는 앱(게임)처럼 키 메시지를 먼저 TSF에 보이고(시험 → 먹으면 실제), 먹지 않은 것만 창에 보낸다.
    unsafe fn pump(keys: &ITfKeystrokeMgr, ms: u64) {
        let start = Instant::now();
        let mut msg = MSG::default();
        while start.elapsed() < Duration::from_millis(ms) {
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let (w, l) = (msg.wParam, msg.lParam);
                    let eaten = match msg.message {
                        WM_KEYDOWN | WM_SYSKEYDOWN => {
                            keys.TestKeyDown(w, l).is_ok_and(|e| e.as_bool())
                                && keys.KeyDown(w, l).is_ok_and(|e| e.as_bool())
                        }
                        WM_KEYUP | WM_SYSKEYUP => {
                            keys.TestKeyUp(w, l).is_ok_and(|e| e.as_bool())
                                && keys.KeyUp(w, l).is_ok_and(|e| e.as_bool())
                        }
                        _ => false,
                    };
                    if !eaten {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 오른쪽 Shift 톡(스캔 코드 0x36).
    unsafe fn tap_right_shift(keys: &ITfKeystrokeMgr) {
        let key = |up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wScan: 0x36,
                    dwFlags: KEYEVENTF_SCANCODE | if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    ..Default::default()
                },
            },
        };
        unsafe {
            SendInput(&[key(false)], size_of::<INPUT>() as i32);
            pump(keys, 40);
            SendInput(&[key(true)], size_of::<INPUT>() as i32);
            pump(keys, 300);
        }
    }

    pub fn run(ui_less: bool, tap: bool) -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            // 게임처럼 창을 만들기 전에 TSF를 켠다(창이 먼저면 윈도우가 이 스레드의 TSF를 보통 방식으로 먼저 켠다).
            let threads: ITfThreadMgrEx = CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
            let mut client = 0;
            threads.ActivateEx(&mut client, if ui_less { TF_TMAE_UIELEMENTENABLEDONLY } else { 0 })?;
            let keys: ITfKeystrokeMgr = threads.cast()?;
            // 창 하나에 포커스를 두고 그 창에 문서를 이어 둔다(입력칸 없는 게임 화면: 문맥은 없다). TSF는 포커스 문서가 있어야
            // 입력기를 켠다.
            let instance = GetModuleHandleW(None)?;
            let class = w!("cssgsg-uiless-host");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            });
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!("cssgsg UI-less host"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                420,
                160,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
            let document = threads.CreateDocumentMgr()?;
            let _ = threads.AssociateFocus(hwnd, &document);
            threads.SetFocus(&document)?;
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            let chosen = profiles.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                LANGID_KO_KR,
                &CLSID_TEXT_SERVICE,
                &GUID_PROFILE,
                HKL::default(),
                TF_IPPMF_FORPROCESS,
            );
            pump(&keys, 500);
            if tap {
                println!("foreground is this window: {}", GetForegroundWindow() == hwnd);
                tap_right_shift(&keys);
            }
            let mut profile = TF_INPUTPROCESSORPROFILE::default();
            let active = profiles
                .GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile)
                .map(|_| profile.clsid == CLSID_TEXT_SERVICE);
            let loaded = GetModuleHandleW(w!("cssgsg_tip.dll")).is_ok();
            println!(
                "{}: ActivateProfile {:?}, cssgsg is the active profile {:?}, cssgsg_tip.dll loaded {}",
                if ui_less { "UI-less only (like a game)" } else { "normal" },
                chosen.map(|_| "ok"),
                active,
                loaded
            );
            let _ = threads.Deactivate();
            let _ = DestroyWindow(hwnd);
        }
        Ok(())
    }
}
