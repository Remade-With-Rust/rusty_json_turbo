# NOTICE

`rusty_json_turbo` is a fork of **serde_json** (<https://github.com/serde-rs/json>),
copyright its authors -- Erick Tryzelaar, David Tolnay and contributors -- and
licensed under MIT OR Apache-2.0. The fork point is serde_json **v1.0.151**
(commit `afdf6fc`, 2026-08-07). `LICENSE-MIT` and `LICENSE-APACHE` are
upstream's, unchanged, and every file under `src/`, `tests/` (except `tests/crate`
edits noted below), `build.rs` and `fuzz/` began as upstream's; the divergences
are listed, one per row, in `docs/UPSTREAM-CHANGES.md`.

The companion fork of **serde** (<https://github.com/Remade-With-Rust/serde>,
branch `turbo`) forks <https://github.com/serde-rs/serde> at v1.0.229 (commit
`a874a1b`, 2026-08-24), same authors, same licences, same terms.

## Corpus

`corpus/canada.json`, `corpus/citm_catalog.json`, `corpus/twitter.json`,
`corpus/jsonchecker/*` and `corpus/roundtrip/*` are the data files of
**serde-rs/json-benchmark** (commit `17b13dd`, MIT OR Apache-2.0), which in turn
carries them from **nativejson-benchmark** (Milo Yip, MIT). `twitter.json` is the
Twitter search-API sample that project published; `canada.json` is the Natural
Earth Canada boundary as GeoJSON; `citm_catalog.json` is a CITM event catalogue.
SHA-256 of every file: `corpus/HASHES.txt`; re-fetch and verify:
`corpus/fetch.ps1`.

## Harness fixtures

`crates/rusty_json_turbo-bench/src/{twitter,citm_catalog,canada,empty,enums,prim_str}.rs`
are verbatim copies of json-benchmark's `src/copy/*.rs` and `src/*.rs` (MIT OR
Apache-2.0) so that the struct columns measure the same shapes the published
serde_json numbers were taken on. `color.rs` is a safe rewrite of the same
fixture (upstream's used raw pointer copies for a 6-byte hex string).

## What is NOT distributed

Nothing here links or distributes a C library. simd-json and sonic-rs are
bench-only dependencies behind `--features competitors` and ship in no artifact.
