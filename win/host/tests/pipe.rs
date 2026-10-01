//! 호스트 실행 파일을 띄워 입력기 쪽 연결(cssgsg_ipc::pipe::Client)로 묻는다. 엔진은 끈다(--no-engine:
//! 히라가나·가타카나만 내는 변환기)라서 Mozc 없이 돈다. 파이프 이름에 시험마다 다른 꼬리를 붙여 진짜 호스트와 겹치지 않고,
//! 설정·한자 기억 폴더도 시험마다 임시 폴더를 쓴다(진짜 %APPDATA%\cssgsg를 건드리지 않는다).
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use cssgsg_core::convert::ConvCmd;
use cssgsg_ipc::pipe::{CallError, Client, pipe_name, user_sid};
use cssgsg_ipc::{PROTOCOL, Reply, Request};

const WAIT: Duration = Duration::from_secs(5);

struct Host {
    child: Child,
    tag: String,
    dir: PathBuf,
}

impl Host {
    /// 호스트를 띄우고 파이프로 답할 때까지 기다린다(시험의 연결은 띄운 호스트를 기다리지 않고 바로 포기한다).
    fn start(test: &str) -> Self {
        let tag = format!("-test-{}-{test}", std::process::id());
        let dir = std::env::temp_dir().join(format!("cssgsg-host{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let host = Self { child: launch(&tag, &dir), tag, dir };
        host.wait_ready();
        host
    }

    fn wait_ready(&self) {
        let began = Instant::now();
        while self.client().call(&Request::Hello { protocol: PROTOCOL }, Duration::from_millis(200)).is_err()
        {
            assert!(began.elapsed() < WAIT, "호스트가 파이프를 만들지 않는다");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn client(&self) -> Client {
        // 시험의 연결은 호스트를 띄우지 않는다(띄운 것은 시험이 끈다).
        Client::new(pipe_name(&user_sid().unwrap(), &self.tag), None)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn launch(tag: &str, dir: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_cssgsg-host"))
        .args(["--tag", tag, "--no-engine", "--idle-exit-secs", "60", "--user-dir"])
        .arg(dir)
        .arg("--profile")
        .arg(dir.join("mozc"))
        .spawn()
        .unwrap()
}

fn start(client: &mut Client, reading: &str) -> Vec<String> {
    match client.call(&Request::Start { reading: reading.into(), learn: true }, WAIT).unwrap() {
        Reply::View { view: Some(view) } => view.candidates,
        other => panic!("{other:?}"),
    }
}

#[test]
fn converts_through_the_pipe() {
    let host = Host::start("convert");
    let mut c = host.client();
    assert_eq!(start(&mut c, "にほんご"), ["にほんご", "ニホンゴ"]);
    assert_eq!(c.engine(), None, "엔진을 끈 호스트");
    match c.call(&Request::Command { cmd: ConvCmd::Next }, WAIT).unwrap() {
        Reply::View { view: Some(view) } => assert_eq!(view.segments, ["ニホンゴ"]),
        other => panic!("{other:?}"),
    }
    assert_eq!(c.call(&Request::Commit, WAIT).unwrap(), Reply::Text { text: "ニホンゴ".into() });
    // 끝난 변환에는 명령을 받지 않는다.
    assert_eq!(c.call(&Request::Commit, WAIT).unwrap(), Reply::Lost);
    assert_eq!(c.call(&Request::Cancel, WAIT).unwrap(), Reply::Done);
    assert_eq!(c.call(&Request::Reload, WAIT).unwrap(), Reply::Done);
}

#[test]
fn a_start_elsewhere_takes_over_the_conversion() {
    let host = Host::start("takeover");
    let (mut a, mut b) = (host.client(), host.client());
    start(&mut a, "かな");
    start(&mut b, "にほんご");
    // a의 변환은 b가 밀어냈다: a는 보이던 글자를 그대로 확정해야 한다(HostConverter).
    assert_eq!(a.call(&Request::Command { cmd: ConvCmd::Next }, WAIT).unwrap(), Reply::Lost);
    assert_eq!(a.call(&Request::Commit, WAIT).unwrap(), Reply::Lost);
    assert_eq!(b.call(&Request::Commit, WAIT).unwrap(), Reply::Text { text: "にほんご".into() });
}

#[test]
fn a_restarted_host_is_reached_without_losing_a_call() {
    let first = Host::start("restart");
    let (tag, dir) = (first.tag.clone(), first.dir.clone());
    let mut c = first.client();
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
    // 호스트가 다시 뜬다(업데이트 등). 쥐고 있던 연결은 끊겼지만 다음 호출은 새 호스트에 닿는다.
    drop(first);
    let second = Host { child: launch(&tag, &dir), tag, dir };
    second.wait_ready();
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
}

#[test]
fn hands_out_settings_and_keeps_hanja_picks() {
    let host = Host::start("files");
    let mut c = host.client();
    let mut sync = |config, learning| match c.call(&Request::Sync { config, learning }, WAIT).unwrap() {
        Reply::Sync { config, learning } => (config, learning),
        other => panic!("{other:?}"),
    };
    // 처음: 설정 파일이 없으면 빈 내용(기본값), 한자 기억도 비었다. 판을 맞추면 더 주지 않는다.
    let (Some(config), Some(learning)) = sync(0, 0) else { panic!("처음에는 둘 다 준다") };
    assert_eq!(
        (config.text.as_str(), learning.text.lines().filter(|l| !l.starts_with('#')).count()),
        ("", 0)
    );
    assert_eq!(sync(config.version, learning.version), (None, None));
    // 설정 파일을 고치면 새것을 준다. 틀린 파일이면 앞의 것을 그대로 쓴다(아무것도 주지 않는다).
    std::fs::write(host.dir.join("config.toml"), "tap_threshold_ms = 250\n").unwrap();
    let (Some(changed), None) = sync(config.version, learning.version) else { panic!("고친 설정") };
    assert!(changed.text.contains("250"));
    std::fs::write(host.dir.join("config.toml"), "tap_threshold_ms = 5\n").unwrap();
    assert_eq!(sync(changed.version, learning.version), (None, None), "범위 밖 값은 받지 않는다");
    // 한자를 고르면 기억에 더하고, 2초 모았다가 저장한다.
    assert_eq!(
        c.call(&Request::HanjaPicked { reading: "한".into(), text: "漢".into() }, WAIT).unwrap(),
        Reply::Done
    );
    let mut sync = |config, learning| match c.call(&Request::Sync { config, learning }, WAIT).unwrap() {
        Reply::Sync { config, learning } => (config, learning),
        other => panic!("{other:?}"),
    };
    let (None, Some(picked)) = sync(changed.version, learning.version) else { panic!("고른 한자") };
    assert!(picked.text.contains("한\t漢\t1\t"), "{}", picked.text);
    let saved = host.dir.join("hanja-learning.tsv");
    let began = Instant::now();
    while !std::fs::read_to_string(&saved).is_ok_and(|t| t.contains("한\t漢")) {
        assert!(began.elapsed() < WAIT, "한자 기억을 저장하지 않는다");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn the_settings_app_clears_what_was_learned() {
    let host = Host::start("clear");
    let mut c = host.client();
    // 한자 기억: 지우면 빈 기억을 바로 저장하고, 입력기들은 다음 Sync에 빈 기억을 받는다.
    c.call(&Request::HanjaPicked { reading: "한".into(), text: "漢".into() }, WAIT).unwrap();
    let Reply::Sync { learning: Some(picked), .. } =
        c.call(&Request::Sync { config: 0, learning: 0 }, WAIT).unwrap()
    else {
        panic!("고른 한자")
    };
    assert_eq!(c.call(&Request::ClearHanjaLearning, WAIT).unwrap(), Reply::Done);
    let Reply::Sync { learning: Some(cleared), .. } =
        c.call(&Request::Sync { config: 0, learning: picked.version }, WAIT).unwrap()
    else {
        panic!("지운 기억")
    };
    assert!(!cleared.text.contains("漢"), "{}", cleared.text);
    let saved = std::fs::read_to_string(host.dir.join("hanja-learning.tsv")).unwrap();
    assert!(!saved.contains("漢"), "{saved}");
    // Mozc 학습: 학습 파일만 지우고 사용자 사전·Mozc 설정은 남긴다. 엔진을 다시 읽은 뒤에도 변환한다.
    let profile = host.dir.join("mozc");
    std::fs::create_dir_all(&profile).unwrap();
    for name in ["segment.db", "boundary.db", "cform.db", "user_dictionary.db", "config1.db"] {
        std::fs::write(profile.join(name), "x").unwrap();
    }
    assert_eq!(c.call(&Request::ClearMozcLearning, WAIT).unwrap(), Reply::Done);
    let left: Vec<String> = std::fs::read_dir(&profile)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left.contains(&"user_dictionary.db".into()) && left.contains(&"config1.db".into()));
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
}

#[test]
fn restart_hands_over_to_a_new_host() {
    let host = Host::start("restart-request");
    let mut c = host.client();
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
    // 다시 시작: 새 호스트가 앞 호스트가 끝나기를 기다렸다가 같은 파이프를 만든다. 쥐고 있던 연결은 다시 이어진다.
    assert_eq!(c.call(&Request::Restart, WAIT).unwrap(), Reply::Done);
    let began = Instant::now();
    loop {
        match c.call(&Request::Hello { protocol: PROTOCOL }, Duration::from_millis(300)) {
            Ok(Reply::Hello { .. }) => break,
            _ => {
                assert!(began.elapsed() < WAIT, "새 호스트가 뜨지 않는다");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
    // 설정 앱 열기: 시험 호스트 옆에는 설정 앱이 없어서 까닭이 온다. 탭 이름 말고는 받지 않는다.
    assert!(matches!(c.call(&Request::OpenSettings { tab: None }, WAIT).unwrap(), Reply::Error { .. }));
    match c.call(&Request::OpenSettings { tab: Some("--evil".into()) }, WAIT).unwrap() {
        Reply::Error { message } => assert!(message.contains("bad tab"), "{message}"),
        other => panic!("{other:?}"),
    }
    // 새 호스트는 시험이 띄운 것이 아니라서 끝내라고 한다.
    assert_eq!(c.call(&Request::Quit, WAIT).unwrap(), Reply::Done);
}

#[test]
fn one_host_per_pipe_and_a_dead_host_is_noticed() {
    let host = Host::start("single");
    let mut c = host.client();
    assert!(matches!(
        c.call(&Request::Hello { protocol: PROTOCOL }, WAIT).unwrap(),
        Reply::Hello { protocol: PROTOCOL, .. }
    ));
    // 같은 이름의 두 번째 호스트는 바로 끝난다.
    let mut second = launch(&host.tag, &host.dir);
    let begun = Instant::now();
    loop {
        if let Some(status) = second.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(begun.elapsed() < WAIT, "두 번째 호스트가 끝나지 않는다");
        std::thread::sleep(Duration::from_millis(20));
    }
    // 첫 호스트는 그대로 답한다.
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
    // 호스트가 죽으면 다음 호출은 실패하고(띄우지 않는 연결), 그 뒤로도 멈추지 않는다.
    drop(host);
    let quick = Duration::from_millis(300);
    assert!(c.call(&Request::Commit, quick).is_err());
    assert_eq!(
        c.call(&Request::Start { reading: "かな".into(), learn: true }, quick),
        Err(CallError::NoHost)
    );
}
