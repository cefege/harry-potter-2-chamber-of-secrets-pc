//! Replays the captured `FGlobalRandoms::Tick` schedule (original C++ compiled
//! standalone, `appFrand` replaced by the LCG below) and requires bit-identical
//! `RandomBases` / `Randoms` after every tick.

use hp2_render::random::{N_RANDS, tick};

const FIXTURE: &str = include_str!("fixtures/random_reference.txt");

/// Same LCG as `capture_frand` in the C++ capture harness. The harness was
/// built with clang's default `-ffp-contract=on`, so `x * 1.5f - 0.25f` is a
/// fused multiply-add there; `mul_add` reproduces that rounding exactly.
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.0 >> 8) as f32 * (1.0f32 / 16777216.0)).mul_add(1.5, -0.25)
    }
}

fn parse_bits(words: &str) -> Vec<u32> {
    words
        .split_whitespace()
        .map(|w| u32::from_str_radix(w, 16).expect("hex word"))
        .collect()
}

#[test]
fn tick_matches_original_cpp_capture_bit_for_bit() {
    let mut lines = FIXTURE.lines().filter(|l| !l.starts_with('#'));
    let count: usize = lines
        .next()
        .and_then(|l| l.strip_prefix("ticks "))
        .expect("ticks header")
        .parse()
        .expect("tick count");

    let mut lcg = Lcg(12345);
    let mut bases = [0.0f32; N_RANDS];
    let mut randoms = [0.0f32; N_RANDS];
    let mut deltas = [0.0f32; N_RANDS];
    let mut last_ticks = 0u32;

    for tick_index in 0..count {
        let t_line = lines.next().expect("T line");
        let time_bits = u32::from_str_radix(t_line.strip_prefix("T ").expect("T"), 16).unwrap();
        let time = f32::from_bits(time_bits);

        let b_bits = parse_bits(lines.next().expect("B line").strip_prefix('B').expect("B"));
        let r_bits = parse_bits(lines.next().expect("R line").strip_prefix('R').expect("R"));
        assert_eq!(b_bits.len(), N_RANDS);
        assert_eq!(r_bits.len(), N_RANDS);

        tick(
            &mut || lcg.next(),
            time,
            &mut bases,
            &mut randoms,
            &mut deltas,
            &mut last_ticks,
        );

        for i in 0..N_RANDS {
            assert_eq!(bases[i].to_bits(), b_bits[i], "tick {tick_index} base[{i}]");
            assert_eq!(
                randoms[i].to_bits(),
                r_bits[i],
                "tick {tick_index} random[{i}]"
            );
        }
    }
}

#[test]
fn clamp_bounds_are_inclusive_and_nan_goes_to_one() {
    // First tick (last_ticks == 0) takes the else branch: bases consume the
    // first N_RANDS frand calls, then each randoms[i] consumes one call.
    let mut bases = [0.0f32; N_RANDS];
    let mut randoms = [0.5f32; N_RANDS];
    let mut deltas = [0.0f32; N_RANDS];
    let mut last = 0u32;
    let mut k = 0usize;
    let mut frand = || {
        k += 1;
        match k.saturating_sub(N_RANDS) {
            1 => -0.5,
            2 => 1.0,
            3 => f32::NAN,
            4 => -0.0,
            5 => 0.25,
            _ => 0.5,
        }
    };
    tick(
        &mut frand,
        0.0,
        &mut bases,
        &mut randoms,
        &mut deltas,
        &mut last,
    );
    assert_eq!(randoms[0], 0.0);
    assert_eq!(randoms[1], 1.0);
    assert_eq!(randoms[2], 1.0);
    assert_eq!(randoms[3].to_bits(), (-0.0f32).to_bits());
    assert_eq!(randoms[4], 0.25);
    assert_eq!(last, 0);
}
