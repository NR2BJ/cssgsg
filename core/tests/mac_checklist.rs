//! docs/mac-checklist.md(설치 뒤 사람이 확인하는 목록)의 "엔진 확인" 칸을 엔진으로 다시 쳐 본다.
//! 사람이 볼 기대값이 엔진과 어긋나지 않게 한다. 칸 형식: `모드:키열` → `기대 화면` (`\n`은 줄바꿈).

use cssgsg_core::sim::Sim;
use cssgsg_core::{Config, Engine, Mode};

const DOC: &str = include_str!("../../docs/mac-checklist.md");

/// 한 줄에서 `모드:키열` → `기대` 쌍을 모두 찾는다.
fn pairs(line: &str) -> Vec<(Mode, String, String)> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else { break };
        let (code, tail) = (&after[..end], &after[end + 1..]);
        let mode = match code.split_once(':') {
            Some(("en", _)) => Some(Mode::En),
            Some(("ko", _)) => Some(Mode::Ko),
            Some(("ja", _)) => Some(Mode::Ja),
            _ => None,
        };
        if let (Some(mode), Some(t)) = (mode, tail.strip_prefix(" → `"))
            && let Some(e) = t.find('`')
        {
            out.push((mode, code[3..].to_string(), t[..e].replace("\\n", "\n")));
            rest = &t[e + 1..];
            continue;
        }
        rest = tail;
    }
    out
}

#[test]
fn checklist_engine_column_matches_engine() {
    let mut checked = 0;
    for line in DOC.lines().filter(|l| l.starts_with("| ")) {
        for (mode, keys, expected) in pairs(line) {
            let mut sim = Sim::new(Engine::new(Config::default())).with_mode(mode);
            sim.type_keys(&keys).unwrap_or_else(|e| panic!("{keys}: {e}"));
            assert_eq!(sim.screen(), expected, "{mode:?} {keys}");
            checked += 1;
        }
    }
    // 표를 고치다 형식이 깨져서 아무것도 안 읽히는 일을 막는다.
    assert!(checked >= 14, "엔진 확인 칸을 {checked}개만 읽었다");
}
