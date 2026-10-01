//! 한국어 한자 변환: 사전 찾기, 글자 하나의 후보, 고른 후보 기억.
//!
//! 사전은 libhangul의 `hanja.txt`와 `mssymbol.txt`(`dict/ko`, BSD-3)를 고치지 않고 바이너리에 넣는다.
//! 두 파일 모두 `읽기:값:뜻` 줄이 읽기 순으로 정렬돼 있어서 그대로 이진 탐색한다(정렬은 테스트가 확인한다).
//! 같은 읽기 안의 줄 순서를 순위로 쓰고, 고른 후보는 기억해 앞으로 올린다([`Learning`]).
//!
//! 바꾸는 것은 조합 중인 글자 하나다(음절 → 한자, 자음 하나 → 기호). `hanja.txt`의 낱말 항목(27만여 개)은 지금 쓰지 않는다.

use std::cmp::Reverse;
use std::collections::HashMap;

const HANJA_TXT: &str = include_str!("../../dict/ko/hanja.txt");
const SYMBOL_TXT: &str = include_str!("../../dict/ko/mssymbol.txt");

/// 사전 한 줄.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub reading: &'static str,
    pub value: &'static str,
    /// 훈음(`나라 이름 한, 한나라 한`)이나 단어 설명(`지명`). 대개 비어 있다.
    pub note: &'static str,
}

/// 정렬된 `읽기:값:뜻` 줄 모음(머리 주석을 뺀 본문).
#[derive(Clone, Copy)]
struct Table {
    body: &'static str,
}

impl Table {
    fn new(text: &'static str) -> Self {
        let mut body = text;
        while body.starts_with('#') || body.starts_with('\n') {
            body = body.split_once('\n').map_or("", |(_, rest)| rest);
        }
        Self { body }
    }

    /// `reading` 읽기의 줄들(파일 순서).
    fn lookup(self, reading: &str) -> impl Iterator<Item = Entry> + '_ {
        let start = self.lower_bound(reading);
        self.body[start..].lines().map(parse).take_while(move |e| e.reading == reading)
    }

    /// 읽기가 `reading` 이상인 첫 줄의 시작 위치.
    fn lower_bound(self, reading: &str) -> usize {
        let bytes = self.body.as_bytes();
        let (mut lo, mut hi) = (0, bytes.len());
        // lo와 hi는 늘 줄의 시작(또는 끝)이다. mid가 든 줄의 읽기와 비교해 줄 단위로 좁힌다.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let line_start = bytes[..mid].iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1);
            let line_end =
                bytes[line_start..].iter().position(|&b| b == b'\n').map_or(bytes.len(), |p| line_start + p);
            if parse(&self.body[line_start..line_end]).reading < reading {
                lo = line_end + 1;
            } else {
                hi = line_start;
            }
        }
        lo.min(bytes.len())
    }
}

fn parse(line: &'static str) -> Entry {
    let mut it = line.splitn(3, ':');
    Entry { reading: it.next().unwrap_or(""), value: it.next().unwrap_or(""), note: it.next().unwrap_or("") }
}

fn hanja_table() -> Table {
    Table::new(HANJA_TXT)
}

fn symbol_table() -> Table {
    Table::new(SYMBOL_TXT)
}

/// 한자 사전에서 읽기가 `reading`인 항목(순위 순).
pub fn lookup(reading: &str) -> impl Iterator<Item = Entry> {
    hanja_table().lookup(reading)
}

/// 자음 하나 + 한자 키로 내는 기호(MS IME 표, `ㅁ` → ＃ ＆ ＊ ＠ § ※ ☆ …).
pub fn symbols(jamo: char) -> impl Iterator<Item = Entry> {
    let mut buf = [0u8; 4];
    let reading: &str = jamo.encode_utf8(&mut buf);
    symbol_table().lookup(reading).collect::<Vec<_>>().into_iter()
}

pub fn is_syllable(c: char) -> bool {
    ('가'..='힣').contains(&c)
}

/// 호환 자모의 자음(ㄱ~ㅎ). 기호표의 읽기다.
pub fn is_consonant(c: char) -> bool {
    ('ㄱ'..='ㅎ').contains(&c)
}

/// 후보 하나: 바꿀 글자와 후보창에 같이 보일 뜻(한자의 훈음, 없으면 빈 문자열).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cand {
    pub text: String,
    pub note: String,
}

/// 조합 중인 글자 하나의 후보(기억한 것 먼저, 그다음 사전 순위).
/// 한글 음절이면 한자와 훈음(`韓 나라 이름 한, 한나라 한`), 자음 하나(ㄱ~ㅎ)면 기호표. 없으면 빈 목록.
pub fn candidates(letter: char, learning: &Learning) -> Vec<Cand> {
    let mut buf = [0u8; 4];
    let reading: &str = letter.encode_utf8(&mut buf);
    let mut cands: Vec<Cand> = if is_syllable(letter) {
        // 한 음절 항목 중 값이 두 글자인 것(莘洞 같은 지명 2개)은 사전 오류라 뺀다.
        lookup(reading)
            .filter(|e| e.value.chars().count() == 1)
            .map(|e| Cand { text: e.value.to_string(), note: e.note.to_string() })
            .collect()
    } else if is_consonant(letter) {
        symbols(letter).map(|e| Cand { text: e.value.to_string(), note: String::new() }).collect()
    } else {
        Vec::new()
    };
    learning.order(reading, &mut cands);
    cands
}

/// 고른 후보 기억. 읽기마다 (글자, 고른 횟수, 마지막으로 고른 순번)을 둔다.
/// 순서: 그 읽기에서 가장 최근에 고른 것 → 많이 고른 것 → 사전 순위. 전체 [`Learning::MAX`]개를 넘으면 가장 오래된 것부터 버린다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Learning {
    picks: HashMap<String, Vec<Pick>>,
    seq: u64,
    count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pick {
    text: String,
    count: u32,
    last: u64,
}

impl Learning {
    pub const MAX: usize = 10_000;

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn record(&mut self, reading: &str, text: &str) {
        if reading.is_empty() || text.is_empty() || [reading, text].iter().any(|s| s.contains(['\t', '\n'])) {
            return;
        }
        self.seq += 1;
        let picks = self.picks.entry(reading.to_string()).or_default();
        match picks.iter_mut().find(|p| p.text == text) {
            Some(p) => {
                p.count = p.count.saturating_add(1);
                p.last = self.seq;
            }
            None => {
                picks.push(Pick { text: text.to_string(), count: 1, last: self.seq });
                self.count += 1;
            }
        }
        while self.count > Self::MAX {
            self.evict_oldest();
        }
    }

    fn evict_oldest(&mut self) {
        let Some((reading, i)) = self
            .picks
            .iter()
            .flat_map(|(r, picks)| picks.iter().enumerate().map(move |(i, p)| (p.last, r, i)))
            .min()
            .map(|(_, r, i)| (r.clone(), i))
        else {
            return;
        };
        let picks = self.picks.get_mut(&reading).expect("방금 찾은 읽기");
        picks.remove(i);
        if picks.is_empty() {
            self.picks.remove(&reading);
        }
        self.count -= 1;
    }

    /// 가장 최근에 고른 것(읽기, 글자). 윈도우 입력기가 고른 것을 엔진 호스트에 알릴 때 쓴다(앱마다 엔진이 따로라
    /// 기억은 호스트가 모아 저장하고 나눠 준다).
    pub fn last_pick(&self) -> Option<(&str, &str)> {
        self.picks
            .iter()
            .flat_map(|(r, picks)| picks.iter().map(move |p| (p.last, r.as_str(), p.text.as_str())))
            .max_by_key(|&(last, _, _)| last)
            .map(|(_, reading, text)| (reading, text))
    }

    /// 한 읽기의 후보를 기억대로 다시 늘어놓는다. 기억에 없는 것은 원래 순서를 지킨다.
    pub fn order(&self, reading: &str, cands: &mut [Cand]) {
        let Some(picks) = self.picks.get(reading) else { return };
        let newest = picks.iter().max_by_key(|p| p.last).map(|p| p.text.as_str());
        cands.sort_by_key(|c| match picks.iter().find(|p| p.text == c.text) {
            Some(p) => (0, Some(p.text.as_str()) != newest, Reverse(p.count), Reverse(p.last)),
            None => (1, false, Reverse(0), Reverse(0)),
        });
    }

    /// 저장 형식: 줄마다 `읽기\t글자\t횟수\t순번`. `#`으로 시작하는 줄은 설명이다.
    pub fn to_tsv(&self) -> String {
        let mut rows: Vec<(&String, &Pick)> =
            self.picks.iter().flat_map(|(r, picks)| picks.iter().map(move |p| (r, p))).collect();
        rows.sort_by_key(|(_, p)| p.last);
        let mut out = String::from("# cssgsg 한자 학습: 읽기\t글자\t고른 횟수\t순번(클수록 최근)\n");
        for (reading, p) in rows {
            out += &format!("{reading}\t{}\t{}\t{}\n", p.text, p.count, p.last);
        }
        out
    }

    /// 저장 형식을 읽는다. 모양이 틀린 줄은 건너뛴다.
    pub fn from_tsv(src: &str) -> Self {
        let mut rows: Vec<(String, Pick)> = src
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let [reading, text, count, last] = f[..] else { return None };
                if reading.is_empty() || text.is_empty() {
                    return None;
                }
                Some((
                    reading.to_string(),
                    Pick { text: text.to_string(), count: count.parse().ok()?, last: last.parse().ok()? },
                ))
            })
            .collect();
        // 최근 것이 남도록, 넘치면 오래된 것부터 버린다.
        rows.sort_by_key(|(_, p)| Reverse(p.last));
        let mut learning = Learning::default();
        for (reading, pick) in rows {
            if learning.count >= Self::MAX {
                break;
            }
            let picks = learning.picks.entry(reading).or_default();
            if picks.iter().any(|p| p.text == pick.text) {
                continue;
            }
            learning.seq = learning.seq.max(pick.last);
            picks.push(pick);
            learning.count += 1;
        }
        learning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(letter: char, learning: &Learning, n: usize) -> Vec<String> {
        candidates(letter, learning).into_iter().take(n).map(|c| c.text).collect()
    }

    fn body_lines(text: &'static str) -> Vec<Entry> {
        Table::new(text).body.lines().map(parse).collect()
    }

    #[test]
    fn tables_are_sorted_for_binary_search() {
        for (name, text, lines) in [("hanja.txt", HANJA_TXT, 303_495), ("mssymbol.txt", SYMBOL_TXT, 988)] {
            let entries = body_lines(text);
            assert_eq!(entries.len(), lines, "{name} 줄 수(dict/ko/README.md)");
            for w in entries.windows(2) {
                assert!(w[0].reading <= w[1].reading, "{name} 정렬이 깨졌다: {:?} > {:?}", w[0], w[1]);
            }
            assert!(entries.iter().all(|e| !e.value.is_empty()), "{name} 빈 값");
        }
    }

    #[test]
    fn lookup_matches_linear_scan() {
        // 이진 탐색이 선형 탐색과 같은지: 읽기 1000개마다 하나, 처음과 끝, 없는 읽기.
        let entries = body_lines(HANJA_TXT);
        let mut readings: Vec<&str> = entries.iter().map(|e| e.reading).collect();
        readings.dedup();
        let mut samples: Vec<&str> = readings.iter().step_by(1000).copied().collect();
        samples.extend([readings[0], readings[readings.len() - 1], "가", "힣힣", "", "ㄱ"]);
        for r in samples {
            let want: Vec<Entry> = entries.iter().filter(|e| e.reading == r).copied().collect();
            assert_eq!(lookup(r).collect::<Vec<_>>(), want, "읽기 {r:?}");
        }
    }

    #[test]
    fn known_entries() {
        let han: Vec<Entry> = lookup("한").take(3).collect();
        assert_eq!(han[0], Entry { reading: "한", value: "韓", note: "나라 이름 한, 한나라 한" });
        assert_eq!((han[1].value, han[2].value), ("漢", "寒"));
        assert_eq!(lookup("대한민국").map(|e| e.value).collect::<Vec<_>>(), ["大韓民國"]);
        assert_eq!(lookup("전기").next().map(|e| e.value), Some("電氣"));
        assert!(symbols('ㅁ').any(|e| e.value == "※"));
        assert_eq!(symbols('ㅁ').count(), 76);
        assert_eq!(symbols('ㅉ').count(), 0);
    }

    #[test]
    fn candidates_for_one_letter() {
        let none = Learning::default();
        let guk = candidates('국', &none);
        assert_eq!(guk[0], Cand { text: "國".into(), note: "나라 국".into() });
        assert_eq!(texts('국', &none, 3), ["國", "局", "菊"]);
        assert_eq!(guk.len(), 58);
        assert_eq!(candidates('한', &none)[0].note, "나라 이름 한, 한나라 한");
        // 한 음절 항목 중 값이 두 글자인 사전 오류(신:莘洞)는 뺀다.
        assert!(candidates('신', &none).iter().all(|c| c.text.chars().count() == 1));
        // 자음 하나는 기호표, 한자가 없는 음절·모음·그 밖은 빈 목록.
        assert_eq!(candidates('ㅁ', &none)[5], Cand { text: "※".into(), note: String::new() });
        assert!(candidates('뭐', &none).is_empty());
        assert!(candidates('ㅏ', &none).is_empty());
        assert!(candidates('a', &none).is_empty());
    }

    #[test]
    fn learning_moves_picks_forward() {
        let mut l = Learning::default();
        assert_eq!(texts('한', &l, 3), ["韓", "漢", "寒"]);
        l.record("한", "寒");
        assert_eq!(texts('한', &l, 3), ["寒", "韓", "漢"]);
        // 가장 최근(寒 1번) → 많이 고른 것(漢 5번, 限 2번) → 사전 순위(韓)
        for _ in 0..5 {
            l.record("한", "漢");
        }
        l.record("한", "限");
        l.record("한", "限");
        l.record("한", "寒");
        assert_eq!(texts('한', &l, 4), ["寒", "漢", "限", "韓"]);
        assert_eq!(l.len(), 3);
        // 기호도 같다. 다른 읽기의 기억은 섞이지 않는다.
        l.record("ㅁ", "★");
        assert_eq!(texts('ㅁ', &l, 1), ["★"]);
        assert_eq!(texts('국', &l, 1), ["國"]);
        // 가장 최근에 고른 것(윈도우 입력기가 엔진 호스트에 알린다).
        assert_eq!(l.last_pick(), Some(("ㅁ", "★")));
        l.record("한", "漢");
        assert_eq!(l.last_pick(), Some(("한", "漢")));
        assert_eq!(Learning::default().last_pick(), None);
    }

    #[test]
    fn learning_round_trips_and_is_bounded() {
        let mut l = Learning::default();
        l.record("한", "漢");
        l.record("대한", "大韓");
        l.record("한", "漢");
        let back = Learning::from_tsv(&l.to_tsv());
        assert_eq!(back, l);
        assert_eq!(Learning::from_tsv("garbage\n한\t韓\tx\t1\n\t韓\t1\t1\n").len(), 0);

        let mut big = Learning::default();
        for i in 0..Learning::MAX + 5 {
            big.record(&format!("읽{i}"), "字");
        }
        assert_eq!(big.len(), Learning::MAX);
        let tsv = big.to_tsv();
        let has = |reading: String| {
            let prefix = reading + "\t";
            tsv.lines().any(|l| l.starts_with(&prefix))
        };
        assert!(!has("읽0".into()) && !has("읽4".into()), "가장 오래된 것부터 버린다");
        assert!(has("읽5".into()) && has(format!("읽{}", Learning::MAX + 4)));
        assert_eq!(Learning::from_tsv(&tsv).len(), Learning::MAX);
    }
}
