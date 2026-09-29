//! 한국어 한자 변환(Option+Enter).
//!
//! 1. 한자 키를 받으면 `hanja_context`로 셸에 앱 글자를 달라고 한다(코어는 앱 글자를 모른다).
//! 2. 셸이 [`Engine::hanja_begin`]으로 조합 글자(없으면 커서) 앞 글자와 선택한 글을 준다.
//!    조합 중이면 앞 한글 + 조합 음절, 선택이 있으면 선택, 아니면 커서 앞 한글에서 사전과 맞는 구간을 찾는다.
//! 3. 찾은 원문을 조합으로 끌어온다(`preedit_replace_before`만큼 앞 글자를 덮는다). 첫 후보가 바로 조합에 보이고 후보창이 뜬다.
//! 4. 고르면 확정하고 기억한다. 취소하면 원문을 그대로 돌려놓고, 조합 중이던 음절은 다시 조합으로 둔다.
//!
//! 앱이 앞 글자를 읽어 주지 않으면(`before`가 None) 엔진이 기억한 "방금 친 한글"(`ko_run`)을 쓴다.
//! 셸이 기준 자리를 몰라 끌어올 수 없으면 빈 글자(`Some("")`)로 다시 부른다: 조합 음절만 바꾸는 변환으로 다시 시작한다.
//! 끌어온 뒤에 앱이 따랐는지 markedRange로 확인하지 않는다. Chromium·WebKit은 조합을 늦게 반영해서 그 값이 옛것이고,
//! 그걸 믿고 다시 시작하면 WebKit에서는 앞 글자가 지워진다.

use super::{
    CAND_GRID_COLUMNS, CAND_GRID_PAGE, CAND_LIST_PAGE, Candidates, Engine, HanjaAnchor, Mode, Output,
    Preedit, Segment,
};
use crate::hangul::KoComposer;
use crate::hanja::{self, Cand, Learning};
use crate::key::{Key, KeyEvent};

/// 선택 변환은 이만큼(UTF-16)까지. 더 길면 바꾸지 않는다(조합으로 끌어오기에는 너무 길다).
const MAX_SELECTION: usize = 64;

/// 맥 한자 키: Option+Enter(macOS 기본 한국어 입력기와 같다). Shift는 같이 눌러도 된다.
pub(super) fn is_hanja_key(ev: &KeyEvent) -> bool {
    matches!(ev.key, Key::ENTER | Key::NUMPAD_ENTER) && ev.mods.alt() && !ev.mods.ctrl() && !ev.mods.meta()
}

/// 한자 변환 중 상태.
pub(super) struct HanjaConv {
    /// 앱에서 끌어와 조합으로 바꾼 원문(글자 단위). 조합 중이던 음절이 있으면 끝 글자다.
    source: Vec<char>,
    cands: Vec<Cand>,
    selected: usize,
    /// 커서 앞 변환(후보 구간이 원문 끝을 공유)인지, 선택 변환(앞을 공유)인지.
    anchored_end: bool,
    /// 변환을 시작할 때의 조합기. 취소하면 되돌린다.
    saved_ko: KoComposer,
    /// 원문 끝에서 조합 중이던 글자 수(0 또는 1).
    composing: usize,
    /// 변환을 시작할 때의 "방금 친 한글". 다시 시작하면 되돌린다.
    saved_run: Vec<char>,
}

impl HanjaConv {
    /// 지금 고른 후보를 넣은 원문. 바뀌는 구간이 포커스(굵은 밑줄)다.
    pub(super) fn preview(&self) -> Preedit {
        let c = &self.cands[self.selected];
        let head: String = self.source[..c.start].iter().collect();
        let tail: String = self.source[c.start + c.len..].iter().collect();
        let (h, m, t) = (c.start, c.text.chars().count(), self.source.len() - c.start - c.len);
        let mut segments = Vec::new();
        if h > 0 {
            segments.push(Segment { start: 0, len: h, focused: false });
        }
        segments.push(Segment { start: h, len: m, focused: true });
        if t > 0 {
            segments.push(Segment { start: h + m, len: t, focused: false });
        }
        Preedit { text: head + &c.text + &tail, segments }
    }

    pub(super) fn candidates(&self, grid: bool) -> Candidates {
        let size = if grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
        Candidates {
            items: self.cands.iter().map(|c| c.text.clone()).collect(),
            notes: self.cands.iter().map(|c| c.note.clone()).collect(),
            selected: Some(self.selected),
            page: Some((self.selected / size + 1, self.cands.len().div_ceil(size))),
            grid,
        }
    }

    /// Shift+←→: 바꿀 구간의 움직이는 경계(커서 앞 변환은 앞 경계, 선택 변환은 뒤 경계)를
    /// 왼쪽/오른쪽으로 한 단계 옮긴 무리의 첫 후보.
    fn edge_jump(&self, left: bool) -> Option<usize> {
        let edge = |c: &Cand| if self.anchored_end { c.start } else { c.start + c.len };
        let here = edge(&self.cands[self.selected]);
        let to = self
            .cands
            .iter()
            .map(edge)
            .filter(|&e| if left { e < here } else { e > here })
            .reduce(|a, b| if left { a.max(b) } else { a.min(b) })?;
        self.cands.iter().position(|c| edge(c) == to)
    }
}

impl Engine {
    /// 고른 후보 기억(셸이 파일로 저장한다).
    pub fn hanja_learning(&self) -> &Learning {
        &self.hanja_learning
    }

    pub fn set_hanja_learning(&mut self, learning: Learning) {
        self.hanja_learning = learning;
    }

    /// 한자 키: 셸에 앱 글자를 달라고 한다.
    pub(super) fn hanja_request(&mut self) -> Output {
        self.hanja_pending = true;
        let anchor = if self.ko.is_composing() { HanjaAnchor::Composing } else { HanjaAnchor::Caret };
        Output { consumed: true, hanja_context: Some(anchor), ..Output::default() }
    }

    /// 한자 변환을 시작한다. 한자 키의 `hanja_context`를 받은 셸이 곧바로 부른다.
    /// - `before`: 기준 자리(조합 글자, 없으면 커서·선택 시작) 바로 앞 글자.
    ///   None이면 앱이 읽어 주지 않은 것이라 엔진이 기억한 방금 친 한글을 쓴다. 기준 자리를 몰라 끌어올 수 없으면 `Some("")`.
    /// - `selected`: 선택한 글(조합 중이 아닐 때만). 없으면 빈 문자열.
    ///
    /// 변환 중에 다시 부르면 처음 상태로 되돌리고 새 글자로 다시 시작한다(셸이 기준 자리를 몰라 끌어올 수 없을 때).
    /// 바꿀 것이 없으면 `consumed`가 false일 수 있다(셸은 한자 키를 앱에 넘긴다).
    pub fn hanja_begin(&mut self, before: Option<&str>, selected: &str) -> Output {
        let snap = self.snapshot();
        let out = self.hanja_start(before, selected);
        self.finish(out, snap)
    }

    fn hanja_start(&mut self, before: Option<&str>, selected: &str) -> Output {
        let restarting = match self.hanja.take() {
            Some(h) => {
                self.ko = h.saved_ko;
                self.ko_run = h.saved_run;
                true
            }
            None => false,
        };
        let pending = std::mem::take(&mut self.hanja_pending);
        if !(restarting || pending) || self.mode != Mode::Ko {
            return Output::pass();
        }
        let composing: Vec<char> = self.ko.preedit().chars().collect();
        let context: Vec<char> = match before {
            Some(text) => text.chars().collect(),
            None => self.ko_run.clone(),
        };
        let learning = &self.hanja_learning;
        let by_selection = composing.is_empty() && !selected.is_empty();
        let found = match composing.as_slice() {
            [] if by_selection => {
                let too_long =
                    selected.encode_utf16().count() > MAX_SELECTION || selected.contains(['\n', '\r']);
                if too_long {
                    None
                } else {
                    hanja::in_selection(&selected.chars().collect::<Vec<_>>(), learning)
                }
            }
            [] => hanja::at_caret(&context, learning),
            [c] if hanja::is_syllable(*c) => {
                let mut text = context;
                text.push(*c);
                hanja::at_caret(&text, learning)
            }
            [c] if hanja::is_consonant(*c) => hanja::at_caret(&[*c], learning),
            _ => None,
        };
        let Some(found) = found else {
            // 바꿀 것이 없다. 조합 중이거나 선택이 있으면 키를 먹는다(선택이 줄바꿈으로 바뀌지 않게). 아니면 앱에 넘긴다.
            return Output {
                consumed: restarting || !composing.is_empty() || by_selection,
                ..Output::default()
            };
        };
        let tail = composing.len();
        let absorbed: &[char] = if by_selection { &[] } else { &found.source[..found.source.len() - tail] };
        let replace_before = absorbed.iter().map(|c| c.len_utf16()).sum();
        // 끌어온 글자는 조합이 됐으니 "방금 친 한글"에서 뺀다(취소하면 확정으로 다시 붙는다). 선택은 어디인지 모르니 비운다.
        let saved_run = self.ko_run.clone();
        if !by_selection && self.ko_run.ends_with(absorbed) {
            self.ko_run.truncate(self.ko_run.len() - absorbed.len());
        } else {
            self.ko_run.clear();
        }
        self.hanja = Some(HanjaConv {
            source: found.source,
            cands: found.cands,
            selected: 0,
            anchored_end: !by_selection,
            saved_ko: std::mem::take(&mut self.ko),
            composing: tail,
            saved_run,
        });
        self.cand_grid = false;
        Output { consumed: true, preedit_replace_before: replace_before, ..Output::default() }
    }

    /// 변환 중 키. 후보창 조작은 일본어 변환과 같다.
    /// - Space / ↓ / 한자 키: 다음 후보, ↑: 이전 후보(끝에서 처음으로 돈다). Tab: 목록 ↔ 격자.
    /// - ←→: 목록에서는 페이지, 격자에서는 한 칸. PageUp/PageDown: 페이지.
    /// - Shift+←→: 바꿀 구간의 경계를 옮긴다(긴 단어 ↔ 짧은 단어의 첫 후보로).
    /// - 1~9: 지금 페이지에서 골라 확정. Enter: 확정. Esc/Backspace: 원래 글자로 되돌린다. 그 밖의 키: 확정하고 새로 처리.
    pub(super) fn hanja_key(&mut self, ev: &KeyEvent) -> Output {
        let (k, shift) = (ev.key, ev.mods.shift());
        let Some(h) = self.hanja.as_ref() else { return self.ko_key(ev) };
        let (n, sel, grid) = (h.cands.len(), h.selected, self.cand_grid);
        let page = if grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
        let within = |i: usize| (i < n).then_some(i);
        let target = match k {
            Key::TAB => {
                self.cand_grid = !grid;
                return Output::eat();
            }
            _ if is_hanja_key(ev) => Some((sel + 1) % n),
            Key::SPACE => Some((sel + 1) % n),
            Key::ARROW_DOWN if grid => within(sel + CAND_GRID_COLUMNS),
            Key::ARROW_UP if grid => sel.checked_sub(CAND_GRID_COLUMNS),
            Key::ARROW_DOWN => Some((sel + 1) % n),
            Key::ARROW_UP => Some((sel + n - 1) % n),
            Key::ARROW_LEFT | Key::ARROW_RIGHT if shift => h.edge_jump(k == Key::ARROW_LEFT),
            Key::ARROW_LEFT if grid => sel.checked_sub(1),
            Key::ARROW_RIGHT if grid => within(sel + 1),
            Key::ARROW_LEFT | Key::PAGE_UP => Some(sel.saturating_sub(page)),
            Key::ARROW_RIGHT | Key::PAGE_DOWN => Some((sel + page).min(n - 1)),
            _ => return self.hanja_other(ev),
        };
        if let (Some(t), Some(h)) = (target, self.hanja.as_mut()) {
            h.selected = t;
        }
        Output::eat()
    }

    fn hanja_other(&mut self, ev: &KeyEvent) -> Output {
        let (k, shift) = (ev.key, ev.mods.shift());
        match k {
            Key::ENTER | Key::NUMPAD_ENTER => {
                // Enter는 확정만 한다. Shift+Enter는 확정하고 Enter를 앱에 넘긴다(일본어와 같다).
                let commit = self.take_composition();
                return if shift { Output::commit_pass(commit) } else { Output::commit_eat(commit) };
            }
            Key::ESCAPE | Key::BACKSPACE => return self.hanja_cancel(),
            _ => {}
        }
        if k.is_digit() && !shift && k != Key::DIGIT0 {
            let page = if self.cand_grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
            if let Some(h) = self.hanja.as_mut() {
                let target = h.selected / page * page + (k.0 - Key::DIGIT1.0) as usize;
                if target < h.cands.len() {
                    h.selected = target;
                    return Output::commit_eat(self.take_composition());
                }
            }
        }
        // 그 밖의 키: 변환을 확정하고 그 키를 새로 처리한다.
        let committed = self.take_composition();
        let mut out = self.ko_key(ev);
        out.commit = committed + &out.commit;
        out
    }

    /// 취소: 끌어온 글자는 그대로 확정해 돌려놓고, 조합 중이던 음절은 다시 조합으로 둔다.
    fn hanja_cancel(&mut self) -> Output {
        let Some(h) = self.hanja.take() else { return Output::eat() };
        let keep = h.source.len() - h.composing;
        self.ko = h.saved_ko;
        Output::commit_eat(h.source[..keep].iter().collect())
    }

    /// 변환 결과를 확정 글자로 꺼내고 고른 것을 기억한다. 변환 중이 아니면 빈 문자열.
    pub(super) fn hanja_commit(&mut self) -> String {
        let Some(h) = self.hanja.take() else { return String::new() };
        let c = &h.cands[h.selected];
        let reading: String = h.source[c.start..c.start + c.len].iter().collect();
        self.hanja_learning.record(&reading, &c.text);
        self.learning_dirty = true;
        h.preview().text
    }
}
