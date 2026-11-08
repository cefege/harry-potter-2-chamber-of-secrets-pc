//! Mock `Hp2GameSaveServices` for the `UGameEngine::SaveGame` replay (`game_engine_save.rs`).
//!
//! Each SaveGame service emits the record the matching harness hook prints. Harness
//! globals that only SaveGame reads (`g_save_pkg_ret`, `g_save_exist`, the hub stack
//! level, the brush tracker, the level's actor list) live in `SV`; `wire` restores them
//! per case. Fields SaveGame never reaches are panicking shims.

use std::cell::RefCell;
use std::ffi::{c_char, c_void};

use hp2_engine::game_save::Hp2GameSaveServices;

use crate::mock::*;

/// Save-only harness state.
#[derive(Default)]
struct Sv {
    pkg_ret: i32,
    exist: i32,
    hub: i32,
    brush: bool,
    actors: Vec<usize>,
}

thread_local! {
    static SV: RefCell<Sv> = RefCell::new(Sv::default());
}

fn sv<R>(f: impl FnOnce(&mut Sv) -> R) -> R {
    SV.with(|s| f(&mut s.borrow_mut()))
}

/// One harness `save_case` setup.
pub struct Wiring {
    pub pkg_ret: i32,
    pub exist: i32,
    pub hub: i32,
    pub brush: bool,
    pub mover: bool,
}

/// Harness `save_case` setup: `world_base(false)`, then the knobs. `Actors` holds
/// `LevelInfo` first, then the pawn when it is a mover.
pub fn wire(w: Wiring) {
    world_base(false);
    sv(|s| {
        *s = Sv {
            pkg_ret: w.pkg_ret,
            exist: w.exist,
            hub: w.hub,
            brush: w.brush,
            actors: vec![LEVELINFO],
        };
        if w.mover {
            s.actors.push(PAWN);
        }
    });
}

/// Writes `s` plus a NUL terminator into a TCHAR buffer.
///
/// # Safety
/// `dst` must be writable for `s.len() + 1` code units.
unsafe fn write_tchar(dst: *mut u32, s: &str) {
    for (i, b) in s.bytes().enumerate() {
        // SAFETY: caller contract.
        unsafe { *dst.add(i) = b as u32 };
    }
    // SAFETY: caller contract.
    unsafe { *dst.add(s.len()) = 0 };
}

unsafe extern "C" fn sv_paint_progress(engine: *mut c_void) -> i32 {
    check_eng(engine);
    emit(&format!("paint_progress {}", nm(addr(engine))));
    0
}

unsafe extern "C" fn sv_make_dir_save_path() -> i32 {
    emit("make_directory \"SavePath\"");
    0
}

unsafe extern "C" fn sv_make_dir_save_slot_path() -> i32 {
    emit("make_directory \"SaveSlotPath\"");
    0
}

unsafe extern "C" fn sv_format_slot_file(out: *mut u32, position: i32) -> i32 {
    let name = format!("SaveSlot/Save{position}.usa");
    emit(&format!("format_slot_file {position} {}", pfs(&tc(&name))));
    // SAFETY: `SaveGame` passes a `TCHAR[256]` buffer.
    unsafe { write_tchar(out, &name) };
    0
}

unsafe extern "C" fn sv_format_game_file(out: *mut u32, index: i32) -> i32 {
    let name = format!("SaveSlot/Game{index}.usa");
    emit(&format!("format_game_file {index} {}", pfs(&tc(&name))));
    // SAFETY: `SaveGame` passes a `TCHAR[256]` buffer.
    unsafe { write_tchar(out, &name) };
    0
}

unsafe extern "C" fn sv_format_save_hub_file(out: *mut u32, position: i32, index: i32) -> i32 {
    let name = format!("SaveSlot/Save{position}{index}.usa");
    emit(&format!(
        "format_save_hub_file {position} {index} {}",
        pfs(&tc(&name))
    ));
    // SAFETY: `SaveGame` passes a `TCHAR[256]` buffer.
    unsafe { write_tchar(out, &name) };
    0
}

unsafe extern "C" fn sv_debugf_saving(filename: *const u32) -> i32 {
    emit(&format!(
        "debugf_str {} {}",
        pfs(&tc("Saving game... filename: %s")),
        pfs(&read_tchar(filename))
    ));
    0
}

unsafe extern "C" fn sv_level_info_set_action(engine: *mut c_void, action: i32) -> i32 {
    check_eng(engine);
    emit(&format!("level_info_set_action {action}"));
    0
}

unsafe extern "C" fn sv_level_info_hub_stack_level(engine: *mut c_void, result: *mut i32) -> i32 {
    let value = (|| {
        check_eng(engine);
        sv(|s| s.hub)
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_begin_slow_task_saving() -> i32 {
    // `LocalizeProgress("Saving")` runs inside the same adapter, then `BeginSlowTask`.
    emit("localize_progress Saving");
    emit(&format!("begin_slow_task {}", pfs(&tc("Localized"))));
    0
}

unsafe extern "C" fn sv_end_slow_task() -> i32 {
    emit("end_slow_task");
    0
}

unsafe extern "C" fn sv_level_has_brush_tracker(engine: *mut c_void, result: *mut i32) -> i32 {
    let value = (|| {
        check_eng(engine);
        sv(|s| s.brush as i32)
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_level_delete_brush_tracker(engine: *mut c_void) -> i32 {
    check_eng(engine);
    emit(&format!("delete_brush_tracker {}", nm(LEVEL)));
    sv(|s| s.brush = false);
    0
}

unsafe extern "C" fn sv_level_cleanup_destroyed(engine: *mut c_void) -> i32 {
    check_eng(engine);
    emit(&format!("cleanup_destroyed {} 1", nm(LEVEL)));
    0
}

unsafe extern "C" fn sv_level_save_package(
    engine: *mut c_void,
    filename: *const u32,
    result: *mut i32,
) -> i32 {
    let value = (|| {
        check_eng(engine);
        let ret = sv(|s| s.pkg_ret);
        emit(&format!(
            "save_package {} {} ret={ret}",
            nm(LEVEL),
            pfs(&read_tchar(filename))
        ));
        ret
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_file_copy(first: *const u32, second: *const u32) -> i32 {
    emit(&format!(
        "file_copy {} {}",
        pfs(&read_tchar(first)),
        pfs(&read_tchar(second))
    ));
    0
}

unsafe extern "C" fn sv_file_size(filename: *const u32, result: *mut i32) -> i32 {
    let value = (|| {
        let name = read_tchar(filename);
        let ret = sv(|s| {
            if s.exist > 0 {
                s.exist -= 1;
                16
            } else {
                0
            }
        });
        emit(&format!("file_size {} ret={ret}", pfs(&name)));
        ret
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_file_delete(filename: *const u32) -> i32 {
    emit(&format!("file_delete {}", pfs(&read_tchar(filename))));
    0
}

unsafe extern "C" fn sv_level_actor_count(engine: *mut c_void, result: *mut i32) -> i32 {
    let value = (|| {
        check_eng(engine);
        sv(|s| s.actors.len() as i32)
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_level_actor(
    engine: *mut c_void,
    index: i32,
    result: *mut *mut c_void,
) -> i32 {
    let value = (|| {
        check_eng(engine);
        ptr(sv(|s| s.actors[index as usize]))
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_actor_is_mover(actor: *mut c_void, result: *mut i32) -> i32 {
    let value = (|| {
        let a = addr(actor);
        let ret = i32::from(a == PAWN);
        emit(&format!("actor_is_mover {} ret={ret}", nm(a)));
        ret
    })();
    unsafe {
        *result = value;
    }
    0
}

unsafe extern "C" fn sv_mover_reset_saved_pos(actor: *mut c_void) -> i32 {
    emit(&format!("mover_reset_saved_pos {}", nm(addr(actor))));
    0
}

unsafe extern "C" fn sv_level_new_brush_tracker(engine: *mut c_void) -> i32 {
    check_eng(engine);
    emit(&format!("new_brush_tracker {}", nm(LEVEL)));
    sv(|s| s.brush = true);
    0
}

unsafe extern "C" fn sv_gcache_flush() -> i32 {
    emit("gcache_flush");
    0
}

unsafe extern "C" fn sv_save_persistent_actor_cache() -> i32 {
    emit("save_persistent_actor_cache");
    0
}

// Unused callbacks retain the existing replay failure behavior.
#[allow(unused_variables)]
unsafe extern "C" fn unused_check_failed(expr: *const c_char, line: i32) -> i32 {
    panic!("unused_check_failed is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_debugf_export_class(actor: *mut c_void) -> i32 {
    panic!("unused_debugf_export_class is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_actor_b_travel(actor: *mut c_void, result: *mut i32) -> i32 {
    panic!("unused_actor_b_travel is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_logf_header(out: *mut c_void, actor: *mut c_void) -> i32 {
    panic!("unused_out_logf_header is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_prop_iter_valid(it: *mut c_void, result: *mut i32) -> i32 {
    panic!("unused_prop_iter_valid is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_prop_iter_next(it: *mut c_void) -> i32 {
    panic!("unused_prop_iter_next is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_prop_array_dim(it: *mut c_void, result: *mut i32) -> i32 {
    panic!("unused_prop_array_dim is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_prop_travel_flag(it: *mut c_void, result: *mut i32) -> i32 {
    panic!("unused_prop_travel_flag is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_prop_export_text(
    it: *mut c_void,
    index: i32,
    value: *mut u32,
    actor: *mut c_void,
    result: *mut i32,
) -> i32 {
    panic!("unused_prop_export_text is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_log_prop_name(out: *mut c_void, it: *mut c_void) -> i32 {
    panic!("unused_out_log_prop_name is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_logf_index(out: *mut c_void, index: i32) -> i32 {
    panic!("unused_out_logf_index is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_log_equals(out: *mut c_void) -> i32 {
    panic!("unused_out_log_equals is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_prop_is_actor_ref(it: *mut c_void, result: *mut i32) -> i32 {
    panic!("unused_prop_is_actor_ref is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_logf_object_ref(
    out: *mut c_void,
    it: *mut c_void,
    actor: *mut c_void,
    index: i32,
) -> i32 {
    panic!("unused_out_logf_object_ref is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_logf_value(out: *mut c_void, value: *const u32) -> i32 {
    panic!("unused_out_logf_value is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_out_logf_footer(out: *mut c_void) -> i32 {
    panic!("unused_out_logf_footer is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_begin_load() -> i32 {
    panic!("unused_begin_load is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_end_load() -> i32 {
    panic!("unused_end_load is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_level_linker(level: *mut c_void, result: *mut *mut c_void) -> i32 {
    panic!("unused_level_linker is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_level_level_info(level: *mut c_void, result: *mut *mut c_void) -> i32 {
    panic!("unused_level_level_info is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_level_info_game(info: *mut c_void, result: *mut *mut c_void) -> i32 {
    panic!("unused_level_info_game is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_game_class_linker(game: *mut c_void, result: *mut *mut c_void) -> i32 {
    panic!("unused_game_class_linker is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_master_map_add_linker(
    net_driver: *mut c_void,
    linker: *mut c_void,
    result: *mut i32,
) -> i32 {
    panic!("unused_master_map_add_linker is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_master_map_compute(net_driver: *mut c_void) -> i32 {
    panic!("unused_master_map_compute is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_server_packages_count(engine: *mut c_void, result: *mut i32) -> i32 {
    panic!("unused_server_packages_count is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_server_package_debugf(engine: *mut c_void, index: i32) -> i32 {
    panic!("unused_server_package_debugf is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_server_package_linker(
    engine: *mut c_void,
    index: i32,
    result: *mut *mut c_void,
) -> i32 {
    panic!("unused_server_package_linker is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_debugf_server_side_only() -> i32 {
    panic!("unused_debugf_server_side_only is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_strnicmp(
    a: *const u32,
    b: *const u32,
    count: i32,
    result: *mut i32,
) -> i32 {
    panic!("unused_strnicmp is not reached by SaveGame");
}
#[allow(unused_variables)]
unsafe extern "C" fn unused_with_properties(out: *mut c_void, actor: *mut c_void) -> i32 {
    panic!("unused_with_properties is not reached by SaveGame");
}

/// The full Save service table, wired to the shared mock objects.
pub fn services() -> Hp2GameSaveServices {
    Hp2GameSaveServices {
        check_failed: unused_check_failed,
        paint_progress: sv_paint_progress,
        make_dir_save_path: sv_make_dir_save_path,
        make_dir_save_slot_path: sv_make_dir_save_slot_path,
        format_slot_file: sv_format_slot_file,
        format_game_file: sv_format_game_file,
        format_save_hub_file: sv_format_save_hub_file,
        debugf_saving: sv_debugf_saving,
        level_info_set_action: sv_level_info_set_action,
        level_info_hub_stack_level: sv_level_info_hub_stack_level,
        begin_slow_task_saving: sv_begin_slow_task_saving,
        end_slow_task: sv_end_slow_task,
        level_has_brush_tracker: sv_level_has_brush_tracker,
        level_delete_brush_tracker: sv_level_delete_brush_tracker,
        level_cleanup_destroyed: sv_level_cleanup_destroyed,
        level_save_package: sv_level_save_package,
        file_copy: sv_file_copy,
        file_size: sv_file_size,
        file_delete: sv_file_delete,
        level_actor_count: sv_level_actor_count,
        level_actor: sv_level_actor,
        actor_is_mover: sv_actor_is_mover,
        mover_reset_saved_pos: sv_mover_reset_saved_pos,
        level_new_brush_tracker: sv_level_new_brush_tracker,
        gcache_flush: sv_gcache_flush,
        save_persistent_actor_cache: sv_save_persistent_actor_cache,
        debugf_export_class: unused_debugf_export_class,
        actor_b_travel: unused_actor_b_travel,
        out_logf_header: unused_out_logf_header,
        prop_iter_valid: unused_prop_iter_valid,
        prop_iter_next: unused_prop_iter_next,
        prop_array_dim: unused_prop_array_dim,
        prop_travel_flag: unused_prop_travel_flag,
        prop_export_text: unused_prop_export_text,
        out_log_prop_name: unused_out_log_prop_name,
        out_logf_index: unused_out_logf_index,
        out_log_equals: unused_out_log_equals,
        prop_is_actor_ref: unused_prop_is_actor_ref,
        out_logf_object_ref: unused_out_logf_object_ref,
        out_logf_value: unused_out_logf_value,
        out_logf_footer: unused_out_logf_footer,
        begin_load: unused_begin_load,
        end_load: unused_end_load,
        level_linker: unused_level_linker,
        level_level_info: unused_level_level_info,
        level_info_game: unused_level_info_game,
        game_class_linker: unused_game_class_linker,
        master_map_add_linker: unused_master_map_add_linker,
        master_map_compute: unused_master_map_compute,
        server_packages_count: unused_server_packages_count,
        server_package_debugf: unused_server_package_debugf,
        server_package_linker: unused_server_package_linker,
        debugf_server_side_only: unused_debugf_server_side_only,
        strnicmp: unused_strnicmp,
        with_properties: unused_with_properties,
    }
}
