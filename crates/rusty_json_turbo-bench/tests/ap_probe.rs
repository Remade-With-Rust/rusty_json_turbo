//! Is "arbitrary_precision breaks a tagged enum with a float field" OUR bug or
//! upstream's? The fork's whole contract is byte-identical behaviour, so the
//! question is not "does it fail" but "does it fail the SAME WAY".
#![cfg(feature = "arbitrary_precision")]

use serde::Deserialize;

// The fields exist to be DESERIALISED INTO, which is the whole experiment;
// nothing reads them afterwards, and `Debug` is what prints them.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind")]
enum Tagged {
    #[serde(rename = "a")]
    A {
        #[allow(dead_code)]
        v: f64,
    },
}

#[derive(Deserialize, Debug)]
struct Plain {
    #[allow(dead_code)]
    v: f64,
}

#[test]
fn both_crates_agree_on_tagged_enum_with_a_float() {
    let doc = br#"{"kind":"a","v":1.5}"#;

    let ours = turbo::from_slice::<Tagged>(doc).map_err(|e| e.to_string());
    let theirs = serde_json_upstream::from_slice::<Tagged>(doc).map_err(|e| e.to_string());
    println!("tagged  ours   = {ours:?}");
    println!("tagged  theirs = {theirs:?}");
    assert_eq!(
        ours.is_err(),
        theirs.is_err(),
        "one crate accepted a tagged enum with a float and the other did not"
    );
    if let (Err(a), Err(b)) = (&ours, &theirs) {
        assert_eq!(a, b, "both failed but with different messages");
    }

    // The control: the same float field NOT behind a tag must work in both.
    let plain = br#"{"v":1.5}"#;
    assert!(turbo::from_slice::<Plain>(plain).is_ok());
    assert!(serde_json_upstream::from_slice::<Plain>(plain).is_ok());
}
