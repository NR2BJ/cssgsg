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

/// 조합표의 한 조합이 어떤 것인지. 조합기는 가리지 않고 모두 받는다. 권장 입력을 알려 줄 때 쓴다(`hangul::encode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComboKind {
    /// 원작자가 권하는 조합(ㅇ+ㄱ=ㄲ, 받침 ㄱ+ㅁ=ㅋ 등, 날개셋 .ist 속 사용법).
    Recommended,
    /// 표준 조합(같은 키 연타, 받침 ㄹ+ㄱ=ㄺ 등).
    Standard,
    /// 역순 편의 조합(오이).
    Reverse,
}

/// 조합표: (앞 낱자, 뒤 낱자) → (합친 낱자, 어떤 조합인지).
type ComboMap = HashMap<(char, char), (char, ComboKind)>;

/// 한국어 배열: 키별 역할과 조합표.
#[derive(Debug, Clone)]
pub struct KoLayout {
    pub id: String,
    pub name: String,
    base: HashMap<Key, Roles>,
    shift: HashMap<Key, Roles>,
    cho: ComboMap,
    jung: ComboMap,
    jong: ComboMap,
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
            cho: parse_kinds(&raw.combine.cho, &raw.combine.cho_recommended, &[], is_cho)?,
            jung: parse_kinds(&raw.combine.jung, &[], &[], is_jung)?,
            jong: parse_kinds(
                &raw.combine.jong,
                &raw.combine.jong_recommended,
                &raw.combine.jong_reverse,
                is_jong,
            )?,
        })
    }

    /// 키의 역할. 배열에 없는 키(쿼티 그대로 나가는 키)는 `None`.
    pub fn roles(&self, key: Key, shift: bool) -> Option<&Roles> {
        if shift { self.shift.get(&key) } else { self.base.get(&key) }
    }

    /// 배열의 모든 키 역할: (키, Shift 여부, 역할). 검증 도구가 쓴다.
    pub fn entries(&self) -> impl Iterator<Item = (Key, bool, &Roles)> {
        self.base.iter().map(|(k, r)| (*k, false, r)).chain(self.shift.iter().map(|(k, r)| (*k, true, r)))
    }

    /// 조합표: (앞, 뒤, 결과).
    pub fn cho_combos(&self) -> impl Iterator<Item = (char, char, char)> + '_ {
        self.cho.iter().map(|(&(a, b), &(c, _))| (a, b, c))
    }
    pub fn jung_combos(&self) -> impl Iterator<Item = (char, char, char)> + '_ {
        self.jung.iter().map(|(&(a, b), &(c, _))| (a, b, c))
    }
    pub fn jong_combos(&self) -> impl Iterator<Item = (char, char, char)> + '_ {
        self.jong.iter().map(|(&(a, b), &(c, _))| (a, b, c))
    }

    pub fn combine_cho(&self, a: char, b: char) -> Option<char> {
        self.cho.get(&(a, b)).map(|&(c, _)| c)
    }
    pub fn combine_jung(&self, a: char, b: char) -> Option<char> {
        self.jung.get(&(a, b)).map(|&(c, _)| c)
    }
    pub fn combine_jong(&self, a: char, b: char) -> Option<char> {
        self.jong.get(&(a, b)).map(|&(c, _)| c)
    }

    /// 초성 조합의 종류(표준·권장). 조합이 아니면 `None`.
    pub fn cho_combo_kind(&self, a: char, b: char) -> Option<ComboKind> {
        self.cho.get(&(a, b)).map(|&(_, k)| k)
    }
    /// 받침 조합의 종류(표준·권장·역순). 조합이 아니면 `None`.
    pub fn jong_combo_kind(&self, a: char, b: char) -> Option<ComboKind> {
        self.jong.get(&(a, b)).map(|&(_, k)| k)
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
    cho_recommended: Vec<String>,
    #[serde(default)]
    jung: Vec<String>,
    #[serde(default)]
    jong: Vec<String>,
    #[serde(default)]
    jong_recommended: Vec<String>,
    #[serde(default)]
    jong_reverse: Vec<String>,
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

/// 표준·권장·역순 세 목록을 한 조합표로 합친다. 같은 조합이 두 목록에 있으면 오류다.
fn parse_kinds(
    standard: &[String],
    recommended: &[String],
    reverse: &[String],
    valid: fn(char) -> bool,
) -> Result<ComboMap, LayoutError> {
    let mut out = HashMap::new();
    for (list, kind) in [
        (standard, ComboKind::Standard),
        (recommended, ComboKind::Recommended),
        (reverse, ComboKind::Reverse),
    ] {
        for ((a, b), c) in parse_combos(list, valid)? {
            if out.insert((a, b), (c, kind)).is_some() {
                return Err(LayoutError(format!("조합 {a}{b}가 두 목록에 있다")));
            }
        }
    }
    Ok(out)
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
    fn combos_keep_their_kind() {
        // 세 목록을 합친 조합표는 나누기 전(0.6.5까지)과 같다: 초성 10, 중성 8, 받침 v18 35 · D 33.
        let v18 = KoLayout::builtin("chamshin-v18").unwrap();
        assert_eq!(
            (v18.cho_combos().count(), v18.jung_combos().count(), v18.jong_combos().count()),
            (10, 8, 35)
        );
        let d = KoLayout::builtin("chamshin-d-v19").unwrap();
        assert_eq!((d.cho_combos().count(), d.jung_combos().count(), d.jong_combos().count()), (10, 8, 33));
        assert_eq!(v18.cho_combo_kind('ㅇ', 'ㄱ'), Some(ComboKind::Recommended));
        assert_eq!(v18.cho_combo_kind('ㄱ', 'ㄱ'), Some(ComboKind::Standard));
        assert_eq!(v18.jong_combo_kind('ㄱ', 'ㅁ'), Some(ComboKind::Recommended));
        assert_eq!(v18.jong_combo_kind('ㄹ', 'ㄱ'), Some(ComboKind::Standard));
        assert_eq!(v18.jong_combo_kind('ㅍ', 'ㄹ'), Some(ComboKind::Reverse));
        assert_eq!(d.jong_combo_kind('ㅍ', 'ㄹ'), Some(ComboKind::Recommended), "D는 ㄿ = ㅍ+ㄹ을 권한다");
        let twice = "[meta]\nid='x'\nname='x'\n[base]\nq = { cho = \"ㄱ\" }\n[combine]\ncho=[\"ㄱㄱ=ㄲ\"]\ncho_recommended=[\"ㄱㄱ=ㄲ\"]\n";
        assert!(KoLayout::from_toml(twice).is_err(), "한 조합이 두 목록에");
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
