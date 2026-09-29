//! 新月配列 공식 TSV(hazkey용 표) 읽기.
//!
//! 표의 행을 우리 조합기가 쓰는 표로 나눈다.
//! - 키 하나 → 가나(무시프트). 출력이 ☆/★/゛이면 그 키가 ☆·★ 앞치기 키, ゛ 뒤치기 키다.
//! - ☆X / ★X → 앞치기 면. kk(☆☆)=も, dd(★★)=ら도 여기로 간다.
//! - ☆゛X → 3타 단축(濁拗音 등).
//! - 가나+゛키 → 뒤치기 표(か→が, あ→ぁ …).

use std::collections::HashMap;

use crate::key::Key;

const SHINGETSU_TSV: &str = include_str!("../../../layouts/ja/shingetsu/shingetsu-ansi-qwerty.tsv");

#[derive(Debug, Clone)]
pub struct KanaLayout {
    pub base: HashMap<Key, String>,
    pub star_key: Key,
    pub black_key: Key,
    pub daku_key: Key,
    pub star: HashMap<Key, String>,
    pub black: HashMap<Key, String>,
    pub star_daku: HashMap<Key, String>,
    pub postfix: HashMap<char, char>,
}

impl KanaLayout {
    pub fn shingetsu() -> Self {
        Self::from_tsv(SHINGETSU_TSV).expect("내장 新月 표는 항상 읽혀야 한다")
    }

    pub fn from_tsv(src: &str) -> Result<Self, String> {
        let rows: Vec<(String, String)> = src
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let cols: Vec<&str> = l.split('\t').collect();
                let input = cols[0].to_string();
                // 3열이면 가운데는 비어 있고 마지막 열이 출력이다(hazkey 형식).
                let output = cols.iter().skip(1).rev().find(|c| !c.is_empty()).unwrap_or(&"");
                (input, output.to_string())
            })
            .collect();

        let single = |s: &str| -> Option<Key> {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) if c.is_ascii() => Key::from_qwerty(c),
                _ => None,
            }
        };

        let find_special = |out: &str| {
            rows.iter()
                .find(|(i, o)| o == out && single(i).is_some())
                .and_then(|(i, _)| single(i))
                .ok_or_else(|| format!("{out} 키가 표에 없다"))
        };
        let star_key = find_special("☆")?;
        let black_key = find_special("★")?;
        let daku_key = find_special("゛")?;
        let daku_char = daku_key.qwerty_char(false).unwrap();
        let star_char = star_key.qwerty_char(false).unwrap();
        let black_char = black_key.qwerty_char(false).unwrap();

        let mut l = KanaLayout {
            base: HashMap::new(),
            star_key,
            black_key,
            daku_key,
            star: HashMap::new(),
            black: HashMap::new(),
            star_daku: HashMap::new(),
            postfix: HashMap::new(),
        };

        for (input, output) in &rows {
            let chars: Vec<char> = input.chars().collect();
            match chars[..] {
                [c] if c.is_ascii() => {
                    let key = Key::from_qwerty(c).ok_or_else(|| format!("알 수 없는 키 {c:?}"))?;
                    if key != star_key && key != black_key && key != daku_key {
                        l.base.insert(key, output.clone());
                    }
                }
                ['☆', '゛', c] => {
                    l.star_daku.insert(key_of(c)?, output.clone());
                }
                ['☆', c] => {
                    l.star.insert(key_of(c)?, output.clone());
                }
                ['★', c] => {
                    l.black.insert(key_of(c)?, output.clone());
                }
                [a, b] if a == star_char && b == star_char => {
                    l.star.insert(star_key, output.clone());
                }
                [a, b] if a == black_char && b == black_char => {
                    l.black.insert(black_key, output.clone());
                }
                [kana, d] if !kana.is_ascii() && d == daku_char => {
                    let mut out = output.chars();
                    match (out.next(), out.next()) {
                        (Some(to), None) => {
                            l.postfix.insert(kana, to);
                        }
                        _ => return Err(format!("뒤치기 {input:?}의 출력이 한 글자가 아니다")),
                    }
                }
                _ => return Err(format!("알 수 없는 행 {input:?}\t{output:?}")),
            }
        }
        Ok(l)
    }
}

fn key_of(c: char) -> Result<Key, String> {
    Key::from_qwerty(c).ok_or_else(|| format!("알 수 없는 키 {c:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shingetsu_tables() {
        let l = KanaLayout::shingetsu();
        assert_eq!((l.star_key, l.black_key, l.daku_key), (Key::K, Key::D, Key::L));
        assert_eq!(l.base[&Key::A], "は");
        assert_eq!(l.base[&Key::SEMICOLON], "き");
        assert_eq!(l.base[&Key::COMMA], "、");
        assert_eq!(l.base[&Key::MINUS], "ー");
        assert_eq!(l.star[&Key::A], "あ");
        assert_eq!(l.star[&Key::K], "も");
        assert_eq!(l.black[&Key::J], "お");
        assert_eq!(l.black[&Key::D], "ら");
        assert_eq!(l.black[&Key::P], "ー");
        assert_eq!(l.star_daku[&Key::Q], "ぴょ");
        assert_eq!(l.star_daku[&Key::L], ";");
        assert_eq!(l.postfix[&'か'], 'が');
        assert_eq!(l.postfix[&'ば'], 'ぱ');
        assert_eq!(l.postfix[&'う'], 'ゔ');
        assert_eq!(l.postfix[&'ゔ'], 'ぅ');
        // 30키 + 기호 키: 글자 26개 중 ☆★゛ 셋을 빼고, ; , . / - [ ] 를 더한다.
        assert_eq!(l.base.len(), 26 - 3 + 7);
    }
}
