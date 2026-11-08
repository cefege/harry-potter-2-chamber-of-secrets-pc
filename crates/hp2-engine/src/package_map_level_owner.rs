//! Direct Core ownership for package maps attached to levels.

use crate::game_save::{self, ServerMasterMapOps};
use hp2_abi::engine::generated::{TArrayRaw, TTransArrayRaw, UGameEngine, ULevel, UPackageMap, UNetDriver, UObject, ALevelInfo};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use std::{ffi::c_void, path::Path, ptr};

const LOAD_NO_FAIL: u32 = 0x0001;

/// Copies package linkers through Core's canonical package-map storage.
///
/// # Safety
/// Both maps are live Core-owned package maps and belong to this engine thread.
pub unsafe fn copy_linkers(
    access: CoreAccess<'_>,
    destination: *mut UPackageMap,
    source: *mut UPackageMap,
) -> RuntimeResult<()> {
    unsafe { access.package_map_copy_linkers(destination, source) }
}

/// Assembles the server master map using canonical Core package loading.
///
/// # Safety
/// Engine, driver and level must be live canonical objects owned by this Core.
/// The generic policy checks the nullable driver and level before dereference.
pub unsafe fn build_server_master_map(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    driver: *mut UNetDriver,
    level: *mut ULevel,
) -> RuntimeResult<()> {
    let mut ops = CoreServerMasterMapOps { access };
    // SAFETY: the public contract requires each pointer to identify a live object;
    // the shared policy validates the null arguments before dereferencing them.
    unsafe {
        game_save::build_server_master_map_with(
            &mut ops,
            engine.cast(),
            driver.cast(),
            level.cast(),
        )
    }
}

struct CoreServerMasterMapOps<'a> {
    access: CoreAccess<'a>,
}

impl ServerMasterMapOps for CoreServerMasterMapOps<'_> {
    type Error = RuntimeError;

    unsafe fn check_failed(&mut self, expression: &'static [u8], _line: i32) -> RuntimeResult<()> {
        let message = std::str::from_utf8(&expression[..expression.len() - 1])
            .map_err(|_| RuntimeError::new("Invalid server master map check"))?;
        Err(RuntimeError::new(message))
    }

    unsafe fn begin_load(&mut self) -> RuntimeResult<()> {
        self.access.begin_load()
    }

    unsafe fn end_load(&mut self) -> RuntimeResult<()> {
        self.access.end_load()
    }

    unsafe fn level_linker(&mut self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*level.cast::<ULevel>()).base.base._linker })
    }

    unsafe fn master_map_add_linker(&mut self, driver: *mut c_void, linker: *mut c_void) -> RuntimeResult<i32> {
        let driver = driver.cast::<UNetDriver>();
        let map = unsafe { (*driver).master_map.cast::<UPackageMap>() };
        if map.is_null() { return Err(RuntimeError::new("Null server master package map")); }
        unsafe { self.access.package_map_add_linker(map, linker.cast::<UObject>()) }
    }

    unsafe fn server_packages_count(&mut self, engine: *mut c_void) -> RuntimeResult<i32> {
        let raw = unsafe { (*engine.cast::<UGameEngine>()).server_packages };
        if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
            return Err(RuntimeError::new("Invalid GameEngine.ServerPackages storage"));
        }
        Ok(raw.num)
    }

    unsafe fn server_package_debugf(&mut self, engine: *mut c_void, index: i32) -> RuntimeResult<()> {
        let text = self.server_package_units(engine, index)?;
        let category = self.access.intern_str("Log", FindName::Add)?;
        let mut message = "Server Package: ".chars().map(u32::from).collect::<Vec<_>>();
        message.extend_from_slice(&text);
        self.access.emit_log(category, &message)
    }

    unsafe fn server_package_linker(&mut self, engine: *mut c_void, index: i32) -> RuntimeResult<*mut c_void> {
        let units = self.server_package_units(engine, index)?;
        let package: String = units.iter().copied().map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect();
        let linker = unsafe { self.access.get_package_linker(Path::new(&package), LOAD_NO_FAIL, ptr::null_mut(), None)? };
        Ok(linker.cast())
    }

    unsafe fn debugf_server_side_only(&mut self) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        let message: Vec<u32> = "   (server-side only)".chars().map(u32::from).collect();
        self.access.emit_log(category, &message)
    }

    unsafe fn level_level_info(&mut self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        let info = unsafe { level_info(level.cast::<ULevel>())? };
        Ok(info.cast())
    }
    unsafe fn level_info_game(&mut self, info: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*info.cast::<ALevelInfo>()).game })
    }

    unsafe fn game_class_linker(&mut self, game: *mut c_void) -> RuntimeResult<*mut c_void> {
        let class = unsafe { (*game.cast::<UObject>()).class.cast::<UObject>() };
        if class.is_null() { return Ok(ptr::null_mut()); }
        Ok(unsafe { (*class)._linker })
    }

    unsafe fn master_map_compute(&mut self, driver: *mut c_void) -> RuntimeResult<()> {
        let map = unsafe { (*driver.cast::<UNetDriver>()).master_map.cast::<UPackageMap>() };
        if map.is_null() { return Err(RuntimeError::new("Null server master package map")); }
        unsafe { self.access.package_map_compute(map) }
    }
}

impl CoreServerMasterMapOps<'_> {
    unsafe fn server_package_units(&self, engine: *mut c_void, index: i32) -> RuntimeResult<&[u32]> {
        let raw: TArrayRaw = unsafe { (*engine.cast::<UGameEngine>()).server_packages };
        if index < 0 || index >= raw.num || raw.data.is_null() {
            return Err(RuntimeError::new("GameEngine.ServerPackages index is out of range"));
        }
        let value = unsafe { raw.data.cast::<TArrayRaw>().add(index as usize).read_unaligned() };
        if value.num < 0 || (value.num > 0 && value.data.is_null()) {
            return Err(RuntimeError::new("Invalid GameEngine.ServerPackages string storage"));
        }
        if value.num == 0 { return Ok(&[]); }
        let units = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) };
        Ok(if units.last() == Some(&0) { &units[..units.len() - 1] } else { units })
    }
}
/// Returns the `Level->Actors(0)` level-info actor used by `ULevel::GetLevelInfo`.
///
/// # Safety
/// `level` is a live canonical ULevel with initialized actor storage.
pub(crate) unsafe fn level_info(level: *mut ULevel) -> RuntimeResult<*mut ALevelInfo> {
    if level.is_null() { return Err(RuntimeError::new("Null ULevel")); }
    let actors = unsafe { (*level).base.actors };
    if actors.array_num < 0 || actors.array_max < actors.array_num {
        return Err(RuntimeError::new("Invalid Level.Actors storage"));
    }
    if actors.array_num == 0 || actors.data.is_null() {
        return Err(RuntimeError::new("Level has no LevelInfo actor"));
    }
    let info = unsafe { *actors.data.cast::<*mut ALevelInfo>() };
    if info.is_null() { return Err(RuntimeError::new("LevelInfo actor is null")); }
    Ok(info)
}
