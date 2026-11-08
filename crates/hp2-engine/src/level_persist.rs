//! Checked persistent-actor save/load policy. Native public methods own the
//! automatic FString/TArray/TMultiMap objects; Rust borrows only raw pointers.
//! Native leaves return status (zero succeeds), never unwind through Rust.
//! Archive flags remain policy results, distinct from a captured exception.
//! The registered UTF-16 wire and initialized load scalars are preserved.

use std::ffi::c_void;

use hp2_abi::engine::generated::AActor;

macro_rules! checked {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = std::mem::MaybeUninit::uninit();
        if unsafe { ($f)($($arg,)* out.as_mut_ptr()) } != 0 { return Err(()); }
        unsafe { out.assume_init() }
    }};
}
macro_rules! checked_void {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        if unsafe { ($f)($($arg),*) } != 0 { return Err(()); }
    }};
}

/// Every callback is a checked native leaf. Context and iterator pointers refer
/// to native automatic objects, not heap-owned opaque factories.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LevelPersistServices {
    pub is_editor: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub build_filenames: unsafe extern "C" fn(context: *mut c_void, map_name: *const u32) -> i32,
    pub filename:
        unsafe extern "C" fn(context: *mut c_void, index: i32, out: *mut *const u32) -> i32,
    pub fstring_ne: unsafe extern "C" fn(context: *mut c_void, b: *const u32, out: *mut i32) -> i32,
    pub app_strcmp: unsafe extern "C" fn(a: *const u32, b: *const u32, out: *mut i32) -> i32,
    pub level_actors_num: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    pub level_actor_at:
        unsafe extern "C" fn(level: *mut c_void, index: i32, out: *mut *mut c_void) -> i32,
    pub actor_name: unsafe extern "C" fn(actor: *mut c_void, out: *mut *const u32) -> i32,
    pub save_array_add: unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void) -> i32,
    pub save_array_num: unsafe extern "C" fn(context: *mut c_void, out: *mut i32) -> i32,
    pub save_array_at:
        unsafe extern "C" fn(context: *mut c_void, index: i32, out: *mut *mut c_void) -> i32,
    pub save_array_empty: unsafe extern "C" fn(context: *mut c_void) -> i32,
    pub load_map_add:
        unsafe extern "C" fn(context: *mut c_void, name: *const u32, actor: *mut c_void) -> i32,
    pub load_map_find:
        unsafe extern "C" fn(context: *mut c_void, name: *const u32, out: *mut *mut c_void) -> i32,
    pub load_map_remove: unsafe extern "C" fn(context: *mut c_void, name: *const u32) -> i32,
    pub load_map_empty: unsafe extern "C" fn(context: *mut c_void) -> i32,
    pub destroy_remaining: unsafe extern "C" fn(
        services: *const Hp2LevelPersistServices,
        level: *mut c_void,
        context: *mut c_void,
    ) -> i32,
    pub load_map_iter_valid: unsafe extern "C" fn(iterator: *mut c_void, out: *mut i32) -> i32,
    pub load_map_iter_value:
        unsafe extern "C" fn(iterator: *mut c_void, out: *mut *mut c_void) -> i32,
    pub load_map_iter_next: unsafe extern "C" fn(iterator: *mut c_void) -> i32,
    pub archive_writer_create: unsafe extern "C" fn(path: *const u32, out: *mut *mut c_void) -> i32,
    pub archive_reader_create: unsafe extern "C" fn(path: *const u32, out: *mut *mut c_void) -> i32,
    pub archive_close: unsafe extern "C" fn(archive: *mut c_void) -> i32,
    pub archive_tell: unsafe extern "C" fn(archive: *mut c_void, out: *mut i32) -> i32,
    pub archive_seek: unsafe extern "C" fn(archive: *mut c_void, position: i32) -> i32,
    pub archive_total_size: unsafe extern "C" fn(archive: *mut c_void, out: *mut i32) -> i32,
    pub archive_get_error: unsafe extern "C" fn(archive: *mut c_void, out: *mut i32) -> i32,
    pub archive_serialize_bytes:
        unsafe extern "C" fn(archive: *mut c_void, data: *mut c_void, len: i32) -> i32,
    pub archive_serialize_int: unsafe extern "C" fn(archive: *mut c_void, value: *mut i32) -> i32,
    pub archive_serialize_sqword:
        unsafe extern "C" fn(archive: *mut c_void, value: *mut i64) -> i32,
    pub file_manager_file_size: unsafe extern "C" fn(path: *const u32, out: *mut i32) -> i32,
    pub file_manager_global_time: unsafe extern "C" fn(path: *const u32, out: *mut i64) -> i32,
    pub actor_serialize_persistence:
        unsafe extern "C" fn(archive: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_destroy: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void) -> i32,
    pub debug_log: unsafe extern "C" fn(context: *mut c_void, message: i32) -> i32,
}

impl Hp2LevelPersistServices {
    /// `AActor::bPersistent` (`UnActor.cpp`): a plain bitfield, no virtual dispatch.
    unsafe fn actor_b_persistent(&self, actor: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_persistent() as i32 })
    }
    /// `AActor::bScriptInitialized` (`UnActor.cpp`): a plain bitfield, no virtual dispatch.
    unsafe fn actor_set_b_script_initialized(&self, actor: *mut c_void, value: i32) -> Result<(), ()> {
        unsafe { (*actor.cast::<AActor>()).set_b_script_initialized(value != 0) };
        Ok(())
    }
}

pub(crate) const WIRE_CAPACITY: usize = 256;
const UNICHAR_BYTES: i32 = 2;

unsafe fn ser_int(s: &Hp2LevelPersistServices, ar: *mut c_void, value: &mut i32) -> Result<(), ()> {
    checked_void!(s.archive_serialize_int, ar, value);
    Ok(())
}
unsafe fn ser_sqword(
    s: &Hp2LevelPersistServices,
    ar: *mut c_void,
    value: &mut i64,
) -> Result<(), ()> {
    checked_void!(s.archive_serialize_sqword, ar, value);
    Ok(())
}
unsafe fn get_error(s: &Hp2LevelPersistServices, ar: *mut c_void) -> Result<bool, ()> {
    Ok(checked!(s.archive_get_error, ar) != 0)
}
unsafe fn write_units(
    s: &Hp2LevelPersistServices,
    ar: *mut c_void,
    wire: &[u16],
) -> Result<bool, ()> {
    for &unit in wire {
        let mut unit = unit;
        checked_void!(
            s.archive_serialize_bytes,
            ar,
            (&mut unit as *mut u16).cast(),
            UNICHAR_BYTES
        );
        if unsafe { get_error(s, ar)? } {
            return Ok(false);
        }
    }
    Ok(true)
}
unsafe fn read_units(
    s: &Hp2LevelPersistServices,
    ar: *mut c_void,
    wire: &mut [u16],
) -> Result<bool, ()> {
    let position = checked!(s.archive_tell, ar);
    let total = checked!(s.archive_total_size, ar);
    if position < 0 || total < position || (wire.len() as i32) * UNICHAR_BYTES > total - position {
        return Ok(false);
    }
    for unit in wire {
        checked_void!(
            s.archive_serialize_bytes,
            ar,
            (unit as *mut u16).cast(),
            UNICHAR_BYTES
        );
        if unsafe { get_error(s, ar)? } {
            return Ok(false);
        }
    }
    Ok(true)
}

// No callback occurs while reading the native source. The fixed wire buffer is
// the original automatic UNICHAR[256], including its terminating zero.
pub(crate) unsafe fn encode_wire(mut source: *const u32, wire: &mut [u16; WIRE_CAPACITY]) -> Option<usize> {
    let mut n = 0;
    loop {
        let cp = unsafe { *source };
        if cp == 0 {
            wire[n] = 0;
            return Some(n + 1);
        }
        if cp > 0x10ffff || (0xd800..=0xdfff).contains(&cp) {
            return None;
        }
        if cp <= 0xffff {
            if n >= WIRE_CAPACITY - 1 {
                return None;
            }
            wire[n] = cp as u16;
            n += 1;
        } else {
            if n >= WIRE_CAPACITY - 2 {
                return None;
            }
            let cp = cp - 0x10000;
            wire[n] = (0xd800 + (cp >> 10)) as u16;
            wire[n + 1] = (0xdc00 + (cp & 0x3ff)) as u16;
            n += 2;
        }
        source = unsafe { source.add(1) };
    }
}
pub(crate) fn decode_wire(wire: &[u16]) -> Option<[u32; WIRE_CAPACITY]> {
    if wire.is_empty() || *wire.last()? != 0 {
        return None;
    }
    let mut dest = [0; WIRE_CAPACITY];
    let mut wi = 0;
    let mut di = 0;
    while wi < wire.len() - 1 {
        let mut cp = wire[wi] as u32;
        if cp == 0 {
            return None;
        }
        if (0xd800..=0xdbff).contains(&cp) {
            wi += 1;
            if wi >= wire.len() - 1 {
                return None;
            }
            let low = wire[wi] as u32;
            if !(0xdc00..=0xdfff).contains(&low) {
                return None;
            }
            cp = 0x10000 + ((cp - 0xd800) << 10) + low - 0xdc00;
        } else if (0xdc00..=0xdfff).contains(&cp) {
            return None;
        }
        if di >= WIRE_CAPACITY - 1 {
            return None;
        }
        dest[di] = cp;
        di += 1;
        wi += 1;
    }
    Some(dest)
}
unsafe fn write_wire_string(
    s: &Hp2LevelPersistServices,
    ar: *mut c_void,
    source: *const u32,
) -> Result<bool, ()> {
    let mut wire = [0; WIRE_CAPACITY];
    let Some(units) = (unsafe { encode_wire(source, &mut wire) }) else {
        return Ok(false);
    };
    let mut bytes = units as i32 * UNICHAR_BYTES;
    unsafe {
        ser_int(s, ar, &mut bytes)?;
    }
    if unsafe { get_error(s, ar)? } {
        return Ok(false);
    }
    unsafe { write_units(s, ar, &wire[..units]) }
}
unsafe fn read_wire_string(
    s: &Hp2LevelPersistServices,
    ar: *mut c_void,
) -> Result<Option<[u32; WIRE_CAPACITY]>, ()> {
    let mut bytes = 0;
    unsafe {
        ser_int(s, ar, &mut bytes)?;
    }
    if unsafe { get_error(s, ar)? }
        || bytes < 2
        || bytes & 1 != 0
        || bytes / 2 > WIRE_CAPACITY as i32
    {
        return Ok(None);
    }
    let mut wire = [0; WIRE_CAPACITY];
    let units = bytes as usize / 2;
    if !unsafe { read_units(s, ar, &mut wire[..units])? } {
        return Ok(None);
    }
    Ok(decode_wire(&wire[..units]))
}
unsafe fn write_identifier(s: &Hp2LevelPersistServices, ar: *mut c_void) -> Result<bool, ()> {
    unsafe { write_units(s, ar, &[80, 65, 48, 0]) }
}
unsafe fn read_identifier(s: &Hp2LevelPersistServices, ar: *mut c_void) -> Result<bool, ()> {
    let mut id = [0; 4];
    if !unsafe { read_units(s, ar, &mut id)? } {
        return Ok(false);
    }
    Ok(id[0] == 80 && id[1] == 65 && (id[2] == 48 || id[2] == 49) && id[3] == 0)
}

/// Runs inside the native TMultiMap::TIterator automatic lifetime.
/// # Safety
/// All pointers and the service table must remain live for this call.
pub unsafe fn destroy_remaining(
    s: &Hp2LevelPersistServices,
    level: *mut c_void,
    iterator: *mut c_void,
) -> Result<(), ()> {
    while checked!(s.load_map_iter_valid, iterator) != 0 {
        let actor = checked!(s.load_map_iter_value, iterator);
        if !actor.is_null() {
            checked_void!(s.actor_destroy, level, actor);
        }
        checked_void!(s.load_map_iter_next, iterator);
    }
    Ok(())
}

/// `ULevel::SavePersistentActors(MapName)`. Returns the `bool` result.
///
/// # Safety
/// `level` is a live `ULevel*`; `map_name` is a NUL-terminated `TCHAR` string.
/// `context` addresses the native method's live filename and save-array locals.
pub unsafe fn save_persistent_actors(
    s: &Hp2LevelPersistServices,
    level: *mut c_void,
    map_name: *const u32,
    context: *mut c_void,
) -> Result<bool, ()> {
    // Make sure we are NOT in the editor.
    if checked!(s.is_editor) != 0 {
        return Ok(false);
    }
    checked_void!(s.build_filenames, context, map_name);
    let final_name = checked!(s.filename, context, 0);
    let map_filename = checked!(s.filename, context, 1);
    let pa_filename = checked!(s.filename, context, 2);

    let mut b_error = false;
    let mut b_wire_error = false;
    let mut pact_ar: *mut c_void = std::ptr::null_mut();

    'body: {
        // go through the level's actor list and find out how many need to be saved

        let mut i = 0;
        while i < checked!(s.level_actors_num, level) {
            let act = checked!(s.level_actor_at, level, i);
            if !act.is_null() && unsafe { s.actor_b_persistent(act)? } != 0 {
                checked_void!(s.save_array_add, context, act);
            }
            i += 1;
        }

        // if there are no actors to save then break out
        if checked!(s.save_array_num, context) == 0 {
            break 'body;
        }

        // open our save pa (persistent actors) file
        pact_ar = checked!(s.archive_writer_create, pa_filename);
        if pact_ar.is_null() || unsafe { get_error(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 0);
            break 'body;
        }

        checked_void!(s.debug_log, context, 1);

        // Block 1a.) PA file identifier.
        if !unsafe { write_identifier(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 2);
            b_error = true;
            break 'body;
        }

        // Block 1b.) Map name.
        if !unsafe { write_wire_string(s, pact_ar, final_name)? } {
            checked_void!(s.debug_log, context, 3);
            b_error = true;
            break 'body;
        }

        // Block 1c.) The level's file size.
        let mut i_to_save = checked!(s.file_manager_file_size, map_filename);
        unsafe { ser_int(s, pact_ar, &mut i_to_save)? };
        if unsafe { get_error(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 4);
            break 'body;
        }

        // Block 1d.) The level's file time.
        let mut file_time = checked!(s.file_manager_global_time, map_filename);
        unsafe { ser_sqword(s, pact_ar, &mut file_time)? };
        if unsafe { get_error(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 5);
            break 'body;
        }

        // Block 2.) Number of persistent actors.
        let mut i_persistent_actors = checked!(s.save_array_num, context);
        unsafe { ser_int(s, pact_ar, &mut i_persistent_actors)? };
        if unsafe { get_error(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 6);
            break 'body;
        }

        // Block 3.) Save all persistent actors.
        let mut i = 0;
        while i < checked!(s.save_array_num, context) {
            let act = checked!(s.save_array_at, context, i);
            // Block 3.1i) Name.
            let name = checked!(s.actor_name, act);
            if !unsafe { write_wire_string(s, pact_ar, name)? } {
                checked_void!(s.debug_log, context, 7);
                b_error = true;
                b_wire_error = true;
                break;
            }

            // Block 3.2i) Dummy size of the tagged properties.
            let pos_size = checked!(s.archive_tell, pact_ar);
            unsafe { ser_int(s, pact_ar, &mut i_to_save)? };
            if unsafe { get_error(s, pact_ar)? } {
                checked_void!(s.debug_log, context, 8);
                break;
            }

            // Block 3.3i) The persistence properties.
            let pos_pre_prop = checked!(s.archive_tell, pact_ar);
            checked_void!(s.actor_serialize_persistence, pact_ar, act);
            if unsafe { get_error(s, pact_ar)? } {
                checked_void!(s.debug_log, context, 9);
                break;
            }
            let pos_post_prop = checked!(s.archive_tell, pact_ar);

            // Block 3.2i) Real size of the tagged properties, written back at pos_size.
            checked_void!(s.archive_seek, pact_ar, pos_size);
            i_to_save = pos_post_prop - pos_pre_prop;
            unsafe { ser_int(s, pact_ar, &mut i_to_save)? };
            checked_void!(s.archive_seek, pact_ar, pos_post_prop);
            if unsafe { get_error(s, pact_ar)? } {
                checked_void!(s.debug_log, context, 10);
                break;
            }
            i += 1;
        }
        if b_wire_error {
            break 'body;
        }

        // Block 4.) EOF token.
        if !unsafe { write_wire_string(s, pact_ar, [69u32, 79, 70, 0].as_ptr())? } {
            checked_void!(s.debug_log, context, 11);
            b_error = true;
            break 'body;
        }

        checked_void!(s.debug_log, context, 12);
    }

    // Explicit Empty precedes archive deletion; exception returns skip it.
    checked_void!(s.save_array_empty, context);
    if !pact_ar.is_null() {
        if unsafe { get_error(s, pact_ar)? } {
            b_error = true;
        }
        checked_void!(s.archive_close, pact_ar);
    }

    Ok(b_error)
}

/// `ULevel::LoadPersistentActors(MapName)`. Returns the `bool` result.
///
/// # Safety
/// `level` is a live `ULevel*`; `map_name` is a NUL-terminated `TCHAR` string.
/// `context` addresses the native method's live filename and load-map locals.
pub unsafe fn load_persistent_actors(
    s: &Hp2LevelPersistServices,
    level: *mut c_void,
    map_name: *const u32,
    context: *mut c_void,
) -> Result<bool, ()> {
    // Make sure we are NOT in the editor.
    if checked!(s.is_editor) != 0 {
        return Ok(false);
    }
    checked_void!(s.build_filenames, context, map_name);
    let map_filename = checked!(s.filename, context, 1);
    let pa_filename = checked!(s.filename, context, 2);

    let mut b_error = false;
    let mut b_wire_error = false;
    let pact_ar: *mut c_void;
    let actors_to_load = context;

    'body: {
        // Open our actors file (if we have one).
        pact_ar = checked!(s.archive_reader_create, pa_filename);
        if pact_ar.is_null()
            || unsafe { get_error(s, pact_ar)? }
            || checked!(s.archive_total_size, pact_ar) == 0
        {
            checked_void!(s.debug_log, context, 13);
            break 'body;
        }

        checked_void!(s.debug_log, context, 14);

        // Create a map of persistent actors we will need to load.
        let mut i_to_load = 0i32;
        let mut i = 0;
        while i < checked!(s.level_actors_num, level) {
            let act = checked!(s.level_actor_at, level, i);
            if !act.is_null() && unsafe { s.actor_b_persistent(act)? } != 0 {
                let name = checked!(s.actor_name, act);
                checked_void!(s.load_map_add, actors_to_load, name, act);
                i_to_load += 1;
            }
            i += 1;
        }

        // If we have no persistent actors to load then break out.
        if i_to_load == 0 {
            break 'body;
        }

        // Block 1a.) PA file identifier.
        if !unsafe { read_identifier(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 15);
            b_error = true;
            break 'body;
        }

        // Block 1b.) Map name the archive affects.
        let Some(str_name) = (unsafe { read_wire_string(s, pact_ar)? }) else {
            checked_void!(s.debug_log, context, 16);
            b_error = true;
            break 'body;
        };

        // Error check the map name: FinalMapName != FString(strName).
        if checked!(s.fstring_ne, context, str_name.as_ptr()) != 0 {
            checked_void!(s.debug_log, context, 17);
            break 'body;
        }

        // Block 1c.) Level file size, compared against iToLoad (as the original does).
        unsafe { ser_int(s, pact_ar, &mut i_to_load)? };
        if i_to_load != checked!(s.file_manager_file_size, map_filename) {
            checked_void!(s.debug_log, context, 18);
            break 'body;
        }

        // Block 1d.) Level file time.
        let mut file_time = 0i64;
        unsafe { ser_sqword(s, pact_ar, &mut file_time)? };
        if file_time != checked!(s.file_manager_global_time, map_filename) {
            checked_void!(s.debug_log, context, 19);
            break 'body;
        }

        // Block 2.) Number of actors in the file.
        let mut i_pactors_in_file = 0i32;
        unsafe { ser_int(s, pact_ar, &mut i_pactors_in_file)? };
        if unsafe { get_error(s, pact_ar)? } {
            checked_void!(s.debug_log, context, 20);
            break 'body;
        }

        // Block 3.) Sync each actor in the file with the level.
        for _ in 0..i_pactors_in_file {
            // Block 3.1i) Name.
            let Some(str_name) = (unsafe { read_wire_string(s, pact_ar)? }) else {
                checked_void!(s.debug_log, context, 21);
                b_error = true;
                b_wire_error = true;
                break;
            };

            // Block 3.2i) Size of this actor's tagged properties.
            let mut i_prop_size = 0i32;
            unsafe { ser_int(s, pact_ar, &mut i_prop_size)? };
            if unsafe { get_error(s, pact_ar)? } {
                checked_void!(s.debug_log, context, 22);
                break;
            }

            let act = checked!(s.load_map_find, actors_to_load, str_name.as_ptr());
            if act.is_null() {
                // Case 3: in the PA file but not in the level. Skip its properties.
                checked_void!(s.debug_log, context, 23);
                let pos = checked!(s.archive_tell, pact_ar);
                checked_void!(s.archive_seek, pact_ar, pos + i_prop_size);
                continue;
            }

            // Names must match.
            if checked!(s.app_strcmp, checked!(s.actor_name, act), str_name.as_ptr()) != 0 {
                checked_void!(s.debug_log, context, 24);
                break;
            }

            // Case 1: in both. Remove from the to-load map and override properties.
            checked_void!(s.load_map_remove, actors_to_load, str_name.as_ptr());
            checked_void!(s.actor_serialize_persistence, pact_ar, act);
            if unsafe { get_error(s, pact_ar)? } {
                checked_void!(s.debug_log, context, 25);
                break;
            }

            // bScriptInitialized = false so the actor is placed in its collision zone at LoadMap.
            unsafe { s.actor_set_b_script_initialized(act, 0)? };
        }
        // Quirk: IF_TRUE_MSG_AND_BREAK inside the loop only exits the loop.
        // Wire errors leave bWireError set and stop here.
        if b_wire_error {
            break 'body;
        }

        // Block 4.) EOF token.
        let Some(str_name) = (unsafe { read_wire_string(s, pact_ar)? }) else {
            checked_void!(s.debug_log, context, 26);
            b_error = true;
            break 'body;
        };
        if checked!(s.app_strcmp, [69u32, 79, 70, 0].as_ptr(), str_name.as_ptr()) != 0 {
            checked_void!(s.debug_log, context, 27);
            break 'body;
        }

        // Case 2: in the level but not in the file. Destroy, in TMultiMap order.
        checked_void!(s.destroy_remaining, s as *const _, level, context);

        checked_void!(s.debug_log, context, 28);
    }

    // *** clean up and return
    checked_void!(s.load_map_empty, actors_to_load);
    if !pact_ar.is_null() {
        if unsafe { get_error(s, pact_ar)? } {
            b_error = true;
        }
        checked_void!(s.archive_close, pact_ar);
    }
    Ok(b_error)
}
