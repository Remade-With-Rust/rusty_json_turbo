# JSONCORP ledger

Every measurement that may be cited lives here, with the run that produced it.
**Anything not in this file does not exist.** README claims are a subset of
this file. A number without its method line is not evidence.

Conventions (mission plan §7.1, `codec-measurement`):

- **Arms.** `ours` = this tree at the named commit; `upstream` = serde_json
  1.0.151 linked in-process under the `serde_json_upstream` rename; `simd-json`
  0.18 and `sonic-rs` 0.5 behind `--features competitors`.
- **Columns.** DOM parse (`from_str::<Value>`, UTF-8 check inside the timed
  region as json-benchmark does), DOM stringify (`to_writer` into a pre-sized,
  cleared `Vec`), struct parse (`from_str::<T>`), struct stringify.
- **Statistic.** Per arm-sample: min per-iteration time over a window; per
  cell: the median of the paired ratios `ours/upstream` (**< 1 means ours is
  faster**), the paired win count and `z = (wins - N/2) / (0.5 * sqrt(N))`.
  MB/s = bytes / median-of-window-minimums.
- **Method line** (printed by `rjson-bench`, pasted verbatim under every table):
  pinned core, priority, clock, pairs, window, allocator, ISA arm, work-parity
  check, null-arm floor for the session, machine, rustc, commit.
- **Reverts** record which kind: measured worse, or inside the noise.

---

## Sessions

### 2026-09-09 -- M0 baseline (commit `b3df966`)

**Environment.** Intel Core i7-14650HX (8P+8E; logical CPUs 0-15 are the P-core
threads), 32 GB, Windows 11 Home 10.0.26200, rustc 1.98.0 (88d9e12ae 2026-08-18),
LLVM 22.1.8, `x86_64-pc-windows-msvc`. Binary: `target/bench/release/rjson-bench.exe`
built with `--features competitors`, `[profile.release] lto = "thin",
codegen-units = 1`, no `-C target-cpu` flag (portable x86-64: SSE2 baseline).
Runner: `tools/pinbench.ps1 -Cpu 2` (affinity 0x4 read back and confirmed on all
four runs, priority High); CPU time 120.2-120.5 s per ~121 s run, i.e. `cpu/wall`
~1.0: single-threaded and never descheduled. Raw per-pair logs: `corpus/runs/m0-*.txt`.

**Method line (identical for all four tables):** in-process paired A/B, one binary,
both arms linked; lead alternated per pair (ABBA); pairs=20; window=250 ms per
arm-sample, statistic=min per-iteration time in the window, verdict=median of paired
ratios + paired wins with z; clock=`std::time::Instant` (QPC); pinned=cpu2/High;
allocator=system (both arms); isa=scalar (no kernels wired at M0); work parity:
identical input bytes, output length asserted equal per pair for the serde_json-shaped
arms; stringify buffers pre-sized and cleared (growth excluded); simd-json's mandatory
input copy excluded from the timed region; commit=b3df966.

**Reading the ratio column:** `a/b time` **< 1 means `a` is faster**.

#### Null arm: upstream vs upstream (the floor)

| file | column | upstream | upstream | upstream/upstream time (median [min, max]) | wins, z |
|---|---|---:|---:|---:|---:|
| twitter | dom-parse | 472 MB/s | 475 MB/s | 1.011x [0.927, 1.075] | 8/20, z = -0.89 |
| twitter | dom-stringify | 1765 MB/s | 1773 MB/s | 1.000x [0.926, 1.112] | 10/20, z = +0.00 |
| twitter | struct-parse | 952 MB/s | 950 MB/s | 0.999x [0.965, 1.030] | 11/20, z = +0.45 |
| twitter | struct-stringify | 1878 MB/s | 1870 MB/s | 0.990x [0.929, 1.039] | 15/20, z = +2.24 |
| citm_catalog | dom-parse | 640 MB/s | 608 MB/s | 0.977x [0.685, 1.486] | 14/20, z = +1.79 |
| citm_catalog | dom-stringify | 1174 MB/s | 1159 MB/s | 0.999x [0.654, 1.942] | 11/20, z = +0.45 |
| citm_catalog | struct-parse | 1364 MB/s | 1389 MB/s | 1.012x [0.908, 1.051] | 7/20, z = -1.34 |
| citm_catalog | struct-stringify | 2021 MB/s | 2026 MB/s | 1.003x [0.914, 1.143] | 8/20, z = -0.89 |
| canada | dom-parse | 404 MB/s | 407 MB/s | 1.005x [0.918, 1.047] | 7/20, z = -1.34 |
| canada | dom-stringify | 969 MB/s | 972 MB/s | 0.995x [0.954, 1.075] | 12/20, z = +0.89 |
| canada | struct-parse | 772 MB/s | 770 MB/s | 0.997x [0.968, 1.033] | 12/20, z = +0.89 |
| canada | struct-stringify | 635 MB/s | 636 MB/s | 0.996x [0.890, 1.033] | 12/20, z = +0.89 |

**Floor, same instance:** medians 0.977-1.012 (within 2.3%); worst pairs mostly
under 1.15, with two spikes on citm_catalog (1.486, 1.942 -- one disturbed pair
each). One identical-code cell (twitter struct-stringify) reads 15/20, z = +2.24 at a
1.0% median: at N=20, **a z above 2 with a median inside 1% is noise**, so a claim
needs both a median outside the floor AND z > 2.

#### Ours vs upstream (identical source at M0 -- the cross-instance floor)

| file | column | ours | upstream | ours/upstream time (median [min, max]) | wins, z |
|---|---|---:|---:|---:|---:|
| twitter | dom-parse | 466 MB/s | 469 MB/s | 1.008x [0.985, 1.231] | 5/20, z = -2.24 |
| twitter | dom-stringify | 1712 MB/s | 1710 MB/s | 1.000x [0.939, 1.027] | 10/20, z = +0.00 |
| twitter | struct-parse | 936 MB/s | 924 MB/s | 0.988x [0.952, 1.021] | 14/20, z = +1.79 |
| twitter | struct-stringify | 1839 MB/s | 1780 MB/s | **0.968x** [0.917, 0.991] | **20/20, z = +4.47** |
| citm_catalog | dom-parse | 712 MB/s | 721 MB/s | 1.005x [0.936, 1.038] | 9/20, z = -0.45 |
| citm_catalog | dom-stringify | 1212 MB/s | 1213 MB/s | 1.000x [0.961, 1.029] | 9/20, z = -0.45 |
| citm_catalog | struct-parse | 1396 MB/s | 1393 MB/s | 0.993x [0.954, 1.066] | 14/20, z = +1.79 |
| citm_catalog | struct-stringify | 2020 MB/s | 2030 MB/s | 0.995x [0.917, 1.091] | 12/20, z = +0.89 |
| canada | dom-parse | 400 MB/s | 402 MB/s | 1.002x [0.948, 1.050] | 9/20, z = -0.45 |
| canada | dom-stringify | 967 MB/s | 970 MB/s | 1.003x [0.965, 1.038] | 8/20, z = -0.89 |
| canada | struct-parse | 742 MB/s | 742 MB/s | 1.001x [0.960, 1.054] | 9/20, z = -0.45 |
| canada | struct-stringify | 668 MB/s | 667 MB/s | 0.998x [0.924, 1.063] | 13/20, z = +1.34 |

**Finding (the instrument asking for help, §7 of codec-measurement).** `ours` and
`upstream` are the same source, byte-identical by the oracle, in one binary -- and
twitter struct-stringify reads **0.968x at 20/20, z = +4.47**. Two separately compiled
crate instances do not share codegen: with thin LTO and one CGU the two copies of the
same generic code land at different alignments and inlining decisions, and on a
~340 us cell that is worth ~3%. Consequences, in force from here on:

- **This table, not the same-instance null, is the floor for any ours-vs-upstream
  claim.** A brick on twitter struct-stringify must move the cell past 0.968x, not
  past 1.0.
- **Sub-3% effects are decided by counters, never by this clock** (codec-measurement
  §15): bytes down each path, allocations, kernel calls, guard branches in the `.s`.
- **Re-take this table whenever the build changes shape** (profile, LTO, a new crate in
  the binary): the bias is a property of the link, not of the code.

#### Ours vs simd-json 0.18.1 (runtime AVX2 dispatch; input copy excluded)

| file | column | ours | simd-json | ours/simd-json time (median [min, max]) | wins, z |
|---|---|---:|---:|---:|---:|
| twitter | dom-parse | 458 MB/s | **743 MB/s** | 1.628x [1.549, 2.052] | 0/20, z = -4.47 |
| twitter | dom-stringify | **1657 MB/s** | 1331 MB/s | 0.802x [0.744, 0.882] | 20/20, z = +4.47 |
| twitter | struct-parse | **911 MB/s** | 845 MB/s | 0.942x [0.874, 1.052] | 17/20, z = +3.13 |
| twitter | struct-stringify | **1892 MB/s** | 1796 MB/s | 0.951x [0.891, 1.015] | 18/20, z = +3.58 |
| citm_catalog | dom-parse | 715 MB/s | 707 MB/s | 0.983x [0.957, 1.020] | 16/20, z = +2.68 |
| citm_catalog | dom-stringify | 1208 MB/s | 1202 MB/s | 0.995x [0.925, 1.081] | 12/20, z = +0.89 |
| citm_catalog | struct-parse | **1360 MB/s** | 1150 MB/s | 0.844x [0.785, 0.946] | 20/20, z = +4.47 |
| citm_catalog | struct-stringify | **2046 MB/s** | 1614 MB/s | 0.787x [0.727, 0.851] | 20/20, z = +4.47 |
| canada | dom-parse | **404 MB/s** | 325 MB/s | 0.801x [0.773, 0.878] | 20/20, z = +4.47 |
| canada | dom-stringify | **941 MB/s** | 680 MB/s | 0.722x [0.686, 0.824] | 20/20, z = +4.47 |
| canada | struct-parse | **708 MB/s** | 508 MB/s | 0.721x [0.665, 0.757] | 20/20, z = +4.47 |
| canada | struct-stringify | **626 MB/s** | 439 MB/s | 0.699x [0.670, 0.747] | 20/20, z = +4.47 |

simd-json's DOM is `BorrowedValue` (its tape → borrowed DOM), written back with
`Writable::write`; struct columns go through its serde bridge. It wins one cell
(twitter DOM parse, 1.63x) and loses or ties the other eleven. The json-benchmark
README's 2.2x on that cell (unpinned, min-of-256, jemalloc, `target-cpu=native`)
does not reproduce under this method; both numbers are recorded.

#### Ours vs sonic-rs 0.5.8 (compile-time ISA; built WITHOUT `-C target-cpu=native`)

| file | column | ours | sonic-rs | ours/sonic-rs time (median [min, max]) | wins, z |
|---|---|---:|---:|---:|---:|
| twitter | dom-parse | 449 MB/s | **1609 MB/s** | 3.583x [3.286, 4.526] | 0/20, z = -4.47 |
| twitter | dom-stringify | **1669 MB/s** | 1557 MB/s | 0.936x [0.901, 0.978] | 20/20, z = +4.47 |
| twitter | struct-parse | 914 MB/s | **937 MB/s** | 1.031x [1.001, 1.051] | 0/20, z = -4.47 |
| twitter | struct-stringify | **1833 MB/s** | 1742 MB/s | 0.964x [0.926, 1.015] | 19/20, z = +4.02 |
| citm_catalog | dom-parse | 668 MB/s | **1366 MB/s** | 2.047x [1.815, 2.170] | 0/20, z = -4.47 |
| citm_catalog | dom-stringify | **1168 MB/s** | 781 MB/s | 0.666x [0.622, 0.692] | 20/20, z = +4.47 |
| citm_catalog | struct-parse | 1312 MB/s | **1399 MB/s** | 1.076x [1.037, 1.135] | 0/20, z = -4.47 |
| citm_catalog | struct-stringify | **1958 MB/s** | 987 MB/s | 0.504x [0.470, 0.533] | 20/20, z = +4.47 |
| canada | dom-parse | 391 MB/s | **592 MB/s** | 1.508x [1.457, 1.568] | 0/20, z = -4.47 |
| canada | dom-stringify | **989 MB/s** | 923 MB/s | 0.936x [0.878, 0.969] | 20/20, z = +4.47 |
| canada | struct-parse | **736 MB/s** | 681 MB/s | 0.923x [0.908, 0.958] | 20/20, z = +4.47 |
| canada | struct-stringify | **656 MB/s** | 637 MB/s | 0.970x [0.936, 1.023] | 18/20, z = +3.58 |

sonic-rs's DOM is its own arena `Value` (not `serde_json::Value`), which is the whole
of its DOM-parse lead (3.6x / 2.0x / 1.5x): the headroom the mission's DOM bricks (B10,
and the v1.x arena `Value`) are aimed at. On the struct columns it is 3-8% ahead on
parse for twitter and citm, behind on canada, and behind on every stringify cell --
half our speed on citm struct-stringify. **Caveat, stated once and carried on every
citation:** sonic-rs selects its SIMD width at compile time and its README mandates
`-C target-cpu=native`; this build is portable SSE2, so its parse numbers here are a
lower bound of what it does when built its documented way. An M1 row re-runs both
competitor arms under `target-cpu=native` for the honest upper bound.

**Competitor caveat, both tables:** output lengths are asserted equal only between the
serde_json-shaped arms; a competitor's stringify MB/s is denominated in OUR output
length for that cell. The lengths were not compared this session (M1 prints both).

#### M0 exit test -- MET

| Bar | Result |
|---|---|
| check matrix green on all 8 targets incl. `--no-default-features --features alloc` | met, plus `alloc,float_roundtrip,arbitrary_precision,raw_value,unbounded_depth`; `tests/crate` no_std probe green |
| oracle test green on every corpus file | met: 66 corpus documents + 118 edge documents + 93,893 distinct number tokens, 0 mismatches, under default and each of `preserve_order`, `float_roundtrip`, `arbitrary_precision`, `raw_value`, `unbounded_depth` |
| `deputy discover` shows exactly upstream's dependency set | met for the library: `cargo tree -p rusty_json_turbo -e normal` = itoa 1.0.18, memchr 2.8.3, serde_core 1.0.229 (the fork), zmij 1.0.23. `deputy discover .` lists 104 pinned crates for the whole workspace (dev, harness and competitor arms included) |
| admissible baseline: pinned, ABBA, N >= 20, null-arm floor, every S1-S3 cell for upstream, simd-json and sonic-rs | met: the four tables above |
| upstream's own suite green against the fork | met (235 tests + 97 doctests; compiletest ignored as upstream ignores it) |
| lint | `cargo fmt`, `clippy -D warnings` (default and `--all-features`), ASCII sources, `cargo deny check`, `cargo audit` all clean |

Reverts this session: none (no brick was attempted; M0 is scaffold and instruments).
