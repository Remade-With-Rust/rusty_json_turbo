//! Paired A/B with the measurement discipline built in, not remembered.
//!
//! - Arms alternate which one LEADS each pair (ABBA), so drift cannot land on
//!   one block and "the second one is warmer" cancels.
//! - Every pair records the per-iteration MINIMUM inside a window for each
//!   arm; the verdict is the median of the paired ratios plus the paired win
//!   count and its z-score, never a single pair.
//! - Work parity is asserted per pair for the serde_json-shaped arms (output
//!   lengths equal; input identical by construction).
//! - The method line is printed with every table; a number without it is not
//!   evidence. The null arm (an arm against itself) is the session's floor.

use std::time::Duration;

use crate::cells::{self, Arm, Column};
use crate::corpus::File;

#[derive(Clone, Debug)]
pub struct Config {
    pub pairs: usize,
    pub window: Duration,
    /// What the wrapper pinned this process to, echoed into the method line.
    pub pinned: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Pair {
    /// Which arm ran first in this pair.
    pub lead: Arm,
    pub a_ns: u64,
    pub b_ns: u64,
    pub a_iters: usize,
    pub b_iters: usize,
}

#[derive(Clone, Debug)]
pub struct Paired {
    pub a: Arm,
    pub b: Arm,
    pub file: File,
    pub column: Column,
    pub bytes: usize,
    pub pairs: Vec<Pair>,
}

impl Paired {
    pub fn n(&self) -> usize {
        self.pairs.len()
    }

    /// `a_ns / b_ns` per pair: below 1 means `a` is faster.
    pub fn ratios(&self) -> Vec<f64> {
        self.pairs
            .iter()
            .map(|p| p.a_ns as f64 / p.b_ns as f64)
            .collect()
    }

    pub fn median_ratio(&self) -> f64 {
        median_f64(&self.ratios())
    }

    pub fn spread(&self) -> (f64, f64) {
        let r = self.ratios();
        let min = r.iter().copied().fold(f64::INFINITY, f64::min);
        let max = r.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (min, max)
    }

    /// Pairs in which `a` was strictly faster.
    pub fn wins(&self) -> usize {
        self.pairs.iter().filter(|p| p.a_ns < p.b_ns).count()
    }

    pub fn z(&self) -> f64 {
        let n = self.n() as f64;
        (self.wins() as f64 - n / 2.0) / (0.5 * n.sqrt())
    }

    pub fn median_a_ns(&self) -> u64 {
        median_u64(self.pairs.iter().map(|p| p.a_ns))
    }

    pub fn median_b_ns(&self) -> u64 {
        median_u64(self.pairs.iter().map(|p| p.b_ns))
    }

    pub fn mbs_a(&self) -> f64 {
        mbs(self.bytes, self.median_a_ns())
    }

    pub fn mbs_b(&self) -> f64 {
        mbs(self.bytes, self.median_b_ns())
    }

    pub fn is_null_arm(&self) -> bool {
        self.a == self.b
    }

    /// One markdown row in the ledger's shape.
    pub fn markdown_row(&self) -> String {
        let (lo, hi) = self.spread();
        format!(
            "| {} | {} | {} **{:.0} MB/s** | {} {:.0} MB/s | **{:.3}x** [{:.3}, {:.3}] | {}/{}, z = {:+.2} |",
            self.file.name(),
            self.column.name(),
            self.a.name(),
            self.mbs_a(),
            self.b.name(),
            self.mbs_b(),
            self.median_ratio(),
            lo,
            hi,
            self.wins(),
            self.n(),
            self.z()
        )
    }

    pub fn markdown_header(&self) -> String {
        format!(
            "| file | column | {} | {} | {}/{} time (median [min, max]) | wins, z |\n|---|---|---:|---:|---:|---:|",
            self.a.name(),
            self.b.name(),
            self.a.name(),
            self.b.name()
        )
    }
}

fn mbs(bytes: usize, ns: u64) -> f64 {
    if ns == 0 {
        return 0.0;
    }
    bytes as f64 / ns as f64 * 1e3
}

fn median_f64(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if s.is_empty() {
        return f64::NAN;
    }
    let mid = s.len() / 2;
    if s.len() % 2 == 0 {
        (s[mid - 1] + s[mid]) / 2.0
    } else {
        s[mid]
    }
}

fn median_u64(it: impl Iterator<Item = u64>) -> u64 {
    let mut s: Vec<u64> = it.collect();
    s.sort_unstable();
    if s.is_empty() {
        return 0;
    }
    s[s.len() / 2]
}

/// Run `cfg.pairs` pairs of (a, b) on one cell, alternating the leading arm.
/// `on_pair` sees each pair as it lands so a long run prints progress.
pub fn run_paired(
    cfg: &Config,
    a: Arm,
    b: Arm,
    file: File,
    column: Column,
    input: &[u8],
    mut on_pair: impl FnMut(usize, &Pair),
) -> Paired {
    assert!(
        a.available() && b.available(),
        "arm not built in: use --features competitors"
    );
    let mut pairs = Vec::with_capacity(cfg.pairs);
    let mut bytes = 0usize;
    for i in 0..cfg.pairs {
        // ABBA: even pairs run a first, odd pairs run b first.
        let a_leads = i % 2 == 0;
        let (first, second) = if a_leads { (a, b) } else { (b, a) };
        let s1 = cells::run(first, file, column, input, cfg.window);
        let s2 = cells::run(second, file, column, input, cfg.window);
        let (sa, sb) = if a_leads { (s1, s2) } else { (s2, s1) };
        if a.is_serde_json_shaped() && b.is_serde_json_shaped() {
            assert_eq!(
                sa.bytes,
                sb.bytes,
                "work parity: {}/{} output lengths differ ({} vs {})",
                file.name(),
                column.name(),
                sa.bytes,
                sb.bytes
            );
        }
        bytes = sa.bytes;
        let pair = Pair {
            lead: first,
            a_ns: sa.min_ns(),
            b_ns: sb.min_ns(),
            a_iters: sa.iters(),
            b_iters: sb.iters(),
        };
        on_pair(i, &pair);
        pairs.push(pair);
    }
    Paired {
        a,
        b,
        file,
        column,
        bytes,
        pairs,
    }
}

/// The provenance every table carries.
pub fn method_line(cfg: &Config, commit: &str) -> String {
    format!(
        "method: in-process paired A/B, one binary, both arms linked; lead alternated per pair (ABBA); \
         pairs={}; window={} ms per arm-sample, statistic=min per-iteration time in the window, \
         verdict=median of paired ratios + paired wins with z; clock=std::time::Instant (QPC on Windows); \
         pinned={}; allocator={} (BOTH arms -- it is a property of the binary); \
         isa=scalar (no kernels wired); work parity: identical input bytes, output length asserted equal \
         per pair for serde_json-shaped arms; stringify buffers pre-sized and cleared (growth excluded); \
         competitors: simd-json input copy excluded from the timed region; \
         machine={} {} ; commit={}{}",
        cfg.pairs,
        cfg.window.as_millis(),
        cfg.pinned,
        crate::alloc_arm::name(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        commit,
        if crate::alloc_arm::counting() {
            " ; !! COUNTING BUILD: every allocation is taxed, these timings are NOT quotable"
        } else {
            ""
        }
    )
}
