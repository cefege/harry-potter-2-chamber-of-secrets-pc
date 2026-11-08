//! Replays the captured original `UnLight.cpp` table fills from
//! `fixtures/light_tables.txt`: `FLightManager::Init` (`LightDist`,
//! `LightDistInc`, `ByteFog`, `FilterTab`) and `FLightManager::ShadowMapGen`.
//! The fixture is written by `Build/light_fixture_harness/sec_tables.inc`
//! against the original `UnLight.cpp` (git ref `c9fc381`) with the product
//! flags. Results must match bit for bit.

#![cfg(target_arch = "aarch64")]

use std::collections::BTreeMap;
use std::ffi::c_char;

use hp2_render::light::{
    BYTE_FOG_LEN, FILTER_TAB_LEN, LIGHT_DIST_LEN, init_byte_fog, init_filter_tab, init_light_dist,
    shadow_map_gen,
};

const FIXTURE: &str = include_str!("fixtures/light_tables.txt");

/// Record count per kind, as emitted by `light_gen_tables`.
const EXPECTED: &[(&str, usize)] = &[
    ("lightdist", LIGHT_DIST_LEN),
    ("lightdistinc", LIGHT_DIST_LEN),
    ("bytefog", BYTE_FOG_LEN),
    ("filter", FILTER_TAB_LEN * 4),
    ("shadow", 4 + 8 + 24 + 24 + 128),
    ("shadow_null", 12 + 128),
    ("shadow_single", 5 + 13),
    ("shadow_single_null", 13),
];

/// `(kind, case, ShadowMaskU, VClamp)`. `shadow` cases have a source; `shadow_null` cases do not.
const SHADOW_CASES: &[(&str, u32, usize, usize)] = &[
    ("shadow", 0, 1, 2),
    ("shadow", 1, 2, 2),
    ("shadow", 2, 3, 4),
    ("shadow", 3, 4, 3),
    ("shadow", 4, 8, 8),
    ("shadow_null", 5, 2, 3),
    ("shadow_null", 6, 8, 8),
];

/// The driver never fires `check` in this section. A call would abort the test.
unsafe extern "C" fn check_must_not_fire(_expr: *const c_char, _line: i32) {
    panic!("oracle check fired during table init");
}

/// Every fixture line whose first token is `kind`, split into fields.
fn rows(kind: &str) -> Vec<Vec<&'static str>> {
    FIXTURE
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|f| f.first() == Some(&kind))
        .collect()
}

fn hex_u32(word: &str) -> u32 {
    u32::from_str_radix(word, 16).unwrap_or_else(|e| panic!("hex word {word:?}: {e}"))
}

/// Checks one float table: rows `kind index bits` in index order.
fn check_f32_table(kind: &str, table: &[f32]) {
    let rows = rows(kind);
    assert_eq!(rows.len(), table.len(), "{kind} row count");
    for (pos, row) in rows.iter().enumerate() {
        let idx: usize = row[1].parse().expect("index");
        assert_eq!(idx, pos, "{kind} index order");
        assert_eq!(hex_u32(row[2]), table[idx].to_bits(), "{kind}[{idx}] bits");
    }
}

/// Source bits for one shadow case. Integer-derived, matching `LightTablesShadowCase`.
fn shadow_src(count: usize) -> Vec<u8> {
    (0..count)
        .map(|k| ((k * 37 + (k >> 3) * 91 + 19) & 0xff) as u8)
        .collect()
}

#[test]
fn tables_replay_matches_oracle() {
    // Exact per-kind counts; no unexpected kinds.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for line in FIXTURE.lines() {
        let kind = line.split_whitespace().next().unwrap_or("");
        assert!(
            EXPECTED.iter().any(|(k, _)| *k == kind),
            "unexpected record kind {kind:?}"
        );
        *counts.entry(kind).or_default() += 1;
    }
    for (kind, n) in EXPECTED {
        assert_eq!(
            counts.get(kind).copied().unwrap_or(0),
            *n,
            "record count for {kind}"
        );
    }

    // FLightManager::Init: light distance tables.
    let mut light_dist = Box::new([0f32; LIGHT_DIST_LEN]);
    let mut light_dist_inc = Box::new([0f32; LIGHT_DIST_LEN]);
    init_light_dist(&mut light_dist, &mut light_dist_inc);
    check_f32_table("lightdist", &light_dist[..]);
    check_f32_table("lightdistinc", &light_dist_inc[..]);

    // FLightManager::Init: fog and filter tables.
    let mut byte_fog = Box::new([0u8; BYTE_FOG_LEN]);
    init_byte_fog(&mut byte_fog);
    let byte_rows = rows("bytefog");
    assert_eq!(byte_rows.len(), BYTE_FOG_LEN, "bytefog row count");
    for (pos, row) in byte_rows.iter().enumerate() {
        let idx: usize = row[1].parse().expect("index");
        assert_eq!(idx, pos, "bytefog index order");
        let value: u8 = row[2].parse().expect("bytefog value");
        assert_eq!(value, byte_fog[idx], "bytefog[{idx}]");
    }

    let filter = init_filter_tab(check_must_not_fire);
    let filter_rows = rows("filter");
    assert_eq!(filter_rows.len(), FILTER_TAB_LEN * 4, "filter row count");
    for row in &filter_rows {
        let i: usize = row[1].parse().expect("filter i");
        let j: usize = row[2].parse().expect("filter j");
        assert_eq!(hex_u32(row[3]), filter[i][j], "filter[{i}][{j}]");
    }

    // FLightManager::ShadowMapGen: each case, in fixture order of its own records.
    for &(kind, case, u, v) in SHADOW_CASES {
        let words = u * v * 2;
        let expected: Vec<u32> = rows(kind)
            .iter()
            .filter(|r| r[1].parse::<u32>().expect("case") == case)
            .enumerate()
            .map(|(pos, r)| {
                assert_eq!(
                    r[2].parse::<usize>().expect("idx"),
                    pos,
                    "{kind} case {case} idx"
                );
                hex_u32(r[3])
            })
            .collect();
        assert_eq!(expected.len(), words, "{kind} case {case} word count");

        let src = shadow_src(u * v);
        let src_opt = if kind == "shadow" {
            Some(&src[..])
        } else {
            None
        };
        let mut dest = vec![0u32; words];
        shadow_map_gen(&filter, u, v, src_opt, &mut dest);
        assert_eq!(dest, expected, "{kind} case {case} words");
    }
}

#[test]
fn single_row_shadow_matches_full_original_write_extent() {
    let filter = init_filter_tab(check_must_not_fire);
    for (kind, case, u, has_src) in [
        ("shadow_single", 7, 1, true),
        ("shadow_single", 8, 3, true),
        ("shadow_single_null", 9, 3, false),
    ] {
        let expected: Vec<u32> = rows(kind)
            .iter()
            .filter(|row| row[1].parse::<u32>().expect("case") == case)
            .map(|row| hex_u32(row[3]))
            .collect();
        assert_eq!(expected.len(), u * 4 + 1);
        let src = shadow_src(u);
        let mut dest = vec![0x5a5a_5a5a; expected.len()];
        shadow_map_gen(&filter, u, 1, has_src.then_some(src.as_slice()), &mut dest);
        assert_eq!(
            dest, expected,
            "{kind} case {case} including boundary sentinel"
        );
    }
}
