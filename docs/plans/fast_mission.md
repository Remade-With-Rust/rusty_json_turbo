# rusty_json_turbo — Mission Plan

> **Mission:** `rusty_json_turbo` is serde_json, forked and made fast the house way: the same
> public API, the same bytes out, the same errors at the same line and column — measurably
> faster on every workload the house actually runs, with the speed earned brick by brick
> under the codec-campaign discipline and gated byte-identical against upstream on every
> commit. Alongside it, a fork of `serde` / `serde_core` / `serde_derive` carries the
> derive-codegen wins the JSON layer cannot reach alone, consumed by house apps through
> `[patch.crates-io]` and offered upstream as PRs.
>
> Reference target: [serde-rs/json](https://github.com/serde-rs/json) v1.0.151 (`afdf6fc`,
> 2026-08-07) and [serde-rs/serde](https://github.com/serde-rs/serde) v1.0.229 (`a874a1b`,
> 2026-08-24). Both MIT OR Apache-2.0. Both pure Rust.
> Part of the [Remade-With-Rust](https://github.com/remade-with-rust) family.

Status: **planning** (decisions in §9 await ratification) · Owner: tim.almond@thehouseinc.xyz · Created: 2026-09-09

---

## 0. What is different about this mission — read before §1

Every other house remake replaces a C library and measures against a C oracle built on a
Linux rig. This one replaces a **Rust** library, and that changes three things in our favour:

1. **The oracle runs everywhere.** Upstream serde_json is a crate. It links into the test
   harness of every dev machine and every CI runner under a renamed dependency
   (`serde_json_upstream = { package = "serde_json", version = "=1.0.151" }`). There is no
   rig, no golden-vector generator, no "nothing C ships" caveat. The differential gate is a
   unit test.
2. **The gate is the strongest we have ever had.** JSON parsing is exact integer and string
   work. The only float in the pipeline is `f64` parsing and printing, and upstream's bits are
   deterministic, so even those are gated `assert_eq!` on `to_bits()`. There is no tolerance,
   no SNR, no corpus BD-rate. Every brick in this campaign gates on byte equality.
3. **The API is the product.** simd-json needs `&mut [u8]`, sonic-rs needs
   `-C target-cpu=native` and its own `Value`. Both are faster than serde_json and neither is a
   drop-in. The mission is the thing nobody has shipped: **same API, same bytes, faster** —
   so that `serde_json = { package = "rusty_json_turbo" }` is the whole migration.

The cost of forking a Rust crate rather than remaking a C one is also real and is stated in
§9: we inherit ~18k lines we did not write, we track upstream releases, and we must never
let the fork drift into a second dialect.

---

## 1. Why this exists

- **JSON is on every request path in the house, by doctrine.** `building-the-new-internet`
  says oxicode for internal wire, **JSON for every public API** — browsers, third parties,
  OpenAPI. Locally, `mata-master` carries 85 `serde` and 75 `serde_json` manifests; `mid`
  parses JWT/JWS bodies and DID documents with it; `rusty_time-api` reports over it;
  `spacedb-crdt` stores JSON values; `deputy-api`/`-store`, `ffai-bench`, `rff-server` all
  speak it. A faster serde_json is a faster house, with no consumer code changed.
- **Upstream's hot paths are scalar per-byte loops.** Whitespace skipping
  (`de.rs:255-266`), integer digits (`de.rs:462-507`, with the `overflow!` check evaluated
  per digit), the escape-table writer (`ser.rs:2079-2133`), literal matching
  (`de.rs:445-460`) and the `IoRead` byte-per-iterator-call path (`read.rs:258-298`) are all
  one byte at a time. The only wide code in the crate is the 8-byte SWAR quote/backslash
  scan (`read.rs:432-481`). These loops are the polynomial/hash-class exception the codec
  skills name: **the compiler cannot vectorise them from the scalar form**, so "auto-vec
  already won" is not a reason to stop. simd-json and sonic-rs show the headroom: 2.2× on
  DOM parse of twitter on this machine, sonic-rs claiming 3–7× DOM and 1.7–2.8× struct
  parse on theirs.
- **The derive layer is where serde itself costs.** `serde_core`'s traits are zero-cost;
  the derive's output is not: a `match &str` chain per key that compiles to a length check
  plus a linear `memcmp` ladder (`serde_derive/src/de/identifier.rs:448-459`), the same
  table emitted three to five times (`visit_str`, `visit_bytes`, `visit_borrowed_*`,
  `visit_u64`), and `Content` buffering of whole subtrees for `flatten` / untagged enums
  (`serde/src/private/de.rs`, 3,501 LOC, a `Vec<(Content, Content)>` per node). Because the
  house owns both sides of the seam, a cross-crate protocol (§6.3, the field-index
  handshake) is possible here and impossible for any single-crate competitor.
- **The house skills are exactly this job.** Byte-identical oracle, counters before clocks,
  redundancy before SIMD, reach census, revert-if-not-faster, one brick per commit: the
  campaign discipline that took rusty_xml to 1.6–2.2× over libxml2 and rusty_erasure past
  Intel's assembly. §10 routes every phase to the skill that governs it.

### Prior art — stated honestly

| Project | What it is | Our position |
|---|---|---|
| **serde_json** 1.0.151 (Rust, dtolnay) | The reference and the oracle. 18.3k LOC, 12 real `unsafe` sites, pure-Rust deps (`itoa`, `memchr`, `zmij`, `serde_core`), `float_roundtrip` via a vendored 3.7k-LOC `lexical`, default float path one multiply and **not correctly rounded**. | The baseline every number is measured against and the byte-for-byte contract we keep. Every brick that needs no house crate is offered upstream. |
| **simd-json** 0.18.1 (Rust, simd-lite) | Port of simdjson: stage-1 structural bitmap + tape, `BorrowedValue`/`OwnedValue` DOM, runtime AVX2/SSE4.2/NEON/simd128 dispatch, MSRV 1.88, MIT OR Apache-2.0. Requires **`&mut [u8]`** input (copies into a padded `AlignedBuf`), "a lot of unsafe", depends on serde_json for its convenience API. | Secondary corpus baseline. On this machine it is 2.2× faster on DOM parse of twitter and **slower than serde_json on struct parse** of canada and citm (tape → serde double pass). If it beats us on any struct cell at v1.0, that is a stop-and-explain finding. |
| **sonic-rs** 0.5.9 (Rust, ByteDance/cloudwego) | serde_json's de/ser code forked with SIMD inserted at long strings, float digits, whitespace (64-byte cached bitmap) and on-demand `get`; bumpalo-arena DOM with flat-Vec objects; **compile-time-only ISA selection** (README mandates `-C target-cpu=native`), Apache-2.0 only, `ahash`/`bumpalo`/`bytes`/`faststr`/`simdutf8` deps. | The closest design to ours and the honest upper bound on the same architecture. We adopt its whitespace bitmap and long-string ideas with runtime dispatch and no new dependencies; we do not adopt its DOM (a different `Value` is an API break, §3 NEVER). Benchmarked at the crossover, never claimed beaten on its home turf (its arena DOM) until we have one (v1.x). |
| **serde_json_core**, **json-rust**, **tinyjson** | no_std/heapless or DOM-only crates. | Not corpus arms. Design inputs only for the `no_std` floor. |

We must beat upstream serde_json on every cell of the corpus and match or beat simd-json on
every struct cell — otherwise the honest answer to "why not upstream?" is "no reason," and
this repo has no mission.

---

## 2. The one-line test, applied

> Could this ship as-is to a user who assumes their data is theirs alone, onto a machine
> you do not own, with no C toolchain anywhere in the build?

| Concern | Answer |
|---|---|
| C toolchain | **None.** Every runtime dependency is pure Rust and stays so: `serde_core`, `itoa` (MIT/Apache), `memchr` (Unlicense/MIT, `default-features = false`), `zmij` (MIT only — allowed by the deny.toml allow-list), optional `indexmap`/`hashbrown`. The `-accel` crate uses `core::arch` intrinsics behind runtime detection; no `cc`, no `nasm`, no `*-sys`. simd-json and sonic-rs are **bench-only dev-dependencies behind a non-default feature**; `serde_stacker → psm` (C, upstream's `unbounded_depth` test dep) is dropped from the tree. |
| Data | The library holds no state — bytes in, values out. No persistence, so the per-entry law applies to callers, not here. |
| Identity | N/A for a library. `mid-verify` becomes a consumer (M7), not a dependency. |
| Machine you don't own | The library is `no_std + alloc` under `--no-default-features --features alloc` (upstream already is; we keep it), builds on `wasm32-unknown-unknown`, and never declares `#[global_allocator]`. Untrusted bytes are the whole input: depth limit kept (`remaining_depth`, 128), no panic on any byte slice (fuzz-proven from M1), typed errors with stable kinds. |
| Test rigs are exempt | No rig exists. The oracle is upstream serde_json linked into the test harness on every machine. Competitor arms run only in `rusty_json_turbo-bench` and ship in nothing. |

---

## 3. Scope — the serde_json parity map

### v1.0 MUST

- **Public API identical to serde_json 1.0.151**, item for item: `from_str` / `from_slice`
  / `from_reader` / `from_value`, `to_string{,_pretty}` / `to_vec{,_pretty}` /
  `to_writer{,_pretty}` / `to_value`, `Value` (the same public enum: `Null`, `Bool`,
  `Number`, `String(String)`, `Array(Vec<Value>)`, `Object(Map)`), `Map`, `Number`,
  `RawValue`, `StreamDeserializer`, `Serializer` / `Deserializer` / `Formatter` /
  `PrettyFormatter` / `CompactFormatter`, `Error` with `Category`, `line()`, `column()`,
  `io_error_kind()`, the `json!` macro. A house crate swaps with one line:
  `serde_json = { package = "rusty_json_turbo", version = "…" }`.
- **Every feature flag with identical semantics**: `std` (default), `alloc`,
  `preserve_order`, `float_roundtrip`, `arbitrary_precision`, `raw_value`,
  `unbounded_depth`; plus house additions `profile` (ZST when off) and `rusty-alloc`
  (**off by default**, the org's `rusty_alloc_default` seam, for deliverables only).
- **The byte-identical contract**, gated in CI on every commit (§7.1): for every input in
  the corpus and the fuzz corpus, fork and upstream produce (a) identical `to_string` /
  `to_vec` / pretty bytes, (b) equal `Value`s, (c) identical `Error` `Display` text
  including line and column and identical `Category`, (d) identical `f64`/`f32` bit patterns
  from every number path — **including the default `f64_from_parts` path, whose
  not-correctly-rounded bits are part of the contract** (a correctly-rounded default would
  be a semantic change and is a separately gated v1.x feature, §9.4).
- **The serde fork passes serde's own suite**: `test_suite` (16k LOC) and all 118 trybuild
  ui cases, plus upstream's Miri job, with derive output that compiles to **no more code**
  than upstream on a 100-struct fixture (`cargo llvm-lines`) and no slower (`cargo build`
  wall, N ≥ 5).
- **Performance vs upstream on JSONCORP** (§7.4 G2), every number with its method line.
- **`forbid(unsafe_code)` in the core**, one `unsafe` island (`-accel`), every kernel with a
  scalar twin as oracle and a reach census proving the shipping path hits it.
- **x86-64 kernels**: SSE2 (the compile baseline — no dispatch needed) and AVX2 behind
  cached runtime detection, for whitespace skip, string scan, escape-needed scan and ASCII
  fast-path UTF-8 validation. Scalar fallback is the SWAR/scalar upstream code, kept
  verbatim.
- **Untrusted-input posture** (rusty_zstd §10 / rusty_xml §3.4 shape): no `unwrap` on a
  parser path (`clippy::unwrap_used` denied), fuzz target per public entry point from M1,
  size bounds derived from input length before any allocation.

### v1.x SHOULD

- aarch64 NEON and wasm32 SIMD128 twins with per-arch census, or a written note why not.
- `float_roundtrip` via core's Eisel-Lemire (`str::parse::<f64>`) replacing the vendored
  `lexical` (−3.7k LOC), gated bit-identical over `tests/lexical` and a 10M-sample fuzz.
- A **`float_correct` feature**: correctly-rounded default float parse (a semantic change,
  opt-in, gated against core's parser).
- `RawValue`/lazy-span driven **untagged and `flatten` deserialization** without `Content`
  buffering (needs the private-newtype hook the `raw_value` feature already uses, extended
  through the forked derive) — the largest structural win on real house payloads (`mid`'s
  JWT bodies use untagged enums).
- An **arena `Value`** (`rusty_json_turbo::dom::Value`, sonic-rs-shaped: bump-allocated,
  flat objects, interned keys) as an *additional* type, never a replacement for
  `serde_json::Value`.
- On-demand `get_path` / `skip` over a structural bitmap (JSONSki-style) for the "read two
  fields of a 200 KB document" shape.
- In-house SIMD UTF-8 validation (simdutf8-class) as an `-accel` kernel.

### NEVER

- **Change a single output byte by default.** No `non_trailing_zero`, no key reordering,
  no "helpfully" canonical output. A speed change that alters output is a bug with good
  timing.
- A second serialization vocabulary. No new `Serialize`/`Deserialize` traits, no
  `rusty_serde` crate name: the trait layer is the ecosystem's contract and the fork of it
  is consumed only via `[patch.crates-io]` (§9.2).
- A `&mut [u8]` input contract, caller-supplied padding, or a `Value` that is not
  `serde_json::Value` on the default path.
- Compile-time-only ISA selection. Portable binaries are SSE2; anything above is runtime
  detected with a scalar fallback that CI compiles and tests (`--no-default-features`).
- Nightly, `cc`, `nasm`, `*-sys`, copyleft, or a dependency added without a stated reason
  in the PR.
- Threading inside `from_*` / `to_*`. Parallelism is the caller's (a batch of documents is
  embarrassingly parallel already; one document is not).

---

## 4. Architecture — the scaffold

Per the house scaffold (`building-the-new-internet` §2). Names final unless the org objects.

```
rusty_json_turbo/                      # git: history-preserving clone of serde-rs/json (§9.2)
├── Cargo.toml                         # workspace; pins live here once; unsafe_code = "deny"
├── docs/plans/fast_mission.md         # this file
├── docs/UPSTREAM-CHANGES.md           # every divergence from upstream, with its brick and ledger row
├── NOTICE.md                          # upstream attribution (dtolnay et al.), licences unchanged
├── crates/
│   ├── rusty_json_turbo/              # LIBRARY. The serde_json fork, API-identical, PUBLISHED.
│   │                                  # #![forbid(unsafe_code)] — upstream's 12 unsafe sites are
│   │                                  # moved behind -accel or replaced with safe code (M2 census).
│   │                                  # no_std + alloc floor kept. Zero new dependencies.
│   ├── rusty_json_turbo-accel/        # LIBRARY. The unsafe island: SSE2/AVX2 (v1.0), NEON/simd128
│   │                                  # (v1.x) kernels + the scalar twins they are gated against.
│   │                                  # Every block: // SAFETY:, *_matches_scalar test, census counter.
│   ├── rusty_json_turbo-cli/          # DELIVERABLE `rjson`. Allocator declared HERE (seam).
│   ├── rusty_json_turbo-bench/        # Harness, never published: upstream + simd-json + sonic-rs arms
│   │                                  # behind features; the compliant paired A/B; ledger writer.
│   └── rusty_json_turbo-alloc/        # The rusty_alloc seam — one crate, one pin (=1.1.6).
├── corpus/                            # §7: JSONCORP inputs (or fetch script + hashes), LEDGER.md
├── fuzz/                              # cargo-fuzz: from_str, from_slice, from_reader, Value round-trip,
│                                      # RawValue, StreamDeserializer, to_string(Value)
└── tools/                             # pinvs.ps1 port, asm census scripts, llvm-lines fixture

../serde/                              # SEPARATE REPO: remade-with-rust/serde, branch `turbo`,
                                       # crates serde, serde_core, serde_derive unchanged in name,
                                       # consumed by [patch.crates-io] in this workspace and in apps.
```

Rules the layout enforces: `#[global_allocator]` only in `rjson`; the library is
`forbid(unsafe_code)` and `-accel` is the only crate that lifts it; every capability is an op
callable by CLI, test and agent; the bench crate is the only place a competitor crate exists.

Design notes the sibling campaigns paid for, applied here:

- **Keep upstream's code as the scalar twin, verbatim.** `parse_whitespace`, the SWAR
  `skip_to_escape`, the `ESCAPE`-table writer and the digit loops stay in the tree as
  `*_scalar` oracles and as the `--no-default-features` fallback. A kernel earns its place by
  measurement; the oracle earns its place by existing.
- **Dispatch at the surface, not per token.** The `-accel` arm is resolved once per
  `Deserializer`/`Serializer` construction (`is_x86_feature_detected!` cached in a
  `OnceLock`-free static), and the whole scan loop lives inside one `#[target_feature]`
  function. The rusty_dds dispatch-site lesson swung the same kernel −47.8% ↔ +38.7%.
- **The bounds-check tax is expected to be ~0** (measured flat on h264, rav1e, dds). The
  safe core is not the performance risk; per-byte call structure is. We do not lift
  `forbid(unsafe)` in core for speed; the bounds-check-ceiling probe (M1) prices it once.
- **`serde_json_upstream` is a hard dependency of the test and bench crates**, pinned
  `=1.0.151`, renamed via `package =`. Bumping the pin is a deliberate commit that re-runs the
  whole differential gate.
- **`RJT_ISA=scalar|sse2|avx2`** is the one-env-var whole-surface ceiling probe
  (codec-vectorize-kernel Step 0): forcing every kernel down a rung prices all remaining
  kernel-width work at once.

---

## 5. Ops before buttons — the API surface

The public API is serde_json's, verbatim (§3). House additions, all `#[doc(hidden)]`-free and
semver-stable from 1.0:

- `rusty_json_turbo::isa() -> Isa` — which kernel arm this process resolved (`Scalar`,
  `Sse2`, `Avx2`, `Neon`, `Simd128`).
- `rusty_json_turbo::census() -> Census` (feature `profile`) — bytes scanned down each
  kernel path, allocations per parse, kernel calls; the reach-census counters that G6 reads.
- `rusty_json_turbo::profile::Stages` (feature `profile`) — the feature-gated stage
  profiler (whitespace / string / number / structure / value-build / write-escape /
  write-number), ZST when the feature is off.

CLI `rjson` (never shadows `jq`; consumer #1 and never the only one):

`rjson validate <file>` · `rjson pretty` / `rjson minify` (`to_writer_pretty` /
`to_writer`, byte-gated against upstream) · `rjson bench --cell <corpus>,<column> [--arm
ours|upstream|simd-json|sonic-rs] [--pairs N]` (prints the §7.1 method line with every
number) · `rjson census <file>` (kernel reach + allocs, a first-class user-visible op) ·
`rjson diff-oracle <file|dir>` (the differential gate as a command: bytes, `Value`, error
text, float bits) · `rjson isa`.

---

## 6. The build-function inventory — every hot path, every target

Line numbers refer to upstream serde_json `afdf6fc` and serde `a874a1b`; they are the sites
the bricks in §8 attach to.

### 6.0 Portable core (all targets, including wasm — `rusty_json_turbo`)

| Stage | Upstream site | Today | Brick (§8) |
|---|---|---|---|
| whitespace skip | `de.rs:255-266` `parse_whitespace` | scalar `peek`/`match`/`eat_char` per byte, called 2–3× per element | B1 scalar fast path; B1s SIMD bitmap |
| byte source | `read.rs:545-571` `SliceRead::{next,peek,discard}` | index + bounds check, `Result<Option<u8>>` | none (fine after inlining); census only |
| byte source | `read.rs:258-298` `IoRead` | one `io::Bytes` iterator call per byte, eager line/col | B7 buffered `IoRead` |
| `from_slice` UTF-8 | `read.rs:868-870` `as_str` | `str::from_utf8` **per string** | B12 ASCII fast path / prevalidated entry |
| dispatch | `de.rs:1393-1467` `deserialize_any` | inlined peek + branch | none |
| framing | `de.rs:1933-1969`, `1986-2034` `SeqAccess`/`MapAccess` | `first` flag, comma/bracket checks | folded into B1 |
| map key | `de.rs:2215-2225` `MapKey::deserialize_any` | `parse_str` → `visit_borrowed_str` → derive `match &str` | B6 key dispatch; B6h field-index handshake (needs the serde fork) |
| strings | `read.rs:494-538` `parse_str_bytes`, `432-481` `skip_to_escape` (SWAR u64) | zero-copy when no escapes; 8-byte scan | B2 16/32-byte twin |
| escapes | `read.rs:874-1021` | `#[cold]` decode, HEX tables | none |
| literals | `de.rs:445-460` `parse_ident` | per-byte compare | B9 4-byte compare |
| integers | `de.rs:462-507` `parse_integer`, `overflow!` per digit | per-digit | B4 8-digit SWAR |
| decimals/exponent | `de.rs:530-621` | per-digit | B4 |
| float (default) | `de.rs:639-672` `f64_from_parts`, `POW10` | one multiply, not correctly rounded | **frozen** (contract); v1.x `float_correct` |
| float (`float_roundtrip`) | `de.rs:624-636` → `lexical/*` (3,707 LOC) | Clinger → 80-bit → bignum | v1.x B8 core Eisel-Lemire |
| skip value | `de.rs:1102-1215` `ignore_value` | iterative with scratch stack | none |
| DOM build | `value/de.rs:22-150` | `String::from` per key/value, `Vec::new` + push, `Map::insert` per entry into `BTreeMap` | B10 bulk map build + reserve |
| DOM map | `map.rs:29-60` | `BTreeMap` (`IndexMap` under `preserve_order`) | none — order is contract |
| raw value | `read.rs:637-655`, `733-747`, `379-397` | zero-copy on Slice/Str; per-byte push on Io | B13 (falls out of B7) |
| ser entry | `ser.rs:2213-2255` `to_vec`/`to_string` | `Vec::with_capacity(128)`, doubling | B5 sink specialisation |
| ser structure | `ser.rs:372`, `620-715`, `1842-1910` | ≥3 `write_all` per field (`,` key `:` value) | B15 separator folding (with B5) |
| ser strings | `ser.rs:2079-2133`, `ESCAPE` `2147-2165` | per-byte table lookup, run flush | B3 escape-mask writer |
| ser ints | `ser.rs:1600-1607` | `itoa::Buffer` then `write_all` | B5 (write into spare capacity) |
| ser floats | `ser.rs:1691-1723` `zmij` | Schubfach, already at parity with sonic-rs on canada | none |
| pretty | `ser.rs:1945-2067` | one `write_all` per indent level | minor, with B5 |
| recursion guard | `de.rs:1372-1387` | `u8` dec/inc | none |

### 6.1 x86-64 (`rusty_json_turbo-accel`)

| Kernel | Implementation |
|---|---|
| `ws_skip_sse2` / `_avx2` | 16/32-byte compare against `{0x20,0x0A,0x0D,0x09}` → movemask → `tzcnt`; sonic-rs-style 64-byte bitmap cached across the 2–3 calls per element |
| `str_scan_sse2` / `_avx2` | `pcmpeqb` for `"` and `\`, unsigned-less-than `0x20` for controls, OR, movemask, `tzcnt`; twin of the u64 SWAR `skip_to_escape` |
| `escape_mask_sse2` / `_avx2` | for the writer: mask of bytes needing escape (`"`, `\`, `< 0x20`); emit whole clean runs with one `extend_from_slice` |
| `ascii_run_sse2` / `_avx2` | longest all-ASCII prefix (movemask of the high bit) so UTF-8 validation only walks non-ASCII tails |
| dispatch | resolved once per (de)serializer; `RJT_ISA` env override for the ceiling probe; SSE2 needs no detection on x86-64 |

### 6.2 aarch64 (v1.x)

| Kernel | Implementation |
|---|---|
| `ws_skip_neon`, `str_scan_neon`, `escape_mask_neon`, `ascii_run_neon` | 128-bit lanes, `vceqq_u8` + narrowing shift for the movemask equivalent; cross-tested under qemu-aarch64 in CI as rusty_erasure does |

### 6.3 The serde fork (`remade-with-rust/serde`, branch `turbo`)

| Site | Today | Brick |
|---|---|---|
| `serde_derive/src/de/identifier.rs:405-473` | four parallel match tables (`visit_u64`, `visit_str`, `visit_bytes`, `visit_borrowed_*`), `match &str` = length check + linear `memcmp` chain | B6d one shared matcher, length-bucketed, emitted once |
| `serde_derive/src/de/struct_.rs:199-419` | `visit_map` with one `Option<T>` per field, `next_value::<IgnoredAny>` for unknown | B6h field-index handshake: `deserialize_struct` hands the deserializer `FIELDS`; the fork's `Deserializer::deserialize_struct` (JSON side) pre-builds a per-`FIELDS`-address lookup and calls a new hidden `Visitor::__visit_field_index(u32)` for hits, `visit_str` for misses (aliases, unknown fields) — semantics identical, one hash instead of a memcmp ladder per key. Lands only if the M1 ceiling probe (stub the match with an index lookup) clears the floor |
| `serde/src/private/de.rs` (`Content`, 3,501 LOC) | whole-subtree buffering for `flatten` / untagged / adjacently tagged | v1.x: lazy span through the raw-value hook |
| `serde_derive` `deserialize_in_place` (non-default feature) | slot reuse for `Vec<Struct>` | B14 measure; enable in the fork if it pays |
| `#[inline]` policy (`serde_core` ~90 sites, derive `struct_.rs:87,182`) | forwarders only | audit with `cargo llvm-lines`; the eight-inlined-fast-paths lesson says do not add |

Everything in the serde fork is gated by upstream's own `test_suite`, its 118 ui tests, its
Miri job, and by G9's code-size and compile-time bars. The fork is a `[patch.crates-io]`
overlay: crate names and versions unchanged (`1.0.229+turbo.N`), so any crate in the graph
that implements `Serialize`/`Deserialize` keeps working.

### 6.4 Cross-cutting build gates (every PR, per workflow doctrine)

```sh
cargo check --target x86_64-unknown-linux-gnu \
            --target x86_64-unknown-linux-musl \
            --target aarch64-unknown-linux-musl \
            --target x86_64-pc-windows-msvc \
            --target aarch64-pc-windows-msvc \
            --target x86_64-apple-darwin \
            --target aarch64-apple-darwin \
            --target wasm32-unknown-unknown
cargo test --no-default-features --features alloc     # the scalar fallback must compile and pass
cargo test --features profile                          # the instruments must not rot
```

- `unsafe_code = "deny"` workspace-wide; lifted only in `rusty_json_turbo-accel`, each block
  with `// SAFETY:` and `clippy::undocumented_unsafe_blocks = "deny"`.
- The differential oracle test (`tests/oracle.rs`) runs on every target that runs tests.
- Miri on the core's suite and on the accel twins (`-Zmiri-strict-provenance`); cargo-fuzz
  targets from M1; `cargo deny` (license allow-list incl. MIT-only `zmij` and Unlicense
  `memchr`; bans `*-sys`); `cargo audit`; `cargo vet` (FFAI already vets `serde_derive`, so
  the fork must carry its own audit entries); Deputy owns the lockfile from commit one.
- `scripts/check-ascii-rs.sh`; README copies diffed; `use-protection-please` audit
  (critical-path tier — it eats untrusted bytes) before v1.0.
- Edition 2021 / MSRV 1.85 (§9.5) checked in CI; `[profile.release] lto = "thin"`,
  `codegen-units = 1`; `overflow-checks` decided by measurement and recorded in the ledger.

---

## 7. The corpus — performance vs serde_json

**JSONCORP v1.** Upstream serde_json is the primary baseline and the oracle; simd-json and
sonic-rs are secondary. No performance claim leaves this repo — README included — unless it
is in `corpus/LEDGER.md` with the run that produced it.

### 7.1 Method

- **Conformance before any speed number, and counts before times.** The gate is byte
  identity against the in-process oracle: `to_string` bytes, `Value` equality, `Error`
  display text with line/column and `Category`, `f64::to_bits` from every number path. Runs
  as a test on every commit over every corpus file, the fuzz corpus, upstream's `tests/`,
  `data/jsonchecker/*` and `data/roundtrip/*` from json-benchmark, and JSONTestSuite.
- **The baseline arm is upstream's own harness shape** (json-benchmark: DOM parse / DOM
  stringify / struct parse / struct stringify, `to_writer` into a pre-sized `Vec` so buffer
  growth is excluded, `from_str` with the UTF-8 check inside the timed region) — the tooling
  its authors trust, ported into `rusty_json_turbo-bench` so both arms share one loop. Where
  our fork changes what the timed region contains (B5 moves growth into the measured path),
  the ledger names both variants.
- **The measurement bar applies in full:** pinned to one P-core (not core 0 — this box is a
  hybrid i7-14650HX, verify the core map before trusting a pin), High priority, timed with a
  real clock and CPU time as the `cpu/wall` validity check only (Windows `TotalProcessorTime`
  is 15.625 ms tick accounting), arms ABBA-interleaved with the **leading arm alternated**,
  N ≥ 20 pairs with win-rate + z-score (N ≥ 31 for any cross-implementation claim), a **null
  arm** (upstream-vs-upstream) establishing the floor every claim must clear, re-run per
  session, arm durations matched (≥ ~15 s per arm), and every published number carries its
  method line. Sub-1% bricks are judged by deterministic counters (bytes down each path,
  allocations, kernel calls, guard branches in the `.s`) with the clock as confirmation, and
  batched behind one switch so the batch carries the timing verdict.
- **Work-count parity per pair:** bytes parsed, values produced, bytes written — printed and
  asserted equal for both arms. Divergent counts void the pair.
- **Every guard covers every arm:** allocator (the bench runs both arms under the same
  allocator and reports as-shipped separately, the rusty_xml two-numbers rule), features
  (`float_roundtrip` on or off in both), ISA (`RJT_ISA` logged), binary freshness (marker +
  mtime, stale-binary checklist).
- **The reach census is a standing corpus column:** % of scanned bytes through each kernel
  per arm per file. Anything under 100% on the shipped path is a defect, not a statistic.
- **Ceiling probes before bricks:** stub the work (whitespace skip → `index += n` on a
  precomputed table; key match → index lookup; `Value` build → discard; escape check →
  `extend_from_slice`) and bound the prize before building. Grep every branch that reads a
  stubbed value first — a stub that steers control flow measures a different program.
- **Real content, not smooth fixtures:** the house-shaped payloads (S4) are real captured
  messages, and coverage counters prove each fixture actually enters the path a brick
  changes (the `m4=0` lesson: a byte-identity gate over content that never reaches the
  branch proves nothing).

### 7.2 Scenarios

| # | Scenario | Config | What it stresses |
|---|---|---|---|
| S1 | **twitter.json** (632 KB) | DOM + struct, parse + stringify | strings, Unicode escapes, wide structs (~40-field `User`) — the headline cell |
| S2 | citm_catalog.json (1.7 MB) | DOM + struct, parse + stringify | objects, integers, repeated keys |
| S3 | canada.json (2.2 MB) | DOM + struct, parse + stringify | floats (parse frozen; print already at parity) — the honest "no win expected" row |
| S4 | **House payloads** | 1–25 KB: mid JWT/JWS bodies + DID docs, `rusty_time-api` reports, spacedb CRDT deltas, deputy receipts, FFAI ledger rows; 10k-message batches | the regime house services live in: per-message latency, allocs per parse, untagged enums, `flatten` |
| S5 | NDJSON stream | 100 MB of log lines through `StreamDeserializer` over `from_reader` | `IoRead` (B7), `RawValue` on Io (B13) |
| S6 | Pathological | 100-deep nesting, 1 MB single string with escapes every 7 bytes, 1 MB of `\uXXXX`, 20-digit integers, 400-digit floats | cold paths stay correct and do not regress; error line/col identity |
| S7 | Conformance | JSONTestSuite (y_/n_/i_), nativejson-benchmark `jsonchecker` + `roundtrip`, upstream `tests/`, `tests/lexical` | correctness only; never timed |
| S8 | Size sweep | S1-shaped synthetic from 1 KB to 64 MB | L2/L3 crossing — gates any layout work (codec-cache-tiles: not cache-bound ⇒ do not build) |
| S9 | Derive fixture | 100 generated structs, 5–60 fields, aliases, `flatten`, untagged | derive code size (`cargo llvm-lines`), compile wall, struct parse with B6/B6h |
| S10 | Batch parallel (v1.x) | S4 × N documents, caller-side rayon | the threads-beat-SIMD law, stated as the caller's lever, never ours |

### 7.3 Metrics (per cell, per arm)

- **Throughput:** MB/s per column (input bytes for parse, output bytes for stringify), min
  and median over pairs, ratio vs upstream with win-rate + z.
- **Determinism:** kernel-reach census %, allocations per parse (hot-path target for struct
  parse of S4: 0 beyond the output value), bytes through each ISA arm, guard-branch count per
  hot function from the release `.s`.
- **Footprint:** `rjson` binary size, core dep count (target: upstream's four, no additions),
  RSS on S5, `cargo llvm-lines` of the S9 fixture, `cargo build` wall of S9.
- **Latency (S4):** ns per message, P50/P99 over 10k messages, both arms same allocator.

### 7.4 Release gates (v1.0 ships when JSONCORP says)

| Gate | Bar | Status |
|---|---|---|
| G1 conformance | Byte-identical to upstream serde_json 1.0.151 on every S1–S7 input and the fuzz corpus: bytes, `Value`, error text + line/col + category, float bits; upstream's own `tests/` green against the fork; JSONTestSuite verdicts identical to upstream's | not started |
| G2 speed vs upstream | Beat upstream on **every** cell of S1, S2, S4, S5 (win-rate ≥ 19/20, z > 2, above the null-arm floor); headline cells S1 DOM parse ≥ **1.8×**, S1 struct parse ≥ **1.4×**, S1 struct stringify ≥ **1.3×**, S4 struct parse ≥ **1.4×** with allocs per parse ≤ upstream's; S3 parse ≥ 1.0× (no regression; no win claimed); no cell anywhere below 0.97× | not started |
| G3 vs prior art | Match or beat simd-json on every struct cell; publish the honest table vs simd-json and sonic-rs with their caveats (mutable input, `target-cpu=native`, own DOM) and ours; the sonic-rs DOM comparison is reported as an upper bound, not a loss, until the arena `Value` lands | not started |
| G4 safety | Fuzzers clean (7 targets), Miri clean on core + twins, no panic from any public API on any byte slice, `forbid(unsafe)` in core intact, every accel block SAFETY-reviewed, `cargo geiger` count ≤ upstream's 12 sites on the default path | not started |
| G5 portability | 8-target check matrix green; `--no-default-features --features alloc` tested; wasm32 test under wasmtime; NEON + simd128 kernels present with per-arch census, or a written note why not | not started |
| G6 reachability | Census = 100% of scanned/written bytes through the intended kernel on every arch shipped, on S1, S2, S4, S5 | not started |
| G7 claims hygiene | README claims ⊆ LEDGER.md, each with its method line; every withdrawn or corrected claim kept in the README's `<sub>` block | not started |
| G8 consumer swap | ≥ 3 house consumers swapped (`rusty_time-api`, `mid-verify`/`mid-signin`, `deputy-api`) with their existing test suites green and their JSON output byte-identical to before; `mata-master` builds with `[patch.crates-io]` | not started |
| G9 derive parity | serde `test_suite` + 118 ui tests + Miri green on the fork; S9 `cargo llvm-lines` ≤ upstream and `cargo build` wall ≤ upstream (N ≥ 5, paired); every derive brick byte-identical on S1/S2/S4/S9 | not started |

`corpus/LEDGER.md` records every run: date, commit, cell set, arms, method line, counts,
numbers, verdict, and for reverts **which kind** (measured worse vs inside the noise). Wins
may be cited; anything not in the ledger does not exist.

### 7.5 The starting point (2026-09-09, INADMISSIBLE — recorded so M0 has something to beat)

json-benchmark, min of 256 trials, **unpinned**, Windows system allocator, `rustc 1.98.0`,
LLVM 22.1.8, i7-14650HX. Run-to-run spread ~5–10%; anything under 10% here is noise. MB/s as
DOM parse / DOM stringify / struct parse / struct stringify:

| file | serde_json `afdf6fc` | simd-json 0.18.1 |
|---|---|---|
| canada | 360 / 970 / 850 / 680 | 360 / 730 / 680 / – |
| citm_catalog | 710 / 1240 / 1510 / 1630 | 860 / 1120 / 1410 / – |
| twitter | 480 / 1950 / 1240 / 2020 | 1070 / 1570 / 1280 / – |

`-C target-cpu=native` changed nothing for either arm (serde_json has no ISA-dependent code;
simd-json runtime-dispatches). Upstream's weakest cells are DOM parse of canada (per-number
`Value` allocation + `BTreeMap` + float parse) and twitter (string allocation), where
simd-json is 2.2× ahead; on struct parse upstream already beats simd-json on canada and
citm. These numbers are the M0 exit's job to re-take under the compliant harness.

---

## 8. The brick catalog — ranked, each with its gate

Order follows `codec-optimize`: redundancy and plumbing first (safe Rust, byte-identical),
SIMD only where the profiler still points and auto-vectorisation demonstrably cannot, the
derive protocol last because it spans two repos. Every brick: one commit, its counter and
its clock in the message, reverted if flat, its ceiling probe run first.

| # | Brick | Site (upstream) | Skill | Gate | Ceiling probe |
|---|---|---|---|---|---|
| B1 | Whitespace fast path: test "next byte is not ws" first, then an 8-byte SWAR membership test; branchless | `de.rs:255` | codec-eliminate-redundancy | token positions identical (errors derive line/col lazily from `index`) | replace with a precomputed skip table on S1; bound = share of `parse_whitespace` |
| B1s | SIMD whitespace bitmap (64-byte cached, SSE2/AVX2 twins) | `-accel` | codec-vectorize-kernel | same + `*_matches_scalar` over all 256 byte values × positions | as B1 |
| B2 | 16/32-byte string scan twin of the SWAR `skip_to_escape` (handle the `index += 1` pre-step uniformly) | `read.rs:432-481` | codec-vectorize-kernel | returned slice / scratch bytes and error codes identical for all 256 bytes and every escape position | `RJT_ISA` rung A/B once B1s exists |
| B3 | Escape-mask writer: SWAR/SSE2 "needs escape" mask per chunk, one `extend_from_slice` per clean run, 6-byte `\u00XX` from a static table | `ser.rs:2079-2133` | codec-eliminate-redundancy → vectorize | output identical over all 256 bytes, all escape positions, S1/S2/S4 | stub the per-byte check with `extend_from_slice` on S1 stringify |
| B4 | 8-digit SWAR integer/fraction parse (one ASCII-digit mask, three multiplies), per-digit tail; `overflow!` semantics preserved (fall into `parse_long_integer` at the same digit) | `de.rs:462-507`, `530-565` | codec-eliminate-redundancy | identical `U64`/`I64`/`F64` variant and value, identical `InvalidNumber` positions (leading zero, `-`, `1.`) | S2/S3 parse with the digit loop stubbed |
| B5 | `to_vec` sink specialisation: write into `Vec` spare capacity with one reserve per token; `itoa`/`zmij` buffers written in place; size memo for the pre-size | `ser.rs:2213`, all `write_all` sites | codec-memory-copies | output bytes identical | count `write_all` calls per S1 stringify (counter), then clock |
| B6 | Key dispatch on the JSON side: length-bucketed compare before the derive's `visit_str` | `de.rs:2215` | codec-eliminate-redundancy | same visitor outcome for every name/alias/unknown | stub key matching with an index lookup on S1 struct parse — this probe also decides B6h |
| B6d | Derive: one shared identifier matcher (length switch, then compare) emitted once and used by `visit_str` / `visit_bytes` / `visit_borrowed_*` | `identifier.rs:405-473` | rusty-blazing-fast (generic Rust) | serde `test_suite` + ui; S9 `llvm-lines` ≤ upstream; byte-identical S1/S4 | S9 code-size delta |
| B6h | Field-index handshake across the seam (§6.3): per-`FIELDS` lookup built once per `deserialize_struct`, hidden `__visit_field_index`, `visit_str` fallback for aliases/unknown | `de.rs:2215` + `struct_.rs:199-419` | codec-eliminate-redundancy ("carry the producer's fact") | identical `__Field` for every key incl. aliases; `unknown_field` text and `FIELDS` order unchanged; both suites green | the B6 probe; lands only if it clears the floor on S1 AND S4 |
| B7 | Buffered `IoRead` (8–64 KB) reusing the `SliceRead` scanners; `\n` counted lazily on error | `read.rs:149-406` | codec-memory-copies | identical values, error line/col/`byte_offset`, `StreamDeserializer` offsets (`tests/stream.rs`), `raw_value` bytes | S5 with `from_reader` vs `from_slice` of the same bytes = the ceiling |
| B9 | `parse_ident` 4-byte slice compare | `de.rs:445` | codec-eliminate-redundancy | identical `ExpectedSomeIdent`/EOF errors on truncated literals | counter only (sub-1%) |
| B10 | `Value` DOM: build `Map` from a sorted `Vec<(String, Value)>` (`BTreeMap::from_iter` on sorted input is O(n)) instead of per-entry `insert`; reserve `Vec` in `visit_seq` by a small probe | `value/de.rs:137-141` | codec-memory-copies | `Value` equality and identical sorted serialisation | S1/S2 DOM parse with value construction discarded |
| B12 | `from_slice` UTF-8: ASCII-run fast path (`ascii_run_*`) so validation walks only non-ASCII tails; per-string semantics kept so `InvalidUnicodeCodePoint` lands on the same string | `read.rs:868-870` | codec-eliminate-redundancy → vectorize | identical error text/position; identical borrowed slices | S1 `from_slice` vs `from_str` = the ceiling |
| B13 | `RawValue` on `IoRead` as a range copy | `read.rs:379-397` | (falls out of B7) | `RawValue::get()` identical | — |
| B14 | Evaluate derive `deserialize_in_place` for `Vec<Struct>`; enable in the fork if it pays | serde_derive feature | codec-measurement | values identical | S2 struct parse |
| B15 | Fold `,` and `:` into the adjacent key/value write on the `Vec` sink | `ser.rs:1842,1884,1910` | codec-memory-copies | bytes identical | counter (write calls) |
| B8 (v1.x) | `float_roundtrip` via core's Eisel-Lemire; delete vendored `lexical` | `de.rs:624-636`, `lexical/*` | codec-eliminate-redundancy | bit-identical f64/f32 over `tests/lexical` + 10M random decimals | S3 parse under `float_roundtrip` |
| B11 (v1.x) | Arena `Value` as an additional type | new module | footprint-decomposition + codec-cache-tiles (gated by S8) | `to_string` identical to `serde_json::Value`'s | S1 DOM parse ceiling = struct parse |

Not bricks, recorded so they are not re-litigated: the recursion guard (a `u8`), `itoa` and
`zmij` internals (already table/Schubfach; S3 stringify is at parity with sonic-rs),
`ignore_value` (already iterative), `BTreeMap → IndexMap` by default (changes key order in
output — that is what `preserve_order` is for), and any `get_unchecked` before the M1
bounds-check census says there is a real one (the h264/dds/rav1e measurement is ~0).

Every brick's `.s` is checked before and after (`cargo rustc --release -p rusty_json_turbo
--lib -- --emit asm`): packed ops counted for anything claimed vectorised, guard branches to
the panic block counted for anything claimed to prove an index, runtime-length `memcpy`
calls counted for anything claimed to remove a copy. A fast path is priced in calls avoided,
never in static instruction count — the census counter, not the `.s`, decides B1, B6 and B6h.

---

## 9. Decisions — recommendations for the owner to ratify

1. **Naming.** Crate and repo `rusty_json_turbo` (crates.io: free; `rusty_json` is taken by
   an unrelated crate). CLI `rjson` (does not shadow `jq`; matches `rzstd`/`rxmlint`/
   `rerasure`). **Recommended.**
2. **What "fork serde" means, concretely.** Two repositories:
   - `remade-with-rust/rusty_json_turbo` — a **history-preserving clone** of
     `serde-rs/json` with the crate renamed and the house workspace layered on top, so
     `git merge upstream/master` stays cheap and every divergence is listed in
     `docs/UPSTREAM-CHANGES.md` (the rusty_png precedent). Published to crates.io under the
     new name; API-identical so the swap is one `package =` line.
   - `remade-with-rust/serde` — a GitHub fork of `serde-rs/serde`, branch `turbo`, crate
     names and versions **unchanged** (`1.0.229+turbo.N`), consumed by this workspace and by
     house apps through `[patch.crates-io]` (by git URL, pinned `rev`, like
     remade_ffmpeg_rs). **Never republished under a new name**: the trait vocabulary is the
     ecosystem's contract, and a renamed serde would fork every `impl Serialize` in
     existence. Every derive brick that needs no house crate is filed upstream as a PR; the
     fork is the integration branch until it merges.
   - Alternative considered and rejected: publish `rusty_serde` / `rusty_serde_derive`. It
     would give the house a crate it can pin on crates.io, at the price of a second trait
     vocabulary and zero ecosystem impls. The `[patch]` overlay keeps every impl and costs
     one line in each app's workspace manifest. **Recommended: the two-repo shape.**
3. **License and attribution.** Both forks stay MIT OR Apache-2.0 (upstream's terms;
   changing them is neither possible nor wanted). `NOTICE.md` names serde's authors and the
   upstream commits; `UPSTREAM-CHANGES.md` is the divergence ledger; the README says
   "forked from serde_json, byte-identical by contract" in its first paragraph. Upstream's
   `LICENSE-*` files remain. **Recommended.**
4. **The byte-identical contract includes the default float path.** `f64_from_parts`'s
   not-correctly-rounded results are what every house consumer has been reading for years;
   v1.0 preserves them bit for bit. A correctly-rounded default is a semantic change and
   ships as the opt-in `float_correct` feature in v1.x, gated against core's parser.
   **Recommended.**
5. **Edition and MSRV.** Stay on **edition 2021** so upstream merges do not fight a
   migration, and set **MSRV 1.85** (indexmap/hashbrown's floor under `preserve_order`, and
   the older house repos' pin) rather than upstream's 1.71 or the new-repo default of 1.95.
   Re-visit at v1.0. **Recommended.**
6. **Dependencies.** Keep upstream's four (`serde_core`, `itoa`, `memchr`, `zmij`); add
   none to the published crates. `zmij` is MIT-only — allowed. `-accel` has zero
   dependencies. Competitor crates are bench-only behind `--features competitors`.
   **Recommended.**
7. **ISA policy.** SSE2 kernels are the x86-64 baseline and need no runtime detection;
   AVX2 behind cached detection; NEON and simd128 in v1.x; **never** a build that requires
   `-C target-cpu=native`, and CI compiles and tests the scalar fallback. **Recommended.**
8. **No rig.** The oracle is upstream serde_json in-process on every machine; the ledger
   names the machine per run (this laptop for M0–M5, whatever CI arm runners exist for the
   NEON row). **Recommended.**
9. **Upstreaming policy.** Every byte-identical, dependency-free brick is offered to
   `serde-rs/json` or `serde-rs/serde` as a PR with its ledger row attached, whether or not
   we expect it merged. The house does not carry patches it could have given back.
   **Recommended.**
10. **Scope guard.** No feature that changes output by default, no second `Value` on the
    default path, no threading inside the API. Anything that violates §3 NEVER needs a new
    decision here, not a PR. **Recommended.**

---

## 10. Skill routing — where the house playbooks deploy on this mission

| Phase | Governing skill | What it dictates here |
|---|---|---|
| Always, before any number | `codec-measurement` | pin (a P-core), real clock + `cpu/wall`, ABBA with alternating lead, null arm per session, N ≥ 20, work-count parity (bytes/values), method line on every ledger row, counters for sub-1% bricks, "impossible number = broken instrument" |
| Scaffold, dependencies, allocator, seams | `building-the-new-internet` | the workspace shape, allocator seam, no `*-sys`, dual-target check gate, pre-flight checklist |
| M1 instruments | `codec-analyzer` | feature-gated stage profiler with its own tax subtracted, deterministic best-of-N, the size sweep (S8), the bounds-check-ceiling probe, the reach census |
| M2 first bricks | `codec-eliminate-redundancy` then `codec-memory-copies` | hoist, table, walk-once, branchless; `Vec` growth, runtime-length `memcpy` calls, emitted-asm census, stale-binary checklist |
| Generic Rust sites (derive output, `Value` building) | `rusty-blazing-fast` | borrow over own, `entry()`, container complexity, std kernels then prove they are called |
| Before any `unsafe` | `rusty-unsafe-optimizations` | count emitted `panic_bounds_check` first; check the safe kernel exists and is reached; SAFETY contract; scalar twin as oracle |
| M3 kernels | `codec-vectorize-kernel` (REACHABILITY.md first) | `*_matches_scalar` written before the SIMD, dispatch at the surface, ISA-rung A/B, census before believing a flat A/B |
| Layout ideas (arena `Value`, key interning) | `codec-cache-tiles` + `footprint-decomposition` | gate with the S8 size sweep; byte-exact footprint decomposition before any layout change |
| Buffer sizes, bitmap widths, depth limits | `parameterizing-a-constant` | derive not transcribe; fixtures in the counted unit; poison every gate; probe one step past |
| A brick that should have worked and did not | `codec-six-whys-unknowns` + `rusty-curiosity` | depth 6 first (is the instrument sound?), one skill per D3 gate; descend one layer and read the siblings at the refuted site |
| Content-shape variance (S4 payloads vs S1) | `codec-content-adaptive-dispatch` (the principle only) | a per-input sign-flip between two strategies is a dispatch trigger, never averaged away; the JSON analogue is short-string vs long-string and small-doc vs large-doc paths |
| Pre-v1.0 | `use-protection-please` | critical-path tier, all 41 gates, `docs/plans/use-protection-please.md`, rendered README table, ★ gates block 1.0.0 |
| Never on this mission | `codec-asm-kernel`, `codec-experimental`, `codec-tune-quality`, `codec-bringup-*`, `codec-search-skip-gate` | there is no bitstream to change, no quality to tune, no C oracle to bring up against, and intrinsics will not top out before the API does |

---

## 11. Milestones — one brick at a time

| M | Deliverable | Exit test |
|---|---|---|
| M0 | Repos (§9.2): clone serde-rs/json with history, rename, house workspace, alloc seam, Deputy, deny.toml, CI (3-OS test, portable, lint, package, portfolio-check, 8-target check matrix); `serde_json_upstream` oracle dep; `tests/oracle.rs` differential gate over S1–S3 + upstream `tests/`; `corpus/` fetch script + hashes; `LEDGER.md` stub; `pinvs.ps1` port; fork of serde-rs/serde on branch `turbo` with `[patch.crates-io]` wired | check matrix green on all 8 targets incl. `--no-default-features --features alloc`; oracle test green (bytes, `Value`, errors, float bits) on every corpus file; `deputy discover` shows exactly upstream's dependency set; **admissible baseline in the ledger**: pinned, ABBA, N ≥ 20, null arm floor printed, every S1–S3 cell for upstream, simd-json and (if it builds on MSVC) sonic-rs |
| M1 | Instruments: `profile` feature stage profiler, census counters (bytes per path, allocs, kernel calls), `rjson census`/`bench`/`diff-oracle`; S4 house corpus captured from real payloads with coverage counters; fuzz targets (7) seeded; emitted-asm census of `de.rs`/`ser.rs`/`read.rs` (guard branches, `memcpy` calls, packed ops); **ceiling probes for B1, B3, B4, B5, B6/B6h, B10, B12** on S1/S2/S4 | ranked worklist in `LEDGER.md` with each brick's ceiling share and the arithmetic (share × plausible speedup vs the null floor); bounds-check ceiling priced once (expect ~0); profiler tax measured and subtracted; fuzzers clean for 1 h each |
| M2 | Safe bricks, byte-identical: B1, B4, B5, B9, B10, B12 (scalar ASCII run), B15, B6 (JSON-side key dispatch); each one commit with counter + clock; sub-1% bricks batched behind one switch | oracle test green after every brick; S1 DOM parse and S1/S2 struct parse improved above the null floor (win-rate + z in the ledger); B-side reverts recorded with their kind; `forbid(unsafe_code)` now on the core (upstream's 12 sites moved or replaced, census says which) |
| M3 | The `-accel` island: `ws_skip`, `str_scan`, `escape_mask`, `ascii_run` in SSE2 and AVX2 with `*_matches_scalar` written first, dispatch at the surface, `RJT_ISA` override, census wired; B1s, B2, B3 (vector half), B12 (vector half) | twins green over all 256 byte values × all positions × Miri; census 100% on S1/S2/S4/S5 shipping path; ISA-rung A/B (`RJT_ISA=scalar` vs `avx2`) in the ledger bounds remaining kernel work; G6 met on x86-64 |
| M4 | The serde fork earns its keep: B6d (shared matcher), B14 (in-place), B6h (field-index handshake) if the M1 probe cleared the floor; `UPSTREAM-CHANGES.md` for both repos; first upstream PRs filed | serde `test_suite` + 118 ui + Miri green on the fork; S9 `llvm-lines` ≤ upstream and build wall ≤ upstream; S1/S4 struct parse improved above floor with B6h on, byte-identical incl. aliases and unknown-field errors; G9 met |
| M5 | The campaign to G2: B7 (buffered `IoRead`) + B13, S5 stream cell, S6 pathological cell, re-profile after every keeper, sub-1% batches; the three-probe rule on every refutation | G2 bars met on S1, S2, S4, S5; S3 ≥ 1.0×; every ledger row carries its method line; a written "what remains is architectural" note for any cell that stalled after two flat bricks |
| M6 | Portability: NEON + simd128 twins with per-arch census (qemu-aarch64 + wasmtime in CI), `--no-default-features` tests on every target, wasm32 test job | G5 met; per-arch census asserted in the portable gate; browser demo checksum == native checksum |
| M7 | Consumer swap: `rusty_time-api`, `mid-verify`/`mid-signin`, `deputy-api` on `rusty_json_turbo` via `package =`; `mata-master` on `[patch.crates-io]` for both forks; their suites green; JSON output byte-identical to before on their fixtures | G8 met; commits in the consumer repos; S4 numbers re-taken on the consumers' real payloads |
| M8 | Hardening: `use-protection-please` audit (critical-path tier), `docs/plans/use-protection-please.md`, rendered README table, `SECURITY.md`, SBOM, `cargo vet` entries, 30 days of continuous fuzzing, Miri in CI | every ★ gate `Completed` or waived with a date; README written FROM the ledger; G1–G9 green |
| v1.0 | crates.io publish of `rusty_json_turbo` (+ `-accel`, `-cli`); the serde fork tagged `1.0.229+turbo.1` and pinned by the house | G1–G9 green; use-protection-please ★ gates clear; README claims ⊆ ledger |

---

## 12. The first week, in order

1. `gh repo fork serde-rs/json --org remade-with-rust --fork-name rusty_json_turbo` and
   `gh repo fork serde-rs/serde --org remade-with-rust`; clone both under `F:/coding/`;
   branch `turbo` on serde; keep `upstream` remotes.
2. Rename the crate, lay the workspace over the clone, add the alloc seam, `deny.toml`,
   `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `NOTICE.md`,
   `docs/UPSTREAM-CHANGES.md` (first entry: the rename).
3. `tests/oracle.rs`: link `serde_json_upstream`, assert bytes / `Value` / errors / float
   bits over S1–S3, upstream's `tests/`, jsonchecker, roundtrip. Make it fail once on purpose
   (poison a byte) before trusting it.
4. Port `pinvs.ps1`; verify the P-core map on this box; take the null arm; take the M0
   baseline with the method line; write the first ledger rows.
5. CI: the rusty_zstd `ci.yml` shape plus rusty_erasure's check matrix; `portfolio-check.yml`
   calling the org workflow; `cargo deny` green with `zmij` on the allow-list.
6. Capture S4 from real house payloads (mid tokens, rusty_time reports, deputy receipts),
   with coverage counters proving each enters untagged/`flatten`/long-string paths.
7. M1's ceiling probes, one per brick, in the order of §8 — before writing any brick.

The plan is finished when the ledger, not this file, says so.
