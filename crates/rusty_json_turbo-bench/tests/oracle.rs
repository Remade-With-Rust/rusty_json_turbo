//! The differential gate as a test: ours vs upstream serde_json, byte for byte.
//!
//! Fails on the first commit that changes an output byte, an error message, a
//! line/column, or a float bit anywhere in JSONCORP. Run with `--nocapture`
//! to see the counts the ledger records.

use rjt_bench::{corpus, oracle};

fn report(mismatches: &[oracle::Mismatch]) -> String {
    let mut s = String::new();
    for m in mismatches.iter().take(20) {
        s.push_str(&m.to_string());
        s.push('\n');
    }
    if mismatches.len() > 20 {
        s.push_str(&format!("... and {} more\n", mismatches.len() - 20));
    }
    s
}

#[test]
fn corpus_documents_are_byte_identical() {
    let docs = corpus::all_documents();
    assert!(
        docs.len() >= 60,
        "corpus looks incomplete: {} documents",
        docs.len()
    );
    let mut mismatches = Vec::new();
    for (name, bytes) in &docs {
        mismatches.extend(oracle::check_document(name, bytes));
    }
    assert!(mismatches.is_empty(), "{}", report(&mismatches));
    println!("oracle: {} corpus documents, 0 mismatches", docs.len());
}

#[test]
fn edge_documents_are_byte_identical() {
    let mut mismatches = Vec::new();
    let mut count = 0usize;
    for doc in oracle::EDGE_DOCUMENTS {
        mismatches.extend(oracle::check_document(
            &format!("edge {doc:?}"),
            doc.as_bytes(),
        ));
        count += 1;
    }
    for doc in oracle::EDGE_BYTES {
        mismatches.extend(oracle::check_document(&format!("edge bytes {doc:?}"), doc));
        count += 1;
    }
    for (name, bytes) in oracle::depth_documents() {
        mismatches.extend(oracle::check_document(&name, &bytes));
        count += 1;
    }
    assert!(mismatches.is_empty(), "{}", report(&mismatches));
    println!("oracle: {count} edge documents, 0 mismatches");
}

#[test]
fn number_tokens_are_bit_identical() {
    let mut tokens = std::collections::BTreeSet::new();
    for f in corpus::File::ALL {
        tokens.extend(oracle::number_tokens(&f.load()));
    }
    for t in oracle::EDGE_NUMBERS {
        tokens.insert((*t).to_owned());
    }
    let mut mismatches = Vec::new();
    for t in &tokens {
        mismatches.extend(oracle::check_number(t));
    }
    assert!(mismatches.is_empty(), "{}", report(&mismatches));
    println!(
        "oracle: {} distinct number tokens, 0 mismatches",
        tokens.len()
    );
}

/// The gate must be able to fail: a difference in either direction is reported.
#[test]
fn oracle_reports_a_difference() {
    let mut out = Vec::new();
    oracle::compare(
        &mut out,
        "poison",
        "self-check",
        "Ok(1)".into(),
        "Ok(2)".into(),
    );
    assert_eq!(
        out.len(),
        1,
        "a differing pair must produce exactly one mismatch"
    );
    let mut out = Vec::new();
    oracle::compare(
        &mut out,
        "poison",
        "self-check",
        "Ok(1)".into(),
        "Ok(1)".into(),
    );
    assert!(out.is_empty(), "an identical pair must produce no mismatch");
}
