//! Canonical `appCleanFileCache` host (`Core/Src/UnUnix.cpp:551-675`).
//!
//! Traversal order, threshold decisions and observable ordering stay in the
//! shared [`hp2_core::file_cache`] policy. This module supplies its leaves from
//! the live Core `System` configuration, Core file-manager discovery
//! (unsorted `read_dir`/`fnmatch`/`lstat` snapshot exactly as
//! `GFileManager->FindFiles(Pattern, 1, 0)` and `core_ucc.rs` implement it),
//! `GFileManager->Delete` (result ignored) and the canonical Unix
//! `GetFileAgeDays` policy. A stack-local Rust context carries `Found`/`Temp`;
//! no native service table or TLS state is published, and every live
//! `GSys->CachePath`/`CacheExt`/`PurgeCacheDays` read happens at its source
//! point so configuration changes made by log output are observed.

use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_records::SystemRecord;
use hp2_core::file_cache::{self, Hp2FileCacheLeaf, Hp2FileCacheServices};
use hp2_core::name_owner::FindName;
use hp2_core::unix_clock;
use hp2_core::unix_text::{Hp2UnixTextFailure, Hp2UnixTextServices};
use std::ffi::{c_void, CString};
use std::path::Path;
use std::os::unix::ffi::OsStrExt;
use std::{ptr, slice};

use hp2_abi::engine::generated::TArrayRaw;

/// One live `appCleanFileCache` invocation.
struct CoreFileCache<'a> {
    access: CoreAccess<'a>,
    /// Live `GSys`; every configuration value is reread from it at its source point.
    system: *mut SystemRecord,
    /// `TArray<FString> Found`, refreshed only by a pass snapshot.
    found: Vec<Vec<u32>>,
    /// Current `FString Temp`, retained across log and delete.
    temp: Vec<u32>,
    error: Option<RuntimeError>,
}

impl CoreFileCache<'_> {
    /// Validates one live `GSys` FString field and returns its code units.
    fn field_units(raw: TArrayRaw, label: &str) -> RuntimeResult<Vec<u32>> {
        if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
            return Err(RuntimeError::new(&format!("Invalid canonical System {label}")));
        }
        if raw.num == 0 {
            return Ok(Vec::new());
        }
        let units = unsafe { slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) };
        Ok(units.iter().copied().take_while(|unit| *unit != 0).collect())
    }

    /// Live `*GSys->CachePath` (`core_records::SystemRecord::cache_path`).
    fn cache_path(&self) -> RuntimeResult<Vec<u32>> {
        if self.system.is_null() {
            return Err(RuntimeError::new("appCleanFileCache has no canonical System"));
        }
        Self::field_units(
            unsafe { ptr::addr_of!((*self.system).cache_path).read_unaligned() },
            "CachePath",
        )
    }

    /// Live `*GSys->CacheExt` (`core_records::SystemRecord::cache_ext`).
    fn cache_ext(&self) -> RuntimeResult<Vec<u32>> {
        if self.system.is_null() {
            return Err(RuntimeError::new("appCleanFileCache has no canonical System"));
        }
        Self::field_units(
            unsafe { ptr::addr_of!((*self.system).cache_ext).read_unaligned() },
            "CacheExt",
        )
    }

    /// Live signed `GSys->PurgeCacheDays`.
    fn purge_cache_days(&self) -> i32 {
        if self.system.is_null() {
            return 0;
        }
        unsafe { ptr::addr_of!((*self.system).purge_cache_days).read_unaligned() }
    }

    fn snapshot(&mut self, pattern: &str) -> RuntimeResult<()> {
        self.found = find_files(pattern);
        Ok(())
    }

    /// `FString::Printf(TEXT("%s") PATH_SEPARATOR TEXT("%s"), *GSys->CachePath, *Found(i))`:
    /// reads the current CachePath and joins with an unconditional separator.
    fn temporary_path(&self, index: i32) -> RuntimeResult<Vec<u32>> {
        let index = usize::try_from(index)
            .map_err(|_| RuntimeError::new("File-cache snapshot index is negative"))?;
        let name = self
            .found
            .get(index)
            .ok_or_else(|| RuntimeError::new("File-cache snapshot index is out of range"))?;
        let mut path = self.cache_path()?;
        path.push(u32::from(std::path::MAIN_SEPARATOR));
        path.extend(name.iter().copied().take_while(|unit| *unit != 0));
        path.push(0);
        Ok(path)
    }

    fn log(&self, message: &str) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        let units: Vec<u32> = message.chars().map(u32::from).collect();
        self.access.emit_log(category, &units)
    }
}

/// `FString::operator*` path join (`UnTemplate.h:1118-1134` -> shared
/// `string_storage::path_append`): insert one separator only when the left
/// side is nonempty and does not already end with it.
fn path_join(left: &[u32], right: &[u32]) -> Vec<u32> {
    let separator = u32::from(std::path::MAIN_SEPARATOR);
    let mut joined = left.to_vec();
    if !joined.is_empty() && joined.last().copied() != Some(separator) {
        joined.push(separator);
    }
    joined.extend_from_slice(right);
    joined
}

fn units_to_string(units: &[u32]) -> RuntimeResult<String> {
    units
        .iter()
        .copied()
        .take_while(|unit| *unit != 0)
        .map(char::from_u32)
        .collect::<Option<String>>()
        .ok_or_else(|| RuntimeError::new("Invalid Unicode cache path"))
}

fn text_units(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain(std::iter::once(0)).collect()
}

/// `GFileManager->FindFiles(Pattern, 1, 0)`: unsorted directory scan with
/// `fnmatch` and the native `lstat` non-directory filter; unreadable
/// directories and non-UTF-8 names yield no entry, in the same order and with
/// the same filtering as the Core registry traversal (`core_ucc.rs:240-266`).
fn find_files(pattern: &str) -> Vec<Vec<u32>> {
    let spec = Path::new(pattern);
    let directory = spec
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let Some(file_pattern) = spec.file_name() else {
        return Vec::new();
    };
    let Ok(pattern) = CString::new(file_pattern.as_bytes()) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return files;
    };
    for entry in entries {
        let Ok(entry) = entry else { break };
        let name = entry.file_name();
        let Some(name_text) = name.to_str() else { continue };
        let Ok(candidate) = CString::new(name.as_bytes()) else { continue };
        // SAFETY: both C strings are live NUL-terminated byte strings.
        if unsafe { libc::fnmatch(pattern.as_ptr(), candidate.as_ptr(), 0) } != 0 {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else { continue };
        if !metadata.is_dir() {
            files.push(text_units(name_text));
        }
    }
    files
}

unsafe extern "C" fn cache_allocate(
    _opaque: *mut c_void,
    size: usize,
    out: *mut *mut c_void,
) -> i32 {
    if out.is_null() {
        return 1;
    }
    let allocation = unsafe { libc::malloc(size.max(1)) };
    if allocation.is_null() {
        return 1;
    }
    unsafe { out.write(allocation) };
    0
}

unsafe extern "C" fn cache_fatal(
    _opaque: *mut c_void,
    _failure: Hp2UnixTextFailure,
) -> i32 {
    1
}

/// Canonical `GetFileAgeDays` over stack-local text services.
fn file_age_days(path: &[u32]) -> RuntimeResult<i32> {
    let services = Hp2UnixTextServices {
        opaque: ptr::null_mut(),
        allocate: cache_allocate,
        fatal: cache_fatal,
    };
    let units = hp2_core::native_text::tchar_units(path);
    unsafe { unix_clock::file_age_days(&services, units.as_ptr()) }
        .map_err(|_| RuntimeError::new("GetFileAgeDays text conversion failed"))
}

/// `GFileManager->Delete(Temp)`: the native result is ignored.
fn delete_file(path: &[u32]) -> RuntimeResult<()> {
    let text = units_to_string(path)?;
    let Ok(candidate) = CString::new(text) else {
        return Ok(());
    };
    // SAFETY: a NUL-terminated path; the result is deliberately ignored.
    let _ = unsafe { libc::unlink(candidate.as_ptr()) };
    Ok(())
}

fn handle_entry(
    context: &mut CoreFileCache<'_>,
    leaf: Hp2FileCacheLeaf,
    argument: i32,
) -> RuntimeResult<i32> {
    use Hp2FileCacheLeaf as L;
    match leaf {
        L::FoundCount => Ok(context.found.len() as i32),
        L::TemporaryPath | L::ExpiredPath => {
            context.temp = context.temporary_path(argument)?;
            Ok(0)
        }
        L::LogTemporary => {
            let text = units_to_string(&context.temp)?;
            context.log(&format!("Deleting temporary file: {text}"))?;
            Ok(0)
        }
        L::DeleteCurrent => delete_file(&context.temp).map(|_| 0),
        L::PurgeCacheDays => Ok(context.purge_cache_days()),
        L::AgeDays => file_age_days(&context.temp),
        L::LogExpired => {
            let text = units_to_string(&context.temp)?;
            context.log(&format!(
                "Purging outdated file from cache: {text} ({argument} days old)"
            ))?;
            Ok(0)
        }
        L::FinishExpiredPath => {
            context.temp.clear();
            Ok(0)
        }
        L::RunTemporaryPass | L::RunExpiredPass => Err(RuntimeError::new(
            "File-cache pass leaf reached the entry dispatcher",
        )),
    }
}

unsafe fn run_pass(opaque: *mut c_void, temporary: bool) -> RuntimeResult<i32> {
    {
        // The snapshot is taken inside the pass guard, before any entry leaf,
        // from the configuration live at that moment.
        let context = unsafe { &mut *opaque.cast::<CoreFileCache<'_>>() };
        let pattern = if temporary {
            // `FString::Printf("%s" PATH_SEPARATOR "*.tmp", *GSys->CachePath)`.
            let mut pattern = units_to_string(&context.cache_path()?)?;
            pattern.push(std::path::MAIN_SEPARATOR);
            pattern.push_str("*.tmp");
            pattern
        } else {
            // `*(GSys->CachePath * TEXT("*") + GSys->CacheExt)`: FString operator*
            // path-joins, then plain concatenation appends CacheExt.
            let mut pattern = path_join(&context.cache_path()?, &[u32::from(b'*')]);
            pattern.extend(context.cache_ext()?);
            units_to_string(&pattern)?
        };
        context.snapshot(&pattern)?;
    }
    let services = Hp2FileCacheServices {
        context: opaque,
        call: file_cache_leaf,
    };
    let result = if temporary {
        unsafe { file_cache::clean_temporary_pass(&services) }
    } else {
        unsafe { file_cache::clean_expired_pass(&services) }
    };
    result.map_err(|_| RuntimeError::new("File-cache cleanup pass failed"))?;
    Ok(0)
}

unsafe extern "C" fn file_cache_leaf(
    opaque: *mut c_void,
    leaf: Hp2FileCacheLeaf,
    argument: i32,
    out: *mut i32,
) -> i32 {
    let result = match leaf {
        Hp2FileCacheLeaf::RunTemporaryPass => unsafe { run_pass(opaque, true) },
        Hp2FileCacheLeaf::RunExpiredPass => unsafe { run_pass(opaque, false) },
        _ => {
            let context = unsafe { &mut *opaque.cast::<CoreFileCache<'_>>() };
            handle_entry(context, leaf, argument)
        }
    };
    match result {
        Ok(value) => {
            if !out.is_null() {
                unsafe { out.write(value) };
            }
            0
        }
        Err(error) => {
            let context = unsafe { &mut *opaque.cast::<CoreFileCache<'_>>() };
            if context.error.is_none() {
                context.error = Some(error);
            }
            1
        }
    }
}

/// Runs the canonical `appCleanFileCache` sequence on Core records.
///
/// # Safety
/// The Core owner and its live System configuration remain valid for the call.
pub unsafe fn clean_file_cache(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let system = access.system()?;
    if system.is_null() {
        return Err(RuntimeError::new("appCleanFileCache has no canonical System"));
    }
    let mut context = CoreFileCache {
        access,
        system,
        found: Vec::new(),
        temp: Vec::new(),
        error: None,
    };
    let services = Hp2FileCacheServices {
        context: (&mut context as *mut CoreFileCache<'_>).cast(),
        call: file_cache_leaf,
    };
    let result = unsafe { file_cache::clean_file_cache(&services) };
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    result.map_err(|_| RuntimeError::new("appCleanFileCache failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).collect()
    }

    #[test]
    fn find_files_matches_native_filter() {
        let root = std::env::temp_dir().join(format!(
            "hp2-file-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("a.tmp"), b"a").unwrap();
        std::fs::write(root.join("b.tmp"), b"b").unwrap();
        std::fs::write(root.join("keep.uxx"), b"c").unwrap();
        std::fs::write(root.join("other.bin"), b"d").unwrap();
        let pattern = format!("{}{}*.tmp", root.display(), std::path::MAIN_SEPARATOR);
        let mut names: Vec<String> = find_files(&pattern)
            .iter()
            .map(|units| units_to_string(units).unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.tmp".to_string(), "b.tmp".to_string()]);
        let expired = format!("{}*.uxx", root.display());
        let names: Vec<String> = find_files(&expired)
            .iter()
            .map(|units| units_to_string(units).unwrap())
            .collect();
        assert_eq!(names, vec!["keep.uxx".to_string()]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn path_join_matches_fstring_operator_multiply() {
        let separator = u32::from(std::path::MAIN_SEPARATOR);
        let star = [u32::from(b'*')];
        let mut expected = units("/cache");
        expected.push(separator);
        expected.push(u32::from(b'*'));
        assert_eq!(path_join(&units("/cache"), &star), expected);
        let mut trailing = units("/cache");
        trailing.push(separator);
        let mut expected = trailing.clone();
        expected.push(u32::from(b'*'));
        assert_eq!(path_join(&trailing, &star), expected);
        assert_eq!(path_join(&[], &star), star.to_vec());
    }
}
