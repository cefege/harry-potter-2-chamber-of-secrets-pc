//! Shared no-op `Hp2ActorServices` stub for consumer tests whose fixtures
//! never exercise a leaf (`GetCylinderExtent`'s mesh/brush branch, `SetBase`'s
//! notify events): every field writes zero/null to its out-param, if any, and
//! returns success. Published once per test binary, like
//! `level_spawn_actor::mock::publish`.

use std::ffi::{c_char, c_void};
use std::sync::Once;

use hp2_engine::actor::{self, Hp2ActorServices};

unsafe extern "C" fn stub_0(_a0: *mut c_void, _a1: *mut i32) -> i32 {
    unsafe {
        _a1.write(0);
    }
    0
}

unsafe extern "C" fn stub_1(_a0: *mut c_void, _a1: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn stub_2(_a0: *const c_void, _a1: u32, _a2: *mut i32) -> i32 {
    unsafe {
        _a2.write(0);
    }
    0
}

unsafe extern "C" fn stub_3(
    _a0: *mut c_void,
    _a1: *mut c_void,
    _a2: *const c_void,
    _a3: i32,
    _a4: i32,
    _a5: *mut i32,
) -> i32 {
    unsafe {
        _a5.write(0);
    }
    0
}

unsafe extern "C" fn stub_4(_a0: *mut c_void, _a1: *mut c_void, _a2: *const c_void) -> i32 {
    0
}

unsafe extern "C" fn stub_5(_a0: *mut c_void, _a1: *const c_void, _a2: *mut i32) -> i32 {
    unsafe {
        _a2.write(0);
    }
    0
}

unsafe extern "C" fn stub_6(_a0: *mut c_void, _a1: u8) -> i32 {
    0
}

unsafe extern "C" fn stub_7(_a0: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn stub_8(_a0: *mut c_void, _a1: *mut *mut c_void) -> i32 {
    unsafe {
        _a1.write(std::ptr::null_mut());
    }
    0
}

unsafe extern "C" fn stub_10(_a0: *const c_char, _a1: i32) -> i32 {
    0
}

unsafe extern "C" fn stub_11(_a0: *mut i32) -> i32 {
    unsafe {
        _a0.write(0);
    }
    0
}

unsafe extern "C" fn stub_12() -> i32 {
    0
}

unsafe extern "C" fn stub_14(
    _a0: *mut c_void,
    _a1: *mut c_void,
    _a2: *mut c_void,
    _a3: *mut c_void,
) -> i32 {
    0
}

unsafe extern "C" fn stub_15(_a0: *mut c_void, _a1: *mut c_void, _a2: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn stub_16(
    _a0: *mut c_void,
    _a1: *mut c_void,
    _a2: i32,
    _a3: *mut c_void,
) -> i32 {
    0
}

unsafe extern "C" fn stub_18(_a0: *mut c_void, _a1: usize) -> i32 {
    0
}

unsafe extern "C" fn stub_19(
    _a0: *mut c_void,
    _a1: *const c_void,
    _a2: *const c_void,
    _a3: *mut c_void,
    _a4: *mut *mut c_void,
) -> i32 {
    unsafe {
        _a4.write(std::ptr::null_mut());
    }
    0
}

unsafe extern "C" fn stub_21(_a0: *mut c_void, _a1: i32, _a2: *mut *const c_void) -> i32 {
    unsafe {
        _a2.write(std::ptr::null());
    }
    0
}

unsafe extern "C" fn stub_22(_a0: *mut c_void, _a1: i32, _a2: *mut *mut c_void) -> i32 {
    unsafe {
        _a2.write(std::ptr::null_mut());
    }
    0
}

unsafe extern "C" fn stub_24(_a0: *mut c_void, _a1: *mut c_void, _a2: i32) -> i32 {
    0
}

unsafe extern "C" fn stub_25(_a0: *mut c_void, _a1: *mut c_void, _a2: *mut i32) -> i32 {
    unsafe {
        _a2.write(0);
    }
    0
}

unsafe extern "C" fn stub_26(_a0: *mut c_void, _a1: f32) -> i32 {
    0
}

/// Publish once per test binary; safe to call from every test (idempotent).
pub fn publish() {
    static PUBLISHED: Once = Once::new();
    PUBLISHED.call_once(|| {
        let table = build_table(stub_15, stub_7);
        // SAFETY: published once, before any reader in this process.
        unsafe { actor::publish_services(table) };
    });
}

/// Builds the shared 72-field `Hp2ActorServices` table; `mesh_extent` and
/// `base_change` are the only two fields that vary across the three
/// `publish*` variants below (each passes its own override, or the no-op
/// stub when it doesn't care).
fn build_table(
    mesh_extent: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    base_change: unsafe extern "C" fn(*mut c_void) -> i32,
) -> Hp2ActorServices {
    Hp2ActorServices {
        level_hash_add_actor: stub_1,
        level_hash_remove_actor: stub_1,
        actor_is_a: stub_2,
        level_far_move_actor: stub_3,
        level_move_actor_to_rotation: stub_4,
        actor_move_smooth: stub_5,
        actor_set_physics: stub_6,
        actor_event_bump: stub_1,
        actor_event_detach: stub_1,
        actor_event_attach: stub_1,
        actor_event_base_change: base_change,
        actor_event_lost_child: stub_1,
        actor_event_gained_child: stub_1,
        level_viewport0_actor_pri: stub_8,
        check_failed: stub_10,
        actor_event_touch: stub_1,
        actor_event_untouch: stub_1,
        is_editor: stub_11,
        gcache_flush: stub_12,
        primitive_validate_actor: stub_1,
        uobject_process_event: stub_14,
        uobject_destroy: stub_7,
        render_interface_destroy: stub_7,
        object_conditional_destroy: stub_7,
        actor_load_localized: stub_7,
        uobject_post_load: stub_7,
        actor_event_possess: stub_7,
        actor_log_possessed: stub_7,
        mesh_collision_extent: mesh_extent,
        brush_collision_extent: stub_15,
        mesh_render_bounding_box: stub_16,
        brush_render_bounding_box_coords: stub_16,
        class_is_uclass_static: stub_0,
        archive_preload: stub_1,
        object_serialize_tagged_persistent: stub_1,
        object_serialize_bin: stub_1,
        archive_count_bytes: stub_18,
        trace_line_texture_hit: stub_19,
        bsp_surf_decal_count: stub_0,
        model_vector: stub_21,
        model_point: stub_21,
        bsp_surf_decal: stub_22,
        actor_get_primitive: stub_8,
        object_is_valid: stub_0,
        actor_end_touch: stub_24,
        actor_touch_to: stub_25,
        render_interface_remove_root: stub_7,
        actor_perform_physics: stub_26,
    }
}

/// Like [`publish`], but routes `GetCylinderExtent`'s mesh-shape branch through
/// `mesh_extent` instead of the no-op stub, for fixtures that recorded
/// `GetCylinderExtent`'s result directly (the brush branch is unreachable once
/// every actor carries a non-null mesh, so it stays a no-op).
pub fn publish_with_mesh_collision_extent(
    mesh_extent: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
) {
    static PUBLISHED: Once = Once::new();
    PUBLISHED.call_once(|| {
        let table = build_table(mesh_extent, stub_7);
        // SAFETY: published once, before any reader in this process.
        unsafe { actor::publish_services(table) };
    });
}

/// Like [`publish_with_mesh_collision_extent`], but additionally routes
/// `SetBase`'s notify event through `base_change` instead of the no-op stub,
/// for fixtures that recorded every `SetBase` call's argument (the harness's
/// `SetBase` override is a pure log, with no base-chain/standing-count
/// bookkeeping, so `base_change` is expected to reset `Base` back to null
/// after each call to keep every call's internal mutation-guard open).
pub fn publish_with_mesh_and_base_change(
    mesh_extent: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    base_change: unsafe extern "C" fn(*mut c_void) -> i32,
) {
    static PUBLISHED: Once = Once::new();
    PUBLISHED.call_once(|| {
        let table = build_table(mesh_extent, base_change);
        // SAFETY: published once, before any reader in this process.
        unsafe { actor::publish_services(table) };
    });
}
