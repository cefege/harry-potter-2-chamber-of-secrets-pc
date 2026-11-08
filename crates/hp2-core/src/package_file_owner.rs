//! Canonical `appFindPackageFile` leaves over live Core.System and Unix I/O.

use crate::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_records::SystemRecord,
    name_owner::FindName,
    native_path, native_text,
    package_file::{self, Hp2PackageFileServices, PackageFileComplete, PackageFileVisitor},
    unix_file_manager_owner,
};
use hp2_abi::engine::generated::TArrayRaw;
use std::{borrow::Cow, ffi::c_void, path::Path, ptr, slice};

/// Resolves the native game's System working directory without changing process CWD.
pub fn system_path<'p>(access: CoreAccess<'_>, path: &'p Path) -> Cow<'p, Path> {
    if path.is_absolute() {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(access.data_root().join("System").join(path))
    }
}

/// The native platform's package-excluded dynamic-library suffix, including NUL.
pub fn dll_extension() -> &'static [u32] {
    if cfg!(target_os = "macos") {
        crate::lit!(".dylib")
    } else {
        crate::lit!(".so")
    }
}

struct Context<'a> {
    access: CoreAccess<'a>,
    system: *mut SystemRecord,
    language: Vec<u32>,
    cd_path: [u32; 256],
    guid: [u32; 33],
    failure: Option<RuntimeError>,
    resolved_path: Option<std::path::PathBuf>,
}

/// Runs the shared native search, preserving its bounded output and GUID-cache policy.
pub fn find_package_file(
    access: CoreAccess<'_>,
    name: &[u32],
    guid: Option<&[u32; 4]>,
    output: &mut [u32; 256],
) -> RuntimeResult<bool> {
    find_package_file_impl(access, name, guid, output).map(|(found, _)| found)
}

/// Resolves native case recovery without repeating the filesystem search.
pub(crate) fn find_package_path(
    access: CoreAccess<'_>,
    name: &[u32],
) -> RuntimeResult<Option<std::path::PathBuf>> {
    let (found, path) = find_package_file_impl(access, name, None, &mut [0; 256])?;
    if !found {
        return Ok(None);
    }
    path.map(Some)
        .ok_or_else(|| RuntimeError::new("Package lookup found no resolved filesystem path"))
}

fn find_package_file_impl(
    access: CoreAccess<'_>,
    name: &[u32],
    guid: Option<&[u32; 4]>,
    output: &mut [u32; 256],
) -> RuntimeResult<(bool, Option<std::path::PathBuf>)> {
    let input = if name.last() == Some(&0) {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(name.iter().copied().chain([0]).collect::<Vec<_>>())
    };
    let mut language = access.language_text()?;
    language.push(0);
    let mut context = Context {
        access,
        system: access.system()?,
        language,
        cd_path: [0; 256],
        guid: [0; 33],
        failure: None,
        resolved_path: None,
    };
    unsafe {
        crate::text_parse::parse_string(
            native_text::tchar_units(access.command_line()).as_ptr(),
            crate::lit!("CDPATH=").as_ptr().cast(),
            context.cd_path.as_mut_ptr().cast(),
            256,
        );
    }
    if let Some(guid) = guid {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for (word, value) in guid.iter().enumerate() {
            for digit in 0..8 {
                context.guid[word * 8 + digit] =
                    u32::from(HEX[((value >> ((7 - digit) * 4)) & 15) as usize]);
            }
        }
    }
    let result = unsafe {
        package_file::find_package_file(
            &SERVICES,
            ptr::from_mut(&mut context).cast(),
            input.as_ptr().cast(),
            guid.is_some(),
            output.as_mut_ptr().cast(),
            output.len(),
        )
    };
    match result {
        Ok(found) => Ok((found, context.resolved_path)),
        Err(status) => Err(context.failure.take().unwrap_or_else(|| {
            RuntimeError::new("Canonical package file lookup failed")
                .with_context(&status.to_string())
        })),
    }
}

unsafe fn context<'a>(raw: *mut c_void) -> &'a mut Context<'a> {
    unsafe { &mut *raw.cast::<Context<'a>>() }
}

unsafe fn units<'a>(raw: *const c_void) -> &'a [u32] {
    let raw = raw.cast::<u32>();
    unsafe { slice::from_raw_parts(raw, native_text::text_len(raw)) }
}

fn status(raw: *mut c_void, result: RuntimeResult<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            let context = unsafe { context(raw) };
            if context.failure.is_none() {
                context.failure = Some(error);
            }
            1
        }
    }
}

unsafe extern "C" fn string(
    raw: *mut c_void,
    kind: i32,
    index: i32,
    out: *mut *const c_void,
) -> i32 {
    let context = unsafe { context(raw) };
    let system = context.system;
    let value = match kind {
        0 => context.language.as_ptr(),
        1 => context.cd_path.as_ptr(),
        2 => {
            let paths = unsafe { ptr::read_unaligned(ptr::addr_of!((*system).paths)) };
            if index < 0 || index >= paths.num {
                return status(
                    raw,
                    Err(RuntimeError::new("Core.System.Paths index is out of range")),
                );
            }
            unsafe { native_text::fstring_text(paths.data.cast::<TArrayRaw>().add(index as usize)) }
        }
        3 => unsafe { native_text::fstring_text(ptr::addr_of!((*system).cache_path)) },
        4 => unsafe { native_text::fstring_text(ptr::addr_of!((*system).cache_ext)) },
        5 => context.guid.as_ptr(),
        6 => dll_extension().as_ptr(),
        7 => crate::lit!("/").as_ptr(),
        _ => {
            return status(
                raw,
                Err(RuntimeError::new("Invalid package file string operation")),
            );
        }
    };
    unsafe {
        out.write(value.cast());
    }
    0
}

unsafe extern "C" fn count(raw: *mut c_void, out: *mut i32) -> i32 {
    let system = unsafe { context(raw) }.system;
    let paths = unsafe { ptr::read_unaligned(ptr::addr_of!((*system).paths)) };
    unsafe {
        out.write(paths.num);
    }
    0
}

unsafe extern "C" fn compare(
    _raw: *mut c_void,
    left: *const c_void,
    right: *const c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(crate::ansi::stricmp(left.cast(), right.cast()));
    }
    0
}

unsafe extern "C" fn size(raw: *mut c_void, path: *const c_void, out: *mut i32) -> i32 {
    let access = unsafe { context(raw) }.access;
    let result = (|| {
        let path = native_path::path_from_units(unsafe { units(path) })?;
        let (size, resolved) =
            unix_file_manager_owner::file_size_resolved(access, &system_path(access, &path))?;
        if size >= 0 {
            unsafe { context(raw) }.resolved_path = resolved;
        }
        unsafe {
            out.write(size);
        }
        Ok(())
    })();
    status(raw, result)
}

unsafe extern "C" fn enumerate(
    raw: *mut c_void,
    spec: *const c_void,
    visitor: PackageFileVisitor,
    complete: PackageFileComplete,
    visitor_context: *mut c_void,
) -> i32 {
    let access = unsafe { context(raw) }.access;
    let files = (|| {
        let path = native_path::path_from_units(unsafe { units(spec) })?;
        unix_file_manager_owner::find_files(access, &system_path(access, &path))
    })();
    let files = match files {
        Ok(files) => files,
        Err(error) => return status(raw, Err(error)),
    };
    // No Context borrow survives visitor reentry into size/diagnostic/touch.
    for mut name in files {
        name.push(0);
        let status = unsafe { visitor(visitor_context, name.as_ptr().cast()) };
        if status != 0 {
            return status;
        }
    }
    unsafe { complete(visitor_context) }
}

unsafe extern "C" fn touch(raw: *mut c_void, path: *const c_void) -> i32 {
    let access = unsafe { context(raw) }.access;
    let result = (|| {
        let path = native_path::path_from_units(unsafe { units(path) })?;
        // appUpdateFileModTime opens/deletes an append writer with GNull; its
        // false return is ignored by CorePackageFileTouch, not a lookup failure.
        let _ = unix_file_manager_owner::open_archive_with_flags(
            access,
            &system_path(access, &path),
            true,
            0x10,
        )?;
        Ok(())
    })();
    status(raw, result)
}

unsafe extern "C" fn diagnostic(
    raw: *mut c_void,
    input: *const c_void,
    output: *const c_void,
) -> i32 {
    let access = unsafe { context(raw) }.access;
    let result = (|| {
        let input = native_path::from_utf32(unsafe { units(input) })?;
        let output = native_path::from_utf32(unsafe { units(output) })?;
        let category = access.intern_str("Log", FindName::Add)?;
        let message: Vec<u32> = format!("Case-insensitive search: {input} -> {output}")
            .chars()
            .map(u32::from)
            .collect();
        access.emit_log(category, &message)
    })();
    status(raw, result)
}

unsafe extern "C" fn too_long(raw: *mut c_void, path: *const c_void, capacity: i32) -> i32 {
    let access = unsafe { context(raw) }.access;
    let result = (|| {
        let path = native_path::from_utf32(unsafe { units(path) })?;
        let category = access.intern_str("Warning", FindName::Add)?;
        let message: Vec<u32> = format!(
            "appFindPackageFile: found {path}, which needs more than {capacity} characters"
        )
        .chars()
        .map(u32::from)
        .collect();
        access.emit_log(category, &message)
    })();
    status(raw, result)
}

static SERVICES: Hp2PackageFileServices = Hp2PackageFileServices {
    string,
    path_count: count,
    compare,
    file_size: size,
    enumerate,
    touch,
    diagnostic,
    too_long,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_access::{CoreOptions, CoreOwner};
    use std::{fs, path::PathBuf};

    #[test]
    fn configured_asset_packages_resolve_relative_to_system() {
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("package-paths-{}", std::process::id()));
        fs::create_dir_all(root.join("System")).unwrap();
        fs::create_dir_all(root.join("Textures")).unwrap();
        fs::create_dir_all(root.join("Maps")).unwrap();
        fs::create_dir_all(root.join("Cache")).unwrap();
        fs::write(root.join("System/Default.ini"),
            "[Core.System]\nPaths=../Textures/*.utx\nPaths=../Maps/*.unr\nCachePath=../Cache\nCacheExt=.uxx\n").unwrap();
        fs::write(root.join("System/DefUser.ini"), "").unwrap();
        fs::write(
            root.join("Textures/NativeProbe.utx"),
            b"actual asset candidate",
        )
        .unwrap();
        fs::write(root.join("Maps/NativeProbe.unr"), b"later map candidate").unwrap();
        // Exercise the public relative-data-root entry, without changing global CWD.
        let mut relative_root = PathBuf::new();
        for _ in std::env::current_dir().unwrap().ancestors().skip(1) {
            relative_root.push("..");
        }
        relative_root.push(root.strip_prefix("/").unwrap());
        let owner = CoreOwner::new(CoreOptions {
            data_root: relative_root,
            system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"),
            command_line: Vec::new(),
            output: |_, _, _| Ok(()),
            log_file: Some(fs::File::create(root.join("runtime.log")).unwrap()),
        })
        .unwrap();
        let access = owner.access();
        let mut filename = [0; 256];
        assert!(
            find_package_file(access, crate::lit!("NativeProbe"), None, &mut filename).unwrap(),
            "Core.System.Paths={:?}; config={:?}",
            unsafe { (*access.system().unwrap()).paths.num },
            access.config_section_units(crate::lit!("Core.System"), None)
        );
        let path = native_path::path_from_units(&filename).unwrap();
        assert_eq!(
            fs::read(system_path(access, &path)).unwrap(),
            b"actual asset candidate"
        );
        assert!(
            find_package_file(access, crate::lit!("NativeProbe.unr"), None, &mut filename).unwrap()
        );
        let path = native_path::path_from_units(&filename).unwrap();
        assert_eq!(
            fs::read(system_path(access, &path)).unwrap(),
            b"later map candidate"
        );
        let recovered = find_package_path(access, crate::lit!("nAtIvEpRoBe.uNr"))
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(recovered).unwrap(), b"later map candidate");

        fs::write(
            root.join("Textures/NativeLocale.frt_utx"),
            b"localized package",
        )
        .unwrap();
        fs::write(
            root.join("Textures/NativeLocale.utx"),
            b"international package",
        )
        .unwrap();
        access.set_language(crate::lit!("frt")).unwrap();
        let localized = find_package_path(access, crate::lit!("NativeLocale"))
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(localized).unwrap(), b"localized package");
        access.set_language(crate::lit!("int")).unwrap();
        let international = find_package_path(access, crate::lit!("NativeLocale"))
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(international).unwrap(), b"international package");

        // A real filesystem hit beyond native's 256-TCHAR output is rejected,
        // not truncated into a different package filename.
        let long_name = "x".repeat(242);
        fs::write(
            root.join("Textures").join(format!("{long_name}.utx")),
            b"long asset",
        )
        .unwrap();
        let long_name: Vec<u32> = long_name.chars().map(u32::from).collect();
        filename.fill(0x7e);
        assert!(!find_package_file(access, &long_name, None, &mut filename).unwrap());
        assert_eq!(filename[0], 0);

        // Lookups read the live rooted System object, not a cached startup list.
        let system = access.system().unwrap();
        let paths = unsafe { ptr::read_unaligned(ptr::addr_of!((*system).paths)) };
        unsafe {
            access
                .assign_fstring(paths.data.cast(), crate::lit!("../Maps/*.unr"))
                .unwrap();
        }
        assert!(
            find_package_file(access, crate::lit!("NativeProbe"), None, &mut filename).unwrap()
        );
        let path = native_path::path_from_units(&filename).unwrap();
        assert_eq!(
            fs::read(system_path(access, &path)).unwrap(),
            b"later map candidate"
        );

        let cache = root.join("Cache/0123456789ABCDEF0000000000000001.uxx");
        fs::write(&cache, b"cache bytes must survive append-open touch").unwrap();
        let guid = [0x01234567, 0x89abcdef, 0, 1];
        assert!(
            find_package_file(
                access,
                crate::lit!("CachedOnly"),
                Some(&guid),
                &mut filename
            )
            .unwrap()
        );
        let path = native_path::path_from_units(&filename).unwrap();
        assert_eq!(
            fs::canonicalize(system_path(access, &path)).unwrap(),
            fs::canonicalize(&cache).unwrap()
        );
        assert_eq!(
            fs::read(cache).unwrap(),
            b"cache bytes must survive append-open touch"
        );
        assert!(
            !find_package_file(access, crate::lit!("CachedOnly"), None, &mut filename).unwrap()
        );
        owner.shutdown().unwrap();
    }
}
