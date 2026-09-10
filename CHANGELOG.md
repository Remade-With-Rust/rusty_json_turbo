# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); performance entries
carry their measured numbers and method line, never an adjective.

## [Unreleased]

### Added

- M0 scaffold: the fork of serde_json 1.0.151 as `rusty_json_turbo` (library
  crate name `serde_json`), the `rjson` CLI, the allocator seam, the harness
  crate with the in-process differential oracle against upstream and the paired
  ABBA benchmark, the JSONCORP corpus with SHA-256 manifest, `deny.toml`, CI,
  and `corpus/LEDGER.md`.

- M0 baseline in `corpus/LEDGER.md` (2026-09-09, commit `b3df966`): pinned
  cpu2/High, ABBA, N = 20, 250 ms windows; null-arm floor (medians within 2.3%);
  ours vs upstream (identical source; one cell 0.968x at 20/20 -- a link-layout
  bias, recorded as that cell's floor); ours vs simd-json 0.18.1 (ours ahead or
  tied on 11 of 12 cells; simd-json 1.63x on twitter DOM parse); ours vs
  sonic-rs 0.5.8 (sonic-rs 1.5-3.6x on DOM parse via its arena Value, 3-8%
  ahead on two struct-parse cells, behind on every stringify cell; measured
  without its mandated `target-cpu=native`, stated as a lower bound).

- M1 instruments: an allocation census behind `--features profile` (a counting
  wrapper around whichever backend is linked, never quoted for timings), the
  `solo` and `census` verbs on `rjson-bench`, `tools/pinvs.ps1` for
  process-level paired A/B between two binaries, and the allocator as a build
  arm (`--features rusty-alloc` / `rusty-alloc-secure`) through the seam.
- M1 fuzz targets: `from_str`, `from_reader`, `roundtrip`, `raw_value`,
  `stream`, and `oracle_diff` (ours against upstream on arbitrary bytes,
  comparing outcome, printed bytes, error text with line/column, and float
  bits) alongside upstream's `from_slice`.
- M1-A result in `corpus/LEDGER.md`: with only the house allocator linked and
  no code change, DOM parse gains 1.46-1.53x (2 s samples; 2.0x at 250 ms) and
  struct parse 1.07-1.17x, while the four zero-allocation stringify cells are
  unmoved -- a control the census supplied for free. The oracle passes under
  `rusty_alloc` and `secure`: no output byte changes. Stated there, and here:
  this is the allocator's win, not the fork's.

- M1 instruments: a **content census** (`rjson-bench probe`) that counts where a
  document's bytes go, a **scan-only ceiling** (`IgnoredAny`, no stubbing) that
  bounds every value-construction brick, an exact **whitespace ceiling** (the
  same document with skippable whitespace removed, asserted to parse equal),
  and **deterministic work counters** (`rjson-bench work`, library feature
  `profile`) with an `RJT_*` A/B knob (feature `knobs`) so two implementations
  can be compared inside one binary rather than across two code layouts.
- **Brick B4**: integer and fraction digits are consumed **eight at a time** --
  one 8-byte load, three integer ops to validate, a three-multiply fold -- while
  the significand is below a bound at which eight more digits provably cannot
  overflow a `u64`; below that bound the chunk takes exactly the branch the
  byte-at-a-time loop would have taken, so the result is byte-identical
  including the digit at which a long number switches to the slow float path.
  Measured in one binary with one env var between the arms, 21 pairs, ABBA,
  pinned: `canada` struct parse **1.083x** (21/21), DOM parse **1.043x**
  (19/21), `citm_catalog` struct parse 1.017x (17/21), `twitter` at the floor --
  the gradient is the corpus census read back, and nine control cells stayed
  inside 1%. A **four-digit** step on top of it was built twice and reverted
  twice: at the fraction call site it hits 99.91% of the time and removes a
  third of `canada`'s `peek` calls, and is still 2.5% slower, because the fold
  has a fixed cost that does not shrink with width. See `corpus/LEDGER.md`.
- **Brick B1s**: the whitespace scan now takes eight bytes per step (SWAR, exact
  zero-byte test), with a four-byte scalar peel so a short run never reaches the
  wide path and an `inline(always)` entry so "no whitespace here" stays one load
  and one test. Byte-identical. Against upstream's original loop, measured in one
  binary with one env var between the arms, 21 pairs on a quiet machine:
  `citm_catalog` scan **1.345x**, struct parse **1.246x**, DOM parse **1.095x**;
  `canada` scan 1.053x; `twitter` scan 1.042x, struct parse 1.036x — each 21/21 —
  with the four stringify control cells unmoved (0.997x–1.008x). Two regressions
  were caught during development by the corpus files the change cannot help, and
  fixed; see `corpus/LEDGER.md`.
- **Brick B1** (first performance change to the JSON code): whitespace skipping
  moved off the per-byte `peek()`/`discard()` path into one walk of the slice,
  via a new sealed-trait method `Read::skip_whitespace` whose default body is
  the loop it replaces. Byte-identical. Peek calls fall 91.1% on
  `citm_catalog`, 95.2% on `twitter`, 20.3% on `canada`; `citm_catalog` scan is
  1.168x faster in 15 of 15 paired runs, with four stringify control cells
  correctly unmoved. Full evidence and limits: `corpus/LEDGER.md`.

### Changed

- Nothing in the library's behaviour. The oracle test proves every corpus file,
  edge document and number token byte-identical to upstream (bytes, `Value`,
  error text with line/column, float bits). Three MSRV-gated clippy sites in
  upstream code were rewritten behaviour-identically (`docs/UPSTREAM-CHANGES.md`).
