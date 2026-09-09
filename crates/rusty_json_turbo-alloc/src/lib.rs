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
