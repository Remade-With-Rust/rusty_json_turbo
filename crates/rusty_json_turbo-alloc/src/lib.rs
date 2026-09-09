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
//! | twitter.json, DOM parse | 20,834 | **1.53x** |
//! | citm_catalog.json, DOM parse | 39,339 | **1.46x** |
//! | canada.json, struct parse | 485 | 1.07x |
//! | any file, stringify | **0** | 1.00x (the control) |
//!
//! The last row is why the others are believable: a stringify into a pre-sized
//! buffer allocates nothing, so the allocator cannot touch it -- and it doesn't.
//! Full tables, method line and caveats: `corpus/LEDGER.md` in the repository.
//! The numbers are Windows x86-64; glibc's malloc should close much of the gap,
//! so treat the Linux figure as unmeasured rather than implied.
//!
//! # Hardened profile
//!
//! ```toml
//! rusty_json_turbo-alloc = { version = "0.1", features = ["secure"] }
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
pub const NAME: &str = if cfg!(feature = "secure") {
    "rusty_alloc 2.0.4 (secure)"
} else if cfg!(feature = "debug_checks") {
    "rusty_alloc 2.0.4 (debug_checks)"
} else {
    "rusty_alloc 2.0.4"
};
