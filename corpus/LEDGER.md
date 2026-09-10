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

---

### 2026-09-09 -- M1-A: the house allocator, and nothing else

**The question.** Take the fork exactly as it is -- not one line of JSON code
changed -- link the house stack's allocator through the seam, and measure. Does
it move?

**Answer: yes, and by a lot, exactly where the work is allocation.** On the
conservative probe: **1.53x** on twitter DOM parse, **1.46x** on citm DOM parse,
1.07x on canada struct parse, and **nothing at all** on the four cells that
allocate zero times. Read the two caveats under "What this is not" before
quoting any of it -- in particular, this is the allocator's win, not the fork's.

**Correctness first.** The full oracle (66 corpus documents, 118 edge documents,
93,893 number tokens) passes under `rusty_alloc` and under `rusty_alloc`
`secure`: **zero mismatches, no output byte changed**. A speed change that
altered output would be a bug with good timing.

#### Why this needs a different harness

A `#[global_allocator]` is one per program. It cannot be an in-process arm the
way `ours` and `upstream` are, so this is a **process-level** paired A/B between
two binaries (`tools/pinvs.ps1`): each binary reports its own per-cell time via
`rjson-bench solo`, the leading binary alternates per pair (ABBA), affinity is
read back on every run, and output bytes are asserted equal per cell per pair.
The binaries differ only in the seam's feature (`--features rusty-alloc`), and
carry no competitor arms.

> **Absolute MB/s in this section are NOT comparable with the M0 tables.**
> Different binaries (no competitor arms linked) and a different regime (a fresh
> process per sample instead of one long-lived process). The **ratio within each
> table** is the result; a cross-table absolute comparison is not admissible.

#### The work-parity instrument, and the control it handed us for free

`rjson-bench census --all` on a counting build (`--features profile`, whose
timings are never quoted) reports allocations per operation. The counts are
**byte-for-byte identical under both allocators** -- same allocs, same bytes,
same reallocs -- so the arms do identical work and what differs is the cost of
each allocation, not how many there are.

| file | column | allocs/op | alloc bytes/op | reallocs/op |
|---|---|---:|---:|---:|
| twitter | dom-parse | 20,834 | 2,071,269 | 11 |
| twitter | struct-parse | 2,762 | 371,567 | 11 |
| twitter | dom-stringify | **0** | 0 | 0 |
| twitter | struct-stringify | **0** | 0 | 0 |
| citm_catalog | dom-parse | 39,339 | 7,681,413 | 1,671 |
| citm_catalog | struct-parse | 2,544 | 201,568 | 1,671 |
| citm_catalog | dom-stringify | **0** | 0 | 0 |
| citm_catalog | struct-stringify | **0** | 0 | 0 |
| canada | dom-parse | 56,061 | 9,754,170 | 1,622 |
| canada | struct-parse | 485 | 668,842 | 1,622 |
| canada | dom-stringify | **0** | 0 | 0 |
| canada | struct-stringify | **0** | 0 | 0 |

**The four zero-allocation cells are a control we did not have to build.** The
stringify columns write into a pre-sized, cleared buffer, so they allocate
nothing and the allocator *cannot* touch them. Whatever they read is the
cross-binary layout-and-noise band for this pair of binaries -- and any cell
inside that band is not attributable to the allocator, however good its z-score
looks.

#### Null arm: the same binary against itself (the floor)

| file | column | sysA | sysB | ratio (median [min, max]) | wins, z |
|---|---|---:|---:|---:|---:|
| twitter | dom-parse | 365 MB/s | 369 MB/s | 1.005x [0.902, 1.088] | 10/20, z = +0.00 |
| twitter | dom-stringify | 1457 MB/s | 1410 MB/s | 0.978x [0.946, 1.045] | 16/20, z = +2.68 |
| twitter | struct-parse | 890 MB/s | 877 MB/s | 1.001x [0.938, 1.049] | 10/20, z = +0.00 |
| twitter | struct-stringify | 1488 MB/s | 1485 MB/s | 1.005x [0.915, 1.045] | 8/20, z = -0.89 |
| citm_catalog | dom-parse | 681 MB/s | 686 MB/s | 1.015x [0.939, 1.072] | 8/20, z = -0.89 |
| citm_catalog | dom-stringify | 1185 MB/s | 1171 MB/s | 0.998x [0.933, 1.053] | 11/20, z = +0.45 |
| citm_catalog | struct-parse | 1340 MB/s | 1334 MB/s | 0.998x [0.967, 1.083] | 11/20, z = +0.45 |
| citm_catalog | struct-stringify | 1788 MB/s | 1782 MB/s | 0.996x [0.950, 1.048] | 13/20, z = +1.34 |
| canada | dom-parse | 379 MB/s | 373 MB/s | 0.983x [0.945, 1.038] | 12/20, z = +0.89 |
| canada | dom-stringify | 931 MB/s | 921 MB/s | 0.990x [0.935, 1.050] | 13/20, z = +1.34 |
| canada | struct-parse | 713 MB/s | 708 MB/s | 1.003x [0.933, 1.062] | 9/20, z = -0.45 |
| canada | struct-stringify | 667 MB/s | 673 MB/s | 1.011x [0.959, 1.056] | 9/20, z = -0.45 |

Floor: medians 0.978-1.015. One identical-binary cell reads z = +2.68 at a 2.2%
median, which is the standing reminder that **z alone is not a verdict** -- an
effect needs a median outside this band as well.

#### system vs rusty_alloc 2.0.4 (20 pairs, 250 ms windows)

`> 1` means **rusty_alloc is faster**. Cells ordered by allocations per op.

| file | column | allocs/op | system | rusty_alloc | ratio (median [min, max]) | wins, z |
|---|---|---:|---:|---:|---:|---:|
| canada | dom-parse | 56,061 | 376 MB/s | **499 MB/s** | **1.320x** [1.267, 1.584] | 20/20, z = 4.47 |
| citm_catalog | dom-parse | 39,339 | 684 MB/s | **1004 MB/s** | **1.486x** [1.417, 1.747] | 20/20, z = 4.47 |
| twitter | dom-parse | 20,834 | 358 MB/s | **724 MB/s** | **1.995x** [1.846, 2.185] | 20/20, z = 4.47 |
| twitter | struct-parse | 2,762 | 880 MB/s | **1024 MB/s** | **1.168x** [1.083, 1.221] | 20/20, z = 4.47 |
| citm_catalog | struct-parse | 2,544 | 1326 MB/s | **1465 MB/s** | **1.121x** [1.038, 1.186] | 20/20, z = 4.47 |
| canada | struct-parse | 485 | 707 MB/s | 752 MB/s | 1.061x [1.001, 1.174] | 20/20, z = 4.47 |
| twitter | dom-stringify | **0** | 1443 MB/s | 1410 MB/s | 0.976x [0.937, 1.021] | 2/20, z = -3.58 |
| citm_catalog | dom-stringify | **0** | 1172 MB/s | 1168 MB/s | 0.990x [0.945, 1.043] | 8/20, z = -0.89 |
| canada | dom-stringify | **0** | 934 MB/s | 983 MB/s | 1.047x [0.985, 1.169] | 19/20, z = 4.02 |
| twitter | struct-stringify | **0** | 1497 MB/s | 1489 MB/s | 1.004x [0.949, 1.062] | 10/20, z = +0.00 |
| citm_catalog | struct-stringify | **0** | 1785 MB/s | 1901 MB/s | 1.070x [1.015, 1.117] | 20/20, z = 4.47 |
| canada | struct-stringify | **0** | 671 MB/s | 670 MB/s | 1.011x [0.963, 1.047] | 12/20, z = -0.89 |

**The shape is the finding.** Sort by allocation count and the effect sorts with
it: tens of thousands of allocations buy 1.3-2.0x, a few thousand buy 1.06-1.17x,
and zero buys nothing consistent. The zero-allocation cells span **0.976x to
1.070x**, which is this binary pair's layout band -- wider than the same-binary
null floor, as two different links should be. Two consequences, both binding:

- **canada struct-parse (1.061x) sits inside the layout band** and is therefore
  not an allocator result, despite 20/20 and z = 4.47. Not claimable.
- **citm struct-stringify reads 1.070x at 20/20 on a cell that allocates zero
  times.** The allocator cannot have done that; it is code placement. It is the
  honest upper bound on the layout component, and it is why the DOM-parse cells
  (1.32-2.00x, far outside it) are the ones that carry the result.

#### Three-probe confirmation: vary the window, not the seed

The obvious alternative explanation is warm-up: a fresh process per sample, so
maybe rusty_alloc merely reaches steady state sooner. Probe it by making each
sample **10x longer** -- if the effect is a start-up artifact it collapses.

| file | column | allocs/op | 250 ms, 20 pairs | 250 ms, 10 pairs | **2000 ms, 10 pairs** |
|---|---|---:|---:|---:|---:|
| twitter | dom-parse | 20,834 | 1.995x | 1.979x | **1.532x** [1.471, 1.645] |
| citm_catalog | dom-parse | 39,339 | 1.486x | 2.115x | **1.462x** [1.392, 1.503] |
| canada | struct-parse | 485 | 1.061x | 1.097x | **1.072x** [1.041, 1.095] |
| twitter | struct-stringify | **0** | 1.004x | 0.988x | **0.977x** [0.882, 1.007] |

It does not collapse. It **shrinks and stabilises**: twitter DOM parse goes
1.99x -> 1.53x when each sample runs 10x longer, so roughly a quarter of the
short-window figure was per-process warm-up and the rest is steady state. The
zero-allocation control holds at ~0.98x throughout. The 2 s column is the
conservative number and the one to quote.

The middle column also earns its place: citm DOM parse read **2.115x** there
because that run's *system* arm read 459 MB/s against 684 and 697 in the other
two. The rusty_alloc arm read 985/1004/1013 MB/s across all three. **The system
allocator is not merely slower here, it is markedly less repeatable** -- which is
itself a result for anything latency-sensitive.

#### rusty_alloc `secure` (guard pages, encrypted free lists)

| file | column | allocs/op | system | secure | ratio | wins, z |
|---|---|---:|---:|---:|---:|---:|
| twitter | dom-parse | 20,834 | 343 MB/s | **665 MB/s** | **1.945x** [1.807, 2.953] | 20/20, z = 4.47 |
| citm_catalog | dom-parse | 39,339 | 667 MB/s | **886 MB/s** | **1.452x** [1.327, 3.133] | 20/20, z = 4.47 |
| canada | dom-parse | 56,061 | 356 MB/s | **463 MB/s** | **1.325x** [0.807, 2.228] | 19/20, z = 4.02 |
| twitter | struct-parse | 2,762 | 848 MB/s | **975 MB/s** | **1.151x** [1.059, 1.789] | 20/20, z = 4.47 |
| citm_catalog | struct-parse | 2,544 | 1289 MB/s | **1377 MB/s** | **1.082x** [1.003, 1.224] | 20/20, z = 4.47 |
| canada | struct-parse | 485 | 675 MB/s | 692 MB/s | 1.051x [0.731, 1.142] | 17/20, z = 3.13 |

The hardened profile keeps essentially the whole win (1.95x / 1.45x / 1.33x on
DOM parse against 2.00x / 1.49x / 1.32x). Its per-pair spread is much wider
(one cell ranges to 3.13x), so its **median** is the usable statistic. Stringify
cells omitted: zero allocations, nothing to measure. Practical reading: on this
workload the safety posture is close to free.

#### What this is not

1. **This is not a win of rusty_json_turbo over serde_json.** At M1 the fork's
   JSON code is upstream's, byte for byte. Upstream serde_json linked against
   `rusty_alloc` would get the same speedup. It is a win *of the house stack*,
   available to anyone who adopts the allocator, and it does **not** count
   toward gate G2, which compares like-for-like allocators.
2. **This is a Windows result.** Rust's `System` allocator here is `HeapAlloc`
   on the process heap; a mimalloc-architecture allocator retaining freed blocks
   in its own free lists is exactly the design that beats it on
   allocate-many-small-then-drop workloads. glibc's malloc has tcache and fastbins
   and should close much of the gap, so the Linux number is a genuinely open
   question and an M1 follow-up, not something to extrapolate.
3. The measurement excludes the **drop** of the parsed value (the timed region
   ends before it, as json-benchmark's does). Deallocation is real work a
   consumer pays, and it is not in these numbers -- if anything that
   understates a free-list allocator's advantage.

#### What it changes about the mission

- **Allocation is the dominant cost of DOM parse, now measured twice.** 20,834
  allocations for a 632 KB document is roughly one allocation every 31 input
  bytes. The plan's B10 (bulk map build, reserve on `visit_seq`) and the v1.x
  arena `Value` are aimed at exactly this, and the M0 finding that sonic-rs's
  1.5-3.6x DOM lead comes from its arena `Value` now has a second, independent
  confirmation from a completely different instrument.
- **The stringify path already allocates zero times per op.** B5's sink
  specialisation cannot win by removing allocations, because there are none to
  remove; it must win on write-call count instead. That reprices the brick
  before it was built -- which is what a ceiling probe is for.
- **Every future timing table must name its allocator**, and the harness now
  prints it in the method line automatically.

**Method line (all M1-A tables).** PROCESS-level paired A/B between two
binaries; each binary reports its own per-cell time via `rjson-bench solo`, so
process launch is outside the number; leading binary alternated per pair (ABBA);
pairs as stated; statistic = min per-iteration time within each arm-sample
window; verdict = median of paired ratios + paired wins with z; pinned
cpu2/High with affinity read back per run; work parity = allocation census
identical between arms + output/input bytes asserted equal per cell per pair;
binaries differ only in the seam feature, no competitor arms; machine and
toolchain as in the M0 environment block; commit `4c8e0b3`+. Raw per-pair logs:
`corpus/runs/m1-alloc-*.txt`.

Reverts this session: none. Nothing was reverted because nothing was changed --
that is the point of the experiment.

---

### 2026-09-09 -- M1-B: the corpus priced, and M2-B1: the first brick

**The instruments first, because they decided what to build.**

#### Content census (deterministic; the clock is not involved)

| file | bytes | ws% | struct% | string% | number% | non-ascii% | strings | numbers |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| twitter | 646,995 | 27.8% | 10.3% | **57.1%** | 1.5% | 14.7% | 18,099 | 2,109 |
| citm_catalog | 1,777,672 | **71.9%** | 8.3% | 12.5% | 7.1% | 0.0% | 26,604 | 14,392 |
| canada | 2,251,060 | 0.0% | 9.9% | 0.0% | **90.1%** | 0.0% | 12 | 111,126 |

Escapes are rare everywhere: 1.7% of twitter's strings contain one, 0.0% of the
other two files'. Cross-checked independently (`python`: raw whitespace bytes,
digit bytes, and the size a minified re-serialisation would be) -- citm is
28.2% of its size when minified, confirming 71.9%; the census's twitter figure
(27.8%) is correctly *lower* than the raw count (28.3%) because whitespace
inside string literals is part of the value and is not skipped.

**The three files are nearly orthogonal**, which makes them a real test matrix:
canada is a float benchmark with no whitespace and no strings, citm is
three-quarters indentation, twitter is strings and Unicode. A brick that helps
one should visibly not help the others, and that is a gate, not a nicety.

#### Ceiling probe (arm=ours, rusty_alloc, 7 rounds, cell order rotated)

`scan` = `IgnoredAny`: walks the document and builds nothing. No stubbing, so
it cannot mis-measure by deleting a branch the program depends on. `no ws` = the
same document with the whitespace a parser skips removed and every token byte
preserved (asserted to parse to an equal `Value`).

| file | scan | struct-parse | dom-parse | scan share of dom | ceiling if Value free | ceiling if ws-skip free |
|---|---:|---:|---:|---:|---:|---:|
| twitter | 314,400 ns | 605,300 ns | 879,800 ns | 36% | **2.80x** | 1.12x (10% of dom) |
| citm_catalog | 878,500 ns | 1,200,000 ns | 1,718,100 ns | 51% | **1.96x** | **1.56x (36% of dom)** |
| canada | 1,477,500 ns | 2,786,000 ns | 4,184,700 ns | 35% | **2.83x** | 1.00x (0% of dom) |

Two decisions fell straight out. **Value construction is 47-64% of DOM parse**
-- the largest ceiling anywhere, and consistent with the allocator experiment
that moved the same cells 1.5x. And **whitespace skipping is 36% of DOM parse on
citm and 64% of its scan**, which is a large, cheap, byte-identical target. The
latter is smaller but far lower risk, so it goes first.

#### Brick B1 -- whitespace skipping off the per-byte path (KEPT)

`parse_whitespace` walked whitespace through `peek()`/`discard()`, paying a
`Result<Option<u8>>` construction, a match and a bounds check for every byte of
indentation. It now delegates to a new sealed-trait method `Read::skip_whitespace`
whose default body is exactly the old loop, overridden by `SliceRead` with one
walk of the slice. `StrRead` delegates; `IoRead` keeps the default.

**Correctness:** upstream's suite (235 tests + 97 doctests), the oracle (66
corpus + 118 edge documents + 93,893 number tokens, every feature flag), and a
300,000-case differential soak -- all green, zero divergences.

**Deterministic verdict (the primary instrument).** Counted with
`rjson-bench work`, one run each, `RJT_WS_FASTPATH` toggling the arm inside one
binary:

| cell | peek before | peek after | removed | discard removed |
|---|---:|---:|---:|---:|
| citm_catalog dom-parse | 1,587,979 | 141,319 | **-91.1%** | -84.5% |
| twitter dom-parse | 250,193 | 11,958 | **-95.2%** | -74.7% |
| canada dom-parse | 2,751,947 | 2,194,321 | **-20.3%** | -33 calls |

**The arithmetic closes exactly, on all three files:**
`peeks removed = whitespace runs + whitespace bytes`
(citm 169,287 + 1,277,373 = 1,446,660, measured 1,446,660; twitter 58,146 +
180,089 = 238,235, measured 238,235; canada 557,593 + 33 = 557,626, measured
557,626). Discards removed equals the whitespace-byte count exactly. An
instrument that reconciles to the byte on three unrelated documents is not
measuring noise.

That exactness also produced a finding the byte census had not: **the brick
removes one `peek` per whitespace *check*, not merely per whitespace byte.**
canada contains 33 whitespace bytes in 2.25 MB and still loses 557,626 peek
calls, because `parse_whitespace` is called 557,593 times and each call used to
pay a `peek()` just to discover the next byte was already a token. So this helps
minified documents too, which is not what the 0%-whitespace row predicted.

**Timing verdict (same binary, one env var between the arms).** Comparing two
*builds* was tried first and abandoned: the four stringify cells, which cannot
reach this code at all, moved 0.866x-1.070x, so build-to-build layout alone is
worth +-13% here and swamps the effect. The knob removes that entirely -- one
binary, one layout. 15 pairs, ABBA, pinned; **ratio > 1 means the brick is
faster**:

| cell | old / brick | brick won |
|---|---:|---:|
| citm_catalog scan | **1.168x** | 15/15, z = 3.87 |
| citm_catalog struct-parse | **1.057x** | 14/15, z = 3.36 |
| twitter scan | **1.055x** | 13/15, z = 2.84 |
| canada scan | **1.036x** | 12/15, z = 2.32 |
| citm_catalog dom-parse | 1.031x | 10/15, z = 1.29 -- under-resolved |
| twitter / canada dom-parse, struct-parse | 0.992x-1.008x | not significant |
| **stringify x4 (the control)** | **0.987x-1.011x** | **not significant, as required** |

The controls are the point: four cells that never call this code did not move,
in the same run that moved `scan` by 16.8% at 15/15.

**Honest limits.** The box was throttled all session -- the untouched upstream
arm read 243 MB/s on citm dom-parse against 640 MB/s at M0, and every core
tested read 350-400 against that 640. So absolute MB/s here are not comparable
with the M0 tables, and the dom-parse cells (where a ~9% effect is predicted)
are under-resolved rather than refuted. The counters are what keep this a
verdict.

**What is left.** Whitespace costs 565,500 ns of citm's 878,500 ns scan. The
brick recovered about 126,000 ns of it -- **roughly a fifth of the available
whitespace cost.** The other four fifths are still there, and that is the
measured, precise target for a wide (SWAR or SIMD) scan, which is the next
brick rather than a guess.

---

### 2026-09-09 -- M2-B1s: the wide whitespace scan (KEPT), and two regressions it taught

#### The count that decided it, and the count that nearly misled

The obvious number is the mean whitespace run: `citm_catalog` has 1,277,373
whitespace bytes over 169,287 calls to the skipper, which reads as 7.5 bytes and
says an eight-byte step is a wash. **That number is a units error.** Those 169,287
are *calls*, and most of them find no whitespace at all. The maximal runs are
what a wide step acts on, and there are only 76,337 of them:

| file | 1 | 2 | 3 | 4 | 5-8 | 9-16 | 17-32 | mean | longest |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| twitter | 13,345 | 1 | 0 | 4 | 2,970 | **12,118** | 388 | 6.25 | 22 |
| citm_catalog | 25,869 | 1 | 0 | 0 | 20 | 5,490 | **44,957** | 16.73 | 30 |
| canada | 9 | 7 | 0 | 1 | 1 | 0 | 0 | 1.83 | 6 |

Share of whitespace **bytes** in runs of at least 8: twitter **92.6%**,
citm_catalog **98.0%**, canada 18.2% (of 33 bytes). That is what justified the
brick -- and the same table's first column is what predicted the regression
below, had it been read closely enough the first time.

#### The brick

Eight bytes per step: four SWAR byte-equality tests (`0x20`, `\n`, `\t`, `\r`)
OR'd into a mask, then `trailing_zeros() >> 3` for the first non-whitespace
byte, taken out of the word already in hand rather than re-loaded. The scalar
walk stays as the oracle and as the arm the `RJT_WS_WIDE=0` knob selects.

**The zero-byte test is the exact one, not the famous one.** The classic
`(x - ONES) & !x & HIGHS` borrows across byte boundaries and reports a byte as
zero because its *neighbour* was. Here `(low7 + 0x7F) | x` cannot carry out of
its own byte. The twin test was poisoned with the classic form to prove it
discriminates: three of four tests failed, at `byte 0x21 at 1, scanning from 0:
wide said 24, scalar 1` -- exactly the borrow bug, which would otherwise have
shipped as "skip a byte that is not whitespace".

Gates: every byte value 0x00-0xFF at every offset 0-23 against the scalar twin,
every length 0-40, 2,000 random mixed buffers, and an explicit case for `0x0b`
and `0x0c` -- the vertical tab and form feed that a `b <= 0x20` or `0x09..=0x0d`
range test would wrongly skip. Plus the oracle, upstream's suite and the soak.

#### Two regressions, both found by files that cannot benefit

The corpus was chosen so that each brick has a file it must *not* help. Both
times, that file is what caught the defect.

1. **`canada.json` read 0.961x at 15/15** (z = 3.87) -- a 4% regression on a
   document with 33 whitespace bytes in 2.25 MB. Cause: the scanner returned
   only an index, so the caller re-loaded the byte it had just examined, on
   every one of 557,593 calls. Fix: return the byte with the index.
2. **`twitter.json` scan read 0.903x at 20/21** (z = 4.15) -- a 10% regression
   on a file that is 27.8% whitespace. Cause: **46% of twitter's whitespace runs
   are a single byte**, and an eight-byte load plus a SWAR test is far more
   expensive than the two compares it replaced. Fix: peel `WS_PEEL = 4` bytes
   scalar before reaching for the wide path, so a short run never touches it;
   and make the entry point `inline(always)` and tiny, so the common
   "no whitespace here at all" case costs one load and one test, as the original
   `peek()` did, with the run-skipping behind a call.

Neither was visible in output: both arms were byte-identical throughout.

#### Final measurement (quiet box, load 2%, 21 pairs, same binary, one env var)

`> 1` means the brick is faster. Null-arm floor this session: 0.983x-1.021x.

| file | column | original | brick | ratio | brick won |
|---|---|---:|---:|---:|---:|
| citm_catalog | scan | 1,565 MB/s | **2,105 MB/s** | **1.345x** | 21/21, z = 4.58 |
| citm_catalog | struct-parse | 1,171 MB/s | **1,461 MB/s** | **1.246x** | 21/21, z = 4.58 |
| citm_catalog | dom-parse | 664 MB/s | **722 MB/s** | **1.095x** | 21/21, z = 4.58 |
| canada | scan | 1,006 MB/s | **1,062 MB/s** | **1.053x** | 21/21, z = 4.58 |
| twitter | scan | 1,472 MB/s | **1,524 MB/s** | **1.042x** | 21/21, z = 4.58 |
| twitter | struct-parse | 791 MB/s | **813 MB/s** | **1.036x** | 18/21, z = 3.27 |
| twitter | dom-parse | 359 MB/s | 366 MB/s | 1.023x | 14/21, z = 1.53 |
| canada | struct-parse / dom-parse | 631 / 329 | 634 / 327 | 1.001x / 0.992x | at the floor |
| **stringify x4 (control)** | | | | **0.997x-1.008x** | **unmoved** |

`canada`'s 1.053x on scan is not a whitespace result -- the file has almost
none. It is the inlined entry point being cheaper than the original `peek()`,
which built a `Result<Option<u8>>` per call. So the restructure pays on
minified documents too, which is where most JSON on a wire actually lives.

**These figures are the whole whitespace campaign** (B1 + B1s against upstream's
original loop), measured in one binary with one environment variable between the
arms, so the two arms cannot differ by code layout.

#### M1 instruments landed alongside it

- **Allocation census** (`rjson-bench census`, `--features profile`): the table
  above. Deterministic, one run, immune to load; the counting wrapper taxes
  every allocation, so its method line self-flags as unquotable and CI asserts
  that flag appears.
- **`solo` + `tools/pinvs.ps1`**: process-level paired A/B, for anything that
  cannot be an in-process arm.
- **Differential soak** (`tests/soak.rs`): seeded generator, corpus mutator and
  number-token generator, all through the oracle. **900,000 cases, 0
  divergences, 2.81 s** at `RJT_SOAK=300000`; CI runs 300,000 per push.
- **Seven fuzz targets**, including `oracle_diff` (ours vs upstream on
  arbitrary bytes). **Wall, recorded so it is not re-diagnosed:** on
  windows-msvc `cargo fuzz run` dies `STATUS_DLL_NOT_FOUND` without the ASan
  runtime on PATH, and then `STATUS_ENTRYPOINT_NOT_FOUND` with the LLVM 22 one
  (`C:\Program Files\LLVM\lib\clang\22\lib\windows`) because it skews against
  what nightly's sanitizer expects; `--sanitizer=none` does not rescue it
  either, since libFuzzer's coverage needs the same runtime (`unresolved
  external symbol __start___sancov_pcs`). The CI `fuzz` job runs `cargo fuzz
  check` on Linux, and the soak covers the same ground everywhere. Fixing the
  Windows runtime is an open M1 item, not a blocker.

### 2026-09-09 -- M2-B4: eight digits per step (KEPT), four digits per step (REFUTED TWICE)

`canada.json` is **90.1% number bytes** (content census, above). Upstream walks
a number one byte at a time: `peek`, range-compare, `eat_char`,
`significand * 10 + digit`, `overflow!`. The brick takes eight digit bytes in
one 8-byte load, validates them in three integer ops, and folds them to a `u32`
with three multiplies.

The change is exact rather than approximate, which is what lets it be
byte-identical. `SAFE_8_DIGIT_SIGNIFICAND = (u64::MAX - 99_999_999) / 100_000_000`
is the largest significand for which eight more digits provably cannot overflow
a `u64`. Below that bound the per-digit `overflow!` check cannot fire, so the
chunk takes exactly the branch the byte-at-a-time loop would have taken --
including the digit at which a long number gives up and switches to the slow
float path. Above it, the loop falls back to bytes.

#### The eight-digit step: KEPT

Same binary, one environment variable between the arms (`RJT_NUM_WIDE=0/1`),
21 pairs, ABBA, pinned cpu2/High. Raw: `corpus/runs/m2-b4-digits.txt`.

| file | column | scalar | eight-at-a-time | ratio | wins, z |
|---|---|---:|---:|---:|---:|
| canada | struct-parse | 555 MB/s | **604 MB/s** | **1.083x** | 21/21, z = 4.58 |
| canada | dom-parse | 296 MB/s | **309 MB/s** | **1.043x** | 19/21, z = 3.71 |
| citm_catalog | struct-parse | 1,361 MB/s | **1,383 MB/s** | **1.017x** | 17/21, z = 2.84 |
| twitter | dom-parse | 357 MB/s | 359 MB/s | 1.008x | at the floor |
| citm_catalog | dom-parse | 689 MB/s | 694 MB/s | 1.006x | at the floor |
| **stringify x6 + scan x3 (control)** | | | | **0.999x-1.010x** | **unmoved** |

The gradient is the corpus census read back: `canada` is 90.1% number and moves
most, `citm_catalog` is 7.1% number and moves a little, `twitter` is 1.5% number
and does not move. Nine control cells that the change cannot reach stayed
inside 1%.

Hit rate, counted rather than assumed (`rjson-bench work`, `--features
turbo/profile`): **108,878 hits in 331,084 calls** on `canada`, a third. The
two-thirds that miss are the loop's exit test, which is the cost of finding the
end of a number and is charged to the winning arm above.

#### The four-digit step: REFUTED, twice, and the second time is the useful one

The obvious follow-on is a four-digit step for the remainder a chunked loop
leaves behind. It was built twice and reverted twice.

**First attempt, at both digit call sites.** `canada` dom-parse 0.967x and
struct-parse 0.964x against eight-only; `citm_catalog` flat.
Raw: `corpus/runs/m2-b4-eight-vs-eightfour.txt`. Against scalar, the combined
step measured `canada` struct-parse **1.035x where eight-only measured 1.083x**
-- it gave back more than half the win.

The first explanation was that the *integer* call site can essentially never
succeed: a short integer part is followed by `.`, `e`, `,` or `}`, so all
111,126 checks there are pure cost. That explanation predicted the fraction-only
placement would win.

**Second attempt, at the fraction call site only.** The prediction was wrong,
and being wrong is the finding. Counters first:

| `canada` dom-parse | four-digit step off | four-digit step on, fraction site only |
|---|---:|---:|
| `peek` calls | 1,323,297 | **879,361** (-33.5%) |
| four-digit hits / calls | 0 / 0 | **110,984 / 111,080 (99.91%)** |

The check hits essentially every time, exactly as predicted, and removes a third
of the file's `peek` calls. It is still slower.

Clock, 61 pairs, same binary, one environment variable, **two stringify controls
the change cannot reach**. Raw: `corpus/runs/2026-09-09-b4-wide4-probe3.txt`,
per-pair times in the `.csv` beside it.

| `canada` cell | median ratio | wins, z | best-of-N |
|---|---:|---:|---:|
| dom-parse (target) | **0.977x** | 51/61, z = +5.25 | **0.975x** |
| struct-parse (target) | 0.995x | 35/61, z = +1.15 | 1.022x |
| dom-stringify (control) | 1.015x | 25/61, z = -1.41 | 1.000x |
| struct-stringify (control) | 1.001x | 30/61, z = -0.13 | 0.997x |

Both statistics agree on dom-parse and both controls are flat on both. The
four-digit step is 2.5% slower on the file it was built for, while doing
demonstrably less work.

#### The law this bought

The fold has a **fixed cost that does not shrink with width**. Four digits of
the scalar loop are four dependent `significand * 10 + digit` steps on bytes
already in L1 behind a perfectly predicted branch -- roughly sixteen cycles of
dependency chain. The four-digit chunk is a load, a validate and a multiply fold
-- roughly the same. Eight digits are thirty-two cycles of scalar against that
same fixed fold, which is why the eight-digit step wins 1.083x and its
four-digit sibling gives half of that back.

**A chunk step pays only when the scalar work it replaces exceeds the fold's
fixed cost. Break-even on this core is above four digits.** That is why B1s'
whitespace scan peels four bytes scalar before going wide, and it is the same
number from the other direction. Two bricks, one boundary.

And the discipline: **removed work is not saved time**. 443,936 `peek` calls
removed, a 99.91% hit rate, and a slower parser. A counter proves an arm did
less. Only the clock decides whether it took less time, and only against a
control the change cannot touch.

#### What this session's runs cost, recorded so it is not re-paid

Two of the four runs above were **inadmissible and thrown away**, both for the
same reason.

- The 21-pair fraction-only run read `canada` dom-parse 0.968x -- and its
  `canada` dom-stringify **control** read 0.966x. Stringify does not parse
  numbers. When a control moves as far as the target, the run's floor is the
  control's movement and the run cannot resolve the claim. The box had been
  between 13% and 100% load for ten minutes.
- The first both-sites run had **no control cell at all**: every cell in it was
  a parse cell. Its shape looked clean only because `citm_catalog` happens to be
  number-poor.

The fix was an instrument, not patience -- this box does not go quiet on demand.
`tools/pinvs.ps1` now takes `-Csv` and prints a **best-of-N** table beside the
median of paired ratios. Noise can only inflate a timing sample, never deflate
one, so the minimum over N pairs is the least-contaminated estimate each arm has
of itself. On a machine that will not settle, run more pairs and read both
statistics: when they agree, and the controls are flat on both, the result
stands.
