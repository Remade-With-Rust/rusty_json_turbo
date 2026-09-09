//! Allocator seam for rusty_json_turbo binaries.
//!
//! House law: `#[global_allocator]` lives in the *deliverable* (`main.rs`),
//! never in a shared library. This crate holds the exact `rusty_alloc-api`
//! pin so feature code never names that crate. Read the pin from `Cargo.toml`,
//! not from a comment.
//!
//! The library crate (`rusty_json_turbo`, crate name `serde_json`) never sets
//! an allocator: a program may define exactly one, and a library that declares
//! it forces the choice on every consumer.

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
