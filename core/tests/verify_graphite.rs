//! Graphite 검증: 공식 macOS 키보드 레이아웃(`layouts/en/official/Graphite.keylayout`)과
//! 엔진이 쓰는 `layouts/en/graphite.toml`을 키마다 대조한다(Shift 없음/있음 모두).

use std::collections::HashMap;

use cssgsg_core::Key;
use cssgsg_core::latin::LatinLayout;

const KEYLAYOUT: &str = include_str!("../../layouts/en/official/Graphite.keylayout");

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let start = tag.find(&pat)? + pat.len();
    let end = tag[start..].find('"')? + start;
    Some(unescape(&tag[start..end]))
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out += &rest[..i];
        let end = rest[i..].find(';').expect("엔티티는 ;로 끝난다") + i;
        let ent = &rest[i + 1..end];
        let c = match ent {
            "quot" => '"',
            "apos" => '\'',
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            _ if ent.starts_with("#x") => {
                char::from_u32(u32::from_str_radix(&ent[2..], 16).unwrap()).unwrap()
            }
            _ if ent.starts_with('#') => char::from_u32(ent[1..].parse().unwrap()).unwrap(),
            _ => panic!("모르는 엔티티 {ent}"),
        };
        out.push(c);
        rest = &rest[end + 1..];
    }
    out + rest
}

/// `<tag ...>` 조각들을 차례로 꺼낸다.
fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name} ");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let end = rest[i..].find('>').unwrap() + i;
        out.push(&rest[i..=end]);
        rest = &rest[end + 1..];
    }
    out
}

/// keyMap index의 키 코드 → 출력. action은 state="none"일 때의 출력으로 푼다.
fn keymap(index: u32) -> HashMap<u16, String> {
    let mut actions = HashMap::new();
    let mut rest = KEYLAYOUT;
    while let Some(i) = rest.find("<action id=") {
        let end = rest[i..].find("</action>").unwrap() + i;
        let block = &rest[i..end];
        let id = attr(tags(block, "action")[0], "id").unwrap();
        if let Some(when) =
            tags(block, "when").into_iter().find(|t| attr(t, "state").as_deref() == Some("none"))
        {
            if let Some(o) = attr(when, "output") {
                actions.insert(id, o);
            }
        }
        rest = &rest[end..];
    }
    let open = format!("<keyMap index=\"{index}\"");
    let start = KEYLAYOUT.find(&open).expect("keyMap이 있어야 한다");
    let end = KEYLAYOUT[start..].find("</keyMap>").unwrap() + start;
    let mut out = HashMap::new();
    for tag in tags(&KEYLAYOUT[start..end], "key") {
        let code: u16 = attr(tag, "code").unwrap().parse().unwrap();
        let output =
            attr(tag, "output").or_else(|| attr(tag, "action").and_then(|a| actions.get(&a).cloned()));
        if let Some(o) = output {
            out.insert(code, o);
        }
    }
    out
}

#[test]
fn engine_graphite_matches_official_keylayout() {
    let ours = LatinLayout::graphite();
    let mut checked = 0;
    let mut failures = Vec::new();
    for (index, shift) in [(0, false), (1, true)] {
        let official = keymap(index);
        for mac in 0..=0x7Fu16 {
            let key = Key::from_mac_keycode(mac);
            if !key.is_printable() {
                continue;
            }
            let want = official.get(&mac).cloned();
            let got = ours.char_for(key, shift, false, false).map(String::from);
            if want != got {
                failures.push(format!("{key:?} shift={shift}: 공식 {want:?} / 엔진 {got:?}"));
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 47 * 2, "글자 키 47개 × Shift 두 상태");
    assert!(failures.is_empty(), "{}개 불일치:\n{}", failures.len(), failures.join("\n"));
}
