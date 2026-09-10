#!/usr/bin/env python3
# ASCII-only source (house lint). Non-ASCII payload content is written with
# \uXXXX escapes here and lands as real UTF-8 bytes in the output -- corpus
# files are DATA, not source, so real non-ASCII there is correct. S6 exists to
# carry byte sequences no source file would: a megabyte of hex escapes, lone
# surrogates, 400-digit numbers.
"""Generate the S5 (NDJSON stream) and S6 (pathological) corpora.

Why these two exist
-------------------
S1-S3 are three famous outliers and S4 is seven house-shaped payloads, but
every one of them is ONE well-formed document read from a slice. Two whole
regimes are therefore unmeasured:

  * S5 -- MANY documents from a READER. `docs/plans/fast_mission.md` 7.2 names
    it "100 MB of log lines through StreamDeserializer over from_reader",
    pricing `IoRead` (B7) and `RawValue` on Io (B13). Per-document overhead is
    a different question from throughput on one big document, and nothing in
    S1-S4 asks it.
  * S6 -- the COLD paths. 100-deep nesting, an escape every 7 bytes, a
    megabyte of `\\uXXXX`, 20-digit integers, 400-digit floats. The gate is
    not speed, it is "cold paths stay correct and do not regress; error
    line/col identity".

100 MB is NOT checked in
------------------------
`s5-log-stream.ndjson` is 10,000 log lines, about 2.6 MB. `--scale N` emits
N * 10,000 lines to a directory of your choosing, so the plan's 100 MB run is
reproducible without 100 MB in git:

    python tools/gen-s5-s6.py --scale 39 --out D:\\tmp\\s5-100mb

Records are seeded PER RECORD (`Rng(S5_SEED ^ (i * GOLDEN))`), so record i is
the same bytes at every scale: a scaled file EXTENDS the checked-in one rather
than replacing it, and `--check` verifies that prefix property.

Must-parse and must-fail
------------------------
Six S6 files are expected to FAIL to parse, on purpose. `tests/oracle.rs`
compares this crate against upstream serde_json byte for byte INCLUDING the
error message, its line, its column and its category, so a must-fail file is a
first-class corpus member -- the strongest test of error identity there is. It
is labelled in three places (the SPECS table below, the manifest, and the
`s6-fail-` filename prefix) so nobody reads a failure as a bug.

Python is NOT the oracle
------------------------
Every file is run through `json.loads`, but Python's decoder is a DIFFERENT
parser with different limits, and four of the six must-fail files parse
CLEANLY under Python:

  * Python accepts lone surrogates (`json.loads('"\\ud800"')` is fine);
    serde_json rejects them.
  * Python's ints are arbitrary precision and its floats saturate to
    `inf`; serde_json errors with "number out of range" instead of ever
    producing an infinity.
  * Python's nesting limit is the interpreter recursion limit (~1000);
    serde_json's is 128.
  * Python also accepts `NaN` / `Infinity` bare literals, which JSON does not.
    No file here contains them (they are S7's and the fuzzers' business), but
    the divergence is why `json.loads` cannot be the gate.

So each file carries BOTH an expected serde outcome and an expected Python
outcome, and the generator fails if Python disagrees with the Python column.
The serde column was measured against upstream serde_json 1.0.151, not
predicted; see MANIFEST.md.

Determinism
-----------
A corpus that changes between runs silently invalidates every measurement ever
taken against it. So, exactly as in `tools/gen-s4.py`:

  * The only entropy is SEED_S5 / SEED_S6, expanded by a splitmix64 written
    out below -- NOT `random`, whose stream is a Python implementation detail.
  * Floats never reach the writer. Every fractional token is produced by
    `num()` or by `Raw()` as a decimal string, so no `float.__repr__` decision
    can move a byte.
  * Dict order is insertion order: what a real producer emits, kept stable.
  * Output is LF. `corpus/s5/.gitattributes` and `corpus/s6/.gitattributes`
    mark the payloads `-text`; the root `.gitattributes` already pins
    `corpus/**/*.json`, but NOT `*.ndjson`, which is the one this adds.

Run
---
  python tools/gen-s5-s6.py            # write corpus/s5 + corpus/s6, census
  python tools/gen-s5-s6.py --check    # regenerate in memory; fail on any diff
  python tools/gen-s5-s6.py --scale 39 --out DIR   # a ~100 MB S5 stream
"""

import hashlib
import json
import os
import re
import sys

SEED_S5 = 0x53355F4E444A534E  # "S5_NDJSN"
SEED_S6 = 0x53365F504154484F  # "S6_PATHO"

# ---- sizes. Frozen: changing any of these changes every hash in MANIFEST.md.
N_LOG_LINES = 10_000  # also the plan's P50/P99-over-10k message count
N_PRETTY_DOCS = 1_200
LINE_MIN = 150  # where log lines actually live
LINE_MAX = 400

ESC7_PERIODS = 149_797  # 7 bytes each -> ~1 MiB of string content
UESC_TARGET = 1_048_576  # ~1 MiB of \uXXXX escapes
N_CHURN = 20_000
N_DUP_KEYS = 10_000
N_BULK_INTS = 2_000
N_BULK_FLOATS = 200
N_BULK_EXP = 200
WIDE_ARITY = 1_000  # beats citm_catalog's 184-field object
DEEP = 100  # the plan's "100-deep nesting"
DEEP_MAX = 127  # the deepest that parses (serde_json's limit is 128)
DEEP_OVER = 128  # the first depth that FAILS
FAIL_ELEMS = 200  # array length in the planted-hazard files
FAIL_AT = 150  # which element carries the hazard

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
CORPUS_DIR = os.path.join(REPO, "corpus")
S5_DIR = os.path.join(CORPUS_DIR, "s5")
S6_DIR = os.path.join(CORPUS_DIR, "s6")

MASK64 = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15


# --------------------------------------------------------------------------
# The random stream: splitmix64, written out so it cannot drift with Python.
# Identical to tools/gen-s4.py's.
# --------------------------------------------------------------------------
class Rng(object):
    __slots__ = ("s",)

    def __init__(self, seed):
        self.s = seed & MASK64

    def u64(self):
        self.s = (self.s + GOLDEN) & MASK64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
        return z ^ (z >> 31)

    def below(self, n):
        return self.u64() % n

    def between(self, lo, hi):
        """Inclusive integer range."""
        return lo + self.below(hi - lo + 1)

    def unit(self):
        return (self.u64() >> 11) / float(1 << 53)

    def frange(self, lo, hi):
        return lo + (hi - lo) * self.unit()

    def pick(self, seq):
        return seq[self.below(len(seq))]

    def chance(self, p):
        return self.unit() < p

    def bytes(self, n):
        out = bytearray()
        while len(out) < n:
            out += self.u64().to_bytes(8, "big")
        return bytes(out[:n])

    def sample(self, seq, k):
        pool = list(seq)
        out = []
        for _ in range(min(k, len(pool))):
            out.append(pool.pop(self.below(len(pool))))
        return out

    def subset(self, seq, k):
        """`sample`, but the survivors keep their original order -- which is
        what a real logger emits: a fixed field order, some fields absent."""
        keep = set()
        pool = list(range(len(seq)))
        for _ in range(min(k, len(pool))):
            keep.add(pool.pop(self.below(len(pool))))
        return [seq[i] for i in range(len(seq)) if i in keep]


# --------------------------------------------------------------------------
# The writer. serde_json's escaping rules, serde_json's pretty layout.
# Identical to tools/gen-s4.py's.
# --------------------------------------------------------------------------
class Raw(object):
    """A verbatim JSON token -- used for every number, so no float repr runs."""

    __slots__ = ("text",)

    def __init__(self, text):
        self.text = text


class RawStr(object):
    """A verbatim JSON string BODY; the quotes are added, the body is not
    touched. This is how S6 carries a megabyte of `\\uXXXX`, a lone surrogate,
    and an escape every 7 bytes exactly as written."""

    __slots__ = ("body",)

    def __init__(self, body):
        self.body = body


_NEEDS_ESCAPE = re.compile(r'[\x00-\x1f"\\]')
_SHORT = {0x08: "\\b", 0x09: "\\t", 0x0A: "\\n", 0x0C: "\\f", 0x0D: "\\r"}


def _esc(s):
    if not _NEEDS_ESCAPE.search(s):
        return s
    out = []
    for ch in s:
        o = ord(ch)
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif o in _SHORT:
            out.append(_SHORT[o])
        elif o < 0x20:
            out.append("\\u%04x" % o)  # serde_json writes lowercase hex
        else:
            out.append(ch)
    return "".join(out)


def num(x, nd):
    """A fixed-point decimal token. Deterministic by construction."""
    return Raw(("%." + str(nd) + "f") % x)


def emit(value, indent=0):
    buf = []
    _w(value, buf, indent, 0)
    return "".join(buf).encode("utf-8")


def _w(v, buf, ind, depth):
    if isinstance(v, Raw):
        buf.append(v.text)
    elif isinstance(v, RawStr):
        buf.append('"')
        buf.append(v.body)
        buf.append('"')
    elif v is True:
        buf.append("true")
    elif v is False:
        buf.append("false")
    elif v is None:
        buf.append("null")
    elif isinstance(v, str):
        buf.append('"')
        buf.append(_esc(v))
        buf.append('"')
    elif isinstance(v, float):
        raise TypeError("floats never reach the writer; use num(x, nd)")
    elif isinstance(v, int):
        buf.append(str(v))
    elif isinstance(v, dict):
        _w_container(v, buf, ind, depth, True)
    elif isinstance(v, (list, tuple)):
        _w_container(v, buf, ind, depth, False)
    else:
        raise TypeError("cannot emit %r" % (type(v),))


def _w_container(v, buf, ind, depth, is_obj):
    open_c, close_c = ("{", "}") if is_obj else ("[", "]")
    items = list(v.items()) if is_obj else list(v)
    if not items:
        buf.append(open_c + close_c)
        return
    if ind == 0:
        buf.append(open_c)
        for i, it in enumerate(items):
            if i:
                buf.append(",")
            if is_obj:
                buf.append('"' + _esc(it[0]) + '":')
                _w(it[1], buf, ind, depth + 1)
            else:
                _w(it, buf, ind, depth + 1)
        buf.append(close_c)
        return
    pad = " " * (ind * (depth + 1))
    end_pad = " " * (ind * depth)
    buf.append(open_c + "\n")
    for i, it in enumerate(items):
        if i:
            buf.append(",\n")
        buf.append(pad)
        if is_obj:
            buf.append('"' + _esc(it[0]) + '": ')
            _w(it[1], buf, ind, depth + 1)
        else:
            _w(it, buf, ind, depth + 1)
    buf.append("\n" + end_pad + close_c)


# --------------------------------------------------------------------------
# Identifier shapes the house actually uses. Ported from tools/gen-s4.py.
# --------------------------------------------------------------------------
_B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
_B32 = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"  # Crockford, as ULID uses


def b58(raw):
    n = int.from_bytes(raw, "big")
    out = ""
    while n:
        n, r = divmod(n, 58)
        out = _B58[r] + out
    for b in raw:
        if b:
            break
        out = "1" + out
    return out or "1"


def ulid(raw16):
    n = int.from_bytes(raw16, "big")
    out = []
    for _ in range(26):
        out.append(_B32[n & 31])
        n >>= 5
    return "".join(reversed(out))


def did(rng):
    return "did:mata:" + b58(rng.bytes(32))


def _civil(z):
    era = (z if z >= 0 else z - 146096) // 146097
    doe = z - era * 146097
    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
    y = yoe + era * 400
    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
    mp = (5 * doy + 2) // 153
    d = doy - (153 * mp + 2) // 5 + 1
    m = mp + 3 if mp < 10 else mp - 9
    return (y + (1 if m <= 2 else 0), m, d)


def iso_ms(millis):
    """RFC 3339 with milliseconds, which is what a house logger emits.

    Fixed civil-time arithmetic, no locale, no tz database.
    """
    t, ms = divmod(millis, 1000)
    days, rem = divmod(t, 86400)
    h, rem = divmod(rem, 3600)
    m, s = divmod(rem, 60)
    y, mo, d = _civil(days + 719468)
    return "%04d-%02d-%02dT%02d:%02d:%02d.%03dZ" % (y, mo, d, h, m, s, ms)


# --------------------------------------------------------------------------
# The content census: a byte-for-byte port of
# crates/rusty_json_turbo-bench/src/content.rs, taken verbatim from
# tools/gen-s4.py so an S5/S6 row is directly comparable with the S1-S4 rows
# the ledger already carries. Validated in main() against S1-S3's published
# percentages.
# --------------------------------------------------------------------------
WS = b" \t\n\r"


def census(buf):
    c = {
        "bytes": len(buf),
        "whitespace": 0,
        "structural": 0,
        "string_bytes": 0,
        "escape_bytes": 0,
        "number_bytes": 0,
        "literal_bytes": 0,
        "non_ascii": sum(1 for b in buf if b >= 0x80),
        "strings": 0,
        "strings_with_escapes": 0,
        "numbers": 0,
        "floats": 0,
        "longest_string": 0,
        "longest_ws_run": 0,
        "ws_bytes_by_len": [0] * 9,
        "ws_runs_by_len": [0] * 9,
        "objects": 0,
        "arrays": 0,
        "keys": 0,
        "depth": 0,
        "literals": 0,
        "distinct_keys": set(),
        "max_obj_arity": 0,
        "max_arr_arity": 0,
    }
    n = len(buf)
    i = 0
    depth = 0
    stack = []  # [is_obj, member_count, saw_content]

    def note_content():
        if stack:
            stack[-1][2] = True

    while i < n:
        b = buf[i]
        if b in WS:
            start = i
            while i < n and buf[i] in WS:
                i += 1
            run = i - start
            c["whitespace"] += run
            c["longest_ws_run"] = max(c["longest_ws_run"], run)
            k = _ws_bucket(run)
            c["ws_runs_by_len"][k] += 1
            c["ws_bytes_by_len"][k] += run
        elif b in b"{[":
            c["objects" if b == 0x7B else "arrays"] += 1
            c["structural"] += 1
            note_content()
            stack.append([b == 0x7B, 0, False])
            depth += 1
            c["depth"] = max(c["depth"], depth)
            i += 1
        elif b in b"}]":
            c["structural"] += 1
            depth -= 1
            if stack:
                is_obj, commas, content = stack.pop()
                arity = commas + 1 if content else 0
                k = "max_obj_arity" if is_obj else "max_arr_arity"
                c[k] = max(c[k], arity)
            i += 1
        elif b == 0x2C:
            c["structural"] += 1
            if stack:
                stack[-1][1] += 1
            i += 1
        elif b == 0x3A:
            c["structural"] += 1
            i += 1
        elif b == 0x22:
            c["structural"] += 2  # both quotes
            c["strings"] += 1
            note_content()
            i += 1
            start = i
            escaped = False
            while i < n:
                if buf[i] == 0x22:
                    break
                if buf[i] == 0x5C:
                    escaped = True
                    step = 6 if i + 1 < n and buf[i + 1] == 0x75 else 2
                    step = min(step, n - i)
                    c["escape_bytes"] += step
                    i += step
                else:
                    i += 1
            ln = i - start
            c["string_bytes"] += ln
            c["longest_string"] = max(c["longest_string"], ln)
            if escaped:
                c["strings_with_escapes"] += 1
            i += 1  # closing quote
            j = i
            while j < n and buf[j] in WS:
                j += 1
            if j < n and buf[j] == 0x3A:
                c["keys"] += 1
                c["distinct_keys"].add(bytes(buf[start:i - 1]))
        elif b == 0x2D or 0x30 <= b <= 0x39:
            c["numbers"] += 1
            note_content()
            start = i
            is_float = False
            while i < n:
                d = buf[i]
                if 0x30 <= d <= 0x39 or d in b"-+":
                    i += 1
                elif d in b".eE":
                    is_float = True
                    i += 1
                else:
                    break
            c["number_bytes"] += i - start
            if is_float:
                c["floats"] += 1
        elif b in b"tfn":
            step = min(5 if b == 0x66 else 4, n - i)
            c["literal_bytes"] += step
            c["literals"] += 1
            note_content()
            i += step
        else:
            i += 1
    return c


def _ws_bucket(nn):
    if nn <= 1:
        return 0
    if nn == 2:
        return 1
    if nn == 3:
        return 2
    if nn == 4:
        return 3
    if nn <= 8:
        return 4
    if nn <= 16:
        return 5
    if nn <= 32:
        return 6
    if nn <= 64:
        return 7
    return 8


def pct(part, whole):
    return 0.0 if not whole else part * 100.0 / whole


def slice_position(buf, i):
    """`SliceRead::position_of_index` from src/read.rs, ported exactly.

    line is 1-based; column is the 0-based offset within the line, which is
    why `peek_position` below passes index+1 and the reported column reads as
    1-based. Used to predict where a must-fail file's error lands; the
    prediction is then confirmed against upstream serde_json.
    """
    nl = buf.rfind(b"\n", 0, i)
    start = nl + 1 if nl >= 0 else 0
    return (1 + buf.count(b"\n", 0, start), i - start)


def peek_position(buf, index):
    """`SliceRead::peek_position`: the position of the byte being peeked."""
    return slice_position(buf, min(len(buf), index + 1))


def nth_byte(buf, byte, n):
    """0-based index of the n-th (1-based) occurrence of `byte`."""
    i = -1
    for _ in range(n):
        i = buf.index(byte, i + 1)
    return i


# ==========================================================================
# S5. Log/telemetry lines, as NDJSON and as a pretty concatenated stream.
# ==========================================================================
LEVELS = (
    ["trace"] * 5 + ["debug"] * 20 + ["info"] * 60 + ["warn"] * 12 + ["error"] * 3
)

# Field kinds, so a value's SHAPE is chosen by the template and its BYTES by
# the seed. `path` and `label` are the only kinds that can emit an escape or a
# non-ASCII byte, and their share is reported in the manifest.
COLLECTIONS = ["contacts", "vault", "notes", "devices", "grants", "media"]
TIERS = ["convergent", "causal", "strong"]
# Split, because an audio codec in a video encoder's log is the kind of detail
# that makes a synthetic corpus read as synthetic.
VCODECS = ["h264", "hevc", "av1", "av2", "vp9"]
ACODECS = ["opus", "flac", "aac", "mp3", "vorbis"]
HOSTS = ["time.mata.example", "pool.ntp.example", "relay-eu-1.mata.example",
         "relay-us-3.mata.example", "box-7.lan", "studio-01.lan"]
ROUTES = ["/v1/status", "/v1/catalog", "/v1/resources", "/v1/media/probe",
          "/v1/ocr/read", "/v1/asr/transcribe", "/v1/vlm/caption", "/v1/compress"]
ERR_KINDS = [
    "tier_unavailable", "capability_expired", "roster_stale", "peer_timeout",
    "shard_missing", "advisory_db_stale", "decode_error", "io",
]
# Real log text carries accents and the occasional non-Latin name. Kept to one
# field kind so the non-ASCII share is a number, not a surprise.
LABELS_ASCII = [
    "studio-01", "laptop-work", "phone", "oem-box", "relay-eu-1", "cam-a",
    "drone-2", "browser-extension", "desktop-studio", "tablet",
]
LABELS_I18N = [
    "R\u00e9p\u00e9tition g\u00e9n\u00e9rale",
    "\u00dcbergabe-Box",
    "c\u00e1mara 3",
    "\u30b9\u30bf\u30b8\u30aa-01",
    "\u0421\u0442\u0443\u0434\u0438\u044f 2",
]
PATHS = [
    "F:\\mata\\node\\store\\L0",
    "F:\\media\\ingest\\2026-09",
    "F:\\mata\\deputy\\prod\\vault.sealed",
    "F:\\media\\ocr\\scan_0041.png",
    "C:\\ProgramData\\mata\\sidecar\\log",
]


def field_value(rng, kind):
    if kind == "node":
        return "nd-" + b58(rng.bytes(6))
    if kind == "box":
        return "box-" + b58(rng.bytes(8))
    if kind == "device":
        return "dev-" + b58(rng.bytes(8))
    if kind == "did":
        return did(rng)
    if kind == "count":
        return rng.between(0, 100000)
    if kind == "small":
        return rng.between(0, 64)
    if kind == "ms":
        return num(rng.frange(0.05, 900.0), 3)
    if kind == "bytes":
        return rng.between(64, 64 * 1024 * 1024)
    if kind == "ratio":
        return num(rng.frange(0.0, 1.0), 4)
    if kind == "tier":
        return rng.pick(TIERS)
    if kind == "collection":
        return rng.pick(COLLECTIONS)
    if kind == "vcodec":
        return rng.pick(VCODECS)
    if kind == "acodec":
        return rng.pick(ACODECS)
    if kind == "host":
        return rng.pick(HOSTS)
    if kind == "route":
        return rng.pick(ROUTES)
    if kind == "shard_k":
        return rng.pick([4, 6, 8])
    if kind == "shard_n":
        return rng.pick([10, 12, 14])  # always > k, as k-of-n requires
    if kind == "bool":
        return rng.chance(0.5)
    if kind == "nullable":
        return None if rng.chance(0.45) else "nd-" + b58(rng.bytes(6))
    if kind == "hex":
        return rng.bytes(8).hex()
    if kind == "path":
        return rng.pick(PATHS)
    if kind == "label":
        # 1 in 8 carries raw UTF-8, so the non-ASCII share is ~1.5% of lines.
        return rng.pick(LABELS_I18N) if rng.chance(0.125) else rng.pick(LABELS_ASCII)
    if kind == "qp":
        return num(rng.frange(18.0, 31.0), 1)
    if kind == "kbps":
        return num(rng.frange(120.0, 42000.0), 2)
    raise AssertionError("unknown field kind %r" % (kind,))


# (target, [messages], [(field, kind)])
TARGETS = [
    ("spacedb::replica", [
        "anti-entropy round complete",
        "peer digest exchanged",
        "delta batch applied",
        "gossip fanout reduced under load",
    ], [("peer", "node"), ("peer_label", "label"), ("entries", "count"),
        ("applied", "count"), ("tier", "tier"), ("ms", "ms"),
        ("collection", "collection")]),
    ("spacedb::durability", [
        "shard set verified on read",
        "placement repair scheduled",
        "k-of-n below threshold, repairing",
        "stripe sealed",
    ], [("stripe", "hex"), ("k", "shard_k"), ("n", "shard_n"),
        ("distinct_nodes", "small"), ("shard_bytes", "bytes"), ("ms", "ms")]),
    ("spacedb::crdt", [
        "reactive query re-ran",
        "tombstones collected",
        "register write converged",
        "or-set merge produced no change",
    ], [("collection", "collection"), ("watchers", "small"),
        ("ops", "count"), ("tombstones", "count"), ("ms", "ms")]),
    ("mid::verify", [
        "assertion verified",
        "nonce envelope outside tolerance",
        "roster resolved from cache",
        "signature check failed",
    ], [("did", "did"), ("device_id", "device"), ("device_label", "label"),
        ("roster_version", "small"), ("tolerance_s", "small"), ("ms", "ms")]),
    ("disco::placement", [
        "content-addressed placement chosen",
        "node left the mesh",
        "node joined the mesh",
        "placement skewed, rebalancing",
    ], [("node", "node"), ("replicas", "small"), ("min_distinct", "small"),
        ("skew", "ratio"), ("ms", "ms")]),
    ("maestro::sched", [
        "job admitted",
        "job deferred, budget exhausted",
        "worker lease renewed",
        "queue depth above watermark",
    ], [("job", "hex"), ("queue_depth", "count"), ("budget_units", "count"),
        ("lease_s", "small"), ("ms", "ms")]),
    ("rff::demux", [
        "probe complete",
        "container reported no duration",
        "stream selected",
        "seek landed on a non-keyframe",
    ], [("file", "path"), ("streams", "small"), ("vcodec", "vcodec"),
        ("acodec", "acodec"), ("probe_score", "small"), ("ms", "ms")]),
    ("rusty_h264::encode", [
        "frame encoded",
        "rate control clamped qp",
        "gop closed",
        "lookahead starved",
    ], [("frame", "count"), ("qp", "qp"), ("kbps", "kbps"),
        ("codec", "vcodec"), ("ms", "ms")]),
    ("carmenta::layout", [
        "reading order resolved",
        "page skew corrected",
        "low-confidence line dropped",
        "column split detected",
    ], [("page", "path"), ("title", "label"), ("lines", "count"),
        ("conf", "ratio"), ("rotation", "small"), ("ms", "ms")]),
    ("mercury::asr", [
        "segment transcribed",
        "vad found no speech",
        "beam widened after low logprob",
        "model warm",
    ], [("clip", "path"), ("speaker", "label"), ("segments", "count"),
        ("no_speech", "ratio"), ("beam", "small"), ("ms", "ms")]),
    ("deputy::scan", [
        "advisory database refreshed",
        "receipt chained",
        "promotion gate failed closed",
        "artifact hashed",
    ], [("stage", "hex"), ("advisories", "count"), ("age_days", "small"),
        ("sealed", "bool"), ("ms", "ms")]),
    ("rtimed::poll", [
        "nts source polled",
        "step threshold exceeded, slewing",
        "source unreachable, demoted",
        "interleaved response accepted",
    ], [("source", "host"), ("offset_ms", "ms"), ("poll_log2", "small"),
        ("nts", "bool"), ("stratum", "small")]),
    ("sidecar::http", [
        "request served",
        "capability catalog listed",
        "unpaired box answered with pair_state open",
        "request rejected, no capability",
    ], [("route", "route"), ("status", "small"), ("elapsed_ms", "ms"),
        ("box_id", "box"), ("bytes_out", "bytes")]),
    ("rusty_zstd::frame", [
        "frame compressed",
        "long window enabled",
        "dictionary unavailable, falling back",
        "ratio below expectation",
    ], [("level", "small"), ("in_bytes", "bytes"), ("out_bytes", "bytes"),
        ("ratio", "ratio"), ("ms", "ms")]),
]

# Available to every target, so the length control below always has somewhere
# to go without inventing a field the target would not log.
GENERIC_FIELDS = [
    ("host", "host"), ("pid", "count"), ("thread", "small"),
    ("build", "hex"), ("node_label", "label"), ("upstream", "nullable"),
]


def build_log_record(rng, i, t0_ms):
    """One log line. Length is held inside [LINE_MIN, LINE_MAX] by adding or
    dropping ONE field after the fact, which keeps the line in the band real
    log lines occupy without hand-tuning fifteen templates."""
    tgt, msgs, tmpl = rng.pick(TARGETS)
    level = rng.pick(LEVELS)
    rec = {}
    rec["ts"] = iso_ms(t0_ms + i * 37 + rng.below(31))
    rec["lvl"] = level
    rec["tgt"] = tgt
    rec["msg"] = rng.pick(msgs)
    if rng.chance(0.85):
        rec["trace"] = ulid(rng.bytes(16))
    rec["span"] = "sp-" + b58(rng.bytes(6))
    fields = {}
    for key, kind in rng.subset(tmpl, rng.between(2, 6)):
        fields[key] = field_value(rng, kind)
    rec["fields"] = fields
    if level in ("warn", "error"):
        rec["err"] = {
            "kind": rng.pick(ERR_KINDS),
            "detail": rng.pick([
                "no peer answered within the deadline",
                "capability expired 4s before use",
                "roster version behind the assertion",
                "shard 3 of 10 missing on two nodes",
                "decoder reported a truncated frame",
            ]),
            "retry_in_ms": None if rng.chance(0.4) else rng.between(50, 30000),
        }
    else:
        rec["err"] = None

    pool = [f for f in (tmpl + GENERIC_FIELDS) if f[0] not in fields]
    line = emit(rec, 0)
    while len(line) < LINE_MIN and pool:
        key, kind = pool.pop(0)
        fields[key] = field_value(rng, kind)
        line = emit(rec, 0)
    while len(line) > LINE_MAX and len(fields) > 1:
        del fields[list(fields.keys())[-1]]
        line = emit(rec, 0)
    if not LINE_MIN <= len(line) <= LINE_MAX:
        raise SystemExit(
            "record %d is %d bytes, outside [%d, %d]" % (i, len(line), LINE_MIN, LINE_MAX))
    return rec, line


def record_rng(i):
    """Per-record seeding, so record i is the same bytes at every --scale and a
    scaled stream EXTENDS the checked-in one instead of replacing it."""
    return Rng(SEED_S5 ^ ((i * GOLDEN) & MASK64))


T0_MS = 1789000000000  # a fixed wall clock; no host clock reaches the output


def build_log_ndjson(n_lines):
    """One complete JSON object per line, no enclosing array.

    Ends WITH a newline, unlike every S4 file. That is deliberate: a log file
    does, and trailing whitespace after the last document is a distinct
    `StreamDeserializer` path (the iterator must return None, not an error).
    """
    out = []
    for i in range(n_lines):
        _, line = build_log_record(record_rng(i), i, T0_MS)
        out.append(line)
        out.append(b"\n")
    return b"".join(out)


# Inter-document whitespace. "" is in the pool on purpose: `}{` with no
# separator at all is its own path, and it is the one a naive stream reader
# gets wrong. `\r` is in the pool for the same reason -- it is JSON
# whitespace, `.gitattributes -text` pins it, and its count is published.
DOC_SEPS = [
    "", "\n", "\n\n", " ", "\t", "\r\n", "\n  \n", " \t \n", "\n\r\n\t ", "\n\n\n",
]
LEAD_WS = "  \n\t\n"
TAIL_WS = "\n\n  \t\n"


def build_log_pretty(n_docs):
    """The same records, pretty-printed, concatenated as one stream.

    `StreamDeserializer` must skip whitespace BETWEEN documents -- leading,
    trailing and every separator in `DOC_SEPS` -- which is a different code
    path from skipping whitespace inside one. The documents are the first
    `n_docs` records of the NDJSON file, so a mismatch between the two files
    is a generator bug and `validate_pair` catches it.
    """
    sep_rng = Rng(SEED_S5 ^ 0xC0FFEE)
    out = [LEAD_WS.encode("ascii")]
    for i in range(n_docs):
        rec, _ = build_log_record(record_rng(i), i, T0_MS)
        out.append(emit(rec, 2))
        if i + 1 < n_docs:
            out.append(sep_rng.pick(DOC_SEPS).encode("ascii"))
    out.append(TAIL_WS.encode("ascii"))
    return b"".join(out)


# ==========================================================================
# S6. One hazard per file, so a failure names itself.
# ==========================================================================

# ---- nesting -------------------------------------------------------------
def deep_array(depth):
    return ("[" * depth + "1" + "]" * depth).encode("ascii")


def deep_object_distinct(depth):
    """A distinct key per level: the depth path with cold key dispatch."""
    parts = []
    for d in range(depth):
        parts.append('{"k%03d":' % d)
    return ("".join(parts) + "1" + "}" * depth).encode("ascii")


def deep_object_pretty(depth):
    """Nested objects, one level per line, so the recursion-limit error lands
    on line `depth` rather than on line 1 -- the only file in the corpus that
    tests the LINE half of error identity at depth."""
    lines = ["{"]
    for d in range(1, depth):
        lines.append("%s\"k\": {" % ("  " * d))
    lines.append("%s\"v\": 1" % ("  " * depth))
    for d in range(depth - 1, -1, -1):
        lines.append("%s}" % ("  " * d))
    return "\n".join(lines).encode("ascii")


def deep_mixed(depth):
    """Alternating array/object to exactly `depth` containers."""
    out = []
    for d in range(depth):
        out.append("[" if d % 2 == 0 else '{"a":')
    out.append("1")
    for d in range(depth - 1, -1, -1):
        out.append("]" if d % 2 == 0 else "}")
    return "".join(out).encode("ascii")


# ---- the escape scanner --------------------------------------------------
ESC7_CYCLE = ['\\"', "\\\\", "\\/", "\\b", "\\f", "\\n", "\\r", "\\t"]
ESC7_ALPHA = "abcdefghijklmnopqrstuvwxyz0123456789 -_.,;:ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def build_escape_every_7(periods):
    """A bare top-level JSON string: 5 literal bytes then one 2-byte escape,
    repeating, so an escape sequence BEGINS every 7 bytes of encoded string
    content and escape bytes are 2/7 = 28.57% of it.

    Bare, not wrapped in an object, so the only work in the file is the string
    scanner: nothing else can absorb the measurement. `\\/` is in the cycle
    because it decodes to `/` and is NOT re-escaped on the way out, which is
    the one escape whose input and output lengths differ.
    """
    rng = Rng(SEED_S6 ^ 0x455343)  # "ESC"
    body = []
    for p in range(periods):
        for _ in range(5):
            body.append(ESC7_ALPHA[rng.below(len(ESC7_ALPHA))])
        body.append(ESC7_CYCLE[p % len(ESC7_CYCLE)])
    return emit(RawStr("".join(body)))


# ---- the hex-escape decoder ---------------------------------------------
# Every u16 here is a legal lone BMP code point. `0022` and `005c` are in the
# list on purpose: they decode to `"` and `\`, which the WRITER must escape
# again, so this file is also a stringify test.
BMP_ESCAPES = [
    0x0000, 0x0001, 0x0008, 0x000A, 0x000D, 0x001F, 0x0020, 0x0022, 0x002F,
    0x0041, 0x005C, 0x007F, 0x00A0, 0x00E9, 0x00FF, 0x0100, 0x0301, 0x03BB,
    0x0416, 0x05D0, 0x0627, 0x0939, 0x0E17, 0x1E9E, 0x2014, 0x2026, 0x2192,
    0x20AC, 0x3053, 0x30C6, 0x4E2D, 0x65E5, 0xAC00, 0xFB01, 0xFEFF, 0xFFFD,
    0xFFFE, 0xFFFF,
]
# Correctly paired surrogates only. `d800 dc00` is U+10000, the first astral
# code point; `dbff dfff` is U+10FFFF, the last code point that exists.
SURROGATE_PAIRS = [
    (0xD800, 0xDC00), (0xDBFF, 0xDFFF), (0xD83D, 0xDE80), (0xD83C, 0xDDEF),
    (0xD83C, 0xDDF5), (0xD83D, 0xDC69), (0xD834, 0xDD1E), (0xD840, 0xDC0B),
]


def _hex4(rng, n):
    """Mixed case on purpose: the hex decoder must accept both, and a
    case-folding bug would otherwise never show up on this corpus."""
    return ("%04X" if rng.chance(0.25) else "%04x") % n


def build_unicode_escapes(target_bytes):
    """A bare top-level JSON string that is NOTHING but `\\uXXXX` escapes, so
    escape bytes are 100% of string bytes -- the hex decoder's ceiling, where
    `s4-ocr-i18n.json` is its realistic case at 17.65%.

    One in nine escapes is a surrogate PAIR. Every pair is emitted as one
    atomic chunk and `validate` re-checks with the lone-surrogate regex, so
    this file cannot drift into the must-fail group.
    """
    rng = Rng(SEED_S6 ^ 0x554553)  # "UES"
    body = []
    size = 0
    pairs = 0
    singles = 0
    while size < target_bytes:
        if rng.below(9) == 0:
            hi, lo = rng.pick(SURROGATE_PAIRS)
            body.append("\\u" + _hex4(rng, hi) + "\\u" + _hex4(rng, lo))
            size += 12
            pairs += 1
        else:
            body.append("\\u" + _hex4(rng, rng.pick(BMP_ESCAPES)))
            size += 6
            singles += 1
    return emit(RawStr("".join(body))), {"pairs": pairs, "singles": singles}


# ---- numbers -------------------------------------------------------------
def digits(rng, n):
    out = [chr(48 + (rng.below(9) + 1))]  # never a leading zero
    for _ in range(n - 1):
        out.append(chr(48 + rng.below(10)))
    return "".join(out)


def build_int_20_digit():
    """20-digit integers on both sides of the u64 and i64 boundaries.

    Every one of these PARSES as a `Value`: serde_json falls back to f64 when
    an integer does not fit u64/i64, so "outside the range" is a TYPED
    outcome, not a parse failure. The keys name which is which, so a typed
    fixture can assert per field and a failure says which boundary moved.
    """
    rng = Rng(SEED_S6 ^ 0x494E54)  # "INT"
    bulk = []
    for i in range(N_BULK_INTS):
        if i % 2 == 0:
            # 20 digits, inside u64: [10^19, u64::MAX]. Right at the
            # `checked_mul(10)` boundary in `parse_integer`.
            bulk.append(Raw(str(rng.between(10_000_000_000_000_000_000,
                                            18_446_744_073_709_551_615))))
        else:
            # 20 digits, outside u64: a first digit of 2..9 puts every one of
            # these above 2e19 > u64::MAX, so all 1,000 take the
            # overflow-to-f64 fallback. Composed digit by digit rather than
            # drawn from a range, because the range is wider than 2^64 and
            # `below()` would silently truncate it.
            bulk.append(Raw(str(rng.between(2, 9))
                            + "".join(chr(48 + rng.below(10)) for _ in range(19))))
    return {
        "i64_max": Raw("9223372036854775807"),
        "i64_max_plus_1": Raw("9223372036854775808"),
        "i64_min": Raw("-9223372036854775808"),
        "i64_min_minus_1": Raw("-9223372036854775809"),
        "u64_max": Raw("18446744073709551615"),
        "u64_max_plus_1": Raw("18446744073709551616"),
        "twenty_digits_smallest": Raw("10000000000000000000"),
        "twenty_nines": Raw("99999999999999999999"),
        "twenty_nines_negative": Raw("-99999999999999999999"),
        "u128_max": Raw("340282366920938463463374607431768211455"),
        "u128_max_plus_1": Raw("340282366920938463463374607431768211456"),
        "note": "every token here parses as a Value; the boundaries are TYPED",
        "bulk_alternating_inside_outside_u64": bulk,
    }


def build_float_400_digit():
    """400-digit floats, and the exponent extremes that stay FINITE.

    Nothing here is expected to be an infinity, and that is the finding:
    serde_json does not produce one. `f64_from_parts` errors with "number out
    of range" the moment the value overflows, so every would-be infinity is a
    must-FAIL document and lives in `s6-fail-float-overflow.json`. The zeros
    ARE here, because underflow is not an error: `1e-400` is Ok(0.0).
    """
    rng = Rng(SEED_S6 ^ 0x464C54)  # "FLT"
    bulk = [Raw(digits(rng, 200) + "." + digits(rng, 200)) for _ in range(N_BULK_FLOATS)]
    return {
        "frac_400": Raw("0." + digits(rng, 400)),
        "int_200_frac_200": Raw(digits(rng, 200) + "." + digits(rng, 200)),
        "sig_400_small_exp": Raw(digits(rng, 400) + "e-400"),
        "f64_max": Raw("1.7976931348623157e308"),
        "e308": Raw("1e308"),
        "e308_neg": Raw("-1e308"),
        "smallest_normal": Raw("2.2250738585072014e-308"),
        "smallest_subnormal": Raw("4.9e-324"),
        "zero_by_underflow_e_minus_400": Raw("1e-400"),
        "negative_zero_by_underflow": Raw("-1e-400"),
        "zero_by_underflow_400_zeros": Raw("0." + "0" * 400 + "1"),
        "note": "expected ZERO: the four underflow tokens. expected INFINITY: "
                "none -- serde_json errors instead, see s6-fail-float-overflow",
        "bulk_400_digit": bulk,
    }


def build_exponent_zeros():
    """Very long numbers whose exponent is almost all zeros, and -0.

    The exponent accumulator is `exp = exp * 10 + digit` guarded by an i32
    overflow check, so a leading zero never advances it: `1e` + 400 zeros is
    exactly 1.0, and the token is 403 bytes long to say so. A zero
    significand or a negative exponent survives an exponent that DOES
    overflow (`0e999...`, `1e-999...` are Ok(0.0)); a positive one does not,
    and that token is in `s6-fail-exponent-overflow.json`.
    """
    rng = Rng(SEED_S6 ^ 0x455850)  # "EXP"
    z = "0" * 400
    bulk = [Raw(digits(rng, 3) + "e" + "0" * 400 + str(rng.below(10)))
            for _ in range(N_BULK_EXP)]
    return {
        "one_e_400_zeros": Raw("1e" + z),
        "one_e_plus_400_zeros_then_5": Raw("1e+" + z + "5"),
        "one_e_minus_400_zeros_then_5": Raw("1e-" + z + "5"),
        "float_e_400_zeros_then_5": Raw("1.5e" + z + "5"),
        "zero_e_400_zeros": Raw("0e" + z),
        "zero_e_overflowing_exponent": Raw("0e99999999999999999999"),
        "one_e_minus_overflowing_exponent": Raw("1e-99999999999999999999"),
        "negative_zero_int": Raw("-0"),
        "negative_zero_float": Raw("-0.0"),
        "negative_zero_exp": Raw("-0e0"),
        "negative_zero_neg_exp": Raw("-0.0e-0"),
        "zero_point_zero_e_zeros": Raw("0.0e-" + z),
        "note": "all Ok: leading zeros never advance the exponent accumulator; "
                "a zero significand or a negative exponent survives i32 overflow",
        "bulk_400_zero_exponents": bulk,
    }


# ---- keys ----------------------------------------------------------------
def build_dup_key(n):
    """One object, one key name, `n` members. Last wins.

    `Map` is a `BTreeMap` by default (and an `IndexMap` under
    `preserve_order`); both overwrite on insert, so the parsed value has
    exactly one member and it is the last. `to_string` of the result is 12
    bytes -- a 130 KB document that round-trips to nothing, which is the
    point.
    """
    parts = ['{"dup":0']
    for i in range(1, n):
        parts.append(',"dup":%d' % i)
    parts.append("}")
    return "".join(parts).encode("ascii")


def build_repeated_keys(depth, arity):
    """The SAME key at every level of a deep nest, then one very wide object.

    `s6-deep-object-100.json` is the cold arm (a distinct key per level, reuse
    1.0); this is the hot arm (reuse `depth`), and the innermost object has
    `arity` distinct members -- 1,000 against citm_catalog's 184, the widest
    object in JSONCORP.
    """
    wide = {}
    for i in range(arity):
        wide["f%04d" % i] = i
    node = wide
    for _ in range(depth):
        node = {"k": node}
    return node


# ---- structure -----------------------------------------------------------
CHURN_SHAPES = [
    [], {}, [[]], [{}], [[[]]], [[[[]]]], [[[[[]]]]],
    {"a": {}}, {"a": []}, {"a": {"b": {}}}, {"a": {"b": {"c": []}}},
    [{}, []], [[], []], [{}, {}, {}], [[], [[]], [[[]]]],
    {"a": [{}]}, [{"a": []}, {}], [[], {}, [[]], {"a": {}}],
    {"a": [], "b": {}}, [[[], []], [{}, {}]],
]


def build_structural_churn(n):
    """`n` tiny empty structures in one array: the maximum structural-byte
    density in JSONCORP, with no payload at all to hide behind.

    "Many tiny documents' worth" of churn, delivered as ONE valid document so
    it is a must-parse corpus member rather than a second stream. The bytes
    are almost entirely `[`, `]`, `{`, `}` and `,`, which is the arm B5 and
    B15 have never had: canada is 9.92% structural, `s4-frame-telemetry` is
    34.14%, and this is above 90%.
    """
    rng = Rng(SEED_S6 ^ 0x434855)  # "CHU"
    return [rng.pick(CHURN_SHAPES) for _ in range(n)]


# ---- the planted-hazard (must-fail) files --------------------------------
GOOD_STRINGS = [
    "plain ascii line",
    "escapes \\t and \\n and \\\" inside",
    "paired surrogate \\ud83d\\ude80 rocket",
    "paired surrogate \\ud800\\udc00 first astral",
    "bmp escape caf\\u00e9 and \\u2014 dash",
    "windows path F:\\\\mata\\\\node\\\\store",
    "max code point \\udbff\\udfff",
    "control \\u0000 then text",
]


def planted_strings(hazard, n=FAIL_ELEMS, at=FAIL_AT):
    """A pretty-printed array of `n` valid strings with ONE hazard at index
    `at`, so the error's LINE and COLUMN are both deep in the file.

    An 8-byte document proves the message; only a planted one proves the
    position, and `error line/col identity` is S6's actual gate. One hazard
    per file: the parser stops at the first, so a file with two hazards only
    ever tests the first.
    """
    rng = Rng(SEED_S6 ^ 0x484150)  # "HAP"
    out = []
    for i in range(n):
        out.append(RawStr(hazard) if i == at else RawStr(rng.pick(GOOD_STRINGS)))
    return out


def planted_numbers(hazard, good, n=FAIL_ELEMS, at=FAIL_AT):
    rng = Rng(SEED_S6 ^ 0x48414E)  # "HAN"
    return [Raw(hazard) if i == at else Raw(rng.pick(good)) for i in range(n)]


GOOD_FLOATS = ["1e308", "-1e308", "1.7976931348623157e308", "4.9e-324",
               "1e-400", "0.5", "-0.0", "2.2250738585072014e-308"]
GOOD_INTS = ["18446744073709551615", "9223372036854775807", "-9223372036854775808",
             "10000000000000000000", "99999999999999999999", "0", "-0"]


# ==========================================================================
# The specification table. One row per file: this is the single source of
# truth for the manifest's must-parse / must-fail split.
#
# serde  : "ok"  -- from_slice::<Value> succeeds
#          "err" -- from_slice::<Value> fails; `reason` names the error
#          "stream" -- MANY documents: from_slice fails with "trailing
#                      characters" and StreamDeserializer yields every one
# python : what json.loads does to the WHOLE file ("ok" / "err" / "stream")
# ==========================================================================
def build_all(scale=1):
    """Every file, as (dirname, name, bytes, spec). Deterministic."""
    files = []

    ndjson = build_log_ndjson(N_LOG_LINES * scale)
    pretty = build_log_pretty(N_PRETTY_DOCS * scale)
    files.append(("s5", "s5-log-stream.ndjson", ndjson, {
        "group": "stream", "serde": "stream", "python": "stream",
        "docs": N_LOG_LINES * scale, "ndjson": True,
        "reason": "many documents: one per line, no enclosing array",
        "prices": "B7 IoRead and B13 RawValue-over-a-reader; per-document "
                  "overhead at 10,000 documents in one file",
    }))
    files.append(("s5", "s5-log-stream-pretty.json", pretty, {
        "group": "stream", "serde": "stream", "python": "stream",
        "docs": N_PRETTY_DOCS * scale,
        "reason": "many documents separated by whitespace, including none",
        "prices": "StreamDeserializer's inter-document whitespace path: "
                  "leading, trailing, ten separator shapes, and `}{`",
    }))

    uesc, uesc_stats = build_unicode_escapes(UESC_TARGET)

    s6 = [
        ("s6-deep-array-100.json", deep_array(DEEP), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "depth 100, under serde_json's limit of 128",
            "prices": "the recursion path at the plan's stated depth; "
                      "`check_recursion` taken 100 times, no key work at all",
        }),
        ("s6-deep-object-100.json", deep_object_distinct(DEEP), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "depth 100 objects, a DISTINCT key per level",
            "prices": "depth with COLD key dispatch: 100 keys, reuse 1.0",
        }),
        ("s6-deep-mixed-127.json", deep_mixed(DEEP_MAX), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "depth 127 alternating array/object: the DEEPEST that parses",
            "prices": "the exact boundary, from the passing side",
        }),
        ("s6-string-escape-7.json", build_escape_every_7(ESC7_PERIODS), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "one ~1 MB string, an escape sequence every 7 bytes",
            "prices": "B2's dirtiest arm: a wide string scan stops every 5 "
                      "bytes. B3's re-escape path on the way out",
        }),
        ("s6-unicode-escapes.json", uesc, {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "one ~1 MB string that is 100%% `\\uXXXX`, %d of them "
                      "surrogate PAIRS" % uesc_stats["pairs"],
            "prices": "the hex decoder's ceiling, and scratch-buffer growth "
                      "to ~350 KB inside one string",
        }),
        ("s6-int-20-digit.json", emit(build_int_20_digit(), 2), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "20-digit integers either side of u64/i64; all PARSE",
            "prices": "the >u64 overflow-to-f64 fallback on 1,000 of the "
                      "2,000 bulk tokens; the other 1,000 sit just inside u64",
        }),
        ("s6-float-400-digit.json", emit(build_float_400_digit(), 2), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "400-digit floats and the FINITE exponent extremes",
            "prices": "`parse_long_integer`'s scratch path at 400 digits; "
                      "underflow-to-zero, which is not an error",
        }),
        ("s6-exponent-zeros.json", emit(build_exponent_zeros(), 2), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "400-zero exponents, overflowing exponents that are "
                      "still Ok, and -0",
            "prices": "the exponent accumulator and its i32 overflow guard",
        }),
        ("s6-dup-key-last-wins.json", build_dup_key(N_DUP_KEYS), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "one object, the key \"dup\" %d times; LAST WINS" % N_DUP_KEYS,
            "prices": "Map::insert overwriting 9,999 times; a 130 KB document "
                      "whose Value is 12 bytes",
        }),
        ("s6-repeated-keys.json", emit(build_repeated_keys(DEEP, WIDE_ARITY), 0), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "depth 101: the SAME key 100 levels deep, then a "
                      "%d-member object" % WIDE_ARITY,
            "prices": "key reuse 100 on the deep path, and the widest object "
                      "in JSONCORP (citm_catalog's record is 184)",
        }),
        ("s6-structural-churn.json", emit(build_structural_churn(N_CHURN), 0), {
            "group": "parse", "serde": "ok", "python": "ok",
            "reason": "%d tiny empty structures in one array" % N_CHURN,
            "prices": "B5/B15 at the highest structural-byte share in "
                      "JSONCORP, with no payload to hide behind",
        }),
        # ---- must FAIL ---------------------------------------------------
        ("s6-fail-deep-array-128.json", deep_array(DEEP_OVER), {
            "group": "fail", "serde": "err", "python": "ok",
            "reason": "depth 128 = serde_json's recursion limit: "
                      "\"recursion limit exceeded\"",
            "diverges": "Python's limit is the interpreter recursion limit "
                        "(~1000), so json.loads accepts this",
            "prices": "the recursion limit from the failing side, on line 1",
        }),
        ("s6-fail-deep-object-128.json", deep_object_pretty(DEEP_OVER), {
            "group": "fail", "serde": "err", "python": "ok",
            "reason": "depth 128 objects, one level per line: "
                      "\"recursion limit exceeded\" on LINE 128",
            "diverges": "Python's limit is the interpreter recursion limit "
                        "(~1000), so json.loads accepts this",
            "prices": "the LINE half of error identity -- every other "
                      "recursion case in the repo reports line 1",
        }),
        ("s6-fail-lone-trailing-surrogate.json",
         emit(planted_strings("lone trailing \\udc00 here"), 2), {
             "group": "fail", "serde": "err", "python": "ok",
             "reason": "a trailing surrogate with no leading one: "
                       "\"lone leading surrogate in hex escape\" (the message "
                       "is upstream's; src/read.rs marks it XXX)",
             "diverges": "Python accepts lone surrogates and returns a str "
                         "containing one",
             "prices": "the first surrogate branch, at a deep line and column",
         }),
        ("s6-fail-lone-leading-surrogate.json",
         emit(planted_strings("lone leading \\ud800 here"), 2), {
             "group": "fail", "serde": "err", "python": "ok",
             "reason": "a leading surrogate followed by a plain byte: "
                       "\"unexpected end of hex escape\"",
             "diverges": "Python accepts lone surrogates",
             "prices": "the branch where the second escape never starts",
         }),
        ("s6-fail-unpaired-hex-pair.json",
         emit(planted_strings("pair broken \\ud800\\u0041 here"), 2), {
             "group": "fail", "serde": "err", "python": "ok",
             "reason": "a leading surrogate followed by a NON-trailing escape: "
                       "\"lone leading surrogate in hex escape\"",
             "diverges": "Python accepts lone surrogates",
             "prices": "the third surrogate branch -- a well-formed second "
                       "escape that is the wrong half",
         }),
        ("s6-fail-float-overflow.json",
         emit(planted_numbers("1e309", GOOD_FLOATS), 2), {
             "group": "fail", "serde": "err", "python": "ok",
             "reason": "1e309 overflows f64: \"number out of range\". "
                       "serde_json NEVER yields an infinity",
             "diverges": "Python's float saturates: json.loads returns inf",
             "prices": "`f64_from_parts`'s is_infinite guard",
         }),
        ("s6-fail-int-400-digit.json",
         emit(planted_numbers(digits(Rng(SEED_S6 ^ 0x424947), 400), GOOD_INTS), 2), {
             "group": "fail", "serde": "err", "python": "ok",
             "reason": "a 400-DIGIT integer is ~1e399, which overflows f64: "
                       "\"number out of range\"",
             "diverges": "Python's ints are arbitrary precision: json.loads "
                         "returns the exact 400-digit integer",
             "prices": "`parse_long_integer`'s overflow guard, which is a "
                       "different site from the exponent one",
         }),
        ("s6-fail-exponent-overflow.json",
         emit(planted_numbers("1e2147483648", GOOD_FLOATS), 2), {
             "group": "fail", "serde": "err", "python": "ok",
             "reason": "the exponent overflows i32 with a non-zero "
                       "significand: \"number out of range\"",
             "diverges": "Python's float saturates: json.loads returns inf",
             "prices": "`parse_exponent_overflow`, the third and coldest of "
                       "the three number-range sites",
         }),
    ]
    for name, blob, spec in s6:
        files.append(("s6", name, blob, spec))
    return files, uesc_stats


# ==========================================================================
# Validation. Python is a check, never the oracle.
# ==========================================================================
LONE_SURROGATE = re.compile(rb"\\u[dD][89abAB][0-9a-fA-F]{2}(?!\\u[dD][c-fC-F])")


def python_stream(buf):
    """Split a whitespace-separated concatenation of JSON documents with
    Python's own decoder. Returns (documents, inter-document ws char runs)."""
    text = buf.decode("utf-8")
    dec = json.JSONDecoder()
    out = []
    runs = []
    i = 0
    n = len(text)
    while True:
        j = i
        while j < n and text[j] in " \t\n\r":
            j += 1
        runs.append(j - i)
        if j >= n:
            break
        doc, end = dec.raw_decode(text, j)
        out.append(doc)
        i = end
    return out, runs


def validate(name, blob, spec):
    """Every gate that can be run without building the crate.

    `json.loads` is used as a CHECK against the declared Python column, not as
    the oracle: four must-fail files parse cleanly here and say so in
    `diverges`.
    """
    blob.decode("utf-8")  # every file must be valid UTF-8

    if spec["serde"] == "stream":
        if spec.get("ndjson"):
            # Line-based, because NDJSON's contract IS one document per line:
            # a line that does not parse on its own is the defect, and this
            # stays linear at --scale 39 where raw_decode over 100 MB does not.
            lines = blob.decode("utf-8").split("\n")
            if lines[-1] != "":
                raise SystemExit("%s: NDJSON must end with a newline" % name)
            docs = [json.loads(l) for l in lines[:-1]]
        else:
            docs, _ = python_stream(blob)
        if len(docs) != spec["docs"]:
            raise SystemExit("%s: python found %d documents, expected %d"
                             % (name, len(docs), spec["docs"]))
        try:
            json.loads(blob.decode("utf-8"))
        except ValueError:
            pass
        else:
            raise SystemExit("%s: a multi-document stream must NOT parse as one "
                             "document; json.loads accepted it" % name)
        return docs

    try:
        doc = json.loads(blob.decode("utf-8"))
        py = "ok"
    except (ValueError, RecursionError):
        doc = None
        py = "err"
    if py != spec["python"]:
        raise SystemExit("%s: json.loads says %s, spec says %s"
                         % (name, py, spec["python"]))
    if spec["group"] == "fail" and "diverges" not in spec and py == "ok":
        raise SystemExit("%s: a must-fail file that Python accepts needs a "
                         "`diverges` reason" % name)

    # A must-PARSE file must not carry a lone surrogate: json.loads accepts
    # one and serde_json does not, so this is the check Python cannot make.
    if spec["group"] != "fail" and LONE_SURROGATE.search(blob):
        raise SystemExit("%s: lone surrogate escape in a must-parse file" % name)
    if spec["group"] != "fail" and doc is not None:
        def walk(v):
            if isinstance(v, str):
                v.encode("utf-8")  # raises on a surrogate that survived json
            elif isinstance(v, dict):
                for k, x in v.items():
                    k.encode("utf-8")
                    walk(x)
            elif isinstance(v, list):
                for x in v:
                    walk(x)
        walk(doc)
    return doc


def validate_pair(ndjson, pretty, n_pretty):
    """The pretty stream must be the SAME data as the first `n_pretty` lines of
    the NDJSON file, or the two files measure different documents and the
    manifest's claim is false."""
    a = [json.loads(l) for l in ndjson.decode("utf-8").splitlines()[:n_pretty]]
    b, _ = python_stream(pretty)
    if a != b:
        raise SystemExit("s5: the pretty stream is not the same data as the "
                         "first %d NDJSON lines" % n_pretty)


def validate_scale_prefix():
    """Record i must be the same bytes at every --scale, or a 100 MB run is a
    different corpus from the checked-in one. Checked on the first 8 records,
    which is enough: the seed is a pure function of i."""
    a = build_log_ndjson(8)
    b = build_log_ndjson(64)
    if not b.startswith(a):
        raise SystemExit("--scale is not prefix-stable: record bytes depend on "
                         "the record COUNT, so a scaled stream is a different "
                         "corpus")


# ==========================================================================
# Line and stream statistics -- the two things S4's census does not report
# and S5's whole point needs.
# ==========================================================================
def line_stats(buf):
    lines = buf.split(b"\n")
    trailing = lines[-1] == b""
    if trailing:
        lines = lines[:-1]
    lens = [len(l) for l in lines]
    return {
        "lines": len(lens),
        "min": min(lens) if lens else 0,
        "max": max(lens) if lens else 0,
        "mean": (sum(lens) / float(len(lens))) if lens else 0.0,
        "trailing_newline": trailing,
        "cr": buf.count(b"\r"),
    }


def record_inventory(buf):
    """What is actually IN the log lines, as counts rather than intentions.

    The `m4=0` lesson applies to a corpus file too: "some lines carry an
    escape" is not a number, and a brick gated on content that is not there
    proves nothing. Counted on the raw line bytes, so an escape means an
    escape in the FILE, not in the decoded string.
    """
    inv = {"levels": {}, "targets": set(), "trace": 0, "err_obj": 0,
           "backslash": 0, "non_ascii": 0, "null_err": 0}
    for line in buf.split(b"\n"):
        if not line:
            continue
        doc = json.loads(line.decode("utf-8"))
        inv["levels"][doc["lvl"]] = inv["levels"].get(doc["lvl"], 0) + 1
        inv["targets"].add(doc["tgt"])
        if "trace" in doc:
            inv["trace"] += 1
        if doc["err"] is None:
            inv["null_err"] += 1
        else:
            inv["err_obj"] += 1
        if b"\\" in line:
            inv["backslash"] += 1
        if any(b >= 0x80 for b in line):
            inv["non_ascii"] += 1
    return inv


def stream_stats(buf, docs):
    """Inter-document whitespace: how much, in how many runs, how long."""
    _, runs = python_stream(buf)
    inner = runs[1:-1] if len(runs) >= 2 else []
    return {
        "docs": docs,
        "lead_ws": runs[0] if runs else 0,
        "tail_ws": runs[-1] if runs else 0,
        "sep_runs": len(inner),
        "sep_zero": sum(1 for r in inner if r == 0),
        "sep_bytes": sum(inner),
        "sep_max": max(inner) if inner else 0,
        "cr": buf.count(b"\r"),
    }


# ==========================================================================
# Drive it.
# ==========================================================================
GITATTRIBUTES_S5 = """\
# S5 payloads are DATA, keyed by SHA-256 in MANIFEST.md and streamed byte for
# byte. The root `.gitattributes` already pins `corpus/**/*.json`, but NOT
# `*.ndjson`, which is the extension this scenario introduces -- and the
# NDJSON file is ALL line endings, so a `core.autocrlf=true` checkout would
# add 10,000 carriage returns, change the size, the whitespace census and the
# hash, and quietly benchmark a different document. `-text` pins both.
*.json -text
*.ndjson -text
"""

GITATTRIBUTES_S6 = """\
# S6 payloads are DATA, keyed by SHA-256 in MANIFEST.md, and six of them exist
# to produce an EXACT error message with an exact line and column. A checkout
# that rewrote line endings would move every one of those columns, so the
# must-fail files would still fail -- at the wrong position, which the oracle
# reports as a mismatch. The root `.gitattributes` already pins
# `corpus/**/*.json`; this is the local, visible statement of the same rule.
*.json -text
"""


def main():
    argv = sys.argv[1:]
    check = "--check" in argv
    scale = 1
    out_dir = None
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--scale":
            i += 1
            scale = int(argv[i])
        elif a == "--out":
            i += 1
            out_dir = os.path.abspath(argv[i])
        elif a == "--check":
            pass
        else:
            raise SystemExit("usage: gen-s5-s6.py [--check] [--scale N --out DIR]")
        i += 1
    if scale < 1:
        raise SystemExit("--scale must be >= 1")
    if scale != 1 and out_dir is None:
        raise SystemExit(
            "--scale %d needs --out DIR. The checked-in corpus is scale 1 and\n"
            "every hash in MANIFEST.md is taken on it; a scaled stream is a\n"
            "reproducible artefact, not a corpus member." % scale)
    if out_dir and os.path.normcase(out_dir).startswith(os.path.normcase(CORPUS_DIR)):
        raise SystemExit("--out must be OUTSIDE corpus/: %s" % out_dir)
    if scale != 1 and check:
        raise SystemExit("--check is defined at scale 1 only")

    src = open(os.path.abspath(__file__), "rb").read()
    try:
        src.decode("ascii")
    except UnicodeDecodeError:
        raise SystemExit("gen-s5-s6.py must stay ASCII (house lint)")

    files, uesc_stats = build_all(scale)
    if scale == 1:
        validate_scale_prefix()

    by_name = dict((n, b) for _, n, b, _ in files)
    for _, name, blob, spec in files:
        validate(name, blob, spec)
    if scale == 1:
        # A scale-1 invariant: both S5 files come from the same record
        # builder, and `validate_scale_prefix` already proves record i does
        # not depend on the record count.
        validate_pair(by_name["s5-log-stream.ndjson"],
                      by_name["s5-log-stream-pretty.json"], N_PRETTY_DOCS)

    # ---- write ----------------------------------------------------------
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
        for sub, name, blob, _ in files:
            if sub != "s5" and scale != 1:
                continue
            with open(os.path.join(out_dir, name), "wb") as fh:
                fh.write(blob)
        if scale != 1:
            # No census and no tables here: every number in MANIFEST.md is a
            # scale-1 fact, and a byte-loop census over 100 MB of Python would
            # cost minutes to restate them wrongly.
            print("S5 at scale %d, written to %s\n" % (scale, out_dir))
            for sub, name, blob, spec in files:
                if sub != "s5":
                    continue
                print("%-30s %12d bytes  %d documents  sha256 %s"
                      % (name, len(blob), spec["docs"],
                         hashlib.sha256(blob).hexdigest()))
            ls = line_stats(by_name["s5-log-stream.ndjson"])
            print("\n%d lines, %d..%d bytes (mean %.1f). Seeding is per record,"
                  " so the first\n%d lines of the NDJSON file are byte-identical"
                  " to the checked-in one, and\nthe pretty stream is identical"
                  " up to the end of document %d -- only its\n%d trailing"
                  " whitespace bytes become a separator once more documents"
                  " follow."
                  % (ls["lines"], ls["min"], ls["max"], ls["mean"], N_LOG_LINES,
                     N_PRETTY_DOCS - 1, len(TAIL_WS)))
            return 0
    elif not check:
        for sub, attrs in (("s5", GITATTRIBUTES_S5), ("s6", GITATTRIBUTES_S6)):
            d = os.path.join(CORPUS_DIR, sub)
            os.makedirs(d, exist_ok=True)
            with open(os.path.join(d, ".gitattributes"), "wb") as fh:
                fh.write(attrs.encode("ascii"))
        for sub, name, blob, _ in files:
            with open(os.path.join(CORPUS_DIR, sub, name), "wb") as fh:
                fh.write(blob)

    # ---- census ---------------------------------------------------------
    rows = []
    for sub, name, blob, spec in files:
        c = census(blob)
        accounted = (c["whitespace"] + c["structural"] + c["string_bytes"]
                     + c["number_bytes"] + c["literal_bytes"])
        if accounted != c["bytes"]:
            raise SystemExit("%s: %d of %d bytes unaccounted by the census"
                             % (name, c["bytes"] - accounted, c["bytes"]))
        rows.append((sub, name, c, hashlib.sha256(blob).hexdigest(), spec, blob))

    refs = []
    for ref in ("twitter.json", "citm_catalog.json", "canada.json"):
        p = os.path.join(CORPUS_DIR, ref)
        if os.path.exists(p):
            refs.append((ref, census(open(p, "rb").read())))
    for ref in ("s4-ocr-i18n.json", "s4-vault-shard.json", "s4-frame-telemetry.json"):
        p = os.path.join(CORPUS_DIR, "s4", ref)
        if os.path.exists(p):
            refs.append((ref, census(open(p, "rb").read())))

    print("S5 + S6 -- seeds 0x%X / 0x%X, %d files, scale %d\n"
          % (SEED_S5, SEED_S6, len(rows), scale))

    print("%-38s %-6s %-6s %s" % ("file", "group", "serde", "expectation"))
    print("-" * 124)
    for _, name, _, _, spec, _ in rows:
        print("%-38s %-6s %-6s %s" % (name, spec["group"], spec["serde"],
                                      spec["reason"]))
    print("\npython column (json.loads on the whole file), and where it DIVERGES")
    print("-" * 124)
    for _, name, _, _, spec, _ in rows:
        print("%-38s python=%-7s %s" % (name, spec["python"],
                                        spec.get("diverges", "(agrees)")))

    hdr1 = ("file", "bytes", "ws%", "str%", "num%", "nonasc%", "lit%", "struct%",
            "depth", "objects", "arrays", "keys")
    print("\n%-38s %10s %7s %7s %7s %8s %6s %8s %6s %8s %7s %7s" % hdr1)
    print("-" * 138)

    def row1(name, c):
        print("%-38s %10d %7.2f %7.2f %7.2f %8.2f %6.2f %8.2f %6d %8d %7d %7d" % (
            name, c["bytes"], pct(c["whitespace"], c["bytes"]),
            pct(c["string_bytes"], c["bytes"]), pct(c["number_bytes"], c["bytes"]),
            pct(c["non_ascii"], c["bytes"]), pct(c["literal_bytes"], c["bytes"]),
            pct(c["structural"], c["bytes"]), c["depth"], c["objects"], c["arrays"],
            c["keys"]))

    for _, name, c, _, _, _ in rows:
        row1(name, c)
    print("-- S1-S3 and three S4 files, same census code (validates the port) --")
    for name, c in refs:
        row1(name, c)

    hdr2 = ("file", "strings", "keys", "esc-str", "esc-B", "escB%str", "mean-str",
            "longest-str")
    print("\n%-38s %9s %8s %8s %9s %9s %9s %12s" % hdr2)
    print("-" * 138)

    def row2(name, c):
        print("%-38s %9d %8d %8d %9d %9.2f %9.1f %12d" % (
            name, c["strings"], c["keys"], c["strings_with_escapes"],
            c["escape_bytes"], pct(c["escape_bytes"], c["string_bytes"]),
            (c["string_bytes"] / float(c["strings"])) if c["strings"] else 0.0,
            c["longest_string"]))

    for _, name, c, _, _, _ in rows:
        row2(name, c)
    print("-- reference --")
    for name, c in refs:
        row2(name, c)

    hdr3 = ("file", "numbers", "floats", "int-only", "num/KB", "ws-runs", "mean-ws",
            "ws>=8B%", "longest-ws")
    print("\n%-38s %9s %8s %9s %8s %9s %8s %8s %11s" % hdr3)
    print("-" * 138)

    def row3(name, c):
        runs = sum(c["ws_runs_by_len"])
        print("%-38s %9d %8d %9d %8.1f %9d %8.2f %8.1f %11d" % (
            name, c["numbers"], c["floats"], c["numbers"] - c["floats"],
            c["numbers"] * 1024.0 / c["bytes"], runs,
            (c["whitespace"] / float(runs)) if runs else 0.0,
            pct(sum(c["ws_bytes_by_len"][4:]), c["whitespace"]), c["longest_ws_run"]))

    for _, name, c, _, _, _ in rows:
        row3(name, c)
    print("-- reference --")
    for name, c in refs:
        row3(name, c)

    hdr4 = ("file", "keys/KB", "distinct", "reuse", "max-obj", "max-arr", "literals",
            "lit/KB")
    print("\n%-38s %8s %9s %8s %8s %9s %9s %8s" % hdr4)
    print("-" * 138)

    def row4(name, c):
        d = len(c["distinct_keys"])
        print("%-38s %8.1f %9d %8.1f %8d %9d %9d %8.1f" % (
            name, c["keys"] * 1024.0 / c["bytes"], d,
            (c["keys"] / float(d)) if d else 0.0, c["max_obj_arity"],
            c["max_arr_arity"], c["literals"], c["literals"] * 1024.0 / c["bytes"]))

    for _, name, c, _, _, _ in rows:
        row4(name, c)
    print("-- reference --")
    for name, c in refs:
        row4(name, c)

    # ---- S5-specific: lines and inter-document whitespace ----------------
    nd = by_name["s5-log-stream.ndjson"]
    pr = by_name["s5-log-stream-pretty.json"]
    ls = line_stats(nd)
    ss = stream_stats(pr, N_PRETTY_DOCS * scale)
    print("\nS5 lines and documents")
    print("-" * 124)
    print("s5-log-stream.ndjson:  %d lines, %d..%d bytes (mean %.1f), "
          "trailing newline %s, CR bytes %d"
          % (ls["lines"], ls["min"], ls["max"], ls["mean"],
             "yes" if ls["trailing_newline"] else "no", ls["cr"]))
    print("s5-log-stream-pretty.json: %d documents, leading ws %d, trailing ws %d, "
          "%d separators (%d of them EMPTY), %d separator bytes, longest %d, CR %d"
          % (ss["docs"], ss["lead_ws"], ss["tail_ws"], ss["sep_runs"], ss["sep_zero"],
             ss["sep_bytes"], ss["sep_max"], ss["cr"]))
    print("bytes per NDJSON record: %.1f  ->  --scale %d is about %.1f MB"
          % (len(nd) / float(N_LOG_LINES * scale),
             (100 * 1024 * 1024) // (len(nd) // scale) + 1,
             ((100 * 1024 * 1024) // (len(nd) // scale) + 1) * (len(nd) / scale)
             / 1048576.0))
    inv = record_inventory(nd)
    print("S5 record inventory over %d lines: %d targets, levels %s"
          % (ls["lines"], len(inv["targets"]),
             ", ".join("%s=%d" % kv for kv in sorted(inv["levels"].items()))))
    print("  trace id present %d, err object %d, err null %d, "
          "line carries a `\\` escape %d, line carries raw non-ASCII %d"
          % (inv["trace"], inv["err_obj"], inv["null_err"],
             inv["backslash"], inv["non_ascii"]))

    print("\nS6 unicode-escape inventory: %d single BMP escapes, %d surrogate PAIRS"
          % (uesc_stats["singles"], uesc_stats["pairs"]))

    # ---- predicted error positions, for the manifest --------------------
    print("\nPredicted must-fail positions (SliceRead formula from src/read.rs;\n"
          "confirmed against upstream serde_json -- see MANIFEST.md)")
    print("-" * 124)
    for _, name, _, _, spec, blob in rows:
        if spec["group"] != "fail":
            continue
        if "deep-array" in name or "deep-object" in name:
            idx = nth_byte(blob, ord("[") if "array" in name else ord("{"), DEEP_OVER)
            line, col = peek_position(blob, idx)
            print("%-38s peek at byte %d -> line %d column %d" % (name, idx, line, col))
        else:
            print("%-38s hazard in element %d of %d (measured, see MANIFEST.md)"
                  % (name, FAIL_AT, FAIL_ELEMS))

    print("\nSHA-256")
    for sub, name, _, h, _, _ in rows:
        print("%s *%s/%s" % (h, sub, name))

    for sub in ("s5", "s6"):
        tot = sum(c["bytes"] for s, _, c, _, _, _ in rows if s == sub)
        cnt = sum(1 for s, _, _, _, _, _ in rows if s == sub)
        print("\n%s: %d bytes (%.2f MB) in %d files" % (sub, tot, tot / 1048576.0, cnt))

    if check:
        bad = 0
        for sub, name, _, _, _, blob in rows:
            p = os.path.join(CORPUS_DIR, sub, name)
            on_disk = open(p, "rb").read() if os.path.exists(p) else b""
            if on_disk != blob:
                print("MISMATCH %s/%s (disk %d bytes, generated %d)"
                      % (sub, name, len(on_disk), len(blob)))
                bad += 1
        for sub, attrs in (("s5", GITATTRIBUTES_S5), ("s6", GITATTRIBUTES_S6)):
            p = os.path.join(CORPUS_DIR, sub, ".gitattributes")
            on_disk = open(p, "rb").read() if os.path.exists(p) else b""
            if on_disk != attrs.encode("ascii"):
                print("MISMATCH %s/.gitattributes" % sub)
                bad += 1
        print("\n--check: %s" % ("%d file(s) differ" % bad if bad else "byte-identical"))
        return 1 if bad else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
