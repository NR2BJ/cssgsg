//! Mozc 변환(`mozc` 기능). 데이터: CSSGSG_MOZC_DATA 또는 build/mozc-out/data/mozc.data.
//! 실행: cargo test -p cssgsg-core --features mozc --test mozc
//! 학습 폴더는 테스트 전체가 임시 폴더 하나를 같이 쓴다(Mozc 설정이 프로세스 전체에 하나라서).
//! 학습은 끈다: 한 테스트가 확정한 후보가 다른 테스트의 1순위를 바꾸지 않게.
//! 테스트는 한 번에 하나씩 돈다(SERIAL). Mozc는 설정·학습 폴더 같은 전역 상태가 있어서, 여러 엔진을 여러 스레드에서
//! 동시에 쓰면 서로 밟는다. 입력기는 Mozc 하나를 메인 스레드에서만 쓰므로 이것이 실제 조건과 같다.
#![cfg(feature = "mozc")]

use std::sync::{Mutex, MutexGuard, OnceLock};

use cssgsg_core::convert::{ConvCmd, Converter};
use cssgsg_core::mozc::MozcConverter;
use cssgsg_core::sim::Sim;
use cssgsg_core::{Config, Engine, Mode};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn paths() -> (String, String) {
    static PROFILE: OnceLock<String> = OnceLock::new();
    let data = std::env::var("CSSGSG_MOZC_DATA")
        .unwrap_or_else(|_| format!("{}/../build/mozc-out/data/mozc.data", env!("CARGO_MANIFEST_DIR")));
    let profile = PROFILE.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("cssgsg-mozc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().into_owned()
    });
    (data, profile.clone())
}

fn mozc() -> MozcConverter {
    let (data, profile) = paths();
    let mut m = MozcConverter::new(&data, &profile).expect("Mozc를 만들 수 없다(tools/mozc/build.sh를 먼저)");
    m.set_learning(false);
    m
}

#[test]
fn converts_a_word_and_walks_candidates() {
    let _serial = serial();
    let mut m = mozc();
    // 첫 변환부터 후보 전체가 오고, 1번이 골라져 있다.
    let v = m.start("にほんご").unwrap();
    assert_eq!(v.segments, vec!["日本語"]);
    assert_eq!(v.selected, Some(0));
    for c in ["日本語", "ニホンゴ", "にほんご"] {
        assert!(v.candidates.iter().any(|x| x == c), "{c} 없음: {:?}", v.candidates);
    }
    // 음역 후보는 하위 목록("そのほかの文字種")으로 접지 않고 본 목록에 펼친다.
    assert!(!v.candidates.iter().any(|x| x == "そのほかの文字種"), "{:?}", v.candidates);
    // 전체 목록 기준 번호로 고른다.
    let v = m.command(ConvCmd::Select(1)).unwrap();
    assert_eq!(v.selected, Some(1));
    assert_eq!(v.segments, vec![v.candidates[1].clone()]);
    let v2 = m.command(ConvCmd::Next).unwrap();
    assert_eq!(v2.selected, Some(2));
    assert_eq!(m.commit(), v2.candidates[2]);
}

#[test]
fn splits_a_sentence_into_segments() {
    let _serial = serial();
    let mut m = mozc();
    let v = m.start("わたしのなまえはなかのです").unwrap();
    assert_eq!(v.segments.concat(), "私の名前は中野です");
    assert!(v.segments.len() >= 2, "{:?}", v.segments);
    assert_eq!(v.focused, 0);
    let v = m.command(ConvCmd::FocusRight).unwrap();
    assert_eq!(v.focused, 1);
    // 문절 줄이기와 늘이기는 서로 되돌린다.
    let before = v.segments.clone();
    let shrunk = m.command(ConvCmd::Shrink).unwrap();
    assert_ne!(shrunk.segments, before);
    let v = m.command(ConvCmd::Expand).unwrap();
    assert_eq!(v.segments.concat().chars().count(), before.concat().chars().count());
    m.cancel();
}

#[test]
fn cancel_forgets_the_conversion() {
    let _serial = serial();
    let mut m = mozc();
    m.start("きょうはいいてんきですね").unwrap();
    m.cancel();
    let v = m.start("にほんご").unwrap();
    assert_eq!(v.segments, vec!["日本語"]);
    m.cancel();
}

fn ja_sim() -> Sim {
    let mut engine = Engine::new(Config::default());
    engine.set_converter(Box::new(mozc()));
    Sim::new(engine).with_mode(Mode::Ja)
}

#[test]
fn shingetsu_keys_to_kanji() {
    let _serial = serial();
    // 新月 c k e u w l = にほんご → Space → 日本語 → Enter
    let mut s = ja_sim();
    s.type_keys("ckeuwl{sp}").unwrap();
    assert_eq!(s.preedit, "日本語");
    s.type_keys("{ent}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("日本語", ""));
}

#[test]
fn escape_returns_to_the_reading() {
    let _serial = serial();
    let mut s = ja_sim();
    s.type_keys("ckeuwl{sp}{esc}").unwrap();
    assert_eq!(s.preedit, "にほんご");
}

#[test]
fn number_key_picks_from_the_candidate_window() {
    let _serial = serial();
    // 첫 Space에 후보창이 뜨고 1번이 골라져 있다. 번호로 고르면 바로 확정한다.
    let mut s = ja_sim();
    s.type_keys("ckeuwl{sp}").unwrap();
    let window = s.candidates.clone().expect("첫 Space에 후보창");
    assert_eq!(window.selected, Some(0));
    assert_eq!(s.preedit, window.items[0]);
    let second = window.items[1].clone();
    s.type_keys("2").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), (second.as_str(), ""));
}

#[test]
fn tab_grid_works_with_mozc() {
    let _serial = serial();
    let mut s = ja_sim();
    s.type_keys("ckeuwl{sp}{tab}{right}").unwrap();
    let c = s.candidates.clone().expect("후보창");
    assert!(c.grid);
    assert_eq!(c.selected, Some(1));
    assert_eq!(s.preedit, c.items[1]);
    s.type_keys("{ent}").unwrap();
    assert_eq!(s.text, c.items[1]);
}

/// 사용자 사전 파일(user_dictionary.db = UserDictionaryStorage 프로토콜 버퍼)을 손으로 만든다.
/// 설정 앱은 config-ffi의 userdict로 같은 파일을 쓴다.
fn user_dictionary(entries: &[(&str, &str)]) -> Vec<u8> {
    fn field(out: &mut Vec<u8>, number: u8, bytes: &[u8]) {
        out.push((number << 3) | 2);
        let mut n = bytes.len();
        while n >= 0x80 {
            out.push((n as u8) | 0x80);
            n >>= 7;
        }
        out.push(n as u8);
        out.extend_from_slice(bytes);
    }
    let mut dict = vec![0x08, 0x2A]; // id = 42
    field(&mut dict, 3, "cssgsg".as_bytes());
    for (key, value) in entries {
        let mut e = Vec::new();
        field(&mut e, 1, key.as_bytes());
        field(&mut e, 2, value.as_bytes());
        e.extend_from_slice(&[0x28, 0x01]); // pos = 名詞
        field(&mut dict, 4, &e);
    }
    let mut storage = Vec::new();
    field(&mut storage, 2, &dict);
    storage
}

#[test]
fn reload_picks_up_the_user_dictionary() {
    let _serial = serial();
    let (_, profile) = paths();
    let mut m = mozc();
    // 두 번째 읽기에는 작은 가나(ゃ っ)와 장음 부호(ー)가 있다(NRIME 때 등록이 안 되던 것).
    let words = [("くもつくもつ", "蜘蛛津雲津"), ("ちゃっきゅーもつ", "茶っ究ー津")];
    let has_word = |m: &mut MozcConverter, (reading, word): (&str, &str)| {
        let v = m.start(reading).unwrap();
        m.cancel();
        v.candidates.iter().any(|c| c == word)
    };
    for w in words {
        assert!(!has_word(&mut m, w), "사전에 없는 낱말로 시험한다: {w:?}");
    }
    let path = std::path::Path::new(&profile).join("user_dictionary.db");
    std::fs::write(&path, user_dictionary(&words)).unwrap();
    m.reload();
    // Mozc는 사용자 사전을 뒤에서 읽는다: 조금 기다린다.
    let found = (0..40).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        words.iter().all(|&w| has_word(&mut m, w))
    });
    std::fs::remove_file(&path).ok();
    m.reload();
    assert!(found, "다시 읽은 뒤 사용자 사전 낱말이 후보에 있어야 한다");
}
