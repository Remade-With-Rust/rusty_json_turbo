//! `serde_json`, re-exported from `rusty_json_turbo`.
//!
//! This crate exists to be the target of a `[patch.crates-io]` entry. See its
//! manifest for why that cannot be done with `rusty_json_turbo` directly, and
//! for the gate that stops it from shadowing the harness's upstream oracle.
//!
//! It is a re-export and not a second compilation of the source, deliberately:
//! two builds of the same code produce two sets of types that do not unify,
//! which is precisely the fault a whole-graph patch exists to prevent. Every
//! `Value`, `Map`, `Number`, `Error` and `RawValue` named through this crate
//! **is** the one `rusty_json_turbo` defines.
//!
//! ```toml
//! [patch.crates-io]
//! serde_json = { git = "https://github.com/Remade-With-Rust/rusty_json_turbo" }
//! ```

#![no_std]
#![forbid(unsafe_code)]

// The whole public surface. Modules re-export as modules, so
// `serde_json::value::RawValue` and `serde_json::de::Deserializer` resolve as
// they always did.
pub use turbo::*;

// `json!` is `#[macro_export]`, so it lives at the crate root rather than in a
// module and a glob does not reliably carry it. Its expansion refers to
// `$crate::...`, and `$crate` means the crate that DEFINED it -- so the
// helpers it calls resolve inside `rusty_json_turbo` and do not need
// re-exporting here.
pub use turbo::json;
