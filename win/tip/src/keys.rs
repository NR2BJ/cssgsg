//! 키 메시지(wParam = 가상 키, lParam = 스캔 코드·플래그)를 코어의 물리 키와 수식키 상태로 바꾼다.

use cssgsg_core::Key;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MAPVK_VK_TO_VSC, MapVirtualKeyW, VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN,
    VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RCONTROL, VK_RIGHT,
    VK_RMENU, VK_RWIN, VK_SHIFT, VK_UP,
};

/// 눌린 키 하나.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub key: Key,
    pub shift: bool,
    pub caps: bool,
    /// Ctrl·Alt·Win 중 하나라도 눌려 있다(단축키 → 먹지 않는다).
    pub command: bool,
}

/// lParam의 스캔 코드(16~23비트)와 확장 비트(24비트). 스캔 코드가 0이면(WPF 앱, 가상 키만 준 SendInput) 가상 키로 찾는다.
pub fn physical_key(wparam: WPARAM, lparam: LPARAM) -> Key {
    let flags = lparam.0 as u32;
    let scan = ((flags >> 16) & 0xFF) as u16;
    if scan != 0 {
        return Key::from_windows_scancode(scan, flags & (1 << 24) != 0);
    }
    // 배치 표(MapVirtualKey)는 확장 키를 알려 주지 않을 때가 있다: VK_DELETE → 0x53(숫자패드 .),
    // 한국어 배치의 VK_RCONTROL → 0(그 자리가 한자 키). 그래서 확장 키는 가상 키로 바로 정한다.
    let vk = VIRTUAL_KEY(wparam.0 as u16);
    match vk {
        VK_INSERT => Key::INSERT,
        VK_DELETE => Key::DELETE,
        VK_HOME => Key::HOME,
        VK_END => Key::END,
        VK_PRIOR => Key::PAGE_UP,
        VK_NEXT => Key::PAGE_DOWN,
        VK_LEFT => Key::ARROW_LEFT,
        VK_RIGHT => Key::ARROW_RIGHT,
        VK_UP => Key::ARROW_UP,
        VK_DOWN => Key::ARROW_DOWN,
        VK_RCONTROL => Key::CONTROL_RIGHT,
        VK_RMENU => Key::ALT_RIGHT,
        VK_LWIN => Key::META_LEFT,
        VK_RWIN => Key::META_RIGHT,
        _ => {
            Key::from_windows_scancode(unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16, false)
        }
    }
}

/// 이 키 메시지를 처리하는 지금의 키 상태(메시지 큐 기준, GetKeyState).
pub fn read(wparam: WPARAM, lparam: LPARAM) -> Press {
    let held = |vk: VIRTUAL_KEY| unsafe { GetKeyState(vk.0 as i32) } < 0;
    Press {
        key: physical_key(wparam, lparam),
        shift: held(VK_SHIFT),
        caps: unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0,
        command: held(VK_CONTROL) || held(VK_MENU) || held(VK_LWIN) || held(VK_RWIN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_OEM_2, VK_Q, VK_RSHIFT};

    fn lparam(scan: u32, extended: bool) -> LPARAM {
        LPARAM((1 | (scan << 16) | if extended { 1 << 24 } else { 0 }) as isize)
    }

    #[test]
    fn reads_scan_code_and_extended_bit() {
        assert_eq!(physical_key(WPARAM(VK_Q.0 as usize), lparam(0x10, false)), Key::Q);
        assert_eq!(physical_key(WPARAM(VK_RSHIFT.0 as usize), lparam(0x36, false)), Key::SHIFT_RIGHT);
        assert_eq!(physical_key(WPARAM(VK_LEFT.0 as usize), lparam(0x4B, true)), Key::ARROW_LEFT);
    }

    #[test]
    fn falls_back_to_the_virtual_key_without_a_scan_code() {
        // 글자·기호는 지금 키보드 배치(이 VM은 한국어 입력기, 자리는 US와 같다)로 찾고, 확장 키는 가상 키로 정한다.
        assert_eq!(physical_key(WPARAM(VK_Q.0 as usize), LPARAM(1)), Key::Q);
        assert_eq!(physical_key(WPARAM(VK_OEM_2.0 as usize), LPARAM(1)), Key::SLASH);
        assert_eq!(physical_key(WPARAM(VK_DELETE.0 as usize), LPARAM(1)), Key::DELETE);
        assert_eq!(physical_key(WPARAM(VK_LEFT.0 as usize), LPARAM(1)), Key::ARROW_LEFT);
        assert_eq!(physical_key(WPARAM(VK_RCONTROL.0 as usize), LPARAM(1)), Key::CONTROL_RIGHT);
    }
}
