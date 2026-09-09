//! `rjson` -- the rusty_json_turbo command line.
//!
//! House law: `#[global_allocator]` lives in the deliverable, never in a
//! library, so this binary installs it through the seam crate.
//!
//! The CLI is consumer #1 of the library and never the only one: every verb
//! here is one library call. It does not shadow `jq`.

#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::process::ExitCode;

#[global_allocator]
static ALLOC: rusty_json_turbo_alloc::Alloc = rusty_json_turbo_alloc::Alloc;

const USAGE: &str = "\
rjson -- rusty_json_turbo command line (serde_json, forked and made fast)

USAGE:
    rjson validate <FILE|->     parse and discard; exit 0 if valid, 1 if not
    rjson pretty   <FILE|->     re-emit as pretty-printed JSON
    rjson minify   <FILE|->     re-emit as compact JSON
    rjson isa                   print which kernel arm this process resolved
    rjson --version

`-` reads standard input. Errors are printed as `error: <message> at line L
column C` and exit 1; usage mistakes exit 2.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(msg)) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Input(msg)) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

enum Failure {
    Usage(String),
    Input(String),
}

impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        Failure::Input(format!("i/o: {e}"))
    }
}

impl From<serde_json::Error> for Failure {
    fn from(e: serde_json::Error) -> Self {
        // serde_json's Display already carries `at line L column C`.
        Failure::Input(e.to_string())
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    let Some(verb) = args.first() else {
        return Err(Failure::Usage("missing command".to_owned()));
    };
    match verb.as_str() {
        "--version" | "-V" => {
            println!(
                "rjson {} (rusty_json_turbo, tracks serde_json 1.0.151)",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        "isa" => {
            // M0: no kernels are wired yet; the arm is the scalar upstream code
            // on every target. M3 replaces this with the real resolver.
            println!("scalar (no accelerated kernels wired at M0)");
            Ok(())
        }
        "validate" | "pretty" | "minify" => {
            let path = args
                .get(1)
                .ok_or_else(|| Failure::Usage(format!("{verb} needs a file argument (or -)")))?;
            if args.len() > 2 {
                return Err(Failure::Usage(format!("unexpected argument {:?}", args[2])));
            }
            let input = read_input(path)?;
            match verb.as_str() {
                "validate" => validate(&input),
                "pretty" => emit(&input, true),
                _ => emit(&input, false),
            }
        }
        other => Err(Failure::Usage(format!("unknown command {other:?}"))),
    }
}

fn read_input(path: &str) -> Result<Vec<u8>, Failure> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin().lock().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        Ok(fs::read(path).map_err(|e| Failure::Input(format!("{path}: {e}")))?)
    }
}

fn validate(input: &[u8]) -> Result<(), Failure> {
    // IgnoredAny walks the document without building a Value.
    serde_json::from_slice::<serde::de::IgnoredAny>(input)?;
    Ok(())
}

fn emit(input: &[u8], pretty: bool) -> Result<(), Failure> {
    let value: serde_json::Value = serde_json::from_slice(input)?;
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    if pretty {
        serde_json::to_writer_pretty(&mut out, &value)?;
    } else {
        serde_json::to_writer(&mut out, &value)?;
    }
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
