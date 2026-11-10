use super::{
    CACHE, CRLF, FILE, HEADER, Hp2ConfigArgs, Hp2ConfigOperation, Hp2ConfigResult,
    Hp2ConfigServices, PAIR, execute, find_tagged,
};
use crate::ansi::{self, Tchar};
use std::ffi::{c_char, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

/// Native `FString`: NUL-terminated host TCHARs.
struct FStr {
    text: Vec<Tchar>,
}

/// Native `FConfigSection` (`TMultiMap<FString,FString>`, insertion-ordered).
struct FSection {
    values: Vec<(Vec<Tchar>, Box<FStr>)>,
}

/// Native `FConfigFile` (`TMap<FString,FConfigSection>` plus flags).
struct FFile {
    sections: Vec<(Vec<Tchar>, Box<FSection>)>,
    dirty: i32,
    no_save: i32,
}

/// Native `FConfigCacheIni` (`TMap<FString,FConfigFile>` plus the two aliases).
struct FCache {
    files: Vec<(Vec<Tchar>, Box<FFile>)>,
    system: FStr,
    user: FStr,
}

/// Native `FConfigContext`, owner of the call's scratch `FString`.
struct FContext {
    scratch: FStr,
}

impl Default for FSection {
    fn default() -> Self {
        Self { values: Vec::new() }
    }
}

impl Default for FFile {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            dirty: 0,
            no_save: 0,
        }
    }
}

impl Default for FContext {
    fn default() -> Self {
        Self {
            scratch: FStr { text: vec![0] },
        }
    }
}

fn terminate(values: &[Tchar]) -> Vec<Tchar> {
    let end = values
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(values.len());
    let mut text = values[..end].to_vec();
    text.push(0);
    text
}

fn wide(text: &str) -> Vec<Tchar> {
    terminate(&text.chars().map(|value| value as Tchar).collect::<Vec<_>>())
}

fn as_text(values: &[Tchar]) -> String {
    values
        .iter()
        .take_while(|unit| **unit != 0)
        .map(|unit| char::from_u32(*unit as u32).unwrap_or('\u{fffd}'))
        .collect()
}

fn assert_text(actual: &[Tchar], expected: &str, context: &str) {
    assert_eq!(as_text(actual), expected, "{context}");
}

/// Reads a NUL-terminated native string without its terminator.
unsafe fn read_units(text: *const Tchar) -> Vec<Tchar> {
    let mut length = 0;
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    unsafe { std::slice::from_raw_parts(text, length) }.to_vec()
}

unsafe fn terminate_ptr(text: *const Tchar) -> Vec<Tchar> {
    let mut units = unsafe { read_units(text) };
    units.push(0);
    units
}

fn path_of(text: *const Tchar) -> PathBuf {
    let units = unsafe { read_units(text) };
    let mut value = String::with_capacity(units.len());
    for unit in units {
        value.push(char::from_u32(unit as u32).unwrap_or('\u{fffd}'));
    }
    PathBuf::from(value)
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hp2-config-ini-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn path_wide(dir: &Path, name: &str) -> Vec<Tchar> {
    wide(&dir.join(name).to_string_lossy())
}

// ---- native leaves (same shape as UnConfigIni.cpp's Config* callbacks) ----

unsafe fn host_context() -> *mut c_void {
    Box::into_raw(Box::new(FContext::default())).cast()
}

unsafe extern "C" fn host_find(
    kind: i32,
    map: *mut c_void,
    key: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let name = key.cast::<Tchar>();
        *out = match kind {
            CACHE => {
                let cache = &mut *map.cast::<FCache>();
                cache
                    .files
                    .iter_mut()
                    .find(|(stored, _)| ansi::stricmp(stored.as_ptr(), name) == 0)
                    .map_or(null_mut(), |(_, file)| (&mut **file as *mut FFile).cast())
            }
            FILE => {
                let file = &mut *map.cast::<FFile>();
                file.sections
                    .iter_mut()
                    .find(|(stored, _)| ansi::stricmp(stored.as_ptr(), name) == 0)
                    .map_or(null_mut(), |(_, section)| {
                        (&mut **section as *mut FSection).cast()
                    })
            }
            _ => {
                let section = &mut *map.cast::<FSection>();
                section
                    .values
                    .iter_mut()
                    .rev()
                    .find(|(stored, _)| ansi::stricmp(stored.as_ptr(), name) == 0)
                    .map_or(null_mut(), |(_, value)| (&mut **value as *mut FStr).cast())
            }
        };
        0
    }
}

unsafe extern "C" fn host_insert(
    kind: i32,
    map: *mut c_void,
    key: *const c_void,
    value: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let name = terminate_ptr(key.cast::<Tchar>());
        match kind {
            CACHE => {
                let cache = &mut *map.cast::<FCache>();
                let mut file = Box::new(FFile::default());
                let pointer = (&mut *file as *mut FFile).cast::<c_void>();
                cache.files.push((name, file));
                *out = pointer;
            }
            FILE => {
                let file = &mut *map.cast::<FFile>();
                let mut section = Box::new(FSection::default());
                let pointer = (&mut *section as *mut FSection).cast::<c_void>();
                file.sections.push((name, section));
                *out = pointer;
            }
            _ => {
                let section = &mut *map.cast::<FSection>();
                let mut slot = Box::new(FStr {
                    text: terminate_ptr(value.cast::<Tchar>()),
                });
                let pointer = (&mut *slot as *mut FStr).cast::<c_void>();
                section.values.push((name, slot));
                *out = pointer;
            }
        }
        0
    }
}

unsafe extern "C" fn host_empty(kind: i32, map: *mut c_void) -> i32 {
    unsafe {
        match kind {
            CACHE => (*map.cast::<FCache>()).files.clear(),
            FILE => (*map.cast::<FFile>()).sections.clear(),
            _ => (*map.cast::<FSection>()).values.clear(),
        }
        0
    }
}

unsafe extern "C" fn host_remove(cache: *mut c_void, key: *const c_void) -> i32 {
    unsafe {
        let cache = &mut *cache.cast::<FCache>();
        let name = key.cast::<Tchar>();
        cache
            .files
            .retain(|(stored, _)| ansi::stricmp(stored.as_ptr(), name) != 0);
        0
    }
}

unsafe extern "C" fn host_count(kind: i32, map: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = match kind {
            CACHE => (*map.cast::<FCache>()).files.len(),
            FILE => (*map.cast::<FFile>()).sections.len(),
            _ => (*map.cast::<FSection>()).values.len(),
        } as i32;
        0
    }
}

unsafe extern "C" fn host_pair(
    kind: i32,
    map: *mut c_void,
    index: i32,
    key: *mut *const c_void,
    value: *mut *mut c_void,
) -> i32 {
    unsafe {
        let index = index as usize;
        match kind {
            CACHE => {
                let cache = &mut *map.cast::<FCache>();
                let (stored, file) = &mut cache.files[index];
                *key = stored.as_ptr().cast();
                *value = (&mut **file as *mut FFile).cast();
            }
            FILE => {
                let file = &mut *map.cast::<FFile>();
                let (stored, section) = &mut file.sections[index];
                *key = stored.as_ptr().cast();
                *value = (&mut **section as *mut FSection).cast();
            }
            _ => {
                let section = &mut *map.cast::<FSection>();
                let (stored, slot) = &mut section.values[index];
                *key = stored.as_ptr().cast();
                *value = (&mut **slot as *mut FStr).cast();
            }
        }
        0
    }
}

unsafe extern "C" fn host_flags(
    file: *mut c_void,
    dirty: *mut *mut i32,
    no_save: *mut *mut i32,
) -> i32 {
    unsafe {
        let file = &mut *file.cast::<FFile>();
        *dirty = &mut file.dirty;
        *no_save = &mut file.no_save;
        0
    }
}

unsafe extern "C" fn host_default_string(
    cache: *mut c_void,
    user: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let cache = &mut *cache.cast::<FCache>();
        let string: *mut FStr = if user != 0 {
            &mut cache.user
        } else {
            &mut cache.system
        };
        *out = string.cast();
        0
    }
}

unsafe extern "C" fn host_text(string: *mut c_void, out: *mut *const c_void) -> i32 {
    unsafe {
        let text: *const Tchar = (*string.cast::<FStr>()).text.as_ptr();
        *out = text.cast();
        0
    }
}

unsafe extern "C" fn host_assign(string: *mut c_void, text: *const c_void) -> i32 {
    unsafe {
        (*string.cast::<FStr>()).text = terminate_ptr(text.cast::<Tchar>());
        0
    }
}

unsafe extern "C" fn host_compare(
    left: *const c_void,
    right: *const c_void,
    sensitive: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        let left = left.cast::<Tchar>();
        let right = right.cast::<Tchar>();
        *out = if sensitive != 0 {
            ansi::strcmp(left, right)
        } else {
            ansi::stricmp(left, right)
        };
        0
    }
}

unsafe extern "C" fn host_file_size(name: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = fs::metadata(path_of(name.cast::<Tchar>()))
            .map(|meta| meta.len() as i32)
            .unwrap_or(-1);
        0
    }
}

unsafe extern "C" fn host_scratch(context: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = (&mut (*context.cast::<FContext>()).scratch as *mut FStr).cast();
        0
    }
}

unsafe extern "C" fn host_load(context: *mut c_void, name: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let path = path_of(name.cast::<Tchar>());
        let loaded = fs::read(&path)
            .ok()
            .and_then(|bytes| crate::file_persistence::decode_ini_text(&bytes));
        match loaded {
            Some(text) => {
                (*context.cast::<FContext>()).scratch.text = terminate(&text);
                *out = 1;
            }
            None => *out = 0,
        }
        0
    }
}

unsafe extern "C" fn host_save(context: *mut c_void, name: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let path = path_of(name.cast::<Tchar>());
        let scratch = &(*context.cast::<FContext>()).scratch.text;
        let length = scratch
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(scratch.len());
        *out = i32::from(crate::file_persistence::save_text_to_path(
            &path,
            &scratch[..length],
        ));
        0
    }
}

unsafe extern "C" fn host_append_format(
    context: *mut c_void,
    format: *const c_void,
    key: *const c_void,
    value: *const c_void,
) -> i32 {
    unsafe {
        let text = &mut (*context.cast::<FContext>()).scratch.text;
        let end = text
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(text.len());
        text.truncate(end);
        let format = format.cast::<Tchar>();
        if ansi::strcmp(format, HEADER.as_ptr()) == 0 {
            text.push(b'[' as Tchar);
            text.extend(read_units(key.cast::<Tchar>()));
            text.extend([b']' as Tchar, 13, 10]);
        } else if ansi::strcmp(format, PAIR.as_ptr()) == 0 {
            text.extend(read_units(key.cast::<Tchar>()));
            text.push(b'=' as Tchar);
            text.extend(read_units(value.cast::<Tchar>()));
            text.extend([13, 10]);
        } else if ansi::strcmp(format, CRLF.as_ptr()) == 0 {
            text.extend([13, 10]);
        } else {
            return 1;
        }
        text.push(0);
        0
    }
}

unsafe extern "C" fn host_convert(kind: i32, text: *const c_void, value: *mut c_void) -> i32 {
    unsafe {
        let text = text.cast::<Tchar>();
        if kind == 0 {
            *value.cast::<i32>() = match ansi::atoi_checked(text) {
                Ok(number) => number,
                Err(number) => number as i32,
            };
        } else {
            let ansi_text: *const c_char = ansi::to_ansi_string(text).cast();
            *value.cast::<f32>() = ansi::libc::atof(ansi_text) as f32;
        }
        0
    }
}

unsafe extern "C" fn host_format(kind: i32, value: *const c_void, text: *mut c_void) -> i32 {
    unsafe {
        let out = text.cast::<Tchar>();
        if kind == 0 {
            let format = [37 as Tchar, 105, 0];
            ansi::libc::swprintf(out, 30, format.as_ptr(), *value.cast::<i32>());
        } else {
            let format = [37 as Tchar, 102, 0];
            ansi::libc::swprintf(out, 30, format.as_ptr(), *value.cast::<f32>() as f64);
        }
        0
    }
}

unsafe extern "C" fn host_log(_output: *mut c_void, _text: *const c_void) -> i32 {
    0
}

unsafe extern "C" fn host_dump_map(_cache: *mut c_void, _output: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn host_format_pair(
    output: *mut c_void,
    key: *const c_void,
    value: *const c_void,
    written: *mut i32,
) -> i32 {
    unsafe {
        let mut index = 0;
        let out = output.cast::<Tchar>();
        for unit in read_units(key.cast::<Tchar>()) {
            *out.add(index) = unit;
            index += 1;
        }
        *out.add(index) = b'=' as Tchar;
        index += 1;
        for unit in read_units(value.cast::<Tchar>()) {
            *out.add(index) = unit;
            index += 1;
        }
        *out.add(index) = 0;
        *written = index as i32;
        0
    }
}

unsafe extern "C" fn host_get_string(
    cache: *mut c_void,
    section: *const c_void,
    key: *const c_void,
    output: *mut c_void,
    size: i32,
    filename: *const c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        let mut a = args(cache);
        a.section = section;
        a.key = key;
        a.filename = filename;
        a.output = output;
        a.size = size;
        match execute(
            &SERVICES,
            host_context(),
            Hp2ConfigOperation::Hp2ConfigGetString,
            &a,
        ) {
            Ok(result) => {
                *out = result.integer;
                0
            }
            Err(code) => code,
        }
    }
}

unsafe extern "C" fn host_set_string(
    cache: *mut c_void,
    section: *const c_void,
    key: *const c_void,
    value: *const c_void,
    filename: *const c_void,
) -> i32 {
    unsafe {
        let mut a = args(cache);
        a.section = section;
        a.key = key;
        a.value = value;
        a.filename = filename;
        match execute(
            &SERVICES,
            host_context(),
            Hp2ConfigOperation::Hp2ConfigSetString,
            &a,
        ) {
            Ok(_) => 0,
            Err(code) => code,
        }
    }
}

unsafe extern "C" fn host_flush(cache: *mut c_void, read: i32, filename: *const c_void) -> i32 {
    unsafe {
        let mut a = args(cache);
        a.filename = filename;
        a.flag = read;
        match execute(
            &SERVICES,
            host_context(),
            Hp2ConfigOperation::Hp2ConfigFlush,
            &a,
        ) {
            Ok(_) => 0,
            Err(code) => code,
        }
    }
}

unsafe extern "C" fn host_find_scoped(
    services: *const Hp2ConfigServices,
    cache: *mut c_void,
    filename: *const c_void,
    create: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        match find_tagged(&*services, host_context(), cache, filename, create) {
            Ok(file) => {
                *out = file;
                0
            }
            Err(code) => code,
        }
    }
}

static SERVICES: Hp2ConfigServices = Hp2ConfigServices {
    find: host_find,
    insert: host_insert,
    empty: host_empty,
    remove: host_remove,
    count: host_count,
    pair: host_pair,
    flags: host_flags,
    default_string: host_default_string,
    text: host_text,
    assign: host_assign,
    compare: host_compare,
    file_size: host_file_size,
    scratch: host_scratch,
    load: host_load,
    save: host_save,
    append_format: host_append_format,
    convert: host_convert,
    format: host_format,
    log: host_log,
    dump_map: host_dump_map,
    format_pair: host_format_pair,
    get_string: host_get_string,
    set_string: host_set_string,
    flush: host_flush,
    find_scoped: host_find_scoped,
};

// ---- FConfigCacheIni methods, through the same policy entry point ----

fn args(object: *mut c_void) -> Hp2ConfigArgs {
    Hp2ConfigArgs {
        object,
        section: null(),
        key: null(),
        value: null(),
        filename: null(),
        output: null_mut(),
        size: 0,
        flag: 0,
        constant: 0,
    }
}

fn run(operation: Hp2ConfigOperation, a: &Hp2ConfigArgs) -> Result<Hp2ConfigResult, i32> {
    unsafe { execute(&SERVICES, host_context(), operation, a) }
}

fn cache_new(system: &[Tchar], user: &[Tchar]) -> *mut FCache {
    Box::into_raw(Box::new(FCache {
        files: Vec::new(),
        system: FStr {
            text: terminate(system),
        },
        user: FStr {
            text: terminate(user),
        },
    }))
}

fn file_new() -> *mut FFile {
    Box::into_raw(Box::new(FFile::default()))
}

fn init(cache: *mut FCache, system: *const Tchar, user: *const Tchar) {
    let mut a = args(cache.cast());
    a.section = system.cast();
    a.value = user.cast();
    run(Hp2ConfigOperation::Hp2ConfigInit, &a).unwrap();
}

fn read_file(file: *mut FFile, filename: &[Tchar]) {
    let mut a = args(file.cast());
    a.filename = filename.as_ptr().cast();
    run(Hp2ConfigOperation::Hp2ConfigRead, &a).unwrap();
}

fn write_file(file: *mut FFile, filename: &[Tchar]) -> i32 {
    let mut a = args(file.cast());
    a.filename = filename.as_ptr().cast();
    run(Hp2ConfigOperation::Hp2ConfigWrite, &a).unwrap().integer
}

fn flush(cache: *mut FCache, filename: *const Tchar, read: i32) {
    let mut a = args(cache.cast());
    a.filename = filename.cast();
    a.flag = read;
    run(Hp2ConfigOperation::Hp2ConfigFlush, &a).unwrap();
}

fn exit(cache: *mut FCache) {
    let a = args(cache.cast());
    run(Hp2ConfigOperation::Hp2ConfigExit, &a).unwrap();
}

fn detach(cache: *mut FCache, filename: *const Tchar) {
    let mut a = args(cache.cast());
    a.filename = filename.cast();
    run(Hp2ConfigOperation::Hp2ConfigDetach, &a).unwrap();
}

fn empty_section(cache: *mut FCache, section: &[Tchar], filename: *const Tchar) {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.filename = filename.cast();
    run(Hp2ConfigOperation::Hp2ConfigEmptySection, &a).unwrap();
}

fn set_string(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: &[Tchar],
    filename: *const Tchar,
) {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.value = value.as_ptr().cast();
    a.filename = filename.cast();
    run(Hp2ConfigOperation::Hp2ConfigSetString, &a).unwrap();
}

fn get_string(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    output: &mut [Tchar],
    filename: *const Tchar,
) -> i32 {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.filename = filename.cast();
    a.output = output.as_mut_ptr().cast();
    a.size = output.len() as i32;
    run(Hp2ConfigOperation::Hp2ConfigGetString, &a)
        .unwrap()
        .integer
}

fn get_fstring(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    output: *mut FStr,
    filename: *const Tchar,
) -> i32 {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.filename = filename.cast();
    a.output = output.cast();
    run(Hp2ConfigOperation::Hp2ConfigGetFString, &a)
        .unwrap()
        .integer
}

fn get_section(
    cache: *mut FCache,
    section: &[Tchar],
    output: &mut [Tchar],
    filename: *const Tchar,
) -> i32 {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.filename = filename.cast();
    a.output = output.as_mut_ptr().cast();
    a.size = output.len() as i32;
    run(Hp2ConfigOperation::Hp2ConfigGetSection, &a)
        .unwrap()
        .integer
}

fn get_int(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: &mut i32,
    filename: *const Tchar,
) -> i32 {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.filename = filename.cast();
    a.output = (value as *mut i32).cast();
    run(Hp2ConfigOperation::Hp2ConfigGetInt, &a)
        .unwrap()
        .integer
}

fn get_float(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: &mut f32,
    filename: *const Tchar,
) -> i32 {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.filename = filename.cast();
    a.output = (value as *mut f32).cast();
    run(Hp2ConfigOperation::Hp2ConfigGetFloat, &a)
        .unwrap()
        .integer
}

fn get_bool(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: &mut i32,
    filename: *const Tchar,
) -> i32 {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.filename = filename.cast();
    a.output = (value as *mut i32).cast();
    run(Hp2ConfigOperation::Hp2ConfigGetBool, &a)
        .unwrap()
        .integer
}

fn set_int(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: i32,
    filename: *const Tchar,
) {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.value = (&value as *const i32).cast();
    a.filename = filename.cast();
    run(Hp2ConfigOperation::Hp2ConfigSetInt, &a).unwrap();
}

fn set_float(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: f32,
    filename: *const Tchar,
) {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.value = (&value as *const f32).cast();
    a.filename = filename.cast();
    run(Hp2ConfigOperation::Hp2ConfigSetFloat, &a).unwrap();
}

fn set_bool(
    cache: *mut FCache,
    section: &[Tchar],
    key: &[Tchar],
    value: i32,
    filename: *const Tchar,
) {
    let mut a = args(cache.cast());
    a.section = section.as_ptr().cast();
    a.key = key.as_ptr().cast();
    a.value = (&value as *const i32).cast();
    a.filename = filename.cast();
    run(Hp2ConfigOperation::Hp2ConfigSetBool, &a).unwrap();
}

// ---- FConfigFile accessors used by the C++ test's assertions ----

fn cache_file(cache: *mut FCache, name: &[Tchar]) -> *mut FFile {
    unsafe {
        let cache = &mut *cache;
        cache
            .files
            .iter_mut()
            .find(|(stored, _)| ansi::stricmp(stored.as_ptr(), name.as_ptr()) == 0)
            .map_or(null_mut(), |(_, file)| &mut **file as *mut FFile)
    }
}

fn cache_has(cache: *mut FCache, name: &[Tchar]) -> bool {
    !cache_file(cache, name).is_null()
}

fn cache_dirty(cache: *mut FCache, name: &[Tchar]) -> Option<i32> {
    let file = cache_file(cache, name);
    if file.is_null() {
        None
    } else {
        Some(unsafe { (*file).dirty })
    }
}

fn section_value(file: *mut FFile, section: &[Tchar], key: &[Tchar]) -> Option<Vec<Tchar>> {
    unsafe {
        let file = &*file;
        let (_, section) = file
            .sections
            .iter()
            .find(|(name, _)| ansi::stricmp(name.as_ptr(), section.as_ptr()) == 0)?;
        let (_, value) = section
            .values
            .iter()
            .rev()
            .find(|(name, _)| ansi::stricmp(name.as_ptr(), key.as_ptr()) == 0)?;
        Some((**value).text.clone())
    }
}

fn count_values(file: *mut FFile, section: &[Tchar], key: &[Tchar]) -> i32 {
    unsafe {
        let file = &*file;
        let Some((_, section)) = file
            .sections
            .iter()
            .find(|(name, _)| ansi::stricmp(name.as_ptr(), section.as_ptr()) == 0)
        else {
            return 0;
        };
        section
            .values
            .iter()
            .filter(|(name, _)| ansi::stricmp(name.as_ptr(), key.as_ptr()) == 0)
            .count() as i32
    }
}

// ---- 1. Parse semantics (FConfigFile::Read) ----

#[test]
fn parse_semantics_table() {
    let dir = test_dir("parse_semantics_table");
    let cases: &[(&str, &[u8], &str, &str, bool, &str)] = &[
        ("plain", b"[S]\r\nK=V\r\n", "S", "K", true, "V"),
        ("lf_only", b"[S]\nK=V\n", "S", "K", true, "V"),
        ("cr_only", b"[S]\rK=V\r", "S", "K", true, "V"),
        (
            "second_equals_in_value",
            b"[S]\nK=a=b\n",
            "S",
            "K",
            true,
            "a=b",
        ),
        ("empty_value", b"[S]\nK=\n", "S", "K", true, ""),
        ("leading_equals_value", b"[S]\nK==v\n", "S", "K", true, "=v"),
        ("quoted", b"[S]\nK=\"quoted\"\n", "S", "K", true, "quoted"),
        (
            "quoted_inner_quotes",
            b"[S]\nK=\"a\"b\"\n",
            "S",
            "K",
            true,
            "a\"b",
        ),
        (
            "unterminated_quote",
            b"[S]\nK=\"partial\n",
            "S",
            "K",
            true,
            "\"partial",
        ),
        (
            "untrimmed_value",
            b"[S]\nK= untrimmed \n",
            "S",
            "K",
            true,
            " untrimmed ",
        ),
        (
            "untrimmed_key",
            b"[S]\n  Key =  Val \n",
            "S",
            "  Key ",
            true,
            "  Val ",
        ),
        (
            "inline_comment_is_value",
            b"[S]\nc=x ; tail\n",
            "S",
            "c",
            true,
            "x ; tail",
        ),
        (
            "comment_with_equals_is_pair",
            b"[S]\n; note=x\n",
            "S",
            "; note",
            true,
            "x",
        ),
        (
            "quoted_key_verbatim",
            b"[S]\n\"Q\"=1\n",
            "S",
            "\"Q\"",
            true,
            "1",
        ),
        ("leading_blanks", b"\n\n[S]\nK=V\n", "S", "K", true, "V"),
        ("before_section_dropped", b"K=V\n[S]\n", "S", "K", false, ""),
        ("no_equals_dropped", b"[S]\nK\n", "S", "K", false, ""),
        (
            "unclosed_section_dropped",
            b"[Unclosed\nK=V\n",
            "Unclosed",
            "K",
            false,
            "",
        ),
        (
            "section_with_trailing_junk_dropped",
            b"[S]tray\nK=V\n",
            "S",
            "K",
            false,
            "",
        ),
        ("empty_section_name", b"[]\nK=V\n", "", "K", true, "V"),
        ("empty_file", b"", "S", "K", false, ""),
    ];
    for (name, text, section, key, found, expected) in cases.iter().copied() {
        fs::write(dir.join(name), text).unwrap();
        let file = file_new();
        read_file(file, &path_wide(&dir, name));
        let value = section_value(file, &wide(section), &wide(key));
        if found {
            let value =
                value.unwrap_or_else(|| panic!("{name}: expected [{section}]{key} to parse"));
            assert_text(&value, expected, &format!("{name}: [{section}]{key}"));
        } else {
            assert!(
                value.is_none(),
                "{name}: expected [{section}]{key} NOT to parse"
            );
        }
    }
}

#[test]
fn parse_semantics_duplicates_and_sections() {
    let dir = test_dir("parse_semantics_duplicates");

    // Missing file: Read leaves the config empty (no error, no throw).
    let file = file_new();
    read_file(file, &path_wide(&dir, "definitely_missing_file.ini"));
    assert!(
        section_value(file, &wide("S"), &wide("K")).is_none(),
        "missing file must yield an empty config"
    );

    // Duplicate keys: both pairs retained, the LAST occurrence wins lookups.
    fs::write(dir.join("dup_keys"), b"[S]\nA=1\nA=2\n").unwrap();
    let file = file_new();
    read_file(file, &path_wide(&dir, "dup_keys"));
    let value = section_value(file, &wide("S"), &wide("A"))
        .expect("duplicate key: expected last-wins value \"2\"");
    assert_text(&value, "2", "duplicate key: expected last-wins value \"2\"");
    assert_eq!(
        count_values(file, &wide("S"), &wide("A")),
        2,
        "duplicate key: both occurrences must be retained"
    );

    // Case-differing duplicates: both stored, lookups case-insensitive.
    fs::write(dir.join("dup_case_keys"), b"[S]\nA=1\na=2\n").unwrap();
    let file = file_new();
    read_file(file, &path_wide(&dir, "dup_case_keys"));
    let value = section_value(file, &wide("S"), &wide("A"))
        .expect("case-differing duplicates: expected \"2\" for lookup A");
    assert_text(
        &value,
        "2",
        "case-differing duplicates: expected \"2\" for lookup A",
    );
    let value = section_value(file, &wide("S"), &wide("a"))
        .expect("case-differing duplicates: lookup must be case-insensitive");
    assert_text(
        &value,
        "2",
        "case-differing duplicates: lookup must be case-insensitive",
    );
    assert_eq!(
        count_values(file, &wide("S"), &wide("A")),
        2,
        "case-differing duplicates: both entries retained"
    );

    // Case-variant section headers merge into the FIRST-seen spelling.
    fs::write(
        dir.join("section_case_merge"),
        b"[Render]\nk=1\n[RENDER]\nk2=2\n",
    )
    .unwrap();
    let file = file_new();
    read_file(file, &path_wide(&dir, "section_case_merge"));
    let value = section_value(file, &wide("render"), &wide("K"))
        .expect("section lookup must be case-insensitive");
    assert_text(&value, "1", "section lookup must be case-insensitive");
    let value = section_value(file, &wide("Render"), &wide("K2"))
        .expect("case-variant section headers must merge into one section");
    assert_text(
        &value,
        "2",
        "case-variant section headers must merge into one section",
    );
    let sections = unsafe { (*file).sections.len() };
    assert_eq!(
        sections, 1,
        "merged case-variant sections must produce a single section, got {sections}"
    );

    // Same key name in different sections stays independent.
    fs::write(dir.join("two_sections"), b"[S]\nk=v\n[T]\nk=w\n").unwrap();
    let file = file_new();
    read_file(file, &path_wide(&dir, "two_sections"));
    let value = section_value(file, &wide("S"), &wide("k")).expect("section S/k");
    assert_text(&value, "v", "section S/k");
    let value = section_value(file, &wide("T"), &wide("k")).expect("section T/k");
    assert_text(&value, "w", "section T/k");
}

// ---- 2. Typed getters: int/float/bool parsing quirks ----

const TYPED_INI: &[u8] = b"[Nums]\n\
IntPlain=42\n\
IntSigned=-7\n\
IntPlus=+7\n\
IntWsGarbage= 42abc\n\
IntHex=0x10\n\
IntWord=yes\n\
IntEmpty=\n\
IntFloaty=3.9\n\
[Reals]\n\
F1=1.5\n\
FExp=2e1\n\
FGarbage=abc\n\
FComma=1,5\n\
[Bools]\n\
BTrue=True\n\
BLower=true\n\
BUpper=TRUE\n\
BOne=1\n\
BZero=0\n\
BNoWord=no\n\
BTwo=2\n\
BSpaceed= true \n\
BOneTrailing=1x\n\
BFalse=False\n\
[Empty]\n";

#[test]
fn typed_getters_int_cases() {
    let dir = test_dir("typed_getters_int");
    fs::write(dir.join("typed.ini"), TYPED_INI).unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, "typed.ini");
    let section = wide("Nums");

    let cases: [(&str, i32); 8] = [
        ("IntPlain", 42),
        ("IntSigned", -7),
        ("IntPlus", 7),
        ("IntWsGarbage", 42),
        ("IntHex", 0),
        ("IntWord", 0),
        ("IntEmpty", 0),
        ("IntFloaty", 3),
    ];
    for (key, expected) in cases {
        let mut value = -12345;
        assert_eq!(
            get_int(cache, &section, &wide(key), &mut value, file.as_ptr()),
            1,
            "GetInt({key}) must return 1 for an existing key"
        );
        assert_eq!(
            value, expected,
            "GetInt({key}): expected {expected}, got {value}"
        );
    }

    // Missing key: returns 0 and leaves the caller's Value untouched.
    let mut value = 987;
    assert_eq!(
        get_int(cache, &section, &wide("Absent"), &mut value, file.as_ptr()),
        0,
        "GetInt missing key must return 0"
    );
    assert_eq!(value, 987, "GetInt missing key must not modify Value");
}

#[test]
fn typed_getters_float_and_bool_cases() {
    let dir = test_dir("typed_getters_float_bool");
    fs::write(dir.join("typed.ini"), TYPED_INI).unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, "typed.ini");
    let reals = wide("Reals");
    let nums = wide("Nums");
    let bools = wide("Bools");

    let cases: [(&str, f32); 4] = [
        ("F1", 1.5),
        ("FExp", 20.0),
        ("FGarbage", 0.0),
        ("FComma", 1.0),
    ];
    for (key, expected) in cases {
        let mut value = -1.0f32;
        assert_eq!(
            get_float(cache, &reals, &wide(key), &mut value, file.as_ptr()),
            1,
            "GetFloat({key}) must return 1 for an existing key"
        );
        assert_eq!(
            value, expected,
            "GetFloat({key}): expected {expected}, got {value}"
        );
    }

    // Integer text converts through atof exactly.
    let mut value = -1.0f32;
    get_float(cache, &nums, &wide("IntPlain"), &mut value, file.as_ptr());
    assert_eq!(
        value, 42.0f32,
        "GetFloat(IntPlain): expected 42, got {value}"
    );

    let cases: [(&str, i32); 10] = [
        ("BTrue", 1),
        ("BLower", 1),
        ("BUpper", 1),
        ("BOne", 1),
        ("BZero", 0),
        ("BNoWord", 0),
        ("BTwo", 0),
        ("BSpaceed", 0),
        ("BOneTrailing", 1),
        ("BFalse", 0),
    ];
    for (key, expected) in cases {
        let mut value = 77;
        assert_eq!(
            get_bool(cache, &bools, &wide(key), &mut value, file.as_ptr()),
            1,
            "GetBool({key}) must return 1 for an existing key"
        );
        assert_eq!(
            value, expected,
            "GetBool({key}): expected {expected}, got {value}"
        );
    }

    // Missing key: returns 0, Value untouched.
    let mut value = 5;
    assert_eq!(
        get_bool(cache, &bools, &wide("Absent"), &mut value, file.as_ptr()),
        0,
        "GetBool missing key must return 0"
    );
    assert_eq!(value, 5, "GetBool missing key must not modify Value");
}

#[test]
fn typed_getters_string_and_section_buffers() {
    let dir = test_dir("typed_getters_buffers");

    // GetString buffer contract: appStrncpy keeps Size-1 characters and NUL-terminates.
    fs::write(dir.join("trunc.ini"), b"[S]\nK=abcdefghij\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, "trunc.ini");
    let section = wide("S");
    let key = wide("K");
    let mut buffer = [b'Z' as Tchar; 5];
    buffer[4] = 0;
    assert_eq!(
        get_string(cache, &section, &key, &mut buffer, file.as_ptr()),
        1,
        "GetString must find K"
    );
    assert_text(
        &buffer,
        "abcd",
        "GetString(Size=5) must truncate to 4 chars",
    );
    let mut tiny = [b'Z' as Tchar; 1];
    assert_eq!(
        get_string(cache, &section, &key, &mut tiny, file.as_ptr()),
        1,
        "GetString(Size=1) must still return 1"
    );
    assert_eq!(tiny[0], 0, "GetString(Size=1) must yield an empty string");

    // GetSection: entries joined "k=v", NUL-separated, with a final extra NUL.
    fs::write(dir.join("two_sections.ini"), b"[S]\nk=v\n[T]\nk=w\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, "two_sections.ini");
    let mut section_out = [0 as Tchar; 64];
    assert_eq!(
        get_section(cache, &section, &mut section_out, file.as_ptr()),
        1,
        "GetSection must find S"
    );
    assert_text(&section_out, "k=v", "GetSection layout mismatch");
    assert_eq!(
        section_out[4], 0,
        "GetSection layout mismatch: layout is \"k=v\" '\\0' '\\0'"
    );

    fs::write(dir.join("sect2.ini"), b"[S]\nK1=V1\nK2=V2\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, "sect2.ini");
    let mut section_out = [0 as Tchar; 64];
    assert_eq!(
        get_section(cache, &section, &mut section_out, file.as_ptr()),
        1,
        "GetSection must find S"
    );
    // "K1=V1" '\0' "K2=V2" '\0' plus the final terminator = 13 TCHARs.
    let expected: [Tchar; 13] = [
        b'K' as Tchar,
        b'1' as Tchar,
        b'=' as Tchar,
        b'V' as Tchar,
        b'1' as Tchar,
        0,
        b'K' as Tchar,
        b'2' as Tchar,
        b'=' as Tchar,
        b'V' as Tchar,
        b'2' as Tchar,
        0,
        0,
    ];
    assert_eq!(
        &section_out[..13],
        &expected[..],
        "GetSection two-pair layout mismatch"
    );

    // Truncated GetSection: the loop stops before an entry that no longer fits.
    let mut truncated = [0 as Tchar; 8];
    assert_eq!(
        get_section(cache, &section, &mut truncated, file.as_ptr()),
        1,
        "GetSection truncated must return 1"
    );
    assert_text(
        &truncated,
        "K1=V1",
        "GetSection(Size=8) must contain only the first pair",
    );
    assert_eq!(
        truncated[6], 0,
        "GetSection(Size=8) must contain only the first pair"
    );
}

// ---- 3. SetString/SetInt/SetFloat/SetBool, Dirty gating, Detach/NoSave ----

#[test]
fn set_and_write_canonical_and_dirty_gating() {
    let dir = test_dir("set_and_write_dirty");

    // Fresh cache + nonexistent file: SetString creates canonical bytes on Write.
    let name = "setwrite_new.ini";
    let _ = fs::remove_file(dir.join(name));
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(
        cache,
        &wide("Video"),
        &wide("Mode"),
        &wide("32"),
        file.as_ptr(),
    );
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        &b"[Video]\r\nMode=32\r\n\r\n"[..],
        "fresh SetString+Flush: unexpected bytes"
    );

    // Same-value SetString leaves the file clean, so Write creates nothing.
    let name = "setwrite_same.ini";
    fs::write(dir.join(name), b"[S]\nK=v\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(cache, &wide("S"), &wide("K"), &wide("v"), file.as_ptr());
    assert_eq!(
        cache_dirty(cache, &file),
        Some(0),
        "same-value SetString must leave the file clean"
    );
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        &b"[S]\nK=v\n"[..],
        "clean file must not be rewritten"
    );

    // QUIRK: a case-ONLY value change is a silent no-op.
    let name = "setwrite_case.ini";
    fs::write(dir.join(name), b"[S]\nK=True\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(cache, &wide("S"), &wide("K"), &wide("TRUE"), file.as_ptr());
    assert_eq!(
        cache_dirty(cache, &file),
        Some(0),
        "case-only SetString must not mark the file dirty"
    );
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        &b"[S]\nK=True\n"[..],
        "case-only SetString must not rewrite the file"
    );
    let mut buffer = [0 as Tchar; 16];
    assert_eq!(
        get_string(cache, &wide("S"), &wide("K"), &mut buffer, file.as_ptr()),
        1,
        "GetString after case-only set"
    );
    assert_text(
        &buffer,
        "True",
        "case-only SetString must keep the old value",
    );

    // A case-insensitively different value updates and dirties.
    let name = "setwrite_update.ini";
    fs::write(dir.join(name), b"[S]\nK=True\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(cache, &wide("S"), &wide("K"), &wide("False"), file.as_ptr());
    assert_eq!(
        cache_dirty(cache, &file),
        Some(1),
        "real value change must dirty the file"
    );
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        &b"[S]\r\nK=False\r\n\r\n"[..],
        "updated value must be written"
    );
}

#[test]
fn set_and_write_typed_and_duplicate_keys() {
    let dir = test_dir("set_and_write_typed");

    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, "setwrite_typed.ini");
    set_int(cache, &wide("S"), &wide("I"), -42, file.as_ptr());
    set_float(cache, &wide("S"), &wide("F"), 0.5, file.as_ptr());
    set_bool(cache, &wide("S"), &wide("B"), 1, file.as_ptr());
    set_bool(cache, &wide("S"), &wide("B2"), 0, file.as_ptr());
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join("setwrite_typed.ini")).unwrap(),
        &b"[S]\r\nI=-42\r\nF=0.500000\r\nB=True\r\nB2=False\r\n\r\n"[..],
        "typed writers byte layout mismatch"
    );

    // SetString on a duplicated key updates only the newest entry.
    let name = "setwrite_dup.ini";
    fs::write(dir.join(name), b"[S]\nA=1\nA=2\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(cache, &wide("S"), &wide("A"), &wide("3"), file.as_ptr());
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        &b"[S]\r\nA=1\r\nA=3\r\n\r\n"[..],
        "SetString on duplicated key must update the newest entry only"
    );
}

#[test]
fn set_and_write_empty_section_detach_and_empty_write() {
    let dir = test_dir("set_and_write_lifecycle");

    // EmptySection: missing is a no-op; existing is cleared and stays as a header.
    let name = "emptysection.ini";
    fs::write(dir.join(name), b"[S]\nK=V\n[T]\nJ=W\n").unwrap();
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    empty_section(cache, &wide("Absent"), file.as_ptr());
    assert_eq!(
        cache_dirty(cache, &file),
        Some(0),
        "EmptySection on missing section must not dirty"
    );
    empty_section(cache, &wide("S"), file.as_ptr());
    assert_eq!(
        cache_dirty(cache, &file),
        Some(1),
        "EmptySection on existing section must dirty"
    );
    flush(cache, file.as_ptr(), 0);
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        &b"[S]\r\n\r\n[T]\r\nJ=W\r\n\r\n"[..],
        "emptied section must remain as a bare header"
    );

    // Detach sets NoSave: even dirty data is never written.
    let name = "detach_nosave.ini";
    let _ = fs::remove_file(dir.join(name));
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(cache, &wide("S"), &wide("K"), &wide("v"), file.as_ptr());
    detach(cache, file.as_ptr());
    flush(cache, file.as_ptr(), 0);
    assert!(
        !dir.join(name).exists(),
        "detached (NoSave) file must not be written by Flush"
    );
    exit(cache); // destructor Flush(1) — must also not write
    assert!(
        !dir.join(name).exists(),
        "detached (NoSave) file must not be written by the destructor"
    );

    // Write of a dirty but EMPTY config: empty-string saves are refused.
    let name = "empty_dirty.ini";
    let _ = fs::remove_file(dir.join(name));
    let file_obj = file_new();
    let path = path_wide(&dir, name);
    assert_eq!(
        write_file(file_obj, &path),
        1,
        "clean Write must return 1 without writing"
    );
    assert!(!dir.join(name).exists(), "clean Write must create nothing");
    unsafe {
        (*file_obj).dirty = 1;
    }
    assert_eq!(
        write_file(file_obj, &path),
        0,
        "dirty empty Write must return 0 (empty string save refused)"
    );
    assert!(
        !dir.join(name).exists(),
        "dirty empty Write must create no file"
    );
}

// ---- 4. Rewrite normalization ----

#[test]
fn rewrite_normalization_fixed_point() {
    let dir = test_dir("rewrite_normalization");
    let name = "rewrite.ini";
    fs::write(
            dir.join(name),
            b"; top comment\r\n[Render]\r\nMode=32\r\n; note=x\r\nQuality=\"High\"\r\nnoequals\r\nMode=16\n[Audio]\n\n[Render]\r\nExtra=1\r\n",
        )
        .unwrap();

    let file = file_new();
    let path = path_wide(&dir, name);
    read_file(file, &path);

    let value = section_value(file, &wide("Render"), &wide("Quality"))
        .expect("rewrite input: quoted value must be stored unquoted");
    assert_text(
        &value,
        "High",
        "rewrite input: quoted value must be stored unquoted",
    );
    let value = section_value(file, &wide("Render"), &wide("MODE"))
        .expect("rewrite input: duplicate key last-wins");
    assert_text(&value, "16", "rewrite input: duplicate key last-wins");
    let value = section_value(file, &wide("Render"), &wide("; note"))
        .expect("rewrite input: comment-with-= must be a pair");
    assert_text(&value, "x", "rewrite input: comment-with-= must be a pair");
    assert_eq!(
        count_values(file, &wide("Render"), &wide("Mode")),
        2,
        "rewrite input: duplicates retained"
    );

    unsafe {
        (*file).dirty = 1;
    }
    assert_eq!(write_file(file, &path), 1, "rewrite Write must succeed");
    let expected: &[u8] = b"[Render]\r\nMode=32\r\n; note=x\r\nQuality=High\r\nMode=16\r\nExtra=1\r\n\r\n[Audio]\r\n\r\n";
    assert_eq!(
        fs::read(dir.join(name)).unwrap(),
        expected,
        "rewrite bytes mismatch"
    );

    // Re-reading the rewritten file is stable (fixed point).
    let again = file_new();
    read_file(again, &path);
    unsafe {
        (*again).dirty = 1;
    }
    let second = path_wide(&dir, "rewrite2.ini");
    assert_eq!(write_file(again, &second), 1, "second rewrite must succeed");
    assert_eq!(
        fs::read(dir.join("rewrite2.ini")).unwrap(),
        expected,
        "second rewrite must be a fixed point"
    );
}

// ---- 5. FConfigCacheIni filename handling ----

#[test]
fn cache_filenames_aliases_and_extension() {
    let dir = test_dir("cache_filenames_aliases");
    fs::write(dir.join("CfgSys.ini"), b"[S]\nA=from_sys\n").unwrap();
    fs::write(dir.join("CfgUserX.ini"), b"[S]\nA=from_user\n").unwrap();
    fs::write(dir.join("PlainName.ini"), b"[S]\nA=from_plain\n").unwrap();

    let sys = path_wide(&dir, "CfgSys.ini");
    let usr = path_wide(&dir, "CfgUserX.ini");
    let cache = cache_new(&[], &[]);
    init(cache, sys.as_ptr(), usr.as_ptr());

    let section = wide("S");
    let key = wide("A");
    let mut buffer = [0 as Tchar; 32];

    // "user.ini" (any casing) is translated to UserIni.
    assert_eq!(
        get_string(
            cache,
            &section,
            &key,
            &mut buffer,
            wide("user.ini").as_ptr()
        ),
        1,
        "user.ini translation must find the file"
    );
    assert_text(&buffer, "from_user", "user.ini must read UserIni");
    assert_eq!(
        get_string(
            cache,
            &section,
            &key,
            &mut buffer,
            wide("USER.INI").as_ptr()
        ),
        1,
        "USER.INI translation must be case-insensitive"
    );
    assert_text(&buffer, "from_user", "USER.INI must read UserIni");

    // "system.ini" likewise translates to SystemIni.
    assert_eq!(
        get_string(
            cache,
            &section,
            &key,
            &mut buffer,
            wide("system.ini").as_ptr()
        ),
        1,
        "system.ini translation"
    );
    assert_text(&buffer, "from_sys", "system.ini must read SystemIni");

    // Filename=NULL defaults to SystemIni.
    assert_eq!(
        get_string(cache, &section, &key, &mut buffer, null()),
        1,
        "NULL filename must default to SystemIni"
    );
    assert_text(&buffer, "from_sys", "NULL filename must read SystemIni");

    // Extension appending on a real lookup.
    let plain = path_wide(&dir, "PlainName");
    assert_eq!(
        get_string(cache, &section, &key, &mut buffer, plain.as_ptr()),
        1,
        "extension must be appended to the extension-less path"
    );
    assert_text(
        &buffer,
        "from_plain",
        "extension-less path must resolve to PlainName.ini",
    );

    // A name that already ends in ".ini" is used verbatim (absolute path).
    assert_eq!(
        get_string(cache, &section, &key, &mut buffer, usr.as_ptr()),
        1,
        "explicit .ini name must be used verbatim"
    );
    assert_text(
        &buffer,
        "from_user",
        "explicit .ini name must not be re-extended",
    );

    // A missing file yields 0 and is not created (CreateIfNotFound=0).
    let missing = path_wide(&dir, "NopeMissing.ini");
    assert_eq!(
        get_string(cache, &section, &key, &mut buffer, missing.as_ptr()),
        0,
        "missing file must return 0"
    );
    assert!(
        !dir.join("NopeMissing.ini").exists(),
        "GetString must not create missing files"
    );

    // The cache is keyed by the RESOLVED filename.
    set_string(
        cache,
        &section,
        &key,
        &wide("changed"),
        wide("user.ini").as_ptr(),
    );
    assert!(
        !cache_file(cache, &usr).is_null(),
        "SetString via user.ini must populate the resolved UserIni entry"
    );
    let mut buffer = [0 as Tchar; 32];
    assert_eq!(
        get_string(cache, &section, &key, &mut buffer, usr.as_ptr()),
        1,
        "resolved path must see the same cache entry"
    );
    assert_text(
        &buffer,
        "changed",
        "user.ini and UserIni path must share one cache entry",
    );
}

#[test]
fn cache_filenames_resolution_rules() {
    let dir = test_dir("cache_filenames_rules");
    fs::write(dir.join("CfgSys.ini"), b"[S]\nA=from_sys\n").unwrap();
    fs::write(dir.join("CfgUserX.ini"), b"[S]\nA=from_user\n").unwrap();
    let sys = path_wide(&dir, "CfgSys.ini");
    let usr = path_wide(&dir, "CfgUserX.ini");
    let rules = cache_new(&[], &[]);
    init(rules, sys.as_ptr(), usr.as_ptr());

    let section = wide("R");
    let key = wide("K");
    let value = wide("v");

    set_string(rules, &section, &key, &value, wide("ab").as_ptr());
    assert!(
        cache_has(rules, &wide("ab.ini")),
        "names shorter than five characters get .ini appended"
    );
    set_string(rules, &section, &key, &value, wide("abcde").as_ptr());
    assert!(
        cache_has(rules, &wide("abcde.ini")),
        "five-character name without a dot gets .ini appended"
    );
    set_string(rules, &section, &key, &value, wide("a.b.c").as_ptr());
    assert!(
        cache_has(rules, &wide("a.b.c")) && !cache_has(rules, &wide("a.b.c.ini")),
        "a dot at position Len-4 counts as an existing extension"
    );
    set_string(rules, &section, &key, &value, wide("wxyz.ini").as_ptr());
    assert!(
        cache_has(rules, &wide("wxyz.ini")) && !cache_has(rules, &wide("wxyz.ini.ini")),
        "explicit .ini names are used verbatim"
    );

    // Every entry is detached so the cache destructor never writes these keys.
    for name in ["ab.ini", "abcde.ini", "a.b.c", "wxyz.ini"] {
        detach(rules, wide(name).as_ptr());
    }
}

#[test]
fn cache_filenames_long_paths() {
    let dir = test_dir("cache_filenames_long_paths");
    fs::write(dir.join("CfgSys.ini"), b"[S]\nA=from_sys\n").unwrap();
    fs::write(dir.join("CfgUserX.ini"), b"[S]\nA=from_user\n").unwrap();
    let sys = path_wide(&dir, "CfgSys.ini");
    let usr = path_wide(&dir, "CfgUserX.ini");

    let dir_text = dir.to_string_lossy().into_owned();
    let mut directory = String::from("long-path");
    while format!("{dir_text}/{directory}").len() + 1 < 160 {
        directory.push('/');
        directory.push_str(&"d".repeat(32));
    }
    fs::create_dir_all(dir.join(&directory)).unwrap();
    let prefix = format!("{dir_text}/{directory}/").len();
    let boundary = if prefix + 8 > 256 { prefix + 8 } else { 256 };

    for length in [boundary - 1, boundary, boundary + 1, boundary + 128] {
        let stem = format!("{directory}/{}", "x".repeat(length - prefix - 4));
        let filename = format!("{stem}.ini");
        let expected = format!("full-path-{length}");
        fs::write(dir.join(&filename), format!("[S]\nA={expected}\n")).unwrap();

        let full = wide(&format!("{dir_text}/{filename}"));
        let no_extension = wide(&format!("{dir_text}/{stem}"));
        let long = cache_new(&[], &[]);
        init(long, full.as_ptr(), usr.as_ptr());

        let section = wide("S");
        let key = wide("A");
        let system_ini = wide("system.ini");
        let lookups: [*const Tchar; 4] = [
            null(),
            system_ini.as_ptr(),
            full.as_ptr(),
            no_extension.as_ptr(),
        ];
        for lookup in lookups {
            let mut actual = [0 as Tchar; 64];
            assert_eq!(
                get_string(long, &section, &key, &mut actual, lookup),
                1,
                "long filename lookup must find the complete {length}-character path"
            );
            assert_eq!(
                as_text(&actual),
                expected,
                "long filename lookup must retain the file's value at {length} characters"
            );
        }

        init(long, sys.as_ptr(), full.as_ptr());
        let mut actual = [0 as Tchar; 64];
        let user_alias = wide("USER.INI");
        assert_eq!(
            get_string(long, &section, &key, &mut actual, user_alias.as_ptr()),
            1,
            "user.ini must resolve the long configured path at {length} characters"
        );
        assert_eq!(
            as_text(&actual),
            expected,
            "user.ini must retain the long configured file's value at {length} characters"
        );
    }
}

// ---- 6. Unicode round-trip ----

#[test]
fn unicode_roundtrip_bom_and_utf16_fixture() {
    let dir = test_dir("unicode_roundtrip");
    // "caf" + U+00E9 + U+4E2D + U+1F3AE (astral) + "!"
    let text: Vec<Tchar> = vec![
        b'c' as Tchar,
        b'a' as Tchar,
        b'f' as Tchar,
        0x00E9,
        0x4E2D,
        0x1F3AE,
        b'!' as Tchar,
        0,
    ];

    let name = "unicode_rt.ini";
    let cache = cache_new(&[], &[]);
    let file = path_wide(&dir, name);
    set_string(cache, &wide("Uni"), &wide("Text"), &text, file.as_ptr());
    flush(cache, file.as_ptr(), 0);

    let bytes = fs::read(dir.join(name)).unwrap();
    assert!(bytes.len() >= 2, "unicode file must exist with a BOM");
    // Any char > U+00FF forces the Unicode save path: FF FE BOM + UTF-16LE.
    assert!(
        bytes[0] == 0xFF && bytes[1] == 0xFE,
        "non-ASCII content must be saved as UTF-16LE with BOM, got {:02X} {:02X}",
        bytes[0],
        bytes[1]
    );
    // U+4E2D in UTF-16LE is the byte pair 2D 4E.
    assert!(
        bytes.windows(2).any(|pair| pair == [0x2D, 0x4E]),
        "U+4E2D must appear as UTF-16LE bytes 2D 4E"
    );
    // U+1F3AE encodes as D83C DFAE; little-endian that is 3C D8 AE DF.
    assert!(
        bytes
            .windows(4)
            .any(|pair| pair == [0x3C, 0xD8, 0xAE, 0xDF]),
        "U+1F3AE must appear as the surrogate pair D83C DFAE (bytes 3C D8 AE DF)"
    );

    {
        let cache = cache_new(&[], &[]);
        let file = path_wide(&dir, name);
        let mut loaded = FStr { text: vec![0] };
        assert_eq!(
            get_fstring(
                cache,
                &wide("Uni"),
                &wide("Text"),
                &mut loaded,
                file.as_ptr()
            ),
            1,
            "unicode reload must find the key"
        );
        assert_eq!(
            as_text(&loaded.text),
            as_text(&text),
            "unicode round-trip must reproduce the exact value"
        );
        assert_eq!(
            loaded.text.iter().position(|unit| *unit == 0),
            Some(7),
            "astral char must decode to ONE UTF-32 host unit"
        );
    }

    // ASCII-only content takes the ANSI path: no BOM, raw bytes.
    {
        let ascii_name = "ascii_only.ini";
        let cache = cache_new(&[], &[]);
        let file = path_wide(&dir, ascii_name);
        set_string(cache, &wide("S"), &wide("K"), &wide("plain"), file.as_ptr());
        flush(cache, file.as_ptr(), 0);
        assert_eq!(
            fs::read(dir.join(ascii_name)).unwrap(),
            &b"[S]\r\nK=plain\r\n\r\n"[..],
            "ASCII-only content must be saved as raw ANSI bytes without BOM"
        );
    }

    // A UTF-16LE BOM fixture is decoded on read.
    {
        let body = wide("[S]\r\nK=V\r\n");
        let mut bytes16 = vec![0xFF, 0xFE];
        for unit in &body[..body.len() - 1] {
            bytes16.extend((*unit as u16).to_le_bytes());
        }
        fs::write(dir.join("utf16_fixture.ini"), &bytes16).unwrap();
        let file = file_new();
        read_file(file, &path_wide(&dir, "utf16_fixture.ini"));
        let value = section_value(file, &wide("S"), &wide("K"))
            .expect("UTF-16LE BOM fixture must decode and parse");
        assert_text(&value, "V", "UTF-16LE BOM fixture must decode and parse");
    }
}
