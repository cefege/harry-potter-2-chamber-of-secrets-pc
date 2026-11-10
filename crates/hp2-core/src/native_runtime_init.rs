//! Native owner for the shared post-platform save/map/cache initialization policy.
use crate::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_records::SystemRecord,
    native_path,
    runtime_init::{self, Hp2RuntimeLeaf as L, RuntimePathServices},
    unix_file_manager_owner,
};
use hp2_abi::engine::generated::TArrayRaw;
use std::{cell::{Cell, RefCell}, ffi::c_void, path::{Path, PathBuf}, ptr};

struct NativePaths<'a> {
    access: CoreAccess<'a>,
    system: *mut SystemRecord,
    files: RefCell<Vec<Vec<u32>>>,
    filename: RefCell<Vec<u32>>,
    failure: RefCell<Option<RuntimeError>>,
    parsed_slot: Cell<Option<i32>>,
}

impl<'a> NativePaths<'a> {
    fn new(access: CoreAccess<'a>, system: *mut SystemRecord) -> Self {
        Self {
            access, system, files: RefCell::new(Vec::new()), filename: RefCell::new(Vec::new()),
            failure: RefCell::new(None), parsed_slot: Cell::new(None),
        }
    }
    fn finish(&self, result: Result<(), i32>) -> RuntimeResult<()> {
        result.map_err(|status| self.failure.borrow_mut().take().unwrap_or_else(|| {
            RuntimeError::new("Canonical save/map/cache operation failed").with_context(&status.to_string())
        }))
    }
    fn text(&self, field: *const TArrayRaw) -> RuntimeResult<&[u32]> {
        let value = unsafe { field.read_unaligned() };
        if value.num == 0 { return Ok(&[0]); }
        if value.num < 0 || value.max < value.num || value.data.is_null() {
            return Err(RuntimeError::new("Invalid runtime path FString storage"));
        }
        Ok(unsafe { std::slice::from_raw_parts(value.data.cast(), value.num as usize) })
    }
    fn save(&self) -> RuntimeResult<&[u32]> { self.text(unsafe { ptr::addr_of!((*self.system).save_path) }) }
    fn slot(&self) -> RuntimeResult<&[u32]> { self.text(unsafe { ptr::addr_of!((*self.system).save_slot_path) }) }
    fn map(&self) -> RuntimeResult<&[u32]> { self.text(unsafe { ptr::addr_of!((*self.system).map_path) }) }
    fn set_text(&self, field: *mut TArrayRaw, value: &[u32]) -> RuntimeResult<()> {
        unsafe { self.access.assign_fstring(field, value) }
    }
    fn path(&self, units: &[u32]) -> RuntimeResult<PathBuf> {
        native_path::path_from_units(units)
    }
    fn path_units(path: &Path) -> RuntimeResult<Vec<u32>> {
        let mut units = native_path::path_to_units(path)?;
        units.push(0);
        Ok(units)
    }
    fn joined(&self, base: &[u32], leaf: &[u32]) -> RuntimeResult<Vec<u32>> {
        let mut path = self.path(base)?;
        let text = native_path::path_from_units(leaf)?;
        path.push(text);
        Self::path_units(&path)
    }
    fn set_save(&self, value: &[u32]) -> RuntimeResult<()> {
        self.set_text(unsafe { ptr::addr_of_mut!((*self.system).save_path) }, value)
    }
    fn set_slot(&self, value: &[u32]) -> RuntimeResult<()> {
        self.set_text(unsafe { ptr::addr_of_mut!((*self.system).save_slot_path) }, value)
    }
    fn current_filename(&self) -> RuntimeResult<PathBuf> { self.path(&self.filename.borrow()) }
    fn cache_path(&self, location: i32) -> RuntimeResult<PathBuf> {
        let path = self.path(self.slot()?)?;
        Ok(if location != 0 { path.join("cache") } else { path })
    }
    fn fail(&self, error: RuntimeError) -> i32 {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() { *failure = Some(error); }
        -1
    }
}

impl RuntimePathServices for NativePaths<'_> {
    fn call(&self, leaf: L, a: i32, b: i32) -> Result<i64, i32> {
        let result: RuntimeResult<i64> = (|| {
            use L::*;
            let paths = unsafe { ptr::read_unaligned(ptr::addr_of!((*self.system).paths)) };
            match leaf {
                HasUserDirectory => Ok(i64::from(!crate::unix_identity::user_dir().is_null() && unsafe { *crate::unix_identity::user_dir() != 0 })),
                UserSavePath => {
                    let user = unsafe { crate::unix_identity::user_dir() };
                    let mut n = 0;
                    while unsafe { user.add(n).read() } != 0 { n += 1; }
                    let root = self.path(unsafe { std::slice::from_raw_parts(user.cast::<u32>(), n + 1) })?;
                    let save = Self::path_units(&root.join("Save"))?;
                    self.set_save(&save)?;
                    Ok(0)
                }
                PathCount => Ok(i64::from(paths.num)),
                MissingPaths => Err(RuntimeError::new("Core.System Paths are unavailable")),
                PublishSaveGlob => {
                    if paths.num < 6 || paths.data.is_null() { return Err(RuntimeError::new("Core.System Paths did not load from System.ini")); }
                    let glob = self.joined(self.save()?, crate::lit!("*.usa"))?;
                    self.set_text(unsafe { paths.data.cast::<TArrayRaw>().add(5) }, &glob)?;
                    Ok(0)
                }
                LocalSavePath => {
                    self.set_save(crate::lit!("..\\Save"))?;
                    Ok(0)
                }
                MakeSaveDirectory => {
                    unix_file_manager_owner::make_directory(self.access, &self.path(self.save()?)?)?;
                    Ok(0)
                }
                ParseSaveSlot => {
                    let needle = crate::lit!("SAVESLOT=");
                    let mut slot = 0;
                    let found = unsafe { crate::parse_integer::parse_int_with(
                        self.access.command_line().as_ptr().cast(), needle.as_ptr().cast(), &mut slot,
                        |_| Ok::<_, RuntimeError>(needle.len() - 1),
                        |text| crate::ansi::atoi_checked(text).map_err(|_| RuntimeError::new("Save slot exceeds the native INT range")),
                    ) }?;
                    if found { self.parsed_slot.set(Some(slot)); }
                    Ok(i64::from(found))
                }
                SetSaveSlot => {
                    let slot = self.parsed_slot.take().ok_or_else(|| RuntimeError::new("Missing parsed save slot"))?;
                    let mut leaf: Vec<u32> = format!("Slot{slot}").chars().map(u32::from).collect(); leaf.push(0);
                    let slot_path = self.joined(self.save()?, &leaf)?;
                    self.set_slot(&slot_path)?;
                    Ok(0)
                }
                DefaultSaveSlot => {
                    self.set_slot(self.save()?)?;
                    Ok(0)
                }
                HasSaveSlot => Ok(i64::from(self.slot()?[0] != 0)),
                LogSaveSlot => {
                    let mut message: Vec<u32> = "Save Slot Path: ".chars().map(u32::from).collect();
                    message.extend(self.slot()?.iter().copied().take_while(|unit| *unit != 0));
                    let category = self.access.intern_str("Log", crate::name_owner::FindName::Add)?;
                    self.access.emit_log(category, &message)?;
                    Ok(0)
                }
                MakeSlotDirectory => { unix_file_manager_owner::make_directory(self.access, &self.path(self.slot()?)?)?; Ok(0) }
                MakeCacheDirectory => { unix_file_manager_owner::make_directory(self.access, &self.cache_path(1)?)?; Ok(0) }
                IsMapPath => {
                    if a < 0 || a >= paths.num { return Err(RuntimeError::new("System.Paths index is out of range")); }
                    let units = self.text(unsafe { paths.data.cast::<TArrayRaw>().add(a as usize) })?;
                    let text = &units[..units.len().saturating_sub(1)];
                    Ok(i64::from(text.len() >= 4 && text[text.len() - 4..].iter().zip(b".unr").all(|(&unit, &byte)| unit == u32::from(byte) || (byte.is_ascii_lowercase() && unit == u32::from(byte - 32)))))
                }
                PathLength => {
                    let units = self.text(unsafe { paths.data.cast::<TArrayRaw>().add(a as usize) })?;
                    Ok(units.len().saturating_sub(1) as i64)
                }
                PathCharacter => {
                    let units = self.text(unsafe { paths.data.cast::<TArrayRaw>().add(a as usize) })?;
                    Ok(units.get(b as usize).copied().unwrap_or(0) as i64)
                }
                SetMapBase => {
                    let services = crate::unix_text::Hp2UnixTextServices {
                        opaque: ptr::null_mut(), allocate: allocate_text, fatal: text_failure,
                    };
                    let base = unsafe { crate::unix_identity::base_dir(&services) }
                        .map_err(|_| RuntimeError::new("appBaseDir initialization failed"))?;
                    let mut count = 0;
                    while unsafe { *base.add(count) } != 0 { count += 1; }
                    let text = unsafe { std::slice::from_raw_parts(base.cast::<u32>(), count + 1) };
                    self.set_text(unsafe { ptr::addr_of_mut!((*self.system).map_path) }, text)?;
                    Ok(0)
                }
                MapBaseSystemOffset => {
                    let map = self.map()?;
                    let found = unsafe { crate::ansi::strstr(map.as_ptr().cast(), crate::lit!("System").as_ptr().cast()) };
                    Ok(if found.is_null() { -1 } else { (unsafe { found.offset_from(map.as_ptr().cast()) }) as i64 })
                }
                TrimMapBase => {
                    let text = self.map()?;
                    let count = (a.max(0) as usize).min(text.len().saturating_sub(1));
                    let mut map = Vec::with_capacity(count + 1);
                    map.extend_from_slice(&text[..count]); map.push(0);
                    self.set_text(unsafe { ptr::addr_of_mut!((*self.system).map_path) }, &map)?;
                    Ok(0)
                }
                AppendMapSuffix => {
                    let units = self.text(unsafe { paths.data.cast::<TArrayRaw>().add(a as usize) })?;
                    let end = b.max(0) as usize;
                    let suffix = units.get(end.saturating_sub(4)..end).unwrap_or(&[]);
                    let text = self.map()?;
                    let mut map = Vec::with_capacity(text.len() + suffix.len());
                    map.extend_from_slice(&text[..text.len().saturating_sub(1)]);
                    map.extend_from_slice(suffix); map.push(0);
                    self.set_text(unsafe { ptr::addr_of_mut!((*self.system).map_path) }, &map)?;
                    Ok(0)
                }
                LoadCacheCall => { runtime_init::load_persistent_cache(self).map_err(|e| RuntimeError::new(&format!("Persistent actor cache load failed ({e})")))?; Ok(0) }
                ConfigSavePath => { self.access.config_set_units(crate::lit!("Core.System"), crate::lit!("SavePath"), self.slot()?)?; Ok(0) }
                ConfigSaveGlob => {
                    let glob = self.joined(self.slot()?, crate::lit!("*.usa"))?;
                    self.access.config_set_units(crate::lit!("Core.System"), crate::lit!("paths"), &glob)?; Ok(0)
                }
                ConfigFlush => { self.access.config_flush_units_from(None, false)?; Ok(0) }
                CacheFind => {
                    let folder = self.cache_path(a)?;
                    let pattern = folder.join(if b != 0 { "*.*" } else { "*_pa.*" });
                    *self.files.borrow_mut() = unix_file_manager_owner::find_files(self.access, &pattern)?;
                    Ok(0)
                }
                CacheCount => Ok(self.files.borrow().len() as i64),
                CacheFilename => {
                    let path = {
                        let files = self.files.borrow();
                        let name = files.get(b as usize).ok_or_else(|| RuntimeError::new("Cache filename index out of range"))?;
                        self.cache_path(a)?.join(self.path(name)?)
                    };
                    *self.filename.borrow_mut() = Self::path_units(&path)?;
                    Ok(0)
                }
                CacheSize => Ok(i64::from(unix_file_manager_owner::file_size(self.access, &self.current_filename()?)?)),
                CacheDelete => { let _ = unix_file_manager_owner::delete_file_path(self.access, &self.current_filename()?, true)?; Ok(0) }
                CacheCopy => {
                    let source = self.current_filename()?;
                    let destination = {
                        let files = self.files.borrow();
                        let name = files.get(b as usize).ok_or_else(|| RuntimeError::new("Cache filename index out of range"))?;
                        self.cache_path(a)?.join(self.path(name)?)
                    };
                    let _ = unix_file_manager_owner::copy_file_paths(self.access, &destination, &source, true)?;
                    Ok(0)
                }
                CacheEmpty => { self.files.borrow_mut().clear(); Ok(0) }
                _ => Err(RuntimeError::new("Unsupported native runtime path operation")),
            }
        })();
        result.map_err(|error| { self.fail(error) })
    }
}

unsafe extern "C" fn allocate_text(_opaque: *mut c_void, size: usize, out: *mut *mut c_void) -> i32 {
    unsafe { out.write(crate::unix_alloc::system_malloc(size)); }
    0
}

unsafe extern "C" fn text_failure(_opaque: *mut c_void, _failure: crate::unix_text::Hp2UnixTextFailure) -> i32 {
    1
}


/// Runs the shared save-slot, map-path, cache-load and configuration policy.
///
/// # Safety
/// `system` must be the live, initialized `Core.System` record owned by the same
/// Core graph as `access`, including valid `Paths`, `SavePath`, `SaveSlotPath` and
/// `MapPath` array storage. The record and that storage must stay valid and must
/// not be mutated by other code until this function returns. Native leaves may
/// reenter Core through `access`.
pub unsafe fn initialize(access: CoreAccess<'_>, system: *mut SystemRecord) -> RuntimeResult<()> {
    let owner = NativePaths::new(access, system);
    owner.finish(runtime_init::initialize_paths(&owner))
}

pub fn save_persistent_cache(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let owner = NativePaths::new(access, access.system()?);
    owner.finish(runtime_init::save_persistent_cache(&owner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_access::{CoreOptions, CoreOwner};

    #[test]
    fn real_cache_transactions_copy_in_both_directions_and_preserve_zero_size_rules() {
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("runtime-cache-{}", std::process::id()));
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(root.join("System/Default.ini"), "[Core.System]\n").unwrap();
        std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
        let slot = root.join("slot");
        let cache = slot.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(slot.join("old_pa.usa"), b"obsolete").unwrap();
        std::fs::write(slot.join("empty_pa.usa"), b"").unwrap();
        std::fs::write(slot.join("keep.txt"), b"unrelated slot file").unwrap();
        std::fs::write(cache.join("current_pa.usa"), b"current persistent actor").unwrap();
        std::fs::write(cache.join("stale.txt"), b"discard on load, retain on save").unwrap();
        let owner = CoreOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(),
            output: |_, _, _| Ok(()), log_file: None,
        }).unwrap();
        let access = owner.access();
        let system = access.system().unwrap();
        let slot_text = NativePaths::path_units(&slot).unwrap();
        unsafe { access.assign_fstring(ptr::addr_of_mut!((*system).save_slot_path), &slot_text).unwrap(); }
        save_persistent_cache(access).unwrap();
        assert_eq!(std::fs::read(slot.join("current_pa.usa")).unwrap(), b"current persistent actor");
        assert!(!slot.join("old_pa.usa").exists());
        assert_eq!(std::fs::metadata(slot.join("empty_pa.usa")).unwrap().len(), 0);
        assert_eq!(std::fs::read(slot.join("keep.txt")).unwrap(), b"unrelated slot file");
        assert!(cache.join("stale.txt").exists());

        std::fs::write(slot.join("current_pa.usa"), b"newer slot state").unwrap();
        let paths = NativePaths::new(access, system);
        paths.finish(runtime_init::load_persistent_cache(&paths)).unwrap();
        assert_eq!(std::fs::read(cache.join("current_pa.usa")).unwrap(), b"newer slot state");
        assert!(!cache.join("stale.txt").exists());
        assert!(!cache.join("empty_pa.usa").exists(), "native FileSize boolean conversion skips zero-size copies");
        assert_eq!(std::fs::read(slot.join("keep.txt")).unwrap(), b"unrelated slot file");
        owner.shutdown().unwrap();
    }
}
