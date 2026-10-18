//! Bit-exact replay of `UCanvas::DrawTile` clip and UV math. The fixture
//! `tests/fixtures/canvas_draw_tile_clip.txt` was recorded by compiling the
//! clip block (`UnCanvas.cpp` lines 88-104) and the UV lines (111-112) from
//! commit 40bde73, verbatim, in a standalone harness. Floats are IEEE bit
//! patterns, so any change in rounding or operation order fails here.

use hp2_engine::canvas::{Hp2CanvasTileRect, clip_tile, uv_factor};

const FIXTURE: &str = include_str!("fixtures/canvas_draw_tile_clip.txt");

fn f(bits: &str) -> f32 {
    f32::from_bits(bits.parse::<u32>().expect("u32 bits"))
}

fn i(s: &str) -> i32 {
    s.parse::<i32>().expect("i32")
}

#[test]
fn draw_tile_clip_matches_original() {
    let (mut clip_cases, mut rejected, mut uv_cases) = (0usize, 0usize, 0usize);
    for line in FIXTURE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t[0] {
            "C" => {
                clip_cases += 1;
                let (fx, fy) = (f(t[1]), f(t[2]));
                let mut r = Hp2CanvasTileRect {
                    x: f(t[3]),
                    y: f(t[4]),
                    xl: f(t[5]),
                    yl: f(t[6]),
                    u: f(t[7]),
                    v: f(t[8]),
                    ul: f(t[9]),
                    vl: f(t[10]),
                };
                let kept = clip_tile(fx, fy, &mut r);
                if t[11] == "R" {
                    rejected += 1;
                    assert!(!kept, "expected reject: {line}");
                    continue;
                }
                assert!(kept, "expected keep: {line}");
                assert_eq!(t[11], "K", "{line}");
                let want = [
                    f(t[12]),
                    f(t[13]),
                    f(t[14]),
                    f(t[15]),
                    f(t[16]),
                    f(t[17]),
                    f(t[18]),
                    f(t[19]),
                ];
                let got = [r.x, r.y, r.xl, r.yl, r.u, r.v, r.ul, r.vl];
                for k in 0..8 {
                    assert_eq!(got[k].to_bits(), want[k].to_bits(), "field {k}: {line}");
                }
            }
            "V" => {
                uv_cases += 1;
                let (uscale, usize_, tex_usize) = (f(t[1]), i(t[2]), i(t[3]));
                let (vscale, vsize, tex_vsize) = (f(t[4]), i(t[5]), i(t[6]));
                let inp: Vec<f32> = (7..11).map(|k| f(t[k])).collect();
                let want: Vec<f32> = (11..15).map(|k| f(t[k])).collect();
                let uf = uv_factor(uscale, usize_, tex_usize);
                let vf = uv_factor(vscale, vsize, tex_vsize);
                let got = [inp[0] * uf, inp[1] * uf, inp[2] * vf, inp[3] * vf];
                for k in 0..4 {
                    assert_eq!(got[k].to_bits(), want[k].to_bits(), "uv field {k}: {line}");
                }
            }
            other => panic!("unknown record {other}"),
        }
    }
    assert!(clip_cases >= 1500 && rejected > 0 && rejected < clip_cases);
    assert_eq!(uv_cases, 300);
}
