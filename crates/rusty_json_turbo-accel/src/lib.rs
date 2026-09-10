//! The SIMD island: SSE2 and AVX2 twins of the JSON byte scanners.
//!
//! # Why this is a separate crate
//!
//! Vector intrinsics are `unsafe`, and the workspace denies `unsafe_code`
//! everywhere else. Rather than open a hole in the parser, the vector code
//! lives here, alone, so `rusty_json_turbo` can keep tightening toward
//! `forbid(unsafe_code)` while still reaching a wide scan. Every `unsafe` in
//! this crate is a load or a compare, each one bounds-checked by the caller
//! immediately above it and carrying its invariant in a `SAFETY` comment.
//!
//! There are no dependencies and no build script. That is a contract, not an
//! accident: `rusty_json_turbo` advertises an empty dependency tree.
//!
//! # The rule every kernel here follows
//!
//! **The scalar version is written first, it is the oracle, and it stays in
//! the tree forever.** Each vector kernel is tested against it over every one
//! of the 256 byte values at every offset, and `RJT_ISA=scalar` runs it in
//! production so the two can be compared inside one binary. A vector kernel
//! whose scalar twin has been deleted cannot be checked and cannot be
//! debugged.
//!
//! # What the corpus said before any of this was written
//!
//! Sizing a kernel is a measurement, not a preference for the widest register
//! available. On JSONCORP, whitespace runs are 16 to 29 bytes on
//! `citm_catalog` and **nothing in the corpus reaches 32 bytes**, so no
//! 32-byte step is ever fully used. That does not make AVX2 worthless -- a
//! wide load that overshoots the end of a run still resolves it in one
//! iteration -- but it does mean the honest figure is steps taken, not bytes
//! covered: 8-byte steps 140,381, 16-byte 95,424, 32-byte 50,467 on
//! `citm_catalog`, and 16,252 against 15,476 on `twitter`, which is nothing.
//!
//! So the whitespace kernel is a `citm_catalog` win and `twitter` is a
//! CONTROL. That distinction was paid for once already: brick B1s cost
//! `twitter` 10% by taking an 8-byte step over its 1-byte runs, until a
//! 4-byte scalar peel was put in front.

#![no_std]
// THE ONE PLACE IN THE WORKSPACE WHERE THIS IS ALLOWED. Vector intrinsics are
// unsafe functions and unaligned loads are unsafe operations; there is no safe
// spelling of either. Every block below is individually justified.
#![allow(unsafe_code)]

#[cfg(feature = "std")]
extern crate std;

/// Which instruction set the scanners will actually use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isa {
    /// The oracle. Always available, always correct, and reachable in
    /// production through `RJT_ISA=scalar`.
    Scalar,
    /// 8 bytes per step with ordinary integer arithmetic. No target feature,
    /// so this is the floor on every architecture.
    Swar,
    /// 16 bytes per step. Baseline on `x86_64`, so it needs no runtime
    /// detection there.
    Sse2,
    /// 32 bytes per step. Detected at runtime; needs `std`.
    Avx2,
}

impl Isa {
    /// The name used by `RJT_ISA` and printed in a method line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Isa::Scalar => "scalar",
            Isa::Swar => "swar",
            Isa::Sse2 => "sse2",
            Isa::Avx2 => "avx2",
        }
    }

    /// Bytes examined per step. The number that prices the kernel.
    #[must_use]
    pub fn step(self) -> usize {
        match self {
            Isa::Scalar => 1,
            Isa::Swar => 8,
            Isa::Sse2 => 16,
            Isa::Avx2 => 32,
        }
    }
}

/// A scanner kernel: index of the first interesting byte at or after `from`.
///
/// Named so the twin tests can hold a table of them without tripping the
/// complex-type lint, and so every kernel is forced into one shape.
pub type Scanner = fn(&[u8], usize) -> usize;

mod dispatch;
mod escape;
mod whitespace;

pub use dispatch::{isa, isa_available};
pub use escape::{first_escape, first_escape_scalar, needs_escape};
pub use whitespace::{first_non_ws, first_non_ws_scalar, is_ws};

#[cfg(test)]
mod tests;
