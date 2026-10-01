//! 일본어 한자 변환기: 사용자당 하나인 엔진 호스트(cssgsg-host.exe)에 파이프로 묻는다(win/ipc, CONCEPT §6.3).
//!
//! 앱의 입력 스레드에서 불리므로 호출마다 시간 제한이 있다. 호스트가 없거나 늦으면 이 앱 안에서 히라가나·가타카나만
//! 내는 변환기로 대신한다(호스트가 Mozc를 못 읽었을 때와 같다). 변환하는 중에 호스트가 죽거나 다른 앱의 변환이 밀어내면
//! 보이던 글자를 그대로 확정한다(학습은 없다). 호스트는 일본어 모드로 바꿀 때 미리 띄운다([`prepare`]).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use cssgsg_core::convert::{ConvCmd, ConvView, Converter, EchoConverter};
use cssgsg_ipc::pipe::{CallError, Client, may_spawn_host, pipe_name, user_sid};
use cssgsg_ipc::{Reply, Request};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

use crate::{debug_log, module};

/// 첫 변환은 호스트가 뜨기를 기다려야 할 수 있다(일본어 모드로 바꿀 때 미리 띄우니 보통은 이미 떠 있다).
const START_TIMEOUT: Duration = Duration::from_millis(800);
/// 변환 중 명령·확정. 엔진은 1ms 안쪽이라 넉넉하다.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(300);

/// 앱의 입력기 하나가 쓰는 호스트 연결(변환기와 텍스트 서비스가 같이 쥔다).
pub type HostLink = Rc<RefCell<Client>>;

/// 호스트 연결을 만든다(아직 잇지는 않는다). 앱 컨테이너·관리자 권한 앱은 호스트를 띄우지 않고, 떠 있는 것만 쓴다.
pub fn link() -> Option<HostLink> {
    let sid = user_sid()?;
    let host = if may_spawn_host() { host_exe() } else { None };
    Some(Rc::new(RefCell::new(Client::new(pipe_name(&sid, ""), host))))
}

/// 미리 잇는다(호스트가 없으면 띄우기만 한다).
pub fn prepare(link: &HostLink) {
    if let Ok(mut client) = link.try_borrow_mut()
        && !client.connected()
    {
        client.prepare();
        if client.connected() {
            debug_log(&format!("host connected, engine {:?}", client.engine()));
        }
    }
}

/// 이 DLL 옆의 cssgsg-host.exe.
fn host_exe() -> Option<PathBuf> {
    let mut buffer = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(module()), &mut buffer) } as usize;
    if n == 0 || n >= buffer.len() {
        return None;
    }
    let dll = PathBuf::from(String::from_utf16_lossy(&buffer[..n]));
    Some(dll.parent()?.join("cssgsg-host.exe"))
}

/// 답의 종류(개발자 기록에는 글자를 남기지 않는다).
fn kind(reply: &Result<Reply, CallError>) -> String {
    match reply {
        Ok(Reply::Hello { .. }) => "hello".into(),
        Ok(Reply::View { view }) => format!("view({})", view.is_some()),
        Ok(Reply::Text { .. }) => "text".into(),
        Ok(Reply::Done) => "done".into(),
        Ok(Reply::Lost) => "lost".into(),
        Ok(Reply::Error { message }) => format!("error {message}"),
        Err(e) => format!("{e:?}"),
    }
}

pub struct HostConverter {
    link: HostLink,
    /// 지금 보이는 변환 화면. 호스트가 확정하지 못하면 이것을 그대로 확정한다.
    shown: Option<ConvView>,
    /// 호스트에 닿지 못할 때 쓰는 이 앱 안의 변환기(히라가나·가타카나).
    local: EchoConverter,
    /// 지금 변환을 `local`이 하고 있다.
    in_local: bool,
}

impl HostConverter {
    pub fn new(link: HostLink) -> Self {
        Self { link, shown: None, local: EchoConverter::default(), in_local: false }
    }

    fn call(&self, request: &Request, timeout: Duration) -> Result<Reply, CallError> {
        match self.link.try_borrow_mut() {
            Ok(mut client) => client.call(request, timeout),
            Err(_) => Err(CallError::Broken("busy".into())),
        }
    }
}

impl Converter for HostConverter {
    fn start(&mut self, reading: &str) -> Option<ConvView> {
        let reply = self.call(&Request::Start { reading: reading.into() }, START_TIMEOUT);
        self.in_local = false;
        let view = match reply {
            Ok(Reply::View { view }) => view,
            Err(e @ (CallError::NoHost | CallError::Timeout | CallError::Broken(_))) => {
                debug_log(&format!("host start: {e:?}; kana only in this app"));
                self.in_local = true;
                self.local.start(reading)
            }
            other => {
                debug_log(&format!("host start: {}", kind(&other)));
                None
            }
        };
        self.shown = view.clone();
        view
    }

    fn command(&mut self, cmd: ConvCmd) -> Option<ConvView> {
        if self.in_local {
            let view = self.local.command(cmd);
            if view.is_some() {
                self.shown = view.clone();
            }
            return view;
        }
        match self.call(&Request::Command { cmd }, COMMAND_TIMEOUT) {
            Ok(Reply::View { view: Some(view) }) => {
                self.shown = Some(view.clone());
                Some(view)
            }
            other => {
                debug_log(&format!("host command: {}", kind(&other)));
                None
            }
        }
    }

    fn commit(&mut self) -> String {
        let shown = self.shown.take().map(|v| v.segments.concat()).unwrap_or_default();
        if std::mem::take(&mut self.in_local) {
            return self.local.commit();
        }
        match self.call(&Request::Commit, COMMAND_TIMEOUT) {
            Ok(Reply::Text { text }) if !text.is_empty() => text,
            other => {
                debug_log(&format!("host commit: {}; committing what was shown", kind(&other)));
                shown
            }
        }
    }

    fn cancel(&mut self) {
        if std::mem::take(&mut self.in_local) {
            self.shown = None;
            self.local.cancel();
            return;
        }
        // 변환 중이 아니면 할 일이 없다(포커스가 바뀔 때마다 불린다: 이것 때문에 잇거나 띄우지 않는다).
        if self.shown.take().is_some()
            && let Ok(mut client) = self.link.try_borrow_mut()
        {
            let _ = client.call_if_connected(&Request::Cancel, COMMAND_TIMEOUT);
        }
    }

    fn reload(&mut self) {
        if let Ok(mut client) = self.link.try_borrow_mut() {
            let _ = client.call_if_connected(&Request::Reload, COMMAND_TIMEOUT);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn without_a_host_kana_conversion_stays_in_the_app() {
        // 띄울 수 없는 앱(앱 컨테이너 등)에서 떠 있는 호스트가 없을 때.
        let link = Rc::new(RefCell::new(Client::new(r"\.\pipe\cssgsg-engine-test-nobody".into(), None)));
        let mut c = HostConverter::new(link);
        let began = Instant::now();
        let view = c.start("かな").unwrap();
        assert!(began.elapsed() < Duration::from_millis(200), "키마다 앱을 멈추지 않는다");
        assert_eq!(view.candidates, ["かな", "カナ"]);
        assert_eq!(c.command(ConvCmd::Next).unwrap().segments, ["カナ"]);
        assert_eq!(c.commit(), "カナ");
        assert_eq!(c.start("かな").unwrap().segments, ["かな"]);
        c.cancel();
        assert_eq!(c.commit(), "", "취소한 뒤에는 확정할 것이 없다");
    }
}
