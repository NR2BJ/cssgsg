use super::*;
use crate::key::Mods;
use crate::sim::Sim;

/// 영어 모드에서 시작하는 시뮬레이터(시험들은 영어에서 `{rs}`·`{ls}`로 옮겨 간다). 입력기는 한국어로 시작한다.
fn sim() -> Sim {
    Sim::new(Engine::new(Config::default())).with_mode(Mode::En)
}

fn sim_with(config: &str) -> Sim {
    Sim::new(Engine::new(Config::from_toml(config).unwrap())).with_mode(Mode::En)
}

#[test]
fn starts_in_korean() {
    let engine = Engine::new(Config::default());
    assert_eq!(engine.mode(), Mode::Ko);
    let mut s = Sim::new(engine);
    s.type_keys("jfs{rs}").unwrap();
    assert_eq!((s.text.as_str(), s.engine.mode()), ("안", Mode::En), "영어 전환 뒤 다시 누르면 한국어");
    s.type_keys("{rs}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ko);
}

#[test]
fn follow_mode_drops_the_composition_and_keeps_the_toggle_target() {
    // 윈도우: 다른 앱의 입력기가 바꾼 모드를 따라간다. 조합은 확정하지 않고 버린다.
    let mut s = Sim::new(Engine::new(Config::default()));
    s.type_keys("jf").unwrap();
    let out = s.engine.follow_mode(Mode::En, Mode::Ja);
    assert_eq!((out.mode, out.commit.as_str()), (Some(Mode::En), ""));
    assert_eq!(out.preedit, Some(Preedit::default()));
    assert_eq!(s.engine.last_non_en(), Mode::Ja);
    s.type_keys("{rs}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ja, "오른쪽 Shift 톡은 따라온 비영어 모드로 간다");
    // 같은 모드면 모드 변경을 알리지 않고, 비영어 자리에 영어가 오면 그대로 둔다.
    assert_eq!(s.engine.follow_mode(Mode::Ja, Mode::En).mode, None);
    assert_eq!(s.engine.last_non_en(), Mode::Ja);
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
fn config_can_change_while_running() {
    // 설정 앱이 바꾸면 모드·학습을 그대로 두고 다음 키부터 새 설정을 쓴다.
    let mut s = sim();
    s.type_keys("{rs}kf3").unwrap();
    assert_eq!(s.screen(), "가3");
    s.engine.set_config(
        Config::from_toml("ko_layout = \"chamshin-d-v19\"\n[taps]\nalt_right = \"toggle_english\"").unwrap(),
    );
    assert_eq!(s.engine.mode(), Mode::Ko);
    s.type_keys("{rs}kf3/aN3").unwrap();
    assert_eq!(s.screen(), "가3갇표3", "오른쪽 Shift 탭은 이제 아무 일도 안 하고 D 배열로 친다");
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
fn password_fields_type_graphite_where_the_shell_can_insert() {
    // 윈도우: 비밀번호 칸에도 글자를 넣을 수 있어서, 모드와 상관없이 Graphite로 바로 확정한다(조합 없음).
    let mut s = Sim::new(Engine::new(Config::default()));
    s.ctx = Context { secure_field: true, secure_latin: true, ..Context::default() };
    assert_eq!(s.engine.mode(), Mode::Ko);
    s.type_keys("jlwwi").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("hello", ""), "한국어 모드여도 Graphite");
    s.type_keys("{ls}J1").unwrap();
    assert_eq!(s.text, "helloH1", "일본어 모드여도 Graphite, 쿼티와 같은 숫자는 앱이 친다");
    // 언어 전환 탭은 그대로 된다.
    assert_eq!(s.engine.mode(), Mode::Ja);
    // 단축키는 쿼티 자리 그대로 앱에 간다(Ctrl+A가 Graphite 글자가 되면 안 된다).
    s.type_keys("{C-a}{C-v}{A-f}").unwrap();
    assert_eq!(s.text, "helloH1");
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
// 참신세벌식 v18: 대 is, 한 hfs, 민 uds, 국 kre, 전 nvs, 기 kd, ㅁ u, 뭐 u.v. 한자 키는 {A-ent}(Option+Enter).
// 바꾸는 것은 조합 중인 글자 하나다.

fn hanja_state(s: &Sim) -> (String, String, Option<String>) {
    let first = s.candidates.as_ref().and_then(|c| c.selected.map(|i| c.items[i].clone()));
    (s.text.clone(), s.preedit.clone(), first)
}

fn option_enter(s: &mut Sim) -> Output {
    let ctx = s.ctx;
    s.engine.handle_key(&KeyEvent::down(Key::ENTER, Mods(Mods::ALT_L), 9.0), &ctx)
}

#[test]
fn hanja_converts_only_the_composing_letter() {
    let mut s = sim();
    s.type_keys("{rs}ishfsudskre").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("대한민", "국"));
    s.type_keys("{A-ent}").unwrap();
    // 앞 글자(대한민)는 그대로, 조합 중인 국만 첫 후보로 바뀌고 후보창이 뜬다.
    assert_eq!(hanja_state(&s), ("대한민".into(), "國".into(), Some("國".into())));
    let c = s.candidates.clone().unwrap();
    assert_eq!(
        (&c.items[..3], c.notes[0].as_str(), c.notes[1].as_str()),
        (&["國", "局", "菊"].map(String::from)[..], "나라 국", "부분 국, 판 국")
    );
    assert_eq!((c.page, c.grid), (Some((1, c.items.len().div_ceil(9))), false));
    s.type_keys("{sp}").unwrap();
    assert_eq!(s.preedit, "局");
    s.type_keys("{ent}").unwrap();
    assert_eq!(hanja_state(&s), ("대한민局".into(), "".into(), None));
}

#[test]
fn hanja_preview_is_one_focused_segment() {
    let mut s = sim();
    s.type_keys("{rs}kre").unwrap();
    let out = option_enter(&mut s);
    assert!(out.consumed && out.commit.is_empty());
    assert_eq!(
        out.preedit.unwrap(),
        Preedit { text: "國".into(), segments: vec![Segment { start: 0, len: 1, focused: true }] }
    );
}

#[test]
fn hanja_escape_restores_the_composition() {
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
fn hanja_key_without_composition_goes_to_the_app() {
    let mut s = sim();
    // {right}는 음절을 확정하고 앱으로 간다. 조합이 없으면 한자 키도 앱으로(줄바꿈 등은 앱이 정한다).
    s.type_keys("{rs}nvskd{right}").unwrap();
    let out = option_enter(&mut s);
    assert!(!out.consumed && out.preedit.is_none() && out.candidates.is_none());
    assert_eq!(s.screen(), "전기");
    // 한자가 없는 음절(뭐)이나 모음만 있을 때는 키만 먹고 그대로 둔다.
    for keys in ["u.v", "f"] {
        let mut s = sim();
        s.type_keys("{rs}").unwrap();
        s.type_keys(keys).unwrap();
        let before = s.preedit.clone();
        let out = option_enter(&mut s);
        assert!(out.consumed && out.preedit.is_none() && out.candidates.is_none(), "{keys}");
        assert_eq!(s.preedit, before);
    }
}

#[test]
fn hanja_numbers_pick_and_learning_reorders() {
    let mut s = sim();
    s.type_keys("{rs}kre{A-ent}").unwrap();
    s.press(Key::DIGIT2, 0);
    assert_eq!(hanja_state(&s), ("局".into(), "".into(), None));
    // 같은 글자를 다시 바꾸면 고른 것이 먼저 나온다.
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
fn hanja_grid_and_paging_match_japanese() {
    let mut s = sim();
    s.type_keys("{rs}kre{A-ent}").unwrap();
    let n = s.candidates.as_ref().unwrap().items.len();
    assert_eq!(n, 58);
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
    s.type_keys("{rs}kre").unwrap();
    s.ctx.secure_field = true;
    let out = option_enter(&mut s);
    assert!(!out.consumed && out.preedit.is_none());
    s.ctx.secure_field = false;
    s.type_keys("{ls}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ja);
    assert!(option_enter(&mut s).candidates.is_none());
    // 일본어 후보에는 뜻이 없다.
    s.type_keys("s{sp}").unwrap();
    assert!(s.candidates.as_ref().unwrap().notes.is_empty());
}

// ---------------------------------------------------------------- 단축키(설정의 [shortcuts])

fn ko_with(config: &str) -> Sim {
    let mut s = sim_with(config);
    s.type_keys("{rs}").unwrap();
    s
}

#[test]
fn combo_shortcuts_switch_languages_even_in_secure_fields() {
    let mut s = sim_with("[shortcuts]\ntoggle_english = \"control+space\"\ntoggle_non_english = \"\"\n");
    s.type_keys("{rs}").unwrap();
    assert_eq!(s.engine.mode(), Mode::En, "오른쪽 Shift 탭은 이제 아무 일도 안 한다");
    s.type_keys("{C-sp}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ko);
    s.type_keys("{ls}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ko, "한↔일 단축키가 없다");
    s.ctx.secure_field = true;
    s.type_keys("{C-sp}").unwrap();
    assert_eq!(s.engine.mode(), Mode::En, "비밀번호 칸에서도 전환은 된다");
}

#[test]
fn hanja_shortcut_follows_the_setting() {
    // 오른쪽 Option 탭으로 바꾸면 그것이 한자 키이고, Option+Return은 예전처럼 앱으로 간다.
    let mut s = ko_with("[shortcuts]\nhanja = \"tap:alt_right\"\n");
    s.type_keys("kre").unwrap();
    s.tap(Key::ALT_RIGHT, Mods::ALT_R);
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "國"));
    s.tap(Key::ALT_RIGHT, Mods::ALT_R);
    assert_eq!(s.preedit, "局", "변환 중에 다시 누르면 다음 후보");
    s.type_keys("{ent}kre{A-ent}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局국", ""), "Option+Return은 조합을 확정하고 앱으로");

    let mut s = ko_with("[shortcuts]\nhanja = \"control+h\"\n");
    s.type_keys("kre{C-h}").unwrap();
    assert_eq!(s.preedit, "國");
}

#[test]
fn japanese_conversion_keys_can_be_turned_off() {
    let mut s = sim_with("[ja]\nconvert_with_space = false\n");
    s.type_keys("{ls}ckeuwl{sp}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("にほんご ", ""), "Space는 확정하고 스페이스");
    assert!(s.candidates.is_none());
    let mut s = sim_with("[ja]\nconvert_with_space = false\nfull_width_space = true\n");
    s.type_keys("{ls}ckeuwl{sp}").unwrap();
    assert_eq!(s.text, "にほんご\u{3000}");
    let mut s = sim_with("[ja]\nconvert_with_tab = false\n");
    s.type_keys("{ls}ckeuwl{tab}").unwrap();
    assert_eq!(s.text, "にほんご\t", "Tab은 확정하고 앱으로");
    s.type_keys("ckeuwl{sp}").unwrap();
    assert_eq!(s.candidates.as_ref().map(|c| c.items[0].as_str()), Some("にほんご"), "Space는 그대로 변환");
}

#[test]
fn yen_sign_on_the_backslash_key() {
    assert_eq!(typed("{ls}\\"), "¥");
    assert_eq!(typed("{ls}|"), "｜");
    // ¥를 끄면 구두점 설정과 상관없이 반각 \(전각 ＼는 내지 않는다). 앱이 치므로 키를 넘긴다.
    for punct in ["japanese", "full_width_western", "half_width_western"] {
        let mut s = sim_with(&format!("[ja]\nyen_sign = false\npunctuation = \"{punct}\"\n"));
        s.type_keys("{ls}ckeu\\").unwrap();
        assert_eq!(s.screen(), "にほん\\", "{punct}");
    }
}

#[test]
fn down_arrow_is_not_a_conversion_key() {
    // 변환 키는 Space와 Tab이다. ↓는 다른 화살표처럼 읽기를 확정하고 앱으로 간다.
    let mut s = sim();
    s.type_keys("{ls}ckeuwl{down}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("にほんご", ""));
    assert!(s.candidates.is_none());
    // 변환 중(후보창)의 ↓는 그대로 다음 후보다.
    s.type_keys("ckeuwl{sp}{down}").unwrap();
    assert_eq!(s.candidates.as_ref().and_then(|c| c.selected), Some(1));
}

#[test]
fn hanja_combo_tells_left_from_right() {
    // 기본은 NRIME처럼 왼쪽 Option+Return. 오른쪽 Option+Return은 한자가 아니다(조합을 확정하고 앱으로).
    let mut s = ko_with("");
    s.type_keys("kre").unwrap();
    s.chord(Key::ALT_RIGHT, Mods::ALT_R, Key::ENTER);
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("국", ""));
    s.type_keys("kre").unwrap();
    s.chord(Key::ALT_LEFT, Mods::ALT_L, Key::ENTER);
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("국", "國"));
}

#[test]
fn control_hanja_shortcut_keeps_the_composition() {
    // ⌘/Control을 누르는 순간 조합을 확정하는데(NRIME, Electron의 ⌘ 단축키), 한자 단축키의 Control이면 남겨 둔다.
    // 0.5.0은 여기서 확정해 버려서 Control+Return이 Discord에 그대로 가서 전송됐다.
    let mut s = ko_with("[shortcuts]\nhanja = \"control_left+enter\"\n");
    s.type_keys("kre").unwrap();
    s.chord(Key::CONTROL_LEFT, Mods::CTRL_L, Key::ENTER);
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("", "國"));
    s.chord(Key::CONTROL_LEFT, Mods::CTRL_L, Key::ENTER);
    assert_eq!(s.preedit, "局", "변환 중에 다시 누르면 다음 후보");
    s.type_keys("{ent}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局", ""));
    // 다른 쪽 Control, 다른 키와의 조합은 전처럼 확정한다.
    s.type_keys("kre").unwrap();
    s.chord(Key::CONTROL_RIGHT, Mods::CTRL_R, Key::ENTER);
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局국", ""));
    s.type_keys("kre{C-a}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局국국", ""));
    // 조합 중이 아니면 Control+Return은 앱으로 간다.
    s.chord(Key::CONTROL_LEFT, Mods::CTRL_L, Key::ENTER);
    assert_eq!(s.preedit, "");
}

#[test]
fn windows_hanja_is_a_right_control_tap() {
    // 윈도우 기본은 오른쪽 Control 탭(한국 키보드의 한자 키 자리). Alt 조합은 앱 메뉴가 먼저 가져가서 쓸 수 없다.
    assert_eq!(Config::windows_default().shortcuts.hanja, crate::shortcut::Shortcut::Tap(Key::CONTROL_RIGHT));
    let mut s = Sim::new(Engine::new(Config::windows_default()));
    s.type_keys("kre{rc}").unwrap();
    assert_eq!(
        (s.text.as_str(), s.preedit.as_str()),
        ("", "國"),
        "Control을 눌러도 조합을 남겨 두고, 떼면 한자"
    );
    s.type_keys("{rc}").unwrap();
    assert_eq!(s.preedit, "局", "변환 중에 다시 톡 하면 다음 후보");
    s.type_keys("{ent}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局", ""));
    // 그 Control로 다른 단축키를 치면 그 키에서 확정하고 앱에 넘긴다(탭이 아니라 한자도 아니다).
    s.type_keys("kre").unwrap();
    s.chord(Key::CONTROL_RIGHT, Mods::CTRL_R, Key::C);
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局국", ""));
    // 왼쪽 Control은 전처럼 누르는 순간 확정한다.
    s.type_keys("kre{C-a}").unwrap();
    assert_eq!((s.text.as_str(), s.preedit.as_str()), ("局국국", ""));
}

// ---------------------------------------------------------------- 빠른 탭 전환 보정

/// 한국어 모드에서 오른쪽 Shift를 누른 채 j를 치고, `release`초 뒤에 Shift를 뗀다(시각은 초).
fn rolled_tap(config: &str, letter_at: f64, release_at: f64) -> Sim {
    let mut s = ko_with(config);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.0 + letter_at));
    if let Some(t) = s.pending_timer.filter(|&t| t <= 10.0 + release_at) {
        s.fire_timer(t);
    }
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.0 + release_at));
    s
}

const BUFFERING: &str = "tap_buffering = true\n";

#[test]
fn quick_tap_buffering_switches_before_the_rolled_letter() {
    // 오른쪽 Shift를 떼기 전에 j를 쳤지만 곧 뗐다: 영어로 바꾼 뒤 j(Graphite h)를 Shift 없이 친다.
    let s = rolled_tap(BUFFERING, 0.03, 0.05);
    assert_eq!((s.engine.mode(), s.screen().as_str()), (Mode::En, "h"));
    // 기본(끔)에서는 Shift를 누른 채 친 j라서 한국어 Shift+j이고 전환하지 않는다.
    let plain = rolled_tap("", 0.03, 0.05);
    assert_eq!(plain.engine.mode(), Mode::Ko);
    let mut expected = ko_with("");
    expected.type_keys("J").unwrap();
    assert_eq!(plain.screen(), expected.screen());
}

#[test]
fn quick_tap_buffering_keeps_shifted_letters_when_not_a_tap() {
    let mut shifted_j = ko_with("");
    shifted_j.type_keys("J").unwrap();
    // 늦게 뗐다(글자를 누르고 30ms 넘게, 참신은 Shift가 글자를 바꾼다): 탭이 아니라 Shift 글자.
    let late = rolled_tap(BUFFERING, 0.03, 0.12);
    assert_eq!((late.engine.mode(), late.screen()), (Mode::Ko, shifted_j.screen()));
    // 타이머가 먼저 울렸다: 누른 그대로 치고, 그 Shift는 이제 탭이 아니다.
    let mut s = ko_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    let out = s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.03));
    assert!(out.consumed && out.commit.is_empty() && out.timer_ms.is_some());
    assert_eq!(s.screen(), "", "잡아 둔 동안은 아무것도 안 보인다");
    let early = s.fire_timer(10.05);
    assert!(early.commit.is_empty() && early.timer_ms.is_some(), "시간 전이면 다시 청한다");
    s.fire_timer(s.pending_timer.unwrap());
    assert_eq!(s.screen(), shifted_j.screen());
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.08));
    assert_eq!(s.engine.mode(), Mode::Ko);
}

/// 한국어에서 Shift+j(참신 Shift 층의 ·)를 친 화면.
fn shifted_j() -> String {
    let mut s = ko_with("");
    s.type_keys("J").unwrap();
    s.screen()
}

#[test]
fn quick_tap_buffering_window_depends_on_what_shift_does() {
    // 新月 글자 키는 Shift를 무시한다(같은 가나): Shift는 탭 말고 뜻이 없어서 글자를 누르고 80ms 안에 떼면 탭이다.
    // 일본어에서 왼쪽 Shift(한↔일)를 누른 채 k를 치고 60ms 뒤에 뗐다: 한국어로 바꾼 뒤 k(참신 초성 ㄱ).
    let mut s = sim_with(BUFFERING);
    s.type_keys("{ls}").unwrap();
    assert_eq!(s.engine.mode(), Mode::Ja);
    s.event(KeyEvent::down(Key::SHIFT_LEFT, Mods(Mods::SHIFT_L), 10.0));
    let out = s.event(KeyEvent::down(Key::K, Mods(Mods::SHIFT_L), 10.03));
    assert!(out.consumed && out.timer_ms.is_some(), "잡아 둔다");
    s.event(KeyEvent::up(Key::SHIFT_LEFT, Mods(0), 10.09));
    assert_eq!((s.engine.mode(), s.preedit.as_str()), (Mode::Ko, "ㄱ"));
    // 참신세벌식은 Shift 층이 따로 있어서(Shift+j = ·) 30ms 안이어야 탭이다. 같은 60ms는 Shift 글자.
    let late = rolled_tap(BUFFERING, 0.03, 0.09);
    assert_eq!((late.engine.mode(), late.screen()), (Mode::Ko, shifted_j()));
    let quick = rolled_tap(BUFFERING, 0.03, 0.055);
    assert_eq!((quick.engine.mode(), quick.screen().as_str()), (Mode::En, "h"));
    // 영어 대문자도 30ms: 오른쪽 Shift를 누른 채 j(Graphite h)를 치고 40ms 뒤에 떼면 대문자 H.
    let mut s = sim_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.02));
    if let Some(t) = s.pending_timer {
        s.fire_timer(t);
    }
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.06));
    assert_eq!((s.engine.mode(), s.screen().as_str()), (Mode::En, "H"));
}

#[test]
fn quick_tap_buffering_leaves_shifted_keys_alone_while_composing_korean() {
    // 낱말 가운데(한글 조합 중)의 Shift 글자는 잡지 않는다: 참신 닫는 따옴표(Shift+f ”)가 곧바로 나간다.
    let mut s = ko_with(BUFFERING);
    s.type_keys("jfs").unwrap();
    assert_eq!(s.preedit, "안");
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    let out = s.event(KeyEvent::down(Key::F, Mods(Mods::SHIFT_R), 10.01));
    assert!(out.timer_ms.is_none(), "잡지 않는다");
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.02));
    assert_eq!((s.engine.mode(), s.screen().as_str()), (Mode::Ko, "안”"));
    // 조합이 끝난 뒤(공백 다음)에는 같은 키도 잡는다(30ms).
    let mut s = ko_with(BUFFERING);
    s.type_keys("jfs ").unwrap();
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    let out = s.event(KeyEvent::down(Key::F, Mods(Mods::SHIFT_R), 10.01));
    assert_eq!(out.timer_ms, Some(30), "잡아 두고 30ms 뒤에 다시 본다");
}

#[test]
fn quick_tap_buffering_waits_for_a_release_on_its_way() {
    // 판정 시간이 지났는데 Shift는 실제로 이미 떼어져 있다: 뗌 이벤트가 앱을 거쳐 늦게 오는 중이다(바쁜 시스템).
    // 누른 그대로 치지 않고 기다렸다가, 도착한 뗌의 시각으로 정한다(NRIME 1.0.12-beta.7).
    let mut s = ko_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.03));
    let deadline = s.pending_timer.unwrap();
    let out = s.fire_timer_held(deadline, Mods(0));
    assert!(out.commit.is_empty() && matches!(out.timer_ms, Some(ms) if ms <= 11), "10ms 뒤에 다시 본다");
    s.fire_timer_held(deadline + 0.2, Mods(0));
    assert_eq!(s.screen(), "", "0.5초까지는 기다린다");
    // 이제 도착했다. 뗀 시각은 글자를 누르고 20ms 뒤였다: 탭.
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.05));
    assert_eq!((s.engine.mode(), s.screen().as_str()), (Mode::En, "h"));

    // 끝내 안 오면(포커스가 옮겨 갔다) 0.5초 뒤에 누른 그대로 친다.
    let mut s = ko_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.03));
    let deadline = s.pending_timer.unwrap();
    s.fire_timer_held(deadline, Mods(0));
    let out = s.fire_timer_held(deadline + 0.5, Mods(0));
    assert!(out.timer_ms.is_none());
    assert_eq!((s.engine.mode(), s.screen()), (Mode::Ko, shifted_j()));
    // 셸이 좌우를 몰라 양쪽 비트를 켜 주면 누르고 있는 것으로 본다: 기다리지 않고 누른 그대로.
    let mut s = ko_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.03));
    let deadline = s.pending_timer.unwrap();
    let out = s.fire_timer_held(deadline, Mods(Mods::SHIFT_L | Mods::SHIFT_R));
    assert!(out.timer_ms.is_none() && s.screen() == shifted_j());
}

#[test]
fn a_key_the_shell_held_back_cancels_the_tap() {
    // Codex에서 Shift+Enter를 다시 보내기 전에 셸이 잡아 둔 키는 엔진을 거치지 않는다. 그래도 Shift를 누른 채 친 것이면
    // 탭이 아니다(셸이 cancel_tap으로 알린다). 알리지 않으면 대문자 하나가 언어 전환이 된다.
    let mut s = ko_with("");
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.engine.cancel_tap();
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.05));
    assert_eq!(s.engine.mode(), Mode::Ko);
}

#[test]
fn quick_tap_buffering_flushes_on_the_next_key_and_respects_combos() {
    // 잡아 둔 뒤 다른 글자: 둘 다 Shift 글자(대문자·기호를 치는 중이다).
    let mut s = ko_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.02));
    s.event(KeyEvent::down(Key::K, Mods(Mods::SHIFT_R), 10.04));
    s.event(KeyEvent::up(Key::SHIFT_RIGHT, Mods(0), 10.05));
    let mut expected = ko_with("");
    expected.type_keys("JK").unwrap();
    assert_eq!((s.engine.mode(), s.screen()), (Mode::Ko, expected.screen()));
    // Shift를 쓰는 조합 단축키가 있으면 Shift 탭을 가로채지 않는다(NRIME와 같다).
    let with_combo =
        rolled_tap(&format!("{BUFFERING}[shortcuts]\ntoggle_non_english = \"shift+space\"\n"), 0.03, 0.05);
    assert_eq!(with_combo.engine.mode(), Mode::Ko);
    // 클릭: 누른 그대로 친다.
    let mut s = ko_with(BUFFERING);
    s.event(KeyEvent::down(Key::SHIFT_RIGHT, Mods(Mods::SHIFT_R), 10.0));
    s.event(KeyEvent::down(Key::J, Mods(Mods::SHIFT_R), 10.02));
    s.type_keys("{click}").unwrap();
    assert_eq!(s.screen(), expected.screen().chars().take(1).collect::<String>());
}
