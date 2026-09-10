//! `rjson-bench` -- the harness binary.
//!
//! ```text
//! rjson-bench bench  [--cell FILE,COLUMN | --all] [--arms A,B] [--pairs N]
//!                    [--window-ms W] [--pinned DESC] [--commit SHA]
//! rjson-bench null   [--cell ...] [--pairs N] ...     (an arm vs itself: the floor)
//! rjson-bench solo   [--cell ...] [--arm A] [--window-ms W]
//!                                                    (ONE arm, machine-readable RESULT lines)
//! rjson-bench census [--cell ... | --all]            (allocations per op; needs --features profile)
//! rjson-bench diff-oracle PATH...                    (files or directories)
//! rjson-bench list
//! ```
//!
//! `bench` and `null` compare two arms INSIDE one process. The allocator cannot
//! be compared that way -- a program has exactly one -- so `solo` exists to be
//! driven by `tools/pinvs.ps1`, which alternates two BINARIES and pairs their
//! self-reported per-cell times.
//!
//! Run everything through the pinning wrappers; the method line echoes the pin,
//! the allocator, and whether this build is taxed by the allocation census.

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use rjt_bench::alloc_arm;
use rjt_bench::cells::{self, Arm, Column};
use rjt_bench::content;
use rjt_bench::corpus::{self, File};
use rjt_bench::harness::{self, Config};
use rjt_bench::oracle;

// House law: the deliverable declares the allocator, never a library. Both
// arms of the allocator experiment are explicit about it -- the system build
// names `System` rather than leaving it implicit, so the two binaries differ
// only in which backend the seam supplies.
#[cfg(feature = "profile")]
#[global_allocator]
static ALLOC: alloc_arm::Counting<alloc_arm::Backend> = alloc_arm::Counting(alloc_arm::BACKEND);

#[cfg(not(feature = "profile"))]
#[global_allocator]
static ALLOC: alloc_arm::Backend = alloc_arm::BACKEND;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first().map(String::as_str) else {
        eprintln!(
            "usage: rjson-bench \n             <bench|null|solo|census|probe|work|latency|diff-oracle|list> ..."
        );
        return ExitCode::from(2);
    };
    match verb {
        "bench" => bench(&args[1..], None),
        "null" => bench(&args[1..], Some((Arm::Upstream, Arm::Upstream))),
        "solo" => solo(&args[1..]),
        "census" => census(&args[1..]),
        "probe" => probe(&args[1..]),
        "work" => work(&args[1..]),
        "latency" => latency(&args[1..]),
        "b6" => b6(&args[1..]),
        "diff-oracle" => diff_oracle(&args[1..]),
        "list" => {
            for f in File::EVERY {
                for c in Column::ALL {
                    println!("{},{}	{}", f.name(), c.name(), f.scenario().name());
                }
            }
            for f in File::EVERY {
                if let Some(path) = f.message_array() {
                    println!("messages {} at {}", f.name(), path);
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
            println!("allocator {}", alloc_arm::name());
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
            // solo takes a single arm; keep one spelling for both.
            "--arm" => {
                let v = next()?;
                let x = Arm::parse(v).ok_or_else(|| format!("unknown arm {v:?}"))?;
                arms = (x, x);
            }
            "--pairs" => pairs = next()?.parse().map_err(|e| format!("--pairs: {e}"))?,
            "--window-ms" => {
                window_ms = next()?.parse().map_err(|e| format!("--window-ms: {e}"))?;
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

fn settle(ms: u64) {
    if ms > 0 {
        // Let the wrapper's affinity and priority take effect before any number.
        std::thread::sleep(Duration::from_millis(ms));
    }
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
    settle(opts.settle_ms);
    let null = a == b;
    println!(
        "rjson-bench: {} vs {}{} | {} cells | {} pairs | {} ms windows | allocator {}",
        a.name(),
        b.name(),
        if null { " (NULL ARM: the floor)" } else { "" },
        opts.cells.len(),
        opts.cfg.pairs,
        opts.cfg.window.as_millis(),
        alloc_arm::name()
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

/// One arm, machine-readable. The unit `tools/pinvs.ps1` pairs across binaries.
fn solo(args: &[String]) -> ExitCode {
    let opts = match parse_opts(args, None) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let arm = opts.arms.0;
    if !arm.available() {
        eprintln!("error: arm {} needs --features competitors", arm.name());
        return ExitCode::from(2);
    }
    settle(opts.settle_ms);
    // The runner reads this to record exactly which binary produced the numbers.
    println!(
        "BINARY|{}|{}|{}|{}|{}",
        alloc_arm::name(),
        alloc_arm::counting(),
        opts.commit,
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for (file, column) in &opts.cells {
        let input = file.load();
        let s = cells::run(arm, *file, *column, &input, opts.cfg.window);
        println!(
            "RESULT|{}|{}|{}|{}|{}|{}|{}",
            file.name(),
            column.name(),
            arm.name(),
            s.min_ns(),
            s.median_ns(),
            s.iters(),
            s.bytes
        );
    }
    ExitCode::SUCCESS
}

/// Allocations per op. Deterministic; needs the counting build.
fn census(args: &[String]) -> ExitCode {
    let opts = match parse_opts(args, None) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if !alloc_arm::counting() {
        eprintln!(
            "error: this build has no counting allocator -- rebuild with --features profile \
             (and never quote a timing from that build)"
        );
        return ExitCode::from(2);
    }
    println!("allocator={} (counting)", alloc_arm::name());
    println!(
        "{:<20} {:<18} {:>10} {:>14} {:>9} {:>9} {:>12}",
        "file", "column", "allocs", "alloc_bytes", "reallocs", "frees", "bytes"
    );
    let mut hist: Vec<(String, [u64; alloc_arm::BUCKETS], u64)> = Vec::new();
    for (file, column) in &opts.cells {
        let input = file.load();
        let (bytes, c) = cells::census_once(*file, *column, &input);
        let c = c.unwrap_or_default();
        println!(
            "{:<20} {:<18} {:>10} {:>14} {:>9} {:>9} {:>12}",
            file.name(),
            column.name(),
            c.allocs,
            c.alloc_bytes,
            c.reallocs,
            c.frees,
            bytes
        );
        hist.push((
            format!("{} {}", file.name(), column.name()),
            c.buckets,
            c.allocs,
        ));
    }
    // A total says "allocation-dominated". Only the histogram says WHICH
    // allocations, and the brick depends entirely on that: a BTreeMap node, a
    // short String for a key, and a doubling Vec are three different bricks.
    println!();
    println!("allocation SIZE histogram (counts by size bucket, and the share of all allocs)");
    print!("{:<40}", "cell");
    for n in alloc_arm::BUCKET_NAMES {
        print!("{n:>9}");
    }
    println!();
    for (label, buckets, total) in &hist {
        print!("{label:<40}");
        for b in buckets {
            print!("{b:>9}");
        }
        println!();
        if *total > 0 {
            print!("{:<40}", "");
            for b in buckets {
                print!("{:>8.1}%", *b as f64 / *total as f64 * 100.0);
            }
            println!();
        }
    }
    ExitCode::SUCCESS
}

/// The deterministic work a parse performs, per corpus file.
///
/// This is the instrument that decides a brick when the clock cannot: exact,
/// one run, identical under any load, and it reports whether the fast path is
/// actually being taken -- which byte-identical output cannot, since a fast
/// path that quietly stopped being reached still produces the right answer.
///
/// Run it twice, `RJT_WS_FASTPATH=1` and `=0`, and diff.
/// Per-message latency, for the S4 container payloads.
///
/// Throughput and latency are different questions and can move in opposite
/// directions. "How fast can we chew 600 KB" is answered by `bench`; "how long
/// to handle ONE message" is answered here, and it is the question a service
/// with a p99 target actually asks. Per-call setup that vanishes into a
/// 600 KB average dominates a 230-byte message.
///
/// S4 ships containers because that is how the payloads arrive, so the records
/// are sliced out first (see `corpus::messages`, which uses the ORACLE's
/// `RawValue` so the original bytes survive) and then parsed one at a time,
/// cycling until `--messages` parses have happened.
///
/// THE TIMER IS THE INSTRUMENT HERE, so its own cost is measured and reported
/// rather than assumed away: a single `Instant::now()` pair costs tens of
/// nanoseconds, which is negligible against a 600 KB parse and is NOT
/// negligible against a small message. The overhead row below is measured in
/// the same loop shape with the parse removed, so a reader can subtract it.
fn latency(args: &[String]) -> ExitCode {
    let mut files: Vec<File> = Vec::new();
    let mut count = 10_000usize;
    let mut column = Column::DomParse;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut next = || -> Result<&str, String> {
            i += 1;
            args.get(i)
                .map(String::as_str)
                .ok_or_else(|| format!("{a} needs a value"))
        };
        let r = match a {
            "--all" => {
                files.extend(
                    File::S4
                        .iter()
                        .copied()
                        .filter(|f| f.message_array().is_some()),
                );
                Ok(())
            }
            "--file" => next().map(|v| match File::parse(v) {
                Some(f) => files.push(f),
                None => {
                    eprintln!("error: unknown file {v:?}");
                    std::process::exit(2);
                }
            }),
            "--messages" => next().and_then(|v| {
                v.parse()
                    .map(|n| count = n)
                    .map_err(|e| format!("--messages: {e}"))
            }),
            "--column" => next().and_then(|v| match Column::parse(v) {
                Some(c) => {
                    column = c;
                    Ok(())
                }
                None => Err(format!("--column: unknown column {v:?}")),
            }),
            other => Err(format!("unknown option {other:?}")),
        };
        if let Err(e) = r {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
        i += 1;
    }
    if files.is_empty() {
        files.extend(
            File::S4
                .iter()
                .copied()
                .filter(|f| f.message_array().is_some()),
        );
    }
    // Per-message latency on a struct needs a fixture for the ELEMENT type,
    // not the document type, and those do not exist yet. Refusing is better
    // than silently measuring the container.
    if !matches!(column, Column::DomParse | Column::Scan) {
        eprintln!(
            "error: --column {} needs a fixture for the element type, not the              document type; only dom-parse and scan are wired",
            column.name()
        );
        return ExitCode::from(2);
    }

    // The timer's own cost AND its granularity, in the same loop shape with
    // the parse removed. Both matter, and reporting only the first is how a
    // latency table quietly lies: on Windows `Instant` is backed by
    // QueryPerformanceCounter, whose tick is coarser than the cost of reading
    // it, so the measured overhead comes out as a flat 0 ns while every
    // reported figure is actually quantised to the tick. A p50 of 1,600 ns
    // against a 100 ns tick carries about 6% granularity error, and a reader
    // cannot know that from an overhead row alone.
    let mut overhead = Vec::with_capacity(4096);
    for _ in 0..4096 {
        let t = Instant::now();
        std::hint::black_box(());
        overhead.push(t.elapsed().as_nanos() as u64);
    }
    overhead.sort_unstable();
    let timer_ns = overhead[overhead.len() / 2];
    // Smallest non-zero gap the clock will admit to: its effective resolution.
    let mut tick_ns = u64::MAX;
    for _ in 0..4096 {
        let t = Instant::now();
        loop {
            let d = t.elapsed().as_nanos() as u64;
            if d > 0 {
                tick_ns = tick_ns.min(d);
                break;
            }
        }
    }

    println!(
        "per-message latency | column {} | {} parses per file",
        column.name(),
        count
    );
    println!(
        "timer: overhead {timer_ns} ns (median of the same loop with the parse removed),          resolution {tick_ns} ns (smallest non-zero interval it will report)"
    );
    if timer_ns == 0 {
        println!(
            "  NOTE: a 0 ns overhead means the read costs less than one tick, NOT that it is              free. Every figure below is quantised to {tick_ns} ns, so treat a p50 within a              few ticks of that as granularity rather than signal."
        );
    }
    println!(
        "  max is reported but is SCHEDULER noise on a shared machine, not the parser; p99 is          the number to operate on."
    );
    println!(
        "{:<20} {:>8} {:>9} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "file", "msgs", "mean B", "p50 ns", "p90 ns", "p99 ns", "max ns", "MB/s"
    );

    for file in files {
        let messages = corpus::messages(file);
        if messages.is_empty() {
            eprintln!("{}: no message array, skipped", file.name());
            continue;
        }
        let total_bytes: usize = messages.iter().map(Vec::len).sum();
        let mean_bytes = total_bytes / messages.len();

        let mut ns: Vec<u64> = Vec::with_capacity(count);
        for k in 0..count {
            let msg = &messages[k % messages.len()];
            let t = Instant::now();
            match column {
                Column::Scan => {
                    let v = turbo::from_slice::<serde::de::IgnoredAny>(msg).unwrap();
                    std::hint::black_box(&v);
                }
                _ => {
                    let v = turbo::from_slice::<turbo::Value>(msg).unwrap();
                    std::hint::black_box(&v);
                }
            }
            ns.push(t.elapsed().as_nanos() as u64);
        }
        ns.sort_unstable();
        let pick = |q: f64| ns[((ns.len() as f64 - 1.0) * q) as usize];
        let p50 = pick(0.50);
        // Throughput implied by the median message, for comparison with the
        // whole-document MB/s figures elsewhere. Stated from p50 rather than
        // the mean because the mean of a latency distribution is not the
        // number anyone operates on.
        let mbps = if p50 > timer_ns {
            mean_bytes as f64 / (p50 - timer_ns) as f64 * 1e3
        } else {
            f64::NAN
        };
        println!(
            "{:<20} {:>8} {:>9} {:>8} {:>8} {:>8} {:>8} {:>9.0}",
            file.name(),
            messages.len(),
            mean_bytes,
            p50,
            pick(0.90),
            pick(0.99),
            ns[ns.len() - 1],
            mbps
        );
    }
    ExitCode::SUCCESS
}

/// THE B6 CEILING PROBE: what would key dispatch cost if it were free?
///
/// Brick B6 (key matching on the JSON side) and B6h (hand a field INDEX across
/// the serde seam instead of a string) are both gated on one number, and the
/// mission plan says how to get it: stub key matching with an index lookup and
/// see what comes back. `b6_probe::FrameTelemetryFast` is that stub -- the same
/// fields, the same types, the same wire names, dispatching on a length and one
/// or two bytes with no string comparison at all.
///
/// Paired and interleaved in ONE process with the lead alternating, so the two
/// arms cannot differ by code layout, and the statistic is the min per
/// iteration in each window, as everywhere else in this harness.
///
/// It reports a CEILING. The fast matcher does not verify the whole key, so a
/// real brick would have to do more and would be slower. The right reading is
/// "key dispatch cannot be worth more than this".
fn b6(args: &[String]) -> ExitCode {
    let mut pairs = 21usize;
    let mut window_ms = 250u64;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        match a {
            "--pairs" | "--window-ms" => {
                let Some(v) = args.get(i).map(String::as_str) else {
                    eprintln!("error: {a} needs a value");
                    return ExitCode::from(2);
                };
                let Ok(parsed) = v.parse::<u64>() else {
                    eprintln!("error: {a}: {v:?} is not a number");
                    return ExitCode::from(2);
                };
                if a == "--pairs" {
                    pairs = parsed as usize;
                } else {
                    window_ms = parsed;
                }
                i += 1;
            }
            // Accepted and recorded by the pinning wrapper, not used here.
            "--pinned" | "--commit" => i += 1,
            other => {
                eprintln!("error: unknown option {other:?}");
                return ExitCode::from(2);
            }
        }
    }

    let input = File::S4FrameTelemetry.load();

    // WORK PARITY FIRST. A timing comparison between two paths that built
    // different values would be meaningless, so the two are checked equal on
    // the real document before anything is timed.
    {
        let derived: rjt_bench::s4_frame_telemetry::FrameTelemetry =
            turbo::from_slice(&input).expect("derived parse");
        let fast: rjt_bench::b6_probe::FrameTelemetryFast =
            turbo::from_slice(&input).expect("fast parse");
        assert_eq!(
            derived.frames.len(),
            fast.frames.len(),
            "the two paths disagreed on frame count"
        );
        for (d, f) in derived.frames.iter().zip(&fast.frames) {
            assert!(
                d.n == f.n
                    && d.pts == f.pts
                    && d.dts == f.dts
                    && d.qp.to_bits() == f.qp.to_bits()
                    && d.bits == f.bits
                    && d.psnr_y.to_bits() == f.psnr_y.to_bits()
                    && d.ssim.to_bits() == f.ssim.to_bits()
                    && d.mb_i == f.mb_i
                    && d.mb_skip == f.mb_skip
                    && d.satd == f.satd
                    && d.refs == f.refs
                    && d.ms.to_bits() == f.ms.to_bits(),
                "the two paths built different values for frame {}",
                d.n
            );
        }
        println!(
            "work parity: {} frames, every checked field equal in both paths",
            derived.frames.len()
        );
    }

    let window = Duration::from_millis(window_ms);
    let mut ratios: Vec<f64> = Vec::with_capacity(pairs);
    let mut derived_ns: Vec<u64> = Vec::with_capacity(pairs);
    let mut fast_ns: Vec<u64> = Vec::with_capacity(pairs);

    fn run_derived(input: &[u8]) -> u64 {
        let t = Instant::now();
        let v: rjt_bench::s4_frame_telemetry::FrameTelemetry = turbo::from_slice(input).unwrap();
        let ns = t.elapsed().as_nanos() as u64;
        std::hint::black_box(&v);
        ns
    }
    fn run_fast(input: &[u8]) -> u64 {
        let t = Instant::now();
        let v: rjt_bench::b6_probe::FrameTelemetryFast = turbo::from_slice(input).unwrap();
        let ns = t.elapsed().as_nanos() as u64;
        std::hint::black_box(&v);
        ns
    }
    let sample = |f: fn(&[u8]) -> u64, input: &[u8]| -> u64 {
        let start = Instant::now();
        let mut best = u64::MAX;
        let mut n = 0;
        loop {
            best = best.min(f(input));
            n += 1;
            if n >= 3 && start.elapsed() >= window {
                break;
            }
        }
        best
    };

    for p in 0..pairs {
        // ABBA: alternate which arm leads, so drift cannot settle on one.
        let (d, f) = if p % 2 == 0 {
            let d = sample(run_derived, &input);
            (d, sample(run_fast, &input))
        } else {
            let f = sample(run_fast, &input);
            (sample(run_derived, &input), f)
        };
        derived_ns.push(d);
        fast_ns.push(f);
        ratios.push(d as f64 / f as f64);
    }

    ratios.sort_by(f64::total_cmp);
    derived_ns.sort_unstable();
    fast_ns.sort_unstable();
    let median = ratios[ratios.len() / 2];
    let wins = ratios.iter().filter(|&&r| r > 1.0).count();
    let n = ratios.len() as f64;
    let z = (wins as f64 - n / 2.0) / (0.5 * n.sqrt());
    let mbps = |ns: u64| input.len() as f64 / ns as f64 * 1e3;

    println!();
    println!(
        "{:<26} {:>12} {:>12} {:>12} {:>12}",
        "arm", "median ns", "MB/s", "best ns", "best MB/s"
    );
    println!(
        "{:<26} {:>12} {:>12.0} {:>12} {:>12.0}",
        "derive (match on &str)",
        derived_ns[derived_ns.len() / 2],
        mbps(derived_ns[derived_ns.len() / 2]),
        derived_ns[0],
        mbps(derived_ns[0])
    );
    println!(
        "{:<26} {:>12} {:>12.0} {:>12} {:>12.0}",
        "free dispatch (CEILING)",
        fast_ns[fast_ns.len() / 2],
        mbps(fast_ns[fast_ns.len() / 2]),
        fast_ns[0],
        mbps(fast_ns[0])
    );
    println!();
    println!(
        "CEILING: derive/free = {:.3}x  [{:.3}, {:.3}]  {}/{} pairs, z = {:+.2}   best-of-N {:.3}x",
        median,
        ratios[0],
        ratios[ratios.len() - 1],
        wins,
        pairs,
        z,
        derived_ns[0] as f64 / fast_ns[0] as f64
    );
    println!(
        "method: in-process paired A/B, one binary, ABBA with the lead alternated; pairs={}; \
         window={} ms per arm-sample, statistic=min per-iteration time in the window; \
         verdict=median of paired ratios + wins with z, and a best-of-N that noise can only \
         inflate; work parity asserted field-by-field before timing; \
         document=s4-frame-telemetry ({} bytes, 46,816 keys, 18 per object, the highest key \
         density in the corpus); isa={}; allocator={}",
        pairs,
        window_ms,
        input.len(),
        turbo::counters::isa(),
        alloc_arm::name()
    );
    println!(
        "NOTE: the free arm does NOT verify whole keys, so this is an UPPER BOUND on what any \
         correct brick could win, not a prediction for one."
    );
    ExitCode::SUCCESS
}

/// Parse into the fixture type and drop it, with the counters running.
///
/// Generic so `by_fixture!` -- the single file-to-type list in `cells` -- can
/// drive it, rather than this verb keeping a second copy of that list that
/// could drift out of step with the first.
fn count_struct_parse<T: cells::Fixture>(input: &[u8]) {
    drop(turbo::from_slice::<T>(input).unwrap());
}

/// Parse OUTSIDE the counted region, reset, then serialize into the counting
/// sink, so the numbers describe serialize work only.
fn count_struct_stringify<T: cells::Fixture>(
    input: &[u8],
    sink: &mut rjt_bench::wcount::CountingWriter,
) {
    let value: T = turbo::from_slice(input).unwrap();
    turbo::counters::reset();
    turbo::to_writer(sink, &value).unwrap();
}

fn work(args: &[String]) -> ExitCode {
    let opts = match parse_opts(args, None) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if !turbo::counters::enabled() {
        eprintln!(
            "error: this build has no work counters -- rebuild the LIBRARY with \
             --features rusty_json_turbo/profile (and never quote a timing from it)"
        );
        return ExitCode::from(2);
    }
    println!(
        "work counters | ws fast path: {} | allocator {} | isa {} (machine offers {})",
        std::env::var("RJT_WS_FASTPATH").unwrap_or_else(|_| "1 (default)".into()),
        alloc_arm::name(),
        turbo::counters::isa(),
        turbo::counters::isa_available()
    );
    println!();
    println!("PARSE work -- per document, deterministic");
    println!(
        "{:<14} {:<14} {:>11} {:>11} {:>11} {:>11} {:>11} {:>13} {:>11} {:>11} {:>13} {:>9} {:>16}",
        "file",
        "column",
        "peek",
        "next",
        "discard",
        "ws_runs",
        "ws_bytes",
        "d8 hit/call",
        "str_scans",
        "str_bytes",
        "borrow/copy",
        "keys",
        "utf8 B/calls"
    );
    let mut ser_rows: Vec<String> = Vec::new();
    let mut seen = Vec::new();
    for (file, column) in &opts.cells {
        if seen.contains(&(*file, *column)) {
            continue;
        }
        seen.push((*file, *column));
        let input = file.load();
        turbo::counters::reset();
        let before = turbo::counters::snapshot();
        match column {
            Column::DomParse => {
                let v: turbo::Value = turbo::from_slice(&input).unwrap();
                drop(v);
            }
            Column::Scan => {
                turbo::from_slice::<serde::de::IgnoredAny>(&input).unwrap();
            }
            Column::StructParse => {
                rjt_bench::by_fixture!(*file, count_struct_parse, (&input));
            }
            Column::DomStringify | Column::StructStringify => {
                // Parse FIRST, then reset, so only serialize work is counted.
                // The sink counts its own calls, which is how brick B5 gets
                // priced without a library change.
                let mut sink = rjt_bench::wcount::CountingWriter::with_capacity(input.len());
                if matches!(column, Column::DomStringify) {
                    let dom: turbo::Value = turbo::from_slice(&input).unwrap();
                    turbo::counters::reset();
                    turbo::to_writer(&mut sink, &dom).unwrap();
                } else {
                    rjt_bench::by_fixture!(*file, count_struct_stringify, (&input, &mut sink));
                }
                let c = turbo::counters::snapshot();
                let esc_clean = c.esc_bytes.saturating_sub(c.esc_hits);
                ser_rows.push(format!(
                    "{:<14} {:<14} {:>11} {:>11} {:>11} {:>11} {:>9.4}% {:>11} {:>11} {:>11.1}",
                    file.name(),
                    column.name(),
                    c.esc_bytes,
                    c.esc_hits,
                    c.esc_frags,
                    c.esc_steps,
                    if c.esc_bytes == 0 {
                        0.0
                    } else {
                        c.esc_hits as f64 / c.esc_bytes as f64 * 100.0
                    },
                    esc_clean,
                    sink.total_calls(),
                    sink.bytes_per_call()
                ));
                continue;
            }
        }
        let c = turbo::counters::snapshot().since(before);
        println!(
            "{:<14} {:<14} {:>11} {:>11} {:>11} {:>11} {:>11} {:>13} {:>11} {:>11} {:>13} {:>9} {:>16}",
            file.name(),
            column.name(),
            c.peek,
            c.next,
            c.discard,
            c.ws_runs,
            c.ws_bytes,
            format!("{}/{}", c.d8_hits, c.d8_calls),
            c.str_scans,
            c.str_bytes,
            format!("{}/{}", c.str_borrows, c.str_copies),
            c.keys,
            format!("{}/{}", c.utf8_bytes, c.utf8_calls)
        );
    }
    if !ser_rows.is_empty() {
        println!();
        println!("SERIALIZE work -- per document, deterministic");
        println!(
            "{:<14} {:<14} {:>11} {:>11} {:>11} {:>11} {:>10} {:>11} {:>11} {:>11}",
            "file",
            "column",
            "esc_bytes",
            "esc_hits",
            "esc_frags",
            "esc_steps",
            "hit rate",
            "clean bytes",
            "sink calls",
            "bytes/call"
        );
        for row in &ser_rows {
            println!("{row}");
        }
    }
    ExitCode::SUCCESS
}

/// Price every brick before building any of it.
///
/// Two instruments, both cheap, in the order `codec-measurement` puts them:
///
/// 1. A **content census** -- a deterministic count of where the document's
///    bytes go. A brick that targets whitespace cannot win more than the
///    whitespace costs, and that is knowable exactly, on any machine, without
///    a clock.
/// 2. A **scan-only ceiling** -- `IgnoredAny` walks the document and builds
///    nothing. Whatever `dom-parse` costs above it is what building the value
///    costs, which is the most any value-construction brick can win back.
///    No stubbing, so the probe cannot mis-measure by deleting a branch the
///    program depends on.
fn probe(args: &[String]) -> ExitCode {
    let opts = match parse_opts(args, None) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    settle(opts.settle_ms);

    let files: Vec<File> = {
        let mut v: Vec<File> = Vec::new();
        for (f, _) in &opts.cells {
            if !v.contains(f) {
                v.push(*f);
            }
        }
        v
    };

    println!("== content census (deterministic; no clock involved)\n");
    println!(
        "{:<14} {:>10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>9}",
        "file", "bytes", "ws%", "struct%", "string%", "number%", "n-ascii%", "strings", "numbers"
    );
    let mut censuses = Vec::new();
    for f in &files {
        let input = f.load();
        let c = content::census(&input);
        println!(
            "{:<14} {:>10} {:>7.1}% {:>7.1}% {:>7.1}% {:>7.1}% {:>7.1}% {:>9} {:>9}",
            f.name(),
            c.bytes,
            c.whitespace_pct(),
            c.structural_pct(),
            c.string_pct(),
            c.number_pct(),
            c.non_ascii_pct(),
            c.strings,
            c.numbers
        );
        censuses.push((*f, c));
    }
    println!(
        "\n{:<14} {:>9} {:>11} {:>9} {:>12} {:>10} {:>12}",
        "file", "escaped%", "mean-str", "longest", "escape-B", "floats", "literals-B"
    );
    for (f, c) in &censuses {
        println!(
            "{:<14} {:>8.1}% {:>11.1} {:>9} {:>12} {:>10} {:>12}",
            f.name(),
            c.escaped_string_pct(),
            c.mean_string_len(),
            c.longest_string,
            c.escape_bytes,
            c.floats,
            c.literal_bytes
        );
    }

    // The distribution that decides a wide whitespace scan. A step of N bytes
    // can only accelerate bytes living in runs of at least N; every shorter run
    // pays the wide path's setup for nothing. The mean cannot tell these apart.
    println!("\nwhitespace RUNS by length (runs / bytes):");
    print!("{:<14}", "file");
    for b in content::WS_BUCKETS {
        print!(" {b:>15}");
    }
    println!(" {:>9} {:>9}", "mean", "longest");
    for (f, c) in &censuses {
        print!("{:<14}", f.name());
        for i in 0..content::WS_BUCKETS.len() {
            print!(" {:>7}/{:<7}", c.ws_runs_by_len[i], c.ws_bytes_by_len[i]);
        }
        println!(" {:>9.2} {:>9}", c.mean_ws_run(), c.longest_ws_run);
    }
    println!("\nshare of whitespace BYTES in runs of at least N (the wide-scan bound):");
    print!("{:<14}", "file");
    for t in content::WS_THRESHOLDS {
        print!("{:>10}", format!(">={t}"));
    }
    println!("{:>10}", "longest");
    for (f, c) in &censuses {
        print!("{:<14}", f.name());
        for t in content::WS_THRESHOLDS {
            print!("{:>9.1}%", c.ws_bytes_in_runs_of_at_least(t));
        }
        println!("{:>10}", c.longest_ws_run);
    }

    // What a wider step would actually buy, which is NOT what the threshold
    // table above says. That one bounds how often a step is FULLY USED; this
    // counts the steps TAKEN, and a wide load that overshoots the end of a run
    // still resolves it in one iteration.
    println!("\nwide-scan STEPS over whitespace, by step size (4-byte scalar peel, as shipped):");
    println!(
        "{:<14} {:>12} {:>12} {:>12} {:>12}   vs 8B",
        "file", "8B (now)", "16B (SSE2)", "32B (AVX2)", "64B"
    );
    for (f, c) in &censuses {
        let s8 = c.wide_steps(8, 4);
        let s16 = c.wide_steps(16, 4);
        let s32 = c.wide_steps(32, 4);
        let s64 = c.wide_steps(64, 4);
        println!(
            "{:<14} {:>12} {:>12} {:>12} {:>12}   16B {:.2}x, 32B {:.2}x",
            f.name(),
            s8,
            s16,
            s32,
            s64,
            s8 as f64 / s16.max(1) as f64,
            s8 as f64 / s32.max(1) as f64
        );
    }

    println!("\n== ceiling probe: what is left when nothing is built\n");
    println!(
        "arm=ours allocator={} rounds={} window={} ms  (cell order ROTATED each round, \
         so no cell is always first; per-cell statistic = min over rounds of the \
         min-per-iteration in that round's window)",
        alloc_arm::name(),
        opts.cfg.pairs,
        opts.cfg.window.as_millis()
    );
    if alloc_arm::counting() {
        println!("!! COUNTING BUILD: timings here are taxed and not quotable");
    }

    for f in &files {
        let input = f.load();
        // The same document with the whitespace a parser skips removed, and
        // nothing else changed. If the two do not parse equal, the probe is
        // measuring two different programs and its numbers are worthless.
        let stripped = content::strip_whitespace(&input);
        let a: turbo::Value = turbo::from_slice(&input).expect("corpus parses");
        let b: turbo::Value = turbo::from_slice(&stripped).expect("stripped corpus parses");
        assert_eq!(a, b, "{}: stripping whitespace changed the value", f.name());
        drop((a, b));

        // (label, column, input) -- rotated each round so no job is always first.
        let jobs: [(&str, Column, &[u8]); 5] = [
            ("scan", Column::Scan, &input),
            ("scan, no ws", Column::Scan, &stripped),
            ("struct-parse", Column::StructParse, &input),
            ("dom-parse", Column::DomParse, &input),
            ("dom-parse, no ws", Column::DomParse, &stripped),
        ];
        let mut best = [u64::MAX; 5];
        for round in 0..opts.cfg.pairs {
            for k in 0..jobs.len() {
                let idx = (k + round) % jobs.len();
                let (_, col, data) = jobs[idx];
                let s = cells::run(Arm::Ours, *f, col, data, opts.cfg.window);
                best[idx] = best[idx].min(s.min_ns());
            }
        }
        let (scan, scan_nows) = (best[0] as f64, best[1] as f64);
        let (structp, dom, dom_nows) = (best[2] as f64, best[3] as f64, best[4] as f64);

        println!(
            "\n-- {} ({} bytes; {} with whitespace removed)",
            f.name(),
            input.len(),
            stripped.len()
        );
        for (i, (label, _, data)) in jobs.iter().enumerate() {
            println!(
                "   {:<18} {:>10} ns  {:>7.0} MB/s",
                label,
                best[i],
                data.len() as f64 / best[i] as f64 * 1e3
            );
        }
        println!(
            "   scanning is {:.0}% of dom-parse and {:.0}% of struct-parse",
            scan / dom * 100.0,
            scan / structp * 100.0
        );
        println!(
            "   CEILING, value construction free : {:.2}x on dom-parse",
            dom / scan
        );
        println!(
            "   CEILING, struct machinery free   : {:.2}x on struct-parse",
            structp / scan
        );
        println!(
            "   CEILING, whitespace skip free    : {:.2}x on scan ({:.0}% of it), \
             {:.2}x on dom-parse ({:.0}% of it)",
            scan / scan_nows,
            (scan - scan_nows) / scan * 100.0,
            dom / dom_nows,
            (dom - dom_nows) / dom * 100.0
        );
    }
    println!(
        "\nRead these as shares, not as verdicts: the cells run block-wise inside \
         one process (rotated, not paired), so a few percent of drift is expected. \
         A ceiling is used to decide whether a brick is worth BUILDING -- and the \
         arithmetic that matters is share x plausible speedup against the floor."
    );
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
