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

use crate::key::Key;

/// 수식키를 혼자 탭했을 때 할 일.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapAction {
    None,
    /// 영어 ↔ 직전 비영어 모드.
    ToggleEnglish,
    /// 한국어 ↔ 일본어.
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
    /// 읽기가 없을 때 Space를 전각 스페이스로.
    pub full_width_space: bool,
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
    /// 모드를 바꿀 때 커서 근처에 A / 한 / あ를 잠깐 보인다.
    pub hud: bool,
    pub hud_position: HudPosition,
    /// Codex처럼 줄바꿈 입력을 전송으로 받는 앱에서 조합 중 Shift+Enter: 확정한 뒤 Shift+Enter를 다시 보내기까지
    /// 기다리는 시간(밀리초). 짧으면 확정이 끝나기 전에 도착해 줄바꿈이 먹힐 수 있다(NRIME 실험값 120).
    pub newline_replay_ms: u32,
}

impl Default for MacConfig {
    fn default() -> Self {
        Self { hud: true, hud_position: HudPosition::Caret, newline_replay_ms: 120 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 한국어 배열: "chamshin-v18"(기본) 또는 "chamshin-d-v19".
    pub ko_layout: String,
    /// 수식키 탭 인식 시간(밀리초). NRIME 기본값과 같은 200.
    pub tap_threshold_ms: u32,
    /// 수식키별 탭 동작. 키 이름: shift_left, shift_right, control_left, control_right,
    /// alt_left, alt_right, meta_left, meta_right.
    pub taps: HashMap<String, TapAction>,
    /// Caps Lock과 Shift가 서로 뒤집는지(윈도우 방식). 맥은 false.
    pub caps_shift_inverts: bool,
    pub ja: JaConfig,
    pub mac: MacConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ko_layout: "chamshin-v18".into(),
            tap_threshold_ms: 200,
            taps: HashMap::from([
                ("shift_right".into(), TapAction::ToggleEnglish),
                ("shift_left".into(), TapAction::ToggleNonEnglish),
            ]),
            caps_shift_inverts: false,
            ja: JaConfig::default(),
            mac: MacConfig::default(),
        }
    }
}

/// 탭 키 이름(설정 파일과 설정 앱이 쓰는 순서).
pub const TAP_KEYS: [&str; 8] = [
    "shift_left",
    "shift_right",
    "control_left",
    "control_right",
    "alt_left",
    "alt_right",
    "meta_left",
    "meta_right",
];

impl Config {
    pub fn from_toml(src: &str) -> Result<Self, String> {
        let c: Config = toml::from_str(src).map_err(|e| e.to_string())?;
        c.validate()?;
        Ok(c)
    }

    /// 값이 맞는지 본다. 파일을 읽을 때와 설정 앱이 쓸 때 같은 검사를 한다.
    pub fn validate(&self) -> Result<(), String> {
        if crate::hangul::KoLayout::BUILTIN.iter().all(|(id, _)| *id != self.ko_layout) {
            return Err(format!("알 수 없는 한국어 배열 {:?}", self.ko_layout));
        }
        if !(50..=1000).contains(&self.tap_threshold_ms) {
            return Err(format!("tap_threshold_ms는 50~1000이어야 한다: {}", self.tap_threshold_ms));
        }
        for name in self.taps.keys() {
            if modifier_key(name).is_none() {
                return Err(format!("알 수 없는 탭 키 이름 {name:?}"));
            }
        }
        if !(20..=1000).contains(&self.mac.newline_replay_ms) {
            return Err(format!(
                "mac.newline_replay_ms는 20~1000이어야 한다: {}",
                self.mac.newline_replay_ms
            ));
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
        line("");
        line("# Caps Lock과 Shift가 서로 뒤집는지(윈도우 방식, 맥은 false)");
        line(&setting(
            "caps_shift_inverts",
            &self.caps_shift_inverts.to_string(),
            self.caps_shift_inverts == d.caps_shift_inverts,
        ));
        line("");
        line(
            "# 수식키 탭: \"toggle_english\"(영어 ↔ 방금 쓰던 언어), \"toggle_non_english\"(한국어 ↔ 일본어)",
        );
        line(
            "# 키 이름: shift_left, shift_right, control_left, control_right, alt_left, alt_right, meta_left, meta_right",
        );
        line(
            "# 이 표를 적으면 기본값(오른쪽 Shift = 영어, 왼쪽 Shift = 한↔일)을 모두 대신한다. 적지 않은 키는 아무 일도 하지 않는다.",
        );
        let taps_default = self.taps == d.taps;
        let taps = if taps_default { &d.taps } else { &self.taps };
        line(if taps_default { "# [taps]" } else { "[taps]" });
        for name in TAP_KEYS {
            let action = taps.get(name).copied().unwrap_or(TapAction::None);
            if action != TapAction::None {
                line(&setting(name, &quoted(tap_action_name(action)), taps_default));
            }
        }
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
        line("# 읽기가 없을 때 Space를 전각 스페이스로");
        line(&setting(
            "full_width_space",
            &ja.full_width_space.to_string(),
            ja.full_width_space == dja.full_width_space,
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
        line(
            "# Codex처럼 줄바꿈 입력을 전송으로 받는 앱에서, 조합 중 Shift+Enter로 확정한 뒤 줄을 바꾸기까지 기다리는 시간(밀리초, 20~1000)",
        );
        line(&setting(
            "newline_replay_ms",
            &mac.newline_replay_ms.to_string(),
            mac.newline_replay_ms == dmac.newline_replay_ms,
        ));
        out
    }

    pub fn tap_action(&self, key: Key) -> TapAction {
        self.taps
            .iter()
            .find(|(name, _)| modifier_key(name) == Some(key))
            .map(|(_, &a)| a)
            .unwrap_or(TapAction::None)
    }
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

fn tap_action_name(a: TapAction) -> &'static str {
    match a {
        TapAction::None => "none",
        TapAction::ToggleEnglish => "toggle_english",
        TapAction::ToggleNonEnglish => "toggle_non_english",
    }
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

fn modifier_key(name: &str) -> Option<Key> {
    Some(match name {
        "shift_left" => Key::SHIFT_LEFT,
        "shift_right" => Key::SHIFT_RIGHT,
        "control_left" => Key::CONTROL_LEFT,
        "control_right" => Key::CONTROL_RIGHT,
        "alt_left" => Key::ALT_LEFT,
        "alt_right" => Key::ALT_RIGHT,
        "meta_left" => Key::META_LEFT,
        "meta_right" => Key::META_RIGHT,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_nrime_usage() {
        let c = Config::default();
        assert_eq!(c.tap_action(Key::SHIFT_RIGHT), TapAction::ToggleEnglish);
        assert_eq!(c.tap_action(Key::SHIFT_LEFT), TapAction::ToggleNonEnglish);
        assert_eq!(c.tap_action(Key::META_LEFT), TapAction::None);
        assert_eq!(c.tap_threshold_ms, 200);
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
        // 표 머리([ja] [mac])와 설명 말고는 모두 주석: 기본값이 바뀌면 따라간다.
        let live: Vec<&str> =
            text.lines().filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('[')).collect();
        assert!(live.is_empty(), "{live:?}");
        for key in
            ["ko_layout", "tap_threshold_ms", "shift_right", "punctuation", "katakana_direct", "hud_position"]
        {
            assert!(text.contains(&format!("# {key} = ")), "{key}");
        }
        // 옛 틀(0.2.x, 모두 주석)과 같은 설정이다.
        assert_eq!(Config::from_toml("# [taps]\n# [ja]\n").unwrap(), Config::default());
    }

    #[test]
    fn changed_settings_are_written_and_read_back() {
        let c = Config {
            ko_layout: "chamshin-d-v19".into(),
            tap_threshold_ms: 250,
            ja: JaConfig {
                punctuation: JaPunct::HalfWidthWestern,
                katakana_direct: false,
                ..JaConfig::default()
            },
            mac: MacConfig { hud: false, hud_position: HudPosition::Mouse, newline_replay_ms: 80 },
            ..Config::default()
        };
        round_trip(&c);
        let text = c.to_toml();
        assert!(text.contains("\nko_layout = \"chamshin-d-v19\"\n"));
        assert!(text.contains("\n# slash_nakaguro = true\n"), "바꾸지 않은 것은 주석 그대로");

        // 탭 표: 바꾸면 표 전체를 적는다. 없음은 적지 않는다. 모두 없음이면 빈 표.
        let t = Config {
            taps: HashMap::from([
                ("alt_right".into(), TapAction::ToggleEnglish),
                ("shift_left".into(), TapAction::None),
            ]),
            ..Config::default()
        };
        round_trip(&Config {
            taps: HashMap::from([("alt_right".into(), TapAction::ToggleEnglish)]),
            ..t.clone()
        });
        let text = t.to_toml();
        assert!(text.contains("\n[taps]\nalt_right = \"toggle_english\"\n\n"), "{text}");
        let none = Config { taps: HashMap::new(), ..Config::default() };
        round_trip(&none);
        assert!(none.to_toml().contains("\n[taps]\n\n[ja]"));
    }
}
