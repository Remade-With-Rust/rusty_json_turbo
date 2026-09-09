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

### 2026-09-09 -- M0 baseline

_Filled in by the M0 exit run; see the `rjson-bench` output pasted below._
