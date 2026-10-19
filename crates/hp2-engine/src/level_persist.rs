//! Persistent actor save/load (`ULevel::SavePersistentActors` and
//! `ULevel::LoadPersistentActors`, `Engine/Src/UnLevel.cpp` @ 7ddb1d3).
//!
//! Strings are `Vec<u32>` copies of `TCHAR` text (`TCHAR` is 4 bytes on this
//! build, so only the `sizeof(TCHAR) == 4` wire branch is ported). The wire
//! format is UTF-16 code units (`UNICHAR`, 2 bytes) with an `INT` byte count
//! prefix and a trailing 0 unit. `FArchive`, `FString`, `TMultiMap`,
//! `GFileManager`, `DestroyActor` and actor field access go through
//! [`Hp2LevelPersistServices`]. Services marked "recorded" are the ones the
//! oracle harness records; all others are plain field or container reads
//! that the oracle does not record.
//!
//! Local `TArray<AActor*> ActorsToSave` is a Rust `Vec<*mut c_void>`: it is
//! never observed through a service in the oracle.
//!
//! Divergence: the commented-out TempActor spawn block in Load Case 3 is a
//! comment in the original and is deleted here.

use std::ffi::c_void;

/// `PERSISTENT_WIRE_NAME_CAPACITY` (`UnLevel.cpp`).
const WIRE_CAPACITY: usize = 256;
/// `sizeof(UNICHAR)`.
const UNICHAR_BYTES: i32 = 2;
/// `MAXINT`.
const MAXINT: i32 = i32::MAX;
/// `NAME_Log` and `NAME_Error` as passed to `debugf`.
const NAME_LOG: i32 = 0;
const NAME_ERROR: i32 = 1;

/// Services for the PERSIST group (`Engine/Src/Hp2LevelServices.cpp`).
/// Pointer arguments: `*const u32` are NUL-terminated `TCHAR` strings; copy-out
/// functions take `(out, cap)` and return the full length including the NUL,
/// so Rust retries with a larger buffer when `ret > cap`. Engine objects are
/// `*mut c_void`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LevelPersistServices {
    // ---- plain globals and fields (not recorded) ----
    /// `GIsEditor`. Returns `UBOOL`.
    pub is_editor: unsafe extern "C" fn() -> i32,
    /// `GSys->MapPath`.
    pub gsys_map_path: unsafe extern "C" fn(out: *mut u32, cap: i32) -> i32,
    /// `GSys->SaveSlotPath`.
    pub gsys_save_slot_path: unsafe extern "C" fn(out: *mut u32, cap: i32) -> i32,
    /// `URL.DefaultMapExt` (`FURL::DefaultMapExt`).
    pub url_default_map_ext: unsafe extern "C" fn(out: *mut u32, cap: i32) -> i32,
    /// `URL.DefaultSaveExt` (`FURL::DefaultSaveExt`).
    pub url_default_save_ext: unsafe extern "C" fn(out: *mut u32, cap: i32) -> i32,
    /// `appPathLeaf(path)`.
    pub app_path_leaf: unsafe extern "C" fn(path: *const u32, out: *mut u32, cap: i32) -> i32,
    /// `FString::InStr(sub)`, case-sensitive as the oracle calls it.
    pub fstring_in_str: unsafe extern "C" fn(s: *const u32, sub: *const u32) -> i32,
    /// `FString::Left(n)`.
    pub fstring_left: unsafe extern "C" fn(s: *const u32, n: i32, out: *mut u32, cap: i32) -> i32,
    /// `a * b` (`FString operator*`, path join).
    pub fstring_path_join:
        unsafe extern "C" fn(a: *const u32, b: *const u32, out: *mut u32, cap: i32) -> i32,
    /// `a + b` and `a += b` (`FString` concatenation).
    pub fstring_append:
        unsafe extern "C" fn(a: *const u32, b: *const u32, out: *mut u32, cap: i32) -> i32,
    /// `FString != FString(str)`. Returns `UBOOL`.
    pub fstring_ne: unsafe extern "C" fn(a: *const u32, b: *const u32) -> i32,
    /// `appStrcmp(a, b)`.
    pub app_strcmp: unsafe extern "C" fn(a: *const u32, b: *const u32) -> i32,
    /// `ULevel::Actors.Num()`.
    pub level_actors_num: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `ULevel::Actors(i)`.
    pub level_actor_at: unsafe extern "C" fn(level: *mut c_void, i: i32) -> *mut c_void,
    /// `pAct->bPersistent`. Returns `UBOOL`.
    pub actor_b_persistent: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `pAct->GetName()`, copy-out.
    pub actor_name: unsafe extern "C" fn(actor: *mut c_void, out: *mut u32, cap: i32) -> i32,
    /// `pAct->bScriptInitialized = v`.
    pub actor_set_b_script_initialized: unsafe extern "C" fn(actor: *mut c_void, v: i32),
    /// `new TMultiMap<FName,AActor*>`.
    pub load_map_new: unsafe extern "C" fn() -> *mut c_void,
    /// `delete` of the map (`ActorsToLoad.Empty()` then destruction).
    pub load_map_delete: unsafe extern "C" fn(map: *mut c_void),
    /// `ActorsToLoad.Add(name, actor)`.
    pub load_map_add: unsafe extern "C" fn(map: *mut c_void, name: *const u32, actor: *mut c_void),
    /// `ActorsToLoad.Find(name)` as `*ppAct`; null when absent.
    pub load_map_find: unsafe extern "C" fn(map: *mut c_void, name: *const u32) -> *mut c_void,
    /// `ActorsToLoad.Remove(name)`.
    pub load_map_remove: unsafe extern "C" fn(map: *mut c_void, name: *const u32),
    /// `TMultiMap::TIterator It(ActorsToLoad)`, heap-allocated.
    pub load_map_iter_begin: unsafe extern "C" fn(map: *mut c_void) -> *mut c_void,
    /// `It` as a bool (`It` is valid). Returns `UBOOL`.
    pub load_map_iter_valid: unsafe extern "C" fn(it: *mut c_void) -> i32,
    /// `It.Value()`.
    pub load_map_iter_value: unsafe extern "C" fn(it: *mut c_void) -> *mut c_void,
    /// `++It`.
    pub load_map_iter_next: unsafe extern "C" fn(it: *mut c_void),
    /// Destroy the heap-allocated iterator.
    pub load_map_iter_end: unsafe extern "C" fn(it: *mut c_void),

    // ---- recorded (harness records these, in call order) ----
    /// `GFileManager->CreateFileWriter(path)`. Returns null on failure.
    pub archive_writer_create: unsafe extern "C" fn(path: *const u32) -> *mut c_void,
    /// `GFileManager->CreateFileReader(path)`. Returns null on failure.
    pub archive_reader_create: unsafe extern "C" fn(path: *const u32) -> *mut c_void,
    /// `delete pActAr`.
    pub archive_close: unsafe extern "C" fn(ar: *mut c_void),
    /// `Ar.Tell()`.
    pub archive_tell: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.Seek(pos)`.
    pub archive_seek: unsafe extern "C" fn(ar: *mut c_void, pos: i32),
    /// `Ar.TotalSize()`.
    pub archive_total_size: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.GetError()`. Returns `UBOOL`.
    pub archive_get_error: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.ByteOrderSerialize(data, len)`.
    pub archive_serialize_bytes: unsafe extern "C" fn(ar: *mut c_void, data: *mut c_void, len: i32),
    /// `Ar << v` for `INT`; the value is read or written through `v`.
    pub archive_serialize_int: unsafe extern "C" fn(ar: *mut c_void, v: *mut i32),
    /// `Ar << v` for `SQWORD`.
    pub archive_serialize_sqword: unsafe extern "C" fn(ar: *mut c_void, v: *mut i64),
    /// `GFileManager->FileSize(path)`.
    pub file_manager_file_size: unsafe extern "C" fn(path: *const u32) -> i32,
    /// `GFileManager->GetGlobalTime(path)`.
    pub file_manager_global_time: unsafe extern "C" fn(path: *const u32) -> i64,
    /// `pAct->SerializePersistence(*pActAr)`.
    pub actor_serialize_persistence: unsafe extern "C" fn(ar: *mut c_void, actor: *mut c_void),
    /// `DestroyActor(pAct)` on `level`.
    pub actor_destroy: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void),
    /// `debugf(name, "%s", msg)`; `name` is `NAME_Log` (0) or `NAME_Error` (1).
    pub debug_log: unsafe extern "C" fn(name: i32, msg: *const u32),
}

/// Wire string, `u32` units, no trailing NUL.
type Text = Vec<u32>;

fn text(s: &str) -> Text {
    s.chars().map(|c| c as u32).collect()
}

/// `s` followed by NUL, for `*const u32` arguments.
fn z(s: &[u32]) -> Text {
    let mut v = s.to_vec();
    v.push(0);
    v
}

/// Copy-out call with `(out, cap)` retry. The service returns the full length
/// including the NUL; the result excludes it.
fn copy_out(f: impl Fn(*mut u32, i32) -> i32) -> Text {
    let mut cap = 256usize;
    loop {
        let mut buf = vec![0u32; cap];
        let n = f(buf.as_mut_ptr(), cap as i32) as usize;
        if n <= cap {
            buf.truncate(n - 1);
            return buf;
        }
        cap = n;
    }
}

/// `pAct->GetName()` copied into a Rust buffer.
unsafe fn actor_name(s: &Hp2LevelPersistServices, actor: *mut c_void) -> Text {
    copy_out(|o, c| unsafe { (s.actor_name)(actor, o, c) })
}

fn debugf(s: &Hp2LevelPersistServices, name: i32, msg: &[u32]) {
    let m = z(msg);
    unsafe { (s.debug_log)(name, m.as_ptr()) }
}

/// `FString` path helpers, in the oracle's call order.
fn path_leaf(s: &Hp2LevelPersistServices, path: &[u32]) -> Text {
    let p = z(path);
    copy_out(|o, c| unsafe { (s.app_path_leaf)(p.as_ptr(), o, c) })
}

fn in_str(s: &Hp2LevelPersistServices, a: &[u32], sub: &[u32]) -> i32 {
    let (a, sub) = (z(a), z(sub));
    unsafe { (s.fstring_in_str)(a.as_ptr(), sub.as_ptr()) }
}

fn left(s: &Hp2LevelPersistServices, a: &[u32], n: i32) -> Text {
    let a = z(a);
    copy_out(|o, c| unsafe { (s.fstring_left)(a.as_ptr(), n, o, c) })
}

fn path_join(s: &Hp2LevelPersistServices, a: &[u32], b: &[u32]) -> Text {
    let (a, b) = (z(a), z(b));
    copy_out(|o, c| unsafe { (s.fstring_path_join)(a.as_ptr(), b.as_ptr(), o, c) })
}

fn append(s: &Hp2LevelPersistServices, a: &[u32], b: &[u32]) -> Text {
    let (a, b) = (z(a), z(b));
    copy_out(|o, c| unsafe { (s.fstring_append)(a.as_ptr(), b.as_ptr(), o, c) })
}

fn gsys_map_path(s: &Hp2LevelPersistServices) -> Text {
    copy_out(|o, c| unsafe { (s.gsys_map_path)(o, c) })
}

fn gsys_save_slot_path(s: &Hp2LevelPersistServices) -> Text {
    copy_out(|o, c| unsafe { (s.gsys_save_slot_path)(o, c) })
}

fn url_default_map_ext(s: &Hp2LevelPersistServices) -> Text {
    copy_out(|o, c| unsafe { (s.url_default_map_ext)(o, c) })
}

fn url_default_save_ext(s: &Hp2LevelPersistServices) -> Text {
    copy_out(|o, c| unsafe { (s.url_default_save_ext)(o, c) })
}

/// Shared filename construction for Save and Load (the first block of each).
/// Returns `(FinalMapName, MapFilename, PAFilename)`.
fn build_filenames(s: &Hp2LevelPersistServices, map_name: &[u32]) -> (Text, Text, Text) {
    // FinalMapName = appPathLeaf(*MapName);
    let mut final_name = path_leaf(s, map_name);
    let map_ext = url_default_map_ext(s);
    // if( FinalMapName.InStr( URL.DefaultMapExt ) < 0 ) { += "." ; += ext }
    if in_str(s, &final_name, &map_ext) < 0 {
        final_name = append(s, &final_name, &text("."));
        final_name = append(s, &final_name, &map_ext);
    }
    // MapFilename = GSys->MapPath * FinalMapName;
    let map_filename = path_join(s, &gsys_map_path(s), &final_name);
    // PAFilename = GSys->SaveSlotPath * "cache" * FinalMapName.Left(InStr-1) + "_pa." + DefaultSaveExt;
    let slot = gsys_save_slot_path(s);
    let cache = path_join(s, &slot, &text("cache"));
    let stem_len = in_str(s, &final_name, &map_ext) - 1;
    let stem = left(s, &final_name, stem_len);
    let joined = path_join(s, &cache, &stem);
    let with_pa = append(s, &joined, &text("_pa."));
    let pa_filename = append(s, &with_pa, &url_default_save_ext(s));
    (final_name, map_filename, pa_filename)
}

/// `Ar << v` for `INT`, returning the value after the call.
unsafe fn ser_int(s: &Hp2LevelPersistServices, ar: *mut c_void, v: &mut i32) {
    unsafe { (s.archive_serialize_int)(ar, v) }
}

unsafe fn ser_sqword(s: &Hp2LevelPersistServices, ar: *mut c_void, v: &mut i64) {
    unsafe { (s.archive_serialize_sqword)(ar, v) }
}

unsafe fn get_error(s: &Hp2LevelPersistServices, ar: *mut c_void) -> bool {
    unsafe { (s.archive_get_error)(ar) != 0 }
}

/// `FArchive::ByteOrderSerialize` on one `UNICHAR`, then `GetError()`.
/// Returns false on error (as `WritePersistentWireUnits` does).
unsafe fn write_units(s: &Hp2LevelPersistServices, ar: *mut c_void, wire: &[u16]) -> bool {
    for &unit in wire {
        let mut u = unit;
        unsafe { (s.archive_serialize_bytes)(ar, (&mut u as *mut u16).cast(), UNICHAR_BYTES) };
        if unsafe { get_error(s, ar) } {
            return false;
        }
    }
    true
}

/// `CanReadPersistentBytes(Ar, ByteCount)`: `Tell()` then `TotalSize()`.
unsafe fn can_read(s: &Hp2LevelPersistServices, ar: *mut c_void, byte_count: i32) -> bool {
    let position = unsafe { (s.archive_tell)(ar) };
    let total = unsafe { (s.archive_total_size)(ar) };
    byte_count >= 0 && position >= 0 && total >= position && byte_count <= total - position
}

/// `ReadPersistentWireUnits`: count check, `CanRead`, then per-unit read with
/// `GetError()` after each unit.
unsafe fn read_units(s: &Hp2LevelPersistServices, ar: *mut c_void, units: i32) -> Option<Vec<u16>> {
    if !(0..=MAXINT / UNICHAR_BYTES).contains(&units) {
        return None;
    }
    let bytes = units * UNICHAR_BYTES;
    if !unsafe { can_read(s, ar, bytes) } {
        return None;
    }
    let mut wire = vec![0u16; units as usize];
    for unit in wire.iter_mut() {
        unsafe { (s.archive_serialize_bytes)(ar, (unit as *mut u16).cast(), UNICHAR_BYTES) };
        if unsafe { get_error(s, ar) } {
            return None;
        }
    }
    Some(wire)
}

/// `EncodePersistentWireString` (TCHAR = 4 bytes branch). Returns the wire
/// units including the trailing 0, or `None` on any rejection.
fn encode_wire(source: &[u32], capacity: usize) -> Option<Vec<u16>> {
    let cap = capacity as i64;
    let mut wire: Vec<u16> = Vec::new();
    for &cp in source {
        let idx = wire.len() as i64;
        if cp > 0x10ffff || (0xd800..=0xdfff).contains(&cp) {
            return None;
        }
        if cp <= 0xffff {
            if idx >= cap - 1 {
                return None;
            }
            wire.push(cp as u16);
        } else {
            if idx >= cap - 2 {
                return None;
            }
            let supplement = cp - 0x10000;
            wire.push((0xd800 + (supplement >> 10)) as u16);
            wire.push((0xdc00 + (supplement & 0x3ff)) as u16);
        }
    }
    if wire.len() as i64 + 1 > (MAXINT / UNICHAR_BYTES) as i64 {
        return None;
    }
    wire.push(0);
    Some(wire)
}

/// `DecodePersistentWireString` (TCHAR = 4 bytes branch). `wire` includes
/// the trailing 0; the result excludes it.
fn decode_wire(wire: &[u16], dest_capacity: usize) -> Option<Text> {
    let units = wire.len();
    if units == 0 || wire[units - 1] != 0 {
        return None;
    }
    let dest_cap = dest_capacity as i64;
    let mut dest: Text = Vec::new();
    let mut wi = 0usize;
    while wi < units - 1 {
        let unit = wire[wi] as u32;
        if unit == 0 {
            return None;
        }
        let mut cp = unit;
        if (0xd800..=0xdbff).contains(&unit) {
            wi += 1;
            if wi >= units - 1 {
                return None;
            }
            let low = wire[wi] as u32;
            if !(0xdc00..=0xdfff).contains(&low) {
                return None;
            }
            cp = 0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00);
        } else if (0xdc00..=0xdfff).contains(&unit) {
            return None;
        }
        if dest.len() as i64 >= dest_cap - 1 {
            return None;
        }
        dest.push(cp);
        wi += 1;
    }
    Some(dest)
}

/// `WritePersistentWireString`: encode, `Ar << WireBytes`, `GetError()`, units.
unsafe fn write_wire_string(s: &Hp2LevelPersistServices, ar: *mut c_void, source: &[u32]) -> bool {
    let Some(wire) = encode_wire(source, WIRE_CAPACITY) else {
        return false;
    };
    let mut wire_bytes = wire.len() as i32 * UNICHAR_BYTES;
    unsafe { ser_int(s, ar, &mut wire_bytes) };
    !unsafe { get_error(s, ar) } && unsafe { write_units(s, ar, &wire) }
}

/// `ReadPersistentWireString`: `Ar << WireBytes`, checks, units, decode.
unsafe fn read_wire_string(s: &Hp2LevelPersistServices, ar: *mut c_void) -> Option<Text> {
    let mut wire_bytes = 0i32;
    unsafe { ser_int(s, ar, &mut wire_bytes) };
    if unsafe { get_error(s, ar) } || wire_bytes < UNICHAR_BYTES || (wire_bytes & 1) != 0 {
        return None;
    }
    let units = wire_bytes / UNICHAR_BYTES;
    if units as usize > WIRE_CAPACITY {
        return None;
    }
    let wire = unsafe { read_units(s, ar, units)? };
    decode_wire(&wire, WIRE_CAPACITY)
}

/// `WritePersistentIdentifier`: "PA0" plus trailing 0.
unsafe fn write_identifier(s: &Hp2LevelPersistServices, ar: *mut c_void) -> bool {
    let id: [u16; 4] = [b'P' as u16, b'A' as u16, b'0' as u16, 0];
    unsafe { write_units(s, ar, &id) }
}

/// `ReadPersistentIdentifier`: `PA0` or `PA1` plus trailing 0.
unsafe fn read_identifier(s: &Hp2LevelPersistServices, ar: *mut c_void) -> bool {
    let Some(id) = (unsafe { read_units(s, ar, 4) }) else {
        return false;
    };
    id[0] == b'P' as u16
        && id[1] == b'A' as u16
        && (id[2] == b'0' as u16 || id[2] == b'1' as u16)
        && id[3] == 0
}

/// `ULevel::SavePersistentActors(MapName)`. Returns the `bool` result.
///
/// # Safety
/// `level` is a live `ULevel*`; `map_name` is a NUL-terminated `TCHAR` string.
pub unsafe fn save_persistent_actors(
    s: &Hp2LevelPersistServices,
    level: *mut c_void,
    map_name: *const u32,
) -> bool {
    // Make sure we are NOT in the editor.
    if unsafe { (s.is_editor)() } != 0 {
        return false;
    }
    let map_name = unsafe { c_text(map_name) };
    let (final_name, map_filename, pa_filename) = build_filenames(s, &map_name);

    let mut b_error = false;
    let mut b_wire_error = false;
    let mut pact_ar: *mut c_void = std::ptr::null_mut();

    'body: {
        // go through the level's actor list and find out how many need to be saved
        let mut actors_to_save: Vec<*mut c_void> = Vec::new();
        let num = unsafe { (s.level_actors_num)(level) };
        for i in 0..num {
            let act = unsafe { (s.level_actor_at)(level, i) };
            if !act.is_null() && unsafe { (s.actor_b_persistent)(act) } != 0 {
                actors_to_save.push(act);
            }
        }

        // if there are no actors to save then break out
        if actors_to_save.is_empty() {
            break 'body;
        }

        // open our save pa (persistent actors) file
        pact_ar = unsafe { (s.archive_writer_create)(z(&pa_filename).as_ptr()) };
        if pact_ar.is_null() || unsafe { get_error(s, pact_ar) } {
            debugf(
                s,
                NAME_LOG,
                &fmt_s(
                    "*** Could not Save Persistent Actor file ",
                    &pa_filename,
                    ", CreateFileWriter had an error!!!!.",
                ),
            );
            break 'body;
        }

        debugf(
            s,
            NAME_LOG,
            &fmt_s(
                "*** Starting to Save Persistent Actors for map ",
                &final_name,
                "",
            ),
        );

        // Block 1a.) PA file identifier.
        if !unsafe { write_identifier(s, pact_ar) } {
            debugf(
                s,
                NAME_ERROR,
                &text("ULevelBase::SavePersistentActors ERROR: Saving map PA file identifier!"),
            );
            b_error = true;
            break 'body;
        }

        // Block 1b.) Map name.
        if !unsafe { write_wire_string(s, pact_ar, &final_name) } {
            debugf(
                s,
                NAME_ERROR,
                &text("ULevelBase::SavePersistentActors ERROR: Saving map name as UTF-16LE!"),
            );
            b_error = true;
            break 'body;
        }

        // Block 1c.) The level's file size.
        let mut i_to_save = unsafe { (s.file_manager_file_size)(z(&map_filename).as_ptr()) };
        unsafe { ser_int(s, pact_ar, &mut i_to_save) };
        if unsafe { get_error(s, pact_ar) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::SavePersistentActors ERROR: Saving map FileSize that this PA file is for!",
                ),
            );
            break 'body;
        }

        // Block 1d.) The level's file time.
        let mut file_time = unsafe { (s.file_manager_global_time)(z(&map_filename).as_ptr()) };
        unsafe { ser_sqword(s, pact_ar, &mut file_time) };
        if unsafe { get_error(s, pact_ar) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::SavePersistentActors ERROR: Saving map FileTime that this PA file is for!",
                ),
            );
            break 'body;
        }

        // Block 2.) Number of persistent actors.
        let mut i_persistent_actors = actors_to_save.len() as i32;
        unsafe { ser_int(s, pact_ar, &mut i_persistent_actors) };
        if unsafe { get_error(s, pact_ar) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::SavePersistentActors ERROR: Saving Number of persistent Actors!",
                ),
            );
            break 'body;
        }

        // Block 3.) Save all persistent actors.
        for &act in &actors_to_save {
            // Block 3.1i) Name.
            let name = unsafe { actor_name(s, act) };
            if !unsafe { write_wire_string(s, pact_ar, &name) } {
                debugf(
                    s,
                    NAME_ERROR,
                    &text("ULevelBase::SavePersistentActors ERROR: Saving actor name as UTF-16LE!"),
                );
                b_error = true;
                b_wire_error = true;
                break;
            }

            // Block 3.2i) Dummy size of the tagged properties.
            let pos_size = unsafe { (s.archive_tell)(pact_ar) };
            unsafe { ser_int(s, pact_ar, &mut i_to_save) };
            if unsafe { get_error(s, pact_ar) } {
                debugf(
                    s,
                    NAME_ERROR,
                    &text(
                        "ULevelBase::SavePersistentActors ERROR: Saving temp size of taggedProperties!",
                    ),
                );
                break;
            }

            // Block 3.3i) The persistence properties.
            let pos_pre_prop = unsafe { (s.archive_tell)(pact_ar) };
            unsafe { (s.actor_serialize_persistence)(pact_ar, act) };
            if unsafe { get_error(s, pact_ar) } {
                debugf(
                    s,
                    NAME_ERROR,
                    &text(
                        "ULevelBase::SavePersistentActors ERROR: Saving tagged properties of actor!",
                    ),
                );
                break;
            }
            let pos_post_prop = unsafe { (s.archive_tell)(pact_ar) };

            // Block 3.2i) Real size of the tagged properties, written back at pos_size.
            unsafe { (s.archive_seek)(pact_ar, pos_size) };
            i_to_save = pos_post_prop - pos_pre_prop;
            unsafe { ser_int(s, pact_ar, &mut i_to_save) };
            unsafe { (s.archive_seek)(pact_ar, pos_post_prop) };
            if unsafe { get_error(s, pact_ar) } {
                debugf(
                    s,
                    NAME_ERROR,
                    &text(
                        "ULevelBase::SavePersistentActors ERROR: Saving real size of taggedProperties!",
                    ),
                );
                break;
            }
        }
        if b_wire_error {
            break 'body;
        }

        // Block 4.) EOF token.
        if !unsafe { write_wire_string(s, pact_ar, &text("EOF")) } {
            debugf(
                s,
                NAME_ERROR,
                &text("ULevelBase::SavePersistentActors ERROR: Saving EOF token as UTF-16LE!"),
            );
            b_error = true;
            break 'body;
        }

        debugf(
            s,
            NAME_LOG,
            &fmt_s(
                "*** Finished Saving Persistent Actors for map ",
                &final_name,
                "",
            ),
        );
    }

    // *** clean up and return
    if !pact_ar.is_null() {
        if unsafe { get_error(s, pact_ar) } {
            b_error = true;
        }
        unsafe { (s.archive_close)(pact_ar) };
    }

    b_error
}

/// `ULevel::LoadPersistentActors(MapName)`. Returns the `bool` result.
///
/// # Safety
/// `level` is a live `ULevel*`; `map_name` is a NUL-terminated `TCHAR` string.
pub unsafe fn load_persistent_actors(
    s: &Hp2LevelPersistServices,
    level: *mut c_void,
    map_name: *const u32,
) -> bool {
    // Make sure we are NOT in the editor.
    if unsafe { (s.is_editor)() } != 0 {
        return false;
    }
    let map_name = unsafe { c_text(map_name) };
    let (final_name, map_filename, pa_filename) = build_filenames(s, &map_name);

    let mut b_error = false;
    let mut b_wire_error = false;
    let pact_ar: *mut c_void;
    let actors_to_load = unsafe { (s.load_map_new)() };

    'body: {
        // Open our actors file (if we have one).
        pact_ar = unsafe { (s.archive_reader_create)(z(&pa_filename).as_ptr()) };
        if pact_ar.is_null()
            || unsafe { get_error(s, pact_ar) }
            || unsafe { (s.archive_total_size)(pact_ar) } == 0
        {
            debugf(
                s,
                NAME_LOG,
                &fmt_s(
                    "*** Could not load Persistent Actor file ",
                    &pa_filename,
                    " (file is probably missing).",
                ),
            );
            break 'body;
        }

        debugf(
            s,
            NAME_LOG,
            &fmt_s(
                "*** Starting to Load Persistent Actors for map ",
                &final_name,
                "",
            ),
        );

        // Create a map of persistent actors we will need to load.
        let mut i_to_load = 0i32;
        let num = unsafe { (s.level_actors_num)(level) };
        for i in 0..num {
            let act = unsafe { (s.level_actor_at)(level, i) };
            if !act.is_null() && unsafe { (s.actor_b_persistent)(act) } != 0 {
                let name = unsafe { actor_name(s, act) };
                unsafe { (s.load_map_add)(actors_to_load, z(&name).as_ptr(), act) };
                i_to_load += 1;
            }
        }

        // If we have no persistent actors to load then break out.
        if i_to_load == 0 {
            break 'body;
        }

        // Block 1a.) PA file identifier.
        if !unsafe { read_identifier(s, pact_ar) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::LoadPersistentActors ERROR: Invalid or truncated PA file identifier!",
                ),
            );
            b_error = true;
            break 'body;
        }

        // Block 1b.) Map name the archive affects.
        let Some(str_name) = (unsafe { read_wire_string(s, pact_ar) }) else {
            debugf(
                s,
                NAME_ERROR,
                &text("ULevelBase::LoadPersistentActors ERROR: Invalid map name UTF-16LE string!"),
            );
            b_error = true;
            break 'body;
        };

        // Error check the map name: FinalMapName != FString(strName).
        if unsafe { (s.fstring_ne)(z(&final_name).as_ptr(), z(&str_name).as_ptr()) } != 0 {
            debugf(
                s,
                NAME_ERROR,
                &text("ULevelBase::LoadPersistentActors ERROR: FinalMapName != Saved map name!"),
            );
            break 'body;
        }

        // Block 1c.) Level file size, compared against iToLoad (as the original does).
        unsafe { ser_int(s, pact_ar, &mut i_to_load) };
        if i_to_load != unsafe { (s.file_manager_file_size)(z(&map_filename).as_ptr()) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::LoadPersistentActors ERROR: Loading map FileSize that this PA file is for!",
                ),
            );
            break 'body;
        }

        // Block 1d.) Level file time.
        let mut file_time = 0i64;
        unsafe { ser_sqword(s, pact_ar, &mut file_time) };
        if file_time != unsafe { (s.file_manager_global_time)(z(&map_filename).as_ptr()) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::LoadPersistentActors ERROR: Saving map FileTime that this PA file is for!",
                ),
            );
            break 'body;
        }

        // Block 2.) Number of actors in the file.
        let mut i_pactors_in_file = 0i32;
        unsafe { ser_int(s, pact_ar, &mut i_pactors_in_file) };
        if unsafe { get_error(s, pact_ar) } {
            debugf(
                s,
                NAME_ERROR,
                &text(
                    "ULevelBase::LoadPersistentActors ERROR: Loading total number of persistent actors!",
                ),
            );
            break 'body;
        }

        // Block 3.) Sync each actor in the file with the level.
        for _ in 0..i_pactors_in_file {
            // Block 3.1i) Name.
            let Some(str_name) = (unsafe { read_wire_string(s, pact_ar) }) else {
                debugf(
                    s,
                    NAME_ERROR,
                    &text(
                        "ULevelBase::LoadPersistentActors ERROR: Invalid actor name UTF-16LE string!",
                    ),
                );
                b_error = true;
                b_wire_error = true;
                break;
            };

            // Block 3.2i) Size of this actor's tagged properties.
            let mut i_prop_size = 0i32;
            unsafe { ser_int(s, pact_ar, &mut i_prop_size) };
            if unsafe { get_error(s, pact_ar) } {
                debugf(
                    s,
                    NAME_ERROR,
                    &text(
                        "ULevelBase::LoadPersistentActors ERROR: Loading size of tagged properties!",
                    ),
                );
                break;
            }

            let act = unsafe { (s.load_map_find)(actors_to_load, z(&str_name).as_ptr()) };
            if act.is_null() {
                // Case 3: in the PA file but not in the level. Skip its properties.
                debugf(s, NAME_LOG, &fmt_actor_missing(&final_name, &pa_filename));
                let pos = unsafe { (s.archive_tell)(pact_ar) };
                unsafe { (s.archive_seek)(pact_ar, pos + i_prop_size) };
                continue;
            }

            // Names must match.
            if unsafe { (s.app_strcmp)(z(&actor_name(s, act)).as_ptr(), z(&str_name).as_ptr()) }
                != 0
            {
                debugf(
                    s,
                    NAME_ERROR,
                    &text("Level actor's name != persistent actor's name!!!!!"),
                );
                break;
            }

            // Case 1: in both. Remove from the to-load map and override properties.
            unsafe { (s.load_map_remove)(actors_to_load, z(&str_name).as_ptr()) };
            unsafe { (s.actor_serialize_persistence)(pact_ar, act) };
            if unsafe { get_error(s, pact_ar) } {
                debugf(
                    s,
                    NAME_ERROR,
                    &text("ULevelBase::LoadPersistentActors ERROR: Loading tagged properties!"),
                );
                break;
            }

            // bScriptInitialized = false so the actor is placed in its collision zone at LoadMap.
            unsafe { (s.actor_set_b_script_initialized)(act, 0) };
        }
        // Quirk: IF_TRUE_MSG_AND_BREAK inside the loop only exits the loop.
        // Wire errors leave bWireError set and stop here.
        if b_wire_error {
            break 'body;
        }

        // Block 4.) EOF token.
        let Some(str_name) = (unsafe { read_wire_string(s, pact_ar) }) else {
            debugf(
                s,
                NAME_ERROR,
                &text("ULevelBase::LoadPersistentActors ERROR: Invalid EOF UTF-16LE string!"),
            );
            b_error = true;
            break 'body;
        };
        if unsafe { (s.app_strcmp)(z(&text("EOF")).as_ptr(), z(&str_name).as_ptr()) } != 0 {
            debugf(
                s,
                NAME_ERROR,
                &text("The EOF token is corrupt in the persistent actor file!!!!!"),
            );
            break 'body;
        }

        // Case 2: in the level but not in the file. Destroy, in TMultiMap order.
        let it = unsafe { (s.load_map_iter_begin)(actors_to_load) };
        while unsafe { (s.load_map_iter_valid)(it) } != 0 {
            let act = unsafe { (s.load_map_iter_value)(it) };
            if !act.is_null() {
                unsafe { (s.actor_destroy)(level, act) };
            }
            unsafe { (s.load_map_iter_next)(it) };
        }
        unsafe { (s.load_map_iter_end)(it) };

        debugf(
            s,
            NAME_LOG,
            &fmt_s(
                "*** Finished Loading Persistent Actors for map ",
                &final_name,
                "",
            ),
        );
    }

    // *** clean up and return
    unsafe { (s.load_map_delete)(actors_to_load) };
    if !pact_ar.is_null() {
        if unsafe { get_error(s, pact_ar) } {
            b_error = true;
        }
        unsafe { (s.archive_close)(pact_ar) };
    }
    b_error
}

/// Reads a NUL-terminated `TCHAR` string into a `Text` (no NUL).
///
/// # Safety
/// `p` points to a NUL-terminated `u32` string.
unsafe fn c_text(p: *const u32) -> Text {
    let mut v = Vec::new();
    let mut i = 0usize;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            return v;
        }
        v.push(c);
        i += 1;
    }
}

/// `"<prefix>%s<suffix>"` with one `%s` argument.
fn fmt_s(prefix: &str, arg: &[u32], suffix: &str) -> Text {
    let mut v = text(prefix);
    v.extend_from_slice(arg);
    v.extend(text(suffix));
    v
}

/// `"*** Actor is NOT in level %s but is in pa file %s! \n. "`.
fn fmt_actor_missing(level_name: &[u32], pa_filename: &[u32]) -> Text {
    let mut v = text("*** Actor is NOT in level ");
    v.extend_from_slice(level_name);
    v.extend(text(" but is in pa file "));
    v.extend_from_slice(pa_filename);
    v.extend(text("! \n. "));
    v
}
