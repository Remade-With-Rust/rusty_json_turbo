//! Which kernel runs, decided once and cached.
//!
//! Detection happens on first use and is stored in an atomic, so the hot path
//! is one relaxed load and a match. It is deliberately NOT a `OnceLock`: this
//! crate is `no_std` by default, and a `u8` in an atomic is enough for a value
//! that only ever moves from "unknown" to one fixed answer.

use core::sync::atomic::{AtomicU8, Ordering};

use crate::Isa;

const UNKNOWN: u8 = 0;
const SCALAR: u8 = 1;
const SWAR: u8 = 2;
const SSE2: u8 = 3;
const AVX2: u8 = 4;

static CHOSEN: AtomicU8 = AtomicU8::new(UNKNOWN);

/// The widest ISA this build can actually reach on this machine.
///
/// `SSE2` is baseline on `x86_64`, so it is claimed without detection -- the
/// target guarantees it. `AVX2` needs a runtime check, and that check needs
/// `std`; a `no_std` build stops at SSE2 rather than guessing.
#[must_use]
pub fn isa_available() -> Isa {
    #[cfg(all(feature = "std", any(target_arch = "x86_64", target_arch = "x86")))]
    {
        if std::is_x86_feature_detected!("avx2") {
            return Isa::Avx2;
        }
    }
    // SSE2 is guaranteed by the `x86_64` target definition, so no detection
    // is needed and none is done. On 32-bit x86 it is NOT guaranteed, which is
    // why that arm falls through to SWAR.
    #[cfg(target_arch = "x86_64")]
    {
        Isa::Sse2
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        Isa::Swar
    }
}

/// The rung actually chosen when nothing overrides it.
///
/// **SSE2, not AVX2, and that is a measurement rather than caution.** On this
/// corpus AVX2 beat the 8-byte SWAR baseline only on `citm_catalog` and lost
/// everywhere else, while SSE2 beat it on EVERY cell:
///
/// | cell | SSE2 vs SWAR | AVX2 vs SWAR |
/// |---|---|---|
/// | `citm_catalog` scan | **1.107x** (61/61) | 1.099x |
/// | `citm_catalog` struct-parse | **1.074x** (61/61) | 1.072x |
/// | `twitter` scan | **1.020x** | **0.992x** (best-of-N 0.958x) |
/// | `canada` scan | 1.009x | 0.997x |
///
/// The reason is in the run lengths, and it is not subtle once counted: the
/// longest whitespace run anywhere in the corpus is **29 bytes**, so a 32-byte
/// step is NEVER fully used, while its costs -- a call that cannot inline, a
/// wider tail to clean up, `vzeroupper` on exit -- are paid on every run. A
/// 16-byte step is fully used by `citm_catalog`'s 17-to-29-byte runs and by
/// `twitter`'s 9-to-16-byte ones.
///
/// AVX2 stays reachable through `RJT_ISA=avx2` precisely so this can be
/// re-measured on a machine or a corpus with longer runs, rather than being
/// deleted on the strength of one box's answer.
fn default_isa() -> Isa {
    #[cfg(target_arch = "x86_64")]
    {
        Isa::Sse2
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        Isa::Swar
    }
}

/// The ISA in force, honouring the `RJT_ISA` override.
///
/// The override is the whole point of the island: it makes an ISA rung an A/B
/// inside ONE binary, with one environment variable between the arms, rather
/// than a comparison between two builds -- where code layout alone has
/// measured plus or minus 13% in this project.
#[must_use]
#[inline]
pub fn isa() -> Isa {
    match CHOSEN.load(Ordering::Relaxed) {
        SCALAR => Isa::Scalar,
        SWAR => Isa::Swar,
        SSE2 => Isa::Sse2,
        AVX2 => Isa::Avx2,
        _ => resolve(),
    }
}

#[cold]
fn resolve() -> Isa {
    let chosen = requested().unwrap_or_else(default_isa);
    // Never claim an ISA this machine cannot run, whatever was requested: an
    // override is for narrowing, and widening it would fault rather than
    // mis-measure.
    let available = isa_available();
    let chosen = if chosen.step() > available.step() {
        available
    } else {
        chosen
    };
    CHOSEN.store(
        match chosen {
            Isa::Scalar => SCALAR,
            Isa::Swar => SWAR,
            Isa::Sse2 => SSE2,
            Isa::Avx2 => AVX2,
        },
        Ordering::Relaxed,
    );
    chosen
}

#[cfg(feature = "std")]
fn requested() -> Option<Isa> {
    match std::env::var("RJT_ISA").ok()?.as_str() {
        "scalar" => Some(Isa::Scalar),
        "swar" => Some(Isa::Swar),
        "sse2" => Some(Isa::Sse2),
        "avx2" => Some(Isa::Avx2),
        _ => None,
    }
}

#[cfg(not(feature = "std"))]
fn requested() -> Option<Isa> {
    None
}
