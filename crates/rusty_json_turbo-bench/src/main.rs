//! `rjson-bench` -- the harness binary.
//!
//! ```text
//! rjson-bench bench [--cell FILE,COLUMN | --all] [--arms A,B] [--pairs N]
//!                   [--window-ms W] [--pinned DESC] [--commit SHA]
//! rjson-bench null  [--cell ...] [--pairs N] ...      (upstream vs upstream: the floor)
//! rjson-bench diff-oracle PATH...                     (files or directories)
//! rjson-bench list
//! ```
//!
//! Run it through `tools/pinbench.ps1` so the process is pinned to one P-core
//! at High priority; the wrapper passes `--pinned` and the bench echoes it into
//! the method line. Every table this prints carries that line.

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use rjt_bench::cells::{Arm, Column};
use rjt_bench::corpus::{self, File};
use rjt_bench::harness::{self, Config};
use rjt_bench::oracle;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first().map(String::as_str) else {
        eprintln!("usage: rjson-bench <bench|null|diff-oracle|list> ...");
        return ExitCode::from(2);
    };
    match verb {
        "bench" => bench(&args[1..], None),
        "null" => bench(&args[1..], Some((Arm::Upstream, Arm::Upstream))),
        "diff-oracle" => diff_oracle(&args[1..]),
        "list" => {
            for f in File::ALL {
                for c in Column::ALL {
                    println!("{},{}", f.name(), c.name());
                }
            }
            for a in Arm::ALL {
                println!(
                    "arm {} {}",
                    a.name(),
                    if a.available() {
                        ""
                    } else {
                        "(needs --features competitors)"
                    }
                );
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command {other:?}");
            ExitCode::from(2)
        }
    }
}

struct Opts {
    cells: Vec<(File, Column)>,
    arms: (Arm, Arm),
    cfg: Config,
    commit: String,
    settle_ms: u64,
}

fn parse_opts(args: &[String], forced_arms: Option<(Arm, Arm)>) -> Result<Opts, String> {
    let mut cells = Vec::new();
    let mut arms = forced_arms.unwrap_or((Arm::Ours, Arm::Upstream));
    let mut pairs = 20usize;
    let mut window_ms = 250u64;
    let mut pinned = String::from("NOT PINNED (run through tools/pinbench.ps1)");
    let mut commit = String::from("uncommitted");
    let mut settle_ms = 0u64;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut next = || -> Result<&str, String> {
            i += 1;
            args.get(i)
                .map(String::as_str)
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a {
            "--all" => {
                for f in File::ALL {
                    for c in Column::ALL {
                        cells.push((f, c));
                    }
                }
            }
            "--cell" => {
                let v = next()?;
                let (f, c) = v
                    .split_once(',')
                    .ok_or_else(|| format!("--cell wants FILE,COLUMN, got {v:?}"))?;
                let f = File::parse(f).ok_or_else(|| format!("unknown file {f:?}"))?;
                if c == "all" {
                    for c in Column::ALL {
                        cells.push((f, c));
                    }
                } else {
                    let c = Column::parse(c).ok_or_else(|| format!("unknown column {c:?}"))?;
                    cells.push((f, c));
                }
            }
            "--arms" => {
                let v = next()?;
                if forced_arms.is_none() {
                    let (x, y) = v
                        .split_once(',')
                        .ok_or_else(|| format!("--arms wants A,B, got {v:?}"))?;
                    let x = Arm::parse(x).ok_or_else(|| format!("unknown arm {x:?}"))?;
                    let y = Arm::parse(y).ok_or_else(|| format!("unknown arm {y:?}"))?;
                    arms = (x, y);
                }
            }
            "--pairs" => pairs = next()?.parse().map_err(|e| format!("--pairs: {e}"))?,
            "--window-ms" => {
                window_ms = next()?.parse().map_err(|e| format!("--window-ms: {e}"))?
            }
            "--pinned" => {
                pinned = next()?.to_owned();
                if settle_ms == 0 {
                    settle_ms = 300;
                }
            }
            "--settle-ms" => {
                settle_ms = next()?.parse().map_err(|e| format!("--settle-ms: {e}"))?
            }
            "--commit" => commit = next()?.to_owned(),
            other => return Err(format!("unknown option {other:?}")),
        }
        i += 1;
    }
    if cells.is_empty() {
        return Err("no cells: pass --all or --cell FILE,COLUMN".to_owned());
    }
    Ok(Opts {
        cells,
        arms,
        cfg: Config {
            pairs,
            window: Duration::from_millis(window_ms),
            pinned,
        },
        commit,
        settle_ms,
    })
}

fn bench(args: &[String], forced_arms: Option<(Arm, Arm)>) -> ExitCode {
    let opts = match parse_opts(args, forced_arms) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let (a, b) = opts.arms;
    if !a.available() || !b.available() {
        eprintln!("error: arm needs --features competitors");
        return ExitCode::from(2);
    }
    if opts.settle_ms > 0 {
        // Give the wrapper time to apply affinity and priority before any
        // number is taken.
        std::thread::sleep(Duration::from_millis(opts.settle_ms));
    }
    let null = a == b;
    println!(
        "rjson-bench: {} vs {}{} | {} cells | {} pairs | {} ms windows",
        a.name(),
        b.name(),
        if null { " (NULL ARM: the floor)" } else { "" },
        opts.cells.len(),
        opts.cfg.pairs,
        opts.cfg.window.as_millis()
    );
    let mut rows = Vec::new();
    let mut header = None;
    for (file, column) in &opts.cells {
        let input = file.load();
        println!(
            "\n== {} / {} ({} bytes)",
            file.name(),
            column.name(),
            input.len()
        );
        let paired = harness::run_paired(&opts.cfg, a, b, *file, *column, &input, |i, p| {
            println!(
                "  pair {:2} lead={:<8} {}={:>10} ns/iter ({:>4} it)  {}={:>10} ns/iter ({:>4} it)  a/b={:.3}",
                i + 1,
                p.lead.name(),
                a.name(),
                p.a_ns,
                p.a_iters,
                b.name(),
                p.b_ns,
                p.b_iters,
                p.a_ns as f64 / p.b_ns as f64
            );
        });
        let (lo, hi) = paired.spread();
        println!(
            "  => median a/b {:.3} [{:.3}, {:.3}]  wins {}/{}  z {:+.2}  {} {:.0} MB/s  {} {:.0} MB/s",
            paired.median_ratio(),
            lo,
            hi,
            paired.wins(),
            paired.n(),
            paired.z(),
            a.name(),
            paired.mbs_a(),
            b.name(),
            paired.mbs_b()
        );
        if null {
            println!(
                "  null-arm floor for this cell: median {:.3}, worst pair {:.3}",
                paired.median_ratio(),
                (1.0 - lo).abs().max((hi - 1.0).abs()) + 1.0
            );
        }
        header.get_or_insert_with(|| paired.markdown_header());
        rows.push(paired.markdown_row());
    }
    println!("\n{}", header.unwrap_or_default());
    for r in &rows {
        println!("{r}");
    }
    println!("\n{}", harness::method_line(&opts.cfg, &opts.commit));
    ExitCode::SUCCESS
}

fn diff_oracle(paths: &[String]) -> ExitCode {
    if paths.is_empty() {
        eprintln!("usage: rjson-bench diff-oracle PATH...");
        return ExitCode::from(2);
    }
    let mut docs: Vec<(String, Vec<u8>)> = Vec::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            docs.extend(corpus::json_files_in(path, p));
        } else {
            match std::fs::read(path) {
                Ok(bytes) => docs.push((p.clone(), bytes)),
                Err(e) => {
                    eprintln!("error: {p}: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let mut total = 0usize;
    let mut tokens = std::collections::BTreeSet::new();
    for (name, bytes) in &docs {
        let m = oracle::check_document(name, bytes);
        println!("{name}: {} bytes, {} mismatches", bytes.len(), m.len());
        for x in &m {
            println!("{x}");
        }
        total += m.len();
        tokens.extend(oracle::number_tokens(bytes));
    }
    let mut num_mismatches = 0usize;
    for t in &tokens {
        let m = oracle::check_number(t);
        for x in &m {
            println!("{x}");
        }
        num_mismatches += m.len();
    }
    println!(
        "diff-oracle: {} documents, {} number tokens, {} document mismatches, {} number mismatches",
        docs.len(),
        tokens.len(),
        total,
        num_mismatches
    );
    if total + num_mismatches == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
