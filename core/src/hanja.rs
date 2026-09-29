//! 한국어 한자 변환: 사전 찾기, 후보 만들기, 고른 후보 기억.
//!
//! 사전은 libhangul의 `hanja.txt`와 `mssymbol.txt`(`dict/ko`, BSD-3)를 고치지 않고 바이너리에 넣는다.
//! 두 파일 모두 `읽기:값:뜻` 줄이 읽기 순으로 정렬돼 있어서 그대로 이진 탐색한다(정렬은 테스트가 확인한다).
//! 같은 읽기 안의 줄 순서를 순위로 쓴다. 한 음절은 대체로 빈도 순이지만 단어는 아니다(대한: 大寒이 大韓보다 앞).
//! 그래서 고른 후보를 기억해 앞으로 올린다([`Learning`]).
//!
//! 후보는 바꿀 구간이 서로 다른 것을 한 목록에 모은다. 커서 앞 "대한민국"이면 大韓民國(4음절) 다음에
//! 民國(끝 2음절), 그다음 國·局·菊…(끝 1음절). 구간을 따로 조절하지 않아도 고르는 후보가 곧 구간이다.

use std::cmp::Reverse;
use std::collections::HashMap;

const HANJA_TXT: &str = include_str!("../../dict/ko/hanja.txt");
const SYMBOL_TXT: &str = include_str!("../../dict/ko/mssymbol.txt");

/// 사전에서 가장 긴 읽기(음절 수). 이보다 긴 앞 글자는 보지 않는다.
pub const MAX_READING: usize = 18;

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

/// 한자 한 글자의 첫 훈음(`클 대, 큰 대` → `클 대`). 사전에 훈음이 없으면 None.
pub fn gloss(syllable: char, hanja: char) -> Option<&'static str> {
    let (mut rb, mut hb) = ([0u8; 4], [0u8; 4]);
    let (reading, value) = (&*syllable.encode_utf8(&mut rb), &*hanja.encode_utf8(&mut hb));
    let note = lookup(reading).find(|e| e.value == value)?.note;
    note.split(", ").next().filter(|s| !s.is_empty())
}

pub fn is_syllable(c: char) -> bool {
    ('가'..='힣').contains(&c)
}

/// 호환 자모의 자음(ㄱ~ㅎ). 기호표의 읽기다.
pub fn is_consonant(c: char) -> bool {
    ('ㄱ'..='ㅎ').contains(&c)
}

/// 후보 하나. 원문 안에서 바꿀 구간(글자 단위)과 바꿀 글자.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cand {
    pub start: usize,
    pub len: usize,
    pub text: String,
    /// 후보창에 같이 보일 뜻. 없으면 빈 문자열.
    pub note: String,
}

/// 변환할 원문과 후보(보일 순서).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub source: Vec<char>,
    pub cands: Vec<Cand>,
}

/// 커서 앞 글자에서 찾는다(끝이 커서). 끝에서 이어진 한글 음절 중 사전과 맞는 끝부분을 긴 것부터 모은다.
/// 원문은 가장 긴 맞는 끝부분이다. 끝 글자가 자음 하나면 기호표를 본다.
pub fn at_caret(before: &[char], learning: &Learning) -> Option<Found> {
    let &last = before.last()?;
    if is_consonant(last) {
        return symbol_found(last, learning);
    }
    let run_len = before.iter().rev().take(MAX_READING).take_while(|&&c| is_syllable(c)).count();
    let run = &before[before.len() - run_len..];
    let mut groups = Vec::new();
    for len in (1..=run.len()).rev() {
        let reading = &run[run.len() - len..];
        let cands = group(reading, run.len() - len, learning);
        if !cands.is_empty() {
            groups.push((len, cands));
        }
    }
    let longest = groups.first()?.0;
    let offset = run.len() - longest;
    let cands = groups
        .into_iter()
        .flat_map(|(_, cands)| cands)
        .map(|c| Cand { start: c.start - offset, ..c })
        .collect();
    Some(Found { source: run[offset..].to_vec(), cands })
}

/// 선택한 글에서 찾는다. 앞 공백 뒤로 이어진 한글 음절 중 사전과 맞는 앞부분을 긴 것부터 모은다
/// (선택 전체가 맞으면 그것이 첫 후보). 원문은 선택 전체다. 선택이 자음 하나면 기호표를 본다.
pub fn in_selection(selected: &[char], learning: &Learning) -> Option<Found> {
    if let [c] = selected
        && is_consonant(*c)
    {
        return symbol_found(*c, learning);
    }
    let lead = selected.iter().take_while(|c| c.is_whitespace()).count();
    let run_len = selected[lead..].iter().take(MAX_READING).take_while(|&&c| is_syllable(c)).count();
    let cands: Vec<Cand> =
        (1..=run_len).rev().flat_map(|len| group(&selected[lead..lead + len], lead, learning)).collect();
    (!cands.is_empty()).then(|| Found { source: selected.to_vec(), cands })
}

fn symbol_found(jamo: char, learning: &Learning) -> Option<Found> {
    let mut cands: Vec<Cand> = symbols(jamo)
        .map(|e| Cand { start: 0, len: 1, text: e.value.to_string(), note: String::new() })
        .collect();
    if cands.is_empty() {
        return None;
    }
    learning.order(&jamo.to_string(), &mut cands);
    Some(Found { source: vec![jamo], cands })
}

/// 읽기 하나의 후보(기억한 것 먼저, 그다음 사전 순위).
fn group(reading: &[char], start: usize, learning: &Learning) -> Vec<Cand> {
    let key: String = reading.iter().collect();
    let mut cands: Vec<Cand> = lookup(&key)
        .filter(|e| !e.value.is_empty())
        .map(|e| Cand { start, len: reading.len(), text: e.value.to_string(), note: note(reading, e) })
        .collect();
    learning.order(&key, &mut cands);
    cands
}

/// 후보창에 보일 뜻. 한 글자는 사전의 훈음 전체, 단어는 사전 설명(`지명` 등)이 있으면 그것,
/// 없으면 글자마다 첫 훈음을 이어 붙인다(電氣 → 번개 전 · 기운 기). 한글이 섞인 값(可決되다)은 한자만.
fn note(reading: &[char], e: Entry) -> String {
    let value: Vec<char> = e.value.chars().collect();
    if reading.len() == 1 && value.len() == 1 {
        return e.note.to_string();
    }
    if !e.note.is_empty() && e.note != e.reading {
        return e.note.to_string();
    }
    if value.len() != reading.len() {
        return String::new();
    }
    let parts: Vec<String> = reading
        .iter()
        .zip(&value)
        .filter(|(_, h)| !is_syllable(**h))
        .map(|(&r, &h)| gloss(r, h).map_or_else(|| r.to_string(), str::to_string))
        .collect();
    parts.join(" · ")
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

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn texts(found: &Found) -> Vec<(usize, usize, &str)> {
        found.cands.iter().map(|c| (c.start, c.len, c.text.as_str())).collect()
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
        assert_eq!(gloss('대', '大'), Some("클 대"));
        assert_eq!(gloss('한', '韓'), Some("나라 이름 한"));
        assert_eq!(gloss('한', '大'), None);
    }

    #[test]
    fn caret_collects_every_matching_suffix_longest_first() {
        let none = Learning::default();
        let found = at_caret(&chars("나는대한민국"), &none).unwrap();
        assert_eq!(found.source, chars("대한민국"));
        let t = texts(&found);
        assert_eq!(t[0], (0, 4, "大韓民國"));
        assert_eq!(t[1], (2, 2, "民國"));
        assert_eq!(t[2], (3, 1, "國"));
        assert!(t[3..].iter().all(|&(s, l, _)| (s, l) == (3, 1)));
        assert_eq!(found.cands[0].note, "클 대 · 나라 이름 한 · 백성 민 · 나라 국");
        assert_eq!(found.cands[2].note, "나라 국");

        // 한글이 아닌 글자에서 끊는다. 끝이 자음이면 기호.
        assert_eq!(at_caret(&chars("abc전기"), &none).unwrap().cands[0].text, "電氣");
        assert_eq!(at_caret(&chars("전기 "), &none), None);
        assert_eq!(at_caret(&chars(""), &none), None);
        let sym = at_caret(&chars("한ㅁ"), &none).unwrap();
        assert_eq!((sym.source.clone(), sym.cands[5].text.as_str()), (chars("ㅁ"), "※"));
    }

    #[test]
    fn selection_collects_prefixes_and_keeps_the_rest() {
        let none = Learning::default();
        let found = in_selection(&chars(" 전기 요금"), &none).unwrap();
        assert_eq!(found.source, chars(" 전기 요금"));
        assert_eq!(texts(&found)[0], (1, 2, "電氣"));
        assert!(texts(&found).iter().any(|&(s, l, _)| (s, l) == (1, 1)));
        assert_eq!(in_selection(&chars("abc"), &none), None);
        assert_eq!(in_selection(&chars("ㅁ"), &none).unwrap().cands[5].text, "※");
    }

    #[test]
    fn learning_moves_picks_forward() {
        let mut l = Learning::default();
        let order = |l: &Learning| {
            at_caret(&chars("한"), l)
                .unwrap()
                .cands
                .iter()
                .take(3)
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&l), ["韓", "漢", "寒"]);
        l.record("한", "寒");
        assert_eq!(order(&l), ["寒", "韓", "漢"]);
        // 가장 최근(寒 1번) → 많이 고른 것(漢 5번, 限 2번) → 사전 순위(韓)
        for _ in 0..5 {
            l.record("한", "漢");
        }
        l.record("한", "限");
        l.record("한", "限");
        l.record("한", "寒");
        let four: Vec<String> =
            at_caret(&chars("한"), &l).unwrap().cands.iter().take(4).map(|c| c.text.clone()).collect();
        assert_eq!(four, ["寒", "漢", "限", "韓"]);
        assert_eq!(l.len(), 3);

        // 다른 길이 무리 안에서만 움직인다
        l.record("민국", "民國");
        let found = at_caret(&chars("대한민국"), &l).unwrap();
        assert_eq!(found.cands[0].text, "大韓民國");
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
