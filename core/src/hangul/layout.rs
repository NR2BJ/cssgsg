//! 한국어 배열 데이터(TOML) 읽기.

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

use super::{is_cho, is_jong, is_jung};
use crate::key::Key;

const CHAMSHIN_V18: &str = include_str!("../../../layouts/ko/chamshin-v18.toml");
const CHAMSHIN_D_V19: &str = include_str!("../../../layouts/ko/chamshin-d-v19.toml");

/// 키가 하는 일.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// 음절 조합 중지(❖): 지금 음절을 확정하고 아무것도 내지 않는다.
    Stop,
}

/// 한 키(와 Shift 상태)가 가진 역할 후보. 어떤 역할로 쓰일지는 조합기가 문맥으로 정한다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roles {
    pub cho: Option<char>,
    pub jung: Option<char>,
    pub jong: Option<char>,
    pub sym: Option<String>,
    pub action: Option<Action>,
}

#[derive(Debug)]
pub struct LayoutError(pub String);

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "한국어 배열 데이터 오류: {}", self.0)
    }
}

impl std::error::Error for LayoutError {}

/// 한국어 배열: 키별 역할과 조합표.
#[derive(Debug, Clone)]
pub struct KoLayout {
    pub id: String,
    pub name: String,
    base: HashMap<Key, Roles>,
    shift: HashMap<Key, Roles>,
    cho: HashMap<(char, char), char>,
    jung: HashMap<(char, char), char>,
    jong: HashMap<(char, char), char>,
}

impl KoLayout {
    /// 내장 배열 목록: (id, 이름).
    pub const BUILTIN: [(&'static str, &'static str); 2] =
        [("chamshin-v18", "참신세벌식"), ("chamshin-d-v19", "참신세벌식D")];

    pub fn builtin(id: &str) -> Option<Self> {
        let src = match id {
            "chamshin-v18" => CHAMSHIN_V18,
            "chamshin-d-v19" => CHAMSHIN_D_V19,
            _ => return None,
        };
        Some(Self::from_toml(src).expect("내장 한국어 배열은 항상 읽혀야 한다"))
    }

    pub fn from_toml(src: &str) -> Result<Self, LayoutError> {
        let raw: RawLayout = toml::from_str(src).map_err(|e| LayoutError(e.to_string()))?;
        Ok(Self {
            id: raw.meta.id,
            name: raw.meta.name,
            base: parse_keys(&raw.base)?,
            shift: parse_keys(&raw.shift)?,
            cho: parse_combos(&raw.combine.cho, is_cho)?,
            jung: parse_combos(&raw.combine.jung, is_jung)?,
            jong: parse_combos(&raw.combine.jong, is_jong)?,
        })
    }

    /// 키의 역할. 배열에 없는 키(쿼티 그대로 나가는 키)는 `None`.
    pub fn roles(&self, key: Key, shift: bool) -> Option<&Roles> {
        if shift { self.shift.get(&key) } else { self.base.get(&key) }
    }

    pub fn combine_cho(&self, a: char, b: char) -> Option<char> {
        self.cho.get(&(a, b)).copied()
    }
    pub fn combine_jung(&self, a: char, b: char) -> Option<char> {
        self.jung.get(&(a, b)).copied()
    }
    pub fn combine_jong(&self, a: char, b: char) -> Option<char> {
        self.jong.get(&(a, b)).copied()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayout {
    meta: RawMeta,
    base: HashMap<String, RawRoles>,
    #[serde(default)]
    shift: HashMap<String, RawRoles>,
    #[serde(default)]
    combine: RawCombine,
}

#[derive(Deserialize)]
struct RawMeta {
    id: String,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawRoles {
    cho: Option<String>,
    jung: Option<String>,
    jong: Option<String>,
    sym: Option<String>,
    action: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawCombine {
    #[serde(default)]
    cho: Vec<String>,
    #[serde(default)]
    jung: Vec<String>,
    #[serde(default)]
    jong: Vec<String>,
}

fn parse_keys(raw: &HashMap<String, RawRoles>) -> Result<HashMap<Key, Roles>, LayoutError> {
    let mut out = HashMap::new();
    for (name, r) in raw {
        let mut chars = name.chars();
        let key = match (chars.next(), chars.next()) {
            (Some(c), None) => Key::from_qwerty(c),
            _ => None,
        }
        .ok_or_else(|| LayoutError(format!("알 수 없는 키 이름 {name:?}")))?;
        let roles = Roles {
            cho: jamo(&r.cho, is_cho, name, "cho")?,
            jung: jamo(&r.jung, is_jung, name, "jung")?,
            jong: jamo(&r.jong, is_jong, name, "jong")?,
            sym: r.sym.clone(),
            action: match r.action.as_deref() {
                None => None,
                Some("stop") => Some(Action::Stop),
                Some(other) => return Err(LayoutError(format!("{name}: 알 수 없는 action {other:?}"))),
            },
        };
        if roles == Roles::default() {
            return Err(LayoutError(format!("{name}: 역할이 하나도 없다")));
        }
        out.insert(key, roles);
    }
    Ok(out)
}

fn jamo(
    v: &Option<String>,
    valid: fn(char) -> bool,
    key: &str,
    field: &str,
) -> Result<Option<char>, LayoutError> {
    let Some(s) = v else { return Ok(None) };
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if valid(c) => Ok(Some(c)),
        _ => Err(LayoutError(format!("{key}.{field}: {s:?}는 이 자리에 올 수 없는 낱자다"))),
    }
}

fn parse_combos(
    list: &[String],
    valid: fn(char) -> bool,
) -> Result<HashMap<(char, char), char>, LayoutError> {
    let mut out = HashMap::new();
    for item in list {
        let chars: Vec<char> = item.chars().collect();
        let [a, b, '=', c] = chars[..] else {
            return Err(LayoutError(format!("조합 {item:?}: \"ㄱㄱ=ㄲ\" 꼴이어야 한다")));
        };
        if !(valid(a) && valid(b) && valid(c)) {
            return Err(LayoutError(format!("조합 {item:?}: 이 자리에 올 수 없는 낱자가 있다")));
        }
        if let Some(prev) = out.insert((a, b), c) {
            if prev != c {
                return Err(LayoutError(format!("조합 {a}{b}가 {prev}와 {c}로 겹친다")));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_layouts_parse() {
        for (id, name) in KoLayout::BUILTIN {
            let l = KoLayout::builtin(id).unwrap();
            assert_eq!(l.id, id);
            assert_eq!(l.name, name);
        }
    }

    #[test]
    fn v18_roles() {
        let l = KoLayout::builtin("chamshin-v18").unwrap();
        let f = l.roles(Key::F, false).unwrap();
        assert_eq!((f.jung, f.jong), (Some('ㅏ'), Some('ㄷ')));
        let o = l.roles(Key::O, false).unwrap();
        assert_eq!((o.cho, o.jung), (Some('ㄹ'), Some('ㅗ')));
        assert_eq!(l.roles(Key::N, true).unwrap().action, Some(Action::Stop));
        assert_eq!(l.roles(Key::DIGIT2, false), None);
        assert_eq!(l.combine_jong('ㄱ', 'ㅁ'), Some('ㅋ'));
        assert_eq!(l.combine_cho('ㅈ', 'ㅇ'), Some('ㅉ'));
        assert_eq!(l.combine_cho('ㄱ', 'ㅅ'), None);
    }

    #[test]
    fn d_v19_differs_where_expected() {
        let l = KoLayout::builtin("chamshin-d-v19").unwrap();
        let two = l.roles(Key::DIGIT2, false).unwrap();
        assert_eq!((two.sym.as_deref(), two.jong), (Some("2"), Some('ㅈ')));
        assert_eq!(l.roles(Key::F, false).unwrap().jong, Some('ㅋ'));
        assert_eq!(l.roles(Key::G, false).unwrap().jong, None);
        assert_eq!(l.roles(Key::B, true).unwrap().jong, None);
        assert_eq!(l.combine_jong('ㄱ', 'ㅁ'), None);
        assert_eq!(l.combine_jong('ㅍ', 'ㄹ'), Some('ㄿ'));
    }

    #[test]
    fn rejects_bad_data() {
        let bad = "[meta]\nid='x'\nname='x'\n[base]\nq = { cho = \"ㅏ\" }\n";
        assert!(KoLayout::from_toml(bad).is_err());
        let bad_combo =
            "[meta]\nid='x'\nname='x'\n[base]\nq = { cho = \"ㄱ\" }\n[combine]\ncho=[\"ㄱ+ㄱ=ㄲ\"]\n";
        assert!(KoLayout::from_toml(bad_combo).is_err());
    }
}
