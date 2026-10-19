//! Replays the original-C++ oracle from `Build/gesture_fixture_harness/` (`UnGesture.cpp` at the
//! pinned ref: its external helpers, and its `execCompareGesture*` exec shells driven through a
//! script frame) against `hp2_engine::gesture`. Every result is compared by its f32 bit pattern.
//!
//! Fixtures:
//! - `gesture_compare.txt`: `cmp2` (`InternalGestureCompare2`, 400) and
//!   `pt` (`InternalPointCompare`, 300).
//! - `gesture_exec.txt`: `xpt` (`execCompareGesturePoint`, 300) and `xcg` (`execCompareGesture`,
//!   301, including one 1100-point clamp case). Target counts are at most 128.
//!
//! Hex fields are f32 bit patterns. Integers are decimal.

use hp2_abi::FVector;
use hp2_engine::gesture::{
    CMousePoint, compare_gesture, compare_gesture_point, internal_gesture_compare2,
    internal_point_compare,
};

const COMPARE_FIXTURE: &str = include_str!("fixtures/gesture_compare.txt");
const EXEC_FIXTURE: &str = include_str!("fixtures/gesture_exec.txt");

const CMP2_RECORDS: usize = 400;
const PT_RECORDS: usize = 300;
const XPT_RECORDS: usize = 300;
const XCG_RECORDS: usize = 301;

/// Token cursor over one fixture line.
struct Tokens<'a> {
    it: std::str::SplitWhitespace<'a>,
    line: &'a str,
}

impl<'a> Tokens<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            it: line.split_whitespace(),
            line,
        }
    }

    fn next_token(&mut self) -> &'a str {
        self.it
            .next()
            .unwrap_or_else(|| panic!("truncated line: {}", self.line))
    }

    fn tag(&mut self) -> &'a str {
        self.next_token()
    }

    fn hex(&mut self) -> f32 {
        let word = self.next_token();
        let bits = u32::from_str_radix(word, 16).expect("hex word");
        f32::from_bits(bits)
    }

    fn int<T: std::str::FromStr>(&mut self) -> T
    where
        T::Err: std::fmt::Debug,
    {
        self.next_token().parse().expect("decimal field")
    }

    fn point(&mut self) -> CMousePoint {
        CMousePoint {
            x_pos: self.hex(),
            y_pos: self.hex(),
        }
    }

    fn vec(&mut self) -> FVector {
        FVector {
            x: self.hex(),
            y: self.hex(),
            z: self.hex(),
        }
    }

    fn done(&mut self) {
        assert!(self.it.next().is_none(), "trailing tokens: {}", self.line);
    }
}

fn assert_bits(line: &str, actual: f32, expected: f32) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "f32 bits differ: {line}"
    );
}

#[test]
fn compare_helpers_are_bit_exact() {
    let (mut cmp2, mut pt) = (0usize, 0usize);
    for line in COMPARE_FIXTURE.lines() {
        let mut t = Tokens::new(line);
        match t.tag() {
            "cmp2" => {
                let accuracy = t.hex();
                let nm: usize = t.int();
                let mouse: Vec<CMousePoint> = (0..nm).map(|_| t.point()).collect();
                let nt: usize = t.int();
                let target: Vec<CMousePoint> = (0..nt).map(|_| t.point()).collect();
                let ret = t.hex();
                t.done();
                assert_bits(
                    line,
                    internal_gesture_compare2(&mouse, &target, accuracy),
                    ret,
                );
                cmp2 += 1;
            }
            "pt" => {
                let point = t.point();
                let nt: usize = t.int();
                let target: Vec<CMousePoint> = (0..nt).map(|_| t.point()).collect();
                let ret = t.hex();
                t.done();
                assert_bits(line, internal_point_compare(point, &target), ret);
                pt += 1;
            }
            other => panic!("unknown compare tag {other}: {line}"),
        }
    }
    assert_eq!(cmp2, CMP2_RECORDS, "cmp2 record count");
    assert_eq!(pt, PT_RECORDS, "pt record count");
}

#[test]
fn exec_shells_are_bit_exact() {
    let (mut xpt, mut xcg) = (0usize, 0usize);
    for line in EXEC_FIXTURE.lines() {
        let mut t = Tokens::new(line);
        match t.tag() {
            "xpt" => {
                // The original's execCompareGesturePoint consumes fAccuracy but never reads it, so
                // the port takes no accuracy argument.
                let _accuracy = t.hex();
                let nt: usize = t.int();
                let targets: Vec<FVector> = (0..nt).map(|_| t.vec()).collect();
                let point = t.vec();
                let ret = t.hex();
                t.done();
                assert_bits(line, compare_gesture_point(&targets, point), ret);
                xpt += 1;
            }
            "xcg" => {
                let accuracy = t.hex();
                let nt: usize = t.int();
                let targets: Vec<FVector> = (0..nt).map(|_| t.vec()).collect();
                let nin: usize = t.int();
                let inputs: Vec<FVector> = (0..nin).map(|_| t.vec()).collect();
                let ret = t.hex();
                t.done();
                assert_bits(line, compare_gesture(&targets, &inputs, accuracy), ret);
                xcg += 1;
            }
            other => panic!("unknown exec tag {other}: {line}"),
        }
    }
    assert_eq!(xpt, XPT_RECORDS, "xpt record count");
    assert_eq!(xcg, XCG_RECORDS, "xcg record count");
}
