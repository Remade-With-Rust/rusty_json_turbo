# JSONCORP

The corpus every number in `LEDGER.md` is taken on, checked in as data so the
differential oracle and the benchmark run on every machine with no network.

| Scenario | Files | Bytes | Stresses |
|---|---|---:|---|
| S1 | `twitter.json` | 631,515 | strings, wide structs, raw non-ASCII (**not** hex escapes -- see below) |
| S2 | `citm_catalog.json` | 1,727,131 | objects, integers, repeated keys |
| S3 | `canada.json` | 2,251,051 | floats (parse path frozen by contract) |
| **S4** | `s4/*.json` (7) | 2,681,326 | **house payloads**: `flatten` tails, internally-tagged enums, `Vec<u8>` fields, minified bodies, an 87 KB single string, hex escapes and surrogate pairs, and the same 33 keys over 2,600 objects. Per-file census, provenance and the brick each one prices: [`s4/MANIFEST.md`](s4/MANIFEST.md) |
| S7 | `jsonchecker/*.json` (34), `roundtrip/*.json` (27) | small | conformance: fail/pass cases and exact round-trips |

**S1 does not contain Unicode escapes.** This table said it did until S4 was
built and the corpus was actually counted. `twitter.json` is 14.75% non-ASCII,
but all of it is raw UTF-8; `citm_catalog.json` has two two-byte escapes and
`canada.json` none. So the hex-escape decoder and the surrogate-pair path were
never on the timed corpus at all. `s4/s4-ocr-i18n.json` puts both under
measurement for the first time (5,671 hex escapes, 1,032 surrogate pairs).

**A note on line endings, because it moves the numbers.** This repo has no
root `.gitattributes`, so a checkout with `core.autocrlf=true` rewrites the
S1-S3 payloads: `twitter.json` is 631,515 bytes on Linux and 646,995 on such a
Windows checkout, a 15,480-byte difference that is entirely carriage returns
counted as whitespace. Byte counts and whitespace percentages for S1-S3 are
therefore platform-dependent today, and a measurement is only comparable with
another taken on the same kind of checkout. S4 is pinned against this with
`s4/.gitattributes` (`-text`). Fixing S1-S3 the same way would change their
bytes and move every number already published against them, so it is a
deliberate, separate decision rather than a drive-by.

Provenance and licences: `../NOTICE.md`. Integrity: `HASHES.txt` (SHA-256 of
every file); `fetch.ps1` re-downloads from the pinned upstream commit and
verifies the manifest. S4 is **generated, not fetched**, so `HASHES.txt` does
not cover it; `python tools/gen-s4.py --check` regenerates it in memory and
fails if any byte on disk differs, and CI runs that on every push. S5 (NDJSON
stream), S6 (pathological) and S8 (size sweep) are still to come, per
`docs/plans/fast_mission.md`.
