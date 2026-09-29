//! 엔진: 모드 전환, 키 라우팅, 셸이 할 일([`Output`]).
//!
//! 셸 규칙(NRIME 교훈):
//! - `commit`이 있으면 `insertText(commit)`만 부른다. insertText가 조합 중 글자를 대신하므로
//!   그 앞뒤에 `setMarkedText("")`를 부르지 않는다(Chromium·JS 에디터에서 글자가 사라진다).
//! - 그다음 `preedit`가 Some이고 비어 있지 않으면 `setMarkedText(preedit)`.
//!   `commit` 없이 `preedit`가 Some(빈 값)이면 조합 중 글자를 지운다.
//! - `consumed`가 false면 원래 키를 앱에 넘긴다(맥: handle에서 false 반환).

use crate::config::{Config, JaConfig, JaPunct, TapAction};
use crate::convert::{ConvCmd, ConvView, Converter, EchoConverter};
use crate::hangul::{KoComposer, KoLayout, KoResult};
use crate::hotkey::TapTracker;
use crate::kana::{KanaComposer, KanaLayout, to_katakana};
use crate::key::{Key, KeyEvent};
use crate::latin::{LatinLayout, qwerty_char_for};

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
    pub items: Vec<String>,
    pub selected: Option<usize>,
    pub page: Option<(usize, usize)>,
}

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
    converter: Box<dyn Converter>,
    latin: LatinLayout,
    taps: TapTracker,
    /// 마지막으로 본 Caps Lock 상태. 일본어 모드에서는 가타카나 입력이다.
    caps: bool,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let ko_layout = KoLayout::builtin(&config.ko_layout)
            .unwrap_or_else(|| KoLayout::builtin("chamshin-v18").unwrap());
        Self {
            config,
            mode: Mode::En,
            last_non_en: Mode::Ko,
            ko_layout,
            ko: KoComposer::new(),
            kana_layout: KanaLayout::shingetsu(),
            kana: KanaComposer::new(),
            conv: None,
            converter: Box::new(EchoConverter::default()),
            latin: LatinLayout::graphite(),
            taps: TapTracker::default(),
            caps: false,
        }
    }

    pub fn set_converter(&mut self, converter: Box<dyn Converter>) {
        if self.conv.take().is_some() {
            self.converter.cancel();
        }
        self.converter = converter;
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// 키 이벤트 하나를 처리한다.
    pub fn handle_key(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        let before = self.snapshot();
        let out = self.route(ev, ctx);
        self.finish(out, before)
    }

    /// 조합 중인 것을 모두 확정한다(마우스 클릭, 포커스 해제).
    pub fn commit_all(&mut self) -> Output {
        let before = self.snapshot();
        let out = Output::commit_eat(self.take_composition());
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
        self.taps.cancel();
        self.finish(Output::eat(), before)
    }

    /// 모드를 바로 바꾼다(메뉴 등). 조합 중인 것은 확정한다.
    pub fn set_mode(&mut self, mode: Mode) -> Output {
        let before = self.snapshot();
        let out = self.switch_to(mode);
        self.finish(out, before)
    }

    fn route(&mut self, ev: &KeyEvent, ctx: &Context) -> Output {
        self.caps = ev.mods.caps();

        let threshold = self.config.tap_threshold_ms as f64 / 1000.0;
        if let Some(tapped) = self.taps.observe(ev, threshold) {
            if !ctx.taps_disabled {
                match self.config.tap_action(tapped) {
                    TapAction::ToggleEnglish => return self.toggle_english(),
                    TapAction::ToggleNonEnglish => return self.toggle_non_english(),
                    TapAction::None => {}
                }
            }
        }

        if !ev.down || ev.key.is_modifier() || ev.key == Key::CAPS_LOCK {
            return Output::pass();
        }

        // ⌘/Ctrl/Option 조합: 조합을 확정하고 앱에 넘긴다(단축키는 쿼티 자리 그대로).
        if ev.mods.command_like() {
            return Output::commit_pass(self.take_composition());
        }

        match self.mode {
            Mode::En => self.en_key(ev, ctx),
            Mode::Ko => self.ko_key(ev),
            Mode::Ja => self.ja_key(ev),
        }
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

    /// 조합 중인 것(한글 음절, 가나 읽기, 변환 결과)을 확정할 글자로 꺼내고 비운다.
    fn take_composition(&mut self) -> String {
        let mut text = self.ko.flush();
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
            Key::SPACE | Key::TAB | Key::ARROW_DOWN if !self.kana.reading().is_empty() => {
                self.kana.cancel_pending();
                if katakana || (k == Key::TAB && shift) {
                    // 가타카나 입력은 변환하지 않는다: 확정하고 키는 앱으로.
                    return Output::commit_pass(self.take_composition());
                }
                let reading = self.ja_styled(self.kana.reading());
                if let Some(view) = self.converter.start(&reading) {
                    self.conv = Some(view);
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
                // Enter는 확정만 한다(줄바꿈 없음).
                return Output::commit_eat(self.take_composition());
            }
            _ => {}
        }

        if k.is_printable() {
            // 글자 키는 Shift를 무시하고 가나. 그 밖의 키(숫자·기호 자리)는 Shift를 따른다.
            if (k.is_letter() || !shift) && self.kana.key(&self.kana_layout, k) {
                return Output::eat();
            }
            if k.is_digit() && !shift {
                // 숫자는 읽기에 넣는다(3じ → 3時 변환).
                let d = k.qwerty_char(false).unwrap();
                self.kana.push_str(&d.to_string());
                return Output::eat();
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

    fn ja_converting_key(&mut self, ev: &KeyEvent) -> Output {
        let (k, shift) = (ev.key, ev.mods.shift());
        let cmd = match k {
            Key::SPACE | Key::ARROW_DOWN => Some(ConvCmd::Next),
            Key::TAB => Some(if shift { ConvCmd::Prev } else { ConvCmd::Next }),
            Key::ARROW_UP => Some(ConvCmd::Prev),
            Key::ARROW_LEFT => Some(if shift { ConvCmd::Shrink } else { ConvCmd::FocusLeft }),
            Key::ARROW_RIGHT => Some(if shift { ConvCmd::Expand } else { ConvCmd::FocusRight }),
            Key::PAGE_DOWN => Some(ConvCmd::NextPage),
            Key::PAGE_UP => Some(ConvCmd::PrevPage),
            _ => None,
        };
        if let Some(cmd) = cmd {
            if let Some(view) = self.converter.command(cmd) {
                self.conv = Some(view);
            }
            return Output::eat();
        }
        match k {
            Key::ENTER | Key::NUMPAD_ENTER => return Output::commit_eat(self.take_composition()),
            Key::ESCAPE | Key::BACKSPACE => {
                // 변환 취소: 읽기로 돌아간다.
                self.converter.cancel();
                self.conv = None;
                return Output::eat();
            }
            _ => {}
        }
        if k.is_digit() && !shift && k != Key::DIGIT0 {
            let n = (k.0 - Key::DIGIT1.0) as usize;
            let on_page = self.conv.as_ref().map_or(0, |v| v.candidates.len());
            if n < on_page {
                // 번호로 고르면 바로 확정한다(NRIME와 같다).
                if let Some(view) = self.converter.command(ConvCmd::SelectOnPage(n)) {
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
            Mode::Ko => plain(self.ko.preedit()),
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
        let view = self.conv.as_ref()?;
        if view.candidates.is_empty() {
            return None;
        }
        Some(Candidates { items: view.candidates.clone(), selected: view.selected, page: view.page })
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
        out
    }
}

/// 일본어 모드의 가나 배열 밖 기호(Shift 기호 등). NRIME의 전각/반각 규칙과 같다.
fn ja_symbol(cfg: &JaConfig, ascii: char) -> String {
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

#[cfg(test)]
mod tests;
