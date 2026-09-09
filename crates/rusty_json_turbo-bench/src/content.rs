//! A deterministic content census: where the bytes of a document actually go.
//!
//! Every brick in the catalog targets one class of byte -- whitespace, string
//! contents, digits, escapes -- and its ceiling is bounded by how many of those
//! bytes exist. That is a *count*, not a duration: exact, reproducible on any
//! machine under any load, and available before a single line of the brick is
//! written. `codec-measurement` puts this instrument first for exactly that
//! reason, and it has already killed one brick here before it was built.
//!
//! The walk is a small JSON scanner. It is not a validator: it assumes the
//! input parsed (the corpus does) and only classifies bytes.

/// Byte accounting for one document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Content {
    pub bytes: usize,
    /// Whitespace between tokens (the only whitespace a parser skips).
    pub whitespace: usize,
    /// Structural punctuation: `{}[],:` and the quotes that delimit strings.
    pub structural: usize,
    /// Bytes inside string literals, excluding the delimiting quotes.
    pub string_bytes: usize,
    /// Bytes inside string literals that are part of a `\` escape.
    pub escape_bytes: usize,
    /// Bytes belonging to number tokens.
    pub number_bytes: usize,
    /// Bytes belonging to `true` / `false` / `null`.
    pub literal_bytes: usize,
    /// Bytes >= 0x80 anywhere in the document.
    pub non_ascii: usize,

    pub strings: usize,
    /// Strings containing at least one `\` escape.
    pub strings_with_escapes: usize,
    pub numbers: usize,
    /// Number tokens containing `.`, `e` or `E`.
    pub floats: usize,
    /// The longest string literal, in bytes.
    pub longest_string: usize,
}

impl Content {
    fn pct(part: usize, whole: usize) -> f64 {
        if whole == 0 {
            0.0
        } else {
            part as f64 * 100.0 / whole as f64
        }
    }

    pub fn whitespace_pct(&self) -> f64 {
        Self::pct(self.whitespace, self.bytes)
    }
    pub fn string_pct(&self) -> f64 {
        Self::pct(self.string_bytes, self.bytes)
    }
    pub fn number_pct(&self) -> f64 {
        Self::pct(self.number_bytes, self.bytes)
    }
    pub fn structural_pct(&self) -> f64 {
        Self::pct(self.structural, self.bytes)
    }
    pub fn non_ascii_pct(&self) -> f64 {
        Self::pct(self.non_ascii, self.bytes)
    }
    pub fn escaped_string_pct(&self) -> f64 {
        Self::pct(self.strings_with_escapes, self.strings)
    }
    pub fn mean_string_len(&self) -> f64 {
        if self.strings == 0 {
            0.0
        } else {
            self.string_bytes as f64 / self.strings as f64
        }
    }
}

/// Classify every byte of a document.
pub fn census(input: &[u8]) -> Content {
    let mut c = Content {
        bytes: input.len(),
        ..Content::default()
    };
    c.non_ascii = input.iter().filter(|&&b| b >= 0x80).count();

    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => {
                c.whitespace += 1;
                i += 1;
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => {
                c.structural += 1;
                i += 1;
            }
            b'"' => {
                // Both quotes count as structural; the contents are string bytes.
                c.structural += 2;
                c.strings += 1;
                i += 1;
                let start = i;
                let mut escaped_here = false;
                while i < input.len() {
                    match input[i] {
                        b'"' => break,
                        b'\\' => {
                            escaped_here = true;
                            // A `\uXXXX` escape is 6 bytes; every other is 2.
                            let n = if input.get(i + 1) == Some(&b'u') {
                                6
                            } else {
                                2
                            };
                            let n = n.min(input.len() - i);
                            c.escape_bytes += n;
                            i += n;
                        }
                        _ => i += 1,
                    }
                }
                let len = i - start;
                c.string_bytes += len;
                c.longest_string = c.longest_string.max(len);
                if escaped_here {
                    c.strings_with_escapes += 1;
                }
                i += 1; // the closing quote
            }
            b'-' | b'0'..=b'9' => {
                c.numbers += 1;
                let start = i;
                let mut is_float = false;
                while i < input.len() {
                    match input[i] {
                        b'0'..=b'9' | b'-' | b'+' => i += 1,
                        b'.' | b'e' | b'E' => {
                            is_float = true;
                            i += 1;
                        }
                        _ => break,
                    }
                }
                c.number_bytes += i - start;
                if is_float {
                    c.floats += 1;
                }
            }
            b't' | b'f' | b'n' => {
                let n = match b {
                    b't' | b'n' => 4,
                    _ => 5,
                };
                let n = n.min(input.len() - i);
                c.literal_bytes += n;
                i += n;
            }
            _ => i += 1,
        }
    }
    c
}

/// Remove the whitespace a parser skips, and nothing else.
///
/// Every token byte survives untouched -- including whitespace *inside* string
/// literals, which a parser does not skip and which is part of the value. So
/// the result parses to a value equal to the original's, and the only work that
/// disappeared is whitespace skipping.
///
/// That makes it an exact ceiling probe for a whitespace brick, without
/// stubbing anything: run the same operation on both inputs and the time
/// difference *is* the cost of skipping whitespace. Re-serializing through the
/// DOM would not do -- it would also renormalise numbers and key order, and
/// then the probe would measure three changes instead of one.
pub fn strip_whitespace(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'"' => {
                let start = i;
                i += 1;
                while i < input.len() {
                    match input[i] {
                        b'"' => {
                            i += 1;
                            break;
                        }
                        b'\\' => i += 2,
                        _ => i += 1,
                    }
                }
                out.extend_from_slice(&input[start..i.min(input.len())]);
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_for_every_byte() {
        // The classes are disjoint and, on well-formed input, exhaustive.
        // `escape_bytes` is a subset of `string_bytes`, so it is not added.
        // Sources stay ASCII (house lint); non-ASCII input is spelled with
        // escapes, which is also how JSON carries it on the wire.
        for doc in [
            &br#"{"a": 1, "b": [true, null, 2.5e3], "c": "x\ny"}"#[..],
            &br#"[]"#[..],
            &b"{\"k\":\"\\u00e9\\\\\"}"[..],
            &b"{\"k\":\"\xc3\xa9\"}"[..],
            &br#"  {  "a"  :  -1  }  "#[..],
        ] {
            let c = census(doc);
            let accounted =
                c.whitespace + c.structural + c.string_bytes + c.number_bytes + c.literal_bytes;
            assert_eq!(
                accounted,
                c.bytes,
                "unaccounted bytes in {:?}: {c:?}",
                String::from_utf8_lossy(doc)
            );
        }
    }

    #[test]
    fn counts_the_shapes_it_claims() {
        let c = census(br#"{"a":"x\ny","b":[1,2.5,-3e4],"c":true}"#);
        assert_eq!(c.strings, 4, "3 keys + 1 value");
        assert_eq!(c.strings_with_escapes, 1);
        assert_eq!(c.numbers, 3);
        assert_eq!(c.floats, 2, "2.5 and -3e4");
        assert_eq!(c.literal_bytes, 4, "true");
        assert_eq!(c.escape_bytes, 2, "the \\n");
    }

    #[test]
    fn counts_unicode_escapes_as_six_bytes() {
        // The JSON text `"\u00e9"`.
        let c = census(b"\"\\u00e9\"");
        assert_eq!(c.escape_bytes, 6);
        assert_eq!(c.strings_with_escapes, 1);
    }

    #[test]
    fn stripping_keeps_every_token_byte() {
        let doc = br#"  { "a b" : [ 1 , "x\"y" ] , "c" : "  keep  me  " }  "#;
        let s = strip_whitespace(doc);
        assert_eq!(
            std::str::from_utf8(&s).unwrap(),
            r#"{"a b":[1,"x\"y"],"c":"  keep  me  "}"#,
            "whitespace inside strings is part of the value and must survive"
        );
        // The census agrees: no whitespace left outside strings.
        assert_eq!(census(&s).whitespace, 0);
    }

    #[test]
    fn counts_raw_utf8_as_non_ascii_not_as_escapes() {
        // The JSON text `"<e-acute>"` carried as raw UTF-8, not escaped.
        let c = census(b"\"\xc3\xa9\"");
        assert_eq!(c.non_ascii, 2);
        assert_eq!(c.escape_bytes, 0);
        assert_eq!(c.string_bytes, 2);
    }
}
