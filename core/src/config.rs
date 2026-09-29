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
}

impl Default for JaConfig {
    fn default() -> Self {
        Self {
            punctuation: JaPunct::Japanese,
            slash_nakaguro: true,
            full_width_space: false,
            caps_katakana_auto_off: true,
        }
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
