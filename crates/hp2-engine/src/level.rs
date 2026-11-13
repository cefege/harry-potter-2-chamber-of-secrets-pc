//! LevelCore: Core `ULevelBase` / `ULevel` functions from
//! `HarryPotter2/Unreal/Engine/Src/UnLevel.cpp` (oracle ref `7ddb1d3`).
//!
//! The C++ shells in `UnLevel.cpp` forward to the exports in
//! `crates/hp2-ffi/src/engine_level.rs`, which call the functions here with the
//! [`Hp2LevelCoreServices`] table defined in `Hp2LevelServices.cpp`. Control flow
//! follows the original line by line. Each service wraps one original C++ call or
//! expression, in original order.
//!
//! Scalar fields of `ULevel`, `AActor` and `FCheckResult` are read and written
//! through the generated mirrors in `hp2_abi::engine::generated`.
//!
//! Native stack continuations retain FString/iterator/RNG lifetimes and nested guards.
//! Native object and hit storage remains raw across reentrant callbacks.
//! Foreign failures return immediately to the exact native guard; outputs are read
//! only following a successful checked status.

use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APlayerPawn, FArchive, FCheckResult, FName, FRotator, UGameEngine, ULevel, UViewport,
};

/// Opaque engine or C++ object pointer.
pub type Obj = *mut c_void;

/// Mirror of `FActorSlotCompactIndexTrace` (`Core/Inc/UnArc.h:17`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FActorSlotCompactIndexTrace {
    pub index: i32,
    pub offset_after: i32,
    pub captured: i32,
}

/// Checked native leaves. Every status is inspected before any output is read.
#[repr(C)]
pub struct Hp2LevelCoreServices {
    pub fstring_caps: unsafe extern "C" fn(dst: Obj, src: Obj) -> i32,
    pub fstring_len: unsafe extern "C" fn(s: Obj, out: *mut i32) -> i32,
    pub fstring_assign_chars: unsafe extern "C" fn(dst: Obj, src: *const u32) -> i32,
    pub fstring_mid: unsafe extern "C" fn(dst: Obj, src: Obj, start: i32, count: i32) -> i32,
    pub fstring_instr_chars: unsafe extern "C" fn(s: Obj, sub: *const u32, out: *mut i32) -> i32,
    pub fstring_instr_fstring: unsafe extern "C" fn(s: Obj, sub: Obj, out: *mut i32) -> i32,
    pub fstring_eq_chars: unsafe extern "C" fn(s: Obj, lit: *const u32, out: *mut i32) -> i32,
    pub fstring_eq_fstring: unsafe extern "C" fn(a: Obj, b: Obj, out: *mut i32) -> i32,
    pub is_editor: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    pub object_get_name: unsafe extern "C" fn(dst: Obj, obj: Obj) -> i32,
    pub actor_tag_stricmp_viewport_name:
        unsafe extern "C" fn(actor: Obj, viewport: Obj, out: *mut i32) -> i32,
    pub app_system_time_from_level_info: unsafe extern "C" fn(info: Obj) -> i32,
    pub app_sprintf_stats:
        unsafe extern "C" fn(result: *mut u32, vals: *const f32, num_moves: i32) -> i32,
    pub gseconds_per_cycle: unsafe extern "C" fn(out: *mut f32) -> i32,
    pub script_cycles_f32: unsafe extern "C" fn(out: *mut f32) -> i32,
    pub script_cycles_clear: unsafe extern "C" fn() -> i32,
    pub script_entry_tag_clear: unsafe extern "C" fn() -> i32,
    pub actors_num: unsafe extern "C" fn(obj: Obj, out: *mut i32) -> i32,
    pub actors_get: unsafe extern "C" fn(obj: Obj, i: i32, out: *mut Obj) -> i32,
    pub actors_empty: unsafe extern "C" fn(obj: Obj, n: i32) -> i32,
    pub actors_add: unsafe extern "C" fn(obj: Obj, n: i32) -> i32,
    pub actors_count_bytes: unsafe extern "C" fn(obj: Obj, ar: Obj) -> i32,
    pub alevel_info_static_class: unsafe extern "C" fn(out: *mut Obj) -> i32,
    pub level_spawn_brush: unsafe extern "C" fn(obj: Obj, out: *mut Obj) -> i32,
    pub level_get_level_info: unsafe extern "C" fn(obj: Obj, out: *mut Obj) -> i32,
    pub level_info_set_high_detail: unsafe extern "C" fn(obj: Obj, v: i32) -> i32,
    pub level_info_has_game: unsafe extern "C" fn(obj: Obj, out: *mut i32) -> i32,
    pub level_info_game_event_detail_change: unsafe extern "C" fn(obj: Obj) -> i32,
    pub level_destroy_actor: unsafe extern "C" fn(obj: Obj, actor: Obj) -> i32,
    pub level_spawn_view_actor: unsafe extern "C" fn(obj: Obj, viewport: Obj) -> i32,
    pub level_single_line_check: unsafe extern "C" fn(
        obj: Obj,
        hit: *mut c_void,
        source: Obj,
        end: *const c_void,
        start: *const c_void,
        flags: u32,
        extent: *const c_void,
        node_flags: u8,
        out: *mut i32,
    ) -> i32,
    pub level_reach_specs_shrink: unsafe extern "C" fn(obj: Obj) -> i32,
    pub level_client_viewports_num: unsafe extern "C" fn(obj: Obj, out: *mut i32) -> i32,
    pub level_client_viewport: unsafe extern "C" fn(obj: Obj, i: i32, out: *mut Obj) -> i32,
    pub actor_iterator_get: unsafe extern "C" fn(iterator: Obj, out: *mut Obj) -> i32,
    pub actor_iterator_next: unsafe extern "C" fn(iterator: Obj) -> i32,
    pub object_get_outer: unsafe extern "C" fn(obj: Obj, out: *mut Obj) -> i32,
    pub level_time_seconds_get_float: unsafe extern "C" fn(obj: Obj, out: *mut f32) -> i32,
    pub collision_hash_new: unsafe extern "C" fn(out: *mut Obj) -> i32,
    pub collision_hash_add_actor: unsafe extern "C" fn(hash: Obj, actor: Obj) -> i32,
    pub collision_hash_remove_actor: unsafe extern "C" fn(hash: Obj, actor: Obj) -> i32,
    pub collision_hash_delete: unsafe extern "C" fn(hash: Obj) -> i32,
    pub brush_tracker_count_bytes: unsafe extern "C" fn(tracker: Obj, ar: Obj) -> i32,
    pub brush_tracker_delete: unsafe extern "C" fn(tracker: Obj) -> i32,
    pub net_driver_delete: unsafe extern "C" fn(driver: Obj) -> i32,
    pub actor_is_player: unsafe extern "C" fn(actor: Obj, out: *mut i32) -> i32,
    pub actor_group_chars: unsafe extern "C" fn(actor: Obj, out: *mut *const u32) -> i32,
    pub actor_event_on_resolve_game_state: unsafe extern "C" fn(actor: Obj) -> i32,
    pub actor_is_owned_by: unsafe extern "C" fn(actor: Obj, other: Obj, out: *mut i32) -> i32,
    pub actor_is_a_alevel_info: unsafe extern "C" fn(actor: Obj, out: *mut i32) -> i32,
    pub actor_is_a_aplayer_pawn: unsafe extern "C" fn(actor: Obj, out: *mut i32) -> i32,
    pub actor_is_a_camera: unsafe extern "C" fn(actor: Obj, out: *mut i32) -> i32,
    pub actor_cast_camera: unsafe extern "C" fn(actor: Obj, out: *mut Obj) -> i32,
    pub actor_class_is_child_of:
        unsafe extern "C" fn(actor: Obj, parent: Obj, out: *mut i32) -> i32,
    pub model_node_izone1_of_actor:
        unsafe extern "C" fn(actor: Obj, item: i32, out: *mut u8) -> i32,
    pub pawn_is_zone_visible: unsafe extern "C" fn(pawn: Obj, zone: u8, out: *mut i32) -> i32,
    pub camera_player_viewport: unsafe extern "C" fn(view: Obj, out: *mut Obj) -> i32,
    pub model_new_root: unsafe extern "C" fn(outer: Obj, root_outside: i32, out: *mut Obj) -> i32,
    pub model_new_brush: unsafe extern "C" fn(outer: Obj, owner: Obj, out: *mut Obj) -> i32,
    pub model_modify: unsafe extern "C" fn(model: Obj) -> i32,
    pub model_shrink_model: unsafe extern "C" fn(model: Obj) -> i32,
    pub model_fast_line_check: unsafe extern "C" fn(
        model: Obj,
        end: *const c_void,
        start: *const c_void,
        out: *mut i32,
    ) -> i32,
    pub uobject_get_outer: unsafe extern "C" fn(obj: Obj, out: *mut Obj) -> i32,
    pub uobject_set_flags: unsafe extern "C" fn(obj: Obj, flags: u32) -> i32,
    pub uobject_clear_flags: unsafe extern "C" fn(obj: Obj, flags: u32) -> i32,
    pub uobject_serialize: unsafe extern "C" fn(obj: Obj, ar: Obj) -> i32,
    pub uobject_destroy: unsafe extern "C" fn(obj: Obj) -> i32,
    pub uobject_modify: unsafe extern "C" fn(obj: Obj) -> i32,
    pub uobject_post_load: unsafe extern "C" fn(obj: Obj) -> i32,
    pub archive_tell: unsafe extern "C" fn(ar: Obj, out: *mut i32) -> i32,
    pub archive_supports_compact_index_trace: unsafe extern "C" fn(ar: Obj, out: *mut i32) -> i32,
    pub archive_set_compact_index_trace:
        unsafe extern "C" fn(ar: Obj, trace: *mut c_void, out: *mut *mut c_void) -> i32,
    pub archive_serialize_i32: unsafe extern "C" fn(ar: Obj, value: *mut i32) -> i32,
    pub archive_serialize_f32: unsafe extern "C" fn(ar: Obj, value: *mut f32) -> i32,
    pub archive_serialize_actors: unsafe extern "C" fn(ar: Obj, obj: Obj) -> i32,
    pub archive_serialize_actor_slot: unsafe extern "C" fn(ar: Obj, obj: Obj, i: i32) -> i32,
    pub archive_serialize_url: unsafe extern "C" fn(ar: Obj, obj: Obj) -> i32,
    pub archive_serialize_net_driver: unsafe extern "C" fn(ar: Obj, slot: *mut c_void) -> i32,
    pub archive_serialize_model: unsafe extern "C" fn(ar: Obj, slot: *mut c_void) -> i32,
    pub archive_serialize_reach_specs: unsafe extern "C" fn(ar: Obj, obj: Obj) -> i32,
    pub archive_serialize_actor_ptr: unsafe extern "C" fn(ar: Obj, slot: *mut c_void) -> i32,
    pub archive_serialize_text_buffer: unsafe extern "C" fn(ar: Obj, slot: *mut c_void) -> i32,
    pub archive_serialize_travel_info: unsafe extern "C" fn(ar: Obj, obj: Obj) -> i32,
    pub archive_preload: unsafe extern "C" fn(ar: Obj, model: Obj) -> i32,
    pub actor_slot_dump_env_path: unsafe extern "C" fn(out: *mut *const c_char) -> i32,
    pub actor_slot_dump_hooks_linked: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub actor_slot_dump_begin: unsafe extern "C" fn(obj: Obj) -> i32,
    pub actor_slot_dump_raw_actor: unsafe extern "C" fn(
        obj: Obj,
        slot: i32,
        actor: Obj,
        reference_before: i32,
        reference_after: i32,
        compact_index: i32,
        captured: i32,
        supported: i32,
    ) -> i32,
    pub actor_slot_dump_end: unsafe extern "C" fn(obj: Obj) -> i32,
    pub screen_invalid_state: unsafe extern "C" fn(pawn: Obj) -> i32,
    pub exclude_caps_contains: unsafe extern "C" fn(exclude: Obj, state: Obj, out: *mut i32) -> i32,
    pub screen_invalid_token: unsafe extern "C" fn(buffer: Obj, token: Obj) -> i32,
    pub debug_matched_viewport: unsafe extern "C" fn(viewport: Obj) -> i32,
    pub post_load_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub screen_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub collision_start_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub collision_end_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub reconcile_identical_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub reconcile_other_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub reconcile_kill_scope: unsafe extern "C" fn(obj: Obj) -> i32,
    pub level_base_serialize: unsafe extern "C" fn(obj: Obj, ar: Obj) -> i32,
    pub level_base_destroy: unsafe extern "C" fn(obj: Obj) -> i32,
    pub legacy_travel_scope: unsafe extern "C" fn(ar: Obj, obj: Obj) -> i32,
    pub archive_serialize_strings: unsafe extern "C" fn(ar: Obj, strings: Obj) -> i32,
    pub strings_num: unsafe extern "C" fn(strings: Obj, out: *mut i32) -> i32,
    pub travel_info_reset: unsafe extern "C" fn(obj: Obj) -> i32,
    pub travel_info_set: unsafe extern "C" fn(obj: Obj, names: Obj, items: Obj, index: i32) -> i32,
}

impl Hp2LevelCoreServices {
    unsafe fn fstring_caps(&self, dst: Obj, src: Obj) -> Result<(), ()> {
        if unsafe { (self.fstring_caps)(dst, src) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn fstring_len(&self, s: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.fstring_len)(s, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn fstring_assign_chars(&self, dst: Obj, src: *const u32) -> Result<(), ()> {
        if unsafe { (self.fstring_assign_chars)(dst, src) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn fstring_mid(&self, dst: Obj, src: Obj, start: i32, count: i32) -> Result<(), ()> {
        if unsafe { (self.fstring_mid)(dst, src, start, count) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn fstring_instr_chars(&self, s: Obj, sub: *const u32) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.fstring_instr_chars)(s, sub, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn fstring_instr_fstring(&self, s: Obj, sub: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.fstring_instr_fstring)(s, sub, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn fstring_eq_chars(&self, s: Obj, lit: *const u32) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.fstring_eq_chars)(s, lit, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn fstring_eq_fstring(&self, a: Obj, b: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.fstring_eq_fstring)(a, b, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn is_editor(&self) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.is_editor)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn check_failed(&self, expr: *const c_char, line: i32) -> i32 {
        unsafe { (self.check_failed)(expr, line) }
    }
    unsafe fn object_get_name(&self, dst: Obj, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.object_get_name)(dst, obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actor_tag_stricmp_viewport_name(&self, actor: Obj, viewport: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_tag_stricmp_viewport_name)(actor, viewport, out.as_mut_ptr()) } != 0
        {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn app_system_time_from_level_info(&self, info: Obj) -> Result<(), ()> {
        if unsafe { (self.app_system_time_from_level_info)(info) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn app_sprintf_stats(
        &self,
        result: *mut u32,
        vals: *const f32,
        num_moves: i32,
    ) -> Result<(), ()> {
        if unsafe { (self.app_sprintf_stats)(result, vals, num_moves) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn gseconds_per_cycle(&self) -> Result<f32, ()> {
        let mut out = std::mem::MaybeUninit::<f32>::uninit();
        if unsafe { (self.gseconds_per_cycle)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn script_cycles_f32(&self) -> Result<f32, ()> {
        let mut out = std::mem::MaybeUninit::<f32>::uninit();
        if unsafe { (self.script_cycles_f32)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn script_cycles_clear(&self) -> Result<(), ()> {
        if unsafe { (self.script_cycles_clear)() } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn script_entry_tag_clear(&self) -> Result<(), ()> {
        if unsafe { (self.script_entry_tag_clear)() } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actors_num(&self, obj: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actors_num)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actors_get(&self, obj: Obj, i: i32) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.actors_get)(obj, i, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actors_empty(&self, obj: Obj, n: i32) -> Result<(), ()> {
        if unsafe { (self.actors_empty)(obj, n) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actors_add(&self, obj: Obj, n: i32) -> Result<(), ()> {
        if unsafe { (self.actors_add)(obj, n) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actors_count_bytes(&self, obj: Obj, ar: Obj) -> Result<(), ()> {
        if unsafe { (self.actors_count_bytes)(obj, ar) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn alevel_info_static_class(&self) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.alevel_info_static_class)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_spawn_brush(&self, obj: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.level_spawn_brush)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_get_level_info(&self, obj: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.level_get_level_info)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_info_set_high_detail(&self, obj: Obj, v: i32) -> Result<(), ()> {
        if unsafe { (self.level_info_set_high_detail)(obj, v) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_info_has_game(&self, obj: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.level_info_has_game)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_info_game_event_detail_change(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.level_info_game_event_detail_change)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_destroy_actor(&self, obj: Obj, actor: Obj) -> Result<(), ()> {
        if unsafe { (self.level_destroy_actor)(obj, actor) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_spawn_view_actor(&self, obj: Obj, viewport: Obj) -> Result<(), ()> {
        if unsafe { (self.level_spawn_view_actor)(obj, viewport) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_single_line_check(
        &self,
        obj: Obj,
        hit: *mut c_void,
        source: Obj,
        end: *const c_void,
        start: *const c_void,
        flags: u32,
        extent: *const c_void,
        node_flags: u8,
    ) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe {
            (self.level_single_line_check)(
                obj,
                hit,
                source,
                end,
                start,
                flags,
                extent,
                node_flags,
                out.as_mut_ptr(),
            )
        } != 0
        {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_reach_specs_shrink(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.level_reach_specs_shrink)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_has_client(&self, obj: Obj) -> Result<i32, ()> {
        let engine = unsafe { (*obj.cast::<ULevel>()).base.engine };
        Ok(if unsafe { (*engine.cast::<UGameEngine>()).client }.is_null() {
            0
        } else {
            1
        })
    }
    unsafe fn level_client_viewports_num(&self, obj: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.level_client_viewports_num)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn level_client_viewport(&self, obj: Obj, i: i32) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.level_client_viewport)(obj, i, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_iterator_get(&self, iterator: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.actor_iterator_get)(iterator, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_iterator_next(&self, iterator: Obj) -> Result<(), ()> {
        if unsafe { (self.actor_iterator_next)(iterator) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn object_get_outer(&self, obj: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.object_get_outer)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_set_xlevel(&self, actor: Obj, level: Obj) -> Result<(), ()> {
        unsafe { (*actor.cast::<AActor>()).x_level = level };
        Ok(())
    }
    unsafe fn level_time_seconds_get_float(&self, obj: Obj) -> Result<f32, ()> {
        let mut out = std::mem::MaybeUninit::<f32>::uninit();
        if unsafe { (self.level_time_seconds_get_float)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn collision_hash_new(&self) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.collision_hash_new)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn collision_hash_add_actor(&self, hash: Obj, actor: Obj) -> Result<(), ()> {
        if unsafe { (self.collision_hash_add_actor)(hash, actor) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn collision_hash_remove_actor(&self, hash: Obj, actor: Obj) -> Result<(), ()> {
        if unsafe { (self.collision_hash_remove_actor)(hash, actor) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn collision_hash_delete(&self, hash: Obj) -> Result<(), ()> {
        if unsafe { (self.collision_hash_delete)(hash) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn brush_tracker_count_bytes(&self, tracker: Obj, ar: Obj) -> Result<(), ()> {
        if unsafe { (self.brush_tracker_count_bytes)(tracker, ar) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn brush_tracker_delete(&self, tracker: Obj) -> Result<(), ()> {
        if unsafe { (self.brush_tracker_delete)(tracker) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn net_driver_delete(&self, driver: Obj) -> Result<(), ()> {
        if unsafe { (self.net_driver_delete)(driver) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actor_collide_actors(&self, actor: Obj) -> Result<i32, ()> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_collide_actors() } as i32)
    }
    unsafe fn actor_is_player(&self, actor: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_is_player)(actor, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_group_chars(&self, actor: Obj) -> Result<*const u32, ()> {
        let mut out = std::mem::MaybeUninit::<*const u32>::uninit();
        if unsafe { (self.actor_group_chars)(actor, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_exclude_game_states(&self, actor: Obj) -> Result<Obj, ()> {
        Ok(unsafe { ptr::addr_of_mut!((*actor.cast::<AActor>()).exclude_game_states) }.cast())
    }
    unsafe fn actor_set_hidden_ed(&self, actor: Obj, v: i32) -> Result<(), ()> {
        unsafe { (*actor.cast::<AActor>()).set_b_hidden_ed(v != 0) };
        Ok(())
    }
    unsafe fn actor_set_in_current_game_state(&self, actor: Obj, v: i32) -> Result<(), ()> {
        unsafe { (*actor.cast::<AActor>()).set_b_in_current_game_state(v != 0) };
        Ok(())
    }
    unsafe fn actor_event_on_resolve_game_state(&self, actor: Obj) -> Result<(), ()> {
        if unsafe { (self.actor_event_on_resolve_game_state)(actor) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actor_is_owned_by(&self, actor: Obj, other: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_is_owned_by)(actor, other, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_is_a_alevel_info(&self, actor: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_is_a_alevel_info)(actor, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_is_a_aplayer_pawn(&self, actor: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_is_a_aplayer_pawn)(actor, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_is_a_camera(&self, actor: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_is_a_camera)(actor, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_cast_camera(&self, actor: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.actor_cast_camera)(actor, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_class_is_child_of(&self, actor: Obj, parent: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_class_is_child_of)(actor, parent, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn model_node_izone1_of_actor(&self, actor: Obj, item: i32) -> Result<u8, ()> {
        let mut out = std::mem::MaybeUninit::<u8>::uninit();
        if unsafe { (self.model_node_izone1_of_actor)(actor, item, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn pawn_current_game_state(&self, pawn: Obj) -> Result<Obj, ()> {
        Ok(unsafe { ptr::addr_of_mut!((*pawn.cast::<APlayerPawn>()).current_game_state) }.cast())
    }
    unsafe fn pawn_game_state_master_list(&self, pawn: Obj) -> Result<Obj, ()> {
        Ok(unsafe { ptr::addr_of_mut!((*pawn.cast::<APlayerPawn>()).game_state_master_list) }
            .cast())
    }
    unsafe fn pawn_game_state_token_len(&self, pawn: Obj) -> Result<i32, ()> {
        Ok(unsafe { (*pawn.cast::<APlayerPawn>()).game_state_token_len })
    }
    unsafe fn pawn_is_zone_visible(&self, pawn: Obj, zone: u8) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.pawn_is_zone_visible)(pawn, zone, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn pawn_get_player(&self, pawn: Obj) -> Result<Obj, ()> {
        Ok(unsafe { (*pawn.cast::<APlayerPawn>()).player })
    }
    unsafe fn pawn_set_player(&self, pawn: Obj, player: Obj) -> Result<(), ()> {
        unsafe { (*pawn.cast::<APlayerPawn>()).player = player };
        Ok(())
    }
    unsafe fn camera_player_viewport(&self, view: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.camera_player_viewport)(view, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn camera_restore_view(&self, view: Obj, viewport: Obj) -> Result<(), ()> {
        unsafe {
            let cam = view.cast::<APlayerPawn>();
            let vp = &*viewport.cast::<UViewport>();
            (*cam).base.ortho_zoom = vp.saved_ortho_zoom;
            (*cam).base.fov_angle = vp.saved_fov_angle;
            (*cam).show_flags = vp.saved_show_flags;
            (*cam).rend_map = vp.saved_rend_map;
            (*cam).misc1 = vp.saved_misc1;
            (*cam).misc2 = vp.saved_misc2;
        }
        Ok(())
    }
    unsafe fn viewport_get_actor(&self, viewport: Obj) -> Result<Obj, ()> {
        Ok(unsafe { (*viewport.cast::<UViewport>()).actor })
    }
    unsafe fn viewport_match_actor(&self, viewport: Obj, actor: Obj) -> Result<(), ()> {
        unsafe {
            (*viewport.cast::<UViewport>()).actor = actor;
            (*actor.cast::<APlayerPawn>()).player = viewport;
        }
        Ok(())
    }
    unsafe fn viewport_remember_actor(&self, viewport: Obj) -> Result<(), ()> {
        unsafe {
            let vp = viewport.cast::<UViewport>();
            let act = (*vp).actor.cast::<APlayerPawn>();
            (*vp).saved_ortho_zoom = (*act).base.ortho_zoom;
            (*vp).saved_fov_angle = (*act).base.fov_angle;
            (*vp).saved_show_flags = (*act).show_flags;
            (*vp).saved_rend_map = (*act).rend_map;
            (*vp).saved_misc1 = (*act).misc1;
            (*vp).saved_misc2 = (*act).misc2;
            (*vp).actor = ptr::null_mut();
        }
        Ok(())
    }
    unsafe fn check_result_assign(&self, dst: *mut c_void, src: *const c_void) -> Result<(), ()> {
        unsafe { *dst.cast::<FCheckResult>() = *src.cast::<FCheckResult>() };
        Ok(())
    }
    unsafe fn model_new_root(&self, outer: Obj, root_outside: i32) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.model_new_root)(outer, root_outside, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn model_new_brush(&self, outer: Obj, owner: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.model_new_brush)(outer, owner, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn model_modify(&self, model: Obj) -> Result<(), ()> {
        if unsafe { (self.model_modify)(model) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn model_shrink_model(&self, model: Obj) -> Result<(), ()> {
        if unsafe { (self.model_shrink_model)(model) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn model_fast_line_check(
        &self,
        model: Obj,
        end: *const c_void,
        start: *const c_void,
    ) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.model_fast_line_check)(model, end, start, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn brush_actor_set_brush(&self, actor: Obj, brush: Obj) -> Result<(), ()> {
        unsafe { (*actor.cast::<AActor>()).brush = brush };
        Ok(())
    }
    unsafe fn uobject_get_outer(&self, obj: Obj) -> Result<Obj, ()> {
        let mut out = std::mem::MaybeUninit::<Obj>::uninit();
        if unsafe { (self.uobject_get_outer)(obj, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn uobject_set_flags(&self, obj: Obj, flags: u32) -> Result<(), ()> {
        if unsafe { (self.uobject_set_flags)(obj, flags) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn uobject_clear_flags(&self, obj: Obj, flags: u32) -> Result<(), ()> {
        if unsafe { (self.uobject_clear_flags)(obj, flags) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn uobject_serialize(&self, obj: Obj, ar: Obj) -> Result<(), ()> {
        if unsafe { (self.uobject_serialize)(obj, ar) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn uobject_destroy(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.uobject_destroy)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn uobject_modify(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.uobject_modify)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn uobject_post_load(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.uobject_post_load)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_is_loading(&self, ar: Obj) -> Result<i32, ()> {
        Ok(unsafe { (*ar.cast::<FArchive>()).ar_is_loading })
    }
    unsafe fn archive_is_saving(&self, ar: Obj) -> Result<i32, ()> {
        Ok(unsafe { (*ar.cast::<FArchive>()).ar_is_saving })
    }
    unsafe fn archive_is_trans(&self, ar: Obj) -> Result<i32, ()> {
        Ok(unsafe { (*ar.cast::<FArchive>()).ar_is_trans })
    }
    unsafe fn archive_ver(&self, ar: Obj) -> Result<i32, ()> {
        Ok(unsafe { (*ar.cast::<FArchive>()).ar_ver })
    }
    unsafe fn archive_tell(&self, ar: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.archive_tell)(ar, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn archive_supports_compact_index_trace(&self, ar: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.archive_supports_compact_index_trace)(ar, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn archive_set_compact_index_trace(
        &self,
        ar: Obj,
        trace: *mut c_void,
    ) -> Result<*mut c_void, ()> {
        let mut out = std::mem::MaybeUninit::<*mut c_void>::uninit();
        if unsafe { (self.archive_set_compact_index_trace)(ar, trace, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn archive_serialize_i32(&self, ar: Obj, value: *mut i32) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_i32)(ar, value) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_f32(&self, ar: Obj, value: *mut f32) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_f32)(ar, value) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_actors(&self, ar: Obj, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_actors)(ar, obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_actor_slot(&self, ar: Obj, obj: Obj, i: i32) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_actor_slot)(ar, obj, i) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_url(&self, ar: Obj, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_url)(ar, obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_net_driver(&self, ar: Obj, slot: *mut c_void) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_net_driver)(ar, slot) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_model(&self, ar: Obj, slot: *mut c_void) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_model)(ar, slot) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_reach_specs(&self, ar: Obj, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_reach_specs)(ar, obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_actor_ptr(&self, ar: Obj, slot: *mut c_void) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_actor_ptr)(ar, slot) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_text_buffer(&self, ar: Obj, slot: *mut c_void) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_text_buffer)(ar, slot) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_travel_info(&self, ar: Obj, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_travel_info)(ar, obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_preload(&self, ar: Obj, model: Obj) -> Result<(), ()> {
        if unsafe { (self.archive_preload)(ar, model) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actor_slot_dump_env_path(&self) -> Result<*const c_char, ()> {
        let mut out = std::mem::MaybeUninit::<*const c_char>::uninit();
        if unsafe { (self.actor_slot_dump_env_path)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_slot_dump_hooks_linked(&self) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.actor_slot_dump_hooks_linked)(out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn actor_slot_dump_begin(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.actor_slot_dump_begin)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actor_slot_dump_raw_actor(
        &self,
        obj: Obj,
        slot: i32,
        actor: Obj,
        reference_before: i32,
        reference_after: i32,
        compact_index: i32,
        captured: i32,
        supported: i32,
    ) -> Result<(), ()> {
        if unsafe {
            (self.actor_slot_dump_raw_actor)(
                obj,
                slot,
                actor,
                reference_before,
                reference_after,
                compact_index,
                captured,
                supported,
            )
        } == 0
        {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn actor_slot_dump_end(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.actor_slot_dump_end)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn screen_invalid_state(&self, pawn: Obj) -> Result<(), ()> {
        if unsafe { (self.screen_invalid_state)(pawn) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn exclude_caps_contains(&self, exclude: Obj, state: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.exclude_caps_contains)(exclude, state, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn screen_invalid_token(&self, buffer: Obj, token: Obj) -> Result<(), ()> {
        if unsafe { (self.screen_invalid_token)(buffer, token) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn debug_matched_viewport(&self, viewport: Obj) -> Result<(), ()> {
        if unsafe { (self.debug_matched_viewport)(viewport) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn post_load_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.post_load_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn screen_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.screen_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn collision_start_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.collision_start_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn collision_end_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.collision_end_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn reconcile_identical_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.reconcile_identical_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn reconcile_other_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.reconcile_other_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn reconcile_kill_scope(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.reconcile_kill_scope)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_base_serialize(&self, obj: Obj, ar: Obj) -> Result<(), ()> {
        if unsafe { (self.level_base_serialize)(obj, ar) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn level_base_destroy(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.level_base_destroy)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn legacy_travel_scope(&self, ar: Obj, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.legacy_travel_scope)(ar, obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn archive_serialize_strings(&self, ar: Obj, strings: Obj) -> Result<(), ()> {
        if unsafe { (self.archive_serialize_strings)(ar, strings) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn strings_num(&self, strings: Obj) -> Result<i32, ()> {
        let mut out = std::mem::MaybeUninit::<i32>::uninit();
        if unsafe { (self.strings_num)(strings, out.as_mut_ptr()) } != 0 {
            return Err(());
        }
        Ok(unsafe { out.assume_init() })
    }
    unsafe fn travel_info_reset(&self, obj: Obj) -> Result<(), ()> {
        if unsafe { (self.travel_info_reset)(obj) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn travel_info_set(
        &self,
        obj: Obj,
        names: Obj,
        items: Obj,
        index: i32,
    ) -> Result<(), ()> {
        if unsafe { (self.travel_info_set)(obj, names, items, index) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

const RF_TRANSACTIONAL: u32 = 0x0000_0001;
const RF_NOT_FOR_CLIENT: u32 = 0x0010_0000;
const RF_NOT_FOR_SERVER: u32 = 0x0020_0000;
const INDEX_NONE: i32 = -1;
const MAXINT: i32 = 0x7fff_ffff;
const NUM_LEVEL_TEXT_BLOCKS: usize = 16;
const TRACE_VIS_BLOCKING: u32 = 0x06;
const ZONE_FACTOR: f64 = 0.85;
const ZONE_DIST_DIM: isize = 64;
const OCCLUSION_DEFAULT: i32 = 0;
const OCCLUSION_BSP: i32 = 2;
const OCCLUSION_STATIC_MESHES: i32 = 3;
const GSTATE: [u32; 7] = [71, 83, 84, 65, 84, 69, 0];
const COMMA: [u32; 2] = [44, 0];
const NONE: [u32; 5] = [78, 79, 78, 69, 0];
const ZERO: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

fn lv(obj: Obj) -> *mut ULevel {
    obj.cast()
}

/// `FDist(V1, V2)` = `appSqrt(Square(dx)+Square(dy)+Square(dz))` (UnMath.h:2161).
fn fdist(v1: FVector, v2: FVector) -> f32 {
    let dx = v2.x - v1.x;
    let dy = v2.y - v1.y;
    let dz = v2.z - v1.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// `Start + V` (operator+ adds component-wise, `X + V.X`).
fn vec_add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

/// `FLOAT * FVector` (operator*(FLOAT, V) returns `V.X * Scale` per component).
fn vec_scale(scale: f32, v: FVector) -> FVector {
    FVector {
        x: v.x * scale,
        y: v.y * scale,
        z: v.z * scale,
    }
}

/// `ULevelBase::Serialize` (UnLevel.cpp:36).
///
/// # Safety
/// `obj` must be a live ULevelBase; `ar` a live FArchive.
pub unsafe fn level_base_serialize(s: &Hp2LevelCoreServices, obj: Obj, ar: Obj) -> Result<(), ()> {
    unsafe {
        s.uobject_serialize(obj, ar)?;
        if s.archive_is_trans(ar)? != 0 {
            s.archive_serialize_actors(ar, obj)?;
        } else {
            // oldver: old-format actor list.
            let mut db_num = s.actors_num(obj)?;
            let mut db_max = db_num;
            s.actors_count_bytes(obj, ar)?;
            s.archive_serialize_i32(ar, &mut db_num)?;
            s.archive_serialize_i32(ar, &mut db_max)?;
            if s.archive_is_loading(ar)? != 0 {
                s.actors_empty(obj, db_num)?;
                s.actors_add(obj, db_num)?;
            }
            let slot_dump_path = s.actor_slot_dump_env_path()?;
            let capture = s.archive_is_loading(ar)? != 0
                && !slot_dump_path.is_null()
                && *slot_dump_path != 0
                && s.actor_slot_dump_hooks_linked()? != 0;
            if capture {
                s.actor_slot_dump_begin(obj)?;
            }
            let mut i = 0;
            while i < s.actors_num(obj)? {
                let mut trace = FActorSlotCompactIndexTrace {
                    index: INDEX_NONE,
                    offset_after: INDEX_NONE,
                    captured: 0,
                };
                let reference_before = if capture {
                    s.archive_tell(ar)?
                } else {
                    INDEX_NONE
                };
                let capture_compact = capture && s.archive_supports_compact_index_trace(ar)? != 0;
                let mut previous: *mut c_void = ptr::null_mut();
                if capture_compact {
                    previous =
                        s.archive_set_compact_index_trace(ar, ptr::addr_of_mut!(trace).cast())?;
                }
                s.archive_serialize_actor_slot(ar, obj, i)?;
                if capture_compact {
                    s.archive_set_compact_index_trace(ar, previous)?;
                }
                if capture {
                    s.actor_slot_dump_raw_actor(
                        obj,
                        i,
                        s.actors_get(obj, i)?,
                        reference_before,
                        trace.offset_after,
                        trace.index,
                        trace.captured,
                        capture_compact as i32,
                    )?;
                }
                i += 1;
            }
            if capture {
                s.actor_slot_dump_end(obj)?;
            }
        }
        // Level variables (after both branches, as in the original).
        s.archive_serialize_url(ar, obj)?;
        if s.archive_is_loading(ar)? == 0 && s.archive_is_saving(ar)? == 0 {
            s.archive_serialize_net_driver(ar, ptr::addr_of_mut!((*lv(obj)).base.net_driver).cast())?;
            s.archive_serialize_net_driver(
                ar,
                ptr::addr_of_mut!((*lv(obj)).base.demo_rec_driver).cast(),
            )?;
        }
    }
    Ok(())
}

/// `ULevelBase::Destroy` (UnLevel.cpp:91).
///
/// # Safety
/// `obj` must be a live ULevelBase.
pub unsafe fn level_base_destroy(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        if !(*level).base.net_driver.is_null() {
            s.net_driver_delete((*level).base.net_driver)?;
            (*level).base.net_driver = ptr::null_mut();
        }
        if !(*level).base.demo_rec_driver.is_null() {
            s.net_driver_delete((*level).base.demo_rec_driver)?;
            (*level).base.demo_rec_driver = ptr::null_mut();
        }
        s.uobject_destroy(obj)?;
    }
    Ok(())
}

/// `ULevel::ULevel` (UnLevel.cpp:123). The C++ constructor body runs `ULevelBase(InEngine)` first.
///
/// # Safety
/// `obj` must be a freshly constructed ULevel.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn level_init(s: &Hp2LevelCoreServices, obj: Obj, root_outside: i32) -> Result<(), ()> {
    unsafe {
        // Allocate subobjects.
        s.uobject_set_flags(obj, RF_TRANSACTIONAL)?;
        let model = s.model_new_root(s.uobject_get_outer(obj)?, root_outside)?;
        (*lv(obj)).model = model;
        s.uobject_set_flags(model, RF_TRANSACTIONAL)?;
        // Spawn the level info. Calls `spawn_actor` directly, not
        // `virtual_spawn_actor`: this runs inside `ULevel::ULevel`
        // (UnLevel.cpp:123), where the vtable still resolves statically to
        // `ULevel::SpawnActor` (no subclass override can be live yet).
        crate::level_spawn_actor::spawn_actor(
            obj,
            s.alevel_info_static_class()?,
            FName { index: 0 },
            ptr::null_mut(),
            ptr::null_mut(),
            FVector { x: 0.0, y: 0.0, z: 0.0 },
            FRotator { pitch: 0, yaw: 0, roll: 0 },
            ptr::null_mut(),
            0,
            0,
        )
        .map_err(|_| ())?;
        if s.level_get_level_info(obj)?.is_null()
            && s.check_failed(c"GetLevelInfo()".as_ptr(), 135) != 0
        {
            return Err(());
        }
        // Spawn the default brush.
        let temp = s.level_spawn_brush(obj)?;
        if temp != s.actors_get(obj, 1)? && s.check_failed(c"Temp==Actors(1)".as_ptr(), 139) != 0 {
            return Err(());
        }
        let brush = s.model_new_brush(s.uobject_get_outer(obj)?, temp)?;
        s.brush_actor_set_brush(temp, brush)?;
        s.uobject_set_flags(
            temp,
            RF_NOT_FOR_CLIENT | RF_NOT_FOR_SERVER | RF_TRANSACTIONAL,
        )?;
        s.uobject_set_flags(
            brush,
            RF_NOT_FOR_CLIENT | RF_NOT_FOR_SERVER | RF_TRANSACTIONAL,
        )?;
        Ok(())
    }
}

/// Typed backend operations for the shared `ScreenActorsByGameState` policy.
///
/// # Safety
/// Implementations must access only live objects belonging to their backend.
pub(crate) trait ScreenActorsOps {
    type Error;
    // Observation must not call script, modify the game, or change its errors.
    unsafe fn screen_trace(&self, _event: &'static str, _actor: Obj, _text: Obj, _value: i32) {}
    unsafe fn actors_num(&self, level: Obj) -> Result<i32, Self::Error>;
    unsafe fn actors_get(&self, level: Obj, index: i32) -> Result<Obj, Self::Error>;
    unsafe fn actor_is_player(&self, actor: Obj) -> Result<i32, Self::Error>;
    unsafe fn fstring_caps(&self, dst: Obj, src: Obj) -> Result<(), Self::Error>;
    unsafe fn pawn_current_game_state(&self, pawn: Obj) -> Result<Obj, Self::Error>;
    unsafe fn fstring_eq_chars(&self, text: Obj, chars: *const u32) -> Result<i32, Self::Error>;
    unsafe fn pawn_game_state_master_list(&self, pawn: Obj) -> Result<Obj, Self::Error>;
    unsafe fn fstring_instr_fstring(&self, text: Obj, sub: Obj) -> Result<i32, Self::Error>;
    unsafe fn screen_invalid_state(&self, pawn: Obj) -> Result<(), Self::Error>;
    unsafe fn fstring_assign_chars(&self, dst: Obj, chars: *const u32) -> Result<(), Self::Error>;
    unsafe fn actor_group_chars(&self, actor: Obj) -> Result<*const u32, Self::Error>;
    unsafe fn actor_exclude_game_states(&self, actor: Obj) -> Result<Obj, Self::Error>;
    unsafe fn fstring_len(&self, text: Obj) -> Result<i32, Self::Error>;
    unsafe fn exclude_caps_contains(&self, exclude: Obj, state: Obj) -> Result<i32, Self::Error>;
    unsafe fn fstring_instr_chars(&self, text: Obj, chars: *const u32) -> Result<i32, Self::Error>;
    unsafe fn is_editor(&self) -> Result<i32, Self::Error>;
    unsafe fn actor_set_hidden_ed(&self, actor: Obj, value: i32) -> Result<(), Self::Error>;
    unsafe fn fstring_mid(&self, dst: Obj, src: Obj, start: i32, count: i32) -> Result<(), Self::Error>;
    unsafe fn pawn_game_state_token_len(&self, pawn: Obj) -> Result<i32, Self::Error>;
    unsafe fn fstring_eq_fstring(&self, first: Obj, second: Obj) -> Result<i32, Self::Error>;
    unsafe fn actor_set_in_current_game_state(&self, actor: Obj, value: i32) -> Result<(), Self::Error>;
    unsafe fn actor_event_on_resolve_game_state(&self, actor: Obj) -> Result<(), Self::Error>;
    unsafe fn object_get_name(&self, dst: Obj, object: Obj) -> Result<(), Self::Error>;
    unsafe fn screen_invalid_token(&self, actor_name: Obj, token: Obj) -> Result<(), Self::Error>;
}

impl ScreenActorsOps for Hp2LevelCoreServices {
    type Error = ();
    unsafe fn actors_num(&self, o: Obj) -> Result<i32, ()> { unsafe { self.actors_num(o) } }
    unsafe fn actors_get(&self, o: Obj, i: i32) -> Result<Obj, ()> { unsafe { self.actors_get(o, i) } }
    unsafe fn actor_is_player(&self, a: Obj) -> Result<i32, ()> { unsafe { self.actor_is_player(a) } }
    unsafe fn fstring_caps(&self, d: Obj, s: Obj) -> Result<(), ()> { unsafe { self.fstring_caps(d, s) } }
    unsafe fn pawn_current_game_state(&self, p: Obj) -> Result<Obj, ()> { unsafe { self.pawn_current_game_state(p) } }
    unsafe fn fstring_eq_chars(&self, s: Obj, c: *const u32) -> Result<i32, ()> { unsafe { self.fstring_eq_chars(s, c) } }
    unsafe fn pawn_game_state_master_list(&self, p: Obj) -> Result<Obj, ()> { unsafe { self.pawn_game_state_master_list(p) } }
    unsafe fn fstring_instr_fstring(&self, s: Obj, sub: Obj) -> Result<i32, ()> { unsafe { self.fstring_instr_fstring(s, sub) } }
    unsafe fn screen_invalid_state(&self, p: Obj) -> Result<(), ()> { unsafe { self.screen_invalid_state(p) } }
    unsafe fn fstring_assign_chars(&self, d: Obj, c: *const u32) -> Result<(), ()> { unsafe { self.fstring_assign_chars(d, c) } }
    unsafe fn actor_group_chars(&self, a: Obj) -> Result<*const u32, ()> { unsafe { self.actor_group_chars(a) } }
    unsafe fn actor_exclude_game_states(&self, a: Obj) -> Result<Obj, ()> { unsafe { self.actor_exclude_game_states(a) } }
    unsafe fn fstring_len(&self, s: Obj) -> Result<i32, ()> { unsafe { self.fstring_len(s) } }
    unsafe fn exclude_caps_contains(&self, e: Obj, s: Obj) -> Result<i32, ()> { unsafe { self.exclude_caps_contains(e, s) } }
    unsafe fn fstring_instr_chars(&self, s: Obj, c: *const u32) -> Result<i32, ()> { unsafe { self.fstring_instr_chars(s, c) } }
    unsafe fn is_editor(&self) -> Result<i32, ()> { unsafe { self.is_editor() } }
    unsafe fn actor_set_hidden_ed(&self, a: Obj, v: i32) -> Result<(), ()> { unsafe { self.actor_set_hidden_ed(a, v) } }
    unsafe fn fstring_mid(&self, d: Obj, s: Obj, i: i32, n: i32) -> Result<(), ()> { unsafe { self.fstring_mid(d, s, i, n) } }
    unsafe fn pawn_game_state_token_len(&self, p: Obj) -> Result<i32, ()> { unsafe { self.pawn_game_state_token_len(p) } }
    unsafe fn fstring_eq_fstring(&self, a: Obj, b: Obj) -> Result<i32, ()> { unsafe { self.fstring_eq_fstring(a, b) } }
    unsafe fn actor_set_in_current_game_state(&self, a: Obj, v: i32) -> Result<(), ()> { unsafe { self.actor_set_in_current_game_state(a, v) } }
    unsafe fn actor_event_on_resolve_game_state(&self, a: Obj) -> Result<(), ()> { unsafe { self.actor_event_on_resolve_game_state(a) } }
    unsafe fn object_get_name(&self, d: Obj, o: Obj) -> Result<(), ()> { unsafe { self.object_get_name(d, o) } }
    unsafe fn screen_invalid_token(&self, n: Obj, t: Obj) -> Result<(), ()> { unsafe { self.screen_invalid_token(n, t) } }
}
/// `ULevel::ScreenActorsByGameState` (`UnLevel.cpp:154`), native adapter.
pub unsafe fn level_screen_actors_body(
    services: &Hp2LevelCoreServices,
    obj: Obj,
    cur_gstate: Obj,
    buffer: Obj,
    token: Obj,
) -> Result<(), ()> {
    unsafe { level_screen_actors_body_with(services, obj, cur_gstate, buffer, token) }
}

pub unsafe fn level_screen_actors_body_with<O: ScreenActorsOps>(
    s: &O,
    obj: Obj,
    cur_gstate: Obj,
    buffer: Obj,
    token: Obj,
) -> Result<(), O::Error> {
    unsafe {
        let mut pawn: Obj = ptr::null_mut();

        // *** Get PlayerPawn: the player pawn has the CurrentGameState.
        let mut i = 0;
        while i < s.actors_num(obj)? {
            let act = s.actors_get(obj, i)?;
            if !act.is_null() && s.actor_is_player(act)? != 0 {
                pawn = act;
                s.fstring_caps(cur_gstate, s.pawn_current_game_state(pawn)?)?;
                break;
            }
            i += 1;
        }

        s.screen_trace("player", pawn, cur_gstate, 0);
        // *** Error checking: if the current state is not set then return.
        if s.fstring_eq_chars(cur_gstate, NONE.as_ptr())? != 0 {
            s.screen_trace("none_state", pawn, cur_gstate, 0);
            return Ok(());
        }
        // If we didn't find the player or its CurGState is invalid, return.
        if pawn.is_null()
            || s.fstring_instr_fstring(s.pawn_game_state_master_list(pawn)?, cur_gstate)? < 0
        {
            s.screen_trace("invalid_state", pawn, cur_gstate, 0);
            s.screen_invalid_state(pawn)?;
            return Ok(());
        }
        s.screen_trace("valid_state", pawn, cur_gstate, 0);

        // *** Screen Actors: screen them according to the current GameState.
        let mut i = 0;
        while i < s.actors_num(obj)? {
            let act = s.actors_get(obj, i)?;
            i += 1;
            if act.is_null() {
                continue;
            }
            // Reset local vars for the next token.
            s.fstring_assign_chars(buffer, s.actor_group_chars(act)?)?;
            s.fstring_caps(buffer, buffer)?;
            s.screen_trace("actor_group", act, buffer, 0);
            let mut has_gstate = false;
            let mut in_cur_gstate = false;
            loop {
                // See if this actor has an excludeGameState list containing CurGState.
                let exclude = s.actor_exclude_game_states(act)?;
                s.screen_trace("actor_exclude", act, exclude, 0);
                if s.fstring_len(exclude)? != 0 {
                    if s.exclude_caps_contains(exclude, cur_gstate)? != 0 {
                        // CurGState is in the actor's *exclude* list.
                        has_gstate = true;
                        in_cur_gstate = false;
                        break;
                    }
                }
                // Is the GState *keyword* in the group name?
                let j = s.fstring_instr_chars(buffer, GSTATE.as_ptr())?;
                if j < 0 {
                    break;
                }
                // This actor has a GState.
                has_gstate = true;
                if s.is_editor()? != 0 {
                    s.actor_set_hidden_ed(act, 0)?;
                }
                // Start the buffer at the "GS" keyword.
                s.fstring_mid(buffer, buffer, j, MAXINT)?;
                // Current token.
                let mut len = s.fstring_instr_chars(buffer, COMMA.as_ptr())?;
                if len < 0 {
                    len = s.fstring_len(buffer)?;
                }
                s.fstring_mid(token, buffer, 0, len)?;
                if s.fstring_len(token)? == s.pawn_game_state_token_len(pawn)?
                    && s.fstring_instr_fstring(s.pawn_game_state_master_list(pawn)?, token)? >= 0
                {
                    // Is this actor's GState the *current* GState?
                    if s.fstring_eq_fstring(token, cur_gstate)? != 0
                        && s.fstring_instr_fstring(s.actor_exclude_game_states(act)?, cur_gstate)?
                            < 0
                    {
                        in_cur_gstate = true;
                        break;
                    }
                } else {
                    // ERROR: the GState wasn't in the master list.
                    s.object_get_name(buffer, act)?;
                    s.screen_invalid_token(buffer, token)?;
                }
                // Cut out the token.
                s.fstring_mid(buffer, buffer, s.fstring_len(token)?, MAXINT)?;
            }
            // Set bInCurrentGameState and resolve the GameState if this actor has one.
            s.screen_trace("decision", act, ptr::null_mut(),
                if has_gstate { i32::from(in_cur_gstate) } else { -1 });
            if has_gstate {
                s.actor_set_in_current_game_state(act, in_cur_gstate as i32)?;
                if s.is_editor()? == 0 {
                    s.screen_trace("resolve_begin", act, ptr::null_mut(), 0);
                    let result = s.actor_event_on_resolve_game_state(act);
                    s.screen_trace("resolve_end", act, ptr::null_mut(), i32::from(result.is_ok()));
                    result?;
                }
            }
        }
        s.screen_trace("screen_complete", ptr::null_mut(), cur_gstate, 0);
    }
    Ok(())
}

/// `ULevel::ShrinkLevel` (UnLevel.cpp:930).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_shrink(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        s.model_shrink_model((*lv(obj)).model)?;
        s.level_reach_specs_shrink(obj)?;
    }
    Ok(())
}

/// Typed leaves used by the Native and Core adapters of Level::DetailChange.
pub(crate) trait DetailChangeOps {
    type Error;
    fn set_high_detail(&mut self, level: Obj, value: i32) -> Result<(), Self::Error>;
    fn has_game(&mut self, level: Obj) -> Result<bool, Self::Error>;
    fn event_detail_change(&mut self, level: Obj) -> Result<(), Self::Error>;
}

struct NativeDetailChangeOps<'a>(&'a Hp2LevelCoreServices);
impl DetailChangeOps for NativeDetailChangeOps<'_> {
    type Error = ();
    fn set_high_detail(&mut self, level: Obj, value: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_info_set_high_detail(level, value) }
    }
    fn has_game(&mut self, level: Obj) -> Result<bool, Self::Error> {
        unsafe { self.0.level_info_has_game(level) }.map(|value| value != 0)
    }
    fn event_detail_change(&mut self, level: Obj) -> Result<(), Self::Error> {
        unsafe { self.0.level_info_game_event_detail_change(level) }
    }
}

/// Runs the source-shared `ULevel::DetailChange` policy with typed owner leaves.
///
/// # Safety
/// `level` must be a live ULevel with a level info.
pub(crate) unsafe fn level_detail_change_with<O: DetailChangeOps>(
    ops: &mut O,
    level: Obj,
    new_detail: i32,
) -> Result<(), O::Error> {
    ops.set_high_detail(level, new_detail)?;
    if ops.has_game(level)? {
        ops.event_detail_change(level)?;
    }
    Ok(())
}

/// `ULevel::DetailChange` (UnLevel.cpp:939).
///
/// # Safety
/// `obj` must be a live ULevel with a level info.
pub unsafe fn level_detail_change(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    new_detail: i32,
) -> Result<(), ()> {
    unsafe { level_detail_change_with(&mut NativeDetailChangeOps(s), obj, new_detail) }
}

/// `ULevel::Modify` (UnLevel.cpp:955). `DoTransArrays` is unused by the original body.
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_modify(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        s.uobject_modify(obj)?;
        s.model_modify((*lv(obj)).model)?;
    }
    Ok(())
}

/// `ULevel::PostLoad` (UnLevel.cpp:962).
///
/// # Safety
/// `obj` must be a live ULevel.
#[allow(clippy::result_unit_err)] // Err(()) means a native operation left an exception pending.
pub unsafe fn level_post_load(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        s.uobject_post_load(obj)?;

        // ENGINE_VERSION>230: retarget actors that share this level's outer.
        s.post_load_scope(obj)?;
        Ok(())
    }
}

pub(crate) trait LevelPostLoadOps {
    type Error;
    fn current_actor(&mut self) -> Result<Obj, Self::Error>;
    unsafe fn object_outer(&mut self, object: Obj) -> Result<Obj, Self::Error>;
    unsafe fn set_xlevel(&mut self, actor: Obj, level: Obj) -> Result<(), Self::Error>;
    fn advance_actor(&mut self) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn level_post_load_with<O: LevelPostLoadOps>(
    ops: &mut O, level: Obj,
) -> Result<(), O::Error> {
    loop {
        let actor = ops.current_actor()?;
        if actor.is_null() { break; }
        if unsafe { ops.object_outer(actor)? == ops.object_outer(level)? } {
            unsafe { ops.set_xlevel(actor, level)?; }
        }
        ops.advance_actor()?;
    }
    Ok(())
}

struct NativeLevelPostLoad<'a> {
    services: &'a Hp2LevelCoreServices,
    iterator: Obj,
}
impl LevelPostLoadOps for NativeLevelPostLoad<'_> {
    type Error = ();
    fn current_actor(&mut self) -> Result<Obj, ()> {
        unsafe { self.services.actor_iterator_get(self.iterator) }
    }
    unsafe fn object_outer(&mut self, object: Obj) -> Result<Obj, ()> {
        unsafe { self.services.object_get_outer(object) }
    }
    unsafe fn set_xlevel(&mut self, actor: Obj, level: Obj) -> Result<(), ()> {
        unsafe { self.services.actor_set_xlevel(actor, level) }
    }
    fn advance_actor(&mut self) -> Result<(), ()> {
        unsafe { self.services.actor_iterator_next(self.iterator) }
    }
}

pub unsafe fn level_post_load_body(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    iterator: Obj,
) -> Result<(), ()> {
    unsafe { level_post_load_with(&mut NativeLevelPostLoad { services: s, iterator }, obj) }
}

/// `ULevel::SetActorCollision` (UnLevel.cpp:973).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_set_actor_collision(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    collision: i32,
) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        // Init collision if first time through.
        if collision != 0 && (*level).hash.is_null() {
            s.collision_start_scope(obj)?;
        } else if !(*level).hash.is_null() && collision == 0 {
            s.collision_end_scope(obj)?;
        }
    }
    Ok(())
}

/// `ULevel::Serialize` (UnLevel.cpp:1008).
///
/// # Safety
/// `obj` must be a live ULevel; `ar` a live FArchive.
pub unsafe fn level_serialize(s: &Hp2LevelCoreServices, obj: Obj, ar: Obj) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        s.level_base_serialize(obj, ar)?;
        let mut approx_time = s.level_time_seconds_get_float(obj)?;
        s.archive_serialize_model(ar, ptr::addr_of_mut!((*level).model).cast())?;
        s.archive_serialize_reach_specs(ar, obj)?;
        s.archive_serialize_f32(ar, &mut approx_time)?;
        s.archive_serialize_actor_ptr(ar, ptr::addr_of_mut!((*level).first_deleted).cast())?;
        for i in 0..NUM_LEVEL_TEXT_BLOCKS {
            s.archive_serialize_text_buffer(ar, ptr::addr_of_mut!((*level).text_blocks[i]).cast())?;
        }
        if s.archive_ver(ar)? > 62 {
            s.archive_serialize_travel_info(ar, obj)?;
        } else if s.archive_ver(ar)? >= 61 {
            s.legacy_travel_scope(ar, obj)?;
        }
        if !(*level).model.is_null() && s.archive_is_trans(ar)? == 0 {
            s.archive_preload(ar, (*level).model)?;
        }
        if !(*level).brush_tracker.is_null() {
            s.brush_tracker_count_bytes((*level).brush_tracker, ar)?;
        }
    }
    Ok(())
}

/// `ULevel::Destroy` (UnLevel.cpp:1041).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_destroy(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        if !(*level).hash.is_null() {
            s.collision_hash_delete((*level).hash)?;
            (*level).hash = ptr::null_mut();
        }
        if !(*level).brush_tracker.is_null() {
            s.brush_tracker_delete((*level).brush_tracker)?;
            (*level).brush_tracker = ptr::null_mut();
        }
        s.level_base_destroy(obj)?;
    }
    Ok(())
}

/// `ULevel::RememberActors` (UnLevel.cpp:1074).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_remember_actors(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        if s.level_has_client(obj)? != 0 {
            let mut i = 0;
            while i < s.level_client_viewports_num(obj)? {
                let viewport = s.level_client_viewport(obj, i)?;
                s.viewport_remember_actor(viewport)?;
                i += 1;
            }
        }
    }
    Ok(())
}

/// `ULevel::ReconcileActors` (UnLevel.cpp:1099).
///
/// # Safety
/// `obj` must be a live ULevel with an engine client.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn level_reconcile_actors(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        if s.is_editor()? == 0 && s.check_failed(c"GIsEditor".as_ptr(), 1102) != 0 {
            return Err(());
        }

        // Dissociate all actor Viewports and remember their view properties.
        let mut i = 0;
        while i < s.actors_num(obj)? {
            let act = s.actors_get(obj, i)?;
            if !act.is_null()
                && s.actor_is_a_aplayer_pawn(act)? != 0
                && !s.pawn_get_player(act)?.is_null()
            {
                s.pawn_set_player(act, ptr::null_mut())?;
            }
            i += 1;
        }

        s.reconcile_identical_scope(obj)?;
        s.reconcile_other_scope(obj)?;
        s.reconcile_kill_scope(obj)?;
        Ok(())
    }
}

/// `ULevel::InitStats` (UnLevel.cpp:1615).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_init_stats(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        (*level).net_tick_cycles = 0;
        (*level).net_diff_cycles = 0;
        (*level).actor_tick_cycles = 0;
        (*level).audio_tick_cycles = 0;
        (*level).find_path_cycles = 0;
        (*level).move_cycles = 0;
        (*level).num_moves = 0;
        (*level).num_reps = 0;
        (*level).num_pv = 0;
        (*level).get_relevant_cycles = 0;
        (*level).num_rpc = 0;
        (*level).see_player = 0;
        (*level).spawning = 0;
        (*level).unused = 0;
        s.script_cycles_clear()?;
        s.script_entry_tag_clear()?;
    }
    Ok(())
}

/// `ULevel::GetStats` (UnLevel.cpp:1624). The format literal stays in C++.
///
/// # Safety
/// `obj` must be a live ULevel; `result` must point to a TCHAR buffer large enough for the format.
pub unsafe fn level_get_stats(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    result: *mut u32,
) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        // `GSecondsPerCycle*1000 * X` evaluates as `(GSecondsPerCycle*1000) * X`.
        let k = s.gseconds_per_cycle()? * 1000.0f32;
        let vals: [f32; 9] = [
            k * s.script_cycles_f32()?,
            k * ((*level).actor_tick_cycles as f32),
            k * ((*level).find_path_cycles as f32),
            k * ((*level).see_player as f32),
            k * ((*level).spawning as f32),
            k * ((*level).audio_tick_cycles as f32),
            k * ((*level).unused as f32),
            k * ((*level).move_cycles as f32),
            k * ((*level).net_tick_cycles as f32),
        ];
        s.app_sprintf_stats(result, vals.as_ptr(), (*level).num_moves)?;
    }
    Ok(())
}

/// `ULevel::UpdateTime` (UnLevel.cpp:1649).
///
/// # Safety
/// `info` must be a live ALevelInfo.
pub unsafe fn level_update_time(s: &Hp2LevelCoreServices, info: Obj) -> Result<(), ()> {
    unsafe { s.app_system_time_from_level_info(info)? }
    Ok(())
}

/// `ULevel::IsAudibleAt` (UnLevel.cpp:1658). `hit` is the C++-owned `Hit` local.
///
/// # Safety
/// `obj` must be a live ULevel; pointers must be valid for the call.
pub unsafe fn level_is_audible_at(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    sound_location: *const FVector,
    listener_location: *const FVector,
    sound_actor: Obj,
    sound_occlusion: i32,
    hit: *mut FCheckResult,
) -> Result<i32, ()> {
    unsafe {
        Ok(match sound_occlusion {
            OCCLUSION_STATIC_MESHES => s.level_single_line_check(
                obj,
                hit.cast(),
                sound_actor,
                sound_location.cast(),
                listener_location.cast(),
                TRACE_VIS_BLOCKING,
                ptr::from_ref(&ZERO).cast(),
                0,
            )?,
            OCCLUSION_DEFAULT | OCCLUSION_BSP => s.model_fast_line_check(
                (*lv(obj)).model,
                sound_location.cast(),
                listener_location.cast(),
            )?,
            // OCCLUSION_NONE (1) and any other value.
            _ => 1,
        })
    }
}

/// `ULevel::CalculateRadiusMultiplier` (UnLevel.cpp:1683).
///
/// # Safety
/// `obj` must be a live ULevel. `zone1`/`zone2` are unchecked, as in the original.
pub unsafe fn level_calculate_radius_multiplier(obj: Obj, zone1: i32, zone2: i32) -> f32 {
    unsafe {
        // ZoneDist is BYTE[64][64] (UnLevel.h:156); index without bounds checks.
        let base = ptr::addr_of!((*lv(obj)).zone_dist).cast::<u8>();
        let distance = i32::from(*base.offset(zone1 as isize * ZONE_DIST_DIM + zone2 as isize));
        // `appPow(ZONE_FACTOR, Distance * Distance)`: int product widened to double.
        ZONE_FACTOR.powf(f64::from(distance * distance)) as f32
    }
}

/// Returns `SourceActor` when it is an APlayerPawn, else null (the `Player` local).
///
/// # Safety
/// `source` must be a live AActor.
unsafe fn visible_player(s: &Hp2LevelCoreServices, source: Obj) -> Result<Obj, ()> {
    unsafe {
        Ok(if s.actor_is_a_aplayer_pawn(source)? != 0 {
            source
        } else {
            ptr::null_mut()
        })
    }
}

/// Searches the `FirstHit` chain for a visible hit (the inner `for` of TraceVisible).
/// `None` means the original `break` or loop exhaustion.
///
/// # Safety
/// `first_hit` must head a valid FCheckResult chain.
unsafe fn find_visible_check(
    s: &Hp2LevelCoreServices,
    source: Obj,
    player: Obj,
    first_hit: *mut FCheckResult,
) -> Result<Option<*mut FCheckResult>, ()> {
    unsafe {
        let mut check = first_hit;
        while !check.is_null() {
            let actor = (*check).actor;
            // Skip owned actors, but return the one nearest actor or level data.
            if source.is_null() || s.actor_is_owned_by(source, actor)? == 0 {
                if s.actor_is_a_alevel_info(actor)? != 0 {
                    // If we're in the rock (node 0), skip the test, then try again.
                    if (*check).item == 0 {
                        return Ok(None);
                    }
                    // Make sure the node we hit is in a visible zone.
                    let zone = s.model_node_izone1_of_actor(actor, (*check).item)?;
                    if player.is_null() || s.pawn_is_zone_visible(player, zone)? != 0 {
                        return Ok(Some(check));
                    }
                } else if !actor.is_null() {
                    // Make sure the actor we hit is in a visible zone.
                    let zone = (*actor.cast::<AActor>()).region.zone_number;
                    if player.is_null() || s.pawn_is_zone_visible(player, zone)? != 0 {
                        return Ok(Some(check));
                    }
                }
            }
            check = (*check).next.cast();
        }
        Ok(None)
    }
}

/// `ULevel::TraceVisible` (UnLevel.cpp:1697). `first_hit` is the C++-owned `FirstHit` local.
///
/// # Safety
/// `obj` must be a live ULevel; all pointers must be valid; `first_hit` is written by the trace.
#[allow(clippy::too_many_arguments)] // Mirrors the C++ signature (UnLevel.cpp:1697).
pub unsafe fn level_trace_visible(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    trace_direction: *const FVector,
    hit: *mut FCheckResult,
    source: Obj,
    start: *const FVector,
    trace_flags: u32,
    distance: i32,
    first_hit: *mut FCheckResult,
) -> Result<(), ()> {
    unsafe {
        let player = visible_player(s, source)?;

        // Trace the entire distance looking for a "selected" zone.
        let mut start_trace = *start;
        let end = vec_add(*start, vec_scale(distance as f32, *trace_direction));
        while (fdist(*start, start_trace) as i32) < distance {
            // Get list of hit actors.
            if s.level_single_line_check(
                obj,
                first_hit.cast(),
                source,
                ptr::from_ref(&end).cast(),
                ptr::from_ref(&start_trace).cast(),
                trace_flags,
                ptr::from_ref(&ZERO).cast(),
                0,
            )? != 0
            {
                break;
            }
            if let Some(check) = find_visible_check(s, source, player, first_hit)? {
                s.check_result_assign(hit.cast(), check.cast_const().cast())?;
                return Ok(());
            }
            // Found a room, but it's the wrong one: move forward one foot, then try again.
            start_trace = vec_add((*first_hit).location, vec_scale(16.0, *trace_direction));
        }

        // Missed section.
        (*hit).time = 1.0;
        (*hit).actor = ptr::null_mut();
    }
    Ok(())
}

/// `ULevel::TraceVisibleObjects` (UnLevel.cpp:1773). `first_hit` is the C++-owned `FirstHit` local.
///
/// # Safety
/// `obj` must be a live ULevel; all pointers must be valid; `first_hit` is written by the trace.
#[allow(clippy::too_many_arguments)] // Mirrors the C++ signature (UnLevel.cpp:1773).
pub unsafe fn level_trace_visible_objects(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    parent_class: Obj,
    trace_direction: *const FVector,
    hit: *mut FCheckResult,
    source: Obj,
    start: *const FVector,
    trace_flags: u32,
    distance: i32,
    first_hit: *mut FCheckResult,
) -> Result<(), ()> {
    unsafe {
        let player = visible_player(s, source)?;

        // Trace the entire distance looking for a matching object in the "selected" zone.
        let mut start_trace = *start;
        let end = vec_add(*start, vec_scale(distance as f32, *trace_direction));
        while fdist(*start, start_trace) < distance as f32 {
            // Get list of hit actors.
            if s.level_single_line_check(
                obj,
                first_hit.cast(),
                source,
                ptr::from_ref(&end).cast(),
                ptr::from_ref(&start_trace).cast(),
                trace_flags,
                ptr::from_ref(&ZERO).cast(),
                0,
            )? != 0
            {
                break;
            }
            // Skip owned actors, but return the nearest actor matching the ParentClass.
            let mut check = first_hit;
            while !check.is_null() {
                let actor = (*check).actor;
                if (source.is_null() || s.actor_is_owned_by(source, actor)? == 0)
                    && s.actor_class_is_child_of(actor, parent_class)? != 0
                {
                    // Make sure the actor we hit is in a visible zone.
                    let zone = (*actor.cast::<AActor>()).region.zone_number;
                    if player.is_null() || s.pawn_is_zone_visible(player, zone)? != 0 {
                        s.check_result_assign(hit.cast(), check.cast_const().cast())?;
                        return Ok(());
                    }
                }
                check = (*check).next.cast();
            }
            // Found a room, but it's the wrong one: move forward 1 foot, then try again.
            start_trace = vec_add((*first_hit).location, vec_scale(16.0, *trace_direction));
        }

        // Missed section.
        (*hit).time = 1.0;
        (*hit).actor = ptr::null_mut();
    }
    Ok(())
}

pub unsafe fn level_collision_start(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        (*level).hash = s.collision_hash_new()?;
        for_colliding_actors(
            || s.actors_num(obj),
            |index| s.actors_get(obj, index),
            |actor| s.actor_collide_actors(actor).map(|value| value != 0),
            |actor| s.collision_hash_add_actor((*level).hash, actor),
        )?;
    }
    Ok(())
}

pub unsafe fn level_collision_end(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        let level = lv(obj);
        // Destroy hash.
        for_colliding_actors(
            || s.actors_num(obj),
            |index| s.actors_get(obj, index),
            |actor| s.actor_collide_actors(actor).map(|value| value != 0),
            |actor| s.collision_hash_remove_actor((*level).hash, actor),
        )?;
        s.collision_hash_delete((*level).hash)?;
        (*level).hash = ptr::null_mut();
    }
    Ok(())
}

/// Shared collision-scope actor order, re-reading the live array length each iteration.
pub(crate) fn for_colliding_actors<E>(
    mut count: impl FnMut() -> Result<i32, E>,
    mut actor_at: impl FnMut(i32) -> Result<Obj, E>,
    mut collides: impl FnMut(Obj) -> Result<bool, E>,
    mut visit: impl FnMut(Obj) -> Result<(), E>,
) -> Result<(), E> {
    let mut index = 0;
    while index < count()? {
        let actor = actor_at(index)?;
        if !actor.is_null() && collides(actor)? {
            visit(actor)?;
        }
        index += 1;
    }
    Ok(())
}

pub unsafe fn level_reconcile_identical(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        // Match Viewports and Viewport-actors with identical names.
        let mut i = 0;
        while i < s.level_client_viewports_num(obj)? {
            let viewport = s.level_client_viewport(obj, i)?;
            if !s.viewport_get_actor(viewport)?.is_null()
                && s.check_failed(c"Viewport->Actor==NULL".as_ptr(), 1116) != 0
            {
                return Err(());
            }
            let mut j = 0;
            while j < s.actors_num(obj)? {
                let act = s.actors_get(obj, j)?;
                if !act.is_null()
                    && s.actor_is_a_camera(act)? != 0
                    && s.actor_tag_stricmp_viewport_name(act, viewport)? == 0
                {
                    s.debug_matched_viewport(viewport)?;
                    s.viewport_match_actor(viewport, act)?;
                    break;
                }
                j += 1;
            }
            i += 1;
        }
    }
    Ok(())
}

pub unsafe fn level_reconcile_other(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        // Match up all remaining Viewports to actors.
        let mut i = 0;
        while i < s.level_client_viewports_num(obj)? {
            let viewport = s.level_client_viewport(obj, i)?;
            if s.viewport_get_actor(viewport)?.is_null() {
                s.level_spawn_view_actor(obj, viewport)?;
            }
            i += 1;
        }
    }
    Ok(())
}

pub unsafe fn level_reconcile_kill(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        // Handle remaining unassociated view actors.
        let mut i = 0;
        while i < s.actors_num(obj)? {
            let view = s.actor_cast_camera(s.actors_get(obj, i)?)?;
            if !view.is_null() {
                let viewport = s.camera_player_viewport(view)?;
                if !viewport.is_null() {
                    s.uobject_clear_flags(view, RF_TRANSACTIONAL)?;
                    s.camera_restore_view(view, viewport)?;
                } else {
                    s.level_destroy_actor(obj, view)?;
                }
            }
            i += 1;
        }
    }
    Ok(())
}

pub unsafe fn level_legacy_travel(
    s: &Hp2LevelCoreServices,
    ar: Obj,
    obj: Obj,
    names: Obj,
    items: Obj,
) -> Result<(), ()> {
    unsafe {
        s.archive_serialize_strings(ar, names)?;
        s.archive_serialize_strings(ar, items)?;
        s.travel_info_reset(obj)?;
        let mut i = 0;
        while i < s.strings_num(names)? {
            s.travel_info_set(obj, names, items, i)?;
            i += 1;
        }
    }
    Ok(())
}

pub unsafe fn level_screen_actors_by_game_state(
    s: &Hp2LevelCoreServices,
    obj: Obj,
) -> Result<(), ()> {
    unsafe {
        s.screen_scope(obj)?;
    }
    Ok(())
}
