//! 물리 키 모델.
//!
//! 키 코드는 USB HID Usage(키보드 페이지 0x07) 값을 그대로 쓴다. 맥 keyCode와
//! 윈도우 scan code는 셸 경계에서 이 값으로 바꾼다([`Key::from_mac_keycode`],
//! [`Key::from_windows_scancode`]).
//! 배열 데이터 파일은 키를 쿼티 자리의 소문자 글자("q", ";", "/")로 적고,
//! [`Key::from_qwerty`]로 읽는다.

use std::fmt;

/// 물리 키. 값은 USB HID Usage ID다.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(pub u16);

impl Key {
    pub const UNKNOWN: Key = Key(0);

    pub const A: Key = Key(0x04);
    pub const B: Key = Key(0x05);
    pub const C: Key = Key(0x06);
    pub const D: Key = Key(0x07);
    pub const E: Key = Key(0x08);
    pub const F: Key = Key(0x09);
    pub const G: Key = Key(0x0A);
    pub const H: Key = Key(0x0B);
    pub const I: Key = Key(0x0C);
    pub const J: Key = Key(0x0D);
    pub const K: Key = Key(0x0E);
    pub const L: Key = Key(0x0F);
    pub const M: Key = Key(0x10);
    pub const N: Key = Key(0x11);
    pub const O: Key = Key(0x12);
    pub const P: Key = Key(0x13);
    pub const Q: Key = Key(0x14);
    pub const R: Key = Key(0x15);
    pub const S: Key = Key(0x16);
    pub const T: Key = Key(0x17);
    pub const U: Key = Key(0x18);
    pub const V: Key = Key(0x19);
    pub const W: Key = Key(0x1A);
    pub const X: Key = Key(0x1B);
    pub const Y: Key = Key(0x1C);
    pub const Z: Key = Key(0x1D);

    pub const DIGIT1: Key = Key(0x1E);
    pub const DIGIT2: Key = Key(0x1F);
    pub const DIGIT3: Key = Key(0x20);
    pub const DIGIT4: Key = Key(0x21);
    pub const DIGIT5: Key = Key(0x22);
    pub const DIGIT6: Key = Key(0x23);
    pub const DIGIT7: Key = Key(0x24);
    pub const DIGIT8: Key = Key(0x25);
    pub const DIGIT9: Key = Key(0x26);
    pub const DIGIT0: Key = Key(0x27);

    pub const ENTER: Key = Key(0x28);
    pub const ESCAPE: Key = Key(0x29);
    pub const BACKSPACE: Key = Key(0x2A);
    pub const TAB: Key = Key(0x2B);
    pub const SPACE: Key = Key(0x2C);
    pub const MINUS: Key = Key(0x2D);
    pub const EQUAL: Key = Key(0x2E);
    pub const BRACKET_LEFT: Key = Key(0x2F);
    pub const BRACKET_RIGHT: Key = Key(0x30);
    pub const BACKSLASH: Key = Key(0x31);
    pub const SEMICOLON: Key = Key(0x33);
    pub const QUOTE: Key = Key(0x34);
    pub const BACKQUOTE: Key = Key(0x35);
    pub const COMMA: Key = Key(0x36);
    pub const PERIOD: Key = Key(0x37);
    pub const SLASH: Key = Key(0x38);
    pub const CAPS_LOCK: Key = Key(0x39);

    pub const F1: Key = Key(0x3A);
    pub const F12: Key = Key(0x45);
    pub const INSERT: Key = Key(0x49);
    pub const HOME: Key = Key(0x4A);
    pub const PAGE_UP: Key = Key(0x4B);
    pub const DELETE: Key = Key(0x4C);
    pub const END: Key = Key(0x4D);
    pub const PAGE_DOWN: Key = Key(0x4E);
    pub const ARROW_RIGHT: Key = Key(0x4F);
    pub const ARROW_LEFT: Key = Key(0x50);
    pub const ARROW_DOWN: Key = Key(0x51);
    pub const ARROW_UP: Key = Key(0x52);
    pub const NUMPAD_ENTER: Key = Key(0x58);

    pub const CONTROL_LEFT: Key = Key(0xE0);
    pub const SHIFT_LEFT: Key = Key(0xE1);
    pub const ALT_LEFT: Key = Key(0xE2);
    pub const META_LEFT: Key = Key(0xE3);
    pub const CONTROL_RIGHT: Key = Key(0xE4);
    pub const SHIFT_RIGHT: Key = Key(0xE5);
    pub const ALT_RIGHT: Key = Key(0xE6);
    pub const META_RIGHT: Key = Key(0xE7);
    /// 맥 Fn(Globe). HID 키보드 페이지에 없어서 따로 잡은 값이다.
    pub const FN: Key = Key(0x0100);

    /// 글자 키(A~Z)인지.
    pub fn is_letter(self) -> bool {
        (Self::A.0..=Self::Z.0).contains(&self.0)
    }

    /// 숫자열 숫자 키(1~0)인지.
    pub fn is_digit(self) -> bool {
        (Self::DIGIT1.0..=Self::DIGIT0.0).contains(&self.0)
    }

    /// 쿼티에서 글자를 내는 키(글자, 숫자, 기호)인지.
    pub fn is_printable(self) -> bool {
        self.qwerty_char(false).is_some()
    }

    /// 수식키(Shift, Control, Option/Alt, Command/Win, Fn)인지. Caps Lock은 포함하지 않는다.
    pub fn is_modifier(self) -> bool {
        self.modifier_bit() != 0
    }

    /// 이 수식키가 [`Mods`]에서 차지하는 비트. 수식키가 아니면 0.
    pub fn modifier_bit(self) -> u32 {
        match self {
            Self::SHIFT_LEFT => Mods::SHIFT_L,
            Self::SHIFT_RIGHT => Mods::SHIFT_R,
            Self::CONTROL_LEFT => Mods::CTRL_L,
            Self::CONTROL_RIGHT => Mods::CTRL_R,
            Self::ALT_LEFT => Mods::ALT_L,
            Self::ALT_RIGHT => Mods::ALT_R,
            Self::META_LEFT => Mods::META_L,
            Self::META_RIGHT => Mods::META_R,
            Self::FN => Mods::FN,
            _ => 0,
        }
    }

    /// 이 수식키 무리(좌우 둘)의 비트. 수식키가 아니면 0. 셸이 좌우를 모를 때(지금 누르고 있는 수식키) 무리로 본다.
    pub fn modifier_family_bits(self) -> u32 {
        match self {
            Self::SHIFT_LEFT | Self::SHIFT_RIGHT => Mods::SHIFT_L | Mods::SHIFT_R,
            Self::CONTROL_LEFT | Self::CONTROL_RIGHT => Mods::CTRL_L | Mods::CTRL_R,
            Self::ALT_LEFT | Self::ALT_RIGHT => Mods::ALT_L | Mods::ALT_R,
            Self::META_LEFT | Self::META_RIGHT => Mods::META_L | Mods::META_R,
            Self::FN => Mods::FN,
            _ => 0,
        }
    }

    /// 쿼티 자리 글자로 키를 찾는다. 배열 데이터 파일에서 쓴다.
    pub fn from_qwerty(c: char) -> Option<Key> {
        if c.is_ascii_lowercase() {
            return Some(Key(Self::A.0 + (c as u16 - 'a' as u16)));
        }
        QWERTY_SYMBOLS.iter().find(|&&(_, base, _)| base == c).map(|&(code, _, _)| Key(code))
    }

    /// 미국 쿼티 배열에서 이 키가 내는 글자. Caps Lock은 따지지 않는다.
    pub fn qwerty_char(self, shift: bool) -> Option<char> {
        if self.is_letter() {
            let c = (b'a' + (self.0 - Self::A.0) as u8) as char;
            return Some(if shift { c.to_ascii_uppercase() } else { c });
        }
        QWERTY_SYMBOLS
            .iter()
            .find(|&&(code, _, _)| code == self.0)
            .map(|&(_, base, shifted)| if shift { shifted } else { base })
    }

    /// 맥 가상 키코드(kVK_*)를 물리 키로 바꾼다.
    pub fn from_mac_keycode(code: u16) -> Key {
        Key(match code {
            0x00 => 0x04,   // A
            0x01 => 0x16,   // S
            0x02 => 0x07,   // D
            0x03 => 0x09,   // F
            0x04 => 0x0B,   // H
            0x05 => 0x0A,   // G
            0x06 => 0x1D,   // Z
            0x07 => 0x1B,   // X
            0x08 => 0x06,   // C
            0x09 => 0x19,   // V
            0x0B => 0x05,   // B
            0x0C => 0x14,   // Q
            0x0D => 0x1A,   // W
            0x0E => 0x08,   // E
            0x0F => 0x15,   // R
            0x10 => 0x1C,   // Y
            0x11 => 0x17,   // T
            0x12 => 0x1E,   // 1
            0x13 => 0x1F,   // 2
            0x14 => 0x20,   // 3
            0x15 => 0x21,   // 4
            0x16 => 0x23,   // 6
            0x17 => 0x22,   // 5
            0x18 => 0x2E,   // =
            0x19 => 0x26,   // 9
            0x1A => 0x24,   // 7
            0x1B => 0x2D,   // -
            0x1C => 0x25,   // 8
            0x1D => 0x27,   // 0
            0x1E => 0x30,   // ]
            0x1F => 0x12,   // O
            0x20 => 0x18,   // U
            0x21 => 0x2F,   // [
            0x22 => 0x0C,   // I
            0x23 => 0x13,   // P
            0x24 => 0x28,   // Return
            0x25 => 0x0F,   // L
            0x26 => 0x0D,   // J
            0x27 => 0x34,   // '
            0x28 => 0x0E,   // K
            0x29 => 0x33,   // ;
            0x2A => 0x31,   // \
            0x2B => 0x36,   // ,
            0x2C => 0x38,   // /
            0x2D => 0x11,   // N
            0x2E => 0x10,   // M
            0x2F => 0x37,   // .
            0x30 => 0x2B,   // Tab
            0x31 => 0x2C,   // Space
            0x32 => 0x35,   // `
            0x33 => 0x2A,   // Delete (Backspace)
            0x35 => 0x29,   // Escape
            0x36 => 0xE7,   // Right Command
            0x37 => 0xE3,   // Command
            0x38 => 0xE1,   // Shift
            0x39 => 0x39,   // Caps Lock
            0x3A => 0xE2,   // Option
            0x3B => 0xE0,   // Control
            0x3C => 0xE5,   // Right Shift
            0x3D => 0xE6,   // Right Option
            0x3E => 0xE4,   // Right Control
            0x3F => 0x0100, // Fn
            0x40 => 0x6C,   // F17
            0x41 => 0x63,   // Keypad .
            0x43 => 0x55,   // Keypad *
            0x45 => 0x57,   // Keypad +
            0x47 => 0x53,   // Keypad Clear
            0x4B => 0x54,   // Keypad /
            0x4C => 0x58,   // Keypad Enter
            0x4E => 0x56,   // Keypad -
            0x4F => 0x6D,   // F18
            0x50 => 0x6E,   // F19
            0x51 => 0x67,   // Keypad =
            0x52 => 0x62,   // Keypad 0
            0x53 => 0x59,   // Keypad 1
            0x54 => 0x5A,   // Keypad 2
            0x55 => 0x5B,   // Keypad 3
            0x56 => 0x5C,   // Keypad 4
            0x57 => 0x5D,   // Keypad 5
            0x58 => 0x5E,   // Keypad 6
            0x59 => 0x5F,   // Keypad 7
            0x5A => 0x6F,   // F20
            0x5B => 0x60,   // Keypad 8
            0x5C => 0x61,   // Keypad 9
            0x60 => 0x3E,   // F5
            0x61 => 0x3F,   // F6
            0x62 => 0x40,   // F7
            0x63 => 0x3C,   // F3
            0x64 => 0x41,   // F8
            0x65 => 0x42,   // F9
            0x67 => 0x44,   // F11
            0x69 => 0x68,   // F13
            0x6A => 0x6B,   // F16
            0x6B => 0x69,   // F14
            0x6D => 0x43,   // F10
            0x6F => 0x45,   // F12
            0x71 => 0x6A,   // F15
            0x72 => 0x75,   // Help
            0x73 => 0x4A,   // Home
            0x74 => 0x4B,   // Page Up
            0x75 => 0x4C,   // Forward Delete
            0x76 => 0x3D,   // F4
            0x77 => 0x4D,   // End
            0x78 => 0x3B,   // F2
            0x79 => 0x4E,   // Page Down
            0x7A => 0x3A,   // F1
            0x7B => 0x50,   // Left
            0x7C => 0x4F,   // Right
            0x7D => 0x51,   // Down
            0x7E => 0x52,   // Up
            _ => 0,
        })
    }

    /// 윈도우 스캔 코드(키 메시지 lParam의 16~23비트, set 1)와 확장 비트(lParam 24비트, E0 접두)를
    /// 물리 키로 바꾼다. NumLock은 확장, Pause는 비확장 0x45로 온다(윈도우 키보드 드라이버 규칙).
    /// 미디어 키처럼 키보드 페이지에 없는 키는 [`Key::UNKNOWN`]이다.
    pub fn from_windows_scancode(scan: u16, extended: bool) -> Key {
        Key(if extended {
            match scan {
                0x1C => 0x58, // Keypad Enter
                0x1D => 0xE4, // Right Control
                0x35 => 0x54, // Keypad /
                0x37 => 0x46, // Print Screen
                0x38 => 0xE6, // Right Alt
                0x45 => 0x53, // Num Lock
                0x47 => 0x4A, // Home
                0x48 => 0x52, // Up
                0x49 => 0x4B, // Page Up
                0x4B => 0x50, // Left
                0x4D => 0x4F, // Right
                0x4F => 0x4D, // End
                0x50 => 0x51, // Down
                0x51 => 0x4E, // Page Down
                0x52 => 0x49, // Insert
                0x53 => 0x4C, // Delete
                0x5B => 0xE3, // Left Windows
                0x5C => 0xE7, // Right Windows
                0x5D => 0x65, // Application (메뉴)
                _ => 0,
            }
        } else {
            match scan {
                0x01 => 0x29, // Escape
                0x02 => 0x1E, // 1
                0x03 => 0x1F, // 2
                0x04 => 0x20, // 3
                0x05 => 0x21, // 4
                0x06 => 0x22, // 5
                0x07 => 0x23, // 6
                0x08 => 0x24, // 7
                0x09 => 0x25, // 8
                0x0A => 0x26, // 9
                0x0B => 0x27, // 0
                0x0C => 0x2D, // -
                0x0D => 0x2E, // =
                0x0E => 0x2A, // Backspace
                0x0F => 0x2B, // Tab
                0x10 => 0x14, // Q
                0x11 => 0x1A, // W
                0x12 => 0x08, // E
                0x13 => 0x15, // R
                0x14 => 0x17, // T
                0x15 => 0x1C, // Y
                0x16 => 0x18, // U
                0x17 => 0x0C, // I
                0x18 => 0x12, // O
                0x19 => 0x13, // P
                0x1A => 0x2F, // [
                0x1B => 0x30, // ]
                0x1C => 0x28, // Enter
                0x1D => 0xE0, // Left Control
                0x1E => 0x04, // A
                0x1F => 0x16, // S
                0x20 => 0x07, // D
                0x21 => 0x09, // F
                0x22 => 0x0A, // G
                0x23 => 0x0B, // H
                0x24 => 0x0D, // J
                0x25 => 0x0E, // K
                0x26 => 0x0F, // L
                0x27 => 0x33, // ;
                0x28 => 0x34, // '
                0x29 => 0x35, // `
                0x2A => 0xE1, // Left Shift
                0x2B => 0x31, // \
                0x2C => 0x1D, // Z
                0x2D => 0x1B, // X
                0x2E => 0x06, // C
                0x2F => 0x19, // V
                0x30 => 0x05, // B
                0x31 => 0x11, // N
                0x32 => 0x10, // M
                0x33 => 0x36, // ,
                0x34 => 0x37, // .
                0x35 => 0x38, // /
                0x36 => 0xE5, // Right Shift
                0x37 => 0x55, // Keypad *
                0x38 => 0xE2, // Left Alt
                0x39 => 0x2C, // Space
                0x3A => 0x39, // Caps Lock
                0x3B => 0x3A, // F1
                0x3C => 0x3B, // F2
                0x3D => 0x3C, // F3
                0x3E => 0x3D, // F4
                0x3F => 0x3E, // F5
                0x40 => 0x3F, // F6
                0x41 => 0x40, // F7
                0x42 => 0x41, // F8
                0x43 => 0x42, // F9
                0x44 => 0x43, // F10
                0x45 => 0x48, // Pause
                0x46 => 0x47, // Scroll Lock
                0x47 => 0x5F, // Keypad 7
                0x48 => 0x60, // Keypad 8
                0x49 => 0x61, // Keypad 9
                0x4A => 0x56, // Keypad -
                0x4B => 0x5C, // Keypad 4
                0x4C => 0x5D, // Keypad 5
                0x4D => 0x5E, // Keypad 6
                0x4E => 0x57, // Keypad +
                0x4F => 0x59, // Keypad 1
                0x50 => 0x5A, // Keypad 2
                0x51 => 0x5B, // Keypad 3
                0x52 => 0x62, // Keypad 0
                0x53 => 0x63, // Keypad .
                0x56 => 0x64, // 102번째 키(ISO \)
                0x57 => 0x44, // F11
                0x58 => 0x45, // F12
                0x59 => 0x67, // Keypad =
                0x64 => 0x68, // F13
                0x65 => 0x69, // F14
                0x66 => 0x6A, // F15
                0x67 => 0x6B, // F16
                0x68 => 0x6C, // F17
                0x69 => 0x6D, // F18
                0x6A => 0x6E, // F19
                0x6B => 0x6F, // F20
                0x6C => 0x70, // F21
                0x6D => 0x71, // F22
                0x6E => 0x72, // F23
                0x76 => 0x73, // F24
                _ => 0,
            }
        })
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.qwerty_char(false) {
            Some(c) => write!(f, "Key({c:?})"),
            None => write!(f, "Key(0x{:02X})", self.0),
        }
    }
}

/// 글자 이외에 글자를 내는 쿼티 키: (HID 코드, 기본, Shift).
const QWERTY_SYMBOLS: &[(u16, char, char)] = &[
    (0x1E, '1', '!'),
    (0x1F, '2', '@'),
    (0x20, '3', '#'),
    (0x21, '4', '$'),
    (0x22, '5', '%'),
    (0x23, '6', '^'),
    (0x24, '7', '&'),
    (0x25, '8', '*'),
    (0x26, '9', '('),
    (0x27, '0', ')'),
    (0x2D, '-', '_'),
    (0x2E, '=', '+'),
    (0x2F, '[', '{'),
    (0x30, ']', '}'),
    (0x31, '\\', '|'),
    (0x33, ';', ':'),
    (0x34, '\'', '"'),
    (0x35, '`', '~'),
    (0x36, ',', '<'),
    (0x37, '.', '>'),
    (0x38, '/', '?'),
];

/// 수식키 상태. 좌우를 구분한다.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods(pub u32);

impl Mods {
    pub const SHIFT_L: u32 = 1 << 0;
    pub const SHIFT_R: u32 = 1 << 1;
    pub const CTRL_L: u32 = 1 << 2;
    pub const CTRL_R: u32 = 1 << 3;
    pub const ALT_L: u32 = 1 << 4;
    pub const ALT_R: u32 = 1 << 5;
    pub const META_L: u32 = 1 << 6;
    pub const META_R: u32 = 1 << 7;
    /// Caps Lock 켜짐(토글 상태). 누르고 있는 수식키가 아니다.
    pub const CAPS: u32 = 1 << 8;
    pub const FN: u32 = 1 << 9;

    const HELD: u32 = Self::SHIFT_L
        | Self::SHIFT_R
        | Self::CTRL_L
        | Self::CTRL_R
        | Self::ALT_L
        | Self::ALT_R
        | Self::META_L
        | Self::META_R
        | Self::FN;

    pub fn shift(self) -> bool {
        self.0 & (Self::SHIFT_L | Self::SHIFT_R) != 0
    }
    pub fn ctrl(self) -> bool {
        self.0 & (Self::CTRL_L | Self::CTRL_R) != 0
    }
    pub fn alt(self) -> bool {
        self.0 & (Self::ALT_L | Self::ALT_R) != 0
    }
    pub fn meta(self) -> bool {
        self.0 & (Self::META_L | Self::META_R) != 0
    }
    pub fn caps(self) -> bool {
        self.0 & Self::CAPS != 0
    }
    /// ⌘/Ctrl/Option(Alt) 중 하나라도 눌려 있는지. 단축키 판정에 쓴다.
    pub fn command_like(self) -> bool {
        self.ctrl() || self.alt() || self.meta()
    }
    /// 누르고 있는 수식키가 있는지(Caps Lock 제외). `except` 비트는 빼고 본다.
    pub fn any_held_except(self, except: u32) -> bool {
        self.0 & Self::HELD & !except != 0
    }
}

impl fmt::Debug for Mods {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMES: [(u32, &str); 10] = [
            (Mods::SHIFT_L, "LShift"),
            (Mods::SHIFT_R, "RShift"),
            (Mods::CTRL_L, "LCtrl"),
            (Mods::CTRL_R, "RCtrl"),
            (Mods::ALT_L, "LAlt"),
            (Mods::ALT_R, "RAlt"),
            (Mods::META_L, "LMeta"),
            (Mods::META_R, "RMeta"),
            (Mods::CAPS, "Caps"),
            (Mods::FN, "Fn"),
        ];
        let names: Vec<&str> = NAMES.iter().filter(|(bit, _)| self.0 & bit != 0).map(|&(_, n)| n).collect();
        write!(f, "Mods[{}]", names.join("+"))
    }
}

/// 키 이벤트. 수식키도 눌림/뗌을 각각 한 이벤트로 받는다(맥 flagsChanged는 셸이 풀어서 보낸다).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    /// true면 눌림, false면 뗌.
    pub down: bool,
    /// 이 이벤트가 반영된 뒤의 수식키 상태.
    pub mods: Mods,
    /// 키 반복(오토리피트)인지.
    pub repeat: bool,
    /// 이벤트 시각(초, 단조 증가). 맥은 NSEvent.timestamp를 그대로 쓴다.
    pub time: f64,
}

impl KeyEvent {
    pub fn down(key: Key, mods: Mods, time: f64) -> Self {
        Self { key, down: true, mods, repeat: false, time }
    }
    pub fn up(key: Key, mods: Mods, time: f64) -> Self {
        Self { key, down: false, mods, repeat: false, time }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwerty_round_trip() {
        for c in "abcdefghijklmnopqrstuvwxyz1234567890-=[]\\;'`,./".chars() {
            let k = Key::from_qwerty(c).unwrap_or_else(|| panic!("no key for {c:?}"));
            assert_eq!(k.qwerty_char(false), Some(c));
        }
        assert_eq!(Key::from_qwerty('A'), None);
        assert_eq!(Key::SLASH.qwerty_char(true), Some('?'));
        assert_eq!(Key::Q.qwerty_char(true), Some('Q'));
    }

    #[test]
    fn mac_keycodes_cover_every_printable_key() {
        let mut seen = std::collections::HashSet::new();
        for code in 0..=0x7Fu16 {
            let k = Key::from_mac_keycode(code);
            if k.is_printable() {
                assert!(seen.insert(k), "duplicate mapping for {k:?}");
            }
        }
        assert_eq!(seen.len(), 26 + 21);
        assert_eq!(Key::from_mac_keycode(0x00), Key::A);
        assert_eq!(Key::from_mac_keycode(0x3C), Key::SHIFT_RIGHT);
        assert_eq!(Key::from_mac_keycode(0x33), Key::BACKSPACE);
    }

    #[test]
    fn windows_scancodes_cover_every_printable_key() {
        let mut seen = std::collections::HashSet::new();
        for extended in [false, true] {
            for scan in 0..=0xFFu16 {
                let k = Key::from_windows_scancode(scan, extended);
                if k.is_printable() {
                    assert!(!extended, "printable key {k:?} behind the E0 prefix");
                    assert!(seen.insert(k), "duplicate mapping for {k:?}");
                }
            }
        }
        assert_eq!(seen.len(), 26 + 21);
        assert_eq!(Key::from_windows_scancode(0x10, false), Key::Q);
        assert_eq!(Key::from_windows_scancode(0x0E, false), Key::BACKSPACE);
    }

    #[test]
    fn windows_scancodes_tell_left_from_right_and_keypad_from_arrows() {
        let sc = Key::from_windows_scancode;
        assert_eq!((sc(0x2A, false), sc(0x36, false)), (Key::SHIFT_LEFT, Key::SHIFT_RIGHT));
        assert_eq!((sc(0x1D, false), sc(0x1D, true)), (Key::CONTROL_LEFT, Key::CONTROL_RIGHT));
        assert_eq!((sc(0x38, false), sc(0x38, true)), (Key::ALT_LEFT, Key::ALT_RIGHT));
        assert_eq!((sc(0x5B, true), sc(0x5C, true)), (Key::META_LEFT, Key::META_RIGHT));
        assert_eq!((sc(0x1C, false), sc(0x1C, true)), (Key::ENTER, Key::NUMPAD_ENTER));
        assert_eq!(sc(0x4B, true), Key::ARROW_LEFT);
        assert_ne!(sc(0x4B, false), Key::ARROW_LEFT); // 숫자패드 4
        assert_eq!((sc(0x53, true), sc(0x47, true)), (Key::DELETE, Key::HOME));
        assert_eq!(sc(0x3A, false), Key::CAPS_LOCK);
        assert_eq!(sc(0x20, true), Key::UNKNOWN); // 음소거(미디어 키)
    }

    #[test]
    fn mods_helpers() {
        let m = Mods(Mods::SHIFT_R | Mods::CAPS);
        assert!(m.shift() && m.caps() && !m.command_like());
        assert!(!m.any_held_except(Mods::SHIFT_R));
        assert!(m.any_held_except(Mods::SHIFT_L));
    }
}
