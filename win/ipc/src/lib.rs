//! 윈도우 입력기와 엔진 호스트 사이의 메시지와 파이프(CONCEPT §6.3, §10.3).
//!
//! 입력기 DLL은 글자를 치는 앱마다 하나씩 뜨고, Mozc는 사용자당 하나인 엔진 호스트(cssgsg-host.exe)에 있다:
//! Mozc는 학습 파일을 한 프로세스만 쓰게 되어 있고, 앱 컨테이너 앱은 학습 폴더에 쓸 수 없다.
//! 입력기는 변환기(`cssgsg_core::convert::Converter`) 함수 하나를 요청 하나로 보낸다. 파이프는 메시지 모드라
//! 요청 하나·응답 하나가 메시지 하나이고, 내용은 JSON 한 줄이다.

use cssgsg_core::convert::{ConvCmd, ConvView};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[cfg(windows)]
pub mod pipe;

/// 메시지 판. 입력기와 호스트가 다른 설치본이면(업데이트 중) 서로 맞지 않는다: 입력기는 그 호스트를 쓰지 않는다.
pub const PROTOCOL: u32 = 1;

/// 메시지 하나의 최대 크기(바이트). 후보가 수백 개여도 넉넉하다.
pub const MAX_MESSAGE: usize = 256 * 1024;

/// 입력기 → 호스트.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// 연결 처음: 메시지 판을 맞춘다.
    Hello { protocol: u32 },
    /// 읽기로 변환을 시작한다. 다른 입력기의 변환이 남아 있었으면 그것을 버린다(포커스는 한 곳이다).
    Start { reading: String },
    /// 변환 중 명령.
    Command { cmd: ConvCmd },
    /// 지금 결과를 확정한다(Mozc가 학습한다).
    Commit,
    /// 변환을 버린다.
    Cancel,
    /// 사용자 사전을 다시 읽는다(설정 앱이 고친 뒤).
    Reload,
}

/// 호스트 → 입력기.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    /// 호스트의 메시지 판과 변환 엔진(Mozc 버전. 엔진을 읽지 못했으면 None: 히라가나·가타카나만 낸다).
    Hello { protocol: u32, engine: Option<String> },
    /// 변환 화면(Start, Command). None이면 엔진이 하지 못했다.
    View { view: Option<ConvView> },
    /// 확정한 글자(Commit).
    Text { text: String },
    /// 했다(Cancel, Reload).
    Done,
    /// 이 입력기의 변환이 아니다: 다른 입력기의 Start가 밀어냈다.
    Lost,
    /// 요청을 읽지 못했다.
    Error { message: String },
}

/// 메시지를 파이프에 실을 바이트로.
pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    // 문자열·숫자·열거만 있는 메시지라 직렬화는 실패하지 않는다.
    serde_json::to_vec(message).unwrap_or_default()
}

/// 받은 바이트를 메시지로. 모양이 틀리면 None.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    serde_json::from_slice(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let view = ConvView {
            segments: vec!["日本語".into()],
            focused: 0,
            candidates: vec!["日本語".into(), "ニホンゴ".into()],
            selected: Some(0),
        };
        let requests = [
            Request::Hello { protocol: PROTOCOL },
            Request::Start { reading: "にほんご".into() },
            Request::Command { cmd: ConvCmd::Select(12) },
            Request::Command { cmd: ConvCmd::FocusLeft },
            Request::Commit,
            Request::Cancel,
            Request::Reload,
        ];
        for r in requests {
            assert_eq!(decode::<Request>(&encode(&r)), Some(r));
        }
        let replies = [
            Reply::Hello { protocol: PROTOCOL, engine: Some("3.34.6239.101".into()) },
            Reply::Hello { protocol: PROTOCOL, engine: None },
            Reply::View { view: Some(view) },
            Reply::View { view: None },
            Reply::Text { text: "日本語".into() },
            Reply::Done,
            Reply::Lost,
            Reply::Error { message: "?".into() },
        ];
        for r in replies {
            assert_eq!(decode::<Reply>(&encode(&r)), Some(r));
        }
        assert_eq!(decode::<Request>(b"{\"Nope\":1}"), None);
    }
}
