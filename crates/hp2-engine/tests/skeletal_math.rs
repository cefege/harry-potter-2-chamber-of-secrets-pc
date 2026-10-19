//! Replays the `skeletal_math` and `skeletal_trig` fixture families against
//! `hp2_engine::skeletal::{AnimVec, Quat}`.
//!
//! Both fixtures are generated from the ORIGINAL C++ by
//! `Build/skeletal_fixture_harness/gen_skeletal.sh`:
//!
//! - `skeletal_math.txt`: one line per case, `Case V3 Inv AX AY AZ Q4 BX BY BZ R4 S W3`.
//!   `V3`, `Inv`, `Q4` and `S` are inputs; the rest are the original
//!   `FAnimVec::Init`, `FAnimVec(FQuat)`, `FAnimVec::Quat` and `FAnimVec::Vector`
//!   results. Floats are `0x%08x` bit patterns, integers are decimal.
//! - `skeletal_trig.txt`: `i 0x%08x` for every `FGlobalMath::TrigFLOAT[i]`.
//!   The original fills it with `appSin((FLOAT)i * 2.0f * PI / (FLOAT)NUM_ANGLES)`
//!   (UnMath.cpp:53), which is recomputed here with the same float/double steps.

use hp2_abi::FVector;
use hp2_engine::skeletal::{AnimVec, Quat};

const MATH: &str = include_str!("fixtures/skeletal_math.txt");
const TRIG: &str = include_str!("fixtures/skeletal_trig.txt");

/// Number of lines in `skeletal_math.txt` (`run_math` cases).
const MATH_CASES: usize = 400;
/// `FGlobalMath::NUM_ANGLES` (UnMath.h:1507); one line per table entry.
const TRIG_ENTRIES: usize = 16384;

/// Whitespace token reader over one fixture line.
struct Toks<'a> {
    it: std::str::SplitWhitespace<'a>,
    line: &'a str,
}

impl<'a> Toks<'a> {
    fn new(line: &'a str) -> Self {
        Toks {
            it: line.split_whitespace(),
            line,
        }
    }

    fn word(&mut self) -> &'a str {
        self.it
            .next()
            .unwrap_or_else(|| panic!("fixture line too short: {}", self.line))
    }

    fn int(&mut self) -> i64 {
        self.word()
            .parse()
            .unwrap_or_else(|e| panic!("bad int in {}: {e}", self.line))
    }

    fn bits(&mut self) -> f32 {
        let w = self.word();
        let hex = w
            .strip_prefix("0x")
            .unwrap_or_else(|| panic!("expected hex float bits, got {w}"));
        f32::from_bits(
            u32::from_str_radix(hex, 16).unwrap_or_else(|e| panic!("bad float bits {w}: {e}")),
        )
    }

    fn vec(&mut self) -> FVector {
        FVector {
            x: self.bits(),
            y: self.bits(),
            z: self.bits(),
        }
    }

    fn quat(&mut self) -> Quat {
        Quat {
            x: self.bits(),
            y: self.bits(),
            z: self.bits(),
            w: self.bits(),
        }
    }

    fn end(&mut self) {
        assert!(
            self.it.next().is_none(),
            "fixture line has trailing tokens: {}",
            self.line
        );
    }
}

fn assert_bits(case: i64, what: &str, got: f32, want: f32) {
    assert_eq!(
        got.to_bits(),
        want.to_bits(),
        "case {case}: {what} got {got:e} (0x{:08x}) want {want:e} (0x{:08x})",
        got.to_bits(),
        want.to_bits()
    );
}

fn assert_vec(case: i64, what: &str, got: FVector, want: FVector) {
    assert_bits(case, &format!("{what}.x"), got.x, want.x);
    assert_bits(case, &format!("{what}.y"), got.y, want.y);
    assert_bits(case, &format!("{what}.z"), got.z, want.z);
}

fn assert_quat(case: i64, what: &str, got: Quat, want: Quat) {
    assert_bits(case, &format!("{what}.x"), got.x, want.x);
    assert_bits(case, &format!("{what}.y"), got.y, want.y);
    assert_bits(case, &format!("{what}.z"), got.z, want.z);
    assert_bits(case, &format!("{what}.w"), got.w, want.w);
}

#[test]
fn math_fixture_replays_bit_exact() {
    let mut cases = 0usize;
    for line in MATH.lines() {
        let mut t = Toks::new(line);
        let case = t.int();

        // Inputs.
        let v = t.vec();
        let inv = t.bits();
        // FAnimVec::Init(V, Inv).
        let ax = t.int();
        let ay = t.int();
        let az = t.int();
        let q = t.quat();
        // FAnimVec(FQuat).
        let bx = t.int();
        let by = t.int();
        let bz = t.int();
        // FAnimVec::Quat().
        let r = t.quat();
        let s = t.bits();
        // FAnimVec::Vector(S).
        let w = t.vec();
        t.end();

        let a = AnimVec::from_vector(v, inv);
        assert_eq!(i64::from(a.x), ax, "case {case}: A.X");
        assert_eq!(i64::from(a.y), ay, "case {case}: A.Y");
        assert_eq!(i64::from(a.z), az, "case {case}: A.Z");

        let b = AnimVec::from_quat(q);
        assert_eq!(i64::from(b.x), bx, "case {case}: B.X");
        assert_eq!(i64::from(b.y), by, "case {case}: B.Y");
        assert_eq!(i64::from(b.z), bz, "case {case}: B.Z");

        assert_quat(case, "R", b.quat(), r);
        assert_vec(case, "W", b.vector(s), w);
        cases += 1;
    }
    assert_eq!(cases, MATH_CASES, "skeletal_math.txt line count");
}

#[test]
fn trig_table_matches_original_formula() {
    let mut entries = 0usize;
    for (want_index, line) in TRIG.lines().enumerate() {
        let mut t = Toks::new(line);
        let i = t.int();
        let want = t.bits();
        t.end();
        assert_eq!(i, want_index as i64, "trig fixture index order");

        // UnMath.cpp:53: TrigFLOAT[i] = appSin((FLOAT)i * 2.0f * PI / (FLOAT)NUM_ANGLES).
        // `appSin` is the double libm `sin` (UnAnsi.cpp); the float steps stay in f32.
        // UnMath.h:30 `#define PI (3.1415926535897932f)`; the nearest f32 is std's PI.
        let pi = std::f32::consts::PI;
        let angle = (i as f32) * 2.0f32 * pi / (TRIG_ENTRIES as f32);
        let got = (f64::from(angle)).sin() as f32;
        assert_bits(i, "TrigFLOAT", got, want);
        entries += 1;
    }
    assert_eq!(entries, TRIG_ENTRIES, "skeletal_trig.txt line count");
}
