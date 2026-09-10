//! COMPACT STRUCT-STRINGIFY, against upstream. Written for brick B15s, KEPT
//! after B15s was reverted, because writing it found a hole in the oracle.
//!
//! B15s makes `CompactFormatter` emit a struct field's key as three sink calls
//! instead of five, folding `,` with the opening quote and the closing quote
//! with the colon. Nothing about the OUTPUT is supposed to change, and the
//! differential oracle cannot see whether it did: the oracle compares
//! `to_string_pretty` (which is `PrettyFormatter`, still on the trait default)
//! and `Value::to_string` (which goes through `serialize_map`, not
//! `serialize_struct`). **The folded path is on neither route.**
//!
//! That hole is the point. **`struct-stringify` is a column the harness times
//! on every file in the corpus, and nothing was checking its bytes.** B15s
//! turned out not to pay and was reverted; this test stays, because the gap it
//! covers has nothing to do with B15s.
//!
//! The comparison arm is upstream serde_json. `serde::Serialize` is
//! crate-agnostic, so the very same fixture types can be handed to both
//! serializers -- which makes this a true differential test rather than a
//! snapshot of our own output.

use serde::{Deserialize, Serialize};

/// One fixture: parse the corpus member into its typed struct, then serialize
/// it with both crates and compare the bytes.
macro_rules! agree {
    ($file:expr, $ty:ty) => {{
        let file = $file;
        let bytes = file.load();
        let one = if file.is_stream() {
            // S5's "document" is one line of the stream, not the file.
            rjt_bench::corpus::stream_documents(file)
                .into_iter()
                .next()
                .expect("a stream has documents")
        } else {
            bytes
        };
        let v: $ty = turbo::from_slice(&one).expect("fixture parses");
        let ours = turbo::to_string(&v).expect("ours compact");
        let up = serde_json_upstream::to_string(&v).expect("upstream compact");
        assert_eq!(
            ours,
            up,
            "{}: compact struct-stringify differs",
            file.name()
        );
        // And the pretty route, which must be untouched: it stays on the
        // trait's default five-call implementation.
        let ours_p = turbo::to_string_pretty(&v).expect("ours pretty");
        let up_p = serde_json_upstream::to_string_pretty(&v).expect("upstream pretty");
        assert_eq!(
            ours_p,
            up_p,
            "{}: pretty struct-stringify differs",
            file.name()
        );
        // The fold must also round-trip: our own output has to parse back.
        let back: $ty = turbo::from_str(&ours).expect("our output re-parses");
        assert_eq!(
            turbo::to_string(&back).expect("re-serialize"),
            ours,
            "{}: not stable under a round trip",
            file.name()
        );
        ours.len()
    }};
}

#[test]
fn every_typed_fixture_serializes_byte_identically_to_upstream() {
    use rjt_bench::corpus::File as F;
    let mut total = 0;
    total += agree!(F::Twitter, rjt_bench::twitter::Twitter);
    total += agree!(F::CitmCatalog, rjt_bench::citm_catalog::CitmCatalog);
    total += agree!(F::Canada, rjt_bench::canada::Canada);
    total += agree!(F::S4MediaProbe, rjt_bench::s4_media_probe::MediaProbe);
    total += agree!(
        F::S4FrameTelemetry,
        rjt_bench::s4_frame_telemetry::FrameTelemetry
    );
    total += agree!(F::S4SigninBatch, rjt_bench::s4_signin_batch::SigninBatch);
    total += agree!(F::S4SyncEnvelope, rjt_bench::s4_sync_envelope::SyncEnvelope);
    total += agree!(F::S4NodeConfig, rjt_bench::s4_node_config::NodeConfig);
    total += agree!(F::S4VaultShard, rjt_bench::s4_vault_shard::VaultShard);
    total += agree!(F::S4OcrI18n, rjt_bench::s4_ocr_i18n::OcrI18n);
    total += agree!(F::S5LogStream, rjt_bench::s5_log_stream::LogRecord);
    // A floor, not a target: the typed fixtures drop fields they do not
    // model, and S5 contributes one line rather than its whole stream. It is
    // here so a future registration bug that skipped fixtures would fail
    // rather than pass quietly.
    assert!(
        total > 4_000_000,
        "the fixtures should be megabytes: {total}"
    );
}

/// Field names the fold has to escape rather than store raw, and a `first`
/// boundary that moves at runtime.
///
/// A `&'static str` field name is not necessarily escape-free -- `rename`
/// takes an arbitrary string -- so the folded path still has to run the escape
/// scanner over the key. These names put a quote, a backslash, a control
/// character and a multi-byte character in a key, which is the one input that
/// could tell a fold that skipped the scan from one that did not.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct AwkwardKeys {
    #[serde(rename = "with\"quote")]
    a: u32,
    #[serde(rename = "with\\backslash")]
    b: u32,
    #[serde(rename = "with\u{1}control")]
    c: u32,
    #[serde(rename = "with\u{2028}separator")]
    d: u32,
    #[serde(rename = "\u{4e2d}\u{6587}")]
    e: u32,
    #[serde(rename = "")]
    f: u32,
    /// Absent sometimes, so which field is FIRST is a runtime fact.
    #[serde(skip_serializing_if = "Option::is_none")]
    g: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct NoFields {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct OneField {
    only: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Nested {
    inner: AwkwardKeys,
    list: Vec<OneField>,
    empty: NoFields,
}

#[test]
fn keys_that_need_escaping_and_boundaries_that_move() {
    let awkward = |g| AwkwardKeys {
        a: 1,
        b: 2,
        c: 3,
        d: 4,
        e: 5,
        f: 6,
        g,
    };
    // `g` present and absent both matter: when it is skipped the LAST field
    // changes, and when the first field of a struct is skipped the `first`
    // flag would move -- so check a shape where it can.
    for g in [None, Some(7)] {
        let v = Nested {
            inner: awkward(g),
            list: vec![OneField { only: true }, OneField { only: false }],
            empty: NoFields {},
        };
        let ours = turbo::to_string(&v).expect("ours");
        let up = serde_json_upstream::to_string(&v).expect("upstream");
        assert_eq!(ours, up, "awkward keys differ (g={g:?})");
        let pretty = turbo::to_string_pretty(&v).expect("ours pretty");
        let up_pretty = serde_json_upstream::to_string_pretty(&v).expect("upstream pretty");
        assert_eq!(pretty, up_pretty, "pretty differs (g={g:?})");
        let back: Nested = turbo::from_str(&ours).expect("re-parses");
        assert_eq!(back, v, "did not round-trip (g={g:?})");
    }

    // A struct with no fields at all: the fold is never reached, and `{}` must
    // still come out.
    assert_eq!(turbo::to_string(&NoFields {}).unwrap(), "{}");
    assert_eq!(
        turbo::to_string(&OneField { only: true }).unwrap(),
        serde_json_upstream::to_string(&OneField { only: true }).unwrap()
    );
}

/// The fold is on `serialize_struct`; every OTHER route to an object key must
/// be unchanged, because they were not touched.
#[test]
fn the_other_routes_to_an_object_key_are_unchanged() {
    use std::collections::BTreeMap;

    // A map: keys arrive through a generic `Serialize`, so they still take the
    // five-call path.
    let mut m: BTreeMap<String, u32> = BTreeMap::new();
    for (i, k) in ["a", "b\"c", "d\\e", "", "\u{1}"].iter().enumerate() {
        m.insert((*k).to_owned(), i as u32);
    }
    assert_eq!(
        turbo::to_string(&m).unwrap(),
        serde_json_upstream::to_string(&m).unwrap(),
        "map keys changed"
    );

    // A `Value`, which goes through `serialize_map` as well.
    let v: turbo::Value = turbo::from_str(r#"{"a":1,"b\"c":[2,{"":null}]}"#).unwrap();
    let u: serde_json_upstream::Value =
        serde_json_upstream::from_str(r#"{"a":1,"b\"c":[2,{"":null}]}"#).unwrap();
    assert_eq!(v.to_string(), u.to_string(), "Value keys changed");

    // An externally tagged enum variant, which writes its name as an object
    // key through `serialize_newtype_variant` -- a different call site again.
    #[derive(Serialize, Deserialize)]
    enum Tagged {
        #[serde(rename = "with\"quote")]
        A(u32),
        B {
            x: u32,
        },
    }
    for t in [Tagged::A(1), Tagged::B { x: 2 }] {
        assert_eq!(
            turbo::to_string(&t).unwrap(),
            serde_json_upstream::to_string(&t).unwrap(),
            "enum variant keys changed"
        );
    }
}
