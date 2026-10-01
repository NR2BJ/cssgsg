//! cssgsg 입력 엔진 코어.
//!
//! 플랫폼 독립 순수 로직이다. 파일·시간·IPC·OS 호출을 하지 않는다.
//! 셸(macOS IMKit, 나중에 Windows TSF)이 키 이벤트를 넣고 [`Output`]대로 화면을 바꾼다.

pub mod config;
pub mod convert;
pub mod engine;
#[cfg(feature = "ffi")]
pub mod ffi;
pub mod hangul;
pub mod hanja;
pub mod hotkey;
pub mod kana;
pub mod key;
pub mod latin;
#[cfg(feature = "mozc-engine")]
pub mod mozc;
pub mod shortcut;
pub mod sim;

pub use config::Config;
pub use engine::{Context, Engine, Mode, Output};
pub use key::{Key, KeyEvent, Mods};
