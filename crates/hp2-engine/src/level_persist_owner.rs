//! Canonical Core adapter for persistent level actor files.
use crate::level_persist::{decode_wire, encode_wire, WIRE_CAPACITY};
use hp2_abi::engine::generated::{AActor, TArrayRaw, ULevel, UObject, UClass, UStruct};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
    object_archive::{ArchiveFlags, ArchiveMapping, ArchiveMode, BorrowedArchiveCursor, ObjectArchive},
    runtime_archive::{RuntimeArchive, RuntimeNameMapping},
};
use std::{cell::RefCell, fs, path::{Path, PathBuf}, ptr, time::UNIX_EPOCH};

const RF_DEBUG_SERIALIZE: u32 = 0x4000_0000;
fn archive_flags() -> ArchiveFlags {
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    ArchiveFlags::new(constants.version as u16, constants.licensee as u16, true)
}

struct Names<'a>(CoreAccess<'a>);

impl RuntimeNameMapping for Names<'_> {
    fn name_text(&self, name_index: i32) -> Result<Vec<u32>, i32> {
        self.0.name_text(name_index).map_err(|_| 1)
    }

    fn add_name(&self, text: &[u32]) -> Result<i32, i32> {
        self.0.intern(text, FindName::Add).map_err(|_| 1)
    }
}

struct FileCursor<'a> {
    archive: RefCell<RuntimeArchive>,
    names: &'a dyn RuntimeNameMapping,
}

impl BorrowedArchiveCursor for FileCursor<'_> {
    fn tell(&self) -> RuntimeResult<usize> {
        let value = self.archive.borrow_mut().tell().map_err(archive_error)?;
        usize::try_from(value).map_err(|_| RuntimeError::new("Negative persistent archive position"))
    }
    fn seek(&self, position: usize) -> RuntimeResult<()> {
        let position = i32::try_from(position).map_err(|_| RuntimeError::new("Persistent archive position exceeds INT"))?;
        self.archive.borrow_mut().seek(position).map_err(archive_error)
    }
    fn total_size(&self) -> RuntimeResult<usize> {
        let value = self.archive.borrow_mut().total_size().map_err(archive_error)?;
        usize::try_from(value).map_err(|_| RuntimeError::new("Negative persistent archive size"))
    }
    fn is_error(&self) -> RuntimeResult<bool> {
        Ok(self.archive.borrow().error().is_some())
    }
    fn read(&self, output: &mut [u8]) -> RuntimeResult<()> {
        let len = i32::try_from(output.len()).map_err(|_| RuntimeError::new("Persistent archive read exceeds INT"))?;
        let mut archive = self.archive.borrow_mut();
        let status = unsafe { archive.serialize(output.as_mut_ptr().cast(), len) };
        if status == 0 || archive.error().is_some() { Ok(()) } else { Err(archive_error(status)) }
    }
    fn write(&self, input: &[u8]) -> RuntimeResult<()> {
        let len = i32::try_from(input.len()).map_err(|_| RuntimeError::new("Persistent archive write exceeds INT"))?;
        let mut archive = self.archive.borrow_mut();
        let status = unsafe { archive.serialize(input.as_ptr().cast_mut().cast(), len) };
        if status == 0 || archive.error().is_some() { Ok(()) } else { Err(archive_error(status)) }
    }
    unsafe fn serialize_name(&self, name_index: *mut i32) -> RuntimeResult<()> {
        let status = unsafe { self.archive.borrow_mut().serialize_name_with(name_index, self.names) };
        status.map_err(|code| archive_error(code))
    }
}

fn archive_error(code: i32) -> RuntimeError {
    RuntimeError::new("Persistent actor archive operation failed").with_context(&code.to_string())
}

struct Mapping<'a> { access: CoreAccess<'a> }
impl ArchiveMapping for Mapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 { return Ok(ptr::null_mut()); }
        let object = self.access.object_at(index - 1);
        if object.is_null() { return Err(RuntimeError::new("Invalid persistent actor object index")); }
        Ok(object)
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { return Ok(0); }
        let index = unsafe { (*object).index };
        index.checked_add(1).ok_or_else(|| RuntimeError::new("Persistent object index overflow"))
    }
    fn load_name(&self, local_index: i32) -> RuntimeResult<i32> { Ok(local_index) }
    fn save_name(&self, global_index: i32) -> RuntimeResult<i32> { Ok(global_index) }
    fn visit_object(&self, _object_slot: *mut *mut UObject) -> RuntimeResult<()> { Ok(()) }
    fn visit_name(&self, _name_index: i32) -> RuntimeResult<()> { Ok(()) }
}

fn system_text(array: TArrayRaw, field: &str) -> RuntimeResult<String> {
    if array.num < 0 || (array.num > 0 && array.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical Engine path").with_context(field));
    }
    let text: String = if array.num == 0 { String::new() } else {
        unsafe { std::slice::from_raw_parts(array.data.cast::<u32>(), array.num as usize) }
            .iter().copied().take_while(|&unit| unit != 0)
            .map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid Engine path character")))
            .collect::<RuntimeResult<_>>()?
    };
    Ok(text)
}

fn resolve_engine_path(access: CoreAccess<'_>, value: TArrayRaw, field: &str) -> RuntimeResult<PathBuf> {
    let path = PathBuf::from(system_text(value, field)?);
    Ok(if path.is_absolute() { path } else { access.data_root().join("System").join(path) })
}

struct PersistPaths { final_name: Vec<u32>, map: PathBuf, pa: PathBuf }
fn paths(access: CoreAccess<'_>, map_name: *const u32) -> RuntimeResult<PersistPaths> {
    if map_name.is_null() { return Err(RuntimeError::new("Null persistent actor map name")); }
    let system = access.system()?;
    let map_root = resolve_engine_path(access, unsafe { ptr::addr_of!((*system).map_path).read_unaligned() }, "MapPath")?;
    let slot_root = resolve_engine_path(access, unsafe { ptr::addr_of!((*system).save_slot_path).read_unaligned() }, "SaveSlotPath")?;
    let mut raw = String::new();
    let mut cursor = map_name;
    loop {
        let unit = unsafe { cursor.read_unaligned() };
        if unit == 0 { break; }
        raw.push(char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid persistent map TCHAR"))?);
        cursor = unsafe { cursor.add(1) };
    }
    let leaf = raw.rsplit(|character| character == '/' || character == '\\').next()
        .filter(|leaf| !leaf.is_empty())
        .ok_or_else(|| RuntimeError::new("Invalid persistent map filename"))?;
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let map_ext = string_units(defaults.default_map_ext())?;
    let save_ext = string_units(defaults.default_save_ext())?;
    let map_ext_units: Vec<u32> = map_ext.chars().map(u32::from).collect();
    let mut final_name = leaf.to_owned();
    let mut final_units: Vec<u32> = final_name.chars().map(u32::from).collect();
    if find_units(&final_units, &map_ext_units).is_none() {
        final_name.push('.');
        final_name.push_str(&map_ext);
        final_units = final_name.chars().map(u32::from).collect();
    }
    let extension_at = find_units(&final_units, &map_ext_units)
        .ok_or_else(|| RuntimeError::new("Persistent map extension missing"))?;
    let stem_end = extension_at.saturating_sub(1);
    let pa_stem = final_units[..stem_end].iter().map(|&unit| {
        char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid persistent map filename"))
    }).collect::<RuntimeResult<String>>()?;
    let pa_name = format!("{pa_stem}_pa.{save_ext}");
    Ok(PersistPaths { final_name: final_units, map: map_root.join(&final_name), pa: slot_root.join("cache").join(pa_name) })
}
fn find_units(haystack: &[u32], needle: &[u32]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    haystack.windows(needle.len()).position(|window| window == needle)
}


fn file_size(path: &Path) -> i32 {
    fs::metadata(path).ok().and_then(|metadata| i32::try_from(metadata.len()).ok()).unwrap_or(-1)
}
fn global_time(path: &Path) -> i64 {
    fs::metadata(path).ok().and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok()).and_then(|duration| i64::try_from(duration.as_secs()).ok()).unwrap_or(0)
}
fn string_units(text: &[u32]) -> RuntimeResult<String> {
    text.iter().take_while(|&&unit| unit != 0).map(|&unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid persistent actor TCHAR"))).collect()
}
fn name_units(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<Vec<u32>> {
    if actor.is_null() { return Err(RuntimeError::new("Null persistent actor")); }
    let index = i32::from_ne_bytes(unsafe { (*actor.cast::<UObject>()).name });
    access.name_text(index)
}
fn persistent(actor: *mut AActor) -> bool { !actor.is_null() && unsafe { (*actor).b_persistent() } }


unsafe fn serialize_persistence(access: CoreAccess<'_>, actor: *mut AActor, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    if actor.is_null() { return Err(RuntimeError::new("Null persistent actor")); }
    let object = actor.cast::<UObject>();
    unsafe { (*object).object_flags |= RF_DEBUG_SERIALIZE; }
    let class = unsafe { (*object).class.cast::<UClass>() };
    if class.is_null() { return Err(RuntimeError::new("Persistent actor has no class")); }
    unsafe { access.preload(class.cast())?; }
    if !archive.is_trans() {
        let defaults = unsafe { access.default_object(class)? };
        let size = unsafe { (*class.cast::<UStruct>()).properties_size };
        unsafe { access.tagged_properties(class.cast(), archive, object.cast(), defaults.cast(), size)?; }
    } else {
        unsafe { access.binary_properties(class.cast(), archive, object.cast())?; }
    }
    let size = unsafe { (*class.cast::<UStruct>()).properties_size.max(0) as usize };
    archive.count_bytes(size, size)
}

fn utf16(text: &[u32]) -> RuntimeResult<[u16; WIRE_CAPACITY]> {
    let value = string_units(text)?;
    let mut scalars = value.chars().map(u32::from).collect::<Vec<_>>();
    scalars.push(0);
    let mut wire = [0u16; WIRE_CAPACITY];
    let written = unsafe { encode_wire(scalars.as_ptr(), &mut wire) }
        .ok_or_else(|| RuntimeError::new("Persistent actor string exceeds wire capacity"))?;
    if written != scalars.len() { return Err(RuntimeError::new("Persistent actor UTF-16 conversion mismatch")); }
    Ok(wire)
}
fn write_i32(cursor: &FileCursor<'_>, value: &mut i32) -> RuntimeResult<()> {
    cursor.write(unsafe { std::slice::from_raw_parts((value as *mut i32).cast::<u8>(), 4) })
}
fn read_i32(cursor: &FileCursor<'_>, value: &mut i32) -> RuntimeResult<()> {
    cursor.read(unsafe { std::slice::from_raw_parts_mut((value as *mut i32).cast::<u8>(), 4) })
}
fn write_i64(cursor: &FileCursor<'_>, value: &mut i64) -> RuntimeResult<()> {
    cursor.write(unsafe { std::slice::from_raw_parts((value as *mut i64).cast::<u8>(), 8) })
}
fn read_i64(cursor: &FileCursor<'_>, value: &mut i64) -> RuntimeResult<()> {
    cursor.read(unsafe { std::slice::from_raw_parts_mut((value as *mut i64).cast::<u8>(), 8) })
}
fn write_wire(cursor: &FileCursor<'_>, text: &[u32]) -> RuntimeResult<()> {
    let units = utf16(text)?;
    let unit_count = units.iter().position(|&unit| unit == 0).unwrap_or(WIRE_CAPACITY) + 1;
    let mut bytes = i32::try_from(unit_count * 2).map_err(|_| RuntimeError::new("Persistent actor string length overflow"))?;
    write_i32(cursor, &mut bytes)?;
    let bytes = unsafe { std::slice::from_raw_parts(units.as_ptr().cast::<u8>(), unit_count * 2) };
    cursor.write(bytes)
}
fn read_wire(cursor: &FileCursor<'_>) -> RuntimeResult<Option<Vec<u32>>> {
    let mut bytes = 0;
    read_i32(cursor, &mut bytes)?;
    if bytes < 2 || bytes & 1 != 0 || bytes as usize / 2 > WIRE_CAPACITY {
        return Ok(None);
    }
    let mut units = vec![0u16; bytes as usize / 2];
    cursor.read(unsafe { std::slice::from_raw_parts_mut(units.as_mut_ptr().cast::<u8>(), units.len() * 2) })?;
    let Some(decoded) = decode_wire(&units) else { return Ok(None); };
    let count = decoded.iter().position(|&unit| unit == 0).unwrap_or(WIRE_CAPACITY);
    Ok(Some(decoded[..count].to_vec()))
}
fn log(access: CoreAccess<'_>, category: &str, message: &str) -> RuntimeResult<()> {
    let category = access.intern_str(category, FindName::Add)?;
    access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())
}
fn log_code(access: CoreAccess<'_>, code: usize, map: &str, pa: &str) -> RuntimeResult<()> {
    let (category, message) = match code {
        0 => ("Log", format!("*** Could not Save Persistent Actor file {pa}, CreateFileWriter had an error!!!!.")),
        1 => ("Log", format!("*** Starting to Save Persistent Actors for map {map}")),
        2 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving map PA file identifier!".into()),
        3 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving map name as UTF-16LE!".into()),
        4 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving map FileSize that this PA file is for!".into()),
        5 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving map FileTime that this PA file is for!".into()),
        6 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving Number of persistent Actors!".into()),
        7 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving actor name as UTF-16LE!".into()),
        8 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving temp size of taggedProperties!".into()),
        9 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving tagged properties of actor!".into()),
        10 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving real size of taggedProperties!".into()),
        11 => ("Error", "ULevelBase::SavePersistentActors ERROR: Saving EOF token as UTF-16LE!".into()),
        12 => ("Log", format!("*** Finished Saving Persistent Actors for map {map}")),
        13 => ("Log", format!("*** Could not load Persistent Actor file {pa} (file is probably missing).")),
        14 => ("Log", format!("*** Starting to Load Persistent Actors for map {map}")),
        15 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Invalid or truncated PA file identifier!".into()),
        16 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Invalid map name UTF-16LE string!".into()),
        17 => ("Error", "ULevelBase::LoadPersistentActors ERROR: FinalMapName != Saved map name!".into()),
        18 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Loading map FileSize that this PA file is for!".into()),
        19 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Saving map FileTime that this PA file is for!".into()),
        20 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Loading total number of persistent actors!".into()),
        21 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Invalid actor name UTF-16LE string!".into()),
        22 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Loading size of tagged properties!".into()),
        23 => ("Log", format!("*** Actor is NOT in level {map} but is in pa file {pa}! \n. ")),
        24 => ("Error", "Level actor's name != persistent actor's name!!!!!".into()),
        25 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Loading tagged properties!".into()),
        26 => ("Error", "ULevelBase::LoadPersistentActors ERROR: Invalid EOF UTF-16LE string!".into()),
        27 => ("Error", "The EOF token is corrupt in the persistent actor file!!!!!".into()),
        _ => ("Log", format!("*** Finished Loading Persistent Actors for map {map}")),
    };
    log(access, category, &message)
}

/// Saves persistent actors to the source-compatible `_pa.usa` file.
///
/// # Errors
/// Returns a Core, archive, or filesystem error.
///
/// # Safety
/// `level` and `map_name` must refer to live canonical Engine storage for this synchronous call.
pub unsafe fn save_persistent_actors(access: CoreAccess<'_>, level: *mut ULevel, map_name: *const u32) -> RuntimeResult<bool> {
    if level.is_null() { return Err(RuntimeError::new("Null persistent actor level")); }
    if access.process_mode().is_editor { return Ok(false); }
    let paths = paths(access, map_name)?;
    let map_name = string_units(&paths.final_name)?;
    let pa_text = paths.pa.to_string_lossy().into_owned();
    let mut actors = Vec::new();
    let count = crate::level_owner::actor_count(level)?;
    for index in 0..count {
        let actor = crate::level_owner::actor_at(level, index);
        if persistent(actor) { actors.push(actor); }
    }
    if actors.is_empty() { return Ok(false); }
    let archive = match RuntimeArchive::create(&paths.pa) {
        Ok(archive) => archive,
        Err(_) => { log_code(access, 0, &map_name, &pa_text)?; return Ok(false); }
    };
    log_code(access, 1, &map_name, &pa_text)?;
    let names = Names(access);
    let cursor = FileCursor { archive: RefCell::new(archive), names: &names };
    let mapping = Mapping { access };
    let borrowed = ObjectArchive::borrowed(&cursor, ArchiveMode::Saving, archive_flags(), &mapping)?;
    let result = (|| -> RuntimeResult<bool> {
        cursor.write(&[80, 0, 65, 0, 48, 0, 0, 0])?;
        if cursor.is_error()? { log_code(access, 2, &map_name, &pa_text)?; return Ok(true); }
        write_wire(&cursor, &paths.final_name)?;
        if cursor.is_error()? { log_code(access, 3, &map_name, &pa_text)?; return Ok(true); }
        let mut size = file_size(&paths.map);
        write_i32(&cursor, &mut size)?;
        if cursor.is_error()? { log_code(access, 4, &map_name, &pa_text)?; return Ok(true); }
        let mut time = global_time(&paths.map);
        write_i64(&cursor, &mut time)?;
        if cursor.is_error()? { log_code(access, 5, &map_name, &pa_text)?; return Ok(true); }
        let mut actor_count = i32::try_from(actors.len()).map_err(|_| RuntimeError::new("Persistent actor count exceeds INT"))?;
        write_i32(&cursor, &mut actor_count)?;
        if cursor.is_error()? { log_code(access, 6, &map_name, &pa_text)?; return Ok(true); }
        for actor in actors {
            let name = name_units(access, actor)?;
            if write_wire(&cursor, &name).is_err() {
                log_code(access, 7, &map_name, &pa_text)?;
                return Ok(true);
            }
            let size_position = cursor.tell()?;
            let mut size = 0;
            write_i32(&cursor, &mut size)?;
            if cursor.is_error()? { log_code(access, 8, &map_name, &pa_text)?; return Ok(true); }
            let payload_position = cursor.tell()?;
            unsafe { serialize_persistence(access, actor, &borrowed)?; }
            if cursor.is_error()? { log_code(access, 9, &map_name, &pa_text)?; return Ok(true); }
            let payload_end = cursor.tell()?;
            cursor.seek(size_position)?;
            size = i32::try_from(payload_end - payload_position)
                .map_err(|_| RuntimeError::new("Persistent actor property payload exceeds INT"))?;
            write_i32(&cursor, &mut size)?;
            cursor.seek(payload_end)?;
            if cursor.is_error()? { log_code(access, 10, &map_name, &pa_text)?; return Ok(true); }
        }
        write_wire(&cursor, &[69, 79, 70, 0])?;
        if cursor.is_error()? { log_code(access, 11, &map_name, &pa_text)?; return Ok(true); }
        Ok(false)
    })();
    let mut archive = cursor.archive.borrow_mut();
    let close_result = archive.flush().map_err(archive_error).and_then(|_| archive.close().map_err(archive_error));
    let had_error = result?;
    close_result?;
    if !had_error { log_code(access, 12, &map_name, &pa_text)?; }
    Ok(had_error)
}

/// Loads persistent actor properties and destroys unmatched persistent actors in source order.
///
/// # Errors
/// Returns a Core, archive, or filesystem error.
///
/// # Safety
/// `level` and `map_name` must refer to live canonical Engine storage for this synchronous call.
pub unsafe fn load_persistent_actors(access: CoreAccess<'_>, level: *mut ULevel, map_name: *const u32) -> RuntimeResult<bool> {
    if level.is_null() { return Err(RuntimeError::new("Null persistent actor level")); }
    if access.process_mode().is_editor { return Ok(false); }
    let paths = paths(access, map_name)?;
    let map_units: Vec<u32> = paths.final_name.iter().copied().take_while(|&unit| unit != 0).collect();
    let map = string_units(&map_units)?;
    let pa_text = paths.pa.to_string_lossy().into_owned();
    let mut file = match RuntimeArchive::open(&paths.pa) {
        Ok(file) => file,
        Err(_) => { log_code(access, 13, &map, &pa_text)?; return Ok(false); }
    };
    let total_size = match file.total_size() {
        Ok(size) => size,
        Err(_) => {
            file.close().map_err(archive_error)?;
            log_code(access, 13, &map, &pa_text)?;
            return Ok(false);
        }
    };
    if total_size == 0 {
        file.close().map_err(archive_error)?;
        log_code(access, 13, &map, &pa_text)?;
        return Ok(false);
    }
    log_code(access, 14, &map, &pa_text)?;
    let names = Names(access);
    let cursor = FileCursor { archive: RefCell::new(file), names: &names };
    let mapping = Mapping { access };
    let borrowed = match ObjectArchive::borrowed(&cursor, ArchiveMode::Loading, archive_flags(), &mapping) {
        Ok(archive) => archive,
        Err(error) => {
            cursor.archive.borrow_mut().close().map_err(archive_error)?;
            return Err(error);
        }
    };
    let mut pending = Vec::new();
    for index in 0..crate::level_owner::actor_count(level)? {
        let actor = crate::level_owner::actor_at(level, index);
        if persistent(actor) { pending.push((name_units(access, actor)?, actor)); }
    }
    if pending.is_empty() {
        cursor.archive.borrow_mut().close().map_err(archive_error)?;
        return Ok(false);
    }
    let mut error = false;
    let result = (|| -> RuntimeResult<()> {
        let mut identifier = [0u8; 8]; cursor.read(&mut identifier)?;
        if identifier[..4] != [80, 0, 65, 0] || !matches!(identifier[4], 48 | 49) || identifier[5..] != [0, 0, 0] {
            log_code(access, 15, &map, &pa_text)?;
            error = true;
            return Ok(());
        }
        let Some(disk_map) = read_wire(&cursor)? else {
            log_code(access, 16, &map, &pa_text)?;
            error = true;
            return Ok(());
        };
        if disk_map != map_units { log_code(access, 17, &map, &pa_text)?; return Ok(()); }
        let mut level_size = 0; read_i32(&cursor, &mut level_size)?;
        if level_size != file_size(&paths.map) { log_code(access, 18, &map, &pa_text)?; return Ok(()); }
        let mut time = 0; read_i64(&cursor, &mut time)?;
        if time != global_time(&paths.map) { log_code(access, 19, &map, &pa_text)?; return Ok(()); }
        let mut number = 0; read_i32(&cursor, &mut number)?;
        if cursor.is_error()? { log_code(access, 20, &map, &pa_text)?; return Ok(()); }
        for _ in 0..number {
            let Some(name) = read_wire(&cursor)? else {
                log_code(access, 21, &map, &pa_text)?;
                error = true;
                return Ok(());
            };
            let mut size = 0; read_i32(&cursor, &mut size)?;
            if cursor.is_error()? { log_code(access, 22, &map, &pa_text)?; break; }
            let start = cursor.tell()?;
            let name_index = access.find_name(&name).ok();
            let found = name_index.and_then(|index| pending.iter().position(|(_, actor)| unsafe {
                i32::from_ne_bytes((*actor.cast::<UObject>()).name) == index
            }));
            let Some(index) = found else {
                log_code(access, 23, &map, &pa_text)?;
                let target = (start as i64).checked_add(i64::from(size))
                    .and_then(|position| usize::try_from(position).ok())
                    .ok_or_else(|| RuntimeError::new("Invalid persistent actor payload offset"))?;
                cursor.seek(target)?;
                continue;
            };
            if pending[index].0 != name {
                log_code(access, 24, &map, &pa_text)?;
                break;
            }
            let actor = pending.remove(index).1;
            crate::game_load_map_owner::game_state_trace("restore_actor_begin", || {
                format!("level={level:p} actor={actor:p} name_utf32={name:?}")
            });
            let restored = unsafe { serialize_persistence(access, actor, &borrowed) };
            crate::game_load_map_owner::game_state_trace("restore_actor_serialize_end", || {
                format!("level={level:p} actor={actor:p} name_utf32={name:?} returned={}", restored.is_ok())
            });
            restored?;
            if cursor.is_error()? { log_code(access, 25, &map, &pa_text)?; break; }
            unsafe { (*actor).set_b_script_initialized(false); }
            crate::game_load_map_owner::game_state_trace("restore_actor_end", || {
                format!("level={level:p} actor={actor:p} name_utf32={name:?}")
            });
        }
        let Some(eof) = read_wire(&cursor)? else {
            log_code(access, 26, &map, &pa_text)?;
            error = true;
            return Ok(());
        };
        if eof != [69, 79, 70, 0] { log_code(access, 27, &map, &pa_text)?; return Ok(()); }
        for (_, actor) in pending.drain(..) {
            unsafe { crate::level_lifecycle_owner::destroy_actor(access, level, actor, false)?; }
        }
        log_code(access, 28, &map, &pa_text)?;
        Ok(())
    })();
    let mut archive = cursor.archive.borrow_mut();
    let close = archive.close().map_err(archive_error);
    result?;
    close?;
    Ok(error)
}
