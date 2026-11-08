//! AActor networking and misc logic (`Engine/Src/UnActor.cpp`, the part past the
//! replication block). The C++ `AActor` methods forward here. Engine objects
//! cross as raw pointers. Every `Hp2ActorServices` callback returns a status;
//! domain results are written through separate output pointers only on success.
//! Policies stop at the first failure, and native wrappers rethrow after Rust
//! returns. No exclusive native-object reference or receive-state lock spans a
//! callback: native virtual dispatch may reenter and mutate the same objects.
//!
//! Null `this`/`Other` is not checked. The C++ dereferences them, so the port
//! does the same raw dereference. That is intentional: the crash is preserved,
//! and no zero or no-op stands in for it.

use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use crate::native_guard::guarded;
use crate::prim;
use crate::vec::size_squared;
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, AMover, APawn, APlayerPawn, APlayerReplicationInfo, FArchive, FBox,
    FBspSurf, FCheckResult, FDecal, FRotator, UClass, UDemoRecDriver, UGameEngine, ULevel,
    UModel, UNetConnection, UNetDriver, UObject, UPlayer, USkeletalMesh, UTexture,
};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::math;

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

/// `AActor::GetTopOwner`: follows the native owner chain to its non-owned root.
///
/// Like the original inline implementation, the actor pointer is dereferenced
/// directly and a cyclic owner graph never terminates.
pub unsafe fn get_top_owner(actor: *mut AActor) -> *mut AActor {
    let mut top = actor;
    while unsafe { !(*top).owner.is_null() } {
        top = unsafe { (*top).owner.cast::<AActor>() };
    }
    top
}

/// `AActor::ToLocal`: inverse actor rotation followed by inverse location.
///
/// # Safety
/// `actor` must point to a live `AActor` whose transform is stable for the call.
pub unsafe fn to_local(actor: *const AActor) -> FCoords {
    unsafe {
        let coords = math::coords_div_rotator(
            &math::unit_coords(),
            math::FRotator {
                pitch: (*actor).rotation.pitch,
                yaw: (*actor).rotation.yaw,
                roll: (*actor).rotation.roll,
            },
            math::trig_table(),
        );
        math::coords_div_point(coords, (*actor).location)
    }
}

/// `AActor::IsBasedOn`: walks `Base`, including the initial actor.
///
/// The native implementation intentionally has no cycle detection. Preserve
/// that behavior rather than silently changing malformed base-chain handling.
pub unsafe fn is_based_on(actor: *const AActor, other: *const AActor) -> bool {
    let mut test = actor;
    while !test.is_null() {
        if test == other {
            return true;
        }
        test = unsafe { (*test).base.cast::<AActor>() };
    }
    false
}

/// `AActor::IsInZone`: actors whose recorded zone is their level's default
/// zone match every query; other actors match only their recorded zone.
pub unsafe fn is_in_zone(actor: *const AActor, zone: *const c_void) -> bool {
    let actor = unsafe { &*actor };
    actor.region.zone == actor.level || actor.region.zone == zone.cast_mut()
}

/// `AActor::WorldSoundRadius`.
pub fn world_sound_radius(actor: &AActor) -> f32 {
    25.0 * (f32::from(actor.sound_radius) + 1.0)
}

/// `AActor::WorldVolumetricRadius`.
pub fn world_volumetric_radius(actor: &AActor) -> f32 {
    25.0 * (f32::from(actor.volume_radius) + 1.0)
}

/// `AActor::WorldLightRadius`.
pub fn world_light_radius(actor: &AActor) -> f32 {
    let scale = if actor.draw_scale > 1.0 {
        actor.draw_scale
    } else {
        1.0
    };
    scale * 25.0 * (f32::from(actor.light_radius) + 1.0)
}

/// `AActor::GetLevel`.
pub unsafe fn get_level(actor: *const AActor) -> *mut c_void {
    unsafe { (*actor).x_level }
}

/// `AActor::IsPendingKill`.
pub unsafe fn is_pending_kill(actor: *const AActor) -> bool {
    unsafe { (*actor).b_delete_me() }
}
/// `PHYS_MovingBrush` (`EngineClasses.h`).
const PHYS_MOVING_BRUSH: u8 = 9;
/// `NM_Client` (`EngineClasses.h`).
const NM_CLIENT: u8 = 3;
/// `RF_Transactional` (`UnObjBas.h`).
const RF_TRANSACTIONAL: u32 = 0x0000_0001;
/// `RF_DebugSerialize` (`UnObjBas.h`).
const RF_DEBUG_SERIALIZE: u32 = 0x4000_0000;
/// `CLASS_Localized` (`UnObjBas.h`).
const CLASS_LOCALIZED: u32 = 0x0000_0020;

/// Engine calls the port needs. Every function takes engine objects as
/// `void*` and does the engine-side work in C++.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ActorServices {
    pub level_hash_add_actor: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void) -> i32,
    pub level_hash_remove_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_is_a:
        unsafe extern "C" fn(obj: *const c_void, class_id: u32, out_value: *mut i32) -> i32,
    pub level_far_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        dest: *const c_void,
        test: i32,
        no_check: i32,
        out_value: *mut i32,
    ) -> i32,
    pub level_move_actor_to_rotation:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, rot: *const c_void) -> i32,
    pub actor_move_smooth:
        unsafe extern "C" fn(actor: *mut c_void, delta: *const c_void, out_value: *mut i32) -> i32,
    pub actor_set_physics: unsafe extern "C" fn(actor: *mut c_void, phys: u8) -> i32,
    pub actor_event_bump: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub actor_event_detach: unsafe extern "C" fn(base: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_event_attach: unsafe extern "C" fn(base: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_event_base_change: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_event_lost_child: unsafe extern "C" fn(owner: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_event_gained_child:
        unsafe extern "C" fn(owner: *mut c_void, actor: *mut c_void) -> i32,
    pub level_viewport0_actor_pri:
        unsafe extern "C" fn(level: *mut c_void, out_value: *mut *mut c_void) -> i32,
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    pub actor_event_touch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub actor_event_untouch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub is_editor: unsafe extern "C" fn(out_value: *mut i32) -> i32,
    pub gcache_flush: unsafe extern "C" fn() -> i32,
    pub primitive_validate_actor:
        unsafe extern "C" fn(prim: *mut c_void, actor: *mut c_void) -> i32,
    pub uobject_process_event: unsafe extern "C" fn(
        obj: *mut c_void,
        func: *mut c_void,
        parms: *mut c_void,
        result: *mut c_void,
    ) -> i32,
    pub uobject_destroy: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub render_interface_destroy: unsafe extern "C" fn(ri: *mut c_void) -> i32,
    pub object_conditional_destroy: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub actor_load_localized: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub uobject_post_load: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub actor_event_possess: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    pub actor_log_possessed: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    pub mesh_collision_extent:
        unsafe extern "C" fn(mesh: *mut c_void, actor: *mut c_void, out: *mut c_void) -> i32,
    pub brush_collision_extent:
        unsafe extern "C" fn(brush: *mut c_void, actor: *mut c_void, out: *mut c_void) -> i32,
    pub mesh_render_bounding_box: unsafe extern "C" fn(
        mesh: *mut c_void,
        actor: *mut c_void,
        exact: i32,
        out: *mut c_void,
    ) -> i32,
    pub brush_render_bounding_box_coords: unsafe extern "C" fn(
        brush: *mut c_void,
        actor: *mut c_void,
        exact: i32,
        out: *mut c_void,
    ) -> i32,
    pub class_is_uclass_static: unsafe extern "C" fn(cls: *mut c_void, out_value: *mut i32) -> i32,
    pub archive_preload: unsafe extern "C" fn(ar: *mut c_void, obj: *mut c_void) -> i32,
    pub object_serialize_tagged_persistent:
        unsafe extern "C" fn(obj: *mut c_void, ar: *mut c_void) -> i32,
    pub object_serialize_bin: unsafe extern "C" fn(obj: *mut c_void, ar: *mut c_void) -> i32,
    pub archive_count_bytes: unsafe extern "C" fn(ar: *mut c_void, size: usize) -> i32,
    pub trace_line_texture_hit: unsafe extern "C" fn(
        actor: *mut c_void,
        trace_end: *const c_void,
        trace_start: *const c_void,
        out_location: *mut c_void,
        out_value: *mut *mut c_void,
    ) -> i32,
    pub bsp_surf_decal_count: unsafe extern "C" fn(surf: *mut c_void, out_value: *mut i32) -> i32,
    pub model_vector:
        unsafe extern "C" fn(model: *mut c_void, index: i32, out_value: *mut *const c_void) -> i32,
    pub model_point:
        unsafe extern "C" fn(model: *mut c_void, index: i32, out_value: *mut *const c_void) -> i32,
    pub bsp_surf_decal:
        unsafe extern "C" fn(surf: *mut c_void, index: i32, out_value: *mut *mut c_void) -> i32,
    pub actor_get_primitive:
        unsafe extern "C" fn(actor: *mut c_void, out_value: *mut *mut c_void) -> i32,
    pub object_is_valid: unsafe extern "C" fn(obj: *mut c_void, out_value: *mut i32) -> i32,
    pub actor_end_touch:
        unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, no_notify: i32) -> i32,
    pub actor_touch_to:
        unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, out_value: *mut i32) -> i32,
    pub render_interface_remove_root: unsafe extern "C" fn(ri: *mut c_void) -> i32,
    /// `this->performPhysics(DeltaSeconds)` through the live C++ vtable.
    pub actor_perform_physics: unsafe extern "C" fn(actor: *mut c_void, delta_seconds: f32) -> i32,
}

impl Hp2ActorServices {
    unsafe fn level_has_hash(&self, level: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { i32::from(!(*level.cast::<ULevel>()).hash.is_null()) })
    }
    unsafe fn level_hash_add_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_hash_add_actor)(level, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_hash_remove_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_hash_remove_actor)(level, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_is_a(&self, obj: *const c_void, class_id: u32) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_is_a)(obj, class_id, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_far_move_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        dest: *const c_void,
        test: i32,
        no_check: i32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.level_far_move_actor)(level, actor, dest, test, no_check, out.as_mut_ptr())
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_move_actor_to_rotation(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        rot: *const c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_move_actor_to_rotation)(level, actor, rot) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_move_smooth(
        &self,
        actor: *mut c_void,
        delta: *const c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_move_smooth)(actor, delta, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_set_physics(&self, actor: *mut c_void, phys: u8) -> Result<(), i32> {
        let status = unsafe { (self.actor_set_physics)(actor, phys) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_bump(&self, actor: *mut c_void, other: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_bump)(actor, other) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_detach(&self, base: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_detach)(base, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_attach(&self, base: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_attach)(base, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_base_change(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_base_change)(actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_lost_child(
        &self,
        owner: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_lost_child)(owner, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_gained_child(
        &self,
        owner: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_gained_child)(owner, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_demo_rec_has_server_connection(&self, level: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe {
            let demo_rec_driver = (*level.cast::<ULevel>()).base.demo_rec_driver
                .cast::<UDemoRecDriver>();
            i32::from(!(*demo_rec_driver).base.server_connection.is_null())
        })
    }
    unsafe fn level_viewport0_actor_pri(&self, level: *mut c_void) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_viewport0_actor_pri)(level, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_net_average_frame_time(&self, level: *mut c_void) -> Result<f32, i32> {
        Ok(unsafe {
            let net_driver = (*level.cast::<ULevel>()).base.net_driver.cast::<UNetDriver>();
            let server_connection = (*net_driver).server_connection.cast::<UNetConnection>();
            (*server_connection).average_frame_time
        })
    }
    unsafe fn check_failed(&self, expr: *const c_char, line: i32) -> Result<(), i32> {
        let status = unsafe { (self.check_failed)(expr, line) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_touch(&self, actor: *mut c_void, other: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_touch)(actor, other) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_event_untouch(
        &self,
        actor: *mut c_void,
        other: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_untouch)(actor, other) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn is_editor(&self) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.is_editor)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn gcache_flush(&self) -> Result<(), i32> {
        let status = unsafe { (self.gcache_flush)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_engine_primitive(
        &self,
        level: *mut c_void,
        slot: u8,
    ) -> Result<*mut c_void, i32> {
        Ok(unsafe {
            let engine = (*level.cast::<ULevel>()).base.engine.cast::<UGameEngine>();
            match slot {
                1 => (*engine).o_cylinder,
                2 => (*engine).r#box,
                4 => (*engine).oval_cylinder,
                5 => (*engine).o_oval_cylinder,
                _ => (*engine).cylinder,
            }
        })
    }
    unsafe fn primitive_validate_actor(
        &self,
        prim: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.primitive_validate_actor)(prim, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_begun_play(&self, level_info: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { i32::from((*level_info.cast::<ALevelInfo>()).b_begun_play()) })
    }
    unsafe fn uobject_process_event(
        &self,
        obj: *mut c_void,
        func: *mut c_void,
        parms: *mut c_void,
        result: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.uobject_process_event)(obj, func, parms, result) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn uobject_destroy(&self, obj: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.uobject_destroy)(obj) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn render_interface_destroy(&self, ri: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.render_interface_destroy)(ri) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn object_conditional_destroy(&self, obj: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.object_conditional_destroy)(obj) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn object_set_transactional(&self, obj: *mut c_void) -> Result<(), i32> {
        unsafe {
            (*obj.cast::<UObject>()).object_flags |= RF_TRANSACTIONAL;
        }
        Ok(())
    }
    unsafe fn actor_class_is_localized(&self, actor: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe {
            let class = (*actor.cast::<UObject>()).class.cast::<UClass>();
            i32::from((*class).class_flags & CLASS_LOCALIZED != 0)
        })
    }
    unsafe fn actor_load_localized(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_load_localized)(actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn uobject_post_load(&self, obj: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.uobject_post_load)(obj) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn model_polys(&self, model: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*model.cast::<UModel>()).polys })
    }
    unsafe fn player_actor(&self, player: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*player.cast::<UPlayer>()).actor })
    }
    unsafe fn player_set_actor(&self, player: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        unsafe {
            (*player.cast::<UPlayer>()).actor = actor;
        }
        Ok(())
    }
    unsafe fn actor_event_possess(&self, pawn: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_event_possess)(pawn) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_log_possessed(&self, pawn: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_log_possessed)(pawn) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn mesh_collision_extent(
        &self,
        mesh: *mut c_void,
        actor: *mut c_void,
        out: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.mesh_collision_extent)(mesh, actor, out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn brush_collision_extent(
        &self,
        brush: *mut c_void,
        actor: *mut c_void,
        out: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.brush_collision_extent)(brush, actor, out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn mesh_render_bounding_box(
        &self,
        mesh: *mut c_void,
        actor: *mut c_void,
        exact: i32,
        out: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.mesh_render_bounding_box)(mesh, actor, exact, out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn brush_render_bounding_box_coords(
        &self,
        brush: *mut c_void,
        actor: *mut c_void,
        exact: i32,
        out: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.brush_render_bounding_box_coords)(brush, actor, exact, out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn object_set_debug_serialize(&self, obj: *mut c_void) -> Result<(), i32> {
        unsafe {
            (*obj.cast::<UObject>()).object_flags |= RF_DEBUG_SERIALIZE;
        }
        Ok(())
    }
    unsafe fn object_class(&self, obj: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*obj.cast::<UObject>()).class })
    }
    unsafe fn class_is_uclass_static(&self, cls: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.class_is_uclass_static)(cls, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn archive_preload(&self, ar: *mut c_void, obj: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.archive_preload)(ar, obj) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn archive_is_loading(&self, ar: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { i32::from((*ar.cast::<FArchive>()).ar_is_loading != 0) })
    }
    unsafe fn archive_is_saving(&self, ar: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { i32::from((*ar.cast::<FArchive>()).ar_is_saving != 0) })
    }
    unsafe fn archive_is_trans(&self, ar: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { i32::from((*ar.cast::<FArchive>()).ar_is_trans != 0) })
    }
    unsafe fn object_serialize_tagged_persistent(
        &self,
        obj: *mut c_void,
        ar: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.object_serialize_tagged_persistent)(obj, ar) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn object_serialize_bin(&self, obj: *mut c_void, ar: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.object_serialize_bin)(obj, ar) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn object_properties_size(&self, obj: *mut c_void) -> Result<usize, i32> {
        Ok(unsafe {
            let class = (*obj.cast::<UObject>()).class.cast::<UClass>();
            (*class).properties_size as usize
        })
    }
    unsafe fn archive_count_bytes(&self, ar: *mut c_void, size: usize) -> Result<(), i32> {
        let status = unsafe { (self.archive_count_bytes)(ar, size) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn trace_line_texture_hit(
        &self,
        actor: *mut c_void,
        trace_end: *const c_void,
        trace_start: *const c_void,
        out_location: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.trace_line_texture_hit)(
                actor,
                trace_end,
                trace_start,
                out_location,
                out.as_mut_ptr(),
            )
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_x_level_model(&self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*(*actor.cast::<AActor>()).x_level.cast::<ULevel>()).model })
    }
    unsafe fn bsp_surf_texture(&self, surf: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*surf.cast::<FBspSurf>()).texture })
    }
    unsafe fn bsp_surf_poly_flags(&self, surf: *mut c_void) -> Result<u32, i32> {
        Ok(unsafe { (*surf.cast::<FBspSurf>()).poly_flags })
    }
    unsafe fn texture_poly_flags(&self, texture: *mut c_void) -> Result<u32, i32> {
        Ok(unsafe { (*texture.cast::<UTexture>()).poly_flags })
    }
    unsafe fn bsp_surf_decal_count(&self, surf: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.bsp_surf_decal_count)(surf, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn bsp_surf_v_normal(&self, surf: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*surf.cast::<FBspSurf>()).v_normal })
    }
    unsafe fn bsp_surf_p_base(&self, surf: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*surf.cast::<FBspSurf>()).p_base })
    }
    unsafe fn model_vector(&self, model: *mut c_void, index: i32) -> Result<*const c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.model_vector)(model, index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn model_point(&self, model: *mut c_void, index: i32) -> Result<*const c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.model_point)(model, index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn bsp_surf_decal(&self, surf: *mut c_void, index: i32) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.bsp_surf_decal)(surf, index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn decal_vertices(&self, decal: *mut c_void) -> Result<*const c_void, i32> {
        Ok(unsafe { (*decal.cast::<FDecal>()).vertices.as_ptr().cast::<c_void>() })
    }
    unsafe fn decal_actor_texture(&self, decal: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*(*decal.cast::<FDecal>()).actor.cast::<AActor>()).texture })
    }
    unsafe fn actor_get_primitive(&self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_get_primitive)(actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn object_is_valid(&self, obj: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.object_is_valid)(obj, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_end_touch(
        &self,
        actor: *mut c_void,
        other: *mut c_void,
        no_notify: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_end_touch)(actor, other, no_notify) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_touch_to(&self, actor: *mut c_void, other: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_touch_to)(actor, other, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn render_interface_remove_root(&self, ri: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.render_interface_remove_root)(ri) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    /// `this->performPhysics(DeltaSeconds)` through the live C++ vtable
    /// (`AActor`/`APawn`/`AInterpolationManager` each override it).
    pub(crate) unsafe fn actor_perform_physics(
        &self,
        actor: *mut c_void,
        delta_seconds: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_perform_physics)(actor, delta_seconds) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
}

static mut ACTOR_SERVICES: Option<Hp2ActorServices> = None;

/// # Safety
/// `value` is the real, immutable `GHp2ActorServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2ActorServices) {
    unsafe { (&raw mut ACTOR_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2ActorServices, i32> {
    unsafe { (&raw const ACTOR_SERVICES).read().ok_or(1) }
}

/// `appFailAssert` for a failed `check()`. The C++ adapter fixes the file to
/// `UnActor.cpp` and passes the oracle line.
fn touch_check(svc: &Hp2ActorServices, ok: bool, expr: &CStr, line: i32) -> Result<(), i32> {
    if !ok {
        unsafe { svc.check_failed(expr.as_ptr(), line)? };
    }
    Ok(())
}

/// Engine leaves used by the shared TouchTo/BeginTouch relationship policy.
pub(crate) trait ActorTouchOps {
    type Error;
    unsafe fn validate_pair(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn validate_touch(&mut self, object: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn is_a(&mut self, actor: *mut AActor, class_id: u32) -> Result<bool, Self::Error>;
    unsafe fn is_player(&mut self, actor: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn end_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn event_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error>;
}

/// Shared `TouchTo(Actor, Other)` slot selection and callback policy.
///
/// # Safety
/// `actor` and `other` must be live actors.
pub(crate) unsafe fn touch_to_with_ops<O: ActorTouchOps>(
    ops: &mut O,
    actor: *mut AActor,
    other: *mut AActor,
) -> Result<bool, O::Error> {
    unsafe { ops.validate_pair(actor, other)?; }
    let other_object = other.cast::<c_void>();
    let mut available = -1i32;
    for index in 0..4 {
        let touching = unsafe { (*actor).touching[index] };
        if touching.is_null() {
            available = index as i32;
        } else if touching == other_object {
            return Ok(true);
        }
    }
    if available == -1 {
        for index in 0..4 {
            let touching = unsafe { (*actor).touching[index] };
            unsafe { ops.validate_touch(touching)?; }
            if unsafe { (*touching.cast::<AActor>()).physics } == PHYS_NONE {
                unsafe { ops.end_touch(actor, touching.cast())?; }
                available = index as i32;
            }
        }
        if available == -1 && unsafe { ops.is_a(other, class::APAWN)? } {
            for index in 0..4 {
                let touching = unsafe { (*actor).touching[index].cast::<AActor>() };
                if !unsafe { ops.is_a(touching, class::APAWN)? } {
                    unsafe { ops.end_touch(actor, touching)?; }
                    available = index as i32;
                    break;
                }
            }
            if available == -1 && unsafe { ops.is_player(other)? } {
                for index in 0..4 {
                    let touching = unsafe { (*actor).touching[index].cast::<AActor>() };
                    if !unsafe { ops.is_a(touching, class::APAWN)? }
                        || !unsafe { ops.is_player(touching)? }
                    {
                        unsafe { ops.end_touch(actor, touching)?; }
                        available = index as i32;
                        break;
                    }
                }
            }
        }
    }
    if available >= 0 {
        unsafe { (*actor).touching[available as usize] = other_object; }
        unsafe { ops.event_touch(actor, other)?; }
        if unsafe { (*actor).touching[available as usize] } != other_object {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Shared projectile/trigger-aware BeginTouch dispatch policy.
///
/// # Safety
/// Both actors must remain live for the duration of each operation.
pub(crate) unsafe fn begin_touch_with_ops<O: ActorTouchOps>(
    ops: &mut O,
    actor: *mut AActor,
    other: *mut AActor,
) -> Result<(), O::Error> {
    if unsafe { ops.is_a(actor, class::APROJECTILE)? }
        && unsafe { ops.is_a(other, class::ATRIGGER)? }
    {
        unsafe { touch_to_with_ops(ops, other, actor)?; }
        if !unsafe { (*actor).b_delete_me() } && !unsafe { (*other).b_delete_me() } {
            unsafe { touch_to_with_ops(ops, actor, other)?; }
        }
    } else if unsafe { ops.is_a(other, class::APROJECTILE)? }
        && unsafe { ops.is_a(actor, class::ATRIGGER)? }
    {
        unsafe { touch_to_with_ops(ops, actor, other)?; }
        if !unsafe { (*actor).b_delete_me() } && !unsafe { (*other).b_delete_me() } {
            unsafe { touch_to_with_ops(ops, other, actor)?; }
        }
    } else if unsafe { touch_to_with_ops(ops, actor, other)? } {
        unsafe { touch_to_with_ops(ops, other, actor)?; }
    }
    Ok(())
}

/// `TouchTo(Actor, Other)`: records `Other` in `Actor`'s touch slots, pruning
/// stale touches when all four slots are full. Returns `UBOOL`.
///
/// Native assertions, including `UObject::IsValid`, use checked leaves. Nested
/// `EndTouch` calls retain their own native guard and re-read the live slots.
///
/// # Safety
/// `actor` and `other` must be live `AActor`s.
pub unsafe fn touch_to(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    other: *mut AActor,
) -> Result<i32, i32> {
    let mut ops = NativeActorTouchOps { svc };
    unsafe {
        touch_to_with_ops(&mut ops, actor, other).map(|touched| if touched { 1 } else { 0 })
    }
}

struct NativeActorTouchOps<'a> {
    svc: &'a Hp2ActorServices,
}

impl ActorTouchOps for NativeActorTouchOps<'_> {
    type Error = i32;

    unsafe fn validate_pair(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), i32> {
        unsafe {
            touch_check(self.svc, !actor.is_null(), c"Actor", 1290)?;
            touch_check(self.svc, !other.is_null(), c"Other", 1291)?;
            touch_check(self.svc, actor != other, c"Actor!=Other", 1292)?;
        }
        Ok(())
    }

    unsafe fn validate_touch(&mut self, object: *mut c_void) -> Result<(), i32> {
        unsafe { touch_check(self.svc, self.svc.object_is_valid(object)? != 0, c"Actor->Touching[i]->IsValid()", 1313) }
    }

    unsafe fn is_a(&mut self, actor: *mut AActor, class_id: u32) -> Result<bool, i32> {
        Ok(unsafe { self.svc.actor_is_a(actor.cast(), class_id)? != 0 })
    }

    unsafe fn is_player(&mut self, actor: *mut AActor) -> Result<bool, i32> {
        Ok(unsafe { (*actor.cast::<APawn>()).b_is_player() })
    }

    unsafe fn end_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), i32> {
        unsafe { self.svc.actor_end_touch(actor.cast(), other.cast(), 0) }
    }

    unsafe fn event_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), i32> {
        unsafe { self.svc.actor_event_touch(actor.cast(), other.cast()) }
    }
}

/// The original file-static network exchange state is intentionally shared and
/// live across reentrant receives. Native engine execution is single-threaded;
/// callers must serialize threads, but must not lock out callback reentry.
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

static mut SAVED: SavedNet = SavedNet {
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
};

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

/// Backend operations shared by native actor collision policy and canonical Core adapters.
pub(crate) trait ActorCollisionOps {
    type Error;
    unsafe fn has_hash(&mut self, level: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn hash_remove(&mut self, level: *mut c_void, actor: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn hash_add(&mut self, level: *mut c_void, actor: *mut AActor) -> Result<(), Self::Error>;
}

/// Updates collision flags and the actor hash through backend operations.
///
/// # Safety
/// `actor`, its `XLevel`, and objects reached by backend operations must be live.
pub(crate) unsafe fn set_collision_with_ops<O: ActorCollisionOps>(
    ops: &mut O,
    actor: *mut AActor,
    new_collide_actors: bool,
    new_block_actors: bool,
    new_block_players: bool,
) -> Result<(), O::Error> {
    let level = unsafe { (*actor).x_level };
    if unsafe { (*actor).b_collide_actors() } && unsafe { ops.has_hash(level)? } {
        unsafe { ops.hash_remove(level, actor)?; }
    }
    unsafe {
        (*actor).set_b_collide_actors(new_collide_actors);
        (*actor).set_b_block_actors(new_block_actors);
        (*actor).set_b_block_players(new_block_players);
    }
    if unsafe { (*actor).b_collide_actors() } && unsafe { ops.has_hash(level)? } {
        unsafe { ops.hash_add(level, actor)?; }
    }
    Ok(())
}

/// Resizes actor collision dimensions while preserving hash remove/add ordering.
///
/// # Safety
/// `actor`, its `XLevel`, and objects reached by backend operations must be live.
pub(crate) unsafe fn set_collision_size_with_ops<O: ActorCollisionOps>(
    ops: &mut O,
    actor: *mut AActor,
    new_radius: f32,
    new_height: f32,
) -> Result<(), O::Error> {
    let level = unsafe { (*actor).x_level };
    if unsafe { (*actor).b_collide_actors() } && unsafe { ops.has_hash(level)? } {
        unsafe { ops.hash_remove(level, actor)?; }
    }
    unsafe {
        (*actor).collision_radius = new_radius;
        (*actor).collision_height = new_height;
    }
    if unsafe { (*actor).b_collide_actors() } && unsafe { ops.has_hash(level)? } {
        unsafe { ops.hash_add(level, actor)?; }
    }
    Ok(())
}

struct NativeActorCollisionOps<'a>(&'a Hp2ActorServices);

impl ActorCollisionOps for NativeActorCollisionOps<'_> {
    type Error = i32;
    unsafe fn has_hash(&mut self, level: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { self.0.level_has_hash(level)? != 0 })
    }
    unsafe fn hash_remove(&mut self, level: *mut c_void, actor: *mut AActor) -> Result<(), i32> {
        unsafe { self.0.level_hash_remove_actor(level, actor.cast()) }
    }
    unsafe fn hash_add(&mut self, level: *mut c_void, actor: *mut AActor) -> Result<(), i32> {
        unsafe { self.0.level_hash_add_actor(level, actor.cast()) }
    }
}

/// `AActor::SetCollision`: update collision bits and maintain the level hash.
///
/// # Safety
/// `actor` must be a live `AActor` whose `XLevel` is a live `ULevel`.
pub unsafe fn set_collision(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_collide_actors: bool,
    new_block_actors: bool,
    new_block_players: bool,
) -> Result<(), i32> {
    unsafe {
        guarded(c"AActor::SetCollision", || {
            let mut ops = NativeActorCollisionOps(svc);
            set_collision_with_ops(&mut ops, actor, new_collide_actors, new_block_actors, new_block_players)
        })
    }
}

/// `AActor::SetCollisionSize`: update dimensions while maintaining the level hash.
///
/// # Safety
/// Same requirements as [`set_collision`].
pub unsafe fn set_collision_size(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_radius: f32,
    new_height: f32,
) -> Result<(), i32> {
    unsafe {
        guarded(c"AActor::SetCollisionSize", || {
            let mut ops = NativeActorCollisionOps(svc);
            set_collision_size_with_ops(&mut ops, actor, new_radius, new_height)
        })
    }
}

/// `AActor::execSetCollisionSize`: assign script width, resize, then return true.
///
/// # Safety
/// `actor` must satisfy [`set_collision_size`]'s requirements; `out_result`
/// must point to a writable `DWORD`.
pub unsafe fn exec_set_collision_size(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_radius: f32,
    new_height: f32,
    new_width: f32,
    out_result: *mut u32,
) -> Result<i32, i32> {
    Ok({
        if actor.is_null() || out_result.is_null() {
            return Err(1);
        }
        unsafe {
            (*actor).collision_width = new_width;
            set_collision_size(svc, actor, new_radius, new_height)?;
            *out_result = 1;
        }
        0
    })
}

/// `AActor::execSetLocation`: call `FarMoveActor` with its C++ defaults
/// (`Test=0`, `bNoCheck=0`) and write the UBOOL result.
///
/// # Safety
/// `actor` and `out_result` must be live pointers for this call.
pub unsafe fn exec_set_location(
    actor: *mut AActor,
    destination: *const FVector,
    out_result: *mut u32,
) -> Result<i32, i32> {
    Ok(unsafe {
        let (lsvc, gmem) = crate::levact::published_services()?;
        let ctx = crate::levact::LevelCtx {
            svc: lsvc,
            gmem,
            ulevel: (*actor).x_level.cast(),
        };
        let destination_v = *destination;
        let moved =
            ctx.svc
                .virtual_far_move_actor(ctx.ulevel.cast(), actor.cast(), &destination_v, 0, 0)?;
        out_result.write_unaligned((moved != 0) as u32);
        0
    })
}

/// `AActor::execSetRotation`: `MoveActor(this, (0,0,0), NewRotation, Hit)`
/// with the engine's default movement flags, then write its UBOOL result.
///
/// # Safety
/// `actor` and `out_result` must be live pointers for this call.
pub unsafe fn exec_set_rotation(
    actor: *mut AActor,
    rotation: FRotator,
    out_result: *mut c_void,
) -> i32 {
    unsafe {
        let (lsvc, gmem) = match crate::levact::published_services() {
            Ok(value) => value,
            Err(status) => return status,
        };
        let ctx = crate::levact::LevelCtx {
            svc: lsvc,
            gmem,
            ulevel: (*actor).x_level.cast(),
        };
        let mut hit: FCheckResult = std::mem::zeroed();
        let zero = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        match ctx.svc.virtual_move_actor(
            ctx.ulevel.cast(),
            actor.cast(),
            &zero,
            &rotation,
            &mut hit,
            0,
            0,
            0,
            0,
        ) {
            Ok(moved) => {
                out_result.cast::<u32>().write_unaligned((moved != 0) as u32);
                0
            }
            Err(status) => status,
        }
    }
}

/// `AActor::execMove`: `MoveActor(this, Delta, Rotation, Hit)`.
///
/// # Safety
/// `actor` and `out_result` must be live pointers for this call.
pub unsafe fn exec_move(actor: *mut AActor, delta: FVector, out_result: *mut c_void) -> i32 {
    unsafe {
        let (lsvc, gmem) = match crate::levact::published_services() {
            Ok(value) => value,
            Err(status) => return status,
        };
        let ctx = crate::levact::LevelCtx {
            svc: lsvc,
            gmem,
            ulevel: (*actor).x_level.cast(),
        };
        let rotation = std::ptr::addr_of!((*actor).rotation).read_unaligned();
        let mut hit: FCheckResult = std::mem::zeroed();
        match ctx.svc.virtual_move_actor(
            ctx.ulevel.cast(),
            actor.cast(),
            &delta,
            &rotation,
            &mut hit,
            0,
            0,
            0,
            0,
        ) {
            Ok(moved) => {
                out_result.cast::<u32>().write_unaligned((moved != 0) as u32);
                0
            }
            Err(status) => status,
        }
    }
}

/// Narrow adapter for `AActor::execFastTrace`'s model line check.
#[repr(C)]
pub struct Hp2ActorFastTraceServices {
    pub context: *mut c_void,
    pub fast_line_check: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        trace_end: *const c_void,
        trace_start: *const c_void,
        out_result: *mut c_void,
    ) -> i32,
}

/// `AActor::execFastTrace`: check `TraceEnd` from `TraceStart`.
///
/// # Safety
/// `actor`, `out_result`, and `services` must remain valid for this call.
pub unsafe fn exec_fast_trace(
    actor: *mut AActor,
    trace_end: FVector,
    trace_start: FVector,
    out_result: *mut c_void,
    services: &Hp2ActorFastTraceServices,
) -> i32 {
    unsafe {
        (services.fast_line_check)(
            services.context,
            actor.cast(),
            (&trace_end as *const FVector).cast(),
            (&trace_start as *const FVector).cast(),
            out_result,
        )
    }
}

/// Narrow adapter for the native `USound::GetDuration` query.
#[repr(C)]
pub struct Hp2ActorSoundDurationServices {
    pub context: *mut c_void,
    pub get_duration: unsafe extern "C" fn(
        context: *mut c_void,
        sound: *mut c_void,
        out_duration: *mut c_void,
    ) -> i32,
}

/// `AActor::execGetSoundDuration`: return zero for no sound, else its duration.
///
/// # Safety
/// `out_result` and `services` must remain valid; non-null `sound` must be live.
pub unsafe fn exec_get_sound_duration(
    sound: *mut c_void,
    out_result: *mut c_void,
    services: &Hp2ActorSoundDurationServices,
) -> i32 {
    unsafe {
        if sound.is_null() {
            out_result.cast::<f32>().write_unaligned(0.0);
            return 0;
        }
        (services.get_duration)(services.context, sound, out_result)
    }
}

/// `AActor::execTrace`: select collision flags and dispatch the line check
/// directly through the published levact hub (`ULevel::SingleLineCheck`).
///
/// # Safety
/// `actor` and the three output pointers must be valid for this call.
pub unsafe fn exec_trace(
    actor: *mut AActor,
    trace_end: FVector,
    trace_start: FVector,
    trace_actors: i32,
    trace_extent: FVector,
    out_hit_actor: *mut c_void,
    out_hit_location: *mut c_void,
    out_hit_normal: *mut c_void,
) -> i32 {
    let trace_flags = if trace_actors != 0 {
        crate::levact::TRACE_ALL_COLLIDING
    } else {
        crate::levact::TRACE_LEVEL | crate::levact::TRACE_MOVERS
    };
    unsafe {
        let (svc, gmem) = match crate::levact::published_services() {
            Ok(value) => value,
            Err(status) => return status,
        };
        let ctx = crate::levact::LevelCtx {
            svc,
            gmem,
            ulevel: get_level(actor).cast(),
        };
        let mut hit: FCheckResult = std::mem::zeroed();
        hit.time = 1.0;
        let query = crate::levact::LineQuery {
            end: trace_end,
            start: trace_start,
            extent: trace_extent,
        };
        match ctx.svc.virtual_single_line_check(
            ctx.ulevel.cast(),
            &mut hit,
            actor.cast(),
            &query.end,
            &query.start,
            trace_flags,
            &query.extent,
            0,
        ) {
            Ok(_) => {
                out_hit_actor.cast::<*mut c_void>().write_unaligned(hit.actor);
                out_hit_location
                    .cast::<FVector>()
                    .write_unaligned(hit.location);
                out_hit_normal.cast::<FVector>().write_unaligned(hit.normal);
                0
            }
            Err(status) => status,
        }
    }
}

/// `AActor::execSetTimer`: reset the counter and replace rate/loop state.
///
/// # Safety
/// `actor` must point to a live `AActor`.
pub unsafe fn exec_set_timer(actor: *mut AActor, timer_rate: f32, looping: i32) {
    unsafe {
        (*actor).timer_counter = 0.0;
        (*actor).timer_rate = timer_rate;
        (*actor).set_b_timer_loop(looping != 0);
    }
}

/// Narrow adapter for `GetStateFrame()->LatentAction`.
#[repr(C)]
pub struct Hp2ActorLatentServices {
    pub context: *mut c_void,
    pub set_latent_action:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, action: i32) -> i32,
}

/// `AActor::execSleep`: set the latent action and countdown.
///
/// # Safety
/// `actor` and `services` must remain valid for the call.
pub unsafe fn exec_sleep(
    actor: *mut AActor,
    seconds: f32,
    sleep_action: i32,
    services: &Hp2ActorLatentServices,
) -> i32 {
    unsafe {
        (*actor).latent_float = seconds;
        (services.set_latent_action)(services.context, actor.cast(), sleep_action)
    }
}

/// `AActor::execPollSleep`: subtract elapsed time and awaken below half a tick.
///
/// # Safety
/// `actor` and `services` must remain valid for the call.
pub unsafe fn poll_sleep(
    actor: *mut AActor,
    delta_seconds: f32,
    services: &Hp2ActorLatentServices,
) -> i32 {
    unsafe {
        (*actor).latent_float -= delta_seconds;
        if (*actor).latent_float < 0.5 * delta_seconds {
            (services.set_latent_action)(services.context, actor.cast(), 0)
        } else {
            0
        }
    }
}

/// `AActor::execFinishInterpolation`: suspend until interpolation completes.
///
/// # Safety
/// `actor` and `services` must remain valid for the call.
pub unsafe fn exec_finish_interpolation(
    actor: *mut AActor,
    latent_action: i32,
    services: &Hp2ActorLatentServices,
) -> i32 {
    unsafe { (services.set_latent_action)(services.context, actor.cast(), latent_action) }
}

/// `AActor::execPollFinishInterpolation`: clear the latent action once
/// `bInterpolating` becomes false.
///
/// # Safety
/// `actor` and `services` must remain valid for the call.
pub unsafe fn poll_finish_interpolation(
    actor: *mut AActor,
    services: &Hp2ActorLatentServices,
) -> i32 {
    unsafe {
        if !(*actor).b_interpolating() {
            (services.set_latent_action)(services.context, actor.cast(), 0)
        } else {
            0
        }
    }
}

/// `AActor::execPollFinishAnim`: clear the latent action when animation finished.
///
/// # Safety
/// `actor` and `services` must remain valid for the call.
pub unsafe fn poll_finish_anim(actor: *mut AActor, services: &Hp2ActorLatentServices) -> i32 {
    unsafe {
        if (*actor).b_anim_finished() {
            (services.set_latent_action)(services.context, actor.cast(), 0)
        } else {
            0
        }
    }
}

/// `AActor::IsOwnedBy`: walk `Owner` from self through the ancestor chain.
///
/// # Safety
/// `actor` and every non-null `Owner` pointer in its chain must point to live
/// actors. Like the original method, cycles are not detected.
pub unsafe fn is_owned_by(actor: *const AActor, test_owner: *const AActor) -> i32 {
    let mut current = actor;
    while !current.is_null() {
        if current == test_owner {
            return 1;
        }
        current = unsafe { (*current).owner.cast::<AActor>() };
    }
    0
}

/// Source operations needed by actor relation transitions.
pub(crate) trait ActorRelationOps {
    type Error;

    /// Dispatches one relation event to its receiver.
    ///
    /// # Safety
    /// Receiver and optional related actor remain live through reentry.
    unsafe fn event(
        &mut self,
        event: ActorRelationEvent,
        receiver: *mut c_void,
        related: *mut c_void,
    ) -> Result<(), Self::Error>;
}

/// Events emitted by SetOwner, SetBase and EndTouch source policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActorRelationEvent {
    LostChild,
    GainedChild,
    Detach,
    Attach,
    BaseChange,
    UnTouch,
}

/// Shared `AActor::SetOwner` transition.
///
/// # Safety
/// `actor` and every non-null owner are live actors.
pub(crate) unsafe fn set_owner_with_ops<O: ActorRelationOps>(
    ops: &mut O,
    actor: *mut AActor,
    new_owner: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        if !(*actor).owner.is_null() {
            ops.event(ActorRelationEvent::LostChild, (*actor).owner, actor.cast())?;
        }
        (*actor).owner = new_owner;
        if !(*actor).owner.is_null() {
            ops.event(ActorRelationEvent::GainedChild, (*actor).owner, actor.cast())?;
        }
        if new_owner.is_null() {
            (*actor).anim_bone = 0;
        }
    }
    Ok(())
}

struct NativeActorRelationOps<'a>(&'a Hp2ActorServices);

impl ActorRelationOps for NativeActorRelationOps<'_> {
    type Error = i32;

    unsafe fn event(
        &mut self,
        event: ActorRelationEvent,
        receiver: *mut c_void,
        related: *mut c_void,
    ) -> Result<(), Self::Error> {
        unsafe {
            match event {
                ActorRelationEvent::LostChild => self.0.actor_event_lost_child(receiver, related),
                ActorRelationEvent::GainedChild => self.0.actor_event_gained_child(receiver, related),
                ActorRelationEvent::Detach => self.0.actor_event_detach(receiver, related),
                ActorRelationEvent::Attach => self.0.actor_event_attach(receiver, related),
                ActorRelationEvent::BaseChange => self.0.actor_event_base_change(receiver),
                ActorRelationEvent::UnTouch => self.0.actor_event_untouch(receiver, related),
            }
        }
    }
}

/// Native adapter for the shared `AActor::SetOwner` transition.
///
/// # Safety
/// `actor` must be live and any non-null `new_owner` must be live.
pub unsafe fn set_owner(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_owner: *mut c_void,
) -> Result<(), i32> {
    unsafe { set_owner_with_ops(&mut NativeActorRelationOps(svc), actor, new_owner) }
}

/// Shared `AActor::SetBase` transition.
///
/// # Safety
/// `actor` and the complete base chain of a non-null `new_base` are live.
pub(crate) unsafe fn set_base_with_ops<O: ActorRelationOps>(
    ops: &mut O,
    actor: *mut AActor,
    new_base: *mut AActor,
    notify_actor: bool,
) -> Result<(), O::Error> {
    unsafe {
        let mut lp = new_base;
        while !lp.is_null() {
            if lp == actor { return Ok(()); }
            lp = (*lp).base.cast();
        }
        if new_base != (*actor).base.cast() {
            let old = (*actor).base.cast::<AActor>();
            if !old.is_null() && old.cast::<c_void>() != (*actor).level {
                (*old).standing_count = (*old).standing_count.wrapping_sub(1);
                ops.event(ActorRelationEvent::Detach, old.cast(), actor.cast())?;
            }
            (*actor).base = new_base.cast();
            if !new_base.is_null() && new_base.cast::<c_void>() != (*actor).level {
                (*new_base).standing_count = (*new_base).standing_count.wrapping_add(1);
                ops.event(ActorRelationEvent::Attach, new_base.cast(), actor.cast())?;
            }
            if notify_actor {
                ops.event(ActorRelationEvent::BaseChange, actor.cast(), ptr::null_mut())?;
            }
        }
    }
    Ok(())
}

/// Native adapter for the shared `AActor::SetBase` transition.
///
/// # Safety
/// `actor` and the complete base chain of a non-null `new_base` are live.
pub unsafe fn set_base(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_base: *mut AActor,
    notify_actor: bool,
) -> Result<(), i32> {
    unsafe {
        guarded(c"AActor::SetBase", || {
            set_base_with_ops(&mut NativeActorRelationOps(svc), actor, new_base, notify_actor)
        })
    }
}


/// `AActor::PreNetReceive`. Saves the replicated state and pulls the actor out
/// of the level hash.
///
/// # Safety
/// `actor` must be a live `AActor` whose `XLevel` is a live `ULevel` with a
/// non-null `Hash` (the C++ dereferences it unconditionally when
/// `bCollideActors` is set).
pub unsafe fn pre_net_receive(svc: &Hp2ActorServices, actor: *mut AActor) -> Result<(), i32> {
    unsafe {
        let s = &raw mut SAVED;
        (*s).location = (*actor).location;
        (*s).rotation = (*actor).rotation;
        (*s).base = (*actor).base as usize;
        (*s).collision = (*actor).b_collide_actors();
        (*s).radius = (*actor).collision_radius;
        (*s).height = (*actor).collision_height;
        (*s).sim_anim = (*actor).sim_anim;
        if svc.actor_is_a(actor.cast(), class::AMOVER)? != 0 {
            (*s).sim_interpolate = (*actor.cast::<AMover>()).sim_interpolate;
        }
        if (*actor).b_collide_actors() {
            svc.level_hash_remove_actor((*actor).x_level, actor.cast())?;
        }
    }
    Ok(())
}

/// `AActor::PostNetReceive`. Swaps the saved state back in and applies the
/// deltas the replicated properties caused.
///
/// # Safety
/// Same requirements as [`pre_net_receive`]. `PreNetReceive` must have run first
/// for the saved values to be meaningful.
pub unsafe fn post_net_receive(svc: &Hp2ActorServices, actor: *mut AActor) -> Result<(), i32> {
    unsafe {
        let s = &raw mut SAVED;
        std::ptr::swap(&raw mut (*actor).location, &raw mut (*s).location);
        std::ptr::swap(&raw mut (*actor).rotation, &raw mut (*s).rotation);
        let cur_base = (*actor).base as usize;
        (*actor).base = (*s).base as *mut c_void;
        (*s).base = cur_base;
        let cur_collide = (*actor).b_collide_actors();
        (*actor).set_b_collide_actors((*s).collision);
        (*s).collision = cur_collide;
        std::ptr::swap(&raw mut (*actor).collision_radius, &raw mut (*s).radius);
        std::ptr::swap(&raw mut (*actor).collision_height, &raw mut (*s).height);
        if (*actor).b_collide_actors() {
            svc.level_hash_add_actor((*actor).x_level, actor.cast())?;
        }

        if svc.actor_is_a(actor.cast(), class::AMOVER)? != 0 {
            let mover = actor.cast::<AMover>();
            if (*s).sim_interpolate != (*mover).sim_interpolate {
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
                svc.actor_set_physics(actor.cast(), PHYS_MOVING_BRUSH)?;
                (*actor).set_b_interpolating(true);
            }
        }
        if svc.actor_is_a(actor.cast(), class::APLAYERPAWN)? != 0
            && !(*(*actor).x_level.cast::<ULevel>()).base.demo_rec_driver
                .is_null()
            && svc.level_demo_rec_has_server_connection((*actor).x_level)? != 0
        {
            let pp = actor.cast::<APlayerPawn>();
            (*pp).base.view_rotation.pitch = (*pp).demo_view_pitch;
            (*pp).base.view_rotation.yaw = (*pp).demo_view_yaw;
        }
        if (*actor).sim_anim != (*s).sim_anim {
            let sa = (*actor).sim_anim;
            (*actor).anim_frame = sa.x * 0.0001_f32;
            (*actor).anim_rate = sa.y * 0.0002_f32;
            (*actor).tween_rate = sa.z * 0.001_f32;
            (*actor).anim_last = sa.w * 0.0001_f32;
            if (*actor).anim_last < 0.0 {
                (*actor).anim_last *= -1.0;
                (*actor).set_b_anim_loop(true);
                if svc.actor_is_a(actor.cast(), class::APAWN)? != 0
                    && (*actor).anim_min_rate < 0.5_f32
                {
                    (*actor).anim_min_rate = 0.5_f32;
                }
            } else {
                (*actor).set_b_anim_loop(false);
            }
        }
        if (*actor).location != (*s).location {
            if svc.actor_is_a(actor.cast(), class::APAWN)? != 0
                && (*actor).role == ROLE_SIMULATED_PROXY
                && !is_nearly_zero((*actor).velocity)
                && size_squared(sub_vec((*actor).location, (*s).location)) < 10000.0_f32
            {
                // Smooth out movement of other players to account for frame-rate jitter.
                let start_error = size_squared(sub_vec((*actor).location, (*s).location));
                let mut new_location = (*actor).location;
                if start_error > 1600.0_f32 {
                    let d = scale_vec(0.35_f32, sub_vec((*s).location, (*actor).location));
                    svc.actor_move_smooth(actor.cast(), (&d as *const FVector).cast())?;
                    if size_squared(sub_vec((*actor).location, (*s).location))
                        > 0.75_f32 * start_error
                    {
                        new_location = add_vec(
                            new_location,
                            scale_vec(0.5_f32, sub_vec((*s).location, (*actor).location)),
                        );
                    }
                } else {
                    new_location = add_vec(
                        new_location,
                        scale_vec(0.15_f32, sub_vec((*s).location, (*actor).location)),
                    );
                }
                svc.level_far_move_actor(
                    (*actor).x_level,
                    actor.cast(),
                    (&new_location as *const FVector).cast(),
                    0,
                    1,
                )?;
            } else {
                svc.level_far_move_actor(
                    (*actor).x_level,
                    actor.cast(),
                    (&(*s).location as *const FVector).cast(),
                    0,
                    1,
                )?;
            }
        }
        if (*actor).rotation.pitch != (*s).rotation.pitch
            || (*actor).rotation.yaw != (*s).rotation.yaw
            || (*actor).rotation.roll != (*s).rotation.roll
        {
            svc.level_move_actor_to_rotation(
                (*actor).x_level,
                actor.cast(),
                (&(*s).rotation as *const FRotator).cast(),
            )?;
        }
        if (*actor).collision_radius != (*s).radius || (*actor).collision_height != (*s).height {
            set_collision_size(svc, actor, (*s).radius, (*s).height)?;
        }
        if (*actor).b_collide_actors() != (*s).collision {
            let block_actors = (*actor).b_block_actors();
            let block_players = (*actor).b_block_players();
            set_collision(
                svc,
                actor,
                (*s).collision,
                block_actors,
                block_players,
            )?;
        }
        if (*actor).base != ((*s).base as *mut AActor).cast() {
            svc.actor_event_bump(actor.cast(), ((*s).base as *mut AActor).cast())?;
            if !((*s).base as *mut AActor).is_null() {
                svc.actor_event_bump(((*s).base as *mut AActor).cast(), actor.cast())?;
            }
            set_base(svc, actor, (*s).base as *mut AActor, true)?;
        }
        (*actor).set_b_just_teleported(false);
        if svc.actor_is_a(actor.cast(), class::APLAYERREPLICATIONINFO)? != 0
            && (*(*actor).level.cast::<ALevelInfo>()).net_mode == NM_CLIENT
        {
            let pri = actor.cast::<APlayerReplicationInfo>();
            if !(*(*actor).x_level.cast::<ULevel>()).base.net_driver.is_null()
                && svc.level_viewport0_actor_pri((*actor).x_level)? == actor.cast()
            {
                // C++ `(INT)` conversion of `(AverageFrameTime * 1000) / 2`.
                let avg = svc.level_net_average_frame_time((*actor).x_level)?;
                let delta = ((avg * 1000.0_f32) / 2.0_f32) as i32;
                (*pri).ping = (*pri).ping.wrapping_sub(delta);
            }
            if (*pri).ping < 0 {
                (*pri).ping = 0;
            }
        }
    }
    Ok(())
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
pub unsafe fn begin_touch(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    other: *mut AActor,
) -> Result<(), i32> {
    let mut ops = NativeActorTouchOps { svc };
    unsafe { begin_touch_with_ops(&mut ops, actor, other) }
}

/// `AActor::EndTouch(other, no_notify_self)`. Clears both touch slots and
/// fires `eventUnTouch` on each side that had one.
///
/// # Safety
/// `actor` and `other` must be live `AActor`s.
/// Shared `AActor::EndTouch(other, no_notify_self)` transition.
///
/// # Safety
/// `actor` and `other` are live actors.
pub(crate) unsafe fn end_touch_with_ops<O: ActorRelationOps>(
    ops: &mut O,
    actor: *mut AActor,
    other: *mut AActor,
    no_notify_self: i32,
) -> Result<(), O::Error> {
    unsafe {
        for i in 0..4 {
            if (*actor).touching[i] == other.cast() {
                (*actor).touching[i] = ptr::null_mut();
                if no_notify_self == 0 {
                    ops.event(ActorRelationEvent::UnTouch, actor.cast(), other.cast())?;
                }
                break;
            }
        }
        for i in 0..4 {
            if (*other).touching[i] == actor.cast() {
                (*other).touching[i] = ptr::null_mut();
                ops.event(ActorRelationEvent::UnTouch, other.cast(), actor.cast())?;
                break;
            }
        }
    }
    Ok(())
}

/// Native adapter for the shared `AActor::EndTouch` transition.
///
/// # Safety
/// `actor` and `other` are live actors.
pub unsafe fn end_touch(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    other: *mut AActor,
    no_notify_self: i32,
) -> Result<(), i32> {
    unsafe { end_touch_with_ops(&mut NativeActorRelationOps(svc), actor, other, no_notify_self) }
}

/// `AActor::IsBrush`: `Brush != NULL && IsA(ABrush::StaticClass())`.
pub unsafe fn is_brush(svc: &Hp2ActorServices, actor: *const AActor) -> Result<bool, i32> {
    unsafe {
        is_brush_with(actor, |value, class| {
            svc.actor_is_a(value, class).map(|v| v != 0)
        })
    }
}

unsafe fn is_brush_with(
    actor: *const AActor,
    is_a: impl Fn(*const c_void, u32) -> Result<bool, i32>,
) -> Result<bool, i32> {
    unsafe { Ok(!(*actor).brush.is_null() && is_a(actor.cast(), class::ABRUSH)?) }
}

/// `AActor::IsStaticBrush`.
pub unsafe fn is_static_brush(svc: &Hp2ActorServices, actor: *const AActor) -> Result<bool, i32> {
    unsafe { Ok(is_brush(svc, actor)? && (*actor).b_static()) }
}

/// `AActor::IsMovingBrush`.
pub unsafe fn is_moving_brush(svc: &Hp2ActorServices, actor: *const AActor) -> Result<bool, i32> {
    unsafe {
        is_moving_brush_with(actor, |value, class| {
            svc.actor_is_a(value, class).map(|v| v != 0)
        })
    }
}

/// Shared moving-brush policy; class identity is resolved by the owning runtime.
///
/// # Safety
/// `actor` is live and `is_a` checks its actual class without retaining the pointer.
pub(crate) unsafe fn is_moving_brush_with(
    actor: *const AActor,
    is_a: impl Fn(*const c_void, u32) -> Result<bool, i32>,
) -> Result<bool, i32> {
    unsafe { Ok(is_brush_with(actor, is_a)? && !(*actor).b_static()) }
}

/// `AActor::GetPlayerPawn`.
pub unsafe fn get_player_pawn(
    svc: &Hp2ActorServices,
    actor: *const AActor,
) -> Result<*mut c_void, i32> {
    unsafe {
        get_player_pawn_with(actor, |value, class| {
            svc.actor_is_a(value, class).map(|v| v != 0)
        })
    }
}

unsafe fn get_player_pawn_with(
    actor: *const AActor,
    is_a: impl Fn(*const c_void, u32) -> Result<bool, i32>,
) -> Result<*mut c_void, i32> {
    unsafe {
        if !is_a(actor.cast(), class::APLAYERPAWN)?
            || (*actor.cast::<APlayerPawn>()).player.is_null()
        {
            return Ok(std::ptr::null_mut());
        }
        Ok(actor.cast_mut().cast())
    }
}

/// `AActor::IsPlayer`.
pub unsafe fn is_player(svc: &Hp2ActorServices, actor: *const AActor) -> Result<bool, i32> {
    unsafe {
        is_player_with(actor, |value, class| {
            svc.actor_is_a(value, class).map(|v| v != 0)
        })
    }
}

pub(crate) unsafe fn is_player_with(
    actor: *const AActor,
    is_a: impl Fn(*const c_void, u32) -> Result<bool, i32>,
) -> Result<bool, i32> {
    unsafe { Ok(is_a(actor.cast(), class::APAWN)? && (*actor.cast::<APawn>()).b_is_player()) }
}

/// `AActor::GetViewRotation`.
pub unsafe fn get_view_rotation(
    svc: &Hp2ActorServices,
    actor: *const AActor,
) -> Result<FRotator, i32> {
    unsafe {
        get_view_rotation_with(actor, |value, class| {
            svc.actor_is_a(value, class).map(|v| v != 0)
        })
    }
}

unsafe fn get_view_rotation_with(
    actor: *const AActor,
    is_a: impl Fn(*const c_void, u32) -> Result<bool, i32>,
) -> Result<FRotator, i32> {
    unsafe {
        if is_a(actor.cast(), class::APAWN)? {
            Ok((*actor.cast::<APawn>()).view_rotation)
        } else {
            Ok((*actor).rotation)
        }
    }
}

/// Fixed rendering overrides on `APlayerPawn`.
pub const fn player_pawn_clear_screen() -> bool {
    false
}

pub const fn player_pawn_recompute_lighting() -> bool {
    false
}

pub const fn player_pawn_can_see() -> bool {
    false
}

pub const fn player_pawn_view_zone(view_zone: i32) -> i32 {
    view_zone
}

pub const fn player_pawn_is_zone_visible() -> bool {
    true
}

pub const fn player_pawn_is_surf_visible() -> bool {
    true
}

pub const fn player_pawn_is_actor_visible() -> bool {
    true
}

/// `AActor::IsBlockedBy`.
fn is_blocked_by_level(actor: &AActor, other: *const AActor) -> Option<bool> {
    (other.cast() == actor.level).then(|| actor.b_collide_world())
}

pub unsafe fn is_blocked_by(
    svc: &Hp2ActorServices,
    actor: *const AActor,
    other: *const AActor,
) -> Result<bool, i32> {
    unsafe {
        is_blocked_by_with(actor, other, |value, class| {
            svc.actor_is_a(value, class).map(|v| v != 0)
        })
    }
}

pub(crate) unsafe fn is_blocked_by_with<E>(
    actor: *const AActor,
    other: *const AActor,
    is_a: impl Fn(*const c_void, u32) -> Result<bool, E>,
) -> Result<bool, E> {
    unsafe {
        if let Some(result) = is_blocked_by_level(&*actor, other) {
            return Ok(result);
        }
        let is_brush = |value: *const AActor| -> Result<bool, E> {
            Ok(!(*value).brush.is_null() && is_a(value.cast(), class::ABRUSH)?)
        };
        let player_pawn = |value: *const AActor| -> Result<bool, E> {
            Ok(is_a(value.cast(), class::APLAYERPAWN)?
                && !(*value.cast::<APlayerPawn>()).player.is_null())
        };
        if is_brush(other)? {
            return Ok((*actor).b_collide_world()
                && if player_pawn(actor)? {
                    (*other).b_block_players()
                } else {
                    (*other).b_block_actors()
                });
        }
        if is_brush(actor)? {
            return Ok((*other).b_collide_world()
                && if player_pawn(other)? {
                    (*actor).b_block_players()
                } else {
                    (*actor).b_block_actors()
                });
        }
        let actor_player_or_projectile =
            player_pawn(actor)? || is_a(actor.cast(), class::APROJECTILE)?;
        let first_blocks = if actor_player_or_projectile {
            (*other).b_block_players()
        } else {
            (*other).b_block_actors()
        };
        if !first_blocks {
            return Ok(false);
        }
        let other_player_or_projectile =
            player_pawn(other)? || is_a(other.cast(), class::APROJECTILE)?;
        Ok(if other_player_or_projectile {
            (*actor).b_block_players()
        } else {
            (*actor).b_block_actors()
        })
    }
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
) -> Result<i32, i32> {
    unsafe {
        is_overlapping_with(actor, other, |value| {
            is_brush(svc, value).map_err(|error| error)
        })
    }
}

pub(crate) unsafe fn is_overlapping_with<E>(
    actor: *const AActor,
    other: *const AActor,
    is_brush: impl Fn(*const AActor) -> Result<bool, E>,
) -> Result<i32, E> {
    unsafe {
        if !is_brush(actor)? && !is_brush(other)? && (*actor).level.cast_const() != other.cast::<c_void>() {
            let dx = (*actor).location.x - (*other).location.x;
            let dy = (*actor).location.y - (*other).location.y;
            let dz = (*actor).location.z - (*other).location.z;
            let r = (*actor).collision_radius + (*other).collision_radius;
            let h = (*actor).collision_height + (*other).collision_height;
            Ok(i32::from(dx * dx + dy * dy < r * r && dz * dz < h * h))
        } else {
            Ok(0)
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
pub fn is_animating(actor: &AActor) -> bool {
    actor.anim_sequence != [0u8; 4] && (actor.anim_rate != 0.0 || actor.tween_rate != 0.0)
}

/// `AActor::execSaveGameExists`: a save file exists only when its size is positive.
pub fn save_game_exists(file_size: i32) -> bool {
    file_size > 0
}

#[cfg(test)]
mod save_game_exists_tests {
    use super::*;

    #[test]
    fn file_size_sign_controls_existence() {
        assert!(!save_game_exists(-1));
        assert!(!save_game_exists(0));
        assert!(save_game_exists(1));
        assert!(save_game_exists(i32::MAX));
    }
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
pub unsafe fn actor_post_edit_change(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
) -> Result<(), i32> {
    if unsafe { svc.is_editor()? } != 0 {
        unsafe { (*actor).set_b_light_changed(true) };
    }
    Ok(())
}

/// `AZoneInfo::PostEditChange` after `Super`: `if (GIsEditor) GCache.Flush()`.
///
/// # Safety
/// `svc` must hold valid service callbacks.
pub unsafe fn zone_info_post_edit_change(svc: &Hp2ActorServices) -> Result<(), i32> {
    if unsafe { svc.is_editor()? } != 0 {
        unsafe { svc.gcache_flush()? };
    }
    Ok(())
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
pub unsafe fn get_primitive(
    svc: &Hp2ActorServices,
    actor: *const AActor,
) -> Result<*mut c_void, i32> {
    unsafe { get_primitive_with(actor, |level, slot| svc.level_engine_primitive(level, slot)) }
}

/// Shared Actor primitive-selection policy for native and canonical owners.
///
/// # Safety
/// `actor` is live; `engine_primitive` resolves its actual XLevel's engine primitive.
pub(crate) unsafe fn get_primitive_with<E>(
    actor: *const AActor,
    engine_primitive: impl FnOnce(*mut c_void, u8) -> Result<*mut c_void, E>,
) -> Result<*mut c_void, E> {
    if unsafe { (*actor).collide_type } == CT_SHAPE {
        let mesh = unsafe { (*actor).mesh };
        if !mesh.is_null() { return Ok(mesh); }
        let brush = unsafe { (*actor).brush };
        if !brush.is_null() { return Ok(brush); }
    }
    let slot = match unsafe { (*actor).collide_type } {
        CT_ORIENTED_CYLINDER => CT_ORIENTED_CYLINDER,
        CT_BOX => CT_BOX,
        CT_ALIGNED_OVAL_CYLINDER => CT_ALIGNED_OVAL_CYLINDER,
        CT_ORIENTED_OVAL_CYLINDER => CT_ORIENTED_OVAL_CYLINDER,
        _ => CT_ALIGNED_CYLINDER,
    };
    engine_primitive(unsafe { (*actor).x_level }, slot)
}

/// C++ virtual dispatch for the selected primitive's world collision bounds.
#[repr(C)]
pub struct Hp2ActorWorldCollisionBoxServices {
    pub context: *mut c_void,
    pub mesh_is_skeletal:
        unsafe extern "C" fn(context: *mut c_void, mesh: *mut c_void, out: *mut i32) -> i32,
    pub primitive_collision_bounding_box: unsafe extern "C" fn(
        context: *mut c_void,
        primitive: *mut c_void,
        actor: *mut c_void,
        world: i32,
        out: *mut c_void,
    ) -> i32,
}

/// `AActor::execGetWorldCollisionBox`: visual mesh/brush override, otherwise
/// the actor's virtual `GetPrimitive`, then its world collision box.
///
/// # Safety
/// `actor` must be live; `out` and both service tables must remain valid.
pub unsafe fn get_world_collision_box(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    visual: i32,
    out: *mut c_void,
    box_services: &Hp2ActorWorldCollisionBoxServices,
) -> Result<i32, i32> {
    if actor.is_null() || out.is_null() {
        return Err(1);
    }
    unsafe {
        let primitive = if visual != 0 && !(*actor).mesh.is_null() {
            (*actor).mesh
        } else if visual != 0 && !(*actor).brush.is_null() {
            (*actor).brush
        } else {
            svc.actor_get_primitive(actor.cast())?
        };
        let status = (box_services.primitive_collision_bounding_box)(
            box_services.context,
            primitive,
            actor.cast(),
            1,
            out,
        );
        if status != 0 {
            return Err(status);
        }
    }
    Ok(0)
}

/// `AActor::execGetRenderExtent`: skeletal meshes use `BoundingBoxSum`;
/// other meshes, brushes, or the virtual actor primitive use local collision
/// bounds. The result is `Max - Min`.
///
/// # Safety
/// `actor`, `out`, and both service tables must remain valid for the call.
pub unsafe fn get_render_extent(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    out: *mut c_void,
    box_services: &Hp2ActorWorldCollisionBoxServices,
) -> Result<i32, i32> {
    if actor.is_null() || out.is_null() {
        return Err(1);
    }
    unsafe {
        let mut is_skeletal = 0;
        if !(*actor).mesh.is_null() {
            let status = (box_services.mesh_is_skeletal)(
                box_services.context,
                (*actor).mesh,
                &raw mut is_skeletal,
            );
            if status != 0 {
                return Err(status);
            }
        }
        let mut local = std::mem::MaybeUninit::<FBox>::zeroed();
        let bounds = if is_skeletal != 0 {
            &raw const (*(*actor).mesh.cast::<USkeletalMesh>()).bounding_box_sum
        } else {
            let primitive = if !(*actor).mesh.is_null() {
                (*actor).mesh
            } else if !(*actor).brush.is_null() {
                (*actor).brush
            } else {
                svc.actor_get_primitive(actor.cast())?
            };
            let status = (box_services.primitive_collision_bounding_box)(
                box_services.context,
                primitive,
                actor.cast(),
                0,
                local.as_mut_ptr().cast(),
            );
            if status != 0 {
                return Err(status);
            }
            local.as_ptr()
        };
        out.cast::<FVector>().write_unaligned(FVector {
            x: (*bounds).max.x - (*bounds).min.x,
            y: (*bounds).max.y - (*bounds).min.y,
            z: (*bounds).max.z - (*bounds).min.z,
        });
    }
    Ok(0)
}

/// `AActor::Spawned`: `if (CollideType == CT_Shape) GetPrimitive()->ValidateActor(this)`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn spawned(svc: &Hp2ActorServices, actor: *mut AActor) -> Result<(), i32> {
    if unsafe { (*actor).collide_type } == CT_SHAPE {
        let prim = unsafe { svc.actor_get_primitive(actor.cast())? };
        unsafe { svc.primitive_validate_actor(prim, actor.cast())? };
    }
    Ok(())
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
) -> Result<(), i32> {
    let level = unsafe { (*actor).level };
    process_event_with(unsafe { svc.level_begun_play(level)? } != 0, || unsafe {
        svc.uobject_process_event(actor.cast(), func, parms, result)
    })
}

/// AActor's virtual ProcessEvent gate; the supplied operation is UObject's
/// implementation, not virtual dispatch back into this guard.
pub(crate) fn process_event_with<E>(
    begun_play: bool, execute: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    if begun_play { execute()?; }
    Ok(())
}

/// `AActor::Destroy`: destroy the render interface, then the aux anims (the array length
/// is re-read each iteration, as in `for_array`), then `UObject::Destroy()`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn destroy(svc: &Hp2ActorServices, actor: *mut AActor) -> Result<(), i32> {
    unsafe {
        if !(*actor).render_interface.is_null() {
            svc.render_interface_remove_root((*actor).render_interface)?;
            svc.object_conditional_destroy((*actor).render_interface)?;
            svc.render_interface_destroy((*actor).render_interface)?;
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
                svc.object_conditional_destroy(elem)?;
            }
            i += 1;
        }
        svc.uobject_destroy(actor.cast())?;
    }
    Ok(())
}

/// `AActor::PostLoad`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn post_load(svc: &Hp2ActorServices, actor: *mut AActor) -> Result<(), i32> {
    unsafe {
        svc.uobject_post_load(actor.cast())?;
        if svc.actor_class_is_localized(actor.cast())? != 0 {
            svc.actor_load_localized(actor.cast())?;
        }
        if !(*actor).brush.is_null() {
            svc.object_set_transactional((*actor).brush)?;
        }
        if !(*actor).brush.is_null() {
            let polys = svc.model_polys((*actor).brush)?;
            if !polys.is_null() {
                svc.object_set_transactional(polys)?;
            }
        }
        if (*actor).collide_type == CT_SHAPE
            && (!(*actor).brush.is_null() || !(*actor).mesh.is_null())
        {
            let prim = svc.actor_get_primitive(actor.cast())?;
            svc.primitive_validate_actor(prim, actor.cast())?;
        }
    }
    Ok(())
}

/// `APlayerPawn::SetPlayer`: detach the previous pawn from `player`, attach `pawn`, then
/// send `eventPossess` and log.
///
/// # Safety
/// `pawn` must be a live `APlayerPawn`. `player` must be a live `UPlayer` unless the check fails.
pub unsafe fn set_player(
    svc: &Hp2ActorServices,
    pawn: *mut APlayerPawn,
    player: *mut c_void,
) -> Result<(), i32> {
    unsafe { set_player_with(&mut NativeSetPlayerOps(svc), pawn, player) }
}

/// `AActor::GetCylinderExtent`: shape-type collision extent from the mesh or brush, else
/// the collision box or cylinder dimensions. The result is written to `out`.
///
/// # Safety
/// `actor` must be a live `AActor`; `out` must be valid for one `FVector` write.
pub unsafe fn get_cylinder_extent(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    out: *mut FVector,
) -> Result<(), i32> {
    unsafe {
        guarded(c"AActor::GetCylinderExtent", || {
            get_cylinder_extent_with(
                actor, (*actor).collision_width, out,
                |mesh, owner, out| svc.mesh_collision_extent(mesh, owner, out),
                |brush, owner, out| svc.brush_collision_extent(brush, owner, out),
            )
        })
    }
}

/// Shared cylinder-extent policy, including mesh-before-brush shape precedence.
///
/// # Safety
/// `actor` is live; `out` is writable; callbacks initialize one `FVector` on success.
pub(crate) unsafe fn get_cylinder_extent_with<E>(
    actor: *mut AActor,
    collision_width: f32,
    out: *mut FVector,
    mesh_extent: impl FnOnce(*mut c_void, *mut c_void, *mut c_void) -> Result<(), E>,
    brush_extent: impl FnOnce(*mut c_void, *mut c_void, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        if (*actor).collide_type == CT_SHAPE {
            if !(*actor).mesh.is_null() {
                return mesh_extent((*actor).mesh, actor.cast(), out.cast());
            }
            if !(*actor).brush.is_null() {
                return brush_extent((*actor).brush, actor.cast(), out.cast());
            }
        }
        let radius = (*actor).collision_radius;
        let width = if (*actor).collide_type == CT_BOX && collision_width != 0.0 {
            collision_width
        } else {
            radius
        };
        out.write(FVector { x: radius, y: width, z: (*actor).collision_height });
        Ok(())
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
) -> Result<(), i32> {
    unsafe {
        if !(*actor).mesh.is_null() {
            svc.mesh_render_bounding_box((*actor).mesh, actor.cast(), exact, out.cast())?;
        } else if !(*actor).brush.is_null() {
            svc.brush_render_bounding_box_coords((*actor).brush, actor.cast(), exact, out.cast())?;
        } else {
            std::ptr::write_bytes(out, 0, 1);
        }
    }
    Ok(())
}

/// `AActor::SerializePersistence` (the guard/unguard and the commented-out frame-state block
/// stay in C++): flag as debug-serialized, preload the class, serialize the class properties
/// (tagged when loading or saving outside transactions, else binary), then count the bytes.
///
/// # Safety
/// `actor` must be a live `AActor`; `ar` must be a live `FArchive`.
pub unsafe fn serialize_persistence(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    ar: *mut c_void,
) -> Result<(), i32> {
    unsafe {
        let obj = actor.cast::<c_void>();
        svc.object_set_debug_serialize(obj)?;
        let the_class = svc.object_class(obj)?;
        let is_uclass = svc.class_is_uclass_static(the_class)? != 0;
        if !is_uclass {
            svc.archive_preload(ar, the_class)?;
        }
        let is_uclass_props = svc.class_is_uclass_static(the_class)? != 0;
        if !is_uclass_props {
            let loading_or_saving =
                svc.archive_is_loading(ar)? != 0 || svc.archive_is_saving(ar)? != 0;
            if loading_or_saving && svc.archive_is_trans(ar)? == 0 {
                svc.object_serialize_tagged_persistent(obj, ar)?;
            } else {
                svc.object_serialize_bin(obj, ar)?;
            }
        }
        let size = svc.object_properties_size(obj)?;
        svc.archive_count_bytes(ar, size)?;
    }
    Ok(())
}

/// Typed source operations for `AActor::TraceTexture`. Calls remain explicit
/// so adapter callbacks and reentrant reads retain their native order.
pub trait TraceTextureOps {
    type Error;
    unsafe fn trace_texture_hit(&mut self, actor: *mut AActor, end: FVector, start: FVector) -> Result<(*mut FBspSurf, FVector), Self::Error>;
    unsafe fn actor_level_model(&mut self, actor: *mut AActor) -> Result<*mut UModel, Self::Error>;
    unsafe fn surface_texture(&mut self, surf: *mut FBspSurf) -> Result<*mut c_void, Self::Error>;
    unsafe fn surface_flags(&mut self, surf: *mut FBspSurf) -> Result<u32, Self::Error>;
    unsafe fn texture_flags(&mut self, texture: *mut c_void) -> Result<u32, Self::Error>;
    unsafe fn decal_count(&mut self, surf: *mut FBspSurf) -> Result<i32, Self::Error>;
    unsafe fn surface_normal_index(&mut self, surf: *mut FBspSurf) -> Result<i32, Self::Error>;
    unsafe fn surface_base_index(&mut self, surf: *mut FBspSurf) -> Result<i32, Self::Error>;
    unsafe fn model_vector(&mut self, model: *mut UModel, index: i32) -> Result<*const c_void, Self::Error>;
    unsafe fn model_point(&mut self, model: *mut UModel, index: i32) -> Result<*const c_void, Self::Error>;
    unsafe fn decal_at(&mut self, surf: *mut FBspSurf, index: i32) -> Result<*mut FDecal, Self::Error>;
    unsafe fn decal_vertices(&mut self, decal: *mut FDecal) -> Result<*const [FVector; 4], Self::Error>;
    unsafe fn decal_texture(&mut self, decal: *mut FDecal) -> Result<*mut c_void, Self::Error>;
}

pub unsafe fn trace_texture_with_ops<O: TraceTextureOps>(
    ops: &mut O, actor: *mut AActor, flags: *mut i32, end: FVector, start: FVector, trace_decals: bool,
) -> Result<*mut c_void, O::Error> {
    unsafe {
        let (surf, hit_location) = ops.trace_texture_hit(actor, end, start)?;
        let model = ops.actor_level_model(actor)?;
        *flags = 0;
        if surf.is_null() { return Ok(ptr::null_mut()); }
        let mut texture = ops.surface_texture(surf)?;
        let texture_flags = if texture.is_null() { 0 } else { ops.texture_flags(texture)? };
        *flags = (ops.surface_flags(surf)? | texture_flags) as i32;
        if trace_decals && ops.decal_count(surf)? != 0 {
            let normal_index = ops.surface_normal_index(surf)?;
            let normal = ops.model_vector(model, normal_index)?;
            let base_index = ops.surface_base_index(surf)?;
            let base = ops.model_point(model, base_index)?;
            let mut index = 0;
            while index < ops.decal_count(surf)? {
                let decal = ops.decal_at(surf, index)?;
                let vertices = ops.decal_vertices(decal)?;
                if point_inside_quad(sub_vec(hit_location, *base.cast::<FVector>()), &*vertices, *normal.cast::<FVector>()) {
                    if !ops.decal_texture(decal)?.is_null() { texture = ops.decal_texture(decal)?; }
                }
                index += 1;
            }
        }
        Ok(texture)
    }
}

/// Retained public service-table wrapper for existing callers.
pub unsafe fn trace_texture(
    svc: &Hp2ActorServices, actor: *mut AActor, trace_end: *const FVector, trace_start: *const FVector,
    flags: *mut i32, trace_decals: bool,
) -> Result<*mut c_void, i32> {
    struct Ops<'a>(&'a Hp2ActorServices);
    impl TraceTextureOps for Ops<'_> {
        type Error = i32;
        unsafe fn trace_texture_hit(&mut self, actor: *mut AActor, end: FVector, start: FVector) -> Result<(*mut FBspSurf, FVector), i32> {
            let mut location = FVector { x: 0.0, y: 0.0, z: 0.0 };
            let surf = unsafe { self.0.trace_line_texture_hit(actor.cast(), (&end as *const FVector).cast(), (&start as *const FVector).cast(), (&mut location as *mut FVector).cast())? };
            Ok((surf.cast(), location))
        }
        unsafe fn actor_level_model(&mut self, actor: *mut AActor) -> Result<*mut UModel, i32> { unsafe { self.0.actor_x_level_model(actor.cast()).map(|p| p.cast()) } }
        unsafe fn surface_texture(&mut self, surf: *mut FBspSurf) -> Result<*mut c_void, i32> { unsafe { self.0.bsp_surf_texture(surf.cast()) } }
        unsafe fn surface_flags(&mut self, surf: *mut FBspSurf) -> Result<u32, i32> { unsafe { self.0.bsp_surf_poly_flags(surf.cast()) } }
        unsafe fn texture_flags(&mut self, texture: *mut c_void) -> Result<u32, i32> { unsafe { self.0.texture_poly_flags(texture) } }
        unsafe fn decal_count(&mut self, surf: *mut FBspSurf) -> Result<i32, i32> { unsafe { self.0.bsp_surf_decal_count(surf.cast()) } }
        unsafe fn surface_normal_index(&mut self, surf: *mut FBspSurf) -> Result<i32, i32> { unsafe { self.0.bsp_surf_v_normal(surf.cast()) } }
        unsafe fn surface_base_index(&mut self, surf: *mut FBspSurf) -> Result<i32, i32> { unsafe { self.0.bsp_surf_p_base(surf.cast()) } }
        unsafe fn model_vector(&mut self, model: *mut UModel, index: i32) -> Result<*const c_void, i32> { unsafe { self.0.model_vector(model.cast(), index) } }
        unsafe fn model_point(&mut self, model: *mut UModel, index: i32) -> Result<*const c_void, i32> { unsafe { self.0.model_point(model.cast(), index) } }
        unsafe fn decal_at(&mut self, surf: *mut FBspSurf, index: i32) -> Result<*mut FDecal, i32> { unsafe { self.0.bsp_surf_decal(surf.cast(), index).map(|p| p.cast()) } }
        unsafe fn decal_vertices(&mut self, decal: *mut FDecal) -> Result<*const [FVector; 4], i32> { unsafe { self.0.decal_vertices(decal.cast()).map(|p| p.cast()) } }
        unsafe fn decal_texture(&mut self, decal: *mut FDecal) -> Result<*mut c_void, i32> { unsafe { self.0.decal_actor_texture(decal.cast()) } }
    }
    unsafe { trace_texture_with_ops(&mut Ops(svc), actor, flags, *trace_end, *trace_start, trace_decals) }
}

#[cfg(test)]
mod animation_tests {
    use super::*;

    fn actor() -> AActor {
        unsafe { std::mem::zeroed() }
    }

    #[test]
    fn animation_requires_a_sequence_and_nonzero_play_or_tween_rate() {
        let mut a = actor();
        assert!(!is_animating(&a));

        a.anim_sequence = [1, 0, 0, 0];
        assert!(!is_animating(&a));

        a.anim_rate = 1.0;
        assert!(is_animating(&a));

        a.anim_rate = 0.0;
        a.tween_rate = -0.5;
        assert!(is_animating(&a));

        a.anim_sequence = [0; 4];
        assert!(!is_animating(&a));
    }
}

#[cfg(test)]
mod actor_inline_tests {
    use super::*;

    #[test]
    fn returns_the_last_actor_in_the_owner_chain() {
        let mut root: AActor = unsafe { std::mem::zeroed() };
        let mut middle: AActor = unsafe { std::mem::zeroed() };
        let mut leaf: AActor = unsafe { std::mem::zeroed() };
        middle.owner = (&mut root as *mut AActor).cast();
        leaf.owner = (&mut middle as *mut AActor).cast();

        assert_eq!(
            unsafe { get_top_owner(&mut leaf) },
            &mut root as *mut AActor
        );
    }

    #[test]
    fn base_chain_includes_self_and_stops_at_null() {
        let mut root: AActor = unsafe { std::mem::zeroed() };
        let mut child: AActor = unsafe { std::mem::zeroed() };
        let unrelated: AActor = unsafe { std::mem::zeroed() };
        child.base = (&mut root as *mut AActor).cast();

        assert!(unsafe { is_based_on(&child, &child) });
        assert!(unsafe { is_based_on(&child, &root) });
        assert!(!unsafe { is_based_on(&child, &unrelated) });
    }

    #[test]
    fn level_default_zone_matches_any_zone_and_explicit_zone_matches_itself() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        let level = 3usize as *mut c_void;
        let zone = 1usize as *mut c_void;
        let other_zone = 2usize as *mut c_void;

        actor.level = level;
        actor.region.zone = level;
        assert!(unsafe { is_in_zone(&actor, zone) });
        actor.region.zone = zone;
        assert!(unsafe { is_in_zone(&actor, zone) });
        assert!(!unsafe { is_in_zone(&actor, other_zone) });
    }

    #[test]
    fn world_radius_helpers_follow_actor_fields() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        actor.sound_radius = 3;
        actor.volume_radius = 5;
        actor.light_radius = 2;
        actor.draw_scale = 0.5;
        assert_eq!(world_sound_radius(&actor), 100.0);
        assert_eq!(world_volumetric_radius(&actor), 150.0);
        assert_eq!(world_light_radius(&actor), 75.0);
        actor.draw_scale = 2.0;
        assert_eq!(world_light_radius(&actor), 150.0);
    }

    #[test]
    fn level_accessor_returns_xlevel() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        let level = 7usize as *mut c_void;
        actor.x_level = level;
        assert_eq!(unsafe { get_level(&actor) }, level);
    }

    #[test]
    fn pending_kill_reads_delete_bit() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        assert!(!unsafe { is_pending_kill(&actor) });
        actor.set_b_delete_me(true);
        assert!(unsafe { is_pending_kill(&actor) });
    }

    #[test]
    fn blocked_by_level_uses_collide_world_before_class_queries() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        let other: AActor = unsafe { std::mem::zeroed() };
        actor.level = (&other as *const AActor).cast_mut().cast();
        actor.set_b_collide_world(true);
        assert_eq!(is_blocked_by_level(&actor, &other), Some(true));
        actor.set_b_collide_world(false);
        assert_eq!(is_blocked_by_level(&actor, &other), Some(false));
    }

    #[test]
    fn blocked_by_stops_before_other_class_queries_when_first_side_does_not_block() {
        let actor: AActor = unsafe { std::mem::zeroed() };
        let other: AActor = unsafe { std::mem::zeroed() };
        let classes = std::cell::RefCell::new(Vec::new());
        assert!(
            !unsafe {
                is_blocked_by_with(&actor, &other, |_, class| {
                    classes.borrow_mut().push(class);
                    Ok::<bool, i32>(false)
                })
            }
            .unwrap()
        );
        assert_eq!(
            classes.into_inner(),
            vec![class::APLAYERPAWN, class::APROJECTILE]
        );
    }

    #[test]
    fn brush_predicate_requires_brush_pointer_before_class_query() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        let calls = std::cell::Cell::new(0);
        assert!(
            !unsafe {
                is_brush_with(&actor, |_, _| {
                    calls.set(calls.get() + 1);
                    Ok(true)
                })
            }
            .unwrap()
        );
        assert_eq!(calls.get(), 0);

        actor.brush = 1usize as *mut c_void;
        assert!(
            unsafe {
                is_brush_with(&actor, |_, class| {
                    calls.set(calls.get() + 1);
                    Ok(class == class::ABRUSH)
                })
            }
            .unwrap()
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn player_predicates_reject_nonmatching_classes_before_subclass_fields() {
        let actor: AActor = unsafe { std::mem::zeroed() };
        let player = unsafe {
            get_player_pawn_with(&actor, |_, class| {
                assert_eq!(class, class::APLAYERPAWN);
                Ok(false)
            })
        }
        .unwrap();
        assert!(player.is_null());
        assert!(
            !unsafe {
                is_player_with(&actor, |_, class| {
                    assert_eq!(class, class::APAWN);
                    Ok(false)
                })
            }
            .unwrap()
        );
    }

    #[test]
    fn view_rotation_uses_actor_rotation_without_pawn_class() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        actor.rotation = FRotator {
            pitch: 100,
            yaw: 200,
            roll: 300,
        };
        let rotation = unsafe {
            get_view_rotation_with(&actor, |_, class| {
                assert_eq!(class, class::APAWN);
                Ok(false)
            })
        }
        .unwrap();
        assert_eq!(rotation.pitch, 100);
        assert_eq!(rotation.yaw, 200);
        assert_eq!(rotation.roll, 300);
    }

    #[test]
    fn player_pawn_rendering_overrides_remain_fixed() {
        assert!(!player_pawn_clear_screen());
        assert!(!player_pawn_recompute_lighting());
        assert!(!player_pawn_can_see());
        assert_eq!(player_pawn_view_zone(-42), -42);
        assert!(player_pawn_is_zone_visible());
        assert!(player_pawn_is_surf_visible());
        assert!(player_pawn_is_actor_visible());
    }
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's per-native service tables (not
    `Hp2ActorServices`, which is published by its own owner elsewhere in this
    file). Each mirrors `hp2_core::vm_object::publish_services`.
-----------------------------------------------------------------------------*/

static mut FAST_TRACE_SERVICES: Option<Hp2ActorFastTraceServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorFastTraceServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_fast_trace_services(value: Hp2ActorFastTraceServices) {
    unsafe { (&raw mut FAST_TRACE_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_fast_trace_services` has run.
pub unsafe fn published_fast_trace_services() -> Result<Hp2ActorFastTraceServices, i32> {
    unsafe { (&raw const FAST_TRACE_SERVICES).read().ok_or(1) }
}

static mut SOUND_DURATION_SERVICES: Option<Hp2ActorSoundDurationServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorSoundDurationServices` table;
/// publication happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_sound_duration_services(value: Hp2ActorSoundDurationServices) {
    unsafe { (&raw mut SOUND_DURATION_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_sound_duration_services` has run.
pub unsafe fn published_sound_duration_services() -> Result<Hp2ActorSoundDurationServices, i32> {
    unsafe { (&raw const SOUND_DURATION_SERVICES).read().ok_or(1) }
}

static mut LATENT_SERVICES: Option<Hp2ActorLatentServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorLatentServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_latent_services(value: Hp2ActorLatentServices) {
    unsafe { (&raw mut LATENT_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_latent_services` has run.
pub unsafe fn published_latent_services() -> Result<Hp2ActorLatentServices, i32> {
    unsafe { (&raw const LATENT_SERVICES).read().ok_or(1) }
}

static mut WORLD_COLLISION_BOX_SERVICES: Option<Hp2ActorWorldCollisionBoxServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorWorldCollisionBoxServices` table;
/// publication happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_world_collision_box_services(value: Hp2ActorWorldCollisionBoxServices) {
    unsafe { (&raw mut WORLD_COLLISION_BOX_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_world_collision_box_services` has run.
pub unsafe fn published_world_collision_box_services()
-> Result<Hp2ActorWorldCollisionBoxServices, i32> {
    unsafe { (&raw const WORLD_COLLISION_BOX_SERVICES).read().ok_or(1) }
}

/// Narrow adapter for `AWarpZoneInfo::execWarp`'s `WarpCoords` field read.
#[repr(C)]
pub struct Hp2ActorWarpServices {
    pub context: *mut c_void,
    pub warp_coords:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, out: *mut c_void) -> i32,
}

/// `AWarpZoneInfo::execWarp`: transform `WarpLocation`/`WarpVelocity` by the
/// zone's transposed `WarpCoords`, and `WarpRotation` by
/// `(UNIT_COORDS / *WarpRotation * WarpCoords.Transpose()).OrthoRotation()`.
///
/// # Safety
/// `actor` must be a live `AWarpZoneInfo`; `warp_location`, `warp_velocity`,
/// `warp_rotation`, and `services` must remain valid for this call.
pub unsafe fn exec_warp(
    actor: *mut c_void,
    warp_location: *mut FVector,
    warp_velocity: *mut FVector,
    warp_rotation: *mut FRotator,
    services: &Hp2ActorWarpServices,
) -> i32 {
    unsafe {
        let mut warp_coords = std::mem::MaybeUninit::<FCoords>::zeroed();
        let status =
            (services.warp_coords)(services.context, actor, warp_coords.as_mut_ptr().cast());
        if status != 0 {
            return status;
        }
        exec_warp_with_coords(
            warp_coords.assume_init(),
            warp_location,
            warp_velocity,
            warp_rotation,
            true,
        );
        0
    }
}

/// Shared coordinate/rotation transform used by Warp and UnWarp.
///
/// # Safety
/// The three output pointers must be non-null and writable.
pub(crate) unsafe fn exec_warp_with_coords(
    warp_coords: FCoords,
    warp_location: *mut FVector,
    warp_velocity: *mut FVector,
    warp_rotation: *mut FRotator,
    transpose_coords: bool,
) {
    unsafe {
        let coords = if transpose_coords { math::coords_transpose(warp_coords) } else { warp_coords };
        let trig = math::trig_table();
        *warp_location = math::vector_transform_point(*warp_location, coords);
        *warp_velocity = math::vector_transform(*warp_velocity, coords);
        let in_rotation = math::FRotator {
            pitch: (*warp_rotation).pitch,
            yaw: (*warp_rotation).yaw,
            roll: (*warp_rotation).roll,
        };
        let divided = math::coords_div_rotator(&math::unit_coords(), in_rotation, trig);
        let out_rotation = math::coords_ortho_rotation(&math::coords_mul(divided, coords), trig);
        *warp_rotation = FRotator {
            pitch: out_rotation.pitch,
            yaw: out_rotation.yaw,
            roll: out_rotation.roll,
        };
    }
}

static mut WARP_SERVICES: Option<Hp2ActorWarpServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorWarpServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_warp_services(value: Hp2ActorWarpServices) {
    unsafe { (&raw mut WARP_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_warp_services` has run.
pub unsafe fn published_warp_services() -> Result<Hp2ActorWarpServices, i32> {
    unsafe { (&raw const WARP_SERVICES).read().ok_or(1) }
}

/// Narrow adapter for `AActor::execSaveGameExists`'s save-slot file-size query.
#[repr(C)]
pub struct Hp2ActorSaveGameServices {
    pub context: *mut c_void,
    pub save_slot_file_size:
        unsafe extern "C" fn(context: *mut c_void, slot: i32, out: *mut i32) -> i32,
}

/// `AActor::execSaveGameExists`: query save slot 9's file size (the original
/// hardcodes `Save9.usa`) and report whether it exists (`FileSize > 0`).
///
/// # Safety
/// `out_result` and `services` must remain valid for this call.
pub unsafe fn exec_save_game_exists(
    out_result: *mut u32,
    services: &Hp2ActorSaveGameServices,
) -> i32 {
    unsafe {
        let mut file_size = 0;
        let status = (services.save_slot_file_size)(services.context, 9, &raw mut file_size);
        if status != 0 {
            return status;
        }
        out_result.write_unaligned(save_game_exists(file_size) as u32);
        0
    }
}

static mut SAVE_GAME_SERVICES: Option<Hp2ActorSaveGameServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorSaveGameServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_save_game_services(value: Hp2ActorSaveGameServices) {
    unsafe { (&raw mut SAVE_GAME_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_save_game_services` has run.
pub unsafe fn published_save_game_services() -> Result<Hp2ActorSaveGameServices, i32> {
    unsafe { (&raw const SAVE_GAME_SERVICES).read().ok_or(1) }
}

pub(crate) trait SetPlayerOps {
    type Error;
    fn check_player(&mut self, valid: bool) -> Result<(), Self::Error>;
    unsafe fn player_actor(&mut self, player: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn set_actor(&mut self, player: *mut c_void, pawn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn possess(&mut self, pawn: *mut APlayerPawn) -> Result<(), Self::Error>;
    unsafe fn log_possessed(&mut self, pawn: *mut APlayerPawn) -> Result<(), Self::Error>;
}

/// Detaches the old pawn before publishing both ends of the new player link.
///
/// # Safety
/// Pawn and non-null player are live records in the adapter's ownership domain.
pub(crate) unsafe fn set_player_with<O: SetPlayerOps>(
    ops: &mut O, pawn: *mut APlayerPawn, player: *mut c_void,
) -> Result<(), O::Error> {
    ops.check_player(!player.is_null())?;
    unsafe {
        let old = ops.player_actor(player)?;
        if !old.is_null() {
            (*old.cast::<APlayerPawn>()).player = ptr::null_mut();
            ops.set_actor(player, ptr::null_mut())?;
        }
        (*pawn).player = player;
        ops.set_actor(player, pawn.cast())?;
        ops.possess(pawn)?;
        ops.log_possessed(pawn)?;
    }
    Ok(())
}

struct NativeSetPlayerOps<'a>(&'a Hp2ActorServices);
impl SetPlayerOps for NativeSetPlayerOps<'_> {
    type Error = i32;
    fn check_player(&mut self, valid: bool) -> Result<(), i32> {
        touch_check(self.0, valid, c"InPlayer!=NULL", 1073)
    }
    unsafe fn player_actor(&mut self, player: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { self.0.player_actor(player) }
    }
    unsafe fn set_actor(&mut self, player: *mut c_void, pawn: *mut c_void) -> Result<(), i32> {
        unsafe { self.0.player_set_actor(player, pawn) }
    }
    unsafe fn possess(&mut self, pawn: *mut APlayerPawn) -> Result<(), i32> {
        unsafe { self.0.actor_event_possess(pawn.cast()) }
    }
    unsafe fn log_possessed(&mut self, pawn: *mut APlayerPawn) -> Result<(), i32> {
        unsafe { self.0.actor_log_possessed(pawn.cast()) }
    }
}
