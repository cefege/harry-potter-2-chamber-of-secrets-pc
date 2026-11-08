//! Checked Rust policy for the native `FCollisionHash` shell.
//!
//! Hash state is accessed only between callbacks, through a raw handle. Native
//! callbacks may reenter and mutate both the current link and the global tag.
//! Numeric divergences retained: float-to-int saturates, negative RNG values use
//! rem_euclid, and CollisionTag wraps. Radius results deliberately leave all
//! fields except Actor/Next untouched, as native default construction does.

use crate::actor;
use crate::prim::{INDEX_NONE, add, empty_result, sub};
use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FBox, FCheckResult, FRotator};
use std::cell::UnsafeCell;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

const BULK_ALLOC: usize = 1024;
const NUM_BUCKETS: usize = 16384;
const BUCKET_MAX: i32 = NUM_BUCKETS as i32 - 1;
pub type Checked<T> = Result<T, i32>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ActColServices {
    pub app_rand: unsafe extern "C" fn(*mut i32) -> i32,
    pub actor_collision_bounding_box: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub actor_is_moving_brush: unsafe extern "C" fn(*const c_void, *mut i32) -> i32,
    pub actor_primitive: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub primitive_point_check: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut i32,
    ) -> i32,
    pub primitive_line_check: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
        *const c_void,
        u32,
        *mut i32,
    ) -> i32,
    pub alloc_check_result: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub debugf_moved_without_hashing: unsafe extern "C" fn(*const c_void) -> i32,
    pub check_failed: unsafe extern "C" fn(*const c_char, *const c_char, i32) -> i32,
}
/// Operations required by the shared collision-hash policy.
///
/// Implementations may reenter the hash from any operation. The policy therefore
/// retains only raw hash handles, never a mutable borrow, across these calls.
pub trait ActColOps {
    /// Supplies the engine's next random integer.
    unsafe fn app_rand(&self, out: *mut i32) -> i32;
    /// Computes the actor's collision bounding box.
    unsafe fn actor_collision_bounding_box(&self, actor: *mut c_void, out: *mut c_void) -> i32;
    /// Reports whether the actor is a moving brush.
    unsafe fn actor_is_moving_brush(&self, actor: *const c_void, out: *mut i32) -> i32;
    /// Resolves an actor's primitive.
    unsafe fn actor_primitive(&self, actor: *mut c_void, out: *mut *mut c_void) -> i32;
    /// Executes the selected primitive's point check.
    unsafe fn primitive_point_check(
        &self, primitive: *mut c_void, actor: *mut c_void, result: *mut c_void,
        location: *const c_void, size: *const c_void, no_hit: *mut i32,
    ) -> i32;
    /// Executes the selected primitive's line check.
    unsafe fn primitive_line_check(
        &self, primitive: *mut c_void, actor: *mut c_void, result: *mut c_void,
        end: *const c_void, start: *const c_void, size: *const c_void,
        flags: u32, no_hit: *mut i32,
    ) -> i32;
    /// Allocates a check result in the requested memory domain.
    unsafe fn alloc_check_result(&self, mem: *mut c_void, out: *mut *mut c_void) -> i32;
    /// Emits the engine diagnostic for an actor moved without hash removal.
    unsafe fn debugf_moved_without_hashing(&self, actor: *const c_void) -> i32;
    /// Reports a failed native-style assertion.
    unsafe fn check_failed(&self, expression: *const c_char, file: *const c_char, line: i32) -> i32;
    unsafe fn actor_cylinder_extent(&self, actor: *mut c_void, out: *mut FVector) -> i32;
}

impl ActColOps for Hp2ActColServices {
    unsafe fn app_rand(&self, out: *mut i32) -> i32 { unsafe { (self.app_rand)(out) } }
    unsafe fn actor_collision_bounding_box(&self, actor: *mut c_void, out: *mut c_void) -> i32 {
        unsafe { (self.actor_collision_bounding_box)(actor, out) }
    }
    unsafe fn actor_is_moving_brush(&self, actor: *const c_void, out: *mut i32) -> i32 {
        unsafe { (self.actor_is_moving_brush)(actor, out) }
    }
    unsafe fn actor_primitive(&self, actor: *mut c_void, out: *mut *mut c_void) -> i32 {
        unsafe { (self.actor_primitive)(actor, out) }
    }
    unsafe fn primitive_point_check(
        &self, primitive: *mut c_void, actor: *mut c_void, result: *mut c_void,
        location: *const c_void, size: *const c_void, no_hit: *mut i32,
    ) -> i32 {
        unsafe { (self.primitive_point_check)(primitive, actor, result, location, size, no_hit) }
    }
    unsafe fn primitive_line_check(
        &self, primitive: *mut c_void, actor: *mut c_void, result: *mut c_void,
        end: *const c_void, start: *const c_void, size: *const c_void,
        flags: u32, no_hit: *mut i32,
    ) -> i32 {
        unsafe { (self.primitive_line_check)(primitive, actor, result, end, start, size, flags, no_hit) }
    }
    unsafe fn alloc_check_result(&self, mem: *mut c_void, out: *mut *mut c_void) -> i32 {
        unsafe { (self.alloc_check_result)(mem, out) }
    }
    unsafe fn debugf_moved_without_hashing(&self, actor: *const c_void) -> i32 {
        unsafe { (self.debugf_moved_without_hashing)(actor) }
    }
    unsafe fn check_failed(&self, expression: *const c_char, file: *const c_char, line: i32) -> i32 {
        unsafe { (self.check_failed)(expression, file, line) }
    }
    unsafe fn actor_cylinder_extent(&self, actor: *mut c_void, out: *mut FVector) -> i32 {
        let result = actor::published_services().and_then(|services| unsafe {
            actor::get_cylinder_extent(&services, actor.cast(), out)
        });
        result.err().unwrap_or(0)
    }
}

#[inline]
fn checked(status: i32) -> Checked<()> {
    if status == 0 { Ok(()) } else { Err(status) }
}
#[track_caller]
fn check_hash(s: &impl ActColOps, ok: bool, expr: &CStr) -> Checked<()> {
    if ok {
        Ok(())
    } else {
        unsafe {
            checked(s.check_failed(
                expr.as_ptr(),
                c"actcol.rs".as_ptr(),
                std::panic::Location::caller().line() as i32,
            ))
        }
    }
}

// The original tables and CollisionTag are engine-thread globals, not a lock.
// Inited is set BEFORE RNG callbacks, so a nested constructor sees partial tables.
struct Globals {
    inited: bool,
    tag: i32,
    x: [i32; NUM_BUCKETS],
    y: [i32; NUM_BUCKETS],
    z: [i32; NUM_BUCKETS],
}
struct EngineGlobal(UnsafeCell<Globals>);
// SAFETY: native collision hashing is confined to the engine thread, including reentry.
unsafe impl Sync for EngineGlobal {}
static GLOBALS: EngineGlobal = EngineGlobal(UnsafeCell::new(Globals {
    inited: false,
    tag: 0,
    x: [0; NUM_BUCKETS],
    y: [0; NUM_BUCKETS],
    z: [0; NUM_BUCKETS],
}));
unsafe fn init_tables(s: &impl ActColOps) -> Checked<()> {
    unsafe {
        let g = GLOBALS.0.get();
        if !(*g).inited {
            (*g).inited = true;
            for i in 0..NUM_BUCKETS {
                (*g).x[i] = i as i32;
                (*g).y[i] = i as i32;
                (*g).z[i] = i as i32;
            }
            for i in 0..NUM_BUCKETS {
                let mut r = 0;
                checked(s.app_rand(&mut r))?;
                (*g).x.swap(i, r.rem_euclid(NUM_BUCKETS as i32) as usize);
                checked(s.app_rand(&mut r))?;
                (*g).y.swap(i, r.rem_euclid(NUM_BUCKETS as i32) as usize);
                checked(s.app_rand(&mut r))?;
                (*g).z.swap(i, r.rem_euclid(NUM_BUCKETS as i32) as usize);
            }
        }
        Ok(())
    }
}
unsafe fn advance_tag() {
    unsafe {
        let g = GLOBALS.0.get();
        (*g).tag = (*g).tag.wrapping_add(1);
    }
}
unsafe fn tag() -> i32 {
    unsafe { (*GLOBALS.0.get()).tag }
}
fn grid(v: f32) -> i32 {
    (((v + 32768.0) * (1.0 / 256.0)) as i32).clamp(0, BUCKET_MAX)
}
fn indices(v: FVector) -> (i32, i32, i32) {
    (grid(v.x), grid(v.y), grid(v.z))
}
unsafe fn slot(x: i32, y: i32, z: i32) -> (usize, i32) {
    unsafe {
        let g = GLOBALS.0.get();
        (
            ((*g).x[x as usize] ^ (*g).y[y as usize] ^ (*g).z[z as usize]) as usize,
            x + (y << 8) + (z << 16),
        )
    }
}
#[derive(Clone, Copy)]
struct Link {
    actor: *mut AActor,
    next: u32,
    i_location: i32,
}
pub struct CollisionHash {
    head: Box<[u32]>,
    links: Vec<Link>,
    available: u32,
    actors_added: i32,
    frags_added: i32,
    used: i32,
    checks: i32,
}

unsafe fn extent(
    s: &impl ActColOps,
    actor: *mut AActor,
) -> Checked<((i32, i32, i32), (i32, i32, i32))> {
    unsafe {
        let mut bx = std::mem::MaybeUninit::<FBox>::uninit();
        checked(s.actor_collision_bounding_box(
            actor.cast(),
            bx.as_mut_ptr().cast(),
        ))?;
        let bx = bx.assume_init();
        Ok((indices(bx.min), indices(bx.max)))
    }
}
unsafe fn allocate(s: &impl ActColOps, mem: *mut c_void) -> Checked<*mut FCheckResult> {
    unsafe {
        let mut out = ptr::null_mut();
        checked(s.alloc_check_result(mem, &mut out))?;
        Ok(out.cast())
    }
}
fn hit(time: f32) -> FCheckResult {
    let mut h = empty_result();
    h.time = time;
    h.item = INDEX_NONE;
    h
}
unsafe fn prepend(
    s: &impl ActColOps,
    mem: *mut c_void,
    head: *mut FCheckResult,
    value: *const FCheckResult,
) -> Checked<*mut FCheckResult> {
    unsafe {
        let p = allocate(s, mem)?;
        ptr::copy_nonoverlapping(value, p, 1);
        (*p).next = head.cast();
        Ok(p)
    }
}

impl CollisionHash {
    pub fn new(s: &impl ActColOps) -> Checked<Self> {
        unsafe {
            init_tables(s)?;
        }
        Ok(Self {
            head: vec![0; NUM_BUCKETS].into_boxed_slice(),
            links: Vec::new(),
            available: 0,
            actors_added: 0,
            frags_added: 0,
            used: 0,
            checks: 0,
        })
    }
    /// No callback occurs while this short mutable borrow exists.
    pub fn tick(&mut self) {
        self.actors_added = 0;
        self.frags_added = 0;
        self.checks = 0;
    }
    fn take_link(&mut self) -> usize {
        if self.available == 0 {
            let base = self.links.len();
            for i in 0..BULK_ALLOC {
                self.links.push(Link {
                    actor: ptr::null_mut(),
                    next: if i + 1 < BULK_ALLOC {
                        (base + i + 2) as u32
                    } else {
                        0
                    },
                    i_location: 0,
                });
            }
            self.available = (base + 1) as u32;
        }
        let idx = self.available as usize - 1;
        self.available = self.links[idx].next;
        idx
    }
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
    /// # Safety
    /// The handle/actors are live engine-thread objects; callbacks may reenter.
    pub unsafe fn add_actor(
        h: *mut Self,
        s: &impl ActColOps,
        actor: *mut AActor,
    ) -> Checked<()> {
        unsafe {
            check_hash(s, (*actor).b_collide_actors(), c"Actor->bCollideActors")?;
            if (*actor).b_delete_me() {
                return Ok(());
            }
            (*h).actors_added += 1;
            let ((x0, y0, z0), (x1, y1, z1)) = extent(s, actor)?;
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let (bucket, i_location) = slot(x, y, z);
                        let idx = (*h).take_link();
                        (&mut (*h).links)[idx] = Link {
                            actor,
                            next: (&(*h).head)[bucket],
                            i_location,
                        };
                        (&mut (*h).head)[bucket] = (idx + 1) as u32;
                        (*h).used += 1;
                        (*h).frags_added += 1;
                    }
                }
            }
            (*actor).col_location = (*actor).location;
            Ok(())
        }
    }
    /// # Safety
    /// Same contract as add_actor.
    pub unsafe fn remove_actor(
        h: *mut Self,
        s: &impl ActColOps,
        actor: *mut AActor,
        thorough: bool,
    ) -> Checked<()> {
        unsafe {
            check_hash(s, (*actor).b_collide_actors(), c"Actor->bCollideActors")?;
            if (*actor).b_delete_me() {
                return Ok(());
            }
            if (*actor).location != (*actor).col_location {
                checked(s.debugf_moved_without_hashing(actor.cast()))?;
            }
            if thorough {
                for b in 0..NUM_BUCKETS {
                    (*h).unlink_matching(b, actor);
                }
            } else {
                let ((x0, y0, z0), (x1, y1, z1)) = extent(s, actor)?;
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        for z in z0..=z1 {
                            let (b, _) = slot(x, y, z);
                            (*h).unlink_matching(b, actor);
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// Returns whether any bucket still links to `actor`.
    ///
    /// # Safety
    /// `h` is a live collision hash and `actor` is a live actor identity.
    pub unsafe fn actor_is_referenced(h: *const Self, actor: *const AActor) -> bool {
        let hash = unsafe { &*h };
        for &head in hash.head.iter() {
            let mut current = head;
            while current != 0 {
                let link = &hash.links[(current - 1) as usize];
                if link.actor == actor.cast_mut() {
                    return true;
                }
                current = link.next;
            }
        }
        false
    }
    /// # Safety
    /// The handle and actors are live; result storage belongs to native GMem.
    pub unsafe fn actor_point_check(
        h: *mut Self,
        s: &impl ActColOps,
        location: FVector,
        size: FVector,
    ) -> Checked<*mut FCheckResult> {
        unsafe {
            let mut result = ptr::null_mut();
            let (x0, y0, z0) = indices(sub(location, size));
            let (x1, y1, z1) = indices(add(location, size));
            advance_tag();
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let (b, il) = slot(x, y, z);
                        let mut cur = (&(*h).head)[b];
                        while cur != 0 {
                            let idx = cur as usize - 1;
                            let l = (&(*h).links)[idx];
                            if !l.actor.is_null()
                                && (*l.actor).collision_tag != tag()
                                && l.i_location == il
                            {
                                (*l.actor).collision_tag = tag();
                                let mut test = hit(1.0);
                                let mut no_hit = 0;
                                let mut primitive = ptr::null_mut();
                                checked(s.actor_primitive(l.actor.cast(), &mut primitive))?;
                                checked(s.primitive_point_check(
                                    primitive,
                                    (&(*h).links)[idx].actor.cast(),
                                    (&raw mut test).cast(),
                                    (&location as *const FVector).cast(),
                                    (&size as *const FVector).cast(),
                                    &mut no_hit,
                                ))?;
                                if no_hit == 0 {
                                    check_hash(
                                        s,
                                        test.actor == (&(*h).links)[idx].actor.cast(),
                                        c"TestHit.Actor==Link->Actor",
                                    )?;
                                    result = prepend(s, ptr::null_mut(), result, &raw const test)?;
                                }
                            }
                            // Native Link->Next is read AFTER all callbacks, even if Link was recycled.
                            cur = (&(*h).links)[idx].next;
                        }
                    }
                }
            }
            Ok(result)
        }
    }
    /// # Safety
    /// Same native GMem lifetime contract as actor_point_check.
    pub unsafe fn actor_radius_check(
        h: *mut Self,
        s: &impl ActColOps,
        location: FVector,
        radius: f32,
    ) -> Checked<*mut FCheckResult> {
        unsafe {
            let mut result: *mut FCheckResult = ptr::null_mut();
            let r = FVector {
                x: radius,
                y: radius,
                z: radius,
            };
            let (x0, y0, z0) = indices(sub(location, r));
            let (x1, y1, z1) = indices(add(location, r));
            advance_tag();
            let radius_sq = radius * radius;
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let (b, il) = slot(x, y, z);
                        let mut cur = (&(*h).head)[b];
                        while cur != 0 {
                            let idx = cur as usize - 1;
                            let l = (&(*h).links)[idx];
                            if !l.actor.is_null()
                                && (*l.actor).collision_tag != tag()
                                && l.i_location == il
                            {
                                (*l.actor).collision_tag = tag();
                                let d = sub((*l.actor).location, location);
                                if d.z.mul_add(d.z, d.x.mul_add(d.x, d.y * d.y)) < radius_sq {
                                    let n = allocate(s, ptr::null_mut())?;
                                    // Default native FCheckResult has no initialized fields. Do not read it as a Rust value.
                                    ptr::addr_of_mut!((*n).actor)
                                        .write((&(*h).links)[idx].actor.cast());
                                    ptr::addr_of_mut!((*n).next).write(result.cast());
                                    result = n;
                                }
                            }
                            cur = (&(*h).links)[idx].next;
                        }
                    }
                }
            }
            Ok(result)
        }
    }
    /// # Safety
    /// Actor/handle are live. Native failure intentionally leaves exchanged pose in place.
    pub unsafe fn actor_encroachment_check(
        h: *mut Self,
        s: &impl ActColOps,
        actor: *mut AActor,
        mut location: FVector,
        mut rotation: FRotator,
    ) -> Checked<*mut FCheckResult> {
        unsafe {
            check_hash(s, !actor.is_null(), c"Actor!=NULL")?;
            std::mem::swap(&mut location, &mut (*actor).location);
            std::mem::swap(&mut rotation, &mut (*actor).rotation);
            let ((x0, y0, z0), (x1, y1, z1)) = extent(s, actor)?;
            let mut result: *mut FCheckResult = ptr::null_mut();
            let mut tail: *mut FCheckResult = ptr::null_mut();
            advance_tag();
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let (b, il) = slot(x, y, z);
                        let mut cur = (&(*h).head)[b];
                        while cur != 0 {
                            let idx = cur as usize - 1;
                            let l = (&(*h).links)[idx];
                            if !l.actor.is_null()
                                && (*l.actor).collision_tag != tag()
                                && l.i_location == il
                            {
                                (*l.actor).collision_tag = tag();
                                let mut test = hit(1.0);
                                let mut moving = 0;
                                checked(s.actor_is_moving_brush(l.actor.cast(), &mut moving))?;
                                if moving == 0 && (&(*h).links)[idx].actor != actor {
                                    let mut primitive = ptr::null_mut();
                                    checked(s.actor_primitive(actor.cast(), &mut primitive))?;
                                    let other = (&(*h).links)[idx].actor;
                                    let mut size = FVector { x: 0.0, y: 0.0, z: 0.0 };
                                    checked(s.actor_cylinder_extent(other.cast(), &mut size))?;
                                    let mut no_hit = 0;
                                    // Pass the actual live location address, just as the const native reference.
                                    let loc = ptr::addr_of!((*(&(*h).links)[idx].actor).location);
                                    checked(s.primitive_point_check(
                                        primitive,
                                        actor.cast(),
                                        (&raw mut test).cast(),
                                        loc.cast(),
                                        (&size as *const FVector).cast(),
                                        &mut no_hit,
                                    ))?;
                                    if no_hit == 0 {
                                        test.actor = (&(*h).links)[idx].actor.cast();
                                        test.primitive = ptr::null_mut();
                                        let n = allocate(s, ptr::null_mut())?;
                                        ptr::write(n, test);
                                        if tail.is_null() {
                                            result = n;
                                        } else {
                                            (*tail).next = n.cast();
                                        }
                                        tail = n;
                                    }
                                }
                            }
                            cur = (&(*h).links)[idx].next;
                        }
                    }
                }
            }
            std::mem::swap(&mut location, &mut (*actor).location);
            std::mem::swap(&mut rotation, &mut (*actor).rotation);
            if !tail.is_null() {
                (*tail).next = ptr::null_mut();
            }
            Ok(result)
        }
    }
    /// # Safety
    /// Mem is a native FMemStack or null (GMem); caller owns its mark scope.
    pub unsafe fn actor_line_check(
        h: *mut Self,
        s: &impl ActColOps,
        mem: *mut c_void,
        end: FVector,
        start: FVector,
        size: FVector,
        flags: u32,
    ) -> Checked<*mut FCheckResult> {
        fn lo(a: f32, b: f32) -> f32 {
            if a <= b { a } else { b }
        }
        fn hi(a: f32, b: f32) -> f32 {
            if a >= b { a } else { b }
        }
        unsafe {
            let mut result = ptr::null_mut();
            advance_tag();
            let min = FVector {
                x: lo(start.x, end.x),
                y: lo(start.y, end.y),
                z: lo(start.z, end.z),
            };
            let max = FVector {
                x: hi(start.x, end.x),
                y: hi(start.y, end.y),
                z: hi(start.z, end.z),
            };
            let (x0, y0, z0) = indices(sub(min, size));
            let (x1, y1, z1) = indices(add(max, size));
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let (b, il) = slot(x, y, z);
                        let mut cur = (&(*h).head)[b];
                        while cur != 0 {
                            let idx = cur as usize - 1;
                            let l = (&(*h).links)[idx];
                            if !l.actor.is_null()
                                && (*l.actor).collision_tag != tag()
                                && l.i_location == il
                            {
                                let mut test = hit(0.0);
                                (*l.actor).collision_tag = tag();
                                let mut no_hit = 0;
                                let mut primitive = ptr::null_mut();
                                checked(s.actor_primitive(l.actor.cast(), &mut primitive))?;
                                checked(s.primitive_line_check(
                                    primitive,
                                    (&(*h).links)[idx].actor.cast(),
                                    (&raw mut test).cast(),
                                    (&end as *const FVector).cast(),
                                    (&start as *const FVector).cast(),
                                    (&size as *const FVector).cast(),
                                    flags,
                                    &mut no_hit,
                                ))?;
                                if no_hit == 0 {
                                    result = prepend(s, mem, result, &raw const test)?;
                                }
                            }
                            cur = (&(*h).links)[idx].next;
                        }
                    }
                }
            }
            Ok(result)
        }
    }
}
