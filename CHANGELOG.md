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
- **M3: the SIMD island.** A new crate, `rusty_json_turbo-accel`, carrying SSE2
  and AVX2 twins of the whitespace and escape scanners. It is the one crate in
  the workspace where `unsafe` is allowed, so the parser itself never writes it
  for a vector load; it has no dependencies and no build script. Each kernel is
  written against a scalar oracle that stays in the tree permanently and is
  reachable in production through `RJT_ISA=scalar`.
  - **SSE2 ships, AVX2 does not, and that is measured.** AVX2 beat the 8-byte
    baseline on `citm_catalog` and lost on `twitter` (0.958x best-of-N) and
    `canada`; SSE2 beat it on **every** cell. The longest whitespace run
    anywhere in the corpus is **29 bytes**, so a 32-byte step is never fully
    used while its costs are paid on every run. AVX2 stays reachable via
    `RJT_ISA=avx2` so it can be re-measured elsewhere.
  - Against the previous 8-byte path, 61 pairs with controls flat:
    `citm_catalog` scan **1.105x** (61/61), `s4-media-probe` scan **1.094x**,
    `citm_catalog` struct parse **1.075x**, and on the serialize side `twitter`
    struct stringify **1.062x**, DOM stringify **1.060x**, `citm_catalog`
    struct stringify **1.053x**. Nothing anywhere below 0.987x.
  - Gated by twin tests against the oracle over all 256 byte values at every
    offset across the 8, 16 and 32-byte boundaries (>500,000 assertions per
    scanner), a poison test proving the suite catches a signed `b < 0x20` and a
    range whitespace test, and the byte-identical oracle plus soak at **all
    four** `RJT_ISA` rungs.
  - Every method line now prints `isa=<rung> (machine offers <ceiling>)`, since
    a run narrowed by an override and a run on a lesser machine otherwise
    produce the same number for different reasons.
- **Brick B15 built, measured and reverted**, and its reason retires B5 too.
  Folding a short string's quotes and contents into one sink call removed
  **36.6%** of `twitter`'s sink calls and was **11-15% slower** (61 pairs,
  61/61, best-of-N agreeing, controls flat). The buffer-zeroing explanation was
  ruled out by shrinking the buffer and getting the same loss. A 1-byte
  `write_all` of a constant is a capacity check and a store; folding trades two
  of those for a runtime-length copy in and out. **Call count is the wrong
  metric for a sink whose calls inline.**
- **Brick B10 respecified before being built**, by an allocation SIZE histogram
  and an object-arity census. A `BTreeMap` node holds eleven pairs and the
  median object has **two** keys, so the planned bulk build would have
  allocated a `Vec` per object to save nothing. The real cost splits: short
  strings dominate the allocation COUNT, `BTreeMap` nodes dominate the BYTES,
  and key reuse runs **80x to 1,419x** -- 46,816 string allocations for 33
  distinct names on one payload. Both fixes need `Map`'s key type or backing
  store to change, so they move to v1.x with the arena `Value`.
- **S4 registered in the harness**, so the house payloads are measurable and
  gated rather than merely present. `File` carries all ten documents with the
  classic trio frozen as `File::S1_S3` (a timing verb's `--all` still means
  those three, so a number quoted today stays comparable with one quoted before
  S4 existed) and `File::EVERY` for the oracle and census. The file-to-fixture
  mapping is now a single `by_fixture!` list that every dispatch site expands,
  so a document cannot be registered for the census and forgotten for the clock.
  Seven fixture modules carry the serde shapes S1-S3 never enter: a `flatten`
  tail over 25 distinct key sets, two internally-tagged enums, `Vec<u8>` byte
  arrays, `Option`-heavy configuration, and an 18-key field matcher over 2,600
  records.
- **A `latency` verb**, for the question `bench` cannot answer: not "how fast
  can we chew 600 KB" but "how long to handle one message", which is what a
  service with a p99 target operates on. S4's containers are sliced into their
  records with the **oracle's** `RawValue` -- deliberately not ours, since
  enabling `raw_value` on the crate under measurement would compile extra paths
  into it and move every timing in the suite. The verb reports p50/p90/p99/max
  and **measures its own timer overhead** in the same loop shape with the parse
  removed, so a reader can subtract it instead of trusting that it is small.
- **CI gates the new work**: `python tools/gen-s4.py --check` (S4 is generated,
  not fetched, so `HASHES.txt` cannot cover it and byte-for-byte regeneration is
  the equivalent check), plus both arms of the escape knob agreeing with
  upstream and a counter assertion that the wide escape scan is still reached.
- **Two corpus corrections, both found by counting rather than reading.** S1
  contains **no** hex escapes -- `corpus/README.md` claimed "Unicode escapes"
  for `twitter.json`, whose 14.75% non-ASCII is all raw UTF-8 -- so the
  hex-escape decoder and surrogate-pair path had never been on the timed corpus
  at all. And with no root `.gitattributes`, a `core.autocrlf=true` checkout
  makes `twitter.json` 646,995 bytes instead of 631,515, a 15,480-byte
  difference that is entirely carriage returns counted as whitespace; S1-S3 byte
  counts are therefore platform-dependent today. Both are now stated in
  `corpus/README.md`.
- **Brick B3**: the serializer's escape scan takes **eight bytes per step**.
  The escape table is nonzero for exactly `0x00..=0x1F`, `"` and a backslash,
  and `b < 0x20` is exactly `b & 0xE0 == 0`, so the predicate is three exact
  SWAR masks with no comparison. Byte-identical: the same
  `write_string_fragment` / `write_char_escape` calls in the same order.
  Measured in one binary with one env var between the arms, parse cells as
  controls: `twitter` struct stringify **1.208x** (61/61, best-of-N 1.207x),
  `twitter` DOM stringify **1.200x** (60/61, best-of-N 1.189x),
  `citm_catalog` struct stringify **1.068x** (61/61), DOM stringify 1.028x --
  with both control cells flat. The escape hit rate that makes it work is
  **0.334%** on `twitter` and **0.0009%** on `citm_catalog`.
- **M1-C: every remaining brick priced** by deterministic count rather than
  guess. B2 (wide string scan) is **demoted to near-closed** -- mean string run
  is 19.0 bytes, 98.3% of strings already keep the zero-copy borrow, and
  upstream is already 8-byte SWAR with `memchr2`. B5 (sink) is **promoted
  against the plan's own gate** -- 2.6 bytes per sink call on `citm_catalog`,
  about 7.3 calls per key. B12 (UTF-8) is **reframed**: the tempting
  validate-once-up-front form is not byte-identical and is recorded as
  rejected. New counters (`STR_*`, `ESC_*`, `KEYS`, `UTF8_*`) plus a counting
  `io::Write` in the harness. Full worklist: `corpus/LEDGER.md`.
- **Emitted-asm census** (`tools/asm-census.ps1`, `docs/ASM-CENSUS.md`):
  per-source-file attribution through CodeView inline-frame chains. 63 panic
  sites (`de.rs` **zero**, closing `get_unchecked` there; three `read.rs` sites
  share an `==`-shaped length check that blocks bound folding, opening a safe
  brick), 46 `mem*` calls with `ser.rs` owning **zero**, and **no
  auto-vectorisation** anywhere.
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
