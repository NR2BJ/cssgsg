//! 한국어: 한글 낱자 표, 참신세벌식 배열 데이터, 갈마들이 조합기.

mod composer;
mod layout;

pub use composer::{KoComposer, KoResult};
pub use layout::{Action, KoLayout, Roles};

/// 초성 19자 (호환 자모, 유니코드 음절 조합 순서).
pub const CHO: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ',
    'ㅍ', 'ㅎ',
];

/// 중성 21자.
pub const JUNG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ', 'ㅟ',
    'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

/// 종성 27자 (받침 없음은 따로 친다).
pub const JONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ', 'ㅂ',
    'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

pub fn is_cho(c: char) -> bool {
    CHO.contains(&c)
}
pub fn is_jung(c: char) -> bool {
    JUNG.contains(&c)
}
pub fn is_jong(c: char) -> bool {
    JONG.contains(&c)
}

/// 초성·중성(·종성)을 완성형 음절 하나로 합친다.
pub fn syllable(cho: char, jung: char, jong: Option<char>) -> Option<char> {
    let l = CHO.iter().position(|&c| c == cho)? as u32;
    let v = JUNG.iter().position(|&c| c == jung)? as u32;
    let t = match jong {
        None => 0,
        Some(j) => JONG.iter().position(|&c| c == j)? as u32 + 1,
    };
    char::from_u32(0xAC00 + (l * 21 + v) * 28 + t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_syllables() {
        assert_eq!(syllable('ㄱ', 'ㅏ', None), Some('가'));
        assert_eq!(syllable('ㅎ', 'ㅏ', Some('ㄴ')), Some('한'));
        assert_eq!(syllable('ㄱ', 'ㅏ', Some('ㄺ')), Some('갉'));
        assert_eq!(syllable('ㅎ', 'ㅣ', Some('ㅎ')), Some('힣'));
        assert_eq!(syllable('ㄸ', 'ㅏ', Some('ㄸ')), None);
    }
}
