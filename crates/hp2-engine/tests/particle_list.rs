//! Replays the original-C++ oracle from `Build/particle_fixture_harness/` (`UParticleListPriv`
//! from `UnParticleList.cpp` at the pinned ref, a `std::list<UParticle>` with an iterator cursor)
//! against `hp2_engine::particle::ParticleList`.
//!
//! `fixtures/particle_list.txt`: 96 scenarios of 60 operations each, one line per operation, in
//! scenario order. Each scenario starts with an empty list. Every operation prints exactly one
//! line:
//!
//! - `add <id> <width hex> <length hex> <px> <py> <pz>` (inputs, hex float bits)
//! - `start => <id>|null` and `next => <id>|null`
//! - `get <id> => <id>|null`
//! - `remove <id> => <size>` (a missing id removes a dummy that is not in the list)
//! - `remove_current => <size>`, `remove_from_front => <size>`, `remove_all => <size>`
//! - `size => <n>`
//! - `update_box => <min xyz> <max xyz> <is_valid>` (hex float bits and a 0/1 flag)

use std::collections::BTreeMap;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{FBox, UParticle};
use hp2_engine::particle::ParticleList;

const LIST_FIXTURE: &str = include_str!("fixtures/particle_list.txt");

const SCENARIOS: usize = 96;
const OPS_PER_SCENARIO: usize = 60;

fn u(hex: &str) -> u32 {
    u32::from_str_radix(hex, 16).expect("hex word")
}

fn f(hex: &str) -> f32 {
    f32::from_bits(u(hex))
}

fn parse_i32(s: &str) -> i32 {
    s.parse().expect("decimal id")
}

fn parse_size(s: &str) -> usize {
    s.parse().expect("decimal size")
}

fn expected_id(s: &str) -> i32 {
    if s == "null" { -1 } else { parse_i32(s) }
}

fn observed_id(p: *mut UParticle) -> i32 {
    if p.is_null() {
        -1
    } else {
        // SAFETY: non-null pointers come from `ParticleList`, which owns boxed particles that are
        // alive for the scenario.
        unsafe { (*p).id }
    }
}

fn assert_bits(line: &str, actual: f32, expected_hex: &str) {
    assert_eq!(actual.to_bits(), u(expected_hex), "float bits: {line}");
}

/// Replays one operation and returns its kind for the per-kind record count.
fn replay(line: &str, list: &mut ParticleList, dummy: &UParticle) -> &'static str {
    let t: Vec<&str> = line.split_whitespace().collect();
    match t[0] {
        "add" => {
            assert_eq!(t.len(), 7, "{line}");
            let mut p: UParticle = unsafe { std::mem::zeroed() };
            p.id = parse_i32(t[1]);
            p.width = f(t[2]);
            p.length = f(t[3]);
            p.position = FVector {
                x: f(t[4]),
                y: f(t[5]),
                z: f(t[6]),
            };
            list.add(&p);
            "add"
        }
        "start" => {
            assert_eq!(t.len(), 3, "{line}");
            assert_eq!(observed_id(list.start()), expected_id(t[2]), "{line}");
            "start"
        }
        "next" => {
            assert_eq!(t.len(), 3, "{line}");
            assert_eq!(
                observed_id(list.next_particle()),
                expected_id(t[2]),
                "{line}"
            );
            "next"
        }
        "get" => {
            assert_eq!(t.len(), 4, "{line}");
            assert_eq!(
                observed_id(list.get_particle(parse_i32(t[1]))),
                expected_id(t[3]),
                "{line}"
            );
            "get"
        }
        "remove" => {
            assert_eq!(t.len(), 4, "{line}");
            let found = list.get_particle(parse_i32(t[1]));
            let target = if found.is_null() {
                ptr::from_ref(dummy)
            } else {
                found.cast_const()
            };
            list.remove(target);
            assert_eq!(list.size(), parse_size(t[3]), "{line}");
            "remove"
        }
        "remove_current" => {
            assert_eq!(t.len(), 3, "{line}");
            list.remove_current();
            assert_eq!(list.size(), parse_size(t[2]), "{line}");
            "remove_current"
        }
        "remove_from_front" => {
            assert_eq!(t.len(), 3, "{line}");
            list.remove_from_front();
            assert_eq!(list.size(), parse_size(t[2]), "{line}");
            "remove_from_front"
        }
        "remove_all" => {
            assert_eq!(t.len(), 3, "{line}");
            list.remove_all();
            assert_eq!(list.size(), parse_size(t[2]), "{line}");
            "remove_all"
        }
        "size" => {
            assert_eq!(t.len(), 3, "{line}");
            assert_eq!(list.size(), parse_size(t[2]), "{line}");
            "size"
        }
        "update_box" => {
            assert_eq!(t.len(), 9, "{line}");
            let mut b: FBox = unsafe { std::mem::zeroed() };
            list.update_box(&mut b);
            assert_bits(line, b.min.x, t[2]);
            assert_bits(line, b.min.y, t[3]);
            assert_bits(line, b.min.z, t[4]);
            assert_bits(line, b.max.x, t[5]);
            assert_bits(line, b.max.y, t[6]);
            assert_bits(line, b.max.z, t[7]);
            assert_eq!(u32::from(b.is_valid), u(t[8]), "{line}");
            "update_box"
        }
        other => panic!("unknown list operation {other}: {line}"),
    }
}

#[test]
fn particle_list_fixtures_bit_exact() {
    let lines: Vec<&str> = LIST_FIXTURE.lines().collect();
    assert_eq!(
        lines.len(),
        SCENARIOS * OPS_PER_SCENARIO,
        "list record count"
    );

    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for scenario in lines.chunks(OPS_PER_SCENARIO) {
        let mut list = ParticleList::new();
        let dummy: UParticle = unsafe { std::mem::zeroed() };
        for line in scenario {
            *counts.entry(replay(line, &mut list, &dummy)).or_default() += 1;
        }
    }

    let expected = [
        ("add", 2426),
        ("get", 449),
        ("next", 815),
        ("remove", 444),
        ("remove_all", 103),
        ("remove_current", 233),
        ("remove_from_front", 212),
        ("size", 318),
        ("start", 456),
        ("update_box", 304),
    ];
    assert_eq!(counts.len(), expected.len(), "operation kinds: {counts:?}");
    for (kind, n) in expected {
        assert_eq!(
            counts.get(kind).copied().unwrap_or(0),
            n,
            "count for {kind}"
        );
    }
    assert_eq!(
        counts.values().sum::<usize>(),
        SCENARIOS * OPS_PER_SCENARIO,
        "total list records"
    );
}
