//! 설치기가 부르는 사용자 쪽 일. 관리자 쪽(파일 복사, 입력기 DLL 등록)은 설치기(win/installer)가 하고, 사용자마다 하는 일은
//! 그 사용자로 돌아야 해서 여기 둔다(설치기가 원래 사용자로 부른다). tools/win/tip-dev.ps1의 enable·disable도 이것을 부른다.
//! - `--install-user`: 내 입력 목록에 cssgsg(한국어)를 넣고, 엔진 호스트를 시작 프로그램에 넣고 띄운다. 0.2.7까지의 en-US 항목은
//!   목록에서 빼고 그 흔적을 지운다([`remove_from_list`]).
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
windows::core::link!("kernel32.dll" "system" fn GetUserDefaultUILanguage() -> u16);

/// 입력 프로필 언어(win/tip/src/lib.rs의 LANGID_KO_KR). 0.2.7까지는 en-US(LANGID_EN_US)였다.
const LANG_KO_KR: u16 = 0x0412;
const LANG_EN_US: u16 = 0x0409;

/// 내 입력 목록의 항목 이름("0412:{CLSID}{PROFILE}", HKCU\Control Panel\International\User Profile\<언어>의 값 이름).
fn item(lang: u16) -> String {
    format!("{lang:04X}:{CLSID}{PROFILE}")
}

/// 그 언어 아래의 cssgsg를 내 입력 목록에 넣는다.
fn add_to_list(lang: u16) -> bool {
    let tip = HSTRING::from(format!("0x{}", item(lang)));
    unsafe { InstallLayoutOrTip(PCWSTR(tip.as_ptr()), 0) }.as_bool()
}

/// 내 입력 목록의 언어들(HKCU\Control Panel\International\User Profile\<언어 태그>)과 그 언어의 입력기 이름들("0412:…").
fn input_languages() -> Vec<(String, Vec<String>)> {
    let Ok(profiles) = CURRENT_USER.open(r"Control Panel\International\User Profile") else {
        return Vec::new();
    };
    let Ok(languages) = profiles.keys() else { return Vec::new() };
    languages
        .collect::<Vec<_>>()
        .into_iter()
        .map(|tag| {
            let inputs = profiles
                .open(&tag)
                .and_then(|k| {
                    k.values().map(|v| v.map(|(name, _)| name).filter(|n| n.contains(':')).collect())
                })
                .unwrap_or_default();
            (tag, inputs)
        })
        .collect()
}

/// 그 언어 아래의 cssgsg가 들어 있는 언어 태그들.
fn languages_with(lang: u16) -> Vec<String> {
    let name = item(lang);
    input_languages()
        .into_iter()
        .filter(|(_, inputs)| inputs.iter().any(|n| n.eq_ignore_ascii_case(&name)))
        .map(|(tag, _)| tag)
        .collect()
}

/// 그 언어 아래의 cssgsg를 내 입력 목록에서 뺀다. InstallLayoutOrTip(ILOT_UNINSTALL)은 언어의 마지막 입력기면 성공이라고 하고
/// 항목을 남기거나, 항목은 빼고 입력기 없는 언어를 남긴다(VM: cssgsg 하나뿐인 en-US, 2026-10-03). 그러면 윈도우 언어 목록
/// (Set-WinUserLanguageList)으로 마저 정리한다: 입력기가 남지 않은 그 언어는 목록에서 뺀다(cssgsg를 넣을 때 따라 들어온 언어다).
/// 윈도우 표시 언어면 빼지 않고 그 언어의 기본 입력기를 둔다.
fn remove_from_list(lang: u16) -> bool {
    let tags = languages_with(lang);
    if tags.is_empty() {
        return true;
    }
    let tip = HSTRING::from(format!("0x{}", item(lang)));
    let _ = unsafe { InstallLayoutOrTip(PCWSTR(tip.as_ptr()), ILOT_UNINSTALL) };
    let empty = |tag: &String| input_languages().iter().any(|(t, inputs)| t == tag && inputs.is_empty());
    if languages_with(lang).is_empty() && !tags.iter().any(empty) {
        return true;
    }
    // LANGID의 아래 10비트가 주 언어(영어 0x09, 한국어 0x12).
    let display = unsafe { GetUserDefaultUILanguage() } & 0x3FF == lang & 0x3FF;
    let quoted: Vec<String> = tags.iter().map(|t| format!("'{t}'")).collect();
    let script = format!(
        "$ErrorActionPreference = 'Stop'; $list = Get-WinUserLanguageList; \
         foreach ($l in @($list)) {{ \
           if (@({tags}) -notcontains $l.LanguageTag) {{ continue }}; \
           [void]$l.InputMethodTips.Remove('{tip}'); \
           if ($l.InputMethodTips.Count -gt 0) {{ continue }}; \
           if (${display}) {{ foreach ($t in (New-WinUserLanguageList $l.LanguageTag)[0].InputMethodTips) {{ $l.InputMethodTips.Add($t) }} }} \
           else {{ [void]$list.Remove($l) }} }}; \
         Set-WinUserLanguageList $list -Force",
        tags = quoted.join(","),
        tip = item(lang),
    );
    let ran = powershell(&script);
    let gone = languages_with(lang).is_empty() && !tags.iter().any(empty);
    log(&format!(
        "input list: {} out of {} through the language list (powershell {ran}, display language {display}): {gone}",
        item(lang),
        tags.join(",")
    ));
    gone
}

/// PowerShell 한 줄을 창 없이 돌리고 성공했는지.
fn powershell(script: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success())
}

/// 0.2.7까지의 en-US 항목을 목록에서 빼고, TSF가 그 언어에 남기는 사용자 흔적(우리 것뿐인 정렬 캐시, 사용자 키)을 지운다.
/// 목록에 없었으면 아무것도 바뀌지 않는다.
fn drop_legacy_profile() -> bool {
    let removed = remove_from_list(LANG_EN_US);
    let _ = CURRENT_USER
        .remove_tree(format!(r"Software\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x{:08x}", LANG_EN_US));
    remove_sort_order(Some(LANG_EN_US));
    removed
}

pub fn install_user() -> i32 {
    let listed = add_to_list(LANG_KO_KR);
    let legacy = drop_legacy_profile();
    let Ok(exe) = std::env::current_exe() else { return 1 };
    let startup = CURRENT_USER
        .create(RUN_KEY)
        .and_then(|k| k.set_string(RUN_VALUE, format!("\"{}\"", exe.display())))
        .is_ok();
    // 지금도 띄운다. 이미 떠 있으면 새것은 바로 끝난다(사용자당 하나).
    let spawned = std::process::Command::new(&exe).spawn().is_ok();
    log(&format!(
        "install-user: input list {listed}, old en-US entry removed {legacy}, startup {startup}, host {spawned}"
    ));
    if listed && startup && spawned { 0 } else { 1 }
}

pub fn uninstall_user() -> i32 {
    let unlisted = remove_from_list(LANG_KO_KR);
    let _ = remove_from_list(LANG_EN_US);
    // Key::open은 읽기만 연다: 지우려면 쓰기로 연다.
    let _ = CURRENT_USER.options().read().write().open(RUN_KEY).and_then(|k| k.remove_value(RUN_VALUE));
    // TSF가 사용자별로 남기는 우리 입력기 키(LanguageProfile\…\Enable=0).
    let _ = CURRENT_USER.remove_tree(format!(r"Software\Microsoft\CTF\TIP\{CLSID}"));
    remove_sort_order(None);
    let quit = quit();
    log(&format!("uninstall-user: input list removed {unlisted}, host quit {}", quit == 0));
    if unlisted { 0 } else { 1 }
}

/// ILOT_UNINSTALL 뒤에도 사용자 TSF 정렬 캐시(SortOrder\AssemblyItem\<언어>)에 우리 항목이 남는다.
/// 그 언어의 항목이 모두 우리 것일 때만 언어째 지운다(남의 입력기 순서는 건드리지 않는다). `only`면 그 언어만 본다.
fn remove_sort_order(only: Option<u16>) {
    let Ok(root) =
        CURRENT_USER.options().read().write().open(r"Software\Microsoft\CTF\SortOrder\AssemblyItem")
    else {
        return;
    };
    let Ok(languages) = root.keys() else { return };
    for language in languages.collect::<Vec<_>>() {
        if only.is_some_and(|lang| !language.eq_ignore_ascii_case(&format!("0x{lang:08x}"))) {
            continue;
        }
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
