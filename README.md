### In The Wild with 10 Active Installs
> [RAG Converter](https://ragconverter.com) uses `rusty_json_turbo` for JSON.
> It makes personal and work files AI-readable without them leaving the machine:
> the whole conversion runs as WebAssembly in the browser tab, with nothing
> uploaded and nothing to install.

# rusty_json_turbo

[![CI](https://github.com/Remade-With-Rust/rusty_json_turbo/actions/workflows/ci.yml/badge.svg)](https://github.com/Remade-With-Rust/rusty_json_turbo/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/Remade-With-Rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network)

> **rusty_json_turbo** is [serde_json](https://github.com/serde-rs/json), forked
> and made fast the house way: the **same public API**, the **same bytes out**,
> the **same errors at the same line and column** -- gated byte-identical
> against upstream on every commit -- with speed earned one measured brick at
> a time. Pure Rust, no C, no `*-sys`, no FFI, `no_std + alloc` capable, dual
> MIT / Apache-2.0 exactly as upstream. The library keeps the crate name
> `serde_json`, so `use serde_json::...` is the whole migration.

Part of **[Remade With Rust](https://github.com/Remade-With-Rust)** by
**[Mata Network](https://www.mata.network/)** -- the JSON layer under every
public API the house ships.
[Jump to the ecosystem](#the-remade-with-rust-ecosystem)

---

## The headline

**Status: M0 complete (scaffold, oracle, harness, baseline).** There is no
performance claim on this page yet: at M0 the fork is upstream's code, and the
ledger says so to within its floor. What exists today:

- **The fork**, at serde_json 1.0.151 (`afdf6fc`), unchanged in behaviour.
  Every upstream test passes against it.
- **The oracle.** Upstream serde_json is linked in-process next to the fork,
  and a differential test proves, over the whole corpus plus a few hundred edge
  documents and every number token in it: identical compact and pretty bytes,
  equal `Value`, identical error text with line, column and category, identical
  `f64`/`f32` **bit patterns**. It runs in CI on every push and under every
  feature flag with both arms on.
- **The harness.** A paired ABBA benchmark over json-benchmark's three files,
  arms alternating lead, a null arm for the floor, a method line on every
  table. Numbers land in [`corpus/LEDGER.md`](corpus/LEDGER.md) and nowhere
  else first.
- **The plan.** [`docs/plans/fast_mission.md`](docs/plans/fast_mission.md):
  the brick catalog, the gates, the milestones, the decisions.

| | serde_json (upstream) | **rusty_json_turbo** |
|---|---|---|
| Public API | the reference | **identical**, item for item, every feature flag |
| Output bytes, errors, float bits | the reference | **byte-identical by contract**, gated per commit |
| C / `*-sys` in the dependency tree | none | **none** |
| `unsafe` | 12 sites | 12 sites inherited; **M2 moves them to one audited island** |
| `no_std + alloc`, `wasm32` | yes | **yes**, checked on 8 targets in CI |
| License | MIT OR Apache-2.0 | **MIT OR Apache-2.0** |
| Speed | the baseline | **measured before claimed** -- see the ledger |

### Performance -- measured rather than asserted

No number enters this README without a method line. The fork's own JSON code is
still upstream's, so **there is no claim yet that this crate is faster than
serde_json** -- that is what the brick campaign is for, and each brick's ledger
row lands before its sentence here.

The M0 baseline is in [`corpus/LEDGER.md`](corpus/LEDGER.md) with its pin,
pairs, window, null-arm floor and machine: upstream against itself (the floor,
medians within 2.3%), ours against upstream (identical source; one cell shows a
3% link-layout bias, which is now that cell's floor), and ours against
simd-json 0.18 and sonic-rs 0.5 on all twelve json-benchmark cells. Two honest
readings from that table: upstream serde_json already beats simd-json on eleven
of twelve cells under this method, and sonic-rs's 1.5-3.6x DOM-parse lead is
its arena `Value`, not its scanner.

There are two performance stories here and they must not be added together in
the reader's head, so they get two tables. **Table 1 is what you can have
today**, from the house stack alone, with no code change anywhere. **Table 2 is
what this project is actually for** — the changes to the JSON code itself, each
of which lands with its own ledger row and its own byte-identical gate.

#### Table 1 — the house allocator, and nothing else changed

Add [`rusty_json_turbo-alloc`](https://crates.io/crates/rusty_json_turbo-alloc)
to your binary and declare it. That is the whole diff: **not one line of JSON
code touched.** Pinned to one core, paired, ABBA-interleaved, allocation counts
proven identical on both sides. `> 1` means the house allocator is faster.

| workload | allocations per parse | platform | rusty_alloc | ratio | window |
|---|---:|---:|---:|---:|:--:|
| twitter, DOM parse | 20,834 | 459 MB/s | **690 MB/s** | **1.53x** | 2 s |
| citm_catalog, DOM parse | 39,339 | 697 MB/s | **1013 MB/s** | **1.46x** | 2 s |
| canada, DOM parse | 56,061 | 376 MB/s | **499 MB/s** | **1.32x** | 250 ms |
| twitter, struct parse | 2,762 | 880 MB/s | **1024 MB/s** | **1.17x** | 250 ms |
| citm_catalog, struct parse | 2,544 | 1326 MB/s | **1465 MB/s** | **1.12x** | 250 ms |
| canada, struct parse | 485 | 723 MB/s | 768 MB/s | 1.07x | 2 s |
| any file, stringify | **0** | 1545 MB/s | 1476 MB/s | 1.00x | 2 s |

<sub>**The last row is the control, and it is why the others are believable.**
A stringify into a pre-sized buffer allocates *zero* times, so the allocator
cannot touch it — and it doesn't. The effect sorts with the allocation count,
which a deterministic census proves is identical under both allocators. The
`window` column matters: at 250 ms samples twitter DOM parse reads 2.0x rather
than 1.53x, so about a quarter of the short-window figure is per-process
warm-up and the longer, smaller number is the honest one; the 250 ms rows are
therefore upper bounds on their own effect. `canada, struct parse` at 1.07x is
inside the cross-binary layout band (0.976–1.070x, measured on the
zero-allocation cells) and so is **not** claimable as an allocator result.
The full oracle passes under `rusty_alloc` and under its hardened `secure`
profile: **no output byte changes**, and `secure` keeps nearly the whole win.</sub>

<sub>**Two things Table 1 is not.** It is **not** a win of this crate over
serde_json: at this milestone the JSON code is upstream's, so upstream linked
against `rusty_alloc` gets exactly the same thing, and none of it counts toward
the speed gate, which compares like-for-like allocators. And it is a **Windows**
measurement, where Rust's platform allocator is `HeapAlloc`; glibc's malloc has
tcache and fastbins and should close much of the gap, so the Linux number is an
open question rather than an extrapolation.</sub>

#### Table 2 — the upgrades that go on top

These are changes to the JSON code, and **none has landed**: this crate is
still upstream's parser. The table is the roadmap and the priority order, not a
claim. Each row ships only with a measured ledger entry and a byte-identical
gate, and any row that does not pay is reverted with the reason recorded. The
two rows marked *repriced* were re-ranked by measurement **before** being
built, which is the point of having instruments first.

| Upgrade | Targets | Mechanism | Status |
|---|---|---|:--:|
| Bulk `Value` map build | DOM parse | build the map from a sorted vector instead of inserting per entry; reserve on sequences | **promoted** — measured allocation-bound, ~1 alloc per 31 input bytes |
| Arena `Value` (additional type) | DOM parse | bump-allocated nodes, flat objects, interned keys — the shape that gives the fastest competitor its 1.5–3.6x DOM lead | planned, v1.x |
| SIMD whitespace skip | every token boundary | 8-byte SWAR, then an SSE2/AVX2 twin with the scalar loop kept as oracle | planned |
| SIMD string scan | string-heavy input | 16/32-byte twin of the existing 8-byte SWAR quote/backslash/control scan | planned |
| Escape-mask writer | stringify | per-chunk "needs escape" mask, one write per clean run | planned |
| 8-digit SWAR integer parse | number-heavy input | validate and convert eight ASCII digits at a time, per-digit tail | planned |
| Key dispatch + derive handshake | struct parse | length-bucketed match, then a field-index handshake across the serde seam, replacing a linear `memcmp` ladder per key | planned |
| Buffered reader | `from_reader` | an internal buffer reusing the slice scanners, instead of one iterator call per byte | planned |
| ASCII fast-path UTF-8 validation | `from_slice` | validate the ASCII run wide, walk only non-ASCII tails | planned |
| Sink specialisation | stringify | fold separators into adjacent writes; write integers and floats into spare capacity | *repriced* — cannot win by removing allocations (there are none); must win on write-call count |
| Correctly-rounded float parse | float-heavy input | core's Eisel-Lemire, replacing the vendored bignum path | planned, v1.x, opt-in (it changes output) |

## What is this?

A **fork**, not a wrapper and not a rewrite: the upstream tree lives at this
repository's root so `git merge upstream/master` stays a plain merge, and
every divergence is one row in
[`docs/UPSTREAM-CHANGES.md`](docs/UPSTREAM-CHANGES.md). Upstream is also the
**oracle**: it is a crate, so it links into the harness on every developer
machine and CI runner, and the correctness gate is a unit test rather than a
rig. Bricks that need nothing house-specific are offered upstream as pull
requests.

The companion fork of `serde` / `serde_core` / `serde_derive` lives at
[Remade-With-Rust/serde](https://github.com/Remade-With-Rust/serde) (branch
`turbo`). Its crate names and versions are unchanged and it is consumed only as
a `[patch.crates-io]` overlay, so every `impl Serialize` in the ecosystem keeps
working.

## The Remade With Rust ecosystem

<!-- ORG BOILERPLATE — keep identical across repos -->

**Remade With Rust** is an initiative by **[Mata Network](https://www.mata.network/)**
to rebuild essential C and C++ tools in Rust — for the memory safety, the
predictable performance, and the freedom of a permissive license. Each project
is a reimplementation, not a fork: same wire protocols and file formats, new
code you can actually depend on.

We build the core to production grade and open-source it so the community can
extend it. No copyleft. No surprises. Just the tools we rely on, made faster and
safer.

| Project | What it is |
|---|---|
| 🎬 **[remade_ffmpeg_rs](https://github.com/Remade-With-Rust/remade_ffmpeg_rs)** | **Our FFmpeg alternative.** Drop-in `ffmpeg` and `ffprobe` binaries — demux → decode → filter → encode → mux, rebuilt as composable Rust crates with **zero GPL/LGPL**. Apache-2.0. |
| 🧠 **[FFAI](https://github.com/Remade-With-Rust/FFAI)** | **Our sister project: media *for* AI.** "The AI media toolkit, remade with rust." Embedded ASR + TTS (**Mercury**), OCR (**Carmenta**) and vision-language captioning (**Argus**) behind an ffmpeg-style, swap-by-name architecture — no Python, no CUDA. MIT OR Apache-2.0. |
| 🌐 **[Mata Network](https://www.mata.network/)** | **The home page.** *"Stop sacrificing your privacy for convenience."* Sovereign, self-hostable privacy infrastructure — wallet & identity, password manager, contact manager, and a browser extension that stops information leaking as you browse. Remade With Rust is its open-source arm. |

→ All projects: **[github.com/Remade-With-Rust](https://github.com/Remade-With-Rust)**

<!-- /ORG BOILERPLATE -->

## Where `unsafe` is allowed

At M0 the library carries upstream's twelve `unsafe` sites unchanged. The
mission plan's M2 moves every one of them behind a single `-accel` island (or
replaces it with safe code the measurement says costs nothing), so that the
core can take `#![forbid(unsafe_code)]` and every kernel keeps its scalar twin
as the oracle and the fallback. Until then: no new `unsafe` anywhere, and the
workspace denies it outside this crate.

## Install

```toml
[dependencies]
# Drop-in: the library's crate name is `serde_json`, so this line alone swaps it.
serde_json = { package = "rusty_json_turbo", version = "0.1" }

# or, keeping the package name in code:
rusty_json_turbo = "0.1"      # then `use serde_json::...` as before
```

| Feature | Default | What it adds |
|---|---|---|
| `std` | yes | `io::Read`/`Write` entry points, `std::error::Error` |
| `alloc` | -- | `no_std` with a heap: `default-features = false, features = ["alloc"]` |
| `preserve_order` | -- | `Map` keeps insertion order (`indexmap`) |
| `float_roundtrip` | -- | exact-round-trip float parsing (~2x slower on floats) |
| `arbitrary_precision` | -- | `Number` keeps the original digits |
| `raw_value` | -- | `RawValue` |
| `unbounded_depth` | -- | `disable_recursion_limit` |
| `profile` | -- | house instruments (stage profiler, reach census); zero-sized when off |

Every feature has the semantics upstream documents at
[docs.rs/serde_json](https://docs.rs/serde_json); upstream's own guide applies
verbatim. MSRV 1.85.

**The library never sets `#[global_allocator]`** — deliberately. A program may
declare exactly one, so it belongs to the binary, and this crate is a drop-in
for `serde_json`, which sits in nearly every Rust dependency graph. To take
Table 1's speedup, add the seam to *your* binary:

```toml
[dependencies]
rusty_json_turbo-alloc = "0.1"
```

```rust
#[global_allocator]
static ALLOC: rusty_json_turbo_alloc::Alloc = rusty_json_turbo_alloc::Alloc;
```

## Where this sits

| Crate | Role |
|---|---|
| **[`rusty_json_turbo`](https://crates.io/crates/rusty_json_turbo)** | **← you are here** — the library: a drop-in `serde_json`, byte-identical by contract |
| [`rusty_json_turbo-alloc`](https://crates.io/crates/rusty_json_turbo-alloc) | the `rusty_alloc` seam, for the binary — Table 1's speedup in one line |
| `rusty_json_turbo-accel` | the SIMD island, arriving with the kernels (M3). Not yet published |
| `rusty_json_turbo-cli` · `-bench` | the `rjson` command line and the measurement harness. Not published: one is a deliverable, the other links the oracle |

The family shares one version, bumped together, so pinning one pins them all.

## Command line

`rjson` (never `jq`) is consumer #1 of the library and never the only one:

```sh
rjson validate file.json      # exit 0 if valid; errors carry line and column
rjson pretty   file.json      # re-emit pretty-printed
rjson minify   -              # from stdin, compact
rjson isa                     # which kernel arm this process resolved
```

## Architecture

```
.                              upstream serde_json 1.0.151, at the root so merges stay plain
├── src/  tests/  build.rs  fuzz/
├── crates/
│   ├── rusty_json_turbo-alloc/   the rusty_alloc seam (one pin)
│   ├── rusty_json_turbo-cli/     rjson (allocator declared here)
│   └── rusty_json_turbo-bench/   the ORACLE + the paired benchmark; links upstream; never published
├── corpus/                       JSONCORP data, HASHES.txt, fetch.ps1, LEDGER.md
├── docs/plans/fast_mission.md    the mission plan;  docs/UPSTREAM-CHANGES.md  the divergence map
└── tools/pinbench.ps1            pinned, High-priority runner for rjson-bench
```

## Correctness and benchmarking

```sh
cargo test -p rusty_json_turbo                 # upstream's suite against the fork
cargo test -p rusty_json_turbo-bench           # the differential oracle over JSONCORP
cargo build --release -p rusty_json_turbo-bench [--features competitors]
powershell -File tools/pinbench.ps1 -- null  --all      # the floor
powershell -File tools/pinbench.ps1 -- bench --all      # ours vs upstream
target/release/rjson-bench diff-oracle path/to/*.json   # the gate, on your own files
```

## Platform support

| Platform | Status |
|---|---|
| Linux x86_64 / aarch64 (gnu, musl) | checked in CI |
| Windows x86_64 / aarch64 | checked in CI; tested on x86_64 |
| macOS x86_64 / aarch64 | checked in CI; tested on arm64 |
| `wasm32-unknown-unknown` | checked in CI (`std` and `alloc`) |
| `no_std + alloc` (`aarch64-unknown-none` probe) | checked in CI |

## Roadmap

- [x] **M0** -- scaffold, oracle, harness, corpus, CI, admissible baseline in the ledger (2026-09-09)
- [ ] **M1** -- instruments, S4 house payloads, fuzz targets, ceiling probes, ranked worklist
  <br><sub>done: allocation census, `solo`/`census` verbs, the cross-binary paired runner, seven fuzz targets, and the house-allocator experiment above</sub>
- [ ] **M2** -- safe byte-identical bricks; core takes `forbid(unsafe_code)`
- [ ] **M3** -- the `-accel` island: SSE2/AVX2 whitespace, string, escape and ASCII kernels
- [ ] **M4** -- the serde fork earns its keep: shared identifier matcher, field-index handshake
- [ ] **M5** -- the campaign to the G2 speed gates
- [ ] **M6** -- NEON and simd128 twins
- [ ] **M7** -- consumer swap: rusty_time-api, mid, deputy, mata-master
- [ ] **M8** -- use-protection-please audit, 30 days of fuzzing, v1.0

Details, gates and decisions: [`docs/plans/fast_mission.md`](docs/plans/fast_mission.md).

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option, exactly as upstream serde_json; the `LICENSE-APACHE` and `LICENSE-MIT`
files are upstream's. Attribution and corpus provenance: [`NOTICE.md`](NOTICE.md).
No GPL/LGPL and no C anywhere in the dependency tree, enforced with
`cargo-deny`.

## About Mata Network

<!-- ORG BOILERPLATE — keep identical across repos -->

**[Mata Network](https://www.mata.network/)** builds sovereign, self-hostable
privacy infrastructure — *"stop sacrificing your privacy for convenience"*:
wallet & identity, a password manager, a contact manager, and a browser
extension that stops your information leaking as you browse.

**Remade With Rust** is our open-source home for the permissively-licensed
building blocks that work depends on — including
[remade_ffmpeg_rs](https://github.com/Remade-With-Rust/remade_ffmpeg_rs) (the
FFmpeg alternative) and [FFAI](https://github.com/Remade-With-Rust/FFAI) (the
AI media toolkit).

→ **[www.mata.network](https://www.mata.network/)**

<!-- /ORG BOILERPLATE -->
