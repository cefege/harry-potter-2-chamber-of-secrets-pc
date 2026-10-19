//! Replays `fixtures/tex_fixtures.txt` (generated from the ORIGINAL UnTex.o by
//! `Build/tex_fixture_harness/gen_tex.sh`) against `hp2_engine::tex`, bit-exact,
//! with per-section case-count asserts.

use hp2_abi::FColor;
use hp2_engine::tex::{
    MipLevel, NUM_PAL_COLORS, TEXF_P8, TEXF_RGBA7, best_match, cache_max_color, color_range,
    downsample_p8, fget_hsv, fix_palette, mip_zero, prime_steps, tick_plan,
};
use std::collections::HashMap;

const FIXTURE: &str = include_str!("fixtures/tex_fixtures.txt");

fn hex_u32(s: &str) -> u32 {
    u32::from_str_radix(s, 16).expect("hex u32")
}

fn color(v: u32) -> FColor {
    FColor {
        r: v as u8,
        g: (v >> 8) as u8,
        b: (v >> 16) as u8,
        a: (v >> 24) as u8,
    }
}

fn parse_palette(fields: &[&str]) -> Vec<FColor> {
    assert_eq!(fields.len(), NUM_PAL_COLORS);
    fields.iter().map(|s| color(hex_u32(s))).collect()
}

fn parse_bytes(fields: &[&str]) -> Vec<u8> {
    fields
        .iter()
        .map(|s| u8::from_str_radix(s, 16).expect("hex byte"))
        .collect()
}

#[test]
fn tex_fixtures_bit_exact() {
    let mut hsv = 0usize;
    let mut bm = 0usize;
    let mut fp = 0usize;
    let mut cmc = 0usize;
    let mut mip = 0usize;
    let mut cr = 0usize;
    let mut tick = 0usize;

    let mut bm_pal: HashMap<String, Vec<FColor>> = HashMap::new();
    let mut fp_in: HashMap<String, Vec<FColor>> = HashMap::new();
    let mut cmc_in: HashMap<String, (usize, u32, u32, Vec<u8>)> = HashMap::new();
    let mut mip_pal: HashMap<String, Vec<FColor>> = HashMap::new();
    let mut mip_in: HashMap<String, (u32, u32, bool, Vec<u8>)> = HashMap::new();
    let mut mip_out: HashMap<String, Vec<(i32, i32, Vec<u8>)>> = HashMap::new();
    let mut mip_zero_expected: HashMap<String, u32> = HashMap::new();
    let mut cr_expected: HashMap<String, u32> = HashMap::new();
    let mut tick_cfg: HashMap<String, (f32, f32, u8)> = HashMap::new();
    let mut tick_state: HashMap<String, (f32, u8, usize)> = HashMap::new();
    // Mip levels are recorded after their mip line; collect them first.
    for line in FIXTURE.lines().filter(|l| l.starts_with("mipdata ")) {
        let f: Vec<&str> = line.split(' ').collect();
        let su: i32 = f[3].parse().unwrap();
        let sv: i32 = f[4].parse().unwrap();
        mip_out
            .entry(f[1].to_string())
            .or_default()
            .push((su, sv, parse_bytes(&f[5..])));
    }

    for line in FIXTURE.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        match f[0] {
            "hsv" => {
                let p = fget_hsv(
                    f[1].parse().unwrap(),
                    f[2].parse().unwrap(),
                    f[3].parse().unwrap(),
                );
                let want = [f[4], f[5], f[6], f[7]];
                for k in 0..4 {
                    assert_eq!(p[k].to_bits(), hex_u32(want[k]), "hsv {line}");
                }
                hsv += 1;
            }
            "bmpal" => {
                bm_pal.insert(f[1].to_string(), parse_palette(&f[2..]));
            }
            "bm" => {
                let pal = &bm_pal[f[1]];
                let c = color(hex_u32(f[2]));
                let first: usize = f[3].parse().unwrap();
                let got = best_match(pal, c, first);
                assert_eq!(got, f[4].parse::<u8>().unwrap(), "bm {line}");
                bm += 1;
            }
            "fpin" => {
                fp_in.insert(f[1].to_string(), parse_palette(&f[2..]));
            }
            "fp" => {
                let mut pal = fp_in[f[1]].clone();
                fix_palette(&mut pal);
                let want = parse_palette(&f[2..]);
                assert_eq!(pal, want, "fp {}", f[1]);
                fp += 1;
            }
            "cmcin" => {
                let usize_: usize = f[2].parse().unwrap();
                let uclamp: u32 = f[3].parse().unwrap();
                let vclamp: u32 = f[4].parse().unwrap();
                cmc_in.insert(
                    f[1].to_string(),
                    (usize_, uclamp, vclamp, parse_bytes(&f[5..])),
                );
            }
            "cmc" => {
                let (usize_, uclamp, vclamp, data) = &cmc_in[f[1]];
                let max = FColor {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                    a: 0xff,
                };
                let got = cache_max_color(
                    TEXF_RGBA7,
                    max,
                    data,
                    *usize_ as i32,
                    *uclamp as i32,
                    *vclamp as i32,
                );
                assert_eq!(
                    (got.r, got.g, got.b, got.a),
                    {
                        let v = color(hex_u32(f[2]));
                        (v.r, v.g, v.b, v.a)
                    },
                    "cmc {}",
                    f[1]
                );
                cmc += 1;
            }
            "mippal" => {
                mip_pal.insert(f[1].to_string(), parse_palette(&f[2..]));
            }
            "mipin" => {
                let ubits: u32 = f[2].parse().unwrap();
                let vbits: u32 = f[3].parse().unwrap();
                let masked = f[4] == "1";
                mip_in.insert(
                    f[1].to_string(),
                    (ubits, vbits, masked, parse_bytes(&f[5..])),
                );
            }
            "mip" => {
                // mip <id> <count> <mipzero>
                mip_zero_expected.insert(f[1].to_string(), hex_u32(f[3]));
                let _count: usize = f[2].parse().unwrap();
                let (ubits, vbits, masked, mip0) = &mip_in[f[1]];
                let pal = &mip_pal[f[1]];
                let zero = mip_zero(pal, mip0, true);
                assert_eq!(color_u32(zero), hex_u32(f[3]), "mip zero {}", f[1]);
                // Build level buffers from the recorded sizes, level 0 = mip0 input.
                let outs = mip_out.get(f[1]).cloned().unwrap_or_default();
                let mut bufs: Vec<(i32, i32, Vec<u8>)> = outs
                    .iter()
                    .map(|(su, sv, d)| (*su, *sv, vec![0u8; d.len()]))
                    .collect();
                assert!(!bufs.is_empty(), "no mipdata for {}", f[1]);
                assert_eq!(bufs[0].2.len(), mip0.len(), "mip0 size {}", f[1]);
                bufs[0].2.copy_from_slice(mip0);
                let _ = (ubits, vbits);
                let mut levels: Vec<MipLevel<'_>> = bufs
                    .iter_mut()
                    .map(|(su, sv, d)| MipLevel {
                        data: d.as_mut_slice(),
                        size_u: *su,
                        size_v: *sv,
                    })
                    .collect();
                downsample_p8(pal, *masked, &mut levels);
                for (k, (lvl, (_, _, want))) in levels.iter().zip(outs.iter()).enumerate() {
                    assert_eq!(lvl.data, want.as_slice(), "mip {} level {}", f[1], k);
                }
                mip += 1;
            }
            // Collected by the prescan above.
            "mipdata" => {}
            "cr" => {
                cr_expected.insert(f[1].to_string(), hex_u32(f[2]));
                let pal = &mip_pal[f[1]];
                let outs = &mip_out[f[1]];
                let refs: Vec<&[u8]> = outs.iter().map(|(_, _, d)| d.as_slice()).collect();
                let got = color_range(Some(pal), &refs);
                assert_eq!(color_u32(got), hex_u32(f[2]), "cr {}", f[1]);
                cr += 1;
            }
            "tickcfg" => {
                let max = f32::from_bits(hex_u32(f[2]));
                let min = f32::from_bits(hex_u32(f[3]));
                let pc: u8 = f[4].parse().unwrap();
                tick_cfg.insert(f[1].to_string(), (max, min, pc));
                tick_state.insert(f[1].to_string(), (0.0, 0, 0));
            }
            "tick" => {
                let seq = f[1];
                let (max, min, prime_count) = tick_cfg[seq];
                let (mut acc, prime_cur, idx) = tick_state[seq];
                let d = f32::from_bits(hex_u32(f[3]));
                // Tick(): Prime() first, then the frame-rate plan.
                let steps = prime_steps(prime_cur, prime_count);
                let new_prime = prime_cur + steps as u8;
                let ticks = tick_plan(max, min, &mut acc, d);
                let new_idx = (idx + (steps + ticks) as usize) % 1000;
                assert_eq!(acc.to_bits(), hex_u32(f[4]), "tick acc {line}", line = line);
                assert_eq!(
                    new_idx,
                    f[5].parse::<usize>().unwrap(),
                    "tick idx {line}",
                    line = line
                );
                assert_eq!(
                    new_prime,
                    f[6].parse::<u8>().unwrap(),
                    "tick prime {line}",
                    line = line
                );
                tick_state.insert(seq.to_string(), (acc, new_prime, new_idx));
                tick += 1;
            }
            other => panic!("unknown fixture record {other}"),
        }
    }

    assert_eq!(hsv, 600, "hsv cases");
    assert_eq!(bm, 512, "bm cases");
    assert_eq!(fp, 6, "fixpalette cases");
    assert_eq!(cmc, 200, "cache max cases");
    assert_eq!(mip, 40, "mip cases");
    assert_eq!(cr, 40, "color range cases");
    assert_eq!(tick, 240, "tick cases");
    let _ = (mip_zero_expected, cr_expected);
}

fn color_u32(c: FColor) -> u32 {
    u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16) | (u32::from(c.a) << 24)
}

#[test]
fn tex_fixture_formats_are_p8_and_rgba7() {
    assert_eq!(TEXF_P8, 0x00);
    assert_eq!(TEXF_RGBA7, 0x01);
}
