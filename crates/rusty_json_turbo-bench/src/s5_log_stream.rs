//! The S5 fixture: one line of an NDJSON log stream.
//!
//! S5 is not one document, it is **10,000 of them** -- newline-delimited, one
//! complete object per line, the shape a service actually writes. That makes
//! it the only corpus scenario that measures PER-DOCUMENT cost rather than
//! throughput on one big blob, and the only one that exercises
//! `StreamDeserializer` at all.
//!
//! Two shapes here are deliberate rather than convenient, because they are the
//! serde paths a log line really has:
//!
//! - **`trace` is `Option<String>`**, and 1,527 of the 10,000 lines omit the
//!   key entirely while 8,473 carry it. An absent key and a present-but-null
//!   key are different things, and only `Option` on a non-`default` field gets
//!   both right.
//! - **`err` is `Option<ErrorDetail>` and is null on 8,579 lines**, with a
//!   further `Option<u64>` inside it that is null on 545 of the 1,421 that are
//!   present. Nested optionality is where a hand-rolled deserializer usually
//!   goes wrong, so the fixture keeps it.
//!
//! `fields` is the one place a `Map` is honest rather than lazy: measured, the
//! 10,000 lines carry **609 distinct key sets over 64 distinct keys**, with
//! string, integer, float and boolean values mixed. It is a field bag, not a
//! struct, and typing it as one would misrepresent the workload.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// One log line.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogRecord {
    pub ts: String,
    pub lvl: Level,
    pub tgt: String,
    pub msg: String,
    /// Absent on 1,527 of the 10,000 lines. Absent, not null.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub trace: Option<String>,
    pub span: String,
    /// A field bag: 609 distinct key sets over 64 keys, mixed value types.
    pub fields: turbo::Map<String, turbo::Value>,
    /// Null on 8,579 of the 10,000 lines.
    pub err: Option<ErrorDetail>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ErrorDetail {
    pub kind: ErrorKind,
    pub detail: String,
    /// Null on 545 of the 1,421 error objects: optionality inside optionality.
    pub retry_in_ms: Option<u64>,
}

enum_str!(Level {
    Trace("trace"),
    Debug("debug"),
    Info("info"),
    Warn("warn"),
    Error("error"),
});

enum_str!(ErrorKind {
    Io("io"),
    PeerTimeout("peer_timeout"),
    ShardMissing("shard_missing"),
    DecodeError("decode_error"),
    RosterStale("roster_stale"),
    CapabilityExpired("capability_expired"),
    TierUnavailable("tier_unavailable"),
    AdvisoryDbStale("advisory_db_stale"),
});
