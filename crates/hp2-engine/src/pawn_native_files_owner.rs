//! Canonical Core adapter for the shared native map-name scan.

use crate::pawn_native_files::{self, GetMapNameOps};
use hp2_abi::engine::generated::{TArrayRaw, UObject};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
};
use hp2_core::native_path::{from_utf32, from_utf16_units, path_from_units, path_to_units, to_utf16};
use std::{ffi::CStr, os::unix::ffi::OsStrExt, path::PathBuf, ptr};


struct CoreMapNameOps<'a> {
    access: CoreAccess<'a>,
    system: *mut hp2_core::core_records::SystemRecord,
    extension: Vec<u16>,
    cd_path: Vec<u16>,
    separator: Vec<u16>,
}

impl<'a> CoreMapNameOps<'a> {
    fn new(access: CoreAccess<'a>) -> RuntimeResult<Self> {
        let system = access.system()?;
        let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
        let separator = vec![std::path::MAIN_SEPARATOR as u16];
        let cd_path = match command_line_cd_path(access)? {
            Some(value) => value,
            None => access.config_get_units(&wide("Engine.Engine"), &wide("CdPath"))?.unwrap_or_default(),
        };
        let mut cd_path = cd_path;
        if !cd_path.is_empty() {
            let palette = access.data_root().join("../Textures/Palettes.utx");
            let palette_size = std::fs::metadata(palette)
                .ok()
                .map(|metadata| metadata.len())
                .filter(|size| *size <= i32::MAX as u64);
            if palette_size.is_none() {
                cd_path.clear();
            } else if cd_path.last().copied() != Some(u32::from(std::path::MAIN_SEPARATOR as u8)) {
                cd_path.push(u32::from(std::path::MAIN_SEPARATOR as u8));
            }
        }
        if !cd_path.is_empty() {
            let path = path_from_units(&cd_path)?;
            let path = if path.is_absolute() { path } else { access.data_root().join(path) };
            cd_path = path_to_units(&path)?;
            if cd_path.last().copied() != Some(std::path::MAIN_SEPARATOR as u32) {
                cd_path.push(std::path::MAIN_SEPARATOR as u32);
            }
        }
        Ok(Self {
            access,
            system,
            extension: to_utf16(defaults.default_map_ext())?,
            cd_path: to_utf16(&cd_path)?,
            separator,
        })
    }

    fn path_text(&self, index: i32) -> RuntimeResult<Vec<u16>> {
        system_path_text(self.system, index)
    }
}

impl GetMapNameOps for CoreMapNameOps<'_> {
    type Error = RuntimeError;

    fn map_extension(&mut self) -> RuntimeResult<Vec<u16>> {
        Ok(self.extension.clone())
    }

    fn cd_path(&mut self) -> RuntimeResult<Vec<u16>> {
        Ok(self.cd_path.clone())
    }

    fn path_separator(&mut self) -> RuntimeResult<Vec<u16>> {
        Ok(self.separator.clone())
    }

    fn paths_count(&mut self) -> RuntimeResult<i32> {
        Ok(system_paths(self.system)?.num)
    }

    fn path_at(&mut self, index: i32) -> RuntimeResult<Vec<u16>> {
        self.path_text(index)
    }

    fn find_files(&mut self, pattern: &[u16]) -> RuntimeResult<Vec<Vec<u16>>> {
        let path = hp2_core::native_path::path_from_utf16(pattern)?;
        hp2_core::unix_file_manager_owner::find_files(self.access, &path)?
            .into_iter().map(|name| to_utf16(&name)).collect()
    }
}


/// Runs the shared `AActor.GetMapName` policy over the canonical System paths.
pub(crate) fn get_map_name(
    access: CoreAccess<'_>,
    prefix: &[u32],
    map: &[u32],
    dir: i32,
) -> RuntimeResult<Vec<u32>> {
    let mut ops = CoreMapNameOps::new(access)?;
    let prefix = to_utf16(prefix)?;
    let map = to_utf16(map)?;
    let result = pawn_native_files::get_map_name_with(&mut ops, &prefix, &map, dir)?;
    from_utf16_units(&result)
}

/// Loads the saved DefaultPlayer/User URL and returns one option value.
pub(crate) fn get_default_url(access: CoreAccess<'_>, option: &[u32]) -> RuntimeResult<Vec<u32>> {
    use hp2_abi::engine::generated::FURL;
    use std::mem::MaybeUninit;

    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut storage = MaybeUninit::<FURL>::uninit();
    unsafe { crate::url_owner::UrlOwner::construct_default(access, &defaults, storage.as_mut_ptr())?; }
    let url = unsafe { storage.assume_init_mut() };
    let result = (|| {
        let section = wide("DefaultPlayer");
        let filename = wide("User");
        unsafe {
            crate::url_owner::UrlOwner::load_config(
                access, &defaults, url, section.as_ptr(), filename.as_ptr(),
            )?;
        }
        let mut key = option.iter().copied().take_while(|&unit| unit != 0).collect::<Vec<_>>();
        key.push(u32::from(b'='));
        key.push(0);
        let empty = wide("");
        let value = unsafe {
            crate::url_owner::UrlOwner::get_option(
                access, &defaults, url, key.as_ptr(), empty.as_ptr(),
            )?
        };
        unsafe { tchar_units(value.cast()) }
    })();
    let cleanup = unsafe { crate::url_owner::UrlOwner::destroy(access, url) };
    result.and_then(|value| cleanup.map(|()| value))
}

/// Applies the shared `PlayerPawn.UpdateURL` policy to canonical GameEngine.LastURL.
pub(crate) fn update_url(
    access: CoreAccess<'_>,
    player: *mut std::ffi::c_void,
    option: &[u32],
    value: &[u32],
    save: bool,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{APlayerPawn, UGameEngine, ULevel};

    let pawn = player.cast::<APlayerPawn>();
    if pawn.is_null() { return Err(RuntimeError::new("UpdateURL received a null canonical PlayerPawn")); }
    let level = unsafe { (*pawn).base.base.x_level.cast::<ULevel>() };
    if level.is_null() { return Err(RuntimeError::new("UpdateURL PlayerPawn has no canonical Level")); }
    let engine = unsafe { (*level).base.engine.cast::<UGameEngine>() };
    if engine.is_null() { return Err(RuntimeError::new("UpdateURL Level has no canonical GameEngine")); }
    let url = unsafe { ptr::addr_of_mut!((*engine).last_url) };
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut ops = CoreUpdateUrlOps { access, defaults: &defaults, url };
    let option = to_utf16(option)?;
    let value = to_utf16(value)?;
    pawn_native_files::update_url_with(&mut ops, &option, &value, save)
}

struct CoreCacheEntryOps<'a> {
    access: CoreAccess<'a>,
}

impl pawn_native_files::CacheEntryOps for CoreCacheEntryOps<'_> {
    type Error = RuntimeError;

    fn cache_entries(&mut self) -> RuntimeResult<Vec<(Vec<u16>, Vec<u16>)>> {
        let system = self.access.system()?;
        let cache_path = fstring_units(unsafe { ptr::read_unaligned(ptr::addr_of!((*system).cache_path)) })?;
        let mut path = path_from_units(&cache_path)?;
        if !path.is_absolute() {
            path = self.access.data_root().join(path);
        }
        let filename = path.join("cache.ini");
        let filename = path_to_units(&filename)?;
        let section = wide("Cache");
        self.access
            .config_section_units(&section, Some(&filename))?
            .into_iter()
            .map(|(key, value)| Ok((to_utf16(&key)?, to_utf16(&value)?)))
            .collect()
    }
}

/// Reads the canonical cache.ini Cache section for the shared native selector.
pub(crate) fn get_cache_entry(
    access: CoreAccess<'_>,
    num: i32,
) -> RuntimeResult<Option<(Vec<u32>, Vec<u32>)>> {
    let mut ops = CoreCacheEntryOps { access };
    pawn_native_files::get_cache_entry_with(&mut ops, num)?
        .map(|(key, value)| Ok((from_utf16_units(&key)?, from_utf16_units(&value)?)))
        .transpose()
}

struct CoreRegistryLookup<'a> {
    access: CoreAccess<'a>,
    rows: Vec<(Vec<u16>, Vec<u16>)>,
}

impl pawn_native_files::RegistryLookupOps for CoreRegistryLookup<'_> {
    type Error = RuntimeError;

    fn classes(&mut self, class_name: &[u16]) -> RuntimeResult<i32> {
        let class_name = from_utf16_units(class_name)?;
        self.rows = core_registry_objects(self.access, &class_name, true)?;
        i32::try_from(self.rows.len()).map_err(|_| RuntimeError::new("Registry result count exceeds INT32"))
    }

    fn textures(&mut self) -> RuntimeResult<i32> {
        self.rows = core_registry_objects(self.access, &[], false)?;
        i32::try_from(self.rows.len()).map_err(|_| RuntimeError::new("Registry result count exceeds INT32"))
    }

    fn object_at(&mut self, index: i32) -> RuntimeResult<(Vec<u16>, Vec<u16>)> {
        self.rows.get(usize::try_from(index).unwrap_or(usize::MAX))
            .cloned()
            .ok_or_else(|| RuntimeError::new("Registry result index is out of range"))
    }
}

pub(crate) fn get_next_int(
    access: CoreAccess<'_>,
    class_name: &[u32],
    index: i32,
) -> RuntimeResult<Vec<u32>> {
    let mut ops = CoreRegistryLookup { access, rows: Vec::new() };
    let class_name = to_utf16(class_name)?;
    let result = pawn_native_files::get_next_int_with(&mut ops, &class_name, index)?;
    from_utf16_units(&result)
}

pub(crate) fn get_next_int_desc(
    access: CoreAccess<'_>,
    class_name: &[u32],
    index: i32,
) -> RuntimeResult<(Vec<u32>, Vec<u32>)> {
    let mut ops = CoreRegistryLookup { access, rows: Vec::new() };
    let class_name = to_utf16(class_name)?;
    let (object, description) =
        pawn_native_files::get_next_int_desc_with(&mut ops, &class_name, index)?;
    Ok((from_utf16_units(&object)?, from_utf16_units(&description)?))
}

pub(crate) fn get_next_skin(
    access: CoreAccess<'_>,
    prefix: &[u32],
    current: &[u32],
    direction: i32,
) -> RuntimeResult<(Vec<u32>, Vec<u32>)> {
    let mut ops = CoreRegistryLookup { access, rows: Vec::new() };
    let prefix = to_utf16(prefix)?;
    let current = to_utf16(current)?;
    let (object, description) =
        pawn_native_files::get_next_skin_with(&mut ops, &prefix, &current, direction)?;
    Ok((from_utf16_units(&object)?, from_utf16_units(&description)?))
}

fn core_registry_objects(
    access: CoreAccess<'_>,
    meta_name: &[u32],
    classes: bool,
) -> RuntimeResult<Vec<(Vec<u16>, Vec<u16>)>> {
    use hp2_core::object_registry::{
        self, Hp2ObjectRegistryArgs, Hp2ObjectRegistryOperation, Hp2ObjectRegistryServices,
    };
    let mut language = access.language_text()?
        .into_iter()
        .map(|unit| unit as hp2_core::ansi::Tchar)
        .collect::<Vec<_>>();
    language.push(0);
    hp2_core::object_registry::synchronize_language(&language)
        .map_err(|status| RuntimeError::new("Unable to synchronize object registry language").with_context(&status.to_string()))?;

    let class_object = if classes {
        access.native_class("Core.Class")?.cast()
    } else {
        access.native_class("Engine.Texture")?.cast()
    };
    let meta_class = if classes {
        let class_class = access.native_class("Core.Class")?;
        Some(unsafe {
            access.find_object_by_name(class_class, meta_name)?
                .ok_or_else(|| RuntimeError::new("Registry metaclass not found"))?
        })
    } else {
        None
    };
    let mut context = CoreRegistryContext {
        access,
        system: access.system()?,
        class_object,
        meta_class: meta_class.unwrap_or(ptr::null_mut()).cast(),
        scratch: Vec::new(),
        failure: None,
    };
    let mut result = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let args = Hp2ObjectRegistryArgs {
        results: ptr::from_mut(&mut result).cast(),
        class_object: context.class_object,
        meta_class: context.meta_class,
        text: ptr::null(),
        force_refresh: 0,
    };
    let services = Hp2ObjectRegistryServices { call: core_registry_leaf };
    let status = unsafe {
        object_registry::run(
            Hp2ObjectRegistryOperation::Objects,
            &args,
            ptr::from_mut(&mut context).cast(),
            &services,
        )
    };
    let copied = match status {
        Ok(()) => (|| {
            let count = usize::try_from(result.num)
                .map_err(|_| RuntimeError::new("Invalid registry result count"))?;
            if count == 0 { return Ok(Vec::new()); }
            if result.data.is_null() { return Err(RuntimeError::new("Null registry result storage")); }
            let entries = unsafe {
                std::slice::from_raw_parts(
                    result.data.cast::<hp2_abi::engine::generated::FRegistryObjectInfo>(),
                    count,
                )
            };
            entries.iter().map(|entry| {
                let object = fstring_units(unsafe { ptr::read_unaligned(ptr::addr_of!(entry.object)) })?;
                let description = fstring_units(unsafe { ptr::read_unaligned(ptr::addr_of!(entry.description)) })?;
                Ok((to_utf16(&object)?, to_utf16(&description)?))
            }).collect::<RuntimeResult<Vec<_>>>()
        })(),
        Err(status) => Err(context.failure.take().unwrap_or_else(|| {
            RuntimeError::new("Canonical object registry failed").with_context(&status.to_string())
        })),
    };
    let cleanup = unsafe { destroy_registry_result(access, &mut result) };
    match (copied, cleanup) {
        (Ok(rows), Ok(())) => Ok(rows),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(error.with_context(&cleanup.to_string())),
    }
}

fn canonical_object_path_name(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
    if object.is_null() { return Err(RuntimeError::new("Null object in canonical path lookup")); }
    let mut segments = Vec::new();
    let mut current = object;
    while !current.is_null() {
        let name = i32::from_ne_bytes(unsafe { (*current).name });
        segments.push(access.name_text(name)?);
        current = unsafe { (*current).outer.cast::<UObject>() };
    }
    let mut path = Vec::new();
    for (index, segment) in segments.iter().rev().enumerate() {
        if index != 0 { path.push(u32::from(b'.')); }
        path.extend_from_slice(segment);
    }
    Ok(path)
}

struct CoreRegistryContext<'a> {
    access: CoreAccess<'a>,
    system: *mut hp2_core::core_records::SystemRecord,
    class_object: *mut std::ffi::c_void,
    meta_class: *mut std::ffi::c_void,
    scratch: Vec<u32>,
    failure: Option<RuntimeError>,
}

unsafe extern "C" fn core_registry_leaf(
    raw: *mut std::ffi::c_void,
    operation: hp2_core::object_registry::Hp2ObjectRegistryLeaf,
    first: usize,
    second: usize,
    output: *mut usize,
) -> i32 {
    let context = unsafe { &mut *raw.cast::<CoreRegistryContext<'_>>() };
    match unsafe { core_registry_leaf_impl(context, operation, first, second, output) } {
        Ok(()) => 0,
        Err(error) => {
            if context.failure.is_none() { context.failure = Some(error); }
            1
        }
    }
}

unsafe fn core_registry_leaf_impl(
    context: &mut CoreRegistryContext<'_>,
    operation: hp2_core::object_registry::Hp2ObjectRegistryLeaf,
    first: usize,
    second: usize,
    output: *mut usize,
) -> RuntimeResult<()> {
    use hp2_core::object_registry::Hp2ObjectRegistryLeaf as L;
    use std::mem::size_of;

    match operation {
        L::PathCount => {
            let paths = unsafe { ptr::read_unaligned(ptr::addr_of!((*context.system).paths)) };
            if paths.num < 0 || (paths.num > 0 && paths.data.is_null()) {
                return Err(RuntimeError::new("Invalid canonical System.Paths storage"));
            }
            unsafe { output.write(paths.num as usize); }
        }
        L::Path => {
            let paths = unsafe { ptr::read_unaligned(ptr::addr_of!((*context.system).paths)) };
            let index = i32::try_from(first).map_err(|_| RuntimeError::new("Invalid registry path index"))?;
            if index >= paths.num || paths.data.is_null() {
                return Err(RuntimeError::new("Registry path index is out of range"));
            }
            let path = unsafe { paths.data.cast::<TArrayRaw>().add(index as usize).read_unaligned() };
            context.scratch = fstring_units(path)?;
            context.scratch.push(0);
            unsafe { output.write(context.scratch.as_ptr() as usize); }
        }
        L::BaseDir => {
            context.scratch = path_to_units(&context.access.data_root().join("System"))?;
            if context.scratch.last().copied() != Some(u32::from(b'/')) {
                context.scratch.push(u32::from(b'/'));
            }
            context.scratch.push(0);
            unsafe { output.write(context.scratch.as_ptr() as usize); }
        }
        L::FindFiles => {
            let pattern = unsafe { tchar_units(first as *const std::ffi::c_void) }?;
            let full_pattern = if pattern == wide("Core.*") {
                path_to_units(&context.access.data_root().join("System").join("Core.*"))?
            } else {
                pattern
            };
            let files = hp2_core::unix_file_manager_owner::find_files(
                context.access, &path_from_units(&full_pattern)?,
            )?;
            let destination = second as *mut TArrayRaw;
            let arrays = unsafe { hp2_core::array_storage::published_allocator() }
                .map_err(|status| RuntimeError::new("Core array allocator is unavailable").with_context(&status.to_string()))?;
            for mut units in files {
                let slots = hp2_core::array_storage::Hp2ArraySlots {
                    data: unsafe { ptr::addr_of_mut!((*destination).data) },
                    num: unsafe { ptr::addr_of_mut!((*destination).num) },
                    max: unsafe { ptr::addr_of_mut!((*destination).max) },
                };
                let index = unsafe {
                    hp2_core::array_storage::add_zeroed(
                        slots,
                        1,
                        size_of::<TArrayRaw>() as i32,
                        ptr::null(),
                        arrays,
                    )
                }.map_err(|status| RuntimeError::new("Unable to grow registry file array").with_context(&status.to_string()))?;
                let field = unsafe { (*destination).data.cast::<TArrayRaw>().add(index as usize) };
                units.push(0);
                unsafe { context.access.assign_fstring(field, &units)?; }
            }
        }
        L::ReadPublic => {
            let filename = unsafe { tchar_units(first as *const std::ffi::c_void) }?;
            let request = unsafe { &mut *(second as *mut hp2_core::object_registry::Hp2ObjectRegistrySection) };
            let section = wide("Public");
            let present = context.access.config_has_section_units(&section, Some(&filename))?;
            if present {
                let pairs = context.access.config_section_units(&section, Some(&filename))?;
                let capacity = usize::try_from(request.capacity)
                    .map_err(|_| RuntimeError::new("Invalid registry section capacity"))?;
                if request.buffer.is_null() || capacity == 0 {
                    return Err(RuntimeError::new("Invalid registry section buffer"));
                }
                let buffer = request.buffer.cast::<u32>();
                let mut at = 0;
                'pairs: for (key, value) in pairs {
                    for unit in key.into_iter().chain([u32::from(b'=')]).chain(value).chain([0]) {
                        if at + 1 >= capacity { break 'pairs; }
                        unsafe { buffer.add(at).write(unit); }
                        at += 1;
                    }
                }
                unsafe { buffer.add(at).write(0); output.write(1); }
            } else {
                unsafe { output.write(0); }
            }
        }
        L::ParseDriverField => {
            let field = unsafe { &*(second as *const hp2_core::object_registry::Hp2ObjectRegistryParse) };
            let mut value = unsafe { tchar_units(first as *const std::ffi::c_void) }?;
            value.push(0);
            const KEYS: [&str; 5] = ["Name=", "Class=", "MetaClass=", "Description=", "Autodetect="];
            let key = wide(KEYS.get(field.field).ok_or_else(|| RuntimeError::new("Invalid registry driver field"))?);
            let mut parsed = vec![0_u32; value.len()];
            unsafe {
                hp2_core::text_parse::parse_string(
                    hp2_core::native_text::tchar_units(&value).as_ptr(),
                    hp2_core::native_text::tchar_units(&key).as_ptr(),
                    parsed.as_mut_ptr().cast::<hp2_core::ansi::Tchar>(),
                    i32::try_from(parsed.len()).map_err(|_| RuntimeError::new("Registry field exceeds INT32"))?,
                );
                context.access.assign_fstring(field.output.cast(), &parsed)?;
            }
        }
        L::ParsePreferenceField => {
            let field = unsafe { &*(second as *const hp2_core::object_registry::Hp2ObjectRegistryParse) };
            let mut value = unsafe { tchar_units(first as *const std::ffi::c_void) }?;
            value.push(0);
            match field.field {
                0..=2 => {
                    const KEYS: [&str; 3] = ["Caption=", "Parent=", "Class="];
                    let key = wide(KEYS[field.field]);
                    let mut parsed = vec![0_u32; value.len()];
                    unsafe {
                        hp2_core::text_parse::parse_string(
                            hp2_core::native_text::tchar_units(&value).as_ptr(),
                            hp2_core::native_text::tchar_units(&key).as_ptr(),
                            parsed.as_mut_ptr().cast::<hp2_core::ansi::Tchar>(),
                            parsed.len() as i32,
                        );
                        context.access.assign_fstring(field.output.cast(), &parsed)?;
                    }
                }
                3 => {
                    let key = wide("Category=");
                    let mut parsed = vec![0_u32; value.len()];
                    unsafe {
                        hp2_core::text_parse::parse_string(
                            hp2_core::native_text::tchar_units(&value).as_ptr(),
                            hp2_core::native_text::tchar_units(&key).as_ptr(),
                            parsed.as_mut_ptr().cast::<hp2_core::ansi::Tchar>(),
                            parsed.len() as i32,
                        );
                    }
                    let category = if parsed[0] == 0 { 0 } else { context.access.intern_str(&from_utf32(&parsed)?, FindName::Add)? };
                    unsafe { (field.output.cast::<i32>()).write_unaligned(category); }
                }
                4 => {
                    let section = wide("General");
                    let key = wide("True");
                    let package = wide("Core");
                    let mut localized_true = context.access.localize_text(&section, &key, Some(&package), None, false)?;
                    localized_true.push(0);
                    let key = wide("Immediate=");
                    unsafe {
                        hp2_core::parse_scalar::parse_bool(
                            hp2_core::native_text::tchar_units(&value).as_ptr(),
                            hp2_core::native_text::tchar_units(&key).as_ptr(),
                            hp2_core::native_text::tchar_units(&localized_true).as_ptr(),
                            field.output.cast(),
                        );
                    }
                }
                _ => return Err(RuntimeError::new("Invalid registry preference field")),
            }
        }
        L::NativeCategory => {
            let index_ptr = first as *mut i32;
            let index = unsafe { index_ptr.read_unaligned() };
            let text = context.access.name_text(index)?;
            let name = from_utf32(&text)?;
            let native = context.access.intern_str(&name, FindName::Intrinsic)?;
            unsafe { index_ptr.write_unaligned(native); }
        }
        L::DllExtension => {
            context.scratch = wide(if cfg!(target_os = "macos") { ".dylib" } else { ".so" });
            unsafe { output.write(context.scratch.as_ptr() as usize); }
        }
        L::LanguageMetaClass => {
            let class = context.access.native_class("Core.Language")?;
            context.scratch = canonical_object_path_name(context.access, class.cast())?;
            context.scratch.push(0);
            unsafe { output.write(context.scratch.as_ptr() as usize); }
        }
        L::CheckClass => {
            if context.class_object.is_null() { return Err(RuntimeError::new("GetRegistryObjects received a null class")); }
        }
        L::CheckMetaClass => {
            if context.class_object == context.access.native_class("Core.Class")?.cast() && context.meta_class.is_null() {
                return Err(RuntimeError::new("GetRegistryObjects requires a metaclass for Class"));
            }
        }
        L::ClassName => {
            let class = context.class_object.cast::<UObject>();
            let name = i32::from_ne_bytes(unsafe { (*class).name });
            context.scratch = context.access.name_text(name)?;
            context.scratch.push(0);
            unsafe { output.write(context.scratch.as_ptr() as usize); }
        }
        L::MetaClassName => {
            if context.meta_class.is_null() {
                context.scratch = vec![0];
            } else {
                context.scratch = canonical_object_path_name(context.access, context.meta_class.cast())?;
                context.scratch.push(0);
            }
            unsafe { output.write(context.scratch.as_ptr() as usize); }
        }
        _ => return Err(RuntimeError::new("Unexpected canonical object registry leaf")),
    }
    Ok(())
}

unsafe fn destroy_registry_result(access: CoreAccess<'_>, result: &mut TArrayRaw) -> RuntimeResult<()> {
    let mut first_error = None;
    if result.num > 0 && result.data.is_null() {
        return Err(RuntimeError::new("Null registry result storage during cleanup"));
    }
    for index in 0..result.num.max(0) {
        let row = unsafe { result.data.cast::<hp2_abi::engine::generated::FRegistryObjectInfo>().add(index as usize) };
        for offset in [0, 16, 32, 48, 64] {
            let field = unsafe { row.cast::<u8>().add(offset).cast::<TArrayRaw>() };
            if let Err(error) = unsafe { access.destroy_owned_fstring_array(field) } {
                first_error.get_or_insert(error);
            }
        }
    }
    if !result.data.is_null() {
        let arrays = unsafe { hp2_core::array_storage::published_allocator() }
            .map_err(|status| RuntimeError::new("Core array allocator is unavailable").with_context(&status.to_string()))?;
        let slots = hp2_core::array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!(result.data),
            num: ptr::addr_of_mut!(result.num),
            max: ptr::addr_of_mut!(result.max),
        };
        if let Err(status) = unsafe { hp2_core::array_storage::release(slots, arrays) } {
            first_error.get_or_insert_with(|| RuntimeError::new("Unable to release registry result array").with_context(&status.to_string()));
        }
    }
    first_error.map_or(Ok(()), Err)
}

struct CoreUpdateUrlOps<'a, 'b> {
    access: CoreAccess<'a>,
    defaults: &'b crate::url_owner::UrlDefaults,
    url: *mut hp2_abi::engine::generated::FURL,
}

impl pawn_native_files::UpdateUrlOps for CoreUpdateUrlOps<'_, '_> {
    type Error = RuntimeError;

    fn add_option(&mut self, text: &[u16]) -> RuntimeResult<()> {
        let mut text = from_utf16_units(text)?;
        text.push(0);
        unsafe { crate::url_owner::UrlOwner::add_option(self.access, self.defaults, self.url, text.as_ptr()) }
    }

    fn save_default(&mut self, option: &[u16]) -> RuntimeResult<()> {
        let section = wide("DefaultPlayer");
        let mut option = from_utf16_units(option)?;
        option.push(0);
        let filename = wide("User");
        unsafe {
            crate::url_owner::UrlOwner::save_config(
                self.access,
                self.defaults,
                self.url,
                section.as_ptr(),
                option.as_ptr(),
                filename.as_ptr(),
            )
        }
    }
}

fn command_line_cd_path(access: CoreAccess<'_>) -> RuntimeResult<Option<Vec<u32>>> {
    let command_line = access.command_line();
    let key = wide("CDPATH=");
    let mut value = vec![0_u32; 256];
    let parsed = unsafe {
        hp2_core::text_parse::parse_string(
            hp2_core::native_text::tchar_units(command_line).as_ptr(),
            hp2_core::native_text::tchar_units(&key).as_ptr(),
            value.as_mut_ptr().cast::<hp2_core::ansi::Tchar>(),
            value.len() as i32,
        )
    };
    Ok(parsed.then(|| value.into_iter().take_while(|&unit| unit != 0).collect()))
}

unsafe fn bytes_path(path: *const u8) -> PathBuf {
    let bytes = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

unsafe fn tchar_string(text: *const std::ffi::c_void) -> RuntimeResult<String> {
    let units = unsafe { tchar_units(text) }?;
    let mut output = String::new();
    for unit in units {
        let character = char::from_u32(unit)
            .ok_or_else(|| RuntimeError::new("Filesystem path contains invalid TCHAR data"))?;
        output.push(character);
    }
    Ok(output)
}

unsafe fn tchar_units(text: *const std::ffi::c_void) -> RuntimeResult<Vec<u32>> {
    if text.is_null() { return Err(RuntimeError::new("Null filesystem text")); }
    let native = text.cast::<hp2_core::ansi::Tchar>();
    let mut length = 0;
    while unsafe { *native.add(length) } != 0 { length += 1; }
    let units = unsafe { std::slice::from_raw_parts(native, length) };
    Ok(hp2_core::native_text::code_units(units).to_vec())
}

fn fstring_units(value: TArrayRaw) -> RuntimeResult<Vec<u32>> {
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical System.Paths FString storage"));
    }
    if value.num <= 1 { return Ok(Vec::new()); }
    let units = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize - 1) };
    Ok(units.to_vec())
}

fn wide(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}


fn path_to_utf16(path: &std::path::Path) -> RuntimeResult<Vec<u16>> {
    to_utf16(&path_to_units(path)?)
}


/// Resets the live local viewport input class's configuration in source order.
///
/// # Safety
/// `pawn` is a live canonical PlayerPawn; any Player/Input records remain live
/// through the config reset and its reentrant default/instance updates.
pub(crate) unsafe fn reset_keyboard(
    access: CoreAccess<'_>, pawn: *mut hp2_abi::engine::generated::APlayerPawn,
) -> RuntimeResult<()> {
    pawn_native_files::reset_keyboard_with(
        || unsafe {
            let player = ptr::addr_of!((*pawn).player).read_unaligned();
            let viewport_class = access.native_class("Engine.Viewport")?;
            if !hp2_core::object_model::is_a(player, viewport_class.cast()) {
                return Ok(ptr::null_mut());
            }
            let viewport = player.cast::<hp2_abi::engine::generated::UViewport>();
            let input = ptr::addr_of!((*viewport).input).read_unaligned();
            if input.is_null() { return Ok(ptr::null_mut()); }
            Ok(ptr::addr_of!((*input.cast::<UObject>()).class).read_unaligned())
        },
        |class| unsafe { access.reset_config(class.cast()) },
    )
}

/// Uses GameEngine's default ServerPackages property, not the live instance array.
pub(crate) fn check_valid_skin_package(
    access: CoreAccess<'_>, package: &[u32], mesh: &[u32],
) -> RuntimeResult<bool> {
    let package = to_utf16(package)?;
    let mesh = to_utf16(mesh)?;
    pawn_native_files::check_valid_skin_package_with(&package, &mesh, || unsafe {
        let class = access.native_class("Engine.GameEngine")?;
        let property_class = access.native_class("Core.Property")?;
        let name = access.intern_str("ServerPackages", FindName::Find)?;
        let property = access.find_field(class.cast(), name, property_class);
        if property.is_null() { return Ok(None); }
        if (*class).defaults.num == 0 { return Ok(Some(Vec::new())); }
        let default = access.default_object(class)?;
        // UnType.h: PPF_Localized=1; source ExportText passes the same default twice.
        let text = access.property_export_text_indexed(property.cast(), default, default, 0, 1)?;
        Ok(Some(to_utf16(&text)?))
    })
}

struct CoreScreenToWorld<'a> { access: CoreAccess<'a> }
unsafe fn screen_client(pawn: *mut std::ffi::c_void) -> RuntimeResult<*mut hp2_abi::engine::generated::UClient> {
    use hp2_abi::engine::generated::{AActor, UEngine, ULevelBase};
    let level = unsafe { (*pawn.cast::<AActor>()).x_level.cast::<ULevelBase>() };
    if level.is_null() { return Err(RuntimeError::new("ScreenToWorld pawn has no Level")); }
    let engine = unsafe { (*level).engine.cast::<UEngine>() };
    if engine.is_null() { return Err(RuntimeError::new("ScreenToWorld Level has no Engine")); }
    let client = unsafe { (*engine).client.cast::<hp2_abi::engine::generated::UClient>() };
    if client.is_null() { return Err(RuntimeError::new("ScreenToWorld Engine has no Client")); }
    Ok(client)
}
impl pawn_native_files::ScreenToWorldOps for CoreScreenToWorld<'_> {
    type Error = RuntimeError;
    unsafe fn viewport_size(&mut self, pawn: *mut std::ffi::c_void) -> RuntimeResult<(i32, i32)> {
        let client = unsafe { screen_client(pawn)? };
        let viewports = unsafe { ptr::addr_of!((*client).viewports).read_unaligned() };
        if viewports.num <= 0 || viewports.max < viewports.num || viewports.data.is_null() {
            return Err(RuntimeError::new("ScreenToWorld Client has no first viewport"));
        }
        let viewport = unsafe { viewports.data.cast::<*mut hp2_abi::engine::generated::UViewport>().read_unaligned() };
        if viewport.is_null() { return Err(RuntimeError::new("ScreenToWorld first viewport is null")); }
        Ok(unsafe { ((*viewport).size_x, (*viewport).size_y) })
    }
    unsafe fn maintain_vertical_fov(&mut self, pawn: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let client = unsafe { screen_client(pawn)? };
        Ok(i32::from(unsafe { (*client).maintain_vertical_fov() }))
    }
    unsafe fn effective_fov(&mut self, fov: f32, x: i32, y: i32, maintain: i32) -> RuntimeResult<f32> {
        Ok(crate::cam_mgr::get_effective_fov_angle(fov, x, y, maintain))
    }
    unsafe fn calc_view(
        &mut self, pawn: *mut std::ffi::c_void, location: &mut hp2_abi::FVector,
        rotation: &mut hp2_abi::engine::generated::FRotator,
    ) -> RuntimeResult<()> {
        let mut params = crate::game_draw_owner::PlayerCalcViewParams {
            view_actor: pawn.cast(), camera_location: *location, camera_rotation: *rotation,
        };
        unsafe { crate::game_draw_owner::player_calc_view_params(self.access, pawn.cast(), &mut params)?; }
        *location = params.camera_location; *rotation = params.camera_rotation;
        Ok(())
    }
}
pub(crate) unsafe fn screen_to_world(
    access: CoreAccess<'_>, pawn: *mut hp2_abi::engine::generated::APlayerPawn,
    screen: hp2_abi::FVector, out: *mut hp2_abi::FVector,
) -> RuntimeResult<()> {
    unsafe { pawn_native_files::screen_to_world_with(&mut CoreScreenToWorld { access }, pawn.cast(), screen, out) }
}

const CACHE_SECTION: &[u32] = &[67, 97, 99, 104, 101];

struct CoreCacheMove<'core, 'arg> {
    access: CoreAccess<'core>,
    system: *mut hp2_core::core_records::SystemRecord,
    cache: hp2_core::CoreConfig,
    ini_name: Vec<u32>,
    guid: &'arg [u32],
}

fn system_paths(system: *mut hp2_core::core_records::SystemRecord) -> RuntimeResult<TArrayRaw> {
    let paths = unsafe { ptr::addr_of!((*system).paths).read_unaligned() };
    if paths.num < 0 || (paths.num > 0 && paths.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical System.Paths storage"));
    }
    Ok(paths)
}

fn system_path_text(
    system: *mut hp2_core::core_records::SystemRecord, index: i32,
) -> RuntimeResult<Vec<u16>> {
    let paths = system_paths(system)?;
    if index < 0 || index >= paths.num {
        return Err(RuntimeError::new("Canonical System.Paths index is out of range"));
    }
    let value = unsafe { paths.data.cast::<TArrayRaw>().add(index as usize).read_unaligned() };
    to_utf16(&fstring_units(value)?)
}

fn cache_move_log(
    access: CoreAccess<'_>, prefix: &str, source: &[u16], destination: Option<&[u16]>,
) -> RuntimeResult<()> {
    let category = access.intern_str("Log", FindName::Add)?;
    if access.name_suppressed(category) { return Ok(()); }
    let mut message = Vec::with_capacity(prefix.len() + source.len()
        + destination.map_or(0, |text| text.len() + 4));
    message.extend(prefix.bytes().map(u32::from));
    for (index, units) in [Some(source), destination].into_iter().enumerate() {
        let Some(units) = units else { break };
        if index != 0 { message.extend(" -> ".bytes().map(u32::from)); }
        for character in char::decode_utf16(units.iter().copied().take_while(|&unit| unit != 0)) {
            message.push(u32::from(character
                .map_err(|_| RuntimeError::new("Canonical cache log contains invalid UTF-16"))?));
        }
    }
    // debugf uses Logf's fixed 4096-TCHAR buffer, not unformatted Log.
    if message.len() >= 4096 { message.clear(); }
    access.emit_log(category, &message)
}

impl pawn_native_files::MoveCacheEntryOps for CoreCacheMove<'_, '_> {
    type Error = RuntimeError;
    fn cached_filename(&mut self) -> RuntimeResult<Option<Vec<u16>>> {
        self.cache.get(Some(&self.ini_name), CACHE_SECTION, self.guid)?
            .map(|mut value| {
                // Native Cache.GetString writes into TCHAR OrigFilename[256].
                value.truncate(255);
                to_utf16(&value)
            }).transpose()
    }
    fn cache_path(&mut self) -> RuntimeResult<Vec<u16>> {
        to_utf16(&fstring_units(unsafe { ptr::addr_of!((*self.system).cache_path).read_unaligned() })?)
    }
    fn cache_ext(&mut self) -> RuntimeResult<Vec<u16>> {
        to_utf16(&fstring_units(unsafe { ptr::addr_of!((*self.system).cache_ext).read_unaligned() })?)
    }
    fn path_separator(&mut self) -> RuntimeResult<Vec<u16>> {
        Ok(vec![std::path::MAIN_SEPARATOR as u16])
    }
    fn paths_count(&mut self) -> RuntimeResult<i32> { Ok(system_paths(self.system)?.num) }
    fn path_at(&mut self, index: i32) -> RuntimeResult<Vec<u16>> {
        system_path_text(self.system, index)
    }
    fn log_no_extension(&mut self, name: &[u16]) -> RuntimeResult<()> {
        cache_move_log(self.access, "MoveCacheEntry: No extension: ", name, None)
    }
    fn log_move(&mut self, source: &[u16], destination: &[u16]) -> RuntimeResult<()> {
        cache_move_log(self.access, "MoveCacheEntry: ", source, Some(destination))
    }
    fn move_file(&mut self, destination: &[u16], source: &[u16]) -> RuntimeResult<bool> {
        let destination = hp2_core::native_path::path_from_utf16(destination)?;
        let source = hp2_core::native_path::path_from_utf16(source)?;
        // FFileManagerUnix::Move defaults: Replace=1, EvenIfReadOnly=0, Attributes=0.
        hp2_core::unix_file_manager_owner::move_file_paths(
            self.access, &destination, &source, true, false, false,
        )
    }
    fn clear_cache_entry(&mut self) -> RuntimeResult<()> {
        self.cache.set(Some(&self.ini_name), CACHE_SECTION, self.guid, &[])
    }
}

pub(crate) fn move_cache_entry(
    access: CoreAccess<'_>, guid: &[u32], new_filename: &[u32],
) -> RuntimeResult<bool> {
    let system = access.system()?;
    let mut ini_name = fstring_units(unsafe { ptr::addr_of!((*system).cache_path).read_unaligned() })?;
    ini_name.reserve(10);
    ini_name.push(std::path::MAIN_SEPARATOR as u32);
    ini_name.extend("cache.ini".bytes().map(u32::from));
    let mut ops = CoreCacheMove {
        access, system, cache: access.new_config_cache(), ini_name, guid,
    };
    let operation = pawn_native_files::move_cache_entry_with(
        &mut ops, &to_utf16(guid)?, &to_utf16(new_filename)?,
    );
    // The native stack-local FConfigCacheIni flushes during destruction, even
    // when the policy exits through an error. Preserve the first error.
    let flush = ops.cache.flush(None, false);
    match operation { Err(error) => Err(error), Ok(value) => flush.map(|()| value) }
}
