//! Direct Rust selection and loading for canonical package objects and classes.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::name_owner::FindName;
use crate::core_loader::{
    self, OwnedLinkerLoad, export_outer_name, linker_for_root, native_name_for,
    find_package_root, package_path, preload, resolve_index,
};
use hp2_abi::engine::generated::{FObjectExport, FPackageInfo, UObject, UClass, UPackageMap};
use std::ffi::c_void;
use std::path::Path;
use std::ptr;

unsafe extern "C" fn sandbox_package_len(map:*mut c_void)->i32 {
    if map.is_null(){return -1;}
    unsafe{ptr::addr_of!((*map.cast::<crate::core_records::PackageMapRecord>()).list).read_unaligned().num}
}
unsafe extern "C" fn sandbox_package_at(map:*mut c_void,index:i32,out:*mut *mut c_void)->i32 {
    if map.is_null()||out.is_null()||index<0{return 1;}
    let list=unsafe{ptr::addr_of!((*map.cast::<crate::core_records::PackageMapRecord>()).list).read_unaligned()};
    if list.num<0||list.max<list.num||(list.num>0&&list.data.is_null())||index>=list.num{return 1;}
    unsafe{out.write(list.data.cast::<FPackageInfo>().add(index as usize).cast())};
    0
}
unsafe extern "C" fn sandbox_package_parent(package:*mut c_void)->*mut c_void {
    if package.is_null(){ptr::null_mut()}else{unsafe{ptr::addr_of!((*package.cast::<FPackageInfo>()).parent).read_unaligned()}}
}
unsafe extern "C" fn sandbox_unused_object_linker(_: *mut c_void)->*mut c_void {ptr::null_mut()}
unsafe extern "C" fn sandbox_unused_object_index(_: *mut c_void)->i32 {-1}
unsafe extern "C" fn sandbox_unused_find_linker(_: *mut c_void,_:*mut c_void,_:*mut *const i32)->i32 {1}
unsafe extern "C" fn sandbox_unused_package_count(_: *mut c_void)->i32 {0}
unsafe extern "C" fn sandbox_unused_package_base(_: *mut c_void)->i32 {0}
unsafe extern "C" fn sandbox_unused_export(_: *mut c_void,_:i32,_:*mut *mut c_void)->i32 {1}
static PACKAGE_MAP_SERVICES: crate::package_map::Hp2PackageMapServices = crate::package_map::Hp2PackageMapServices {
    object_linker:sandbox_unused_object_linker,
    object_linker_index:sandbox_unused_object_index,
    find_linker:sandbox_unused_find_linker,
    package_len:sandbox_package_len,
    package_at:sandbox_package_at,
    package_parent:sandbox_package_parent,
    package_object_count:sandbox_unused_package_count,
    package_object_base:sandbox_unused_package_base,
    export_object:sandbox_unused_export,
};

fn sandbox_supports_package(access:CoreAccess<'_>,map:*mut UPackageMap,outer:*mut UObject)->RuntimeResult<bool> {
    if map.is_null(){return Ok(true);}
    let list=unsafe{ptr::addr_of!((*map.cast::<crate::core_records::PackageMapRecord>()).list).read_unaligned()};
    if list.num<0||list.max<list.num||(list.num>0&&list.data.is_null()){return Err(RuntimeError::new("Invalid PackageMap sandbox list"));}
    let supported=unsafe{crate::package_map::supports_package(map.cast(),outer.cast(),&PACKAGE_MAP_SERVICES)}
        .map(|result|result!=0)
        .map_err(|code|RuntimeError::new(&format!("Unable to check PackageMap sandbox ({code})")))?;
    if !supported {
        let format=access.localize_text(crate::lit!("Errors"),crate::lit!("Sandbox"),Some(crate::lit!("Core")),None,false)?;
        let name_index=i32::from_ne_bytes(unsafe{(*outer).name});
        let package_name=access.name_text(name_index)?;
        let mut message=Vec::new();
        let mut index=0;
        let format=format.iter().copied().take_while(|unit|*unit!=0).collect::<Vec<_>>();
        while index<format.len() {
            if format.get(index..index+2)==Some(&[u32::from(b'%'),u32::from(b's')]) {
                message.extend(package_name.iter().copied().take_while(|unit|*unit!=0));
                index+=2;
            } else {
                message.push(format[index]);
                index+=1;
            }
        }
        let category=access.intern_str("Log",crate::name_owner::FindName::Add)?;
        access.emit_log(category,&message)?;
    }
    Ok(supported)
}

/// Loads an object by name through canonical outer lookup and package exports.
///
/// # Safety
/// `class` and `outer` are null or live canonical objects in the same Core owner.
pub(crate) unsafe fn static_load_object(
    access: CoreAccess<'_>,
    class: *mut UClass,
    outer: *mut UObject,
    name: &[u32],
    flags: u32,
) -> RuntimeResult<*mut UObject> {
    unsafe { static_load_object_with_sandbox(access,class,outer,name,flags,None,ptr::null_mut()) }
}

unsafe fn static_load_object_with_sandbox(
    access: CoreAccess<'_>,
    class: *mut UClass,
    outer: *mut UObject,
    name: &[u32],
    flags: u32,
    filename: Option<&[u32]>,
    sandbox: *mut UPackageMap,
) -> RuntimeResult<*mut UObject> {
    const LOAD_DISALLOW_FILES: u32 = 0x0040;
    let Some(resolved) = crate::core_reflection::resolve_configured_load_name(access, name)? else {
        return missing_load(flags, name, "Configured object name was not found");
    };
    let text = resolved.as_ref();
    if text.is_empty() { return Err(RuntimeError::new("Object name is empty")); }
    let separator = text.iter().position(|unit| *unit == u32::from('.'));
    let (mut target_outer, linker) = if let Some(filename) = filename.filter(|_| flags & LOAD_DISALLOW_FILES == 0) {
        let mut root = outer;
        if let Some(dot) = separator {
            let package_name = &text[..dot];
            let index = access.intern(package_name, FindName::Add)?;
            root = find_package_root(access, index)?;
            if root.is_null() {
                let package_flags = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.public_flag;
                root = unsafe {
                    access.construct_object(access.native_class("Core.Package")?, ptr::null_mut(), index, package_flags, ptr::null_mut())?
                };
            }
        } else {
            while !root.is_null() {
                let parent = unsafe { (*root).outer.cast::<UObject>() };
                if parent.is_null() { break; }
                root = parent;
            }
        }
        let path = crate::native_path::path_from_units(filename)?;
        let linker = unsafe { core_loader::get_package_linker(access, &path, flags, root, None)? };
        let package = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.linker_root.cast::<UObject>() };
        (if separator.is_none() && !outer.is_null() { outer } else { package }, linker)
    } else if let Some(separator) = separator {
        let package_name = &text[..separator];
        if package_name.is_empty() { return Err(RuntimeError::new("Package name is empty")); }
        let package_name_index = access.intern(package_name, FindName::Add)?;
        let root = find_package_root(access, package_name_index)?;
        if !root.is_null() {
            if let Some(linker) = linker_for_root(access, root) {
                (root, linker)
            } else if flags & LOAD_DISALLOW_FILES != 0 {
                (root, ptr::null_mut())
            } else {
                let linker = match get_package_linker_file(access, package_name, flags)? {
                    Some(linker) => linker,
                    // GetPackageLinker's LOAD_AllowDll path returns no linker
                    // when the module is loaded but its package file is absent.
                    // Never take this branch for an existing or broken file.
                    None if access.has_loaded_native_module(root) => ptr::null_mut(),
                    None => return missing_load(flags, text, "Package was not found"),
                };
                (root, linker)
            }
        } else if flags & LOAD_DISALLOW_FILES != 0 {
            return missing_load(flags, text, "Package was not found");
        } else {
            let Some(linker) = get_package_linker_file(access, package_name, flags)? else {
                return missing_load(flags, text, "Package was not found");
            };
            let root = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.linker_root.cast::<UObject>() };
            if root.is_null() { return Err(RuntimeError::new("Package linker has no root")); }
            (root, linker)
        }
    } else if !outer.is_null() {
        let mut root = outer;
        let mut linker = None;
        while !root.is_null() {
            if let Some(found) = linker_for_root(access, root) {
                linker = Some(found);
                break;
            }
            root = unsafe { (*root).outer.cast::<UObject>() };
        }
        (outer, linker.unwrap_or(ptr::null_mut()))
    } else {
        (access.transient_package(), ptr::null_mut())
    };
    if !sandbox.is_null() && separator.is_some() && !sandbox_supports_package(access,sandbox,target_outer)? {
        return missing_load(flags,text,"Package is not supported by the PackageMap sandbox");
    }

    let object_path = separator.map_or(text, |dot| &text[dot + 1..]);
    let mut segments = object_path.split(|unit| *unit == u32::from('.')).filter(|part| !part.is_empty()).peekable();
    let mut segment = segments.next().ok_or_else(|| RuntimeError::new("Object name is empty"))?;
    while segments.peek().is_some() {
        let name_index = access.intern(segment, FindName::Add)?;
        let mut found = unsafe { access.find_object(ptr::null_mut(), target_outer, name_index, true) };
        if found.is_null() && !linker.is_null() {
            let exports = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.export_map };
            if exports.num > 0 && exports.data.is_null() {
                return Err(RuntimeError::new("Export table has no backing storage"));
            }
            let outer_name = i32::from_ne_bytes(unsafe { (*target_outer).name });
            for row in 0..exports.num.max(0) as usize {
                let export = unsafe { ptr::read_unaligned(exports.data.cast::<FObjectExport>().add(row)) };
                if i32::from_ne_bytes(export.object_name) == name_index
                    && export_outer_name(linker, export.package_index)? == outer_name
                {
                    found = unsafe { core_loader::create_export(access, linker, row)? };
                    if !found.is_null() { break; }
                }
            }
        }
        if found.is_null() { return missing_load(flags, text, "Object outer was not found"); }
        target_outer = found;
        segment = segments.next().ok_or_else(|| RuntimeError::new("Object name is empty"))?;
    }

    let name_index = access.intern(segment, FindName::Add)?;
    let found = unsafe { access.find_object(class, target_outer, name_index, true) };
    // Source order: `Linker->Create` precedes `FindObject` in StaticLoadObject.
    // CreateExport reuses only the current linker's own export object, so a
    // by-name hit is the cached result only when it belongs to this linker
    // (or no file is involved); an object whose linker is a different, reset
    // linker is stale and must be re-created from the export table. A
    // registered native class found before its export is bound keeps falling
    // through so LinkerCreate applies its reflected properties and defaults.
    let found_linker = if found.is_null() { ptr::null_mut() } else { unsafe { (*found)._linker }.cast::<UObject>() };
    if !found.is_null()
        && (linker.is_null()
            || flags & LOAD_DISALLOW_FILES != 0
            || found_linker == linker)
    {
        if !found_linker.is_null() { unsafe { preload(access, found)?; } }
        return Ok(found);
    }
    if linker.is_null() { return missing_load(flags, text, "Object was not found"); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    let exports = unsafe { (*owner).raw.base.export_map };
    if exports.num > 0 && exports.data.is_null() {
        return Err(RuntimeError::new("Export table has no backing storage"));
    }
    // ULinkerLoad::Create uses INDEX_NONE: match class/name across package groups.
    for row in 0..exports.num.max(0) as usize {
        let export = unsafe { ptr::read_unaligned(exports.data.cast::<FObjectExport>().add(row)) };
        if i32::from_ne_bytes(export.object_name) != name_index {
            continue;
        }
        let export_class = if export.class_index == 0 {
            access.native_class("Class")?.cast()
        } else {
            unsafe { resolve_index(access, linker, export.class_index)? }
        };
        if export_class.is_null() || (!class.is_null()
            && !unsafe { crate::object_model::is_child_of(export_class.cast(), class.cast()) })
        {
            continue;
        }
        let object = unsafe { core_loader::create_export(access, linker, row)? };
        unsafe { preload(access, object)?; }
        return Ok(object);
    }
    if !found.is_null() {
        return Ok(found);
    }
    missing_load(flags, text, "Object was not found")
}

fn get_package_linker_file(
    access: CoreAccess<'_>,
    name: &[u32],
    flags: u32,
) -> RuntimeResult<Option<*mut UObject>> {
    let Some(path) = package_path(access, Path::new(&native_name_for(name)))? else {
        return Ok(None);
    };
    let linker = unsafe { core_loader::get_package_linker(access, &path, flags, ptr::null_mut(), None) }
        .map_err(|error| error.with_context("UObject::GetPackageLinker"))?;
    if linker.is_null() { return Err(RuntimeError::new("Package loader returned a null linker")); }
    Ok(Some(linker))
}

/// Loads a class export and checks its reflected ancestry against `base`.
///
/// # Safety
/// `base` and `outer` are null or live canonical objects in the same Core owner.
pub(crate) unsafe fn static_load_class(
    access: CoreAccess<'_>,
    base: *mut UClass,
    outer: *mut UObject,
    name: &[u32],
    flags: u32,
) -> RuntimeResult<*mut UClass> {
    unsafe { static_load_class_with_sandbox(access,base,outer,name,flags,ptr::null_mut()) }
}

unsafe fn static_load_class_with_sandbox(
    access: CoreAccess<'_>,
    base: *mut UClass,
    outer: *mut UObject,
    name: &[u32],
    flags: u32,
    sandbox: *mut UPackageMap,
) -> RuntimeResult<*mut UClass> {
    let class_meta = access.native_class("Class")?;
    let class_context = static_class_context(access, class_meta.cast(), outer, name)?;
    let object = unsafe { static_load_object_with_sandbox(access, class_meta.cast(), outer, name, flags, None, sandbox) }
        .map_err(|error| error.with_context("UObject::StaticLoadObject").with_context(&class_context).with_context("UObject::StaticLoadClass"))?;
    let class = object.cast::<UClass>();
    if !base.is_null() && !class.is_null() && !unsafe { crate::object_model::is_child_of(class.cast(), base.cast()) } {
        return Err(RuntimeError::new("Loaded class is not a child of the requested base").with_context(&native_name_for(name)));
    }
    Ok(class)
}

fn static_class_context(
    access: CoreAccess<'_>,
    object_class: *mut UClass,
    outer: *mut UObject,
    name: &[u32],
) -> RuntimeResult<String> {
    let object_class = object_class.cast::<UObject>();
    let class_name_index = i32::from_ne_bytes(unsafe { (*object_class).name });
    let class_name = access.name_text(class_name_index)?;
    let class_package = unsafe { (*object_class).outer.cast::<UObject>() };
    let class_name: String = if class_package.is_null() {
        class_name.into_iter().filter_map(char::from_u32).collect()
    } else {
        let package_index = i32::from_ne_bytes(unsafe { (*class_package).name });
        let package_name: String = access.name_text(package_index)?.into_iter()
            .filter_map(char::from_u32).collect();
        format!("{package_name}.{}", class_name.into_iter().filter_map(char::from_u32).collect::<String>())
    };
    let outer_name = if outer.is_null() {
        "NULL".to_owned()
    } else {
        let index = i32::from_ne_bytes(unsafe { (*outer).name });
        access.name_text(index)?.into_iter()
            .filter_map(char::from_u32)
            .collect()
    };
    let object_name = native_name_for(name);
    Ok(format!("({class_name} {object_name} {outer_name})"))
}

fn missing_load(flags: u32, name: &[u32], diagnostic: &str) -> RuntimeResult<*mut UObject> {
    const LOAD_NO_FAIL: u32 = 0x0001;
    if flags & LOAD_NO_FAIL != 0 {
        Err(RuntimeError::new(diagnostic).with_context(&native_name_for(name)))
    } else {
        Ok(ptr::null_mut())
    }
}

impl CoreAccess<'_> {
    /// Loads a reflected object using canonical lookup and its package's export map.
    ///
    /// # Safety
    /// `class` and `outer` are null or live canonical objects in this Core owner.
    pub unsafe fn static_load_object(
        self,
        class: *mut UClass,
        outer: *mut UObject,
        name: &[u32],
        flags: u32,
    ) -> RuntimeResult<*mut UObject> {
        unsafe { core_loader::with_load_scope(self, || static_load_object(self, class, outer, name, flags)) }
            .map_err(|error| error.with_context("UObject::StaticLoadObject"))
    }

    /// Loads an object using the explicit source filename independently of its object name.
    ///
    /// # Safety
    /// `class` and `outer` are null or live canonical objects in this Core owner.
    pub unsafe fn static_load_object_from_file(
        self,
        class: *mut UClass,
        outer: *mut UObject,
        name: &[u32],
        filename: &[u32],
        flags: u32,
    ) -> RuntimeResult<*mut UObject> {
        unsafe {
            core_loader::with_load_scope(self, || {
                static_load_object_with_sandbox(self, class, outer, name, flags, Some(filename), ptr::null_mut())
            })
        }.map_err(|error| error.with_context("UObject::StaticLoadObject"))
    }

    /// Loads a class export and verifies that it derives from the requested base.
    ///
    /// # Safety
    /// `base` and `outer` are null or live canonical objects in this Core owner.
    pub unsafe fn static_load_class(
        self,
        base: *mut UClass,
        outer: *mut UObject,
        name: &[u32],
        flags: u32,
    ) -> RuntimeResult<*mut UClass> {
        unsafe { core_loader::with_load_scope(self, || static_load_class(self, base, outer, name, flags)) }
    }
    /// Loads a reflected class through a canonical UPackageMap sandbox.
    ///
    /// # Safety
    /// All objects, package-map entries, and the name are live in this Core owner.
    pub unsafe fn static_load_class_with_package_map(
        self,
        base: *mut UClass,
        outer: *mut UObject,
        name: &[u32],
        flags: u32,
        sandbox: *mut UPackageMap,
    ) -> RuntimeResult<*mut UClass> {
        unsafe { core_loader::with_load_scope(self, || static_load_class_with_sandbox(self,base,outer,name,flags,sandbox)) }
    }
}
