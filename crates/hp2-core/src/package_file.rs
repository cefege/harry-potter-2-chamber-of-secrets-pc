//! `appFindPackageFile` search policy; native state and filesystem leaves remain in C++.
//!
//! Exact probes stop at the first hit. Enumeration intentionally does not: the
//! last matching filename's size result wins, as in the original native loop.
//!
//! UE1 composed every candidate with unchecked `appStrcpy`/`appStrcat` into
//! TCHAR[256] stack buffers, which overflowed once a search path (for example the
//! absolute save directory under a long HOME) plus a package name passed 256.
//! Candidates are now composed in owned strings, so probes always see the full
//! path; only the caller's `Out` is fixed-size, and a hit that does not fit it is
//! reported through `too_long` and treated as not found instead of truncated.

use crate::ansi::Tchar;
use std::ffi::c_void;

pub const LANGUAGE: i32 = 0;
pub const CD_PATH: i32 = 1;
pub const SEARCH_PATH: i32 = 2;
pub const CACHE_PATH: i32 = 3;
pub const CACHE_EXT: i32 = 4;
pub const GUID: i32 = 5;
pub const DLL_EXT: i32 = 6;
pub const SEPARATOR: i32 = 7;

pub type PackageFileVisitor = unsafe extern "C" fn(*mut c_void, *const c_void) -> i32;
pub type PackageFileComplete = unsafe extern "C" fn(*mut c_void) -> i32;

/// All callbacks return zero on success, otherwise the captured native exception
/// status. `enumerate` visits native filenames in order, then calls `complete`
/// while the array is still alive. It stops at the first nonzero callback result,
/// returns that status unchanged, and destroys its array before returning.
#[repr(C)]
pub struct Hp2PackageFileServices {
    pub string: unsafe extern "C" fn(*mut c_void, i32, i32, *mut *const c_void) -> i32,
    pub path_count: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub compare: unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, *mut i32) -> i32,
    pub file_size: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
    pub enumerate: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        PackageFileVisitor,
        PackageFileComplete,
        *mut c_void,
    ) -> i32,
    pub touch: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub diagnostic: unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void) -> i32,
    /// Log that the found file `path` needs more than `capacity` TCHARs of output.
    pub too_long: unsafe extern "C" fn(*mut c_void, *const c_void, i32) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// A NUL-terminated owned TCHAR string.
struct Text(Vec<Tchar>);

impl Text {
    fn new() -> Self {
        Self(vec![0])
    }

    fn ptr(&self) -> *const Tchar {
        self.0.as_ptr()
    }

    /// Code units before the terminator.
    fn units(&self) -> &[Tchar] {
        &self.0[..self.0.len() - 1]
    }

    fn clear(&mut self) {
        self.0.clear();
        self.0.push(0);
    }

    fn push_units(&mut self, units: &[Tchar]) {
        self.0.pop();
        self.0.extend_from_slice(units);
        self.0.push(0);
    }

    /// `appStrcat` of a NUL-terminated native string.
    ///
    /// # Safety
    /// `source` must be NUL-terminated.
    unsafe fn push(&mut self, mut source: *const Tchar) {
        self.0.pop();
        unsafe {
            while *source != 0 {
                self.0.push(*source);
                source = source.add(1);
            }
        }
        self.0.push(0);
    }

    fn push_char(&mut self, value: u8) {
        self.push_units(&[value as Tchar]);
    }

    fn set(&mut self, other: &Text) {
        self.0.clone_from(&other.0);
    }
}

unsafe fn length(mut text: *const Tchar) -> usize {
    let mut count = 0;
    while unsafe { *text } != 0 {
        count += 1;
        text = unsafe { text.add(1) };
    }
    count
}

struct Resolver<'a> {
    services: &'a Hp2PackageFileServices,
    context: *mut c_void,
}

impl Resolver<'_> {
    unsafe fn string(&self, kind: i32, index: i32) -> Result<*const Tchar, i32> {
        let mut result = std::ptr::null();
        status(unsafe { (self.services.string)(self.context, kind, index, &mut result) })?;
        Ok(result.cast())
    }

    unsafe fn count(&self) -> Result<i32, i32> {
        let mut result = 0;
        status(unsafe { (self.services.path_count)(self.context, &mut result) })?;
        Ok(result)
    }

    unsafe fn equal(&self, left: *const Tchar, right: *const Tchar) -> Result<bool, i32> {
        let mut result = 0;
        status(unsafe {
            (self.services.compare)(self.context, left.cast(), right.cast(), &mut result)
        })?;
        Ok(result == 0)
    }

    unsafe fn exists(&self, name: &Text) -> Result<bool, i32> {
        let mut result = -1;
        status(unsafe { (self.services.file_size)(self.context, name.ptr().cast(), &mut result) })?;
        Ok(result >= 0)
    }

    unsafe fn cd_limit(&self) -> Result<i32, i32> {
        Ok(1 + i32::from(unsafe { *self.string(CD_PATH, 0)? } != 0))
    }

    /// Builds the directory prefix into `out` and returns the extension: the text
    /// after the search path's `*` (if any), or the cache extension.
    unsafe fn prefix(&self, out: &mut Text, cd: i32, index: i32) -> Result<Option<Text>, i32> {
        unsafe {
            out.clear();
            if cd != 0 {
                out.push(self.string(CD_PATH, 0)?);
                const SYSTEM: [Tchar; 7] = [83, 121, 115, 116, 101, 109, 0];
                out.push(SYSTEM.as_ptr());
                out.push(self.string(SEPARATOR, 0)?);
            }
            if index < self.count()? {
                out.push(self.string(SEARCH_PATH, index)?);
                match out.units().iter().position(|&ch| ch == b'*' as Tchar) {
                    Some(star) => {
                        let mut extension = Text::new();
                        extension.push_units(&out.units()[star + 1..]);
                        out.0.truncate(star);
                        out.0.push(0);
                        Ok(Some(extension))
                    }
                    None => Ok(None),
                }
            } else {
                out.push(self.string(CACHE_PATH, 0)?);
                out.push(self.string(SEPARATOR, 0)?);
                let mut extension = Text::new();
                extension.push(self.string(CACHE_EXT, 0)?);
                Ok(Some(extension))
            }
        }
    }

    /// Reports a found path that cannot be returned in `capacity` TCHARs.
    unsafe fn fits(&self, path: &Text, capacity: usize) -> Result<bool, i32> {
        if path.0.len() <= capacity {
            return Ok(true);
        }
        let capacity = i32::try_from(capacity).unwrap_or(i32::MAX);
        status(unsafe { (self.services.too_long)(self.context, path.ptr().cast(), capacity) })?;
        Ok(false)
    }

    unsafe fn finish(&self, index: i32, out: &Text) -> Result<bool, i32> {
        if index == unsafe { self.count()? } {
            status(unsafe { (self.services.touch)(self.context, out.ptr().cast()) })?;
        }
        Ok(true)
    }
}

struct Enumeration<'a> {
    resolver: &'a Resolver<'a>,
    normalized: &'a Text,
    input_extension: &'a Text,
    base: &'a Text,
    base_extension: &'a Text,
    prefix: &'a Text,
    output: &'a mut Text,
    input: *const Tchar,
    index: i32,
    capacity: usize,
    found: bool,
    too_long: bool,
}

unsafe extern "C" fn visit(context: *mut c_void, filename: *const c_void) -> i32 {
    let state = unsafe { &mut *context.cast::<Enumeration<'_>>() };
    let mut run = || -> Result<(), i32> {
        unsafe {
            let filename = filename.cast::<Tchar>();
            if state.resolver.equal(filename, state.normalized.ptr())?
                || state
                    .resolver
                    .equal(filename, state.input_extension.ptr())?
                || state.resolver.equal(filename, state.base.ptr())?
                || state.resolver.equal(filename, state.base_extension.ptr())?
            {
                state.output.set(state.prefix);
                state.output.push(filename);
                state.found = state.resolver.exists(state.output)?;
            }
        }
        Ok(())
    };
    match run() {
        Ok(()) => 0,
        Err(error) => error,
    }
}

unsafe extern "C" fn complete(context: *mut c_void) -> i32 {
    let state = unsafe { &mut *context.cast::<Enumeration<'_>>() };
    let mut run = || -> Result<(), i32> {
        if state.found {
            let resolver = state.resolver;
            unsafe {
                if !resolver.fits(state.output, state.capacity)? {
                    state.too_long = true;
                    return Ok(());
                }
                status((resolver.services.diagnostic)(
                    resolver.context,
                    state.input.cast(),
                    state.output.ptr().cast(),
                ))?;
                resolver.finish(state.index, state.output)?;
            }
        }
        Ok(())
    };
    match run() {
        Ok(()) => 0,
        Err(error) => error,
    }
}

/// Writes the last composed candidate to the caller's buffer, as the native code
/// left it there (found or not); an empty string when it does not fit.
///
/// # Safety
/// `output` must have `capacity >= 1` writable TCHARs.
unsafe fn publish(output: *mut Tchar, capacity: usize, text: &Text) {
    let source: &[Tchar] = if text.0.len() <= capacity {
        &text.0
    } else {
        &[0]
    };
    unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), output, source.len()) };
}

/// Resolve a package without taking ownership of native objects.
///
/// # Safety
/// Inputs and callback-returned strings must be NUL-terminated TCHAR strings.
/// `output` must have `capacity >= 1` writable TCHARs. `has_guid` requires a valid
/// native GUID in context. Callbacks must not unwind across the ABI and must honor
/// the service contracts, keeping returned strings live until their next mutation.
/// No callbacks are made after the first error. Once the first candidate has been
/// composed, `output` receives the last one (or an empty string when it does not
/// fit), even on error, as the native code left its buffer.
pub unsafe fn find_package_file(
    services: &Hp2PackageFileServices,
    context: *mut c_void,
    input: *const Tchar,
    has_guid: bool,
    output: *mut Tchar,
    capacity: usize,
) -> Result<bool, i32> {
    let mut candidate = None;
    let found = unsafe { search(services, context, input, has_guid, &mut candidate, capacity) };
    if let Some(candidate) = &candidate {
        unsafe { publish(output, capacity, candidate) };
    }
    found
}

unsafe fn search(
    services: &Hp2PackageFileServices,
    context: *mut c_void,
    input: *const Tchar,
    has_guid: bool,
    candidate: &mut Option<Text>,
    capacity: usize,
) -> Result<bool, i32> {
    unsafe {
        let resolver = Resolver { services, context };
        let dll = resolver.string(DLL_EXT, 0)?;
        let input_len = length(input);
        let dll_len = length(dll);
        if input_len > dll_len && resolver.equal(input.add(input_len - dll_len), dll)? {
            return Ok(false);
        }
        const INT: [Tchar; 4] = [105, 110, 116, 0];
        let international = !resolver.equal(resolver.string(LANGUAGE, 0)?, INT.as_ptr())?;
        // UE1 copied at most 255 code units of the input, so a longer path was
        // probed truncated and could match a different file. The whole input is
        // probed; a hit that does not fit `output` is logged by `fits`.
        let mut normalized = Text::new();
        let mut copied = 0;
        while *input.add(copied) != 0 {
            let ch = *input.add(copied);
            normalized.push_units(&[if ch == b'\\' as Tchar {
                b'/' as Tchar
            } else {
                ch
            }]);
            copied += 1;
        }
        let mut base = Text::new();
        let base_start = normalized
            .units()
            .iter()
            .rposition(|&ch| ch == b'/' as Tchar)
            .map_or(0, |slash| slash + 1);
        base.push_units(&normalized.units()[base_start..]);
        let has_directory = base_start != 0;
        let output = candidate.insert(Text::new());
        output.set(&normalized);
        if resolver.exists(output)? {
            return resolver.fits(output, capacity);
        }
        let mut prefix = Text::new();
        let mut cd = 0;
        while cd < resolver.cd_limit()? {
            // The native exact pass deliberately skips path zero on CD.
            let mut index = cd;
            while index < resolver.count()? + i32::from(has_guid) {
                let mut language_pass = 0;
                while language_pass < 1 + i32::from(international) {
                    let extension = resolver.prefix(&mut prefix, cd, index)?;
                    let mut variant = 0;
                    let variants = if has_directory { 2 } else { 1 };
                    while variant < variants {
                        output.set(&prefix);
                        if index < resolver.count()? {
                            output.push_units(if variant == 0 {
                                normalized.units()
                            } else {
                                base.units()
                            });
                        } else {
                            output.push(resolver.string(GUID, 0)?);
                        }
                        let mut found = resolver.exists(output)?;
                        if !found && let Some(extension) = &extension {
                            output.push_char(b'.');
                            if international && language_pass == 0 {
                                output.push(resolver.string(LANGUAGE, 0)?);
                                output.push_char(b'_');
                            }
                            // Native `Ext+1`: skip the extension's first code unit.
                            output.push_units(extension.units().get(1..).unwrap_or(&[]));
                            found = resolver.exists(output)?;
                        }
                        if found {
                            if !resolver.fits(output, capacity)? {
                                return Ok(false);
                            }
                            return resolver.finish(index, output);
                        }
                        variant += 1;
                    }
                    language_pass += 1;
                }
                index += 1;
            }
            cd += 1;
        }
        let mut spec = Text::new();
        let mut input_extension = Text::new();
        let mut base_extension = Text::new();
        cd = 0;
        while cd < resolver.cd_limit()? {
            let mut index = 0;
            while index < resolver.count()? + i32::from(has_guid) {
                let extension = resolver.prefix(&mut prefix, cd, index)?;
                output.set(&prefix);
                if index < resolver.count()? {
                    output.push_units(base.units());
                } else {
                    output.push(resolver.string(GUID, 0)?);
                }
                spec.set(&prefix);
                spec.push_char(b'*');
                input_extension.clear();
                base_extension.clear();
                if let Some(extension) = &extension {
                    spec.push_units(extension.units());
                    input_extension.set(&normalized);
                    input_extension.push_units(extension.units());
                    base_extension.set(&base);
                    base_extension.push_units(extension.units());
                }
                let mut state = Enumeration {
                    resolver: &resolver,
                    normalized: &normalized,
                    input_extension: &input_extension,
                    base: &base,
                    base_extension: &base_extension,
                    prefix: &prefix,
                    output: &mut *output,
                    input,
                    index,
                    capacity,
                    found: false,
                    too_long: false,
                };
                status((services.enumerate)(
                    context,
                    spec.ptr().cast(),
                    visit,
                    complete,
                    (&mut state as *mut Enumeration<'_>).cast(),
                ))?;
                if state.too_long {
                    return Ok(false);
                }
                if state.found {
                    return Ok(true);
                }
                index += 1;
            }
            cd += 1;
        }
        Ok(false)
    }
}
