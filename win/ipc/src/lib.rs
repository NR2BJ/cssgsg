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

/// 메시지 하나의 최대 크기(바이트). 한자 기억 전체(1만 개, 200KB 안팎)도 들어간다. 이보다 크면 연결을 끊는다.
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;

/// 입력기 → 호스트.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// 연결 처음: 메시지 판을 맞춘다.
    Hello { protocol: u32 },
    /// 읽기로 변환을 시작한다. 다른 입력기의 변환이 남아 있었으면 그것을 버린다(포커스는 한 곳이다).
    /// `learn`이 거짓이면(시크릿 창 같은 개인 입력칸, 입력 범위 IS_PRIVATE) 이 변환은 학습하지 않는다.
    Start {
        reading: String,
        #[serde(default = "yes")]
        learn: bool,
    },
    /// 변환 중 명령.
    Command { cmd: ConvCmd },
    /// 지금 결과를 확정한다(Mozc가 학습한다).
    Commit,
    /// 변환을 버린다.
    Cancel,
    /// 사용자 사전을 다시 읽는다(설정 앱이 고친 뒤).
    Reload,
    /// 호스트를 끝낸다(엔진을 내려 학습을 마무리한 뒤). 설치기가 파일을 바꾸기 전에 보낸다(cssgsg-host.exe --quit).
    Quit,
    /// 설정 파일과 한자 기억을 맞춘다(입력기를 켤 때, 입력칸이 바뀔 때). 가진 판(처음은 0)과 다르면 호스트가 새것을 준다.
    /// 앱 컨테이너 앱은 설정 폴더를 읽을 수 없고 앱마다 엔진이 따로라, 파일은 호스트가 읽고 쓴다.
    Sync { config: u64, learning: u64 },
    /// 한자를 하나 골랐다. 호스트가 기억에 더하고 저장한다(다른 앱은 다음 Sync에 받는다).
    HanjaPicked { reading: String, text: String },
    /// 고른 한자 기억을 모두 지운다(설정 앱). 입력기들은 다음 Sync에 빈 기억을 받는다.
    ClearHanjaLearning,
    /// Mozc가 배운 변환(문절 나누기, 고른 후보)을 지운다(설정 앱). 사용자 사전은 그대로다.
    /// 호스트는 엔진을 내리고(학습 파일을 놓게) 파일을 지운 뒤 다시 읽는다.
    ClearMozcLearning,
    /// 설정 앱을 연다(작업 표시줄 모드 아이콘 메뉴). `tab`은 처음 보일 탭(learn, practice …). 입력기가 직접 띄우면 앱 컨테이너
    /// 앱 안에서는 설정 앱이 그 컨테이너에 갇히고 관리자 권한 앱에서는 관리자가 되므로 호스트가 띄운다.
    OpenSettings { tab: Option<String> },
    /// 호스트를 다시 띄운다(메뉴·설정 앱의 "다시 시작"): 새 호스트를 띄우고(이 호스트가 끝나기를 기다린다) 학습을 마무리한 뒤 끝난다.
    /// 앱 컨테이너 앱처럼 호스트를 띄울 수 없는 곳에서 눌러도 호스트가 비지 않는다. 받아 둔 새 Mozc 엔진도 이때 쓴다.
    Restart,
    /// Mozc 엔진 상태(설정 앱 일본어 탭): 쓰는 엔진, 받아 두고 기다리는 새 엔진, 마지막 확인. 답은 [`Reply::Engine`].
    EngineStatus,
    /// 새 Mozc 엔진을 지금 확인한다(하루 한 번을 기다리지 않고). 확인은 뒤에서 하고 바로 답한다(상태로 지켜본다).
    CheckEngine,
}

/// Mozc 엔진 하나: 판(Mozc 버전), upstream 커밋 날짜·커밋, 래퍼 판, 내려받은 것인지(아니면 설치본에 든 것).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineBuild {
    pub version: String,
    pub date: String,
    pub commit: String,
    /// cssgsg 래퍼(mozc/cssgsg, C API) 판(`mozc/cssgsg/WRAPPER_REVISION`, CONCEPT §6.3). 래퍼만 고쳐 같은 Mozc 커밋으로 다시
    /// 빌드한 엔진은 이것만 크다. 판을 적지 않던 엔진은 0.
    #[serde(default)]
    pub wrapper: u32,
    pub downloaded: bool,
}

/// 엔진 업데이트 상태(맥 MozcStatus와 같은 몫). 시각은 유닉스 초.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInfo {
    /// 지금 쓰는 엔진. 읽지 못했으면 None(가나만 낸다).
    pub active: Option<EngineBuild>,
    /// 받아 두었고 호스트가 다시 시작하면 쓸 새 엔진.
    pub pending: Option<EngineBuild>,
    pub checking: bool,
    pub checked_at: Option<u64>,
    /// 마지막 확인이 실패한 때와 까닭(성공하면 지운다).
    pub failed_at: Option<u64>,
    pub failure: Option<String>,
}

fn yes() -> bool {
    true
}

/// 판이 붙은 파일 내용. 판은 내용의 해시라(0이 아니다) 같은 내용이면 같은 판이다.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versioned {
    pub version: u64,
    pub text: String,
}

/// 내용의 판(FNV-1a 64, 0이 되지 않게).
pub fn version_of(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash | 1
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
    /// 했다(Cancel, Reload, 지우기).
    Done,
    /// 이 입력기의 변환이 아니다: 다른 입력기의 Start가 밀어냈다.
    Lost,
    /// 요청을 읽지 못했거나 하지 못했다(까닭).
    Error { message: String },
    /// Sync의 답: 판이 다른 것만 담는다. 설정은 늘 올바른 것만 준다(틀린 파일은 호스트가 기록하고 앞의 것을 쓴다).
    Sync { config: Option<Versioned>, learning: Option<Versioned> },
    /// EngineStatus의 답.
    Engine { info: EngineInfo },
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
            Request::Start { reading: "にほんご".into(), learn: true },
            Request::Start { reading: "にほんご".into(), learn: false },
            Request::Sync { config: 0, learning: 42 },
            Request::HanjaPicked { reading: "한".into(), text: "韓".into() },
            Request::Command { cmd: ConvCmd::Select(12) },
            Request::Command { cmd: ConvCmd::FocusLeft },
            Request::Commit,
            Request::Cancel,
            Request::Reload,
            Request::Quit,
            Request::ClearHanjaLearning,
            Request::ClearMozcLearning,
            Request::OpenSettings { tab: None },
            Request::OpenSettings { tab: Some("practice".into()) },
            Request::Restart,
            Request::EngineStatus,
            Request::CheckEngine,
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
            Reply::Sync { config: Some(Versioned { version: 7, text: "a = 1\n".into() }), learning: None },
            Reply::Engine {
                info: EngineInfo {
                    active: Some(EngineBuild {
                        version: "3.34.6239.101".into(),
                        date: "2026-09-28".into(),
                        commit: "a069a88d4cb5c011de0f9aebb6c149a1c808d904".into(),
                        wrapper: 1,
                        downloaded: false,
                    }),
                    checked_at: Some(1_790_000_000),
                    ..EngineInfo::default()
                },
            },
        ];
        for r in replies {
            assert_eq!(decode::<Reply>(&encode(&r)), Some(r));
        }
        assert_eq!(decode::<Request>(b"{\"Nope\":1}"), None);
        // 래퍼 판 전의 호스트가 보낸 엔진은 래퍼 판 0이다.
        let old = r#"{"version":"3.34.6239.101","date":"2026-09-28","commit":"a069a88","downloaded":true}"#;
        assert_eq!(serde_json::from_str::<EngineBuild>(old).map(|b| b.wrapper).ok(), Some(0));
        // 앞 판의 입력기가 보낸 Start(learn 없음)는 학습한다.
        assert_eq!(
            decode::<Request>("{\"Start\":{\"reading\":\"かな\"}}".as_bytes()),
            Some(Request::Start { reading: "かな".into(), learn: true })
        );
    }

    #[test]
    fn versions_follow_the_content() {
        assert_eq!(version_of("a"), version_of("a"));
        assert_ne!(version_of("a"), version_of("b"));
        assert_ne!(version_of(""), 0, "0은 '아직 없음'이다");
    }
}
