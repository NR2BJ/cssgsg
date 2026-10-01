//! Mozc 엔진 고르기: 설치본에 든 것(exe 옆 mozc 폴더)과 엔진 호스트가 받아 둔 것(%LOCALAPPDATA%\cssgsg\mozc-engines\<커밋>\).
//! 맥 MozcComponents와 같은 규칙이다: 받은 것은 설치본보다 새것(upstream 커밋 날짜, 같으면 Mozc 버전)만 쓰고 새것부터 읽어 본다.
//! 읽지 못한 엔진과 읽다가 호스트가 죽은 엔진(시작을 끝내지 못한 것이 두 번)은 나쁜 엔진으로 적고 다시 쓰지 않는다.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cssgsg_ipc::EngineBuild;

pub const LIBRARY: &str = "cssgsg_mozc.dll";
pub const DATA: &str = "mozc.data";
pub const MANIFEST: &str = "manifest.json";
const BAD: &str = "bad.txt";
/// 받은 엔진 폴더 안의 "시작을 끝내지 못한 횟수" 기록.
const STARTS: &str = ".starts";
/// 이만큼 시작을 끝내지 못하면 나쁜 엔진이다.
const START_LIMIT: u32 = 2;

/// 고를 수 있는 엔진 하나.
#[derive(Clone, Debug)]
pub struct Engine {
    pub dir: PathBuf,
    pub build: EngineBuild,
}

/// 받은 엔진 폴더: %LOCALAPPDATA%\cssgsg\mozc-engines.
pub fn root() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("cssgsg").join("mozc-engines")
}

/// 설치본에 든 엔진(`<dir>\MOZC_VERSION`: "<커밋> <날짜> <버전>").
pub fn bundled(dir: &Path) -> Option<Engine> {
    let line = std::fs::read_to_string(dir.join("MOZC_VERSION")).ok()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 || !dir.join(LIBRARY).is_file() {
        return None;
    }
    Some(Engine {
        dir: dir.to_path_buf(),
        build: EngineBuild {
            commit: parts[0].into(),
            date: parts[1].into(),
            version: parts[2].into(),
            downloaded: false,
        },
    })
}

/// 받은 엔진 폴더 하나의 매니페스트(이 C API 판이고 파일이 다 있는 것만).
pub fn read_downloaded(dir: &Path, abi: i32) -> Option<Engine> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let m: serde_json::Value = serde_json::from_str(&text).ok()?;
    if m["abi"].as_i64()? != i64::from(abi) || !dir.join(LIBRARY).is_file() || !dir.join(DATA).is_file() {
        return None;
    }
    Some(Engine {
        dir: dir.to_path_buf(),
        build: EngineBuild {
            commit: m["commit"].as_str()?.into(),
            date: m["date"].as_str()?.into(),
            version: m["version"].as_str()?.into(),
            downloaded: true,
        },
    })
}

/// 받아 둔 엔진들(나쁜 엔진 빼고).
pub fn downloaded(abi: i32) -> Vec<Engine> {
    let bad = bad();
    let Ok(entries) = std::fs::read_dir(root()) else { return Vec::new() };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_hexdigit()))
        .filter_map(|e| read_downloaded(&e.path(), abi))
        .filter(|e| !bad.contains(&e.build.commit))
        .collect()
}

/// Mozc 버전 비교("3.34.6239.101"): 숫자 마디마다.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// `a`가 `b`보다 새 엔진인지: 커밋 날짜로, 같으면 버전으로. 같은 커밋은 새것이 아니다(맥 MozcComponents.isNewer).
pub fn is_newer(a: &EngineBuild, b: &EngineBuild) -> bool {
    if a.commit == b.commit {
        return false;
    }
    if a.date != b.date {
        return a.date > b.date;
    }
    version_key(&a.version) > version_key(&b.version)
}

/// 읽어 볼 차례: 설치본보다 새로 받은 것(새것부터), 그다음 설치본.
pub fn candidates(bundled: Option<Engine>, mut downloaded: Vec<Engine>) -> Vec<Engine> {
    if let Some(base) = &bundled {
        downloaded.retain(|e| is_newer(&e.build, &base.build));
    }
    downloaded.sort_by(|a, b| {
        if is_newer(&a.build, &b.build) {
            std::cmp::Ordering::Less
        } else if is_newer(&b.build, &a.build) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    downloaded.into_iter().chain(bundled).collect()
}

/// 나쁜 엔진의 커밋.
pub fn bad() -> HashSet<String> {
    std::fs::read_to_string(root().join(BAD))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn mark_bad(commit: &str) {
    let mut all = bad();
    if all.insert(commit.to_string()) {
        let mut list: Vec<String> = all.into_iter().collect();
        list.sort();
        let _ = std::fs::create_dir_all(root());
        let _ = std::fs::write(root().join(BAD), list.join("\n") + "\n");
    }
}

/// 받은 엔진을 읽기 전에 부른다. 앞서 시작을 끝내지 못한 것이 이미 한도면 나쁜 엔진으로 적고 false.
pub fn begin_start(engine: &Engine) -> bool {
    if !engine.build.downloaded {
        return true;
    }
    let file = engine.dir.join(STARTS);
    let tries: u32 = std::fs::read_to_string(&file).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
    if tries >= START_LIMIT {
        mark_bad(&engine.build.commit);
        return false;
    }
    let _ = std::fs::write(&file, (tries + 1).to_string());
    true
}

/// 읽고 변환까지 해 봤다(또는 읽지 못해 나쁜 엔진으로 적었다): 시작 기록을 지운다.
pub fn end_start(engine: &Engine) {
    if engine.build.downloaded {
        let _ = std::fs::remove_file(engine.dir.join(STARTS));
    }
}

/// 받아 둔 엔진 중 쓰지 않을 것(지금 쓰는 것과 기다리는 것 말고)을 지운다. 쓰는 중인 DLL은 지울 수 없으니 실패는 넘긴다.
pub fn prune(keep: &[&str]) {
    let Ok(entries) = std::fs::read_dir(root()) else { return };
    for e in entries.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().into_owned();
        let is_engine = !name.is_empty() && name.chars().all(|c| c.is_ascii_hexdigit());
        if is_engine && !keep.contains(&name.as_str()) && e.path().is_dir() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(commit: &str, date: &str, version: &str) -> EngineBuild {
        EngineBuild { commit: commit.into(), date: date.into(), version: version.into(), downloaded: true }
    }

    #[test]
    fn newer_follows_the_commit_date_then_the_version() {
        let old = build("aaaa", "2026-09-28", "3.34.6239.101");
        assert!(is_newer(&build("bbbb", "2026-10-05", "3.34.6239.101"), &old));
        assert!(!is_newer(&build("bbbb", "2026-09-20", "3.34.6300.101"), &old), "날짜가 먼저");
        assert!(is_newer(&build("bbbb", "2026-09-28", "3.34.6240.101"), &old), "같은 날이면 버전");
        assert!(!is_newer(&build("aaaa", "2026-12-01", "9.9.9.9"), &old), "같은 커밋은 새것이 아니다");
    }

    #[test]
    fn candidates_put_newer_downloads_before_the_bundled_engine() {
        let engine = |b: EngineBuild| Engine { dir: PathBuf::from(&b.commit), build: b };
        let bundled = Engine {
            dir: PathBuf::from("bundled"),
            build: EngineBuild { downloaded: false, ..build("base", "2026-09-28", "3.34.6239.101") },
        };
        let order = candidates(
            Some(bundled),
            vec![
                engine(build("older", "2026-09-01", "3.34.6000.101")),
                engine(build("newest", "2026-11-01", "3.35.1.101")),
                engine(build("newer", "2026-10-01", "3.34.6300.101")),
            ],
        );
        let commits: Vec<&str> = order.iter().map(|e| e.build.commit.as_str()).collect();
        assert_eq!(commits, ["newest", "newer", "base"]);
    }
}
