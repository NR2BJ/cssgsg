//! 新月配列 가나 조합기.
//!
//! 읽기(가나 문자열)는 이 조합기가 들고 있다. 변환 엔진은 "읽기 → 후보"로만 쓴다.
//! 뒤치기 ゛가 이미 친 가나를 바꾸기 때문에, 변환 엔진 안의 조합기에 한 글자씩 먹이지 않는다.
//!
//! - ☆(K) 뒤에는 ☆면, ★(D) 뒤에는 ★면의 가나가 나온다. ☆☆=も, ★★=ら.
//! - ☆ 뒤 ゛(L)는 3타 단축 면(☆゛X)이다.
//! - ゛(L)는 읽기의 마지막 가나에 붙는다: か→が, は→ば→ぱ, あ→ぁ, う→ゔ→ぅ.
//!   붙을 가나가 없으면 ゛ 글자 자체를 넣고, ゛에 한 번 더 치면 ゜가 된다.
//! - 앞치기 뒤에 정의되지 않은 키가 오면 앞치기를 취소하고 그 키를 새로 처리한다.
//! - Backspace는 키 입력 하나를 되돌린다(が → か, ☆ 취소).
//! - 바로 확정(가타카나): 마지막 키가 붙인 글자(뒤치기가 아직 바꿀 수 있는 부분)만 남기고 앞은 꺼낼 수 있다
//!   (`take_settled`). 3타 단축처럼 한 키가 두 글자를 내면 둘을 함께 남긴다.

use super::layout::KanaLayout;
use crate::key::Key;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pending {
    #[default]
    None,
    Star,
    Black,
    StarDaku,
}

impl Pending {
    /// 조합 중 글자 끝에 붙여 보여 줄 앞치기 표시.
    pub fn marker(self) -> &'static str {
        match self {
            Pending::None => "",
            Pending::Star => "☆",
            Pending::Black => "★",
            Pending::StarDaku => "☆゛",
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Snapshot {
    reading: String,
    pending: Pending,
    unit_start: usize,
}

#[derive(Debug, Default, Clone)]
pub struct KanaComposer {
    reading: String,
    pending: Pending,
    /// 마지막 키가 붙인 글자가 읽기에서 시작하는 바이트 위치. 그 앞은 뒤치기로도 바뀌지 않는다.
    unit_start: usize,
    history: Vec<Snapshot>,
}

impl KanaComposer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 읽기도 앞치기도 없는지.
    pub fn is_empty(&self) -> bool {
        self.reading.is_empty() && self.pending == Pending::None
    }

    pub fn reading(&self) -> &str {
        &self.reading
    }

    pub fn pending(&self) -> Pending {
        self.pending
    }

    /// 화면에 보일 조합 중 글자: 읽기(+가타카나) + 앞치기 표시(☆/★).
    pub fn display(&self, katakana: bool) -> String {
        let mut s = if katakana { super::to_katakana(&self.reading) } else { self.reading.clone() };
        s.push_str(self.pending.marker());
        s
    }

    /// 읽기를 꺼내고 비운다. 걸려 있던 앞치기는 버린다.
    pub fn take(&mut self) -> String {
        let r = std::mem::take(&mut self.reading);
        self.clear();
        r
    }

    pub fn clear(&mut self) {
        self.reading.clear();
        self.pending = Pending::None;
        self.unit_start = 0;
        self.history.clear();
    }

    /// 더는 바뀌지 않는 앞부분(마지막 키가 붙인 글자 앞)을 꺼낸다. 되돌리기 기록도 그만큼 떼어 낸다.
    /// 앞부분이 다 차기 전의 기록은 꺼낸 글자를 되살리므로 버린다(그다음 Backspace는 앱으로 간다).
    pub fn take_settled(&mut self) -> String {
        if self.unit_start == 0 {
            return String::new();
        }
        let settled: String = self.reading.drain(..self.unit_start).collect();
        self.unit_start = 0;
        self.history = std::mem::take(&mut self.history)
            .into_iter()
            .filter_map(|s| {
                let rest = s.reading.strip_prefix(settled.as_str())?.to_string();
                Some(Snapshot {
                    reading: rest,
                    pending: s.pending,
                    unit_start: s.unit_start.saturating_sub(settled.len()),
                })
            })
            .collect();
        settled
    }

    /// 앞치기만 취소한다(읽기는 그대로). 취소했으면 true.
    pub fn cancel_pending(&mut self) -> bool {
        if self.pending == Pending::None {
            return false;
        }
        self.pending = Pending::None;
        true
    }

    /// 가나 표에 없는 글자(숫자, 기호)를 읽기에 더한다.
    pub fn push_str(&mut self, s: &str) {
        self.save();
        self.pending = Pending::None;
        self.append(s);
    }

    /// 키 입력 하나를 되돌린다. 되돌릴 게 없으면 false.
    pub fn backspace(&mut self) -> bool {
        match self.history.pop() {
            Some(s) => {
                self.restore(s);
                true
            }
            None => false,
        }
    }

    /// 가나 배열 키 하나를 처리한다. 이 배열이 쓰지 않는 키면 false를 돌려주고 아무것도 바꾸지 않는다.
    pub fn key(&mut self, l: &KanaLayout, key: Key) -> bool {
        match self.pending {
            Pending::None => {
                if key == l.star_key {
                    self.save();
                    self.pending = Pending::Star;
                } else if key == l.black_key {
                    self.save();
                    self.pending = Pending::Black;
                } else if key == l.daku_key {
                    self.save();
                    self.apply_daku(l);
                } else if let Some(kana) = l.base.get(&key) {
                    self.save();
                    self.append(kana);
                } else {
                    return false;
                }
                true
            }
            Pending::Star if key == l.daku_key => {
                self.save();
                self.pending = Pending::StarDaku;
                true
            }
            Pending::Star => self.shifted(l, key, |l| &l.star),
            Pending::Black => self.shifted(l, key, |l| &l.black),
            Pending::StarDaku => self.shifted(l, key, |l| &l.star_daku),
        }
    }

    fn shifted(
        &mut self,
        l: &KanaLayout,
        key: Key,
        table: fn(&KanaLayout) -> &std::collections::HashMap<Key, String>,
    ) -> bool {
        if let Some(kana) = table(l).get(&key) {
            self.save();
            self.pending = Pending::None;
            self.append(kana);
            return true;
        }
        // 정의되지 않은 조합: 걸려 있는 앞치기를 모두 없던 일로 하고 이 키를 새로 처리한다.
        while self.pending != Pending::None {
            match self.history.pop() {
                Some(s) => self.restore(s),
                None => self.pending = Pending::None,
            }
        }
        self.key(l, key)
    }

    fn apply_daku(&mut self, l: &KanaLayout) {
        if let Some(last) = self.reading.chars().last() {
            let replaced = l.postfix.get(&last).copied().or(if last == '゛' { Some('゜') } else { None });
            if let Some(to) = replaced {
                self.reading.pop();
                // 바뀐 글자는 마지막 키가 붙인 글자에 속한다.
                self.unit_start = self.unit_start.min(self.reading.len());
                self.reading.push(to);
                return;
            }
        }
        self.append("゛");
    }

    /// 새 키가 글자를 붙인다. 그 글자가 새 "마지막 키의 글자"가 된다.
    fn append(&mut self, s: &str) {
        self.unit_start = self.reading.len();
        self.reading.push_str(s);
    }

    fn save(&mut self) {
        self.history.push(Snapshot {
            reading: self.reading.clone(),
            pending: self.pending,
            unit_start: self.unit_start,
        });
    }

    fn restore(&mut self, s: Snapshot) {
        self.reading = s.reading;
        self.pending = s.pending;
        self.unit_start = s.unit_start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 쿼티 글자로 적은 키열을 넣고 표시 글자를 돌려준다. '<'는 Backspace.
    fn t(keys: &str) -> String {
        let l = KanaLayout::shingetsu();
        let mut c = KanaComposer::new();
        for ch in keys.chars() {
            if ch == '<' {
                c.backspace();
                continue;
            }
            let key = Key::from_qwerty(ch).unwrap();
            assert!(c.key(&l, key), "가나 키가 아님: {ch:?}");
        }
        c.display(false)
    }

    #[test]
    fn unshifted_kana() {
        assert_eq!(t("sfgi"), "かとたい");
        assert_eq!(t(";o"), "きの");
        assert_eq!(t("[y,.]"), "「つ、。」");
    }

    #[test]
    fn prefix_shifts() {
        assert_eq!(t("ka"), "あ");
        assert_eq!(t("kk"), "も");
        assert_eq!(t("dj"), "お");
        assert_eq!(t("dd"), "ら");
        assert_eq!(t("dk"), "も");
        assert_eq!(t("kd"), "ら");
        assert_eq!(t("dp"), "ー");
        assert_eq!(t("k"), "☆");
        assert_eq!(t("d"), "★");
        assert_eq!(t("kl"), "☆゛");
    }

    #[test]
    fn postfix_daku() {
        assert_eq!(t("sl"), "が");
        assert_eq!(t("al"), "ば");
        assert_eq!(t("all"), "ぱ");
        assert_eq!(t("kal"), "ぁ");
        assert_eq!(t("jl"), "ゔ");
        assert_eq!(t("jll"), "ぅ");
        assert_eq!(t("yl"), "づ");
        assert_eq!(t("l"), "゛");
        assert_eq!(t("ll"), "゜");
        // 뒤치기 표에 없는 가나: ゛ 글자를 덧붙인다.
        assert_eq!(t("sll"), "が゛");
    }

    #[test]
    fn three_stroke_shortcuts() {
        assert_eq!(t("klq"), "ぴょ");
        assert_eq!(t("klc"), "じゃ");
        assert_eq!(t("klh"), "みゃ");
        assert_eq!(t("klt"), "でぃ");
        assert_eq!(t("kll"), ";");
    }

    #[test]
    fn undefined_prefix_combo_restarts_key() {
        // ☆ 뒤 오른손 키(y)는 ☆면에 없다: ☆ 취소 후 y=つ.
        assert_eq!(t("ky"), "つ");
        // ★ 뒤 왼손 키(s)도 마찬가지: s=か.
        assert_eq!(t("ds"), "か");
    }

    #[test]
    fn backspace_one_key() {
        assert_eq!(t("sl<"), "か");
        assert_eq!(t("k<"), "");
        assert_eq!(t("ka<"), "☆");
        assert_eq!(t("ka<<"), "");
        assert_eq!(t("sf<"), "か");
        // 취소된 앞치기는 되돌리기 기록에 남지 않는다.
        assert_eq!(t("skly"), "かつ");
        assert_eq!(t("skly<"), "か");
        assert_eq!(t("skly<<"), "");
    }

    /// 가타카나 바로 확정처럼 매 키 뒤에 앞부분을 꺼낸다. (꺼낸 글자, 남은 표시). '<'는 Backspace.
    fn settle(keys: &str) -> (String, String) {
        let l = KanaLayout::shingetsu();
        let mut c = KanaComposer::new();
        let mut settled = String::new();
        for ch in keys.chars() {
            if ch == '<' {
                c.backspace();
            } else {
                assert!(c.key(&l, Key::from_qwerty(ch).unwrap()), "가나 키가 아님: {ch:?}");
            }
            settled += &c.take_settled();
        }
        (settled, c.display(false))
    }

    #[test]
    fn settle_keeps_only_the_last_key_open() {
        // に ほ ん は새 가나가 붙을 때 확정되고, 마지막 ご(こ+゛)만 남는다.
        assert_eq!(settle("ckeuwl"), ("にほん".into(), "ご".into()));
        // 3타 단축은 한 키가 두 글자를 낸다: 둘을 함께 남기고, 다음 가나가 오면 함께 확정한다.
        assert_eq!(settle("klq"), ("".into(), "ぴょ".into()));
        assert_eq!(settle("klqs"), ("ぴょ".into(), "か".into()));
        // 뒤치기는 남아 있는 마지막 글자에 붙는다.
        assert_eq!(settle("sfl"), ("か".into(), "ど".into()));
        // 앞치기 중에는 앞 글자가 남아 있다가, 가나가 나오면 확정된다.
        assert_eq!(settle("sk"), ("".into(), "か☆".into()));
        assert_eq!(settle("ska"), ("か".into(), "あ".into()));
        // 정의되지 않은 앞치기 조합: ☆를 취소하고 새로 친 가나 앞까지 확정한다.
        assert_eq!(settle("sfky"), ("かと".into(), "つ".into()));
    }

    #[test]
    fn backspace_after_settle_undoes_only_the_open_part() {
        let l = KanaLayout::shingetsu();
        let mut c = KanaComposer::new();
        for ch in "sfl".chars() {
            c.key(&l, Key::from_qwerty(ch).unwrap());
            c.take_settled();
        }
        assert_eq!(c.display(false), "ど");
        assert!(c.backspace());
        assert_eq!(c.display(false), "と");
        assert!(c.backspace());
        assert_eq!(c.display(false), "");
        // 확정한 か는 조합기에 없다: 다음 Backspace는 앱이 처리한다.
        assert!(!c.backspace());
    }

    #[test]
    fn words() {
        // にほんご = に(c) ほ(☆e) ん(u) ご(こ w + ゛ l)
        assert_eq!(t("ckeuwl"), "にほんご");
        // ありがとう = あ(☆a) り(p) が(か s + ゛ l) と(f) う(j)
        assert_eq!(t("kapslfj"), "ありがとう");
        // こんにちは = こ(w) ん(u) に(c) ち(★;) は(a)
        assert_eq!(t("wucd;a"), "こんにちは");
    }
}
