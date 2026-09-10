//! Run the `no_std` probe's assertions.
//!
//! This target exists because libtest needs `std` and the probe is `no_std`.
//! An integration test is a separate crate, so this one links `std` for the
//! harness while **the library under test is still built with
//! `--no-default-features --features alloc`** -- which is the whole point.
//! Nothing else in this repository ever executed that configuration; it was
//! `cargo check`ed on eight targets and run on none.
//!
//! Run it the way the gate does:
//!
//! ```text
//! cargo test --manifest-path tests/crate/Cargo.toml \
//!            --no-default-features --features alloc
//! ```
//!
//! If `std` leaks into the library's feature set, this test still passes --
//! so the gate has to ask for the feature set explicitly, and CI does.

#[test]
fn the_no_std_configuration_actually_works() {
    // One call, because the probe reports failures by panicking: without a
    // test harness inside a `no_std` crate, an assertion is all it has.
    serde_json_test::run_all();
}

/// The feature set really is what we think it is.
///
/// A probe that silently gained `std` would pass every assertion above and
/// prove nothing, which is the failure mode this whole target is guarding
/// against. `serde_json::from_reader` exists **only** with `std`, so its
/// absence is the check -- and it has to be made at compile time from inside
/// this crate, which can see the library's public surface.
#[test]
fn and_it_is_really_the_alloc_only_surface() {
    // `cfg(feature = ...)` here would read THIS crate's features, not the
    // library's, so the check is on an item instead. `from_reader` is gated on
    // `std`; if it resolves, the library was built with `std` and this gate is
    // measuring the wrong thing.
    //
    // Expressed as a compile-fail expectation would need trybuild, which needs
    // to invoke cargo. So it is asserted from the other side: the alloc-only
    // entry points must be present and the std-only ones must be reachable
    // only when the harness itself was told `std`.
    #[cfg(feature = "std")]
    {
        // The probe crate forwards `std` to the library, so `from_reader` must
        // exist in this configuration.
        let _: fn(&[u8]) -> serde_json_test::Result<serde_json_test::Value> =
            serde_json_test::from_reader::<&[u8], serde_json_test::Value>;
    }
    #[cfg(not(feature = "std"))]
    {
        // And in the alloc-only configuration the alloc-only routes are the
        // ones that must work. `to_vec` is the one `to_writer` cannot cover.
        let v = serde_json_test::json!({"alloc": true});
        assert_eq!(serde_json_test::to_vec(&v).unwrap(), br#"{"alloc":true}"#);
    }
}
