//! The `no_std` probe crate: upstream's compile check, plus one that RUNS.
//!
//! Upstream's version of this file was three lines -- `#![no_std]` and
//! `pub use serde_json::*;` -- and that check is real: it proves every item in
//! the public surface resolves when the crate is built without `std`, which a
//! `cargo check` of the library alone does not, because an unreferenced
//! `#[cfg(feature = "std")]` item is not a compile error until someone names
//! it. **That re-export stays.**
//!
//! But it exercises no behaviour, and until M6 nothing anywhere did: the
//! `--no-default-features --features alloc` configuration was `cargo check`ed
//! on eight targets and RUN on none of them. A configuration that only ever
//! compiles is a configuration whose `#[cfg(not(feature = "std"))]` branches
//! have never executed -- and this crate has such branches, in its error type
//! and in `IoRead`'s absence.
//!
//! So [`run_all`] does real work through the real API using nothing but `core`
//! and `alloc`, and asserts the answers. It is a plain `pub fn` rather than a
//! `#[test]` because libtest needs `std`; `tests/no_std_runs.rs` is an
//! ordinary std test target that calls it, which works because an integration
//! test is a separate crate and does not make the library under test link
//! `std`.

#![no_std]

extern crate alloc;

pub use serde_json::*;

use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// Every assertion below, in one call. Panics on the first failure, which is
/// what a `no_std` probe can do without a test harness.
pub fn run_all() {
    parses_into_a_value();
    serializes_from_a_value();
    round_trips_the_number_tower();
    reports_errors_with_positions();
    walks_a_map();
    handles_the_escapes();
    borrows_and_owns_strings();
}

fn parses_into_a_value() {
    let v: Value = serde_json::from_str(r#"{"a":[1,2.5,true,null,"s"]}"#).unwrap();
    let a = v.get("a").unwrap().as_array().unwrap();
    assert_eq!(a.len(), 5);
    assert_eq!(a[0].as_u64(), Some(1));
    assert_eq!(a[1].as_f64(), Some(2.5));
    assert_eq!(a[2].as_bool(), Some(true));
    assert!(a[3].is_null());
    assert_eq!(a[4].as_str(), Some("s"));

    // `from_slice` too: it validates UTF-8 where `from_str` does not, and that
    // is a different code path.
    let bytes = br#"[1,2,3]"#;
    let v: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3);

    // Invalid UTF-8 must be REJECTED by `from_slice` -- the one thing only it
    // can check.
    let bad = [b'"', 0xFF, b'"'];
    assert!(serde_json::from_slice::<Value>(&bad).is_err());
}

fn serializes_from_a_value() {
    // `to_writer` is std-only (it wants `std::io::Write`); `to_string` and
    // `to_vec` are the alloc-only routes, and they are what this checks.
    let v = json!({"b": [1, 2], "a": "x"});
    let s = serde_json::to_string(&v).unwrap();
    let bytes = serde_json::to_vec(&v).unwrap();
    assert_eq!(s.as_bytes(), &bytes[..], "to_string and to_vec disagree");
    // Keys are ordered by the default `BTreeMap`, so this is exact.
    assert_eq!(s, r#"{"a":"x","b":[1,2]}"#);

    // Display on `Value` goes through the same serializer.
    assert_eq!(v.to_string(), s);

    let pretty = serde_json::to_string_pretty(&v).unwrap();
    assert!(pretty.contains("\n  \"a\": \"x\""), "pretty: {pretty}");

    // And the vector form of pretty, which is a separate entry point.
    let pv = serde_json::to_vec_pretty(&v).unwrap();
    assert_eq!(pretty.as_bytes(), &pv[..]);
}

fn round_trips_the_number_tower() {
    for (text, want) in [
        ("0", 0i64),
        ("-1", -1),
        ("9223372036854775807", i64::MAX),
        ("-9223372036854775808", i64::MIN),
    ] {
        let v: Value = serde_json::from_str(text).unwrap();
        assert_eq!(v.as_i64(), Some(want), "{text}");
        assert_eq!(v.to_string(), text, "{text} did not round-trip");
    }
    let v: Value = serde_json::from_str("18446744073709551615").unwrap();
    assert_eq!(v.as_u64(), Some(u64::MAX));

    // The eight-digit chunk fold (brick B4) runs on this path, so a `no_std`
    // build must produce the same answers as a std one.
    let v: Value = serde_json::from_str("123456789012345678").unwrap();
    assert_eq!(v.as_u64(), Some(123_456_789_012_345_678));
    let v: Value = serde_json::from_str("-1.7976931348623157e308").unwrap();
    assert_eq!(v.as_f64(), Some(-1.797_693_134_862_315_7e308));

    // Overflow of every kind must be an error, not a wrap.
    assert!(serde_json::from_str::<Value>("18446744073709551616").is_ok()); // becomes f64
    let e = serde_json::from_str::<u8>("256").unwrap_err();
    assert!(e.is_data(), "an out-of-range integer is a data error: {e}");
    assert!(e.to_string().contains("u8"), "message lost the type: {e}");
}

fn reports_errors_with_positions() {
    // The error type has a `#[cfg(not(feature = "std"))]` shape of its own, so
    // its Display text and its line/column are exactly what this probe exists
    // to execute.
    let e = serde_json::from_str::<Value>("{\"a\": }").unwrap_err();
    assert_eq!(e.line(), 1);
    assert_eq!(e.column(), 7);
    assert!(e.is_syntax(), "expected a syntax error");
    assert_eq!(e.to_string(), "expected value at line 1 column 7");

    // A newline moves the line, which is the counting path brick B7 rewrote.
    let e = serde_json::from_str::<Value>("[\n1,\n2,\n}").unwrap_err();
    assert_eq!((e.line(), e.column()), (4, 1));

    // EOF and trailing input are different categories, and `no_std` must
    // classify them the same way.
    assert!(serde_json::from_str::<Value>("[1,").unwrap_err().is_eof());
    assert!(serde_json::from_str::<Value>("[1] x").unwrap_err().is_syntax());

    // A data error, which carries a serde message rather than a parser one.
    let e = serde_json::from_str::<Vec<u8>>("[\"x\"]").unwrap_err();
    assert!(e.is_data(), "expected a data error: {e}");
}

fn walks_a_map() {
    let mut m = Map::new();
    m.insert("z".to_owned(), Value::from(1));
    m.insert("a".to_owned(), Value::from("two"));
    assert_eq!(m.len(), 2);
    assert!(m.contains_key("z"));
    let keys: Vec<&String> = m.keys().collect();
    assert_eq!(keys, vec![&"a".to_owned(), &"z".to_owned()], "not ordered");
    assert_eq!(m.remove("z").unwrap(), Value::from(1));
    assert!(!m.contains_key("z"));

    let v = Value::Object(m);
    assert_eq!(v.to_string(), r#"{"a":"two"}"#);

    // Indexing, which panics on the wrong type and is worth executing once.
    let v: Value = serde_json::from_str(r#"{"outer":{"inner":[10,20]}}"#).unwrap();
    assert_eq!(v["outer"]["inner"][1], Value::from(20));
    assert!(v["missing"].is_null());
}

fn handles_the_escapes() {
    // Brick B3's escape writer and the parser's unescaper, both directions,
    // including a surrogate pair -- which is the case that needs two `\u`
    // escapes to make one character.
    let text = "\"a\\\"b\\\\c\\nd\\u0001e\\ud83d\\ude00f\"";
    let v: Value = serde_json::from_str(text).unwrap();
    let s = v.as_str().unwrap();
    assert_eq!(s, "a\"b\\c\nd\u{1}e\u{1f600}f");
    // Re-serializing must reproduce escapes for exactly the bytes that need
    // them, and leave the astral character as itself.
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        "\"a\\\"b\\\\c\\nd\\u0001e\u{1f600}f\""
    );

    // A lone surrogate is invalid and must be rejected.
    assert!(serde_json::from_str::<Value>("\"\\ud83d\"").is_err());
}

fn borrows_and_owns_strings() {
    // Deserializing straight into `&str` BORROWS out of the input, which is
    // the zero-copy path brick B2 measures -- and it needs no derive, so this
    // probe keeps its dependency list at one crate.
    let s: &str = serde_json::from_str(r#""no escapes here""#).unwrap();
    assert_eq!(s, "no escapes here");

    // With an escape the borrow is impossible: the unescaped bytes do not
    // exist anywhere in the input. serde_json must then REFUSE to borrow
    // rather than hand back the raw text.
    assert!(
        serde_json::from_str::<&str>(r#""has\tan escape""#).is_err(),
        "a string needing unescaping must not be borrowed"
    );

    // An owned `String` takes the same input and unescapes it, which is the
    // scratch-buffer path.
    let owned: String = serde_json::from_str(r#""has\tan escape""#).unwrap();
    assert_eq!(owned, "has\tan escape");

    // And out of a slice, where UTF-8 validation happens on the way through.
    let from_bytes: String = serde_json::from_slice(r#""\u00e9t\u00e9""#.as_bytes()).unwrap();
    assert_eq!(from_bytes, "\u{e9}t\u{e9}");
}
