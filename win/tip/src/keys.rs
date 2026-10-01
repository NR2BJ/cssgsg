//! 키 메시지(wParam = 가상 키, lParam = 스캔 코드·플래그)를 코어의 키 이벤트로 바꾼다.

use cssgsg_core::{Key, KeyEvent, Mods};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MAPVK_VK_TO_VSC, MapVirtualKeyW, VIRTUAL_KEY, VK_CAPITAL, VK_DELETE, VK_DOWN, VK_END,
    VK_HOME, VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_NEXT, VK_PRIOR, VK_RCONTROL,
    VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_UP,
};

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

/// 이 키 메시지를 처리하는 때의 수식키 상태(메시지 큐 기준, GetKeyState, 좌우 구분).
/// 이 이벤트의 수식키 자신은 눌림/뗌을 그대로 반영한다: 엔진은 "이 이벤트가 반영된 뒤"의 상태를 받는데,
/// 키 상태가 아직 안 바뀌었을 수 있다.
pub fn mods(key: Key, down: bool) -> Mods {
    const SIDES: [(VIRTUAL_KEY, u32); 8] = [
        (VK_LSHIFT, Mods::SHIFT_L),
        (VK_RSHIFT, Mods::SHIFT_R),
        (VK_LCONTROL, Mods::CTRL_L),
        (VK_RCONTROL, Mods::CTRL_R),
        (VK_LMENU, Mods::ALT_L),
        (VK_RMENU, Mods::ALT_R),
        (VK_LWIN, Mods::META_L),
        (VK_RWIN, Mods::META_R),
    ];
    let mut bits = 0;
    for (vk, bit) in SIDES {
        if unsafe { GetKeyState(vk.0 as i32) } < 0 {
            bits |= bit;
        }
    }
    if caps_on() {
        bits |= Mods::CAPS;
    }
    let own = key.modifier_bit();
    if own != 0 {
        if down {
            bits |= own;
        } else {
            bits &= !own;
        }
    }
    Mods(bits)
}

/// Caps Lock 켜짐(토글 상태).
pub fn caps_on() -> bool {
    (unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1) != 0
}

/// 키 메시지 하나 → 엔진 키 이벤트. 자동 반복은 lParam 30비트(직전에 이미 눌려 있었다).
pub fn event(down: bool, wparam: WPARAM, lparam: LPARAM, time: f64) -> KeyEvent {
    let key = physical_key(wparam, lparam);
    let repeat = down && (lparam.0 as u32) & (1 << 30) != 0;
    KeyEvent { key, down, mods: mods(key, down), repeat, time }
}

/// 키 메시지 시각(GetMessageTime, 밀리초, 49.7일마다 한 바퀴 돈다)을 줄지 않는 초로 바꾼다.
#[derive(Default)]
pub struct Clock {
    last: Option<u32>,
    laps: u64,
    seconds: f64,
}

impl Clock {
    pub fn seconds(&mut self, raw_ms: u32) -> f64 {
        if let Some(last) = self.last
            && raw_ms < last
            && last - raw_ms > 0x8000_0000
        {
            self.laps += 1;
        }
        self.last = Some(raw_ms);
        let now = ((self.laps << 32) + raw_ms as u64) as f64 / 1000.0;
        // 메시지 시각은 앞뒤가 조금 바뀌어 올 수 있다(보낸 메시지와 입력 메시지). 엔진에는 줄지 않게 준다.
        self.seconds = self.seconds.max(now);
        self.seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{SetKeyboardState, VK_OEM_2, VK_Q, VK_SHIFT};

    fn lparam(scan: u32, extended: bool) -> LPARAM {
        LPARAM((1 | (scan << 16) | if extended { 1 << 24 } else { 0 }) as isize)
    }

    fn hold(keys: &[VIRTUAL_KEY]) {
        let mut state = [0u8; 256];
        for k in keys {
            state[k.0 as usize] = 0x80;
        }
        unsafe { SetKeyboardState(&state).unwrap() };
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

    #[test]
    fn modifier_events_carry_their_own_side_and_state() {
        // 키 상태가 아직 안 바뀌었어도 이 이벤트의 수식키는 눌림/뗌을 따른다. 다른 수식키는 키 상태에서 좌우를 읽는다.
        hold(&[]);
        let down = event(true, WPARAM(VK_SHIFT.0 as usize), lparam(0x36, false), 1.0);
        assert_eq!((down.key, down.mods), (Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R)));
        hold(&[VK_SHIFT, VK_RSHIFT, VK_LCONTROL]);
        let up = event(false, WPARAM(VK_SHIFT.0 as usize), lparam(0x36, false), 1.1);
        assert_eq!(up.mods, Mods(Mods::CTRL_L));
        let q = event(true, WPARAM(VK_Q.0 as usize), lparam(0x10, false), 1.2);
        assert_eq!(q.mods, Mods(Mods::SHIFT_R | Mods::CTRL_L));
        hold(&[]);
    }

    #[test]
    fn auto_repeat_is_bit_30_of_a_key_down() {
        let first = event(true, WPARAM(VK_Q.0 as usize), lparam(0x10, false), 1.0);
        let again = event(true, WPARAM(VK_Q.0 as usize), LPARAM(lparam(0x10, false).0 | 1 << 30), 1.1);
        let up =
            event(false, WPARAM(VK_Q.0 as usize), LPARAM(lparam(0x10, false).0 | 1 << 30 | 1 << 31), 1.2);
        assert_eq!((first.repeat, again.repeat, up.repeat), (false, true, false));
    }

    #[test]
    fn clock_never_goes_back_and_survives_the_wrap() {
        let mut c = Clock::default();
        assert_eq!(c.seconds(1_000), 1.0);
        assert_eq!(c.seconds(900), 1.0, "조금 앞선 시각은 그대로 둔다");
        assert_eq!(c.seconds(1_500), 1.5);
        let mut c = Clock::default();
        c.seconds(u32::MAX - 999);
        let after = c.seconds(1_000);
        assert!(
            (after - (u32::MAX as f64 + 1.0 + 1_000.0) / 1000.0).abs() < 1e-6,
            "한 바퀴 돌아도 이어진다: {after}"
        );
    }
}
