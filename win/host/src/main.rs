//! 엔진 호스트(cssgsg-host.exe): 사용자당 하나 뜨는 일본어 변환 프로세스(CONCEPT §6.3, §10.3).
//!
//! 윈도우 입력기는 글자를 치는 앱마다 하나씩 뜬다. Mozc는 학습 파일을 한 프로세스만 쓰게 되어 있고, 앱 컨테이너 앱은
//! 학습 폴더에 쓸 수 없어서 Mozc는 여기 하나만 둔다.
//!
//! 로그인할 때 시작 프로그램(HKCU …\Run의 cssgsg)으로 떠서 늘 켜져 있다(사용자 결정 2026-10-02: 스토어 앱·관리자 앱은
//! 호스트를 띄울 수 없으니, 그런 앱에서도 변환되게). 죽으면 보통 앱의 입력기가 다시 띄운다. 메시지는 cssgsg-ipc.
//! 엔진(cssgsg_mozc.dll, mozc.data)은 이 실행 파일 옆의 mozc 폴더, 학습은 %LOCALAPPDATA%\cssgsg\mozc.
//!
//! 옵션(시험·개발용): --tag <붙임>(파이프 이름), --engine-dir <폴더>, --profile <폴더>, --no-engine(히라가나·가타카나만),
//! --idle-exit-secs <초>(이어진 입력기 없이 그만큼 지나면 끝난다. 주지 않으면 끝나지 않는다).
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod files;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod setup;

#[cfg(windows)]
fn main() {
    host::main();
}

#[cfg(not(windows))]
fn main() {}
