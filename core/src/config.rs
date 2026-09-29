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
    /// 0.5.x의 줄바꿈 대기(Electron `shift_enter_delay_ms`, Codex `newline_replay_ms`, 0.5.1~0.5.3 `newline_delay_offset_ms`).
    /// 0.6.0부터 쓰지 않는다: 기다리지 않고 다음 런루프 차례에 줄을 바꾼다. 그 값들은 macOS가 권한 없이 보낸 키를
    /// 소리 없이 버리던 때 정한 것이고, 권한이 있으면 기다림 없이 늘 됐다(NRIME 1.0.12, Discord 9/9, Codex 11/11).
    /// 옛 설정 파일이 오류 나지 않게 읽기만 하고 버린다.
    #[serde(skip_serializing)]
    pub shift_enter_delay_ms: Option<i64>,
    #[serde(skip_serializing)]
    pub newline_replay_ms: Option<i64>,
    #[serde(skip_serializing)]
    pub newline_delay_offset_ms: Option<i64>,
}

impl Default for MacConfig {
    fn default() -> Self {
        Self {
            hud: true,
            hud_position: HudPosition::Caret,
            candidate_font_size: 14,
            shift_enter_delay_ms: None,
            newline_replay_ms: None,
            newline_delay_offset_ms: None,
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
        let mut c: Config =
            toml::Value::Table(table).try_into().map_err(|e: toml::de::Error| e.to_string())?;
        if let Some(taps) = c.taps.take()
            && !has_shortcuts
        {
            c.shortcuts = shortcuts_from_legacy_taps(&taps)?;
        }
        // 0.5.x의 줄바꿈 대기는 버린다(MacConfig 설명).
        c.mac.shift_enter_delay_ms = None;
        c.mac.newline_replay_ms = None;
        c.mac.newline_delay_offset_ms = None;
        c.validate()?;
        Ok(c)
    }

    /// 값이 맞는지 본다. 파일을 읽을 때와 설정 앱이 쓸 때 같은 검사를 한다.
    pub fn validate(&self) -> Result<(), String> {
        if crate::hangul::KoLayout::BUILTIN.iter().all(|(id, _)| *id != self.ko_layout) {
            return Err(format!("알 수 없는 한국어 배열입니다: {:?}", self.ko_layout));
        }
        let ranges = [
            ("tap_threshold_ms", self.tap_threshold_ms, 50, 1000),
            ("tap_overlap_ms", self.tap_overlap_ms, 30, 80),
            ("mac.candidate_font_size", self.mac.candidate_font_size, 10, 28),
        ];
        for (name, value, lo, hi) in ranges {
            if !(lo..=hi).contains(&value) {
                return Err(format!("{name}는 {lo}~{hi} 사이여야 합니다(지금 {value})"));
            }
        }
        let all = self.shortcuts.all();
        for (i, &(action, s)) in all.iter().enumerate() {
            let name = Shortcuts::field_name(action);
            s.validate().map_err(|e| format!("shortcuts.{name}: {e}"))?;
            if let Some(&(other, _)) = all[..i].iter().find(|&&(_, t)| s != Shortcut::None && t == s) {
                let other = Shortcuts::field_name(other);
                return Err(format!("shortcuts.{other}와 shortcuts.{name}에 같은 단축키가 있습니다: {s}"));
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
        line("# cssgsg 설정 파일입니다. 설정 앱(메뉴 막대 cssgsg → 설정…)에서 바꾸면 바로 적용됩니다.");
        line(
            "# 직접 고쳐도 됩니다. 고친 뒤 설정 앱을 열거나 메뉴의 \"cssgsg 다시 시작\"을 누르면 적용됩니다.",
        );
        line(
            "# 앞에 #이 붙은 줄은 기본값입니다(기본값이 바뀌면 따라갑니다). #을 지우고 값을 고치면 그 값을 씁니다.",
        );
        line("");
        line(
            "# 한국어 배열: \"chamshin-v18\"(참신세벌식 v18, 기본형) 또는 \"chamshin-d-v19\"(참신세벌식 D v19)",
        );
        line(&setting("ko_layout", &quoted(&self.ko_layout), self.ko_layout == d.ko_layout));
        line("");
        line("# 탭 인식 시간(밀리초, 50~1000): 수식키만 누른 뒤 이 시간 안에 떼야 탭으로 봅니다");
        line(&setting(
            "tap_threshold_ms",
            &self.tap_threshold_ms.to_string(),
            self.tap_threshold_ms == d.tap_threshold_ms,
        ));
        line(
            "# 빠른 탭 전환 보정(실험적, 보류 중이라 설정 앱에는 없음): 탭할 수식키를 떼기 전에 다음 글자를 눌러도 전환한 뒤의 글자로 칩니다",
        );
        line(&setting(
            "tap_buffering",
            &self.tap_buffering.to_string(),
            self.tap_buffering == d.tap_buffering,
        ));
        line("# 빠른 탭 전환 보정의 겹침 허용 시간(밀리초, 30~80)");
        line(&setting(
            "tap_overlap_ms",
            &self.tap_overlap_ms.to_string(),
            self.tap_overlap_ms == d.tap_overlap_ms,
        ));
        line("");
        line("# Caps Lock과 Shift가 서로 뒤집히는지 여부(윈도우 방식, 맥은 false)");
        line(&setting(
            "caps_shift_inverts",
            &self.caps_shift_inverts.to_string(),
            self.caps_shift_inverts == d.caps_shift_inverts,
        ));
        line("");
        line(
            "# 단축키: \"tap:수식키\"는 수식키만 짧게 누르기, \"수식키+키\"는 같이 누르기, \"\"는 없음입니다.",
        );
        line(
            "# 수식키: shift_left, shift_right, control_left, control_right, alt_left, alt_right, meta_left, meta_right",
        );
        line(
            "# 같이 누를 때 좌우를 가리지 않으려면 control, alt, shift를 씁니다. ⌘(meta) 조합은 쓸 수 없습니다. 키: a~z, 0~9, space, enter, tab, f1~f20, left …",
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
        line("# / 키로 ・(나카구로) 입력");
        line(&setting(
            "slash_nakaguro",
            &ja.slash_nakaguro.to_string(),
            ja.slash_nakaguro == dja.slash_nakaguro,
        ));
        line("# \\ 키로 ¥ 입력(false면 반각 \\)");
        line(&setting("yen_sign", &ja.yen_sign.to_string(), ja.yen_sign == dja.yen_sign));
        line("# 공백 너비: true면 전각(U+3000), false면 반각. 입력 중에 누른 Space는 변환입니다");
        line(&setting(
            "full_width_space",
            &ja.full_width_space.to_string(),
            ja.full_width_space == dja.full_width_space,
        ));
        line("# 입력 중 Space로 변환(false면 입력한 가나를 확정하고 공백을 넣습니다)");
        line(&setting(
            "convert_with_space",
            &ja.convert_with_space.to_string(),
            ja.convert_with_space == dja.convert_with_space,
        ));
        line("# 입력 중 Tab으로 변환(false면 입력한 가나를 확정하고 Tab을 앱에 넘깁니다)");
        line(&setting(
            "convert_with_tab",
            &ja.convert_with_tab.to_string(),
            ja.convert_with_tab == dja.convert_with_tab,
        ));
        line("# 일본어 모드를 나가면 Caps Lock(가타카나) 끄기");
        line(&setting(
            "caps_katakana_auto_off",
            &ja.caps_katakana_auto_off.to_string(),
            ja.caps_katakana_auto_off == dja.caps_katakana_auto_off,
        ));
        line(
            "# Caps Lock 가타카나를 치는 대로 바로 확정(마지막 글자만 잠깐 조합으로 남습니다). false면 히라가나처럼 조합으로 둡니다",
        );
        line(&setting(
            "katakana_direct",
            &ja.katakana_direct.to_string(),
            ja.katakana_direct == dja.katakana_direct,
        ));
        line("");
        let (mac, dmac) = (&self.mac, &d.mac);
        line("[mac]");
        line("# 모드를 바꿀 때 커서 근처에 G/ㅊ/月 잠깐 표시");
        line(&setting("hud", &mac.hud.to_string(), mac.hud == dmac.hud));
        line(
            "# 모드 표시 위치: \"caret\"(커서 위, 커서 위치를 알 수 없는 앱에서는 표시하지 않음) 또는 \"mouse\"(마우스 옆)",
        );
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
        return Err(format!("알 수 없는 탭 키 이름입니다: {name:?}"));
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
        assert_eq!(c.mac.candidate_font_size, 14);
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
    fn old_newline_waits_are_read_and_dropped() {
        // 0.5.x 파일의 줄바꿈 대기는 오류 없이 읽고 버린다. 다시 쓰지도 않는다.
        for toml in [
            "[mac]\nshift_enter_delay_ms = 35\nnewline_replay_ms = 150",
            "[mac]\nnewline_delay_offset_ms = -30",
            "[mac]\nshift_enter_delay_ms = 4\nnewline_replay_ms = 5000",
        ] {
            let c = Config::from_toml(toml).unwrap_or_else(|e| panic!("{toml}: {e}"));
            assert_eq!(c, Config::default(), "{toml}");
            assert!(!c.to_toml().contains("delay"), "{toml}");
        }
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
                ..MacConfig::default()
            },
            ..Config::default()
        };
        round_trip(&c);
        let text = c.to_toml();
        assert!(text.contains("\nko_layout = \"chamshin-d-v19\"\n"));
        assert!(text.contains("\ntoggle_english = \"control_left+space\"\n"));
        assert!(text.contains("\ncandidate_font_size = 18\n"));
        assert!(text.contains("\ntoggle_non_english = \"\"\n"), "없음은 빈 글자열");
        assert!(text.contains("\nhanja = \"tap:alt_right\"\n"));
        assert!(text.contains("\n# slash_nakaguro = true\n"), "바꾸지 않은 것은 주석 그대로");
    }
}
