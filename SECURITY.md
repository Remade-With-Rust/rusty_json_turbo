# Security policy

## Reporting a vulnerability

Email **tim.almond@thehouseinc.xyz** with the details. You will get an
acknowledgement within **72 hours** and a status update at least every **14
days**. Please give us **90 days** of coordinated disclosure before publishing;
we will credit you in the advisory unless you ask otherwise.

Do not open public issues for suspected vulnerabilities. A vulnerability that
also exists in upstream serde_json is reported to upstream as well, with your
consent, so both trees are fixed together.

## Supported versions

Only the latest published release receives security fixes.

## Threat model

**Assets.** The correctness of every byte this library reads and writes: a
document that parses must parse to the same value upstream serde_json would
produce, a value that serialises must serialise to the same bytes, and an
invalid document must be rejected at the same place with the same error.
Consumers (mID token bodies, rusty_time reports, SpaceDB values, Deputy
receipts, every public API the house ships) stake their wire compatibility on
that contract.

**Adversaries and untrusted input.** The whole input is hostile by definition:
JSON arrives from the network, from files, from other tenants. The library
opens no sockets, files or processes itself, holds no secrets and logs nothing;
its attack surface is the bytes handed to `from_*` and the values handed to
`to_*`.

**STRIDE pass.**
- *Spoofing / Repudiation / Information disclosure*: out of scope -- no
  identity, no logging, no secret state. Authenticity of a document is the
  consumer's job.
- *Tampering*: a divergence from upstream is the failure mode that matters.
  It is gated by the in-process differential oracle (bytes, `Value`, error
  text with line and column, float bits) over the corpus, several hundred edge
  documents and every number token, on every commit and under every feature
  flag.
- *Denial of service*: hostile input must never panic, overflow or recurse
  without bound. The recursion limit (128) is on by default; every public entry
  point returns a typed `Error`; the fuzz target from upstream is kept and M1
  adds one per public entry point. Release builds set `overflow-checks` per the
  measured decision recorded in the ledger.
- *Elevation of privilege*: the only `unsafe` is upstream's twelve sites,
  inherited unchanged at M0 and inventoried in `docs/UPSTREAM-CHANGES.md`. M2
  confines them to one audited island with a `SAFETY:` comment per block and a
  scalar twin gated byte-identical on every architecture.

**Residual risks** are listed in `docs/plans/use-protection-please.md` once the
hardening audit runs (mission plan M8).
