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
    /// Walk the document and build **nothing** (`IgnoredAny`).
    ///
    /// The ceiling probe for every value-construction brick, and it needs no
    /// stubbing: this is a real, correct API doing real scanning work, so it
    /// cannot mis-measure by removing a branch the rest of the program depends
    /// on. Whatever `dom-parse` costs above this is what building the `Value`
    /// costs, and that is the most any DOM brick can ever win back.
    Scan,
}

impl Column {
    /// The four json-benchmark columns, unchanged, so every table published so
    /// far stays comparable. `Scan` is deliberately NOT here.
    pub const ALL: [Column; 4] = [
        Column::DomParse,
        Column::DomStringify,
        Column::StructParse,
        Column::StructStringify,
    ];

    /// The three cells a ceiling probe compares, cheapest first.
    pub const PROBE: [Column; 3] = [Column::Scan, Column::StructParse, Column::DomParse];

    pub fn parse(s: &str) -> Option<Column> {
        match s {
            "dom-parse" | "dom_parse" => Some(Column::DomParse),
            "dom-stringify" | "dom_stringify" => Some(Column::DomStringify),
            "struct-parse" | "struct_parse" => Some(Column::StructParse),
            "struct-stringify" | "struct_stringify" => Some(Column::StructStringify),
            "scan" => Some(Column::Scan),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Column::DomParse => "dom-parse",
            Column::DomStringify => "dom-stringify",
            Column::StructParse => "struct-parse",
            Column::StructStringify => "struct-stringify",
            Column::Scan => "scan",
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
impl Fixture for crate::s4_media_probe::MediaProbe {}
impl Fixture for crate::s4_frame_telemetry::FrameTelemetry {}
impl Fixture for crate::s4_signin_batch::SigninBatch {}
impl Fixture for crate::s4_sync_envelope::SyncEnvelope {}
impl Fixture for crate::s4_node_config::NodeConfig {}
impl Fixture for crate::s4_vault_shard::VaultShard {}
impl Fixture for crate::s4_ocr_i18n::OcrI18n {}
impl Fixture for crate::s5_log_stream::LogRecord {}

/// Call a generic function with the fixture type belonging to `$file`.
///
/// THE ONLY place a corpus file is mapped to its Rust type. Every dispatch
/// site expands this one list, so a file cannot be registered for the census
/// and forgotten for the clock -- a split that would report a number for a
/// cell nobody was actually running.
///
/// `$call` is an `ident` rather than a `path` because a `path` fragment cannot
/// be followed by a turbofish, and every arm needs one. Paths are `$crate`-
/// rooted so the binary crate can expand this too.
#[macro_export]
macro_rules! by_fixture {
    ($file:expr, $call:ident, ($($arg:expr),* $(,)?)) => {
        match $file {
            $crate::corpus::File::Canada => $call::<$crate::canada::Canada>($($arg),*),
            $crate::corpus::File::CitmCatalog => {
                $call::<$crate::citm_catalog::CitmCatalog>($($arg),*)
            }
            $crate::corpus::File::Twitter => $call::<$crate::twitter::Twitter>($($arg),*),
            $crate::corpus::File::S4MediaProbe => {
                $call::<$crate::s4_media_probe::MediaProbe>($($arg),*)
            }
            $crate::corpus::File::S4FrameTelemetry => {
                $call::<$crate::s4_frame_telemetry::FrameTelemetry>($($arg),*)
            }
            $crate::corpus::File::S4SigninBatch => {
                $call::<$crate::s4_signin_batch::SigninBatch>($($arg),*)
            }
            $crate::corpus::File::S4SyncEnvelope => {
                $call::<$crate::s4_sync_envelope::SyncEnvelope>($($arg),*)
            }
            $crate::corpus::File::S4NodeConfig => {
                $call::<$crate::s4_node_config::NodeConfig>($($arg),*)
            }
            $crate::corpus::File::S4VaultShard => {
                $call::<$crate::s4_vault_shard::VaultShard>($($arg),*)
            }
            $crate::corpus::File::S4OcrI18n => {
                $call::<$crate::s4_ocr_i18n::OcrI18n>($($arg),*)
            }
            // S5's "document" is ONE LINE of the stream, not the file. The
            // file itself is not valid JSON, so a whole-file column on it is
            // meaningless -- `cells::run` is never asked for one, and the
            // stream verb slices the lines first.
            $crate::corpus::File::S5LogStream
            | $crate::corpus::File::S5LogStreamPretty => {
                $call::<$crate::s5_log_stream::LogRecord>($($arg),*)
            }
        }
    };
}

/// One `ours` op for a cell, with the allocations it made.
///
/// Exactly one iteration, so the count is per-operation and not an average.
/// Returns `None` for the census on a build without the counting wrapper.
pub fn census_once(
    file: File,
    column: Column,
    input: &[u8],
) -> (usize, Option<crate::alloc_arm::Census>) {
    crate::by_fixture!(file, census_typed, (column, input))
}

fn census_typed<T: Fixture>(
    column: Column,
    input: &[u8],
) -> (usize, Option<crate::alloc_arm::Census>) {
    // The stringify columns build their value OUTSIDE the measured region, as
    // the timed cells do, so the census counts the same work the clock times.
    match column {
        Column::DomParse => {
            let (v, c) = crate::alloc_arm::measure(|| {
                let s = std::str::from_utf8(input).unwrap();
                turbo::from_str::<turbo::Value>(s).unwrap()
            });
            drop(v);
            (input.len(), c)
        }
        Column::StructParse => {
            let (v, c) = crate::alloc_arm::measure(|| {
                let s = std::str::from_utf8(input).unwrap();
                turbo::from_str::<T>(s).unwrap()
            });
            drop(v);
            (input.len(), c)
        }
        Column::Scan => {
            // `IgnoredAny` is a zero-sized Copy marker: there is nothing to drop,
            // which is the point of the column.
            let (_, c) = crate::alloc_arm::measure(|| {
                let s = std::str::from_utf8(input).unwrap();
                turbo::from_str::<serde::de::IgnoredAny>(s).unwrap()
            });
            (input.len(), c)
        }
        Column::DomStringify => {
            let dom: turbo::Value = turbo::from_slice(input).unwrap();
            let mut buf = Vec::with_capacity(input.len());
            let ((), c) = crate::alloc_arm::measure(|| {
                buf.clear();
                turbo::to_writer(&mut buf, &dom).unwrap();
            });
            (buf.len(), c)
        }
        Column::StructStringify => {
            let value: T = turbo::from_slice(input).unwrap();
            let mut buf = Vec::with_capacity(input.len());
            let ((), c) = crate::alloc_arm::measure(|| {
                buf.clear();
                turbo::to_writer(&mut buf, &value).unwrap();
            });
            (buf.len(), c)
        }
    }
}

/// Run one (arm, file, column) cell for `window`, returning every iteration.
pub fn run(arm: Arm, file: File, column: Column, input: &[u8], window: Duration) -> Sample {
    crate::by_fixture!(file, run_typed, (arm, column, input, window))
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
                Column::Scan => {
                    let ns = measure(window, || {
                        timed(|| {
                            let s = std::str::from_utf8(black_box(input)).unwrap();
                            $json::from_str::<serde::de::IgnoredAny>(s).unwrap()
                        })
                    });
                    Sample {
                        ns,
                        bytes: input.len(),
                    }
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
        Column::Scan => {
            let ns = measure(window, || {
                scratch.copy_from_slice(input);
                timed(|| {
                    simd_json::serde::from_slice::<serde::de::IgnoredAny>(&mut scratch).unwrap()
                })
            });
            Sample {
                ns,
                bytes: input.len(),
            }
        }
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
        Column::Scan => {
            let ns = measure(window, || {
                timed(|| sonic_rs::from_str::<serde::de::IgnoredAny>(black_box(text)).unwrap())
            });
            Sample {
                ns,
                bytes: input.len(),
            }
        }
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
