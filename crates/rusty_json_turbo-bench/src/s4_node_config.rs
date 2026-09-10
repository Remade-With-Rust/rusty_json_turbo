#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap as Map;

/// A disco node configuration file: the `Option`-heavy path, nine `null`
/// positions spread over eight nested structs, at the cold end of the key-reuse
/// sweep (152 distinct keys used 1.6 times each) so any brick with a build cost
/// is charged for it here.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NodeConfig {
    pub config_version: u32,
    pub profile: String,
    pub identity: Identity,
    pub store: Store,
    pub crdt: Crdt,
    pub consistency: Consistency,
    pub replica: Replica,
    pub durability: Durability,
    pub access: Access,
    pub query: Query,
    pub vector: Vector,
    pub meter: Meter,
    pub engines: Engines,
    pub catalog: Vec<CatalogEntry>,
    pub supply_chain: SupplyChain,
    pub time: Time,
    pub logging: Logging,
    pub schemas: Vec<Schema>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Identity {
    /// Null until the node is paired.
    pub did: Option<String>,
    pub device_label: String,
    pub keystore: Keystore,
    pub roster: RosterPolicy,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Keystore {
    pub backend: String,
    pub kdf: Kdf,
    pub aead: String,
    pub rotate_days: u32,
    pub require_presence: bool,
    pub hardware_bound: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Kdf {
    pub name: String,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RosterPolicy {
    pub hard_cap: u32,
    pub signed_at_tolerance_secs: u32,
    pub allow_self_issue: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Store {
    pub layer: String,
    pub path: String,
    pub engine: String,
    pub encrypt_at_rest: bool,
    pub fsync: String,
    pub cache_mib: u32,
    pub compression: Compression,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Compression {
    pub codec: String,
    pub level: u32,
    pub long_window: bool,
    /// Null when no trained dictionary is configured.
    pub dict: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Crdt {
    pub layer: String,
    pub reactive_queries: bool,
    pub gc_tombstones_days: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Consistency {
    pub layer: String,
    pub default_tier: String,
    pub allow_strong: bool,
    pub read_your_writes: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Replica {
    pub layer: String,
    pub transport: Transport,
    pub anti_entropy: AntiEntropy,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Transport {
    pub kind: String,
    pub quic: bool,
    pub mdns: bool,
    pub relays: Vec<String>,
    /// Null when STUN is not used.
    pub stun: Option<String>,
    pub max_streams: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AntiEntropy {
    pub interval_s: u32,
    pub jitter_s: u32,
    pub batch_entries: u32,
    pub gossip_fanout: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Durability {
    pub layer: String,
    pub erasure: Erasure,
    pub placement: Placement,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Erasure {
    // `impl` is a keyword; the wire name is unchanged.
    #[cfg_attr(feature = "serde", serde(rename = "impl"))]
    pub impl_name: String,
    pub k: u32,
    pub n: u32,
    pub shard_kib: u32,
    pub verify_on_read: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Placement {
    pub strategy: String,
    pub min_distinct_nodes: u32,
    pub repair_when_below: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Access {
    pub layer: String,
    pub identity: String,
    pub curve: String,
    pub audit_log: bool,
    pub capabilities: Capabilities,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Capabilities {
    pub default_ttl_s: u32,
    pub allow_delegation: bool,
    pub spend_budget_units: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Query {
    pub layer: String,
    pub wasm: bool,
    pub deterministic: bool,
    pub fuel: u32,
    pub compute_to_data: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Vector {
    pub layer: String,
    pub enabled: bool,
    /// Null until an index is built.
    pub dim: Option<u32>,
    pub metric: String,
    /// Null until an index is built.
    pub index: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Meter {
    pub layer: String,
    pub enabled: bool,
    pub currency: String,
    pub price_read: u32,
    pub price_write: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Engines {
    pub media: MediaEngine,
    pub asr: AsrEngine,
    pub tts: TtsEngine,
    pub ocr: OcrEngine,
    pub vlm: VlmEngine,
    pub detect: DetectEngine,
    pub compress: CompressEngine,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MediaEngine {
    // `crate` is a keyword; the wire name is unchanged.
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
    pub probe_only: bool,
    pub decoders: Vec<String>,
    pub encoders: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AsrEngine {
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
    /// Null until a weights file is supplied.
    pub model: Option<String>,
    pub engine: String,
    pub beam: u32,
    pub vad: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TtsEngine {
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
    /// Null until a voice is selected.
    pub voice: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OcrEngine {
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
    pub streaming: bool,
    pub reading_order: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VlmEngine {
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
    pub model: String,
    pub max_tokens: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DetectEngine {
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
    pub model: String,
    pub track: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CompressEngine {
    #[cfg_attr(feature = "serde", serde(rename = "crate"))]
    pub crate_name: String,
    pub enabled: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CatalogEntry {
    pub capability: String,
    pub status: String,
    /// Null for a capability that is only planned.
    pub backing: Option<String>,
    pub http: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SupplyChain {
    pub tool: String,
    pub stages: Vec<String>,
    pub gate: Gate,
    pub vault: Vault,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Gate {
    pub fail_closed: bool,
    pub allow_dirty: bool,
    pub require_receipts: bool,
    pub advisory_db_max_age_days: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Vault {
    pub path: String,
    pub sealed: bool,
    pub hash: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Time {
    pub daemon: String,
    pub sources: Vec<TimeSource>,
    pub step_threshold_s: f64,
    /// Null when the daemon may never step the clock.
    pub makestep: Option<f64>,
    pub serve: TimeServe,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimeSource {
    pub host: String,
    pub nts: bool,
    pub poll_log2: u32,
    pub prefer: bool,
    pub iburst: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimeServe {
    pub enabled: bool,
    pub rate_limit_qps: u32,
    pub interleaved: bool,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Logging {
    pub level: String,
    pub format: String,
    pub targets: Vec<String>,
    pub redact_dids: bool,
    pub sample_rate: f64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Schema {
    pub name: String,
    pub fields: Map<String, SchemaField>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SchemaField {
    pub crdt: String,
    pub tier: String,
    pub indexed: bool,
    pub required: bool,
    /// Null for every field but the counter, whose default is `0`; this is the
    /// one position in S4 whose wire type the document under-constrains.
    pub default: Option<u32>,
    pub max_bytes: u32,
}
