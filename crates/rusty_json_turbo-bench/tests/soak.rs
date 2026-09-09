//! A differential soak: generated and mutated inputs, ours against upstream.
//!
//! `fuzz/fuzz_targets/oracle_diff.rs` asks the same question under libFuzzer,
//! which is coverage-guided and therefore better at finding the weird case --
//! but on Windows it needs a matching ASan runtime, and CI runs it only on
//! Linux. This test asks it everywhere, on every `cargo test`, with a fixed
//! seed so a failure reproduces exactly.
//!
//! It is deliberately cheap by default (a few thousand cases, ~a second).
//! `RJT_SOAK=200000 cargo test -p rusty_json_turbo-bench --release --test soak`
//! turns it into a real soak.

use rjt_bench::{corpus, oracle};

/// A tiny deterministic PRNG, so a failing case is reproducible from its seed
/// alone -- no corpus file to lose.
struct Lcg(u64);

impl Lcg {
    fn next_u32(&mut self) -> u32 {
        // Numerical Recipes LCG; fine for shaping test inputs.
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            self.next_u32() as usize % n
        }
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// Bytes that look enough like JSON to reach the interesting code paths.
fn generate(rng: &mut Lcg, depth: usize, out: &mut String) {
    const ATOMS: [&str; 22] = [
        "null",
        "true",
        "false",
        "0",
        "-0",
        "1",
        "-1",
        "1e5",
        "1E-5",
        "1.5",
        "0.1",
        "18446744073709551616",
        "9007199254740993",
        "1e309",
        "4.9e-324",
        "\"\"",
        "\"a\"",
        "\"\\u00e9\"",
        "\"\\ud83d\\ude00\"",
        "\"\\ud800\"",
        "\"\\\\\"",
        "\"\\u0000\"",
    ];
    if depth == 0 || rng.below(3) == 0 {
        out.push_str(rng.pick(&ATOMS));
        return;
    }
    let n = rng.below(4);
    if rng.below(2) == 0 {
        out.push('[');
        for i in 0..n {
            if i > 0 {
                out.push(',');
            }
            generate(rng, depth - 1, out);
        }
        out.push(']');
    } else {
        out.push('{');
        for i in 0..n {
            if i > 0 {
                out.push(',');
            }
            out.push('"');
            out.push_str(rng.pick(&["k", "", "a b", "\\u00e9", "dup", "dup"]));
            out.push_str("\":");
            generate(rng, depth - 1, out);
        }
        out.push('}');
    }
}

/// Corrupt a document the way a network or a disk would.
fn mutate(rng: &mut Lcg, bytes: &mut Vec<u8>) {
    if bytes.is_empty() {
        bytes.push(b'{');
        return;
    }
    match rng.below(6) {
        0 => {
            let i = rng.below(bytes.len());
            bytes[i] = rng.next_u32() as u8;
        }
        1 => {
            let i = rng.below(bytes.len());
            bytes[i] = *rng.pick(b"{}[]\",:\\ \n\t0123456789eE+-.");
        }
        2 => {
            let i = rng.below(bytes.len());
            bytes.remove(i);
        }
        3 => {
            let i = rng.below(bytes.len() + 1);
            bytes.insert(i, *rng.pick(b"{}[]\",:\\ \x00\xff"));
        }
        4 => {
            let n = rng.below(bytes.len());
            bytes.truncate(n);
        }
        _ => {
            let i = rng.below(bytes.len());
            let j = rng.below(bytes.len());
            bytes.swap(i, j);
        }
    }
}

fn cases() -> usize {
    std::env::var("RJT_SOAK")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4000)
}

fn assert_agrees(name: &str, bytes: &[u8], seed: u64) {
    let mismatches = oracle::check_document(name, bytes);
    assert!(
        mismatches.is_empty(),
        "divergence from upstream at seed {seed}\ninput: {:?}\n{}",
        String::from_utf8_lossy(bytes),
        mismatches
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn generated_documents_agree_with_upstream() {
    let n = cases();
    let mut rng = Lcg(0x5eed_1234_5678_9abc);
    for i in 0..n {
        let seed = rng.0;
        let mut text = String::new();
        generate(&mut rng, 4, &mut text);
        assert_agrees(&format!("generated #{i}"), text.as_bytes(), seed);
    }
    println!("soak: {n} generated documents, 0 divergences");
}

#[test]
fn mutated_documents_agree_with_upstream() {
    let n = cases();
    let mut rng = Lcg(0x0d15_ea5e_0000_0001);
    // Mutate real documents: the shapes a corpus file actually contains, broken.
    let seeds: Vec<Vec<u8>> = corpus::json_files_in(&corpus::corpus_dir().join("roundtrip"), "rt")
        .into_iter()
        .map(|(_, b)| b)
        .chain(std::iter::once(
            br#"{"a":[1,2,{"b":"\u00e9"}],"c":null}"#.to_vec(),
        ))
        .collect();
    assert!(!seeds.is_empty(), "no seed documents found");
    for i in 0..n {
        let seed = rng.0;
        let mut bytes = seeds[rng.below(seeds.len())].clone();
        for _ in 0..=rng.below(4) {
            mutate(&mut rng, &mut bytes);
        }
        assert_agrees(&format!("mutated #{i}"), &bytes, seed);
    }
    println!("soak: {n} mutated documents, 0 divergences");
}

#[test]
fn generated_numbers_agree_bit_for_bit() {
    let n = cases();
    let mut rng = Lcg(0x000f_10a7_0000_0002);
    for _ in 0..n {
        // Assemble number-ish text from the parts that decide which parse path
        // runs: sign, digit count, fraction, exponent.
        let mut t = String::new();
        if rng.below(4) == 0 {
            t.push('-');
        }
        let digits = 1 + rng.below(25);
        for _ in 0..digits {
            t.push((b'0' + (rng.below(10) as u8)) as char);
        }
        if rng.below(2) == 0 {
            t.push('.');
            for _ in 0..=rng.below(20) {
                t.push((b'0' + (rng.below(10) as u8)) as char);
            }
        }
        if rng.below(3) == 0 {
            t.push(*rng.pick(&['e', 'E']));
            if rng.below(2) == 0 {
                t.push(*rng.pick(&['+', '-']));
            }
            for _ in 0..=rng.below(3) {
                t.push((b'0' + (rng.below(10) as u8)) as char);
            }
        }
        let m = oracle::check_number(&t);
        assert!(
            m.is_empty(),
            "float/int divergence on {t:?}:\n{}",
            m.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    println!("soak: {n} generated number tokens, 0 divergences");
}
