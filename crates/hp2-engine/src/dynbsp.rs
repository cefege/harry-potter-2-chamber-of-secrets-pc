//! Port of `Engine/Src/UnDynBsp.cpp` (`FMovingBrushTracker`).
//!
//! The tracker owns the sporadic BSP bookkeeping for moving brushes. Model
//! arrays (surfs, vectors, points, nodes, verts) and actor/brush reads go
//! through a [`DynBspHost`]. The C++ side implements the host from the
//! `Hp2DynBspServices` table, and the fixture harness implements it from the
//! original object. `FPoly` values are owned copies (C++ pointer swaps become
//! moves); node records are copied in and out by index, which is equivalent
//! because no reference is held across a host call that could move them.
//!
//! Divergences from the C++ (documented):
//! - `ZoneMask |= 1 << iZone` with `iZone >= 64` is UB in C++; Rust uses
//!   `wrapping_shl`, which matches the arm64 register-shift semantics.
//! - `appErrorf` paths inside `fpoly` surface as panics, which the FFI guard
//!   converts into an error return.
//! - `TArray::Set` growth is zero-filled by the host (`set_num`), replacing
//!   the C++ `appMemzero` on the new range.
//! - `MoverFlags` / `re-add` diagnostics format `%f` via Rust `{:.6}`, which
//!   matches printf for finite values only.

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicI32, Ordering};

use hp2_abi::engine::generated::{FPoly, FRotator};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert};
use hp2_abi::{FCoords, FPlane, FVector};

use crate::fpoly::{self, SP_BACK, SP_COPLANAR, SP_FRONT, SP_SPLIT, VERTEX_THRESHOLD};
use crate::model::sphere_of_points;
use crate::trace::{NF_IS_BACK, NF_IS_FRONT, NF_IS_NEW, precompute_sphere_filter};

pub const INDEX_NONE: i32 = -1;
/// `MAX_NODES` (UnModel.h).
pub const MAX_NODES: i32 = 65536;
/// `MAXINT`.
pub const MAXINT: i32 = i32::MAX;

/// `NF_NotCsg`.
const NF_NOT_CSG: u8 = 0x01;
/// `NF_ShootThrough`.
const NF_SHOOT_THROUGH: u8 = 0x02;
/// `NF_NotVisBlocking`.
const NF_NOT_VIS_BLOCKING: u8 = 0x04;

const PF_INVISIBLE: u32 = 0x0000_0001;
const PF_MASKED: u32 = 0x0000_0002;
const PF_NOT_SOLID: u32 = 0x0000_0008;
const PF_SPECIAL_LIT: u32 = 0x0010_0000;
const PF_PORTAL: u32 = 0x0400_0000;
/// `PF_NoAddToBSP = PF_EdCut | PF_EdProcessed | PF_Selected | PF_Memorized`.
const PF_NO_ADD_TO_BSP: u32 = 0xC300_0000;

/// Opaque `AActor*` / `UModel*` pointers handed back by the host.
pub type Obj = *mut c_void;

/// Which `UModel` array a host call refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Surfs,
    Vectors,
    Points,
    Nodes,
    Verts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    Init,
    Log,
    Warning,
}

/// Everything the tracker needs from the engine.
pub trait DynBspHost {
    fn num(&self, pool: Pool) -> i32;
    /// Resize to `n`. New elements are zero-filled.
    fn set_num(&mut self, pool: Pool, n: i32);
    fn shrink(&mut self, pool: Pool);
    fn remove(&mut self, pool: Pool, index: i32, count: i32);
    /// Raw element storage of a model array (valid until the next resize).
    fn surfs_mut(&mut self) -> &mut [FBspSurf];
    fn vectors_mut(&mut self) -> &mut [FVector];
    fn points_mut(&mut self) -> &mut [FVector];
    fn verts_mut(&mut self) -> &mut [FVert];
    fn nodes_mut(&mut self) -> &mut [FBspNode];

    fn level_actor_count(&self) -> i32;
    fn level_actor(&self, i: i32) -> Obj;
    fn level_full_name(&self) -> String;

    fn is_moving_brush(&self, actor: Obj) -> bool;
    fn is_mover(&self, actor: Obj) -> bool;
    fn actor_name(&self, actor: Obj) -> String;
    fn location(&self, actor: Obj) -> FVector;
    fn rotation(&self, actor: Obj) -> FRotator;
    fn pre_pivot(&self, actor: Obj) -> FVector;
    fn saved_pos(&self, actor: Obj) -> FVector;
    fn set_saved_pos(&mut self, actor: Obj, v: FVector);
    fn saved_rot(&self, actor: Obj) -> FRotator;
    fn set_saved_rot(&mut self, actor: Obj, r: FRotator);
    fn assimilated(&self, actor: Obj) -> bool;
    fn set_assimilated(&mut self, actor: Obj, v: bool);
    fn special_lit(&self, actor: Obj) -> bool;
    fn brush(&self, actor: Obj) -> Obj;
    /// `ABrush::BuildCoords(&Coords, NULL)`: returns (point, vector, orientation).
    fn build_coords(&mut self, actor: Obj) -> (FCoords, FCoords, f32);

    fn brush_mover_link(&self, brush: Obj) -> i32;
    fn set_brush_mover_link(&mut self, brush: Obj, v: i32);
    /// `Brush->Polys->Element` as a raw slice; valid for the whole tracker call.
    fn brush_polys(&mut self, brush: Obj) -> &mut [FPoly];

    fn debug(&mut self, kind: LogKind, msg: &str);
}

#[derive(Clone, Copy, Debug)]
struct FActorIndex {
    actor: Obj,
    index: i32,
}

impl Default for FActorIndex {
    fn default() -> Self {
        Self {
            actor: std::ptr::null_mut(),
            index: 0,
        }
    }
}

#[derive(Clone, Copy)]
enum PrevLink {
    Brush(Obj),
    Node(i32),
}

static MOVER_DROPPED_POLYS: AtomicI32 = AtomicI32::new(0);

static MOVER_DEBUG: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("HP2_MOVER_DEBUG")
        .map(|v| v.starts_with('1'))
        .unwrap_or(false)
});

fn mover_debug_enabled() -> bool {
    *MOVER_DEBUG
}

fn zero_poly() -> FPoly {
    // SAFETY: `FPoly` is a plain-old-data mirror of the C++ struct; all-zero is
    // the state `new(GMem)FPoly` produces (zero-filled memory in the allocator).
    unsafe { std::mem::zeroed() }
}

fn res<T>(r: Result<T, String>) -> T {
    match r {
        Ok(v) => v,
        // `appErrorf` in the original; the FFI guard turns this into an error.
        Err(e) => panic!("{e}"),
    }
}

fn is_csg(n: &FBspNode) -> bool {
    n.num_vertices > 0 && (n.node_flags & (NF_IS_NEW | NF_NOT_CSG)) == 0
}

fn xyz(v: FPlane) -> FVector {
    FVector {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

fn rot_eq(a: FRotator, b: FRotator) -> bool {
    a.pitch == b.pitch && a.yaw == b.yaw && a.roll == b.roll
}

/// `FVector::operator|` on the xyz parts (FPlane inherits it).
fn dot3(a: FVector, b: FVector) -> f32 {
    fpoly::dot(a, b)
}

pub struct DynBsp<H: DynBspHost> {
    host: H,
    i_node_rover: i32,
    i_vert_rover: i32,
    i_first_roving_point: i32,
    i_original_top_surf: i32,
    i_original_top_vector: i32,
    i_original_top_point: i32,
    i_original_top_node: i32,
    i_original_top_vert: i32,
    add_actor: Obj,
    fpoly_normal: FVector,
    prev_link: PrevLink,
    /// `GroupActors` in push order; the C++ list head is the last element.
    group: Vec<Obj>,
    i_free_points: i32,
    point_owners: Vec<Obj>,
    point_free_links: Vec<i32>,
    node_maps: Vec<FActorIndex>,
    vert_pool_owners: Vec<Obj>,
}

impl<H: DynBspHost> DynBsp<H> {
    /// `FMovingBrushTracker::FMovingBrushTracker`.
    pub fn new(host: H) -> Self {
        let i_original_top_surf = host.num(Pool::Surfs);
        let i_original_top_vector = host.num(Pool::Vectors);
        let i_original_top_point = host.num(Pool::Points);
        let i_original_top_node = host.num(Pool::Nodes);
        let i_original_top_vert = host.num(Pool::Verts);
        let mut s = Self {
            host,
            i_node_rover: 0,
            i_vert_rover: 0,
            i_first_roving_point: 0,
            i_original_top_surf,
            i_original_top_vector,
            i_original_top_point,
            i_original_top_node,
            i_original_top_vert,
            add_actor: std::ptr::null_mut(),
            fpoly_normal: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            prev_link: PrevLink::Node(INDEX_NONE),
            group: Vec::new(),
            i_free_points: INDEX_NONE,
            point_owners: Vec::new(),
            point_free_links: Vec::new(),
            node_maps: Vec::new(),
            vert_pool_owners: Vec::new(),
        };

        // Setup all brushes.
        for i in 0..s.host.level_actor_count() {
            let a = s.host.level_actor(i);
            if !a.is_null() && s.host.is_moving_brush(a) {
                s.setup_actor_brush(a);
            }
        }

        // Save roving info.
        s.i_first_roving_point = s.host.num(Pool::Points);

        // Shrink tables.
        s.host.shrink(Pool::Vectors);
        s.host.shrink(Pool::Surfs);

        // Allocate changing structures.
        s.i_node_rover = s.expand_db(Pool::Nodes, 512, MAX_NODES);
        s.i_vert_rover = s.expand_db(Pool::Verts, 512, MAXINT);
        let node_count = s.host.num(Pool::Nodes) - s.i_original_top_node;
        s.node_maps = vec![FActorIndex::default(); node_count.max(0) as usize];
        let vert_count = s.host.num(Pool::Verts) - s.i_original_top_vert;
        s.vert_pool_owners = vec![std::ptr::null_mut(); vert_count.max(0) as usize];

        // Now setup and update all brushes.
        s.update(std::ptr::null_mut());

        let name = s.host.level_full_name();
        s.host.debug(
            LogKind::Init,
            &format!("Initialized moving brush tracker for {name}"),
        );
        s
    }

    /// `~FMovingBrushTracker`. Consumes the tracker and returns the host.
    pub fn shutdown(mut self) -> H {
        let h = &mut self.host;
        h.remove(
            Pool::Surfs,
            self.i_original_top_surf,
            h.num(Pool::Surfs) - self.i_original_top_surf,
        );
        h.remove(
            Pool::Points,
            self.i_original_top_point,
            h.num(Pool::Points) - self.i_original_top_point,
        );
        h.remove(
            Pool::Vectors,
            self.i_original_top_vector,
            h.num(Pool::Vectors) - self.i_original_top_vector,
        );
        h.remove(
            Pool::Nodes,
            self.i_original_top_node,
            h.num(Pool::Nodes) - self.i_original_top_node,
        );
        h.remove(
            Pool::Verts,
            self.i_original_top_vert,
            h.num(Pool::Verts) - self.i_original_top_vert,
        );
        let num = h.num(Pool::Nodes);
        for n in h.nodes_mut().iter_mut().take(num as usize) {
            if n.i_front >= num {
                n.i_front = INDEX_NONE;
            }
            if n.i_back >= num {
                n.i_back = INDEX_NONE;
            }
            if n.i_plane >= num {
                n.i_plane = INDEX_NONE;
            }
        }
        let name = self.host.level_full_name();
        self.host.debug(
            LogKind::Init,
            &format!("Shut down moving brush tracker for {name}"),
        );
        self.host
    }

    /// `FMovingBrushTracker::CountBytes`. Returns (num_bytes, max_bytes) per array
    /// in the order NodeMaps, VertPoolOwners, PointOwners, PointFreeLinks.
    pub fn count_bytes(&self) -> [(usize, usize); 4] {
        let f = |len: usize, cap: usize, elem: usize| (len * elem, cap * elem);
        [
            f(
                self.node_maps.len(),
                self.node_maps.capacity(),
                size_of::<FActorIndex>(),
            ),
            f(
                self.vert_pool_owners.len(),
                self.vert_pool_owners.capacity(),
                size_of::<Obj>(),
            ),
            f(
                self.point_owners.len(),
                self.point_owners.capacity(),
                size_of::<Obj>(),
            ),
            f(
                self.point_free_links.len(),
                self.point_free_links.capacity(),
                size_of::<i32>(),
            ),
        ]
    }

    /// The host the tracker drives (model state, for replay tests and diagnostics).
    pub fn host(&self) -> &H {
        &self.host
    }

    /// Mutable host access (replay tests apply pose and actor writes here).
    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    /// `SurfIsDynamic`.
    pub fn surf_is_dynamic(&self, i_surf: i32) -> bool {
        i_surf >= self.i_original_top_surf
    }

    /// `Flush`.
    pub fn flush(&mut self, actor: Obj) {
        if self.host.is_moving_brush(actor) {
            self.flush_actor_brush(actor);
        }
    }

    /// `Update`. `in_actor` may be null (update every moving brush).
    pub fn update(&mut self, in_actor: Obj) {
        let actor = if !in_actor.is_null() && self.host.is_mover(in_actor) {
            in_actor
        } else {
            std::ptr::null_mut()
        };
        self.group.clear();
        if actor.is_null() {
            // Update all actor brushes.
            for i in 0..self.host.level_actor_count() {
                let a = self.host.level_actor(i);
                if !a.is_null() && self.host.is_moving_brush(a) {
                    self.add_group_actor(a);
                }
            }
        } else if self.host.saved_pos(actor) != self.host.location(actor)
            || !rot_eq(self.host.saved_rot(actor), self.host.rotation(actor))
        {
            // Update the single brush.
            if mover_debug_enabled() {
                let name = self.host.actor_name(actor);
                let l = self.host.location(actor);
                let r = self.host.rotation(actor);
                self.host.debug(
                    LogKind::Log,
                    &format!(
                        "MoverTracker: re-add {name} at ({:.6},{:.6},{:.6}) rot({},{},{})",
                        l.x, l.y, l.z, r.pitch, r.yaw, r.roll
                    ),
                );
            }
            self.add_group_actor(actor);
        }

        // Make a complete list of all actors that need flushing. The C++ walk
        // starts at the list head, so only actors present now are visited.
        let n = self.group.len();
        for idx in (0..n).rev() {
            let a = self.group[idx];
            let brush = self.host.brush(a);
            let mut i_node = self.host.brush_mover_link(brush);
            while i_node != INDEX_NONE {
                let node = self.node(i_node);
                self.force_group_flush(i_node);
                i_node = node.i_render_bound;
            }
        }

        // Recursively flush all affected brushes.
        for idx in (0..self.group.len()).rev() {
            let a = self.group[idx];
            self.flush_actor_brush(a);
        }

        // Add all moving brush sporadic data to the world.
        for idx in (0..self.group.len()).rev() {
            let actor = self.group[idx];
            self.host.set_assimilated(actor, false);
            self.add_actor = actor;
            let brush = self.host.brush(actor);

            // Build transformed polys.
            let (point_xf, vector_xf, orientation) = self.host.build_coords(actor);
            let pre = self.host.pre_pivot(actor);
            let location = self.host.location(actor);
            let src: Vec<FPoly> = self.host.brush_polys(brush).to_vec();
            let total: i32 = src.iter().map(|p| p.num_vertices).sum();
            let mut transformed = src;
            let mut points: Vec<FVector> = Vec::with_capacity(total.max(0) as usize);
            for poly in transformed.iter_mut() {
                fpoly::transform(poly, &point_xf, &vector_xf, pre, location, orientation);
                for j in 0..poly.num_vertices.max(0) as usize {
                    points.push(poly.vertex[j]);
                }
            }
            let sphere = sphere_of_points(points.iter().copied());
            precompute_sphere_filter(self.host.nodes_mut(), sphere);

            // Go through list of all brush FPolys and update their corresponding Bsp surfaces.
            self.prev_link = PrevLink::Brush(brush);
            for poly in transformed.iter() {
                let i_surf = poly.i_link;
                let surf = self.surf(i_surf);
                self.fpoly_normal = poly.normal;
                self.set_point(surf.p_base, poly.base);
                self.set_vector(surf.v_normal, poly.normal);
                self.set_vector(surf.v_texture_u, poly.texture_u);
                self.set_vector(surf.v_texture_v, poly.texture_v);

                // Filter the brush's FPoly through the Bsp, creating new sporadic Bsp nodes
                // (and their corresponding VertPools and points) for all outside leaves.
                if self.host.num(Pool::Nodes) > 0 {
                    self.filter_fpoly(i_surf, 0, INDEX_NONE, *poly, true);
                }
            }
            self.set_prev_link(INDEX_NONE);

            // Tag all newly-added nodes as non-new.
            let mut i_node = self.host.brush_mover_link(brush);
            while i_node != INDEX_NONE {
                let mut node = self.node(i_node);
                let owner = self.node_maps[self.map_index(i_node)].actor;
                debug_assert!(owner == actor);
                node.node_flags &= !NF_IS_NEW;
                self.set_node(i_node, node);
                i_node = node.i_render_bound;
            }
        }
    }

    // ---- element access -------------------------------------------------

    fn node(&mut self, i: i32) -> FBspNode {
        self.host.nodes_mut()[i as usize]
    }

    fn set_node(&mut self, i: i32, n: FBspNode) {
        self.host.nodes_mut()[i as usize] = n;
    }

    fn map_index(&self, i_node: i32) -> usize {
        (i_node - self.i_original_top_node) as usize
    }

    fn surf(&mut self, i: i32) -> FBspSurf {
        self.surf_slice()[i as usize]
    }

    fn surf_slice(&mut self) -> &mut [FBspSurf] {
        self.host.surfs_mut()
    }

    fn set_point(&mut self, i: i32, v: FVector) {
        self.host.points_mut()[i as usize] = v;
    }

    fn set_vector(&mut self, i: i32, v: FVector) {
        self.host.vectors_mut()[i as usize] = v;
    }

    fn vert(&mut self, i: i32) -> FVert {
        self.host.verts_mut()[i as usize]
    }

    fn set_vert(&mut self, i: i32, v: FVert) {
        self.host.verts_mut()[i as usize] = v;
    }

    fn set_prev_link(&mut self, v: i32) {
        match self.prev_link {
            PrevLink::Brush(b) => self.host.set_brush_mover_link(b, v),
            PrevLink::Node(i) => {
                let mut n = self.node(i);
                n.i_render_bound = v;
                self.set_node(i, n);
            }
        }
    }

    /// `ExpandDb`.
    fn expand_db(&mut self, pool: Pool, slack: i32, max_count: i32) -> i32 {
        let n = self.host.num(pool);
        let add = (2 * n + slack).min(max_count) - n;
        if add > 0 {
            self.host.set_num(pool, n + add);
        } else {
            self.host.shrink(pool);
        }
        n
    }

    // ---- allocation ------------------------------------------------------

    /// `NewThing` for `NodeMaps`.
    fn new_node_index(&mut self, owner: FActorIndex) -> i32 {
        let num_things = self.i_original_top_node;
        let max_things = self.host.num(Pool::Nodes);
        let start = self.i_node_rover;
        let mut top = self.i_node_rover;
        while top < max_things {
            let slot = (top - num_things) as usize;
            if self.node_maps[slot].actor.is_null() {
                self.node_maps[slot] = owner;
                self.i_node_rover = top;
                return top;
            }
            top += 1;
        }
        top = num_things;
        while top < start {
            let slot = (top - num_things) as usize;
            if self.node_maps[slot].actor.is_null() {
                self.node_maps[slot] = owner;
                self.i_node_rover = top;
                return top;
            }
            top += 1;
        }
        self.i_node_rover = top;
        INDEX_NONE
    }

    /// `NewThang` for points.
    fn new_point(&mut self, owner: Obj) -> i32 {
        let first = self.i_first_roving_point;
        let index = if self.i_free_points != INDEX_NONE {
            let index = self.i_free_points;
            self.i_free_points = self.point_free_links[(index - first) as usize];
            index
        } else {
            let index = self.host.num(Pool::Points);
            self.host.set_num(Pool::Points, index + 1);
            self.point_owners.push(std::ptr::null_mut());
            self.point_free_links.push(INDEX_NONE);
            index
        };
        let hi = (index - first) as usize;
        self.point_owners[hi] = owner;
        self.point_free_links[hi] = INDEX_NONE;
        index
    }

    /// `FreeThang` for points.
    fn free_point(&mut self, index: i32) {
        let hi = (index - self.i_first_roving_point) as usize;
        self.point_owners[hi] = std::ptr::null_mut();
        self.point_free_links[hi] = self.i_free_points;
        self.i_free_points = index;
    }

    /// `NewVertPoolIndex`.
    fn new_vert_pool_index(&mut self, actor: Obj, num_verts: i32) -> i32 {
        let i_start = self.i_vert_rover;
        let mut num_free = 0;
        let mut owner_index = self.i_vert_rover - self.i_original_top_vert;
        while self.i_vert_rover + num_verts < self.host.num(Pool::Verts) {
            if self.vert_pool_owners[owner_index as usize].is_null() {
                num_free += 1;
                if num_free >= num_verts {
                    while num_free > 0 {
                        self.vert_pool_owners[owner_index as usize] = actor;
                        owner_index -= 1;
                        num_free -= 1;
                    }
                    return self.i_vert_rover + 1 - num_verts;
                }
            } else {
                num_free = 0;
            }
            self.i_vert_rover += 1;
            owner_index += 1;
        }

        self.i_vert_rover = self.i_original_top_vert;
        num_free = 0;
        owner_index = 0;
        while self.i_vert_rover + num_verts < i_start {
            if self.vert_pool_owners[owner_index as usize].is_null() {
                num_free += 1;
                if num_free >= num_verts {
                    while num_free > 0 {
                        self.vert_pool_owners[owner_index as usize] = actor;
                        owner_index -= 1;
                        num_free -= 1;
                    }
                    return self.i_vert_rover + 1 - num_verts;
                }
            } else {
                num_free = 0;
            }
            self.i_vert_rover += 1;
            owner_index += 1;
        }
        INDEX_NONE
    }

    /// `FreeVertPoolIndex`.
    fn free_vert_pool_index(&mut self, i: i32, num_verts: i32) {
        for j in 0..num_verts {
            let slot = (j.wrapping_add(i).wrapping_sub(self.i_original_top_vert)) as usize;
            self.vert_pool_owners[slot] = std::ptr::null_mut();
        }
    }

    fn mover_drop_warning(&mut self, pool: &str, i_surf: i32) {
        let n = MOVER_DROPPED_POLYS.fetch_add(1, Ordering::Relaxed) + 1;
        if n <= 32 || n % 1024 == 0 {
            let name = self.host.actor_name(self.add_actor);
            self.host.debug(
                LogKind::Warning,
                &format!(
                    "FMovingBrushTracker: {pool} exhausted adding brush {name} surf {i_surf} - mover geometry dropped ({n} total)"
                ),
            );
        }
    }

    // ---- per-brush operations ---------------------------------------------

    /// `SetupActorBrush`.
    fn setup_actor_brush(&mut self, actor: Obj) {
        let brush = self.host.brush(actor);
        self.host.set_brush_mover_link(brush, INDEX_NONE);
        self.host.set_saved_pos(
            actor,
            FVector {
                x: -1.0,
                y: -1.0,
                z: -1.0,
            },
        );
        self.host.set_saved_rot(
            actor,
            FRotator {
                pitch: 123,
                yaw: 456,
                roll: 789,
            },
        );
        self.host.set_assimilated(actor, false);
        let special_lit = self.host.special_lit(actor);
        let n = self.host.brush_polys(brush).len();
        for i in 0..n {
            if mover_debug_enabled() && i == 0 {
                let polys = self.host.brush_polys(brush);
                let mut seen: Vec<u32> = Vec::new();
                for p in polys.iter() {
                    let f = p.poly_flags;
                    if !seen.contains(&f) && seen.len() < 16 {
                        seen.push(f);
                    }
                }
                let flags: Vec<String> = seen.iter().map(|f| format!("0x{f:08x}")).collect();
                let name = self.host.actor_name(actor);
                self.host.debug(
                    LogKind::Log,
                    &format!("MoverFlags: {name} polys={n} flags=[{}]", flags.join(",")),
                );
            }
            // Create new surface elements.
            let poly = self.host.brush_polys(brush)[i];
            let i_surf = self.host.num(Pool::Surfs);
            self.host.set_num(Pool::Surfs, i_surf + 1);
            let v_normal = self.add_zeroed(Pool::Vectors);
            let v_texture_u = self.add_zeroed(Pool::Vectors);
            let v_texture_v = self.add_zeroed(Pool::Vectors);
            let p_base = self.add_zeroed(Pool::Points);
            let mut surf = self.surf(i_surf);
            surf.v_normal = v_normal;
            surf.v_texture_u = v_texture_u;
            surf.v_texture_v = v_texture_v;
            surf.p_base = p_base;
            surf.i_light_map = poly.i_brush_poly;
            surf.texture = poly.texture;
            surf.pan_u = poly.pan_u;
            surf.pan_v = poly.pan_v;
            surf.i_brush_poly = i as i32;
            surf.actor = actor;
            surf.poly_flags = poly.poly_flags & !PF_NO_ADD_TO_BSP;
            if special_lit {
                surf.poly_flags |= PF_SPECIAL_LIT;
            }
            self.surf_slice()[i_surf as usize] = surf;
            self.host.brush_polys(brush)[i].i_link = i_surf;
        }
    }

    /// `TArray::AddZeroed`: appends a zeroed element and returns its index.
    fn add_zeroed(&mut self, pool: Pool) -> i32 {
        let index = self.host.num(pool);
        self.host.set_num(pool, index + 1);
        index
    }

    /// `AddPolyFragment`.
    fn add_poly_fragment(
        &mut self,
        i_add_surf: i32,
        i_parent0: i32,
        i_coplanar_parent: i32,
        is_front0: i32,
        ed: &FPoly,
    ) {
        let i_first_parent = i_parent0;
        let zone_front = is_front0 as usize;
        let mut i_parent = i_parent0;
        let mut is_front = is_front0;

        // If this node is meant to be added as a coplanar, handle it now.
        let mut different = false;
        if i_coplanar_parent != INDEX_NONE {
            different = i_parent != i_coplanar_parent;
            i_parent = i_coplanar_parent;
            is_front = 2;
            let mut parent = self.node(i_parent);
            while parent.i_plane != INDEX_NONE {
                i_parent = parent.i_plane;
                parent = self.node(i_parent);
            }
        }

        // Create a new sporadic node.
        let i_node = self.new_node_index(FActorIndex {
            actor: self.add_actor,
            index: i_parent,
        });
        if i_node == INDEX_NONE {
            self.mover_drop_warning("node pool", i_add_surf);
            return;
        }
        let parent = self.node(i_parent);
        let first_parent = self.node(i_first_parent);

        // Set node's info.
        let mut node = FBspNode {
            plane: FPlane {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            zone_mask: parent.zone_mask,
            i_vert_pool: INDEX_NONE,
            i_surf: i_add_surf,
            i_back: INDEX_NONE,
            i_front: INDEX_NONE,
            i_plane: INDEX_NONE,
            i_collision_bound: INDEX_NONE,
            i_render_bound: INDEX_NONE,
            i_zone: [0, 0],
            num_vertices: ed.num_vertices as u8,
            node_flags: NF_IS_NEW,
            i_leaf: [0, 0],
        };
        let base_dot = dot3(ed.base, ed.normal);
        node.plane = FPlane {
            x: ed.normal.x,
            y: ed.normal.y,
            z: ed.normal.z,
            w: base_dot,
        };

        // Set node flags.
        if ed.poly_flags & PF_NOT_SOLID != 0 {
            node.node_flags |= NF_NOT_CSG;
        }
        if ed.poly_flags & (PF_INVISIBLE | PF_PORTAL) != 0 {
            node.node_flags |= NF_NOT_VIS_BLOCKING;
        }
        if ed.poly_flags & PF_MASKED != 0 {
            node.node_flags |= NF_SHOOT_THROUGH;
        }

        // Set leaf and zone info.
        if i_coplanar_parent == INDEX_NONE || different {
            node.i_zone = [
                first_parent.i_zone[zone_front],
                first_parent.i_zone[zone_front],
            ];
            node.i_leaf = [
                first_parent.i_leaf[zone_front],
                first_parent.i_leaf[zone_front],
            ];
        } else {
            let is_flipped = (dot3(xyz(node.plane), xyz(parent.plane)) < 0.0) as usize;
            node.i_zone[0] = parent.i_zone[is_flipped];
            node.i_leaf[0] = parent.i_leaf[is_flipped];
            node.i_zone[1] = parent.i_zone[1 - is_flipped];
            node.i_leaf[1] = parent.i_leaf[1 - is_flipped];
        }
        self.set_node(i_node, node);
        let mut fp = self.node(i_first_parent);
        fp.zone_mask |=
            1u64.wrapping_shl(node.i_zone[0] as u32) | 1u64.wrapping_shl(node.i_zone[1] as u32);
        self.set_node(i_first_parent, fp);

        // Allocate this node's vertex pool and vertices.
        let i_vert_pool = self.new_vert_pool_index(self.add_actor, ed.num_vertices);
        node.i_vert_pool = i_vert_pool;
        self.set_node(i_node, node);
        if i_vert_pool == INDEX_NONE {
            self.mover_drop_warning("vertex pool", i_add_surf);
            let slot = self.map_index(i_node);
            self.node_maps[slot] = FActorIndex::default();
            return;
        }

        // Add vertices.
        for i in 0..ed.num_vertices {
            let p = self.new_point(self.add_actor);
            if p == INDEX_NONE {
                self.mover_drop_warning("point pool", i_add_surf);
                for j in (0..i).rev() {
                    let pv = self.vert(i_vert_pool + j).p_vertex;
                    self.free_point(pv);
                }
                let slot = self.map_index(i_node);
                self.node_maps[slot] = FActorIndex::default();
                return;
            }
            self.set_vert(
                i_vert_pool + i,
                FVert {
                    p_vertex: p,
                    i_side: INDEX_NONE,
                },
            );
            self.set_point(p, ed.vertex[i as usize]);
        }

        // Update linked lists. Child slot 0 = iBack, 1 = iFront, 2 = iPlane (iChild[] layout).
        let mut parent_node = self.node(i_parent);
        match is_front {
            0 => parent_node.i_back = i_node,
            1 => parent_node.i_front = i_node,
            _ => parent_node.i_plane = i_node,
        }
        self.set_node(i_parent, parent_node);
        self.set_prev_link(i_node);
        self.prev_link = PrevLink::Node(i_node);
    }

    /// `FilterFPoly`.
    fn filter_fpoly(
        &mut self,
        i_add_surf: i32,
        i_node0: i32,
        i_coplanar0: i32,
        ed0: FPoly,
        outside0: bool,
    ) {
        let mut i_node = i_node0;
        let mut i_coplanar = i_coplanar0;
        let mut ed = ed0;
        let mut outside = outside0;
        loop {
            // FilterLoop:
            if ed.num_vertices >= VERTEX_THRESHOLD {
                // Must split to avoid vertex overflow.
                let mut front = zero_poly();
                res(fpoly::split_in_half(&mut ed, &mut front));
                self.filter_fpoly(i_add_surf, i_node, i_coplanar, front, outside);
            }
            let node = self.node(i_node);
            let mut temp_front = zero_poly();
            let mut temp_back = zero_poly();
            let split = if node.node_flags & NF_IS_FRONT != 0 {
                SP_FRONT
            } else if node.node_flags & NF_IS_BACK != 0 {
                SP_BACK
            } else {
                res(fpoly::split_with_plane_fast(
                    &ed,
                    node.plane,
                    Some((&mut temp_front, &mut temp_back)),
                ))
            };
            let at_back = match split {
                SP_COPLANAR | SP_FRONT => {
                    if split == SP_COPLANAR && dot3(xyz(node.plane), self.fpoly_normal) >= 0.0 {
                        i_coplanar = i_node;
                    }
                    // Front:
                    let node = self.node(i_node);
                    outside = outside || is_csg(&node);
                    if node.i_front != INDEX_NONE {
                        i_node = node.i_front;
                        continue;
                    } else if outside {
                        self.add_poly_fragment(i_add_surf, i_node, i_coplanar, 1, &ed);
                    }
                    return;
                }
                SP_BACK => true,
                SP_SPLIT => {
                    // Handle front fragment.
                    let node = self.node(i_node);
                    if node.i_front != INDEX_NONE {
                        self.filter_fpoly(
                            i_add_surf,
                            node.i_front,
                            i_coplanar,
                            temp_front,
                            outside || is_csg(&node),
                        );
                    } else if outside || is_csg(&node) {
                        self.add_poly_fragment(i_add_surf, i_node, i_coplanar, 1, &temp_front);
                    }
                    // Handle back fragment.
                    ed = temp_back;
                    true
                }
                _ => return,
            };
            if at_back {
                // Back:
                let node = self.node(i_node);
                outside = outside && !is_csg(&node);
                if node.i_back != INDEX_NONE {
                    i_node = node.i_back;
                    continue;
                } else if outside {
                    self.add_poly_fragment(i_add_surf, i_node, i_coplanar, 0, &ed);
                }
                return;
            }
        }
    }

    fn add_group_actor(&mut self, actor: Obj) {
        if !self.host.assimilated(actor) {
            self.host.set_assimilated(actor, true);
            self.group.push(actor);
            let loc = self.host.location(actor);
            let rot = self.host.rotation(actor);
            self.host.set_saved_pos(actor, loc);
            self.host.set_saved_rot(actor, rot);
        }
    }

    /// `ForceGroupFlush`.
    fn force_group_flush(&mut self, i_node: i32) {
        let mut node = self.node(i_node);
        if node.node_flags & NF_IS_NEW == 0 {
            node.node_flags |= NF_IS_NEW;
            self.set_node(i_node, node);
            if node.i_front != INDEX_NONE {
                self.force_group_flush(node.i_front);
            }
            if node.i_back != INDEX_NONE {
                self.force_group_flush(node.i_back);
            }
            if node.i_plane != INDEX_NONE {
                self.force_group_flush(node.i_plane);
            }
            let owner = self.node_maps[self.map_index(i_node)].actor;
            if !owner.is_null() {
                self.add_group_actor(owner);
            }
        }
    }

    /// `FlushActorBrush`.
    fn flush_actor_brush(&mut self, actor: Obj) {
        let brush = self.host.brush(actor);
        let mut i_node = self.host.brush_mover_link(brush);
        while i_node != INDEX_NONE {
            let node = self.node(i_node);
            let i_parent = self.node_maps[self.map_index(i_node)].index;
            let mut parent = self.node(i_parent);

            // Remove references to this node from its parents.
            if parent.i_front == i_node {
                parent.i_front = INDEX_NONE;
            } else if parent.i_back == i_node {
                parent.i_back = INDEX_NONE;
            } else if parent.i_plane == i_node {
                parent.i_plane = INDEX_NONE;
            }
            self.set_node(i_parent, parent);

            // Free all sporadic data.
            for j in 0..node.num_vertices as i32 {
                let pv = self.vert(node.i_vert_pool + j).p_vertex;
                self.free_point(pv);
            }
            self.free_vert_pool_index(node.i_vert_pool, node.num_vertices as i32);
            let slot = self.map_index(i_node);
            self.node_maps[slot] = FActorIndex::default();
            i_node = node.i_render_bound;
        }
        self.host.set_brush_mover_link(brush, INDEX_NONE);
    }
}

/// C services table for the tracker (`Hp2DynBspServices.h`). Field order must
/// match the C++ struct. Model arrays are addressed by `Pool` as `u32`
/// (Surfs=0, Vectors=1, Points=2, Nodes=3, Verts=4).
#[repr(C)]
pub struct Hp2DynBspServices {
    pub array_num: unsafe extern "C" fn(model: *mut c_void, pool: u32) -> i32,
    pub array_data: unsafe extern "C" fn(model: *mut c_void, pool: u32) -> *mut c_void,
    pub array_set_num: unsafe extern "C" fn(model: *mut c_void, pool: u32, n: i32),
    pub array_shrink: unsafe extern "C" fn(model: *mut c_void, pool: u32),
    pub array_remove: unsafe extern "C" fn(model: *mut c_void, pool: u32, index: i32, count: i32),
    pub level_actor_count: unsafe extern "C" fn(level: *mut c_void) -> i32,
    pub level_actor: unsafe extern "C" fn(level: *mut c_void, i: i32) -> *mut c_void,
    pub level_full_name: unsafe extern "C" fn(level: *mut c_void) -> *const c_char,
    pub actor_is_moving_brush: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_is_mover: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_name: unsafe extern "C" fn(actor: *mut c_void) -> *const c_char,
    pub actor_location: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    pub actor_rotation: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    pub actor_pre_pivot: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    pub actor_saved_pos: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    pub set_actor_saved_pos: unsafe extern "C" fn(actor: *mut c_void, v: *const c_void),
    pub actor_saved_rot: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    pub set_actor_saved_rot: unsafe extern "C" fn(actor: *mut c_void, v: *const c_void),
    pub actor_assimilated: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub set_actor_assimilated: unsafe extern "C" fn(actor: *mut c_void, v: i32),
    pub actor_special_lit: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_brush: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `ABrush::BuildCoords(&Point, &Vector)`: returns the orientation.
    pub actor_build_coords:
        unsafe extern "C" fn(actor: *mut c_void, point: *mut c_void, vector: *mut c_void) -> f32,
    pub brush_mover_link: unsafe extern "C" fn(brush: *mut c_void) -> i32,
    pub set_brush_mover_link: unsafe extern "C" fn(brush: *mut c_void, v: i32),
    /// Writes `Brush->Polys->Element` storage to `out`; returns the count.
    pub brush_polys: unsafe extern "C" fn(brush: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `FArchive::CountBytes(num, max)`.
    pub archive_count_bytes: unsafe extern "C" fn(ar: *mut c_void, num: usize, max: usize),
    /// kind: 0 = NAME_Init, 1 = NAME_Log, 2 = NAME_Warning.
    pub debug: unsafe extern "C" fn(kind: u32, msg: *const c_char),
}

/// `DynBspHost` backed by the C services table and one `ULevel`/`UModel` pair.
pub struct CHost {
    svc: &'static Hp2DynBspServices,
    level: *mut c_void,
    model: *mut c_void,
}

impl CHost {
    /// # Safety
    /// `svc` must outlive the tracker (it is a C++ global), and `level`/`model`
    /// must stay valid for the tracker's lifetime.
    pub unsafe fn new(
        svc: &'static Hp2DynBspServices,
        level: *mut c_void,
        model: *mut c_void,
    ) -> Self {
        Self { svc, level, model }
    }

    fn pool_slice<T>(&mut self, pool: Pool) -> &mut [T] {
        let n = self.num(pool).max(0) as usize;
        // SAFETY: the model array holds `num(pool)` live elements of `T`; no
        // resize happens while this borrow is alive (tracker methods never
        // hold two pool borrows at once).
        unsafe {
            let p = (self.svc.array_data)(self.model, pool as u32).cast::<T>();
            if n == 0 || p.is_null() {
                &mut []
            } else {
                std::slice::from_raw_parts_mut(p, n)
            }
        }
    }

    fn cstr(p: *const c_char) -> String {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: services return NUL-terminated buffers valid until the next call.
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        }
    }
}

impl Pool {
    fn id(self) -> u32 {
        match self {
            Pool::Surfs => 0,
            Pool::Vectors => 1,
            Pool::Points => 2,
            Pool::Nodes => 3,
            Pool::Verts => 4,
        }
    }
}

// `Obj` values are opaque C++ handles produced by the host services table, not
// caller-supplied memory; every dereference happens inside the C++ thunks.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl DynBspHost for CHost {
    fn num(&self, pool: Pool) -> i32 {
        // SAFETY: services are valid for the tracker's lifetime (see `CHost::new`).
        unsafe { (self.svc.array_num)(self.model, pool.id()) }
    }
    fn set_num(&mut self, pool: Pool, n: i32) {
        unsafe { (self.svc.array_set_num)(self.model, pool.id(), n) }
    }
    fn shrink(&mut self, pool: Pool) {
        unsafe { (self.svc.array_shrink)(self.model, pool.id()) }
    }
    fn remove(&mut self, pool: Pool, index: i32, count: i32) {
        unsafe { (self.svc.array_remove)(self.model, pool.id(), index, count) }
    }
    fn surfs_mut(&mut self) -> &mut [FBspSurf] {
        self.pool_slice(Pool::Surfs)
    }
    fn vectors_mut(&mut self) -> &mut [FVector] {
        self.pool_slice(Pool::Vectors)
    }
    fn points_mut(&mut self) -> &mut [FVector] {
        self.pool_slice(Pool::Points)
    }
    fn verts_mut(&mut self) -> &mut [FVert] {
        self.pool_slice(Pool::Verts)
    }
    fn nodes_mut(&mut self) -> &mut [FBspNode] {
        self.pool_slice(Pool::Nodes)
    }
    fn level_actor_count(&self) -> i32 {
        unsafe { (self.svc.level_actor_count)(self.level) }
    }
    fn level_actor(&self, i: i32) -> Obj {
        unsafe { (self.svc.level_actor)(self.level, i) }
    }
    fn level_full_name(&self) -> String {
        Self::cstr(unsafe { (self.svc.level_full_name)(self.level) })
    }
    fn is_moving_brush(&self, actor: Obj) -> bool {
        unsafe { (self.svc.actor_is_moving_brush)(actor) != 0 }
    }
    fn is_mover(&self, actor: Obj) -> bool {
        unsafe { (self.svc.actor_is_mover)(actor) != 0 }
    }
    fn actor_name(&self, actor: Obj) -> String {
        Self::cstr(unsafe { (self.svc.actor_name)(actor) })
    }
    fn location(&self, actor: Obj) -> FVector {
        let mut v = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        unsafe { (self.svc.actor_location)(actor, std::ptr::addr_of_mut!(v).cast()) };
        v
    }
    fn rotation(&self, actor: Obj) -> FRotator {
        let mut r = FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        };
        unsafe { (self.svc.actor_rotation)(actor, std::ptr::addr_of_mut!(r).cast()) };
        r
    }
    fn pre_pivot(&self, actor: Obj) -> FVector {
        let mut v = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        unsafe { (self.svc.actor_pre_pivot)(actor, std::ptr::addr_of_mut!(v).cast()) };
        v
    }
    fn saved_pos(&self, actor: Obj) -> FVector {
        let mut v = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        unsafe { (self.svc.actor_saved_pos)(actor, std::ptr::addr_of_mut!(v).cast()) };
        v
    }
    fn set_saved_pos(&mut self, actor: Obj, v: FVector) {
        unsafe { (self.svc.set_actor_saved_pos)(actor, std::ptr::addr_of!(v).cast()) }
    }
    fn saved_rot(&self, actor: Obj) -> FRotator {
        let mut r = FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        };
        unsafe { (self.svc.actor_saved_rot)(actor, std::ptr::addr_of_mut!(r).cast()) };
        r
    }
    fn set_saved_rot(&mut self, actor: Obj, r: FRotator) {
        unsafe { (self.svc.set_actor_saved_rot)(actor, std::ptr::addr_of!(r).cast()) }
    }
    fn assimilated(&self, actor: Obj) -> bool {
        unsafe { (self.svc.actor_assimilated)(actor) != 0 }
    }
    fn set_assimilated(&mut self, actor: Obj, v: bool) {
        unsafe { (self.svc.set_actor_assimilated)(actor, v as i32) }
    }
    fn special_lit(&self, actor: Obj) -> bool {
        unsafe { (self.svc.actor_special_lit)(actor) != 0 }
    }
    fn brush(&self, actor: Obj) -> Obj {
        unsafe { (self.svc.actor_brush)(actor) }
    }
    fn build_coords(&mut self, actor: Obj) -> (FCoords, FCoords, f32) {
        let zero = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut point = FCoords {
            origin: zero,
            x_axis: zero,
            y_axis: zero,
            z_axis: zero,
        };
        let mut vector = point;
        let orientation = unsafe {
            (self.svc.actor_build_coords)(
                actor,
                std::ptr::addr_of_mut!(point).cast(),
                std::ptr::addr_of_mut!(vector).cast(),
            )
        };
        (point, vector, orientation)
    }
    fn brush_mover_link(&self, brush: Obj) -> i32 {
        unsafe { (self.svc.brush_mover_link)(brush) }
    }
    fn set_brush_mover_link(&mut self, brush: Obj, v: i32) {
        unsafe { (self.svc.set_brush_mover_link)(brush, v) }
    }
    fn brush_polys(&mut self, brush: Obj) -> &mut [FPoly] {
        let mut data: *mut c_void = std::ptr::null_mut();
        let n = unsafe { (self.svc.brush_polys)(brush, &mut data) }.max(0) as usize;
        if n == 0 || data.is_null() {
            &mut []
        } else {
            // SAFETY: the brush's polys array has `n` live elements until the
            // tracker finishes this brush (no resize happens in between).
            unsafe { std::slice::from_raw_parts_mut(data.cast::<FPoly>(), n) }
        }
    }
    fn debug(&mut self, kind: LogKind, msg: &str) {
        let k = match kind {
            LogKind::Init => 0,
            LogKind::Log => 1,
            LogKind::Warning => 2,
        };
        let c = CString::new(msg.replace('\0', "")).unwrap_or_default();
        unsafe { (self.svc.debug)(k, c.as_ptr()) }
    }
}
