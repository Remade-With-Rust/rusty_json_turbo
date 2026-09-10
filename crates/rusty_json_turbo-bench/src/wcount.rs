//! A counting `io::Write`, to price brick B5 without touching the library.
//!
//! B5 proposes folding separators into adjacent writes and writing scalars
//! straight into a `Vec`'s spare capacity. At M1-A that brick was already
//! repriced once: the allocation census showed a stringify op into a pre-sized
//! buffer allocates **zero** times, so there are no allocations for B5 to
//! remove and no allocator effect for it to ride. What is left is the
//! `write_all` COUNT, and the mission plan's gate says so in as many words --
//! count the calls per stringify first, and if the count is already near one
//! per token, do not build the brick.
//!
//! That gate needs an instrument, and this is the cheapest possible one: the
//! serializer already writes through `io::Write`, so wrapping the sink counts
//! every call at the exact boundary the brick would change. No library code is
//! touched, which also means this instrument cannot perturb what it measures by
//! adding a counter to a hot inner loop.

use std::io::{self, Write};

/// A `Vec<u8>` sink that counts how it was called.
///
/// `write_all` is the serializer's normal route, but `Formatter` methods are
/// free to call `write` or `write_fmt` instead, so all three are counted
/// separately: a brick that reduced `write_all` calls by turning them into
/// `write_fmt` calls would not be an improvement, and only separate counts can
/// show that.
#[derive(Default)]
pub struct CountingWriter {
    /// The bytes written, so the caller can still check work parity.
    pub buf: Vec<u8>,
    /// Calls to `write_all`.
    pub write_all_calls: u64,
    /// Calls to `write`.
    pub write_calls: u64,
    /// Calls to `write_fmt`, which formats through `core::fmt` machinery.
    pub write_fmt_calls: u64,
    /// Calls to `flush`.
    pub flush_calls: u64,
    /// Total bytes handed to the sink across every call.
    pub bytes: u64,
}

impl CountingWriter {
    /// A sink pre-sized like the harness pre-sizes its real one, so the
    /// measured call pattern is the one the benchmark actually produces.
    #[must_use]
    pub fn with_capacity(n: usize) -> Self {
        CountingWriter {
            buf: Vec::with_capacity(n),
            ..Default::default()
        }
    }

    /// Every call, whatever route it took.
    #[must_use]
    pub fn total_calls(&self) -> u64 {
        self.write_all_calls + self.write_calls + self.write_fmt_calls
    }

    /// Mean bytes per call. The number B5 lives or dies by: a mean near the
    /// mean token length means the serializer is already making one call per
    /// token and there is nothing to fold.
    #[must_use]
    pub fn bytes_per_call(&self) -> f64 {
        let calls = self.total_calls();
        if calls == 0 {
            return 0.0;
        }
        self.bytes as f64 / calls as f64
    }

    /// Forget the counts, keep the capacity. Lets one buffer serve many
    /// iterations without an allocation confusing the picture.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.write_all_calls = 0;
        self.write_calls = 0;
        self.write_fmt_calls = 0;
        self.flush_calls = 0;
        self.bytes = 0;
    }
}

impl Write for CountingWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_calls += 1;
        self.bytes += data.len() as u64;
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all_calls += 1;
        self.bytes += data.len() as u64;
        self.buf.extend_from_slice(data);
        Ok(())
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> io::Result<()> {
        self.write_fmt_calls += 1;
        // Route through a scratch string so the inner `write_str` calls this
        // type does NOT see are not miscounted as separate sink calls.
        let s = args.to_string();
        self.bytes += s.len() as u64;
        self.buf.extend_from_slice(s.as_bytes());
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_calls += 1;
        Ok(())
    }
}
