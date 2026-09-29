//! 新月配列 검증.
//!
//! 엔진은 hazkey용 TSV를 읽는다. 여기서는 그와 별개인 공식 파일, Google 일본어 입력용 로마자 표
//! (`shingetsu-romantable.txt`)를 기준으로 삼는다. 이 표는 뒤치기 ゛를 가나가 아니라 원래 키열로 적었다
//! (예: `sl → が`, `kel → ぼ`). 그래서 우리 TSV 해석이나 조합기 규칙이 틀리면 여기서 드러난다.

use std::collections::{BTreeMap, HashMap};

use cssgsg_core::Key;
use cssgsg_core::kana::{KanaComposer, KanaLayout, Pending};

const ROMANTABLE: &str = include_str!("../../layouts/ja/shingetsu/shingetsu-romantable.txt");

fn rows() -> Vec<(String, String)> {
    ROMANTABLE
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (i, o) = l.split_once('\t').expect("탭으로 나뉜 두 열");
            (i.to_string(), o.to_string())
        })
        .collect()
}

/// 키열을 빈 상태에서 친다. 가나 배열이 받지 않는 키가 있으면 None.
fn type_seq(l: &KanaLayout, seq: &str) -> Option<KanaComposer> {
    let mut c = KanaComposer::new();
    for ch in seq.chars() {
        let key = Key::from_qwerty(ch)?;
        if !c.key(l, key) {
            return None;
        }
    }
    Some(c)
}

#[test]
fn every_official_romantable_row_types_the_same_kana() {
    let l = KanaLayout::shingetsu();
    let rows = rows();
    assert!(rows.len() > 100, "표가 너무 짧다: {}", rows.len());
    let mut failures = Vec::new();
    for (input, output) in &rows {
        match type_seq(&l, input) {
            Some(c) if c.display(false) == *output && c.pending() == Pending::None => {}
            Some(c) => failures.push(format!("{input:5} 표 {output:?} / 엔진 {:?}", c.display(false))),
            None => failures.push(format!("{input:5} 표 {output:?} / 엔진이 키를 받지 않음")),
        }
    }
    assert!(failures.is_empty(), "{}개 행 불일치:\n{}", failures.len(), failures.join("\n"));
}

/// 가나 → 가장 짧은 키열(길이가 같으면 사전순). 신게츠 공식 사이트의 타수 집계(count_keys.mjs)와 같은 기준.
fn official_shortest() -> BTreeMap<String, String> {
    let mut best: BTreeMap<String, String> = BTreeMap::new();
    for (input, output) in rows() {
        let better = match best.get(&output) {
            None => true,
            Some(cur) => (input.len(), &input) < (cur.len(), cur),
        };
        if better {
            best.insert(output, input);
        }
    }
    best
}

/// 공식 로마자 표가 쓰는 키: 영문 3줄 30키와 [ ]. 숫자열의 `-`(ー)는 공식 표에 없고 집계도 ★p로 센다.
const TABLE_KEYS: &str = "qwertyuiopasdfghjkl;zxcvbnm,./[]";

/// 엔진으로 공식 표의 키만 써서 모든 키열(길이 `max_len`까지)을 쳐 보고, 결과가 가나 1~2자인 가장 짧은 키열을 찾는다.
fn engine_shortest(max_len: usize) -> HashMap<String, String> {
    let l = KanaLayout::shingetsu();
    let alphabet: Vec<char> = TABLE_KEYS.chars().collect();
    let mut best: HashMap<String, String> = HashMap::new();
    let mut frontier: Vec<(String, KanaComposer)> = vec![(String::new(), KanaComposer::new())];
    for _ in 0..max_len {
        let mut next = Vec::new();
        for (seq, c) in &frontier {
            for &ch in &alphabet {
                let mut c2 = c.clone();
                if !c2.key(&l, Key::from_qwerty(ch).unwrap()) {
                    continue;
                }
                let s2 = format!("{seq}{ch}");
                let reading = c2.reading().to_string();
                if c2.pending() == Pending::None && !reading.is_empty() {
                    let better = match best.get(&reading) {
                        None => true,
                        Some(cur) => (s2.len(), &s2) < (cur.len(), cur),
                    };
                    if better {
                        best.insert(reading.clone(), s2.clone());
                    }
                }
                if reading.chars().count() <= 2 {
                    next.push((s2, c2));
                }
            }
        }
        frontier = next;
    }
    best
}

#[test]
fn shortest_key_count_per_kana_matches_official_table() {
    // 공식 표 기준 가나별 최소 타수와, 엔진에서 실제로 가능한 최소 타수가 같은지 본다.
    // (같은 길이의 다른 키열은 괜찮다. 예: ぎょ = klr 또는 ;lt)
    let l = KanaLayout::shingetsu();
    let official = official_shortest();
    let mut ours = engine_shortest(4);
    // 5타 이상인 2자 가나(예: ぢゅ = ★;゛ + ☆v)는 한 자씩 이어 친 것으로 찾는다.
    for kana in official.keys() {
        let chars: Vec<char> = kana.chars().collect();
        if chars.len() == 2 {
            if let (Some(a), Some(b)) = (ours.get(&chars[0].to_string()), ours.get(&chars[1].to_string())) {
                let joined = format!("{a}{b}");
                let typed = type_seq(&l, &joined).map(|c| c.display(false));
                let shorter = ours.get(kana).is_none_or(|cur| joined.len() < cur.len());
                if typed.as_deref() == Some(kana.as_str()) && shorter {
                    ours.insert(kana.clone(), joined);
                }
            }
        }
    }
    let mut failures = Vec::new();
    for (kana, seq) in &official {
        match ours.get(kana) {
            Some(s) if s.len() == seq.len() => {}
            other => failures.push(format!("{kana}: 공식 {seq:?}({}타) / 엔진 {other:?}", seq.len())),
        }
    }
    assert!(failures.is_empty(), "{}개 불일치:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn random_kana_text_round_trips() {
    // 공식 표의 가나(゛゜와 기호 제외)를 무작위로 이어 붙인 글을 가나별 최단 키열로 쳐서
    // 그대로 나오는지 본다. 앞 가나에 뒤 키가 잘못 붙는 간섭이 없는지 확인한다.
    let l = KanaLayout::shingetsu();
    let table: Vec<(String, String)> = official_shortest()
        .into_iter()
        .filter(|(kana, _)| {
            kana.chars().all(|c| ('\u{3041}'..='\u{3096}').contains(&c) || "ー、。・「」".contains(c))
        })
        .collect();
    assert!(table.len() > 80);
    let mut seed: u64 = 0x5eed;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    for _ in 0..3000 {
        let len = 1 + next() % 12;
        let (mut text, mut keys) = (String::new(), String::new());
        for _ in 0..len {
            let (kana, seq) = &table[next() % table.len()];
            text += kana;
            keys += seq;
        }
        let c = type_seq(&l, &keys).expect("모든 키를 받아야 한다");
        assert_eq!(c.display(false), text, "키열 {keys:?}");
    }
}
