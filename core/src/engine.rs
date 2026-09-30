//! 엔진: 모드 전환, 키 라우팅, 셸이 할 일([`Output`]).
//!
//! 셸 규칙(NRIME 교훈):
//! - `commit`이 있으면 `insertText(commit)`만 부른다. insertText가 조합 중 글자를 대신하므로
//!   그 앞뒤에 `setMarkedText("")`를 부르지 않는다(Chromium·JS 에디터에서 글자가 사라진다).
//! - 그다음 `preedit`가 Some이고 비어 있지 않으면 `setMarkedText(preedit)`.
//!   `commit` 없이 `preedit`가 Some(빈 값)이면 조합 중 글자를 지운다.
//! - `consumed`가 false면 원래 키를 앱에 넘긴다(맥: handle에서 false 반환).

use crate::config::{Config, JaConfig, JaPunct};
use crate::convert::{ConvCmd, ConvView, Converter, EchoConverter};
use crate::hangul::{KoComposer, KoLayout, KoResult};
use crate::hanja::Learning;
use crate::hotkey::TapTracker;
use crate::kana::{KanaComposer, KanaLayout, to_katakana};
use crate::key::{Key, KeyEvent, Mods};
use crate::latin::{LatinLayout, qwerty_char_for};
use crate::shortcut::ShortcutAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Mode {
    En = 0,
    Ko = 1,
    Ja = 2,
}

impl Mode {
    pub fn from_i32(v: i32) -> Option<Mode> {
        match v {
            0 => Some(Mode::En),
            1 => Some(Mode::Ko),
            2 => Some(Mode::Ja),
            _ => None,
        }
    }
}

/// 셸이 알려주는 입력 문맥.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    /// 윈도우 게임 모드: 영어 모드가 쿼티 그대로 통과한다.
    pub game_mode: bool,
    /// 수식키 탭 전환을 끈다(플레이 중에도 IME를 켜 두는 게임 등).
    pub taps_disabled: bool,
    /// 비밀번호 칸 등: 조합하지 않고 키를 모두 넘긴다. 언어 전환 탭은 그대로 된다(갇히지 않게).
    pub secure_field: bool,
}

/// 조합 중 글자와 밑줄 구간. 구간 위치는 글자(char) 단위다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub start: usize,
    pub len: usize,
    /// 변환 중 포커스된 문절(굵은 밑줄).
    pub focused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidates {
    /// 포커스된 문절의 후보 전체.
    pub items: Vec<String>,
    /// 후보마다 같이 보일 뜻(한자의 훈음 등). 없으면 빈 목록.
    pub notes: Vec<String>,
    /// 고른 후보(전체 목록 기준).
    pub selected: Option<usize>,
    /// (지금 페이지, 전체 페이지), 1부터. 페이지 크기는 목록/격자에 따라 다르다.
    pub page: Option<(usize, usize)>,
    /// 격자(펼친) 모드. Tab으로 바꾼다(NRIME와 같다).
    pub grid: bool,
}

/// 후보창 한 페이지: 목록 9개, 격자 5열 × 6행. 맥 셸 CandidatePanel과 같아야 한다.
pub const CAND_LIST_PAGE: usize = 9;
pub const CAND_GRID_COLUMNS: usize = 5;
pub const CAND_GRID_PAGE: usize = 30;

/// 키 하나를 처리한 뒤 셸이 할 일.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    /// true면 키를 먹는다. false면 `commit`을 넣은 뒤 원래 키를 앱에 넘긴다.
    pub consumed: bool,
    /// 앱에 확정해 넣을 글자. 없으면 빈 문자열.
    pub commit: String,
    /// 조합 중 글자. None이면 그대로 둔다.
    pub preedit: Option<Preedit>,
    /// 후보창. None이면 그대로, Some(None)이면 닫는다.
    pub candidates: Option<Option<Candidates>>,
    /// 모드가 바뀌었으면 새 모드.
    pub mode: Option<Mode>,
    /// Caps Lock을 꺼 달라는 요청.
    pub caps_lock_off: bool,
    /// 한자 학습이 바뀌었다(셸이 저장한다).
    pub learning_changed: bool,
    /// 이만큼(밀리초) 뒤에 [`Engine::timer`]를 불러 달라(빠른 탭 전환 보정이 잡아 둔 글자를 내보낼 때).
    pub timer_ms: Option<u32>,
}

impl Output {
    fn pass() -> Self {
        Self::default()
    }
    fn eat() -> Self {
        Self { consumed: true, ..Self::default() }
    }
    fn commit_pass(commit: String) -> Self {
        Self { commit, ..Self::default() }
    }
    fn commit_eat(commit: String) -> Self {
        Self { consumed: true, commit, ..Self::default() }
    }
}

pub struct Engine {
    config: Config,
    mode: Mode,
    last_non_en: Mode,
    ko_layout: KoLayout,
    ko: KoComposer,
    kana_layout: KanaLayout,
    kana: KanaComposer,
    /// 변환 중이면 Some.
    conv: Option<ConvView>,
    /// 한국어 한자 변환 중이면 Some.
    hanja: Option<ko_hanja::HanjaConv>,
    hanja_learning: Learning,
    learning_dirty: bool,
    /// 후보창이 격자(펼친) 모드인지. 변환을 시작할 때 목록으로 돌아간다.
    cand_grid: bool,
    converter: Box<dyn Converter>,
    latin: LatinLayout,
    taps: TapTracker,
    /// 마지막으로 본 Caps Lock 상태. 일본어 모드에서는 가타카나 입력이다.
    caps: bool,
    /// 빠른 탭 전환 보정이 잡아 둔 글자(탭할 수식키를 떼기 전에 누른 것).
    buffered: Option<Buffered>,
}

/// 빠른 탭 전환 보정: 탭할 수식키를 누른 채 친 글자를 잠깐 잡아 둔다(NRIME와 같다).
/// 수식키를 곧(판정 시간 안에) 떼면 탭으로 보고 전환한 뒤 그 글자를 수식키 없이 친다.
/// 늦게 떼거나, 다른 키를 치거나, 시간이 다 되면 누른 그대로(Shift 글자) 친다.
#[derive(Clone, Copy, Debug)]
struct Buffered {
    ev: KeyEvent,
    modifier: Key,
    /// 글자를 누르고 이 시간 안에 수식키를 떼면 탭이다(Shift가 이 글자를 바꾸는지에 따라, 아래 상수).
    window: f64,
    /// 이 시각이 지나면 누른 그대로 친다(셸이 타이머로 [`Engine::timer`]를 부른다).
    deadline: f64,
    /// 시간이 다 됐을 때 수식키가 이미 떼어져 있어서, 뗌 이벤트를 기다리기 시작한 때.
    waiting_since: Option<f64>,
}

/// 빠른 탭 전환 보정의 판정 시간(NRIME 1.0.12-beta.7, 사용자 타자를 재서 정한 고정값). 글자를 누르고 이 안에 수식키를 떼면 탭.
/// Shift가 글자를 바꾸는 키(영어 대문자, 참신세벌식 Shift 기호, 숫자·기호 자리): 30ms. NRIME 사용자의 빠른 쌍자음은
/// 글자를 누르고 47ms 만에 Shift를 떼기도 해서(5%가 54ms 안), 0.6.1까지의 한 가지 50ms로는 50번에 1번꼴로 전환됐다.
const SHIFTED_LETTER_TAP_WINDOW: f64 = 0.030;
/// Shift가 아무것도 바꾸지 않는 키(新月의 글자 키는 Shift를 무시하고 같은 가나): Shift는 탭 말고는 뜻이 없어서 80ms.
/// 그래도 끝은 있다. 글자를 한참 넘게 누른 Shift는 탭이 아니다.
const SHIFTLESS_LETTER_TAP_WINDOW: f64 = 0.080;
/// 시간이 다 됐는데 수식키가 실제로는 이미 떼어져 있으면, 뗌 이벤트가 앱을 거쳐 오는 중이다(시스템이 바쁘면 키보드보다
/// 늦다). 그 이벤트의 시각으로 판정하도록 10ms마다, 0.5초까지 기다린다(NRIME 1.0.12-beta.7). 0.6.1까지는 20ms를 한 번
/// 더 기다리고 말아서, 느릴 때 탭이 쌍자음·대문자로 나갔다. 끝내 안 오면(포커스가 옮겨 갔다) 누른 그대로 친다.
const RELEASE_POLL: f64 = 0.010;
const RELEASE_WAIT_LIMIT: f64 = 0.5;

impl Engine {
    pub fn new(config: Config) -> Self {
        let ko_layout = KoLayout::builtin(&config.ko_layout)
            .unwrap_or_else(|| KoLayout::builtin("chamshin-v18").unwrap());
        Self {
            config,
            // 입력기를 켜면 한국어로 시작한다(2026-09-29 사용자 요청).
            mode: Mode::Ko,
            last_non_en: Mode::Ko,
            ko_layout,
            ko: KoComposer::new(),
            kana_layout: KanaLayout::shingetsu(),
            kana: KanaComposer::new(),
            conv: None,
            hanja: None,
            hanja_learning: Learning::default(),
            learning_dirty: false,
            cand_grid: false,
            converter: Box::new(EchoConverter::default()),
            latin: LatinLayout::graphite(),
            taps: TapTracker::default(),
            caps: false,
            buffered: None,
        }
    }

    pub fn set_converter(&mut self, converter: Box<dyn Converter>) {
        if self.conv.take().is_some() {
            self.converter.cancel();
        }
        self.converter = converter;
    }

    /// 변환기의 사용자 사전을 다시 읽는다(설정 앱이 고친 뒤). 변환 중이면 버리고 읽기로 돌아간다.
    pub fn reload_converter(&mut self) {
        if self.conv.take().is_some() {
            self.converter.cancel();
        }
        self.converter.reload();
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// 설정을 바꾼다(설정 앱이 설정 파일을 고쳤을 때). 조합 중인 것·모드·학습은 그대로 두고 다음 키부터 새 설정을 쓴다.
    pub fn set_config(&mut self, config: Config) {
        if let Some(layout) = KoLayout::builtin(&config.ko_layout) {
            self.ko_layout = layout;
        }
        self.config = config;
    }

    /// 키 이벤트 하나를 처리한다.
    pub fn handle_key(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        let before = self.snapshot();
        let out = self.route(ev, ctx);
        self.finish(out, before)
    }

    /// 조합 중인 것을 모두 확정한다(마우스 클릭, 포커스 해제). 잡아 둔 글자가 있으면 누른 그대로 먼저 친다.
    pub fn commit_all(&mut self) -> Output {
        let before = self.snapshot();
        let flushed = self.flush_buffered(&Context::default());
        let out = merge(flushed, Output::commit_eat(self.take_composition()));
        self.finish(out, before)
    }

    /// 셸이 `timer_ms`만큼 기다렸다 부른다(`now`는 키 이벤트와 같은 시계, 초). `held`는 지금 실제로 누르고 있는
    /// 수식키다(셸이 좌우를 모르면 양쪽 비트를 켠다).
    /// 빠른 탭 전환 보정이 잡아 둔 글자를 시간이 다 됐으면 누른 그대로 친다. 그 수식키가 이미 떼어졌으면 오는 중인
    /// 뗌 이벤트를 기다린다(RELEASE_WAIT_LIMIT).
    pub fn timer(&mut self, now: f64, held: Mods) -> Output {
        let before = self.snapshot();
        let out = match self.buffered {
            Some(b) if now + 0.001 >= b.deadline => {
                let released = held.0 & b.modifier.modifier_family_bits() == 0;
                let since = b.waiting_since.unwrap_or(now);
                if released && now - since < RELEASE_WAIT_LIMIT {
                    self.buffered = Some(Buffered { waiting_since: Some(since), ..b });
                    Output { consumed: true, timer_ms: Some(millis(RELEASE_POLL)), ..Output::default() }
                } else {
                    self.flush_buffered(&Context::default())
                }
            }
            Some(b) => {
                Output { consumed: true, timer_ms: Some(millis(b.deadline - now)), ..Output::default() }
            }
            None => Output::default(),
        };
        self.finish(out, before)
    }

    /// 마우스 클릭: 조합을 확정하고 진행 중인 탭을 무효로 한다.
    pub fn mouse_down(&mut self) -> Output {
        self.taps.cancel();
        self.commit_all()
    }

    /// 확정하지 않고 모두 버린다(입력기 활성화 때. 여기서 확정하면 Electron에서 글자가 겹친다).
    pub fn reset(&mut self) -> Output {
        let before = self.snapshot();
        self.ko.clear();
        self.kana.clear();
        if self.conv.take().is_some() {
            self.converter.cancel();
        }
        self.hanja = None;
        self.buffered = None;
        self.taps.cancel();
        self.finish(Output::eat(), before)
    }

    /// 모드를 바로 바꾼다(메뉴 등). 조합 중인 것은 확정한다.
    pub fn set_mode(&mut self, mode: Mode) -> Output {
        let before = self.snapshot();
        let flushed = self.flush_buffered(&Context::default());
        let out = merge(flushed, self.switch_to(mode));
        self.finish(out, before)
    }

    fn route(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        self.caps = ev.mods.caps();
        let threshold = self.config.tap_threshold_ms as f64 / 1000.0;

        // 빠른 탭 전환 보정: 잡아 둔 글자가 있으면 이 이벤트로 정한다.
        if let Some(b) = self.buffered {
            if !ev.down && ev.key == b.modifier {
                self.buffered = None;
                let tapped = self.taps.observe(ev, threshold) == Some(b.modifier);
                let quick = ev.time - b.ev.time < b.window;
                let action = self.config.shortcuts.for_tap(b.modifier);
                if let (true, true, Some(action), false) = (tapped, quick, action, ctx.taps_disabled) {
                    let switched = self.shortcut_action(action, ctx);
                    let bare = KeyEvent { mods: Mods(b.ev.mods.0 & !b.modifier.modifier_bit()), ..b.ev };
                    return merge(switched, self.replay(&bare, ctx));
                }
                // 탭이 아니다: 누른 그대로 친다.
                return self.replay(&b.ev, ctx);
            }
            if ev.down || ev.key.is_modifier() {
                // 다른 키나 수식키: 잡아 둔 글자를 누른 그대로 먼저 치고 이 이벤트를 처리한다.
                let flushed = self.flush_buffered(ctx);
                return merge(flushed, self.route(ev, ctx));
            }
            return Output::pass();
        }
        if let Some(out) = self.try_buffer(ev, ctx, threshold) {
            return out;
        }

        if let Some(tapped) = self.taps.observe(ev, threshold)
            && !ctx.taps_disabled
            && let Some(action) = self.config.shortcuts.for_tap(tapped)
        {
            return self.shortcut_action(action, ctx);
        }
        self.route_key(ev, ctx)
    }

    /// 탭 판정 뒤의 키 처리.
    fn route_key(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        // ⌘/Ctrl을 누르는 순간 조합을 확정한다(NRIME). Electron에서는 ⌘+키가 handle을 거치지 않고
        // performKeyEquivalent로 가서, 조합 중 글자가 그 단축키에 먹힌다.
        if ev.down
            && !ev.repeat
            && matches!(ev.key, Key::META_LEFT | Key::META_RIGHT | Key::CONTROL_LEFT | Key::CONTROL_RIGHT)
        {
            // 한자 단축키가 이 Control을 쓰면(예: 왼쪽 Control+Return) 조합을 남겨 둔다. 여기서 확정하면 단축키가 올 때
            // 바꿀 글자가 없어서 Return이 앱으로 간다(Discord에서 전송, 0.5.0). Control+키는 Chrome도 입력기에 보낸다.
            if self.mode == Mode::Ko
                && !ctx.secure_field
                && self.config.shortcuts.hanja.combo_uses(ev.key)
                && (self.hanja.is_some() || !self.ko.preedit().is_empty())
            {
                return Output::pass();
            }
            return Output::commit_pass(self.take_composition());
        }

        if !ev.down || ev.key.is_modifier() || ev.key == Key::CAPS_LOCK {
            return Output::pass();
        }

        // 조합 단축키. 언어 전환은 비밀번호 칸에서도 된다(갇히지 않게). 한자는 한국어 모드에서만.
        if let Some(action) = self.config.shortcuts.for_combo(ev) {
            let hanja = action == ShortcutAction::Hanja;
            if (!ev.repeat || hanja) && (!hanja || (self.mode == Mode::Ko && !ctx.secure_field)) {
                return self.shortcut_action(action, ctx);
            }
        }

        if ctx.secure_field {
            return Output::pass();
        }

        // ⌘/Ctrl/Option 조합: 조합을 확정하고 앱에 넘긴다(단축키는 쿼티 자리 그대로).
        if ev.mods.command_like() {
            return Output::commit_pass(self.take_composition());
        }

        match self.mode {
            Mode::En => self.en_key(ev, ctx),
            Mode::Ko if self.hanja.is_some() => self.hanja_key(ev),
            Mode::Ko => self.ko_key(ev),
            Mode::Ja => self.ja_key(ev),
        }
    }

    fn shortcut_action(&mut self, action: ShortcutAction, ctx: &Context) -> Output {
        match action {
            ShortcutAction::ToggleEnglish => self.toggle_english(),
            ShortcutAction::ToggleNonEnglish => self.toggle_non_english(),
            ShortcutAction::Hanja if self.mode == Mode::Ko && !ctx.secure_field => self.hanja_shortcut(),
            ShortcutAction::Hanja => Output::pass(),
        }
    }

    /// 빠른 탭 전환 보정을 시작할지: 켜져 있고, 탭 단축키 수식키 하나만 누른 채 탭 시간 안에 글자를 쳤고,
    /// 그 수식키를 쓰는 조합 단축키가 없을 때(NRIME와 같다). 한글 조합 중에 Shift가 바꾸는 키는 잡지 않는다.
    /// 잡으면 키를 먹고 타이머를 청한다.
    fn try_buffer(&mut self, ev: &KeyEvent, ctx: &Context, threshold: f64) -> Option<Output> {
        if !self.config.tap_buffering || ctx.taps_disabled || ctx.secure_field {
            return None;
        }
        if !ev.down || ev.repeat || !ev.key.is_printable() || ev.mods.command_like() {
            return None;
        }
        let (modifier, pressed) = self.taps.candidate()?;
        let only_it = !ev.mods.any_held_except(modifier.modifier_bit());
        if self.config.shortcuts.for_tap(modifier).is_none()
            || ev.time - pressed >= threshold
            || !only_it
            || self.config.shortcuts.combo_uses_family_of(modifier)
        {
            return None;
        }
        let shift_matters = self.shift_matters(ev.key);
        // 낱말 가운데서 언어를 바꾸는 사람은 없다. 한글을 조합하는 중에 Shift가 바꾸는 키(참신세벌식의 닫는 따옴표·말줄임표
        // 같은 Shift 기호, 받침 ㅋ)는 잡지 않고 바로 친다(NRIME: 쌍자음의 68%가 낱말 가운데였고 가장 빠른 다섯이 모두 그랬다).
        if self.mode == Mode::Ko && shift_matters && (self.hanja.is_some() || !self.ko.preedit().is_empty()) {
            return None;
        }
        let window = if shift_matters { SHIFTED_LETTER_TAP_WINDOW } else { SHIFTLESS_LETTER_TAP_WINDOW };
        let deadline = (ev.time + window).min(pressed + threshold);
        self.buffered = Some(Buffered { ev: *ev, modifier, window, deadline, waiting_since: None });
        Some(Output { consumed: true, timer_ms: Some(millis(deadline - ev.time)), ..Output::default() })
    }

    /// Shift가 이 키의 글자를 바꾸는지(지금 모드에서). 바꾸지 않는 키에서 Shift는 탭 말고는 뜻이 없다.
    /// - 영어(Graphite): 글자는 대문자, 기호 자리는 Shift 기호.
    /// - 한국어(참신세벌식): Shift 층이 따로 있어서(「」『』…· 같은 기호, 받침 ㅋ, ❖) 사실상 모든 글자 키를 바꾼다.
    /// - 일본어(新月): 글자 키(A~Z 자리)는 Shift를 무시하고 같은 가나를 낸다. 숫자·기호 자리는 Shift 기호.
    fn shift_matters(&self, key: Key) -> bool {
        let qwerty = key.qwerty_char(false) != key.qwerty_char(true);
        match self.mode {
            Mode::En => {
                self.latin.char_for(key, false, false, false) != self.latin.char_for(key, true, false, false)
            }
            Mode::Ko => match (self.ko_layout.roles(key, false), self.ko_layout.roles(key, true)) {
                (None, None) => qwerty,
                (base, shifted) => base != shifted,
            },
            Mode::Ja => !key.is_letter() && qwerty,
        }
    }

    /// 셸이 엔진에 넘기지 않은 키가 눌렸다(Shift+Enter를 다시 보내기 전에 잡아 둔 키). 진행 중인 수식키 탭을 무효로 한다:
    /// 그 키를 Shift와 같이 눌렀다면 Shift는 탭이 아니다.
    pub fn cancel_tap(&mut self) {
        self.taps.cancel();
    }

    /// 잡아 둔 글자를 누른 그대로 친다. 그 수식키는 이제 탭이 아니다.
    fn flush_buffered(&mut self, ctx: &Context) -> Output {
        let Some(b) = self.buffered.take() else { return Output::default() };
        self.taps.cancel();
        self.replay(&b.ev, ctx)
    }

    /// 잡아 둔 키를 지금 친다. 원래 키는 이미 먹었으니 앱에 넘길 수 없다: 넘겨야 하는 키면 앱이 넣었을 글자를 대신 넣는다.
    fn replay(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        let mut out = self.route_key(ev, ctx);
        if !out.consumed {
            if let Some(c) =
                qwerty_char_for(ev.key, ev.mods.shift(), ev.mods.caps(), self.config.caps_shift_inverts)
            {
                out.commit.push(c);
            }
            out.consumed = true;
        }
        out
    }

    // ---------------------------------------------------------------- 모드

    fn toggle_english(&mut self) -> Output {
        let target = if self.mode == Mode::En { self.last_non_en } else { Mode::En };
        self.switch_to(target)
    }

    fn toggle_non_english(&mut self) -> Output {
        let target = match self.mode {
            Mode::Ko => Mode::Ja,
            Mode::Ja => Mode::Ko,
            Mode::En => {
                if self.last_non_en == Mode::Ko {
                    Mode::Ja
                } else {
                    Mode::Ko
                }
            }
        };
        self.switch_to(target)
    }

    fn switch_to(&mut self, mode: Mode) -> Output {
        let mut out = Output::commit_pass(self.take_composition());
        if mode == self.mode {
            return out;
        }
        if self.mode == Mode::Ja && self.caps && self.config.ja.caps_katakana_auto_off {
            out.caps_lock_off = true;
            self.caps = false;
        }
        if mode != Mode::En {
            self.last_non_en = mode;
        }
        self.mode = mode;
        out.mode = Some(mode);
        out
    }

    /// 조합 중인 것(한글 음절, 한자 변환 결과, 가나 읽기, 변환 결과)을 확정할 글자로 꺼내고 비운다.
    fn take_composition(&mut self) -> String {
        let mut text = self.ko.flush();
        text += &self.hanja_commit();
        if self.conv.take().is_some() {
            text += &self.converter.commit();
            self.kana.clear();
        }
        if !self.kana.is_empty() {
            let reading = self.kana.take();
            text += &self.ja_text(&reading);
        }
        text
    }

    // ---------------------------------------------------------------- 영어

    fn en_key(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        if ctx.game_mode {
            return Output::pass();
        }
        let (shift, caps, inv) = (ev.mods.shift(), ev.mods.caps(), self.config.caps_shift_inverts);
        match self.latin.char_for(ev.key, shift, caps, inv) {
            Some(c) if Some(c) != qwerty_char_for(ev.key, shift, caps, inv) => {
                Output::commit_eat(c.to_string())
            }
            _ => Output::pass(),
        }
    }

    // ---------------------------------------------------------------- 한국어

    fn ko_key(&mut self, ev: &KeyEvent) -> Output {
        let shift = ev.mods.shift();
        if ev.key == Key::BACKSPACE {
            return if self.ko.backspace() { Output::eat() } else { Output::pass() };
        }
        let Some(roles) = self.ko_layout.roles(ev.key, shift) else {
            // 배열에 없는 키(쿼티 기호, Space, Enter, 화살표…): 음절을 확정하고 앱에 넘긴다.
            return Output::commit_pass(self.ko.flush());
        };
        match self.ko.input(&self.ko_layout, roles) {
            KoResult::Composed { commit } | KoResult::Stopped { commit } => Output::commit_eat(commit),
            KoResult::Symbol { commit, sym } => {
                let qwerty = qwerty_char_for(ev.key, shift, ev.mods.caps(), self.config.caps_shift_inverts);
                if qwerty.map(String::from).as_deref() == Some(sym.as_str()) {
                    // 쿼티와 같은 글자는 먹지 않는다(비번 칸 등에서 삼켜지는 것을 막는다).
                    Output::commit_pass(commit)
                } else {
                    Output::commit_eat(commit + &sym)
                }
            }
        }
    }

    // ---------------------------------------------------------------- 일본어

    fn ja_key(&mut self, ev: &KeyEvent) -> Output {
        if self.conv.is_some() {
            return self.ja_converting_key(ev);
        }
        let (k, shift, katakana) = (ev.key, ev.mods.shift(), ev.mods.caps());

        match k {
            Key::BACKSPACE => {
                return if self.kana.backspace() { Output::eat() } else { Output::pass() };
            }
            Key::ESCAPE => {
                if self.kana.cancel_pending() {
                    return Output::eat();
                }
                if !self.kana.is_empty() {
                    self.kana.clear();
                    return Output::eat();
                }
                return Output::pass();
            }
            Key::SPACE | Key::TAB if !self.kana.reading().is_empty() => {
                self.kana.cancel_pending();
                // 변환 키는 Space와 Tab 둘이다(설정). ↓는 변환 키가 아니다(사용자 결정, 다른 화살표처럼 확정하고 앱으로).
                let trigger = if k == Key::SPACE {
                    self.config.ja.convert_with_space
                } else {
                    self.config.ja.convert_with_tab
                };
                if !trigger {
                    // 변환 키가 아니다: 읽기를 확정한다. Space는 스페이스를 넣고 Tab은 앱에 넘긴다.
                    let commit = self.take_composition();
                    if k == Key::SPACE {
                        let space = if self.config.ja.full_width_space { "\u{3000}" } else { " " };
                        return Output::commit_eat(commit + space);
                    }
                    return Output::commit_pass(commit);
                }
                if katakana || (k == Key::TAB && shift) {
                    // 가타카나 입력은 변환하지 않는다: 확정하고 키는 앱으로.
                    return Output::commit_pass(self.take_composition());
                }
                let reading = self.ja_styled(self.kana.reading());
                if let Some(view) = self.converter.start(&reading) {
                    self.conv = Some(view);
                    self.cand_grid = false;
                }
                // 변환 엔진이 아직 없으면 읽기를 그대로 둔다(입력 스레드는 기다리지 않는다).
                return Output::eat();
            }
            Key::SPACE => {
                self.kana.cancel_pending();
                if self.config.ja.full_width_space {
                    return Output::commit_eat("\u{3000}".into());
                }
                return Output::pass();
            }
            Key::ENTER | Key::NUMPAD_ENTER => {
                self.kana.cancel_pending();
                if self.kana.reading().is_empty() {
                    return Output::pass();
                }
                // Enter는 확정만 한다(줄바꿈 없음). Shift+Enter는 확정하고 Enter를 앱에 넘겨 줄을 바꾼다.
                let commit = self.take_composition();
                return if shift { Output::commit_pass(commit) } else { Output::commit_eat(commit) };
            }
            _ => {}
        }

        if k.is_printable() {
            // 글자 키는 Shift를 무시하고 가나. 그 밖의 키(숫자·기호 자리)는 Shift를 따른다.
            if (k.is_letter() || !shift) && self.kana.key(&self.kana_layout, k) {
                return self.ja_typed(katakana);
            }
            if k.is_digit() && !shift {
                // 숫자는 읽기에 넣는다(3じ → 3時 변환).
                let d = k.qwerty_char(false).unwrap();
                self.kana.push_str(&d.to_string());
                return self.ja_typed(katakana);
            }
            // 그 밖의 기호: 읽기를 확정하고 설정대로 기호를 낸다.
            let ascii = k.qwerty_char(shift).unwrap();
            let commit = self.take_composition();
            let sym = ja_symbol(&self.config.ja, ascii);
            if sym == ascii.to_string() {
                return Output::commit_pass(commit);
            }
            return Output::commit_eat(commit + &sym);
        }

        // 화살표, Delete 등: 읽기를 확정하고 앱에 넘긴다.
        Output::commit_pass(self.take_composition())
    }

    /// 가나(또는 숫자)를 읽기에 넣은 뒤. 가타카나 입력(Caps Lock)은 변환하지 않으므로 바로 확정하고,
    /// 뒤치기(゛)가 아직 바꿀 수 있는 마지막 키의 글자만 조합으로 남긴다(ja.katakana_direct).
    fn ja_typed(&mut self, katakana: bool) -> Output {
        if katakana && self.config.ja.katakana_direct {
            let settled = self.kana.take_settled();
            if !settled.is_empty() {
                return Output::commit_eat(self.ja_text(&settled));
            }
        }
        Output::eat()
    }

    /// 변환 중 키. 후보창 조작은 NRIME와 같다.
    /// - Space / ↓: 다음 후보, ↑: 이전 후보(끝에서 처음으로 돈다).
    /// - Tab: 목록 ↔ 격자(펼치기). 격자에서는 ←→가 한 칸, ↑↓가 한 줄.
    /// - 목록에서 ←→: 문절이 하나면 페이지를 넘기고, 여럿이면 문절을 옮긴다. Shift+←→는 문절 길이.
    /// - PageUp/PageDown: 페이지. 1~9: 지금 페이지에서 골라 바로 확정. Enter: 확정. Esc/Backspace: 읽기로 돌아간다.
    fn ja_converting_key(&mut self, ev: &KeyEvent) -> Output {
        let (k, shift) = (ev.key, ev.mods.shift());
        let (n, sel, segments) = match &self.conv {
            Some(v) => (v.candidates.len(), v.selected.unwrap_or(0), v.segments.len()),
            None => (0, 0, 0),
        };
        let grid = self.cand_grid && n > 0;
        let page = if grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
        let pick = |i: usize| (i < n).then_some(ConvCmd::Select(i));
        let cmd = match k {
            Key::TAB if n > 0 => {
                self.cand_grid = !self.cand_grid;
                return Output::eat();
            }
            Key::SPACE | Key::ARROW_DOWN if n == 0 => Some(ConvCmd::Next),
            Key::ARROW_UP if n == 0 => Some(ConvCmd::Prev),
            Key::SPACE => pick((sel + 1) % n),
            Key::ARROW_DOWN if grid => pick(sel + CAND_GRID_COLUMNS),
            Key::ARROW_UP if grid => sel.checked_sub(CAND_GRID_COLUMNS).and_then(pick),
            Key::ARROW_DOWN => pick((sel + 1) % n),
            Key::ARROW_UP => pick((sel + n - 1) % n),
            Key::ARROW_LEFT if shift => Some(ConvCmd::Shrink),
            Key::ARROW_RIGHT if shift => Some(ConvCmd::Expand),
            Key::ARROW_LEFT if grid => sel.checked_sub(1).and_then(pick),
            Key::ARROW_RIGHT if grid => pick(sel + 1),
            Key::ARROW_LEFT if segments <= 1 && n > 0 => pick(sel.saturating_sub(page)),
            Key::ARROW_RIGHT if segments <= 1 && n > 0 => pick((sel + page).min(n - 1)),
            Key::ARROW_LEFT => Some(ConvCmd::FocusLeft),
            Key::ARROW_RIGHT => Some(ConvCmd::FocusRight),
            Key::PAGE_DOWN if n > 0 => pick((sel + page).min(n - 1)),
            Key::PAGE_UP if n > 0 => pick(sel.saturating_sub(page)),
            Key::PAGE_DOWN | Key::PAGE_UP => None,
            _ => {
                return self.ja_converting_other(ev);
            }
        };
        if let Some(cmd) = cmd
            && let Some(view) = self.converter.command(cmd)
        {
            self.conv = Some(view);
        }
        Output::eat()
    }

    /// 변환 중 후보 이동이 아닌 키: 확정, 취소, 번호 고르기, 그 밖의 키(확정하고 새로 처리).
    fn ja_converting_other(&mut self, ev: &KeyEvent) -> Output {
        let (k, shift) = (ev.key, ev.mods.shift());
        match k {
            Key::ENTER | Key::NUMPAD_ENTER => {
                let commit = self.take_composition();
                return if shift { Output::commit_pass(commit) } else { Output::commit_eat(commit) };
            }
            Key::ESCAPE | Key::BACKSPACE => {
                // 변환 취소: 읽기로 돌아간다.
                self.converter.cancel();
                self.conv = None;
                return Output::eat();
            }
            _ => {}
        }
        if k.is_digit() && !shift && k != Key::DIGIT0 {
            let (n, sel) =
                self.conv.as_ref().map_or((0, 0), |v| (v.candidates.len(), v.selected.unwrap_or(0)));
            let page = if self.cand_grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
            let target = sel / page * page + (k.0 - Key::DIGIT1.0) as usize;
            if target < n {
                // 번호로 고르면 바로 확정한다(NRIME와 같다).
                if let Some(view) = self.converter.command(ConvCmd::Select(target)) {
                    self.conv = Some(view);
                }
                return Output::commit_eat(self.take_composition());
            }
        }
        // 그 밖의 키: 변환을 확정하고 그 키를 새로 처리한다.
        let committed = self.take_composition();
        let mut out = self.ja_key(ev);
        out.commit = committed + &out.commit;
        out
    }

    /// 읽기 → 화면/확정 글자: 구두점 설정, Caps Lock 가타카나.
    fn ja_text(&self, reading: &str) -> String {
        let styled = self.ja_styled(reading);
        if self.caps { to_katakana(&styled) } else { styled }
    }

    fn ja_styled(&self, reading: &str) -> String {
        ja_styled(&self.config.ja, reading)
    }

    // ---------------------------------------------------------------- 화면 상태

    fn snapshot(&self) -> (Preedit, Option<Candidates>) {
        (self.preedit_now(), self.candidates_now())
    }

    fn preedit_now(&self) -> Preedit {
        let plain = |text: String| {
            let len = text.chars().count();
            let segments = if len == 0 { vec![] } else { vec![Segment { start: 0, len, focused: false }] };
            Preedit { text, segments }
        };
        match self.mode {
            Mode::En => Preedit::default(),
            Mode::Ko => match &self.hanja {
                Some(h) => h.preview(),
                None => plain(self.ko.preedit()),
            },
            Mode::Ja => match &self.conv {
                Some(view) => {
                    let mut p = Preedit::default();
                    for (i, seg) in view.segments.iter().enumerate() {
                        let len = seg.chars().count();
                        p.segments.push(Segment {
                            start: p.text.chars().count(),
                            len,
                            focused: i == view.focused,
                        });
                        p.text += seg;
                    }
                    p
                }
                None => {
                    let text = self.ja_text(self.kana.reading()) + self.kana.pending().marker();
                    plain(text)
                }
            },
        }
    }

    fn candidates_now(&self) -> Option<Candidates> {
        if let Some(h) = &self.hanja {
            return Some(h.candidates(self.cand_grid));
        }
        let view = self.conv.as_ref()?;
        if view.candidates.is_empty() {
            return None;
        }
        let size = if self.cand_grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
        let page = (view.selected.unwrap_or(0) / size + 1, view.candidates.len().div_ceil(size));
        Some(Candidates {
            items: view.candidates.clone(),
            notes: Vec::new(),
            selected: view.selected,
            page: Some(page),
            grid: self.cand_grid,
        })
    }

    fn finish(&mut self, mut out: Output, before: (Preedit, Option<Candidates>)) -> Output {
        let (preedit, candidates) = self.snapshot();
        // insertText가 조합 중 글자를 대신하므로, 확정이 있었으면 새 preedit를 항상 알려 준다.
        if !out.commit.is_empty() || preedit != before.0 {
            out.preedit = Some(preedit);
        }
        if candidates != before.1 {
            out.candidates = Some(candidates);
        }
        out.learning_changed = std::mem::take(&mut self.learning_dirty);
        out
    }
}

/// 두 결과를 잇는다(잡아 둔 글자를 먼저 친 뒤 지금 이벤트). 조합·후보는 finish가 마지막 상태로 알린다.
fn merge(first: Output, second: Output) -> Output {
    Output {
        consumed: second.consumed,
        commit: first.commit + &second.commit,
        mode: second.mode.or(first.mode),
        caps_lock_off: first.caps_lock_off || second.caps_lock_off,
        timer_ms: second.timer_ms,
        ..second
    }
}

fn millis(secs: f64) -> u32 {
    // 30.05 - 30.02 = 0.030000000000001 같은 부동소수 오차로 1ms를 더 청하지 않게 조금 깎고 올린다.
    // 타이머가 조금 일러도 엔진이 받아 준다(timer의 1ms 여유).
    (secs * 1000.0 - 1e-6).ceil().max(1.0) as u32
}

/// 일본어 모드의 가나 배열 밖 기호(Shift 기호 등). NRIME의 전각/반각 규칙과 같다.
fn ja_symbol(cfg: &JaConfig, ascii: char) -> String {
    // \ 자리는 ¥ 아니면 반각 \ 하나다. 전각 ＼는 쓸 데가 없고, ¥를 끄는 까닭은 경로·코드에 쓸 \다.
    if ascii == '\\' {
        return if cfg.yen_sign { "¥".into() } else { "\\".into() };
    }
    match cfg.punctuation {
        JaPunct::HalfWidthWestern => ascii.to_string(),
        style => match ascii {
            '~' if style == JaPunct::Japanese => "〜".into(),
            '!'..='~' => char::from_u32(ascii as u32 + 0xFEE0).unwrap().to_string(),
            _ => ascii.to_string(),
        },
    }
}

/// 가나 배열 자리의 구두점(、。「」・)을 설정대로 바꾼다.
fn ja_styled(cfg: &JaConfig, s: &str) -> String {
    use JaPunct::*;
    s.chars()
        .map(|c| match (c, cfg.punctuation) {
            ('、', FullWidthWestern) => '，',
            ('、', HalfWidthWestern) => ',',
            ('。', FullWidthWestern) => '．',
            ('。', HalfWidthWestern) => '.',
            ('「', FullWidthWestern) => '［',
            ('「', HalfWidthWestern) => '[',
            ('」', FullWidthWestern) => '］',
            ('」', HalfWidthWestern) => ']',
            ('・', HalfWidthWestern) if !cfg.slash_nakaguro => '/',
            ('・', _) if !cfg.slash_nakaguro => '／',
            _ => c,
        })
        .collect()
}

mod ko_hanja;
#[cfg(test)]
mod tests;
