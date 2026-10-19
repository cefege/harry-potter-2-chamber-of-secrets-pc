//! `AActor::fixedTurn` replayed against records captured from the original
//! `UnPhysic.cpp` (-O3, compiled from 58a73d4, `AActor` bitfields set in the
//! harness). Record: `current desired deltaRate fixedDir rotateToDesired result`.

use hp2_engine::physic::fixed_turn;

#[test]
fn fixed_turn_matches_original() {
    let text = include_str!("fixtures/physic_fixed_turn.txt");
    let mut count = 0;
    for line in text.lines() {
        let f: Vec<i32> = line
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        assert_eq!(f.len(), 6, "bad record: {line}");
        let got = fixed_turn(f[0], f[1], f[2], f[3] != 0, f[4] != 0);
        assert_eq!(
            got, f[5],
            "fixedTurn({}, {}, {}, fixed={}, rotate={})",
            f[0], f[1], f[2], f[3], f[4]
        );
        count += 1;
    }
    assert_eq!(count, 12168);
}
