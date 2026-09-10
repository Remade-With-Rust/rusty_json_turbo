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

**Status: the instruments are built and the first brick has landed.** What
exists today:

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
- **The instruments.** A content census that counts where a document's bytes
  actually go, a scan-only ceiling that bounds every value-construction change,
  and deterministic work counters with an A/B knob — so a change can be decided
  by counting the work it removed, on a machine too busy to trust a clock.
- **The first brick**, below.
- **The plan.** [`docs/plans/fast_mission.md`](docs/plans/fast_mission.md):
  the brick catalog, the gates, the milestones, the decisions.

| | serde_json (upstream) | **rusty_json_turbo** |
|---|---|---|
| Public API | the reference | **identical**, item for item, every feature flag |
| Output bytes, errors, float bits | the reference | **byte-identical by contract**, gated per commit |
| C / `*-sys` in the dependency tree | none | **none** |
| `unsafe` | 12 sites | 12 sites inherited; **M2 moves them to one audited island** |
| `no_std + alloc`, `wasm32` | yes | **yes** -- compiled on 9 targets, and both *tested*: the suite runs on `wasm32-wasip1`, `no_std + alloc` runs in four feature configurations, and the browser target's output bytes are checksum-matched to native |
| License | MIT OR Apache-2.0 | **MIT OR Apache-2.0** |
| Speed | the baseline | **measured before claimed** -- see the ledger |

### Performance -- measured rather than asserted

No number enters this README without a method line, and each brick's ledger row
lands before its sentence here. The campaign has just started: one brick is in,
so the honest summary is "measurably less work on the parsing path, not yet a
headline speed claim against serde_json".

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

Changes to the JSON code itself. **One has landed.** Each row ships only with a
measured ledger entry and a byte-identical gate, and any row that does not pay
is reverted with the reason recorded. The rows marked *repriced* or *promoted*
were re-ranked by measurement **before** being built, which is the whole point
of having instruments first.

| Upgrade | Targets | Mechanism | Status |
|---|---|---|:--:|
| **Whitespace off the per-byte path** | every token boundary | skip a whitespace run in one walk of the slice instead of a `Result<Option<u8>>` round trip per byte | ✅ **landed** — see below |
| **Wide whitespace scan** | pretty-printed input | eight bytes per step (SWAR), with a short-run peel and the scalar walk kept as the oracle | ✅ **landed** — see below |
| SIMD whitespace scan | pretty-printed input | an SSE2/AVX2 twin of the above, in a separate crate so the parser never writes `unsafe` | ❌ **built, then switched OFF by default** — measured against *upstream* it loses on 13 of 15 cells. A `#[target_feature]` function cannot be inlined, so reaching the island costs more inlining than the width buys. Reachable with `--features accel`; see `corpus/LEDGER.md` |
| Bulk `Value` map build | DOM parse | build the map from a sorted vector instead of inserting per entry; reserve on sequences | **promoted** — measured allocation-bound, ~1 alloc per 31 input bytes |
| Arena `Value` (additional type) | DOM parse | bump-allocated nodes, flat objects, interned keys — the shape that gives the fastest competitor its 1.5–3.6x DOM lead | planned, v1.x |
| SIMD string scan | string-heavy input | 16/32-byte twin of the existing 8-byte SWAR quote/backslash/control scan | **demoted** — mean string run is 19 bytes, 98.3% already zero-copy, and upstream is already 8-byte SWAR with `memchr2` |
| **Escape-mask writer** | stringify | per-chunk "needs escape" mask, one write per clean run | ✅ **landed** — see below |
| **8-digit integer / fraction parse** | number-heavy input | validate and convert eight ASCII digits in one 8-byte load, per-digit tail | ✅ **landed** — see below |
| Key dispatch + derive handshake | struct parse | length-bucketed match, then a field-index handshake across the serde seam, replacing a linear `memcmp` ladder per key | planned |
| **Buffered reader** | `from_reader` | an internal window reusing the slice scanners, instead of one iterator call per byte | ✅ **landed** — the reader-to-slice gap closed from 1.55x to 1.22x on `citm_catalog`, 1.28x to 1.14x on `twitter`; and you no longer want a `BufReader` |
| ASCII fast-path UTF-8 validation | `from_slice` | validate the ASCII run wide, walk only non-ASCII tails | **reframed** — 18,099 short validations per `twitter` parse, but validating once up front is *not* byte-identical, so that form is rejected |
| **Sink specialisation** | stringify | fold separators into adjacent writes; write integers and floats into spare capacity | **promoted** — measured **2.6 bytes per sink call** on `citm_catalog`, about 7.3 calls per key; it must win on call count alone, and the count says there is room |
| Correctly-rounded float parse | float-heavy input | core's Eisel-Lemire, replacing the vendored bignum path | planned, v1.x, opt-in (it changes output) |

##### Landed: the whitespace path

`citm_catalog.json` is **71.0% whitespace**, and skipping it was **36% of the
time to parse that file into a `Value`** — measured, not guessed, by running the
same document with the skippable whitespace removed. It went through
`peek()`/`discard()` one byte at a time. It now walks the run in one pass, eight
bytes per step where the run is long enough to pay for it.

Measured in one binary with one environment variable between the arms, so the
two arms cannot differ by code layout. 21 pairs, pinned, ABBA, on a quiet
machine. `> 1` means the new path is faster; the session's null-arm floor was
0.983x–1.021x.

| file | workload | before | after | ratio |
|---|---|---:|---:|---:|
| citm_catalog | scan | 1,565 MB/s | **2,105 MB/s** | **1.345x** |
| citm_catalog | struct parse | 1,171 MB/s | **1,461 MB/s** | **1.246x** |
| citm_catalog | DOM parse | 664 MB/s | **722 MB/s** | **1.095x** |
| canada | scan | 1,006 MB/s | **1,062 MB/s** | **1.053x** |
| twitter | scan | 1,472 MB/s | **1,524 MB/s** | **1.042x** |
| twitter | struct parse | 791 MB/s | **813 MB/s** | **1.036x** |
| any file | stringify | — | — | 0.997x–1.008x (control, unmoved) |

**Eight digits per step** (`canada.json` is 90.08% number bytes):

| file | workload | before | after | ratio |
|---|---|---:|---:|---:|
| canada | struct parse | 555 MB/s | **604 MB/s** | **1.083x** |
| canada | DOM parse | 296 MB/s | **309 MB/s** | **1.043x** |
| citm_catalog | struct parse | 1,361 MB/s | **1,383 MB/s** | **1.017x** |
| twitter | any workload | — | — | at the floor (1.5% number bytes) |
| stringify ×6, scan ×3 | — | — | — | 0.999x–1.010x (control, unmoved) |

**Eight bytes per escape step, on stringify** (`twitter` is 58.1% string; the
escape hit rate is 0.334%):

| file | workload | before | after | ratio |
|---|---|---:|---:|---:|
| twitter | struct stringify | 1,774 MB/s | **2,153 MB/s** | **1.208x** |
| twitter | DOM stringify | 1,837 MB/s | **2,202 MB/s** | **1.200x** |
| citm_catalog | struct stringify | 1,870 MB/s | **2,007 MB/s** | **1.068x** |
| citm_catalog | DOM stringify | 1,261 MB/s | **1,306 MB/s** | **1.028x** |
| parse cells ×2 | — | — | — | 0.995x–1.002x (control, unmoved) |

<sub>Both twitter rows won every one of 61 paired runs bar one, and a best-of-N
statistic agrees with the median on every row. **The controls are parse cells
here** — stringify cells were the control for every parse brick so far, and the
roles simply swap. `canada` is not quoted: it holds 90 string bytes in 2.25 MB,
so there is nothing for this to win.</sub>

<sub>**The counter that proves it is switched on**, which no output gate can:
scanner steps fall from 367,917 to 101,382 on `twitter` and 221,379 to 108,825
on `citm_catalog`, while the count of fragments written stays identical — same
output, less walking. **And note the opposite tuning to the whitespace scan:**
that one peels four bytes scalar first because 46% of its runs are a single
byte; this one has no peel, because a string scan runs the length of a whole
string. Same technique, opposite shape, because the census said so.</sub>

<sub>The gradient across the three files is the corpus census read back:
`canada` is 90.08% number and moves most, `citm_catalog` is 7.35% and moves a
little, `twitter` is 1.5% and does not move. Nine control cells stayed inside 1%.
The step is exact, not approximate — below a bound where eight more digits
provably cannot overflow a `u64`, the chunk takes the same branch the
byte-at-a-time loop would have taken, including the digit at which a long number
switches to the slow float path.</sub>

**Against upstream serde_json, net of instantiation bias.** The middle column
is what a naive ours-vs-upstream run reports; the right column is what our code
actually changed. They differ because two crate instantiations get laid out and
inlined differently: at a commit where our source was **byte-identical** to
upstream, the same comparison already read 0.807x on `twitter` struct stringify
and 1.004x on `citm_catalog` DOM parse. That per-cell floor is divided out
here.

| file | workload | raw ratio | **net of bias** |
|---|---|---:|---:|
| twitter | struct stringify | 1.53x | **1.23x** |
| twitter | DOM stringify | 1.43x | **1.21x** |
| citm_catalog | struct parse | 1.22x | **1.21x** |
| citm_catalog | DOM parse | 1.09x | **1.09x** |
| citm_catalog | struct stringify | 1.12x | **1.08x** |
| twitter | DOM parse | 1.04x | **1.05x** |
| canada | struct parse | 1.04x | **1.04x** |
| citm_catalog | DOM stringify | 1.14x | **1.03x** |
| canada | struct stringify | 1.03x | **1.02x** |
| canada | DOM parse | 1.01x | **1.02x** |
| twitter | struct parse | 0.99x | **0.99x** |
| canada | DOM stringify | 1.01x | **~1.00x** |

<sub>**The bias is almost entirely in the stringify column** — every parse cell's
floor sits between 0.979x and 1.004x, while three of four stringify cells sit
between 0.807x and 0.938x. The serializer is the tighter code path and far more
sensitive to layout, which is exactly why the raw stringify figures looked so
much better than the raw parse ones. Measured with `tools/biasvs.ps1`: two
binaries interleaved round by round, each normalised against its own upstream
arm so drift cancels; 9 rounds, 9 pairs each, pinned.</sub>

<sub>**The correction has its own error bar of about 5%**, because the bias
itself moves between builds of a reference that should be equivalent. Treat
anything within 5% of 1.00 as unchanged, and read these to two significant
figures at most.</sub>

<sub>**The brick figures elsewhere in this README are not affected.** Those come
from knob A/B runs — one binary, one crate instance, one environment variable
between the arms — so there is no instantiation difference to bias them. That
is why this project measures bricks that way.</sub>

<sub>**Two cells are flat and both are understood.** `twitter` struct parse is
at 0.99x: a ceiling probe made key dispatch *completely free* and bought 0.6%,
so the derive is already near-optimal and there is nothing there to win. And
`twitter` DOM parse is 1.05x against an ambition of 1.8x, because DOM parse
makes roughly one allocation per 31 input bytes — short strings dominate the
count, map nodes dominate the bytes, and about 80% of every node is unused at a
median arity of two. Closing that needs a different `Value` type, not another
brick.</sub>

<sub>**One honest caveat on the absolute MB/s above.** They were taken before
the corpus was pinned platform-independent. There was no root `.gitattributes`,
so a `core.autocrlf=true` checkout inflated `twitter.json` from 631,514 to
646,995 bytes -- 15,481 carriage returns, every one counted as whitespace --
and `citm_catalog.json` by 50,468. **No ratio in this README is affected**:
both arms of every paired run read the same file, so 1.208x is 1.208x either
way. The absolute MB/s figures are on the 2.4% larger documents and will read
slightly differently now, and the byte-percentages have been recomputed. The
manifest was regenerated and is now verified on all three CI runners; the full
account is in `corpus/LEDGER.md` under M1-D.</sub>

<sub>**A four-digit step was built on top of it twice and reverted twice**, and
the second time is the one worth reading. At the fraction call site it hits
**110,984 times out of 111,080** and removes **a third of `canada`'s `peek`
calls** — and is still **2.5% slower** (61 pairs, 51 wins, best-of-N agreeing,
two stringify controls flat). The fold has a fixed cost that does not shrink
with width: four scalar digit steps are already about as cheap as one fold, so
break-even sits above four digits. Removed work is not saved time. Full
evidence in `corpus/LEDGER.md`.</sub>

<sub>Every row above 1.04x won 21 of 21 paired runs. **`canada.json`'s 1.053x is
not a whitespace result** — that file has 33 whitespace bytes in 2.25 MB. It is
the entry point now costing one load and one test where it used to build a
`Result<Option<u8>>`, so minified documents gain too, which is where most JSON
on a wire lives. The four stringify cells never reach this code and did not
move, which is what makes the rest believable.</sub>

<sub>**Two regressions were found and fixed on the way, both by files that
cannot benefit.** Returning only an index made the caller re-load the byte it
had just examined and cost 4% on `canada`; and reaching for an eight-byte step
on a *one-byte* run — 46% of `twitter`'s whitespace runs — cost 10% on twitter
scan. Neither was visible in the output: both arms stayed byte-identical
throughout. Keeping a file in the corpus that a change must *not* help is what
caught them. Full history, counts and method line:
[`corpus/LEDGER.md`](corpus/LEDGER.md).</sub>

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

**Compiled** means `cargo check` in three feature configurations. **Tested**
means the real test suite runs there. The distinction is the point: a check
cannot catch a wrong *answer*, and the places a JSON parser gets a different
answer on a different target are real ones -- `usize` width in the recursion
guard and the reader's window arithmetic, float formatting, and byte order
anywhere a multi-byte load is used, which this crate does on the whitespace
scan, the escape scan and the eight-digit number fold.

| Platform | Status |
|---|---|
| Linux x86_64 (gnu, musl) | compiled; **full suite tested in CI** |
| Windows x86_64 | compiled; **full suite tested in CI** |
| macOS aarch64 | compiled; **full suite tested in CI**, and the per-arch census runs there |
| Linux aarch64 (gnu) | compiled; **suite, oracle, census and no_std probe run under qemu-user in CI** (correctness only -- a qemu timing is meaningless and none is taken) |
| Linux aarch64 (musl), Windows aarch64, macOS x86_64 | compiled in CI |
| `wasm32-wasip1` | **full suite tested under wasmtime**: 249 library tests, the differential oracle over the whole corpus, the soak, and every brick gate |
| `wasm32-unknown-unknown` (the browser target) | compiled, **and its output bytes are checksum-compared with native** -- see below |
| `no_std + alloc` | compiled on every target above, **and RUN** in four feature configurations, natively and on wasm |
| `no_std` with no `std` at all (`aarch64-unknown-none`) | compiled in CI |

Two of those deserve spelling out, because they are the ones most projects
claim without checking.

**The differential oracle passes on `wasm32`.** Output bytes, `Value`, error
text with line and column, float bit patterns and `StreamDeserializer` offsets
-- all identical to upstream serde_json, over 94 documents, on a **32-bit**
target.

**The browser target is checked by checksum, not by assertion.**
`wasm32-wasip1` has a platform underneath it; a browser does not. So
`crates/rusty_json_turbo-wasmdemo` embeds three corpus documents, parses and
re-serializes them inside `wasm32-unknown-unknown`, and its FNV-1a of the
output bytes must equal native's exactly -- including **2,063,469 bytes of
re-serialized `canada`**, which is 2.25 MB of floating point and the one value
family where a difference would be a silent wrong answer rather than a crash.
The parser is linked alloc-only there, and the module is asserted to need **no
host imports at all**.

**A per-arch census guards the thing no output gate can see.** The wide
scanners are 8-byte SWAR behind length guards, so an architecture where those
guards never passed would produce completely correct output at the speed of
the per-byte fallback. The census asserts they are reached, and measured, every
counter reading on `wasm32` is byte-for-byte identical to `x86_64`.

**NEON and simd128 twins are deliberately not shipped**, and the reason is
measured rather than asserted: the corpus's mean whitespace run is 7.2 bytes
with the longest at 29, so a 16-byte vector kernel has the same problem AVX2
had at 32 -- which lost on every cell but one. Plus there is no aarch64
hardware here to measure on, and this project does not ship unmeasured
optimisations. The full argument is in [`corpus/LEDGER.md`](corpus/LEDGER.md).

## Roadmap

- [x] **M0** -- scaffold, oracle, harness, corpus, CI, admissible baseline in the ledger (2026-09-09)
- [ ] **M1** -- instruments, S4 house payloads, fuzz targets, ceiling probes, ranked worklist
  <br><sub>done: allocation census, `solo`/`census` verbs, the cross-binary paired runner, seven fuzz targets, and the house-allocator experiment above</sub>
- [ ] **M2** -- safe byte-identical bricks; core takes `forbid(unsafe_code)`
  <br><sub>first brick landed: whitespace off the per-byte path</sub>
- [ ] **M3** -- the `-accel` island: SSE2/AVX2 whitespace, string, escape and ASCII kernels
- [ ] **M4** -- the serde fork earns its keep: shared identifier matcher, field-index handshake
- [ ] **M5** -- the campaign to the G2 speed gates
- [x] **M6** -- portability: the suite on `wasm32-wasip1`, `no_std` executed rather than checked, a per-arch census, and the browser target's bytes checksum-matched to native (2026-09-10)
  <br><sub>NEON and simd128 twins are a written, measured note rather than code -- the corpus's 7.2-byte mean whitespace run rules out a 16-byte kernel, and there is no aarch64 hardware to measure one on. aarch64-Linux runs under qemu-user for correctness</sub>
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
