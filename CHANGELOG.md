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

### Changed

- Nothing in the library's behaviour. The oracle test proves every corpus file,
  edge document and number token byte-identical to upstream (bytes, `Value`,
  error text with line/column, float bits).
