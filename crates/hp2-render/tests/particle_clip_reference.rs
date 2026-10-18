//! Bit-exact check of `hp2_render::particle_clip` against outputs captured from
//! the original (51a4782) `Render/Src/UnParticleRn.cpp` `ClipPolygon` and
//! `ComputeClipCodes` (`tests/fixtures/particle_clip_reference.txt`: 800 clip
//! records, 600 code records).
//!
//! Each CLIP record gives `count allocs` followed by the output list. A new
//! crossing vertex is `new<k>`: the k-th `New<FTransTexture>(GMem)` call of that
//! record. The Rust side hands out pool slot k on the k-th call, so the test
//! pins both allocation order and the output pointer identity.
//!
//! Captured on arm64 with `-ffp-contract=on`, so the whole file is aarch64-only.
#![cfg(target_arch = "aarch64")]

use std::cell::Cell;

use hp2_abi::{FPlane, FVector};
use hp2_render::particle::{FTransTexture, FTransform, ParticleFrame};
use hp2_render::particle_clip::{clip_polygon, compute_clip_codes};

const FIXTURE: &str = include_str!("fixtures/particle_clip_reference.txt");
const POOL: usize = 256;

fn fbits(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).unwrap())
}

fn ubits(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn frame(t: &[&str]) -> ParticleFrame {
    ParticleFrame {
        proj_z: fbits(t[0]),
        rproj_z: fbits(t[1]),
        fx15: fbits(t[2]),
        fy15: fbits(t[3]),
        prjxm: fbits(t[4]),
        prjxp: fbits(t[5]),
        prjym: fbits(t[6]),
        prjyp: fbits(t[7]),
    }
}

fn blank() -> FTransTexture {
    FTransTexture {
        transform: FTransform {
            point: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            flags: 0,
            screen_x: 0.0,
            screen_y: 0.0,
            int_y: 0,
            rz: 0.0,
        },
        normal: FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        light: FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        fog: FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        u: 0.0,
        v: 0.0,
    }
}

/// Input vertex from the 9-float record layout: point xyz, u v, light xyzw.
fn vert9(s: &[&str]) -> FTransTexture {
    let mut v = blank();
    v.transform.point = FVector {
        x: fbits(s[0]),
        y: fbits(s[1]),
        z: fbits(s[2]),
    };
    v.u = fbits(s[3]);
    v.v = fbits(s[4]);
    v.light = FPlane {
        x: fbits(s[5]),
        y: fbits(s[6]),
        z: fbits(s[7]),
        w: fbits(s[8]),
    };
    v
}

#[test]
fn clip_polygon_matches_original_cpp() {
    let mut records = 0;
    let mut nonzero = 0;
    let mut with_allocs = 0;
    for (line_no, line) in FIXTURE
        .lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("CLIP "))
    {
        records += 1;
        let (lhs, rhs) = line.split_once(" |").expect("record separator");
        let t: Vec<&str> = lhs.split_whitespace().collect();
        // t[0] = "CLIP", t[1..9] = frame, t[9] = code, t[10] = n, then 9 floats per input.
        let fr = frame(&t[1..9]);
        let code = u8::from_str_radix(t[9], 16).unwrap();
        let n = ubits(t[10]) as usize;

        let mut inputs: Vec<FTransTexture> = (0..n)
            .map(|k| vert9(&t[11 + 9 * k..11 + 9 * k + 9]))
            .collect();
        let in_ptrs: Vec<*mut FTransTexture> = inputs.iter_mut().map(|v| v as *mut _).collect();
        let mut pts = [std::ptr::null_mut::<FTransTexture>(); 64];
        pts[..n].copy_from_slice(&in_ptrs);

        // rhs = "count allocs out..."; out tokens are `in<k>` or `new<k>` (+14 fields).
        let out_toks: Vec<&str> = rhs.split_whitespace().collect();
        let want_count = ubits(out_toks[0]) as i32;
        let want_allocs = ubits(out_toks[1]) as usize;

        let mut pool: Vec<FTransTexture> = (0..POOL).map(|_| blank()).collect();
        let base = pool.as_mut_ptr();
        let next = Cell::new(0usize);
        let mut alloc = || {
            let k = next.get();
            assert!(k < POOL, "record {line_no}: pool exhausted");
            next.set(k + 1);
            unsafe { base.add(k) }
        };
        let count = unsafe { clip_polygon(&fr, pts.as_mut_ptr(), n as i32, code, &mut alloc) };
        assert_eq!(
            next.get(),
            want_allocs,
            "record {line_no}: GMem allocation count"
        );
        assert_eq!(count, want_count, "record {line_no}: count");
        if want_allocs > 0 {
            with_allocs += 1;
        }
        if count == 0 {
            continue;
        }
        nonzero += 1;

        let mut cursor = 2usize;
        #[allow(clippy::needless_range_loop)] // `p` also labels the assertion messages
        for p in 0..count as usize {
            let tok = out_toks[cursor];
            cursor += 1;
            if let Some(k) = tok.strip_prefix("in") {
                let k: usize = k.parse().unwrap();
                assert_eq!(
                    pts[p], in_ptrs[k],
                    "record {line_no}: out[{p}] should be input {k}"
                );
                continue;
            }
            let k: usize = tok
                .strip_prefix("new")
                .unwrap_or_else(|| panic!("record {line_no}: out[{p}] token {tok}"))
                .parse()
                .unwrap();
            assert!(k < want_allocs, "record {line_no}: out[{p}] allocation {k}");
            assert_eq!(
                pts[p],
                unsafe { base.add(k) },
                "record {line_no}: out[{p}] should be allocation {k}"
            );
            let v = unsafe { &*pts[p] };
            let want: Vec<&str> = out_toks[cursor..cursor + 14].to_vec();
            cursor += 14;
            let got = [
                v.transform.point.x.to_bits(),
                v.transform.point.y.to_bits(),
                v.transform.point.z.to_bits(),
                v.transform.rz.to_bits(),
                v.transform.screen_x.to_bits(),
                v.transform.screen_y.to_bits(),
                v.transform.int_y as u32,
                v.u.to_bits(),
                v.v.to_bits(),
                v.light.x.to_bits(),
                v.light.y.to_bits(),
                v.light.z.to_bits(),
                v.light.w.to_bits(),
                v.transform.flags as u32,
            ];
            for (i, (g, w)) in got.iter().zip(&want).enumerate() {
                assert_eq!(*g, ubits(w), "record {line_no}: out[{p}] field {i}");
            }
        }
        assert_eq!(cursor, out_toks.len(), "record {line_no}: trailing tokens");
    }
    assert_eq!(records, 800);
    assert!(nonzero >= 600, "only {nonzero} non-empty clip records");
    assert!(
        with_allocs >= 500,
        "only {with_allocs} records allocated crossings"
    );
}

#[test]
fn compute_clip_codes_matches_original_cpp() {
    let mut records = 0;
    let mut seen = [false; 256];
    for line in FIXTURE.lines().filter(|l| l.starts_with("CODES ")) {
        records += 1;
        let (lhs, rhs) = line.split_once(" |").expect("record separator");
        let t: Vec<&str> = lhs.split_whitespace().collect();
        // t[0] = "CODES", t[1..9] = frame, t[9..12] = point xyz.
        let fr = frame(&t[1..9]);
        let mut tt = vert9(&[t[9], t[10], t[11], "0", "0", "0", "0", "0", "0"]);
        compute_clip_codes(&fr, &mut tt);
        let want = u8::from_str_radix(rhs.trim(), 16).unwrap();
        seen[want as usize] = true;
        assert_eq!(tt.transform.flags, want, "{line}");
    }
    assert_eq!(records, 600);
    // Coverage: the empty code and each single-plane code (FVF_OUT_XMIN/XMAX/YMIN/YMAX).
    for flag in [0x00u8, 0x04, 0x08, 0x10, 0x20] {
        assert!(
            seen[flag as usize],
            "no CODES record with flags {flag:#04x}"
        );
    }
}
