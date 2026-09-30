//! 시뮬레이터: 키열을 엔진에 넣고, 앱 화면에 무엇이 찍히는지 흉내 낸다.
//! 테스트, CLI, 오이 차분 비교에서 같이 쓴다.
//!
//! 키열 문법
//! - 보통 글자: 쿼티 자리의 키. 대문자와 Shift 기호(!@# …)는 Shift를 누른 것으로 본다.
//! - `{이름}`: 특수 키. sp bs ent esc tab left right up down pgup pgdn del
//!   rs(오른쪽 Shift 탭) ls(왼쪽 Shift 탭) caps(Caps Lock 토글) click(마우스 클릭)
//! - `{S-이름}`: Shift+특수 키. `{M-x}` ⌘/Win+x, `{C-x}` Ctrl+x, `{A-x}` Option/Alt+x(모두 왼쪽). 한자 키는 `{A-ent}`.
//!   입력기가 실제로 받는 순서대로 수식키 누름 → 키 → 수식키 뗌을 보낸다(Shift 대문자도 같다).
//!   0.5.0은 키 하나만 보내서, ⌘/Control을 누르는 순간의 확정을 시험하지 못했다.
//!
//! 앱 화면은 확정 글자 + 조합 글자이고 커서는 늘 끝이다(화살표는 커서를 옮기지 않는다). 선택은 없다.

use crate::engine::{Candidates, Context, Engine, Mode, Output};
use crate::key::{Key, KeyEvent, Mods};
use crate::latin::qwerty_char_for;

pub struct Sim {
    pub engine: Engine,
    pub ctx: Context,
    /// 앱에 확정된 글자.
    pub text: String,
    /// 조합 중 글자.
    pub preedit: String,
    pub candidates: Option<Candidates>,
    pub caps: bool,
    /// 엔진이 청한 타이머 시각(빠른 탭 전환 보정). `event`로 넣었을 때만 채운다.
    pub pending_timer: Option<f64>,
    /// 마지막으로 넣은 이벤트 뒤에 누르고 있는 수식키(타이머에 준다).
    held: Mods,
    time: f64,
}

impl Sim {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            ctx: Context::default(),
            text: String::new(),
            preedit: String::new(),
            candidates: None,
            caps: false,
            pending_timer: None,
            held: Mods(0),
            time: 1.0,
        }
    }

    /// 모드를 바꿔 놓고 시작한다.
    pub fn with_mode(mut self, mode: Mode) -> Self {
        let out = self.engine.set_mode(mode);
        self.apply(None, &out);
        self
    }

    /// 앱 화면: 확정된 글자 + 조합 중 글자.
    pub fn screen(&self) -> String {
        self.text.clone() + &self.preedit
    }

    /// 키열을 친다. 문법 오류면 Err.
    pub fn type_keys(&mut self, keys: &str) -> Result<(), String> {
        let mut chars = keys.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => name.push(ch),
                        None => return Err(format!("닫는 }}가 없다: {{{name}")),
                    }
                }
                self.special(&name)?;
            } else {
                let (key, shift) = char_key(c).ok_or_else(|| format!("키로 칠 수 없는 글자 {c:?}"))?;
                if shift {
                    self.chord(Key::SHIFT_LEFT, Mods::SHIFT_L, key);
                } else {
                    self.press(key, 0);
                }
            }
        }
        Ok(())
    }

    fn special(&mut self, name: &str) -> Result<(), String> {
        if let Some((prefix, rest)) = name.split_once('-') {
            let (modifier, bit) = match prefix {
                "S" => (Key::SHIFT_LEFT, Mods::SHIFT_L),
                "M" => (Key::META_LEFT, Mods::META_L),
                "C" => (Key::CONTROL_LEFT, Mods::CTRL_L),
                "A" => (Key::ALT_LEFT, Mods::ALT_L),
                _ => return Err(format!("알 수 없는 수식키 {prefix:?}")),
            };
            let key = match named_key(rest) {
                Some(k) => k,
                None => {
                    let mut it = rest.chars();
                    match (it.next(), it.next()) {
                        (Some(c), None) => char_key(c).map(|(k, _)| k).ok_or("알 수 없는 키")?,
                        _ => return Err(format!("알 수 없는 키 {rest:?}")),
                    }
                }
            };
            self.chord(modifier, bit, key);
            return Ok(());
        }
        match name {
            "rs" => self.tap(Key::SHIFT_RIGHT, Mods::SHIFT_R),
            "ls" => self.tap(Key::SHIFT_LEFT, Mods::SHIFT_L),
            "caps" => {
                self.caps = !self.caps;
                self.press(Key::CAPS_LOCK, 0);
            }
            "click" => {
                let out = self.engine.mouse_down();
                self.apply(None, &out);
            }
            _ => {
                let key = named_key(name).ok_or_else(|| format!("알 수 없는 특수 키 {{{name}}}"))?;
                self.press(key, 0);
            }
        }
        Ok(())
    }

    fn mods(&self, bits: u32) -> Mods {
        Mods(bits | if self.caps { Mods::CAPS } else { 0 })
    }

    /// 키 하나를 누르고 뗀다.
    pub fn press(&mut self, key: Key, mod_bits: u32) {
        self.time += 0.03;
        let ev = KeyEvent::down(key, self.mods(mod_bits), self.time);
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
        self.time += 0.03;
        let ev = KeyEvent::up(key, self.mods(mod_bits), self.time);
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
    }

    /// 이벤트 하나를 그대로 넣는다(겹쳐 누르기처럼 키열 문법으로 못 쓰는 시험용). 타이머 요청은 `pending_timer`에 남는다.
    pub fn event(&mut self, ev: KeyEvent) -> Output {
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
        self.pending_timer = out.timer_ms.map(|ms| ev.time + ms as f64 / 1000.0);
        out
    }

    /// 셸처럼 타이머 시각에 엔진을 부른다. 수식키는 마지막으로 넣은 이벤트 그대로 누르고 있다고 본다.
    pub fn fire_timer(&mut self, now: f64) -> Output {
        self.fire_timer_held(now, self.held)
    }

    /// 타이머를 부르며, 그때 실제로 누르고 있는 수식키를 따로 준다(뗌 이벤트가 늦게 오는 경우).
    pub fn fire_timer_held(&mut self, now: f64, held: Mods) -> Output {
        let out = self.engine.timer(now, held);
        self.apply(None, &out);
        self.pending_timer = out.timer_ms.map(|ms| now + ms as f64 / 1000.0);
        out
    }

    /// 수식키를 혼자 짧게 누른다.
    /// 수식키를 누른 채 키 하나를 친다: 수식키 누름, 키 누름·뗌, 수식키 뗌(입력기가 받는 순서).
    pub fn chord(&mut self, modifier: Key, bit: u32, key: Key) {
        self.time += 0.03;
        let ev = KeyEvent::down(modifier, self.mods(bit), self.time);
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
        self.press(key, bit);
        self.time += 0.03;
        let ev = KeyEvent::up(modifier, self.mods(0), self.time);
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
    }

    pub fn tap(&mut self, key: Key, bit: u32) {
        self.time += 0.03;
        let ev = KeyEvent::down(key, self.mods(bit), self.time);
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
        self.time += 0.05;
        let ev = KeyEvent::up(key, self.mods(0), self.time);
        let out = self.engine.handle_key(&ev, &self.ctx);
        self.apply(Some(&ev), &out);
    }

    fn apply(&mut self, ev: Option<&KeyEvent>, out: &Output) {
        if !out.commit.is_empty() {
            self.text += &out.commit;
            self.preedit.clear();
        }
        if let Some(p) = &out.preedit {
            self.preedit = p.text.clone();
        }
        if let Some(c) = &out.candidates {
            self.candidates = c.clone();
        }
        if out.caps_lock_off {
            self.caps = false;
        }
        let Some(ev) = ev else { return };
        self.held = Mods(ev.mods.0 & !Mods::CAPS);
        if out.consumed || !ev.down || ev.key.is_modifier() || ev.mods.command_like() {
            return;
        }
        // 앱이 원래 키를 처리한다(OS 레이아웃은 쿼티).
        match ev.key {
            Key::BACKSPACE => {
                self.text.pop();
            }
            Key::ENTER | Key::NUMPAD_ENTER => self.text.push('\n'),
            Key::SPACE => self.text.push(' '),
            Key::TAB => self.text.push('\t'),
            k => {
                if let Some(c) = qwerty_char_for(k, ev.mods.shift(), ev.mods.caps(), false) {
                    self.text.push(c);
                }
            }
        }
    }
}

fn named_key(name: &str) -> Option<Key> {
    Some(match name {
        "sp" | "space" => Key::SPACE,
        "bs" => Key::BACKSPACE,
        "ent" | "enter" => Key::ENTER,
        "esc" => Key::ESCAPE,
        "tab" => Key::TAB,
        "left" => Key::ARROW_LEFT,
        "right" => Key::ARROW_RIGHT,
        "up" => Key::ARROW_UP,
        "down" => Key::ARROW_DOWN,
        "pgup" => Key::PAGE_UP,
        "pgdn" => Key::PAGE_DOWN,
        "del" => Key::DELETE,
        _ => return None,
    })
}

/// 글자 → (키, Shift 여부). 대문자와 Shift 기호는 Shift.
pub fn char_key(c: char) -> Option<(Key, bool)> {
    if c == ' ' {
        return Some((Key::SPACE, false));
    }
    if c.is_ascii_uppercase() {
        return Key::from_qwerty(c.to_ascii_lowercase()).map(|k| (k, true));
    }
    if let Some(k) = Key::from_qwerty(c) {
        return Some((k, false));
    }
    (0x04..0x40u16).map(Key).find(|k| k.qwerty_char(true) == Some(c)).map(|k| (k, true))
}
