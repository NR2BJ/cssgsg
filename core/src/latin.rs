//! 영어: 쿼티 물리 키를 Graphite 글자로 바꾼다.
//!
//! OS 레이아웃은 쿼티(맥 ABC, 윈도우 US) 그대로 둔다. 입력기가 영어 모드에서 글자만 바꿔 넣고,
//! ⌘/Ctrl/Option 조합은 건드리지 않는다. 그래서 단축키는 어디서나 쿼티 자리다.

use std::collections::HashMap;

use serde::Deserialize;

use crate::key::Key;

const GRAPHITE: &str = include_str!("../../layouts/en/graphite.toml");

/// 쿼티 물리 자리 순서. graphite.toml의 줄과 같은 순서다.
const QWERTY_ROWS: [&str; 4] = ["`1234567890-=", "qwertyuiop[]\\", "asdfghjkl;'", "zxcvbnm,./"];

#[derive(Debug, Clone)]
pub struct LatinLayout {
    pub id: String,
    pub name: String,
    map: HashMap<Key, (char, char)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    meta: RawMeta,
    layout: RawRows,
}

#[derive(Deserialize)]
struct RawMeta {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRows {
    rows: Vec<String>,
    shift_rows: Vec<String>,
}

impl LatinLayout {
    pub fn graphite() -> Self {
        Self::from_toml(GRAPHITE).expect("내장 Graphite 데이터는 항상 읽혀야 한다")
    }

    pub fn from_toml(src: &str) -> Result<Self, String> {
        let raw: Raw = toml::from_str(src).map_err(|e| e.to_string())?;
        let rows = &raw.layout;
        if rows.rows.len() != QWERTY_ROWS.len() || rows.shift_rows.len() != QWERTY_ROWS.len() {
            return Err("rows와 shift_rows는 4줄이어야 한다".into());
        }
        let mut map = HashMap::new();
        for (i, qwerty) in QWERTY_ROWS.iter().enumerate() {
            let base: Vec<char> = rows.rows[i].chars().collect();
            let shift: Vec<char> = rows.shift_rows[i].chars().collect();
            if base.len() != qwerty.chars().count() || shift.len() != base.len() {
                return Err(format!("{}번째 줄 글자 수가 쿼티 줄과 다르다", i + 1));
            }
            for (j, q) in qwerty.chars().enumerate() {
                let key = Key::from_qwerty(q).expect("쿼티 줄 글자는 항상 키가 있다");
                map.insert(key, (base[j], shift[j]));
            }
        }
        Ok(Self { id: raw.meta.id, name: raw.meta.name, map })
    }

    /// 이 키가 낼 글자. `shift_inverts_caps`가 true면 Caps Lock과 Shift가 서로 뒤집는다(윈도우 방식).
    pub fn char_for(&self, key: Key, shift: bool, caps: bool, shift_inverts_caps: bool) -> Option<char> {
        let &(base, shifted) = self.map.get(&key)?;
        let c = if shift { shifted } else { base };
        Some(apply_caps(c, shift, caps, shift_inverts_caps))
    }
}

/// 쿼티(OS 레이아웃)가 이 키로 낼 글자. 입력기가 안 끼었을 때 앱이 받을 글자다.
pub fn qwerty_char_for(key: Key, shift: bool, caps: bool, shift_inverts_caps: bool) -> Option<char> {
    key.qwerty_char(shift).map(|c| apply_caps(c, shift, caps, shift_inverts_caps))
}

/// Caps Lock은 글자에만 적용된다.
fn apply_caps(c: char, shift: bool, caps: bool, shift_inverts_caps: bool) -> char {
    if !caps || !c.is_ascii_alphabetic() {
        return c;
    }
    let upper = if shift_inverts_caps { !shift } else { true };
    if upper { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(q: char, shift: bool) -> char {
        LatinLayout::graphite().char_for(Key::from_qwerty(q).unwrap(), shift, false, false).unwrap()
    }

    #[test]
    fn graphite_rows() {
        let typed: String = "qwertyuiop".chars().map(|q| g(q, false)).collect();
        assert_eq!(typed, "bldwz'fouj");
        let typed: String = "asdfghjkl;'".chars().map(|q| g(q, false)).collect();
        assert_eq!(typed, "nrtsgyhaei,");
        assert_eq!(g('/', true), '<');
        assert_eq!(g('.', true), '"');
        assert_eq!(g('\'', true), '?');
        assert_eq!(g('-', false), '[');
        assert_eq!(g('g', false), 'g');
    }

    #[test]
    fn caps_lock_applies_to_letters_only() {
        let l = LatinLayout::graphite();
        assert_eq!(l.char_for(Key::A, false, true, false), Some('N'));
        assert_eq!(l.char_for(Key::A, true, true, false), Some('N'));
        assert_eq!(l.char_for(Key::A, true, true, true), Some('n'));
        assert_eq!(l.char_for(Key::QUOTE, false, true, false), Some(','));
    }
}
