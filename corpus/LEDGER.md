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

### 2026-09-09 -- M1-C: every remaining brick priced, and M2-B3: the escape writer (KEPT)

The M1 exit test asks for a ranked worklist with each brick's ceiling share and
the arithmetic behind it. This is that worklist. Every number below is a
deterministic count from `rjson-bench work` (library `--features profile`), not
a clock, so it is exact and reproducible on any machine under any load.

#### The instruments added to get it

- **`STR_SCANS` / `STR_BYTES` / `STR_BORROWS` / `STR_COPIES`** (brick B2): runs
  of the escape scanner, bytes it advanced over, and -- the number that
  matters -- how often the zero-copy borrow was kept versus lost to a scratch
  copy.
- **`ESC_BYTES` / `ESC_HITS` / `ESC_FRAGS` / `ESC_STEPS`** (brick B3): bytes the
  serializer's escape loop examined, how many actually needed escaping, clean
  runs handed to the writer, and steps the scanner took. `ESC_STEPS` is the
  counter that says whether a wide path is ENGAGED, which no output gate can
  say: a fast path that silently stopped being taken still produces the right
  answer.
- **`KEYS`** (brick B6): object keys the JSON side handed over.
- **`UTF8_BYTES` / `UTF8_CALLS`** (brick B12): the whole UTF-8 validation bill
  that `from_slice` pays and `from_str` does not.
- **`CountingWriter`** (brick B5), in the bench crate rather than the library:
  the serializer already writes through `io::Write`, so wrapping the sink
  counts every call at exactly the boundary B5 would change -- no library code
  touched, and therefore no chance of the instrument perturbing what it
  measures by adding a counter to a hot inner loop.

#### The census

| file | keys | str runs | str bytes | mean run | borrow / copy | utf8 bytes / calls |
|---|---:|---:|---:|---:|---:|---:|
| twitter | 13,345 | 19,327 | 366,689 | 19.0 B | 17,787 / 312 | 367,917 / 18,099 |
| citm_catalog | 25,869 | 26,606 | 221,377 | 8.3 B | 26,603 / 1 | 221,379 / 26,604 |
| canada | 8 | 12 | 90 | 7.5 B | 12 / 0 | 90 / 12 |

| file | column | esc bytes | esc hits | hit rate | sink calls | bytes/call |
|---|---|---:|---:|---:|---:|---:|
| twitter | dom-stringify | 367,917 | 1,228 | **0.334%** | 93,564 | 5.0 |
| twitter | struct-stringify | 373,965 | 1,228 | 0.328% | 97,128 | 4.9 |
| citm_catalog | dom-stringify | 221,379 | **2** | **0.0009%** | 189,201 | **2.6** |
| canada | dom-stringify | 90 | 0 | 0% | 334,397 | 6.2 |

#### What each brick is now worth, and why

**B2 (wide string scan) -- DEMOTED, close to closed.** Three independent counts
say so. The mean string run is **19.0 bytes** on twitter and **8.3** on
citm_catalog, so a 16- or 32-byte step has almost nothing to step over.
**98.3%** of twitter's strings and all but one of citm's already keep the
zero-copy borrow, so there is no copy to remove. And upstream is *already*
8-byte SWAR here (Mycroft's algorithm) with `memchr2` -- itself SIMD -- on the
non-validating path. B2 was written into the plan as though it were competing
with a byte-at-a-time loop. It is not. Do not open an `unsafe` island for it.

**B3 (escape writer) -- BUILT, and the numbers are below.** The serializer *was*
byte-at-a-time with a table lookup per byte, and the hit rate is **0.334%** on
twitter and **0.0009%** on citm_catalog. Practically all of that walking was
spent proving bytes were ordinary.

**B5 (sink specialisation) -- PROMOTED, against the plan's own gate.** The gate
read: count the calls per stringify first, and if the count is already near one
per token, do not build. It is **2.6 bytes per call** on citm_catalog --
189,201 sink calls for 25,869 keys, about **7.3 calls per key**. The serializer
is handing the writer a quote, a colon and a comma as separate calls. The
brick's claim is not "removes an allocation" (M1-A already showed stringify
allocates zero times) and now not "removes a copy" either -- the emitted-asm
census says `ser.rs` owns **zero** `mem*` calls. It must win on call count
alone, and the call count says there is room.

**B6 (key dispatch) -- unchanged, still needs its own probe.** 25,869 keys per
citm_catalog document and 13,345 per twitter. The count is now known; what is
still unknown is the comparison count per key, which lives in derive-generated
code that `--emit asm --lib` does not contain. Priced at M4, not here.

**B12 (UTF-8 validation) -- REFRAMED, and the obvious version is not
byte-identical.** `from_slice` validates **367,917 bytes in 18,099 separate
calls** on twitter, a mean of 20 bytes each, and 221,379 bytes in 26,604 calls
on citm_catalog, a mean of 8.3. The interesting part is not that the bytes are
many but that the *calls* are: `core`'s validator has a wide ASCII fast path
that barely gets started on a 20-byte slice. The tempting fix -- validate the
whole input once at entry, then take the unchecked path per string -- would be
one long vectorised pass instead of eighteen thousand short ones. **It also
changes behaviour**: a document with invalid UTF-8 in a region the parser never
reaches currently succeeds and would then fail. That breaks G1, so B12 stays
per-string, or becomes an opt-in feature, and the up-front form is recorded
here as rejected rather than pending.

#### Brick B3: the escape writer, eight bytes per step (KEPT)

The escape table is nonzero for exactly `0x00..=0x1F`, `"` and `\`. So the
predicate is `b < 0x20 || b == 0x22 || b == 0x5C`, and `b < 0x20` is *exactly*
`b & 0xE0 == 0` because 0x20 is a single bit. All three fall out of the exact
zero-byte test with no comparison and no approximation.

The B1s primitives moved into a new `src/swar.rs` so the one subtle function in
this crate has one definition, one set of tests, and is paid for once. Its
module docs carry the trap: the famous `(x - ONES) & !x & HIGHS` borrows across
lane boundaries and reports a byte as zero because its neighbour was.

**No scalar peel here, deliberately, and the contrast with B1s is the point.**
The whitespace scanner peels four bytes because 46% of twitter's whitespace runs
are a single byte, so an 8-byte load had to be avoided. Here the scan runs the
length of a whole string, so the entry cost is amortised over every byte and a
peel would only add a branch. Same technique, opposite tuning, because the
measured input shape is opposite. That is what the census is for.

**Engaged, deterministically** (`ESC_STEPS`, one per chunk wide, one per byte
scalar; identical `ESC_FRAGS` in both arms, so the fragment sequence is
unchanged):

| cell | scalar steps | wide steps | removed |
|---|---:|---:|---:|
| twitter dom-stringify | 367,917 | 101,382 | **-72.4%** |
| citm_catalog dom-stringify | 221,379 | 108,825 | **-50.8%** |

**Clock.** Same binary, one environment variable between the arms
(`RJT_ESC_WIDE=0/1`), 61 pairs, ABBA, pinned cpu2/High, on a **loaded** box --
so both the median of paired ratios and the best-of-N per-arm minimum are
given, and only cells where the two agree are claimed. **The controls are parse
cells**, which this change cannot reach; stringify cells were the controls for
every parse brick so far, and the roles simply swap.
Raw: `corpus/runs/2026-09-09-b3-escape-wide.txt` (+ `.csv`).

| file | column | scalar | wide | median | wins, z | best-of-N |
|---|---|---:|---:|---:|---:|---:|
| twitter | struct-stringify | 1,774 MB/s | **2,153 MB/s** | **1.208x** | 61/61, z = -7.81 | **1.207x** |
| twitter | dom-stringify | 1,837 MB/s | **2,202 MB/s** | **1.200x** | 60/61, z = -7.55 | **1.189x** |
| citm_catalog | struct-stringify | 1,870 MB/s | **2,007 MB/s** | **1.068x** | 61/61, z = -7.81 | **1.072x** |
| citm_catalog | dom-stringify | 1,261 MB/s | **1,306 MB/s** | **1.028x** | 57/81, z = -3.67 | **1.032x** |
| twitter | dom-parse (control) | 331 MB/s | 331 MB/s | 0.995x | 27/61 | 1.007x |
| citm_catalog | struct-parse (control) | 1,276 MB/s | 1,276 MB/s | 1.002x | 34/61 | 1.000x |

`citm_catalog` dom-stringify needed a **second probe** to be claimable at all.
On the 61-pair run its two statistics disagreed in sign -- median 1.028x against
best-of-N 0.980x -- and a cell whose statistics disagree is not a result. An
81-pair run on that cell alone brought them into agreement (1.028x and 1.032x),
and it is quoted from that run. Recorded because the rule earned its keep: when
the median and the best-of-N disagree, run more pairs; do not pick the one you
prefer.

The gradient is the census again. twitter is 57.1% string and gains 1.20x;
citm_catalog is 12.5% string and gains 1.03-1.07x; `canada` has 90 string bytes
in 2.25 MB and is not quoted because there is nothing there to win.

**The gate that earns the `unsafe` in this function.** `format_escaped_str_contents`
calls `str::from_utf8_unchecked` on the clean runs and
`hint::unreachable_unchecked` on a byte the table does not flag. So a mask that
flagged one byte too few would corrupt output, and one that flagged one byte too
many would be undefined behaviour. Four tests stand behind it:
`escape_mask_agrees_with_table` (all 256 byte values in all 8 lanes, mask
against table, exact equality), `wide_scan_matches_scalar` (>100,000 cases:
every byte value at every offset in every length 0-40, all-clean and all-escape
runs across the step boundary, and seeded random content over the interesting
alphabet), `non_ascii_is_never_flagged` (0x80-0xFF must never be flagged, or a
multi-byte sequence would be split), and
`the_gate_would_catch_an_off_by_one_range`, which poisons the mask with `0xC0`
instead of `0xE0` and asserts the gate notices. A suite that cannot fail is not
a gate.

#### The emitted-asm census (`docs/ASM-CENSUS.md`, `tools/asm-census.ps1`)

Whole-crate, per-source-file attribution via CodeView `.cv_loc` plus
`.cv_inline_site_id`, with the full inline-frame chain rebuilt so a bounds check
whose innermost frame is `core/src/slice/index.rs` counts against the file that
inlined it. `de.rs` + `ser.rs` + `read.rs` own 9,136 of 18,246 instructions.

- **Panic / bounds sites: 63.** `read.rs` 23, `ser.rs` 2, **`de.rs` 0**. That
  zero closes `get_unchecked` in `de.rs`: the plan said not to reach for it
  until the census found a real one, and it did not. It also **opens a safe
  brick**: three `read.rs` sites share one shape, `if self.index == self.slice.len()`
  followed by `self.slice[self.index]`. `==` does not tell LLVM `index < len`,
  so the bound is re-checked. A `<`-form check or `get()` folds it with no
  `unsafe`. 63 static sites in cold blocks can still be 0.0% of runtime, so
  this is an opened question, not a measured tax.
- **`mem*` calls: 46** -- 36 `memcpy`, 10 `memmove`, **0 `memset`**, every one a
  runtime length. `ser.rs` owns **zero**, which reprices B5 and B15 down
  independently of M1-A: there is no copy on the serialize path to remove.
  `read.rs`'s six are `extend_from_slice` on the scratch path, i.e. escaped
  strings only -- the borrowed fast path emits no copy at all, which is the
  `STR_BORROWS` counter confirmed from the other side.
- **Auto-vectorisation: none.** Seven packed instructions in the whole crate and
  neither kind is vector work (five are LLVM's u64-to-f64 idiom in number
  parsing, two are register moves). So B1v, B2, B3 and B12 compete with scalar
  code and none of their claims is pre-empted by the compiler. Baseline ISA is
  plain `x86-64`, so this is expected rather than surprising, and the landed
  8-byte SWAR scans correctly do not appear -- a control proving the census sees
  what it should.
- **Stated coverage gap:** `--emit asm --lib` contains only this crate's own
  instantiations, so the whole derive-generated struct-parse path is absent and
  the census prices **B6, B6h and B14 not at all**. Counts are also pre-LTO.

Three instrument bugs were found and fixed while building it, each of which had
already produced plausible-looking wrong numbers -- a panic-symbol regex that
silently missed 37 of the 63 sites, a PowerShell one-element array unrolling to
a bare string and mis-bucketing 4,373 instructions, and `Split-Path -Leaf`
rendering every `core` file as `mod.rs`. The tool now also holds the SHA-256
the compiler stamps into each `.cv_file` directive against the working tree, so
a stale report says `CHANGED since the build` instead of lying quietly.

### 2026-09-09 -- M1-D: S4 registered, and a corpus defect that invalidated the manifest

Two things here. The registration is the planned work. The defect was found on
the way, by counting rather than by reading, and it is the more important of
the two because it silently made CI and a developer measure different documents.

#### The defect: the corpus was not the same bytes on every platform

There was no root `.gitattributes`. With `core.autocrlf=true`, a checkout
rewrote every payload's line endings, and three things followed:

1. **`twitter.json` was 646,995 bytes on such a checkout and 631,514 in the
   repository** -- a 15,481-byte difference that is *entirely* carriage returns,
   every one of them counted as whitespace by the content census. So CI was
   benchmarking a document 2.4% smaller than a Windows developer was, and
   `citm_catalog.json` differed by 50,468 bytes (2.8%).
2. **`HASHES.txt` held the CRLF hashes**, so the integrity manifest was valid on
   Windows only. `corpus/fetch.ps1` re-downloads the payloads from the pinned
   upstream commit -- arriving with LF -- and verifies them against that
   manifest. The one script whose entire job is integrity **would have failed
   for anyone who ran it.**
3. **Nothing noticed, because CI never ran the verification.** That is the
   actual root cause. A manifest that is never checked is a comment.

**Verified exactly reversible before changing anything.** For all 66 corpus
files: no lone `CR` exists anywhere (every `CR` is immediately followed by `LF`,
so none is data inside a string -- a raw control character in a JSON string is
invalid JSON in any case), and stripping `CRLF` to `LF` reproduced **every git
blob byte-for-byte**. So the LF form is the canonical stored form and the
conversion is precisely the inverse of what the checkout did, rather than a
guess about which bytes were meant.

**Fixed:** `corpus/**/*.json -text` in a root `.gitattributes`, the seven
affected payloads normalised, `HASHES.txt` regenerated, and the manifest now
verified **in the three-OS test job** -- on ubuntu, windows and macos -- which
is what turns "platform-independent" from an intention into a checked claim.
All 66 files now agree across the manifest, the working tree and the git blob.

Two conformance cases changed bytes and were checked for intent, not just for
hash: `jsonchecker/fail27.json` is a literal newline inside a string and
`fail28.json` is a backslash followed by one. Both were `CRLF` and are now
`LF`; both still fail to parse, and for the same reason as before.

#### What moved, stated precisely

**No ratio moved.** Every paired measurement in this ledger has both arms
reading the same file, so `1.208x` is `1.208x` on either corpus. What moved is
absolute MB/s (a cell has 2.4% fewer bytes to chew) and every byte-percentage.

Re-censused with an **independent instrument** -- a hand-written tokeniser in
`python`, not a re-run of the Rust census, so the two cross-check each other:

| file | bytes (was) | whitespace | string | number | literal | structural | non-ASCII |
|---|---|---:|---:|---:|---:|---:|---:|
| twitter | **631,514** (646,995) | **26.07%** (27.8%) | 64.19% | 1.56% | 3.39% | 4.80% | **15.11%** (14.7%) |
| citm_catalog | **1,727,204** (1,777,672) | **71.03%** (71.9%) | 15.90% | **7.35%** (7.1%) | 0.29% | 5.43% | 0.02% |
| canada | **2,251,051** (2,251,060) | 0.00% | 0.01% | **90.08%** (90.1%) | 0.00% | 9.92% | 0.00% |

The string column needs its definition stated, because the two instruments
disagree by design and that disagreement is worth recording rather than
resolving in favour of whichever is prettier. This tokeniser counts a string
token **including its delimiters**; the Rust census counts string **content**,
which is what a scanner actually walks. For twitter the gap is 38,684 bytes
against 19,327 strings times two quotes = 38,654, so the two agree to within
the escape count -- which is the cross-check passing. Content-only, twitter is
**58.06%** string (366,689 of 631,514), against the 57.1% published on the
CRLF file.

`canada` is unaffected in substance: nine carriage returns in 2.25 MB.

#### The registration: S4 is now measurable and gated

- **`corpus::File` carries all ten documents.** The classic trio is frozen as
  `File::S1_S3`, and a timing verb's `--all` still means exactly those three in
  exactly that order, so a number quoted today stays comparable with one quoted
  before S4 existed. `File::EVERY` is both scenarios, and `all_documents()`
  iterates it -- which puts all seven house payloads under `tests/oracle.rs`'s
  byte-identical gate with no further work. Four tests keep the registration
  honest: every file loads and is non-empty, `parse` round-trips every file's
  own name, the two scenario constants partition `EVERY` exactly, and `S1_S3`
  is asserted frozen.
- **One file-to-fixture list.** `by_fixture!` is the only place a corpus file is
  mapped to its Rust type, and every dispatch site expands it. Before this
  there were three separate `match file` ladders; a document registered in one
  and forgotten in another would have reported a number for a cell nobody was
  running.
- **Seven fixture modules**, each carrying serde shapes S1-S3 never enter: a
  `flatten` tail over 25 distinct key sets, two internally-tagged enums, `Vec<u8>`
  byte arrays, `Option`-heavy configuration, and an 18-key field matcher over
  2,600 records. A fixture that modelled these as `Value` would compile,
  deserialize, and measure nothing new -- so each one is round-trip tested
  against the document it models, and the test asserts `Value` equality rather
  than byte equality, since key order legitimately differs through a struct.
- **A `latency` verb**, for the question `bench` cannot ask. "How long to handle
  one message" is not "how fast can we chew 600 KB", and the two can move in
  opposite directions when per-call setup dominates. S4 ships containers, so
  the records are sliced out and parsed one at a time.
  - The slicing uses the **oracle's** `RawValue`, deliberately not ours.
    `RawValue` returns the original bytes, whitespace included, which matters
    because `s4-media-probe.json` is 54.9% whitespace and re-serializing each
    record through `Value` would have quietly measured a minified document.
    Switching `raw_value` on for `turbo` would have compiled extra `cfg`-gated
    paths into the crate under measurement and moved every timing in the suite;
    Cargo features are additive, so a consumer could not have undone it either.
    The oracle is never timed, so slicing with its copy costs nothing.
  - The verb **measures its own timer overhead** in the same loop shape with
    the parse removed, and prints it, so a reader can subtract it instead of
    being told it is small. At message sizes of a few hundred bytes an
    `Instant::now()` pair is not negligible, which is exactly why it is
    reported rather than assumed.
  - Per-message **struct** parse is refused rather than faked: it needs a
    fixture for the element type, not the document type, and those do not
    exist. `--column struct-parse` errors out saying so. Silently measuring the
    container instead would be the failure the plan calls `m4=0`.
- **CI gates the new work**: the corpus manifest on three platforms; `python
  tools/gen-s4.py --check`, since S4 is generated rather than fetched and
  `HASHES.txt` therefore cannot cover it; both arms of the escape knob agreeing
  with upstream; and a counter assertion that the wide escape scan is still
  reached, on the same pattern as the whitespace knob's.

#### One more corpus correction, from the same counting

`corpus/README.md` credited `twitter.json` with "Unicode escapes". It has none.
Its 15.11% non-ASCII is all raw UTF-8; `citm_catalog.json` has two two-byte
escapes and `canada.json` none. **So the hex-escape decoder and the
surrogate-pair path were never on the timed corpus at all** -- they were
exercised only by the conformance set and the soak. `s4-ocr-i18n.json` puts
both under measurement for the first time, with 5,671 hex escapes and 1,032
surrogate pairs.

#### First measurements off S4, which is what the registration was for

Deterministic counts, `rjson-bench work`, library `--features profile`.

| file | column | keys | str runs | str bytes | mean run | borrow / copy | esc hit rate |
|---|---|---:|---:|---:|---:|---:|---:|
| s4-frame-telemetry | struct-parse | 46,816 | 49,420 | 176,964 | 3.6 B | 49,420 / 0 | -- |
| s4-sync-envelope | struct-parse | 13,182 | 19,529 | 122,584 | 6.3 B | 19,529 / 0 | -- |
| s4-media-probe | struct-parse | 11,806 | 17,946 | 169,812 | 9.5 B | 17,567 / 76 | -- |
| s4-ocr-i18n | dom-parse | 17,923 | 33,254 | 212,940 | 6.4 B | **21,438 / 1,371** | -- |
| s4-vault-shard | dom-parse | 80 | **136** | 438,791 | **3,227 B** | 136 / 0 | -- |
| s4-ocr-i18n | dom-stringify | -- | -- | -- | -- | -- | **2.73%** |

Three of these are regimes the classic trio simply does not contain, and each
one bears on a brick:

- **`s4-vault-shard` has a mean string run of 3,227 bytes**, against 19.0 on
  `twitter` and 8.3 on `citm_catalog` -- a 170x difference, from a single
  87,384-byte value. This is the long-string regime **brick B2 was written
  for**, and until now the corpus had no example of it. B2 stays demoted for
  S1-S3, where the strings are far too short to feed a wider step, but this
  file is where it must be re-priced before the question is closed. The same
  document is 136 scanner runs for 438,791 bytes; `twitter` is 19,327 runs for
  366,689.
- **`s4-ocr-i18n` has a 2.73% escape hit rate**, eight times `twitter`'s 0.334%
  and three thousand times `citm_catalog`'s 0.0009%. It is also the only corpus
  file that loses the zero-copy borrow at any rate worth measuring: **1,371
  copies against 21,438 borrows, 6.0%**, where `twitter` is 1.7% and
  `citm_catalog` is one copy in 26,604. Escaped strings force a scratch copy,
  so this is the file that prices the escape DECODER -- and per the emitted-asm
  census, the scratch path is also where `read.rs`'s six `memcpy` calls live.
  The corpus had no such file before.
- **`s4-frame-telemetry` is 46,816 keys over 3.6-byte mean strings** -- the
  highest key density in the corpus at 80.3 keys/KB, against `citm_catalog`'s
  15. This is the field-matcher workload for **B6 / B6d / B6h**, and it is the
  reason those bricks could not honestly be priced on S1-S3.

#### And the latency finding, which throughput could not have shown

`rjson-bench latency --all --messages 6000`, per-message `dom-parse`, records
sliced out of their containers with the oracle's `RawValue` so the original
bytes survive. Timer overhead measured at 0 ns against a **100 ns resolution**,
so every figure is quantised to 100 ns and the tool says so in its own output.

| file | msgs | mean B | p50 ns | p90 ns | p99 ns | implied MB/s |
|---|---:|---:|---:|---:|---:|---:|
| s4-frame-telemetry | 2,600 | 227 | 1,600 | 1,700 | 1,900 | 142 |
| s4-signin-batch | 600 | 781 | 4,100 | 7,000 | 20,000 | 190 |
| s4-sync-envelope | 280 | 757 | 6,700 | 10,700 | 16,700 | **113** |
| s4-media-probe | 75 | 3,189 | 23,400 | 29,300 | 48,800 | 136 |
| s4-ocr-i18n | 90 | 4,391 | 29,400 | 63,400 | 102,700 | 149 |
| s4-vault-shard | 4 | 87,623 | 12,000 | 12,900 | 19,000 | **7,302** |

**A 65x spread in bytes per second, driven by message size alone.** A 757-byte
sync entry runs at 113 MB/s; an 87 KB vault shard at 7,302 MB/s. Nothing about
the parser changes between those two rows -- what changes is how much per-call
setup each byte has to carry. Every throughput number this project has
published is from the right-hand end of that range, on documents of 631 KB and
up, and a house service handling 757-byte sync entries is operating at a
fiftieth of the advertised rate.

That is not a defect, it is a different question, and it is exactly the
question `bench` cannot ask. It also names the next brick class honestly: for
small messages the win is in per-call setup, not in any of the byte-at-a-time
kernels the campaign has been sharpening. `s4-node-config` exists in the corpus
for the same reason (key reuse 1.6, so nothing amortises).

`max` is reported and deliberately not quoted: it runs 30x to 100x above p99
because it is scheduler noise on a shared machine, not the parser. p99 is the
operating number.

#### Coverage this unlocked, stated as a count

The oracle's byte-identical gate now covers **73 corpus documents**, up from
66. The seven additions include the first hex escapes and the first surrogate
pairs the timed corpus has ever held, and they pass byte-identically to
upstream on the first run.

One upstream limitation was found and pinned down rather than worked around:
an internally-tagged enum with an `f64` field behind the tag **cannot** be
deserialised under `arbitrary_precision`, because the tag makes serde buffer
the content and a buffered number becomes a map under that feature.
`tests/ap_probe.rs` puts the minimal case through **both** crates and asserts
they fail identically, with the same message, and that the same field outside a
tag still works in both. So the fork's behaviour is correct -- identical
failure is part of a byte-identical contract -- and the affected fixture case
is skipped under that one feature with the reason recorded at the skip.

A second, smaller trap came from the same direction and is the day-one oracle
bug seen from the other side: under `arbitrary_precision` a `Number` keeps its
original digits, so `0.95920` in `s4-frame-telemetry.json` compares unequal to
the `0.9592` any `f64` field re-serialises. The value is identical; the
trailing zero is formatting. The fixture gate now compares such numbers by
parsed value while still treating an integer-versus-float difference as the
fixture bug it is.

### 2026-09-10 -- M2: B10 respecified by census, and B15/B5 refuted by the clock

Two bricks the plan had promoted, both settled here, neither the way the plan
expected. One was killed before it was built; the other was built, measured,
and reverted. Together they retire the whole "reduce allocation and call
counts" line of attack on the serializer, and redirect the DOM one.

#### B10: the census refutes the brick as specified, and names the real target

B10 was "build `Map` from a sorted `Vec<(String, Value)>` -- `BTreeMap` from
sorted input is O(n) -- instead of per-entry `insert`". The premise is true and
irrelevant, and one count says why.

**Object arity, measured over the whole corpus:**

| file | objects | keys | mean arity | median | <= 8 keys |
|---|---:|---:|---:|---:|---:|
| citm_catalog | 10,937 | 25,869 | 2.4 | **2** | **97.7%** |
| twitter | 1,264 | 13,345 | 10.6 | 4 | 71.8% |
| canada | 4 | 8 | 2.0 | 2 | 100% |
| s4-sync-envelope | 3,398 | 13,182 | 3.9 | 3 | 91.7% |
| s4-ocr-i18n | 3,367 | 17,923 | 5.3 | 4 | 98.2% |
| s4-frame-telemetry | 2,602 | 46,816 | 18.0 | 18 | 0% |

A Rust `BTreeMap` node holds **eleven** key-value pairs. At a median arity of
2, and with 97.7% of `citm_catalog`'s objects at eight keys or fewer, **the map
is a single node already**. A sorted-Vec bulk build would allocate a `Vec` per
object in order to save nothing, and would lose outright on the 97.7%. Not
built.

#### Where the allocations actually are

The allocation census gained a **size histogram**, because a total says
"allocation-dominated" and nothing about which allocations, and the brick
depends entirely on that.

| cell | allocs | <= 8 B | <= 16 B | 65-128 B | 513 B - 1 K |
|---|---:|---:|---:|---:|---:|
| citm_catalog dom-parse | 39,339 | **54.8%** | 9.5% | 4.5% | **27.9%** |
| twitter dom-parse | 20,834 | 30.0% | 26.7% | 4.1% | 12.4% |
| canada dom-parse | 56,061 | 0.0% | 0.0% | **100%** | 0.0% |
| s4-frame-telemetry dom-parse | 59,823 | **82.6%** | 0.0% | 4.3% | 13.0% |

Three separate populations, and they call for three different bricks:

1. **Short strings dominate the COUNT.** 54.8% of `citm_catalog`'s allocations
   are eight bytes or less; 82.6% of `s4-frame-telemetry`'s. These are object
   keys and short string values.
2. **`BTreeMap` nodes dominate the BYTES.** `citm_catalog`'s 10,978 allocations
   in the 513 B - 1 K bucket are one node per object, and at roughly 700 bytes
   each they account for essentially all of its 7.68 MB. **A two-key object is
   paying for an eleven-key node -- around 80% of every node is unused.**
3. **`canada` is neither.** 100% of its allocations are one `Vec<Value>` per
   coordinate array, 56,045 of them, with a 2.9% realloc rate. Already tight;
   the plan's "reserve `Vec` in `visit_seq`" has almost nothing to reclaim.

**Key reuse, which prices the interning idea:**

| file | keys | distinct | reuse | keys <= 8 B |
|---|---:|---:|---:|---:|
| citm_catalog | 25,869 | **321** | **80.6x** | 83.4% |
| twitter | 13,345 | **94** | **142.0x** | 34.1% |
| s4-frame-telemetry | 46,816 | **33** | **1,418.7x** | 100% |

`citm_catalog` allocates 25,869 `String`s for 321 distinct names.
`s4-frame-telemetry` allocates 46,816 for **thirty-three**.

So B10's real content is two changes, and **both need `Map`'s key type or
backing store to change**, which is a v1.x item and not a byte-identical M2
brick: intern the keys, and stop paying for an eleven-key node to hold two
entries. The plan already carries "arena `Value`, interned keys" at v1.x; what
this adds is the arithmetic that says how much is there.

#### B15 / B5: BUILT, MEASURED, REVERTED -- and the reason retires both

The counting sink said the serializer wrote **2.6 bytes per call** on
`citm_catalog`: 189,201 sink calls for 25,869 keys, about 7.3 per key. The
plan's own gate was "count the calls first, and do not build if it is already
near one per token". It was not, so B15 was built: fold a short escape-free
string's opening quote, contents and closing quote into ONE `write_all` through
a 64-byte stack buffer.

It worked, deterministically, exactly as designed:

| cell | sink calls, off | on | removed | bytes/call |
|---|---:|---:|---:|---:|
| twitter dom-stringify | 93,564 | 59,273 | **-36.6%** | 5.0 -> 7.9 |
| citm_catalog dom-stringify | 189,201 | 135,995 | **-28.1%** | 2.6 -> 3.7 |
| s4-frame-telemetry struct-stringify | 298,231 | 199,391 | **-33.1%** | 2.0 -> 3.0 |
| canada dom-stringify (control) | 334,397 | 334,373 | -0.0% | unchanged |

And it was **11 to 15 percent slower**. 61 pairs, same binary, one environment
variable, ABBA, pinned. Raw: `corpus/runs/2026-09-10-b15-string-fold.txt`.

| cell | median | wins, z | best-of-N |
|---|---:|---:|---:|
| citm_catalog struct-stringify | **0.848x** | 61/61, z = +7.81 | 0.901x |
| twitter dom-stringify | **0.895x** | 61/61, z = +7.81 | 0.888x |
| twitter struct-stringify | **0.895x** | 61/61, z = +7.81 | 0.889x |
| citm_catalog dom-stringify | **0.924x** | 61/61, z = +7.81 | 0.926x |
| twitter dom-parse (control) | 0.999x | 33/61 | 1.011x |
| canada dom-stringify (control) | 0.987x | 36/61 | 0.973x |

**The first explanation was wrong, and finding that out is the result.** The
obvious suspect was the 64-byte stack buffer's zeroing. So the buffer was cut
to 16 bytes and re-measured: `twitter` dom-stringify 0.913x, `citm_catalog`
struct-stringify 0.882x -- **the same loss**. The loss does not track the
buffer size, so the zeroing is not the mechanism. Raw:
`corpus/runs/2026-09-10-b15-fold16.txt`.

The mechanism is that **a sink "call" here is not a call**.
`writer.write_all(b"\"")` on a `Vec` is `extend_from_slice` with a
COMPILE-TIME length of one: a capacity check and a single store, which inlines
and constant-folds to a handful of instructions. Folding replaces two of those
with a runtime-length `copy_from_slice` into a buffer plus a runtime-length
copy out of it. That is strictly more work, and it puts `memcpy` calls into
`ser.rs` -- **the file the emitted-asm census reported as owning exactly zero
of them**. The census called this and the clock confirmed it from the other
side.

**So call count is the wrong metric for a sink whose calls inline.** That
retires B15 and B5 together, since B5's claim had already been narrowed at
M1-A and by the asm census to "call count alone" -- there are no allocations to
remove (stringify allocates zero) and no copies to remove (`ser.rs` owns no
`mem*`). All three of its possible claims are now measured and gone.

The plan's other half of B15 -- folding the constant separators, `,` then `"`
into `,"` and `"` then `:` into `":` -- survives the same reasoning but is
**priced below the measurement floor and not built alone**. It would fold about
40,000 of `citm_catalog`'s 189,201 calls, saving roughly one capacity check
each, on the order of two or three instructions out of eight, against a cell
that takes millions of cycles. That is well under 1%, which is where this
project's own rule says counters decide and the clock cannot, and where the
plan says sub-1% bricks are batched behind one switch rather than measured
individually.

#### The pattern across three refutations now

B4x removed 443,936 `peek` calls at a 99.91% hit rate and was 2.5% slower.
B15 removed 36.6% of sink calls and was 11% slower. In both cases a counter
proved the arm did strictly less, and the clock said it took more.

**A counter proves an arm did less work. Only the clock decides whether it took
less time.** What the counters are genuinely good for is the other question --
whether a fast path is plugged in at all, which no output gate can answer -- and
for pricing a brick *before* it is built, which is what killed B10 for the cost
of one histogram instead of a day's work.

### 2026-09-10 -- M3 opens with an instrument bug that would have built the wrong kernel

M3 is the SIMD island: SSE2 and AVX2 twins of the scalar kernels, behind
runtime dispatch. Before opening an `unsafe` crate for it, the question is
which kernel and at what width. The probe already had a table for exactly
that, and the table was wrong.

#### The impossible number

`rjson-bench probe` reported, for `citm_catalog`, that **92.4% of whitespace
bytes lie in runs of at least 32 bytes** -- in the same table that reported the
**longest run in the file is 29**. Both cannot be true.

The cause: `ws_bytes_in_runs_of_at_least(32)` answered by summing the bucketed
histogram from the `17-32` bucket upward. That bucket is mostly runs SHORTER
than 32, so the function reported ">= 17" under the label ">= 32", and every
threshold was shifted one bucket. The fix counts the thresholds exactly, from
the run lengths themselves, at census time.

**What it would have cost.** A 32-byte AVX2 whitespace kernel sized on the old
table would have been justified by "92.4% of the bytes are in runs long enough
to fill a step". The true figure is **0.0%** -- no whitespace run in the entire
corpus reaches 32 bytes. That kernel would have been written, tested, wrapped
in `unsafe`, and never once fully utilised.

Corrected, and self-consistent with the longest-run column:

| file | >=4 | >=8 | >=16 | >=32 | >=64 | longest |
|---|---:|---:|---:|---:|---:|---:|
| citm_catalog | 97.9% | 97.9% | **92.4%** | **0.0%** | 0.0% | 29 |
| twitter | 91.9% | 79.5% | **4.2%** | 0.0% | 0.0% | 21 |
| canada | 20.8% | 0.0% | 0.0% | 0.0% | 0.0% | 5 |

#### And a second instrument, because the first answers the wrong question

"How often is a step fully used" is not "how many steps get taken", and for
sizing a kernel it is the second that matters: **a wide load that overshoots
the end of a run still resolves it in one iteration**. A 29-byte run never
fills a 32-byte step and is still finished by one.

So the probe now also reports steps taken, per step size, after the shipped
4-byte scalar peel:

| file | 8 B (shipped) | 16 B (SSE2) | 32 B (AVX2) | gain vs 8 B |
|---|---:|---:|---:|---:|
| citm_catalog | 140,381 | 95,424 | **50,467** | 16 B 1.47x, **32 B 2.78x** |
| twitter | 16,252 | 15,864 | 15,476 | 16 B 1.02x, **32 B 1.05x** |
| canada | 1 | 1 | 1 | none |

Both tables are now kept, side by side, because they disagree and the
disagreement is the point: by the first, AVX2 looks worthless (0% of bytes fill
a step); by the second, AVX2 cuts `citm_catalog`'s wide-scan steps **2.78x**.
The second is the one that prices the brick.

#### What this sets up for M3

The whitespace kernel is a **`citm_catalog`-only win** and the corpus says so
from three directions: 92.4% of its whitespace is in runs of 16 to 29 bytes,
against `twitter`'s 4.2% and `canada`'s nothing. `twitter` gains 1.05x in steps
and should be treated as a control, not a target -- exactly the role it played
for B1s, where an 8-byte step on its 1-byte runs cost 10% before the peel was
added.

The remaining headroom is real but bounded. On the same run, `citm_catalog`
scan is 665,800 ns and the same document with skippable whitespace removed is
311,900 ns, so whitespace handling is **roughly half of what a scan still
costs** there, and 2.13x is the bound if it were free.

Recorded before the island is opened, so the first kernel is chosen on numbers
rather than on the appeal of the widest available register. And after three
refutations in a row -- B4x, B15, and B10 killed at the census -- the step
count is a price, not a promise: it says the arm will do less work, and only
the clock decides whether it takes less time.

### 2026-09-10 -- M3: the SIMD island lands, and the widest register loses

`rusty_json_turbo-accel` is the one crate in the workspace where `unsafe` is
allowed. It carries SSE2 and AVX2 twins of the two byte scanners, each written
against a scalar oracle that stays in the tree permanently and is reachable in
production through `RJT_ISA=scalar`. No `unsafe` for a vector load is written
in the parser, which is what lets the parser keep tightening toward
`forbid(unsafe_code)`.

No dependencies, no build script. That was a constraint, not a preference: the
crate advertises an empty dependency tree.

#### The headline, and it is not the one the hardware suggests

**SSE2 ships. AVX2 does not, and the reason is measured.**

| cell | SSE2 vs SWAR | AVX2 vs SWAR |
|---|---:|---:|
| `citm_catalog` scan | **1.107x** (61/61) | 1.099x (61/61) |
| `citm_catalog` struct-parse | **1.074x** (61/61) | 1.072x (61/61) |
| `twitter` scan | **1.020x** | **0.992x**, best-of-N **0.958x** |
| `canada` scan | 1.009x | 0.997x, best-of-N 0.976x |

AVX2 beat the 8-byte baseline on one file and lost on the other two. SSE2 beat
it on **every** cell. The cause is the run-length census taken before either
was written: **the longest whitespace run anywhere in the corpus is 29 bytes**,
so a 32-byte step is never fully used, while its costs -- a call that cannot
inline, a wider tail, `vzeroupper` on exit -- are paid on every run. A 16-byte
step is fully used by `citm_catalog`'s 17-to-29-byte runs and by `twitter`'s
9-to-16-byte ones.

AVX2 stays in the tree and stays reachable, so this can be re-measured on a
machine or a corpus with longer runs rather than deleted on one box's answer.

#### The shipped result

Same binary, one environment variable between the arms, 61 pairs, ABBA, pinned.
`RJT_ISA=swar` is the pre-M3 8-byte path; `sse2` is what now ships. Raw:
`corpus/runs/2026-09-10-m3-shipped-sse2.txt` and `-m3-escape-guarded.txt`.

**Whitespace scanner:**

| file | column | before | after | ratio | wins, z | best-of-N |
|---|---|---:|---:|---:|---:|---:|
| citm_catalog | scan | 2,075 MB/s | **2,297 MB/s** | **1.105x** | 61/61, z = -7.81 | 1.115x |
| s4-media-probe | scan | 1,704 MB/s | **1,873 MB/s** | **1.094x** | 60/61, z = -7.55 | 1.060x |
| citm_catalog | struct-parse | 1,321 MB/s | **1,419 MB/s** | **1.075x** | 61/61, z = -7.81 | 1.020x |
| s4-media-probe | dom-parse | 386 MB/s | **408 MB/s** | **1.051x** | 50/61, z = -4.99 | 1.106x |
| twitter | scan | 1,529 MB/s | 1,559 MB/s | 1.023x | 54/61 | 1.001x |
| citm_catalog | dom-parse | 480 MB/s | 487 MB/s | 1.016x | 44/61 | 1.034x |
| canada | scan (control) | 1,019 MB/s | 1,026 MB/s | 1.002x | 33/61 | 1.011x |
| canada | dom-stringify (control) | 1,032 MB/s | 1,037 MB/s | 1.006x | 37/61 | 0.987x |

**Escape scanner:**

| file | column | before | after | ratio | wins, z | best-of-N |
|---|---|---:|---:|---:|---:|---:|
| twitter | struct-stringify | 2,056 MB/s | **2,165 MB/s** | **1.062x** | 60/61, z = -7.55 | 1.062x |
| twitter | dom-stringify | 1,894 MB/s | **2,012 MB/s** | **1.060x** | 61/61, z = -7.81 | 1.063x |
| citm_catalog | struct-stringify | 1,870 MB/s | **1,964 MB/s** | **1.053x** | 59/61, z = -7.30 | 1.055x |
| citm_catalog | dom-stringify | 1,199 MB/s | **1,236 MB/s** | **1.032x** | 58/61, z = -7.04 | 1.051x |
| s4-ocr-i18n | dom-stringify | 765 MB/s | 784 MB/s | 1.028x | 51/61 | 1.031x |
| twitter | dom-parse (control) | 375 MB/s | 376 MB/s | 1.001x | 30/61 | 0.989x |

Every target gains, both controls are flat, and nothing anywhere is below
0.987x. These stack on top of the earlier bricks rather than replacing them:
this is the wide scan against the 8-byte SWAR that B1s and B3 already landed.

#### Two things were built, measured, and thrown away on the way

**An escalation gate for the whitespace scanner: REVERTED.** The idea was to
take one inline SWAR step before calling any vector kernel, since a
`#[target_feature]` function cannot be inlined and so pays call and setup cost
a two-byte run cannot repay. It cost `citm_catalog` half its win -- scan fell
from 1.099x to 1.046x, struct-parse from 1.072x to 1.016x -- and did **not**
recover `twitter`, which stayed at 0.978x. The theory was wrong.

Reading the run lengths afterwards says why it could not have worked. After
the parser's own 4-byte peel, `twitter`'s 12,118 runs of 9-16 bytes have 5 to
12 bytes left, so an 8-byte step often does not finish them and the vector call
happened anyway -- the gate added a step without avoiding anything. And
`citm_catalog`'s runs of 17-32 have 13 to 28 left, which one wide step finishes
outright, so there the gate was pure added cost.

**A length guard for the escape scanner: KEPT, and it is a different thing.**
Before it, `citm_catalog` struct-stringify measured **0.970x** on the best-of-N
statistic -- right at this project's own 0.97x floor -- because its mean string
is 8.3 bytes, shorter than one vector step, so the kernel's loop condition
failed immediately and the call was pure cost. The guard declines to make a
call that provably cannot pay:

| cell | before guard | after guard |
|---|---:|---:|
| citm_catalog struct-stringify | 0.993x, best-of-N **0.970x** | **1.053x**, best-of-N 1.055x |
| citm_catalog dom-stringify | -- | **1.032x**, best-of-N 1.051x |
| twitter struct-stringify | 1.080x | 1.062x |

The distinction matters and is the reusable part: the failed gate added a
**scan step** that the vector call then repeated; the guard adds a **length
compare** and removes a call. One does more work to avoid work; the other just
declines.

#### The bug the twin test caught on its first run

The escalation gate's `swar_step` returned `Option<usize>`, collapsing "fewer
than eight bytes remain" and "all eight were whitespace" into one `None`. The
caller then advanced by eight over bytes that had never been examined, and a
one-byte buffer came back with the wrong index.

It was caught immediately, and only because the **dispatcher** had just been
added to the twin table alongside the kernels. Testing the kernels alone would
have missed it entirely: every kernel was correct, and the composition was not.
`SwarStep` is now a three-way enum so the two cases cannot be confused again.

#### What the island is gated by

- **Twins against the oracle** over every one of the 256 byte values at every
  offset in buffers spanning the 8, 16 and 32-byte boundaries, uniform runs of
  every length 0-70 across all three boundaries, and seeded mixed content --
  more than 500,000 assertions per scanner, asserted to be more than 500,000 so
  the spread cannot quietly thin out.
- **The two bytes a careless range test swallows**: `0x0B` and `0x0C` are not
  JSON whitespace, and a `b <= 0x20` test would take them.
- **The signedness trap**, which is why `b < 0x20` is written `b & 0xE0 == 0`
  everywhere: SSE2 and AVX2 byte compares are SIGNED, so a naive `cmplt`
  against `0x20` flags every byte from `0x80` up and splits UTF-8 sequences.
- **A poison test** asserting the suite would catch exactly those two mistakes,
  because a suite that cannot fail is not a gate.
- **The byte-identical contract at every rung**: oracle, soak and the S4
  fixtures all green under `RJT_ISA=scalar`, `swar`, `sse2` and `avx2`.
- **Cross-target**: the island compiles for `wasm32-unknown-unknown`,
  `aarch64-unknown-linux-musl` and `x86_64-unknown-linux-gnu` with
  `--no-default-features`, falling back to SWAR where there is no x86.

#### One release-ordering consequence, recorded so it is not rediscovered

`rusty_json_turbo` now depends on `rusty_json_turbo-accel`, and `cargo package`
resolves every dependency against the crates.io index **even with
`--no-verify`**. So **the island must be published before the next release of
the parent**, or the release will fail at the packaging step rather than at the
upload.

CI now packages the two leaf crates in full and checks the parent's manifest
and file list instead, asserting that the crate would ship `src/lib.rs`,
`src/read.rs`, `src/ser.rs` and `README.md`, and would NOT ship the corpus or
any build output. The full check comes back the moment the island is on
crates.io.

#### One instrument note

Every method line now carries `isa=<rung> (machine offers <ceiling>)`. The two
are printed separately on purpose: a run narrowed by an override and a run on a
machine with nothing wider produce the same number for very different reasons,
and a ledger row that cannot tell them apart is not evidence.

### 2026-09-10 -- M4: the B6 ceiling probe, and three bricks it retires

M4 is the milestone where the serde fork is supposed to earn its keep, and its
headline speed work was B6 (match object keys on the JSON side) and B6h (hand a
field INDEX across the serde seam instead of a string). Both were gated on one
measurement the plan spelled out: stub key matching with an index lookup and
see what comes back. That probe is now built and run, and the answer is that
there is nothing there.

#### The probe

`b6_probe::FrameTelemetryFast` has the same fields, the same types and the same
wire names as the derived fixture. The only difference is how a key becomes a
field: the derive emits `match __value { "n" => ..., "pts" => ... }`, which
lowers to a length switch and a chain of `memcmp`; the probe dispatches on the
length and one or two individual bytes with **no string comparison anywhere**.

The eighteen field names separate perfectly that way, which is what makes this
a true ceiling rather than merely a faster matcher:

```text
len 1  n t                       -> byte 0
len 2  qp ms                     -> byte 0
len 3  pts dts cpb ref           -> byte 0
len 4  bits ssim satd mb_i mb_p mb_b
                                 -> byte 0, then byte 1 for s*, byte 3 for mb_*
len 6  psnr_y psnr_u psnr_v      -> byte 5
len 7  mb_skip                   -> unique
```

`s4-frame-telemetry.json` is the right document and deliberately the most
favourable one in the corpus: 2,600 objects sharing one exact 18-key tuple,
**46,816 keys at 78.4 keys/KB**, 33 distinct names reused 1,418 times each. If
key dispatch cannot be shown to matter here it cannot matter anywhere.

Work parity is asserted field by field over all 2,600 frames before anything is
timed, so the two arms provably build the same values.

#### The result: the ceiling is zero

In-process paired A/B, one binary, ABBA with the lead alternated, 41 pairs,
250 ms windows, pinned. Raw: `corpus/runs/2026-09-10-m4-b6-ceiling.txt`.

| arm | median | MB/s | best | best MB/s |
|---|---:|---:|---:|---:|
| derive (`match` on `&str`) | 1,605,500 ns | 372 | 1,526,800 ns | 391 |
| free dispatch (**ceiling**) | 1,613,400 ns | 370 | 1,574,500 ns | 379 |

**derive / free = 1.006x, 29/41 pairs, z = +2.65, best-of-N 0.970x.**

The two statistics disagree in sign, which by this project's own rule means the
cell is unresolved rather than a small win -- and an unresolved cell at 0.6% is
indistinguishable from nothing. Making key dispatch **completely free** on the
most key-dense document in the corpus buys, at most, six tenths of one percent.

#### What that retires

- **B6 (key dispatch on the JSON side): REFUTED.** There is no headroom to
  claim.
- **B6h (field-index handshake across the serde seam): REFUTED, and this is the
  one that matters.** The plan said it "lands only if it clears the floor on S1
  AND S4". There is no floor to clear. It was also the most invasive change
  contemplated anywhere in this project -- a hidden `__visit_field_index`
  method, a per-`FIELDS` lookup built once per `deserialize_struct`, and a
  fallback path for aliases and unknown keys, spanning both forks. All of that
  is now unnecessary, on evidence, before any of it was written.
- **S1 is bounded by the same number without a second probe.** `twitter` has
  13,345 keys in 631,514 bytes, **21.1 keys/KB**, against frame-telemetry's
  78.4 -- 3.7x less key-dense. A ceiling of 0.6% on the dense file bounds the
  sparse one below 0.2%.

#### Why it is zero, which is worth knowing before the next such idea

Rust's `match` on `&str` is not a linear string search. It lowers to a switch
on length and then, within a length class, comparisons on the bytes -- for keys
of one to seven bytes those are a couple of instructions each. **The derive is
already close to the cheapest dispatch the field set admits.** 46,816 keys at a
handful of cycles each is on the order of a quarter of a million cycles against
a parse that takes about five million, and the probe removes only part of that.

The general form, and it is the same shape as three earlier refutations in this
ledger: a step that looks like a lot of work because it happens a lot of times
can still be a small fraction of the total, and only measurement distinguishes
those two. B4x removed 443,936 `peek` calls and was slower. B15 removed 36.6%
of sink calls and was 11% slower. B10 died at the census. B6h dies at the
ceiling probe, having cost one afternoon instead of a fork of `serde_derive`.

#### The assembly says the same thing, which makes it three probes

The emitted-asm census was pointed at the BENCH library this time -- the one
that actually contains derive-generated code, the coverage gap the M1-C census
recorded and could not close. Across **111 `__FieldVisitor::visit_str` bodies**:

| | count |
|---|---:|
| instructions, all 111 bodies | 10,757 |
| calls | 54 |
| **of which `memcmp` / `bcmp`** | **20** |
| inline compare / test instructions | 477 |
| jump tables | 0 |

**Twenty `memcmp` calls in total, and most matchers have none.** The largest --
`twitter`'s 40-field `User` -- is 412 instructions with 6 `memcmp` and 30 inline
compares. `rustc` is not emitting a linear string search: it lowers `match &str`
into a switch on length and then INLINE byte and word comparisons, reaching for
`memcmp` only on a few longer keys.

So the derive already emits close to the cheapest dispatch the field set
admits. The clock said the ceiling was 0.6%; the instruction count says why.

#### B14 (`deserialize_in_place` for `Vec<Struct>`): REFUTED, and by a test

B14 asks whether the derive's in-place path pays. It cannot pay if nothing
calls it, and that is a question about reachability rather than speed -- so it
is answered by `tests/b14_reachability.rs` rather than by a benchmark.

`Deserialize::deserialize_in_place` has a DEFAULT body, so an impl that is
never called looks exactly like one that is. The test gives a type whose
in-place impl **panics**, then parses it through every entry point this crate
has: `from_slice`, `from_str`, `from_reader`, and `Deserializer::into_iter`.
None of them reaches it.

That is decisive by construction: `from_slice` calls `T::deserialize`, and
there is **no public function on this crate that deserializes INTO an existing
value**. Enabling the feature can therefore only add generated code and never
remove work. Turning it on and re-measuring `llvm-lines` confirmed the
direction with nothing to show for it: 1,159,484 against 1,159,476, +8 lines.

B14 becomes worth re-measuring the day a reuse entry point exists -- a
`from_slice_into(&mut T)` or similar -- which is a NEW PUBLIC API and belongs
with the streaming work at S5, not here. The test asserts the entry-point list
so it fails loudly if that day arrives.

#### B6d (one shared identifier matcher): measured, and not worth a fork alone

B6d is the one M4 brick that was never a speed claim. The plan files it under
code size, with the exit test "S9 `llvm-lines` <= upstream".

Measured: the derive's identifier code is **58,416 llvm lines, 5.04%** of the
bench library.

| generated method | llvm lines |
|---|---:|
| `deserialize_identifier` plumbing, per `__FieldVisitor` | 25,550 |
| `visit_str` | 15,578 |
| **`visit_bytes`** | **13,158** |
| `visit_u64` | 1,706 |
| `visit_borrowed_str` | 710 |
| `expecting` | 234 |
| `visit_borrowed_bytes` | 92 |

`visit_str` and `visit_bytes` are near-duplicates of each other -- the same
match, once on `&str` and once on `&[u8]` -- and **serde_json never calls
`visit_bytes` at all**. Collapsing the four into one shared matcher plus thin
wrappers would remove on the order of 13,500 llvm lines, about **1.2% of the
crate**.

That is real but it is a compile-time figure, not a runtime one: `llvm-lines`
counts what is submitted to LLVM, before dead-code elimination, so the binary
effect is smaller again. And the asm count above puts the whole population at
10,757 instructions.

#### M4's verdict: the serde fork does NOT earn its keep

The milestone was written as a conditional -- "the serde fork earns its keep" --
and the answer is no, on evidence, with all three of its bricks settled:

| brick | verdict | evidence |
|---|---|---|
| B6 (JSON-side key dispatch) | **REFUTED** | ceiling 1.006x median / 0.970x best-of-N at 41 pairs |
| B6h (field-index handshake across the seam) | **REFUTED** | no floor to clear; the most invasive change in the plan, avoided |
| B14 (`deserialize_in_place`) | **REFUTED** | unreachable through every entry point, proven by test |
| B6d (shared matcher) | **not worth a fork alone** | 1.2% of llvm lines, pre-DCE, no runtime claim |

So the fork stays exactly where it is: `Remade-With-Rust/serde` branch `turbo`,
unmodified at `a874a1b`, consumed through `[patch.crates-io]` and carrying
**zero divergences**. Keeping the patch wired costs nothing and means a future
finding can be acted on the same day it appears; forking `serde_derive` for a
1.2% compile-time figure would mean tracking upstream and keeping serde's own
478-test suite plus 118 ui tests plus Miri green, permanently, for no measured
speed at all.

The fork's baseline is recorded so any future change has a gate to clear:
**478 passed, 0 failed, 5 ignored** on `cargo test --workspace` at `a874a1b`.

What this milestone bought is the three refutations, and the largest of them
never had to be written. B6h spanned both forks -- a hidden
`__visit_field_index`, a per-`FIELDS` lookup built once per
`deserialize_struct`, an alias and unknown-key fallback -- and one afternoon's
probe retired it. That is the milestone working as intended rather than failing.

### 2026-09-10 -- M5: the B7 ceiling, and a documentation hazard found measuring it

M5 is the campaign to G2, and its biggest listed brick is B7: replace `IoRead`'s
per-byte path with a buffer that reuses the `SliceRead` scanners. The plan names
the ceiling exactly -- `from_reader` against `from_slice` of the same bytes --
so that is what was measured first.

#### What `from_reader` costs today

`IoRead` wraps `reader.bytes()`, an iterator yielding one `io::Result<u8>` per
byte, wrapped again in a `LineColIterator` that does line and column
bookkeeping on every one. Its string path pushes every byte into scratch, so
unlike `SliceRead` there is no borrowed fast path at all.

Three arms in one process, leading arm rotated per round, min-per-iteration
statistic, work parity asserted (all three build an equal `Value`) before
timing. 15 pairs. `rjson-bench b7`.

| file | `from_slice` | `from_reader` over `&[u8]` | `from_reader` over `BufReader` | ceiling |
|---|---:|---:|---:|---:|
| citm_catalog | **646 MB/s** | 412 MB/s | 375 MB/s | **1.55x** |
| twitter | **369 MB/s** | 289 MB/s | 248 MB/s | **1.28x** |
| canada | 273 MB/s | 269 MB/s | 320 MB/s | 1.10x (range [0.79, 1.24]) |
| s4-frame-telemetry | 146 MB/s | 140 MB/s | 141 MB/s | 1.02x |

**The ceiling tracks whitespace, and that is the whole story.** `citm_catalog`
is 71.0% whitespace and shows 1.55x; `twitter` is 26.1% and shows 1.28x;
`canada` has 33 whitespace bytes in 2.25 MB and `s4-frame-telemetry` is
minified, and both show nothing.

The reason is structural: every whitespace brick this project has landed --
B1, B1s and the M3 SIMD island -- works on a SLICE. `IoRead` has no slice, so
none of them applies, and every whitespace byte goes back through the per-byte
iterator. The reader path is still running the 2026-09-09 parser.

#### The gap decomposes, which decides how B7 has to be built

Running the same probe with the whitespace fast path switched off
(`RJT_WS_FASTPATH=0`) separates the two causes on `citm_catalog`:

| arm | ns | |
|---|---:|---|
| `from_slice`, whitespace bricks on | 2,983,600 | what ships |
| `from_slice`, whitespace bricks off | 4,218,300 | the bricks are worth **1.41x** here |
| `from_reader` | 4,637,900 | only **1.10x** behind slice-without-bricks |

So of the 1.55x ceiling, roughly **1.41x is the missing slice scanners and only
1.10x is the per-byte iterator**. That rules out the cheap version of B7: simply
buffering the bytes while keeping the byte-at-a-time protocol would recover the
smaller share. To get the ceiling, the buffer has to be a slice the existing
scanners run over -- which is what the plan specified, and now there is a number
saying why.

#### A documentation hazard, found on the way

`IoRead::new`'s own documentation says: *"you will want to apply your own
buffering because serde_json will not buffer the input. See
`std::io::BufReader`."*

Measured, a `BufReader` makes an in-memory reader **slower, not faster**:
`citm_catalog` 412 -> 375 MB/s, `twitter` 289 -> 248 MB/s. It adds a second
per-byte layer on top of one that is already the bottleneck, and there are no
syscalls for it to amortise.

The advice is right for a `File`, where without it `bytes()` means one `read`
syscall per byte, and catastrophically so. It is wrong for anything already in
memory, and the documentation does not distinguish the two cases. That is worth
fixing regardless of whether B7 is ever built, because it is advice this crate
gives its users today.

#### S6's gate, answered: the cold paths agree, at every rung

S6's exit test is not a speed bar. It is "cold paths stay correct and do not
regress; error line/col identity" -- and after four bricks and a SIMD island
have gone through the scanners, that is the question worth asking.

The timed corpus cannot answer it. S1-S3 contain **no hex escape, no surrogate
pair, no nesting past a handful of levels, no 400-digit float and no integer
near the `u64` boundary**. Those are exactly the paths where a byte-identical
contract breaks without anything noticing.

`tests/s6_cold_paths.rs` puts 130-odd cases through BOTH crates and compares
everything the contract covers: whether it parsed, the printed bytes, and for a
failure the error text WITH its line and column.

| group | what it covers |
|---|---|
| deep nesting | arrays and objects at 1, 2, 31, 32, 33, 63, 64, 100, 126, 127, **128**, 129, 200, 500 -- straddling serde_json's default 128-frame recursion limit, so the limit itself must match |
| escape-heavy strings | an escape every seven bytes over 20,000 groups, all eight two-character escapes |
| hex escapes | 3,500 BMP escapes across the 1, 2 and 3-byte UTF-8 boundaries |
| surrogate pairs | 2,000 well-formed pairs, plus lone high, lone low, high-then-high, high-then-bad-escape, truncated hex, non-hex digits, high-at-EOF |
| number boundaries | `u64::MAX` and +1, `i64::MIN` and -1, `-0`, `-0.0`, `1e308`, `1e309`, `1e-400`, 20-digit exponents both signs, and 400-digit fraction / integer / with-exponent |
| duplicate keys | last-wins, and the same key 5,000 times |
| structural churn | nested empties, 5,000 empty arrays, 5,000 empty objects |
| **error positions** | 28 malformed inputs including newline, tab and CRLF line counting, a 200-byte whitespace run before the error, and mixed whitespace before the error |

**Result: no divergence. 7 suites, 0 failures, at all four `RJT_ISA` rungs
(`scalar`, `swar`, `sse2`, `avx2`) and with the whitespace fast path both on
and off.**

The error-position group is the one that earns its keep here. A wide whitespace
scanner that miscounts lines is **invisible** until an error has to name one --
the output is byte-identical on every valid document, so the oracle and the
soak would both pass. The "200-byte whitespace run then error" and "mixed
whitespace then error" cases exist specifically to catch that, and they are run
at every rung because the rung is what changes the scanner. CI now does the
same.

This is the first thing in the project measured against S6's actual bar, and
the honest reading is that it found nothing -- which for a correctness gate is
the result you want, and is worth far more than the same suite would be if it
had never been run.

#### Brick B7 LANDED: the reader path gets a window, and the scanners with it

`IoRead` no longer wraps `reader.bytes()`. It holds an 16 KiB window, reads
into it in 8 KiB blocks, and -- the point -- hands that window to the same wide
whitespace scanner the slice path uses. Line and column are counted with
`memchr` when an error asks, not on every byte.

**The gap to `from_slice`, which B7 cannot affect and which therefore serves as
the in-run control.** 31 pairs, three arms, leading arm rotated, work parity
asserted before timing.

| file | gap before B7 | gap after | reader gained |
|---|---:|---:|---:|
| citm_catalog | 1.55x | **1.22x** | **1.27x** |
| twitter | 1.28x | **1.14x** | **1.12x** |
| canada | 1.10x | **1.08x** | 1.02x |
| s4-frame-telemetry | 1.02x | 1.03x | neutral |

Nothing regressed. The gradient is the whitespace census again: `citm_catalog`
at 71.0% gains most, `canada` with 33 whitespace bytes in 2.25 MB gains least.

**And `BufReader` is now clearly the wrong thing to do**, where before it was
merely unhelpful: `twitter` reads 367 MB/s through a bare `&[u8]` and 298
through a `BufReader`, because it layers a second buffer over the one this
reader now keeps itself. The documentation on `IoRead::new` was rewritten
twice in one session for this -- first to split the file case from the
in-memory case, then again once the buffer landed and made the advice obsolete.

#### Getting there took four measured attempts, and three of them were losses

This is worth recording in full, because each failure was a different lesson
and the first three all looked like progress.

| attempt | citm | twitter | canada | frame-tel | verdict |
|---|---:|---:|---:|---:|---|
| before B7 | 1.55x | 1.28x | 1.10x | 1.02x | the baseline |
| 1. buffer only | 1.69x | **1.73x** | 1.15x | 1.12x | **worse everywhere** |
| 2. + wide whitespace scan | 1.32x | 1.27x | **1.32x** | — | citm gains, canada regresses |
| 3. + register lookahead | 1.27x | 1.24x | 1.16x | 1.10x | closer, two still behind |
| 4. + tiny entry | **1.22x** | **1.14x** | **1.08x** | 1.03x | **kept** |

1. **A buffer alone made every file worse.** Predictable in hindsight and
   predicted by the decomposition -- of the 1.55x, only 1.10x was the iterator
   and 1.41x was the missing scanners -- so paying the buffer's cost without
   using it as a slice was all cost. It also zeroed 8 KiB per refill via
   `resize`, as much zeroing as parsing.
2. **The scanner fixed `citm_catalog` and broke `canada`.** The whitespace-free
   file gains nothing from a whitespace scanner and pays the buffer for it.
3. **Serving `peek` from the buffer was the real cost.** A length compare, a
   bounds-checked index and a store on every call, and `canada.json` makes 2.2
   million of them. Upstream held the peeked byte in an `Option` in a register;
   putting that back, ON TOP of the buffer, recovered most of it. The hot path
   should stay the shape upstream tuned it to; the buffer belongs underneath.
4. **The refill branch was polluting the hot path**, until `fill` was marked
   `#[cold]` `#[inline(never)]` -- it happens once per 8 KiB and was sitting in
   a path taken once per byte. And the scanner still needed **a tiny entry**:
   on a minified document there is no whitespace at all, so every call was
   dispatching on the ISA to find its answer at offset zero.

**That tiny entry is the third time this project has needed the same thing.**
B1s needed a 4-byte scalar peel because 46% of `twitter`'s whitespace runs are
one byte. M3's escape scanner needed a length guard because `citm_catalog`'s
mean string is 8.3 bytes, shorter than one vector step. Now the reader's
whitespace scan needs a one-byte check because minified documents have no
whitespace to scan.

**The law: a wide scanner must not be reached until something cheap has ruled
out the common case.** Three bricks, three widths, one shape. It is now the
first thing to write, not the fix applied after a regression.

#### What held it honest

The contract here is unusually strict -- error line and column, byte offsets,
and `RawValue` bytes are all observable -- and two gates caught real bugs:

- **`raw_value` broke immediately.** Skipping a whitespace run wholesale
  skipped feeding those bytes to the raw-value buffer, which `discard` used to
  do one byte at a time, so `{"foo": 2}` came back as `{"foo":2}`. Upstream's
  own `test_boxed_raw_value` found it on the first run.
- **A reader that hands over ONE BYTE PER `read`** was added to
  `tests/s6_cold_paths.rs` as a third route beside slice and reader. It forces
  a refill between almost every byte, so every token, escape and line boundary
  gets split at an arbitrary point. All 7 suites pass on all three routes.

`LineColIterator` is now unreferenced and `src/iter.rs` is deleted -- the whole
per-byte line-counting apparatus is gone, replaced by `memchr` on demand.

### 2026-09-10 -- M3 REVERSED, and the measurement error that hid it

**The SIMD island was a net loss. It is now off by default.** M3's own
measurement said it won, and that measurement was wrong in a way this project
had already written down.

#### What M3 measured, and why it could not see this

M3's arms were `RJT_ISA=swar` against `RJT_ISA=sse2` -- **two rungs of the
island, compared with each other.** Both arms were inside the island. What that
answers is "is sixteen bytes better than eight, once you are already paying to
reach the island". What it never asked is "is reaching the island better than
not reaching it", because there was no arm outside it.

The island's SWAR rung is NOT the same code as the pre-island in-crate SWAR:
the island's version lives in another crate, behind a `#[target_feature]`
function that cannot be inlined. So the baseline was already regressed, and
comparing a slightly-less-regressed arm to it produced a clean-looking 1.105x
at 61 of 61 wins for a brick that was making the crate slower.

**The rule was already in this ledger, from M1-A: "comparing two builds of this
crate is nearly useless for a brick." The lesson generalises further than it
was written. An A/B is only worth what its BASELINE is worth, and a baseline
inside the thing under test is not a baseline.**

#### What the correct comparison says

Ours against UPSTREAM, in one process, 11 pairs -- the comparison M3 never ran.
Lower is better; these are ours/upstream time.

| cell | island OFF | island ON |
|---|---:|---:|
| canada struct-parse | **1.029** | 1.236 |
| canada dom-parse | **0.958** | 1.245 |
| canada scan | **1.120** | 1.335 |
| twitter struct-parse | **0.969** | 1.159 |
| twitter scan | **1.048** | 1.280 |
| twitter dom-parse | **0.966** | 1.029 |
| citm struct-stringify | **0.951** | 1.154 |
| citm dom-stringify | **0.944** | 1.018 |
| citm struct-parse | **0.822** | 0.931 |
| citm scan | **0.882** | 0.995 |

**Off is better on thirteen of fifteen cells, and the island wins none of them
convincingly.**

#### The mechanism, and it is not the scan

`canada scan` is the tell. That file holds **33 whitespace bytes in 2.25 MB**,
so the island cannot be doing any scanning there at all -- and it still costs
1.120x to 1.335x. Whatever the island is doing wrong, it is not scanning badly.

A `#[target_feature]` function cannot be inlined into a caller that lacks the
feature. So reaching the island puts a **non-inlinable call inside
`scan_ws_run`**, which makes `scan_ws` too big to inline, which stops
`skip_whitespace` and `parse_whitespace` from inlining into the parser's hot
loop. That cascade costs more than the extra eight bytes of width buy.

**That is a general result about unsafe SIMD islands in Rust rather than a fact
about this corpus: the cost of an island is not the dispatch, it is the
inlining you lose at every call site that can no longer see through it.** A
wider kernel has to beat that, not merely beat the narrower kernel.

The crate stays -- it is correct, twin-tested over every byte value at every
offset, and reachable via `--features accel` -- so this can be re-measured
where the trade comes out differently. What changed is the default, and the
reason sits in `Cargo.toml` beside the feature so nobody switches it on without
reading it.

### 2026-09-10 -- G2 measured on the shipping default

The milestone's exit test, and the first full ours-against-upstream run since
M0. Pinned cpu2/High, ABBA, 21 pairs, both arms in one binary.
Raw: `corpus/runs/2026-09-10-g2-final.txt`; null arm
`corpus/runs/2026-09-10-g2-null.txt` (medians 0.988x-1.018x).

| file | column | ours | upstream | ours/upstream | wins |
|---|---|---:|---:|---:|---:|
| twitter | struct-stringify | **2,289 MB/s** | 1,503 | **0.654x = 1.53x** | 21/21 |
| twitter | dom-stringify | **2,002 MB/s** | 1,386 | **0.698x = 1.43x** | 21/21 |
| citm_catalog | scan | **2,294 MB/s** | 1,881 | **0.820x = 1.22x** | 21/21 |
| citm_catalog | struct-parse | **1,717 MB/s** | 1,405 | **0.823x = 1.22x** | 21/21 |
| citm_catalog | dom-stringify | **1,268 MB/s** | 1,125 | **0.876x = 1.14x** | 21/21 |
| citm_catalog | struct-stringify | **2,134 MB/s** | 1,916 | **0.893x = 1.12x** | 21/21 |
| citm_catalog | dom-parse | **769 MB/s** | 715 | **0.917x = 1.09x** | 21/21 |
| twitter | dom-parse | 469 MB/s | 452 | 0.959x | 17/21 |
| canada | struct-parse | 764 MB/s | 741 | 0.964x | 21/21 |
| canada | struct-stringify | 661 MB/s | 643 | 0.974x | 20/21 |
| canada | dom-stringify | 979 MB/s | 961 | 0.986x | 16/21 |
| canada | dom-parse | 405 MB/s | 394 | 0.992x | 15/21 |
| twitter | struct-parse | 914 MB/s | 934 | 1.009x | 9/21 |
| twitter | scan | 1,737 MB/s | 1,816 | **1.047x** | 0/21 |
| canada | scan | 1,265 MB/s | 1,603 | **1.271x** | 0/21 |

**Twelve of fifteen cells beat upstream, ten of them at 20 or 21 wins of 21.**

#### Against G2's bars, honestly

| bar | status |
|---|---|
| S1 struct stringify >= 1.3x | **MET, 1.53x** |
| S3 (canada) parse >= 1.0x, no regression | **MET** (1.04x struct, 1.01x DOM) |
| no cell below 0.97x on the four json-benchmark columns | **MET on S1-S3** -- worst is twitter struct-parse at 1.009x, inside the null floor |
| S1 DOM parse >= 1.8x | **NOT MET, 1.04x** |
| S1 struct parse >= 1.4x | **NOT MET, 0.99x** |
| S4 struct parse >= 1.4x | **NOT MET**, and two S4 cells LOSE |

#### Two groups of failing cells, separated by what is known about them

**`scan`, where the cell cannot resolve anything.** Measured across six builds
of nearly identical source, `canada scan` read 1.308, 1.003, 0.887, 0.979 and
1.291 -- a **40% swing** with no code change to explain it. That cell's
cross-instance floor swamps any effect it could carry. It is also not a
shipping workload: `scan` is the `IgnoredAny` ceiling-probe column, an
instrument. Recorded as unresolvable rather than claimed as a regression, on
the same grounds M0 recorded one cell at 0.968x with byte-identical source.

**`s4-frame-telemetry`, where the loss is stable and the cause is NOT known.**
struct-parse 1.139x and struct-stringify 1.135x, at 0 wins of 11, reproduced
across two independent builds. Every brick was toggled off in turn --
`RJT_NUM_WIDE=0`, `RJT_WS_FASTPATH=0`, `RJT_ESC_WIDE=0` -- and **none is
responsible**; the ratio moves by under 2% for all three. So this is not B4
mis-firing on short numbers, which was the obvious hypothesis and is refuted.

That leaves the honest answer: **unexplained**. The leading candidate is the
same cross-instance instantiation bias, which M0 measured at up to **18% per
cell with byte-identical source** (twitter dom-stringify 0.835x, canada
dom-parse 1.094x, on code that was character-for-character upstream's), and
which should grow as our crate grows relative to upstream's. That is a
hypothesis, not a measurement, and it is written here as one.

#### What remains is architectural

The exit test asks for this note where a cell has stalled, and two have.

**S1 DOM parse is at 1.04x against a 1.8x bar, and no brick will close it.**
The allocation census settled why: DOM parse makes **20,834 allocations per
632 KB**, about one per 31 input bytes. Short strings dominate the COUNT --
54.8% of citm's allocations are eight bytes or fewer -- and `BTreeMap` nodes
dominate the BYTES: 10,978 nodes account for essentially all of citm's 7.68 MB,
and roughly 80% of each node is unused at a median arity of 2. Key reuse runs
**80x to 1,419x**: 46,816 string allocations for 33 distinct names on one
payload.

Closing that requires changing what `Value` IS -- interned keys and a
bump-allocated node store, a different type with a different public API. That
is the v1.x arena `Value` the plan already carries, and it is the only item on
the list with 1.8x in it. B10 was correctly killed at the census for proposing
to reach it with a sorted `Vec`.

**S1 struct parse is at 0.99x against a 1.4x bar, and M4 already closed that
route.** The B6 ceiling probe made key dispatch completely free and bought
0.6%, and the assembly shows twenty `memcmp` calls across 111 derive-generated
matchers. The derive is near-optimal, so struct parse is bounded by allocation
and by field construction, not by dispatch.

**Where this project has actually won is serialization and whitespace-dense
parsing**: 1.53x and 1.43x on twitter stringify, 1.22x on citm scan and struct
parse. Those came from removing redundant work on a slice, and the ceiling
probes say that seam is now largely worked out.
