//! cssgsg-cli: 엔진을 터미널에서 쳐본다.
//!
//! 사용법
//!   cssgsg-cli type  [--mode en|ko|ja] [--config 파일] 키열...   각 키열의 화면 결과를 한 줄씩
//!   cssgsg-cli batch [--mode en|ko|ja] [--config 파일]           표준입력 한 줄 = 키열 하나, JSON 문자열로 출력
//!   cssgsg-cli repl  [--mode en|ko|ja] [--config 파일]           한 줄씩 쳐 가며 상태를 본다
//!   cssgsg-cli layout-json 배열ID                                  배열 데이터를 JSON으로 (교차 검증·학습 페이지용)
//!       배열ID: chamshin-v18, chamshin-d-v19, graphite, shingetsu
//!
//! 공통 옵션 --ko-layout 배열ID 는 한국어 배열만 바꾼다(설정 파일 없이).
//!
//! 키열 문법은 cssgsg_core::sim 참고: 보통 글자는 쿼티 자리, 대문자·Shift 기호는 Shift,
//! {sp} {bs} {ent} {esc} {tab} {left} … {rs}/{ls}(Shift 탭) {caps} {click} {S-x} {M-x} {C-x} {A-x}.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use cssgsg_core::hangul::{Action, KoLayout};
use cssgsg_core::kana::KanaLayout;
use cssgsg_core::latin::LatinLayout;
use cssgsg_core::sim::Sim;
use cssgsg_core::{Config, Engine, Key, Mode};

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
            "--ko-layout" => {
                let id = it.next().ok_or("--ko-layout 뒤에 배열 ID")?;
                if KoLayout::builtin(id).is_none() {
                    return Err(format!("알 수 없는 한국어 배열 {id:?}"));
                }
                config.ko_layout = id.clone();
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

/// 글자 키 47개(HID 순서).
fn printable_keys() -> Vec<Key> {
    let mut keys: Vec<Key> = (0..0x80u16).map(Key::from_mac_keycode).filter(|k| k.is_printable()).collect();
    keys.sort();
    keys.dedup();
    keys
}

fn layout_json(id: &str) -> Result<String, String> {
    match id {
        "graphite" => Ok(graphite_json()),
        "shingetsu" => Ok(shingetsu_json()),
        _ => ko_layout_json(id),
    }
}

/// Graphite: 키마다 (기본, Shift) 글자.
fn graphite_json() -> String {
    let l = LatinLayout::graphite();
    let rows: Vec<String> = printable_keys()
        .into_iter()
        .map(|k| {
            let c = |shift| l.char_for(k, shift, false, false).map(String::from).unwrap_or_default();
            format!(
                "{{\"key\":{},\"base\":{},\"shift\":{}}}",
                json(&k.qwerty_char(false).unwrap().to_string()),
                json(&c(false)),
                json(&c(true))
            )
        })
        .collect();
    format!("{{\"id\":\"graphite\",\"keys\":[\n{}\n]}}", rows.join(",\n"))
}

/// 新月配列: 키마다 무시프트/☆면/★면 가나, 앞치기·뒤치기 키, 3타 단축표, 뒤치기 표.
fn shingetsu_json() -> String {
    let l = KanaLayout::shingetsu();
    let name = |k: Key| json(&k.qwerty_char(false).unwrap().to_string());
    let opt = |v: Option<&String>| v.map_or("null".to_string(), |s| json(s));
    let keys: Vec<String> = printable_keys()
        .into_iter()
        .map(|k| {
            format!(
                "{{\"key\":{},\"base\":{},\"star\":{},\"black\":{}}}",
                name(k),
                opt(l.base.get(&k)),
                opt(l.star.get(&k)),
                opt(l.black.get(&k))
            )
        })
        .collect();
    let mut star_daku: Vec<(Key, &String)> = l.star_daku.iter().map(|(k, v)| (*k, v)).collect();
    star_daku.sort();
    let star_daku: Vec<String> =
        star_daku.iter().map(|(k, v)| format!("{{\"key\":{},\"out\":{}}}", name(*k), json(v))).collect();
    let mut postfix: Vec<(char, char)> = l.postfix.iter().map(|(a, b)| (*a, *b)).collect();
    postfix.sort();
    let postfix: Vec<String> = postfix
        .iter()
        .map(|(a, b)| format!("{{\"from\":{},\"to\":{}}}", json(&a.to_string()), json(&b.to_string())))
        .collect();
    format!(
        "{{\"id\":\"shingetsu\",\"star_key\":{},\"black_key\":{},\"daku_key\":{},\"keys\":[\n{}\n],\"star_daku\":[{}],\"postfix\":[{}]}}",
        name(l.star_key),
        name(l.black_key),
        name(l.daku_key),
        keys.join(",\n"),
        star_daku.join(","),
        postfix.join(",")
    )
}

/// 한국어 배열의 글자 키 47개 × Shift 두 상태의 역할, 조합표. 배열에 없는 키는 쿼티 글자(passthrough).
fn ko_layout_json(id: &str) -> Result<String, String> {
    let l = KoLayout::builtin(id).ok_or_else(|| format!("알 수 없는 배열 {id:?}"))?;
    let keys = printable_keys();
    let opt = |c: Option<char>| c.map_or("null".to_string(), |c| json(&c.to_string()));
    let mut rows = Vec::new();
    for shift in [false, true] {
        for &k in &keys {
            let name = json(&k.qwerty_char(false).unwrap().to_string());
            let row = match l.roles(k, shift) {
                Some(r) => format!(
                    "{{\"key\":{name},\"shift\":{shift},\"cho\":{},\"jung\":{},\"jong\":{},\"sym\":{},\"action\":{}}}",
                    opt(r.cho),
                    opt(r.jung),
                    opt(r.jong),
                    r.sym.as_deref().map_or("null".to_string(), json),
                    if r.action == Some(Action::Stop) { "\"stop\"" } else { "null" },
                ),
                None => format!(
                    "{{\"key\":{name},\"shift\":{shift},\"passthrough\":{}}}",
                    json(&k.qwerty_char(shift).unwrap().to_string())
                ),
            };
            rows.push(row);
        }
    }
    let combos = |list: Vec<(char, char, char)>| {
        let mut list = list;
        list.sort();
        list.iter()
            .map(|(a, b, c)| {
                format!("[{},{},{}]", json(&a.to_string()), json(&b.to_string()), json(&c.to_string()))
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    Ok(format!(
        "{{\"id\":{},\"name\":{},\"keys\":[\n{}\n],\"combine\":{{\"cho\":[{}],\"jung\":[{}],\"jong\":[{}]}}}}",
        json(&l.id),
        json(&l.name),
        rows.join(",\n"),
        combos(l.cho_combos().collect()),
        combos(l.jung_combos().collect()),
        combos(l.jong_combos().collect())
    ))
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
        "layout-json" => match opts.rest.first().map(|id| layout_json(id)) {
            Some(Ok(j)) => {
                println!("{j}");
                Ok(())
            }
            Some(Err(e)) => Err(io::Error::other(e)),
            None => Err(io::Error::other(
                "배열 ID가 필요하다 (chamshin-v18, chamshin-d-v19, graphite, shingetsu)",
            )),
        },
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
