#!/usr/bin/env python3
# ASCII-only source (house lint). Non-ASCII payload content is written with
# \uXXXX escapes here and lands as real UTF-8 bytes in the .json output --
# corpus/s4/*.json are DATA, not source, so real non-ASCII there is correct.
"""Generate the S4 house-payload corpus: JSON shaped like the payloads the
Remade-With-Rust house actually moves.

Why S4 exists
-------------
S1-S3 (twitter / citm_catalog / canada) are three famous, deliberately
orthogonal outliers. They are not the JSON this crate will parse in
production: no house service emits a 2.2 MB array of float pairs, and none of
the three contains an untagged enum, a `flatten` tail, a `Vec<u8>` field, a
90 KB string, a minified body, or the same 20 keys repeated a few thousand
times. S4 supplies those, so a brick is priced against real work.

Determinism
-----------
A corpus that changes between runs silently invalidates every measurement ever
taken against it. So:

  * The only entropy is SEED, expanded by a splitmix64 written out below --
    NOT `random`, whose stream is a Python implementation detail.
  * Floats never reach the writer. Every fractional token is produced by
    `num()` as a fixed-point decimal string, so no `float.__repr__` decision
    can move a byte.
  * Dict order is insertion order: what a real producer emits, kept stable.
  * Output is LF, no trailing newline, and `corpus/s4/.gitattributes` marks
    the payloads `-text` so no checkout can retranslate them. That hazard is
    not hypothetical: this box has `core.autocrlf=true`, which is why
    `twitter.json` is 646,995 bytes in the working tree and 631,515 in
    `corpus/README.md` -- one `\\r` per line, 2.4% of the file, counted as
    whitespace by the census.

Run
---
  python tools/gen-s4.py            # write corpus/s4/*.json + print the census
  python tools/gen-s4.py --check    # regenerate in memory; fail if disk differs
"""

import base64
import hashlib
import json
import os
import re
import sys

SEED = 0x53345F484F555345  # "S4_HOUSE"

# Sized so a 250 ms benchmark window sees a useful iteration count without
# checking in bytes that prove nothing. Tuned once, then frozen: changing any
# of these changes every hash in MANIFEST.md.
N_PROBE_FILES = 75
N_FRAMES = 2600
N_ASSERTIONS = 600
N_SYNC_ENTRIES = 280
N_SHARDS = 4
SHARD_RAW_BYTES = 64 * 1024
N_FFAI_ITEMS = 90

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
OUT_DIR = os.path.join(REPO, "corpus", "s4")
CORPUS_DIR = os.path.join(REPO, "corpus")

MASK64 = (1 << 64) - 1


# --------------------------------------------------------------------------
# The random stream: splitmix64, written out so it cannot drift with Python.
# --------------------------------------------------------------------------
class Rng(object):
    __slots__ = ("s",)

    def __init__(self, seed):
        self.s = seed & MASK64

    def u64(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK64
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


# --------------------------------------------------------------------------
# The writer. serde_json's escaping rules, serde_json's pretty layout.
# --------------------------------------------------------------------------
class Raw(object):
    """A verbatim JSON token -- used for every number, so no float repr runs."""

    __slots__ = ("text",)

    def __init__(self, text):
        self.text = text


class RawStr(object):
    """A verbatim JSON string BODY; the quotes are added, the body is not
    touched. This is how the escape-heavy file carries `\\uXXXX` sequences and
    surrogate pairs exactly as a foreign producer would have written them."""

    __slots__ = ("body",)

    def __init__(self, body):
        self.body = body


_NEEDS_ESCAPE = re.compile(r'[\x00-\x1f"\\]')
_SHORT = {0x08: "\\b", 0x09: "\\t", 0x0A: "\\n", 0x0C: "\\f", 0x0D: "\\r"}


def _esc(s):
    if not _NEEDS_ESCAPE.search(s):
        return s  # the common case, and the one the vault file needs to be fast
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
# Identifier shapes the house actually uses.
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
    # `is_valid_did()` in kms-types accepts `did:mata:<base58-pubkey>`.
    return "did:mata:" + b58(rng.bytes(32))


def iso(rng, base=1789000000, span=86400):
    t = base + rng.below(span)
    # Fixed civil-time arithmetic, no locale, no tz database.
    days, rem = divmod(t, 86400)
    h, rem = divmod(rem, 3600)
    m, s = divmod(rem, 60)
    y, mo, d = _civil(days + 719468)
    return "%04d-%02d-%02dT%02d:%02d:%02dZ" % (y, mo, d, h, m, s)


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


# --------------------------------------------------------------------------
# The content census: a byte-for-byte port of
# crates/rusty_json_turbo-bench/src/content.rs, so an S4 row is directly
# comparable with the S1-S3 rows the ledger already carries. Verified against
# the three existing files in `main()`.
#
# Extended with depth / object / array / key counts, which the Rust instrument
# does not report and the manifest asks for.
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
        # Not in content.rs. These four are what actually bound B6 and B10:
        # a key-dispatch table is sized by DISTINCT keys, a per-object compare
        # chain by arity, and a bulk map build by how many entries one
        # `visit_map` sees.
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
            # A string is a KEY iff the next non-whitespace byte is ':'.
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


# ==========================================================================
# 1. s4-media-probe.json -- rffprobe -print_format json, over a media library
# ==========================================================================
CONTAINERS = [
    ("mp4", "QuickTime / MOV", "mp4", 100),
    ("matroska,webm", "Matroska / WebM", "mkv", 100),
    ("avi", "AVI (Audio Video Interleaved)", "avi", 100),
    ("mov,mp4,m4a,3gp,3g2,mj2", "QuickTime / MOV", "mov", 100),
    ("webm", "Matroska / WebM", "webm", 100),
]
VCODECS = [
    ("h264", "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (rusty_h264)", "High", "avc1", 41),
    ("hevc", "H.265 / HEVC (High Efficiency Video Coding) (rusty_h265)", "Main 10", "hvc1", 120),
    ("av1", "Alliance for Open Media AV1 (rusty-av1-toolkit)", "Main", "av01", 8),
    ("vp9", "Google VP9 (remade_ffmpeg_rs)", "Profile 0", "vp09", 31),
    ("av2", "Alliance for Open Media AV2 (rusty_av2d)", "Main", "av02", 51),
]
ACODECS = [
    ("aac", "AAC (Advanced Audio Coding)", "LC", "mp4a", "fltp", 48000, 2, "stereo", None),
    ("opus", "Opus (rusty-opus)", None, "Opus", "fltp", 48000, 2, "stereo", 312),
    ("flac", "FLAC (Free Lossless Audio Codec) (rusty_flac)", None, "fLaC", "s16", 44100, 2, "stereo", None),
    ("mp3", "MP3 (MPEG audio layer 3)", None, "mp4a", "fltp", 44100, 2, "stereo", 1105),
    ("vorbis", "Vorbis", None, "vorb", "fltp", 48000, 2, "stereo", None),
    ("aac", "AAC (Advanced Audio Coding)", "LC", "mp4a", "fltp", 48000, 6, "5.1(side)", None),
]
RESOLUTIONS = [(1920, 1080), (1280, 720), (3840, 2160), (854, 480), (2560, 1440), (720, 576)]
FPS = [("30000/1001", 29.97), ("25/1", 25.0), ("24000/1001", 23.976), ("60/1", 60.0), ("50/1", 50.0)]
PIXFMTS = ["yuv420p", "yuv420p10le", "yuv422p", "yuvj420p"]
LANGS = ["eng", "jpn", "deu", "fra", "spa", "und", "kor", "rus"]
DISPOSITION_KEYS = [
    "default", "dub", "original", "comment", "lyrics", "karaoke", "forced",
    "hearing_impaired", "visual_impaired", "clean_effects", "attached_pic",
    "timed_thumbnails", "non_diegetic", "captions", "descriptions",
    "dependent", "still_image",
]
# A handful of real accented titles: media metadata genuinely carries them, and
# the exact share is reported in the manifest so the axis stays honest.
TITLES = [
    "Ingest clip", "Camera A", "Camera B", "Drone pass",
    "R\u00e9p\u00e9tition g\u00e9n\u00e9rale", "\u00dcbergabe",
]


def disposition(rng, default=1, attached=0):
    d = {}
    for k in DISPOSITION_KEYS:
        d[k] = 0
    d["default"] = default
    d["attached_pic"] = attached
    if rng.chance(0.06):
        d["forced"] = 1
    return d


def build_media_probe(rng):
    files = []
    for fi in range(N_PROBE_FILES):
        fmt_name, fmt_long, ext, score = rng.pick(CONTAINERS)
        vc, vlong, vprof, vtag, vlevel = rng.pick(VCODECS)
        ac = rng.pick(ACODECS)
        w, h = rng.pick(RESOLUTIONS)
        fps_str, fps_val = rng.pick(FPS)
        dur = rng.frange(4.0, 2400.0)
        nb_frames = int(dur * fps_val)
        vbits = rng.between(600_000, 42_000_000)
        abits = rng.between(64_000, 512_000)
        tb = "1/%d" % rng.pick([1000, 12800, 24000, 90000])
        pix = rng.pick(PIXFMTS)
        streams = []
        streams.append({
            "index": 0,
            "codec_name": vc,
            "codec_long_name": vlong,
            "profile": vprof,
            "codec_type": "video",
            "codec_tag_string": vtag,
            "codec_tag": "0x%08x" % int.from_bytes(vtag.encode("ascii"), "little"),
            "width": w,
            "height": h,
            "coded_width": w,
            "coded_height": (h + 15) // 16 * 16,
            "closed_captions": 0,
            "film_grain": 1 if vc in ("av1", "av2") and rng.chance(0.3) else 0,
            "has_b_frames": rng.between(0, 3),
            "sample_aspect_ratio": "1:1",
            "display_aspect_ratio": rng.pick(["16:9", "4:3", "256:135"]),
            "pix_fmt": pix,
            "level": vlevel,
            "color_range": rng.pick(["tv", "pc"]),
            "color_space": rng.pick(["bt709", "bt2020nc", "smpte170m"]),
            "color_transfer": rng.pick(["bt709", "smpte2084", "arib-std-b67"]),
            "color_primaries": rng.pick(["bt709", "bt2020"]),
            "chroma_location": "left",
            "field_order": "progressive",
            "refs": rng.between(1, 4),
            "is_avc": "true" if vc == "h264" else "false",
            "nal_length_size": "4",
            "id": "0x%x" % (fi * 2 + 1),
            "r_frame_rate": fps_str,
            "avg_frame_rate": fps_str,
            "time_base": tb,
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": nb_frames,
            "duration": "%.6f" % dur,
            "bit_rate": "%d" % vbits,
            "bits_per_raw_sample": "10" if "10le" in pix else "8",
            "nb_frames": "%d" % nb_frames,
            "extradata_size": rng.between(34, 220),
            "disposition": disposition(rng),
            "tags": {
                "language": rng.pick(LANGS),
                "handler_name": rng.pick(["VideoHandler", "Core Media Video", "rff video"]),
                "vendor_id": rng.pick(["[0][0][0][0]", "FFMP", "RFFM"]),
                "encoder": "%s (rff %d.%d.%d)" % (vc, 0, rng.between(3, 9), rng.between(0, 12)),
            },
        })
        streams.append({
            "index": 1,
            "codec_name": ac[0],
            "codec_long_name": ac[1],
            "profile": ac[2],
            "codec_type": "audio",
            "codec_tag_string": ac[3],
            "codec_tag": "0x%08x" % (int.from_bytes(ac[3].encode("ascii"), "little") & 0xFFFFFFFF),
            "sample_fmt": ac[4],
            "sample_rate": "%d" % ac[5],
            "channels": ac[6],
            "channel_layout": ac[7],
            "bits_per_sample": 0,
            "initial_padding": ac[8],
            "id": "0x%x" % (fi * 2 + 2),
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/%d" % ac[5],
            "start_pts": 0 if ac[8] is None else -ac[8],
            "start_time": "0.000000" if ac[8] is None else "-%.6f" % (ac[8] / float(ac[5])),
            "duration_ts": int(dur * ac[5]),
            "duration": "%.6f" % dur,
            "bit_rate": "%d" % abits,
            "nb_frames": "%d" % int(dur * ac[5] / 1024.0),
            "extradata_size": rng.between(2, 40),
            "disposition": disposition(rng),
            "tags": {
                "language": rng.pick(LANGS),
                "handler_name": rng.pick(["SoundHandler", "Core Media Audio"]),
                "vendor_id": "[0][0][0][0]",
            },
        })
        if rng.chance(0.28):
            streams.append({
                "index": 2,
                "codec_name": "subrip",
                "codec_long_name": "SubRip subtitle",
                "codec_type": "subtitle",
                "codec_tag_string": "[0][0][0][0]",
                "codec_tag": "0x0000",
                "id": "0x%x" % (fi * 2 + 3),
                "r_frame_rate": "0/0",
                "avg_frame_rate": "0/0",
                "time_base": "1/1000",
                "start_pts": 0,
                "start_time": "0.000000",
                "duration_ts": int(dur * 1000),
                "duration": "%.6f" % dur,
                "extradata_size": 0,
                "disposition": disposition(rng, default=0),
                "tags": {"language": rng.pick(LANGS), "title": rng.pick(TITLES)},
            })
        chapters = []
        if rng.chance(0.22):
            marks = sorted(rng.sample(range(1, int(dur) or 2), min(4, max(1, int(dur) - 1))))
            prev = 0.0
            for ci, mk in enumerate(marks):
                chapters.append({
                    "id": ci,
                    "time_base": "1/1000",
                    "start": int(prev * 1000),
                    "start_time": "%.6f" % prev,
                    "end": mk * 1000,
                    "end_time": "%.6f" % float(mk),
                    "tags": {"title": "Chapter %d" % (ci + 1)},
                })
                prev = float(mk)
        total = int((vbits + abits) * dur / 8.0)
        entry = {
            "programs": [],
            "streams": streams,
            "chapters": chapters,
            "format": {
                "filename": "F:\\media\\ingest\\2026-09\\%s_%04d.%s" % (
                    rng.pick(["cam", "drone", "screen", "studio"]), fi, ext),
                "nb_streams": len(streams),
                "nb_programs": 0,
                "format_name": fmt_name,
                "format_long_name": fmt_long,
                "start_time": "0.000000",
                "duration": "%.6f" % dur,
                "size": "%d" % total,
                "bit_rate": "%d" % (vbits + abits),
                "probe_score": score,
                "tags": {
                    "major_brand": rng.pick(["isom", "mp42", "qt  "]),
                    "minor_version": "512",
                    "compatible_brands": "isomiso2avc1mp41",
                    "creation_time": iso(rng),
                    "encoder": "rff version 0.%d.%d" % (rng.between(3, 9), rng.between(0, 12)),
                    "title": rng.pick(TITLES),
                    "_STATISTICS_WRITING_APP": "rff 0.9.1",
                    "_STATISTICS_TAGS": "BPS DURATION NUMBER_OF_FRAMES NUMBER_OF_BYTES",
                    "BPS": "%d" % vbits,
                    "DURATION": "%02d:%02d:%09.6f" % (
                        int(dur) // 3600, (int(dur) % 3600) // 60, dur % 60),
                    "NUMBER_OF_FRAMES": "%d" % nb_frames,
                    "NUMBER_OF_BYTES": "%d" % total,
                },
            },
        }
        files.append(entry)
    return {
        "probe_version": "rffprobe 0.9.1",
        "probe_flags": ["-show_streams", "-show_format", "-show_chapters", "-print_format", "json"],
        "root": "F:\\media\\ingest\\2026-09",
        "scanned_at": iso(rng),
        "files": files,
    }


# ==========================================================================
# 2. s4-frame-telemetry.json -- the long homogeneous array of small objects
# ==========================================================================
def build_frame_telemetry(rng):
    frames = []
    pts = 0
    gop = 0
    cum_bits = 0
    for n in range(1, N_FRAMES + 1):
        gop -= 1
        if gop <= 0:
            ftype = "I"
            gop = 250
        elif n % 3 == 0:
            ftype = "B"
        else:
            ftype = "P"
        if ftype == "I":
            bits = rng.between(90_000, 320_000)
            qp = rng.frange(18.0, 24.0)
        elif ftype == "P":
            bits = rng.between(8_000, 64_000)
            qp = rng.frange(21.0, 28.0)
        else:
            bits = rng.between(1_200, 18_000)
            qp = rng.frange(23.0, 31.0)
        cum_bits += bits
        mb = 8160
        mb_i = mb if ftype == "I" else rng.between(0, 900)
        mb_p = 0 if ftype == "I" else rng.between(1500, 6200)
        mb_b = rng.between(1000, 5000) if ftype == "B" else 0
        frames.append({
            "n": n,
            "pts": pts,
            "dts": pts - (3003 if ftype == "B" else 0),
            "t": ftype,
            "qp": num(qp, 1),
            "bits": bits,
            "psnr_y": num(rng.frange(35.0, 45.0), 3),
            "psnr_u": num(rng.frange(38.0, 47.0), 3),
            "psnr_v": num(rng.frange(38.0, 47.0), 3),
            "ssim": num(rng.frange(0.958, 0.9994), 5),
            "mb_i": mb_i,
            "mb_p": mb_p,
            "mb_b": mb_b,
            "mb_skip": max(0, mb - mb_i - mb_p - mb_b),
            "satd": rng.between(40_000, 2_400_000),
            "ref": [0] if ftype != "B" else [0, 1],
            "cpb": num(rng.frange(0.02, 0.97), 4),
            "ms": num(rng.frange(0.8, 31.0), 3),
        })
        pts += 3003
    return {
        "encoder": "rusty_h264 0.9.1",
        "clip": "tears_of_steel_1080p_30fps",
        "preset": "medium",
        "tune": None,
        "crf": num(23.0, 1),
        "threads": 12,
        "started_at": iso(rng),
        "frames": frames,
        "summary": {
            "frames": N_FRAMES,
            "kbps": num(cum_bits / (N_FRAMES / 29.97) / 1000.0, 2),
            "bytes": cum_bits // 8,
            "psnr_avg": num(rng.frange(39.0, 41.0), 4),
            "ssim_avg": num(rng.frange(0.978, 0.991), 6),
            "encode_s": num(rng.frange(40.0, 90.0), 3),
            "fps": num(rng.frange(28.0, 64.0), 2),
        },
    }


# ==========================================================================
# 3. s4-signin-batch.json -- kms-types envelopes; Vec<u8> as number arrays
# ==========================================================================
AUDIENCES = [
    "https://vault.mata.example/v1/signin",
    "https://contacts.mata.example/v1/signin",
    "https://pass.mata.example/v1/signin",
    "http://127.0.0.1:4243/v1/status",
]
PURPOSES = ["signin", "roster-update", "capability-grant", "device-pair"]
DEVICE_LABELS = ["laptop-work", "phone", "tablet", "desktop-studio", "browser-extension", "oem-box"]


def build_signin_batch(rng):
    issuer = did(rng)
    assertions = []
    for i in range(N_ASSERTIONS):
        subject = did(rng)
        issued = 1789000000 + i * 7 + rng.below(5)
        env = {
            "envelope_version": 2,
            "envelope_type": "mata.nonce.v2",
            "nonce": list(rng.bytes(32)),
            "did": subject,
            "audience": rng.pick(AUDIENCES),
            "purpose": rng.pick(PURPOSES),
            "issued_at": issued,
            "expires_at": issued + 300,
            "issuer": issuer,
        }
        assertions.append({
            "envelope_version": 2,
            "envelope_type": "mata.signed_assertion.v2",
            "nonce_envelope": env,
            "device_id": "dev-" + b58(rng.bytes(8)),
            "signature": list(rng.bytes(64)),
        })
    rosters = []
    for _ in range(12):
        subject = did(rng)
        n = rng.between(1, 5)
        rosters.append({
            "did": subject,
            "roster_version": rng.between(1, 40),
            "roster": [
                {
                    "device_id": "dev-" + b58(rng.bytes(8)),
                    "pubkey": list(rng.bytes(33)),
                    "added_at": 1780000000 + rng.below(9000000),
                    "label": rng.pick(DEVICE_LABELS),
                }
                for _ in range(n)
            ],
            "updated_at": 1789000000 + rng.below(90000),
        })
    return {
        "batch_version": 1,
        "produced_by": "mid-signin 0.6.3",
        "verifier": "mid-verify 0.6.3",
        "roster_hard_cap": 16,
        "signed_at_tolerance_secs": 300,
        "count": N_ASSERTIONS,
        "assertions": assertions,
        "resolved_rosters": rosters,
    }


# ==========================================================================
# 4. s4-sync-envelope.json -- API envelope + per-entry CRDT deltas
#    Untagged enums (fields.*), internally-tagged enums (outcome), nulls,
#    and a `flatten` tail of unknown x_* keys.
# ==========================================================================
FIRST = ["Ada", "Ines", "Tomas", "Yuki", "Omar", "Lena", "Piet", "Nadia", "Kofi", "Sanne",
         "Milos", "Rhea", "Bjorn", "Aiko", "Dara", "Ewan"]
LAST = ["Nakamura", "Okafor", "Lindqvist", "Duarte", "Hassan", "Novak", "Whitfield", "Bergeron",
        "Sokolov", "Mbeki", "Halvorsen", "Ferraro"]
TAG_POOL = ["family", "work", "mesh", "oem", "press", "beta", "vip", "archive", "muted"]
COLLECTIONS = ["contacts", "vault", "notes", "devices", "grants"]
TIERS = ["convergent", "causal", "strong"]


def crdt_field(rng, name):
    """One of four wire shapes -- serde's untagged-enum path on the way in."""
    kind = rng.below(4)
    if kind == 0:
        return {"crdt": "register", "value": name, "ts": 1789000000 + rng.below(90000),
                "node": "nd-" + b58(rng.bytes(6))}
    if kind == 1:
        ops = []
        for _ in range(rng.between(1, 4)):
            ops.append({"at": rng.below(400), "ins": rng.pick(
                ["ok", " and ", "note: ", "see thread", "moved", "n/a"]), "del": rng.below(3)})
        return {"crdt": "text", "ops": ops, "len": rng.between(0, 900)}
    if kind == 2:
        return {"crdt": "or_set",
                "adds": rng.sample(TAG_POOL, rng.between(0, 4)),
                "removes": rng.sample(TAG_POOL, rng.between(0, 2))}
    return {"crdt": "counter", "delta": rng.between(-4, 12), "seen": rng.between(0, 4000)}


def build_sync_envelope(rng):
    entries = []
    for i in range(N_SYNC_ENTRIES):
        coll = rng.pick(COLLECTIONS)
        name = "%s %s" % (rng.pick(FIRST), rng.pick(LAST))
        fields = {}
        for fname in rng.sample(
                ["display_name", "bio", "tags", "unread", "email", "phone", "org", "note"],
                rng.between(3, 6)):
            fields[fname] = crdt_field(rng, name if fname == "display_name" else fname)
        outcome_kind = rng.below(4)
        if outcome_kind == 0:
            outcome = {"type": "local"}
        elif outcome_kind == 1:
            outcome = {"type": "committed", "tier": rng.pick(TIERS)}
        elif outcome_kind == 2:
            outcome = {"type": "merged", "tier": "convergent", "peers": rng.between(1, 6)}
        else:
            outcome = {"type": "rejected", "reason": rng.pick(
                ["tier_unavailable", "capability_expired", "roster_stale"])}
        e = {
            "id": ulid(rng.bytes(16)),
            "collection": coll,
            "tier": rng.pick(TIERS),
            "version": {
                "node": "nd-" + b58(rng.bytes(6)),
                "counter": rng.between(1, 90000),
                "hlc": iso(rng)[:-1] + ".%03dZ" % rng.below(1000),
            },
            "fields": fields,
            "outcome": outcome,
            "deleted": rng.chance(0.05),
            "shards": None if rng.chance(0.7) else {
                "k": 6, "n": 10,
                "placement": ["nd-" + b58(rng.bytes(6)) for _ in range(rng.between(2, 5))],
                "repair_due": rng.chance(0.2),
            },
            "grant": None if rng.chance(0.6) else {
                "granted_to": did(rng),
                "scope": [coll + ":read", coll + ":write"],
                "expires_at": 1789000000 + rng.below(2592000),
                "budget_units": rng.between(0, 5000),
                "revoked": False,
            },
        }
        # The `flatten` tail: keys no named field claims.
        for k in rng.sample(["x_source", "x_starred", "x_last_seen", "x_device", "x_import_id"],
                            rng.between(1, 3)):
            if k == "x_starred":
                e[k] = rng.chance(0.3)
            elif k == "x_last_seen":
                e[k] = 1789000000 + rng.below(90000)
            elif k == "x_import_id":
                e[k] = None
            else:
                e[k] = rng.pick(["import.vcf", "mesh", "oem-pair", "extension", "manual"])
        entries.append(e)
    return {
        "api_version": "v1",
        "served_by": "mata-oem-sidecar 0.3.7",
        "listen": "127.0.0.1:4243",
        "box_id": "box-" + b58(rng.bytes(8)),
        "maker_did": did(rng),
        "owner_did": None,
        "pair_state": "open",
        "request_id": ulid(rng.bytes(16)),
        "elapsed_ms": num(rng.frange(0.4, 18.0), 3),
        "warnings": [],
        "data": {
            "collection": "contacts",
            "cursor": b58(rng.bytes(12)),
            "has_more": True,
            "count": N_SYNC_ENTRIES,
            "entries": entries,
        },
        "meter": {
            "read_units": rng.between(100, 9000),
            "write_units": rng.between(0, 400),
            "budget_remaining": rng.between(0, 100000),
            "priced_at": iso(rng),
        },
    }


# ==========================================================================
# 5. s4-node-config.json -- small, deep, string-heavy, bools and nulls
# ==========================================================================
def build_node_config(rng):
    def collection(name, fields):
        return {
            "name": name,
            "fields": {
                f: {
                    "crdt": c,
                    "tier": t,
                    "indexed": idx,
                    "required": req,
                    "default": dflt,
                    "max_bytes": mx,
                }
                for (f, c, t, idx, req, dflt, mx) in fields
            },
        }

    return {
        "config_version": 3,
        "profile": "disco-node",
        "identity": {
            "did": None,
            "device_label": "studio-01",
            "keystore": {
                "backend": "spacedb-store",
                "kdf": {"name": "argon2id", "m_cost_kib": 262144, "t_cost": 3, "p_cost": 4},
                "aead": "aes-256-gcm",
                "rotate_days": 90,
                "require_presence": True,
                "hardware_bound": False,
            },
            "roster": {"hard_cap": 16, "signed_at_tolerance_secs": 300, "allow_self_issue": True},
        },
        "store": {
            "layer": "L0",
            "path": "F:\\mata\\node\\store",
            "engine": "kv",
            "encrypt_at_rest": True,
            "fsync": "on-commit",
            "cache_mib": 512,
            "compression": {"codec": "rusty_zstd", "level": 9, "long_window": True, "dict": None},
        },
        "crdt": {"layer": "L1", "reactive_queries": True, "gc_tombstones_days": 30},
        "consistency": {
            "layer": "L3",
            "default_tier": "convergent",
            "allow_strong": False,
            "read_your_writes": True,
        },
        "replica": {
            "layer": "L2",
            "transport": {
                "kind": "iroh",
                "quic": True,
                "mdns": True,
                "relays": ["relay-eu-1.mata.example", "relay-us-3.mata.example"],
                "stun": None,
                "max_streams": 64,
            },
            "anti_entropy": {"interval_s": 15, "jitter_s": 4, "batch_entries": 512, "gossip_fanout": 3},
        },
        "durability": {
            "layer": "L2",
            "erasure": {"impl": "rusty_erasure", "k": 6, "n": 10, "shard_kib": 64, "verify_on_read": True},
            "placement": {"strategy": "content-addressed", "min_distinct_nodes": 7, "repair_when_below": 8},
        },
        "access": {
            "layer": "L5",
            "identity": "mid",
            "curve": "p-256",
            "audit_log": True,
            "capabilities": {"default_ttl_s": 3600, "allow_delegation": False, "spend_budget_units": 50000},
        },
        "query": {"layer": "L4", "wasm": True, "deterministic": True, "fuel": 20000000, "compute_to_data": True},
        "vector": {"layer": "L4", "enabled": False, "dim": None, "metric": "cosine", "index": None},
        "meter": {"layer": "L6", "enabled": True, "currency": "units", "price_read": 1, "price_write": 4},
        "engines": {
            "media": {"crate": "remade_ffmpeg_rs", "enabled": True, "probe_only": False,
                      "decoders": ["h264", "hevc", "av1", "av2", "vp9"],
                      "encoders": ["h264", "av1", "vp9"]},
            "asr": {"crate": "mercury", "enabled": True, "model": None, "engine": "whisper-candle",
                    "beam": 5, "vad": True},
            "tts": {"crate": "mercury", "enabled": False, "voice": None},
            "ocr": {"crate": "carmenta", "enabled": True, "streaming": True, "reading_order": True},
            "vlm": {"crate": "argus", "enabled": False, "model": "SmolVLM-256M", "max_tokens": 32},
            "detect": {"crate": "diana", "enabled": False, "model": "yolo26n", "track": True},
            "compress": {"crate": "rusty_zstd", "enabled": True},
        },
        "catalog": [
            {"capability": "media.probe", "status": "available", "backing": "remade-ffmpeg", "http": "/v1/media/probe"},
            {"capability": "ocr.read", "status": "available", "backing": "carmenta", "http": "/v1/ocr/read"},
            {"capability": "asr.transcribe", "status": "available", "backing": "mercury", "http": "/v1/asr/transcribe"},
            {"capability": "vlm.caption", "status": "preview", "backing": "argus", "http": "/v1/vlm/caption"},
            {"capability": "vision.depth", "status": "planned", "backing": None, "http": "/v1/vision/depth"},
        ],
        "supply_chain": {
            "tool": "deputy",
            "stages": ["discover", "acquire", "analyze", "scan", "promote", "deploy"],
            "gate": {"fail_closed": True, "allow_dirty": False, "require_receipts": True,
                     "advisory_db_max_age_days": 7},
            "vault": {"path": "F:\\mata\\deputy\\prod", "sealed": True, "hash": "sha-256"},
        },
        "time": {
            "daemon": "rtimed",
            "sources": [
                {"host": "time.mata.example", "nts": True, "poll_log2": 6, "prefer": True, "iburst": True},
                {"host": "pool.ntp.example", "nts": False, "poll_log2": 8, "prefer": False, "iburst": False},
            ],
            "step_threshold_s": num(0.128, 3),
            "makestep": None,
            "serve": {"enabled": False, "rate_limit_qps": 4000, "interleaved": True},
        },
        "logging": {"level": "info", "format": "json", "targets": ["stdout"], "redact_dids": True,
                    "sample_rate": num(1.0, 1)},
        "schemas": [
            collection("contacts", [
                ("display_name", "register", "convergent", True, True, None, 256),
                ("bio", "text", "convergent", False, False, None, 4096),
                ("tags", "or_set", "convergent", True, False, None, 512),
                ("unread", "counter", "convergent", False, False, 0, 8),
            ]),
            collection("vault", [
                ("title", "register", "causal", True, True, None, 256),
                ("secret", "register", "strong", False, True, None, 8192),
                ("shared_with", "or_set", "causal", False, False, None, 1024),
            ]),
        ],
    }


# ==========================================================================
# 6. s4-vault-shard.json -- a few very long base64 strings
# ==========================================================================
def build_vault_shard(rng):
    stripe = ulid(rng.bytes(16))
    shards = []
    for i in range(N_SHARDS):
        raw = rng.bytes(SHARD_RAW_BYTES)
        shards.append({
            "index": i,
            "role": "data" if i < N_SHARDS - 1 else "parity",
            "node": "nd-" + b58(rng.bytes(6)),
            "len": SHARD_RAW_BYTES,
            "sha256": hashlib.sha256(raw).hexdigest(),
            "nonce": base64.b64encode(rng.bytes(12)).decode("ascii"),
            "aad": base64.b64encode(("spacedb/vault/" + stripe).encode("ascii")).decode("ascii"),
            "sealed": base64.b64encode(raw).decode("ascii"),
        })
    return {
        "manifest_version": 1,
        "kind": "spacedb.durability.shard_set.v1",
        "collection": "vault",
        "stripe": stripe,
        "k": 6,
        "n": 10,
        "erasure": {"impl": "rusty_erasure", "field": "GF(2^8)", "matrix": "vandermonde"},
        "aead": "aes-256-gcm",
        "kdf": {"name": "argon2id", "m_cost_kib": 262144, "t_cost": 3, "p_cost": 4,
                "salt": base64.b64encode(rng.bytes(16)).decode("ascii")},
        "sealed_at": iso(rng),
        "sealed_by": did(rng),
        "shards": shards,
        "model_shard": {
            "engine": "mercury",
            "model": "whisper-small-int8",
            "part": "1-of-3",
            "sha256": hashlib.sha256(b"whisper-small-int8/1").hexdigest(),
            "bytes": SHARD_RAW_BYTES,
            "weights_b64": base64.b64encode(rng.bytes(SHARD_RAW_BYTES)).decode("ascii"),
        },
        "receipts": [
            {"stage": stage, "at": iso(rng), "by": did(rng), "ok": True,
             "chain": hashlib.sha256(stage.encode("ascii")).hexdigest()}
            for stage in ["acquire", "analyze", "scan", "promote"]
        ],
    }


# ==========================================================================
# 7. s4-ocr-i18n.json -- the escape path, in both directions
# ==========================================================================
# Raw non-ASCII: what serde_json itself emits (it never escapes above 0x7f).
RAW_TEXT = [
    "\u65e5\u672c\u8a9e\u306e\u5b57\u5e55\u3001\u30c6\u30ec\u30d3\u756a\u7d44",
    "\u4e2d\u6587\u5b57\u5e55\u6d4b\u8bd5\uff1a\u7b2c\u4e09\u7ae0",
    "\ud55c\uad6d\uc5b4 \uc790\ub9c9 \ud14c\uc2a4\ud2b8",
    "\u0420\u0443\u0441\u0441\u043a\u0438\u0435 \u0441\u0443\u0431\u0442\u0438\u0442\u0440\u044b",
    "\u0395\u03bb\u03bb\u03b7\u03bd\u03b9\u03ba\u03ac \u03c5\u03c0\u03cc\u03c4\u03b9\u03c4\u03bb\u03bf\u03b9",
    "\u0627\u0644\u0639\u0631\u0628\u064a\u0629 \u0627\u0644\u0641\u0635\u062d\u0649",
    "\u05e2\u05d1\u05e8\u05d9\u05ea \u05db\u05ea\u05d5\u05d1\u05d9\u05ea",
    "\u0939\u093f\u0928\u094d\u0926\u0940 \u0938\u092c\u091f\u093e\u0907\u091f\u0932",
    "\u0e20\u0e32\u0e29\u0e32\u0e44\u0e17\u0e22",
    "R\u00e9p\u00e9tition g\u00e9n\u00e9rale \u00e0 20\u00a0h",
    "\u00dcbergabeprotokoll, Se\u00dfion 4",
    "Se\u00f1al de prueba \u2014 c\u00e1mara 3",
    "e\u0301chelle combinante",  # decomposed: combining acute, as OCR emits
    "temp \u00b1 0.5\u00b0C \u2192 stable \u2026",
    "\U0001F680 launch frame",
    "\U0001F1EF\U0001F1F5 region tag",
    "\U0001F469\u200D\U0001F4BB operator seat",
]
# The same class of content as a foreign producer writes it: \uXXXX escapes,
# including surrogate pairs for astral code points. Verbatim string bodies.
ESCAPED_TEXT = [
    "\\u65e5\\u672c\\u8a9e\\u306e\\u5b57\\u5e55",
    "\\u0420\\u0443\\u0441\\u0441\\u043a\\u0438\\u0439",
    "caf\\u00e9 \\u2014 table 12",
    "\\ud83d\\ude80 \\ud83d\\ude80 double rocket",
    "flag \\ud83c\\uddef\\ud83c\\uddf5 pair",
    "zwj \\ud83d\\udc69\\u200d\\ud83d\\udcbb seat",
    "null byte \\u0000 then text",
    "vertical tab \\u000b and escape \\u001b[0m",
    "\\u00e9\\u00e8\\u00ea\\u00eb accents",
]
# ASCII that the WRITER must escape on the way out: quotes, backslashes,
# and real control characters.
DIRTY_ASCII = [
    'he said "left of frame" twice',
    "path F:\\media\\ocr\\page_0041.png",
    "regex \\d+\\s*(px|pt)\\b",
    "line one\nline two\nline three",
    "col\tsep\tvalues",
    "carriage\rreturn artifact",
    "mixed \"quote\" and \\backslash\\ and\ttab",
    "json in json: {\"k\": \"v\"}",
]
LANG_TAGS = ["ja", "zh-Hans", "ko", "ru", "el", "ar", "he", "hi", "th", "fr", "de", "es", "en", "und"]


JOINERS = [". ", " ", " -- ", "\n", "\t| ", "; ", " \u2014 "]


def i18n_text(rng):
    """One value composed of 2-6 fragments drawn from all three registers.

    Composing rather than picking matters: a 20-character value is swamped by
    its own key bytes, and the file then measures key dispatch instead of the
    escape path. Real OCR lines and ASR segments are sentences.

    The result is always a verbatim body, so raw UTF-8 and escape sequences sit
    inside ONE string -- which is what a mixed-producer pipeline hands a parser.
    """
    pieces = []
    for _ in range(rng.between(2, 6)):
        r = rng.unit()
        if r < 0.42:
            pieces.append(_esc(rng.pick(RAW_TEXT)))
        elif r < 0.70:
            pieces.append(rng.pick(ESCAPED_TEXT))
        else:
            pieces.append(_esc(rng.pick(DIRTY_ASCII)))
    return RawStr(_esc(rng.pick(JOINERS)).join(pieces))


def build_ocr_i18n(rng):
    items = []
    for i in range(N_FFAI_ITEMS):
        kind = i % 3
        if kind == 0:
            lines = []
            for li in range(rng.between(14, 30)):
                lines.append({
                    "index": li,
                    "text": i18n_text(rng),
                    "conf": num(rng.frange(0.62, 0.999), 4),
                    "bbox": [rng.below(1600), rng.below(2200), rng.between(20, 900), rng.between(12, 60)],
                    "lang": rng.pick(LANG_TAGS),
                    "rtl": rng.chance(0.15),
                    "baseline_skew": num(rng.frange(-1.5, 1.5), 3),
                })
            items.append({
                "kind": "ocr.page",
                "engine": "carmenta 0.5.0",
                "source": "F:\\media\\ocr\\scan_%04d.png" % i,
                "detector": rng.pick(["db-resnet18", "east", "craft"]),
                "recognizer": rng.pick(["crnn-vgg", "parseq", "trocr-small"]),
                "reading_order": "column-major",
                "page": {"width": 1654, "height": 2339, "dpi": 200, "rotation": rng.pick([0, 90, 180, 270])},
                "lines": lines,
                "churn": 0,
            })
        elif kind == 1:
            segs = []
            t = 0.0
            for si in range(rng.between(10, 24)):
                d = rng.frange(0.8, 5.5)
                segs.append({
                    "id": si,
                    "start": num(t, 3),
                    "end": num(t + d, 3),
                    "text": i18n_text(rng),
                    "speaker": rng.pick(["SPEAKER_00", "SPEAKER_01", None]),
                    "avg_logprob": num(rng.frange(-1.2, -0.05), 4),
                    "no_speech_prob": num(rng.frange(0.0, 0.4), 4),
                    "words": [
                        {"w": rng.pick(["the", "frame", "\u3053\u308c", "\u0434\u0430", "left", "\u00e9t\u00e9"]),
                         "s": num(t, 2), "e": num(t + rng.frange(0.05, 0.4), 2),
                         "p": num(rng.frange(0.3, 1.0), 3)}
                        for _ in range(rng.between(2, 6))
                    ],
                })
                t += d
            items.append({
                "kind": "asr.transcript",
                "engine": "mercury 0.7.2",
                "source": "F:\\media\\asr\\talk_%04d.wav" % i,
                "model": "whisper-small-int8",
                "language": rng.pick(LANG_TAGS),
                "wer_holdout": num(rng.frange(0.061, 0.089), 4),
                "segments": segs,
            })
        else:
            items.append({
                "kind": "vlm.caption",
                "engine": "argus 0.3.1",
                "source": "F:\\media\\frames\\frame_%06d.png" % (i * 137),
                "model": "SmolVLM-256M",
                "prompt": "describe this \"frame\" precisely\nno speculation",
                "caption": i18n_text(rng),
                "alternates": [i18n_text(rng) for _ in range(rng.between(2, 4))],
                "tokens": rng.between(18, 32),
                "byte_identical_to_reference": True,
                "detections": [
                    {"label": rng.pick(["person", "laptop", "cup", "chair", "\u732b", "\u0441\u0442\u0443\u043b"]),
                     "conf": num(rng.frange(0.4, 0.99), 3),
                     "xyxy": [rng.below(1900), rng.below(1000), rng.below(1900), rng.below(1000)],
                     "track_id": rng.below(400)}
                    for _ in range(rng.between(0, 5))
                ],
            })
    return {
        "bundle": "ffai.results.v1",
        "engines": {"ocr": "carmenta 0.5.0", "asr": "mercury 0.7.2", "vlm": "argus 0.3.1",
                    "detect": "diana 0.4.0"},
        "note": "text is carried in BOTH forms: raw UTF-8 (what serde_json emits) "
                "and \\uXXXX escapes (what other producers emit)",
        "items": items,
    }


# ==========================================================================
# Drive it.
# ==========================================================================
FILES = [
    # (name, builder, indent)
    ("s4-media-probe.json", build_media_probe, 4),
    ("s4-frame-telemetry.json", build_frame_telemetry, 0),
    ("s4-signin-batch.json", build_signin_batch, 0),
    ("s4-sync-envelope.json", build_sync_envelope, 0),
    ("s4-node-config.json", build_node_config, 2),
    ("s4-vault-shard.json", build_vault_shard, 0),
    ("s4-ocr-i18n.json", build_ocr_i18n, 0),
]

GITATTRIBUTES = """\
# S4 payloads are DATA, keyed by SHA-256 in MANIFEST.md and benchmarked byte
# for byte. `core.autocrlf=true` on a Windows checkout would rewrite every
# newline, changing the size, the whitespace census and every hash -- exactly
# what happened to S1-S3 (twitter.json: 646,995 bytes here, 631,515 in
# corpus/README.md). `-text` pins them.
*.json -text
"""

LONE_SURROGATE = re.compile(rb"\\u[dD][89abAB][0-9a-fA-F]{2}(?!\\u[dD][c-fC-F])")


def validate(name, blob):
    """Every gate that can be run without building the crate."""
    doc = json.loads(blob.decode("utf-8"))  # UTF-8 + JSON grammar
    if LONE_SURROGATE.search(blob):
        raise SystemExit("%s: lone surrogate escape -- serde_json would reject" % name)

    def walk(v):
        if isinstance(v, str):
            v.encode("utf-8")  # raises on a lone surrogate that survived json
        elif isinstance(v, dict):
            for k, x in v.items():
                k.encode("utf-8")
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)

    walk(doc)
    return doc


def main():
    check = "--check" in sys.argv[1:]

    src = open(os.path.abspath(__file__), "rb").read()
    try:
        src.decode("ascii")
    except UnicodeDecodeError:
        raise SystemExit("gen-s4.py must stay ASCII (house lint)")

    built = []
    for name, builder, indent in FILES:
        rng = Rng(SEED ^ (int(hashlib.sha256(name.encode("ascii")).hexdigest()[:16], 16)))
        blob = emit(builder(rng), indent)
        validate(name, blob)
        built.append((name, blob, indent))

    if not check:
        os.makedirs(OUT_DIR, exist_ok=True)
        with open(os.path.join(OUT_DIR, ".gitattributes"), "wb") as fh:
            fh.write(GITATTRIBUTES.encode("ascii"))
        for name, blob, _ in built:
            with open(os.path.join(OUT_DIR, name), "wb") as fh:
                fh.write(blob)

    rows = []
    for name, blob, indent in built:
        c = census(blob)
        # The same invariant content.rs asserts: on well-formed input the
        # classes are disjoint and exhaustive. If this fails the census is
        # lying and no percentage below it means anything.
        accounted = (c["whitespace"] + c["structural"] + c["string_bytes"]
                     + c["number_bytes"] + c["literal_bytes"])
        if accounted != c["bytes"]:
            raise SystemExit("%s: %d of %d bytes unaccounted by the census"
                             % (name, c["bytes"] - accounted, c["bytes"]))
        rows.append((name, indent, c, hashlib.sha256(blob).hexdigest()))

    # S1-S3 are censused with the same code and printed in the same tables:
    # an S4 row is only useful next to the rows the ledger already carries,
    # and reproducing their published percentages is what proves this port.
    refs = []
    for ref in ("twitter.json", "citm_catalog.json", "canada.json"):
        p = os.path.join(CORPUS_DIR, ref)
        if os.path.exists(p):
            refs.append((ref, None, census(open(p, "rb").read()), None))

    print("S4 house-payload corpus -- seed 0x%X, %d files\n" % (SEED, len(built)))

    hdr1 = ("file", "bytes", "ws%", "str%", "num%", "nonasc%", "lit%", "struct%",
            "depth", "objects", "arrays", "keys")
    print("%-26s %10s %7s %7s %7s %8s %6s %8s %6s %8s %7s %7s" % hdr1)
    print("-" * 124)

    def row1(name, c):
        print("%-26s %10d %7.2f %7.2f %7.2f %8.2f %6.2f %8.2f %6d %8d %7d %7d" % (
            name, c["bytes"], pct(c["whitespace"], c["bytes"]),
            pct(c["string_bytes"], c["bytes"]), pct(c["number_bytes"], c["bytes"]),
            pct(c["non_ascii"], c["bytes"]), pct(c["literal_bytes"], c["bytes"]),
            pct(c["structural"], c["bytes"]), c["depth"], c["objects"], c["arrays"],
            c["keys"]))

    for name, _, c, _ in rows:
        row1(name, c)
    print("-- S1-S3, as the harness reads them (validates this census port) --")
    for name, _, c, _ in refs:
        row1(name, c)

    hdr2 = ("file", "strings", "keys", "esc-str", "esc-B", "escB%str", "mean-str", "longest-str")
    print("\n%-26s %9s %8s %8s %8s %9s %9s %12s" % hdr2)
    print("-" * 124)

    def row2(name, c):
        print("%-26s %9d %8d %8d %8d %9.2f %9.1f %12d" % (
            name, c["strings"], c["keys"], c["strings_with_escapes"], c["escape_bytes"],
            pct(c["escape_bytes"], c["string_bytes"]),
            (c["string_bytes"] / float(c["strings"])) if c["strings"] else 0.0,
            c["longest_string"]))

    for name, _, c, _ in rows:
        row2(name, c)
    print("-- S1-S3 --")
    for name, _, c, _ in refs:
        row2(name, c)

    hdr3 = ("file", "numbers", "floats", "int-only", "num/KB", "ws-runs", "mean-ws", "ws>=8B%",
            "longest-ws")
    print("\n%-26s %9s %8s %9s %8s %9s %8s %8s %11s" % hdr3)
    print("-" * 124)

    def row3(name, c):
        runs = sum(c["ws_runs_by_len"])
        print("%-26s %9d %8d %9d %8.1f %9d %8.2f %8.1f %11d" % (
            name, c["numbers"], c["floats"], c["numbers"] - c["floats"],
            c["numbers"] * 1024.0 / c["bytes"], runs,
            (c["whitespace"] / float(runs)) if runs else 0.0,
            pct(sum(c["ws_bytes_by_len"][4:]), c["whitespace"]), c["longest_ws_run"]))

    for name, _, c, _ in rows:
        row3(name, c)
    print("-- S1-S3 --")
    for name, _, c, _ in refs:
        row3(name, c)

    hdr4 = ("file", "keys/KB", "distinct", "reuse", "max-obj", "max-arr", "literals", "lit/KB")
    print("\n%-26s %8s %9s %8s %8s %9s %9s %8s" % hdr4)
    print("-" * 124)

    def row4(name, c):
        d = len(c["distinct_keys"])
        print("%-26s %8.1f %9d %8.1f %8d %9d %9d %8.1f" % (
            name, c["keys"] * 1024.0 / c["bytes"], d,
            (c["keys"] / float(d)) if d else 0.0, c["max_obj_arity"], c["max_arr_arity"],
            c["literals"], c["literals"] * 1024.0 / c["bytes"]))

    for name, _, c, _ in rows:
        row4(name, c)
    print("-- S1-S3 --")
    for name, _, c, _ in refs:
        row4(name, c)

    print("\nSHA-256")
    for name, _, _, h in rows:
        print("%s *%s" % (h, name))

    total = sum(c["bytes"] for _, _, c, _ in rows)
    print("\n%d bytes (%.2f MB) in %d files %s %s"
          % (total, total / 1048576.0, len(rows),
             "verified against" if check else "written to", OUT_DIR))

    if check:
        bad = 0
        for name, blob, _ in built:
            p = os.path.join(OUT_DIR, name)
            on_disk = open(p, "rb").read() if os.path.exists(p) else b""
            if on_disk != blob:
                print("MISMATCH %s (disk %d bytes, generated %d)" % (name, len(on_disk), len(blob)))
                bad += 1
        print("\n--check: %s" % ("%d file(s) differ" % bad if bad else "byte-identical"))
        return 1 if bad else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
