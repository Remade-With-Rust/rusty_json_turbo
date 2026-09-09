//! The four json-benchmark columns over the three files, for every arm.
//!
//! Timing happens INSIDE each op so the value under test is dropped after the
//! clock stops (json-benchmark does the same: `_keep` outlives `timer.stop()`)
//! and so simd-json's mandatory input copy stays outside the timed region.
//! Stringify columns write into a pre-sized `Vec` that is cleared, not
//! reallocated, between iterations -- buffer growth is excluded, as upstream's
//! harness excludes it, and the ledger says so.

use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::corpus::File;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm {
    Ours,
    Upstream,
    SimdJson,
    SonicRs,
}

impl Arm {
    pub const ALL: [Arm; 4] = [Arm::Ours, Arm::Upstream, Arm::SimdJson, Arm::SonicRs];

    pub fn parse(s: &str) -> Option<Arm> {
        match s {
            "ours" | "turbo" => Some(Arm::Ours),
            "upstream" | "serde_json" => Some(Arm::Upstream),
            "simd-json" | "simd_json" => Some(Arm::SimdJson),
            "sonic-rs" | "sonic_rs" => Some(Arm::SonicRs),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Arm::Ours => "ours",
            Arm::Upstream => "upstream",
            Arm::SimdJson => "simd-json",
            Arm::SonicRs => "sonic-rs",
        }
    }

    /// Competitor arms exist only under `--features competitors`.
    pub fn available(self) -> bool {
        match self {
            Arm::Ours | Arm::Upstream => true,
            Arm::SimdJson | Arm::SonicRs => cfg!(feature = "competitors"),
        }
    }

    /// Both serde_json-shaped arms must produce byte-identical output, so their
    /// output lengths are asserted equal per pair (work parity). Competitors
    /// may legitimately print numbers differently; for them it is reported.
    pub fn is_serde_json_shaped(self) -> bool {
        matches!(self, Arm::Ours | Arm::Upstream)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    DomParse,
    DomStringify,
    StructParse,
    StructStringify,
}

impl Column {
    pub const ALL: [Column; 4] = [
        Column::DomParse,
        Column::DomStringify,
        Column::StructParse,
        Column::StructStringify,
    ];

    pub fn parse(s: &str) -> Option<Column> {
        match s {
            "dom-parse" | "dom_parse" => Some(Column::DomParse),
            "dom-stringify" | "dom_stringify" => Some(Column::DomStringify),
            "struct-parse" | "struct_parse" => Some(Column::StructParse),
            "struct-stringify" | "struct_stringify" => Some(Column::StructStringify),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Column::DomParse => "dom-parse",
            Column::DomStringify => "dom-stringify",
            Column::StructParse => "struct-parse",
            Column::StructStringify => "struct-stringify",
        }
    }
}

/// One arm-sample: every per-iteration time observed inside one window, and
/// the byte count the throughput is denominated in (input for parse, output
/// for stringify).
pub struct Sample {
    pub ns: Vec<u64>,
    pub bytes: usize,
}

impl Sample {
    pub fn min_ns(&self) -> u64 {
        self.ns.iter().copied().min().unwrap_or(0)
    }

    pub fn median_ns(&self) -> u64 {
        let mut v = self.ns.clone();
        v.sort_unstable();
        v[v.len() / 2]
    }

    pub fn iters(&self) -> usize {
        self.ns.len()
    }
}

/// The struct each file deserialises into.
pub trait Fixture: serde::de::DeserializeOwned + serde::Serialize {}
impl Fixture for crate::canada::Canada {}
impl Fixture for crate::citm_catalog::CitmCatalog {}
impl Fixture for crate::twitter::Twitter {}

/// Run one (arm, file, column) cell for `window`, returning every iteration.
pub fn run(arm: Arm, file: File, column: Column, input: &[u8], window: Duration) -> Sample {
    match file {
        File::Canada => run_typed::<crate::canada::Canada>(arm, column, input, window),
        File::CitmCatalog => {
            run_typed::<crate::citm_catalog::CitmCatalog>(arm, column, input, window)
        }
        File::Twitter => run_typed::<crate::twitter::Twitter>(arm, column, input, window),
    }
}

/// Repeat `op` until `window` has elapsed (and at least three times), collecting
/// the nanoseconds each op reports for its own timed region.
fn measure(window: Duration, mut op: impl FnMut() -> u64) -> Vec<u64> {
    let mut ns = Vec::with_capacity(64);
    let start = Instant::now();
    loop {
        ns.push(op());
        if ns.len() >= 3 && start.elapsed() >= window {
            break;
        }
    }
    ns
}

/// Time one call; the result is kept alive until after the clock stops.
fn timed<R>(f: impl FnOnce() -> R) -> u64 {
    let t0 = Instant::now();
    let r = f();
    let ns = t0.elapsed().as_nanos() as u64;
    black_box(&r);
    drop(r);
    ns
}

fn run_typed<T: Fixture>(arm: Arm, column: Column, input: &[u8], window: Duration) -> Sample {
    match arm {
        Arm::Ours => ours::<T>(column, input, window),
        Arm::Upstream => upstream::<T>(column, input, window),
        #[cfg(feature = "competitors")]
        Arm::SimdJson => simd_json_arm::<T>(column, input, window),
        #[cfg(feature = "competitors")]
        Arm::SonicRs => sonic_rs_arm::<T>(column, input, window),
        #[cfg(not(feature = "competitors"))]
        Arm::SimdJson | Arm::SonicRs => {
            panic!("arm {} needs --features competitors", arm.name())
        }
    }
}

/// The two serde_json-shaped arms are the same code against two crates.
macro_rules! serde_json_arm {
    ($name:ident, $json:ident) => {
        fn $name<T: Fixture>(column: Column, input: &[u8], window: Duration) -> Sample {
            match column {
                Column::DomParse => {
                    let ns = measure(window, || {
                        timed(|| {
                            let s = std::str::from_utf8(black_box(input)).unwrap();
                            $json::from_str::<$json::Value>(s).unwrap()
                        })
                    });
                    Sample {
                        ns,
                        bytes: input.len(),
                    }
                }
                Column::DomStringify => {
                    let dom: $json::Value = $json::from_slice(input).unwrap();
                    let mut buf = Vec::with_capacity(input.len());
                    let mut out = 0usize;
                    let ns = measure(window, || {
                        buf.clear();
                        let n = timed(|| $json::to_writer(&mut buf, &dom).unwrap());
                        out = buf.len();
                        n
                    });
                    Sample { ns, bytes: out }
                }
                Column::StructParse => {
                    let ns = measure(window, || {
                        timed(|| {
                            let s = std::str::from_utf8(black_box(input)).unwrap();
                            $json::from_str::<T>(s).unwrap()
                        })
                    });
                    Sample {
                        ns,
                        bytes: input.len(),
                    }
                }
                Column::StructStringify => {
                    let value: T = $json::from_slice(input).unwrap();
                    let mut buf = Vec::with_capacity(input.len());
                    let mut out = 0usize;
                    let ns = measure(window, || {
                        buf.clear();
                        let n = timed(|| $json::to_writer(&mut buf, &value).unwrap());
                        out = buf.len();
                        n
                    });
                    Sample { ns, bytes: out }
                }
            }
        }
    };
}

serde_json_arm!(ours, turbo);
serde_json_arm!(upstream, serde_json_upstream);

#[cfg(feature = "competitors")]
fn simd_json_arm<T: Fixture>(column: Column, input: &[u8], window: Duration) -> Sample {
    // simd-json parses in place, so every parse needs a fresh copy of the
    // input. The copy is outside the timed region, as in json-benchmark.
    let mut scratch = input.to_vec();
    match column {
        Column::DomParse => {
            let ns = measure(window, || {
                scratch.copy_from_slice(input);
                timed(|| simd_json::to_borrowed_value(&mut scratch).unwrap())
            });
            Sample {
                ns,
                bytes: input.len(),
            }
        }
        Column::DomStringify => {
            use simd_json::prelude::Writable;
            let dom = simd_json::to_borrowed_value(&mut scratch).unwrap();
            let mut buf = Vec::with_capacity(input.len());
            let mut out = 0usize;
            let ns = measure(window, || {
                buf.clear();
                let n = timed(|| dom.write(&mut buf).unwrap());
                out = buf.len();
                n
            });
            Sample { ns, bytes: out }
        }
        Column::StructParse => {
            let ns = measure(window, || {
                scratch.copy_from_slice(input);
                timed(|| simd_json::serde::from_slice::<T>(&mut scratch).unwrap())
            });
            Sample {
                ns,
                bytes: input.len(),
            }
        }
        Column::StructStringify => {
            let value: T = simd_json::serde::from_slice(&mut scratch).unwrap();
            let mut buf = Vec::with_capacity(input.len());
            let mut out = 0usize;
            let ns = measure(window, || {
                buf.clear();
                let n = timed(|| simd_json::serde::to_writer(&mut buf, &value).unwrap());
                out = buf.len();
                n
            });
            Sample { ns, bytes: out }
        }
    }
}

#[cfg(feature = "competitors")]
fn sonic_rs_arm<T: Fixture>(column: Column, input: &[u8], window: Duration) -> Sample {
    let text = std::str::from_utf8(input).unwrap();
    match column {
        Column::DomParse => {
            let ns = measure(window, || {
                timed(|| sonic_rs::from_str::<sonic_rs::Value>(black_box(text)).unwrap())
            });
            Sample {
                ns,
                bytes: input.len(),
            }
        }
        Column::DomStringify => {
            let dom: sonic_rs::Value = sonic_rs::from_str(text).unwrap();
            let mut buf = Vec::with_capacity(input.len());
            let mut out = 0usize;
            let ns = measure(window, || {
                buf.clear();
                let n = timed(|| sonic_rs::to_writer(&mut buf, &dom).unwrap());
                out = buf.len();
                n
            });
            Sample { ns, bytes: out }
        }
        Column::StructParse => {
            let ns = measure(window, || {
                timed(|| sonic_rs::from_str::<T>(black_box(text)).unwrap())
            });
            Sample {
                ns,
                bytes: input.len(),
            }
        }
        Column::StructStringify => {
            let value: T = sonic_rs::from_str(text).unwrap();
            let mut buf = Vec::with_capacity(input.len());
            let mut out = 0usize;
            let ns = measure(window, || {
                buf.clear();
                let n = timed(|| sonic_rs::to_writer(&mut buf, &value).unwrap());
                out = buf.len();
                n
            });
            Sample { ns, bytes: out }
        }
    }
}
