//! 단축키: 수식키 탭(`"tap:shift_right"`)과 수식키+키 조합(`"alt+enter"`). 설정 파일과 설정 앱이 같은 글자열을 쓴다.
//!
//! - 탭은 수식키 하나를 혼자 짧게 눌렀다 떼는 것이다(좌우를 가린다, [`crate::hotkey::TapTracker`]).
//! - 조합은 수식키 종류(control, alt, shift, meta)와 키 하나다. 수식키는 좌우를 가리지 않고, 종류가 정확히 같아야 한다.
//!   NRIME는 조합도 좌우를 가려서 기본값(왼쪽 Option+Return)이 오른쪽 Option으로는 안 됐다.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::key::{Key, KeyEvent, Mods};

/// 단축키로 하는 일.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    /// 영어 ↔ 방금 쓰던 비영어.
    ToggleEnglish,
    /// 한국어 ↔ 일본어.
    ToggleNonEnglish,
    /// 한국어 한자 변환(조합 중인 글자).
    Hanja,
}

/// 조합 단축키의 수식키 종류(좌우 없음).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModFamilies(u8);

impl ModFamilies {
    pub const CONTROL: u8 = 1;
    pub const ALT: u8 = 2;
    pub const SHIFT: u8 = 4;
    pub const META: u8 = 8;
    /// 파일에 적는 순서와 이름.
    const NAMES: [(u8, &'static str); 4] =
        [(Self::CONTROL, "control"), (Self::ALT, "alt"), (Self::SHIFT, "shift"), (Self::META, "meta")];

    pub fn of(mods: Mods) -> Self {
        let mut bits = 0;
        if mods.ctrl() {
            bits |= Self::CONTROL;
        }
        if mods.alt() {
            bits |= Self::ALT;
        }
        if mods.shift() {
            bits |= Self::SHIFT;
        }
        if mods.meta() {
            bits |= Self::META;
        }
        Self(bits)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, bit: u8) -> bool {
        self.0 & bit != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// 단축키 하나.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shortcut {
    #[default]
    None,
    /// 수식키 하나를 혼자 짧게 누르기(좌우를 가린다).
    Tap(Key),
    /// 수식키 종류 + 수식키가 아닌 키.
    Combo(ModFamilies, Key),
}

impl Shortcut {
    /// 설정 글자열을 읽는다. 빈 글자열이나 "none"은 없음.
    pub fn parse(text: &str) -> Result<Shortcut, String> {
        let text = text.trim();
        if text.is_empty() || text == "none" {
            return Ok(Shortcut::None);
        }
        if let Some(name) = text.strip_prefix("tap:") {
            return modifier_from_name(name)
                .map(Shortcut::Tap)
                .ok_or_else(|| format!("탭 키는 수식키여야 한다: {name:?}"));
        }
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (&key_name, mod_names) = parts.split_last().ok_or("빈 단축키")?;
        let mut bits = 0;
        for name in mod_names {
            let bit = ModFamilies::NAMES
                .iter()
                .find(|(_, n)| n == name)
                .map(|&(b, _)| b)
                .ok_or_else(|| format!("모르는 수식키 {name:?} (control, alt, shift, meta)"))?;
            bits |= bit;
        }
        let key = key_from_name(key_name).ok_or_else(|| format!("모르는 키 이름 {key_name:?}"))?;
        Ok(Shortcut::Combo(ModFamilies(bits), key))
    }

    /// 값이 쓸 만한지. 수식키 없이 글자·숫자·Space·Return 같은 키만 두면 그 키를 칠 수 없게 된다.
    pub fn validate(self) -> Result<(), String> {
        if let Shortcut::Combo(mods, key) = self
            && mods.is_empty()
            && (key.is_printable()
                || matches!(key, Key::SPACE | Key::ENTER | Key::TAB | Key::BACKSPACE | Key::ESCAPE))
        {
            return Err(format!("수식키 없이 {self}만 단축키로 쓸 수 없다(그 키를 칠 수 없게 된다)"));
        }
        Ok(())
    }

    /// 이 키 눌림이 조합 단축키인지(수식키 종류가 정확히 같아야 한다. Caps Lock·Fn은 보지 않는다).
    pub fn matches_combo(self, ev: &KeyEvent) -> bool {
        match self {
            Shortcut::Combo(mods, key) => ev.down && ev.key == key && ModFamilies::of(ev.mods) == mods,
            _ => false,
        }
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Shortcut::None => Ok(()),
            Shortcut::Tap(key) => write!(f, "tap:{}", modifier_name(key).unwrap_or("?")),
            Shortcut::Combo(mods, key) => {
                for (bit, name) in ModFamilies::NAMES {
                    if mods.contains(bit) {
                        write!(f, "{name}+")?;
                    }
                }
                f.write_str(key_name(key).unwrap_or("?"))
            }
        }
    }
}

impl Serialize for Shortcut {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Shortcut {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Shortcut::parse(&text).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------- 키 이름

/// 탭에 쓰는 수식키 이름(설정 파일과 설정 앱이 쓰는 순서).
pub const MODIFIER_NAMES: [&str; 8] = [
    "shift_left",
    "shift_right",
    "control_left",
    "control_right",
    "alt_left",
    "alt_right",
    "meta_left",
    "meta_right",
];

const MODIFIERS: [(&str, Key); 8] = [
    ("shift_left", Key::SHIFT_LEFT),
    ("shift_right", Key::SHIFT_RIGHT),
    ("control_left", Key::CONTROL_LEFT),
    ("control_right", Key::CONTROL_RIGHT),
    ("alt_left", Key::ALT_LEFT),
    ("alt_right", Key::ALT_RIGHT),
    ("meta_left", Key::META_LEFT),
    ("meta_right", Key::META_RIGHT),
];

pub fn modifier_from_name(name: &str) -> Option<Key> {
    MODIFIERS.iter().find(|(n, _)| *n == name).map(|&(_, k)| k)
}

pub fn modifier_name(key: Key) -> Option<&'static str> {
    MODIFIERS.iter().find(|(_, k)| *k == key).map(|&(n, _)| n)
}

/// 이름 있는 키(글자·숫자·F키 말고). HID 값.
const NAMED: [(&str, u16); 25] = [
    ("enter", 0x28),
    ("escape", 0x29),
    ("backspace", 0x2A),
    ("tab", 0x2B),
    ("space", 0x2C),
    ("minus", 0x2D),
    ("equal", 0x2E),
    ("bracket_left", 0x2F),
    ("bracket_right", 0x30),
    ("backslash", 0x31),
    ("semicolon", 0x33),
    ("quote", 0x34),
    ("grave", 0x35),
    ("comma", 0x36),
    ("period", 0x37),
    ("slash", 0x38),
    ("home", 0x4A),
    ("page_up", 0x4B),
    ("delete", 0x4C),
    ("end", 0x4D),
    ("page_down", 0x4E),
    ("right", 0x4F),
    ("left", 0x50),
    ("down", 0x51),
    ("up", 0x52),
];

/// 설정 파일의 키 이름(수식키가 아닌 키). 모르는 키면 None.
pub fn key_name(key: Key) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t",
        "u", "v", "w", "x", "y", "z",
    ];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    const FKEYS: [&str; 20] = [
        "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12", "f13", "f14", "f15",
        "f16", "f17", "f18", "f19", "f20",
    ];
    let k = key.0;
    match k {
        0x04..=0x1D => Some(LETTERS[(k - 0x04) as usize]),
        0x1E..=0x27 => Some(DIGITS[(k - 0x1E) as usize]),
        0x3A..=0x45 => Some(FKEYS[(k - 0x3A) as usize]),
        0x68..=0x6F => Some(FKEYS[(k - 0x68 + 12) as usize]),
        0x58 => Some("keypad_enter"),
        _ => NAMED.iter().find(|&&(_, code)| code == k).map(|&(n, _)| n),
    }
}

pub fn key_from_name(name: &str) -> Option<Key> {
    (0x04..=0x6Fu16).map(Key).find(|&k| key_name(k) == Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints() {
        for text in [
            "tap:shift_right",
            "alt+enter",
            "control+shift+space",
            "meta+f5",
            "control+alt+shift+meta+k",
            "f13",
        ] {
            let s = Shortcut::parse(text).unwrap();
            assert_eq!(s.to_string(), text, "되돌아간다");
        }
        assert_eq!(Shortcut::parse("").unwrap(), Shortcut::None);
        assert_eq!(Shortcut::parse("none").unwrap(), Shortcut::None);
        // 수식키 이름 순서는 control, alt, shift, meta로 맞춘다
        assert_eq!(Shortcut::parse("shift+control+space").unwrap().to_string(), "control+shift+space");
        assert!(Shortcut::parse("tap:a").is_err());
        assert!(Shortcut::parse("hyper+a").is_err());
        assert!(Shortcut::parse("alt+enterr").is_err());
    }

    #[test]
    fn plain_typing_keys_are_rejected() {
        assert!(Shortcut::parse("a").unwrap().validate().is_err());
        assert!(Shortcut::parse("space").unwrap().validate().is_err());
        assert!(Shortcut::parse("enter").unwrap().validate().is_err());
        assert!(Shortcut::parse("f13").unwrap().validate().is_ok());
        assert!(Shortcut::parse("alt+a").unwrap().validate().is_ok());
    }

    #[test]
    fn combos_match_either_side_but_exact_families() {
        let hanja = Shortcut::parse("alt+enter").unwrap();
        let ev = |mods| KeyEvent::down(Key::ENTER, Mods(mods), 1.0);
        assert!(hanja.matches_combo(&ev(Mods::ALT_L)));
        assert!(hanja.matches_combo(&ev(Mods::ALT_R)), "오른쪽 Option도 된다");
        assert!(hanja.matches_combo(&ev(Mods::ALT_L | Mods::CAPS)), "Caps Lock은 보지 않는다");
        assert!(!hanja.matches_combo(&ev(Mods::ALT_L | Mods::SHIFT_L)), "Shift가 더 있으면 다른 단축키");
        assert!(!hanja.matches_combo(&ev(0)));
        assert!(!hanja.matches_combo(&KeyEvent::up(Key::ENTER, Mods(Mods::ALT_L), 1.0)));
    }

    #[test]
    fn every_mac_key_has_a_round_trip_name() {
        for code in 0..0x80u16 {
            let key = Key::from_mac_keycode(code);
            if let Some(name) = key_name(key) {
                assert_eq!(key_from_name(name), Some(key), "{name}");
            }
        }
        assert_eq!(key_name(Key::from_mac_keycode(0x24)), Some("enter"));
        assert_eq!(key_name(Key::from_mac_keycode(0x69)), Some("f13"));
        assert_eq!(key_name(Key::SHIFT_LEFT), None, "수식키는 키 이름이 없다(탭으로 쓴다)");
    }
}
