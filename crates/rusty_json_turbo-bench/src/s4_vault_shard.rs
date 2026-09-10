#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A k-of-n erasure shard set for an encrypted vault stripe, plus one FFAI
/// model-weight shard and four hash-chained Deputy receipts: a flat,
/// string-dominated struct whose `model_shard.weights_b64` puts one 87,384-byte
/// escape-free `String` field through the derive in a single `visit_str`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VaultShard {
    pub manifest_version: u32,
    pub kind: String,
    pub collection: String,
    pub stripe: String,
    pub k: u32,
    pub n: u32,
    pub erasure: Erasure,
    pub aead: String,
    pub kdf: Kdf,
    pub sealed_at: String,
    pub sealed_by: String,
    pub shards: Vec<Shard>,
    pub model_shard: ModelShard,
    pub receipts: Vec<Receipt>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Erasure {
    // `impl` is a keyword; the wire name is unchanged.
    #[cfg_attr(feature = "serde", serde(rename = "impl"))]
    pub impl_name: String,
    pub field: String,
    pub matrix: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Kdf {
    pub name: String,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub salt: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Shard {
    pub index: u32,
    pub role: ShardRole,
    pub node: String,
    pub len: u32,
    pub sha256: String,
    pub nonce: String,
    pub aad: String,
    /// 87,384 bytes of base64: no escape, no non-ASCII, spans every buffer
    /// refill an `IoRead` can offer.
    pub sealed: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ModelShard {
    pub engine: String,
    pub model: String,
    pub part: String,
    pub sha256: String,
    pub bytes: u32,
    pub weights_b64: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Receipt {
    pub stage: String,
    pub at: String,
    pub by: String,
    pub ok: bool,
    pub chain: String,
}

enum_str!(ShardRole {
    Data("data"),
    Parity("parity"),
});
