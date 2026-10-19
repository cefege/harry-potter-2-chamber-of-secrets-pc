//! Port of `Engine/Src/UnActCol.cpp` (`FCollisionHash`): the actor grid.
//!
//! Bucket heads and links live here. Links are `u32` indices (`index + 1`,
//! `0` = null) into `links`, so the bucket chains and the free list mirror the
//! C++ pointer chains exactly. The C++ shell `FCollisionHash` keeps only the
//! virtual forwarding and the handle. Everything that touches an `AActor`'s
//! primitive, bounding box, RNG, check-result allocation, or assert/debug
//! output goes through `Hp2ActColServices`.
//!
//! Divergences (documented, see the port report):
//! - `appRound` is `static_cast<INT>(f)` on this build (truncation). Rust `as`
//!   saturates out-of-range values and maps NaN to 0; C++ leaves that UB.
//! - `appRand() % NUM_BUCKETS` uses `rem_euclid`; a negative `appRand()` is UB in C++.
//! - `CollisionTag++` wraps; signed overflow is UB in C++.
//! - Fresh `FCheckResult`s are zero-filled; C++ `new FCheckResult` leaves
//!   unset fields uninitialized.

use std::ffi::{CStr, c_char, c_void};
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FBox, FCheckResult, FRotator};

use crate::prim::{INDEX_NONE, add, empty_result, sub};

const BULK_ALLOC: usize = 1024;
const NUM_BUCKETS: usize = 16384;
const BUCKET_MAX: i32 = NUM_BUCKETS as i32 - 1;
const GRAN_XY: f32 = 256.0;
const GRAN_Z: f32 = 256.0;
const XY_OFS: f32 = 32768.0;
const Z_OFS: f32 = 32768.0;

/// `FCollisionHash::CollisionTag` (process-wide static).
static COLLISION_TAG: AtomicI32 = AtomicI32::new(0);

/// Per-slice services table (`Hp2ActColServices` in `Hp2ActColServices.h`).
/// `#[repr(C)]` mirror; field order is ABI. Object pointers cross the ABI as
/// `void*` (same convention as `Hp2GeometryServices`), so cbindgen never has to
/// emit the C++ engine types. The typed wrappers below restore the signatures.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ActColServices {
    /// `appRand()`.
    pub app_rand: unsafe extern "C" fn() -> i32,
    /// `actor->GetPrimitive()->GetCollisionBoundingBox(actor, true)`: writes `FBox*`.
    pub actor_collision_bounding_box: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    /// `actor->GetCylinderExtent()`: writes `FVector*`.
    pub actor_cylinder_extent: unsafe extern "C" fn(actor: *const c_void, out: *mut c_void),
    /// `actor->IsMovingBrush()`: returns UBOOL.
    pub actor_is_moving_brush: unsafe extern "C" fn(actor: *const c_void) -> i32,
    /// `owner->GetPrimitive()->PointCheck(*res, owner, *location, *extent, 0)`: returns INT.
    pub primitive_point_check: unsafe extern "C" fn(
        owner: *mut c_void,
        res: *mut c_void,
        location: *const c_void,
        extent: *const c_void,
    ) -> i32,
    /// `actor->GetPrimitive()->LineCheck(*res, actor, *end, *start, *size, flags)`: returns INT.
    pub primitive_line_check: unsafe extern "C" fn(
        actor: *mut c_void,
        res: *mut c_void,
        end: *const c_void,
        start: *const c_void,
        size: *const c_void,
        flags: u32,
    ) -> i32,
    /// `new(mem)FCheckResult`, uninitialized storage. A null `mem` means `GMem`.
    pub alloc_check_result: unsafe extern "C" fn(mem: *mut c_void) -> *mut c_void,
    /// `debugf(TEXT("%s moved without proper hashing"), Actor->GetFullName())`.
    pub debugf_moved_without_hashing: unsafe extern "C" fn(actor: *const c_void),
    /// `appFailAssert(expr, file, line)` for a failed `check()`.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, file: *const c_char, line: i32),
}

impl Hp2ActColServices {
    /// `actor->GetPrimitive()->GetCollisionBoundingBox(actor, true)`.
    ///
    /// # Safety
    /// `actor` must be a live `AActor`.
    unsafe fn collision_box(&self, actor: *mut AActor) -> FBox {
        let mut out = std::mem::MaybeUninit::<FBox>::zeroed();
        unsafe {
            (self.actor_collision_bounding_box)(actor.cast(), out.as_mut_ptr().cast());
            out.assume_init()
        }
    }

    /// `actor->GetCylinderExtent()`.
    ///
    /// # Safety
    /// `actor` must be a live `AActor`.
    unsafe fn cylinder_extent(&self, actor: *const AActor) -> FVector {
        let mut out = std::mem::MaybeUninit::<FVector>::zeroed();
        unsafe {
            (self.actor_cylinder_extent)(actor.cast(), out.as_mut_ptr().cast());
            out.assume_init()
        }
    }

    /// `actor->IsMovingBrush()`.
    ///
    /// # Safety
    /// `actor` must be a live `AActor`.
    unsafe fn is_moving_brush(&self, actor: *const AActor) -> i32 {
        unsafe { (self.actor_is_moving_brush)(actor.cast()) }
    }

    /// `owner->GetPrimitive()->PointCheck(*res, owner, *loc, *ext, 0)`.
    ///
    /// # Safety
    /// All pointers must be valid for the duration of the call.
    unsafe fn point_check(
        &self,
        owner: *mut AActor,
        res: *mut FCheckResult,
        loc: *const FVector,
        ext: *const FVector,
    ) -> i32 {
        unsafe { (self.primitive_point_check)(owner.cast(), res.cast(), loc.cast(), ext.cast()) }
    }

    /// `actor->GetPrimitive()->LineCheck(*res, actor, *end, *start, *size, flags)`.
    ///
    /// # Safety
    /// All pointers must be valid for the duration of the call.
    unsafe fn line_check(
        &self,
        actor: *mut AActor,
        res: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        size: *const FVector,
        flags: u32,
    ) -> i32 {
        unsafe {
            (self.primitive_line_check)(
                actor.cast(),
                res.cast(),
                end.cast(),
                start.cast(),
                size.cast(),
                flags,
            )
        }
    }

    /// `new(mem)FCheckResult` (storage only; null `mem` means `GMem`).
    ///
    /// # Safety
    /// `mem` must be null or a valid `FMemStack*`.
    unsafe fn alloc_result(&self, mem: *mut c_void) -> *mut FCheckResult {
        unsafe { (self.alloc_check_result)(mem).cast() }
    }

    /// `debugf(TEXT("%s moved without proper hashing"), Actor->GetFullName())`.
    ///
    /// # Safety
    /// `actor` must be a live `AActor`.
    unsafe fn debugf_moved(&self, actor: *const AActor) {
        unsafe { (self.debugf_moved_without_hashing)(actor.cast()) }
    }
}

/// `Clamp<INT>(x, 0, NUM_BUCKETS-1)`.
#[inline]
fn clamp_index(x: i32) -> i32 {
    if x < 0 {
        0
    } else if x < BUCKET_MAX {
        x
    } else {
        BUCKET_MAX
    }
}

/// `GetHashIndices` per axis: `Clamp<INT>(appRound((v + ofs) * (1.f/gran)), 0, NUM_BUCKETS-1)`.
/// `appRound` is `static_cast<INT>(f)` on this build.
#[inline]
fn grid(v: f32, ofs: f32, gran: f32) -> i32 {
    clamp_index(((v + ofs) * (1.0 / gran)) as i32)
}

/// `FCollisionHash::GetHashIndices`.
#[inline]
fn hash_indices(loc: FVector) -> (i32, i32, i32) {
    (
        grid(loc.x, XY_OFS, GRAN_XY),
        grid(loc.y, XY_OFS, GRAN_XY),
        grid(loc.z, Z_OFS, GRAN_Z),
    )
}

/// Static lookup tables. C++ builds these once, in the first constructor, from
/// `appRand()`: identity fill, then for each i one exchange per table (X, Y, Z).
/// The initializer needs the services pointer, so this stays a `OnceLock`.
struct Tables {
    x: Box<[i32]>,
    y: Box<[i32]>,
    z: Box<[i32]>,
}

static TABLES: OnceLock<Tables> = OnceLock::new();

fn tables(svc: &Hp2ActColServices) -> &'static Tables {
    TABLES.get_or_init(|| {
        let identity = || {
            (0..NUM_BUCKETS as i32)
                .collect::<Vec<i32>>()
                .into_boxed_slice()
        };
        let mut t = Tables {
            x: identity(),
            y: identity(),
            z: identity(),
        };
        for i in 0..NUM_BUCKETS {
            // SAFETY: `app_rand` is the engine RNG callback from the services table.
            let rx = unsafe { (svc.app_rand)() }.rem_euclid(NUM_BUCKETS as i32) as usize;
            t.x.swap(i, rx);
            let ry = unsafe { (svc.app_rand)() }.rem_euclid(NUM_BUCKETS as i32) as usize;
            t.y.swap(i, ry);
            let rz = unsafe { (svc.app_rand)() }.rem_euclid(NUM_BUCKETS as i32) as usize;
            t.z.swap(i, rz);
        }
        t
    })
}

/// `FCollisionLink`. `next` is `index + 1` (0 = null).
#[derive(Clone, Copy)]
struct Link {
    actor: *mut AActor,
    next: u32,
    i_location: i32,
}

/// Cell range covered by an actor (`GetActorExtent`).
#[derive(Clone, Copy)]
struct Extent {
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
    z0: i32,
    z1: i32,
}

/// Rust state behind the C++ `FCollisionHash` shell.
pub struct CollisionHash {
    tables: &'static Tables,
    /// `Hash[NUM_BUCKETS]`: first link as `index + 1`.
    head: Box<[u32]>,
    /// Link pool. Blocks of `BULK_ALLOC` are appended, as in C++.
    links: Vec<Link>,
    /// `Available`: free-list head as `index + 1`.
    available: u32,
    /// `GActorsAdded`, `GFragsAdded`, `GUsed`, `GChecks` (statistics only).
    actors_added: i32,
    frags_added: i32,
    used: i32,
    checks: i32,
}

#[inline]
fn next_tag() -> i32 {
    COLLISION_TAG
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1)
}

/// `FCheckResult(InTime)`: `Item = INDEX_NONE`, everything else zero.
fn hit_with_time(time: f32) -> FCheckResult {
    let mut r = empty_result();
    r.time = time;
    r.item = INDEX_NONE;
    r
}

/// `check(cond)`: forwards a failed assertion to the C++ `appFailAssert`.
fn check_hash(svc: &Hp2ActColServices, ok: bool, expr: &CStr, line: u32) {
    if !ok {
        // SAFETY: the services table is valid for the duration of the FFI call.
        unsafe {
            (svc.check_failed)(expr.as_ptr(), c"actcol.rs".as_ptr(), line as i32);
        }
    }
}

/// Bounds of `FBox(0) + Start + End`, as built by `FBox::operator+=`:
/// the first point seeds Min/Max, then each point widens them with
/// `::Min(A,B) = (A<=B) ? A : B` and `::Max(A,B) = (A>=B) ? A : B`.
fn span_of(start: FVector, end: FVector) -> (FVector, FVector) {
    fn cmin(a: f32, b: f32) -> f32 {
        if a <= b { a } else { b }
    }
    fn cmax(a: f32, b: f32) -> f32 {
        if a >= b { a } else { b }
    }
    let min = FVector {
        x: cmin(start.x, end.x),
        y: cmin(start.y, end.y),
        z: cmin(start.z, end.z),
    };
    let max = FVector {
        x: cmax(start.x, end.x),
        y: cmax(start.y, end.y),
        z: cmax(start.z, end.z),
    };
    (min, max)
}

impl CollisionHash {
    /// `FCollisionHash::FCollisionHash`.
    pub fn new(svc: &Hp2ActColServices) -> Self {
        Self {
            tables: tables(svc),
            head: vec![0u32; NUM_BUCKETS].into_boxed_slice(),
            links: Vec::new(),
            available: 0,
            actors_added: 0,
            frags_added: 0,
            used: 0,
            checks: 0,
        }
    }

    /// `FCollisionHash::Tick`.
    pub fn tick(&mut self) {
        self.actors_added = 0;
        self.frags_added = 0;
        self.checks = 0;
    }

    /// `GetHashLink`: bucket index and `iLocation` for a cell.
    #[inline]
    fn slot(&self, x: i32, y: i32, z: i32) -> (usize, i32) {
        let t = self.tables;
        let bucket = (t.x[x as usize] ^ t.y[y as usize] ^ t.z[z as usize]) as usize;
        let i_location = x + (y << 8) + (z << 16);
        (bucket, i_location)
    }

    /// Pops a link from `Available`, allocating a bulk block when empty.
    fn take_link(&mut self) -> usize {
        if self.available == 0 {
            let base = self.links.len();
            for i in 0..BULK_ALLOC {
                let next = if i + 1 < BULK_ALLOC {
                    (base + i + 2) as u32
                } else {
                    0
                };
                self.links.push(Link {
                    actor: ptr::null_mut(),
                    next,
                    i_location: 0,
                });
            }
            self.available = (base + 1) as u32;
        }
        let idx = self.available as usize - 1;
        self.available = self.links[idx].next;
        idx
    }

    /// `GetActorExtent`: cell range of the actor's current bounding box.
    fn actor_extent(&self, svc: &Hp2ActColServices, actor: *mut AActor) -> Extent {
        // SAFETY: the actor is a live engine object owned by the caller.
        let bx = unsafe { svc.collision_box(actor) };
        let (x0, y0, z0) = hash_indices(bx.min);
        let (x1, y1, z1) = hash_indices(bx.max);
        Extent {
            x0,
            x1,
            y0,
            y1,
            z0,
            z1,
        }
    }

    /// `FCollisionHash::AddActor`.
    ///
    /// # Safety
    /// `actor` must be a valid, live `AActor`.
    pub unsafe fn add_actor(&mut self, svc: &Hp2ActColServices, actor: *mut AActor) {
        // SAFETY: caller contract.
        let (collide, deleted) = unsafe { ((*actor).b_collide_actors(), (*actor).b_delete_me()) };
        check_hash(svc, collide, c"Actor->bCollideActors", line!());
        if deleted {
            return;
        }
        // CheckActorNotReferenced: its body is dead (`if( 0 && ... )`), so nothing to do.
        self.actors_added += 1;
        let e = self.actor_extent(svc, actor);
        for x in e.x0..=e.x1 {
            for y in e.y0..=e.y1 {
                for z in e.z0..=e.z1 {
                    let (bucket, i_location) = self.slot(x, y, z);
                    let idx = self.take_link();
                    self.links[idx] = Link {
                        actor,
                        next: self.head[bucket],
                        i_location,
                    };
                    self.head[bucket] = (idx + 1) as u32;
                    self.used += 1;
                    self.frags_added += 1;
                }
            }
        }
        // SAFETY: caller contract.
        unsafe {
            (*actor).col_location = (*actor).location;
        }
    }

    /// Walks one bucket chain, dropping every link whose actor is `actor` or
    /// null, and returns the dropped links to `Available` (C++ `RemoveActor` loop).
    fn unlink_matching(&mut self, bucket: usize, actor: *mut AActor) {
        let mut prev: Option<usize> = None;
        let mut cur = self.head[bucket];
        while cur != 0 {
            let idx = cur as usize - 1;
            let link = self.links[idx];
            if !link.actor.is_null() && link.actor != actor {
                prev = Some(idx);
                cur = link.next;
            } else {
                match prev {
                    None => self.head[bucket] = link.next,
                    Some(p) => self.links[p].next = link.next,
                }
                self.links[idx].next = self.available;
                self.available = cur;
                self.used -= 1;
                cur = link.next;
            }
        }
    }

    /// `FCollisionHash::RemoveActor`.
    ///
    /// # Safety
    /// `actor` must be a valid, live `AActor`.
    pub unsafe fn remove_actor(
        &mut self,
        svc: &Hp2ActColServices,
        actor: *mut AActor,
        thorough: bool,
    ) {
        // SAFETY: caller contract.
        let (collide, deleted, location, col_location) = unsafe {
            (
                (*actor).b_collide_actors(),
                (*actor).b_delete_me(),
                (*actor).location,
                (*actor).col_location,
            )
        };
        check_hash(svc, collide, c"Actor->bCollideActors", line!());
        if deleted {
            return;
        }
        if location != col_location {
            // SAFETY: services table is valid for the call.
            unsafe { svc.debugf_moved(actor) };
        }

        if thorough {
            // Search the entire hash array, to find scraps left by sloppy move code.
            for b in 0..NUM_BUCKETS {
                self.unlink_matching(b, actor);
            }
        } else {
            // Search only where the actor should be.
            let e = self.actor_extent(svc, actor);
            for x in e.x0..=e.x1 {
                for y in e.y0..=e.y1 {
                    for z in e.z0..=e.z1 {
                        let (bucket, _) = self.slot(x, y, z);
                        self.unlink_matching(bucket, actor);
                    }
                }
            }
        }
        // CheckActorNotReferenced: dead body, nothing to do.
    }

    /// Allocates a result from `mem` (null = `GMem`) and prepends it to `head`
    /// (`New->Next = Result`).
    ///
    /// # Safety
    /// `head` must be null or a valid result from the same allocator.
    unsafe fn prepend(
        svc: &Hp2ActColServices,
        mem: *mut c_void,
        head: *mut FCheckResult,
        mut value: FCheckResult,
    ) -> *mut FCheckResult {
        value.next = head as *mut c_void;
        // SAFETY: the allocator returns uninitialized storage sized for FCheckResult.
        unsafe {
            let p = svc.alloc_result(mem);
            ptr::write(p, value);
            p
        }
    }

    /// `FCollisionHash::ActorPointCheck`. Allocates from `GMem`.
    ///
    /// # Safety
    /// Services and the hashed actors must be valid.
    pub unsafe fn actor_point_check(
        &mut self,
        svc: &Hp2ActColServices,
        location: FVector,
        extent: FVector,
    ) -> *mut FCheckResult {
        let mut result: *mut FCheckResult = ptr::null_mut();
        let (x0, y0, z0) = hash_indices(sub(location, extent));
        let (x1, y1, z1) = hash_indices(add(location, extent));
        let tag = next_tag();
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    let (bucket, i_location) = self.slot(x, y, z);
                    let mut cur = self.head[bucket];
                    while cur != 0 {
                        let link = self.links[cur as usize - 1];
                        // SAFETY: links hold live actors.
                        unsafe {
                            if !link.actor.is_null()
                                && (*link.actor).collision_tag != tag
                                && link.i_location == i_location
                            {
                                (*link.actor).collision_tag = tag;
                                let mut test = hit_with_time(1.0);
                                if svc.point_check(link.actor, &mut test, &location, &extent) == 0 {
                                    check_hash(
                                        svc,
                                        test.actor == link.actor as *mut c_void,
                                        c"TestHit.Actor == Link->Actor",
                                        line!(),
                                    );
                                    result = Self::prepend(svc, ptr::null_mut(), result, test);
                                }
                            }
                        }
                        cur = link.next;
                    }
                }
            }
        }
        result
    }

    /// `FCollisionHash::ActorRadiusCheck`. Allocates from `GMem`.
    ///
    /// # Safety
    /// Services and the hashed actors must be valid.
    pub unsafe fn actor_radius_check(
        &mut self,
        svc: &Hp2ActColServices,
        location: FVector,
        radius: f32,
    ) -> *mut FCheckResult {
        let mut result: *mut FCheckResult = ptr::null_mut();
        let r = FVector {
            x: radius,
            y: radius,
            z: radius,
        };
        let (x0, y0, z0) = hash_indices(sub(location, r));
        let (x1, y1, z1) = hash_indices(add(location, r));
        let tag = next_tag();
        let radius_sq = radius * radius;
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    let (bucket, i_location) = self.slot(x, y, z);
                    let mut cur = self.head[bucket];
                    while cur != 0 {
                        let link = self.links[cur as usize - 1];
                        // SAFETY: links hold live actors.
                        unsafe {
                            if !link.actor.is_null()
                                && (*link.actor).collision_tag != tag
                                && link.i_location == i_location
                            {
                                (*link.actor).collision_tag = tag;
                                let d = sub((*link.actor).location, location);
                                // FVector::SizeSquared
                                // UnMath.h:772 `X*X + Y*Y + Z*Z`, compiled to fmadd(z,z,fmadd(x,x,y*y)).
                                let size_sq = d.z.mul_add(d.z, d.x.mul_add(d.x, d.y * d.y));
                                if size_sq < radius_sq {
                                    let mut hit = empty_result();
                                    hit.actor = link.actor as *mut c_void;
                                    result = Self::prepend(svc, ptr::null_mut(), result, hit);
                                }
                            }
                        }
                        cur = link.next;
                    }
                }
            }
        }
        result
    }

    /// `FCollisionHash::ActorEncroachmentCheck`. The C++ signature takes an
    /// `FMemStack&`, but the body allocates from `GMem`; that quirk is kept.
    ///
    /// # Safety
    /// `actor` must be a valid, live `AActor`; services must be valid.
    pub unsafe fn actor_encroachment_check(
        &mut self,
        svc: &Hp2ActColServices,
        actor: *mut AActor,
        mut location: FVector,
        mut rotation: FRotator,
    ) -> *mut FCheckResult {
        check_hash(svc, !actor.is_null(), c"Actor != NULL", line!());
        // Exchange(Location, Actor->Location); Exchange(Rotation, Actor->Rotation)
        // SAFETY: `actor` is valid per the contract; the swaps are short-lived borrows.
        unsafe {
            std::mem::swap(&mut location, &mut (*actor).location);
            std::mem::swap(&mut rotation, &mut (*actor).rotation);
        }
        let e = self.actor_extent(svc, actor);
        let mut head: *mut FCheckResult = ptr::null_mut();
        let mut tail: *mut FCheckResult = ptr::null_mut();
        let tag = next_tag();
        for x in e.x0..=e.x1 {
            for y in e.y0..=e.y1 {
                for z in e.z0..=e.z1 {
                    let (bucket, i_location) = self.slot(x, y, z);
                    let mut cur = self.head[bucket];
                    while cur != 0 {
                        let link = self.links[cur as usize - 1];
                        // SAFETY: links hold live actors.
                        unsafe {
                            if !link.actor.is_null()
                                && (*link.actor).collision_tag != tag
                                && link.i_location == i_location
                            {
                                (*link.actor).collision_tag = tag;
                                let mut test = hit_with_time(1.0);
                                if svc.is_moving_brush(link.actor) == 0 && link.actor != actor {
                                    let extent = svc.cylinder_extent(link.actor);
                                    let loc = (*link.actor).location;
                                    if svc.point_check(actor, &mut test, &loc, &extent) == 0 {
                                        test.actor = link.actor as *mut c_void;
                                        test.primitive = ptr::null_mut();
                                        let n = svc.alloc_result(ptr::null_mut());
                                        ptr::write(n, test);
                                        (*n).next = ptr::null_mut();
                                        if tail.is_null() {
                                            head = n;
                                        } else {
                                            (*tail).next = n as *mut c_void;
                                        }
                                        tail = n;
                                    }
                                }
                            }
                        }
                        cur = link.next;
                    }
                }
            }
        }
        // Exchange back; `*PrevLink = NULL`.
        // SAFETY: as above.
        unsafe {
            std::mem::swap(&mut location, &mut (*actor).location);
            std::mem::swap(&mut rotation, &mut (*actor).rotation);
            if !tail.is_null() {
                (*tail).next = ptr::null_mut();
            }
        }
        head
    }

    /// `FCollisionHash::ActorLineCheck`. Allocates from `mem` (null = `GMem`).
    ///
    /// # Safety
    /// `mem` must be a valid `FMemStack` or null; services must be valid.
    pub unsafe fn actor_line_check(
        &mut self,
        svc: &Hp2ActColServices,
        mem: *mut c_void,
        end: FVector,
        start: FVector,
        size: FVector,
        flags: u32,
    ) -> *mut FCheckResult {
        let mut result: *mut FCheckResult = ptr::null_mut();
        let tag = next_tag();
        let (bmin, bmax) = span_of(start, end);
        let (x0, y0, z0) = hash_indices(sub(bmin, size));
        let (x1, y1, z1) = hash_indices(add(bmax, size));
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    let (bucket, i_location) = self.slot(x, y, z);
                    let mut cur = self.head[bucket];
                    while cur != 0 {
                        let link = self.links[cur as usize - 1];
                        // SAFETY: links hold live actors.
                        unsafe {
                            if !link.actor.is_null()
                                && (*link.actor).collision_tag != tag
                                && link.i_location == i_location
                            {
                                let mut hit = hit_with_time(0.0);
                                (*link.actor).collision_tag = tag;
                                if svc.line_check(link.actor, &mut hit, &end, &start, &size, flags)
                                    == 0
                                {
                                    result = Self::prepend(svc, mem, result, hit);
                                }
                            }
                        }
                        cur = link.next;
                    }
                }
            }
        }
        result
    }
}
