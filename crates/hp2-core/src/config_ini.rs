//! Runtime FConfigCacheIni policy, deliberately distinct from the launcher INI parser.
//!
//! Native maps retain their typed objects and existing case-insensitive comparisons.
//! All text is borrowed from those objects or the call's native scratch FString;
//! parsing edits that scratch in place. Callback status is separate from UBOOL.
use crate::ansi::{self, Tchar};
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::{null, null_mut};

const CACHE: i32 = 0;
const FILE: i32 = 1;
const SECTION: i32 = 2;
const EMPTY: [Tchar; 1] = [0];
const EXT: [Tchar; 5] = [46, 105, 110, 105, 0];
const USER: [Tchar; 9] = [85, 115, 101, 114, 46, 105, 110, 105, 0];
const SYSTEM: [Tchar; 11] = [83, 121, 115, 116, 101, 109, 46, 105, 110, 105, 0];
const TRUE: [Tchar; 5] = [84, 114, 117, 101, 0];
const FALSE: [Tchar; 6] = [70, 97, 108, 115, 101, 0];
const HEADER: [Tchar; 7] = [91, 37, 115, 93, 13, 10, 0];
const PAIR: [Tchar; 8] = [37, 115, 61, 37, 115, 13, 10, 0];
const CRLF: [Tchar; 3] = [13, 10, 0];
const DUMP: [Tchar; 11] = [70, 105, 108, 101, 115, 32, 109, 97, 112, 58, 0];

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ConfigOperation {
    Hp2ConfigRead,
    Hp2ConfigWrite,
    Hp2ConfigFind,
    Hp2ConfigFlush,
    Hp2ConfigDetach,
    Hp2ConfigGetString,
    Hp2ConfigGetSection,
    Hp2ConfigGetSectionPrivate,
    Hp2ConfigSetString,
    Hp2ConfigEmptySection,
    Hp2ConfigInit,
    Hp2ConfigDump,
    Hp2ConfigGetInt,
    Hp2ConfigGetFloat,
    Hp2ConfigGetBool,
    Hp2ConfigSetInt,
    Hp2ConfigSetFloat,
    Hp2ConfigSetBool,
    Hp2ConfigGetFString,
    Hp2ConfigExit,
    Hp2ConfigGetStr,
}

/// Pointers retain their native types: object is a cache (or file for Read/Write),
/// value is TCHAR text (or an INT/FLOAT for typed setters), output is the getter's
/// destination (or FOutputDevice for Dump). Init uses section/value for System/User.
#[repr(C)]
pub struct Hp2ConfigArgs {
    pub object: *mut c_void,
    pub section: *const c_void,
    pub key: *const c_void,
    pub value: *const c_void,
    pub filename: *const c_void,
    pub output: *mut c_void,
    pub size: i32,
    pub flag: i32,
    pub constant: i32,
}

#[repr(C)]
#[derive(Default)]
pub struct Hp2ConfigResult {
    pub integer: i32,
    pub object: *mut c_void,
}

/// Each leaf returns zero or a captured native exception status. Only scoped
/// lifetime and virtual-dispatch leaves reenter Rust policy. Map kinds are
/// cache=0, file=1, section=2; section insertion always Add's (never Set's).
#[repr(C)]
pub struct Hp2ConfigServices {
    pub find: unsafe extern "C" fn(i32, *mut c_void, *const c_void, *mut *mut c_void) -> i32,
    pub insert: unsafe extern "C" fn(
        i32,
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut *mut c_void,
    ) -> i32,
    pub empty: unsafe extern "C" fn(i32, *mut c_void) -> i32,
    pub remove: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub count: unsafe extern "C" fn(i32, *mut c_void, *mut i32) -> i32,
    pub pair:
        unsafe extern "C" fn(i32, *mut c_void, i32, *mut *const c_void, *mut *mut c_void) -> i32,
    pub flags: unsafe extern "C" fn(*mut c_void, *mut *mut i32, *mut *mut i32) -> i32,
    pub default_string: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub text: unsafe extern "C" fn(*mut c_void, *mut *const c_void) -> i32,
    pub assign: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub compare: unsafe extern "C" fn(*const c_void, *const c_void, i32, *mut i32) -> i32,
    pub file_size: unsafe extern "C" fn(*const c_void, *mut i32) -> i32,
    pub scratch: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub load: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
    pub save: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
    pub append_format:
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, *const c_void) -> i32,
    /// kind=0 integer, kind=1 float. Native CRT/locale conversion stays native.
    pub convert: unsafe extern "C" fn(i32, *const c_void, *mut c_void) -> i32,
    pub format: unsafe extern "C" fn(i32, *const c_void, *mut c_void) -> i32,
    pub log: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub dump_map: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub format_pair:
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, *mut i32) -> i32,
    /// Virtual native calls preserve derived cache overrides and capture throws.
    pub get_string: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut c_void,
        i32,
        *const c_void,
        *mut i32,
    ) -> i32,
    pub set_string: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *const c_void,
        *const c_void,
        *const c_void,
    ) -> i32,
    pub flush: unsafe extern "C" fn(*mut c_void, i32, *const c_void) -> i32,
    /// Establishes native FPushMemTag and scratch lifetimes, calls find_tagged,
    /// then destroys both before returning, including on callback failure.
    pub find_scoped: unsafe extern "C" fn(
        *const Hp2ConfigServices,
        *mut c_void,
        *const c_void,
        i32,
        *mut *mut c_void,
    ) -> i32,
}

fn status(code: i32) -> Result<(), i32> {
    if code == 0 { Ok(()) } else { Err(code) }
}

unsafe fn length(mut text: *const Tchar) -> usize {
    let mut n = 0;
    while unsafe { *text } != 0 {
        n += 1;
        text = unsafe { text.add(1) };
    }
    n
}

/// Own a stable, terminated lookup name across native callbacks. Ordinary names
/// stay on the stack; only names that exceed it allocate spill storage.
unsafe fn copy_filename(
    stack: &mut [MaybeUninit<Tchar>; 256],
    spill: &mut Vec<MaybeUninit<Tchar>>,
    source: *const Tchar,
    length: usize,
    suffix: &[Tchar],
) -> *mut Tchar {
    let required = length + suffix.len();
    let output = if required <= stack.len() {
        stack.as_mut_ptr().cast::<Tchar>()
    } else {
        spill.reserve(required);
        spill.as_mut_ptr().cast::<Tchar>()
    };
    unsafe {
        std::ptr::copy_nonoverlapping(source, output, length);
        std::ptr::copy_nonoverlapping(suffix.as_ptr(), output.add(length), suffix.len());
    }
    output
}

struct Config<'a> {
    s: &'a Hp2ConfigServices,
    context: *mut c_void,
}

/// Parses the native INI line grammar by splitting the mutable TCHAR scratch in place.
///
/// Empty lines are skipped, section/key spelling and ordering are retained, and
/// only a section's first `=` separates its key and value.
///
/// # Safety
/// `cursor` must point to a writable NUL-terminated TCHAR buffer.
pub(crate) unsafe fn parse_in_place<E>(
    mut cursor: *mut Tchar,
    mut section: impl FnMut(*const Tchar, usize) -> Result<(), E>,
    mut pair: impl FnMut(*const Tchar, usize, *const Tchar, usize) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        loop {
            while *cursor == 13 || *cursor == 10 {
                cursor = cursor.add(1);
            }
            if *cursor == 0 {
                return Ok(());
            }
            let start = cursor;
            while *cursor != 0 && *cursor != 13 && *cursor != 10 {
                cursor = cursor.add(1);
            }
            let done = *cursor == 0;
            let len = cursor.offset_from(start) as usize;
            *cursor = 0;
            if !done {
                cursor = cursor.add(1);
            }
            if len >= 2 && *start == 91 && *start.add(len - 1) == 93 {
                *start.add(len - 1) = 0;
                section(start.add(1), len - 2)?;
            } else {
                let mut split = start;
                while *split != 0 && *split != 61 {
                    split = split.add(1);
                }
                if *split == 61 {
                    *split = 0;
                    let mut value = split.add(1);
                    let mut value_len = len - (split.offset_from(start) as usize) - 1;
                    if value_len >= 2 && *value == 34 && *value.add(value_len - 1) == 34 {
                        *value.add(value_len - 1) = 0;
                        value = value.add(1);
                        value_len -= 2;
                    }
                    pair(start, split.offset_from(start) as usize, value, value_len)?;
                }
            }
            if done {
                return Ok(());
            }
        }
    }
}

impl Config<'_> {
    unsafe fn find(
        &self,
        kind: i32,
        map: *mut c_void,
        key: *const c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        status(unsafe { (self.s.find)(kind, map, key, &mut out) })?;
        Ok(out)
    }
    unsafe fn insert(
        &self,
        kind: i32,
        map: *mut c_void,
        key: *const c_void,
        value: *const c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        status(unsafe { (self.s.insert)(kind, map, key, value, &mut out) })?;
        Ok(out)
    }
    unsafe fn section(
        &self,
        file: *mut c_void,
        name: *const c_void,
        force: bool,
    ) -> Result<*mut c_void, i32> {
        let mut section = unsafe { self.find(FILE, file, name)? };
        if section.is_null() && force {
            section = unsafe { self.insert(FILE, file, name, null())? };
        }
        Ok(section)
    }
    unsafe fn count(&self, kind: i32, map: *mut c_void) -> Result<i32, i32> {
        let mut out = 0;
        status(unsafe { (self.s.count)(kind, map, &mut out) })?;
        Ok(out)
    }
    unsafe fn pair(
        &self,
        kind: i32,
        map: *mut c_void,
        index: i32,
    ) -> Result<(*const c_void, *mut c_void), i32> {
        let mut key = null();
        let mut value = null_mut();
        status(unsafe { (self.s.pair)(kind, map, index, &mut key, &mut value) })?;
        Ok((key, value))
    }
    unsafe fn flags(&self, file: *mut c_void) -> Result<(*mut i32, *mut i32), i32> {
        let mut dirty = null_mut();
        let mut no_save = null_mut();
        status(unsafe { (self.s.flags)(file, &mut dirty, &mut no_save) })?;
        Ok((dirty, no_save))
    }
    unsafe fn default_string(&self, cache: *mut c_void, user: i32) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        status(unsafe { (self.s.default_string)(cache, user, &mut out) })?;
        Ok(out)
    }
    unsafe fn text(&self, string: *mut c_void) -> Result<*const Tchar, i32> {
        let mut out = null();
        status(unsafe { (self.s.text)(string, &mut out) })?;
        Ok(out.cast())
    }
    unsafe fn equal(
        &self,
        a: *const c_void,
        b: *const c_void,
        sensitive: i32,
    ) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { (self.s.compare)(a, b, sensitive, &mut out) })?;
        Ok(out == 0)
    }
    unsafe fn scratch(&self) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        status(unsafe { (self.s.scratch)(self.context, &mut out) })?;
        Ok(out)
    }
    unsafe fn read(&self, file: *mut c_void, filename: *const c_void) -> Result<(), i32> {
        unsafe {
            status((self.s.empty)(FILE, file))?;
            let mut loaded = 0;
            status((self.s.load)(self.context, filename, &mut loaded))?;
            if loaded == 0 {
                return Ok(());
            }
            let cursor = self.text(self.scratch()?)?.cast_mut();
            let section = std::cell::Cell::new(null_mut());
            parse_in_place(
                cursor,
                |name, _| -> Result<(), i32> {
                    section.set(unsafe { self.section(file, name.cast(), true)? });
                    Ok(())
                },
                |key, _, value, _| -> Result<(), i32> {
                    let current = section.get();
                    if !current.is_null() {
                        unsafe { self.insert(SECTION, current, key.cast(), value.cast())?; }
                    }
                    Ok(())
                },
            )?;
            Ok(())
        }
    }
    unsafe fn write(&self, file: *mut c_void, filename: *const c_void) -> Result<i32, i32> {
        unsafe {
            let (dirty, no_save) = self.flags(file)?;
            if *dirty == 0 || *no_save != 0 {
                return Ok(1);
            }
            // Deliberately before formatting and saving, including failed writes.
            *dirty = 0;
            status((self.s.assign)(self.scratch()?, EMPTY.as_ptr().cast()))?;
            for i in 0..self.count(FILE, file)? {
                let (name, section) = self.pair(FILE, file, i)?;
                status((self.s.append_format)(
                    self.context,
                    HEADER.as_ptr().cast(),
                    name,
                    null(),
                ))?;
                for j in 0..self.count(SECTION, section)? {
                    let (key, value) = self.pair(SECTION, section, j)?;
                    status((self.s.append_format)(
                        self.context,
                        PAIR.as_ptr().cast(),
                        key,
                        self.text(value)?.cast(),
                    ))?;
                }
                status((self.s.append_format)(
                    self.context,
                    CRLF.as_ptr().cast(),
                    null(),
                    null(),
                ))?;
            }
            let mut saved = 0;
            status((self.s.save)(self.context, filename, &mut saved))?;
            Ok(saved)
        }
    }
    unsafe fn cache_find(
        &self,
        cache: *mut c_void,
        filename: *const c_void,
        create: bool,
    ) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        status(unsafe {
            (self.s.find_scoped)(self.s, cache, filename, i32::from(create), &mut out)
        })?;
        Ok(out)
    }
    unsafe fn cache_find_tagged(
        &self,
        cache: *mut c_void,
        filename: *const c_void,
        create: bool,
    ) -> Result<*mut c_void, i32> {
        unsafe {
            let mut normalized = [MaybeUninit::<Tchar>::uninit(); 256];
            let mut spill = Vec::new();
            let source = if filename.is_null() {
                self.text(self.default_string(cache, 0)?)?
            } else {
                filename.cast()
            };
            let n = length(source);
            // This is intentionally NOT a conventional extension check.
            let suffix = if n < 5 || (*source.add(n - 4) != 46 && *source.add(n - 5) != 46) {
                EXT.as_slice()
            } else {
                EMPTY.as_slice()
            };
            let mut name = copy_filename(&mut normalized, &mut spill, source, n, suffix);
            let replacement = if self.equal(name.cast(), USER.as_ptr().cast(), 0)? {
                self.text(self.default_string(cache, 1)?)?
            } else if self.equal(name.cast(), SYSTEM.as_ptr().cast(), 0)? {
                self.text(self.default_string(cache, 0)?)?
            } else {
                null()
            };
            if !replacement.is_null() {
                name = copy_filename(
                    &mut normalized,
                    &mut spill,
                    replacement,
                    length(replacement),
                    &EMPTY,
                );
            }
            let mut file = self.find(CACHE, cache, name.cast())?;
            if file.is_null() {
                let mut exists = -1;
                if !create {
                    status((self.s.file_size)(name.cast(), &mut exists))?;
                }
                if create || exists >= 0 {
                    file = self.insert(CACHE, cache, name.cast(), null())?;
                    self.read(file, name.cast())?;
                }
            }
            Ok(file)
        }
    }
    unsafe fn flush(
        &self,
        cache: *mut c_void,
        filename: *const c_void,
        read: bool,
    ) -> Result<(), i32> {
        unsafe {
            for i in 0..self.count(CACHE, cache)? {
                let (key, file) = self.pair(CACHE, cache, i)?;
                if filename.is_null() || self.equal(key, filename, 0)? {
                    self.write(file, key)?;
                }
            }
            if read {
                if filename.is_null() {
                    status((self.s.empty)(CACHE, cache))?;
                } else {
                    status((self.s.remove)(cache, filename))?;
                }
            }
            Ok(())
        }
    }
    unsafe fn get_string(&self, a: &Hp2ConfigArgs, out: *mut Tchar, size: i32) -> Result<i32, i32> {
        unsafe {
            *out = 0;
            let file = self.cache_find(a.object, a.filename, false)?;
            if file.is_null() {
                return Ok(0);
            }
            let section = self.section(file, a.section, false)?;
            if section.is_null() {
                return Ok(0);
            }
            let value = self.find(SECTION, section, a.key)?;
            if value.is_null() {
                return Ok(0);
            }
            ansi::strncpy(out, self.text(value)?, size);
            Ok(1)
        }
    }
    unsafe fn get_section(&self, a: &Hp2ConfigArgs) -> Result<i32, i32> {
        unsafe {
            let out = a.output.cast::<Tchar>();
            *out = 0;
            let file = self.cache_find(a.object, a.filename, false)?;
            if file.is_null() {
                return Ok(0);
            }
            let section = self.section(file, a.section, false)?;
            if section.is_null() {
                return Ok(0);
            }
            let mut end = out;
            for i in 0..self.count(SECTION, section)? {
                let (key, value) = self.pair(SECTION, section, i)?;
                let key = key.cast::<Tchar>();
                // Preserve the original key-only size predicate (not a value-size check).
                if end.offset_from(out) + length(key) as isize + 1 >= a.size as isize {
                    break;
                }
                let mut written = 0;
                status((self.s.format_pair)(
                    end.cast(),
                    key.cast(),
                    self.text(value)?.cast(),
                    &mut written,
                ))?;
                end = end.offset(written as isize + 1);
            }
            *end = 0;
            Ok(1)
        }
    }
    unsafe fn set_string(&self, a: &Hp2ConfigArgs, value: *const c_void) -> Result<(), i32> {
        unsafe {
            let file = self.cache_find(a.object, a.filename, true)?;
            let section = self.section(file, a.section, true)?;
            let old = self.find(SECTION, section, a.key)?;
            if old.is_null() {
                self.insert(SECTION, section, a.key, value)?;
                *self.flags(file)?.0 = 1;
            } else if !self.equal(self.text(old)?.cast(), value, 0)? {
                *self.flags(file)?.0 = i32::from(!self.equal(self.text(old)?.cast(), value, 1)?);
                status((self.s.assign)(old, value))?;
            }
            Ok(())
        }
    }
}

/// Find policy inside the native allocation-tag scope. Separate from cache_find
/// so every internal lookup gets exactly one tag push/pop without heap guards.
///
/// # Safety
/// The services, scratch context, cache and filename satisfy execute's contract;
/// the caller retains its native FPushMemTag until this function returns.
pub unsafe fn find_tagged(
    s: &Hp2ConfigServices,
    context: *mut c_void,
    cache: *mut c_void,
    filename: *const c_void,
    create: i32,
) -> Result<*mut c_void, i32> {
    unsafe { Config { s, context }.cache_find_tagged(cache, filename, create != 0) }
}

/// # Safety
/// Services and args must describe live native objects and correctly sized native
/// output buffers. Same preconditions as the corresponding FConfigCacheIni method;
/// in particular, GetSection retains its historical key-only capacity predicate.
pub unsafe fn execute(
    s: &Hp2ConfigServices,
    context: *mut c_void,
    operation: Hp2ConfigOperation,
    a: &Hp2ConfigArgs,
) -> Result<Hp2ConfigResult, i32> {
    use Hp2ConfigOperation::*;
    let c = Config { s, context };
    let mut result = Hp2ConfigResult::default();
    unsafe {
        match operation {
            Hp2ConfigRead => c.read(a.object, a.filename)?,
            Hp2ConfigWrite => result.integer = c.write(a.object, a.filename)?,
            Hp2ConfigFind => result.object = c.cache_find(a.object, a.filename, a.flag != 0)?,
            Hp2ConfigFlush => c.flush(a.object, a.filename, a.flag != 0)?,
            Hp2ConfigDetach => {
                let file = c.cache_find(a.object, a.filename, true)?;
                if !file.is_null() {
                    *c.flags(file)?.1 = 1;
                }
            }
            Hp2ConfigGetString => result.integer = c.get_string(a, a.output.cast(), a.size)?,
            Hp2ConfigGetSection => result.integer = c.get_section(a)?,
            Hp2ConfigGetSectionPrivate => {
                let file = c.cache_find(a.object, a.filename, a.flag != 0)?;
                if !file.is_null() {
                    result.object = c.section(file, a.section, a.flag != 0)?;
                    if !result.object.is_null() && (a.flag != 0 || a.constant == 0) {
                        *c.flags(file)?.0 = 1;
                    }
                }
            }
            Hp2ConfigSetString => c.set_string(a, a.value)?,
            Hp2ConfigEmptySection => {
                let file = c.cache_find(a.object, a.filename, false)?;
                if !file.is_null() {
                    let section = c.section(file, a.section, false)?;
                    if !section.is_null() && c.count(SECTION, section)? != 0 {
                        status((s.empty)(SECTION, section))?;
                        *c.flags(file)?.0 = 1;
                    }
                }
            }
            Hp2ConfigInit => {
                status((s.assign)(c.default_string(a.object, 0)?, a.section))?;
                status((s.assign)(c.default_string(a.object, 1)?, a.value))?;
            }
            Hp2ConfigDump => {
                status((s.log)(a.output, DUMP.as_ptr().cast()))?;
                status((s.dump_map)(a.object, a.output))?;
            }
            Hp2ConfigGetStr => {
                status((s.get_string)(
                    a.object,
                    a.section,
                    a.key,
                    a.output,
                    1024,
                    a.filename,
                    &mut result.integer,
                ))?;
            }
            Hp2ConfigGetFString => {
                let mut text = [0 as Tchar; 4096];
                status((s.get_string)(
                    a.object,
                    a.section,
                    a.key,
                    text.as_mut_ptr().cast(),
                    4096,
                    a.filename,
                    &mut result.integer,
                ))?;
                status((s.assign)(a.output, text.as_ptr().cast()))?;
            }
            Hp2ConfigGetInt | Hp2ConfigGetFloat | Hp2ConfigGetBool => {
                let mut text = [0 as Tchar; 80];
                status((s.get_string)(
                    a.object,
                    a.section,
                    a.key,
                    text.as_mut_ptr().cast(),
                    80,
                    a.filename,
                    &mut result.integer,
                ))?;
                if result.integer != 0 {
                    match operation {
                        Hp2ConfigGetBool => {
                            let mut value = 1i32;
                            if !c.equal(text.as_ptr().cast(), TRUE.as_ptr().cast(), 0)? {
                                status((s.convert)(
                                    0,
                                    text.as_ptr().cast(),
                                    (&mut value as *mut i32).cast(),
                                ))?;
                            }
                            *a.output.cast::<i32>() = i32::from(value == 1);
                        }
                        Hp2ConfigGetFloat => {
                            status((s.convert)(1, text.as_ptr().cast(), a.output))?
                        }
                        _ => status((s.convert)(0, text.as_ptr().cast(), a.output))?,
                    }
                }
            }
            Hp2ConfigSetInt | Hp2ConfigSetFloat => {
                let mut text = [0 as Tchar; 30];
                let kind = i32::from(matches!(operation, Hp2ConfigSetFloat));
                status((s.format)(kind, a.value, text.as_mut_ptr().cast()))?;
                status((s.set_string)(
                    a.object,
                    a.section,
                    a.key,
                    text.as_ptr().cast(),
                    a.filename,
                ))?;
            }
            Hp2ConfigSetBool => {
                let value = if *a.value.cast::<i32>() != 0 {
                    TRUE.as_ptr()
                } else {
                    FALSE.as_ptr()
                };
                status((s.set_string)(
                    a.object,
                    a.section,
                    a.key,
                    value.cast(),
                    a.filename,
                ))?;
            }
            Hp2ConfigExit => status((s.flush)(a.object, 1, null()))?,
        }
    }
    Ok(result)
}

// Ports `Tests/ConfigIniTests.cpp` (FConfigCacheIni/FConfigFile contracts) onto the
// Rust policy above. The tests own a minimal native host that mirrors the leaves in
// `HarryPotter2/Unreal/Core/Src/UnConfigIni.cpp` (TMap/TMultiMap file+section
// storage, FString, flags, SystemIni/UserIni aliases, ANSI/UTF-16LE text files,
// wcstol/atof conversion and CRT formatting) and drive every operation through
// `execute`, exactly as `FConfigCacheIni` does.
#[cfg(test)]
mod tests;
