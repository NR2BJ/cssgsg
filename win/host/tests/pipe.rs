//! 호스트 실행 파일을 띄워 입력기 쪽 연결(cssgsg_ipc::pipe::Client)로 묻는다. 엔진은 끈다(--no-engine:
//! 히라가나·가타카나만 내는 변환기)라서 Mozc 없이 돈다. 파이프 이름에 시험마다 다른 꼬리를 붙여 진짜 호스트와 겹치지 않는다.
#![cfg(windows)]

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use cssgsg_core::convert::ConvCmd;
use cssgsg_ipc::pipe::{CallError, Client, pipe_name, user_sid};
use cssgsg_ipc::{PROTOCOL, Reply, Request};

const WAIT: Duration = Duration::from_secs(5);

struct Host {
    child: Child,
    tag: String,
}

impl Host {
    /// 호스트를 띄우고 파이프로 답할 때까지 기다린다(시험의 연결은 띄운 호스트를 기다리지 않고 바로 포기한다).
    fn start(test: &str) -> Self {
        let tag = format!("-test-{}-{test}", std::process::id());
        let host = Self { child: launch(&tag), tag };
        let began = Instant::now();
        while host.client().call(&Request::Hello { protocol: PROTOCOL }, Duration::from_millis(200)).is_err()
        {
            assert!(began.elapsed() < WAIT, "호스트가 파이프를 만들지 않는다");
            std::thread::sleep(Duration::from_millis(20));
        }
        host
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

fn launch(tag: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_cssgsg-host"))
        .args(["--tag", tag, "--no-engine", "--idle-exit-secs", "60"])
        .spawn()
        .unwrap()
}

fn start(client: &mut Client, reading: &str) -> Vec<String> {
    match client.call(&Request::Start { reading: reading.into() }, WAIT).unwrap() {
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
    let tag = first.tag.clone();
    let mut c = first.client();
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
    // 호스트가 다시 뜬다(업데이트 등). 쥐고 있던 연결은 끊겼지만 다음 호출은 새 호스트에 닿는다.
    drop(first);
    let second = Host { child: launch(&tag), tag };
    let began = Instant::now();
    while second.client().call(&Request::Hello { protocol: PROTOCOL }, Duration::from_millis(200)).is_err() {
        assert!(began.elapsed() < WAIT, "새 호스트가 파이프를 만들지 않는다");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(start(&mut c, "かな"), ["かな", "カナ"]);
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
    let mut second = launch(&host.tag);
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
    assert_eq!(c.call(&Request::Start { reading: "かな".into() }, quick), Err(CallError::NoHost));
}
