# rusty_json_turbo-alloc

[![crates.io](https://img.shields.io/crates/v/rusty_json_turbo-alloc?logo=rust)](https://crates.io/crates/rusty_json_turbo-alloc)
[![docs.rs](https://img.shields.io/docsrs/rusty_json_turbo-alloc?logo=docsdotrs)](https://docs.rs/rusty_json_turbo-alloc)
[![CI](https://github.com/Remade-With-Rust/rusty_json_turbo/actions/workflows/ci.yml/badge.svg)](https://github.com/Remade-With-Rust/rusty_json_turbo/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/Remade-With-Rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network/)

> **Two lines in your binary, and JSON parsing gets substantially faster.**
> The [`rusty_alloc`](https://crates.io/crates/rusty_alloc) seam for
> [`rusty_json_turbo`](https://crates.io/crates/rusty_json_turbo) deliverables:
> it holds the exact pin, the `secure` choice and the `debug_checks` choice in
> one place, so no feature code ever names the allocator crate.

**Most users want the library — [`rusty_json_turbo`](https://crates.io/crates/rusty_json_turbo).**
Add this one to the crate that *is* the program: your binary, your service,
your WASM entry point.

Part of **[Remade With Rust](https://github.com/Remade-With-Rust)** by
**[Mata Network](https://www.mata.network/)**.

---

## Why a crate for two lines

`#[global_allocator]` is process-wide — a program may declare exactly **one**.
So it belongs to the *deliverable*, never to a library: a library that declares
it forces the choice on every consumer, and two such libraries cannot be linked
into one program at all.

That is why `rusty_json_turbo` does not install one. It is a drop-in for
`serde_json`, which sits in nearly every Rust dependency graph, often several
times over; Cargo features are additive, so a library that switched the
allocator on could force it onto an application that had already chosen one —
and that application could not fix the build from its own manifest. The choice
stays where it can be made honestly, and this crate makes it one line.

## Use

```sh
cargo add rusty_json_turbo-alloc
```

```rust
#[global_allocator]
static ALLOC: rusty_json_turbo_alloc::Alloc = rusty_json_turbo_alloc::Alloc;

fn main() {
    // ... nothing else changes.
}
```

## What it buys

Building a `Value` is allocation-bound: roughly **one allocation per 31 input
bytes** on a real document. Measured on the project's corpus — pinned to one
core, paired, ABBA-interleaved, against the platform allocator, with the
allocation counts proven identical on both sides:

| workload | allocations per parse | platform | rusty_alloc | ratio |
|---|---:|---:|---:|---:|
| `twitter.json`, DOM parse | 20,834 | 459 MB/s | **690 MB/s** | **1.53x** |
| `citm_catalog.json`, DOM parse | 39,339 | 697 MB/s | **1013 MB/s** | **1.46x** |
| `canada.json`, DOM parse | 56,061 | 376 MB/s | **499 MB/s** | **1.32x** |
| `twitter.json`, struct parse | 2,762 | 880 MB/s | **1024 MB/s** | **1.17x** |
| `canada.json`, struct parse | 485 | 723 MB/s | 768 MB/s | 1.07x |
| any file, stringify | **0** | 1545 MB/s | 1476 MB/s | 1.00x |

<sub>**The last row is the control, and it is why the rest is believable.** A
stringify into a pre-sized buffer allocates *zero* times, so the allocator
cannot touch it — and it doesn't. The effect sorts with the allocation count.
Numbers are Windows x86-64, where Rust's platform allocator is `HeapAlloc`;
glibc's malloc has tcache and fastbins and should close much of the gap, so the
Linux figure is unmeasured rather than implied. Full tables, method line and
every caveat: [`corpus/LEDGER.md`](https://github.com/Remade-With-Rust/rusty_json_turbo/blob/master/corpus/LEDGER.md).</sub>

## Features

| Feature | Default | Effect |
|---|:--:|---|
| `secure` | — | Guard pages and encrypted free lists — the profile for a service eating untrusted bytes. On this workload it kept nearly the whole speedup. |
| `debug_checks` | — | Extra allocator assertions. Debug profiles only. |

## What you are adopting it for

The house line is that `rusty_alloc` is adopted for the **safety posture**
first: a double free aborts instead of putting a block on a free list twice and
handing identical memory to two owners. Treat that abort as a bug to fix, never
a check to disable. The speed above is real, measured, and still the second
reason.

## Where this sits

| Crate | Role |
|---|---|
| [`rusty_json_turbo`](https://crates.io/crates/rusty_json_turbo) | the library — a drop-in `serde_json`, **depend on this** |
| **[`rusty_json_turbo-alloc`](https://crates.io/crates/rusty_json_turbo-alloc)** | **← you are here** — the allocator seam, for the binary |

## The Remade With Rust ecosystem

<!-- ORG BOILERPLATE — keep identical across repos -->

**Remade With Rust** is an initiative by **[Mata Network](https://www.mata.network/)**
to rebuild essential C and C++ tools in Rust — for the memory safety, the
predictable performance, and the freedom of a permissive license. Each project
is a reimplementation, not a fork: same wire protocols and file formats, new
code you can actually depend on. No copyleft. No surprises.

| Project | What it is |
|---|---|
| 🎬 **[remade_ffmpeg_rs](https://github.com/Remade-With-Rust/remade_ffmpeg_rs)** | **Our FFmpeg alternative.** Drop-in `ffmpeg` and `ffprobe` binaries — demux → decode → filter → encode → mux, rebuilt as composable Rust crates with **zero GPL/LGPL**. Apache-2.0. |
| 🧠 **[FFAI](https://github.com/Remade-With-Rust/FFAI)** | **Our sister project: media *for* AI.** Embedded ASR + TTS (**Mercury**), OCR (**Carmenta**) and vision-language captioning (**Argus**) behind an ffmpeg-style, swap-by-name architecture — no Python, no CUDA. MIT OR Apache-2.0. |
| 🌐 **[Mata Network](https://www.mata.network/)** | **The home page.** *"Stop sacrificing your privacy for convenience."* Sovereign, self-hostable privacy infrastructure. Remade With Rust is its open-source arm. |

→ All projects: **[github.com/Remade-With-Rust](https://github.com/Remade-With-Rust)**

<!-- /ORG BOILERPLATE -->

## License

MIT OR Apache-2.0, at your option.
