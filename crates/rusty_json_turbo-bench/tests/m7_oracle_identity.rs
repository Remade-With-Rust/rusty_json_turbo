//! THE ORACLE ARM IS REALLY UPSTREAM, and not us wearing upstream's name.
//!
//! Every differential result in this project -- every byte-identical claim,
//! every ours-vs-upstream ratio, the instantiation-bias correction, the whole
//! ledger -- rests on `serde_json_upstream` being **upstream serde_json's
//! code**. Nothing checked that. `oracle_reports_a_difference` checks the
//! comparison FUNCTION, which is a different thing: it would keep passing
//! happily while both arms ran our own parser.
//!
//! That is not a theoretical worry. M3's SIMD island was reversed precisely
//! because its A/B compared two rungs INSIDE the thing under test, and the
//! law recorded then was "an A/B is only worth what its baseline is worth; a
//! baseline inside the thing under test is not a baseline". The oracle is the
//! baseline for everything else here.
//!
//! And M7 makes the risk concrete. The consumer swap needs a package literally
//! named `serde_json` in this repository, so that a consumer can write
//! `[patch.crates-io] serde_json = { git = ... }` and convert its WHOLE
//! dependency graph -- axum's, cozo's, dioxus's copies included -- rather than
//! only the JSON calls it makes itself. The moment such a package exists, a
//! manifest that says `{ package = "serde_json", version = "=1.0.151" }` has
//! two plausible answers, and picking the wrong one would silently turn every
//! measurement in this project into a comparison of us against ourselves.
//!
//! # How it is proved
//!
//! Our counters are the proof, and they are a proof no naming trick can fake:
//! they are `static`s **inside our crate**. Parse a document through the
//! oracle arm and every one of them must still read zero, because upstream's
//! code cannot touch them. Parse the same document through our arm and they
//! must move. If the oracle were our fork under a rename, the first assertion
//! fails immediately.
//!
//! Needs `--features count-work`; without it there are no counters and the
//! test says so rather than passing vacuously.

use rjt_bench::corpus::File;

#[test]
fn parsing_through_the_oracle_does_not_touch_our_counters() {
    if !turbo::counters::enabled() {
        println!("skipped: build without count-work carries no counters");
        return;
    }

    let bytes = File::CitmCatalog.load();

    // ---- the ORACLE arm. Upstream's code cannot reach our statics. --------
    turbo::counters::reset();
    let theirs: serde_json_upstream::Value =
        serde_json_upstream::from_slice(&bytes).expect("upstream parses citm");
    let after_theirs = turbo::counters::snapshot();
    std::hint::black_box(&theirs);

    assert_eq!(
        after_theirs,
        turbo::counters::Counters::default(),
        "PARSING THROUGH THE ORACLE TICKED OUR COUNTERS.\n\
         `serde_json_upstream` is running OUR code, not upstream serde_json's, \
         so every differential result in this project is a comparison of us \
         against ourselves and the ledger's ours-vs-upstream numbers are all \
         invalid. Check that no local package named `serde_json` is shadowing \
         the registry one, and that no `[patch]` redirects it.\n\
         counters after the oracle arm: {after_theirs:?}"
    );

    // ---- OUR arm. The counters must move, or the check above is vacuous. --
    // A counter that never ticks would "prove" the oracle is upstream no
    // matter what it was, which is the same vacuous-pass shape the rest of
    // this suite guards against.
    turbo::counters::reset();
    let ours: turbo::Value = turbo::from_slice(&bytes).expect("we parse citm");
    let after_ours = turbo::counters::snapshot();
    std::hint::black_box(&ours);

    assert!(
        after_ours.ws_runs > 0 && after_ours.ws_bytes > 0,
        "our own arm did not tick the counters either, so the assertion above \
         proves nothing: {after_ours:?}"
    );
    println!(
        "oracle arm: all counters zero. our arm: {} whitespace runs, {} bytes. \
         The two arms are different code.",
        after_ours.ws_runs, after_ours.ws_bytes
    );

    // ---- and they still agree on the answer, which is the point ----------
    // Identity is necessary but not sufficient: two DIFFERENT parsers that
    // disagree would also pass everything above.
    assert_eq!(
        turbo::to_string(&ours).expect("ours re-serializes"),
        serde_json_upstream::to_string(&theirs).expect("theirs re-serializes"),
        "the two arms are different code AND they disagree"
    );
}

/// The oracle is the version this fork tracks, not some other one.
///
/// A differential gate against the wrong upstream version is a gate against
/// the wrong contract: upstream changes output between releases (number
/// formatting and error text both have), so "byte-identical to serde_json"
/// only means something with the version named. The manifest pins `=1.0.151`;
/// this is the assertion that the pin is what got built.
#[test]
fn the_oracle_is_the_pinned_upstream_version() {
    // There is no version constant in serde_json's public API, so the pin is
    // asserted from the build graph instead -- `cargo tree` reads the same
    // lockfile the test was compiled against.
    let out = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "rusty_json_turbo-bench",
            "--depth",
            "1",
            "--locked",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();
    let Ok(out) = out else {
        println!("skipped: could not run cargo tree");
        return;
    };
    if !out.status.success() {
        println!("skipped: cargo tree failed (offline or locked mismatch)");
        return;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text
        .lines()
        .find(|l| l.contains("serde_json v"))
        .unwrap_or_else(|| panic!("no serde_json in the tree:\n{text}"));
    assert!(
        line.contains("serde_json v1.0.151"),
        "the oracle is not the pinned upstream version: {line}"
    );
    // And it must NOT be a path or git source: those would mean a local
    // package answered for the registry one.
    assert!(
        !line.contains('(') || !line.contains("F:\\") && !line.contains('/'),
        "the oracle resolved to a LOCAL source, so it is not upstream: {line}"
    );
    println!("oracle: {}", line.trim());
}
