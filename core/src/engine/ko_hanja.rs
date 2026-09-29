//! 한국어 한자 변환(Option+Enter): 조합 중인 글자 하나를 바꾼다.
//!
//! 음절이면 한자, 자음 하나면 기호표. 조합 중이 아니면 한자 키는 앱으로 간다.
//! 한국어 한자는 일본어처럼 여러 글자를 묶은 낱말로 고를 일이 드물어서 글자 하나만 바꾼다(2026-09-29 사용자 결정).
//! 0.3.0은 앞 글자까지 조합으로 끌어와 낱말로 바꿨는데, Discord(Chromium)가 setMarkedText의 replacementRange를
//! 무시해 "대한민大韓民國"처럼 겹쳤다. 조합 글자 하나만 바꾸면 앱에 따로 기대는 것이 없다.

use super::{
    CAND_GRID_COLUMNS, CAND_GRID_PAGE, CAND_LIST_PAGE, Candidates, Engine, Output, Preedit, Segment,
};
use crate::hangul::KoComposer;
use crate::hanja::{self, Cand, Learning};
use crate::key::{Key, KeyEvent};

/// 한자 변환 중 상태.
pub(super) struct HanjaConv {
    /// 바꾸는 글자(조합 중이던 음절이나 자음).
    reading: char,
    cands: Vec<Cand>,
    selected: usize,
    /// 변환을 시작할 때의 조합기. 취소하면 되돌린다.
    saved_ko: KoComposer,
}

impl HanjaConv {
    /// 지금 고른 후보(굵은 밑줄).
    pub(super) fn preview(&self) -> Preedit {
        let text = self.cands[self.selected].text.clone();
        let len = text.chars().count();
        Preedit { text, segments: vec![Segment { start: 0, len, focused: true }] }
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
}

impl Engine {
    /// 고른 후보 기억(셸이 파일로 저장한다).
    pub fn hanja_learning(&self) -> &Learning {
        &self.hanja_learning
    }

    pub fn set_hanja_learning(&mut self, learning: Learning) {
        self.hanja_learning = learning;
    }

    /// 한자 단축키(기본 Option+Return, 설정의 shortcuts.hanja): 변환 중이면 다음 후보, 아니면 변환을 시작한다.
    pub(super) fn hanja_shortcut(&mut self) -> Output {
        match self.hanja.as_mut() {
            Some(h) => {
                h.selected = (h.selected + 1) % h.cands.len();
                Output::eat()
            }
            None => self.hanja_start(),
        }
    }

    /// 한자 키: 조합 중인 글자 하나를 바꾸기 시작한다. 첫 후보가 바로 조합에 보이고 후보창이 뜬다.
    /// 조합 중이 아니면 키를 앱에 넘기고, 조합 중인데 후보가 없으면(한자가 없는 음절) 키만 먹는다.
    pub(super) fn hanja_start(&mut self) -> Output {
        let composing: Vec<char> = self.ko.preedit().chars().collect();
        let [letter] = composing[..] else {
            return if composing.is_empty() { Output::pass() } else { Output::eat() };
        };
        let cands = hanja::candidates(letter, &self.hanja_learning);
        if !cands.is_empty() {
            let saved_ko = std::mem::take(&mut self.ko);
            self.hanja = Some(HanjaConv { reading: letter, cands, selected: 0, saved_ko });
            self.cand_grid = false;
        }
        Output::eat()
    }

    /// 변환 중 키. 후보창 조작은 일본어 변환과 같다.
    /// - Space / ↓: 다음 후보(한자 단축키도, `hanja_shortcut`), ↑: 이전 후보(끝에서 처음으로 돈다). Tab: 목록 ↔ 격자.
    /// - ←→: 목록에서는 페이지, 격자에서는 한 칸. PageUp/PageDown: 페이지.
    /// - 1~9: 지금 페이지에서 골라 확정. Enter: 확정. Esc/Backspace: 원래 글자(조합)로. 그 밖의 키: 확정하고 새로 처리.
    pub(super) fn hanja_key(&mut self, ev: &KeyEvent) -> Output {
        let Some(h) = self.hanja.as_ref() else { return self.ko_key(ev) };
        let (n, sel, grid) = (h.cands.len(), h.selected, self.cand_grid);
        let page = if grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
        let within = |i: usize| (i < n).then_some(i);
        let target = match ev.key {
            Key::TAB => {
                self.cand_grid = !grid;
                return Output::eat();
            }
            Key::SPACE => Some((sel + 1) % n),
            Key::ARROW_DOWN if grid => within(sel + CAND_GRID_COLUMNS),
            Key::ARROW_UP if grid => sel.checked_sub(CAND_GRID_COLUMNS),
            Key::ARROW_DOWN => Some((sel + 1) % n),
            Key::ARROW_UP => Some((sel + n - 1) % n),
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
            Key::ESCAPE | Key::BACKSPACE => {
                // 취소: 조합 중이던 글자로 돌아간다.
                if let Some(h) = self.hanja.take() {
                    self.ko = h.saved_ko;
                }
                return Output::eat();
            }
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

    /// 변환 결과를 확정 글자로 꺼내고 고른 것을 기억한다. 변환 중이 아니면 빈 문자열.
    pub(super) fn hanja_commit(&mut self) -> String {
        let Some(h) = self.hanja.take() else { return String::new() };
        let text = h.cands[h.selected].text.clone();
        self.hanja_learning.record(&h.reading.to_string(), &text);
        self.learning_dirty = true;
        text
    }
}
