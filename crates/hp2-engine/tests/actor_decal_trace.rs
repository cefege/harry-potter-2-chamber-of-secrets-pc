//! Replays the `trace_decal_flip` family of `tests/fixtures/actor_unactor.txt` through the
//! Rust decal step of `trace_texture` (`actor::decal_texture_override`, which runs
//! `point_inside_quad` on the surface-local hit point). The fixture was produced by the
//! contract-on oracle build of `UnActor.cpp`, and each case was selected because the
//! contract-off build of the same TU gives a different texture for it.
//!
//! Inputs per case (18 f32 bit patterns, from the `bits=` knob): Verts[4] = words 0..12,
//! SurfNormal = words 12..15, Hit.Location = words 15..18. Surface base is the origin, so
//! the local hit point equals Hit.Location.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_engine::actor::decal_texture_override;

const TEX0: usize = 0x1000;
const TEX1: usize = 0x2000;

fn fixture() -> &'static str {
    include_str!("fixtures/actor_unactor.txt")
}

/// `(bits, expected texture)` for each `trace_decal_flip` case, in fixture order.
fn decal_cases() -> Vec<([u32; 18], String)> {
    let mut out = Vec::new();
    let mut in_decal = false;
    let mut bits = None;
    for line in fixture().lines() {
        if let Some(rest) = line.strip_prefix("case ") {
            let mut parts = rest.splitn(3, ' ');
            in_decal = parts.next() == Some("trace_decal_flip");
            if in_decal {
                let _idx = parts.next();
                let knobs = parts.next().expect("decal case without knobs");
                let hex = knobs
                    .strip_prefix("decal_flip bits=")
                    .expect("decal knobs format");
                let mut b = [0u32; 18];
                for (i, h) in hex.split(',').enumerate() {
                    b[i] = u32::from_str_radix(h, 16).expect("hex word");
                }
                bits = Some(b);
            }
        } else if let (true, Some(tex)) = (in_decal, line.strip_prefix("res texture=")) {
            let name = tex.split(' ').next().expect("texture name").to_string();
            out.push((bits.take().expect("res without case"), name));
        }
    }
    out
}

fn ptr(v: usize) -> *mut c_void {
    v as *mut c_void
}

#[test]
fn trace_decal_flip_cases_match_contract_on_oracle() {
    let cases = decal_cases();
    assert_eq!(cases.len(), 8, "decal flip family must have 8 cases");

    let (mut saw_tex0, mut saw_tex1) = (false, false);
    for (bits, want) in &cases {
        let f: [f32; 18] = bits.map(f32::from_bits);
        let verts = [0usize, 1, 2, 3].map(|k| FVector {
            x: f[3 * k],
            y: f[3 * k + 1],
            z: f[3 * k + 2],
        });
        let normal = FVector {
            x: f[12],
            y: f[13],
            z: f[14],
        };
        let hit = FVector {
            x: f[15],
            y: f[16],
            z: f[17],
        };
        let got = decal_texture_override(
            ptr(TEX0),
            hit,
            normal,
            1,
            |_| ptr(1),
            |_| verts,
            |_| ptr(TEX1),
        );
        let want_ptr = match want.as_str() {
            "tex0" => {
                saw_tex0 = true;
                TEX0
            }
            "tex1" => {
                saw_tex1 = true;
                TEX1
            }
            other => panic!("unexpected texture name {other}"),
        };
        assert_eq!(got as usize, want_ptr, "bits {bits:08x?}");
    }
    assert!(saw_tex0 && saw_tex1, "family must cover both outcomes");
}
