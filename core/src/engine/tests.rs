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

/// 후보 n개(c0, c1, …)를 내는 변환기. 후보창 조작(페이지·격자·번호) 시험용.
struct ManyConverter {
    n: usize,
    selected: Option<usize>,
}

impl ManyConverter {
    fn view(&self) -> Option<crate::convert::ConvView> {
        let selected = self.selected?;
        Some(crate::convert::ConvView {
            segments: vec![format!("c{selected}")],
            focused: 0,
            candidates: (0..self.n).map(|i| format!("c{i}")).collect(),
            selected: Some(selected),
        })
    }
}

impl crate::convert::Converter for ManyConverter {
    fn start(&mut self, _reading: &str) -> Option<crate::convert::ConvView> {
        self.selected = Some(0);
        self.view()
    }
    fn command(&mut self, cmd: crate::convert::ConvCmd) -> Option<crate::convert::ConvView> {
        use crate::convert::ConvCmd;
        let sel = self.selected?;
        self.selected = Some(match cmd {
            ConvCmd::Next => (sel + 1) % self.n,
            ConvCmd::Prev => (sel + self.n - 1) % self.n,
            ConvCmd::Select(i) if i < self.n => i,
            ConvCmd::Select(_) => return None,
            _ => sel,
        });
        self.view()
    }
    fn commit(&mut self) -> String {
        self.selected.take().map(|i| format!("c{i}")).unwrap_or_default()
    }
    fn cancel(&mut self) {
        self.selected = None;
    }
}

fn many(n: usize) -> Sim {
    let mut engine = Engine::new(Config::default());
    engine.set_converter(Box::new(ManyConverter { n, selected: None }));
    Sim::new(engine).with_mode(Mode::Ja)
}

/// (선택, 페이지, 격자)
fn cand_state(s: &Sim) -> (Option<usize>, Option<(usize, usize)>, bool) {
    let c = s.candidates.as_ref().expect("후보창");
    (c.selected, c.page, c.grid)
}

#[test]
fn first_space_shows_all_candidates_with_the_first_selected() {
    let mut s = many(40);
    s.type_keys("s{sp}").unwrap();
    let c = s.candidates.clone().expect("첫 Space에 후보창");
    assert_eq!(c.items.len(), 40);
    assert_eq!((c.selected, c.page, c.grid), (Some(0), Some((1, 5)), false));
    // 글자는 1번 후보로 바뀐 채 조합 중이다.
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "c0"));
}

#[test]
fn list_mode_paging_and_wrapping() {
    let mut s = many(40);
    s.type_keys("s{sp}").unwrap();
    // 문절이 하나면 ←→는 페이지(9개)를 넘긴다(NRIME와 같다).
    s.type_keys("{right}").unwrap();
    assert_eq!(cand_state(&s), (Some(9), Some((2, 5)), false));
    s.type_keys("{left}").unwrap();
    assert_eq!(cand_state(&s).0, Some(0));
    s.type_keys("{pgdn}{pgdn}").unwrap();
    assert_eq!(cand_state(&s).0, Some(18));
    s.type_keys("{pgup}").unwrap();
    assert_eq!(cand_state(&s).0, Some(9));
    // ↑↓, Space는 한 칸. 끝에서 처음으로 돈다.
    s.type_keys("{up}").unwrap();
    assert_eq!(cand_state(&s).0, Some(8));
    let mut s = many(40);
    s.type_keys("s{sp}{up}").unwrap();
    assert_eq!(cand_state(&s).0, Some(39));
    s.type_keys("{sp}").unwrap();
    assert_eq!(cand_state(&s).0, Some(0));
}

#[test]
fn tab_expands_into_a_grid() {
    let mut s = many(40);
    s.type_keys("s{sp}{tab}").unwrap();
    // 격자: 5열 × 6행 = 30개가 한 페이지.
    assert_eq!(cand_state(&s), (Some(0), Some((1, 2)), true));
    s.type_keys("{right}{right}").unwrap();
    assert_eq!(cand_state(&s).0, Some(2));
    s.type_keys("{down}").unwrap();
    assert_eq!(cand_state(&s).0, Some(7));
    s.type_keys("{up}{left}").unwrap();
    assert_eq!(cand_state(&s).0, Some(1));
    // 맨 앞에서 ←, 맨 위에서 ↑은 움직이지 않는다.
    s.type_keys("{left}{left}{up}").unwrap();
    assert_eq!(cand_state(&s).0, Some(0));
    s.type_keys("{pgdn}").unwrap();
    assert_eq!(cand_state(&s), (Some(30), Some((2, 2)), true));
    // Tab으로 목록으로 돌아간다(고른 후보는 그대로).
    s.type_keys("{tab}").unwrap();
    assert_eq!(cand_state(&s), (Some(30), Some((4, 5)), false));
}

#[test]
fn number_keys_pick_on_the_current_page() {
    // 목록 2페이지(9~17)에서 3 → 11번을 골라 확정한다.
    let mut s = many(40);
    s.type_keys("s{sp}{right}3").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str(), s.candidates.is_none()), ("c11", "", true));
    // 페이지 밖 번호는 확정하고 그 키를 새로 친다(숫자는 읽기에 들어간다).
    let mut s = many(3);
    s.type_keys("s{sp}5").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("c0", "5"));
}

#[test]
fn escape_leaves_the_grid_and_the_conversion() {
    let mut s = many(40);
    s.type_keys("s{sp}{tab}{esc}").unwrap();
    assert_eq!((s.preedit.as_str(), s.candidates.is_none()), ("か", true));
    // 다시 변환하면 목록으로 시작한다.
    s.type_keys("{sp}").unwrap();
    assert_eq!(cand_state(&s), (Some(0), Some((1, 5)), false));
}

// ---------------------------------------------------------------- 한국어 한자 변환
// 참신세벌식 v18: 대 is, 한 hfs, 민 uds, 국 kre, 전 nvs, 기 kd, ㅁ u. 한자 키는 {A-ent}(Option+Enter).

fn hanja_state(s: &Sim) -> (String, String, Option<String>) {
    let first = s.candidates.as_ref().and_then(|c| c.selected.map(|i| c.items[i].clone()));
    (s.text.clone(), s.preedit.clone(), first)
}

fn option_enter(s: &mut Sim) -> Output {
    let ctx = s.ctx;
    s.engine.handle_key(&KeyEvent::down(Key::ENTER, Mods(Mods::ALT_L), 9.0), &ctx)
}

#[test]
fn hanja_pulls_the_word_before_the_composing_syllable() {
    let mut s = sim();
    s.type_keys("{rs}ishfsudskre").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("대한민", "국"));
    s.type_keys("{A-ent}").unwrap();
    // 앞 세 글자를 조합으로 끌어와 첫 후보를 보인다.
    assert_eq!(hanja_state(&s), ("".into(), "大韓民國".into(), Some("大韓民國".into())));
    let c = s.candidates.clone().unwrap();
    assert_eq!(c.notes[0], "클 대 · 나라 이름 한 · 백성 민 · 나라 국");
    assert_eq!((c.items[1].as_str(), c.items[2].as_str(), c.notes[2].as_str()), ("民國", "國", "나라 국"));
    assert_eq!((c.page, c.grid), (Some((1, c.items.len().div_ceil(9))), false));
    // 다음 후보는 더 짧은 구간: 바뀌지 않는 앞부분은 그대로 보인다.
    s.type_keys("{sp}").unwrap();
    assert_eq!(s.preedit, "대한民國");
    s.type_keys("{sp}").unwrap();
    assert_eq!(s.preedit, "대한민國");
    s.type_keys("{ent}").unwrap();
    assert_eq!(hanja_state(&s), ("대한민國".into(), "".into(), None));
}

#[test]
fn hanja_output_asks_for_context_then_absorbs_it() {
    let mut e = Engine::new(Config::default());
    e.set_mode(Mode::Ko);
    let mut s = Sim::new(e);
    s.type_keys("ishfsudskre").unwrap();
    let out = option_enter(&mut s);
    assert_eq!((out.consumed, out.hanja_context), (true, Some(HanjaAnchor::Composing)));
    assert!(out.preedit.is_none() && out.commit.is_empty());
    let out = s.engine.hanja_begin(Some("앞 대한민"), "");
    assert_eq!(out.preedit_replace_before, 3);
    assert_eq!(out.preedit.unwrap().segments, vec![Segment { start: 0, len: 4, focused: true }]);
    // 스스로 부른 hanja_begin(한자 키 없이)은 아무 일도 하지 않는다.
    let mut fresh = Engine::new(Config::default());
    fresh.set_mode(Mode::Ko);
    assert_eq!(fresh.hanja_begin(Some("대한"), ""), Output::default());
}

#[test]
fn hanja_escape_restores_text_and_composition() {
    let mut s = sim();
    s.type_keys("{rs}ishfsudskre{A-ent}{sp}{esc}").unwrap();
    assert_eq!(hanja_state(&s), ("대한민".into(), "국".into(), None));
    // 조합이 살아 있다: Backspace는 받침을 지운다.
    s.type_keys("{bs}").unwrap();
    assert_eq!(s.screen(), "대한민구");
    let mut s = sim();
    s.type_keys("{rs}kre{A-ent}{bs}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "국"));
}

#[test]
fn hanja_converts_committed_text_before_the_caret() {
    let mut s = sim();
    // {right}는 음절을 확정하고 앱으로 간다(시뮬레이터에서 커서는 끝 그대로).
    s.type_keys("{rs}nvskd{right}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("전기", ""));
    s.type_keys("{A-ent}").unwrap();
    assert_eq!(hanja_state(&s), ("".into(), "電氣".into(), Some("電氣".into())));
    s.type_keys("{esc}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("전기", ""));
    s.type_keys("{A-ent}{ent}").unwrap();
    assert_eq!(s.text, "電氣");
    // 커서 앞이 한글이 아니면 한자 키는 앱으로 간다.
    let out = option_enter(&mut s);
    assert!(out.hanja_context.is_some());
    assert!(!s.engine.hanja_begin(Some("電氣"), "").consumed);
    assert!(!option_enter_begin(&mut s, "", ""));
}

fn option_enter_begin(s: &mut Sim, before: &str, selected: &str) -> bool {
    option_enter(s);
    s.engine.hanja_begin(Some(before), selected).consumed
}

#[test]
fn hanja_numbers_pick_and_learning_reorders() {
    let mut s = sim();
    s.type_keys("{rs}kre{A-ent}").unwrap();
    let items = s.candidates.clone().unwrap().items;
    assert_eq!(&items[..3], ["國", "局", "菊"]);
    s.press(Key::DIGIT2, 0);
    assert_eq!(hanja_state(&s), ("局".into(), "".into(), None));
    // 같은 읽기를 다시 바꾸면 고른 것이 먼저 나온다.
    s.type_keys("kre{A-ent}").unwrap();
    assert_eq!(s.candidates.as_ref().unwrap().items[..3], ["局", "國", "菊"]);
    assert_eq!(s.engine.hanja_learning().len(), 1);
    // 확정에는 학습이 바뀌었다는 표시가 붙는다.
    let out = s.engine.commit_all();
    assert!(out.learning_changed && out.commit == "局");
    assert!(!s.engine.commit_all().learning_changed);
}

#[test]
fn hanja_other_keys_commit_and_continue() {
    assert_eq!(typed("{rs}kre{A-ent}kf"), "國가");
    assert_eq!(typed("{rs}kre{A-ent}{sp}{sp}{rs}"), "菊");
    assert_eq!(typed("{rs}kre{A-ent}{click}kf"), "國가");
    assert_eq!(typed("{rs}kre{A-ent}{M-c}"), "國");
    assert_eq!(typed("{rs}kre{A-ent}{S-ent}"), "國\n");
    // 한자 키를 또 누르면 다음 후보
    assert_eq!(typed("{rs}kre{A-ent}{A-ent}{ent}"), "局");
}

#[test]
fn hanja_symbols_for_a_lone_consonant() {
    let mut s = sim();
    s.type_keys("{rs}u{A-ent}").unwrap();
    let c = s.candidates.clone().unwrap();
    assert_eq!(c.items.len(), 76);
    assert_eq!(c.items[5], "※");
    s.press(Key::DIGIT6, 0);
    assert_eq!(s.screen(), "※");
}

#[test]
fn hanja_selection_is_replaced_as_a_whole() {
    let mut e = Engine::new(Config::default());
    e.set_mode(Mode::Ko);
    let mut s = Sim::new(e);
    let _ = option_enter(&mut s);
    let out = s.engine.hanja_begin(Some("앞"), " 전기 요금");
    assert_eq!(out.preedit_replace_before, 0, "선택은 조합이 대신한다");
    let p = out.preedit.unwrap();
    assert_eq!(p.text, " 電氣 요금");
    assert_eq!(p.segments.iter().filter(|g| g.focused).count(), 1);
    assert_eq!(p.segments[1], Segment { start: 1, len: 2, focused: true });
    let out = s.engine.handle_key(&KeyEvent::down(Key::ESCAPE, Mods(0), 10.0), &Context::default());
    assert_eq!(out.commit, " 전기 요금");
    // 너무 긴 선택, 줄바꿈이 든 선택, 바꿀 것이 없는 선택: 키는 먹고(선택이 줄바꿈으로 바뀌지 않게) 바꾸지 않는다.
    for sel in ["가".repeat(65), "전기\n요금".into(), "abc".into()] {
        let _ = option_enter(&mut s);
        let out = s.engine.hanja_begin(Some(""), &sel);
        assert!(out.consumed && out.preedit.is_none(), "{sel:?}");
    }
}

#[test]
fn hanja_restart_without_context_keeps_only_the_syllable() {
    // 앱이 replacementRange를 무시하면 셸이 빈 글자로 다시 부른다.
    let mut e = Engine::new(Config::default());
    e.set_mode(Mode::Ko);
    let mut s = Sim::new(e);
    s.type_keys("ishfsudskre").unwrap();
    let _ = option_enter(&mut s);
    assert_eq!(s.engine.hanja_begin(Some("대한민"), "").preedit_replace_before, 3);
    let out = s.engine.hanja_begin(Some(""), "");
    assert!(out.consumed);
    assert_eq!((out.preedit.unwrap().text, out.preedit_replace_before), ("國".into(), 0));
    let out = s.engine.handle_key(&KeyEvent::down(Key::ESCAPE, Mods(0), 10.0), &Context::default());
    assert_eq!((out.commit.as_str(), out.preedit.unwrap().text.as_str()), ("", "국"));
    // 커서 앞 변환을 다시 부르면 바꿀 것이 없어 조합을 지운다(앱 글자는 그대로 남는다).
    let mut s = sim();
    s.type_keys("{rs}nvskd{right}{A-ent}").unwrap();
    let out = s.engine.hanja_begin(Some(""), "");
    assert!(out.consumed && out.commit.is_empty());
    assert_eq!(out.preedit.unwrap().text, "");
}

#[test]
fn hanja_shift_arrows_move_the_range_edge() {
    let mut s = sim();
    s.type_keys("{rs}ishfsudskre{A-ent}").unwrap();
    let preedit = |s: &mut Sim, keys: &str| {
        s.type_keys(keys).unwrap();
        s.preedit.clone()
    };
    assert_eq!(preedit(&mut s, "{S-right}"), "대한民國");
    assert_eq!(preedit(&mut s, "{S-right}"), "대한민國");
    assert_eq!(preedit(&mut s, "{S-right}"), "대한민國");
    assert_eq!(preedit(&mut s, "{S-left}"), "대한民國");
    assert_eq!(preedit(&mut s, "{S-left}{S-left}"), "大韓民國");
}

#[test]
fn hanja_grid_and_paging_match_japanese() {
    let mut s = sim();
    s.type_keys("{rs}kre{A-ent}").unwrap();
    let n = s.candidates.as_ref().unwrap().items.len();
    assert!(n > 30, "국 후보 {n}개");
    s.type_keys("{right}").unwrap();
    assert_eq!(cand_state(&s), (Some(9), Some((2, n.div_ceil(9))), false));
    s.type_keys("{tab}{down}").unwrap();
    assert_eq!(cand_state(&s), (Some(14), Some((1, n.div_ceil(30))), true));
    // 격자에서 ↑는 첫 줄에서 멈춘다(돌지 않는다).
    s.type_keys("{up}{up}{up}{left}").unwrap();
    assert_eq!(cand_state(&s).0, Some(3));
    s.type_keys("{right}{up}").unwrap();
    assert_eq!(cand_state(&s).0, Some(4));
    // 번호는 격자 페이지(30개) 기준
    s.press(Key::DIGIT3, 0);
    assert_eq!(s.text, "菊");
}

#[test]
fn hanja_key_is_ignored_in_secure_fields_and_other_modes() {
    let mut s = sim();
    s.type_keys("{rs}").unwrap();
    s.ctx.secure_field = true;
    let out = option_enter(&mut s);
    assert!(!out.consumed && out.hanja_context.is_none());
    s.ctx.secure_field = false;
    s.type_keys("{ls}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ja);
    assert!(option_enter(&mut s).hanja_context.is_none());
    // 일본어 후보에는 뜻이 없다.
    s.type_keys("s{sp}").unwrap();
    assert!(s.candidates.as_ref().unwrap().notes.is_empty());
}

/// 한자 키 → 앱이 앞 글자를 읽어 주지 않은 것으로(`before` None) 변환을 시작한다.
fn begin_without_app_text(s: &mut Sim) -> Output {
    let _ = option_enter(s);
    s.engine.hanja_begin(None, "")
}

fn ko_sim() -> Sim {
    let mut e = Engine::new(Config::default());
    e.set_mode(Mode::Ko);
    Sim::new(e)
}

#[test]
fn hanja_uses_the_just_typed_run_when_the_app_gives_no_text() {
    // Chromium은 조합 중에 조합 밖 글자를 주지 않는다: 엔진이 기억한 방금 친 한글(대한민)로 끌어온다.
    let mut s = ko_sim();
    s.type_keys("ishfsudskre").unwrap();
    let out = begin_without_app_text(&mut s);
    assert_eq!((out.preedit.unwrap().text, out.preedit_replace_before), ("大韓民國".into(), 3));
    // 취소하면 되돌린 글자가 다시 방금 친 한글이 된다.
    let out = s.engine.handle_key(&KeyEvent::down(Key::ESCAPE, Mods(0), 10.0), &Context::default());
    assert_eq!(out.commit, "대한민");
    assert_eq!(begin_without_app_text(&mut s).preedit_replace_before, 3);
}

#[test]
fn hanja_run_follows_what_the_app_did() {
    let replace = |keys: &str| {
        let mut s = ko_sim();
        s.type_keys(keys).unwrap();
        let out = begin_without_app_text(&mut s);
        (out.preedit.map(|p| p.text).unwrap_or_default(), out.preedit_replace_before)
    };
    // 조합을 Backspace로 다 지운 뒤의 Backspace는 앱이 앞 글자(민)를 지운다: 대한 + 국 → 한국만 맞는다.
    assert_eq!(replace("ishfsudskre{bs}{bs}{bs}{bs}kre").1, 1);
    // 앱이 받은 키(Space, 화살표), 모드 전환, 클릭, ⌘, 기호는 방금 친 한글을 끊는다.
    for keys in [
        "ishfsuds{sp}kre",
        "ishfsuds{right}kre",
        "ishfsuds{rs}{rs}kre",
        "ishfsuds{click}kre",
        "ishfsuds{M-a}kre",
        "ishfsudsQkre",
    ] {
        assert_eq!(replace(keys), ("國".into(), 0), "{keys}");
    }
    // 한자로 확정한 뒤는 한글이 아니다.
    assert_eq!(replace("ishfsudskre{A-ent}{ent}kre"), ("國".into(), 0));
    // 기호 뒤로는 새로 센다(× 뒤의 민 + 국).
    assert_eq!(replace("isHudskre").1, 1);
    // 글자를 바꾸지 않는 이벤트(Caps Lock, 키 뗌)는 끊지 않는다.
    assert_eq!(replace("ishfsuds{caps}{caps}kre").1, 3);
    assert_eq!(replace("ishfsudskre").1, 3);
}
