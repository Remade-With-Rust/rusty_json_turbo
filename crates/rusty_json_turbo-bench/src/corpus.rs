//! JSONCORP on disk: the benchmark files plus the conformance sets.
//!
//! # Two scenarios, kept apart on purpose
//!
//! [`File::S1_S3`] is the classic trio -- `twitter`, `citm_catalog`, `canada`.
//! They are deliberately orthogonal (27.8% whitespace / 57.1% string; 71.9%
//! whitespace; 90.1% number) and that orthogonality is used as a GATE: a brick
//! that helps one must visibly not help the others. Every table published so
//! far is over those three, so the constant stays exactly three files long and
//! in that order.
//!
//! [`File::S4`] is the house-payload scenario: seven documents shaped like the
//! JSON Remade-With-Rust services actually move. They exist because S1-S3, for
//! all their usefulness, contain no untagged enum, no `flatten` tail, no
//! `Vec<u8>` field, no minified body, no string longer than 463 bytes and no
//! hex escape at all. See `corpus/s4/MANIFEST.md` for what each one prices.
//!
//! [`File::EVERY`] is both, for the oracle and the census. `--all` on a timing
//! verb still means S1-S3, so a number quoted today is comparable with one
//! quoted before S4 existed.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum File {
    // S1-S3: the classic trio. Order is load-bearing (see `S1_S3`).
    Canada,
    CitmCatalog,
    Twitter,
    // S4: the house payloads, under `corpus/s4/`.
    S4MediaProbe,
    S4FrameTelemetry,
    S4SigninBatch,
    S4SyncEnvelope,
    S4NodeConfig,
    S4VaultShard,
    S4OcrI18n,
}

/// Which corpus scenario a file belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// The classic orthogonal trio. Every historical table is over these.
    S1S3,
    /// The house payloads.
    S4,
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Scenario::S1S3 => "S1-S3",
            Scenario::S4 => "S4",
        }
    }
}

impl File {
    /// The classic trio, in the order every published table uses.
    ///
    /// Named `ALL` historically; kept as an alias so nothing that referred to
    /// it breaks, but new code should say which scenario it means.
    pub const S1_S3: [File; 3] = [File::Twitter, File::CitmCatalog, File::Canada];

    /// Alias for [`File::S1_S3`]. A timing verb's `--all` means THIS, so that a
    /// number quoted today is comparable with one quoted before S4 existed.
    pub const ALL: [File; 3] = File::S1_S3;

    /// The seven house payloads.
    pub const S4: [File; 7] = [
        File::S4MediaProbe,
        File::S4FrameTelemetry,
        File::S4SigninBatch,
        File::S4SyncEnvelope,
        File::S4NodeConfig,
        File::S4VaultShard,
        File::S4OcrI18n,
    ];

    /// Every benchmark document, both scenarios. For the oracle and census,
    /// which want coverage rather than comparability.
    pub const EVERY: [File; 10] = [
        File::Twitter,
        File::CitmCatalog,
        File::Canada,
        File::S4MediaProbe,
        File::S4FrameTelemetry,
        File::S4SigninBatch,
        File::S4SyncEnvelope,
        File::S4NodeConfig,
        File::S4VaultShard,
        File::S4OcrI18n,
    ];

    pub fn parse(s: &str) -> Option<File> {
        match s {
            "canada" => Some(File::Canada),
            "citm" | "citm_catalog" | "citm-catalog" => Some(File::CitmCatalog),
            "twitter" => Some(File::Twitter),
            // Short names as well as full ones: the full name is what appears
            // in a table, the short name is what a person types.
            "media-probe" | "s4-media-probe" | "probe" => Some(File::S4MediaProbe),
            "frame-telemetry" | "s4-frame-telemetry" | "telemetry" => Some(File::S4FrameTelemetry),
            "signin-batch" | "s4-signin-batch" | "signin" => Some(File::S4SigninBatch),
            "sync-envelope" | "s4-sync-envelope" | "sync" => Some(File::S4SyncEnvelope),
            "node-config" | "s4-node-config" | "config" => Some(File::S4NodeConfig),
            "vault-shard" | "s4-vault-shard" | "vault" => Some(File::S4VaultShard),
            "ocr-i18n" | "s4-ocr-i18n" | "ocr" => Some(File::S4OcrI18n),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            File::Canada => "canada",
            File::CitmCatalog => "citm_catalog",
            File::Twitter => "twitter",
            File::S4MediaProbe => "s4-media-probe",
            File::S4FrameTelemetry => "s4-frame-telemetry",
            File::S4SigninBatch => "s4-signin-batch",
            File::S4SyncEnvelope => "s4-sync-envelope",
            File::S4NodeConfig => "s4-node-config",
            File::S4VaultShard => "s4-vault-shard",
            File::S4OcrI18n => "s4-ocr-i18n",
        }
    }

    pub fn scenario(self) -> Scenario {
        match self {
            File::Canada | File::CitmCatalog | File::Twitter => Scenario::S1S3,
            _ => Scenario::S4,
        }
    }

    /// S4 lives in its own subdirectory, so the scenario decides the path.
    pub fn path(self) -> PathBuf {
        match self.scenario() {
            Scenario::S1S3 => corpus_dir().join(format!("{}.json", self.name())),
            Scenario::S4 => corpus_dir()
                .join("s4")
                .join(format!("{}.json", self.name())),
        }
    }

    pub fn load(self) -> Vec<u8> {
        let path = self.path();
        fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    /// The array field holding this document's repeated records, if it has one,
    /// and the name is the JSON key.
    ///
    /// S4 ships containers -- 2,600 frames in one document, 600 assertions,
    /// 280 entries -- because that is how the payloads really arrive. A
    /// per-message latency cell needs the individual records, so this says
    /// where to find them. `None` means the document is one message.
    pub fn message_array(self) -> Option<&'static str> {
        match self {
            File::S4FrameTelemetry => Some("frames"),
            File::S4SigninBatch => Some("assertions"),
            File::S4MediaProbe => Some("files"),
            File::S4OcrI18n => Some("items"),
            File::S4VaultShard => Some("shards"),
            // `sync-envelope` nests its records one level down, under `data`.
            File::S4SyncEnvelope => Some("data.entries"),
            File::S4NodeConfig | File::Canada | File::CitmCatalog | File::Twitter => None,
        }
    }
}

/// `<repo>/corpus`, located relative to this crate so tests and the binary
/// agree wherever they are run from.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
}

/// Every `.json` file under the corpus: both benchmark scenarios, then
/// `jsonchecker/*` and `roundtrip/*`, as (display name, bytes).
///
/// This is what `tests/oracle.rs` gates over, so adding a file to
/// [`File::EVERY`] puts it under the byte-identical contract with no further
/// work -- which is the cheapest possible way to register a corpus file.
pub fn all_documents() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for f in File::EVERY {
        let label = match f.scenario() {
            Scenario::S1S3 => format!("{}.json", f.name()),
            Scenario::S4 => format!("s4/{}.json", f.name()),
        };
        out.push((label, f.load()));
    }
    // S5 and S6 join the byte-identical gate here, which is the cheapest
    // possible registration: no enum variant, no fixture, and the must-FAIL
    // members are handled exactly as `jsonchecker/fail*.json` already are --
    // the oracle compares error text with line and column, so a document that
    // fails identically in both crates is a pass.
    for sub in ["jsonchecker", "roundtrip", "s5", "s6"] {
        out.extend(json_files_in(&corpus_dir().join(sub), sub));
    }
    out
}

/// All `.json` files directly under `dir`, sorted by name, as (`prefix/name`, bytes).
pub fn json_files_in(dir: &Path, prefix: &str) -> Vec<(String, Vec<u8>)> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        // `.ndjson` as well as `.json`, or S5's stream file is SILENTLY
        // SKIPPED -- a corpus member that is present, listed, and never
        // actually gated. That failure mode leaves every count looking right.
        .filter(|p| p.extension().is_some_and(|x| x == "json" || x == "ndjson"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|p| {
            let name = format!("{prefix}/{}", p.file_name().unwrap().to_string_lossy());
            let bytes = fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            (name, bytes)
        })
        .collect()
}

/// The individual records inside a container document, with their original
/// bytes intact.
///
/// S4 ships containers because that is how the payloads really arrive -- 2,600
/// frames in one document, 600 assertions, 280 entries. A LATENCY question is
/// a different question from a throughput one, though: "how long to handle one
/// message" is not "how fast can we chew 600 KB", and the two can move in
/// opposite directions when per-call setup dominates. So this slices the
/// container into its records.
///
/// The slicing uses the ORACLE's `RawValue`, not ours, and that is not an
/// accident. `RawValue` hands back the original bytes -- whitespace, key order
/// and all -- which matters because `s4-media-probe.json` is 54.9% whitespace
/// and re-serializing each record through `Value` would quietly measure a
/// minified document instead of the one on disk. Enabling `raw_value` on
/// `turbo` would have compiled extra paths into the crate under measurement;
/// enabling it on the oracle, which is never timed, costs nothing.
///
/// Returns an empty vector for a document with no record array
/// ([`File::message_array`] is `None`), so a caller can treat "one message"
/// and "many messages" uniformly.
pub fn messages(file: File) -> Vec<Vec<u8>> {
    let Some(path) = file.message_array() else {
        return Vec::new();
    };
    let bytes = file.load();
    let doc: serde_json_upstream::Value = serde_json_upstream::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", file.name()));

    // `path` is dotted so a nested container ("data.entries") can be reached
    // without a second mechanism.
    let mut node = &doc;
    for key in path.split('.') {
        node = node
            .get(key)
            .unwrap_or_else(|| panic!("{}: no key {key:?} on the path {path:?}", file.name()));
    }
    let array = node
        .as_array()
        .unwrap_or_else(|| panic!("{}: {path:?} is not an array", file.name()));

    // Re-serialize each element through the ORACLE. The element bytes are the
    // ones the oracle read, so this is a faithful slice of the input rather
    // than a re-rendering by the crate under test.
    array
        .iter()
        .map(|v| serde_json_upstream::to_vec(v).expect("re-serialize a record"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered file must exist on disk and be non-empty. A corpus
    /// entry that silently fails to load would make a benchmark measure
    /// nothing while still reporting a number.
    #[test]
    fn every_file_loads() {
        for f in File::EVERY {
            let bytes = f.load();
            assert!(
                bytes.len() > 1000 || f == File::S4NodeConfig,
                "{} is only {} bytes",
                f.name(),
                bytes.len()
            );
        }
    }

    /// `parse` must round-trip every file's own name, or `--cell <name>` would
    /// silently not match the file the tables call by that name.
    #[test]
    fn parse_accepts_every_name() {
        for f in File::EVERY {
            assert_eq!(File::parse(f.name()), Some(f), "name {}", f.name());
        }
    }

    /// The two scenario constants must partition `EVERY` exactly: no file
    /// missing, none counted twice.
    #[test]
    fn scenarios_partition_every() {
        let mut joined: Vec<File> = File::S1_S3.to_vec();
        joined.extend(File::S4);
        assert_eq!(joined.len(), File::EVERY.len());
        for f in File::EVERY {
            assert_eq!(
                joined.iter().filter(|&&x| x == f).count(),
                1,
                "{} is not in exactly one scenario",
                f.name()
            );
        }
    }

    /// S1-S3 stays three files in this order, because every published table is
    /// over it. If this fails, a historical number stopped being comparable.
    #[test]
    fn s1_s3_is_frozen() {
        assert_eq!(
            File::S1_S3,
            [File::Twitter, File::CitmCatalog, File::Canada]
        );
        assert_eq!(File::ALL, File::S1_S3);
    }
}
