//! 게임처럼 글자는 IMM32로 받고 TSF는 UILess 전용으로 켠 창에서, 입력기가 앱에 보내는 조합 메시지를 그대로 찍는다.
//! 오버워치(2026-10-03)가 이렇다: cssgsg의 조합 글자는 입력칸 맨 앞에 겹쳐 그려지고, MS 한국어 입력기는 제자리에 쓰인다.
//! 같은 글자를 MS 한국어 입력기와 cssgsg로 쳐서 WM_IME_COMPOSITION의 모양(플래그, 조합 글자, 커서 자리, 확정 글자)을 견준다.
//!
//!   cargo run -p cssgsg-tip --example imm_spy -- ms       MS 한국어 입력기(두벌식)로 "안녕"
//!   cargo run -p cssgsg-tip --example imm_spy -- cssgsg   cssgsg(참신세벌식, 한국어 모드)로 "안녕"
//!   ... -- cssgsg fast                                    키를 한꺼번에 보낸다(앞 키를 다 처리하기 전에 다음 키가 와 있다: 빠른 타자)
//!   ... -- cssgsg fast keys=24,21,1f,32,39                 칠 키(쿼티 자리 스캔 코드, 16진)를 직접 준다(여기는 "안" + ㄴ + Space)
//!
//! 창을 띄우고 스스로 키를 보낸다(SendInput). 그동안 다른 창을 누르지 않는다.

fn main() {
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let ms = args.first().is_some_and(|a| a == "ms");
        let fast = args.iter().any(|a| a == "fast");
        // keys=24,21,1f: 쿼티 자리 스캔 코드(16진)를 직접 준다.
        let keys = args.iter().find_map(|a| a.strip_prefix("keys=")).map(|list| {
            list.split(',').map(|k| u16::from_str_radix(k.trim(), 16).expect("scan code in hex")).collect()
        });
        if let Err(e) = imp::run(ms, fast, keys) {
            eprintln!("{e:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::time::{Duration, Instant};

    use cssgsg_tip::{CLSID_TEXT_SERVICE, GUID_PROFILE, LANGID_KO_KR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, SendInput, SetFocus,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr, ITfInputProcessorProfileMgr, ITfThreadMgrEx,
        TF_IPPMF_FORPROCESS, TF_PROFILETYPE_INPUTPROCESSOR, TF_TMAE_UIELEMENTENABLEDONLY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE,
        PeekMessageW, RegisterClassW, SetForegroundWindow, TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW,
        WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    use windows::core::{GUID, Result, w};

    /// MS 한국어 입력기(win/tip 개발 기록: ko 0412:{A028AE76…}{B5FE1F02…}).
    const CLSID_MS_KOREAN: GUID = GUID::from_u128(0xA028AE76_01B1_46C2_99C4_ACD9858AE02F);
    const PROFILE_MS_KOREAN: GUID = GUID::from_u128(0xB5FE1F02_D5F2_4445_9C03_C568F23C99A1);

    const WM_KEYDOWN: u32 = 0x0100;
    const WM_CHAR: u32 = 0x0102;
    const WM_IME_STARTCOMPOSITION: u32 = 0x010D;
    const WM_IME_ENDCOMPOSITION: u32 = 0x010E;
    const WM_IME_COMPOSITION: u32 = 0x010F;
    const WM_IME_NOTIFY: u32 = 0x0282;
    const WM_IME_CHAR: u32 = 0x0286;
    const GCS_COMPSTR: u32 = 0x0008;
    const GCS_CURSORPOS: u32 = 0x0080;
    const GCS_DELTASTART: u32 = 0x0100;
    const GCS_RESULTSTR: u32 = 0x0800;
    const IME_CMODE_NATIVE: u32 = 0x0001;

    #[link(name = "imm32")]
    unsafe extern "system" {
        fn ImmGetContext(hwnd: HWND) -> isize;
        fn ImmReleaseContext(hwnd: HWND, himc: isize) -> i32;
        fn ImmGetCompositionStringW(himc: isize, index: u32, buf: *mut c_void, len: u32) -> i32;
        fn ImmSetOpenStatus(himc: isize, open: i32) -> i32;
        fn ImmSetConversionStatus(himc: isize, conversion: u32, sentence: u32) -> i32;
        fn ImmGetProperty(hkl: isize, index: u32) -> u32;
        fn ImmIsIME(hkl: isize) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetKeyboardLayout(thread: u32) -> isize;
    }

    /// 앱이 볼 수 있는 입력 언어와 입력기 성질(게임이 언어로 조합 표시 방식을 고를 수 있다).
    unsafe fn print_layout(when: &str) {
        unsafe {
            let hkl = GetKeyboardLayout(0);
            println!(
                "{when}: keyboard layout {:#010x}, ImmIsIME {}, IGP_PROPERTY {:#x}, IGP_CONVERSION {:#x}, IGP_SENTENCE {:#x}, IGP_UI {:#x}, IGP_SETCOMPSTR {:#x}, IGP_SELECT {:#x}",
                hkl as u32,
                ImmIsIME(hkl),
                ImmGetProperty(hkl, 0x04),
                ImmGetProperty(hkl, 0x08),
                ImmGetProperty(hkl, 0x0C),
                ImmGetProperty(hkl, 0x10),
                ImmGetProperty(hkl, 0x14),
                ImmGetProperty(hkl, 0x18),
            );
        }
    }

    /// 조합 정보 바이트(속성) 또는 u32 목록(문절).
    unsafe fn comp_bytes(hwnd: HWND, index: u32) -> Vec<u8> {
        unsafe {
            let himc = ImmGetContext(hwnd);
            let bytes = ImmGetCompositionStringW(himc, index, std::ptr::null_mut(), 0);
            let mut buf = vec![0u8; bytes.max(0) as usize];
            if bytes > 0 {
                ImmGetCompositionStringW(himc, index, buf.as_mut_ptr().cast(), bytes as u32);
            }
            ImmReleaseContext(hwnd, himc);
            buf
        }
    }

    /// 조합 문자열 하나(글자) 또는 숫자 하나(커서 자리 등).
    unsafe fn comp_string(hwnd: HWND, index: u32) -> String {
        unsafe {
            let himc = ImmGetContext(hwnd);
            let bytes = ImmGetCompositionStringW(himc, index, std::ptr::null_mut(), 0);
            let mut buf = vec![0u16; (bytes.max(0) as usize).div_ceil(2)];
            if bytes > 0 {
                ImmGetCompositionStringW(himc, index, buf.as_mut_ptr().cast(), bytes as u32);
            }
            ImmReleaseContext(hwnd, himc);
            String::from_utf16_lossy(&buf)
        }
    }

    unsafe fn comp_number(hwnd: HWND, index: u32) -> i32 {
        unsafe {
            let himc = ImmGetContext(hwnd);
            let n = ImmGetCompositionStringW(himc, index, std::ptr::null_mut(), 0);
            ImmReleaseContext(hwnd, himc);
            n
        }
    }

    /// 플래그 이름.
    fn flags(l: u32) -> String {
        let names = [
            (0x0001, "COMPREADSTR"),
            (0x0002, "COMPREADATTR"),
            (0x0004, "COMPREADCLAUSE"),
            (0x0008, "COMPSTR"),
            (0x0010, "COMPATTR"),
            (0x0020, "COMPCLAUSE"),
            (0x0080, "CURSORPOS"),
            (0x0100, "DELTASTART"),
            (0x0200, "RESULTREADSTR"),
            (0x0400, "RESULTREADCLAUSE"),
            (0x0800, "RESULTSTR"),
            (0x1000, "RESULTCLAUSE"),
            (0x2000, "CS_INSERTCHAR"),
            (0x4000, "CS_NOMOVECARET"),
        ];
        names.iter().filter(|(b, _)| l & b != 0).map(|(_, n)| *n).collect::<Vec<_>>().join("|")
    }

    extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_IME_STARTCOMPOSITION => {
                    println!("  STARTCOMPOSITION");
                    return LRESULT(0);
                }
                WM_IME_ENDCOMPOSITION => {
                    println!("  ENDCOMPOSITION");
                    return LRESULT(0);
                }
                WM_IME_COMPOSITION => {
                    let l = lparam.0 as u32;
                    let ch = char::from_u32(wparam.0 as u32).map(|c| c.to_string()).unwrap_or_default();
                    let mut line = format!("  COMPOSITION wParam={:?} [{}]", ch, flags(l));
                    if l & GCS_COMPSTR != 0 {
                        line += &format!(" comp={:?}", comp_string(hwnd, GCS_COMPSTR));
                    }
                    if l & GCS_CURSORPOS != 0 {
                        line += &format!(" cursor={}", comp_number(hwnd, GCS_CURSORPOS));
                    }
                    if l & GCS_DELTASTART != 0 {
                        line += &format!(" delta={}", comp_number(hwnd, GCS_DELTASTART));
                    }
                    if l & GCS_RESULTSTR != 0 {
                        line += &format!(" result={:?}", comp_string(hwnd, GCS_RESULTSTR));
                    }
                    // 플래그와 상관없이 앱이 읽을 수 있는 것.
                    let clause: Vec<u32> = comp_bytes(hwnd, 0x20)
                        .chunks_exact(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    line += &format!(
                        "\n      any: cursor={} delta={} attr={:?} clause={:?} read={:?} readattr={:?}",
                        comp_number(hwnd, GCS_CURSORPOS),
                        comp_number(hwnd, GCS_DELTASTART),
                        comp_bytes(hwnd, 0x10),
                        clause,
                        comp_string(hwnd, 0x01),
                        comp_bytes(hwnd, 0x02),
                    );
                    println!("{line}");
                    return LRESULT(0);
                }
                WM_IME_NOTIFY => println!("  NOTIFY {:#x} {:#x}", wparam.0, lparam.0),
                // 앱이 언어 바뀜을 아는 길.
                0x0051 => println!("  INPUTLANGCHANGE charset {} layout {:#010x}", wparam.0, lparam.0 as u32),
                0x0281 => println!("  IME_SETCONTEXT {} {:#x}", wparam.0, lparam.0 as u32),
                0x0285 => println!("  IME_SELECT {} {:#010x}", wparam.0, lparam.0 as u32),
                0x0288 => println!("  IME_REQUEST {:#x}", wparam.0),
                // 0xE5(VK_PROCESSKEY)면 입력기가 그 키를 가져갔다.
                WM_KEYDOWN => println!("  KEYDOWN vk={:#04x}", wparam.0),
                WM_IME_CHAR => println!("  IME_CHAR {:?}", char::from_u32(wparam.0 as u32)),
                WM_CHAR => println!("  CHAR {:?}", char::from_u32(wparam.0 as u32)),
                _ => {}
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    unsafe fn pump(ms: u64) {
        let start = Instant::now();
        let mut msg = MSG::default();
        while start.elapsed() < Duration::from_millis(ms) {
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 쿼티 자리(스캔 코드)로 한 키씩 친다. `fast`면 모두 한꺼번에 보내고 나서 메시지를 처리한다.
    unsafe fn type_keys(scans: &[u16], fast: bool) {
        let key = |scan: u16, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    // 0xE038처럼 0xFF보다 크면 확장 키(E0 접두): 오른쪽 Alt·Ctrl.
                    wScan: scan & 0xFF,
                    dwFlags: KEYEVENTF_SCANCODE
                        | if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }
                        | if scan > 0xFF { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) },
                    ..Default::default()
                },
            },
        };
        if fast {
            println!("keys {scans:#04x?} (all at once)");
            let all: Vec<INPUT> =
                scans.iter().flat_map(|&scan| [key(scan, false), key(scan, true)]).collect();
            unsafe {
                SendInput(&all, size_of::<INPUT>() as i32);
                pump(1500);
            }
            return;
        }
        for &scan in scans {
            println!("key {scan:#04x}");
            unsafe {
                SendInput(&[key(scan, false)], size_of::<INPUT>() as i32);
                pump(60);
                SendInput(&[key(scan, true)], size_of::<INPUT>() as i32);
                pump(120);
            }
        }
    }

    pub fn run(ms: bool, fast: bool, keys: Option<Vec<u16>>) -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            // 게임처럼 창보다 먼저 TSF를 UILess 전용으로 켠다(cssgsg는 이것을 보고 게임 스레드로 다룬다). 키는 TSF에 넘기지 않고
            // 보통 메시지로 처리한다: 입력기는 IMM32(CUAS)를 거쳐 키를 받는다(오버워치 기록과 같은 길).
            let threads: ITfThreadMgrEx = CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
            let mut client = 0;
            threads.ActivateEx(&mut client, TF_TMAE_UIELEMENTENABLEDONLY)?;
            let instance = GetModuleHandleW(None)?;
            let class = w!("cssgsg-imm-spy");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            });
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!("cssgsg IMM spy"),
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
            pump(300);
            print_layout("before");
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            let (lang, clsid, profile) = if ms {
                (0x0412, CLSID_MS_KOREAN, PROFILE_MS_KOREAN)
            } else {
                (LANGID_KO_KR, CLSID_TEXT_SERVICE, GUID_PROFILE)
            };
            profiles.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                lang,
                &clsid,
                &profile,
                HKL::default(),
                TF_IPPMF_FORPROCESS,
            )?;
            pump(500);
            print_layout("after ActivateProfile");
            if ms {
                // MS 한국어 입력기를 한글 상태로.
                let himc = ImmGetContext(hwnd);
                ImmSetOpenStatus(himc, 1);
                ImmSetConversionStatus(himc, IME_CMODE_NATIVE, 0);
                ImmReleaseContext(hwnd, himc);
                pump(200);
                println!("== Microsoft Korean IME (두벌식) 안녕: d k s s u d, Space, d k, Enter");
                type_keys(
                    keys.as_deref().unwrap_or(&[0x20, 0x25, 0x1F, 0x1F, 0x16, 0x20, 0x39, 0x20, 0x25, 0x1C]),
                    fast,
                );
            } else {
                println!("== cssgsg (참신세벌식, 한국어 모드여야 한다) 안녕: j f s m t d, Space, j f, Enter");
                type_keys(
                    keys.as_deref().unwrap_or(&[0x24, 0x21, 0x1F, 0x32, 0x14, 0x20, 0x39, 0x24, 0x21, 0x1C]),
                    fast,
                );
            }
            pump(500);
            let _ = threads.Deactivate();
            let _ = DestroyWindow(hwnd);
        }
        Ok(())
    }
}
