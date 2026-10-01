//! 설치기가 부르는 사용자 쪽 일. 관리자 쪽(파일 복사, 입력기 DLL 등록)은 설치기(win/installer)가 하고, 사용자마다 하는 일은
//! 그 사용자로 돌아야 해서 여기 둔다(설치기가 원래 사용자로 부른다). tools/win/tip-dev.ps1의 enable·disable도 이것을 부른다.
//! - `--install-user`: 내 입력 목록에 cssgsg(en-US)를 넣고, 엔진 호스트를 시작 프로그램에 넣고 띄운다.
//! - `--uninstall-user`: 목록에서 빼고, TSF가 사용자별로 남기는 흔적(정렬 캐시, 사용자 키)과 시작 프로그램을 지우고, 호스트를 끈다.
//!   학습·사용자 사전(%LOCALAPPDATA%\cssgsg)은 남긴다.
//! - `--quit`: 떠 있는 호스트를 끈다(엔진을 내려 학습을 마무리한 뒤). 설치기가 파일을 바꾸기 전에 부른다.

use std::time::{Duration, Instant};

use cssgsg_ipc::pipe::{Client, pipe_name, user_sid};
use cssgsg_ipc::{PROTOCOL, Request};
use windows::core::{BOOL, HSTRING, PCWSTR};
use windows_registry::{CURRENT_USER, Key};

use crate::host::log;

/// win/tip/src/lib.rs의 CLSID_TEXT_SERVICE, GUID_PROFILE.
const CLSID: &str = "{A9227DC2-56BC-4023-AE29-82A9AAA4EE14}";
const PROFILE: &str = "{DCFBD969-D52F-4AFB-ABC0-271CC58FE18C}";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "cssgsg";
const ILOT_UNINSTALL: u32 = 1;

// input.dll에 있고 SDK 헤더·가져오기 라이브러리는 없다(문서: LoadLibrary로 찾는다). windows의 link!는 raw-dylib라 된다.
windows::core::link!("input.dll" "system" fn InstallLayoutOrTip(psz: PCWSTR, dwflags: u32) -> BOOL);

/// 내 입력 목록에 넣거나(0) 뺀다(ILOT_UNINSTALL).
fn input_list(flags: u32) -> bool {
    let tip = HSTRING::from(format!("0x0409:{CLSID}{PROFILE}"));
    unsafe { InstallLayoutOrTip(PCWSTR(tip.as_ptr()), flags) }.as_bool()
}

pub fn install_user() -> i32 {
    let listed = input_list(0);
    let Ok(exe) = std::env::current_exe() else { return 1 };
    let startup = CURRENT_USER
        .create(RUN_KEY)
        .and_then(|k| k.set_string(RUN_VALUE, format!("\"{}\"", exe.display())))
        .is_ok();
    // 지금도 띄운다. 이미 떠 있으면 새것은 바로 끝난다(사용자당 하나).
    let spawned = std::process::Command::new(&exe).spawn().is_ok();
    log(&format!("install-user: input list {listed}, startup {startup}, host {spawned}"));
    if listed && startup && spawned { 0 } else { 1 }
}

pub fn uninstall_user() -> i32 {
    let unlisted = input_list(ILOT_UNINSTALL);
    // Key::open은 읽기만 연다: 지우려면 쓰기로 연다.
    let _ = CURRENT_USER.options().read().write().open(RUN_KEY).and_then(|k| k.remove_value(RUN_VALUE));
    // TSF가 사용자별로 남기는 우리 입력기 키(LanguageProfile\…\Enable=0).
    let _ = CURRENT_USER.remove_tree(format!(r"Software\Microsoft\CTF\TIP\{CLSID}"));
    remove_sort_order();
    let quit = quit();
    log(&format!("uninstall-user: input list removed {unlisted}, host quit {}", quit == 0));
    if unlisted { 0 } else { 1 }
}

/// ILOT_UNINSTALL 뒤에도 사용자 TSF 정렬 캐시(SortOrder\AssemblyItem\<언어>)에 우리 항목이 남는다.
/// 그 언어의 항목이 모두 우리 것일 때만 언어째 지운다(남의 입력기 순서는 건드리지 않는다).
fn remove_sort_order() {
    let Ok(root) =
        CURRENT_USER.options().read().write().open(r"Software\Microsoft\CTF\SortOrder\AssemblyItem")
    else {
        return;
    };
    let Ok(languages) = root.keys() else { return };
    for language in languages.collect::<Vec<_>>() {
        let mut clsids = Vec::new();
        collect_clsids(&root, &language, &mut clsids);
        if !clsids.is_empty() && clsids.iter().all(|c| c.eq_ignore_ascii_case(CLSID)) {
            let _ = root.remove_tree(&language);
        }
    }
}

fn collect_clsids(parent: &Key, path: &str, out: &mut Vec<String>) {
    let Ok(key) = parent.open(path) else { return };
    if let Ok(clsid) = key.get_string("CLSID") {
        out.push(clsid);
    }
    if let Ok(children) = key.keys() {
        for child in children.collect::<Vec<_>>() {
            collect_clsids(&key, &child, out);
        }
    }
}

/// 떠 있는 호스트에 끝내라고 하고, 파이프가 없어질 때까지(3초까지) 기다린다. 떠 있지 않았으면 바로 0.
pub fn quit() -> i32 {
    let Some(sid) = user_sid() else { return 1 };
    let name = pipe_name(&sid, "");
    if Client::new(name.clone(), None).call(&Request::Quit, Duration::from_secs(2)).is_err() {
        return 0;
    }
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(3) {
        let gone = Client::new(name.clone(), None)
            .call(&Request::Hello { protocol: PROTOCOL }, Duration::from_millis(100))
            .is_err();
        if gone {
            return 0;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    1
}
