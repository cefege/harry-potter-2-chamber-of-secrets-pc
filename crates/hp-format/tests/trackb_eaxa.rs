//! Track B gate: EA-XA decoder bit-exactness against fixtures extracted
//! from the first-party minimal-TU test (`Tests/EaxaTests.cpp`, ctest
//! `eaxa_decoder`). Vectors live in `Tests/Fixtures/eaxa/` and are shared
//! with the C++ oracle; they are never regenerated from this implementation.

use hp_format::eaxa::{BYTES_PER_BLOCK, EaxaDecoder, EaxaError, SAMPLES_PER_BLOCK};
use serde_json::Value;
use std::fs;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Tests/Fixtures/eaxa");

fn fixture(name: &str) -> Value {
    let path = format!("{FIXTURE_DIR}/{name}.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("missing fixture {path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("bad fixture {path}: {e}"))
}

fn pcm(value: &Value) -> Vec<i16> {
    value
        .as_array()
        .expect("pcm array")
        .iter()
        .map(|v| v.as_i64().expect("i16 sample") as i16)
        .collect()
}

fn hex_bytes(value: &Value) -> Vec<u8> {
    let hex = value.as_str().expect("hex string");
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex byte"))
        .collect()
}

/// FNV-1a 64 over each sample's little-endian byte pair — the same digest
/// the C++ oracle's `HashSamples` computes.
fn fnv1a(samples: &[i16]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for s in samples {
        for b in s.to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}
fn decode_fixture(name: &str) {
    let doc = fixture(name);
    let input = hex_bytes(&doc["input_hex"]);
    assert_eq!(input.len() % BYTES_PER_BLOCK, 0, "{name}: block alignment");
    let history = &doc["reset_history"];
    let feeds = doc.get("feed_count").and_then(Value::as_u64).unwrap_or(1) as usize;

    // Each C++ Feed installs a fresh stream while the decoder's predictor
    // history persists across feeds; replicate by feeding per round.
    let mut decoder = EaxaDecoder::new();
    decoder.reset_history(
        history[0].as_i64().unwrap() as i16,
        history[1].as_i64().unwrap() as i16,
    );

    let expected = pcm(&doc["expected_pcm"]);
    let per_feed = doc
        .get("num_samples")
        .and_then(Value::as_u64)
        .unwrap_or(input.len() as u64 / BYTES_PER_BLOCK as u64 * SAMPLES_PER_BLOCK as u64)
        as usize;

    let chunk_sizes = [3usize, 7, 257];
    let mut next = 0;
    let mut got = Vec::with_capacity(expected.len());
    for _round in 0..feeds {
        decoder
            .feed(&input, per_feed)
            .unwrap_or_else(|e| panic!("{name}: feed rejected: {e}"));
        // Each C++ Feed installs a fresh stream promising exactly per_feed
        // samples; drain this feed's promise before installing the next so
        // predictor history (not stream state) crosses the boundary.
        let round_target = got.len() + per_feed;
        while got.len() < round_target {
            let want = chunk_sizes[next % chunk_sizes.len()].min(expected.len() - got.len());
            next += 1;
            let mut buf = vec![0i16; want];
            let produced = decoder
                .decode(&mut buf)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                produced > 0,
                "{name}: stream ended early at {} of {}",
                got.len(),
                expected.len()
            );
            got.extend_from_slice(&buf[..produced]);
        }
    }
    assert_eq!(
        got, expected,
        "{name}: decoded PCM diverges from extracted vector"
    );

    // End of stream must be a quiet Ok(0), and it must be stable.
    let mut probe = [0i16; 1];
    assert_eq!(
        decoder.decode(&mut probe).unwrap(),
        0,
        "{name}: post-EOS decode"
    );
}

#[test]
fn signed_nibbles_and_low_high_order() {
    decode_fixture("signed_nibbles");
}

#[test]
fn partial_requests_cap_at_promised_samples() {
    decode_fixture("partial_requests");
}

#[test]
fn positive_clipping_saturates() {
    decode_fixture("clipping_positive");
}

#[test]
fn negative_clipping_saturates() {
    decode_fixture("clipping_negative");
}

#[test]
fn history_survives_feed_boundaries() {
    decode_fixture("history_two_feeds");
}

#[test]
fn predictor_sweep_matches_oracle_hashes() {
    let doc = fixture("predictor_sweep");
    let packed = hex_bytes(&doc["packed_hex"]);
    let range = doc["range"].as_u64().unwrap() as u32;
    let history = &doc["reset_history"];
    for predictor in 0..16usize {
        let mut block = Vec::with_capacity(BYTES_PER_BLOCK);
        block.push(((predictor as u8) << 4) | range as u8);
        block.extend_from_slice(&packed);
        let mut d = EaxaDecoder::new();
        d.reset_history(
            history[0].as_i64().unwrap() as i16,
            history[1].as_i64().unwrap() as i16,
        );
        d.feed(&block, SAMPLES_PER_BLOCK).unwrap();
        let out = d.decode_to_end().unwrap();
        assert_eq!(out.len(), SAMPLES_PER_BLOCK, "predictor {predictor}");
        let expect =
            u64::from_str_radix(doc["fnv1a_hashes_hex"][predictor].as_str().unwrap(), 16).unwrap();
        assert_eq!(fnv1a(&out), expect, "predictor {predictor} vector mismatch");
    }
}

#[test]
fn range_sweep_matches_oracle_hashes() {
    let doc = fixture("range_sweep");
    let packed = hex_bytes(&doc["packed_hex"]);
    let predictor = doc["predictor"].as_u64().unwrap() as u8;
    let history = &doc["reset_history"];
    for r in 0..16u32 {
        let mut block = Vec::with_capacity(BYTES_PER_BLOCK);
        block.push((predictor << 4) | r as u8);
        block.extend_from_slice(&packed);
        let mut d = EaxaDecoder::new();
        d.reset_history(
            history[0].as_i64().unwrap() as i16,
            history[1].as_i64().unwrap() as i16,
        );
        d.feed(&block, SAMPLES_PER_BLOCK).unwrap();
        let out = d.decode_to_end().unwrap();
        let expect =
            u64::from_str_radix(doc["fnv1a_hashes_hex"][r as usize].as_str().unwrap(), 16).unwrap();
        assert_eq!(fnv1a(&out), expect, "range {r} vector mismatch");
    }
}

#[test]
fn malformed_feeds_are_transactional() {
    let good = [0x0cu8, 0x21, 0x43, 0x65, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let truncated = &good[..14];
    let mut d = EaxaDecoder::new();
    d.feed(&good, 5).expect("valid baseline feed");

    assert_eq!(
        d.feed(truncated, 5),
        Err(EaxaError::BadBlockLength { got: 14 }),
        "partial block accepted"
    );
    assert_eq!(
        d.feed(&good, 29),
        Err(EaxaError::CapacityExceeded {
            requested: 29,
            capacity: 28
        }),
        "insufficient encoded data accepted"
    );

    // Rejected feeds left the prior stream intact.
    let mut out = [0i16; 5];
    assert_eq!(
        d.decode(&mut out).unwrap(),
        5,
        "rejected Feed changed prior stream"
    );
    assert_eq!(&out, &[1, 2, 3, 4, 5], "transactional Feed output mismatch");

    // Empty stream is legal and decodes nothing.
    d.feed(&[], 0).expect("empty stream rejected");
    assert_eq!(
        d.decode(&mut out).unwrap(),
        0,
        "empty stream produced samples"
    );
}
