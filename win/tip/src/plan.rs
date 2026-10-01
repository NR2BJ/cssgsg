//! 엔진 출력([`Output`])을 문서에 할 일로 바꾼다. TSF와 떼어 둔 순수 함수라서 그대로 시험한다.
//!
//! 맥 셸의 규칙(insertText가 조합 중 글자를 대신한다)을 TSF로 옮기면:
//! - 확정: 조합이 있으면 조합 글자를 확정할 글자로 바꾸고 조합을 끝낸다. 없으면 커서 자리에 넣는다.
//! - 그다음 새 조합 글자가 있으면 커서 자리에서 조합을 (다시) 시작해 그 글자로 바꾼다.
//! - 확정 없이 조합 글자가 비면 조합 글자를 지우고 조합을 끝낸다.
//!
//! 조합 구간 위치는 엔진은 글자(char) 단위, TSF는 UTF-16 단위라서 여기서 바꾼다.

use cssgsg_core::Output;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// 확정한다(조합이 있으면 그 자리, 없으면 커서 자리).
    Commit(Vec<u16>),
    /// 조합 글자를 이것으로 바꾼다(조합이 없으면 커서 자리에서 시작한다).
    Compose { text: Vec<u16>, segments: Vec<Segment> },
    /// 조합 글자를 지우고 조합을 끝낸다.
    Clear,
}

/// 조합 안의 밑줄 구간(UTF-16 단위). `focused`는 변환 중 포커스된 문절(굵은 밑줄).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub start: usize,
    pub len: usize,
    pub focused: bool,
}

pub fn plan(out: &Output) -> Vec<Op> {
    let mut ops = Vec::new();
    let committed = !out.commit.is_empty();
    if committed {
        ops.push(Op::Commit(out.commit.encode_utf16().collect()));
    }
    match &out.preedit {
        Some(p) if !p.text.is_empty() => {
            // 글자 단위 위치 → UTF-16 단위 위치.
            let mut offsets = Vec::with_capacity(p.text.len() + 1);
            let mut at = 0;
            for c in p.text.chars() {
                offsets.push(at);
                at += c.len_utf16();
            }
            offsets.push(at);
            let unit = |i: usize| offsets[i.min(offsets.len() - 1)];
            let segments = p
                .segments
                .iter()
                .map(|s| Segment {
                    start: unit(s.start),
                    len: unit(s.start + s.len) - unit(s.start),
                    focused: s.focused,
                })
                .collect();
            ops.push(Op::Compose { text: p.text.encode_utf16().collect(), segments });
        }
        // 확정이 조합을 이미 끝냈다.
        Some(_) if !committed => ops.push(Op::Clear),
        _ => {}
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssgsg_core::engine::{Preedit, Segment as EngineSegment};

    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    fn preedit(text: &str, segments: &[(usize, usize, bool)]) -> Option<Preedit> {
        let segments =
            segments.iter().map(|&(start, len, focused)| EngineSegment { start, len, focused }).collect();
        Some(Preedit { text: text.into(), segments })
    }

    #[test]
    fn nothing_to_do() {
        assert_eq!(plan(&Output::default()), vec![]);
    }

    #[test]
    fn commit_then_new_composition() {
        let out =
            Output { commit: "안".into(), preedit: preedit("ㄴ", &[(0, 1, false)]), ..Output::default() };
        assert_eq!(
            plan(&out),
            vec![
                Op::Commit(u16s("안")),
                Op::Compose {
                    text: u16s("ㄴ"),
                    segments: vec![Segment { start: 0, len: 1, focused: false }]
                },
            ]
        );
    }

    #[test]
    fn commit_ends_the_composition_by_itself() {
        let out = Output { commit: "안".into(), preedit: Some(Preedit::default()), ..Output::default() };
        assert_eq!(plan(&out), vec![Op::Commit(u16s("안"))]);
    }

    #[test]
    fn emptied_composition_is_cleared() {
        let out = Output { preedit: Some(Preedit::default()), ..Output::default() };
        assert_eq!(plan(&out), vec![Op::Clear]);
    }

    #[test]
    fn segments_move_to_utf16_units() {
        // 𠮷(U+20BB7)은 UTF-16 두 단위다. 두 번째 문절이 포커스.
        let out =
            Output { preedit: preedit("𠮷野家", &[(0, 2, false), (2, 1, true)]), ..Output::default() };
        assert_eq!(
            plan(&out),
            vec![Op::Compose {
                text: u16s("𠮷野家"),
                segments: vec![
                    Segment { start: 0, len: 3, focused: false },
                    Segment { start: 3, len: 1, focused: true },
                ],
            }]
        );
    }
}
