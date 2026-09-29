//! 설정. 맥과 윈도우가 같은 스키마(config.toml)를 쓴다.
//!
//! NRIME에서 안 쓰던 기능(한/일 직접 전환 단축키, 실시간 변환, 예측 입력, F6~F10 변환 키,
//! 앱별 언어 기억)은 넣지 않는다. 바꿔본 적은 없어도 아쉬울 파라미터(탭 인식 시간 등)는 남긴다.
//!
//! 설정 앱은 파일을 JSON으로 읽고 고친 뒤 [`Config::to_toml`]로 다시 쓴다(`config-ffi`). 파일에는 설정마다
//! 설명을 달고, 기본값과 같은 설정은 주석으로 둔다. 그래야 나중에 기본값이 바뀌면 따라간다.

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::key::{Key, KeyEvent};
use crate::shortcut::{Shortcut, ShortcutAction};

/// 0.4.0까지의 `[taps]` 표(수식키별 탭 동작). 읽기만 하고 `[shortcuts]`로 옮긴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapAction {
    None,
    ToggleEnglish,
    ToggleNonEnglish,
}

/// 일본어 모드 구두점 모양(NRIME와 같은 세 가지).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JaPunct {
    /// 、。「」 (新月 기본)
    Japanese,
    /// ，．［］
    FullWidthWestern,
    /// ,.[]
    HalfWidthWestern,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct JaConfig {
    pub punctuation: JaPunct,
    /// `/` 자리를 ・로 쓸지(끄면 ／ 또는 /).
    pub slash_nakaguro: bool,
    /// 공백을 전각(U+3000)으로 친다. 변환하지 않는 Space(조합 중이 아닐 때, Space 변환을 껐을 때)에 쓴다.
    pub full_width_space: bool,
    /// `\` 키를 ¥로(NRIME 기본과 같다). 끄면 반각 \(경로·코드용. 전각 ＼는 쓸 일이 없어서 내지 않는다).
    pub yen_sign: bool,
    /// 조합 중(가나를 치는 중) Space로 변환한다. 끄면 읽기를 확정하고 공백을 넣는다.
    pub convert_with_space: bool,
    /// 조합 중 Tab으로 변환한다. 끄면 읽기를 확정하고 Tab을 앱에 넘긴다. ↓는 변환 키가 아니다.
    pub convert_with_tab: bool,
    /// 일본어 모드에서 켠 Caps Lock(가타카나)을 다른 모드로 나갈 때 끈다.
    pub caps_katakana_auto_off: bool,
    /// Caps Lock 가타카나는 변환하지 않으므로 바로 확정한다. 뒤치기(゛)가 바꿀 수 있는 마지막 키의 글자만 조합으로 남긴다.
    pub katakana_direct: bool,
}

impl Default for JaConfig {
    fn default() -> Self {
        Self {
            punctuation: JaPunct::Japanese,
            slash_nakaguro: true,
            full_width_space: false,
            yen_sign: true,
            convert_with_space: true,
            convert_with_tab: true,
            caps_katakana_auto_off: true,
            katakana_direct: true,
        }
    }
}

/// 모드 HUD를 띄울 자리.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HudPosition {
    /// 커서 위(커서 자리를 모르면 안 띄운다).
    #[default]
    Caret,
    /// 마우스 옆(늘 뜬다).
    Mouse,
}

/// 맥 셸 설정. 엔진은 쓰지 않고, 셸이 `cssgsg_engine_mac_settings`로 읽는다(설정 파일을 한 곳에 두려고).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MacConfig {
    /// 모드를 바꿀 때 커서 근처에 G / ㅊ / 月을 잠깐 보인다.
    pub hud: bool,
    pub hud_position: HudPosition,
    /// 후보창 글자 크기(포인트).
    pub candidate_font_size: u32,
    /// 조합 중 Shift+Enter 줄바꿈 대기를 앱 종류별 기본값에서 이만큼(밀리초, -50~50) 늘이거나 줄인다.
    /// 기본값(NRIME 실험값): Electron 앱은 확정한 뒤 15ms 기다렸다 줄바꿈을 넣고(⌘/Option+키 재전송도 같다),
    /// Codex처럼 줄바꿈 입력을 전송으로 받는 앱은 120ms 기다렸다 Shift+Enter 키를 다시 보낸다. 둘은 따로 쓰인다(더하지 않는다).
    pub newline_delay_offset_ms: i32,
    /// 0.5.0의 `shift_enter_delay_ms`·`newline_replay_ms`(절댓값). 읽으면 `newline_delay_offset_ms`로 옮긴다.
    #[serde(skip_serializing)]
    pub shift_enter_delay_ms: Option<u32>,
    #[serde(skip_serializing)]
    pub newline_replay_ms: Option<u32>,
}

impl MacConfig {
    /// Electron 앱의 기본 줄바꿈 대기(밀리초).
    pub const SHIFT_ENTER_BASE_MS: i32 = 15;
    /// Codex류 앱의 기본 Shift+Enter 재전송 대기(밀리초).
    pub const NEWLINE_REPLAY_BASE_MS: i32 = 120;

    /// Electron 앱: 확정한 뒤 줄바꿈을 넣기까지(⌘/Option+키 재전송도) 기다리는 시간. 5ms보다 짧게는 하지 않는다.
    pub fn shift_enter_delay_ms(&self) -> u32 {
        (Self::SHIFT_ENTER_BASE_MS + self.newline_delay_offset_ms).max(5) as u32
    }

    /// Codex류 앱: 확정한 뒤 Shift+Enter 키를 다시 보내기까지 기다리는 시간.
    pub fn newline_replay_ms(&self) -> u32 {
        (Self::NEWLINE_REPLAY_BASE_MS + self.newline_delay_offset_ms).max(5) as u32
    }
}

impl Default for MacConfig {
    fn default() -> Self {
        Self {
            hud: true,
            hud_position: HudPosition::Caret,
            candidate_font_size: 14,
            newline_delay_offset_ms: 0,
            shift_enter_delay_ms: None,
            newline_replay_ms: None,
        }
    }
}

/// 단축키 셋. 한국어·일본어로 바로 가는 단축키는 두지 않는다(사용자 결정, NRIME에는 있었다).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Shortcuts {
    /// 영어 ↔ 방금 쓰던 비영어.
    pub toggle_english: Shortcut,
    /// 한국어 ↔ 일본어.
    pub toggle_non_english: Shortcut,
    /// 한국어 한자 변환.
    pub hanja: Shortcut,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle_english: Shortcut::Tap(Key::SHIFT_RIGHT),
            toggle_non_english: Shortcut::Tap(Key::SHIFT_LEFT),
            hanja: Shortcut::parse("alt_left+enter").expect("기본 한자 단축키"),
        }
    }
}

impl Shortcuts {
    fn all(&self) -> [(ShortcutAction, Shortcut); 3] {
        [
            (ShortcutAction::ToggleEnglish, self.toggle_english),
            (ShortcutAction::ToggleNonEnglish, self.toggle_non_english),
            (ShortcutAction::Hanja, self.hanja),
        ]
    }

    /// 설정 파일의 이름.
    fn field_name(action: ShortcutAction) -> &'static str {
        match action {
            ShortcutAction::ToggleEnglish => "toggle_english",
            ShortcutAction::ToggleNonEnglish => "toggle_non_english",
            ShortcutAction::Hanja => "hanja",
        }
    }

    /// 이 수식키 탭에 붙은 일.
    pub fn for_tap(&self, key: Key) -> Option<ShortcutAction> {
        self.all().into_iter().find(|&(_, s)| s == Shortcut::Tap(key)).map(|(a, _)| a)
    }

    /// 이 키 눌림에 붙은 조합 단축키.
    pub fn for_combo(&self, ev: &KeyEvent) -> Option<ShortcutAction> {
        self.all().into_iter().find(|&(_, s)| s.matches_combo(ev)).map(|(a, _)| a)
    }

    /// 조합 단축키 중 이 수식키와 같은 종류(좌우 어느 쪽이든)를 쓰는 것이 있는지
    /// (빠른 탭 전환 보정이 그 수식키를 가로채지 않게).
    pub fn combo_uses_family_of(&self, modifier: Key) -> bool {
        self.all()
            .into_iter()
            .any(|(_, s)| matches!(s, Shortcut::Combo(mods, _) if mods.uses_family_of(modifier)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 한국어 배열: "chamshin-v18"(기본) 또는 "chamshin-d-v19".
    pub ko_layout: String,
    /// 수식키 탭 인식 시간(밀리초). NRIME 기본값과 같은 200.
    pub tap_threshold_ms: u32,
    /// 빠른 탭 전환 보정(실험적, NRIME와 같다): 탭할 수식키를 떼기 전에 다음 글자를 눌러도, 곧 떼면 전환한 뒤의 글자로 친다.
    pub tap_buffering: bool,
    /// 빠른 탭 전환 보정에서 글자를 누르고 수식키를 떼기까지 이 시간(밀리초, 30~80) 안이면 탭으로 본다.
    pub tap_overlap_ms: u32,
    /// Caps Lock과 Shift가 서로 뒤집는지(윈도우 방식). 맥은 false.
    pub caps_shift_inverts: bool,
    pub shortcuts: Shortcuts,
    /// 0.4.0까지의 수식키 탭 표. 읽으면 `shortcuts`로 옮기고 다시 쓰지 않는다.
    #[serde(skip_serializing)]
    pub taps: Option<HashMap<String, TapAction>>,
    pub ja: JaConfig,
    pub mac: MacConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ko_layout: "chamshin-v18".into(),
            tap_threshold_ms: 200,
            tap_buffering: false,
            tap_overlap_ms: 50,
            caps_shift_inverts: false,
            shortcuts: Shortcuts::default(),
            taps: None,
            ja: JaConfig::default(),
            mac: MacConfig::default(),
        }
    }
}

impl Config {
    pub fn from_toml(src: &str) -> Result<Self, String> {
        let table: toml::Table = src.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let has_shortcuts = table.contains_key("shortcuts");
        let has_offset = table
            .get("mac")
            .and_then(toml::Value::as_table)
            .is_some_and(|mac| mac.contains_key("newline_delay_offset_ms"));
        let mut c: Config =
            toml::Value::Table(table).try_into().map_err(|e: toml::de::Error| e.to_string())?;
        if let Some(taps) = c.taps.take()
            && !has_shortcuts
        {
            c.shortcuts = shortcuts_from_legacy_taps(&taps)?;
        }
        // 0.5.0의 절댓값 두 개 → 기본값과의 차이. Codex 값이 있으면 그것을, 없으면 Electron 값을 따른다.
        let (shift_enter, replay) = (c.mac.shift_enter_delay_ms.take(), c.mac.newline_replay_ms.take());
        if !has_offset {
            let offset = match (replay, shift_enter) {
                (Some(ms), _) => ms as i32 - MacConfig::NEWLINE_REPLAY_BASE_MS,
                (None, Some(ms)) => ms as i32 - MacConfig::SHIFT_ENTER_BASE_MS,
                (None, None) => 0,
            };
            c.mac.newline_delay_offset_ms = offset.clamp(-50, 50);
        }
        c.validate()?;
        Ok(c)
    }

    /// 값이 맞는지 본다. 파일을 읽을 때와 설정 앱이 쓸 때 같은 검사를 한다.
    pub fn validate(&self) -> Result<(), String> {
        if crate::hangul::KoLayout::BUILTIN.iter().all(|(id, _)| *id != self.ko_layout) {
            return Err(format!("알 수 없는 한국어 배열 {:?}", self.ko_layout));
        }
        let ranges = [
            ("tap_threshold_ms", self.tap_threshold_ms, 50, 1000),
            ("tap_overlap_ms", self.tap_overlap_ms, 30, 80),
            ("mac.candidate_font_size", self.mac.candidate_font_size, 10, 28),
        ];
        for (name, value, lo, hi) in ranges {
            if !(lo..=hi).contains(&value) {
                return Err(format!("{name}는 {lo}~{hi}이어야 한다: {value}"));
            }
        }
        let offset = self.mac.newline_delay_offset_ms;
        if !(-50..=50).contains(&offset) {
            return Err(format!("mac.newline_delay_offset_ms는 -50~50이어야 한다: {offset}"));
        }
        let all = self.shortcuts.all();
        for (i, &(action, s)) in all.iter().enumerate() {
            let name = Shortcuts::field_name(action);
            s.validate().map_err(|e| format!("shortcuts.{name}: {e}"))?;
            if let Some(&(other, _)) = all[..i].iter().find(|&&(_, t)| s != Shortcut::None && t == s) {
                let other = Shortcuts::field_name(other);
                return Err(format!("shortcuts.{other}와 shortcuts.{name}에 같은 단축키를 썼다: {s}"));
            }
        }
        Ok(())
    }

    /// 설정 파일 내용. 설정마다 설명을 달고, 기본값과 같은 설정은 주석(`# `)으로 둔다.
    /// 읽으면 같은 설정이 된다(`from_toml(to_toml(c)) == c`, 테스트가 확인한다).
    pub fn to_toml(&self) -> String {
        let d = Config::default();
        let mut out = String::new();
        let mut line = |text: &str| {
            out.push_str(text);
            out.push('\n');
        };
        line("# cssgsg 설정. 설정 앱(메뉴 막대 cssgsg → 설정…)에서 바꾸면 바로 적용된다.");
        line("# 직접 고쳐도 된다. 고친 뒤 설정 앱을 열거나 메뉴의 \"cssgsg 다시 시작\"을 누르면 적용된다.");
        line(
            "# 앞에 #이 붙은 설정은 기본값이다(기본값이 바뀌면 따라간다). #을 지우고 값을 고치면 그 값을 쓴다.",
        );
        line("");
        line("# 한국어 배열: \"chamshin-v18\"(기본형) 또는 \"chamshin-d-v19\"(D)");
        line(&setting("ko_layout", &quoted(&self.ko_layout), self.ko_layout == d.ko_layout));
        line("");
        line("# 수식키를 혼자 짧게 누를 때 이 시간(밀리초, 50~1000)보다 빨리 떼야 탭이다");
        line(&setting(
            "tap_threshold_ms",
            &self.tap_threshold_ms.to_string(),
            self.tap_threshold_ms == d.tap_threshold_ms,
        ));
        line(
            "# 빠른 탭 전환 보정(실험적): 탭할 수식키를 떼기 전에 다음 글자를 눌러도, 곧 떼면 전환한 뒤의 글자로 친다",
        );
        line(&setting(
            "tap_buffering",
            &self.tap_buffering.to_string(),
            self.tap_buffering == d.tap_buffering,
        ));
        line("# 빠른 탭 전환 보정: 글자를 누르고 수식키를 떼기까지 이 시간(밀리초, 30~80) 안이면 탭이다");
        line(&setting(
            "tap_overlap_ms",
            &self.tap_overlap_ms.to_string(),
            self.tap_overlap_ms == d.tap_overlap_ms,
        ));
        line("");
        line("# Caps Lock과 Shift가 서로 뒤집는지(윈도우 방식, 맥은 false)");
        line(&setting(
            "caps_shift_inverts",
            &self.caps_shift_inverts.to_string(),
            self.caps_shift_inverts == d.caps_shift_inverts,
        ));
        line("");
        line(
            "# 단축키: \"tap:수식키\"는 수식키를 혼자 짧게 누르기, \"수식키+키\"는 같이 누르기, \"\"는 없음.",
        );
        line(
            "# 수식키: shift_left, shift_right, control_left, control_right, alt_left, alt_right, meta_left, meta_right",
        );
        line(
            "# 같이 누를 때 좌우를 가리지 않으려면 control, alt, shift. ⌘(meta) 조합은 쓸 수 없다. 키: a~z, 0~9, space, enter, tab, f1~f20, left …",
        );
        line("[shortcuts]");
        let (s, ds) = (&self.shortcuts, &d.shortcuts);
        line(&setting(
            "toggle_english",
            &quoted(&s.toggle_english.to_string()),
            s.toggle_english == ds.toggle_english,
        ));
        line(&setting(
            "toggle_non_english",
            &quoted(&s.toggle_non_english.to_string()),
            s.toggle_non_english == ds.toggle_non_english,
        ));
        line(&setting("hanja", &quoted(&s.hanja.to_string()), s.hanja == ds.hanja));
        line("");
        let (ja, dja) = (&self.ja, &d.ja);
        line("[ja]");
        line(
            "# 구두점: \"japanese\"(、。「」), \"full_width_western\"(，．［］), \"half_width_western\"(,.[])",
        );
        line(&setting(
            "punctuation",
            &quoted(ja_punct_name(ja.punctuation)),
            ja.punctuation == dja.punctuation,
        ));
        line("# / 자리를 ・로");
        line(&setting(
            "slash_nakaguro",
            &ja.slash_nakaguro.to_string(),
            ja.slash_nakaguro == dja.slash_nakaguro,
        ));
        line("# \\ 키를 ¥로. 끄면 반각 \\");
        line(&setting("yen_sign", &ja.yen_sign.to_string(), ja.yen_sign == dja.yen_sign));
        line("# 공백을 전각(U+3000)으로 친다. false면 반각. 조합 중 Space는 변환이다(convert_with_space)");
        line(&setting(
            "full_width_space",
            &ja.full_width_space.to_string(),
            ja.full_width_space == dja.full_width_space,
        ));
        line("# 조합 중 Space로 변환한다. 끄면 읽기를 확정하고 공백을 넣는다");
        line(&setting(
            "convert_with_space",
            &ja.convert_with_space.to_string(),
            ja.convert_with_space == dja.convert_with_space,
        ));
        line("# 조합 중 Tab으로 변환한다. 끄면 읽기를 확정하고 Tab을 앱에 넘긴다");
        line(&setting(
            "convert_with_tab",
            &ja.convert_with_tab.to_string(),
            ja.convert_with_tab == dja.convert_with_tab,
        ));
        line("# 일본어 모드에서 켠 Caps Lock(가타카나)을 다른 모드로 나갈 때 끈다");
        line(&setting(
            "caps_katakana_auto_off",
            &ja.caps_katakana_auto_off.to_string(),
            ja.caps_katakana_auto_off == dja.caps_katakana_auto_off,
        ));
        line(
            "# Caps Lock 가타카나는 치는 대로 바로 확정한다(마지막 글자만 잠깐 조합). false면 히라가나처럼 조합으로 들고 있다",
        );
        line(&setting(
            "katakana_direct",
            &ja.katakana_direct.to_string(),
            ja.katakana_direct == dja.katakana_direct,
        ));
        line("");
        let (mac, dmac) = (&self.mac, &d.mac);
        line("[mac]");
        line("# 모드를 바꿀 때 커서 근처에 G/ㅊ/月을 잠깐 보인다");
        line(&setting("hud", &mac.hud.to_string(), mac.hud == dmac.hud));
        line("# HUD 자리: \"caret\"(커서 위, 커서 자리를 모르면 안 보임) 또는 \"mouse\"(마우스 옆)");
        line(&setting(
            "hud_position",
            &quoted(hud_position_name(mac.hud_position)),
            mac.hud_position == dmac.hud_position,
        ));
        line("# 후보창 글자 크기(포인트, 10~28)");
        line(&setting(
            "candidate_font_size",
            &mac.candidate_font_size.to_string(),
            mac.candidate_font_size == dmac.candidate_font_size,
        ));
        line(
            "# 조합 중 Shift+Enter 줄바꿈 대기 조정(밀리초, -50~50). 기본값: Electron 앱 15, Codex류 앱 120(따로 쓰인다)",
        );
        line(&setting(
            "newline_delay_offset_ms",
            &mac.newline_delay_offset_ms.to_string(),
            mac.newline_delay_offset_ms == dmac.newline_delay_offset_ms,
        ));
        out
    }
}

/// 0.4.0까지의 `[taps]` 표를 단축키로 옮긴다. 그 표는 기본값을 통째로 대신했으므로, 표에 없는 전환은 없음이다.
/// 한자 단축키는 그대로(Option+Return) 둔다.
fn shortcuts_from_legacy_taps(taps: &HashMap<String, TapAction>) -> Result<Shortcuts, String> {
    let mut s = Shortcuts {
        toggle_english: Shortcut::None,
        toggle_non_english: Shortcut::None,
        ..Shortcuts::default()
    };
    for name in crate::shortcut::MODIFIER_NAMES {
        let Some(&action) = taps.get(name) else { continue };
        let key = crate::shortcut::modifier_from_name(name).expect("수식키 이름");
        match action {
            TapAction::ToggleEnglish if s.toggle_english == Shortcut::None => {
                s.toggle_english = Shortcut::Tap(key)
            }
            TapAction::ToggleNonEnglish if s.toggle_non_english == Shortcut::None => {
                s.toggle_non_english = Shortcut::Tap(key)
            }
            _ => {}
        }
    }
    if let Some(name) = taps.keys().find(|n| crate::shortcut::modifier_from_name(n).is_none()) {
        return Err(format!("알 수 없는 탭 키 이름 {name:?}"));
    }
    Ok(s)
}

/// `key = value` 한 줄. 기본값이면 주석으로.
fn setting(key: &str, value: &str, is_default: bool) -> String {
    let mut s = String::new();
    if is_default {
        s.push_str("# ");
    }
    let _ = write!(s, "{key} = {value}");
    s
}

/// TOML 기본 문자열(설정 값은 따옴표·역슬래시가 없는 이름뿐이지만 그래도 막는다).
fn quoted(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

fn ja_punct_name(p: JaPunct) -> &'static str {
    match p {
        JaPunct::Japanese => "japanese",
        JaPunct::FullWidthWestern => "full_width_western",
        JaPunct::HalfWidthWestern => "half_width_western",
    }
}

fn hud_position_name(p: HudPosition) -> &'static str {
    match p {
        HudPosition::Caret => "caret",
        HudPosition::Mouse => "mouse",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Mods;

    #[test]
    fn defaults_match_nrime_usage() {
        let c = Config::default();
        assert_eq!(c.shortcuts.for_tap(Key::SHIFT_RIGHT), Some(ShortcutAction::ToggleEnglish));
        assert_eq!(c.shortcuts.for_tap(Key::SHIFT_LEFT), Some(ShortcutAction::ToggleNonEnglish));
        assert_eq!(c.shortcuts.for_tap(Key::META_LEFT), None);
        // 한자는 NRIME처럼 왼쪽 Option+Return(좌우를 가린다).
        let option_return = |side| KeyEvent::down(Key::ENTER, Mods(side), 1.0);
        assert_eq!(c.shortcuts.for_combo(&option_return(Mods::ALT_L)), Some(ShortcutAction::Hanja));
        assert_eq!(c.shortcuts.for_combo(&option_return(Mods::ALT_R)), None);
        assert_eq!(c.tap_threshold_ms, 200);
        assert!(!c.tap_buffering && c.ja.yen_sign && c.ja.convert_with_space && c.ja.convert_with_tab);
        assert_eq!(
            (c.mac.shift_enter_delay_ms(), c.mac.newline_replay_ms(), c.mac.candidate_font_size),
            (15, 120, 14)
        );
    }

    #[test]
    fn parses_partial_toml() {
        let c = Config::from_toml(
            "ko_layout = \"chamshin-d-v19\"\n[ja]\npunctuation = \"full_width_western\"\nfull_width_space = true\n",
        )
        .unwrap();
        assert_eq!(c.ko_layout, "chamshin-d-v19");
        assert_eq!(c.ja.punctuation, JaPunct::FullWidthWestern);
        assert!(c.ja.full_width_space && c.ja.slash_nakaguro);
        assert!(Config::from_toml("ko_layout = \"dubeolsik\"").is_err());
        assert!(Config::from_toml("[taps]\nshift_middle = \"none\"").is_err());
        assert!(Config::from_toml("unknown_field = 1").is_err());
        assert!(Config::from_toml("tap_threshold_ms = 20").is_err());
        assert!(Config::from_toml("[shortcuts]\nhanja = \"a\"").is_err(), "수식키 없는 글자 키");
        let err = Config::from_toml("[shortcuts]\ntoggle_english = \"tap:shift_left\"").unwrap_err();
        assert!(
            err.contains("shortcuts.toggle_english") && err.contains("shortcuts.toggle_non_english"),
            "기본값(한↔일 왼쪽 Shift)과 겹친다. 어느 설정인지 말한다: {err}"
        );
        let err = Config::from_toml("[shortcuts]\nhanja = \"a\"").unwrap_err();
        assert!(err.starts_with("shortcuts.hanja: "), "{err}");
    }

    #[test]
    fn legacy_taps_table_becomes_shortcuts() {
        // 0.4.0 설정 앱이 쓴 [taps]: 적힌 것만 전환이고 나머지는 없음. 한자는 기본값.
        let c = Config::from_toml("[taps]\nalt_right = \"toggle_english\"\n").unwrap();
        assert_eq!(c.shortcuts.toggle_english, Shortcut::Tap(Key::ALT_RIGHT));
        assert_eq!(c.shortcuts.toggle_non_english, Shortcut::None);
        assert_eq!(c.shortcuts.hanja, Shortcuts::default().hanja);
        assert_eq!(c.taps, None, "옮긴 뒤에는 들고 있지 않는다");
        // [shortcuts]가 있으면 그쪽이 이긴다.
        let c = Config::from_toml("[taps]\nalt_right = \"toggle_english\"\n[shortcuts]\n").unwrap();
        assert_eq!(c.shortcuts, Shortcuts::default());
        // 옛 틀(0.2.x, 모두 주석)과 0.4.0 기본 파일도 기본값이다.
        assert_eq!(Config::from_toml("# [taps]\n# [ja]\n").unwrap(), Config::default());
        assert!(!Config::default().to_toml().contains("[taps]"));
    }

    #[test]
    fn newline_delay_is_one_adjustment_of_two_defaults() {
        let mac = |offset| MacConfig { newline_delay_offset_ms: offset, ..MacConfig::default() };
        assert_eq!((mac(0).shift_enter_delay_ms(), mac(0).newline_replay_ms()), (15, 120));
        assert_eq!((mac(20).shift_enter_delay_ms(), mac(20).newline_replay_ms()), (35, 140));
        assert_eq!(
            (mac(-50).shift_enter_delay_ms(), mac(-50).newline_replay_ms()),
            (5, 70),
            "5ms 밑으로는 안 간다"
        );
        assert!(Config::from_toml("[mac]\nnewline_delay_offset_ms = 60").is_err());
        assert!(Config::from_toml("[mac]\nnewline_delay_offset_ms = -51").is_err());
        // 0.5.0 파일의 절댓값은 차이로 옮긴다(Codex 값이 먼저, 범위는 ±50).
        let offset = |toml: &str| Config::from_toml(toml).unwrap().mac.newline_delay_offset_ms;
        assert_eq!(offset("[mac]\nnewline_replay_ms = 150"), 30);
        assert_eq!(offset("[mac]\nshift_enter_delay_ms = 25"), 10);
        assert_eq!(offset("[mac]\nshift_enter_delay_ms = 25\nnewline_replay_ms = 100"), -20);
        assert_eq!(offset("[mac]\nnewline_replay_ms = 500"), 50);
        assert_eq!(
            offset("[mac]\nnewline_replay_ms = 150\nnewline_delay_offset_ms = -5"),
            -5,
            "새 값이 있으면 그것"
        );
        let c = Config::from_toml("[mac]\nnewline_replay_ms = 150").unwrap();
        assert_eq!(
            (c.mac.shift_enter_delay_ms, c.mac.newline_replay_ms),
            (None, None),
            "옮긴 뒤에는 들고 있지 않는다"
        );
        assert!(!c.to_toml().contains("newline_replay_ms ="), "다시 쓰지 않는다");
    }

    /// 설정 파일로 쓴 것을 다시 읽으면 같은 설정이다.
    fn round_trip(c: &Config) {
        let text = c.to_toml();
        let back = Config::from_toml(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(&back, c, "\n{text}");
    }

    #[test]
    fn default_file_has_every_setting_commented() {
        let text = Config::default().to_toml();
        round_trip(&Config::default());
        // 표 머리([shortcuts] [ja] [mac])와 설명 말고는 모두 주석: 기본값이 바뀌면 따라간다.
        let live: Vec<&str> =
            text.lines().filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('[')).collect();
        assert!(live.is_empty(), "{live:?}");
        for key in [
            "ko_layout",
            "tap_threshold_ms",
            "tap_buffering",
            "toggle_english",
            "hanja",
            "punctuation",
            "yen_sign",
            "convert_with_tab",
            "katakana_direct",
            "candidate_font_size",
            "newline_delay_offset_ms",
        ] {
            assert!(text.contains(&format!("# {key} = ")), "{key}");
        }
        assert!(text.contains("# hanja = \"alt_left+enter\""));
    }

    #[test]
    fn changed_settings_are_written_and_read_back() {
        let c = Config {
            ko_layout: "chamshin-d-v19".into(),
            tap_threshold_ms: 250,
            tap_buffering: true,
            tap_overlap_ms: 60,
            shortcuts: Shortcuts {
                toggle_english: Shortcut::parse("control_left+space").unwrap(),
                toggle_non_english: Shortcut::None,
                hanja: Shortcut::Tap(Key::ALT_RIGHT),
            },
            ja: JaConfig {
                punctuation: JaPunct::HalfWidthWestern,
                yen_sign: false,
                convert_with_tab: false,
                katakana_direct: false,
                ..JaConfig::default()
            },
            mac: MacConfig {
                hud: false,
                hud_position: HudPosition::Mouse,
                candidate_font_size: 18,
                newline_delay_offset_ms: -20,
                ..MacConfig::default()
            },
            ..Config::default()
        };
        round_trip(&c);
        let text = c.to_toml();
        assert!(text.contains("\nko_layout = \"chamshin-d-v19\"\n"));
        assert!(text.contains("\ntoggle_english = \"control_left+space\"\n"));
        assert!(text.contains("\nnewline_delay_offset_ms = -20\n"));
        assert!(text.contains("\ntoggle_non_english = \"\"\n"), "없음은 빈 글자열");
        assert!(text.contains("\nhanja = \"tap:alt_right\"\n"));
        assert!(text.contains("\n# slash_nakaguro = true\n"), "바꾸지 않은 것은 주석 그대로");
    }
}
