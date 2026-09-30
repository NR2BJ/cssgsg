//! 음절을 치는 방법(키열) 만들기. 원작자 사용법(날개셋 .ist 속 글)만 따르고 조합기의 판정 코드는 쓰지 않는다.
//! 그래서 조합기 검증(`tests/verify_chamshin.rs`: 11,172자 × 모든 방법)의 정답 쪽이 되고, 타자 연습 페이지가
//! 권장 입력을 알려 주는 데도 쓴다(`cssgsg-cli encode`, `tools/practice`).
//!
//! - 초성: 오른손 초성 키. 쌍자음은 ㅇ+홑자음(ㅉ은 ㅈ+ㅇ, 권장) 또는 같은 키 연타.
//! - 중성: 왼손 모음 키. ㅗ·ㅜ는 오른손(o, .)으로도 친다(뒤 받침 키가 겹모음 짝이 아닐 때만).
//!   ㅘㅙㅚㅝㅞㅟ는 오른손 ㅗ/ㅜ + 왼손 모음. ㅑ는 초성 뒤 b.
//! - 종성: 왼손 받침 키(갈마들이), 기호+받침 겸용 키(Shift+B, D의 숫자열), 겹받침은 조합표의 두 키.
//!   기호+받침 겸용 키는 받침 뒤에서는 기호라서 겹받침의 둘째 키로 쓰지 않는다.
//!
//! 권하는 정도(`Encoding::rank`, 작을수록 권장)는 배열 학습 페이지(learn/template.html)의 규칙과 같다:
//! 쌍자음은 ㅇ+홑자음, 홑모음 ㅗ·ㅜ는 왼손, 받침 ㅋ은 ㄱ+ㅁ(Shift+B보다), 겹받침은 원작자 권장 조합 → 표준 → 역순 편의.

use std::collections::HashMap;

use super::layout::{ComboKind, KoLayout};
use crate::key::Key;

/// 키 하나와 Shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stroke {
    pub key: Key,
    pub shift: bool,
}

impl Stroke {
    /// 키열 문법(`sim`)의 글자: 쿼티 자리 글자, Shift면 쿼티 Shift 글자(대문자·기호).
    pub fn sim_char(self) -> char {
        self.key.qwerty_char(self.shift).expect("글자를 내는 키")
    }
}

/// 한 음절을 치는 방법 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoding {
    pub strokes: Vec<Stroke>,
    /// 원작자 사용법이 얼마나 권하는가. 0이 권장 입력이고 클수록 덜 권한다(쳐도 되는 방법).
    pub rank: u8,
}

impl Encoding {
    /// 키열 문법 글자열(예: "kfe", 받침 ㅋ Shift+B는 "kfB").
    pub fn sim_keys(&self) -> String {
        self.strokes.iter().map(|s| s.sim_char()).collect()
    }
}

/// 역할별로 그 역할을 가진 키를 모은 표.
struct Keys {
    /// 초성 키(오른손). 초성+중성 겸용(o, b)도 포함.
    cho: HashMap<char, Vec<Stroke>>,
    /// 왼손 모음 키(모음, 또는 모음+받침).
    left_jung: HashMap<char, Vec<Stroke>>,
    /// 오른손 모음(초성·기호와 겸용: o=ㅗ, .=ㅜ, b=ㅑ).
    right_jung: HashMap<char, Vec<Stroke>>,
    /// 왼손 받침 키(모음+받침 갈마들이).
    left_jong: HashMap<char, Vec<Stroke>>,
    /// 기호+받침 겸용 키(Shift+B, D의 숫자열).
    sym_jong: HashMap<char, Vec<Stroke>>,
}

impl Keys {
    fn of(l: &KoLayout) -> Self {
        let mut k = Keys {
            cho: HashMap::new(),
            left_jung: HashMap::new(),
            right_jung: HashMap::new(),
            left_jong: HashMap::new(),
            sym_jong: HashMap::new(),
        };
        let mut entries: Vec<_> = l.entries().collect();
        // 해시 순서에 기대지 않게 키 순서로(같은 순위 안의 차례가 늘 같다).
        entries.sort_by_key(|(key, shift, _)| (*shift, key.0));
        for (key, shift, r) in entries {
            let s = Stroke { key, shift };
            if let Some(c) = r.cho {
                k.cho.entry(c).or_default().push(s);
            }
            match (r.cho.is_some() || r.sym.is_some(), r.jung, r.jong) {
                (true, Some(v), _) => k.right_jung.entry(v).or_default().push(s),
                (false, Some(v), _) => k.left_jung.entry(v).or_default().push(s),
                _ => {}
            }
            match (r.jung, r.sym.is_some(), r.jong) {
                (Some(_), _, Some(j)) => k.left_jong.entry(j).or_default().push(s),
                (None, true, Some(j)) => k.sym_jong.entry(j).or_default().push(s),
                _ => {}
            }
        }
        k
    }
}

fn kind_rank(kind: Option<ComboKind>) -> u8 {
    match kind {
        Some(ComboKind::Recommended) => 0,
        Some(ComboKind::Standard) | None => 1,
        Some(ComboKind::Reverse) => 2,
    }
}

/// 한 음절을 치는 방법들. 권하는 것부터(순위가 같으면 키가 적은 것부터).
pub fn encodings(l: &KoLayout, cho: char, jung: char, jong: Option<char>) -> Vec<Encoding> {
    let k = Keys::of(l);

    // 초성: (키열, 순위)
    let mut chos: Vec<(Vec<Stroke>, u8)> =
        k.cho.get(&cho).into_iter().flatten().map(|&s| (vec![s], 0)).collect();
    let mut cho_combos: Vec<_> = l.cho_combos().filter(|&(_, _, c)| c == cho).collect();
    cho_combos.sort();
    for (a, b, _) in cho_combos {
        let rank = kind_rank(l.cho_combo_kind(a, b));
        for &sa in k.cho.get(&a).into_iter().flatten() {
            for &sb in k.cho.get(&b).into_iter().flatten() {
                chos.push((vec![sa, sb], rank));
            }
        }
    }

    // 종성: (키열, 첫 키가 왼손 모음 키라면 그 모음, 순위)
    let mut jongs: Vec<(Vec<Stroke>, Option<char>, u8)> = Vec::new();
    let left_vowel_of = |s: Stroke| -> Option<char> {
        l.roles(s.key, s.shift)
            .filter(|r| r.jong.is_some() && r.cho.is_none() && r.sym.is_none())
            .and_then(|r| r.jung)
    };
    match jong {
        None => jongs.push((vec![], None, 0)),
        Some(j) => {
            for &s in k.left_jong.get(&j).into_iter().flatten() {
                jongs.push((vec![s], left_vowel_of(s), 0));
            }
            // 기호+받침 겸용 키(받침 ㅋ의 Shift+B)는 권장 조합(ㄱ+ㅁ)보다 뒤, 표준 조합과 같은 순위다.
            for &s in k.sym_jong.get(&j).into_iter().flatten() {
                jongs.push((vec![s], None, 1));
            }
            let mut jong_combos: Vec<_> = l.jong_combos().filter(|&(_, _, c)| c == j).collect();
            jong_combos.sort();
            for (a, b, _) in jong_combos {
                let rank = kind_rank(l.jong_combo_kind(a, b));
                let firsts =
                    k.left_jong.get(&a).into_iter().flatten().chain(k.sym_jong.get(&a).into_iter().flatten());
                for &sa in firsts {
                    for &sb in k.left_jong.get(&b).into_iter().flatten() {
                        jongs.push((vec![sa, sb], left_vowel_of(sa), rank));
                    }
                }
            }
        }
    }

    // 중성: 오른손 ㅗ/ㅜ 뒤에 받침 첫 키가 겹모음 짝이면 그 키는 겹모음이 되므로 쓰지 않는다.
    // 홑모음 ㅗ·ㅜ는 왼손을 권한다(오른손은 겹모음을 위한 자리, 배열 학습 페이지와 같다).
    let has_left = k.left_jung.contains_key(&jung);
    let mut out = Vec::new();
    for (c, cho_rank) in &chos {
        for (jo, first_vowel, jong_rank) in &jongs {
            let mut jungs: Vec<(Vec<Stroke>, u8)> = Vec::new();
            for &s in k.left_jung.get(&jung).into_iter().flatten() {
                jungs.push((vec![s], 0));
            }
            for &s in k.right_jung.get(&jung).into_iter().flatten() {
                let breaks = first_vowel.is_some_and(|v| l.combine_jung(jung, v).is_some());
                if !breaks {
                    jungs.push((vec![s], if has_left { 1 } else { 0 }));
                }
            }
            let mut jung_combos: Vec<_> = l.jung_combos().filter(|&(_, _, v)| v == jung).collect();
            jung_combos.sort();
            for (a, b, _) in jung_combos {
                for &sa in k.right_jung.get(&a).into_iter().flatten() {
                    for &sb in k.left_jung.get(&b).into_iter().flatten() {
                        jungs.push((vec![sa, sb], 0));
                    }
                }
            }
            for (ju, jung_rank) in &jungs {
                out.push(Encoding {
                    strokes: [c.clone(), ju.clone(), jo.clone()].concat(),
                    rank: cho_rank + jung_rank + jong_rank,
                });
            }
        }
    }
    out.sort_by_key(|e| (e.rank, e.strokes.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::{KoComposer, KoResult, syllable};

    fn decompose(s: char) -> (char, char, Option<char>) {
        crate::hangul::decompose(s).unwrap()
    }

    /// 낱말을 음절마다 권장 입력으로 이은 키열.
    fn recommended(l: &KoLayout, word: &str) -> String {
        word.chars()
            .map(|s| {
                let (cho, jung, jong) = decompose(s);
                assert_eq!(syllable(cho, jung, jong), Some(s));
                encodings(l, cho, jung, jong)[0].sim_keys()
            })
            .collect()
    }

    #[test]
    fn recommended_input_is_what_the_learning_page_teaches() {
        // learn/template.html의 참신세벌식 규칙과 표(권장 열)에 적힌 키열.
        let l = KoLayout::builtin("chamshin-v18").unwrap();
        for (word, keys) in [
            ("한", "hfs"),
            ("감사합니다", "kfalfhfzmdif"),
            ("과", "kof"),
            ("궈", "k.v"),
            ("왜", "jos"),
            ("뒤", "i.d"),
            ("곧", "kgf"),
            ("갸", "kb"),
            ("의", "jw"),
            ("까", "jkf"),
            ("따", "jif"),
            ("빠", "j;f"),
            ("싸", "jlf"),
            ("짜", "njf"),
            ("있", "jdq"),
            ("부엌", ";rjvea"),
            ("밖", ";fee"),
            ("몫", "ugeq"),
            ("앉", "jfes"),
            ("않", "jfds"),
            ("읽", "jdew"),
            ("삶", "lfwa"),
            ("넓", "mvsa"),
            ("곬", "kgwq"),
            ("핥", "hfwr"),
            ("읊", "jcda"),
            ("싫", "lddw"),
            ("없", "jvxz"),
        ] {
            assert_eq!(recommended(&l, word), keys, "{word}");
        }
        // 쳐도 되는 다른 방법도 목록에 있다: 오른손 ㅗ + 받침 ㄱ(곡), 연타 쌍자음(까), 받침 ㅋ Shift+B(갘).
        let ways = |s: char| {
            let (cho, jung, jong) = decompose(s);
            encodings(&l, cho, jung, jong).iter().map(Encoding::sim_keys).collect::<Vec<_>>()
        };
        assert_eq!(ways('곡')[0], "kge");
        assert!(ways('곡').contains(&"koe".to_string()));
        assert!(ways('까').contains(&"kkf".to_string()));
        assert!(ways('갘').contains(&"kfB".to_string()) && ways('갘')[0] == "kfea");
    }

    #[test]
    fn every_way_of_every_syllable_types_back_exactly() {
        // 음절마다 모든 방법을 조합기에 넣어 그 음절 하나가 나오는지(tests/verify_chamshin.rs가 두 배열 전수로 본다).
        let l = KoLayout::builtin("chamshin-v18").unwrap();
        for s in ['각', '까', '과', '곧', '갘', '읽', '없', '뛁', '쐒'] {
            let (cho, jung, jong) = decompose(s);
            let ways = encodings(&l, cho, jung, jong);
            assert!(!ways.is_empty(), "{s}");
            assert_eq!(ways[0].rank, ways.iter().map(|w| w.rank).min().unwrap());
            for way in ways {
                let mut c = KoComposer::new();
                for st in &way.strokes {
                    let r = l.roles(st.key, st.shift).unwrap();
                    assert!(
                        matches!(c.input(&l, r), KoResult::Composed { commit } if commit.is_empty()),
                        "{s}"
                    );
                }
                assert_eq!(c.flush(), s.to_string(), "{s} {}", way.sim_keys());
            }
        }
    }
}
