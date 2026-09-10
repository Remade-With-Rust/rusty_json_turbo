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

| 2026-09-09 | `src/read.rs`, `src/de.rs:255` | **Brick B1.** New sealed-trait method `Read::skip_whitespace`, default body = the loop `parse_whitespace` used to contain; `SliceRead` overrides it with one walk of the slice, `StrRead` delegates, `IoRead` keeps the default. `parse_whitespace` now delegates | whitespace is 71.9% of citm_catalog and 36% of the time to parse it into a `Value`; the old route paid a `Result<Option<u8>>` round trip per byte | **none** -- byte-identical, oracle + 300k soak green | LEDGER 2026-09-09 M2-B1: peek calls -91.1%/-95.2%/-20.3%, `peeks removed = ws_runs + ws_bytes` exactly; citm scan 1.168x at 15/15 |
| 2026-09-09 | `src/counters.rs` (new), `src/read.rs` | Deterministic work counters (`peek`, `next`, `discard`, `ws_runs`, `ws_bytes`) behind `profile`; `RJT_WS_FASTPATH` A/B knob behind `knobs`. Both compile out entirely when off | the clock cannot decide a brick on a throttled laptop; a count can, and it also proves the fast path is still *reached* | none when the features are off | -- |
| 2026-09-09 | `Cargo.toml` | `profile = ["std", "knobs"]`, new `knobs = ["std"]` | the counters need 64-bit atomics and the knob needs an environment; a measurement build is a host build | none | -- |

**Fork point, precisely.** The tree is upstream `master` at `afdf6fc`, which is the
v1.0.151 tag plus four commits that change no behaviour: remove the deprecated
`authors` field (99edc94, a3e9758), rename deprecated `f64` constants in
`lexical/math.rs` (577729e), and allow a clippy lint in a test (afdf6fc). The oracle
pins crates.io **1.0.151** (the tag), so those four commits are themselves under the
gate and it passes.

### `src/ser.rs`, `src/swar.rs` -- eight bytes per escape step (brick B3)

`format_escaped_str_contents` no longer walks one byte at a time through the
`ESCAPE` table. It asks `scan_to_escape` for the next byte needing an escape and
writes the clean run between, which produces the same `write_string_fragment` /
`write_char_escape` sequence with the same contents in the same order. The
scanner takes eight bytes per step using three exact SWAR masks, and
`scan_to_escape_scalar` -- the byte-at-a-time form, straight off upstream's
table -- stays in the tree permanently as its oracle and is reachable in
production via `RJT_ESC_WIDE=0`. The exact SWAR primitives moved to a new
`src/swar.rs` so the one subtle function in this crate has a single definition
and a single set of tests. Output is unchanged and the oracle gates it.

### `src/read.rs`, `src/ser.rs`, `src/de.rs` -- probe counters

`skip_to_escape` gained an `inline(always)` counting wrapper (the scanner itself
is untouched, now `skip_to_escape_impl`), and `as_str`, the borrow/copy branches
of `parse_str_bytes`, `MapKey::deserialize_any` and the escape loop each
increment a counter. Every one compiles to nothing without `--features profile`.

### `src/de.rs`, `src/read.rs` -- eight digits per step (brick B4)

`parse_integer` and `parse_decimal` consume eight ASCII digits per iteration via
a new sealed-trait method `Read::take_8_digits`, whose trait default returns
`None` so every reader that does not implement it keeps upstream's exact
byte-at-a-time behaviour. The chunk runs only while the significand is at or
below `SAFE_8_DIGIT_SIGNIFICAND`, the largest value for which eight more digits
cannot overflow a `u64`, so the per-digit `overflow!` check provably could not
have fired over that span. Output, errors and float bits are unchanged and the
oracle gates it. There is deliberately **no four-digit variant**; the comment in
`parse_decimal` records why, and `corpus/LEDGER.md` records the measurements.

### `src/read.rs` -- `IoRead::new` documentation (M5)

Upstream tells the reader to wrap the input in `std::io::BufReader` because
serde_json does not buffer, without distinguishing where the data comes from.
That advice is essential for a `File` -- `bytes()` is one `read` syscall per
byte without it -- and actively harmful for data already in memory, where there
are no syscalls to amortise and it only adds a second per-byte layer on top of
the bottleneck. Measured: a `BufReader` around a `&[u8]` costs `citm_catalog`
412 -> 375 MB/s and `twitter` 289 -> 248 MB/s.

The rewritten comment splits the two cases and points at `from_slice` for bytes
already in memory, which is 1.28x to 1.55x faster than any reader path because
it can scan whitespace sixteen bytes at a time. Documentation only; no
behaviour changes.

## The serde fork: ZERO divergences, and that is a decision

`Remade-With-Rust/serde` branch `turbo` sits at `a874a1b`, **unmodified**, and
is consumed through `[patch.crates-io]`. It carries no changes at all, which is
the recorded outcome of milestone M4 rather than an unfinished state.

M4 was written as a conditional -- "the serde fork earns its keep" -- with three
bricks. All three are now measured and none of them justifies a change:

| brick | verdict | evidence |
|---|---|---|
| B6, key dispatch on the JSON side | **refuted** | ceiling probe: 1.006x median, 0.970x best-of-N, 41 pairs, on the corpus's most key-dense document |
| B6h, field-index handshake across the seam | **refuted** | no floor to clear; it was the most invasive change contemplated anywhere in this project and was never written |
| B14, derive `deserialize_in_place` | **refuted** | unreachable: a panicking in-place impl is never called through `from_slice`, `from_str`, `from_reader` or `Deserializer::into_iter` |
| B6d, one shared identifier matcher | **not worth a fork alone** | 1.2% of `llvm-lines`, pre-dead-code-elimination, with no runtime claim |

The mechanism behind the first two is worth keeping: `rustc` lowers a
`match` on `&str` into a switch on length and then INLINE byte comparisons.
Across 111 derive-generated `visit_str` bodies in the harness there are **twenty
`memcmp` calls in total**, and most matchers have none. The derive already emits
close to the cheapest dispatch a field set admits, so there is nothing for a
JSON-side matcher or a seam handshake to remove.

**Why the patch stays wired anyway.** It costs nothing to keep and means a
future finding can be acted on the day it appears. Forking `serde_derive` for a
1.2% compile-time figure would mean tracking upstream indefinitely and keeping
serde's own suite green forever, for no measured speed.

**The gate any future change must clear**, recorded at `a874a1b`:
`cargo test --workspace` on the fork is **478 passed, 0 failed, 5 ignored**.

Full evidence: `corpus/LEDGER.md`, section M4.

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
