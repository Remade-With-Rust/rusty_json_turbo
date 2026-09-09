//! Which allocator this binary was linked with, and the allocation census.
//!
//! The global allocator is a **link-time** property: one per program, chosen in
//! the deliverable. So it cannot be A/B'd inside one process the way `ours` and
//! `upstream` are -- it needs two binaries and the process-level paired runner
//! (`tools/pinvs.ps1`). This module is what makes those two binaries
//! self-describing, so a ledger row can name exactly what it measured.
//!
//! Under `--features profile` the chosen backend is wrapped in a counting
//! allocator. That counter is the **work-parity instrument** for the allocator
//! experiment: the JSON code is identical across the arms, so the number of
//! allocations must be identical too, and what differs is the cost of each.
//! It is deterministic, needs no pinning, and one run settles it.
//!
//! **Never take a timing number from a `profile` build**: an atomic increment
//! per allocation is exactly the profiler tax `codec-measurement` warns about.
//! The census runs on its own build; the clock runs on a build without it.

// The one place this crate writes `unsafe`, and it is unavoidable: `GlobalAlloc`
// is an unsafe trait because the compiler cannot check an allocator's contract.
// Fenced to this module, every impl and every block carrying its invariant.
#![allow(unsafe_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicU64, Ordering};

/// The backend this binary links, selected at build time by the features.
#[cfg(feature = "rusty-alloc")]
pub type Backend = alloc_seam::Alloc;
#[cfg(not(feature = "rusty-alloc"))]
pub type Backend = std::alloc::System;

/// A value of the backend, for the `#[global_allocator]` static.
#[cfg(feature = "rusty-alloc")]
pub const BACKEND: Backend = alloc_seam::Alloc;
#[cfg(not(feature = "rusty-alloc"))]
pub const BACKEND: Backend = std::alloc::System;

/// The name that goes in the method line.
pub const fn name() -> &'static str {
    #[cfg(feature = "rusty-alloc")]
    {
        alloc_seam::NAME
    }
    #[cfg(not(feature = "rusty-alloc"))]
    {
        "system"
    }
}

/// True when this binary carries the counting wrapper, i.e. its timings are
/// taxed and must not be quoted.
pub const fn counting() -> bool {
    cfg!(feature = "profile")
}

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static REALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

/// Wraps the backend and counts what passes through it.
pub struct Counting<A>(pub A);

// SAFETY: every method forwards to the wrapped allocator with the same layout
// and pointer it was given, so the backend's contract is upheld unchanged; the
// counters are atomics and add no aliasing or lifetime obligations.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Counting<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: `layout` is forwarded exactly as the caller supplied it, so
        // the backend sees a caller-valid layout and its own contract holds.
        unsafe { self.0.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREES.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `ptr` came from this same allocator (we only ever return the
        // backend's own pointers) with this same `layout`; both are forwarded
        // untouched.
        unsafe { self.0.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: as `alloc` -- the layout is passed through unchanged.
        unsafe { self.0.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOCS.fetch_add(1, Ordering::Relaxed);
        if new_size > layout.size() {
            ALLOC_BYTES.fetch_add((new_size - layout.size()) as u64, Ordering::Relaxed);
        }
        // SAFETY: `ptr`/`layout` describe a live block from this allocator and
        // `new_size` is the caller's, all forwarded unchanged; the counters
        // touch no memory the backend owns.
        unsafe { self.0.realloc(ptr, layout, new_size) }
    }
}

/// A reading of the counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Census {
    pub allocs: u64,
    pub alloc_bytes: u64,
    pub reallocs: u64,
    pub frees: u64,
}

impl Census {
    pub fn read() -> Census {
        Census {
            allocs: ALLOCS.load(Ordering::Relaxed),
            alloc_bytes: ALLOC_BYTES.load(Ordering::Relaxed),
            reallocs: REALLOCS.load(Ordering::Relaxed),
            frees: FREES.load(Ordering::Relaxed),
        }
    }

    /// What happened between two readings.
    pub fn since(self, earlier: Census) -> Census {
        Census {
            allocs: self.allocs - earlier.allocs,
            alloc_bytes: self.alloc_bytes - earlier.alloc_bytes,
            reallocs: self.reallocs - earlier.reallocs,
            frees: self.frees - earlier.frees,
        }
    }
}

/// Run `f` once and report what it allocated. Returns `None` on a build
/// without the counting wrapper, so a caller cannot mistake zero for silence.
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, Option<Census>) {
    if !counting() {
        return (f(), None);
    }
    let before = Census::read();
    let value = f();
    let after = Census::read();
    (value, Some(after.since(before)))
}
