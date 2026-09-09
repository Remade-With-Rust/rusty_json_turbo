# JSONCORP

The corpus every number in `LEDGER.md` is taken on, checked in as data so the
differential oracle and the benchmark run on every machine with no network.

| Scenario | Files | Bytes | Stresses |
|---|---|---:|---|
| S1 | `twitter.json` | 631,515 | strings, Unicode escapes, wide structs |
| S2 | `citm_catalog.json` | 1,727,131 | objects, integers, repeated keys |
| S3 | `canada.json` | 2,251,051 | floats (parse path frozen by contract) |
| S7 | `jsonchecker/*.json` (34), `roundtrip/*.json` (27) | small | conformance: fail/pass cases and exact round-trips |

Provenance and licences: `../NOTICE.md`. Integrity: `HASHES.txt` (SHA-256 of
every file); `fetch.ps1` re-downloads from the pinned upstream commit and
verifies the manifest. S4 (house payloads), S5 (NDJSON stream), S6
(pathological) and S8 (size sweep) land at M1 per `docs/plans/fast_mission.md`.
