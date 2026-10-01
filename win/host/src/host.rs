//! 엔진 호스트 본체. 파이프 하나에 입력기마다 스레드 하나, 변환 엔진은 엔진 스레드 하나가 쥔다
//! (Mozc는 한 인스턴스를 한 스레드에서만 쓴다, mozc/README.md).
//!
//! 변환은 한 번에 하나다(포커스는 한 곳). 입력기 A가 변환하는 중에 입력기 B가 Start를 보내면 A의 변환은 버리고,
//! A가 그 뒤에 보내는 명령에는 [`Reply::Lost`]로 답한다(A는 보이던 글자를 그대로 확정한다).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use cssgsg_core::convert::{Converter, EchoConverter};
use cssgsg_core::mozc::MozcConverter;
use cssgsg_ipc::pipe::{pipe_name, pipe_sddl, user_sid};
use cssgsg_ipc::{PROTOCOL, Reply, Request, decode, encode};

use crate::files::UserFiles;

/// 파이프 인스턴스의 버퍼 크기(요청은 작다). 답은 이보다 커도 된다: 메시지 모드라 입력기가 이어 읽는다.
const PIPE_BUFFER: u32 = 64 * 1024;
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::core::HSTRING;

struct Options {
    tag: String,
    engine_dir: Option<PathBuf>,
    profile: Option<PathBuf>,
    no_engine: bool,
    /// 이어진 입력기가 하나도 없이 이만큼 지나면 끝난다. 보통은 None: 로그인부터 늘 켜 둔다(시험만 준다).
    idle_exit: Option<Duration>,
    /// 설정·한자 기억 폴더(보통 %APPDATA%\cssgsg, 시험은 따로).
    user_dir: Option<PathBuf>,
}

fn options() -> Options {
    let mut o = Options {
        tag: String::new(),
        engine_dir: None,
        profile: None,
        no_engine: false,
        idle_exit: None,
        user_dir: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--tag" => o.tag = args.next().unwrap_or_default(),
            "--engine-dir" => o.engine_dir = args.next().map(PathBuf::from),
            "--profile" => o.profile = args.next().map(PathBuf::from),
            "--no-engine" => o.no_engine = true,
            "--user-dir" => o.user_dir = args.next().map(PathBuf::from),
            "--idle-exit-secs" => {
                o.idle_exit = args.next().and_then(|s| s.parse().ok()).map(Duration::from_secs);
            }
            _ => log(&format!("unknown option {a}")),
        }
    }
    o
}

pub fn main() {
    // 설치기가 부르는 사용자 쪽 일(setup.rs).
    match std::env::args().nth(1).as_deref() {
        Some("--install-user") => std::process::exit(crate::setup::install_user()),
        Some("--uninstall-user") => std::process::exit(crate::setup::uninstall_user()),
        Some("--quit") => std::process::exit(crate::setup::quit()),
        _ => {}
    }
    let options = options();
    let Some(sid) = user_sid() else {
        log("no user SID");
        return;
    };
    let name = HSTRING::from(pipe_name(&sid, &options.tag));
    let Some(security) = Security::new(&pipe_sddl(&sid)) else {
        log("pipe security descriptor failed");
        return;
    };
    // 첫 인스턴스는 FILE_FLAG_FIRST_PIPE_INSTANCE로 만든다: 이미 떠 있는 호스트가 있으면 실패하고 끝난다(사용자당 하나).
    let Some(first) = create_instance(&name, &security, true) else {
        log("another host owns the pipe; exiting");
        return;
    };
    log(&format!("host started (protocol {PROTOCOL})"));

    let (jobs, inbox) = mpsc::channel::<Job>();
    let clients = Arc::new(AtomicUsize::new(0));
    {
        let clients = clients.clone();
        let idle_exit = options.idle_exit;
        std::thread::spawn(move || engine(inbox, &options, &clients, idle_exit));
    }

    let mut next = Some(first);
    let mut id = 0u64;
    loop {
        let Some(pipe) = next.take().or_else(|| create_instance(&name, &security, false)) else {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };
        let connected = match unsafe { ConnectNamedPipe(pipe, None) } {
            Ok(()) => true,
            Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
        };
        if !connected {
            let _ = unsafe { CloseHandle(pipe) };
            continue;
        }
        id += 1;
        let (jobs, clients) = (jobs.clone(), clients.clone());
        let pipe = Pipe(pipe);
        std::thread::spawn(move || serve(pipe, id, &jobs, &clients));
    }
}

/// 파이프 보안 기술자(SDDL에서 만든다).
struct Security {
    descriptor: PSECURITY_DESCRIPTOR,
}

impl Security {
    fn new(sddl: &str) -> Option<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &HSTRING::from(sddl),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .ok()?;
        Some(Self { descriptor })
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor.0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        unsafe { LocalFree(Some(HLOCAL(self.descriptor.0))) };
    }
}

fn create_instance(name: &HSTRING, security: &Security, first: bool) -> Option<HANDLE> {
    let open =
        PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { FILE_FLAGS_AND_ATTRIBUTES(0) };
    let attributes = security.attributes();
    let pipe = unsafe {
        CreateNamedPipeW(
            name,
            open,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            PIPE_BUFFER,
            PIPE_BUFFER,
            0,
            Some(&attributes),
        )
    };
    (!pipe.is_invalid()).then_some(pipe)
}

/// 스레드로 넘기는 파이프 핸들.
struct Pipe(HANDLE);
// SAFETY: 파이프 핸들은 커널 개체 번호다. 넘긴 뒤에는 받은 스레드만 쓴다.
unsafe impl Send for Pipe {}

enum Job {
    Call {
        client: u64,
        request: Request,
        reply: Sender<Reply>,
    },
    /// 입력기가 떠났다(앱이 끝났다 등). 그 입력기의 변환이 남았으면 버린다.
    Gone(u64),
}

/// 입력기 하나와 주고받는다: 요청 메시지를 읽어 엔진 스레드에 넘기고 답을 쓴다.
fn serve(pipe: Pipe, client: u64, jobs: &Sender<Job>, clients: &AtomicUsize) {
    let pipe = pipe.0;
    clients.fetch_add(1, Ordering::SeqCst);
    let mut buffer = vec![0u8; PIPE_BUFFER as usize];
    loop {
        let mut read = 0u32;
        if unsafe { ReadFile(pipe, Some(&mut buffer), Some(&mut read), None) }.is_err() {
            // 입력기가 끊었거나, 메시지가 한도보다 크다(ERROR_MORE_DATA): 어느 쪽이든 이 연결은 끝낸다.
            break;
        }
        let reply = match decode::<Request>(&buffer[..read as usize]) {
            Some(request) => {
                let (tx, rx) = mpsc::channel();
                if jobs.send(Job::Call { client, request, reply: tx }).is_err() {
                    break;
                }
                rx.recv().unwrap_or(Reply::Error { message: "engine stopped".into() })
            }
            None => Reply::Error { message: "bad request".into() },
        };
        let bytes = encode(&reply);
        let mut written = 0u32;
        if unsafe { WriteFile(pipe, Some(&bytes), Some(&mut written), None) }.is_err() {
            break;
        }
    }
    let _ = jobs.send(Job::Gone(client));
    unsafe {
        let _ = DisconnectNamedPipe(pipe);
        let _ = CloseHandle(pipe);
    }
    clients.fetch_sub(1, Ordering::SeqCst);
}

/// 엔진 스레드: 변환 엔진을 만들고 요청을 차례로 처리한다. `idle_exit`을 주면(시험) 이어진 입력기가 없이 그만큼 지났을 때
/// 프로세스를 끝낸다.
fn engine(inbox: Receiver<Job>, options: &Options, clients: &AtomicUsize, idle_exit: Option<Duration>) {
    let (mut converter, mut version) = load_engine(options);
    let mut files = UserFiles::open(options.user_dir.clone().unwrap_or_else(default_user_dir));
    let mut owner: Option<u64> = None;
    let mut last = Instant::now();
    loop {
        match inbox.recv_timeout(Duration::from_secs(1)) {
            Ok(Job::Call { client, request, reply }) => {
                last = Instant::now();
                if request == Request::Quit {
                    // 설치기가 파일을 바꾸려 한다: 한자 기억을 저장하고 엔진을 내려 학습을 마무리하고, 답이 입력기에 닿을 틈을 두고 끝낸다.
                    let _ = reply.send(Reply::Done);
                    log("asked to quit");
                    files.save(true);
                    drop(converter);
                    std::thread::sleep(Duration::from_millis(100));
                    std::process::exit(0);
                }
                let answer = match request {
                    Request::Quit => Reply::Done,
                    Request::Hello { .. } => Reply::Hello { protocol: PROTOCOL, engine: version.clone() },
                    Request::Start { reading, learn } => {
                        if owner.is_some() {
                            converter.cancel();
                        }
                        owner = Some(client);
                        converter.set_learning(learn);
                        Reply::View { view: converter.start(&reading) }
                    }
                    Request::Sync { config, learning } => files.sync(config, learning),
                    Request::HanjaPicked { reading, text } => {
                        files.picked(&reading, &text);
                        Reply::Done
                    }
                    Request::Command { cmd } if owner == Some(client) => {
                        Reply::View { view: converter.command(cmd) }
                    }
                    Request::Commit if owner == Some(client) => {
                        owner = None;
                        Reply::Text { text: converter.commit() }
                    }
                    Request::Cancel => {
                        if owner == Some(client) {
                            owner = None;
                            converter.cancel();
                        }
                        Reply::Done
                    }
                    Request::Command { .. } | Request::Commit => Reply::Lost,
                    Request::Reload => {
                        converter.reload();
                        Reply::Done
                    }
                    Request::ClearHanjaLearning => {
                        files.clear_learning();
                        log("hanja learning cleared");
                        Reply::Done
                    }
                    Request::ClearMozcLearning => {
                        // 엔진이 학습 파일을 열어 두고 있어서(윈도우는 연 파일을 지울 수 없다) 먼저 내린다.
                        // 하던 변환은 버린다(그 입력기의 다음 명령은 Lost: 보이던 글자를 그대로 확정한다).
                        owner = None;
                        drop(std::mem::replace(&mut converter, Box::new(EchoConverter::default())));
                        let cleared = clear_mozc_learning(&profile_dir(options));
                        (converter, version) = load_engine(options);
                        match cleared {
                            Ok(n) => {
                                log(&format!("Mozc learning cleared ({n} files)"));
                                Reply::Done
                            }
                            Err(message) => {
                                log(&format!("Mozc learning: {message}"));
                                Reply::Error { message }
                            }
                        }
                    }
                };
                let _ = reply.send(answer);
            }
            Ok(Job::Gone(client)) => {
                if owner == Some(client) {
                    owner = None;
                    converter.cancel();
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                files.save(false);
                if let Some(limit) = idle_exit
                    && clients.load(Ordering::SeqCst) == 0
                    && last.elapsed() >= limit
                {
                    log("idle; exiting");
                    // 한자 기억을 저장하고 엔진을 내려 학습을 마무리한 뒤 끝낸다.
                    files.save(true);
                    drop(converter);
                    std::process::exit(0);
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if clients.load(Ordering::SeqCst) > 0 {
            last = Instant::now();
        }
    }
}

/// 변환 엔진: Mozc(이 실행 파일 옆 mozc 폴더)를 읽고, 못 읽으면 히라가나·가타카나만 내는 변환기.
fn load_engine(options: &Options) -> (Box<dyn Converter>, Option<String>) {
    if options.no_engine {
        return (Box::new(EchoConverter::default()), None);
    }
    let dir = options.engine_dir.clone().unwrap_or_else(|| exe_dir().join("mozc"));
    let profile = profile_dir(options);
    // Mozc는 프로필 폴더를 만들지 않고, 비우면 upstream Mozc의 폴더(%LOCALAPPDATA%\Mozc)를 쓴다.
    if let Err(e) = std::fs::create_dir_all(&profile) {
        log(&format!("profile folder: {e}"));
    }
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let started = Instant::now();
    match MozcConverter::load(
        &path(&dir.join("cssgsg_mozc.dll")),
        &path(&dir.join("mozc.data")),
        &path(&profile),
    ) {
        Ok(mozc) => {
            let version = mozc.version();
            log(&format!("Mozc {version} ready in {} ms", started.elapsed().as_millis()));
            (Box::new(mozc), Some(version))
        }
        Err(e) => {
            log(&format!("Mozc unavailable ({e}); kana only"));
            (Box::new(EchoConverter::default()), None)
        }
    }
}

/// Mozc가 배운 것을 적는 파일(맥 설정 앱이 지우는 것과 같다): 문절 나누기, 고른 후보, 전각·반각, 추천 기록.
/// 사용자 사전(user_dictionary.db)과 Mozc 설정(config1.db)은 남긴다.
const MOZC_LEARNING: [&str; 4] = ["segment.db", "boundary.db", "cform.db", ".history.db"];

/// 학습 파일을 지운다(엔진을 내린 뒤). 지운 수, 못 지운 파일이 있으면 까닭.
fn clear_mozc_learning(profile: &Path) -> Result<usize, String> {
    let mut removed = 0;
    for name in MOZC_LEARNING {
        match std::fs::remove_file(profile.join(name)) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{name}: {e}")),
        }
    }
    Ok(removed)
}

fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default()
}

/// Mozc 학습·사용자 사전 폴더(`--profile`, 보통 %LOCALAPPDATA%\cssgsg\mozc).
fn profile_dir(options: &Options) -> PathBuf {
    options.profile.clone().unwrap_or_else(default_profile)
}

fn default_profile() -> PathBuf {
    let base =
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| exe_dir().join("profile"));
    base.join("cssgsg").join("mozc")
}

/// 설정·한자 기억 폴더: %APPDATA%\cssgsg(맥의 ~/Library/Application Support/cssgsg).
fn default_user_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| exe_dir().join("profile"));
    base.join("cssgsg")
}

/// 개발자 기록(HKCU\Software\cssgsg DebugLog=1): 입력기와 같은 파일에 남긴다. 읽기·글자는 남기지 않는다.
pub(crate) fn log(message: &str) {
    use std::io::Write;
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let on = *ON.get_or_init(|| {
        windows_registry::CURRENT_USER
            .open("Software\\cssgsg")
            .and_then(|k| k.get_u32("DebugLog"))
            .is_ok_and(|v| v != 0)
    });
    if !on {
        return;
    }
    let Some(base) = std::env::var_os("LOCALAPPDATA") else { return };
    let dir = Path::new(&base).join("cssgsg");
    let _ = std::fs::create_dir_all(&dir);
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("tip-debug.log"))
    else {
        return;
    };
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let _ = writeln!(
        file,
        "{:02}:{:02}:{:02}.{:03} cssgsg-host.exe[{}] {message}",
        t.wHour,
        t.wMinute,
        t.wSecond,
        t.wMilliseconds,
        std::process::id()
    );
}
