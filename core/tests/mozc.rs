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
    let v = m.start("にほんご").unwrap();
    assert_eq!(v.segments, vec!["日本語"]);
    // 첫 변환은 후보창 없이, 다음 후보부터 후보창이 뜬다(MS-IME와 같다).
    let v = m.command(ConvCmd::Next).unwrap();
    for c in ["日本語", "ニホンゴ", "にほんご"] {
        assert!(v.candidates.iter().any(|x| x == c), "{c} 없음: {:?}", v.candidates);
    }
    // 음역 후보는 하위 목록("そのほかの文字種")으로 접지 않고 본 목록에 펼친다.
    assert!(!v.candidates.iter().any(|x| x == "そのほかの文字種"), "{:?}", v.candidates);
    assert!(v.page.is_some());
    let selected = v.candidates[v.selected.unwrap()].clone();
    assert_eq!(v.segments, vec![selected.clone()]);
    assert_eq!(m.commit(), selected);
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
    // 두 번째 Space에 후보창이 뜬다. 번호로 고르면 바로 확정한다.
    let mut s = ja_sim();
    s.type_keys("ckeuwl{sp}{sp}").unwrap();
    let window = s.candidates.clone().expect("후보창");
    let second = window.items[1].clone();
    s.type_keys("2").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), (second.as_str(), ""));
}
