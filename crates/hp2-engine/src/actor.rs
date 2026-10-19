//! AActor networking and misc logic (`Engine/Src/UnActor.cpp`, the part past the
//! replication block). The C++ `AActor` methods forward here. Engine objects
//! cross as `*mut c_void`; every engine-side call goes through the
//! `Hp2ActorServices` table passed on each call, the same way the console port
//! does it.
//!
//! Null `this`/`Other` is not checked. The C++ dereferences them, so the port
//! does the same raw dereference. That is intentional: the crash is preserved,
//! and no zero or no-op stands in for it.

use std::ffi::{CStr, c_char, c_void};
use std::sync::Mutex;

use crate::prim;
use crate::vec::size_squared;
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, AMover, APawn, APlayerPawn, APlayerReplicationInfo, FBox, FRotator, ULevel,
};
use hp2_abi::{FCoords, FPlane, FVector};

/// Class identities for `Hp2ActorServices::actor_is_a`. The C++ adapter maps
/// each to the matching `StaticClass()` check.
pub mod class {
    pub const AMOVER: u32 = 0;
    pub const APAWN: u32 = 1;
    pub const APLAYERPAWN: u32 = 2;
    pub const APLAYERREPLICATIONINFO: u32 = 3;
    pub const APROJECTILE: u32 = 4;
    pub const ATRIGGER: u32 = 5;
    pub const ABRUSH: u32 = 6;
}

/// `ROLE_SimulatedProxy` (`EngineClasses.h`).
const ROLE_SIMULATED_PROXY: u8 = 2;
/// `PHYS_None` (`EngineClasses.h`).
const PHYS_NONE: u8 = 0;
/// `PHYS_MovingBrush` (`EngineClasses.h`).
const PHYS_MOVING_BRUSH: u8 = 9;
/// `NM_Client` (`EngineClasses.h`).
const NM_CLIENT: u8 = 3;

/// Engine calls the port needs. Every function takes engine objects as
/// `void*` and does the engine-side work in C++.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ActorServices {
    /// `ULevel::Hash != NULL`.
    pub level_has_hash: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `ULevel::Hash->AddActor(actor)`.
    pub level_hash_add_actor: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void),
    /// `ULevel::Hash->RemoveActor(actor)`.
    pub level_hash_remove_actor: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void),
    /// `obj->IsA(<class>::StaticClass())` for the `class::*` ids.
    pub actor_is_a: unsafe extern "C" fn(obj: *const c_void, class_id: u32) -> i32,
    /// `ULevel::FarMoveActor(actor, dest, test, no_check)`.
    pub level_far_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        dest: *const c_void,
        test: i32,
        no_check: i32,
    ) -> i32,
    /// `ULevel::MoveActor(actor, FVector(0,0,0), rot, Hit, 0, 0, 0, 1)` with a
    /// locally constructed `FCheckResult Hit`.
    pub level_move_actor_to_rotation:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, rot: *const c_void),
    /// `AActor::moveSmooth(delta)`.
    pub actor_move_smooth: unsafe extern "C" fn(actor: *mut c_void, delta: *const c_void) -> i32,
    /// `AActor::setPhysics(phys)` with the default floor `NULL`.
    pub actor_set_physics: unsafe extern "C" fn(actor: *mut c_void, phys: u8),
    /// `AActor::eventBump(other)`.
    pub actor_event_bump: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void),
    /// `base->eventDetach(actor)`.
    pub actor_event_detach: unsafe extern "C" fn(base: *mut c_void, actor: *mut c_void),
    /// `base->eventAttach(actor)`.
    pub actor_event_attach: unsafe extern "C" fn(base: *mut c_void, actor: *mut c_void),
    /// `actor->eventBaseChange()`.
    pub actor_event_base_change: unsafe extern "C" fn(actor: *mut c_void),
    /// `owner->eventLostChild(actor)`.
    pub actor_event_lost_child: unsafe extern "C" fn(owner: *mut c_void, actor: *mut c_void),
    /// `owner->eventGainedChild(actor)`.
    pub actor_event_gained_child: unsafe extern "C" fn(owner: *mut c_void, actor: *mut c_void),
    /// `GetLevel()->DemoRecDriver->ServerConnection != NULL`. The caller checks
    /// `DemoRecDriver` first.
    pub level_demo_rec_has_server_connection: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `GetLevel()->Engine->Client->Viewports(0)->Actor->PlayerReplicationInfo`,
    /// null when `Viewports(0)->Actor` is null.
    pub level_viewport0_actor_pri: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `GetLevel()->NetDriver->ServerConnection->AverageFrameTime`. The caller
    /// checks `NetDriver` first.
    pub level_net_average_frame_time: unsafe extern "C" fn(level: *mut c_void) -> f32,
    /// `appFailAssert(expr, "UnActor.cpp", line)` for a failed `check()` in `touch_to`.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32),
    /// `AActor::eventTouch(other)`.
    pub actor_event_touch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void),
    /// `AActor::eventUnTouch(other)`.
    pub actor_event_untouch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void),
    /// `GIsEditor != 0`.
    pub is_editor: unsafe extern "C" fn() -> i32,
    /// `GCache.Flush()`.
    pub gcache_flush: unsafe extern "C" fn(),
    /// `GetLevel()->Engine->{Cylinder,OCylinder,Box,OvalCylinder,OOvalCylinder}`
    /// selected by `slot` (the `CT_*` value; 0 is the default). The adapter keeps
    /// its `checkSlow` for the selected field.
    pub level_engine_primitive: unsafe extern "C" fn(level: *mut c_void, slot: u8) -> *mut c_void,
    /// `UPrimitive::ValidateActor(actor)`.
    pub primitive_validate_actor: unsafe extern "C" fn(prim: *mut c_void, actor: *mut c_void),
    /// `Level->bBegunPlay` (nonzero when begun).
    pub level_begun_play: unsafe extern "C" fn(level_info: *mut c_void) -> i32,
    /// `Super::ProcessEvent(Function, Parms, Result)` on the actor.
    pub uobject_process_event: unsafe extern "C" fn(
        obj: *mut c_void,
        func: *mut c_void,
        parms: *mut c_void,
        result: *mut c_void,
    ),
    /// `UObject::Destroy()` (qualified, non-virtual).
    pub uobject_destroy: unsafe extern "C" fn(obj: *mut c_void),
    /// `RenderInterface->RemoveFromRoot(); ConditionalDestroy(); delete`.
    pub render_interface_destroy: unsafe extern "C" fn(ri: *mut c_void),
    /// `Obj->ConditionalDestroy()`.
    pub object_conditional_destroy: unsafe extern "C" fn(obj: *mut c_void),
    /// `Obj->SetFlags(RF_Transactional)`.
    pub object_set_transactional: unsafe extern "C" fn(obj: *mut c_void),
    /// `GetClass()->ClassFlags & CLASS_Localized` for the actor.
    pub actor_class_is_localized: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Actor->LoadLocalized()`.
    pub actor_load_localized: unsafe extern "C" fn(actor: *mut c_void),
    /// `UObject::PostLoad()` (qualified, non-virtual).
    pub uobject_post_load: unsafe extern "C" fn(obj: *mut c_void),
    /// `Model->Polys`.
    pub model_polys: unsafe extern "C" fn(model: *mut c_void) -> *mut c_void,
    /// `UPlayer::Actor`.
    pub player_actor: unsafe extern "C" fn(player: *mut c_void) -> *mut c_void,
    /// `UPlayer::Actor = actor`.
    pub player_set_actor: unsafe extern "C" fn(player: *mut c_void, actor: *mut c_void),
    /// `APlayerPawn::eventPossess()`.
    pub actor_event_possess: unsafe extern "C" fn(pawn: *mut c_void),
    /// `debugf(NAME_Log, "Possessed PlayerPawn: %s", GetFullName())`.
    pub actor_log_possessed: unsafe extern "C" fn(pawn: *mut c_void),
    /// `Mesh->GetCollisionExtent(actor)`; writes the FVector to `out`.
    pub mesh_collision_extent:
        unsafe extern "C" fn(mesh: *mut c_void, actor: *mut c_void, out: *mut c_void),
    /// `Brush->GetCollisionExtent(actor)`; writes the FVector to `out`.
    pub brush_collision_extent:
        unsafe extern "C" fn(brush: *mut c_void, actor: *mut c_void, out: *mut c_void),
    /// `Mesh->GetRenderBoundingBox(actor, exact)`; writes the FCoords to `out`.
    pub mesh_render_bounding_box:
        unsafe extern "C" fn(mesh: *mut c_void, actor: *mut c_void, exact: i32, out: *mut c_void),
    /// `Brush->GetRenderBoundingBox(actor, exact).GetCoords()`; writes the FCoords to `out`.
    pub brush_render_bounding_box_coords:
        unsafe extern "C" fn(brush: *mut c_void, actor: *mut c_void, exact: i32, out: *mut c_void),
    /// `SetFlags(RF_DebugSerialize)` on `UObject`.
    pub object_set_debug_serialize: unsafe extern "C" fn(obj: *mut c_void),
    /// `GetClass()` for a `UObject`.
    pub object_class: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `Class == UClass::StaticClass()`, as nonzero/zero.
    pub class_is_uclass_static: unsafe extern "C" fn(cls: *mut c_void) -> i32,
    /// `Ar.Preload(UObject*)`.
    pub archive_preload: unsafe extern "C" fn(ar: *mut c_void, obj: *mut c_void),
    /// `Ar.IsLoading()`.
    pub archive_is_loading: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.IsSaving()`.
    pub archive_is_saving: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.IsTrans()`.
    pub archive_is_trans: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `GetClass()->SerializeTaggedPersistentProperties(Ar, this, GetClass())`.
    pub object_serialize_tagged_persistent: unsafe extern "C" fn(obj: *mut c_void, ar: *mut c_void),
    /// `GetClass()->SerializeBin(Ar, this)`.
    pub object_serialize_bin: unsafe extern "C" fn(obj: *mut c_void, ar: *mut c_void),
    /// `GetClass()->GetPropertiesSize()`.
    pub object_properties_size: unsafe extern "C" fn(obj: *mut c_void) -> usize,
    /// `Ar.CountBytes(size, size)`.
    pub archive_count_bytes: unsafe extern "C" fn(ar: *mut c_void, size: usize),
    /// `FCheckResult Hit(1.0); GetLevel()->SingleLineCheck(Hit, actor, end, start, TRACE_Level, FVector(0,0,0))`;
    /// writes `Hit.Location` to `out_location` and returns `Hit.BspSurf()` (null when none).
    pub trace_line_texture_hit: unsafe extern "C" fn(
        actor: *mut c_void,
        trace_end: *const c_void,
        trace_start: *const c_void,
        out_location: *mut c_void,
    ) -> *mut c_void,
    /// `XLevel->Model`.
    pub actor_x_level_model: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `Surf->Texture`.
    pub bsp_surf_texture: unsafe extern "C" fn(surf: *mut c_void) -> *mut c_void,
    /// `Surf->PolyFlags`.
    pub bsp_surf_poly_flags: unsafe extern "C" fn(surf: *mut c_void) -> u32,
    /// `Texture->PolyFlags` (texture is non-null).
    pub texture_poly_flags: unsafe extern "C" fn(texture: *mut c_void) -> u32,
    /// `Surf->Decals.Num()`.
    pub bsp_surf_decal_count: unsafe extern "C" fn(surf: *mut c_void) -> i32,
    /// `Surf->vNormal`.
    pub bsp_surf_v_normal: unsafe extern "C" fn(surf: *mut c_void) -> i32,
    /// `Surf->pBase`.
    pub bsp_surf_p_base: unsafe extern "C" fn(surf: *mut c_void) -> i32,
    /// `Model->Vectors(index)`, copied to `out`.
    pub model_vector: unsafe extern "C" fn(model: *mut c_void, index: i32, out: *mut c_void),
    /// `Model->Points(index)`, copied to `out`.
    pub model_point: unsafe extern "C" fn(model: *mut c_void, index: i32, out: *mut c_void),
    /// `&Surf->Decals(index)`.
    pub bsp_surf_decal: unsafe extern "C" fn(surf: *mut c_void, index: i32) -> *mut c_void,
    /// Copies `Decal->Vertices[0..4]` to `out`.
    pub decal_vertices: unsafe extern "C" fn(decal: *mut c_void, out: *mut c_void),
    /// `Decal->Actor->Texture`.
    pub decal_actor_texture: unsafe extern "C" fn(decal: *mut c_void) -> *mut c_void,
    /// Virtual `AActor::GetPrimitive()` on `actor`, so overrides (ABrush, AParticleFX) run.
    pub actor_get_primitive: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `UObject::IsValid()` on `obj`: null, object-table index and slot checks. Returns 0 or 1.
    pub object_is_valid: unsafe extern "C" fn(obj: *mut c_void) -> i32,
}

/// `appFailAssert` for a failed `check()`. The C++ adapter fixes the file to
/// `UnActor.cpp` and passes the oracle line.
fn touch_check(svc: &Hp2ActorServices, ok: bool, expr: &CStr, line: i32) {
    if !ok {
        unsafe { (svc.check_failed)(expr.as_ptr(), line) };
    }
}

/// `TouchTo(Actor, Other)`: records `Other` in `Actor`'s touch slots, pruning
/// stale touches when all four slots are full. Returns `UBOOL`.
///
/// The four `check()` assertions run as in `DO_CHECK=1` builds. The
/// `IsValid()` assertion is approximated as a non-null test on the slot, since
/// the services table does not expose `UObject::IsValid`.
///
/// # Safety
/// `actor` and `other` must be live `AActor`s.
pub unsafe fn touch_to(svc: &Hp2ActorServices, actor: *mut AActor, other: *mut AActor) -> i32 {
    unsafe {
        touch_check(svc, !actor.is_null(), c"Actor", 1290);
        touch_check(svc, !other.is_null(), c"Other", 1291);
        touch_check(svc, actor != other, c"Actor!=Other", 1292);
        let a = actor.cast::<c_void>();
        let o = other.cast::<c_void>();
        let mut available: i32 = -1;
        for i in 0..4 {
            let t = (*actor).touching[i];
            if t.is_null() {
                available = i as i32;
            } else if t == o {
                return 1;
            }
        }
        if available == -1 {
            for i in 0..4 {
                let t = (*actor).touching[i];
                touch_check(
                    svc,
                    (svc.object_is_valid)(t.cast()) != 0,
                    c"Actor->Touching[i]->IsValid()",
                    1313,
                );
                if (*t.cast::<AActor>()).physics == PHYS_NONE {
                    touch_check(svc, actor != t.cast(), c"Other!=this", 1406);
                    end_touch(svc, actor, t.cast(), 0);
                    available = i as i32;
                }
            }
            if available == -1 && (svc.actor_is_a)(o, class::APAWN) != 0 {
                for i in 0..4 {
                    let t = (*actor).touching[i];
                    if (svc.actor_is_a)(t, class::APAWN) == 0 {
                        touch_check(svc, actor != t.cast(), c"Other!=this", 1406);
                        end_touch(svc, actor, t.cast(), 0);
                        available = i as i32;
                        break;
                    }
                }
                if available == -1 && (*other.cast::<APawn>()).b_is_player() {
                    for i in 0..4 {
                        let t = (*actor).touching[i];
                        if (svc.actor_is_a)(t, class::APAWN) == 0
                            || !(*t.cast::<APawn>()).b_is_player()
                        {
                            touch_check(svc, actor != t.cast(), c"Other!=this", 1406);
                            end_touch(svc, actor, t.cast(), 0);
                            available = i as i32;
                            break;
                        }
                    }
                }
            }
        }
        if available >= 0 {
            (*actor).touching[available as usize] = o;
            (svc.actor_event_touch)(a, o);
            if (*actor).touching[available as usize] != o {
                return 0;
            }
        }
        1
    }
}

/// Values `PreNetReceive` saves and `PostNetReceive` exchanges back. C++ keeps
/// these as file-static variables. `base` is stored as `usize` so the static
/// `Mutex` stays `Send`.
#[derive(Clone, Copy)]
struct SavedNet {
    location: FVector,
    rotation: FRotator,
    base: usize,
    collision: bool,
    radius: f32,
    height: f32,
    sim_anim: FPlane,
    sim_interpolate: FVector,
}

static SAVED: Mutex<SavedNet> = Mutex::new(SavedNet {
    location: FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    rotation: FRotator {
        pitch: 0,
        yaw: 0,
        roll: 0,
    },
    base: 0,
    collision: false,
    radius: 0.0,
    height: 0.0,
    sim_anim: FPlane {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    },
    sim_interpolate: FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
});

fn saved() -> std::sync::MutexGuard<'static, SavedNet> {
    SAVED.lock().unwrap_or_else(|e| e.into_inner())
}

fn sub_vec(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn add_vec(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn scale_vec(s: f32, a: FVector) -> FVector {
    FVector {
        x: s * a.x,
        y: s * a.y,
        z: s * a.z,
    }
}

/// `FVector::IsNearlyZero` with `KINDA_SMALL_NUMBER`.
fn is_nearly_zero(a: FVector) -> bool {
    const KINDA_SMALL_NUMBER: f32 = 1.0e-4;
    a.x.abs() < KINDA_SMALL_NUMBER
        && a.y.abs() < KINDA_SMALL_NUMBER
        && a.z.abs() < KINDA_SMALL_NUMBER
}

/// `AActor::SetCollision`: untouch, set the three collision bits, touch.
///
/// # Safety
/// `actor` must be a live `AActor` whose `XLevel` is a live `ULevel`.
pub unsafe fn set_collision(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_collide_actors: bool,
    new_block_actors: bool,
    new_block_players: bool,
) {
    unsafe {
        let level = (*actor).x_level;
        if (*actor).b_collide_actors() && (svc.level_has_hash)(level) != 0 {
            (svc.level_hash_remove_actor)(level, actor.cast());
        }
        (*actor).set_b_collide_actors(new_collide_actors);
        (*actor).set_b_block_actors(new_block_actors);
        (*actor).set_b_block_players(new_block_players);
        if (*actor).b_collide_actors() && (svc.level_has_hash)(level) != 0 {
            (svc.level_hash_add_actor)(level, actor.cast());
        }
    }
}

/// `AActor::SetCollisionSize`: untouch, set radius and height, touch.
///
/// # Safety
/// Same requirements as [`set_collision`].
pub unsafe fn set_collision_size(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_radius: f32,
    new_height: f32,
) {
    unsafe {
        let level = (*actor).x_level;
        if (*actor).b_collide_actors() && (svc.level_has_hash)(level) != 0 {
            (svc.level_hash_remove_actor)(level, actor.cast());
        }
        (*actor).collision_radius = new_radius;
        (*actor).collision_height = new_height;
        if (*actor).b_collide_actors() && (svc.level_has_hash)(level) != 0 {
            (svc.level_hash_add_actor)(level, actor.cast());
        }
    }
}

/// `AActor::SetOwner`.
///
/// # Safety
/// `actor` must be a live `AActor`. Non-null owners must be live actors.
pub unsafe fn set_owner(svc: &Hp2ActorServices, actor: *mut AActor, new_owner: *mut c_void) {
    unsafe {
        if !(*actor).owner.is_null() {
            (svc.actor_event_lost_child)((*actor).owner, actor.cast());
        }
        (*actor).owner = new_owner;
        if !(*actor).owner.is_null() {
            (svc.actor_event_gained_child)((*actor).owner, actor.cast());
        }
        if new_owner.is_null() {
            (*actor).anim_bone = 0;
        }
    }
}

/// `AActor::SetBase`. Refuses a base that has `actor` in its base chain.
///
/// # Safety
/// `actor` must be a live `AActor`. `new_base` must be null or a live `AActor`
/// whose `Base` chain is valid.
pub unsafe fn set_base(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_base: *mut AActor,
    notify_actor: bool,
) {
    unsafe {
        let mut lp = new_base;
        while !lp.is_null() {
            if lp == actor {
                return;
            }
            lp = (*lp).base.cast();
        }
        if new_base != (*actor).base.cast() {
            let level = (*actor).level;
            let old = (*actor).base.cast::<AActor>();
            if !old.is_null() && old.cast::<c_void>() != level {
                (*old).standing_count = (*old).standing_count.wrapping_sub(1);
                (svc.actor_event_detach)(old.cast(), actor.cast());
            }
            (*actor).base = new_base.cast();
            if !new_base.is_null() && new_base.cast::<c_void>() != level {
                (*new_base).standing_count = (*new_base).standing_count.wrapping_add(1);
                (svc.actor_event_attach)(new_base.cast(), actor.cast());
            }
            if notify_actor {
                (svc.actor_event_base_change)(actor.cast());
            }
        }
    }
}

/// `AActor::PreNetReceive`. Saves the replicated state and pulls the actor out
/// of the level hash.
///
/// # Safety
/// `actor` must be a live `AActor` whose `XLevel` is a live `ULevel` with a
/// non-null `Hash` (the C++ dereferences it unconditionally when
/// `bCollideActors` is set).
pub unsafe fn pre_net_receive(svc: &Hp2ActorServices, actor: *mut AActor) {
    unsafe {
        let mut s = saved();
        s.location = (*actor).location;
        s.rotation = (*actor).rotation;
        s.base = (*actor).base as usize;
        s.collision = (*actor).b_collide_actors();
        s.radius = (*actor).collision_radius;
        s.height = (*actor).collision_height;
        s.sim_anim = (*actor).sim_anim;
        if (svc.actor_is_a)(actor.cast(), class::AMOVER) != 0 {
            s.sim_interpolate = (*actor.cast::<AMover>()).sim_interpolate;
        }
        if (*actor).b_collide_actors() {
            (svc.level_hash_remove_actor)((*actor).x_level, actor.cast());
        }
    }
}

/// `AActor::PostNetReceive`. Swaps the saved state back in and applies the
/// deltas the replicated properties caused.
///
/// # Safety
/// Same requirements as [`pre_net_receive`]. `PreNetReceive` must have run first
/// for the saved values to be meaningful.
pub unsafe fn post_net_receive(svc: &Hp2ActorServices, actor: *mut AActor) {
    unsafe {
        let mut s = saved();
        std::mem::swap(&mut (*actor).location, &mut s.location);
        std::mem::swap(&mut (*actor).rotation, &mut s.rotation);
        let cur_base = (*actor).base as usize;
        (*actor).base = s.base as *mut c_void;
        s.base = cur_base;
        let cur_collide = (*actor).b_collide_actors();
        (*actor).set_b_collide_actors(s.collision);
        s.collision = cur_collide;
        std::mem::swap(&mut (*actor).collision_radius, &mut s.radius);
        std::mem::swap(&mut (*actor).collision_height, &mut s.height);
        let level = (*actor).x_level;
        if (*actor).b_collide_actors() {
            (svc.level_hash_add_actor)(level, actor.cast());
        }
        let saved_sim_interpolate = s.sim_interpolate;
        let saved_sim_anim = s.sim_anim;
        let saved_location = s.location;
        let saved_rotation = s.rotation;
        let saved_radius = s.radius;
        let saved_height = s.height;
        let saved_collision = s.collision;
        let saved_base = s.base as *mut AActor;
        drop(s);

        if (svc.actor_is_a)(actor.cast(), class::AMOVER) != 0 {
            let mover = actor.cast::<AMover>();
            if saved_sim_interpolate != (*mover).sim_interpolate {
                let si = (*mover).sim_interpolate;
                (*mover).old_pos = (*mover).sim_old_pos;
                (*mover).old_rot.yaw = (*mover).sim_old_rot_yaw;
                (*mover).old_rot.pitch = (*mover).sim_old_rot_pitch;
                (*mover).old_rot.roll = (*mover).sim_old_rot_roll;
                (*mover).phys_alpha = si.x * 0.01_f32;
                (*mover).phys_rate = si.y * 0.01_f32;
                // C++ `(INT)` conversion; out-of-range values differ from C (UB there).
                let keynums = si.z as i32;
                (*mover).key_num = (keynums & 255) as u8;
                (*mover).prev_key_num = (keynums >> 8) as u8;
                (svc.actor_set_physics)(actor.cast(), PHYS_MOVING_BRUSH);
                (*actor).set_b_interpolating(true);
            }
        }
        if (svc.actor_is_a)(actor.cast(), class::APLAYERPAWN) != 0
            && !(*level.cast::<ULevel>()).demo_rec_driver.is_null()
            && (svc.level_demo_rec_has_server_connection)(level) != 0
        {
            let pp = actor.cast::<APlayerPawn>();
            (*pp).base.view_rotation.pitch = (*pp).demo_view_pitch;
            (*pp).base.view_rotation.yaw = (*pp).demo_view_yaw;
        }
        if (*actor).sim_anim != saved_sim_anim {
            let sa = (*actor).sim_anim;
            (*actor).anim_frame = sa.x * 0.0001_f32;
            (*actor).anim_rate = sa.y * 0.0002_f32;
            (*actor).tween_rate = sa.z * 0.001_f32;
            (*actor).anim_last = sa.w * 0.0001_f32;
            if (*actor).anim_last < 0.0 {
                (*actor).anim_last *= -1.0;
                (*actor).set_b_anim_loop(true);
                if (svc.actor_is_a)(actor.cast(), class::APAWN) != 0
                    && (*actor).anim_min_rate < 0.5_f32
                {
                    (*actor).anim_min_rate = 0.5_f32;
                }
            } else {
                (*actor).set_b_anim_loop(false);
            }
        }
        if (*actor).location != saved_location {
            if (svc.actor_is_a)(actor.cast(), class::APAWN) != 0
                && (*actor).role == ROLE_SIMULATED_PROXY
                && !is_nearly_zero((*actor).velocity)
                && size_squared(sub_vec((*actor).location, saved_location)) < 10000.0_f32
            {
                // Smooth out movement of other players to account for frame-rate jitter.
                let start_error = size_squared(sub_vec((*actor).location, saved_location));
                let mut new_location = (*actor).location;
                if start_error > 1600.0_f32 {
                    let d = scale_vec(0.35_f32, sub_vec(saved_location, (*actor).location));
                    (svc.actor_move_smooth)(actor.cast(), (&d as *const FVector).cast());
                    if size_squared(sub_vec((*actor).location, saved_location))
                        > 0.75_f32 * start_error
                    {
                        new_location = add_vec(
                            new_location,
                            scale_vec(0.5_f32, sub_vec(saved_location, (*actor).location)),
                        );
                    }
                } else {
                    new_location = add_vec(
                        new_location,
                        scale_vec(0.15_f32, sub_vec(saved_location, (*actor).location)),
                    );
                }
                (svc.level_far_move_actor)(
                    level,
                    actor.cast(),
                    (&new_location as *const FVector).cast(),
                    0,
                    1,
                );
            } else {
                (svc.level_far_move_actor)(
                    level,
                    actor.cast(),
                    (&saved_location as *const FVector).cast(),
                    0,
                    1,
                );
            }
        }
        if (*actor).rotation.pitch != saved_rotation.pitch
            || (*actor).rotation.yaw != saved_rotation.yaw
            || (*actor).rotation.roll != saved_rotation.roll
        {
            (svc.level_move_actor_to_rotation)(
                level,
                actor.cast(),
                (&saved_rotation as *const FRotator).cast(),
            );
        }
        if (*actor).collision_radius != saved_radius || (*actor).collision_height != saved_height {
            set_collision_size(svc, actor, saved_radius, saved_height);
        }
        if (*actor).b_collide_actors() != saved_collision {
            let block_actors = (*actor).b_block_actors();
            let block_players = (*actor).b_block_players();
            set_collision(svc, actor, saved_collision, block_actors, block_players);
        }
        if (*actor).base != saved_base.cast() {
            (svc.actor_event_bump)(actor.cast(), saved_base.cast());
            if !saved_base.is_null() {
                (svc.actor_event_bump)(saved_base.cast(), actor.cast());
            }
            set_base(svc, actor, saved_base, true);
        }
        (*actor).set_b_just_teleported(false);
        if (svc.actor_is_a)(actor.cast(), class::APLAYERREPLICATIONINFO) != 0
            && (*(*actor).level.cast::<ALevelInfo>()).net_mode == NM_CLIENT
        {
            let pri = actor.cast::<APlayerReplicationInfo>();
            if !(*level.cast::<ULevel>()).net_driver.is_null()
                && (svc.level_viewport0_actor_pri)(level) == actor.cast()
            {
                // C++ `(INT)` conversion of `(AverageFrameTime * 1000) / 2`.
                let avg = (svc.level_net_average_frame_time)(level);
                let delta = ((avg * 1000.0_f32) / 2.0_f32) as i32;
                (*pri).ping = (*pri).ping.wrapping_sub(delta);
            }
            if (*pri).ping < 0 {
                (*pri).ping = 0;
            }
        }
    }
}

/// `AActor::GetSkin`. `Index < ARRAY_COUNT(MultiSkins)` compares as unsigned
/// (`size_t`), so a negative index returns NULL.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn get_skin(actor: *const AActor, index: i32) -> *mut c_void {
    unsafe {
        if (index as u32) < 8 {
            std::ptr::addr_of!((*actor).multi_skins)
                .cast::<*mut c_void>()
                .offset(index as isize)
                .read_unaligned()
        } else {
            std::ptr::null_mut()
        }
    }
}

/// `AActor::GetNetPriority`. `Sent` and `Lag` are unused by the C++ body.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn get_net_priority(actor: *const AActor, time: f32) -> f32 {
    unsafe {
        if (*actor).b_always_relevant() {
            let freq = (*actor).net_update_frequency * 0.1_f32;
            // `::Min` is `(A<=B) ? A : B`.
            let capped = if freq <= 1.0_f32 { freq } else { 1.0_f32 };
            (*actor).net_priority * time * capped
        } else {
            (*actor).net_priority * time
        }
    }
}

/// `AActor::BeginTouch(other)`. Projectile/trigger pairs get the reverse-ordered
/// notification so trigger callbacks cannot be suppressed by projectile teardown.
///
/// # Safety
/// `actor` and `other` must be live `AActor`s.
pub unsafe fn begin_touch(svc: &Hp2ActorServices, actor: *mut AActor, other: *mut AActor) {
    unsafe {
        let a_proj = (svc.actor_is_a)(actor.cast(), class::APROJECTILE) != 0;
        if a_proj && (svc.actor_is_a)(other.cast(), class::ATRIGGER) != 0 {
            touch_to(svc, other, actor);
            if !(*actor).b_delete_me() && !(*other).b_delete_me() {
                touch_to(svc, actor, other);
            }
        } else if (svc.actor_is_a)(other.cast(), class::APROJECTILE) != 0
            && (svc.actor_is_a)(actor.cast(), class::ATRIGGER) != 0
        {
            touch_to(svc, actor, other);
            if !(*actor).b_delete_me() && !(*other).b_delete_me() {
                touch_to(svc, other, actor);
            }
        } else if touch_to(svc, actor, other) != 0 {
            touch_to(svc, other, actor);
        }
    }
}

/// `AActor::EndTouch(other, no_notify_self)`. Clears both touch slots and
/// fires `eventUnTouch` on each side that had one.
///
/// # Safety
/// `actor` and `other` must be live `AActor`s.
pub unsafe fn end_touch(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    other: *mut AActor,
    no_notify_self: i32,
) {
    unsafe {
        for i in 0..4 {
            if (*actor).touching[i] == other.cast() {
                (*actor).touching[i] = std::ptr::null_mut();
                if no_notify_self == 0 {
                    (svc.actor_event_untouch)(actor.cast(), other.cast());
                }
                break;
            }
        }
        for i in 0..4 {
            if (*other).touching[i] == actor.cast() {
                (*other).touching[i] = std::ptr::null_mut();
                (svc.actor_event_untouch)(other.cast(), actor.cast());
                break;
            }
        }
    }
}

/// `AActor::IsBrush`: `Brush != NULL && IsA(ABrush::StaticClass())`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn is_brush(svc: &Hp2ActorServices, actor: *const AActor) -> bool {
    unsafe { !(*actor).brush.is_null() && (svc.actor_is_a)(actor.cast(), class::ABRUSH) != 0 }
}

/// `AActor::IsOverlapping`. Written as plain `*` and `+`: each `Square` is a
/// separate inline call, so the original does not contract these into FMAs.
///
/// # Safety
/// `actor` and `other` must be live `AActor`s.
pub unsafe fn is_overlapping(
    svc: &Hp2ActorServices,
    actor: *const AActor,
    other: *const AActor,
) -> i32 {
    unsafe {
        if !is_brush(svc, actor)
            && !is_brush(svc, other)
            && (*actor).level.cast_const() != other.cast::<c_void>()
        {
            let dx = (*actor).location.x - (*other).location.x;
            let dy = (*actor).location.y - (*other).location.y;
            let dz = (*actor).location.z - (*other).location.z;
            let r = (*actor).collision_radius + (*other).collision_radius;
            let h = (*actor).collision_height + (*other).collision_height;
            i32::from(dx * dx + dy * dy < r * r && dz * dz < h * h)
        } else {
            0
        }
    }
}

/// `AActor::GetVisibilityBox`: `FBox(Location - Extent, Location + Extent)`,
/// where `Extent` is `(VisibilityRadius + 1, VisibilityRadius + 1, VisibilityHeight + 1)`.
/// The two-vector `FBox` constructor sets `IsValid = 1`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn get_visibility_box(actor: *const AActor) -> FBox {
    unsafe {
        let r = (*actor).visibility_radius + 1.0_f32;
        let extent = FVector {
            x: r,
            y: r,
            z: (*actor).visibility_height + 1.0_f32,
        };
        let loc = (*actor).location;
        let mut bx: FBox = std::mem::zeroed();
        bx.min = sub_vec(loc, extent);
        bx.max = add_vec(loc, extent);
        bx.is_valid = 1;
        bx
    }
}

/// `AActor::IsAnimating`: `AnimSequence != NAME_None && (AnimRate != 0 || TweenRate != 0)`.
/// `NAME_None` is index 0 (first entry of `EName`).
fn is_animating(actor: &AActor) -> bool {
    actor.anim_sequence != [0u8; 4] && (actor.anim_rate != 0.0 || actor.tween_rate != 0.0)
}

/// `ACarcass::UpdateFrequency`. `::Min(NetUpdateFrequency, 4.f)` is `(A<=B) ? A : B`.
/// The viewer and view vectors are unused by the C++ body.
///
/// # Safety
/// `actor` must be a live `ACarcass`.
pub unsafe fn carcass_update_frequency(actor: *const AActor) -> f32 {
    let a = unsafe { &*actor };
    if a.physics == PHYS_NONE && !is_animating(a) {
        let f = a.net_update_frequency;
        return if f <= 4.0 { f } else { 4.0 };
    }
    a.net_update_frequency
}

/// `AActor::GetHitActor`: `HitActor` when set, otherwise `this`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn get_hit_actor(actor: *mut AActor) -> *mut c_void {
    let h = unsafe { (*actor).hit_actor };
    if h.is_null() { actor.cast() } else { h }
}

/// Cross product `a ^ b`, `UnMath.h:681` (non-PS2 branch). Contracted as built with
/// `-ffp-contract=on`: the first product of each component is fused and the second is
/// rounded then negated (`fnmul` + `fmadd` in the disassembly).
fn cross_fused(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.y.mul_add(b.z, -(a.z * b.y)),
        y: a.z.mul_add(b.x, -(a.x * b.z)),
        z: a.x.mul_add(b.y, -(a.y * b.x)),
    }
}

// `FPlane::PlaneDot` (`UnMath.h:1021`) = `X*P.X + Y*P.Y + Z*P.Z - W`. `prim::dot` has the
// contracted order (`Y*P.Y` rounded, `fma(X)`, `fma(Z)`) that the disassembly shows for
// both `operator|` and `PlaneDot`, so the same `prim::dot` covers `W` and `Dot`.

/// `PointInsideQuad` (static in `UnActor.cpp`). Four clip planes, one per edge
/// `(Verts[j], Verts[i])` with `j` starting at 3. A sign flip between consecutive
/// planes rejects the point. The plane `(V0, ClipNorm)` has `W = V0 | ClipNorm`.
/// `on_plane(ClipNorm, W, Dot)` observes each computed plane (used by the bit-exact test).
#[inline]
fn point_inside_quad_each(
    point: FVector,
    verts: &[FVector; 4],
    normal: FVector,
    mut on_plane: impl FnMut(FVector, f32, f32),
) -> bool {
    let mut prev_dot = 0.0f32;
    let mut j = 3usize;
    for i in 0..4 {
        let v0 = verts[j];
        let v1 = verts[i];
        let clip_norm = cross_fused(normal, sub_vec(v1, v0));
        let w = prim::dot(v0, clip_norm);
        let dot = prim::dot(clip_norm, point) - w;
        on_plane(clip_norm, w, dot);
        if dot * prev_dot < 0.0 {
            return false;
        }
        prev_dot = dot;
        j = i;
    }
    true
}

/// `PointInsideQuad`; see [`point_inside_quad_each`].
pub fn point_inside_quad(point: FVector, verts: &[FVector; 4], normal: FVector) -> bool {
    point_inside_quad_each(point, verts, normal, |_, _, _| {})
}

/// Test hook: per-plane `(ClipNorm, W, Dot)` for the bit-exact fixture.
pub fn point_inside_quad_planes(
    point: FVector,
    verts: &[FVector; 4],
    normal: FVector,
) -> (bool, Vec<(FVector, f32, f32)>) {
    let mut planes = Vec::with_capacity(4);
    let r = point_inside_quad_each(point, verts, normal, |n, w, d| planes.push((n, w, d)));
    (r, planes)
}

/// Decal step of `execTraceTexture`: starts from `texture` and replaces it with the texture of
/// each decal whose quad contains the hit point in surface-local space. Later decals win.
/// `decal_texture` runs only for containing decals, as in the oracle (`Decal->Actor->Texture`
/// dereferences the decal's actor).
pub fn decal_texture_override(
    mut texture: *mut c_void,
    hit_local: FVector,
    surf_normal: FVector,
    decal_count: i32,
    mut decal_at: impl FnMut(i32) -> *mut c_void,
    mut decal_vertices: impl FnMut(*mut c_void) -> [FVector; 4],
    mut decal_texture: impl FnMut(*mut c_void) -> *mut c_void,
) -> *mut c_void {
    for d in 0..decal_count {
        let decal = decal_at(d);
        let verts = decal_vertices(decal);
        if point_inside_quad(hit_local, &verts, surf_normal) {
            let t = decal_texture(decal);
            if !t.is_null() {
                texture = t;
            }
        }
    }
    texture
}

/// `AActor::PostEditChange` after `Super`: `if (GIsEditor) bLightChanged = 1`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn actor_post_edit_change(svc: &Hp2ActorServices, actor: *mut AActor) {
    if unsafe { (svc.is_editor)() } != 0 {
        unsafe { (*actor).set_b_light_changed(true) };
    }
}

/// `AZoneInfo::PostEditChange` after `Super`: `if (GIsEditor) GCache.Flush()`.
///
/// # Safety
/// `svc` must hold valid service callbacks.
pub unsafe fn zone_info_post_edit_change(svc: &Hp2ActorServices) {
    if unsafe { (svc.is_editor)() } != 0 {
        unsafe { (svc.gcache_flush)() };
    }
}

/// `CollideType` values (`EngineClasses.h`).
pub const CT_ALIGNED_CYLINDER: u8 = 0;
pub const CT_ORIENTED_CYLINDER: u8 = 1;
pub const CT_BOX: u8 = 2;
pub const CT_SHAPE: u8 = 3;
pub const CT_ALIGNED_OVAL_CYLINDER: u8 = 4;
pub const CT_ORIENTED_OVAL_CYLINDER: u8 = 5;

/// `AActor::GetPrimitive`. A `CT_Shape` actor uses its mesh, then its brush. Otherwise it
/// uses the engine primitive for its collide type. The C++ `default` case is aligned cylinder.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn get_primitive(svc: &Hp2ActorServices, actor: *const AActor) -> *mut c_void {
    let a = unsafe { &*actor };
    if a.collide_type == CT_SHAPE {
        if !a.mesh.is_null() {
            return a.mesh;
        }
        if !a.brush.is_null() {
            return a.brush;
        }
    }
    let slot = match a.collide_type {
        CT_ORIENTED_CYLINDER => 1,
        CT_BOX => 2,
        CT_ALIGNED_OVAL_CYLINDER => 4,
        CT_ORIENTED_OVAL_CYLINDER => 5,
        _ => CT_ALIGNED_CYLINDER,
    };
    unsafe { (svc.level_engine_primitive)(a.x_level, slot) }
}

/// `AActor::Spawned`: `if (CollideType == CT_Shape) GetPrimitive()->ValidateActor(this)`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn spawned(svc: &Hp2ActorServices, actor: *mut AActor) {
    if unsafe { (*actor).collide_type } == CT_SHAPE {
        let prim = unsafe { (svc.actor_get_primitive)(actor.cast()) };
        unsafe { (svc.primitive_validate_actor)(prim, actor.cast()) };
    }
}

/// `AActor::ProcessEvent`: `if (Level->bBegunPlay) Super::ProcessEvent(Function, Parms, Result)`.
///
/// # Safety
/// `actor` must be a live `AActor` with a valid `Level`.
pub unsafe fn process_event(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    func: *mut c_void,
    parms: *mut c_void,
    result: *mut c_void,
) {
    let level = unsafe { (*actor).level };
    if unsafe { (svc.level_begun_play)(level) } != 0 {
        unsafe { (svc.uobject_process_event)(actor.cast(), func, parms, result) };
    }
}

/// `AActor::Destroy`: destroy the render interface, then the aux anims (the array length
/// is re-read each iteration, as in `for_array`), then `UObject::Destroy()`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn destroy(svc: &Hp2ActorServices, actor: *mut AActor) {
    unsafe {
        if !(*actor).render_interface.is_null() {
            (svc.render_interface_destroy)((*actor).render_interface);
            (*actor).render_interface = std::ptr::null_mut();
        }
        let mut i: i32 = 0;
        while i < (*actor).aux_anims.num {
            let elem = *(*actor)
                .aux_anims
                .data
                .cast::<*mut c_void>()
                .add(i as usize);
            if !elem.is_null() {
                (svc.object_conditional_destroy)(elem);
            }
            i += 1;
        }
        (svc.uobject_destroy)(actor.cast());
    }
}

/// `AActor::PostLoad`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn post_load(svc: &Hp2ActorServices, actor: *mut AActor) {
    unsafe {
        (svc.uobject_post_load)(actor.cast());
        if (svc.actor_class_is_localized)(actor.cast()) != 0 {
            (svc.actor_load_localized)(actor.cast());
        }
        if !(*actor).brush.is_null() {
            (svc.object_set_transactional)((*actor).brush);
        }
        if !(*actor).brush.is_null() {
            let polys = (svc.model_polys)((*actor).brush);
            if !polys.is_null() {
                (svc.object_set_transactional)(polys);
            }
        }
        if (*actor).collide_type == CT_SHAPE
            && (!(*actor).brush.is_null() || !(*actor).mesh.is_null())
        {
            let prim = (svc.actor_get_primitive)(actor.cast());
            (svc.primitive_validate_actor)(prim, actor.cast());
        }
    }
}

/// `APlayerPawn::SetPlayer`: detach the previous pawn from `player`, attach `pawn`, then
/// send `eventPossess` and log.
///
/// # Safety
/// `pawn` must be a live `APlayerPawn`. `player` must be a live `UPlayer` unless the check fails.
pub unsafe fn set_player(svc: &Hp2ActorServices, pawn: *mut APlayerPawn, player: *mut c_void) {
    touch_check(svc, !player.is_null(), c"InPlayer!=NULL", 1073);
    unsafe {
        let old = (svc.player_actor)(player);
        if !old.is_null() {
            (*old.cast::<APlayerPawn>()).player = std::ptr::null_mut();
            (svc.player_set_actor)(player, std::ptr::null_mut());
        }
        (*pawn).player = player;
        (svc.player_set_actor)(player, pawn.cast());
        (svc.actor_event_possess)(pawn.cast());
        (svc.actor_log_possessed)(pawn.cast());
    }
}

/// `AActor::GetCylinderExtent`: shape-type collision extent from the mesh or brush, else
/// the collision box or cylinder dimensions. The result is written to `out`.
///
/// # Safety
/// `actor` must be a live `AActor`; `out` must be valid for one `FVector` write.
pub unsafe fn get_cylinder_extent(svc: &Hp2ActorServices, actor: *mut AActor, out: *mut FVector) {
    unsafe {
        if (*actor).collide_type == CT_SHAPE {
            if !(*actor).mesh.is_null() {
                (svc.mesh_collision_extent)((*actor).mesh, actor.cast(), out.cast());
                return;
            }
            if !(*actor).brush.is_null() {
                (svc.brush_collision_extent)((*actor).brush, actor.cast(), out.cast());
                return;
            }
        }
        let radius = (*actor).collision_radius;
        let width = if (*actor).collide_type == CT_BOX {
            let cw = (*actor).collision_width;
            if cw != 0.0 { cw } else { radius }
        } else {
            radius
        };
        *out = FVector {
            x: radius,
            y: width,
            z: (*actor).collision_height,
        };
    }
}

/// `AActor::GetRenderBoundingBox`: mesh first, then brush, else the zero `FCoords`.
/// The result is written to `out`.
///
/// # Safety
/// `actor` must be a live `AActor`; `out` must be valid for one `FCoords` write.
pub unsafe fn get_render_bounding_box(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    exact: i32,
    out: *mut FCoords,
) {
    unsafe {
        if !(*actor).mesh.is_null() {
            (svc.mesh_render_bounding_box)((*actor).mesh, actor.cast(), exact, out.cast());
        } else if !(*actor).brush.is_null() {
            (svc.brush_render_bounding_box_coords)((*actor).brush, actor.cast(), exact, out.cast());
        } else {
            std::ptr::write_bytes(out, 0, 1);
        }
    }
}

/// `AActor::SerializePersistence` (the guard/unguard and the commented-out frame-state block
/// stay in C++): flag as debug-serialized, preload the class, serialize the class properties
/// (tagged when loading or saving outside transactions, else binary), then count the bytes.
///
/// # Safety
/// `actor` must be a live `AActor`; `ar` must be a live `FArchive`.
pub unsafe fn serialize_persistence(svc: &Hp2ActorServices, actor: *mut AActor, ar: *mut c_void) {
    unsafe {
        let obj = actor.cast::<c_void>();
        (svc.object_set_debug_serialize)(obj);
        let the_class = (svc.object_class)(obj);
        let is_uclass = (svc.class_is_uclass_static)(the_class) != 0;
        if !is_uclass {
            (svc.archive_preload)(ar, the_class);
        }
        let is_uclass_props = (svc.class_is_uclass_static)(the_class) != 0;
        if !is_uclass_props {
            let loading_or_saving =
                (svc.archive_is_loading)(ar) != 0 || (svc.archive_is_saving)(ar) != 0;
            if loading_or_saving && (svc.archive_is_trans)(ar) == 0 {
                (svc.object_serialize_tagged_persistent)(obj, ar);
            } else {
                (svc.object_serialize_bin)(obj, ar);
            }
        }
        let size = (svc.object_properties_size)(obj);
        (svc.archive_count_bytes)(ar, size);
    }
}

/// `AActor::execTraceTexture` core. The P_GET macros and `P_FINISH` stay in C++, which forwards
/// the decoded arguments and stores the returned texture into `Result`.
///
/// # Safety
/// `actor` must be a live `AActor`; `flags` must be a valid `INT*`; the vector pointers must be valid.
pub unsafe fn trace_texture(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    trace_end: *const FVector,
    trace_start: *const FVector,
    flags: *mut i32,
    trace_decals: bool,
) -> *mut c_void {
    unsafe {
        let zero = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut hit_location = zero;
        let surf = (svc.trace_line_texture_hit)(
            actor.cast(),
            trace_end.cast(),
            trace_start.cast(),
            (&mut hit_location as *mut FVector).cast(),
        );
        let model = (svc.actor_x_level_model)(actor.cast());
        let mut texture: *mut c_void = std::ptr::null_mut();
        *flags = 0;
        if !surf.is_null() {
            texture = (svc.bsp_surf_texture)(surf);
            let texture_flags = if texture.is_null() {
                0
            } else {
                (svc.texture_poly_flags)(texture)
            };
            *flags = ((svc.bsp_surf_poly_flags)(surf) | texture_flags) as i32;
            let decal_count = (svc.bsp_surf_decal_count)(surf);
            if trace_decals && decal_count != 0 {
                let mut surf_normal = zero;
                (svc.model_vector)(
                    model,
                    (svc.bsp_surf_v_normal)(surf),
                    (&mut surf_normal as *mut FVector).cast(),
                );
                let mut surf_base = zero;
                (svc.model_point)(
                    model,
                    (svc.bsp_surf_p_base)(surf),
                    (&mut surf_base as *mut FVector).cast(),
                );
                let hit_local = sub_vec(hit_location, surf_base);
                texture = decal_texture_override(
                    texture,
                    hit_local,
                    surf_normal,
                    decal_count,
                    |d| (svc.bsp_surf_decal)(surf, d),
                    |decal| {
                        let mut verts = [zero; 4];
                        (svc.decal_vertices)(decal, verts.as_mut_ptr().cast());
                        verts
                    },
                    |decal| (svc.decal_actor_texture)(decal),
                );
            }
        }
        texture
    }
}
