//! Replays the `skeletal_apply_anim`, `skeletal_frame`, `skeletal_root` and
//! `skeletal_bounds` fixture families against `hp2_engine::skeletal`.
//!
//! The fixtures come from the ORIGINAL C++ (`Build/skeletal_fixture_harness`,
//! pinned ref 32dcfda). Each line streams its inputs (the mesh block, the anim
//! block, and the actor blocks) and then the outputs, in the order written by the
//! harness `run_*` functions. This file rebuilds the same objects as mirror
//! structs, calls the port, and compares every output token bit-exactly.
//!
//! Host semantics mirrored from the C++ harness:
//! - `SinTab`/`CosTab`: the 16384-entry table from `skeletal_trig.txt`, indexed with
//!   the `ANGLE_SHIFT` and mask of UnMath.h:1520-1527.
//! - GCache: one block per owner (CID_SkeletalData), cleared per case; unlock is a no-op.
//! - `DestroyActor` hook: records the aux actor pointer; `AuxAnims.Remove(index)` is
//!   performed on the owner's array.
//! - `DisplayBones`: `DebugPivots`/`DebugParents` sized to `SpaceBases.Num()` and
//!   filled as UnSkeletalMesh.cpp:789-800 does. `DisplayInfluences`: `DebugVerts`
//!   sized to `FrameVerts` (UnSkeletalMesh.cpp:803-805). Only the counts and the
//!   pivot/parent values are compared (the harness prints nothing else).
//! - `debugf` (missing-link warning) is discarded; `check()` failures panic.
//!   Field mismatches are collected per family and reported together at the end of
//!   each test (case number and field, got/want bits, ULP distance); structural
//!   errors (missing or trailing tokens) panic at once.
//!
//! Allocations (mirror objects, arrays, cache blocks) are leaked on purpose: the
//! replay needs stable addresses for the lifetime of a case, and the test process
//! is short-lived.
//!
//! Not replayed (not printed by the harness): `AnimRate` (no mirror field; no C++
//! path under test reads it), `Points` (consumed and skipped), `Exact` for
//! `GetRenderBoundingBox` (the C++ ignores it; the Rust port has no such parameter),
//! and the `FBox` pad bytes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::mem::zeroed;
use std::ptr::{copy, null_mut};

use hp2_abi::engine::generated::{
    AActor, FAnalogTrack, FBox, FMeshAnimSeq, FMeshBone, FMeshFace, FNamedBone, FPlace,
    MotionChunk, TArrayRaw, TRefArrayRaw, UAnimation, USkeletalMesh, VBoneInfIndex, VBoneInfluence,
};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_engine::skeletal::{self, AnimVec, SkelHost};

const APPLY: &str = include_str!("fixtures/skeletal_apply_anim.txt");
const FRAME: &str = include_str!("fixtures/skeletal_frame.txt");
const ROOT: &str = include_str!("fixtures/skeletal_root.txt");
const BOUNDS: &str = include_str!("fixtures/skeletal_bounds.txt");
const TRIG: &str = include_str!("fixtures/skeletal_trig.txt");

/// Line counts of the fixture families (`run_apply`, `run_frame`, `run_root`, `run_bounds`).
const APPLY_CASES: usize = 300;
const FRAME_CASES: usize = 300;
const ROOT_CASES: usize = 200;
const BOUNDS_CASES: usize = 200;
/// `FGlobalMath::NUM_ANGLES` (UnMath.h:1507).
const TRIG_ENTRIES: usize = 16384;

/// Distance in units in the last place between two f32 values (0 when bit-identical).
fn ulp_distance(a: f32, b: f32) -> u32 {
    // Map sign-magnitude bits onto a monotonic unsigned line.
    let ord = |x: u32| {
        if x & 0x8000_0000 != 0 {
            !x
        } else {
            x | 0x8000_0000
        }
    };
    ord(a.to_bits()).abs_diff(ord(b.to_bits()))
}

fn zero<T>() -> T {
    // SAFETY: every mirror type used here is plain data (integers, floats, raw pointers,
    // arrays of those), for which the all-zero bit pattern is a valid value.
    unsafe { zeroed() }
}

/// Views a mirror `TArrayRaw`/`TRefArrayRaw` payload as a slice.
///
/// # Safety
/// `data` must point to `num` live, initialised `T` (or `num` must be 0).
unsafe fn elems<'a, T>(data: *mut c_void, num: i32) -> &'a [T] {
    if num <= 0 {
        &[]
    } else {
        // SAFETY: the caller guarantees `num` initialised elements at `data`.
        unsafe { std::slice::from_raw_parts(data.cast::<T>(), num as usize) }
    }
}

fn load_trig() -> Vec<f32> {
    let table: Vec<f32> = TRIG
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let mut w = line.split_whitespace();
            let idx: usize = w.next().unwrap().parse().unwrap();
            assert_eq!(idx, i, "trig fixture index order");
            let hex = w.next().unwrap().strip_prefix("0x").unwrap();
            f32::from_bits(u32::from_str_radix(hex, 16).unwrap())
        })
        .collect();
    assert_eq!(table.len(), TRIG_ENTRIES, "skeletal_trig.txt line count");
    table
}

/// Token reader over one fixture line. `expect_*` consume the printed output token
/// and compare it with the value the port produced.
struct Line<'a> {
    it: std::str::SplitWhitespace<'a>,
    case: i64,
    /// Field mismatches for this case, in fixture order; `expect_*` never panic.
    misses: Vec<String>,
}

impl<'a> Line<'a> {
    fn new(text: &'a str) -> Self {
        let mut it = text.split_whitespace();
        let case = it
            .next()
            .and_then(|w| w.parse().ok())
            .unwrap_or_else(|| panic!("missing case index: {text}"));
        Line {
            it,
            case,
            misses: Vec::new(),
        }
    }

    fn word(&mut self) -> &'a str {
        self.it
            .next()
            .unwrap_or_else(|| panic!("case {}: fixture line ends early", self.case))
    }

    fn int(&mut self) -> i64 {
        let w = self.word();
        w.parse()
            .unwrap_or_else(|e| panic!("case {}: bad int {w}: {e}", self.case))
    }

    fn bits(&mut self) -> f32 {
        let w = self.word();
        let hex = w
            .strip_prefix("0x")
            .unwrap_or_else(|| panic!("case {}: expected float bits, got {w}", self.case));
        f32::from_bits(
            u32::from_str_radix(hex, 16)
                .unwrap_or_else(|e| panic!("case {}: bad float bits {w}: {e}", self.case)),
        )
    }

    fn vec(&mut self) -> FVector {
        FVector {
            x: self.bits(),
            y: self.bits(),
            z: self.bits(),
        }
    }

    fn quat4(&mut self) -> [f32; 4] {
        [self.bits(), self.bits(), self.bits(), self.bits()]
    }

    fn anim_vec(&mut self) -> AnimVec {
        AnimVec {
            x: self.int() as i16,
            y: self.int() as i16,
            z: self.int() as i16,
        }
    }

    /// `pn`: an FName printed as its index; the mirror stores the index as LE bytes.
    fn name(&mut self) -> [u8; 4] {
        (self.int() as i32).to_le_bytes()
    }

    fn coords(&mut self) -> FCoords {
        FCoords {
            origin: self.vec(),
            x_axis: self.vec(),
            y_axis: self.vec(),
            z_axis: self.vec(),
        }
    }

    fn fbox(&mut self) -> FBox {
        let min = self.vec();
        let max = self.vec();
        let mut b: FBox = zero();
        b.min = min;
        b.max = max;
        b.is_valid = self.int() as u8;
        b
    }

    fn expect_int(&mut self, label: &str, got: i64) {
        let want = self.int();
        if got != want {
            self.misses.push(format!("{label}: got {got} want {want}"));
        }
    }

    fn expect_bits(&mut self, label: &str, got: f32) {
        let want = self.bits();
        if got.to_bits() != want.to_bits() {
            self.misses.push(format!(
                "{label}: got {got:e} (0x{:08x}) want {want:e} (0x{:08x}) ulp {}",
                got.to_bits(),
                want.to_bits(),
                ulp_distance(got, want)
            ));
        }
    }

    fn expect_vec(&mut self, label: &str, got: FVector) {
        self.expect_bits(&format!("{label}.x"), got.x);
        self.expect_bits(&format!("{label}.y"), got.y);
        self.expect_bits(&format!("{label}.z"), got.z);
    }

    fn expect_coords(&mut self, label: &str, got: FCoords) {
        self.expect_vec(&format!("{label}.origin"), got.origin);
        self.expect_vec(&format!("{label}.x"), got.x_axis);
        self.expect_vec(&format!("{label}.y"), got.y_axis);
        self.expect_vec(&format!("{label}.z"), got.z_axis);
    }

    fn expect_box(&mut self, label: &str, min: FVector, max: FVector, valid: bool) {
        self.expect_vec(&format!("{label}.min"), min);
        self.expect_vec(&format!("{label}.max"), max);
        self.expect_int(&format!("{label}.valid"), valid as i64);
    }

    /// Checks the line is fully consumed and returns this case's mismatches, prefixed with the case index.
    fn end(mut self) -> Vec<String> {
        assert!(
            self.it.next().is_none(),
            "case {}: trailing tokens in fixture line",
            self.case
        );
        let case = self.case;
        self.misses
            .into_iter()
            .map(|m| format!("case {case}: {m}"))
            .collect()
    }
}

/// Per-case host. Leaks mirror allocations (see the module comment).
struct Host<'a> {
    trig: &'a [f32],
    /// GCache blocks keyed by owner (CID_SkeletalData): owner address -> block.
    blocks: RefCell<HashMap<usize, *mut u8>>,
    /// Actors passed to `GetLevel()->DestroyActor` (aux removal), in call order.
    destroyed: RefCell<Vec<usize>>,
}

impl<'a> Host<'a> {
    fn new(trig: &'a [f32]) -> Self {
        Host {
            trig,
            blocks: RefCell::new(HashMap::new()),
            destroyed: RefCell::new(Vec::new()),
        }
    }

    fn keep<T: 'static>(&self, v: T) -> *mut T {
        Box::leak(Box::new(v)) as *mut T
    }

    fn array<T: 'static>(&self, v: Vec<T>) -> TArrayRaw {
        let mut v = std::mem::ManuallyDrop::new(v);
        let num = v.len() as i32;
        TArrayRaw {
            data: v.as_mut_ptr().cast::<c_void>(),
            num,
            max: num,
        }
    }

    fn tref<T: 'static>(&self, v: Vec<T>) -> TRefArrayRaw {
        let mut v = std::mem::ManuallyDrop::new(v);
        let mut r: TRefArrayRaw = zero();
        r.num = v.len() as i32;
        r.data = v.as_mut_ptr().cast::<c_void>();
        r
    }
}

impl SkelHost for Host<'_> {
    fn sin_tab(&self, index: i32) -> f32 {
        // SinTab(i) = TrigFLOAT[(i >> ANGLE_SHIFT) & (NUM_ANGLES-1)], UnMath.h:1520.
        self.trig[((index >> 2) & (TRIG_ENTRIES as i32 - 1)) as usize]
    }

    fn cos_tab(&self, index: i32) -> f32 {
        // CosTab(i) = TrigFLOAT[((i + 16384) >> ANGLE_SHIFT) & (NUM_ANGLES-1)], UnMath.h:1527.
        let shifted = index.wrapping_add(TRIG_ENTRIES as i32) >> 2;
        self.trig[(shifted & (TRIG_ENTRIES as i32 - 1)) as usize]
    }

    fn cache_get(&self, owner: *const AActor, item: &mut *mut c_void) -> *mut u8 {
        match self.blocks.borrow().get(&(owner as usize)) {
            Some(&block) => {
                *item = block.cast();
                block
            }
            None => null_mut(),
        }
    }

    fn cache_create(&self, owner: *const AActor, bytes: usize, item: &mut *mut c_void) -> *mut u8 {
        let words = bytes.div_ceil(8).max(1);
        let block = Box::leak(vec![0u64; words].into_boxed_slice()).as_mut_ptr() as *mut u8;
        *item = block.cast();
        self.blocks.borrow_mut().insert(owner as usize, block);
        block
    }

    fn cache_flush(&self, owner: *const AActor) {
        self.blocks.borrow_mut().remove(&(owner as usize));
    }

    fn cache_unlock(&self, _item: *mut c_void) {}

    fn warn_missing_link(&self, _mesh: *const USkeletalMesh, _bone: usize) {}

    fn remove_aux(&self, owner: *mut AActor, index: usize) {
        // SAFETY: the port passes a live owner whose AuxAnims holds `index`.
        unsafe {
            // AActor is packed: read and write the TArray by value, never by reference.
            let field = std::ptr::addr_of_mut!((*owner).aux_anims);
            let mut arr = std::ptr::read_unaligned(field);
            let base = arr.data.cast::<*mut AActor>();
            assert!(index < arr.num as usize, "remove_aux index out of range");
            self.destroyed.borrow_mut().push(*base.add(index) as usize);
            copy(
                base.add(index + 1),
                base.add(index),
                arr.num as usize - index - 1,
            );
            arr.num -= 1;
            std::ptr::write_unaligned(field, arr);
        }
    }

    fn debug_pivots(&self, mesh: *mut USkeletalMesh, bases: &[FCoords]) {
        // UnSkeletalMesh.cpp:791-798: DebugPivots/DebugParents sized to SpaceBases.Num().
        // SAFETY: `mesh` is a live USkeletalMesh with a RefSkeleton of at least `bases.len()`.
        unsafe {
            let refs = elems::<FMeshBone>((*mesh).ref_skeleton.data, (*mesh).ref_skeleton.num);
            let pivots: Vec<FVector> = bases.iter().map(|c| c.origin).collect();
            let parents: Vec<i32> = (0..bases.len()).map(|s| refs[s].parent_index).collect();
            (*mesh).debug_pivots = self.array(pivots);
            (*mesh).debug_parents = self.array(parents);
        }
    }

    fn debug_influences(&self, mesh: *mut USkeletalMesh) {
        // UnSkeletalMesh.cpp:804: DebugVerts.Add(FrameVerts). Values are not compared.
        // SAFETY: `mesh` is a live USkeletalMesh.
        unsafe {
            let n = (*mesh).frame_verts.max(0) as usize;
            (*mesh).debug_verts = self.array(vec![0i32; n]);
        }
    }

    fn fail_check(&self, expr: &CStr) {
        panic!("skeletal check() failed: {}", expr.to_string_lossy());
    }
}

/// Mesh block (`put_mesh`). Returns the mesh and the DefaultAnimation presence flag.
fn read_mesh(l: &mut Line, host: &Host) -> (*mut USkeletalMesh, bool) {
    let m: *mut USkeletalMesh = host.keep(zero::<USkeletalMesh>());
    let frame_verts = l.int();
    let weapon_bone_index = l.int();
    let skeletal_depth = l.int();
    let origin = l.vec();
    let rot_origin = (l.int(), l.int(), l.int());
    let scale = l.vec();
    let bounding_box = l.fbox();
    let bounding_sphere = FPlane {
        x: l.bits(),
        y: l.bits(),
        z: l.bits(),
        w: l.bits(),
    };
    let weapon_adjust = l.coords();
    let classic_weapon_coords = l.coords();
    let display_bones = l.int();
    let display_influences = l.int();

    let nb = l.int() as usize;
    let mut bones = Vec::with_capacity(nb);
    for _ in 0..nb {
        let mut b: FMeshBone = zero();
        b.name = l.name();
        b.flags = l.int() as _;
        b.parent_index = l.int() as _;
        b.num_children = l.int() as _;
        b.depth = l.int() as _;
        let pos = l.vec();
        let quat = l.quat4();
        b.bone_pos.place = FPlace { quat, pos };
        b.bone_pos.length = l.bits();
        b.bone_pos.x_size = l.bits();
        b.bone_pos.y_size = l.bits();
        b.bone_pos.z_size = l.bits();
        bones.push(b);
    }

    // Points: streamed for completeness; the port has no use for them.
    let npoints = l.int();
    for _ in 0..npoints {
        let _ = l.vec();
    }

    let nbwi = l.int();
    let mut bwi = Vec::with_capacity(nbwi as usize);
    for _ in 0..nbwi {
        let mut e: VBoneInfIndex = zero();
        e.weight_index = l.int() as _;
        e.number = l.int() as _;
        bwi.push(e);
    }

    let nbw = l.int();
    let mut bw = Vec::with_capacity(nbw as usize);
    for _ in 0..nbw {
        let mut e: VBoneInfluence = zero();
        e.point_index = l.int() as _;
        e.bone_weight = l.int() as _;
        bw.push(e);
    }

    let nlp = l.int();
    let local_points: Vec<FVector> = (0..nlp).map(|_| l.vec()).collect();

    let nfaces = l.int();
    let mut faces = Vec::with_capacity(nfaces as usize);
    for _ in 0..nfaces {
        let mut f: FMeshFace = zero();
        f.i_wedge = [l.int() as _, l.int() as _, l.int() as _];
        faces.push(f);
    }

    let nbb = l.int();
    let bounding_boxes: Vec<FBox> = (0..nbb).map(|_| l.fbox()).collect();
    let default_anim = l.int() != 0;

    // SAFETY: `m` was just allocated and is not aliased.
    unsafe {
        (*m).frame_verts = frame_verts as _;
        (*m).weapon_bone_index = weapon_bone_index as _;
        (*m).skeletal_depth = skeletal_depth as _;
        (*m).origin = origin;
        (*m).rot_origin.pitch = rot_origin.0 as _;
        (*m).rot_origin.yaw = rot_origin.1 as _;
        (*m).rot_origin.roll = rot_origin.2 as _;
        (*m).scale = scale;
        (*m).bounding_box = bounding_box;
        (*m).bounding_sphere = bounding_sphere;
        (*m).weapon_adjust = weapon_adjust;
        (*m).classic_weapon_coords = classic_weapon_coords;
        (*m).display_bones = display_bones as _;
        (*m).display_influences = display_influences as _;
        (*m).ref_skeleton = host.array(bones);
        (*m).bone_weight_idx = host.array(bwi);
        (*m).bone_weights = host.array(bw);
        (*m).local_points = host.array(local_points);
        (*m).faces = host.array(faces);
        (*m).bounding_boxes = host.array(bounding_boxes);
    }
    (m, default_anim)
}

/// Anim block (`put_anim`).
fn read_anim(l: &mut Line, host: &Host) -> *mut UAnimation {
    let a: *mut UAnimation = host.keep(zero::<UAnimation>());

    let nrb = l.int();
    let mut ref_bones = Vec::with_capacity(nrb as usize);
    for _ in 0..nrb {
        let mut b: FNamedBone = zero();
        b.name = l.name();
        b.flags = l.int() as _;
        b.parent_index = l.int() as _;
        ref_bones.push(b);
    }

    let nseq = l.int();
    let mut seqs = Vec::with_capacity(nseq as usize);
    for _ in 0..nseq {
        let mut s: FMeshAnimSeq = zero();
        s.name = l.name();
        s.group = l.name();
        s.start_frame = l.int() as _;
        s.num_frames = l.int() as _;
        s.rate = l.bits();
        seqs.push(s);
    }

    let nmv = l.int();
    let mut moves = Vec::with_capacity(nmv as usize);
    for _ in 0..nmv {
        let mut mv: MotionChunk = zero();
        mv.root_speed3_d = l.vec();
        mv.track_time = l.bits();
        mv.start_bone = l.int() as _;
        mv.flags = l.int() as _;

        let nbi = l.int();
        let bone_indices: Vec<i32> = (0..nbi).map(|_| l.int() as i32).collect();
        mv.bone_indices = host.array(bone_indices);

        let ntr = l.int();
        let mut tracks = Vec::with_capacity(ntr as usize);
        for _ in 0..ntr {
            let mut tr: FAnalogTrack = zero();
            tr.flags = l.int() as _;
            tr.pos_scale = l.bits();
            tr.time_scale = l.bits();
            let k = l.int() as usize;
            let key_quat: Vec<AnimVec> = (0..k).map(|_| l.anim_vec()).collect();
            let key_pos: Vec<AnimVec> = (0..k).map(|_| l.anim_vec()).collect();
            let key_delta: Vec<u8> = (0..k).map(|_| l.int() as u8).collect();
            tr.key_quat = host.tref(key_quat);
            tr.key_pos = host.tref(key_pos);
            tr.key_delta = host.tref(key_delta);
            tracks.push(tr);
        }
        mv.anim_tracks = host.array(tracks);
        moves.push(mv);
    }

    // SAFETY: `a` was just allocated and is not aliased.
    unsafe {
        (*a).ref_bones = host.array(ref_bones);
        (*a).anim_seqs = host.array(seqs);
        (*a).moves = host.array(moves);
    }
    a
}

/// Actor block (`put_actor`). `anim` is the actor's `SkelAnim` (the C++ `new_actor`
/// sets `O->SkelAnim = A` for every actor). `prev` is the preceding actor block (the
/// Owner link target); `mesh` is the mesh the Mesh flag links to.
fn read_actor(
    l: &mut Line,
    host: &Host,
    anim: *mut UAnimation,
    prev: *mut AActor,
    mesh: *mut USkeletalMesh,
) -> *mut AActor {
    let a: *mut AActor = host.keep(zero::<AActor>());
    let anim_sequence = l.name();
    let anim_frame = l.bits();
    // AnimRate: printed by the harness, but AActor has no mirror field for it and no
    // C++ path under test reads it.
    let _anim_rate = l.bits();
    let anim_last = l.bits();
    let tween_rate = l.bits();
    let tween_alpha = l.bits();
    let anim_bone = l.int();
    let b_anim_loop = l.int() != 0;
    let b_anim_move = l.int() != 0;
    let b_anim_transient = l.int() != 0;
    let b_anim_by_owner = l.int() != 0;
    let physics = l.int();
    let b_align_bottom = l.int() != 0;
    let b_collide_world = l.int() != 0;
    let collide_type = l.int();
    let collision_height = l.bits();
    let draw_scale = l.bits();
    let wideness = l.int();
    let pre_pivot = l.vec();
    let saved_pre_pivot_z = l.bits();
    let location = l.vec();
    let rotation = (l.int(), l.int(), l.int());
    let has_owner = l.int() != 0;
    let has_mesh = l.int() != 0;
    assert!(
        !has_owner || !prev.is_null(),
        "Owner flag without a preceding actor"
    );

    // SAFETY: `a` was just allocated and is not aliased.
    unsafe {
        (*a).anim_sequence = anim_sequence;
        (*a).skel_anim = anim.cast();
        (*a).anim_frame = anim_frame;
        (*a).anim_last = anim_last;
        (*a).tween_rate = tween_rate;
        (*a).tween_alpha = tween_alpha;
        (*a).anim_bone = anim_bone as _;
        (*a).set_b_anim_loop(b_anim_loop);
        (*a).set_b_anim_move(b_anim_move);
        (*a).set_b_anim_transient(b_anim_transient);
        (*a).set_b_anim_by_owner(b_anim_by_owner);
        (*a).physics = physics as _;
        (*a).set_b_align_bottom(b_align_bottom);
        (*a).set_b_collide_world(b_collide_world);
        (*a).collide_type = collide_type as _;
        (*a).collision_height = collision_height;
        (*a).draw_scale = draw_scale;
        (*a).wideness = wideness as _;
        (*a).pre_pivot = pre_pivot;
        (*a).saved_pre_pivot_z = saved_pre_pivot_z;
        (*a).location = location;
        (*a).rotation.pitch = rotation.0 as _;
        (*a).rotation.yaw = rotation.1 as _;
        (*a).rotation.roll = rotation.2 as _;
        if has_owner {
            (*a).owner = prev.cast();
        }
        if has_mesh {
            (*a).mesh = mesh.cast();
        }
    }
    a
}

fn replay_apply(text: &str, host: &Host) -> Vec<String> {
    let mut l = Line::new(text);
    // SAFETY: all pointers come from `host` and stay live for this case.
    unsafe {
        let (m, default_anim) = read_mesh(&mut l, host);
        let anim = read_anim(&mut l, host);
        if default_anim {
            (*m).default_animation = anim.cast();
        }
        let o = read_actor(&mut l, host, anim, null_mut(), m);
        let naux = l.int() as usize;
        let mut aux: Vec<*mut AActor> = Vec::with_capacity(naux);
        for _ in 0..naux {
            aux.push(read_actor(&mut l, host, anim, o, m));
        }
        (*o).aux_anims = host.array(aux.clone());
        let root_only = l.int() != 0;
        skeletal::apply_anim(host, m, o, null_mut(), root_only);

        l.expect_bits("AnimFrame", (*o).anim_frame);
        l.expect_bits("AnimLast", (*o).anim_last);
        l.expect_bits("TweenAlpha", (*o).tween_alpha);
        l.expect_int("AuxAnims.Num", (*o).aux_anims.num as i64);
        let destroyed = host.destroyed.borrow().clone();
        for (a, p) in aux.iter().enumerate() {
            let hit = destroyed.contains(&(*p as usize));
            l.expect_int(&format!("aux[{a}].destroyed"), hit as i64);
        }
        let nb = (*m).ref_skeleton.num;
        for b in 0..nb {
            let c = skeletal::get_bone_coords(host, m, o, b);
            l.expect_coords(&format!("bone[{b}]"), c);
        }
        l.end()
    }
}

fn replay_frame(text: &str, host: &Host) -> Vec<String> {
    let mut l = Line::new(text);
    // SAFETY: all pointers come from `host` and stay live for this case.
    unsafe {
        let (m, default_anim) = read_mesh(&mut l, host);
        let anim = read_anim(&mut l, host);
        if default_anim {
            (*m).default_animation = anim.cast();
        }
        let parent = read_actor(&mut l, host, anim, null_mut(), m);
        let o = read_actor(&mut l, host, anim, parent, m);
        let c = l.coords();
        let nv = l.int() as usize;
        let lod_in = l.int() as i32;
        let stride = l.int() as i32;

        let mut lod = lod_in;
        // NaN sentinel in every X/Y/Z slot, as run_frame prefills: unwritten slots stay visible.
        let mut buf = vec![0u8; nv * stride as usize];
        for v in 0..nv {
            for k in 0..3 {
                let at = v * stride as usize + 4 * k;
                buf[at..at + 4].copy_from_slice(&0x7FC0_BEEFu32.to_le_bytes());
            }
        }
        skeletal::get_frame(host, m, buf.as_mut_ptr(), stride, c, o, &mut lod);
        l.expect_int("Lod", lod as i64);

        for v in 0..nv {
            let off = v * stride as usize;
            let f = |k: usize| {
                let at = off + 4 * k;
                f32::from_bits(u32::from_le_bytes(buf[at..at + 4].try_into().unwrap()))
            };
            l.expect_vec(
                &format!("vert[{v}]"),
                FVector {
                    x: f(0),
                    y: f(1),
                    z: f(2),
                },
            );
        }

        if (*m).display_bones != 0 || (*m).display_influences != 0 {
            l.expect_int("DebugPivots.Num", (*m).debug_pivots.num as i64);
            let pivots = elems::<FVector>((*m).debug_pivots.data, (*m).debug_pivots.num);
            for (p, pv) in pivots.iter().enumerate() {
                l.expect_vec(&format!("pivot[{p}]"), *pv);
            }
            l.expect_int("DebugParents.Num", (*m).debug_parents.num as i64);
            let parents = elems::<i32>((*m).debug_parents.data, (*m).debug_parents.num);
            for (p, pa) in parents.iter().enumerate() {
                l.expect_int(&format!("parent[{p}]"), *pa as i64);
            }
            l.expect_int("DebugVerts.Num", (*m).debug_verts.num as i64);
        }
        l.end()
    }
}

fn replay_root(text: &str, host: &Host) -> Vec<String> {
    let mut l = Line::new(text);
    // SAFETY: all pointers come from `host` and stay live for this case.
    unsafe {
        let (m, default_anim) = read_mesh(&mut l, host);
        let anim = read_anim(&mut l, host);
        if default_anim {
            (*m).default_animation = anim.cast();
        }
        let o = read_actor(&mut l, host, anim, null_mut(), m);
        let adj = l.vec();

        let r1 = skeletal::get_root_movement(host, m, o);
        l.expect_vec("RootMovement", r1);
        skeletal::adjust_root_movement(host, o, adj);
        let r2 = skeletal::get_root_movement(host, m, o);
        l.expect_vec("RootMovement.adjusted", r2);
        let cyc = skeletal::anim_cycle_movement(host, m, o);
        l.expect_vec("AnimCycleMovement", cyc);
        l.end()
    }
}

fn replay_bounds(text: &str, host: &Host) -> Vec<String> {
    let mut l = Line::new(text);
    // SAFETY: all pointers come from `host` and stay live for this case.
    unsafe {
        let (m, default_anim) = read_mesh(&mut l, host);
        let anim = read_anim(&mut l, host);
        if default_anim {
            (*m).default_animation = anim.cast();
        }
        // The harness sets O->Mesh = M, so the Mesh flag links to the mesh block.
        let o = read_actor(&mut l, host, anim, null_mut(), m);
        let _exact = l.int(); // GetRenderBoundingBox(O, Exact): the C++ ignores Exact.
        let world = l.int() != 0;
        let new_scale = l.vec();
        let want = l.int();

        let render = skeletal::get_render_bounding_box(host, m, o);
        l.expect_coords("RenderBox", render);
        let collision = skeletal::get_collision_bounding_box(host, m, o, world);
        l.expect_box(
            "CollisionBox",
            collision.min,
            collision.max,
            collision.valid,
        );
        let mesh_coords = skeletal::get_mesh_coords(host, o, m);
        l.expect_coords("MeshCoords", mesh_coords);
        let adjust = skeletal::mesh_adjust(o, m);
        l.expect_vec("MeshAdjust", adjust);

        skeletal::set_scale(m, new_scale);
        l.expect_vec("Scale", (*m).scale);
        l.expect_bits("MeshScaleMax", (*m).mesh_scale_max);

        skeletal::flip_faces(m);
        let faces = elems::<FMeshFace>((*m).faces.data, (*m).faces.num);
        for (f, face) in faces.iter().enumerate() {
            for k in 0..3 {
                l.expect_int(&format!("face[{f}].iWedge[{k}]"), face.i_wedge[k] as i64);
            }
        }

        let bi = skeletal::bone_index(m, (want as i32).to_le_bytes());
        l.expect_int("BoneIndex", bi as i64);
        let nb = (*m).ref_skeleton.num;
        for b in 0..nb {
            let name = skeletal::bone_name(m, b);
            l.expect_int(&format!("BoneName[{b}]"), i32::from_le_bytes(name) as i64);
        }
        l.end()
    }
}

/// Asserts the fixture line count, then that every replayed field matched.
fn assert_replayed(family: &str, cases: usize, expected: usize, misses: &[String]) {
    assert_eq!(cases, expected, "skeletal_{family}.txt line count");
    assert!(
        misses.is_empty(),
        "{family}: {} mismatching fields\n{}",
        misses.len(),
        misses.join("\n")
    );
}

#[test]
fn apply_anim_fixture_replays_bit_exact() {
    let trig = load_trig();
    let mut cases = 0usize;
    let mut misses = Vec::new();
    for (i, line) in APPLY.lines().enumerate() {
        assert_eq!(Line::new(line).case, i as i64, "apply_anim case order");
        misses.extend(replay_apply(line, &Host::new(&trig)));
        cases += 1;
    }
    assert_replayed("apply_anim", cases, APPLY_CASES, &misses);
}

#[test]
fn frame_fixture_replays_bit_exact() {
    let trig = load_trig();
    let mut cases = 0usize;
    let mut misses = Vec::new();
    for (i, line) in FRAME.lines().enumerate() {
        assert_eq!(Line::new(line).case, i as i64, "frame case order");
        misses.extend(replay_frame(line, &Host::new(&trig)));
        cases += 1;
    }
    assert_replayed("frame", cases, FRAME_CASES, &misses);
}

#[test]
fn root_fixture_replays_bit_exact() {
    let trig = load_trig();
    let mut cases = 0usize;
    let mut misses = Vec::new();
    for (i, line) in ROOT.lines().enumerate() {
        assert_eq!(Line::new(line).case, i as i64, "root case order");
        misses.extend(replay_root(line, &Host::new(&trig)));
        cases += 1;
    }
    assert_replayed("root", cases, ROOT_CASES, &misses);
}

#[test]
fn bounds_fixture_replays_bit_exact() {
    let trig = load_trig();
    let mut cases = 0usize;
    let mut misses = Vec::new();
    for (i, line) in BOUNDS.lines().enumerate() {
        assert_eq!(Line::new(line).case, i as i64, "bounds case order");
        misses.extend(replay_bounds(line, &Host::new(&trig)));
        cases += 1;
    }
    assert_replayed("bounds", cases, BOUNDS_CASES, &misses);
}
