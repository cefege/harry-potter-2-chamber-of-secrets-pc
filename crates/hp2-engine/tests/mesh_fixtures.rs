//! Replays the original-C++ mesh oracle fixtures (`fixtures/mesh_frame.txt`,
//! `fixtures/lodmesh_frame.txt`, `fixtures/mesh_bounds.txt`,
//! `fixtures/mesh_anim_seq.txt`) against `hp2_engine::mesh`,
//! `hp2_engine::lodmesh` and `hp2_engine::coords`.
//!
//! The fixtures were produced by linking the pristine 8e34caa `UnMesh.cpp`,
//! `ULodMesh.cpp`, `UnMath.cpp`, `UnCache.cpp` and `UnAnsi.cpp` objects with a
//! harness that drives the original `UMesh::GetFrame`, `ULodMesh::GetFrame`,
//! `GetRenderBoundingBox`, `GetCollisionBoundingBox`, `GetCollisionExtent`,
//! `GetBoneCoords` and `GetAnimSeq` with deterministic inputs. The inputs are
//! regenerated below with the same xorshift64 stream and call order. Floats
//! are IEEE-754 bit patterns in hex, so every comparison is bit-exact.
//!
//! The harness source was built under the per-agent output directory and
//! deleted with it; it is not in the repository. The fixtures are therefore
//! not regenerable from this tree. Any change to the RNG draw order, the
//! `fillMesh`/`makeOwner` field order, or the `Scale` draw (`rvnz`) must be
//! mirrored here and the fixtures re-derived from the original C++.
//!
//! Mesh line: `T <seed> frames=.. special=.. seqs=.. packed=..` followed by
//! `C <n> seq=<i>|miss frame=<bits> [lodreq=<n>] | <verts...>` per call.
//! Bounds line: `B <seed> render: <coords> coll: <min> <max> valid=<n> extent: <v> bone<n>: <coords>`.
//! Anim line: `A <seed> q<n> key=<k> idx=<i>`.
//!
//! The LOD harness fills the output with `0xCDCDCDCD` and `ULodMesh::GetFrame`
//! writes only `VertexNum` entries, so the Rust side fills the same sentinel
//! and emits only the entries it writes.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::fmt::Write as _;
use std::sync::LazyLock;

use hp2_abi::engine::generated::FRotator;
use hp2_abi::{FCoords, FVector};
use hp2_engine::coords::{self, Trig};
use hp2_engine::lodmesh::{self, LodView};
use hp2_engine::mesh::{
    self, ActorPose, AnimSeqRaw, AnimState, CacheBlock, CacheKey, FBoxRaw, FrameCache, MeshView,
    OwnerView,
};

const SEQ_FRAMES: i32 = 16;
const SENTINEL: u32 = 0xCDCD_CDCD;
const SEED_MUL: u64 = 0x9E37_79B9_7F4A_7C15;

static TRIG: LazyLock<Vec<f32>> = LazyLock::new(|| {
    (0..16384i32)
        .map(|i| {
            let arg = (i as f32) * 2.0f32 * PI / 16384.0f32;
            (arg as f64).sin() as f32
        })
        .collect()
});

// `GMath.SinTab(i) = TrigFLOAT[(i>>2)&16383]`.
fn sin_tab(angle: i32) -> f32 {
    TRIG[((angle >> 2) & 16383) as usize]
}

// `GMath.CosTab(i) = TrigFLOAT[((i+16384)>>2)&16383]`.
fn cos_tab(angle: i32) -> f32 {
    TRIG[((angle.wrapping_add(16384) >> 2) & 16383) as usize]
}

/// Xorshift64 state as in the harness (`rnd`, `ri`, `rq`, `rv`, `rvnz`).
struct Rng(u64);

impl Rng {
    /// `rs = seed * 0x9E3779B97F4A7C15 + offset`; offsets 1, 7, 13 select the trace kind.
    fn seeded(seed: u64, offset: u64) -> Self {
        Rng(seed.wrapping_mul(SEED_MUL).wrapping_add(offset))
    }

    fn rnd(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn ri(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.rnd() % (hi - lo + 1) as u64) as i32
    }

    fn rq(&mut self, lo: i32, hi: i32) -> f32 {
        self.ri(lo * 64, hi * 64) as f32 / 64.0
    }

    fn rv(&mut self, lo: i32, hi: i32) -> FVector {
        let x = self.rq(lo, hi);
        let y = self.rq(lo, hi);
        let z = self.rq(lo, hi);
        FVector { x, y, z }
    }

    fn sign(&mut self) -> f32 {
        if self.ri(0, 1) != 0 { -1.0 } else { 1.0 }
    }

    /// Nonzero per-axis values in +/-[1,4]: sign, then magnitude, per axis.
    fn rvnz(&mut self) -> FVector {
        let sx = self.sign();
        let x = sx * self.rq(1, 4);
        let sy = self.sign();
        let y = sy * self.rq(1, 4);
        let sz = self.sign();
        let z = sz * self.rq(1, 4);
        FVector { x, y, z }
    }
}

/// `FMeshVert` bitfield word: `INT X:11, Y:11, Z:10`, little-endian bit order.
fn pack_vert(x: i32, y: i32, z: i32) -> u32 {
    (x as u32 & 0x7FF) | ((y as u32 & 0x7FF) << 11) | ((z as u32 & 0x3FF) << 22)
}

struct MeshData {
    frame_verts: i32,
    verts: Vec<u32>,
    seqs: Vec<AnimSeqRaw>,
    bounding_boxes: Vec<FBoxRaw>,
    bounding_box: FBoxRaw,
    scale: FVector,
    origin: FVector,
    rot_origin: FRotator,
}

impl MeshData {
    fn view(&self) -> MeshView<'_> {
        MeshView {
            frame_verts: self.frame_verts,
            verts: &self.verts,
            anim_seqs: &self.seqs,
            bounding_boxes: &self.bounding_boxes,
            bounding_box: self.bounding_box,
            scale: self.scale,
            origin: self.origin,
            rot_origin: self.rot_origin,
        }
    }
}

/// Mirrors `fillMesh` in the C++ harness; RNG call order matters.
fn fill_mesh(r: &mut Rng) -> MeshData {
    let frame_verts = r.ri(1, 24);
    let n = (frame_verts * SEQ_FRAMES) as usize;
    let mut verts = Vec::with_capacity(n);
    for _ in 0..n {
        let x = r.ri(-1024, 1023);
        let y = r.ri(-1024, 1023);
        let z = r.ri(-512, 511);
        verts.push(pack_vert(x, y, z));
    }
    let ns = r.ri(1, 3);
    let mut seqs = Vec::new();
    for k in 0..ns {
        let start_frame = r.ri(0, 5);
        let num_frames = r.ri(1, 8);
        let rate = r.rq(1, 30);
        seqs.push(AnimSeqRaw {
            name: 200 + k,
            group: 0,
            start_frame,
            num_frames,
            rate,
            notifys: [0, 0],
        });
    }
    let bb_min = r.rv(-64, 0);
    let bb_max = r.rv(0, 64);
    let bounding_box = FBoxRaw {
        min: bb_min,
        max: bb_max,
        is_valid: 1,
    };
    let scale = r.rvnz();
    let origin = r.rv(-32, 32);
    let pitch = r.ri(-65536, 65535);
    let yaw = r.ri(-65536, 65535);
    let roll = r.ri(-65536, 65535);
    let rot_origin = FRotator { pitch, yaw, roll };
    let mut bounding_boxes = Vec::with_capacity(16);
    for _ in 0..16 {
        let min = r.rv(-64, 0);
        let max = r.rv(0, 64);
        bounding_boxes.push(FBoxRaw {
            min,
            max,
            is_valid: 1,
        });
    }
    MeshData {
        frame_verts,
        verts,
        seqs,
        bounding_boxes,
        bounding_box,
        scale,
        origin,
        rot_origin,
    }
}

struct Actor {
    pose: ActorPose,
    frame: f32,
}

/// Mirrors `makeOwner` (the `byOwner` argument is drawn by the caller first).
fn make_actor(r: &mut Rng) -> Actor {
    let location = r.rv(-64, 64);
    let pre_pivot = r.rv(-8, 8);
    let pitch = r.ri(-65536, 65535);
    let yaw = r.ri(-65536, 65535);
    let roll = r.ri(-65536, 65535);
    let draw_scale = r.rq(1, 2);
    let frame = r.rq(-4, 20);
    Actor {
        pose: ActorPose {
            location,
            pre_pivot,
            rotation: FRotator { pitch, yaw, roll },
            draw_scale,
        },
        frame,
    }
}

/// Mirrors `makeCoords`: identity axes at a random origin.
fn make_coords(r: &mut Rng) -> FCoords {
    let mut c = coords::unit_coords();
    c.origin = r.rv(-128, 128);
    c
}

/// `GCache` stand-in: one locked block per id, persisting across calls within a
/// trace (the harness flushes between traces). Boxes keep block addresses stable.
#[derive(Default)]
struct MockCache {
    blocks: HashMap<u64, Box<[u8]>>,
}

impl FrameCache for MockCache {
    fn get(&mut self, id: u64, _len: usize) -> Option<CacheBlock> {
        let b = self.blocks.get_mut(&id)?;
        Some(CacheBlock {
            ptr: b.as_mut_ptr(),
            len: b.len(),
            item: std::ptr::null_mut(),
        })
    }

    fn create(&mut self, id: u64, len: usize) -> CacheBlock {
        let mut b = vec![0xAAu8; len].into_boxed_slice();
        let block = CacheBlock {
            ptr: b.as_mut_ptr(),
            len,
            item: std::ptr::null_mut(),
        };
        self.blocks.insert(id, b);
        block
    }

    fn flush(&mut self, id: u64) {
        self.blocks.remove(&id);
    }

    fn unlock(&mut self, _block: CacheBlock) {}
}

fn pf(out: &mut String, x: f32) {
    write!(out, " {:08x}", x.to_bits()).unwrap();
}

fn pv(out: &mut String, v: FVector) {
    pf(out, v.x);
    pf(out, v.y);
    pf(out, v.z);
}

fn pc(out: &mut String, c: FCoords) {
    pv(out, c.origin);
    pv(out, c.x_axis);
    pv(out, c.y_axis);
    pv(out, c.z_axis);
}

/// One `T`/`C` trace: `ri(2,4)` calls against a single owner and parent.
fn mesh_trace(seed: u64, lod: bool, out: &mut String) {
    let mut r = Rng::seeded(seed, 1);
    let m = fill_mesh(&mut r);
    let fv = m.frame_verts;
    let special = if lod { r.ri(0, fv) } else { 0 };
    let parent = make_actor(&mut r);
    let by_owner = r.ri(0, 3) == 0;
    let owner_actor = make_actor(&mut r);
    let ncalls = r.ri(2, 4);
    let packed = m.verts.iter().fold(0u32, |a, &w| a.wrapping_add(w));
    writeln!(
        out,
        "T {seed} frames={fv} special={special} seqs={} packed={packed:08x}",
        m.seqs.len()
    )
    .unwrap();

    let parent_anim = AnimState {
        sequence: 200,
        frame: parent.frame,
    };
    let key = CacheKey {
        id: 1,
        mesh: 0x1000,
    };
    let mut cache = MockCache::default();
    let sin = &sin_tab;
    let cos = &cos_tab;
    let t = Trig { sin, cos };
    for c in 0..ncalls {
        let sel = r.ri(0, 9);
        let (idx, seq) = if sel == 0 {
            (-1, 999)
        } else {
            let i = r.ri(0, m.seqs.len() as i32 - 1);
            (i, 200 + i)
        };
        let frame = r.rq(-4, 20);
        write!(out, "C {c}").unwrap();
        if idx < 0 {
            out.push_str(" miss");
        } else {
            write!(out, " seq={idx}").unwrap();
        }
        write!(out, " frame={:08x}", frame.to_bits()).unwrap();
        let coords = make_coords(&mut r);
        let owner = OwnerView {
            pose: owner_actor.pose,
            anim: AnimState {
                sequence: seq,
                frame,
            },
            parent_anim: Some(parent_anim),
            anim_by_owner: by_owner,
        };
        let req = if lod { r.ri(0, fv) } else { 0 };
        let mut res = vec![[SENTINEL; 3]; fv as usize];
        let mut lod_ret = None;
        {
            let mut emit = |i: usize, p: FVector| {
                res[i] = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
            };
            if lod {
                let lv = LodView {
                    base: m.view(),
                    special_verts: special,
                    lod_request: req,
                };
                lod_ret = Some(lodmesh::get_frame(
                    &lv, &owner, coords, &t, &mut cache, key, &mut emit,
                ));
            } else {
                mesh::get_frame(&m.view(), &owner, coords, &t, &mut cache, key, &mut emit);
            }
        }
        if let Some(ret) = lod_ret {
            write!(out, " lodreq={ret}").unwrap();
        }
        out.push_str(" |");
        for v in &res {
            for b in v {
                write!(out, " {b:08x}").unwrap();
            }
        }
        out.push('\n');
    }
}

/// One `B` line: render, collision, extent and a bone's coords for an unowned actor.
fn bounds_trace(seed: u64, out: &mut String) {
    let mut r = Rng::seeded(seed, 7);
    let m = fill_mesh(&mut r);
    let a = make_actor(&mut r);
    let owner = OwnerView {
        pose: a.pose,
        anim: AnimState {
            sequence: 200,
            frame: a.frame,
        },
        parent_anim: None,
        anim_by_owner: false,
    };
    let view = m.view();
    let sin = &sin_tab;
    let cos = &cos_tab;
    let t = Trig { sin, cos };
    let render = mesh::render_bounding_box(&view, &owner, &t);
    let cb = mesh::collision_bounding_box(&view, &owner, (seed & 2) != 0, &t);
    let extent = mesh::collision_extent(&view, &owner, &t);
    let bone = r.ri(0, 3);
    let bone_c = mesh::bone_coords(&view, &owner, &t);

    write!(out, "B {seed} render:").unwrap();
    pc(out, render);
    out.push_str(" coll:");
    pv(out, cb.min);
    pv(out, cb.max);
    write!(out, " valid={}", cb.valid as i32).unwrap();
    out.push_str(" extent:");
    pv(out, extent);
    write!(out, " bone{bone}:").unwrap();
    pc(out, bone_c);
    out.push('\n');
}

/// `A` lines: `GetAnimSeq` lookups for three known names and one miss.
fn anim_trace(seed: u64, out: &mut String) {
    let mut r = Rng::seeded(seed, 13);
    let m = fill_mesh(&mut r);
    let n = m.seqs.len() as i32;
    for q in 0..4 {
        let key = if q == 3 { 999 } else { 200 + r.ri(0, n - 1) };
        let idx = mesh::anim_seq_index(&m.seqs, key).map_or(-1, |i| i as i32);
        writeln!(out, "A {seed} q{q} key={key} idx={idx}").unwrap();
    }
}

fn assert_fixture(name: &str, want: &str, got: &str) {
    let want_lines: Vec<&str> = want.lines().collect();
    let got_lines: Vec<&str> = got.lines().collect();
    assert_eq!(got_lines.len(), want_lines.len(), "{name}: line count");
    for (i, (g, w)) in got_lines.iter().zip(&want_lines).enumerate() {
        assert_eq!(g, w, "{name}: line {}", i + 1);
    }
}

fn count_prefix(text: &str, prefix: &str) -> usize {
    text.lines().filter(|l| l.starts_with(prefix)).count()
}

#[test]
fn mesh_frame_matches_oracle() {
    let want = include_str!("fixtures/mesh_frame.txt");
    let mut got = String::new();
    for seed in 1..=32 {
        mesh_trace(seed, false, &mut got);
    }
    assert_eq!(count_prefix(want, "T "), 32, "mesh traces");
    assert_eq!(count_prefix(want, "C "), 99, "mesh calls");
    assert_fixture("mesh_frame", want, &got);
}

#[test]
fn lodmesh_frame_matches_oracle() {
    let want = include_str!("fixtures/lodmesh_frame.txt");
    let mut got = String::new();
    for seed in 1..=32 {
        mesh_trace(seed, true, &mut got);
    }
    assert_eq!(count_prefix(want, "T "), 32, "lod traces");
    assert_eq!(count_prefix(want, "C "), 98, "lod calls");
    assert_fixture("lodmesh_frame", want, &got);
}

#[test]
fn mesh_bounds_matches_oracle() {
    let want = include_str!("fixtures/mesh_bounds.txt");
    let mut got = String::new();
    for seed in 1..=32 {
        bounds_trace(seed, &mut got);
    }
    assert_eq!(count_prefix(want, "B "), 32, "bounds cases");
    assert_fixture("mesh_bounds", want, &got);
}

#[test]
fn mesh_anim_seq_matches_oracle() {
    let want = include_str!("fixtures/mesh_anim_seq.txt");
    let mut got = String::new();
    for seed in 1..=16 {
        anim_trace(seed, &mut got);
    }
    assert_eq!(count_prefix(want, "A "), 64, "anim-seq cases");
    assert_fixture("mesh_anim_seq", want, &got);
}
