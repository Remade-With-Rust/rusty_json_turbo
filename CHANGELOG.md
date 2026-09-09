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

### Changed

- Nothing in the library's behaviour. The oracle test proves every corpus file,
  edge document and number token byte-identical to upstream (bytes, `Value`,
  error text with line/column, float bits). Three MSRV-gated clippy sites in
  upstream code were rewritten behaviour-identically (`docs/UPSTREAM-CHANGES.md`).
