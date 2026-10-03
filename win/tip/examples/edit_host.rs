//! 옛 Win32 입력칸(EDIT 컨트롤)을 띄워 스스로 키를 치고 글을 읽는다. 옛 메모장과 같은 길(IMM32 → CUAS)이다. 윈도우 11의
//! notepad.exe는 스토어 메모장이 있으면 그쪽으로 넘어가서(TSF 앱) 이것을 쓴다. 입력기는 세션에서 고른 것(examples/use_tip).
//!
//!   cargo run -p cssgsg-tip --example edit_host -- keys=24,21,1f,39            쿼티 자리 스캔 코드(16진, E038처럼 확장 키,
//!                                                                              12a·22a처럼 누르기만·떼기만)
//!   ... -- fast keys=…                                                          키를 한꺼번에 보낸다(빠른 타자)
//!
//! 창을 띄우고 스스로 키를 보낸다(SendInput). 그동안 다른 창을 누르지 않는다.

fn main() {
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let fast = args.iter().any(|a| a == "fast");
        let keys: Vec<u16> = args
            .iter()
            .find_map(|a| a.strip_prefix("keys="))
            .map(|list| {
                list.split(',').map(|k| u16::from_str_radix(k.trim(), 16).expect("hex scan code")).collect()
            })
            .unwrap_or_default();
        match imp::run(&keys, fast) {
            Ok(text) => println!("text: [{}]", text.replace("\r\n", "<CRLF>")),
            Err(e) => {
                eprintln!("{e:?}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
        KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, SendInput, SetFocus,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, ES_AUTOVSCROLL,
        ES_MULTILINE, GetWindowTextLengthW, GetWindowTextW, HMENU, MSG, PM_REMOVE, PeekMessageW,
        RegisterClassW, SetForegroundWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
        WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    use windows::core::{Result, w};

    extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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

    /// 쿼티 자리(스캔 코드)로 친다. 0xFF보다 크면 확장 키. `fast`면 모두 한꺼번에 보내고 나서 메시지를 처리한다.
    unsafe fn type_keys(scans: &[u16], fast: bool) {
        let key = |scan: u16, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wScan: scan & 0xFF,
                    dwFlags: KEYEVENTF_SCANCODE
                        | if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }
                        | if scan & 0xFF00 == 0xE000 { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) },
                    ..Default::default()
                },
            },
        };
        unsafe {
            if fast {
                let all: Vec<INPUT> = scans.iter().flat_map(|&s| [key(s, false), key(s, true)]).collect();
                SendInput(&all, size_of::<INPUT>() as i32);
                pump(1500);
                return;
            }
            for &scan in scans {
                // 1xx: 누르기만, 2xx: 떼기만(Shift를 누른 채 다른 키를 칠 때).
                let (press, release) = match scan & 0xFF00 {
                    0x0100 => (true, false),
                    0x0200 => (false, true),
                    _ => (true, true),
                };
                let scan = if press != release { scan & 0xFF } else { scan };
                if press {
                    SendInput(&[key(scan, false)], size_of::<INPUT>() as i32);
                    pump(60);
                }
                if release {
                    SendInput(&[key(scan, true)], size_of::<INPUT>() as i32);
                    pump(120);
                }
            }
        }
    }

    pub fn run(keys: &[u16], fast: bool) -> Result<String> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("cssgsg-edit-host");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            });
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!("cssgsg edit host"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                480,
                240,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL) as u32),
                8,
                8,
                440,
                180,
                Some(hwnd),
                Some(HMENU(1 as _)),
                Some(instance.into()),
                None,
            )?;
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(edit));
            pump(500);
            eprintln!("keyboard layout before: {:#010x}", GetKeyboardLayout(0).0 as usize as u32);
            type_keys(keys, fast);
            pump(500);
            eprintln!("keyboard layout after: {:#010x}", GetKeyboardLayout(0).0 as usize as u32);
            let mut buf = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
            let n = GetWindowTextW(edit, &mut buf);
            let text = String::from_utf16_lossy(&buf[..n as usize]);
            let _ = DestroyWindow(hwnd);
            Ok(text)
        }
    }
}
