#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A batch of 600 mID sign-in verifications, each a `SignedAssertion` wrapping
/// a verbatim `NonceEnvelope`, plus 12 resolved `DidDocumentV2` rosters: the
/// `Vec<u8>` path, because serde puts a 32-byte nonce, a 64-byte signature and
/// a 33-byte pubkey on the wire as 61,184 integer tokens.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SigninBatch {
    pub batch_version: u32,
    pub produced_by: String,
    pub verifier: String,
    pub roster_hard_cap: u32,
    pub signed_at_tolerance_secs: u32,
    pub count: u32,
    pub assertions: Vec<SignedAssertion>,
    pub resolved_rosters: Vec<DidDocumentV2>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SignedAssertion {
    pub envelope_version: u8,
    pub envelope_type: String,
    pub nonce_envelope: NonceEnvelope,
    pub device_id: String,
    /// 64 small integers on the wire. `Vec<u8>` on purpose: not `&[u8]` and not
    /// `serde_bytes`, since the number array IS the form under test.
    pub signature: Vec<u8>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NonceEnvelope {
    pub envelope_version: u8,
    pub envelope_type: String,
    /// 32 small integers on the wire; see [`SignedAssertion::signature`].
    pub nonce: Vec<u8>,
    pub did: String,
    pub audience: String,
    pub purpose: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub issuer: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DidDocumentV2 {
    pub did: String,
    pub roster_version: u32,
    pub roster: Vec<RosterEntry>,
    pub updated_at: u64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RosterEntry {
    pub device_id: String,
    /// 33 small integers on the wire; see [`SignedAssertion::signature`].
    pub pubkey: Vec<u8>,
    pub added_at: u64,
    pub label: String,
}
