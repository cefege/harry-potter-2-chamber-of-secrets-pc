//! File, URL, skin, map and console natives of `Engine/Src/UnPawn.cpp`
//! (`crates/hp2-engine/src/pawn_native_files.rs`). The C++ shells keep the
//! thunk plumbing and call these exports for the decision logic and the math.
//! Engine calls go through the `Hp2PawnNativeFilesServices` table passed on
//! every call. Text is NUL-terminated UNICHAR. String results are copied by C++
//! before the next string-returning call.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_engine::pawn_native_files::{self, Hp2PawnNativeFilesServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnNativeFilesServices`.
unsafe fn svc_of<'a>(
    p: *const Hp2PawnNativeFilesServices,
) -> Option<&'a Hp2PawnNativeFilesServices> {
    unsafe { p.as_ref() }
}

/// `AActor::execGetNextInt` / `execGetNextIntDesc` index into the registry list.
/// Returns the index, or `-1` for the empty result.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_registry_index(count: i32, current: i32) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_files_registry_index",
        -1,
        move || pawn_native_files::registry_index(count, current),
    )
}

/// `AActor::execGetCacheEntry`: `empty[i] != 0` marks the `i`-th `Cache` entry
/// whose value is empty. Returns the `num`-th non-empty index, or `-1`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_cache_entry_index(
    num: i32,
    empty: *const u8,
    count: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_files_cache_entry_index",
        -1,
        move || unsafe {
            if empty.is_null() || count <= 0 {
                return -1;
            }
            let flags: Vec<bool> = std::slice::from_raw_parts(empty, count as usize)
                .iter()
                .map(|&b| b != 0)
                .collect();
            pawn_native_files::cache_entry_index(num, &flags)
        },
    )
}

/// `AActor::execGetNextSkin`: picks the next texture name from the registry
/// snapshot `objects[0..count]`. Returns the index, or `-1` for the empty result.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_next_skin_index(
    objects: *const *const u16,
    count: i32,
    prefix: *const u16,
    current: *const u16,
    dir: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_files_next_skin_index",
        -1,
        move || unsafe {
            let names = pawn_native_files::object_names(objects, count);
            let prefix = pawn_native_files::units(prefix);
            let current = pawn_native_files::units(current);
            match pawn_native_files::next_skin_index(&names, &prefix, &current, dir) {
                Some(i) => i as i32,
                None => -1,
            }
        },
    )
}

/// `APawn::execCheckValidSkinPackage`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_check_valid_skin_package(
    svc: *const Hp2PawnNativeFilesServices,
    pkg: *const u16,
    mesh: *const u16,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_files_check_valid_skin_package",
        0,
        move || unsafe {
            match svc_of(svc) {
                Some(s) => {
                    let pkg = pawn_native_files::units(pkg);
                    let mesh = pawn_native_files::units(mesh);
                    i32::from(pawn_native_files::check_valid_skin_package(s, &pkg, &mesh))
                }
                None => 0,
            }
        },
    )
}

/// `APlayerPawn::execUpdateURL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_update_url(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
    option: *const u16,
    value: *const u16,
    save: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_files_update_url",
        (),
        move || unsafe {
            if let Some(s) = svc_of(svc) {
                let option = pawn_native_files::units(option);
                let value = pawn_native_files::units(value);
                pawn_native_files::update_url(s, this, &option, &value, save != 0);
            }
        },
    )
}

/// `APlayerPawn::execGetDefaultURL`. Returns a buffer valid until the next
/// string-returning call on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_get_default_url(
    svc: *const Hp2PawnNativeFilesServices,
    option: *const u16,
) -> *const u16 {
    guard(
        "hp2_rs_engine_pawn_native_files_get_default_url",
        std::ptr::null(),
        move || unsafe {
            match svc_of(svc) {
                Some(s) => {
                    let option = pawn_native_files::units(option);
                    pawn_native_files::publish_units(&pawn_native_files::get_default_url(
                        s, &option,
                    ))
                }
                None => std::ptr::null(),
            }
        },
    )
}

/// `APlayerPawn::execMoveCacheEntry`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_move_cache_entry(
    svc: *const Hp2PawnNativeFilesServices,
    guid: *const u16,
    new_filename: *const u16,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_files_move_cache_entry",
        0,
        move || unsafe {
            match svc_of(svc) {
                Some(s) => {
                    let guid = pawn_native_files::units(guid);
                    let new_filename = pawn_native_files::units(new_filename);
                    i32::from(pawn_native_files::move_cache_entry(s, &guid, &new_filename))
                }
                None => 0,
            }
        },
    )
}

/// `AActor::execGetMapName`. Returns a buffer valid until the next
/// string-returning call on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_get_map_name(
    svc: *const Hp2PawnNativeFilesServices,
    prefix: *const u16,
    map_name: *const u16,
    dir: i32,
) -> *const u16 {
    guard(
        "hp2_rs_engine_pawn_native_files_get_map_name",
        std::ptr::null(),
        move || unsafe {
            match svc_of(svc) {
                Some(s) => {
                    let prefix = pawn_native_files::units(prefix);
                    let map = pawn_native_files::units(map_name);
                    pawn_native_files::publish_units(&pawn_native_files::get_map_name(
                        s, &prefix, &map, dir,
                    ))
                }
                None => std::ptr::null(),
            }
        },
    )
}

/// `APlayerPawn::execConsoleCommand`. Returns a buffer valid until the next
/// string-returning call on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_console_command(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
    command: *const u16,
) -> *const u16 {
    guard(
        "hp2_rs_engine_pawn_native_files_console_command",
        std::ptr::null(),
        move || unsafe {
            match svc_of(svc) {
                Some(s) => {
                    let command = pawn_native_files::units(command);
                    pawn_native_files::publish_units(&pawn_native_files::console_command(
                        s, this, &command,
                    ))
                }
                None => std::ptr::null(),
            }
        },
    )
}

/// `APlayerPawn::execResetKeyboard`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_reset_keyboard(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_files_reset_keyboard",
        (),
        move || unsafe {
            if let Some(s) = svc_of(svc) {
                pawn_native_files::reset_keyboard(s, this);
            }
        },
    )
}

/// `APawn::execFindBestInventoryPath` after its clock. Writes `*weight_out`
/// when the search improved on `weight`. Returns the best path or NULL.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_find_best_inventory_path(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
    weight: f32,
    predict: i32,
    weight_out: *mut f32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_files_find_best_inventory_path",
        std::ptr::null_mut(),
        move || unsafe {
            match svc_of(svc) {
                Some(s) if !weight_out.is_null() => pawn_native_files::find_best_inventory_path(
                    s,
                    this,
                    weight,
                    predict != 0,
                    weight_out,
                ),
                _ => std::ptr::null_mut(),
            }
        },
    )
}

/// `APlayerPawn::execScreenToWorld`: writes the world point to `out` (`FVector`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_screen_to_world(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
    x: f32,
    y: f32,
    z: f32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_files_screen_to_world",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), out.is_null()) {
                pawn_native_files::screen_to_world(
                    s,
                    this,
                    FVector { x, y, z },
                    out.cast::<FVector>(),
                );
            }
        },
    )
}

/// `AActor::execGetURLMap`. Returns the services' UNICHAR buffer, valid until the
/// next string-returning call on this thread; the caller copies it at once.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_url_map(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
) -> *const u16 {
    guard(
        "hp2_rs_engine_pawn_native_files_url_map",
        std::ptr::null(),
        move || unsafe {
            match svc_of(svc) {
                Some(s) => pawn_native_files::url_map(s, this),
                None => std::ptr::null(),
            }
        },
    )
}

/// `APlayerPawn::execGetEntryLevel`. Returns the entry `ALevelInfo*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_files_entry_level(
    svc: *const Hp2PawnNativeFilesServices,
    this: *mut c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_files_entry_level",
        std::ptr::null_mut(),
        move || unsafe {
            match svc_of(svc) {
                Some(s) => pawn_native_files::entry_level(s, this),
                None => std::ptr::null_mut(),
            }
        },
    )
}
