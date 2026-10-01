//! 작업 표시줄 모드 아이콘(G/ㅊ/月)의 메뉴. 맥 메뉴 막대의 cssgsg 메뉴와 같은 몫이다: 모드 고르기, 설정·배열 학습·타자 연습
//! 열기, 다시 시작(엔진 호스트를 다시 띄운다). 맥의 "종료"는 없다: 윈도우 입력기는 앱마다 뜨는 DLL이라 끌 프로세스가 없고,
//! 다른 입력기로는 Win+Space로 바꾼다.
//!
//! 아이콘을 누르면 TSF가 이 앱의 입력기에 OnClick을 부르고(왼쪽·오른쪽), 메뉴는 이 스레드에서 띄운다(TrackPopupMenuEx).
//! TSF가 메뉴를 대신 그려 주는 길(ITfMenu, InitMenu)에도 같은 항목을 채운다.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Once;

use cssgsg_core::Mode;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::UI::TextServices::{ITfMenu, TF_LBMENUF_CHECKED, TF_LBMENUF_SEPARATOR};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, IsWindow, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, PostMessageW, RegisterClassExW, SetForegroundWindow, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPMPARAMS, TrackPopupMenuEx, WM_NULL,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, w};

use crate::module;

/// 메뉴에서 고른 것.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Mode(Mode),
    Settings,
    Learn,
    Practice,
    Restart,
}

/// 고른 것을 처리하는 쪽(텍스트 서비스가 만든다).
pub type Handler = Rc<dyn Fn(Command)>;

const SEPARATOR: u32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    Ko,
    En,
    Ja,
}

/// 설정 앱에서 고른 화면 언어(HKCU\Software\cssgsg AppLanguage). 없으면 윈도우 표시 언어(한국어·일본어, 그 밖은 영어),
/// 읽을 수 없으면(앱 컨테이너) 한국어.
fn language() -> Language {
    match windows_registry::CURRENT_USER.open("Software\\cssgsg").and_then(|k| k.get_string("AppLanguage")) {
        Ok(v) if v == "en" => Language::En,
        Ok(v) if v == "ja" => Language::Ja,
        Ok(v) if v == "ko" => Language::Ko,
        _ => match unsafe { GetUserDefaultUILanguage() } & 0x3FF {
            0x12 => Language::Ko,
            0x11 => Language::Ja,
            0 => Language::Ko,
            _ => Language::En,
        },
    }
}

/// 메뉴 항목: (번호, 글자, 체크). 번호 0은 구분선.
fn items(mode: Mode) -> Vec<(u32, String, bool)> {
    let lang = language();
    let t = |ko: &str, en: &str, ja: &str| {
        match lang {
            Language::Ko => ko,
            Language::En => en,
            Language::Ja => ja,
        }
        .to_string()
    };
    vec![
        (1, t("G   영어 (Graphite)", "G   English (Graphite)", "G   英語（Graphite）"), mode == Mode::En),
        (
            2,
            t(
                "ㅊ  한국어 (참신세벌식)",
                "ㅊ  Korean (Chamshin Sebeolsik)",
                "ㅊ  韓国語（チャムシン3ボル式）",
            ),
            mode == Mode::Ko,
        ),
        (3, t("月  일본어 (新月)", "月  Japanese (Shingetsu)", "月  日本語（新月）"), mode == Mode::Ja),
        (SEPARATOR, String::new(), false),
        (10, t("설정…", "Settings…", "設定…"), false),
        (11, t("배열 학습", "Layouts", "配列の学習"), false),
        (12, t("타자 연습", "Typing Practice", "タイピング練習"), false),
        (SEPARATOR, String::new(), false),
        (20, t("다시 시작", "Restart", "再起動"), false),
    ]
}

/// 메뉴 번호 → 고른 것.
pub fn command(id: u32) -> Option<Command> {
    Some(match id {
        1 => Command::Mode(Mode::En),
        2 => Command::Mode(Mode::Ko),
        3 => Command::Mode(Mode::Ja),
        10 => Command::Settings,
        11 => Command::Learn,
        12 => Command::Practice,
        20 => Command::Restart,
        _ => return None,
    })
}

/// TSF가 그리는 메뉴(ITfMenu)를 채운다.
pub fn fill(menu: &ITfMenu, mode: Mode) {
    for (id, text, checked) in items(mode) {
        let flags = if id == SEPARATOR {
            TF_LBMENUF_SEPARATOR
        } else if checked {
            TF_LBMENUF_CHECKED
        } else {
            0
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let _ = unsafe {
            menu.AddMenuItem(id, flags, HBITMAP::default(), HBITMAP::default(), &wide, std::ptr::null_mut())
        };
    }
}

thread_local! {
    /// 메뉴 주인 창(보이지 않는다). 스레드마다 하나.
    static OWNER: Cell<Option<HWND>> = const { Cell::new(None) };
}

const OWNER_CLASS: windows::core::PCWSTR = w!("cssgsg.MenuOwner");

unsafe extern "system" fn owner_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn owner() -> Option<HWND> {
    if let Some(hwnd) = OWNER.get()
        && unsafe { IsWindow(Some(hwnd)) }.as_bool()
    {
        return Some(hwnd);
    }
    static CLASS: Once = Once::new();
    CLASS.call_once(|| unsafe {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(owner_proc),
            hInstance: module().into(),
            lpszClassName: OWNER_CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class);
    });
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            OWNER_CLASS,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module().into()),
            None,
        )
    }
    .ok()?;
    OWNER.set(Some(hwnd));
    Some(hwnd)
}

/// 아이콘 옆에 메뉴를 띄우고 고른 것을 돌려준다(고르지 않으면 None). `around`는 아이콘 사각형(가리지 않게).
pub fn show(at: POINT, around: Option<RECT>, mode: Mode) -> Option<Command> {
    let owner = owner()?;
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for (id, text, checked) in items(mode) {
            let _ = if id == SEPARATOR {
                AppendMenuW(menu, MF_SEPARATOR, 0, None)
            } else {
                let flags = if checked { MF_STRING | MF_CHECKED } else { MF_STRING };
                AppendMenuW(menu, flags, id as usize, &HSTRING::from(text))
            };
        }
        // 메뉴 밖을 누르면 닫히게 주인을 앞으로 가져온다(알림 영역 아이콘 메뉴와 같다).
        let _ = SetForegroundWindow(owner);
        let params =
            around.map(|r| TPMPARAMS { cbSize: std::mem::size_of::<TPMPARAMS>() as u32, rcExclude: r });
        let chosen = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN).0,
            at.x,
            at.y,
            owner,
            params.as_ref().map(|p| p as *const TPMPARAMS),
        );
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        command(chosen.0 as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_has_a_command_and_the_mode_is_checked() {
        let all = items(Mode::Ja);
        for (id, text, _) in &all {
            if *id != SEPARATOR {
                assert!(command(*id).is_some() && !text.is_empty(), "{id}");
            }
        }
        let checked: Vec<u32> = all.iter().filter(|i| i.2).map(|i| i.0).collect();
        assert_eq!(checked, [3]);
        assert_eq!(command(2), Some(Command::Mode(Mode::Ko)));
        assert_eq!(command(0), None);
    }
}
