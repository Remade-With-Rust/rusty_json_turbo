use crate::error::{Error, ErrorCode, Result};
use alloc::vec::Vec;
use core::cmp;
use core::mem;
use core::ops::Deref;
use core::str;

#[cfg(feature = "std")]
use crate::io;
#[cfg(feature = "std")]
use crate::iter::LineColIterator;

#[cfg(feature = "raw_value")]
use crate::raw::BorrowedRawDeserializer;
#[cfg(all(feature = "raw_value", feature = "std"))]
use crate::raw::OwnedRawDeserializer;
#[cfg(all(feature = "raw_value", feature = "std"))]
use alloc::string::String;
#[cfg(feature = "raw_value")]
use serde::de::Visitor;

/// Trait used by the deserializer for iterating over input. This is manually
/// "specialized" for iterating over `&[u8]`. Once feature(specialization) is
/// stable we can use actual specialization.
///
/// This trait is sealed and cannot be implemented for types outside of
/// `serde_json`.
pub trait Read<'de>: private::Sealed {
    #[doc(hidden)]
    fn next(&mut self) -> Result<Option<u8>>;
    #[doc(hidden)]
    fn peek(&mut self) -> Result<Option<u8>>;

    /// Only valid after a call to peek(). Discards the peeked byte.
    #[doc(hidden)]
    fn discard(&mut self);

    /// Skip insignificant whitespace and peek the next byte, without consuming
    /// it. Equivalent to `while peek() is whitespace { discard() }; peek()`.
    ///
    /// This exists as its own method because whitespace is the single largest
    /// class of byte in pretty-printed JSON -- 71.9% of `citm_catalog.json`,
    /// where skipping it measured **36% of the time to parse the document into
    /// a `Value`**. Going through `peek()`/`discard()` costs a `Result<Option>`
    /// construction, a match and a bounds check for every one of those bytes.
    /// A reader that owns a contiguous buffer can do far better, so it gets to
    /// override this; the default is exactly the loop it replaces.
    ///
    /// Only ` `, `\n`, `\t` and `\r` are insignificant. Any other byte <= 0x20
    /// is invalid at this position and must be RETURNED rather than skipped, so
    /// that the parser reports it at the right line and column.
    #[doc(hidden)]
    #[inline]
    fn skip_whitespace(&mut self) -> Result<Option<u8>> {
        loop {
            match tri!(self.peek()) {
                Some(b' ' | b'\n' | b'\t' | b'\r') => self.discard(),
                other => return Ok(other),
            }
        }
    }

    /// If the next eight bytes are all ASCII digits, consume them and return
    /// their value; otherwise consume nothing and return `None`.
    ///
    /// The number path is the whitespace path all over again: `canada.json` is
    /// **90.1% number bytes** -- 2.03 MB of digits across 111,126 numbers,
    /// about 17 digits each -- and every one of them goes through a
    /// `peek_or_null()`, a range test, an overflow check and an `eat_char()`.
    /// After the whitespace work that file still made 2.19 million `peek` and
    /// 2.14 million `discard` calls, essentially all of them here.
    ///
    /// The caller must only use this while an overflow is impossible for the
    /// whole chunk, so that the per-digit overflow check it replaces could not
    /// have fired either. The default implementation declines, which is correct
    /// for any reader that is not backed by a contiguous buffer.
    #[doc(hidden)]
    #[inline]
    fn take_8_digits(&mut self) -> Option<u32> {
        None
    }

    /// Position of the most recent call to next().
    ///
    /// The most recent call was probably next() and not peek(), but this method
    /// should try to return a sensible result if the most recent call was
    /// actually peek() because we don't always know.
    ///
    /// Only called in case of an error, so performance is not important.
    #[doc(hidden)]
    fn position(&self) -> Position;

    /// Position of the most recent call to peek().
    ///
    /// The most recent call was probably peek() and not next(), but this method
    /// should try to return a sensible result if the most recent call was
    /// actually next() because we don't always know.
    ///
    /// Only called in case of an error, so performance is not important.
    #[doc(hidden)]
    fn peek_position(&self) -> Position;

    /// Offset from the beginning of the input to the next byte that would be
    /// returned by next() or peek().
    #[doc(hidden)]
    fn byte_offset(&self) -> usize;

    /// Assumes the previous byte was a quotation mark. Parses a JSON-escaped
    /// string until the next quotation mark using the given scratch space if
    /// necessary. The scratch space is initially empty.
    #[doc(hidden)]
    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>>;

    /// Assumes the previous byte was a quotation mark. Parses a JSON-escaped
    /// string until the next quotation mark using the given scratch space if
    /// necessary. The scratch space is initially empty.
    ///
    /// This function returns the raw bytes in the string with escape sequences
    /// expanded but without performing unicode validation.
    #[doc(hidden)]
    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, [u8]>>;

    /// Assumes the previous byte was a quotation mark. Parses a JSON-escaped
    /// string until the next quotation mark but discards the data.
    #[doc(hidden)]
    fn ignore_str(&mut self) -> Result<()>;

    /// Assumes the previous byte was a hex escape sequence ('\u') in a string.
    /// Parses next hexadecimal sequence.
    #[doc(hidden)]
    fn decode_hex_escape(&mut self) -> Result<u16>;

    /// Switch raw buffering mode on.
    ///
    /// This is used when deserializing `RawValue`.
    #[cfg(feature = "raw_value")]
    #[doc(hidden)]
    fn begin_raw_buffering(&mut self);

    /// Switch raw buffering mode off and provides the raw buffered data to the
    /// given visitor.
    #[cfg(feature = "raw_value")]
    #[doc(hidden)]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>;

    /// Whether StreamDeserializer::next needs to check the failed flag. True
    /// for IoRead, false for StrRead and SliceRead which can track failure by
    /// truncating their input slice to avoid the extra check on every next
    /// call.
    #[doc(hidden)]
    const should_early_return_if_failed: bool;

    /// Mark a persistent failure of StreamDeserializer, either by setting the
    /// flag or by truncating the input data.
    #[doc(hidden)]
    fn set_failed(&mut self, failed: &mut bool);
}

pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub enum Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    Borrowed(&'b T),
    Copied(&'c T),
}

impl<'b, 'c, T> Deref for Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Reference::Borrowed(b) => b,
            Reference::Copied(c) => c,
        }
    }
}

/// JSON input source that reads from a std::io input stream.
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub struct IoRead<R>
where
    R: io::Read,
{
    iter: LineColIterator<io::Bytes<R>>,
    /// Temporary storage of peeked byte.
    ch: Option<u8>,
    #[cfg(feature = "raw_value")]
    raw_buffer: Option<Vec<u8>>,
}

/// JSON input source that reads from a slice of bytes.
//
// This is more efficient than other iterators because peek() can be read-only
// and we can compute line/col position only if an error happens.
pub struct SliceRead<'a> {
    slice: &'a [u8],
    /// Index of the *next* byte that will be returned by next() or peek().
    index: usize,
    #[cfg(feature = "raw_value")]
    raw_buffering_start_index: usize,
}

/// JSON input source that reads from a UTF-8 string.
//
// Able to elide UTF-8 checks by assuming that the input is valid UTF-8.
pub struct StrRead<'a> {
    delegate: SliceRead<'a>,
    #[cfg(feature = "raw_value")]
    data: &'a str,
}

// Prevent users from implementing the Read trait.
mod private {
    pub trait Sealed {}
}

//////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "std")]
impl<R> IoRead<R>
where
    R: io::Read,
{
    /// Create a JSON input source to read from a std::io input stream.
    ///
    /// When reading from a source against which short reads are not efficient, such
    /// as a [`File`], you will want to apply your own buffering because serde_json
    /// will not buffer the input. See [`std::io::BufReader`].
    ///
    /// [`File`]: std::fs::File
    pub fn new(reader: R) -> Self {
        IoRead {
            iter: LineColIterator::new(reader.bytes()),
            ch: None,
            #[cfg(feature = "raw_value")]
            raw_buffer: None,
        }
    }
}

#[cfg(feature = "std")]
impl<R> private::Sealed for IoRead<R> where R: io::Read {}

#[cfg(feature = "std")]
impl<R> IoRead<R>
where
    R: io::Read,
{
    fn parse_str_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        validate: bool,
        result: F,
    ) -> Result<T>
    where
        T: 's,
        F: FnOnce(&'s Self, &'s [u8]) -> Result<T>,
    {
        loop {
            let ch = tri!(next_or_eof(self));
            if !is_escape(ch, true) {
                scratch.push(ch);
                continue;
            }
            match ch {
                b'"' => {
                    return result(self, scratch);
                }
                b'\\' => {
                    tri!(parse_escape(self, validate, scratch));
                }
                _ => {
                    if validate {
                        return error(self, ErrorCode::ControlCharacterWhileParsingString);
                    }
                    scratch.push(ch);
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl<'de, R> Read<'de> for IoRead<R>
where
    R: io::Read,
{
    #[inline]
    fn next(&mut self) -> Result<Option<u8>> {
        match self.ch.take() {
            Some(ch) => {
                #[cfg(feature = "raw_value")]
                {
                    if let Some(buf) = &mut self.raw_buffer {
                        buf.push(ch);
                    }
                }
                Ok(Some(ch))
            }
            None => match self.iter.next() {
                Some(Err(err)) => Err(Error::io(err)),
                Some(Ok(ch)) => {
                    #[cfg(feature = "raw_value")]
                    {
                        if let Some(buf) = &mut self.raw_buffer {
                            buf.push(ch);
                        }
                    }
                    Ok(Some(ch))
                }
                None => Ok(None),
            },
        }
    }

    #[inline]
    fn peek(&mut self) -> Result<Option<u8>> {
        match self.ch {
            Some(ch) => Ok(Some(ch)),
            None => match self.iter.next() {
                Some(Err(err)) => Err(Error::io(err)),
                Some(Ok(ch)) => {
                    self.ch = Some(ch);
                    Ok(self.ch)
                }
                None => Ok(None),
            },
        }
    }

    #[cfg(not(feature = "raw_value"))]
    #[inline]
    fn discard(&mut self) {
        self.ch = None;
    }

    #[cfg(feature = "raw_value")]
    fn discard(&mut self) {
        if let Some(ch) = self.ch.take() {
            if let Some(buf) = &mut self.raw_buffer {
                buf.push(ch);
            }
        }
    }

    fn position(&self) -> Position {
        Position {
            line: self.iter.line(),
            column: self.iter.col(),
        }
    }

    fn peek_position(&self) -> Position {
        // The LineColIterator updates its position during peek() so it has the
        // right one here.
        self.position()
    }

    fn byte_offset(&self) -> usize {
        match self.ch {
            Some(_) => self.iter.byte_offset() - 1,
            None => self.iter.byte_offset(),
        }
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        self.parse_str_bytes(scratch, true, as_str)
            .map(Reference::Copied)
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, [u8]>> {
        self.parse_str_bytes(scratch, false, |_, bytes| Ok(bytes))
            .map(Reference::Copied)
    }

    fn ignore_str(&mut self) -> Result<()> {
        loop {
            let ch = tri!(next_or_eof(self));
            if !is_escape(ch, true) {
                continue;
            }
            match ch {
                b'"' => {
                    return Ok(());
                }
                b'\\' => {
                    tri!(ignore_escape(self));
                }
                _ => {
                    return error(self, ErrorCode::ControlCharacterWhileParsingString);
                }
            }
        }
    }

    fn decode_hex_escape(&mut self) -> Result<u16> {
        let a = tri!(next_or_eof(self));
        let b = tri!(next_or_eof(self));
        let c = tri!(next_or_eof(self));
        let d = tri!(next_or_eof(self));
        match decode_four_hex_digits(a, b, c, d) {
            Some(val) => Ok(val),
            None => error(self, ErrorCode::InvalidEscape),
        }
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        self.raw_buffer = Some(Vec::new());
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw_buffer.take().unwrap();
        let raw = match String::from_utf8(raw) {
            Ok(raw) => raw,
            Err(_) => return error(self, ErrorCode::InvalidUnicodeCodePoint),
        };
        visitor.visit_map(OwnedRawDeserializer {
            raw_value: Some(raw),
        })
    }

    const should_early_return_if_failed: bool = true;

    #[inline]
    #[cold]
    fn set_failed(&mut self, failed: &mut bool) {
        *failed = true;
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a> SliceRead<'a> {
    /// Create a JSON input source to read from a slice of bytes.
    pub fn new(slice: &'a [u8]) -> Self {
        SliceRead {
            slice,
            index: 0,
            #[cfg(feature = "raw_value")]
            raw_buffering_start_index: 0,
        }
    }

    fn position_of_index(&self, i: usize) -> Position {
        let start_of_line = match memchr::memrchr(b'\n', &self.slice[..i]) {
            Some(position) => position + 1,
            None => 0,
        };
        Position {
            line: 1 + memchr::memchr_iter(b'\n', &self.slice[..start_of_line]).count(),
            column: i - start_of_line,
        }
    }

    fn skip_to_escape(&mut self, forbid_control_characters: bool) {
        // Immediately bail-out on empty strings and consecutive escapes (e.g. \u041b\u0435)
        if self.index == self.slice.len()
            || is_escape(self.slice[self.index], forbid_control_characters)
        {
            return;
        }
        self.index += 1;

        let rest = &self.slice[self.index..];

        if !forbid_control_characters {
            self.index += memchr::memchr2(b'"', b'\\', rest).unwrap_or(rest.len());
            return;
        }

        // We wish to find the first byte in range 0x00..=0x1F or " or \. Ideally, we'd use
        // something akin to memchr3, but the memchr crate does not support this at the moment.
        // Therefore, we use a variation on Mycroft's algorithm [1] to provide performance better
        // than a naive loop. It runs faster than equivalent two-pass memchr2+SWAR code on
        // benchmarks and it's cross-platform, so probably the right fit.
        // [1]: https://groups.google.com/forum/#!original/comp.lang.c/2HtQXvg7iKc/xOJeipH6KLMJ

        #[cfg(fast_arithmetic = "64")]
        type Chunk = u64;
        #[cfg(fast_arithmetic = "32")]
        type Chunk = u32;

        const STEP: usize = mem::size_of::<Chunk>();
        const ONE_BYTES: Chunk = Chunk::MAX / 255; // 0x0101...01

        for chunk in rest.chunks_exact(STEP) {
            let chars = Chunk::from_le_bytes(chunk.try_into().unwrap());
            let contains_ctrl = chars.wrapping_sub(ONE_BYTES * 0x20) & !chars;
            let chars_quote = chars ^ (ONE_BYTES * Chunk::from(b'"'));
            let contains_quote = chars_quote.wrapping_sub(ONE_BYTES) & !chars_quote;
            let chars_backslash = chars ^ (ONE_BYTES * Chunk::from(b'\\'));
            let contains_backslash = chars_backslash.wrapping_sub(ONE_BYTES) & !chars_backslash;
            let masked = (contains_ctrl | contains_quote | contains_backslash) & (ONE_BYTES << 7);
            if masked != 0 {
                // SAFETY: chunk is in-bounds for slice
                self.index = unsafe { chunk.as_ptr().offset_from(self.slice.as_ptr()) } as usize
                    + masked.trailing_zeros() as usize / 8;
                return;
            }
        }

        self.index += rest.len() / STEP * STEP;
        self.skip_to_escape_slow();
    }

    #[cold]
    #[inline(never)]
    fn skip_to_escape_slow(&mut self) {
        while self.index < self.slice.len() && !is_escape(self.slice[self.index], true) {
            self.index += 1;
        }
    }

    /// The big optimization here over IoRead is that if the string contains no
    /// backslash escape sequences, the returned &str is a slice of the raw JSON
    /// data so we avoid copying into the scratch space.
    fn parse_str_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        validate: bool,
        result: F,
    ) -> Result<Reference<'a, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'f> FnOnce(&'s Self, &'f [u8]) -> Result<&'f T>,
    {
        // Index of the first byte not yet copied into the scratch space.
        let mut start = self.index;

        loop {
            self.skip_to_escape(validate);
            if self.index == self.slice.len() {
                return error(self, ErrorCode::EofWhileParsingString);
            }
            match self.slice[self.index] {
                b'"' => {
                    if scratch.is_empty() {
                        // Fast path: return a slice of the raw JSON without any
                        // copying.
                        let borrowed = &self.slice[start..self.index];
                        self.index += 1;
                        return result(self, borrowed).map(Reference::Borrowed);
                    } else {
                        scratch.extend_from_slice(&self.slice[start..self.index]);
                        self.index += 1;
                        return result(self, scratch).map(Reference::Copied);
                    }
                }
                b'\\' => {
                    scratch.extend_from_slice(&self.slice[start..self.index]);
                    self.index += 1;
                    tri!(parse_escape(self, validate, scratch));
                    start = self.index;
                }
                _ => {
                    self.index += 1;
                    return error(self, ErrorCode::ControlCharacterWhileParsingString);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Whitespace scanning.
//
// Insignificant JSON whitespace is exactly ` `, `\n`, `\t`, `\r`. Every other
// byte <= 0x20 is invalid where whitespace is allowed and must be RETURNED, not
// skipped, so the parser reports it at the right line and column. That rules
// out the tempting `b <= 0x20` test and any range test over 0x09..=0x0D, which
// would silently accept the vertical tab and form feed.
//
// Why a wide scan is worth it here, measured rather than assumed: on
// `citm_catalog.json` **98.0% of whitespace bytes live in runs of 8 or more**,
// and 92.3% in runs of 17 to 32 -- deep indentation, one run per line. On
// `twitter.json` it is 92.6% in runs of 8 or more. A per-byte loop walks all of
// it one compare at a time. (The run *mean* alone would have said the opposite;
// the distribution is what decides, and the two differ because most calls to
// `skip_whitespace` find no whitespace at all.)
// ---------------------------------------------------------------------------

const WS_ONES: u64 = 0x0101_0101_0101_0101;
const WS_HIGHS: u64 = 0x8080_8080_8080_8080;

#[inline(always)]
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\t' | b'\r')
}

/// High bit set in each byte of `x` that is zero, and clear in every other.
///
/// Exact per byte, unlike the classic `(x - ONES) & !x & HIGHS`, whose
/// subtraction borrows across byte boundaries and reports a byte as zero
/// because its neighbour was. Here the addition is `low7 + 0x7F <= 0xFE`, which
/// cannot carry out of its own byte, so no byte can be contaminated.
#[inline(always)]
fn zero_bytes(x: u64) -> u64 {
    // `low7 + 0x7F` sets bit 7 iff low7 != 0; OR the original bit 7 back in, so
    // the high bit marks a NON-zero byte. Invert for the zero bytes.
    !(((x & !WS_HIGHS).wrapping_add(!WS_HIGHS)) | x) & WS_HIGHS
}

/// High bit set in each byte of `x` equal to `c`.
#[inline(always)]
fn eq_bytes(x: u64, c: u8) -> u64 {
    zero_bytes(x ^ (c as u64).wrapping_mul(WS_ONES))
}

/// High bit set in each byte of `x` that is NOT insignificant whitespace.
#[inline(always)]
fn non_ws_bytes(x: u64) -> u64 {
    let ws = eq_bytes(x, b' ') | eq_bytes(x, b'\n') | eq_bytes(x, b'\t') | eq_bytes(x, b'\r');
    !ws & WS_HIGHS
}

// Both scanners return the byte they stopped on as well as its index.
//
// Returning only the index looked tidier and cost a measured 4% on
// `canada.json`: the caller had to load that byte a second time, on every one
// of the 557,593 calls a whitespace-free document makes. The file that CANNOT
// benefit from a whitespace change is the one that exposed it -- which is the
// whole reason to keep such a file in the corpus and to require that it reads
// exactly the old number.

/// The oracle: the first byte at or after `from` that is not insignificant
/// whitespace, with its index. One byte at a time.
///
/// Stays in the tree forever. It is what the wide scan is gated against, and
/// what runs when the knob turns the wide path off.
#[inline]
fn scan_ws_scalar(slice: &[u8], from: usize) -> (usize, Option<u8>) {
    let mut i = from;
    while i < slice.len() {
        let b = slice[i];
        if !is_ws(b) {
            return (i, Some(b));
        }
        i += 1;
    }
    (i, None)
}

/// How many bytes to walk one at a time before reaching for a wide step.
///
/// Not a guess. The run-length census says **46% of `twitter.json`'s whitespace
/// runs are a single byte** (13,345 of 28,826) and 34% of `citm_catalog`'s are.
/// Paying an eight-byte load and a SWAR test to discover that costs far more
/// than the two compares it replaces, and measured **0.903x on twitter scan,
/// 20/21** -- a real regression on a file with plenty of whitespace. Peeling a
/// few bytes first means a short run never reaches the wide path, while a long
/// one pays four extra compares out of seventeen or more.
const WS_PEEL: usize = 4;

/// Skip a whitespace run that is already known to have started, and return the
/// byte that ended it.
///
/// Deliberately NOT `inline(always)`: it runs only when there is whitespace to
/// skip, so inlining it into every call site would bloat the hot path that
/// mostly finds no whitespace at all.
#[inline]
fn scan_ws_run(slice: &[u8], from: usize) -> (usize, Option<u8>) {
    #[cfg(feature = "knobs")]
    if !ws_wide() {
        return scan_ws_scalar(slice, from);
    }

    let mut i = from;
    // Peel a short run without touching the wide path (see `WS_PEEL`).
    let peel = (from + WS_PEEL).min(slice.len());
    while i < peel {
        let b = slice[i];
        if !is_ws(b) {
            return (i, Some(b));
        }
        i += 1;
    }

    // Written so there is no panicking branch at all: no indexing, no unwrap,
    // no unreachable. Both fallible steps are provably infallible here and fold
    // away, and if either ever were not, the scalar tail below finishes the job
    // correctly rather than aborting.
    while let Some(window) = slice.get(i..i + 8) {
        let Ok(bytes) = <[u8; 8]>::try_from(window) else {
            break;
        };
        let chunk = u64::from_le_bytes(bytes);
        let m = non_ws_bytes(chunk);
        if m != 0 {
            // Little-endian: byte 0 is the lowest address, so the first set
            // high bit marks the first non-whitespace byte. Take the byte out
            // of the word already in hand rather than loading it again.
            let k = (m.trailing_zeros() >> 3) as usize;
            return (i + k, Some((chunk >> (k * 8)) as u8));
        }
        i += 8;
    }
    scan_ws_scalar(slice, i)
}

// ---------------------------------------------------------------------------
// Digit runs.
// ---------------------------------------------------------------------------

/// Are all eight bytes ASCII digits?
///
/// For a digit `c` in `0x30..=0x39`: `c & 0xF0 == 0x30`, and `c + 6` stays
/// inside `0x36..=0x3F` so `(c + 6) & 0xF0 == 0x30` too, giving `0x33` once the
/// second nibble is folded down. Any other byte fails its OWN test -- a
/// non-digit can carry into its neighbour, but it can never rescue itself, so
/// the chunk is always rejected and a false accept is impossible.
#[inline(always)]
fn all_8_digits(chunk: u64) -> bool {
    const LO: u64 = 0x0606_0606_0606_0606;
    const HI: u64 = 0xF0F0_F0F0_F0F0_F0F0;
    const WANT: u64 = 0x3333_3333_3333_3333;
    ((chunk & HI) | (((chunk.wrapping_add(LO)) & HI) >> 4)) == WANT
}

/// The value of eight ASCII digits, first character most significant.
///
/// The standard three-multiply fold (as in fast_float and simdjson). `chunk`
/// must have come from a little-endian load, so the first character is the
/// lowest byte, and every byte must be a digit -- check [`all_8_digits`] first.
/// Every step is deliberately WRAPPING. The intermediate products overflow
/// `u64` by design and the answer is read out of the high half, so plain `*`
/// would be correct in a release build and **panic in a debug build** -- a
/// consumer-visible bug that only shows up off the happy path. The unit test
/// below runs in debug, which is how this was caught.
#[inline(always)]
fn eight_digits_value(chunk: u64) -> u32 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 0x000F_4240_0000_0064; // 100 + (1_000_000 << 32)
    const MUL2: u64 = 0x0000_2710_0000_0001; // 1 + (10_000 << 32)
    let v = chunk.wrapping_sub(0x3030_3030_3030_3030);
    let v = v.wrapping_mul(10).wrapping_add(v >> 8);
    let lo = (v & MASK).wrapping_mul(MUL1);
    let hi = ((v >> 16) & MASK).wrapping_mul(MUL2);
    (lo.wrapping_add(hi) >> 32) as u32
}

// A FOUR-digit step was built on top of this one, twice, and refuted twice.
// It is not here, and the reason is a law rather than an accident -- see
// `parse_decimal` in `de.rs` and brick B4 in `corpus/LEDGER.md`.

/// Is the wide digit scan on? `knobs` builds only.
#[cfg(feature = "knobs")]
fn num_wide() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("RJT_NUM_WIDE").map_or(true, |v| v != "0"))
}

/// Is the wide whitespace scan on? `knobs` builds only.
#[cfg(feature = "knobs")]
fn ws_wide() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("RJT_WS_WIDE").map_or(true, |v| v != "0"))
}

/// The first non-whitespace byte at or after `from`, with its index.
///
/// `inline(always)`, and tiny on purpose. The overwhelmingly common case is
/// that there is no whitespace to skip at all -- `canada.json` takes it on
/// essentially all of its 557,593 calls, `citm_catalog` on 92,950 of 169,287 --
/// and that case must cost exactly what the original `peek()` cost: one load,
/// one test. Everything else is behind a call.
#[inline(always)]
fn scan_ws(slice: &[u8], from: usize) -> (usize, Option<u8>) {
    match slice.get(from) {
        Some(&b) if !is_ws(b) => (from, Some(b)),
        None => (from, None),
        _ => scan_ws_run(slice, from),
    }
}

/// Is the whitespace fast path on? `knobs` builds only; cached, so the
/// environment is read once per process rather than once per whitespace run.
///
/// This is the A/B instrument for the whitespace brick: one binary, one code
/// layout, one env var between the arms. Comparing two *builds* instead would
/// confound the change with its layout, and layout alone is worth +-13% here.
#[cfg(feature = "knobs")]
fn ws_fastpath() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("RJT_WS_FASTPATH").map_or(true, |v| v != "0"))
}

impl<'a> private::Sealed for SliceRead<'a> {}

impl<'a> Read<'a> for SliceRead<'a> {
    #[inline]
    fn next(&mut self) -> Result<Option<u8>> {
        // `Ok(self.slice.get(self.index).map(|ch| { self.index += 1; *ch }))`
        // is about 10% slower.
        crate::counters::add(&crate::counters::NEXT, 1);
        Ok(if self.index < self.slice.len() {
            let ch = self.slice[self.index];
            self.index += 1;
            Some(ch)
        } else {
            None
        })
    }

    #[inline]
    fn peek(&mut self) -> Result<Option<u8>> {
        // `Ok(self.slice.get(self.index).map(|ch| *ch))` is about 10% slower
        // for some reason.
        crate::counters::add(&crate::counters::PEEK, 1);
        Ok(if self.index < self.slice.len() {
            Some(self.slice[self.index])
        } else {
            None
        })
    }

    #[inline]
    fn discard(&mut self) {
        crate::counters::add(&crate::counters::DISCARD, 1);
        self.index += 1;
    }

    #[inline]
    fn take_8_digits(&mut self) -> Option<u32> {
        #[cfg(feature = "knobs")]
        if !num_wide() {
            return None;
        }
        // No `?`: the crate denies `clippy::question_mark_used`.
        let Some(window) = self.slice.get(self.index..self.index + 8) else {
            return None;
        };
        let Ok(bytes) = <[u8; 8]>::try_from(window) else {
            return None;
        };
        let chunk = u64::from_le_bytes(bytes);
        crate::counters::add(&crate::counters::D8_CALLS, 1);
        if all_8_digits(chunk) {
            crate::counters::add(&crate::counters::D8_HITS, 1);
            self.index += 8;
            Some(eight_digits_value(chunk))
        } else {
            None
        }
    }

    #[inline]
    fn skip_whitespace(&mut self) -> Result<Option<u8>> {
        // MEASUREMENT ARM, `profile` builds only: `RJT_WS_FASTPATH=0` forces the
        // old byte-at-a-time route so both arms can be COUNTED in one binary.
        // The knob is resolved once and cached, and it is read once per
        // whitespace run rather than per byte, so it does not itself become the
        // thing being measured.
        #[cfg(feature = "knobs")]
        if !ws_fastpath() {
            loop {
                match tri!(self.peek()) {
                    Some(b' ' | b'\n' | b'\t' | b'\r') => self.discard(),
                    other => return Ok(other),
                }
            }
        }

        // One walk of the slice, eight bytes at a time where the run is long
        // enough, instead of a `Result<Option<u8>>` round trip per byte through
        // peek()/discard(). Byte-for-byte the same decision as the default
        // loop: exactly the four insignificant whitespace bytes are skipped,
        // and the first byte that is not one of them is returned WITHOUT being
        // consumed.
        let slice = self.slice;
        let start = self.index;
        let (i, next) = scan_ws(slice, start);
        self.index = i;
        crate::counters::add(&crate::counters::WS_RUNS, 1);
        crate::counters::add(&crate::counters::WS_BYTES, (i - start) as u64);
        Ok(next)
    }

    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    fn peek_position(&self) -> Position {
        // Cap it at slice.len() just in case the most recent call was next()
        // and it returned the last byte.
        self.position_of_index(cmp::min(self.slice.len(), self.index + 1))
    }

    fn byte_offset(&self) -> usize {
        self.index
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.parse_str_bytes(scratch, true, as_str)
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'a, 's, [u8]>> {
        self.parse_str_bytes(scratch, false, |_, bytes| Ok(bytes))
    }

    fn ignore_str(&mut self) -> Result<()> {
        loop {
            self.skip_to_escape(true);
            if self.index == self.slice.len() {
                return error(self, ErrorCode::EofWhileParsingString);
            }
            match self.slice[self.index] {
                b'"' => {
                    self.index += 1;
                    return Ok(());
                }
                b'\\' => {
                    self.index += 1;
                    tri!(ignore_escape(self));
                }
                _ => {
                    return error(self, ErrorCode::ControlCharacterWhileParsingString);
                }
            }
        }
    }

    #[inline]
    fn decode_hex_escape(&mut self) -> Result<u16> {
        match self.slice[self.index..] {
            [a, b, c, d, ..] => {
                self.index += 4;
                match decode_four_hex_digits(a, b, c, d) {
                    Some(val) => Ok(val),
                    None => error(self, ErrorCode::InvalidEscape),
                }
            }
            _ => {
                self.index = self.slice.len();
                error(self, ErrorCode::EofWhileParsingString)
            }
        }
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        self.raw_buffering_start_index = self.index;
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'a>,
    {
        let raw = &self.slice[self.raw_buffering_start_index..self.index];
        let raw = match str::from_utf8(raw) {
            Ok(raw) => raw,
            Err(_) => return error(self, ErrorCode::InvalidUnicodeCodePoint),
        };
        visitor.visit_map(BorrowedRawDeserializer {
            raw_value: Some(raw),
        })
    }

    const should_early_return_if_failed: bool = false;

    #[inline]
    #[cold]
    fn set_failed(&mut self, _failed: &mut bool) {
        self.slice = &self.slice[..self.index];
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a> StrRead<'a> {
    /// Create a JSON input source to read from a UTF-8 string.
    pub fn new(s: &'a str) -> Self {
        StrRead {
            delegate: SliceRead::new(s.as_bytes()),
            #[cfg(feature = "raw_value")]
            data: s,
        }
    }
}

impl<'a> private::Sealed for StrRead<'a> {}

impl<'a> Read<'a> for StrRead<'a> {
    #[inline]
    fn next(&mut self) -> Result<Option<u8>> {
        self.delegate.next()
    }

    #[inline]
    fn peek(&mut self) -> Result<Option<u8>> {
        self.delegate.peek()
    }

    #[inline]
    fn discard(&mut self) {
        self.delegate.discard();
    }

    #[inline]
    fn skip_whitespace(&mut self) -> Result<Option<u8>> {
        self.delegate.skip_whitespace()
    }

    #[inline]
    fn take_8_digits(&mut self) -> Option<u32> {
        self.delegate.take_8_digits()
    }

    #[inline]
    fn position(&self) -> Position {
        self.delegate.position()
    }

    fn peek_position(&self) -> Position {
        self.delegate.peek_position()
    }

    fn byte_offset(&self) -> usize {
        self.delegate.byte_offset()
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.delegate.parse_str_bytes(scratch, true, |_, bytes| {
            // The deserialization input came in as &str with a UTF-8 guarantee,
            // and the \u-escapes are checked along the way, so don't need to
            // check here.
            Ok(unsafe { str::from_utf8_unchecked(bytes) })
        })
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'a, 's, [u8]>> {
        self.delegate.parse_str_raw(scratch)
    }

    fn ignore_str(&mut self) -> Result<()> {
        self.delegate.ignore_str()
    }

    fn decode_hex_escape(&mut self) -> Result<u16> {
        self.delegate.decode_hex_escape()
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        self.delegate.begin_raw_buffering();
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'a>,
    {
        let raw = &self.data[self.delegate.raw_buffering_start_index..self.delegate.index];
        visitor.visit_map(BorrowedRawDeserializer {
            raw_value: Some(raw),
        })
    }

    const should_early_return_if_failed: bool = false;

    #[inline]
    #[cold]
    fn set_failed(&mut self, failed: &mut bool) {
        self.delegate.set_failed(failed);
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'de, R> private::Sealed for &mut R where R: Read<'de> {}

impl<'de, R> Read<'de> for &mut R
where
    R: Read<'de>,
{
    fn next(&mut self) -> Result<Option<u8>> {
        R::next(self)
    }

    fn peek(&mut self) -> Result<Option<u8>> {
        R::peek(self)
    }

    fn discard(&mut self) {
        R::discard(self);
    }

    fn position(&self) -> Position {
        R::position(self)
    }

    fn peek_position(&self) -> Position {
        R::peek_position(self)
    }

    fn byte_offset(&self) -> usize {
        R::byte_offset(self)
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        R::parse_str(self, scratch)
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, [u8]>> {
        R::parse_str_raw(self, scratch)
    }

    fn ignore_str(&mut self) -> Result<()> {
        R::ignore_str(self)
    }

    fn decode_hex_escape(&mut self) -> Result<u16> {
        R::decode_hex_escape(self)
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        R::begin_raw_buffering(self);
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        R::end_raw_buffering(self, visitor)
    }

    const should_early_return_if_failed: bool = R::should_early_return_if_failed;

    fn set_failed(&mut self, failed: &mut bool) {
        R::set_failed(self, failed);
    }
}

//////////////////////////////////////////////////////////////////////////////

/// Marker for whether StreamDeserializer can implement FusedIterator.
pub trait Fused: private::Sealed {}
impl<'a> Fused for SliceRead<'a> {}
impl<'a> Fused for StrRead<'a> {}

fn is_escape(ch: u8, including_control_characters: bool) -> bool {
    ch == b'"' || ch == b'\\' || (including_control_characters && ch < 0x20)
}

fn next_or_eof<'de, R>(read: &mut R) -> Result<u8>
where
    R: ?Sized + Read<'de>,
{
    match tri!(read.next()) {
        Some(b) => Ok(b),
        None => error(read, ErrorCode::EofWhileParsingString),
    }
}

fn peek_or_eof<'de, R>(read: &mut R) -> Result<u8>
where
    R: ?Sized + Read<'de>,
{
    match tri!(read.peek()) {
        Some(b) => Ok(b),
        None => error(read, ErrorCode::EofWhileParsingString),
    }
}

fn error<'de, R, T>(read: &R, reason: ErrorCode) -> Result<T>
where
    R: ?Sized + Read<'de>,
{
    let position = read.position();
    Err(Error::syntax(reason, position.line, position.column))
}

fn as_str<'de, 's, R: Read<'de>>(read: &R, slice: &'s [u8]) -> Result<&'s str> {
    str::from_utf8(slice).or_else(|_| error(read, ErrorCode::InvalidUnicodeCodePoint))
}

/// Parses a JSON escape sequence and appends it into the scratch space. Assumes
/// the previous byte read was a backslash.
fn parse_escape<'de, R: Read<'de>>(
    read: &mut R,
    validate: bool,
    scratch: &mut Vec<u8>,
) -> Result<()> {
    let ch = tri!(next_or_eof(read));

    match ch {
        b'"' => scratch.push(b'"'),
        b'\\' => scratch.push(b'\\'),
        b'/' => scratch.push(b'/'),
        b'b' => scratch.push(b'\x08'),
        b'f' => scratch.push(b'\x0c'),
        b'n' => scratch.push(b'\n'),
        b'r' => scratch.push(b'\r'),
        b't' => scratch.push(b'\t'),
        b'u' => return parse_unicode_escape(read, validate, scratch),
        _ => return error(read, ErrorCode::InvalidEscape),
    }

    Ok(())
}

/// Parses a JSON \u escape and appends it into the scratch space. Assumes `\u`
/// has just been read.
#[cold]
fn parse_unicode_escape<'de, R: Read<'de>>(
    read: &mut R,
    validate: bool,
    scratch: &mut Vec<u8>,
) -> Result<()> {
    let mut n = tri!(read.decode_hex_escape());

    // Non-BMP characters are encoded as a sequence of two hex escapes,
    // representing UTF-16 surrogates. If deserializing a utf-8 string the
    // surrogates are required to be paired, whereas deserializing a byte string
    // accepts lone surrogates.
    if validate && n >= 0xDC00 && n <= 0xDFFF {
        // XXX: This is actually a trailing surrogate.
        return error(read, ErrorCode::LoneLeadingSurrogateInHexEscape);
    }

    loop {
        if n < 0xD800 || n > 0xDBFF {
            // Every u16 outside of the surrogate ranges is guaranteed to be a
            // legal char.
            push_wtf8_codepoint(n as u32, scratch);
            return Ok(());
        }

        // n is a leading surrogate, we now expect a trailing surrogate.
        let n1 = n;

        if tri!(peek_or_eof(read)) == b'\\' {
            read.discard();
        } else {
            return if validate {
                read.discard();
                error(read, ErrorCode::UnexpectedEndOfHexEscape)
            } else {
                push_wtf8_codepoint(n1 as u32, scratch);
                Ok(())
            };
        }

        if tri!(peek_or_eof(read)) == b'u' {
            read.discard();
        } else {
            return if validate {
                read.discard();
                error(read, ErrorCode::UnexpectedEndOfHexEscape)
            } else {
                push_wtf8_codepoint(n1 as u32, scratch);
                // The \ prior to this byte started an escape sequence, so we
                // need to parse that now. This recursive call does not blow the
                // stack on malicious input because the escape is not \u, so it
                // will be handled by one of the easy nonrecursive cases.
                parse_escape(read, validate, scratch)
            };
        }

        let n2 = tri!(read.decode_hex_escape());

        if n2 < 0xDC00 || n2 > 0xDFFF {
            if validate {
                return error(read, ErrorCode::LoneLeadingSurrogateInHexEscape);
            }
            push_wtf8_codepoint(n1 as u32, scratch);
            // If n2 is a leading surrogate, we need to restart.
            n = n2;
            continue;
        }

        // This value is in range U+10000..=U+10FFFF, which is always a valid
        // codepoint.
        let n = ((((n1 - 0xD800) as u32) << 10) | (n2 - 0xDC00) as u32) + 0x1_0000;
        push_wtf8_codepoint(n, scratch);
        return Ok(());
    }
}

/// Adds a WTF-8 codepoint to the end of the buffer. This is a more efficient
/// implementation of String::push. The codepoint may be a surrogate.
#[inline]
fn push_wtf8_codepoint(n: u32, scratch: &mut Vec<u8>) {
    if n < 0x80 {
        scratch.push(n as u8);
        return;
    }

    scratch.reserve(4);

    // SAFETY: After the `reserve` call, `scratch` has at least 4 bytes of
    // allocated but uninitialized memory after its last initialized byte, which
    // is where `ptr` points. All reachable match arms write `encoded_len` bytes
    // to that region and update the length accordingly, and `encoded_len` is
    // always <= 4.
    unsafe {
        let ptr = scratch.as_mut_ptr().add(scratch.len());

        let encoded_len = match n {
            0..=0x7F => unreachable!(),
            0x80..=0x7FF => {
                ptr.write(((n >> 6) & 0b0001_1111) as u8 | 0b1100_0000);
                2
            }
            0x800..=0xFFFF => {
                ptr.write(((n >> 12) & 0b0000_1111) as u8 | 0b1110_0000);
                ptr.add(1)
                    .write(((n >> 6) & 0b0011_1111) as u8 | 0b1000_0000);
                3
            }
            0x1_0000..=0x10_FFFF => {
                ptr.write(((n >> 18) & 0b0000_0111) as u8 | 0b1111_0000);
                ptr.add(1)
                    .write(((n >> 12) & 0b0011_1111) as u8 | 0b1000_0000);
                ptr.add(2)
                    .write(((n >> 6) & 0b0011_1111) as u8 | 0b1000_0000);
                4
            }
            0x11_0000.. => unreachable!(),
        };
        ptr.add(encoded_len - 1)
            .write((n & 0b0011_1111) as u8 | 0b1000_0000);

        scratch.set_len(scratch.len() + encoded_len);
    }
}

/// Parses a JSON escape sequence and discards the value. Assumes the previous
/// byte read was a backslash.
fn ignore_escape<'de, R>(read: &mut R) -> Result<()>
where
    R: ?Sized + Read<'de>,
{
    let ch = tri!(next_or_eof(read));

    match ch {
        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
        b'u' => {
            // At this point we don't care if the codepoint is valid. We just
            // want to consume it. We don't actually know what is valid or not
            // at this point, because that depends on if this string will
            // ultimately be parsed into a string or a byte buffer in the "real"
            // parse.

            tri!(read.decode_hex_escape());
        }
        _ => {
            return error(read, ErrorCode::InvalidEscape);
        }
    }

    Ok(())
}

const fn decode_hex_val_slow(val: u8) -> Option<u8> {
    match val {
        b'0'..=b'9' => Some(val - b'0'),
        b'A'..=b'F' => Some(val - b'A' + 10),
        b'a'..=b'f' => Some(val - b'a' + 10),
        _ => None,
    }
}

const fn build_hex_table(shift: usize) -> [i16; 256] {
    let mut table = [0; 256];
    let mut ch = 0;
    while ch < 256 {
        table[ch] = match decode_hex_val_slow(ch as u8) {
            Some(val) => (val as i16) << shift,
            None => -1,
        };
        ch += 1;
    }
    table
}

static HEX0: [i16; 256] = build_hex_table(0);
static HEX1: [i16; 256] = build_hex_table(4);

fn decode_four_hex_digits(a: u8, b: u8, c: u8, d: u8) -> Option<u16> {
    let a = HEX1[a as usize] as i32;
    let b = HEX0[b as usize] as i32;
    let c = HEX1[c as usize] as i32;
    let d = HEX0[d as usize] as i32;

    let codepoint = ((a | b) << 8) | c | d;

    // A single sign bit check.
    if codepoint >= 0 {
        Some(codepoint as u16)
    } else {
        None
    }
}

#[cfg(test)]
mod ws_tests {
    use super::{is_ws, scan_ws, scan_ws_scalar};

    /// Index only, for the comparisons below.
    fn wide(s: &[u8], from: usize) -> usize {
        scan_ws(s, from).0
    }
    fn scalar(s: &[u8], from: usize) -> usize {
        scan_ws_scalar(s, from).0
    }

    /// The byte each scanner reports must be the byte at the index it reports.
    /// Returning a stale or re-loaded byte here would be invisible to an
    /// index-only comparison and would corrupt every token boundary.
    fn agree(s: &[u8], from: usize) {
        let (wi, wb) = scan_ws(s, from);
        let (si, sb) = scan_ws_scalar(s, from);
        assert_eq!(wi, si, "index disagreement at {from} in {s:?}");
        assert_eq!(wb, sb, "byte disagreement at {from} in {s:?}");
        assert_eq!(wb, s.get(wi).copied(), "byte is not the one at the index");
    }
    use alloc::vec;
    use alloc::vec::Vec;

    /// The gate the house requires of any wide kernel: it must agree with its
    /// scalar twin on every byte value at every offset, not merely on a corpus.
    #[test]
    fn wide_matches_scalar_for_every_byte_at_every_offset() {
        // A window long enough that the wide loop runs at least twice, so a
        // byte can be placed before, inside and after the first chunk.
        for pos in 0..24usize {
            for b in 0..=255u8 {
                let mut buf = [b' '; 24];
                buf[pos] = b;
                for from in 0..24usize {
                    let want = scalar(&buf, from);
                    let got = wide(&buf, from);
                    assert_eq!(
                        got, want,
                        "byte {b:#04x} at {pos}, scanning from {from}: wide said {got}, scalar {want}"
                    );
                    // ... and the byte reported must be the byte at that index.
                    agree(&buf, from);
                }
            }
        }
    }

    /// Every length from empty to past two chunks, all-whitespace and none.
    #[test]
    fn wide_matches_scalar_at_every_length() {
        for len in 0..40usize {
            for fill in [b' ', b'\t', b'\n', b'\r', b'x', 0x0b, 0x0c, 0x00] {
                let buf = vec![fill; len];
                for from in 0..=len {
                    assert_eq!(
                        wide(&buf, from),
                        scalar(&buf, from),
                        "fill {fill:#04x}, len {len}, from {from}"
                    );
                    agree(&buf, from);
                }
            }
        }
    }

    /// Mixed content, including the bytes a range test would wrongly skip.
    #[test]
    fn wide_matches_scalar_on_mixed_content() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let alphabet = [
            b' ', b' ', b' ', b'\t', b'\n', b'\r', // whitespace, over-represented
            0x0b, 0x0c, 0x00, 0x1f, // NOT whitespace, and <= 0x20: the trap
            b'{', b'}', b'"', b'1', b'a', 0xff,
        ];
        for _ in 0..2000 {
            let mut buf = Vec::new();
            for _ in 0..64 {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                buf.push(alphabet[(seed >> 33) as usize % alphabet.len()]);
            }
            for from in 0..buf.len() {
                assert_eq!(
                    wide(&buf, from),
                    scalar(&buf, from),
                    "buf {buf:?} from {from}"
                );
                agree(&buf, from);
            }
        }
    }

    /// The control chars a `b <= 0x20` or `0x09..=0x0d` test would wrongly
    /// treat as whitespace. Skipping them would silently accept invalid JSON.
    #[test]
    fn vertical_tab_and_form_feed_are_not_whitespace() {
        for b in [0x0b_u8, 0x0c, 0x00, 0x01, 0x1f, 0x21] {
            assert!(!is_ws(b), "{b:#04x} must not be skippable whitespace");
            let mut buf = [b' '; 16];
            buf[10] = b;
            assert_eq!(wide(&buf, 0), 10);
            assert_eq!(scalar(&buf, 0), 10);
        }
    }
}

#[cfg(test)]
mod digit_tests {
    use super::{all_8_digits, eight_digits_value};
    use alloc::format;

    /// What the SWAR fold must agree with, computed the obvious way.
    fn scalar_value(s: &[u8; 8]) -> u32 {
        s.iter()
            .fold(0u32, |acc, &c| acc * 10 + u32::from(c - b'0'))
    }

    #[test]
    fn accepts_only_all_digit_chunks() {
        let digits = *b"12345678";
        assert!(all_8_digits(u64::from_le_bytes(digits)));
        // A single non-digit anywhere must reject the chunk. Byte values are
        // exhaustive, positions are exhaustive.
        for pos in 0..8 {
            for b in 0..=255u8 {
                let mut buf = digits;
                buf[pos] = b;
                let want = b.is_ascii_digit();
                assert_eq!(
                    all_8_digits(u64::from_le_bytes(buf)),
                    want,
                    "byte {b:#04x} at {pos}"
                );
            }
        }
    }

    #[test]
    fn value_matches_scalar_on_every_boundary_and_at_random() {
        for s in [
            b"00000000",
            b"00000001",
            b"10000000",
            b"99999999",
            b"12345678",
            b"87654321",
            b"09090909",
            b"90909090",
        ] {
            let chunk = u64::from_le_bytes(*s);
            assert!(all_8_digits(chunk));
            assert_eq!(eight_digits_value(chunk), scalar_value(s), "{:?}", s);
        }
        // Every digit in every position, the rest zeroes: catches a fold that
        // weights a position wrongly.
        for pos in 0..8 {
            for d in b'0'..=b'9' {
                let mut buf = *b"00000000";
                buf[pos] = d;
                let chunk = u64::from_le_bytes(buf);
                assert_eq!(
                    eight_digits_value(chunk),
                    scalar_value(&buf),
                    "digit {} at {pos}",
                    d as char
                );
            }
        }
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..20_000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let n = (seed >> 32) % 100_000_000;
            let s = format!("{n:08}");
            let buf = <[u8; 8]>::try_from(s.as_bytes()).unwrap();
            let chunk = u64::from_le_bytes(buf);
            assert!(all_8_digits(chunk), "{s}");
            assert_eq!(eight_digits_value(chunk), n as u32, "{s}");
        }
    }
}
