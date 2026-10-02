//! 새 Mozc 엔진 받기(맥 MozcUpdater의 윈도우판). 엔진 호스트가 뜨고 5분 뒤, 그다음 하루 한 번(그리고 설정 앱의 "지금 확인")
//! GitHub 릴리스 목록을 보고, 이 C API 판의 더 새 엔진(`mozc-<판>-<yyyymmdd>-<커밋 7자리>[-w<래퍼 판>]` 태그의
//! `cssgsg-mozc-windows-x64.zip`)이 있으면 받는다. GitHub가 적은 SHA-256과 묶음 안 manifest.json의 파일별 해시를 확인하고
//! mozc-engines\<커밋>[-w<래퍼 판>]\에 둔다. 다음에 호스트가 뜰 때부터 쓴다(설정 앱 "지금 적용"은 호스트 다시 시작).
//! 엔진 고르기는 [`crate::engines`]. 래퍼 판 규칙은 맥 0.7.2와 같다(CONCEPT §6.3): 같은 커밋이면 래퍼 판이 큰 것이 새것이고,
//! 래퍼 판이 없는 옛 모양 태그는 같은 커밋이면 받지 않는다(받아 보아야 판을 안다).
//!
//! 보내는 것은 목록 요청과 파일 받기뿐이다(User-Agent에 개인 정보 없음). 네트워크는 윈도우 WinHTTP, 해시는 BCrypt,
//! 압축 풀기는 윈도우에 든 tar.exe(10 1803부터)를 쓴다.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cssgsg_ipc::{EngineBuild, EngineInfo};

use crate::engines::{self, DATA, Engine, LIBRARY, MANIFEST};
use crate::host::log;

const RELEASES: &str = "https://api.github.com/repos/NR2BJ/cssgsg/releases?per_page=100";
const TAG_PREFIX: &str = "mozc-";
pub const ASSET: &str = "cssgsg-mozc-windows-x64.zip";
const FIRST_CHECK: Duration = Duration::from_secs(5 * 60);
const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// 받을 묶음의 상한(지금 14MB).
const MAX_DOWNLOAD: usize = 200 * 1024 * 1024;
const STATUS: &str = "status.json";

/// 릴리스가 내놓은 엔진.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub abi: i32,
    pub date: String,
    pub commit_prefix: String,
    /// 태그의 래퍼 판. 래퍼 판이 없는 옛 모양 태그면 None(받아 보아야 manifest로 안다).
    pub wrapper: Option<u32>,
    pub url: String,
    pub digest: String,
}

/// 엔진 상태(설정 앱이 `EngineStatus`로 읽는다)와 "지금 확인" 신호.
pub struct Updater {
    pub info: Arc<Mutex<EngineInfo>>,
    wake: Sender<()>,
}

impl Updater {
    /// 확인 스레드를 띄운다. `installed`: 설치본·받은 엔진(새것을 가릴 때 견준다), `active`: 지금 쓰는 엔진.
    pub fn start(abi: i32, bundled_dir: PathBuf, active: Option<EngineBuild>, automatic: bool) -> Self {
        let info = Arc::new(Mutex::new(load_status(active)));
        let (wake, inbox) = std::sync::mpsc::channel();
        let shared = info.clone();
        std::thread::spawn(move || run(abi, &bundled_dir, &shared, &inbox, automatic));
        Self { info, wake }
    }

    /// 지금 확인한다(하루 한 번을 기다리지 않고).
    pub fn check_now(&self) {
        let _ = self.wake.send(());
    }

    pub fn snapshot(&self) -> EngineInfo {
        self.info.lock().map(|i| i.clone()).unwrap_or_default()
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 지난 확인 시각·실패를 다시 읽는다(호스트가 다시 떠도 설정 앱에 보이게). 기다리는 엔진은 폴더에서 다시 찾는다.
fn load_status(active: Option<EngineBuild>) -> EngineInfo {
    let saved: serde_json::Value = std::fs::read_to_string(engines::root().join(STATUS))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    EngineInfo {
        active,
        pending: None,
        checking: false,
        checked_at: saved["checked_at"].as_u64(),
        failed_at: saved["failed_at"].as_u64(),
        failure: saved["failure"].as_str().map(String::from),
    }
}

fn save_status(info: &EngineInfo) {
    let value = serde_json::json!({
        "checked_at": info.checked_at,
        "failed_at": info.failed_at,
        "failure": info.failure,
    });
    let _ = std::fs::create_dir_all(engines::root());
    let _ = std::fs::write(engines::root().join(STATUS), value.to_string());
}

fn run(abi: i32, bundled_dir: &Path, info: &Mutex<EngineInfo>, inbox: &Receiver<()>, automatic: bool) {
    // 받아 두고 아직 쓰지 않은 엔진(지난번에 받았다)을 기다리는 엔진으로 보인다.
    refresh_pending(abi, bundled_dir, info);
    let mut wait = if automatic { Some(FIRST_CHECK) } else { None };
    loop {
        let woke = match wait {
            Some(d) => inbox.recv_timeout(d),
            None => inbox.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        if woke == Err(RecvTimeoutError::Disconnected) {
            return;
        }
        if let Ok(mut i) = info.lock() {
            i.checking = true;
        }
        let result = check(abi, bundled_dir, info);
        if let Ok(mut i) = info.lock() {
            i.checking = false;
            match &result {
                Ok(outcome) => {
                    i.checked_at = Some(now());
                    i.failed_at = None;
                    i.failure = None;
                    log(&format!("engine update check: {outcome}"));
                }
                Err(reason) => {
                    i.failed_at = Some(now());
                    i.failure = Some(reason.clone());
                    log(&format!("engine update check failed: {reason}"));
                }
            }
            save_status(&i);
        }
        refresh_pending(abi, bundled_dir, info);
        wait = if automatic { Some(INTERVAL) } else { None };
    }
}

/// 받은 엔진 중 지금 쓰는 것보다 새것(다시 시작하면 쓸 것)을 기다리는 엔진으로 적는다.
fn refresh_pending(abi: i32, bundled_dir: &Path, info: &Mutex<EngineInfo>) {
    let first =
        engines::candidates(engines::bundled(bundled_dir), engines::downloaded(abi)).into_iter().next();
    if let Ok(mut i) = info.lock() {
        i.pending = match (&first, &i.active) {
            (Some(e), Some(active)) if engines::is_newer(&e.build, active) => Some(e.build.clone()),
            (Some(e), None) if e.build.downloaded => Some(e.build.clone()),
            _ => None,
        };
    }
}

/// 확인하고 새것이 있으면 받는다. 성공하면 한 줄 요약.
fn check(abi: i32, bundled_dir: &Path, info: &Mutex<EngineInfo>) -> Result<String, String> {
    let (status, body) = http::get(RELEASES, "application/vnd.github+json", 16 * 1024 * 1024)?;
    if status != 200 {
        return Err(format!("GitHub {status}"));
    }
    let releases: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let bad: Vec<(String, u32)> = engines::bad().iter().filter_map(|name| engines::identity(name)).collect();
    let Some(offer) = newest(&releases, abi, &bad) else {
        return Ok("no engine for this C API".into());
    };
    let mut installed: Vec<EngineBuild> = engines::downloaded(abi).into_iter().map(|e| e.build).collect();
    installed.extend(engines::bundled(bundled_dir).map(|e| e.build));
    if let Some(active) = info.lock().ok().and_then(|i| i.active.clone()) {
        installed.push(active);
    }
    if !wanted(&offer, &installed) {
        return Ok(format!("up to date ({} {} w{:?})", offer.date, offer.commit_prefix, offer.wrapper));
    }
    let engine = install(&offer, abi)?;
    // 지금 쓰는 것과 새로 받은 것만 남긴다(폴더 이름으로).
    let active = info.lock().ok().and_then(|i| i.active.clone());
    let keep: Vec<String> = [Some(&engine.build), active.as_ref()]
        .into_iter()
        .flatten()
        .map(|b| engines::directory_name(&b.commit.to_ascii_lowercase(), b.wrapper))
        .collect();
    engines::prune(&keep);
    Ok(format!(
        "downloaded {} ({}, {}, wrapper {})",
        engine.build.version,
        engine.build.date,
        &engine.build.commit[..7],
        engine.build.wrapper
    ))
}

/// 릴리스 하나 → 내놓은 엔진(태그 모양, 초안 아님, 윈도우 묶음과 GitHub 해시가 있을 때만).
/// 태그는 `mozc-<C API 판>-<yyyymmdd>-<커밋 7자리 이상>`, 래퍼 판이 있으면 뒤에 `-w<판>`(숫자 1~6자리).
pub fn offer(release: &serde_json::Value) -> Option<Offer> {
    if release["draft"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?.strip_prefix(TAG_PREFIX)?;
    let parts: Vec<&str> = tag.split('-').collect();
    let wrapper = match parts.len() {
        3 => None,
        4 => {
            let digits = parts[3].strip_prefix('w')?;
            if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some(digits.parse().ok()?)
        }
        _ => return None,
    };
    let abi: i32 = parts[0].parse().ok()?;
    let d = parts[1];
    if d.len() != 8 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let commit = parts[2];
    if commit.len() < 7 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let asset = release["assets"].as_array()?.iter().find(|a| a["name"].as_str() == Some(ASSET))?;
    let digest = asset["digest"].as_str()?;
    if !digest.to_ascii_lowercase().starts_with("sha256:") {
        return None;
    }
    Some(Offer {
        abi,
        date: format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]),
        commit_prefix: commit.to_ascii_lowercase(),
        wrapper,
        url: asset["browser_download_url"].as_str()?.into(),
        digest: digest.to_ascii_lowercase(),
    })
}

/// 이 판의 엔진 중 나쁜 엔진이 아닌 가장 새것(날짜, 같으면 래퍼 판). `bad`: 나쁜 엔진의 (커밋, 래퍼 판).
/// 래퍼 판을 모르는 옛 모양 태그는 같은 커밋의 나쁜 엔진이 있으면 건너뛴다(맥 newest).
pub fn newest(releases: &serde_json::Value, abi: i32, bad: &[(String, u32)]) -> Option<Offer> {
    releases
        .as_array()?
        .iter()
        .filter_map(offer)
        .filter(|o| {
            o.abi == abi
                && !bad.iter().any(|(commit, wrapper)| {
                    commit.starts_with(&o.commit_prefix) && o.wrapper.is_none_or(|w| w == *wrapper)
                })
        })
        .max_by(|a, b| (&a.date, a.wrapper.unwrap_or(0)).cmp(&(&b.date, b.wrapper.unwrap_or(0))))
}

/// 받을 만한가: 여기 있는 어느 것보다 새것이다. 같은 커밋이면 래퍼 판이 더 커야 한다(태그에 판이 없으면 같은 커밋은 받지 않는다,
/// 맥 isWanted).
pub fn wanted(offer: &Offer, installed: &[EngineBuild]) -> bool {
    installed.iter().all(|b| {
        if b.commit.to_ascii_lowercase().starts_with(&offer.commit_prefix) {
            offer.wrapper.is_some_and(|w| w > b.wrapper)
        } else {
            offer.date > b.date
        }
    })
}

/// 받아서 확인하고 mozc-engines\<커밋>[-w<래퍼 판>]\에 푼다.
fn install(offer: &Offer, abi: i32) -> Result<Engine, String> {
    let root = engines::root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let (status, zip) = http::get(&offer.url, "application/octet-stream", MAX_DOWNLOAD)?;
    if status != 200 {
        return Err(format!("download {status}"));
    }
    if format!("sha256:{}", sha256_hex(&zip)?) != offer.digest {
        return Err("the download's SHA-256 doesn't match GitHub's".into());
    }
    let staging = root.join(format!(".staging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = unpack_and_verify(&zip, &staging, offer, abi);
    let (commit, wrapper) = match result {
        Ok(identity) => identity,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let destination = root.join(engines::directory_name(&commit.to_ascii_lowercase(), wrapper));
    let _ = std::fs::remove_dir_all(&destination);
    std::fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
    engines::read_downloaded(&destination, abi).ok_or_else(|| "unpacked engine is incomplete".into())
}

/// 윈도우에 든 tar.exe(bsdtar). PATH의 다른 tar(Git의 GNU tar 등)는 zip을 모른다.
fn system_tar() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("tar.exe")
}

/// 묶음을 풀고 매니페스트(판, 커밋·날짜, 래퍼 판, 파일 해시)를 확인한다. (커밋, 래퍼 판)을 돌려준다.
fn unpack_and_verify(zip: &[u8], staging: &Path, offer: &Offer, abi: i32) -> Result<(String, u32), String> {
    let archive = staging.join("engine.zip");
    std::fs::write(&archive, zip).map_err(|e| e.to_string())?;
    let tar = system_tar();
    let status = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new(tar)
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(staging)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("tar: {e}"))?
    };
    let _ = std::fs::remove_file(&archive);
    if !status.success() {
        return Err(format!("tar exited with {status}"));
    }
    let manifest: serde_json::Value = std::fs::read_to_string(staging.join(MANIFEST))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("no manifest.json")?;
    if manifest["abi"].as_i64() != Some(i64::from(abi))
        || manifest["abi"].as_i64() != Some(i64::from(offer.abi))
    {
        return Err(format!("C API {:?}", manifest["abi"]));
    }
    let commit = manifest["commit"].as_str().unwrap_or_default().to_string();
    if !commit.to_ascii_lowercase().starts_with(&offer.commit_prefix)
        || manifest["date"].as_str() != Some(offer.date.as_str())
        || commit.is_empty()
        || !commit.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("the manifest's commit or date differs from the release".into());
    }
    // 래퍼 판을 적지 않던 묶음은 0. 태그에 판이 있으면 같아야 한다.
    let wrapper = match &manifest["wrapper"] {
        serde_json::Value::Null => 0,
        w => w
            .as_u64()
            .and_then(|w| u32::try_from(w).ok())
            .ok_or("the manifest's wrapper revision isn't a number")?,
    };
    if offer.wrapper.is_some_and(|expected| expected != wrapper) {
        return Err(format!(
            "the manifest's wrapper revision {wrapper} differs from the release ({:?})",
            offer.wrapper
        ));
    }
    for name in [LIBRARY, DATA] {
        let expected = manifest["files"][name].as_str().unwrap_or_default().to_ascii_lowercase();
        let data = std::fs::read(staging.join(name)).map_err(|e| format!("{name}: {e}"))?;
        if expected.is_empty() || sha256_hex(&data)? != expected {
            return Err(format!("{name}: SHA-256 doesn't match the manifest"));
        }
    }
    Ok((commit, wrapper))
}

/// SHA-256(16진 소문자). 윈도우 BCrypt.
pub fn sha256_hex(data: &[u8]) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
    let mut out = [0u8; 32];
    let status = unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out) };
    if status.is_err() {
        return Err(format!("BCryptHash {:#x}", status.0));
    }
    Ok(out.iter().map(|b| format!("{b:02x}")).collect())
}

/// 윈도우 WinHTTP로 GET(리디렉션을 따라간다, 프록시는 시스템 설정).
mod http {
    use std::ffi::c_void;

    use windows::Win32::Networking::WinHttp::{
        URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_FLAG_SECURE,
        WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
        WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen,
        WinHttpOpenRequest, WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData,
        WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
    };
    use windows::core::{HSTRING, PCWSTR, w};

    struct Handle(*mut c_void);

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                let _ = unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }

    fn handle(p: *mut c_void, what: &str) -> Result<Handle, String> {
        if p.is_null() {
            Err(format!("{what}: {}", windows::core::Error::from_thread()))
        } else {
            Ok(Handle(p))
        }
    }

    /// (상태 코드, 본문). 본문이 `limit`보다 크면 실패.
    pub fn get(url: &str, accept: &str, limit: usize) -> Result<(u32, Vec<u8>), String> {
        let agent = HSTRING::from(format!(
            "cssgsg-host/{} (+https://github.com/NR2BJ/cssgsg)",
            env!("CARGO_PKG_VERSION")
        ));
        let wide: Vec<u16> = url.encode_utf16().collect();
        let mut parts = URL_COMPONENTS {
            dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
            dwHostNameLength: u32::MAX,
            dwUrlPathLength: u32::MAX,
            dwExtraInfoLength: u32::MAX,
            ..Default::default()
        };
        unsafe { WinHttpCrackUrl(&wide, 0, &mut parts) }.map_err(|e| format!("url: {e}"))?;
        let piece = |p: windows::core::PWSTR, n: u32| unsafe {
            String::from_utf16_lossy(std::slice::from_raw_parts(p.0, n as usize))
        };
        let host = piece(parts.lpszHostName, parts.dwHostNameLength);
        let path = piece(parts.lpszUrlPath, parts.dwUrlPathLength)
            + &piece(parts.lpszExtraInfo, parts.dwExtraInfoLength);
        unsafe {
            let session = handle(
                WinHttpOpen(&agent, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0),
                "open",
            )?;
            let _ = WinHttpSetTimeouts(session.0, 15_000, 15_000, 30_000, 60_000);
            let connection =
                handle(WinHttpConnect(session.0, &HSTRING::from(host), parts.nPort, 0), "connect")?;
            let secure =
                if parts.nScheme.0 == 2 { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) };
            let request = handle(
                WinHttpOpenRequest(
                    connection.0,
                    w!("GET"),
                    &HSTRING::from(path),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    std::ptr::null(),
                    secure,
                ),
                "request",
            )?;
            let headers: Vec<u16> = format!("Accept: {accept}\r\n").encode_utf16().collect();
            WinHttpAddRequestHeaders(request.0, &headers, WINHTTP_ADDREQ_FLAG_ADD)
                .map_err(|e| format!("headers: {e}"))?;
            WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| format!("send: {e}"))?;
            WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| format!("receive: {e}"))?;
            let mut status = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut status as *mut u32 as *mut c_void),
                &mut size,
                std::ptr::null_mut(),
            )
            .map_err(|e| format!("status: {e}"))?;
            let mut body = Vec::new();
            loop {
                let mut available = 0u32;
                WinHttpQueryDataAvailable(request.0, &mut available).map_err(|e| format!("read: {e}"))?;
                if available == 0 {
                    break;
                }
                if body.len() + available as usize > limit {
                    return Err("response too large".into());
                }
                let start = body.len();
                body.resize(start + available as usize, 0);
                let mut read = 0u32;
                WinHttpReadData(request.0, body[start..].as_mut_ptr() as *mut c_void, available, &mut read)
                    .map_err(|e| format!("read: {e}"))?;
                body.truncate(start + read as usize);
            }
            Ok((status, body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, asset: &str, draft: bool) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "draft": draft,
            "assets": [{
                "name": asset,
                "browser_download_url": format!("https://example.invalid/{tag}/{asset}"),
                "digest": "sha256:ABCDEF"
            }]
        })
    }

    #[test]
    fn offers_are_read_from_engine_releases_only() {
        let list = serde_json::json!([
            release("mozc-1-20260928-a069a88", ASSET, false),
            release("mozc-1-20261005-bbbbbbb", ASSET, false),
            release("mozc-1-20261012-ccccccc", "cssgsg-mozc.zip", false),
            release("mozc-1-20261019-ddddddd", ASSET, true),
            release("mozc-2-20261026-eeeeeee", ASSET, false),
            release("win-v0.1.0", "cssgsg-setup.exe", false),
            release("mozc-1-2026-1005-bbbbbbb", ASSET, false),
        ]);
        let o = newest(&list, 1, &[]).unwrap();
        assert_eq!(
            (o.date.as_str(), o.commit_prefix.as_str(), o.wrapper),
            ("2026-10-05", "bbbbbbb", None),
            "맥 묶음만·초안·다른 판은 뺀다"
        );
        assert_eq!(o.digest, "sha256:abcdef");
        let bad = [("bbbbbbbffff".to_string(), 0)];
        assert_eq!(newest(&list, 1, &bad).unwrap().commit_prefix, "a069a88", "나쁜 엔진은 건너뛴다");
    }

    #[test]
    fn tags_may_carry_a_wrapper_revision() {
        let wrapper = |tag: &str| offer(&release(tag, ASSET, false)).map(|o| o.wrapper);
        assert_eq!(wrapper("mozc-1-20261005-bbbbbbb"), Some(None));
        assert_eq!(wrapper("mozc-1-20261005-bbbbbbb-w1"), Some(Some(1)));
        assert_eq!(wrapper("mozc-1-20261005-bbbbbbb-w123456"), Some(Some(123_456)));
        for tag in [
            "mozc-1-20261005-bbbbbbb-w",
            "mozc-1-20261005-bbbbbbb-wx",
            "mozc-1-20261005-bbbbbbb-1",
            "mozc-1-20261005-bbbbbbb-w1234567",
            "mozc-1-20261005-bbbbbbb-w+1",
            "mozc-1-20261005-bbbbbbb-w1-x",
        ] {
            assert_eq!(wrapper(tag), None, "{tag}");
        }
    }

    #[test]
    fn the_newest_offer_follows_the_date_then_the_wrapper() {
        let list = serde_json::json!([
            release("mozc-1-20261005-bbbbbbb", ASSET, false),
            release("mozc-1-20261005-bbbbbbb-w2", ASSET, false),
            release("mozc-1-20261005-bbbbbbb-w1", ASSET, false),
            release("mozc-1-20260928-a069a88-w9", ASSET, false),
        ]);
        let pick = |bad: &[(String, u32)]| newest(&list, 1, bad).map(|o| (o.commit_prefix, o.wrapper));
        assert_eq!(pick(&[]), Some(("bbbbbbb".into(), Some(2))), "같은 날이면 래퍼 판이 큰 것");
        let commit = "bbbbbbbffff".to_string();
        assert_eq!(
            pick(&[(commit.clone(), 2)]),
            Some(("bbbbbbb".into(), Some(1))),
            "나쁜 엔진은 커밋과 래퍼 판으로"
        );
        assert_eq!(
            pick(&[(commit.clone(), 2), (commit, 1)]),
            Some(("a069a88".into(), Some(9))),
            "판을 모르는 옛 태그는 같은 커밋의 나쁜 엔진이 있으면 건너뛴다"
        );
    }

    #[test]
    fn only_newer_engines_are_wanted() {
        let offer = Offer {
            abi: 1,
            date: "2026-10-05".into(),
            commit_prefix: "bbbbbbb".into(),
            wrapper: None,
            url: String::new(),
            digest: String::new(),
        };
        let bundled = EngineBuild {
            version: "3.34.6239.101".into(),
            date: "2026-09-28".into(),
            commit: "a069a88d4cb5c011de0f9aebb6c149a1c808d904".into(),
            wrapper: 0,
            downloaded: false,
        };
        assert!(wanted(&offer, std::slice::from_ref(&bundled)));
        let same = EngineBuild { commit: "bbbbbbbffff".into(), date: "2026-10-05".into(), ..bundled.clone() };
        assert!(!wanted(&offer, &[bundled.clone(), same.clone()]), "이미 받았다");
        let later = EngineBuild { date: "2026-10-09".into(), commit: "ccc".into(), ..bundled.clone() };
        assert!(!wanted(&offer, &[later]), "더 새것이 있다");

        // 같은 커밋: 태그의 래퍼 판이 더 커야 받는다.
        let rebuilt = Offer { wrapper: Some(1), ..offer.clone() };
        assert!(wanted(&rebuilt, &[bundled.clone(), same.clone()]), "래퍼만 고친 엔진");
        let same_w1 = EngineBuild { wrapper: 1, ..same };
        assert!(!wanted(&rebuilt, &[bundled.clone(), same_w1.clone()]), "같은 래퍼 판은 이미 받았다");
        assert!(!wanted(&offer, &[bundled, same_w1]), "판을 모르는 옛 태그는 같은 커밋이면 받지 않는다");
    }

    /// 받은 묶음의 manifest 래퍼 판은 태그와 같아야 한다(태그에 판이 없으면 manifest의 판을 쓴다, 없으면 0).
    #[test]
    fn the_manifest_wrapper_must_match_the_tag() {
        let dir = std::env::temp_dir().join(format!("cssgsg-updater-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join(LIBRARY), b"dll").unwrap();
        std::fs::write(src.join(DATA), b"data").unwrap();
        let commit = "bbbbbbbffff0000000000000000000000000000a";
        let zip = |wrapper: &str| {
            let manifest = format!(
                r#"{{"abi": 1, "commit": "{commit}", "date": "2026-10-05", "version": "3.34.6300.101"{wrapper},
                 "files": {{"{LIBRARY}": "{}", "{DATA}": "{}"}}}}"#,
                sha256_hex(b"dll").unwrap(),
                sha256_hex(b"data").unwrap()
            );
            std::fs::write(src.join(MANIFEST), manifest).unwrap();
            let out = dir.join("engine.zip");
            let status = std::process::Command::new(system_tar())
                .args(["--format=zip", "-cf"])
                .arg(&out)
                .arg("-C")
                .arg(&src)
                .args([LIBRARY, DATA, MANIFEST])
                .status()
                .unwrap();
            assert!(status.success());
            std::fs::read(out).unwrap()
        };
        let offer = |wrapper: Option<u32>| Offer {
            abi: 1,
            date: "2026-10-05".into(),
            commit_prefix: "bbbbbbb".into(),
            wrapper,
            url: String::new(),
            digest: String::new(),
        };
        let mut n = 0;
        let mut verify = |zip: &[u8], offer: &Offer| {
            n += 1;
            let staging = dir.join(format!("staging-{n}"));
            std::fs::create_dir_all(&staging).unwrap();
            unpack_and_verify(zip, &staging, offer, 1).map(|(_, wrapper)| wrapper)
        };
        let w1 = zip(r#", "wrapper": 1"#);
        assert_eq!(verify(&w1, &offer(Some(1))), Ok(1));
        assert!(verify(&w1, &offer(Some(2))).is_err(), "태그와 다른 판");
        assert_eq!(verify(&w1, &offer(None)), Ok(1), "옛 모양 태그는 manifest의 판");
        let old = zip("");
        assert_eq!(verify(&old, &offer(None)), Ok(0), "판을 적지 않던 묶음은 0");
        assert!(verify(&old, &offer(Some(1))).is_err());
        assert!(verify(&zip(r#", "wrapper": "1""#), &offer(Some(1))).is_err(), "판은 숫자");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_matches_a_known_value() {
        assert_eq!(
            sha256_hex(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
