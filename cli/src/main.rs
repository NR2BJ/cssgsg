//! cssgsg-cli: 엔진을 터미널에서 쳐본다.
//!
//! 사용법
//!   cssgsg-cli type  [--mode en|ko|ja] [--config 파일] 키열...   각 키열의 화면 결과를 한 줄씩
//!   cssgsg-cli batch [--mode en|ko|ja] [--config 파일]           표준입력 한 줄 = 키열 하나, JSON 문자열로 출력
//!   cssgsg-cli repl  [--mode en|ko|ja] [--config 파일]           한 줄씩 쳐 가며 상태를 본다
//!
//! 키열 문법은 cssgsg_core::sim 참고: 보통 글자는 쿼티 자리, 대문자·Shift 기호는 Shift,
//! {sp} {bs} {ent} {esc} {tab} {left} … {rs}/{ls}(Shift 탭) {caps} {click} {S-x} {M-x} {C-x} {A-x}.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use cssgsg_core::sim::Sim;
use cssgsg_core::{Config, Engine, Mode};

struct Opts {
    mode: Mode,
    config: Config,
    rest: Vec<String>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut mode = Mode::Ko;
    let mut config = Config::default();
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mode" => {
                mode = match it.next().map(String::as_str) {
                    Some("en") => Mode::En,
                    Some("ko") => Mode::Ko,
                    Some("ja") => Mode::Ja,
                    other => return Err(format!("--mode는 en/ko/ja 중 하나: {other:?}")),
                }
            }
            "--config" => {
                let path = it.next().ok_or("--config 뒤에 파일 경로")?;
                let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                config = Config::from_toml(&src)?;
            }
            _ => rest.push(a.clone()),
        }
    }
    Ok(Opts { mode, config, rest })
}

fn run_keys(opts: &Opts, keys: &str) -> Result<String, String> {
    let mut sim = Sim::new(Engine::new(opts.config.clone())).with_mode(opts.mode);
    sim.type_keys(keys)?;
    Ok(sim.screen())
}

fn json(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out += "\\\"",
            '\\' => out += "\\\\",
            '\n' => out += "\\n",
            '\t' => out += "\\t",
            c if (c as u32) < 0x20 => out += &format!("\\u{:04x}", c as u32),
            c => out.push(c),
        }
    }
    out + "\""
}

fn repl(opts: &Opts) -> io::Result<()> {
    let mut sim = Sim::new(Engine::new(opts.config.clone())).with_mode(opts.mode);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    writeln!(stdout, "키열을 치고 Enter. 끝내려면 Ctrl-D. (모드 {:?})", sim.engine.mode())?;
    for line in stdin.lock().lines() {
        let line = line?;
        if let Err(e) = sim.type_keys(&line) {
            writeln!(stdout, "오류: {e}")?;
            continue;
        }
        writeln!(
            stdout,
            "모드 {:?} | 확정 {} | 조합 {}",
            sim.engine.mode(),
            json(&sim.text),
            json(&sim.preedit)
        )?;
        if let Some(c) = &sim.candidates {
            let items: Vec<String> = c
                .items
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    if Some(i) == c.selected { format!("[{}.{s}]", i + 1) } else { format!("{}.{s}", i + 1) }
                })
                .collect();
            writeln!(stdout, "후보 {}", items.join(" "))?;
        }
        stdout.flush()?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("사용법: cssgsg-cli type|batch|repl [--mode en|ko|ja] [--config 파일] [키열...]");
        return ExitCode::from(2);
    };
    let opts = match parse(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let result = match cmd.as_str() {
        "type" => {
            let mut ok = true;
            for keys in &opts.rest {
                match run_keys(&opts, keys) {
                    Ok(screen) => println!("{screen}"),
                    Err(e) => {
                        eprintln!("{keys}: {e}");
                        ok = false;
                    }
                }
            }
            if ok { Ok(()) } else { Err(io::Error::other("키열 오류")) }
        }
        "batch" => (|| {
            let stdin = io::stdin();
            let mut out = io::BufWriter::new(io::stdout());
            for line in stdin.lock().lines() {
                let line = line?;
                match run_keys(&opts, &line) {
                    Ok(screen) => writeln!(out, "{}", json(&screen))?,
                    Err(e) => writeln!(out, "{}", json(&format!("ERROR: {e}")))?,
                }
            }
            out.flush()
        })(),
        "repl" => repl(&opts),
        other => {
            eprintln!("알 수 없는 명령 {other:?}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
