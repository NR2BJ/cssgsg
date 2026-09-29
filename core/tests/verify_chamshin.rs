//! 참신세벌식 검증: 현대 한글 음절 11,172자 전부를 원작자 사용법의 입력 방법으로 쳐서 그대로 나오는지 본다.
//!
//! 키열을 만드는 쪽(인코더)은 조합기의 판정 코드를 쓰지 않고, 원작자 사용법(날개셋 .ist 속 글)만 따른다.
//! - 초성: 오른손 초성 키. 쌍자음은 ㅇ+홑자음(ㅉ은 ㅈ+ㅇ) 또는 같은 키 연타.
//! - 중성: 왼손 모음 키. ㅗ·ㅜ는 오른손(o, .)으로도 친다(뒤 받침 키가 겹모음 짝이 아닐 때만).
//!   ㅘㅙㅚㅝㅞㅟ는 오른손 ㅗ/ㅜ + 왼손 모음. ㅑ는 초성 뒤 b.
//! - 종성: 왼손 받침 키(갈마들이), 기호+받침 겸용 키(Shift+B, D의 숫자열), 겹받침은 조합표의 두 키.
//!   기호+받침 겸용 키는 받침 뒤에서는 기호라서 겹받침의 둘째 키로 쓰지 않는다.
//!
//! 배열 데이터 자체가 원작자 배열과 같은지는 `tools/crosscheck`(타닥·오이 배열과 대조)가 본다.

use std::collections::HashMap;

use cssgsg_core::Key;
use cssgsg_core::hangul::{CHO, JONG, JUNG, KoComposer, KoLayout, KoResult, syllable};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Stroke {
    key: Key,
    shift: bool,
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
        for (key, shift, r) in l.entries() {
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

/// 한 음절을 치는 방법들.
fn encodings(l: &KoLayout, k: &Keys, cho: char, jung: char, jong: Option<char>) -> Vec<Vec<Stroke>> {
    // 초성
    let mut chos: Vec<Vec<Stroke>> = k.cho.get(&cho).into_iter().flatten().map(|&s| vec![s]).collect();
    for (a, b, c) in l.cho_combos() {
        if c == cho {
            for &sa in k.cho.get(&a).into_iter().flatten() {
                for &sb in k.cho.get(&b).into_iter().flatten() {
                    chos.push(vec![sa, sb]);
                }
            }
        }
    }

    // 종성: (키열, 첫 키가 왼손 모음 키라면 그 모음)
    let mut jongs: Vec<(Vec<Stroke>, Option<char>)> = Vec::new();
    let left_vowel_of = |s: Stroke| -> Option<char> {
        l.roles(s.key, s.shift)
            .filter(|r| r.jong.is_some() && r.cho.is_none() && r.sym.is_none())
            .and_then(|r| r.jung)
    };
    match jong {
        None => jongs.push((vec![], None)),
        Some(j) => {
            for &s in
                k.left_jong.get(&j).into_iter().flatten().chain(k.sym_jong.get(&j).into_iter().flatten())
            {
                jongs.push((vec![s], left_vowel_of(s)));
            }
            for (a, b, c) in l.jong_combos() {
                if c != j {
                    continue;
                }
                let firsts =
                    k.left_jong.get(&a).into_iter().flatten().chain(k.sym_jong.get(&a).into_iter().flatten());
                for &sa in firsts {
                    for &sb in k.left_jong.get(&b).into_iter().flatten() {
                        jongs.push((vec![sa, sb], left_vowel_of(sa)));
                    }
                }
            }
        }
    }

    // 중성: 오른손 ㅗ/ㅜ 뒤에 받침 첫 키가 겹모음 짝이면 그 키는 겹모음이 되므로 쓰지 않는다.
    let mut out = Vec::new();
    for c in &chos {
        for (jo, first_vowel) in &jongs {
            let mut jungs: Vec<Vec<Stroke>> = Vec::new();
            for &s in k.left_jung.get(&jung).into_iter().flatten() {
                jungs.push(vec![s]);
            }
            for &s in k.right_jung.get(&jung).into_iter().flatten() {
                let breaks = first_vowel.is_some_and(|v| l.combine_jung(jung, v).is_some());
                if !breaks {
                    jungs.push(vec![s]);
                }
            }
            for (a, b, v) in l.jung_combos() {
                if v != jung {
                    continue;
                }
                for &sa in k.right_jung.get(&a).into_iter().flatten() {
                    for &sb in k.left_jung.get(&b).into_iter().flatten() {
                        jungs.push(vec![sa, sb]);
                    }
                }
            }
            for ju in &jungs {
                out.push([c.clone(), ju.clone(), jo.clone()].concat());
            }
        }
    }
    out
}

/// 키열을 쳐서 (확정 글자 모음, 마지막 조합 중 글자)를 돌려준다.
fn type_strokes(l: &KoLayout, strokes: &[Stroke]) -> (Vec<String>, String) {
    let mut c = KoComposer::new();
    let mut commits = Vec::new();
    for s in strokes {
        let r = l.roles(s.key, s.shift).expect("배열에 있는 키");
        match c.input(l, r) {
            KoResult::Composed { commit } | KoResult::Stopped { commit } => {
                if !commit.is_empty() {
                    commits.push(commit)
                }
            }
            KoResult::Symbol { commit, sym } => {
                commits.push(commit);
                commits.push(sym);
            }
        }
    }
    (commits, c.flush())
}

fn verify_all_syllables(layout_id: &str) -> (usize, usize) {
    let l = KoLayout::builtin(layout_id).unwrap();
    let k = Keys::of(&l);
    let (mut syllables, mut runs) = (0, 0);
    let mut unreachable = Vec::new();
    let mut failures = Vec::new();
    for &cho in &CHO {
        for &jung in &JUNG {
            for jong in std::iter::once(None).chain(JONG.iter().copied().map(Some)) {
                let want = syllable(cho, jung, jong).unwrap().to_string();
                let encs = encodings(&l, &k, cho, jung, jong);
                if encs.is_empty() {
                    unreachable.push(want);
                    continue;
                }
                syllables += 1;
                for enc in encs {
                    runs += 1;
                    let (commits, last) = type_strokes(&l, &enc);
                    if !commits.is_empty() || last != want {
                        let keys: String = enc
                            .iter()
                            .map(|s| {
                                let c = s.key.qwerty_char(false).unwrap();
                                if s.shift { c.to_ascii_uppercase() } else { c }
                            })
                            .collect();
                        failures.push(format!("{want}: 키열 {keys:?} → 확정 {commits:?} + {last:?}"));
                    }
                }
            }
        }
    }
    assert!(
        unreachable.is_empty(),
        "{layout_id}: 칠 방법이 없는 음절 {}개: {:?}",
        unreachable.len(),
        unreachable
    );
    assert!(
        failures.is_empty(),
        "{layout_id}: {}개 실패 (처음 30개):\n{}",
        failures.len(),
        failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n")
    );
    (syllables, runs)
}

#[test]
fn v18_every_modern_syllable_by_every_documented_method() {
    let (syllables, runs) = verify_all_syllables("chamshin-v18");
    assert_eq!(syllables, 11172);
    // 음절마다 여러 입력 방법(쌍자음 두 가지, ㅗ·ㅜ 좌우 손, 겹받침 순서 여러 개)을 다 친다. 현재 27,600가지.
    assert!(runs > 2 * syllables, "입력 방법 수 {runs}");
}

#[test]
fn d_v19_every_modern_syllable_by_every_documented_method() {
    let (syllables, _) = verify_all_syllables("chamshin-d-v19");
    assert_eq!(syllables, 11172);
}

#[test]
fn random_words_type_back_exactly() {
    // 음절 1~5개짜리 낱말을 무작위로 만들고, 음절마다 입력 방법 하나를 골라 이어 친다.
    for layout_id in ["chamshin-v18", "chamshin-d-v19"] {
        let l = KoLayout::builtin(layout_id).unwrap();
        let k = Keys::of(&l);
        let mut seed: u64 = 0xC0FFEE;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        for _ in 0..3000 {
            let n = 1 + next() % 5;
            let (mut word, mut strokes) = (String::new(), Vec::new());
            for _ in 0..n {
                let cho = CHO[next() % CHO.len()];
                let jung = JUNG[next() % JUNG.len()];
                let jong = if next() % 3 == 0 { None } else { Some(JONG[next() % JONG.len()]) };
                let encs = encodings(&l, &k, cho, jung, jong);
                word.push(syllable(cho, jung, jong).unwrap());
                strokes.extend(encs[next() % encs.len()].iter().copied());
            }
            let (commits, last) = type_strokes(&l, &strokes);
            assert_eq!(commits.concat() + &last, word, "{layout_id}");
        }
    }
}

#[test]
fn documented_examples() {
    // 원작자 사용법의 예시와 규칙.
    let v18 = KoLayout::builtin("chamshin-v18").unwrap();
    let d = KoLayout::builtin("chamshin-d-v19").unwrap();
    let t = |l: &KoLayout, keys: &str| -> String {
        let strokes: Vec<Stroke> = keys
            .chars()
            .map(|c| Stroke {
                key: Key::from_qwerty(c.to_ascii_lowercase()).unwrap(),
                shift: c.is_ascii_uppercase(),
            })
            .collect();
        let (commits, last) = type_strokes(l, &strokes);
        commits.concat() + &last
    };
    // ‘ㅋㅋ’ → ㅋ + Shift+n + ㅋ
    assert_eq!(t(&v18, "bNb"), "ㅋㅋ");
    // 받침 ㅋ: ㄱ+ㅁ (권장) 또는 Shift+b
    assert_eq!(t(&v18, "kfea"), "갘");
    assert_eq!(t(&v18, "kfB"), "갘");
    // 쌍자음: ㅇ+ㄱ/ㄷ/ㅂ/ㅅ, ㅈ+ㅇ, 또는 연타
    assert_eq!(t(&v18, "jkfjifj;fjlfnjf"), "까따빠싸짜");
    assert_eq!(t(&v18, "kkfiif;;fllfnnf"), "까따빠싸짜");
    // 초성 없이 낱자: 왼손 모음만으로 조합 (ㅣ+ㅏ→ㅑ)
    assert_eq!(t(&v18, "df"), "ㅑ");
    assert_eq!(t(&v18, "gf"), "ㅘ");
    // D: '표3' → 표 + Shift+n + 3
    assert_eq!(t(&d, "/aN3"), "표3");
    // D: 받침 ㅋ은 f 갈마들이
    assert_eq!(t(&d, "kff"), "갘");
}
