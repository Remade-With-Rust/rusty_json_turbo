#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap as Map;

use crate::empty;

use turbo::Value;

/// One sidecar API response wrapping a SpaceDB sync page of 280 per-entry CRDT
/// deltas: the `flatten` tail (565 unmatched `x_*` keys through the
/// unknown-field fallback), two internally-tagged enums (`outcome` on `type`,
/// `fields.*` on `crdt`) and `null` in `Option` position, all in one document.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SyncEnvelope {
    pub api_version: String,
    pub served_by: String,
    pub listen: String,
    pub box_id: String,
    pub maker_did: String,
    /// Null until the box is claimed.
    pub owner_did: Option<String>,
    pub pair_state: String,
    pub request_id: String,
    pub elapsed_ms: f64,
    pub warnings: empty::Array,
    pub data: SyncPage,
    pub meter: Meter,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SyncPage {
    pub collection: String,
    pub cursor: String,
    pub has_more: bool,
    pub count: u32,
    pub entries: Vec<Entry>,
}

/// One CRDT delta. Nine named fields and a varying tail: 280 entries take 25
/// distinct key sets, so every key beyond the nine lands in [`Entry::tail`]
/// through serde's unknown-field path.
///
/// No `deny_unknown_fields` here, and not by omission: it cannot coexist with
/// `flatten`. The round-trip test is what proves nothing was dropped.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Entry {
    pub id: String,
    pub collection: String,
    pub tier: Tier,
    pub version: EntryVersion,
    pub fields: Map<String, CrdtValue>,
    pub outcome: Outcome,
    pub deleted: bool,
    /// Present on every entry, null on 194 of 280.
    pub shards: Option<ShardSet>,
    /// Present on every entry, null on 181 of 280.
    pub grant: Option<Grant>,
    /// The varying tail: `x_source`, `x_import_id`, `x_device`, `x_last_seen`,
    /// `x_starred`, in 25 combinations, with `String`, `null`, integer and
    /// `bool` values. `Value` because the tail is genuinely heterogeneous.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub tail: Map<String, Value>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EntryVersion {
    pub node: String,
    pub counter: u32,
    pub hlc: String,
}

/// What the store did with the delta, internally tagged on `type`: four
/// variants with four different field sets, which is why this is an enum and
/// not one struct with three `Option`s.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
pub enum Outcome {
    Local,
    Committed { tier: Tier },
    Merged { tier: Tier, peers: u32 },
    Rejected { reason: String },
}

/// One field's CRDT value, internally tagged on `crdt`. Four variants, evenly
/// split over 1,250 values.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "crdt", rename_all = "snake_case"))]
pub enum CrdtValue {
    Register {
        value: String,
        ts: u64,
        node: String,
    },
    Text {
        ops: Vec<TextOp>,
        len: u32,
    },
    OrSet {
        adds: Vec<String>,
        removes: Vec<String>,
    },
    Counter {
        /// Signed: the corpus carries deltas down to -4.
        delta: i64,
        seen: u32,
    },
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TextOp {
    pub at: u32,
    pub ins: String,
    pub del: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ShardSet {
    pub k: u32,
    pub n: u32,
    pub placement: Vec<String>,
    pub repair_due: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Grant {
    pub granted_to: String,
    pub scope: Vec<String>,
    pub expires_at: u64,
    pub budget_units: u32,
    pub revoked: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Meter {
    pub read_units: u32,
    pub write_units: u32,
    pub budget_remaining: u32,
    pub priced_at: String,
}

enum_str!(Tier {
    Convergent("convergent"),
    Causal("causal"),
    Strong("strong"),
});
