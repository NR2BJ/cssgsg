//! 파이프 이름·보안과 입력기 쪽 연결([`Client`]). 호스트 쪽(파이프 만들기·받기)은 cssgsg-host가 이 이름과 보안으로 한다.
//!
//! - 이름은 사용자 SID로 가른다(`\\.\pipe\cssgsg-engine-<SID>`). 한 사용자의 세션이 여럿이어도 호스트는 하나다.
//! - 보안: 이 사용자·SYSTEM·앱 컨테이너(ALL APPLICATION PACKAGES)만, 낮은 무결성(앱 컨테이너)도 쓸 수 있다(Mozc와 같다).
//!   입력기는 연결한 뒤 파이프 주인이 이 사용자인지 본다(다른 사용자가 이름을 먼저 차지했을 때).
//! - 입력기는 앱의 입력 스레드에서 부른다. 그래서 모든 호출에 시간 제한이 있고, 넘으면 연결을 끊는다(늦게 온 답이 다음 답과
//!   섞이지 않게). 호스트가 없으면 띄운다: 앱 컨테이너·관리자 권한 앱에서는 띄우지 않는다([`may_spawn_host`]).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, ERROR_PIPE_BUSY, ERROR_SUCCESS, GENERIC_READ, GENERIC_WRITE, HANDLE,
    HLOCAL, LocalFree, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_KERNEL_OBJECT};
use windows::Win32::Security::{
    EqualSid, GetTokenInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_ELEVATION,
    TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenIsAppContainer, TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING, SECURITY_IDENTIFICATION,
    SECURITY_SQOS_PRESENT,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Pipes::{
    PIPE_READMODE_MESSAGE, SetNamedPipeHandleState, TransactNamedPipe, WaitNamedPipeW,
};
use windows::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CreateEventW, CreateProcessW, GetCurrentProcess, OpenProcessToken,
    PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW, WaitForSingleObject,
};
use windows::core::{HSTRING, PWSTR};

use crate::{MAX_MESSAGE, PROTOCOL, Reply, Request, decode, encode};

/// 이 프로세스 토큰의 정보 하나. 버퍼는 u64로 잡아 구조체 정렬을 맞춘다.
fn token_info(class: TOKEN_INFORMATION_CLASS) -> Option<Vec<u64>> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, class, None, 0, &mut len);
        let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
        let read = GetTokenInformation(
            token,
            class,
            Some(buf.as_mut_ptr().cast()),
            (buf.len() * 8) as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        read.ok()?;
        Some(buf)
    }
}

/// 지금 사용자의 SID 문자열(S-1-5-21-…).
pub fn user_sid() -> Option<String> {
    let buf = token_info(TokenUser)?;
    // SAFETY: TokenUser 버퍼는 TOKEN_USER로 시작하고, SID는 같은 버퍼 안을 가리킨다.
    let user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }.ok()?;
    let sid = unsafe { text.to_string() }.ok();
    unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    sid
}

/// 엔진 호스트 파이프 이름. `tag`는 시험이 진짜 호스트와 겹치지 않게 붙인다(보통 빈 문자열).
pub fn pipe_name(sid: &str, tag: &str) -> String {
    format!(r"\\.\pipe\cssgsg-engine-{sid}{tag}")
}

/// 호스트가 파이프에 거는 보안(SDDL). 앱 컨테이너 안의 입력기는 사용자 SID와 컨테이너 쪽(AC) 검사를 둘 다 통과해야 해서
/// 둘 다 준다. 무결성 표시는 낮음(LW)이라 앱 컨테이너(낮은 무결성)도 쓸 수 있다.
pub fn pipe_sddl(sid: &str) -> String {
    format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})(A;;GA;;;AC)S:(ML;;NW;;;LW)")
}

/// 이 프로세스가 호스트를 띄워도 되는지. 앱 컨테이너(스토어 앱, 시작 메뉴 검색) 안에서 띄우면 호스트가 그 컨테이너를
/// 물려받아 학습 폴더에 쓰지 못하고, 관리자 권한 앱에서 띄우면 호스트도 관리자가 되어 보통 앱이 쓰지 못한다
/// (파이프 주인이 Administrators가 된다). 그런 앱은 이미 떠 있는 호스트만 쓴다.
pub fn may_spawn_host() -> bool {
    let container = token_info(TokenIsAppContainer).is_none_or(|b| b[0] as u32 != 0);
    // SAFETY: TokenElevation 버퍼는 TOKEN_ELEVATION이다.
    let elevated = token_info(TokenElevation)
        .is_none_or(|b| unsafe { (*(b.as_ptr() as *const TOKEN_ELEVATION)).TokenIsElevated } != 0);
    !container && !elevated
}

/// 호출이 안 된 까닭.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallError {
    /// 호스트가 없다(아직 뜨는 중이거나, 띄울 수 없는 앱).
    NoHost,
    /// 쓰면 안 되는 호스트다(다른 사용자가 만든 파이프, 다른 판).
    Refused(String),
    /// 정해진 시간 안에 답이 없었다. 연결은 끊었다.
    Timeout,
    /// 연결이 끊겼다(호스트가 죽었다 등).
    Broken(String),
}

/// 입력기 쪽 연결. 앱의 입력 스레드 하나에서만 쓴다.
pub struct Client {
    name: String,
    /// 호스트 실행 파일. None이면 띄우지 않는다([`may_spawn_host`]).
    host: Option<PathBuf>,
    pipe: Option<HANDLE>,
    event: Option<HANDLE>,
    buffer: Vec<u8>,
    /// 마지막으로 띄워 본 때(10초에 한 번까지).
    tried: Option<Instant>,
    /// 마지막으로 띄운 때(그 뒤 잠깐은 파이프가 생기기를 기다린다).
    spawned: Option<Instant>,
    engine: Option<String>,
}

impl Client {
    pub fn new(name: String, host: Option<PathBuf>) -> Self {
        Self {
            name,
            host,
            pipe: None,
            event: None,
            buffer: Vec::new(),
            tried: None,
            spawned: None,
            engine: None,
        }
    }

    /// 연결된 호스트의 변환 엔진(Mozc 버전). 연결 전이거나 엔진이 없으면 None.
    pub fn engine(&self) -> Option<&str> {
        self.engine.as_deref()
    }

    pub fn connected(&self) -> bool {
        self.pipe.is_some()
    }

    /// 미리 잇는다(일본어 모드로 바꿀 때). 호스트가 없으면 띄우기만 하고 기다리지 않는다.
    pub fn prepare(&mut self) {
        let _ = self.connect(Duration::ZERO);
    }

    /// 요청 하나를 보내고 답을 받는다. 연결이 없으면 잇고(호스트가 없으면 띄운 뒤 기다린다), 연결과 답 모두 `timeout` 안에.
    pub fn call(&mut self, request: &Request, timeout: Duration) -> Result<Reply, CallError> {
        let deadline = Instant::now() + timeout;
        let was_connected = self.pipe.is_some();
        self.connect(timeout)?;
        match self.exchange(request, deadline) {
            // 쥐고 있던 연결이 끊겼다: 호스트가 다시 떴을 수 있다(업데이트, 다시 시작). 새로 이어 한 번만 더 보낸다.
            Err(CallError::Broken(_)) if was_connected => {
                self.connect(deadline.saturating_duration_since(Instant::now()))?;
                self.exchange(request, deadline)
            }
            other => other,
        }
    }

    /// 이미 연결되어 있을 때만 보낸다(취소·사전 다시 읽기: 이것 때문에 호스트를 띄우지 않는다).
    pub fn call_if_connected(&mut self, request: &Request, timeout: Duration) -> Option<Reply> {
        self.pipe?;
        self.exchange(request, Instant::now() + timeout).ok()
    }

    fn exchange(&mut self, request: &Request, deadline: Instant) -> Result<Reply, CallError> {
        let bytes = self.transact(&encode(request), deadline)?;
        decode(&bytes).ok_or_else(|| {
            self.disconnect();
            CallError::Broken("reply is not a message".into())
        })
    }

    fn connect(&mut self, wait: Duration) -> Result<(), CallError> {
        if self.pipe.is_some() {
            return Ok(());
        }
        let deadline = Instant::now() + wait;
        let name = HSTRING::from(self.name.as_str());
        let pipe = loop {
            let opened = unsafe {
                CreateFileW(
                    &name,
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    None,
                )
            };
            match opened {
                Ok(pipe) => break pipe,
                Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() => {
                    // 인스턴스가 모두 다른 입력기와 이어져 있다: 호스트가 새로 만들 때까지 잠깐.
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(CallError::NoHost);
                    }
                    let _ = unsafe { WaitNamedPipeW(&name, left.as_millis().clamp(1, 1000) as u32) };
                }
                Err(_) => {
                    // 파이프가 없다: 호스트를 띄운다(조금 전에 해 봤으면 말고). 막 띄운 호스트만 파이프가 생길 때까지 기다린다.
                    // 띄울 수 없는 앱이거나 띄우지 못했으면 바로 포기한다(키마다 앱을 멈추지 않게).
                    self.spawn();
                    let coming = self.spawned.is_some_and(|t| t.elapsed() < Duration::from_secs(3));
                    if !coming || Instant::now() >= deadline {
                        return Err(CallError::NoHost);
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
            }
        };
        self.pipe = Some(pipe);
        let ready = unsafe { SetNamedPipeHandleState(pipe, Some(&PIPE_READMODE_MESSAGE), None, None) }
            .map_err(|e| CallError::Broken(format!("message mode: {e}")))
            .and_then(|()| {
                if owned_by_this_user(pipe) { Ok(()) } else { Err(CallError::Refused("pipe owner".into())) }
            })
            .and_then(|()| {
                let deadline = Instant::now() + wait.max(Duration::from_millis(500));
                match self.exchange(&Request::Hello { protocol: PROTOCOL }, deadline)? {
                    Reply::Hello { protocol, engine } if protocol == PROTOCOL => {
                        self.engine = engine;
                        Ok(())
                    }
                    other => Err(CallError::Refused(format!("hello: {other:?}"))),
                }
            });
        if ready.is_err() {
            self.disconnect();
        }
        ready
    }

    /// 요청 메시지 하나를 쓰고 답 메시지 하나를 읽는다(TransactNamedPipe). `deadline`을 넘으면 취소하고 끊는다.
    fn transact(&mut self, request: &[u8], deadline: Instant) -> Result<Vec<u8>, CallError> {
        let pipe = self.pipe.ok_or(CallError::NoHost)?;
        let event = match self.event {
            Some(event) => event,
            None => {
                let event = unsafe { CreateEventW(None, true, false, None) }
                    .map_err(|e| CallError::Broken(format!("event: {e}")))?;
                self.event = Some(event);
                event
            }
        };
        if self.buffer.len() < MAX_MESSAGE {
            self.buffer.resize(MAX_MESSAGE, 0);
        }
        let mut overlapped = OVERLAPPED { hEvent: event, ..Default::default() };
        let mut read = 0u32;
        // SAFETY: 버퍼와 OVERLAPPED는 이 함수가 끝나기 전에(완료되거나 취소가 끝난 뒤) 놓지 않는다.
        let started = unsafe {
            TransactNamedPipe(
                pipe,
                Some(request.as_ptr().cast()),
                request.len() as u32,
                Some(self.buffer.as_mut_ptr().cast()),
                self.buffer.len() as u32,
                &mut read,
                Some(&mut overlapped),
            )
        };
        if let Err(e) = started
            && e.code() != ERROR_IO_PENDING.to_hresult()
        {
            self.disconnect();
            return Err(CallError::Broken(format!("transact: {e}")));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        let waited = unsafe { WaitForSingleObject(event, left.as_millis().min(u32::MAX as u128 - 1) as u32) };
        if waited != WAIT_OBJECT_0 {
            // 시간이 다 됐다: 취소하고, 버퍼를 놓기 전에 취소가 끝나기를 기다린다.
            unsafe {
                let _ = CancelIoEx(pipe, Some(&overlapped));
                let _ = GetOverlappedResult(pipe, &overlapped, &mut read, true);
            }
            self.disconnect();
            return Err(CallError::Timeout);
        }
        if let Err(e) = unsafe { GetOverlappedResult(pipe, &overlapped, &mut read, false) } {
            // 답이 버퍼보다 크거나(ERROR_MORE_DATA, 메시지 한도를 넘었다) 파이프가 끊겼다.
            self.disconnect();
            return Err(CallError::Broken(format!("reply: {e}")));
        }
        Ok(self.buffer[..read as usize].to_vec())
    }

    fn spawn(&mut self) {
        let Some(exe) = self.host.as_deref() else { return };
        if self.tried.is_some_and(|t| t.elapsed() < Duration::from_secs(10)) {
            return;
        }
        self.tried = Some(Instant::now());
        // 앱의 작업 개체에서 떨어져 나가게 띄운다(앱이 끝날 때 같이 끝나지 않게). 작업 개체가 허락하지 않으면 그대로.
        let launched = spawn_process(exe, CREATE_BREAKAWAY_FROM_JOB)
            .or_else(|_| spawn_process(exe, PROCESS_CREATION_FLAGS(0)));
        if launched.is_ok() {
            self.spawned = Some(Instant::now());
        }
    }

    fn disconnect(&mut self) {
        if let Some(pipe) = self.pipe.take() {
            let _ = unsafe { CloseHandle(pipe) };
        }
        self.engine = None;
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.disconnect();
        if let Some(event) = self.event.take() {
            let _ = unsafe { CloseHandle(event) };
        }
    }
}

fn spawn_process(exe: &Path, flags: PROCESS_CREATION_FLAGS) -> windows::core::Result<()> {
    let app = HSTRING::from(exe.as_os_str());
    let mut command: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain(Some(0)).collect();
    let dir = HSTRING::from(exe.parent().unwrap_or(exe).as_os_str());
    let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..Default::default() };
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            &app,
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            flags,
            None,
            &dir,
            &startup,
            &mut info,
        )?;
        let _ = CloseHandle(info.hThread);
        let _ = CloseHandle(info.hProcess);
    }
    Ok(())
}

/// 파이프 주인이 이 프로세스의 사용자인지.
fn owned_by_this_user(pipe: HANDLE) -> bool {
    let Some(me) = token_info(TokenUser) else { return false };
    // SAFETY: TokenUser 버퍼는 TOKEN_USER로 시작한다.
    let my_sid = unsafe { (*(me.as_ptr() as *const TOKEN_USER)).User.Sid };
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let status = unsafe {
        GetSecurityInfo(
            pipe,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            Some(&mut descriptor),
        )
    };
    if status != ERROR_SUCCESS {
        return false;
    }
    let same = unsafe { EqualSid(owner, my_sid) }.is_ok();
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    same
}
