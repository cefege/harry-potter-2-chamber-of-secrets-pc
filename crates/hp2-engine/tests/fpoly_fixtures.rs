//! Replays the original-C++ FPoly fixtures (`fixtures/fpoly_ops.txt`) against
//! `hp2_engine::fpoly`. The fixtures were captured by compiling `UnFPoly.cpp` and
//! `UnMath.cpp` from commit 40bde73 (standalone oracle, see the port notes).
//!
//! Each line is `<op> <inputs> => <outputs> [| W:msg] [| E:msg]`. Inputs are
//! parsed back into the polygon/plane/coords values; the output text is
//! regenerated with the harness's formatting (`%08x` float bits, decimal
//! counts/flags, `W:`/`E:` message trail) and compared whitespace-normalized.

use hp2_abi::engine::generated::FPoly;
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_engine::fpoly::{self, Log};

const FIXTURE: &str = include_str!("fixtures/fpoly_ops.txt");

#[derive(Default)]
struct Rec(Vec<String>);

impl Log for Rec {
    fn warn(&mut self, msg: &str) {
        self.0.push(format!("W:{msg}"));
    }
}

fn blank() -> FPoly {
    // SAFETY: `FPoly` is plain data (floats, integers, raw pointers); the
    // oracle's blank polygon is memset to zero.
    unsafe { std::mem::zeroed() }
}

struct Toks<'a>(std::str::SplitWhitespace<'a>);

impl<'a> Toks<'a> {
    fn word(&mut self) -> &'a str {
        self.0.next().expect("fixture token")
    }
    fn int(&mut self) -> i32 {
        self.word().parse().expect("decimal int")
    }
    fn uint(&mut self) -> u32 {
        self.word().parse().expect("decimal uint")
    }
    fn hex(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.word(), 16).expect("hex float bits"))
    }
    fn vec(&mut self) -> FVector {
        FVector {
            x: self.hex(),
            y: self.hex(),
            z: self.hex(),
        }
    }
    fn coords(&mut self) -> FCoords {
        FCoords {
            origin: self.vec(),
            x_axis: self.vec(),
            y_axis: self.vec(),
            z_axis: self.vec(),
        }
    }
    /// `fp(p)` input layout: N, PolyFlags, Base, Normal, TU, TV, then N vertices.
    fn poly(&mut self) -> FPoly {
        let mut p = blank();
        p.num_vertices = self.int();
        p.poly_flags = self.uint();
        p.base = self.vec();
        p.normal = self.vec();
        p.texture_u = self.vec();
        p.texture_v = self.vec();
        let n = p.num_vertices as usize;
        assert!(p.num_vertices >= 0 && n <= 16, "fixture poly N={n}");
        for i in 0..n {
            p.vertex[i] = self.vec();
        }
        p
    }
}

fn hx(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

fn hv(v: FVector) -> String {
    format!(" {} {} {}", hx(v.x), hx(v.y), hx(v.z))
}

/// Harness `fp(p)`: N, PolyFlags (unsigned decimal), four vectors, then vertices
/// only when `0 <= N <= 16`.
fn fp(p: &FPoly) -> String {
    let mut s = format!(" {} {}", p.num_vertices, p.poly_flags);
    for v in [p.base, p.normal, p.texture_u, p.texture_v] {
        s += &hv(v);
    }
    if (0..=16).contains(&p.num_vertices) {
        for v in &p.vertex[..p.num_vertices as usize] {
            s += &hv(*v);
        }
    }
    s
}

/// Harness `msgs()`: each queued message as ` | <msg>`.
fn msgs(log: &Rec) -> String {
    log.0.iter().map(|m| format!(" | {m}")).collect()
}

/// Whitespace-separated tokens of a fixture or output line.
fn tokens(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}

/// Token layout of `fp`: `N flags base(3) normal(3) TU(3) TV(3) verts...`.
/// Geometry (everything except TU/TV) for change detection.
fn geometry_key(p: &FPoly) -> Vec<String> {
    let mut t = tokens(&fp(p));
    t.drain(8..14);
    t
}

/// Original `FPoly::Split` copies `Front` (declared `FPoly Front, Back;` with no
/// initialiser) into `*this` only when the split really happens (`SP_Split`).
/// `SplitWithPlaneFast` never writes `TextureU` / `TextureV`, so the original's
/// split output then carries indeterminate words there. Documented divergence.
///
/// Output token layout: `ret N flags base(3) normal(3) TU(3) TV(3) verts...`, so
/// tokens 9..=14 are TU/TV. `run` masks them only when its `*this` geometry
/// changed; the expected line is masked at the same positions only in that case.
/// Every other split line compares TU/TV exactly.
fn mask_tu_tv(toks: &mut [String]) {
    for t in &mut toks[9..=14] {
        *t = "*".to_owned();
    }
}

fn run(op: &str, t: &mut Toks<'_>) -> String {
    let mut log = Rec::default();
    match op {
        "reverse" => {
            let mut p = t.poly();
            fpoly::reverse(&mut p);
            format!("{}{}", fp(&p), msgs(&log))
        }
        "fix" => {
            let mut p = t.poly();
            let r = fpoly::fix(&mut p, &mut log);
            format!("{r}{}{}", fp(&p), msgs(&log))
        }
        "area" => {
            let p = t.poly();
            let a = fpoly::area(&p);
            format!("{}{}", hx(a), msgs(&log))
        }
        "calcnormal" => {
            let silent = t.int() != 0;
            let mut p = t.poly();
            let r = fpoly::calc_normal(&mut p, silent, &mut log);
            format!("{r}{}{}", fp(&p), msgs(&log))
        }
        "finalize" => {
            let no_error = t.int() != 0;
            let mut p = t.poly();
            match fpoly::finalize(&mut p, no_error, &mut log) {
                Ok(r) => format!("{r}{}{}", fp(&p), msgs(&log)),
                Err(m) => {
                    log.0.push(format!("E:{m}"));
                    format!("x{}{}", fp(&p), msgs(&log))
                }
            }
        }
        "faces" => {
            let a = t.poly();
            let b = t.poly();
            let r = fpoly::faces(&a, &b);
            format!("{r}{}", msgs(&log))
        }
        "splitplane" => {
            let very_precise = t.int() != 0;
            let null_out = t.int() != 0;
            let base = t.vec();
            let normal = t.vec();
            let p = t.poly();
            let mut front = blank();
            let mut back = blank();
            let out = if null_out {
                None
            } else {
                Some((&mut front, &mut back))
            };
            let r = fpoly::split_with_plane(&p, base, normal, out, very_precise, &mut log)
                .expect("splitplane");
            format!("{r}{}{}{}", fp(&front), fp(&back), msgs(&log))
        }
        "splitfast" => {
            let x = t.hex();
            let y = t.hex();
            let z = t.hex();
            let w = t.hex();
            let p = t.poly();
            let mut front = blank();
            let mut back = blank();
            let r = fpoly::split_with_plane_fast(
                &p,
                FPlane { x, y, z, w },
                Some((&mut front, &mut back)),
            )
            .expect("splitfast");
            format!("{r}{}{}{}", fp(&front), fp(&back), msgs(&log))
        }
        "splitinhalf" => {
            let mut p = t.poly();
            let mut other = blank();
            if let Err(m) = fpoly::split_in_half(&mut p, &mut other) {
                log.0.push(format!("E:{m}"));
            }
            format!("{}{}{}", fp(&other), fp(&p), msgs(&log))
        }
        "split" => {
            let no_overflow = t.int() != 0;
            let normal = t.vec();
            let base = t.vec();
            let mut p = t.poly();
            let before = geometry_key(&p);
            let r = fpoly::split(&mut p, normal, base, no_overflow).expect("split");
            let mut toks = tokens(&format!("{r}{}", fp(&p)));
            if geometry_key(&p) != before {
                mask_tu_tv(&mut toks);
            }
            format!("{}{}", toks.join(" "), msgs(&log))
        }
        "removecolinears" => {
            let mut p = t.poly();
            let r = fpoly::remove_colinears(&mut p);
            format!("{r}{}{}", fp(&p), msgs(&log))
        }
        "transform" => {
            let orient = t.hex();
            let pre = t.vec();
            let post = t.vec();
            let point = t.coords();
            let vector = t.coords();
            let mut p = t.poly();
            fpoly::transform(&mut p, &point, &vector, pre, post, orient);
            format!("{}{}", fp(&p), msgs(&log))
        }
        other => panic!("unknown fixture op `{other}`"),
    }
}

#[test]
fn fpoly_fixtures_match_original_cpp() {
    let mut total = 0usize;
    let mut per_op = std::collections::BTreeMap::<String, usize>::new();
    let mut mismatches = Vec::new();
    for (idx, line) in FIXTURE.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (lhs, rhs) = line.split_once("=>").expect("fixture line has `=>`");
        let mut lhs = Toks(lhs.split_whitespace());
        let op = lhs.word().to_owned();
        let actual = tokens(&run(&op, &mut lhs));
        let mut expected = tokens(rhs);
        total += 1;
        *per_op.entry(op.clone()).or_default() += 1;
        // Only a split that changed `*this` leaves TU/TV indeterminate in the original.
        if op == "split" && actual.len() >= 15 && actual[9] == "*" && expected.len() >= 15 {
            mask_tu_tv(&mut expected);
        }
        if actual != expected {
            mismatches.push(format!(
                "line {}: {op}\n  expected: {}\n  actual:   {}",
                idx + 1,
                expected.join(" "),
                actual.join(" ")
            ));
        }
    }
    assert!(total > 0, "no fixture lines");
    assert!(
        mismatches.is_empty(),
        "{} of {total} fixture lines mismatched (per op: {per_op:?})\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .map(|m| m.lines().next().unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Output overflow of `SplitWithPlane`. The original's check (UnFPoly.cpp:173) tests
/// the input count, which `check()` already bounds, so a front list grown past 16
/// is unguarded UB there. This port reports the same message as an error instead.
/// Derived from the C++ source, not from an oracle fixture line.
#[test]
fn split_with_plane_reports_front_overflow() {
    // Regular 16-gon of radius 100 in the z=0 plane; the plane x = -99 cuts off
    // one vertex, leaving 15 front vertices plus 2 crossings = 17 > 16.
    let mut p: FPoly = unsafe { std::mem::zeroed() };
    p.num_vertices = 16;
    for (i, v) in p.vertex.iter_mut().enumerate() {
        let a = i as f64 * std::f64::consts::TAU / 16.0;
        *v = FVector {
            x: (100.0 * a.cos()) as f32,
            y: (100.0 * a.sin()) as f32,
            z: 0.0,
        };
    }
    let mut front: FPoly = unsafe { std::mem::zeroed() };
    let mut back: FPoly = unsafe { std::mem::zeroed() };
    let base = FVector {
        x: -99.0,
        y: 0.0,
        z: 0.0,
    };
    let normal = FVector {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    let err = fpoly::split_with_plane(
        &p,
        base,
        normal,
        Some((&mut front, &mut back)),
        false,
        &mut fpoly::NoLog,
    )
    .expect_err("17 front vertices must overflow");
    assert_eq!(err, "FPoly::SplitWithPlane: Vertex overflow");
}
