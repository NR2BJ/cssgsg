//! 일본어: 新月配列(신게츠) 가나 조합기.

mod composer;
mod layout;

pub use composer::{KanaComposer, Pending};
pub use layout::KanaLayout;

/// 히라가나를 가타카나로 바꾼다. 그 밖의 글자는 그대로 둔다.
pub fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309E}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn katakana() {
        assert_eq!(to_katakana("にほんご、ゔぁ"), "ニホンゴ、ヴァ");
        assert_eq!(to_katakana("ー「abc」"), "ー「abc」");
    }
}
