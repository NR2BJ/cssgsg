//! 일본어 한자 변환기: 사용자당 하나인 엔진 호스트(cssgsg-host.exe)에 파이프로 묻는다(win/ipc, CONCEPT §6.3).
//!
//! 앱의 입력 스레드에서 불리므로 호출마다 시간 제한이 있다. 호스트가 없거나 늦으면 이 앱 안에서 히라가나·가타카나만
//! 내는 변환기로 대신한다(호스트가 Mozc를 못 읽었을 때와 같다). 변환하는 중에 호스트가 죽거나 다른 앱의 변환이 밀어내면
//! 보이던 글자를 그대로 확정한다(학습은 없다). 호스트는 일본어 모드로 바꿀 때 미리 띄운다([`prepare`]).
//!
//! 설정 파일과 한자 기억도 호스트가 읽고 쓴다: 입력기는 켤 때와 입력칸이 바뀔 때 판을 맞추고([`sync`]),
//! 한자를 고르면 알린다([`report_pick`]). 시크릿 창 같은 개인 입력칸(입력 범위 IS_PRIVATE)에서는 둘 다 배우지 않는다.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use cssgsg_core::convert::{ConvCmd, ConvView, Converter, EchoConverter};
use cssgsg_ipc::pipe::{CallError, Client, may_spawn_host, pipe_name, user_sid};
use cssgsg_ipc::{Reply, Request, Versioned};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};

use crate::{debug_log, module};

/// 첫 변환은 호스트가 뜨기를 기다려야 할 수 있다(일본어 모드로 바꿀 때 미리 띄우니 보통은 이미 떠 있다).
const START_TIMEOUT: Duration = Duration::from_millis(800);
/// 변환 중 명령·확정. 엔진은 1ms 안쪽이라 넉넉하다.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(300);
/// 설정·한자 기억 맞추기(켤 때, 입력칸이 바뀔 때).
const SYNC_TIMEOUT: Duration = Duration::from_millis(300);

/// 앱의 입력기 하나가 쓰는 호스트 연결과 지금 입력칸의 사정(변환기와 텍스트 서비스가 같이 쥔다).
pub struct Link {
    pub client: RefCell<Client>,
    /// 지금 입력칸이 개인 입력칸(시크릿 창 등)이다: 학습하지 않는다. 텍스트 서비스가 입력칸을 가릴 때 적는다.
    pub private: Cell<bool>,
}

pub type HostLink = Rc<Link>;

/// 호스트 연결을 만든다(아직 잇지는 않는다). 앱 컨테이너·관리자 권한 앱은 호스트를 띄우지 않고, 떠 있는 것만 쓴다.
pub fn link() -> Option<HostLink> {
    let sid = user_sid()?;
    let host = if may_spawn_host() { host_exe() } else { None };
    Some(Rc::new(Link {
        client: RefCell::new(Client::new(pipe_name(&sid, ""), host)),
        private: Cell::new(false),
    }))
}

/// 미리 잇는다(호스트가 없으면 띄우기만 한다).
pub fn prepare(link: &HostLink) {
    if let Ok(mut client) = link.client.try_borrow_mut()
        && !client.connected()
    {
        client.prepare();
        if client.connected() {
            debug_log(&format!("host connected, engine {:?}", client.engine()));
        }
    }
}

/// 호스트와 설정 파일·한자 기억의 판을 맞춘다. 가진 판(설정, 한자 기억)과 다른 것만 돌아온다. 연결이 없으면 잇는다
/// (호스트가 없으면 띄우기만 하고 기다리지 않는다: 늘 켜 두는 호스트라 보통은 떠 있다).
pub fn sync(link: &HostLink, known: (u64, u64)) -> Option<(Option<Versioned>, Option<Versioned>)> {
    let mut client = link.client.try_borrow_mut().ok()?;
    if !client.connected() {
        client.prepare();
    }
    match client.call_if_connected(&Request::Sync { config: known.0, learning: known.1 }, SYNC_TIMEOUT)? {
        Reply::Sync { config, learning } => Some((config, learning)),
        _ => None,
    }
}

/// 한자를 하나 골랐다고 호스트에 알린다(호스트가 기억에 더하고 저장한다). 개인 입력칸이면 알리지 않는다.
pub fn report_pick(link: &HostLink, reading: &str, text: &str) {
    if link.private.get() {
        return;
    }
    if let Ok(mut client) = link.client.try_borrow_mut() {
        let request = Request::HanjaPicked { reading: reading.into(), text: text.into() };
        let _ = client.call_if_connected(&request, COMMAND_TIMEOUT);
    }
}

/// 메뉴에서 연 설정 앱·다시 시작: 호스트가 처음 뜰 수도 있어서 넉넉히(그동안 메뉴를 고른 앱만 잠깐 기다린다).
const MENU_TIMEOUT: Duration = Duration::from_secs(2);

/// 설정 앱을 연다(작업 표시줄 아이콘 메뉴). 호스트가 띄운다: 이 앱이 앱 컨테이너면 설정 앱이 그 안에 갇히고, 관리자 권한
/// 앱이면 설정 앱도 관리자가 된다. 호스트에 닿지 못하면 띄워도 되는 앱에서만 직접 띄운다.
pub fn open_settings(link: Option<&HostLink>, tab: Option<&str>) {
    // 호스트가 띄운 설정 창이 앞으로 올 수 있게, 지금 앞에 있는(메뉴를 고른) 이 앱이 허락해 둔다.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    let request = Request::OpenSettings { tab: tab.map(str::to_string) };
    let asked = link
        .and_then(|l| l.client.try_borrow_mut().ok())
        .map(|mut client| client.call(&request, MENU_TIMEOUT));
    if let Some(Ok(Reply::Done)) = asked {
        return;
    }
    debug_log(&format!("open settings through the host: {:?}", asked.as_ref().map(kind)));
    if !may_spawn_host() {
        return;
    }
    let Some(exe) = beside_dll(&["settings", "cssgsg-settings.exe"]) else { return };
    let mut command = std::process::Command::new(exe);
    if let Some(tab) = tab {
        command.args(["--tab", tab]);
    }
    if let Err(e) = command.spawn() {
        debug_log(&format!("open settings: {e}"));
    }
}

/// 엔진 호스트를 다시 띄운다(메뉴의 "다시 시작"). 떠 있지 않았으면 띄우기만 한다.
pub fn restart(link: Option<&HostLink>) {
    let Some(mut client) = link.and_then(|l| l.client.try_borrow_mut().ok()) else { return };
    match client.call(&Request::Restart, MENU_TIMEOUT) {
        Ok(Reply::Done) => debug_log("host restarting"),
        other => {
            debug_log(&format!("host restart: {}", kind(&other)));
            client.prepare();
        }
    }
}

/// 이 DLL 옆의 cssgsg-host.exe.
fn host_exe() -> Option<PathBuf> {
    beside_dll(&["cssgsg-host.exe"])
}

/// 이 DLL이 있는 폴더(설치 폴더) 아래의 파일.
fn beside_dll(parts: &[&str]) -> Option<PathBuf> {
    let mut buffer = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(module()), &mut buffer) } as usize;
    if n == 0 || n >= buffer.len() {
        return None;
    }
    let dll = PathBuf::from(String::from_utf16_lossy(&buffer[..n]));
    let mut path = dll.parent()?.to_path_buf();
    for part in parts {
        path.push(part);
    }
    Some(path)
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
        Ok(Reply::Sync { .. }) => "sync".into(),
        Ok(Reply::Engine { .. }) => "engine".into(),
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
        match self.link.client.try_borrow_mut() {
            Ok(mut client) => client.call(request, timeout),
            Err(_) => Err(CallError::Broken("busy".into())),
        }
    }
}

impl Converter for HostConverter {
    fn start(&mut self, reading: &str) -> Option<ConvView> {
        let learn = !self.link.private.get();
        let reply = self.call(&Request::Start { reading: reading.into(), learn }, START_TIMEOUT);
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
            && let Ok(mut client) = self.link.client.try_borrow_mut()
        {
            let _ = client.call_if_connected(&Request::Cancel, COMMAND_TIMEOUT);
        }
    }

    fn reload(&mut self) {
        if let Ok(mut client) = self.link.client.try_borrow_mut() {
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
        let client = Client::new(r"\\.\pipe\cssgsg-engine-test-nobody".into(), None);
        let link = Rc::new(Link { client: RefCell::new(client), private: Cell::new(false) });
        let mut c = HostConverter::new(link.clone());
        let began = Instant::now();
        let view = c.start("かな").unwrap();
        assert!(began.elapsed() < Duration::from_millis(200), "키마다 앱을 멈추지 않는다");
        assert_eq!(view.candidates, ["かな", "カナ"]);
        assert_eq!(c.command(ConvCmd::Next).unwrap().segments, ["カナ"]);
        assert_eq!(c.commit(), "カナ");
        assert_eq!(c.start("かな").unwrap().segments, ["かな"]);
        c.cancel();
        assert_eq!(c.commit(), "", "취소한 뒤에는 확정할 것이 없다");
        // 맞추기와 한자 알리기도 기다리지 않고 지나간다.
        let began = Instant::now();
        assert_eq!(sync(&link, (0, 0)), None);
        report_pick(&link, "한", "漢");
        assert!(began.elapsed() < Duration::from_millis(200));
    }
}
