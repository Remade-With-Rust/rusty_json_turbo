//! The rusty_json_turbo harness.
//!
//! Two jobs, one crate, because both need upstream serde_json linked in-process
//! next to ours:
//!
//! - [`oracle`]: the differential gate. For every document, both crates must
//!   produce identical bytes, an equal `Value`, identical error text with the
//!   same line and column, and identical float bits. Runs as a test.
//! - [`harness`] over [`cells`]: the paired ABBA benchmark. Arms alternate lead,
//!   every number carries its method line, and the null arm (upstream against
//!   itself) is the floor a claim must clear.
//!
//! The fixture types (`twitter`, `citm_catalog`, `canada`, ...) are verbatim
//! from serde-rs/json-benchmark so the struct columns measure the shapes the
//! published serde_json numbers were taken on. See `NOTICE.md`.

// The fixture files are upstream's; their lint posture is theirs.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_assert,
    clippy::match_single_binding,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::needless_lifetimes,
    clippy::needless_return,
    clippy::new_without_default,
    clippy::ptr_as_ptr,
    clippy::redundant_static_lifetimes,
    clippy::semicolon_if_nothing_returned,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::unused_io_amount,
    clippy::used_underscore_binding
)]

#[macro_use]
pub mod enums;

pub mod canada;
pub mod citm_catalog;
pub mod color;
pub mod empty;
pub mod prim_str;
pub mod twitter;

// S4, the house payloads. One module per document, each doc-commented with the
// serde path it puts under measurement -- `flatten` tails, internally-tagged
// enums, `Vec<u8>` fields and high-arity field matchers that S1-S3 never enter.
pub mod s4_frame_telemetry;
pub mod s4_media_probe;
pub mod s4_node_config;
pub mod s4_ocr_i18n;
pub mod s4_signin_batch;
pub mod s4_sync_envelope;
pub mod s4_vault_shard;

pub mod alloc_arm;
pub mod cells;
pub mod content;
pub mod corpus;
pub mod harness;
pub mod oracle;
pub mod wcount;
