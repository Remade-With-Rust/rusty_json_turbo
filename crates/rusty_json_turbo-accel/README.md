# rusty_json_turbo-accel

The SIMD island for [`rusty_json_turbo`](https://crates.io/crates/rusty_json_turbo):
SSE2 and AVX2 twins of the JSON byte scanners.

**This is the one crate in the workspace where `unsafe` is allowed**, and that
is its whole reason to exist. Vector intrinsics are unsafe functions and there
is no safe spelling of an unaligned 16-byte load, so the vector code lives here
rather than opening a hole in the parser. Every `unsafe` block is a load or a
compare, bounds-checked by the line above it and carrying its invariant in a
`SAFETY` comment.

No dependencies. No build script. That is a contract, not an accident:
`rusty_json_turbo` advertises an empty dependency tree and this must not change
it.

## The rule every kernel follows

**The scalar version is written first, it is the oracle, and it stays in the
tree forever.** Each vector kernel is tested against it over every one of the
256 byte values at every offset, and `RJT_ISA=scalar` runs it in production so
the two can be compared inside one binary. A fast path whose scalar twin has
been deleted cannot be checked and cannot be debugged.

## Choosing a rung is a measurement, not a preference

`RJT_ISA` selects `scalar`, `swar`, `sse2` or `avx2`. It can only ever narrow:
asking for a rung the machine does not have gets the widest one it does.

The default is **SSE2, not AVX2**, because that is what measured faster. On the
JSONCORP corpus, AVX2 beat the 8-byte SWAR baseline on one file and lost on the
other two, while SSE2 beat it on every cell:

| cell | SSE2 vs SWAR | AVX2 vs SWAR |
|---|---:|---:|
| `citm_catalog` scan | **1.107x** | 1.099x |
| `citm_catalog` struct-parse | **1.074x** | 1.072x |
| `twitter` scan | **1.020x** | **0.992x** |
| `canada` scan | 1.009x | 0.997x |

The longest whitespace run anywhere in that corpus is **29 bytes**, so a
32-byte step is never fully used, while its costs -- a call that cannot inline,
a wider tail, `vzeroupper` on exit -- are paid on every run. AVX2 stays in the
tree and stays reachable so this can be re-measured on a machine or a corpus
with longer runs, rather than deleted on one box's answer.

## What the tests cover

- Every kernel against the scalar oracle over all 256 byte values at every
  offset in buffers spanning the 8, 16 and 32-byte boundaries, uniform runs of
  every length 0-70, and seeded mixed content. More than 500,000 assertions per
  scanner, asserted to be more than 500,000 so the spread cannot quietly thin.
- The **dispatcher**, not only the kernels. That caught a real bug on its first
  run: a step that collapsed "fewer than eight bytes remain" and "all eight
  were whitespace" into one `None`, so the caller skipped bytes it had never
  examined. Every kernel was correct; the composition was not.
- `0x0B` and `0x0C` are **not** JSON whitespace, which a `b <= 0x20` range test
  would get wrong.
- Nothing at or above `0x80` is ever flagged for escaping, which a signed
  `b < 0x20` compare would get wrong -- SSE2 and AVX2 byte compares are signed,
  which is why the crate writes that test as `b & 0xE0 == 0` throughout.
- A **poison test** asserting the suite would catch exactly those two mistakes.
  A suite that cannot fail is not a gate.

## Licence

MIT OR Apache-2.0, matching the parent crate.
