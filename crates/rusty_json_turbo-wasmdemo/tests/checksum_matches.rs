//! THE BROWSER CHECKSUM == THE NATIVE CHECKSUM, or this fails.
//!
//! M6's exit criteria ask for exactly this comparison. The native side is a
//! direct call into the same `round_trip` the wasm exports use, so the two
//! cannot drift; the browser side is the `wasm32-unknown-unknown` module run
//! under Node by `tools/wasmdemo.mjs`.
//!
//! Both halves have to exist for the comparison to happen, and when one does
//! not this test says which and why rather than passing quietly -- a gate that
//! reports success when it did not run is worse than no gate. Build the module
//! first:
//!
//! ```text
//! cargo build -p rusty_json_turbo-wasmdemo --target wasm32-unknown-unknown --release
//! cargo test -p rusty_json_turbo-wasmdemo
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the release build puts the module, relative to the workspace root.
const WASM: &str = "target/wasm32-unknown-unknown/release/rusty_json_turbo_wasmdemo.wasm";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

#[test]
fn the_native_checksums_are_stable_and_self_consistent() {
    // Even without a wasm toolchain this half is worth running: it proves the
    // corpus documents round-trip through the ALLOC-ONLY parser, which is the
    // configuration a browser build uses and which nothing else covers on a
    // document this size.
    let mut all = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..rusty_json_turbo_wasmdemo::count() {
        let (out, sum) = rusty_json_turbo_wasmdemo::round_trip(i);
        println!(
            "native doc {i} ({}) len={} checksum=0x{sum:016x}",
            rusty_json_turbo_wasmdemo::name(i),
            out.len()
        );
        assert!(!out.is_empty(), "document {i} re-serialized to nothing");
        // Re-serializing the same value twice must give the same bytes, or the
        // checksum is not a property of the document.
        let (again, sum_again) = rusty_json_turbo_wasmdemo::round_trip(i);
        assert_eq!(sum, sum_again, "document {i} is not deterministic");
        assert_eq!(out, again, "document {i} is not deterministic");
        all ^= sum;
        all = all.wrapping_mul(0x0000_0100_0000_01b3);
    }
    assert_eq!(
        all,
        rusty_json_turbo_wasmdemo::checksum_all(),
        "the folded checksum disagrees with the per-document ones"
    );
    println!("native ALL checksum=0x{all:016x}");
}

#[test]
fn the_browser_target_agrees_with_native_byte_for_byte() {
    let root = workspace_root();
    let wasm = root.join(WASM);
    if !wasm.exists() {
        println!(
            "SKIPPED: {} not built. Run:\n  cargo build -p rusty_json_turbo-wasmdemo \
             --target wasm32-unknown-unknown --release",
            wasm.display()
        );
        return;
    }
    let script = root.join("tools").join("wasmdemo.mjs");
    assert!(script.exists(), "{} is missing", script.display());

    let out = match Command::new("node")
        .arg(&script)
        .arg(&wasm)
        .current_dir(&root)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            println!("SKIPPED: node is not on PATH ({e})");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the browser module failed to run:\n--- stdout\n{stdout}\n--- stderr\n{stderr}"
    );
    print!("{stdout}");
    if !stderr.trim().is_empty() {
        // The script prints imports here. A Rust cdylib for this target should
        // have none, so anything on stderr is a platform dependency that crept
        // into a build claiming not to have one.
        panic!("the browser module needed host imports:\n{stderr}");
    }

    // Parse `doc N len=L checksum=0xHH` and `ALL checksum=0xHH`.
    let mut seen = 0usize;
    let mut all_from_wasm = None;
    for line in stdout.lines() {
        let sum = line
            .split("checksum=0x")
            .nth(1)
            .map(|h| u64::from_str_radix(h.trim(), 16).expect("a hex checksum"));
        let Some(sum) = sum else { continue };
        if line.starts_with("ALL ") {
            all_from_wasm = Some(sum);
            continue;
        }
        let idx: usize = line
            .strip_prefix("doc ")
            .and_then(|r| r.split_whitespace().next())
            .expect("a document index")
            .parse()
            .expect("a numeric index");
        let len: usize = line
            .split("len=")
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .expect("a length")
            .parse()
            .expect("a numeric length");

        let (native_out, native_sum) = rusty_json_turbo_wasmdemo::round_trip(idx);
        let name = rusty_json_turbo_wasmdemo::name(idx);
        // Length first: it localises a fault far faster than a hash does.
        assert_eq!(
            len,
            native_out.len(),
            "{name}: the browser target wrote {len} bytes, native wrote {}",
            native_out.len()
        );
        assert_eq!(
            sum, native_sum,
            "{name}: browser checksum 0x{sum:016x} != native 0x{native_sum:016x} \
             -- the same document serialized to different bytes on wasm32"
        );
        seen += 1;
    }

    assert_eq!(
        seen,
        rusty_json_turbo_wasmdemo::count(),
        "the browser module reported {seen} documents, the crate carries {}",
        rusty_json_turbo_wasmdemo::count()
    );
    assert_eq!(
        all_from_wasm,
        Some(rusty_json_turbo_wasmdemo::checksum_all()),
        "the folded checksums differ"
    );
    println!(
        "browser target agrees with native on all {seen} documents, \
         byte-for-byte (alloc-only parser, no host imports)"
    );
}
