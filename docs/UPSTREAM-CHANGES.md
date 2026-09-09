# Divergences from upstream serde_json

Every difference between this tree and upstream `serde-rs/json` at the fork
point (v1.0.151, `afdf6fc`), one per row. A row that changes behaviour cites
the ledger run that gated it byte-identical; a row that does not says so. Keep
this current in the same commit as the change: it is the merge map for
`git merge upstream/master` and the honesty ledger for consumers.

| Date | Area | Change | Why | Behaviour | Ledger |
|---|---|---|---|---|---|
| 2026-09-09 | `Cargo.toml` | Package renamed `rusty_json_turbo` 0.1.0; library target keeps crate name `serde_json`; MSRV 1.71 -> 1.85; `exclude` list; `profile = []` feature; crate-level `[lints.rust]` (`unsafe_code = "allow"` until M2, `unsafe_op_in_unsafe_fn = "deny"`); workspace, `[workspace.*]`, `[profile.release]`, `[patch.crates-io]` to the serde fork appended | mission plan §4, §9.1, §9.2, §9.5, §9.11 | none | -- |
| 2026-09-09 | `tests/crate/Cargo.toml`, `fuzz/Cargo.toml` | path dependency gains `package = "rusty_json_turbo"` | the package was renamed; both keep depending on the root crate | none | -- |
| 2026-09-09 | `.gitignore` | `Cargo.lock` is now committed; local `.cargo/config.toml` and `corpus/runs/` ignored | house supply-chain rule (Deputy owns the lockfile from commit one) | none | -- |
| 2026-09-09 | `README.md` | replaced with the house README (no performance claim until the ledger has one) | upstream's README documents upstream; ours documents the contract | none | -- |
| 2026-09-09 | `.github/workflows/ci.yml` | replaced: 3-OS tests, upstream feature matrix, differential oracle per feature, 8-target check matrix, no_std probe, MSRV, lint (fmt, clippy -D warnings, ASCII, cargo-deny), harness, package; `portfolio-check.yml` added | house CI shape | none | -- |
| 2026-09-09 | new files | `deny.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `NOTICE.md`, `SECURITY.md`, `CHANGELOG.md`, `docs/`, `corpus/`, `crates/`, `scripts/`, `tools/` | house scaffold | none | -- |
| 2026-09-09 | `src/value/mod.rs:241` | `#[allow(clippy::io_other_error)]` on the local `io_error` fn | clippy `io_other_error` is MSRV-gated and woke up at 1.85 (upstream's 1.71 suppressed it); the call cannot change because the no_std shim (`src/io/core.rs`) has no `Error::other` | none | -- |
| 2026-09-09 | `src/de.rs:827`, `src/lexical/math.rs:504`, `tests/test.rs:1906,1918` | `iter::repeat(x).take(n)` -> `iter::repeat_n(x, n)` | clippy `manual_repeat_n`, same MSRV cause; identical iteration | none | oracle green, M0 |

**Fork point, precisely.** The tree is upstream `master` at `afdf6fc`, which is the
v1.0.151 tag plus four commits that change no behaviour: remove the deprecated
`authors` field (99edc94, a3e9758), rename deprecated `f64` constants in
`lexical/math.rs` (577729e), and allow a clippy lint in a test (afdf6fc). The oracle
pins crates.io **1.0.151** (the tag), so those four commits are themselves under the
gate and it passes.

## Known inherited exceptions (not divergences, tracked)

- `serde_stacker` (dev-dependency) pulls `psm`, which compiles C. Dev-only, never
  in an artifact, excluded from `cargo deny` via `exclude-dev`. M1 replaces the
  single doc example that references it and drops the dependency.
- 12 `unsafe` sites in `src/` (read.rs: `offset_from` in the SWAR scanner,
  `from_utf8_unchecked` in `StrRead::parse_str`, raw writes + `set_len` in
  `push_wtf8_codepoint`; ser.rs: `from_utf8_unchecked` x2, `unreachable_unchecked`,
  `String::from_utf8_unchecked` x2; raw.rs: `transmute` x3 and
  `from_string_unchecked`; value/mod.rs: `from_utf8_unchecked`). M2 moves or
  replaces them so the core can take `forbid(unsafe_code)`.

## Merge recipe

```sh
git fetch upstream
git merge upstream/master          # src/, tests/, build.rs merge cleanly: nothing moved
# resolve Cargo.toml (keep ours, port any new upstream dependency/feature rows)
cargo test --workspace             # the oracle re-gates the merge byte-identical
# add a row above for anything upstream changed that we had also changed
```
