//! The shim IS the fork, not a lookalike.
//!
//! A whole-graph `[patch.crates-io]` is only safe if every crate that says
//! `serde_json::Value` ends up naming ONE type. If this shim recompiled the
//! source instead of re-exporting it, each would still compile and each would
//! still behave correctly -- and a `Value` passed from a patched axum to
//! patched consumer code would not type-check. That fault is invisible to any
//! behavioural test, so it is checked the only way it can be: by writing code
//! that compiles only if the types are identical.

#[test]
fn the_shim_and_the_fork_are_the_same_types() {
    // Built through the shim, then MOVED into a binding typed by the fork.
    // Two distinct types would fail to compile here, not fail an assertion.
    let via_shim: serde_json::Value = serde_json::json!({"a": [1, 2.5, null], "b": "x"});
    let via_fork: turbo::Value = via_shim;
    assert!(via_fork.is_object());

    // And the other direction, through every type a consumer boundary carries.
    let map: serde_json::Map<String, serde_json::Value> = turbo::Map::new();
    let _: turbo::Map<String, turbo::Value> = map;

    let n: serde_json::Number = turbo::Number::from(7u64);
    let _: turbo::Number = n;

    let e: serde_json::Error = turbo::from_str::<turbo::Value>("{").unwrap_err();
    let _: turbo::Error = e;
}

#[test]
fn the_public_surface_still_works_through_the_shim() {
    // The macro, both entry points, both directions, and a nested module path
    // -- the four things a consumer's `use serde_json::...` lines actually name.
    let v = serde_json::json!({"z": 1, "a": [true, null, 2.5]});
    let s = serde_json::to_string(&v).expect("to_string");
    assert_eq!(s, r#"{"a":[true,null,2.5],"z":1}"#);
    assert_eq!(serde_json::to_vec(&v).expect("to_vec"), s.as_bytes());

    let back: serde_json::Value = serde_json::from_str(&s).expect("from_str");
    assert_eq!(back, v);
    let from_bytes: serde_json::Value = serde_json::from_slice(s.as_bytes()).expect("from_slice");
    assert_eq!(from_bytes, v);

    // A module path, not just the re-exported root items.
    let mut de = serde_json::Deserializer::from_str(&s);
    let streamed: serde_json::Value =
        serde::Deserialize::deserialize(&mut de).expect("Deserializer");
    assert_eq!(streamed, v);
    de.end().expect("end");

    // Errors carry position, which is the part a consumer's error handling
    // touches most.
    let err = serde_json::from_str::<serde_json::Value>("[1,\n2,\n}").unwrap_err();
    assert_eq!((err.line(), err.column()), (3, 1));
    assert!(err.is_syntax());

    // std::io route, which only exists with `std` -- so this also checks that
    // the default feature forwarded.
    let mut out = Vec::new();
    serde_json::to_writer(&mut out, &v).expect("to_writer");
    assert_eq!(out, s.as_bytes());
    let read: serde_json::Value = serde_json::from_reader(&out[..]).expect("from_reader");
    assert_eq!(read, v);
}
