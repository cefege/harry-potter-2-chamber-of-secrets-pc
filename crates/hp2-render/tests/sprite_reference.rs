//! Bit-exact check of `hp2_render::sprite` against the original
//! `FDynamicSprite::Setup` extent blocks (`tests/fixtures/sprite_reference.txt`,
//! 1500 cases per axis, verbatim copies compiled from `UnSprite.cpp`).

use hp2_render::sprite::{extent_x, extent_y};

const FIXTURE: &str = include_str!("fixtures/sprite_reference.txt");

fn f(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).unwrap())
}
fn i(s: &str) -> i32 {
    u32::from_str_radix(s, 16).unwrap() as i32
}

/// Returns `(inputs, outputs)` per record with the given tag.
fn records(tag: &str) -> Vec<(Vec<String>, Vec<String>)> {
    FIXTURE
        .lines()
        .filter(|l| l.starts_with(tag))
        .map(|l| {
            let (lhs, rhs) = l.split_once(" |").expect("record separator");
            let inp: Vec<String> = lhs.split_whitespace().skip(1).map(str::to_string).collect();
            let out: Vec<String> = rhs.split_whitespace().map(str::to_string).collect();
            (inp, out)
        })
        .collect()
}

#[test]
fn extent_x_matches_capture() {
    let recs = records("EXTX");
    assert_eq!(recs.len(), 1500);
    let mut visible = 0;
    for (inp, out) in &recs {
        let e = extent_x(f(&inp[0]), f(&inp[1]), f(&inp[2]), i(&inp[3]), i(&inp[4]));
        assert_eq!(
            e.visible as u8,
            u8::from_str_radix(&out[0], 16).unwrap(),
            "{inp:?}"
        );
        assert_eq!(e.lo, i(&out[1]), "{inp:?}");
        assert_eq!(e.hi, i(&out[2]), "{inp:?}");
        visible += e.visible as u32;
    }
    assert!(
        visible > 0 && visible < recs.len() as u32,
        "fixture must cover accept and reject"
    );
}

#[test]
fn extent_y_matches_capture() {
    let recs = records("EXTY");
    assert_eq!(recs.len(), 1500);
    let mut visible = 0;
    for (inp, out) in &recs {
        let e = extent_y(f(&inp[0]), f(&inp[1]), f(&inp[2]), i(&inp[3]), i(&inp[4]));
        assert_eq!(
            e.visible as u8,
            u8::from_str_radix(&out[0], 16).unwrap(),
            "{inp:?}"
        );
        assert_eq!(e.lo, i(&out[1]), "{inp:?}");
        assert_eq!(e.hi, i(&out[2]), "{inp:?}");
        visible += e.visible as u32;
    }
    assert!(
        visible > 0 && visible < recs.len() as u32,
        "fixture must cover accept and reject"
    );
}
