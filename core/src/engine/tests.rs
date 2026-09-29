use super::*;
use crate::key::Mods;
use crate::sim::Sim;

fn sim() -> Sim {
    Sim::new(Engine::new(Config::default()))
}

fn sim_with(config: &str) -> Sim {
    Sim::new(Engine::new(Config::from_toml(config).unwrap()))
}

fn typed(keys: &str) -> String {
    let mut s = sim();
    s.type_keys(keys).unwrap();
    s.screen()
}

#[test]
fn english_is_graphite_on_qwerty() {
    // Graphite의 h e l l o는 쿼티 자리 j l w w i다.
    assert_eq!(typed("jlwwi"), "hello");
    assert_eq!(typed("JLWWI"), "HELLO");
    assert_eq!(typed("{caps}jlwwi"), "HELLO");
    // 쿼티와 같은 글자(g x 숫자)는 먹지 않고 앱이 그대로 친다.
    assert_eq!(typed("gx1"), "gx1");
    assert_eq!(typed("'"), ",");
    assert_eq!(typed("\""), "?");
}

#[test]
fn shortcuts_stay_on_qwerty() {
    let mut s = sim();
    s.type_keys("{rs}kf").unwrap();
    assert_eq!(s.preedit, "가");
    // ⌘C: 조합을 확정하고 키는 앱으로(단축키는 쿼티 자리 그대로).
    s.type_keys("{M-c}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("가", ""));
}

#[test]
fn mode_toggles_follow_nrime() {
    let mut s = sim();
    let modes = |s: &Sim| s.engine.mode();
    assert_eq!(modes(&s), Mode::En);
    s.type_keys("{rs}").unwrap();
    assert_eq!(modes(&s), Mode::Ko);
    s.type_keys("{ls}").unwrap();
    assert_eq!(modes(&s), Mode::Ja);
    s.type_keys("{rs}").unwrap();
    assert_eq!(modes(&s), Mode::En);
    // 영어 → 직전 비영어(일본어)로 돌아간다.
    s.type_keys("{rs}").unwrap();
    assert_eq!(modes(&s), Mode::Ja);
    s.type_keys("{ls}").unwrap();
    assert_eq!(modes(&s), Mode::Ko);
    // 영어에서 LShift 탭: 직전 비영어(한국어)의 반대인 일본어.
    s.type_keys("{rs}{ls}").unwrap();
    assert_eq!(modes(&s), Mode::Ja);
}

#[test]
fn korean_sentences() {
    assert_eq!(typed("{rs}jfsmtdhfleja"), "안녕하세요");
    assert_eq!(typed("{rs}kf{sp}kf"), "가 가");
    assert_eq!(typed("{rs}kf1"), "가1");
    assert_eq!(typed("{rs}kfQ"), "가「");
    assert_eq!(typed("{rs}kf?"), "가?");
    assert_eq!(typed("{rs}kof{bs}"), "고");
    assert_eq!(typed("{rs}kf{bs}{bs}{bs}"), "");
    assert_eq!(typed("{rs}jfs{bs}{bs}{bs}{bs}"), "");
    assert_eq!(typed("{rs}N"), "");
    assert_eq!(typed("{rs}bNbNb"), "ㅋㅋㅋ");
}

#[test]
fn switching_commits_composition() {
    let mut s = sim();
    s.type_keys("{rs}kf{rs}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("가", ""));
    s.type_keys("{rs}kf{ls}").unwrap();
    assert_eq!((s.text.as_str(), s.engine.mode()), ("가가", Mode::Ja));
    s.type_keys("s{rs}").unwrap();
    assert_eq!(s.text, "가가か");
}

#[test]
fn click_commits_and_reset_discards() {
    let mut s = sim();
    s.type_keys("{rs}kf{click}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("가", ""));
    s.type_keys("kf").unwrap();
    let out = s.engine.reset();
    assert_eq!(out.commit, "");
    assert_eq!(out.preedit.map(|p| p.text), Some(String::new()));
}

#[test]
fn japanese_kana_and_enter() {
    let mut s = sim();
    s.type_keys("{ls}ckeuwl").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "にほんご"));
    s.type_keys("{ent}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("にほんご", ""));
    // 앞치기 표시
    s.type_keys("k").unwrap();
    assert_eq!(s.preedit, "☆");
    s.type_keys("{esc}").unwrap();
    assert_eq!(s.preedit, "");
}

#[test]
fn japanese_conversion_with_echo_converter() {
    let mut s = sim();
    s.type_keys("{ls}ckeuwl{sp}").unwrap();
    let c = s.candidates.clone().expect("후보창");
    assert_eq!(c.items, vec!["にほんご", "ニホンゴ"]);
    assert_eq!(c.selected, Some(0));
    s.type_keys("{sp}").unwrap();
    assert_eq!(s.preedit, "ニホンゴ");
    s.type_keys("{ent}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("ニホンゴ", ""));
    assert_eq!(s.candidates, None);

    // 번호로 고르면 바로 확정
    assert_eq!(typed("{ls}ckeuwl{sp}2"), "ニホンゴ");
    // 변환 중 가나 키: 확정하고 새 읽기 시작
    assert_eq!(typed("{ls}s{sp}f"), "かと");
    // 변환 취소는 읽기로
    let mut s = sim();
    s.type_keys("{ls}s{sp}{esc}").unwrap();
    assert_eq!((s.preedit.as_str(), s.candidates.is_none()), ("か", true));
}

#[test]
fn conversion_preedit_marks_focused_segment() {
    let mut e = Engine::new(Config::default());
    let ctx = Context::default();
    e.set_mode(Mode::Ja);
    let ev = |k| KeyEvent::down(k, Mods::default(), 1.0);
    e.handle_key(&ev(Key::S), &ctx);
    let out = e.handle_key(&ev(Key::SPACE), &ctx);
    let p = out.preedit.expect("변환 화면");
    assert_eq!(p.text, "か");
    assert_eq!(p.segments, vec![Segment { start: 0, len: 1, focused: true }]);
}

#[test]
fn japanese_symbols_and_shift() {
    // 가나 배열 밖 기호는 기본(일본식)에서 전각.
    assert_eq!(typed("{ls}!"), "！");
    assert_eq!(typed("{ls}s?"), "か？");
    // 글자 키의 Shift는 무시(가나).
    assert_eq!(typed("{ls}S{ent}"), "か");
    // 숫자는 읽기에 들어간다.
    let mut s = sim();
    s.type_keys("{ls}3cf").unwrap();
    assert_eq!(s.preedit, "3にと");
    // 반각 서양식
    let mut s = sim_with("[ja]\npunctuation = \"half_width_western\"");
    s.type_keys("{ls}!s,.").unwrap();
    assert_eq!(s.screen(), "!か,.");
    // 전각 서양식
    let mut s = sim_with("[ja]\npunctuation = \"full_width_western\"");
    s.type_keys("{ls}s,.").unwrap();
    assert_eq!(s.preedit, "か，．");
}

#[test]
fn japanese_space_width() {
    assert_eq!(typed("{ls}{sp}"), " ");
    let mut s = sim_with("[ja]\nfull_width_space = true");
    s.type_keys("{ls}{sp}").unwrap();
    assert_eq!(s.screen(), "\u{3000}");
}

#[test]
fn caps_lock_is_katakana_in_japanese() {
    let mut s = sim();
    s.type_keys("{ls}{caps}ckeuwl").unwrap();
    // 가타카나는 변환하지 않으므로 바로 확정한다. 뒤치기가 바꿀 수 있는 마지막 글자만 조합으로 남는다.
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("ニホン", "ゴ"));
    s.type_keys("{sp}").unwrap();
    assert_eq!(s.screen(), "ニホンゴ ");
    // 일본어 모드를 나가면 Caps Lock을 끈다.
    s.type_keys("{rs}").unwrap();
    assert!(!s.caps);
    // 한국어 모드는 Caps Lock을 무시한다.
    let mut s = sim();
    s.type_keys("{rs}{caps}kf").unwrap();
    assert_eq!(s.preedit, "가");
}

#[test]
fn game_mode_and_disabled_taps() {
    let mut s = sim();
    s.ctx.game_mode = true;
    s.type_keys("jlwwi").unwrap();
    assert_eq!(s.screen(), "jlwwi");

    let mut s = sim();
    s.ctx.taps_disabled = true;
    s.type_keys("{rs}").unwrap();
    assert_eq!(s.engine.mode(), Mode::En);
}

#[test]
fn d_variant_from_config() {
    let mut s = sim_with("ko_layout = \"chamshin-d-v19\"");
    s.type_keys("{rs}kf3/aN3").unwrap();
    assert_eq!(s.screen(), "갇표3");
}

#[test]
fn commit_always_reports_new_preedit() {
    // ㅎ 뒤 ㅎ: 앞 ㅎ가 확정되고 새 ㅎ가 조합 중. 화면 글자는 같지만 insertText가 조합 중 글자를
    // 대신하므로 preedit를 다시 알려야 한다.
    let mut e = Engine::new(Config::default());
    let ctx = Context::default();
    e.set_mode(Mode::Ko);
    let ev = |k| KeyEvent::down(k, Mods::default(), 1.0);
    e.handle_key(&ev(Key::H), &ctx);
    let out = e.handle_key(&ev(Key::H), &ctx);
    assert_eq!(out.commit, "ㅎ");
    assert_eq!(out.preedit.map(|p| p.text), Some("ㅎ".into()));
}

#[test]
fn command_or_control_down_commits_composition() {
    // ⌘를 누르는 순간 확정(키는 앱으로). 그 뒤 ⌘C는 조합이 없으니 그냥 통과.
    let mut e = Engine::new(Config::default());
    let ctx = Context::default();
    e.set_mode(Mode::Ko);
    e.handle_key(&KeyEvent::down(Key::K, Mods::default(), 1.0), &ctx);
    e.handle_key(&KeyEvent::down(Key::F, Mods::default(), 1.1), &ctx);
    let out = e.handle_key(&KeyEvent::down(Key::META_LEFT, Mods(Mods::META_L), 1.2), &ctx);
    assert_eq!((out.consumed, out.commit.as_str()), (false, "가"));
    let out = e.handle_key(&KeyEvent::down(Key::C, Mods(Mods::META_L), 1.3), &ctx);
    assert_eq!((out.consumed, out.commit.as_str()), (false, ""));
    // Shift(수식키)를 누르는 건 확정하지 않는다.
    e.handle_key(&KeyEvent::down(Key::K, Mods::default(), 2.0), &ctx);
    let out = e.handle_key(&KeyEvent::down(Key::SHIFT_LEFT, Mods(Mods::SHIFT_L), 2.1), &ctx);
    assert_eq!(out.commit, "");
}

#[test]
fn japanese_shift_enter_commits_and_passes_enter() {
    let mut s = sim();
    s.type_keys("{ls}s{S-ent}").unwrap();
    assert_eq!(s.screen(), "か\n");
    // 변환 중 Shift+Enter도 확정하고 줄을 바꾼다.
    let mut s = sim();
    s.type_keys("{ls}s{sp}{S-ent}").unwrap();
    assert_eq!(s.screen(), "か\n");
    // 그냥 Enter는 확정만.
    assert_eq!(typed("{ls}s{ent}"), "か");
}

#[test]
fn secure_field_passes_keys_but_keeps_taps_consistent() {
    let mut s = sim();
    s.ctx.secure_field = true;
    // 영어 모드여도 Graphite로 바꾸지 않는다(비밀번호는 쿼티).
    s.type_keys("jlwwi").unwrap();
    assert_eq!(s.screen(), "jlwwi");
    // 언어 전환 탭은 된다.
    s.type_keys("{rs}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ko);
    // Shift+글자는 탭이 아니다(글자가 탭 판정을 끊는다).
    let mut e = Engine::new(Config::default());
    let ctx = Context { secure_field: true, ..Context::default() };
    e.handle_key(&KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 1.0), &ctx);
    e.handle_key(&KeyEvent::down(Key::A, Mods(Mods::SHIFT_R), 1.02), &ctx);
    let out = e.handle_key(&KeyEvent::up(Key::SHIFT_RIGHT, Mods::default(), 1.05), &ctx);
    assert_eq!(out.mode, None);
}

#[test]
fn katakana_direct_can_be_turned_off() {
    let mut s = sim_with("[ja]\nkatakana_direct = false");
    s.type_keys("{ls}{caps}ckeuwl").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "ニホンゴ"));
    // 히라가나는 설정과 상관없이 변환을 위해 조합으로 남는다.
    let mut s = sim();
    s.type_keys("{ls}ckeuwl").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "にほんご"));
}

#[test]
fn katakana_direct_backspace() {
    // か(s) と(f): か는 と를 칠 때 확정. Backspace는 と를 되돌리고, 그다음은 앱이 カ를 지운다.
    let mut s = sim();
    s.type_keys("{ls}{caps}sf{bs}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("カ", ""));
    s.type_keys("{bs}").unwrap();
    assert_eq!(s.screen(), "");
}

#[test]
fn katakana_direct_shows_the_same_text_as_composing() {
    // 무작위 키열(Backspace 없음, Caps Lock 켠 채): 바로 확정해도 화면의 글자는 조합으로 들고 있을 때와 같아야 한다.
    // (중간에 Caps Lock을 끄면 다르다: katakana_direct_keeps_what_was_typed_as_katakana)
    // 조합으로 남는 것은 마지막 키의 글자(최대 2자)와 앞치기 표시(최대 2자)뿐이다.
    let keys: Vec<&str> =
        "a s d f g h j k l ; q w e r t y u i o p z x c v b n m , . / [ ] 1 2 3 {sp} {ent} - '"
            .split(' ')
            .collect();
    let mut seed: u64 = 0x5eed;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    for _ in 0..3000 {
        let len = 1 + next() % 20;
        let line: String = (0..len).map(|_| keys[next() % keys.len()]).collect();
        let keys = format!("{{ls}}{{caps}}{line}");
        let mut direct = sim();
        let mut composing = sim_with("[ja]\nkatakana_direct = false");
        direct.type_keys(&keys).unwrap();
        composing.type_keys(&keys).unwrap();
        assert_eq!(direct.screen(), composing.screen(), "{keys}");
        assert!(direct.preedit.chars().count() <= 4, "{keys}: 조합이 길다 {:?}", direct.preedit);
    }
}

#[test]
fn katakana_direct_keeps_what_was_typed_as_katakana() {
    // Caps Lock을 켜고 친 글자는 그때 가타카나로 확정된다. 끄면 남은 조합(마지막 글자)만 히라가나로 보인다.
    let mut s = sim();
    s.type_keys("{ls}{caps}sf{caps}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("カ", "と"));
}
