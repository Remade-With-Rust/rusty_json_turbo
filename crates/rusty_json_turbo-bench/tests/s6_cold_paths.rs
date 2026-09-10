//! S6's gate, as a test: do the COLD paths still agree with upstream?
//!
//! The corpus this project measures on exercises the hot paths hard and the
//! cold ones barely at all. S1-S3 contain no hex escape, no surrogate pair, no
//! deep nesting, no 400-digit float and no integer near the `u64` boundary.
//! Those paths are where a byte-identical contract quietly breaks, because
//! nothing in the timed corpus would notice.
//!
//! Every case below runs through BOTH crates and is compared on everything
//! that is part of the contract: whether it parsed at all, the printed bytes,
//! and -- for a failure -- the error text WITH its line and column. A case
//! that fails in both crates for the same reason at the same position is a
//! pass; that is the point, not that any particular input is accepted.
//!
//! Escape sequences here are built from an explicit `BS` constant rather than
//! written inline, because a JSON escape is a BACKSLASH FOLLOWED BY A LETTER
//! and Rust's own string escapes make it easy to write instead the character
//! that denotes: a backslash-n written inline IS a newline, not the two
//! bytes a JSON escape needs.
//! Getting that wrong does not fail -- it silently tests the
//! wrong input, which is worse.

/// One backslash, as data.
const BS: char = '\u{5C}';

fn agree(name: &str, doc: &str) {
    let ours: Result<turbo::Value, _> = turbo::from_str(doc);
    let theirs: Result<serde_json_upstream::Value, _> = serde_json_upstream::from_str(doc);

    match (&ours, &theirs) {
        (Ok(a), Ok(b)) => {
            let a_s = turbo::to_string(a).expect("ours re-serialize");
            let b_s = serde_json_upstream::to_string(b).expect("theirs re-serialize");
            assert_eq!(a_s, b_s, "{name}: both parsed but printed differently");
        }
        (Err(a), Err(b)) => {
            assert_eq!(
                a.to_string(),
                b.to_string(),
                "{name}: both failed but with different messages"
            );
            assert_eq!(
                (a.line(), a.column()),
                (b.line(), b.column()),
                "{name}: both failed but at different positions"
            );
        }
        (Ok(_), Err(e)) => panic!("{name}: WE accepted what upstream rejected ({e})"),
        (Err(e), Ok(_)) => panic!("{name}: WE rejected what upstream accepted ({e})"),
    }
}

#[test]
fn deep_nesting() {
    // Straddles serde_json's default 128-frame recursion limit on both sides,
    // so the limit itself is part of what must match.
    for depth in [
        1usize, 2, 31, 32, 33, 63, 64, 100, 126, 127, 128, 129, 200, 500,
    ] {
        let arrays = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        agree(&format!("arrays deep {depth}"), &arrays);

        let objects = format!("{}1{}", r#"{"a":"#.repeat(depth), "}".repeat(depth));
        agree(&format!("objects deep {depth}"), &objects);
    }
}

#[test]
fn escape_heavy_strings() {
    // An escape every seven bytes, which is what the plan asks S6 to hold.
    let escapes = [
        format!("{BS}n"),
        format!("{BS}t"),
        format!("{BS}r"),
        format!("{BS}\""),
        format!("{BS}{BS}"),
        format!("{BS}/"),
        format!("{BS}b"),
        format!("{BS}f"),
    ];
    let mut s = String::from("\"");
    for i in 0..20_000usize {
        s.push_str("abcdef");
        s.push_str(&escapes[i % escapes.len()]);
    }
    s.push('"');
    agree("escape every 7 bytes", &s);
}

#[test]
fn hex_escapes_and_surrogates() {
    // Well-formed BMP escapes, across the 1, 2 and 3-byte UTF-8 boundaries.
    let mut s = String::from("\"");
    for cp in [0x0041u32, 0x00FF, 0x0100, 0x07FF, 0x0800, 0xFFFD, 0x20AC] {
        for _ in 0..500 {
            s.push_str(&format!("{BS}u{cp:04X}"));
        }
    }
    s.push('"');
    agree("hex escapes, BMP", &s);

    // Well-formed surrogate PAIRS: the path S1-S3 never enter at all.
    let mut p = String::from("\"");
    for _ in 0..2000 {
        p.push_str(&format!("{BS}uD83D{BS}uDE00")); // U+1F600
    }
    p.push('"');
    agree("surrogate pairs", &p);

    // Malformed on purpose. The requirement is that the two crates AGREE, not
    // that any particular one of these is accepted.
    let cases = [
        ("lone high surrogate", format!("\"{BS}uD800\"")),
        ("lone low surrogate", format!("\"{BS}uDC00\"")),
        ("high then non-surrogate", format!("\"{BS}uD800A\"")),
        ("high then bad escape", format!("\"{BS}uD800{BS}x\"")),
        ("high then second high", format!("\"{BS}uD800{BS}uD800\"")),
        ("truncated hex", format!("\"{BS}u00\"")),
        ("non-hex digits", format!("\"{BS}uZZZZ\"")),
        ("high surrogate at eof", format!("\"{BS}uD800")),
        ("bad escape letter", format!("\"{BS}q\"")),
        ("backslash at eof", format!("\"{BS}")),
    ];
    for (name, doc) in &cases {
        agree(name, doc);
    }
}

#[test]
fn numbers_at_every_boundary() {
    for (name, doc) in [
        ("u64 max", "18446744073709551615"),
        ("u64 max + 1", "18446744073709551616"),
        ("i64 min", "-9223372036854775808"),
        ("i64 min - 1", "-9223372036854775809"),
        ("20 digits", "12345678901234567890"),
        ("20 digits negative", "-12345678901234567890"),
        ("zero", "0"),
        ("negative zero", "-0"),
        ("negative zero float", "-0.0"),
        ("1e308", "1e308"),
        ("1e309 overflows", "1e309"),
        ("1e-400 underflows", "1e-400"),
        ("huge exponent", "1e99999999999999999999"),
        ("huge negative exponent", "1e-99999999999999999999"),
        ("exponent with leading zeros", "1e00000000000000000005"),
        ("tiny fraction", "0.00000000000000000000001"),
    ] {
        agree(name, doc);
    }
    // The 400-digit cases the plan names explicitly. Each routes differently:
    // a long fraction, a long integer, and a long fraction with an exponent.
    let long_frac = format!("0.{}", "1234567890".repeat(40));
    agree("400-digit fraction", &long_frac);
    agree("400-digit integer", &"9".repeat(400));
    agree("400-digit with exponent", &format!("{long_frac}e10"));
    agree("400-digit with big exponent", &format!("{long_frac}e308"));
}

#[test]
fn duplicate_and_repeated_keys() {
    // Last-wins is the documented behaviour; the crates must agree on which.
    agree("duplicate keys", r#"{"a":1,"a":2,"a":3}"#);
    let many = vec![r#""k":1"#; 5000].join(",");
    agree("same key 5000 times", &format!("{{{many}}}"));
}

#[test]
fn structural_churn() {
    agree("nested empties", "[[[[[]]]]]");
    agree("empty object chain", r#"{"a":{"b":{"c":{}}}}"#);
    let wide = vec!["[]"; 5000].join(",");
    agree("5000 empty arrays", &format!("[{wide}]"));
    let objs = vec!["{}"; 5000].join(",");
    agree("5000 empty objects", &format!("[{objs}]"));
}

/// The malformed inputs whose ERROR POSITION is the contract, not just the
/// fact of failure. An off-by-one in a scanner shows up here and nowhere else,
/// which is exactly why the newline cases are included: a whitespace scanner
/// that miscounts lines is invisible until an error has to name one.
#[test]
fn error_positions_are_identical() {
    let nl_cases = [
        ("newline then error on line 3", "[\n1,\n@\n]".to_string()),
        ("tab then error", "[\t\t@]".to_string()),
        ("crlf then error", "[\r\n1,\r\n@]".to_string()),
        ("many newlines then error", format!("{}@", "\n".repeat(50))),
        (
            "long whitespace run then error",
            format!("[{}@]", " ".repeat(200)),
        ),
        (
            "mixed whitespace then error",
            format!("[{}@]", " \t\r\n".repeat(50)),
        ),
        ("control char in string", "\"a\nb\"".to_string()),
        ("nul in string", "\"a\u{0}b\"".to_string()),
    ];
    for (name, doc) in &nl_cases {
        agree(name, doc);
    }

    for (name, doc) in [
        ("trailing comma array", "[1,2,]"),
        ("trailing comma object", r#"{"a":1,}"#),
        ("missing colon", r#"{"a" 1}"#),
        ("unclosed string", "\"abc"),
        ("unclosed array", "[1,2"),
        ("unclosed object", r#"{"a":1"#),
        ("bare word", "tru"),
        ("leading zero", "01"),
        ("leading plus", "+1"),
        ("lone minus", "-"),
        ("dot without digits", "1."),
        ("exponent without digits", "1e"),
        ("exponent sign only", "1e+"),
        ("two dots", "1.2.3"),
        ("empty input", ""),
        ("only whitespace", "   \n\t "),
        ("trailing garbage", "1 2"),
        ("comment attempt", "[1] // no"),
        ("single quotes", "'a'"),
        ("unquoted key", "{a:1}"),
    ] {
        agree(name, doc);
    }
}
