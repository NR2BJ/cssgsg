//! 설정. 맥과 윈도우가 같은 스키마(config.toml)를 쓴다.
//!
//! NRIME에서 안 쓰던 기능(한/일 직접 전환 단축키, 실시간 변환, 예측 입력, F6~F10 변환 키,
//! 앱별 언어 기억)은 넣지 않는다. 바꿔본 적은 없어도 아쉬울 파라미터(탭 인식 시간 등)는 남긴다.

use std::collections::HashMap;

use serde::Deserialize;

use crate::key::Key;

/// 수식키를 혼자 탭했을 때 할 일.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapAction {
    None,
    /// 영어 ↔ 직전 비영어 모드.
    ToggleEnglish,
    /// 한국어 ↔ 일본어.
    ToggleNonEnglish,
}

/// 일본어 모드 구두점 모양(NRIME와 같은 세 가지).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JaPunct {
    /// 、。「」 (新月 기본)
    Japanese,
    /// ，．［］
    FullWidthWestern,
    /// ,.[]
    HalfWidthWestern,
}

#[derive(Clone, Debug, Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HudPosition {
    /// 커서 위(커서 자리를 모르면 안 띄운다).
    #[default]
    Caret,
    /// 마우스 옆(늘 뜬다).
    Mouse,
}

/// 맥 셸 설정. 엔진은 쓰지 않고, 셸이 `cssgsg_engine_mac_settings`로 읽는다(설정 파일을 한 곳에 두려고).
#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

impl Config {
    pub fn from_toml(src: &str) -> Result<Self, String> {
        let c: Config = toml::from_str(src).map_err(|e| e.to_string())?;
        if crate::hangul::KoLayout::BUILTIN.iter().all(|(id, _)| *id != c.ko_layout) {
            return Err(format!("알 수 없는 한국어 배열 {:?}", c.ko_layout));
        }
        for name in c.taps.keys() {
            if modifier_key(name).is_none() {
                return Err(format!("알 수 없는 탭 키 이름 {name:?}"));
            }
        }
        if !(20..=1000).contains(&c.mac.newline_replay_ms) {
            return Err(format!("mac.newline_replay_ms는 20~1000이어야 한다: {}", c.mac.newline_replay_ms));
        }
        Ok(c)
    }

    pub fn tap_action(&self, key: Key) -> TapAction {
        self.taps
            .iter()
            .find(|(name, _)| modifier_key(name) == Some(key))
            .map(|(_, &a)| a)
            .unwrap_or(TapAction::None)
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
    }
}
