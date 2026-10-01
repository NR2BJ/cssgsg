//! 단축키: 수식키 탭(`"tap:shift_right"`)과 수식키+키 조합(`"alt_left+enter"`). 설정 파일과 설정 앱이 같은 글자열을 쓴다.
//!
//! - 탭은 수식키 하나를 혼자 짧게 눌렀다 떼는 것이다(좌우를 가린다, [`crate::hotkey::TapTracker`]).
//! - 조합은 수식키와 키 하나다. 수식키는 좌우를 가린다(`alt_left`, NRIME와 같다, 2026-09-29 사용자 결정).
//!   손으로 적을 때는 좌우를 가리지 않는 이름(`alt`)도 쓸 수 있다. 설정 앱 녹화는 늘 좌우를 가려 적는다.
//! - ⌘(meta) 조합은 쓸 수 없다. Chrome·Discord 같은 앱은 ⌘+키를 입력기에 보내지 않고 먼저 가져간다.

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

/// 수식키 종류: 파일에 적는 이름과 순서, 왼쪽·오른쪽 비트(`Mods`).
const FAMILIES: [(&str, u32, u32); 4] = [
    ("control", Mods::CTRL_L, Mods::CTRL_R),
    ("alt", Mods::ALT_L, Mods::ALT_R),
    ("shift", Mods::SHIFT_L, Mods::SHIFT_R),
    ("meta", Mods::META_L, Mods::META_R),
];

/// 조합 단축키의 수식키. 종류마다 없음, 왼쪽, 오른쪽, 양쪽, 아무 쪽(좌우 안 가림) 중 하나다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComboMods {
    /// 좌우를 가린 수식키(`Mods`의 SHIFT_L … META_R 비트). 그 종류는 정확히 이 쪽만 눌려 있어야 한다.
    sides: u32,
    /// 좌우를 가리지 않는 종류(그 종류의 두 쪽 비트). 어느 쪽이든 눌려 있으면 된다.
    either: u32,
}

impl ComboMods {
    /// 이 수식키 상태(`Mods`)가 맞는지. 종류마다 보고, Caps Lock·Fn은 보지 않는다.
    pub fn matches(self, mods: Mods) -> bool {
        FAMILIES.iter().all(|&(_, left, right)| {
            let pair = left | right;
            let held = mods.0 & pair;
            if self.either & pair != 0 { held != 0 } else { held == self.sides & pair }
        })
    }

    pub fn is_empty(self) -> bool {
        self.sides == 0 && self.either == 0
    }

    /// 이 수식키(좌우를 가린 키)가 조합에 들어 있는지.
    pub fn uses(self, modifier: Key) -> bool {
        let bit = modifier.modifier_bit();
        bit != 0 && (self.sides & bit != 0 || self.either & family_pair(bit) != 0)
    }

    /// 이 수식키와 같은 종류(좌우 어느 쪽이든)를 쓰는지.
    pub fn uses_family_of(self, modifier: Key) -> bool {
        (self.sides | self.either) & family_pair(modifier.modifier_bit()) != 0
    }
}

/// 수식키 비트 하나 → 그 종류의 두 쪽 비트.
fn family_pair(bit: u32) -> u32 {
    FAMILIES.iter().map(|&(_, l, r)| l | r).find(|pair| pair & bit != 0).unwrap_or(0)
}

/// 단축키 하나.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shortcut {
    #[default]
    None,
    /// 수식키 하나를 혼자 짧게 누르기(좌우를 가린다).
    Tap(Key),
    /// 수식키 + 수식키가 아닌 키.
    Combo(ComboMods, Key),
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
                .ok_or_else(|| format!("탭 키는 수식키여야 합니다: {name:?}"));
        }
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (&key_name, mod_names) = parts.split_last().ok_or("빈 단축키입니다")?;
        let mut mods = ComboMods::default();
        for &name in mod_names {
            if let Some(&(_, left, right)) = FAMILIES.iter().find(|(n, _, _)| *n == name) {
                mods.either |= left | right;
            } else if let Some(key) = modifier_from_name(name) {
                mods.sides |= key.modifier_bit();
            } else {
                return Err(format!(
                    "알 수 없는 수식키입니다: {name:?} (control_left, alt_right … 또는 좌우를 가리지 않는 control, alt, shift, meta)"
                ));
            }
        }
        if mods.sides & mods.either != 0 {
            return Err(format!(
                "같은 수식키를 좌우를 가린 이름과 가리지 않은 이름으로 함께 적을 수 없습니다: {text:?}"
            ));
        }
        let key = key_from_name(key_name).ok_or_else(|| format!("알 수 없는 키 이름입니다: {key_name:?}"))?;
        Ok(Shortcut::Combo(mods, key))
    }

    /// 값이 쓸 만한지. 수식키 없이 글자·숫자·Space·Return 같은 키만 두면 그 키를 칠 수 없게 된다.
    /// ⌘ 조합은 Chrome·Discord 같은 앱이 입력기에 보내지 않아서 앱마다 되다 말다 한다.
    pub fn validate(self) -> Result<(), String> {
        let Shortcut::Combo(mods, key) = self else { return Ok(()) };
        if mods.is_empty()
            && (key.is_printable()
                || matches!(key, Key::SPACE | Key::ENTER | Key::TAB | Key::BACKSPACE | Key::ESCAPE))
        {
            return Err(format!(
                "수식키 없이 {self}만으로는 단축키를 만들 수 없습니다(그 키를 칠 수 없게 됩니다)"
            ));
        }
        if mods.uses_family_of(Key::META_LEFT) {
            return Err(format!(
                "⌘(meta) 조합은 쓸 수 없습니다: {self} (Chrome·Discord 같은 앱은 ⌘+키를 입력기에 보내지 않습니다)"
            ));
        }
        Ok(())
    }

    /// 이 키 눌림이 조합 단축키인지(수식키가 정확히 맞아야 한다. Caps Lock·Fn은 보지 않는다).
    pub fn matches_combo(self, ev: &KeyEvent) -> bool {
        match self {
            Shortcut::Combo(mods, key) => ev.down && ev.key == key && mods.matches(ev.mods),
            _ => false,
        }
    }

    /// 조합 단축키이고 이 수식키(좌우를 가린 키)를 쓰는지.
    pub fn combo_uses(self, modifier: Key) -> bool {
        matches!(self, Shortcut::Combo(mods, _) if mods.uses(modifier))
    }

    /// 이 수식키(좌우를 가린 키)를 누르는 것으로 시작하는지: 그 키의 탭이거나, 그 키를 쓰는 조합.
    pub fn starts_with(self, modifier: Key) -> bool {
        match self {
            Shortcut::Tap(key) => key == modifier,
            _ => self.combo_uses(modifier),
        }
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Shortcut::None => Ok(()),
            Shortcut::Tap(key) => write!(f, "tap:{}", modifier_name(key).unwrap_or("?")),
            Shortcut::Combo(mods, key) => {
                for (name, left, right) in FAMILIES {
                    if mods.either & (left | right) != 0 {
                        write!(f, "{name}+")?;
                    } else {
                        if mods.sides & left != 0 {
                            write!(f, "{name}_left+")?;
                        }
                        if mods.sides & right != 0 {
                            write!(f, "{name}_right+")?;
                        }
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
            "alt_left+enter",
            "alt+enter",
            "control_left+shift_right+space",
            "control_left+control_right+k",
            "control+alt_right+shift+k",
            "meta_left+f5",
            "f13",
        ] {
            let s = Shortcut::parse(text).unwrap();
            assert_eq!(s.to_string(), text, "되돌아간다");
        }
        assert_eq!(Shortcut::parse("").unwrap(), Shortcut::None);
        assert_eq!(Shortcut::parse("none").unwrap(), Shortcut::None);
        // 수식키 이름 순서는 control, alt, shift, meta(종류 안에서는 왼쪽, 오른쪽)로 맞춘다
        assert_eq!(
            Shortcut::parse("shift_right+control_left+space").unwrap().to_string(),
            "control_left+shift_right+space"
        );
        assert!(Shortcut::parse("tap:a").is_err());
        assert!(Shortcut::parse("hyper+a").is_err());
        assert!(Shortcut::parse("alt+enterr").is_err());
        assert!(
            Shortcut::parse("alt+alt_left+enter").is_err(),
            "같은 종류를 가려서도 안 가려서도 적으면 안 된다"
        );
    }

    #[test]
    fn plain_typing_keys_and_command_combos_are_rejected() {
        assert!(Shortcut::parse("a").unwrap().validate().is_err());
        assert!(Shortcut::parse("space").unwrap().validate().is_err());
        assert!(Shortcut::parse("enter").unwrap().validate().is_err());
        assert!(Shortcut::parse("f13").unwrap().validate().is_ok());
        assert!(Shortcut::parse("alt_left+a").unwrap().validate().is_ok());
        assert!(Shortcut::parse("meta_left+f5").unwrap().validate().is_err(), "⌘ 조합");
        assert!(Shortcut::parse("control+meta+k").unwrap().validate().is_err());
        assert!(Shortcut::parse("tap:meta_right").unwrap().validate().is_ok(), "⌘ 탭은 된다");
    }

    #[test]
    fn combos_match_the_exact_side() {
        let ev = |mods| KeyEvent::down(Key::ENTER, Mods(mods), 1.0);
        let left = Shortcut::parse("alt_left+enter").unwrap();
        assert!(left.matches_combo(&ev(Mods::ALT_L)));
        assert!(!left.matches_combo(&ev(Mods::ALT_R)), "오른쪽 Option은 다른 단축키");
        assert!(!left.matches_combo(&ev(Mods::ALT_L | Mods::ALT_R)), "양쪽을 같이 누르면 아니다");
        assert!(left.matches_combo(&ev(Mods::ALT_L | Mods::CAPS)), "Caps Lock은 보지 않는다");
        assert!(!left.matches_combo(&ev(Mods::ALT_L | Mods::SHIFT_L)), "Shift가 더 있으면 다른 단축키");
        assert!(!left.matches_combo(&ev(0)));
        assert!(!left.matches_combo(&KeyEvent::up(Key::ENTER, Mods(Mods::ALT_L), 1.0)));
        let both = Shortcut::parse("control_left+control_right+enter").unwrap();
        assert!(both.matches_combo(&ev(Mods::CTRL_L | Mods::CTRL_R)));
        assert!(!both.matches_combo(&ev(Mods::CTRL_L)));
        // 좌우를 가리지 않는 이름(손으로 적은 파일)
        let either = Shortcut::parse("alt+enter").unwrap();
        assert!(either.matches_combo(&ev(Mods::ALT_L)) && either.matches_combo(&ev(Mods::ALT_R)));
        assert!(either.matches_combo(&ev(Mods::ALT_L | Mods::ALT_R)));
        assert!(!either.matches_combo(&ev(Mods::ALT_L | Mods::CTRL_R)));
    }

    #[test]
    fn combo_uses_the_named_modifier_only() {
        let s = Shortcut::parse("control_left+enter").unwrap();
        assert!(s.combo_uses(Key::CONTROL_LEFT));
        assert!(!s.combo_uses(Key::CONTROL_RIGHT));
        assert!(!s.combo_uses(Key::ALT_LEFT));
        let either = Shortcut::parse("control+enter").unwrap();
        assert!(either.combo_uses(Key::CONTROL_LEFT) && either.combo_uses(Key::CONTROL_RIGHT));
        assert!(!Shortcut::Tap(Key::CONTROL_LEFT).combo_uses(Key::CONTROL_LEFT), "탭은 조합이 아니다");
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
