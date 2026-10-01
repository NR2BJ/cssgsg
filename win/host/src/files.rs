//! 사용자 파일: 설정(config.toml)과 한자 기억(hanja-learning.tsv), %APPDATA%\cssgsg.
//!
//! 앱 컨테이너 앱의 입력기는 이 폴더를 읽을 수 없고 앱마다 엔진이 따로라, 호스트가 읽고 쓰고 입력기에 판으로 나눠 준다
//! (`Request::Sync`). 설정은 맥과 같은 파일·형식이고, 적지 않은 한자 단축키만 윈도우 기본값이다(`Config::from_toml_windows`).
//! 한자 기억은 입력기가 고를 때마다 알려 오고(`Request::HanjaPicked`), 2초 모았다가 저장한다(맥과 같다).

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use cssgsg_core::config::Config;
use cssgsg_core::hanja::Learning;
use cssgsg_ipc::{Reply, Versioned, version_of};

use crate::host::log;

pub const CONFIG: &str = "config.toml";
pub const LEARNING: &str = "hanja-learning.tsv";

/// 한자 기억을 모았다가 저장하는 틈.
const SAVE_AFTER: Duration = Duration::from_secs(2);

pub struct UserFiles {
    dir: PathBuf,
    /// 마지막으로 읽은 올바른 설정 파일 내용(없으면 빈 내용 = 기본값).
    config: Versioned,
    /// 그때 파일의 (고친 시각, 크기). 바뀌었을 때만 다시 읽는다.
    stamp: Option<(SystemTime, u64)>,
    learning: Learning,
    learning_version: u64,
    /// 저장하지 않은 한자 기억이 처음 생긴 때.
    dirty_since: Option<Instant>,
}

impl UserFiles {
    pub fn open(dir: PathBuf) -> Self {
        let learning = Learning::from_tsv(&std::fs::read_to_string(dir.join(LEARNING)).unwrap_or_default());
        let learning_version = version_of(&learning.to_tsv());
        let mut files = Self {
            dir,
            config: Versioned { version: version_of(""), text: String::new() },
            stamp: None,
            learning,
            learning_version,
            dirty_since: None,
        };
        files.refresh_config();
        files
    }

    /// 설정 파일이 바뀌었으면 다시 읽는다. 틀린 파일은 기록하고 앞의 것을 그대로 쓴다(입력기는 늘 올바른 설정만 받는다).
    fn refresh_config(&mut self) {
        let path = self.dir.join(CONFIG);
        let stamp =
            std::fs::metadata(&path).ok().map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
        if stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        let text = match stamp {
            Some(_) => match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    log(&format!("config.toml: {e}"));
                    return;
                }
            },
            None => String::new(),
        };
        match Config::from_toml_windows(&text) {
            Ok(_) => self.config = Versioned { version: version_of(&text), text },
            Err(e) => log(&format!("config.toml is not valid, keeping the previous settings: {e}")),
        }
    }

    /// 입력기가 가진 판과 다른 것만 준다.
    pub fn sync(&mut self, config: u64, learning: u64) -> Reply {
        self.refresh_config();
        Reply::Sync {
            config: (config != self.config.version).then(|| self.config.clone()),
            learning: (learning != self.learning_version)
                .then(|| Versioned { version: self.learning_version, text: self.learning.to_tsv() }),
        }
    }

    /// 한자를 하나 골랐다.
    pub fn picked(&mut self, reading: &str, text: &str) {
        self.learning.record(reading, text);
        self.learning_version = version_of(&self.learning.to_tsv());
        self.dirty_since.get_or_insert_with(Instant::now);
    }

    /// 한자 기억을 모두 지운다(설정 앱). 빈 기억을 바로 저장하고, 입력기들은 다음 Sync에 받는다.
    pub fn clear_learning(&mut self) {
        self.learning = Learning::default();
        self.learning_version = version_of(&self.learning.to_tsv());
        self.dirty_since = Some(Instant::now());
        self.save(true);
    }

    /// 모아 둔 한자 기억을 저장한다(2초 모였거나 `now`). 임시 파일에 쓰고 바꿔 넣는다(쓰다 끊겨도 앞 파일이 남는다).
    pub fn save(&mut self, now: bool) {
        let Some(since) = self.dirty_since else { return };
        if !now && since.elapsed() < SAVE_AFTER {
            return;
        }
        let path = self.dir.join(LEARNING);
        let temp = self.dir.join(format!("{LEARNING}.tmp"));
        let written = std::fs::create_dir_all(&self.dir)
            .and_then(|()| std::fs::write(&temp, self.learning.to_tsv()))
            .and_then(|()| std::fs::rename(&temp, &path));
        match written {
            Ok(()) => self.dirty_since = None,
            Err(e) => log(&format!("hanja learning: {e}")),
        }
    }
}
