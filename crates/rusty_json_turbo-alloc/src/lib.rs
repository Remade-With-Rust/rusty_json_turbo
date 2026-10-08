//! The `rusty_alloc` seam for `rusty_json_turbo` deliverables.
//!
//! # Why a crate for two lines
//!
//! `#[global_allocator]` is process-wide: a program may declare exactly **one**.
//! So it is a property of the *deliverable*, not of a component, and a library
//! that declares it forces the choice on every consumer and makes two such
//! libraries impossible to link together. That is why
//! [`rusty_json_turbo`](https://crates.io/crates/rusty_json_turbo) -- a drop-in
//! for `serde_json`, which sits in nearly every Rust dependency graph -- does
//! **not** install one, and this crate exists instead.
//!
//! It holds the three decisions that would otherwise be re-made in every
//! binary: the exact pin (read it from `Cargo.toml`, not from a comment), the
//! `secure` choice, and the `debug_checks` choice. Feature code never names
//! `rusty_alloc-api`.
//!
//! # Use
//!
//! In your binary, once:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: rusty_json_turbo_alloc::Alloc = rusty_json_turbo_alloc::Alloc;
//! ```
//!
//! That is the whole change. On JSON parsing it is worth a great deal, because
//! building a `Value` is allocation-bound -- roughly **one allocation per 31
//! input bytes** on a real document. Measured on the project's corpus, pinned,
//! paired and interleaved, against the platform allocator:
//!
//! | workload | allocations per parse | ratio |
//! |---|---:|---:|
//! | twitter.json, DOM parse | 20,834 | **1.652x** |
//! | citm_catalog.json, DOM parse | 39,339 | **1.570x** |
//! | canada.json, DOM parse | 56,061 | **1.466x** |
//! | citm_catalog.json, struct parse | 2,544 | 1.149x |
//! | twitter.json, stringify | **0** | **0.949x** (the control) |
//!
//! Re-measured on **rusty_alloc 2.2.5**, 15 pairs, 2-second windows; every
//! parse cell unanimous at 0/15 or 1/15 wins for the platform allocator.
//!
//! **The control row is the first thing to read, and it no longer reads 1.00.**
//! A stringify into a pre-sized buffer allocates nothing, so the allocator
//! cannot touch it -- on 2.0.4 that row measured exactly 1.00x, and on 2.2.5 it
//! measures 0.949x: the allocator build is ~5% slower on work that does not
//! allocate. Either the two binaries differ in layout or 2.2.5 carries a fixed
//! cost reaching non-allocating paths, and one A/B cannot separate them. The
//! parse gains are therefore if anything understated, and a workload that
//! allocates nothing should not expect a win.
//!
//! Full tables, method line and caveats: `corpus/LEDGER.md` in the repository.
//! The numbers are Windows x86-64; glibc's malloc should close much of the gap,
//! so treat the Linux figure as unmeasured rather than implied.
//!
//! # Hardened profile
//!
//! ```toml
//! rusty_json_turbo-alloc = { version = "0.3", features = ["secure"] }
//! ```
//!
//! Guard pages and encrypted free lists, for a service eating untrusted bytes.
//! On this workload it kept nearly the whole speedup.
//!
//! # What you are adopting it for
//!
//! The house line is that `rusty_alloc` is adopted for the **safety posture**
//! first: a double free aborts instead of handing the same memory to two
//! owners. Treat that abort as a bug to fix, never a check to disable. The
//! speed above is real and measured, and it is still the second reason.

#![no_std]

pub use rusty_alloc_api::RustyAlloc as Alloc;

/// Which build of the allocator this seam was compiled with, for a method line.
///
/// A measurement that does not name its allocator is not reproducible: the
/// allocator is a link-time property of the binary, so it cannot be read back
/// at runtime from anywhere else.
/// The allocator's own version, re-exported.
///
/// DERIVED, never transcribed: `rusty_alloc` defines it as
/// `env!("CARGO_PKG_VERSION")`, so it cannot disagree with the allocator that
/// is actually linked.
///
/// This used to be a literal baked into [`NAME`] (`"rusty_alloc 2.0.4"`), and
/// that is a worse defect than it looks: the benchmark harness prints the
/// allocator on the METHOD LINE of every number it reports, so after the
/// 2.2.5 upgrade every measurement in the ledger would have named 2.0.4 while
/// running 2.2.5. **A version in a string is a claim about the build, and only
/// the build may make it.**
///
/// The pieces are separate rather than one concatenated constant because this
/// crate is `no_std` and has exactly one dependency; const string
/// concatenation of a non-literal needs either `alloc` or another crate, and
/// neither is worth adding to a published crate for a display string. The
/// caller joins them -- see `NAME`, `PROFILE`.
pub use rusty_alloc_api::VERSION;

/// Which profile is compiled in: `""`, `" (secure)"` or `" (debug_checks)"`.
pub const PROFILE: &str = if cfg!(feature = "secure") {
    " (secure)"
} else if cfg!(feature = "debug_checks") {
    " (debug_checks)"
} else {
    ""
};

/// The allocator's name, without the version. Join with [`VERSION`] and
/// [`PROFILE`] for the full identification a method line wants.
pub const NAME: &str = "rusty_alloc";
