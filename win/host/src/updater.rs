//! 새 Mozc 엔진 받기(맥 MozcUpdater의 윈도우판). 엔진 호스트가 뜨고 5분 뒤, 그다음 하루 한 번(그리고 설정 앱의 "지금 확인")
//! GitHub 릴리스 목록을 보고, 이 C API 판의 더 새 엔진(`mozc-<판>-<yyyymmdd>-<커밋 7자리>` 태그의 `cssgsg-mozc-windows-x64.zip`)이
//! 있으면 받는다. GitHub가 적은 SHA-256과 묶음 안 manifest.json의 파일별 해시를 확인하고 mozc-engines\<커밋>\에 둔다.
//! 다음에 호스트가 뜰 때부터 쓴다(설정 앱 "지금 적용"은 호스트 다시 시작). 엔진 고르기는 [`crate::engines`].
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
    let bad = engines::bad();
    let Some(offer) = newest(&releases, abi, |commit| bad.iter().any(|b| b.starts_with(commit))) else {
        return Ok("no engine for this C API".into());
    };
    let mut installed: Vec<EngineBuild> = engines::downloaded(abi).into_iter().map(|e| e.build).collect();
    installed.extend(engines::bundled(bundled_dir).map(|e| e.build));
    if let Some(active) = info.lock().ok().and_then(|i| i.active.clone()) {
        installed.push(active);
    }
    if !wanted(&offer, &installed) {
        return Ok(format!("up to date ({} {})", offer.date, offer.commit_prefix));
    }
    let engine = install(&offer, abi)?;
    // 지금 쓰는 것과 새로 받은 것만 남긴다.
    let active = info.lock().ok().and_then(|i| i.active.clone());
    let keep: Vec<&str> = [Some(engine.build.commit.as_str()), active.as_ref().map(|a| a.commit.as_str())]
        .into_iter()
        .flatten()
        .collect();
    engines::prune(&keep);
    Ok(format!("downloaded {} ({}, {})", engine.build.version, engine.build.date, &engine.build.commit[..7]))
}

/// 릴리스 하나 → 내놓은 엔진(태그 모양, 초안 아님, 윈도우 묶음과 GitHub 해시가 있을 때만).
pub fn offer(release: &serde_json::Value) -> Option<Offer> {
    if release["draft"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?.strip_prefix(TAG_PREFIX)?;
    let parts: Vec<&str> = tag.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
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
        url: asset["browser_download_url"].as_str()?.into(),
        digest: digest.to_ascii_lowercase(),
    })
}

/// 이 판의 엔진 중 나쁜 엔진이 아닌 가장 새것(날짜로).
pub fn newest(releases: &serde_json::Value, abi: i32, is_bad: impl Fn(&str) -> bool) -> Option<Offer> {
    releases
        .as_array()?
        .iter()
        .filter_map(offer)
        .filter(|o| o.abi == abi && !is_bad(&o.commit_prefix))
        .max_by(|a, b| a.date.cmp(&b.date))
}

/// 받을 만한가: 여기 있는 어느 것보다 새것이고 이미 있는 것이 아니다(맥 isWanted).
pub fn wanted(offer: &Offer, installed: &[EngineBuild]) -> bool {
    !installed
        .iter()
        .any(|b| b.commit.to_ascii_lowercase().starts_with(&offer.commit_prefix) || b.date >= offer.date)
}

/// 받아서 확인하고 mozc-engines\<커밋>\에 푼다.
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
    let commit = match result {
        Ok(commit) => commit,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let destination = root.join(&commit);
    let _ = std::fs::remove_dir_all(&destination);
    std::fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
    engines::read_downloaded(&destination, abi).ok_or_else(|| "unpacked engine is incomplete".into())
}

/// 묶음을 풀고 매니페스트(판, 커밋·날짜, 파일 해시)를 확인한다. 커밋을 돌려준다.
fn unpack_and_verify(zip: &[u8], staging: &Path, offer: &Offer, abi: i32) -> Result<String, String> {
    let archive = staging.join("engine.zip");
    std::fs::write(&archive, zip).map_err(|e| e.to_string())?;
    let tar = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("tar.exe");
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
    for name in [LIBRARY, DATA] {
        let expected = manifest["files"][name].as_str().unwrap_or_default().to_ascii_lowercase();
        let data = std::fs::read(staging.join(name)).map_err(|e| format!("{name}: {e}"))?;
        if expected.is_empty() || sha256_hex(&data)? != expected {
            return Err(format!("{name}: SHA-256 doesn't match the manifest"));
        }
    }
    Ok(commit)
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
        let o = newest(&list, 1, |_| false).unwrap();
        assert_eq!(
            (o.date.as_str(), o.commit_prefix.as_str()),
            ("2026-10-05", "bbbbbbb"),
            "맥 묶음만·초안·다른 판은 뺀다"
        );
        assert_eq!(o.digest, "sha256:abcdef");
        assert_eq!(
            newest(&list, 1, |c| c == "bbbbbbb").unwrap().commit_prefix,
            "a069a88",
            "나쁜 엔진은 건너뛴다"
        );
    }

    #[test]
    fn only_newer_engines_are_wanted() {
        let offer = Offer {
            abi: 1,
            date: "2026-10-05".into(),
            commit_prefix: "bbbbbbb".into(),
            url: String::new(),
            digest: String::new(),
        };
        let bundled = EngineBuild {
            version: "3.34.6239.101".into(),
            date: "2026-09-28".into(),
            commit: "a069a88d4cb5c011de0f9aebb6c149a1c808d904".into(),
            downloaded: false,
        };
        assert!(wanted(&offer, std::slice::from_ref(&bundled)));
        let same = EngineBuild { commit: "bbbbbbbffff".into(), date: "2026-10-05".into(), ..bundled.clone() };
        assert!(!wanted(&offer, &[bundled.clone(), same]), "이미 받았다");
        let later = EngineBuild { date: "2026-10-09".into(), commit: "ccc".into(), ..bundled };
        assert!(!wanted(&offer, &[later]), "더 새것이 있다");
    }

    #[test]
    fn sha256_matches_a_known_value() {
        assert_eq!(
            sha256_hex(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
