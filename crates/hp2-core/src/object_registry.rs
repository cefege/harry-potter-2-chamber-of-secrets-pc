//! Rust-owned registry tables, language, independent results and enumeration scratch.
//! Native leaves retain filesystem/config access, typed parsing and localization.
use crate::ansi::{self, Tchar};
use crate::array_storage::{self as array, Hp2ArrayServices, Hp2ArraySlots};
use crate::string_storage;
use hp2_abi::engine::generated::{FPreferencesInfo, FRegistryObjectInfo, TArrayRaw};
use std::{cell::UnsafeCell, ffi::c_void, ptr, rc::Rc};

type Result<T = ()> = std::result::Result<T, i32>;
const EMPTY: TArrayRaw = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
const TAG: [Tchar; 9] = [82, 101, 103, 105, 115, 116, 114, 121, 0];
static ZERO: [Tchar; 1] = [0];
// The general language survives registry cache release, as the SDK's static
// buffer did. Engine-thread serialization also covers callers reading its view.
static mut LANGUAGE: [Tchar; 64] = {
    let mut language = [0; 64];
    language[0] = b'i' as Tchar;
    language[1] = b'n' as Tchar;
    language[2] = b't' as Tchar;
    language
};

/// Stable NUL-terminated SDK view; reads are engine-thread serialized.
pub fn current_language() -> *const Tchar {
    (&raw const LANGUAGE).cast()
}

/// Synchronizes the Core-owned active language used by registry cache discovery.
///
/// # Errors
/// Returns an error when the terminated language exceeds the historical 64-TCHAR buffer.
pub fn synchronize_language(language: &[Tchar]) -> Result {
    let length = language.iter().position(|&unit| unit == 0).unwrap_or(language.len());
    if length >= 64 { return Err(1); }
    unsafe {
        let destination = &mut *(&raw mut LANGUAGE);
        destination.fill(0);
        destination[..length].copy_from_slice(&language[..length]);
    }
    Ok(())
}


/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectRegistryOperation { Cache, Objects, Preferences, Language }
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectRegistryLeaf {
    PathCount, Path, BaseDir, FindFiles, ReadPublic,
    ParseDriverField, ParsePreferenceField, NativeCategory, DllExtension,
    LanguageMetaClass, CheckClass, CheckMetaClass, ClassName, MetaClassName,
    LocalizeWord, NextObjectIndex, ObjectCount,
    ObjectAt, LanguageChange,
}
#[repr(C)]
pub struct Hp2ObjectRegistryArgs {
    pub results: *mut c_void,
    pub class_object: *mut c_void,
    pub meta_class: *mut c_void,
    pub text: *const c_void,
    pub force_refresh: i32,
}
#[repr(C)]
pub struct Hp2ObjectRegistryServices {
    pub call: unsafe extern "C" fn(*mut c_void, Hp2ObjectRegistryLeaf, usize, usize, *mut usize) -> i32,
}
#[repr(C)]
pub struct Hp2ObjectRegistryParse {
    pub output: *mut c_void,
    pub field: usize,
}
#[repr(C)]
pub struct Hp2ObjectRegistrySection {
    pub buffer: *mut c_void,
    pub capacity: i32,
}
use Hp2ObjectRegistryLeaf as L;
struct Flow<'a> { context: *mut c_void, services: &'a Hp2ObjectRegistryServices }
impl Flow<'_> {
    unsafe fn call(&self, op: L, a: usize, b: usize) -> Result<usize> {
        let mut out = 0;
        let status = unsafe { (self.services.call)(self.context, op, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
    unsafe fn text(&self, op: L, a: usize) -> Result<Vec<Tchar>> {
        // Snapshot native ring/config strings before any allocating callback.
        unsafe { snapshot(self.call(op, a, 0)? as *const Tchar) }
    }
}
unsafe fn slots(value: *mut TArrayRaw) -> Hp2ArraySlots {
    // SAFETY: Headers are canonical and live; packed pointer slots are unaligned.
    unsafe { Hp2ArraySlots { data: ptr::addr_of_mut!((*value).data), num: ptr::addr_of_mut!((*value).num), max: ptr::addr_of_mut!((*value).max) } }
}
unsafe fn text(value: *const TArrayRaw) -> *const Tchar {
    unsafe { if ptr::addr_of!((*value).num).read_unaligned() == 0 { ZERO.as_ptr() } else { ptr::addr_of!((*value).data).read_unaligned().cast() } }
}
unsafe fn snapshot(input: *const Tchar) -> Result<Vec<Tchar>> {
    let mut len = 0;
    unsafe { while input.add(len).read() != 0 { len += 1; } }
    let mut copy = Vec::new();
    copy.try_reserve_exact(len + 1).map_err(|_| 1)?;
    unsafe { copy.extend_from_slice(std::slice::from_raw_parts(input, len + 1)); }
    Ok(copy)
}
const fn wide(value: &str) -> [Tchar; 16] {
    let mut out = [0; 16];
    let mut index = 0;
    while index < value.len() { out[index] = value.as_bytes()[index] as Tchar; index += 1; }
    out
}
unsafe fn equal(a: *const Tchar, b: *const Tchar) -> bool { unsafe { ansi::stricmp(a, b) == 0 } }
unsafe fn assign(dst: *mut TArrayRaw, src: *const Tchar, arrays: Hp2ArrayServices) -> Result {
    unsafe { string_storage::assign_with_allocator(slots(dst), None, src, false, arrays, TAG.as_ptr().cast()) }
}
unsafe fn take_header(value: *mut TArrayRaw) -> TArrayRaw {
    // Native headers may have four-byte rather than Rust pointer alignment.
    unsafe {
        let old = value.read_unaligned();
        value.write_unaligned(EMPTY);
        old
    }
}
unsafe fn free_header(value: *mut TArrayRaw, arrays: Hp2ArrayServices) -> Result {
    // Detach before calling the allocator: reentry cannot free this owner twice.
    let data = unsafe { take_header(value).data };
    if data.is_null() { return Ok(()); }
    let status = unsafe { (arrays.free)(arrays.context, data) };
    if status == 0 { Ok(()) } else { Err(status) }
}
struct OwnedArray { raw: UnsafeCell<TArrayRaw>, width: i32, strings: usize, arrays: Hp2ArrayServices }
impl OwnedArray {
    fn new(width: usize, strings: usize, arrays: Hp2ArrayServices) -> Self {
        Self { raw: UnsafeCell::new(EMPTY), width: width as i32, strings, arrays }
    }
    fn header(&self) -> *mut TArrayRaw { self.raw.get() }
    unsafe fn len(&self) -> i32 { unsafe { (*self.header()).num } }
    unsafe fn at(&self, index: i32) -> *mut u8 { unsafe { (*self.header()).data.cast::<u8>().add(index as usize * self.width as usize) } }
    unsafe fn append(&self) -> Result<i32> {
        let raw = self.header();
        let mut pending_num = unsafe { (*raw).num };
        let before_max = unsafe { (*raw).max };
        let mut pending_slots = unsafe { slots(raw) };
        pending_slots.num = &raw mut pending_num;
        let added = unsafe { array::add(pending_slots, 1, self.width, TAG.as_ptr().cast(), self.arrays) };
        match added {
            Ok(index) => {
                // Reentrant readers see only initialized records, including
                // while the allocator is growing the table's backing storage.
                unsafe {
                    self.at(index).write_bytes(0, self.width as usize);
                    (*raw).num = pending_num;
                }
                Ok(index)
            }
            Err(status) => { unsafe { (*raw).max = before_max; } Err(status) }
        }
    }
    unsafe fn clear(&self) -> Result {
        // Retire the complete header before any free callback.
        let mut old = unsafe { ptr::replace(self.header(), EMPTY) };
        unsafe { clear_table(&mut old, self.width, self.strings, self.arrays) }
    }
}
// Nested calls record destructor failures independently, without replacing the
// first operation/native exception.
static mut DROP_STATUS: *mut i32 = ptr::null_mut();
impl Drop for OwnedArray {
    fn drop(&mut self) {
        if let Err(status) = unsafe { OwnedArray::clear(self) } {
            unsafe {
                let slot = DROP_STATUS;
                if !slot.is_null() && slot.read() == 0 { slot.write(status); }
            }
        }
    }
}
unsafe fn clear_table(raw: *mut TArrayRaw, width: i32, strings: usize, arrays: Hp2ArrayServices) -> Result {
    let old = unsafe { take_header(raw) };
    if old.data.is_null() { return Ok(()); }
    let mut first = Ok(());
    for index in 0..old.num {
        for field in 0..strings {
            let header = unsafe { old.data.cast::<u8>().add(index as usize * width as usize).cast::<TArrayRaw>().add(field) };
            let result = unsafe { free_header(header, arrays) };
            if first.is_ok() { first = result; }
        }
    }
    let status = unsafe { (arrays.free)(arrays.context, old.data) };
    first.and(if status == 0 { Ok(()) } else { Err(status) })
}
struct Cache {
    language: OwnedArray,
    drivers: OwnedArray,
    preferences: OwnedArray,
}
impl Cache {
    fn new(arrays: Hp2ArrayServices) -> Self {
        Self { language: OwnedArray::new(size_of::<Tchar>(), 0, arrays), drivers: OwnedArray::new(size_of::<FRegistryObjectInfo>(), 5, arrays), preferences: OwnedArray::new(size_of::<FPreferencesInfo>(), 3, arrays) }
    }
}
// Engine-thread only. A call pins its cache generation while allocator/parser
// callbacks may refresh or exit. Retired generations live until their last reader
// returns; no pointer into a freed generation survives reentry.
static mut CACHE: Option<Rc<Cache>> = None;
unsafe fn current() -> Option<Rc<Cache>> { unsafe { (*(&raw const CACHE)).clone() } }
unsafe fn publish(cache: Option<Rc<Cache>>) -> Result {
    let old = unsafe { ptr::replace(&raw mut CACHE, cache) };
    if let Some(old) = old {
        if let Ok(old) = Rc::try_unwrap(old) {
            let preferences = unsafe { old.preferences.clear() };
            let drivers = unsafe { old.drivers.clear() };
            let language = unsafe { old.language.clear() };
            return preferences.and(drivers).and(language);
        }
    }
    Ok(())
}
/// Release the registry generation, including its language, on subsystem exit.
/// # Safety
/// Engine-thread serialized with registry calls and allocator publication.
pub unsafe fn release() -> Result {
    unsafe { publish(None) }
}

// These exact bounds preserve the native bounded path formatting and Public
// section limit. The storage and all intermediate offsets now belong to Rust.
fn join(buffer: &mut [Tchar], at: usize, a: &[Tchar], b: &[Tchar]) {
    let mut out = at;
    for c in a.iter().take_while(|c| **c != 0).chain(b.iter().take_while(|c| **c != 0)) {
        if out + 1 >= buffer.len() { break; }
        buffer[out] = *c;
        out += 1;
    }
    buffer[out] = 0;
}
fn end(buffer: &[Tchar], start: usize) -> usize { buffer[start..].iter().position(|c| *c == 0).map_or(buffer.len(), |n| start + n) }
fn find(buffer: &[Tchar], start: usize, needle: &[Tchar]) -> Option<usize> {
    let stop = end(buffer, start);
    buffer[start..stop].windows(needle.len()).position(|s| s == needle).map(|n| n + start)
}

/// Removes the registry tuple envelope in place and returns the value offset.
pub(crate) fn prepare_registry_value(value: &mut [Tchar]) -> usize {
    let start = if value.first() == Some(&40) {
        value[0] = 0;
        1
    } else {
        0
    };
    let stop = end(value, start);
    if stop > start && value[stop - 1] == 41 {
        value[stop - 1] = 0;
    }
    start
}
unsafe fn parse_records(f: &Flow<'_>, cache: &Cache, buffer: &mut [Tchar]) -> Result {
    let mut key = 0;
    while key < buffer.len() && buffer[key] != 0 {
        let next = end(buffer, key) + 1;
        if let Some(eq) = find(buffer, key, &[61]) {
            buffer[eq] = 0;
            let value = eq + 1 + prepare_registry_value(&mut buffer[eq + 1..]);
            let driver = unsafe { equal(buffer.as_ptr().add(key), wide("Object").as_ptr()) };
            let preference = unsafe { equal(buffer.as_ptr().add(key), wide("Preferences").as_ptr()) };
            if driver || preference {
                let table = if driver { &cache.drivers } else { &cache.preferences };
                let index = unsafe { table.append()? };
                // Parse into a stable local header, not a relocatable table slot.
                // Commit each field even on failure, retaining the native prefix.
                for field in 0..5 {
                    if driver || field < 3 {
                        let mut output = EMPTY;
                        let request = Hp2ObjectRegistryParse { output: (&raw mut output).cast(), field };
                        let result = unsafe { f.call(if driver { L::ParseDriverField } else { L::ParsePreferenceField }, buffer.as_ptr().add(value) as usize, (&raw const request) as usize) };
                        if result.is_err() { output.num = 0; if output.data.is_null() { output.max = 0; } }
                        unsafe { table.at(index).cast::<TArrayRaw>().add(field).write_unaligned(output); }
                        result?;
                    } else {
                        let mut output = 0i32;
                        let request = Hp2ObjectRegistryParse { output: (&raw mut output).cast(), field };
                        let result = unsafe { f.call(L::ParsePreferenceField, buffer.as_ptr().add(value) as usize, (&raw const request) as usize) };
                        let record = unsafe { table.at(index).cast::<FPreferencesInfo>() };
                        unsafe {
                            if field == 3 { ptr::addr_of_mut!((*record).category).write_unaligned(output.to_ne_bytes()); }
                            else { ptr::addr_of_mut!((*record).immediate).write_unaligned(output); }
                        }
                        result?;
                    }
                }
                if preference {
                    let category = unsafe { ptr::addr_of!((*table.at(index).cast::<FPreferencesInfo>()).category).read_unaligned() };
                    let mut category = i32::from_ne_bytes(category);
                    unsafe { f.call(L::NativeCategory, (&raw mut category) as usize, 0)?; }
                }
            }
        }
        key = next;
    }
    Ok(())
}
pub(crate) trait RegistryFiles {
    type Error;
    fn len(&self) -> usize;
    fn name(&self, index: usize) -> std::result::Result<Vec<Tchar>, Self::Error>;
    fn clear(self) -> std::result::Result<(), Self::Error>;
}

pub(crate) trait RegistryDiscoveryOps {
    type Error;
    type Files: RegistryFiles<Error = Self::Error>;
    fn path_count(&self) -> std::result::Result<usize, Self::Error>;
    fn path(&self, index: usize) -> std::result::Result<Vec<Tchar>, Self::Error>;
    fn base_dir(&self) -> std::result::Result<Vec<Tchar>, Self::Error>;
    fn language(&self) -> std::result::Result<Vec<Tchar>, Self::Error>;
    fn files(&self, pattern: &[Tchar]) -> std::result::Result<Self::Files, Self::Error>;
    /// Append the Public section if present; false enables the .int fallback.
    fn visit_public(&self, filename: &[Tchar]) -> std::result::Result<bool, Self::Error>;
}

/// Shared registry search order, bounded path formatting and language fallback.
pub(crate) fn discover_public_with<O: RegistryDiscoveryOps>(ops: &O) -> std::result::Result<(), O::Error> {
    let mut filename = [0 as Tchar; 4096];
    let mut path = 0;
    while path < ops.path_count()? {
        let base = ops.base_dir()?;
        let pattern = ops.path(path)?;
        join(&mut filename, 0, &base, &pattern);
        if let Some(tmp) = find(&filename, 0, &[42, 46]) {
            join(&mut filename, tmp, &wide("*.int"), &[0]);
            let files = ops.files(&filename)?;
            let mut file = 0;
            while file < files.len() {
                let base = ops.base_dir()?;
                let name = files.name(file)?;
                join(&mut filename, tmp, &base, &name);
                let extension = end(&filename, tmp).saturating_sub(4).max(tmp);
                let language = ops.language()?;
                join(&mut filename, extension, &wide("."), &language);
                if !ops.visit_public(&filename[tmp..])? {
                    join(&mut filename, extension, &wide(".int"), &[0]);
                    ops.visit_public(&filename[tmp..])?;
                }
                file += 1;
            }
            files.clear()?;
        }
        path += 1;
    }
    Ok(())
}

impl RegistryFiles for OwnedArray {
    type Error = i32;
    fn len(&self) -> usize { unsafe { OwnedArray::len(self) as usize } }
    fn name(&self, index: usize) -> Result<Vec<Tchar>> {
        unsafe { snapshot(text(self.at(index as i32).cast())) }
    }
    fn clear(self) -> Result { unsafe { OwnedArray::clear(&self) } }
}

struct LegacyRegistryDiscovery<'a, 'b> {
    flow: &'a Flow<'b>,
    cache: &'a Cache,
    arrays: Hp2ArrayServices,
}
impl RegistryDiscoveryOps for LegacyRegistryDiscovery<'_, '_> {
    type Error = i32;
    type Files = OwnedArray;
    fn path_count(&self) -> Result<usize> { unsafe { self.flow.call(L::PathCount, 0, 0) } }
    fn path(&self, index: usize) -> Result<Vec<Tchar>> { unsafe { self.flow.text(L::Path, index) } }
    fn base_dir(&self) -> Result<Vec<Tchar>> { unsafe { self.flow.text(L::BaseDir, 0) } }
    fn language(&self) -> Result<Vec<Tchar>> { unsafe { snapshot(current_language()) } }
    fn files(&self, pattern: &[Tchar]) -> Result<OwnedArray> {
        let files = OwnedArray::new(size_of::<TArrayRaw>(), 1, self.arrays);
        unsafe { self.flow.call(L::FindFiles, pattern.as_ptr() as usize, files.header() as usize)?; }
        Ok(files)
    }
    fn visit_public(&self, filename: &[Tchar]) -> Result<bool> {
        let mut buffer = [0 as Tchar; 32767];
        let request = Hp2ObjectRegistrySection { buffer: buffer.as_mut_ptr().cast(), capacity: buffer.len() as i32 };
        let found = unsafe { self.flow.call(L::ReadPublic, filename.as_ptr() as usize, (&raw const request) as usize)? } != 0;
        if found { unsafe { parse_records(self.flow, self.cache, &mut buffer)?; } }
        Ok(found)
    }
}

unsafe fn cache(f: &Flow<'_>, force: i32, arrays: Hp2ArrayServices) -> Result<Rc<Cache>> {
    let language = unsafe { snapshot(current_language())? };
    if force == 0 {
        if let Some(old) = unsafe { current() } {
            if unsafe { equal(text(old.language.header()), language.as_ptr()) } { return Ok(old); }
        }
    }
    let cache = Rc::new(Cache::new(arrays));
    unsafe { assign(cache.language.header(), language.as_ptr(), arrays)?; publish(Some(cache.clone()))?; }
    discover_public_with(&LegacyRegistryDiscovery { flow: f, cache: &cache, arrays })?;
    let core = OwnedArray::new(size_of::<TArrayRaw>(), 1, arrays);
    unsafe { f.call(L::FindFiles, wide("Core.*").as_ptr() as usize, core.header() as usize)?; }
    let mut file = 0;
    while file < unsafe { core.len() } {
        let name = unsafe { snapshot(text(core.at(file).cast()))? };
        let stop = name.len() - 1;
        if let Some(dot) = name[..stop].iter().rposition(|c| *c == 46) {
            let extension = &name[dot..];
            let dll = unsafe { f.text(L::DllExtension, 0)? };
            if !unsafe { equal(extension.as_ptr(), dll.as_ptr()) || equal(extension.as_ptr(), wide(".u").as_ptr()) || equal(extension.as_ptr(), wide(".ilk").as_ptr()) } {
                let index = unsafe { cache.drivers.append()? };
                let first = name[..stop].iter().position(|c| *c == 46).unwrap_or(dot);
                for field in 0..3 {
                    let meta;
                    let class = wide("Class");
                    let value: &[Tchar] = match field {
                        0 => &name[first + 1..],
                        1 => &class,
                        _ => { meta = unsafe { f.text(L::LanguageMetaClass, 0)? }; &meta }
                    };
                    let mut output = EMPTY;
                    let result = unsafe { assign(&raw mut output, value.as_ptr(), arrays) };
                    if result.is_err() { output.num = 0; if output.data.is_null() { output.max = 0; } }
                    unsafe { cache.drivers.at(index).cast::<TArrayRaw>().add(field).write_unaligned(output); }
                    result?;
                }
            }
        }
        file += 1;
    }
    unsafe { core.clear()?; }
    Ok(cache)
}

unsafe fn append_result(result: *mut TArrayRaw, source: &OwnedArray, index: i32, arrays: Hp2ArrayServices) -> Result {
    // Copy headers, not payload ownership. The cache generation stays pinned and
    // its published strings are immutable while destination callbacks reenter.
    let mut headers = [EMPTY; 5];
    for (field, header) in headers.iter_mut().enumerate().take(source.strings) {
        *header = unsafe { source.at(index).cast::<TArrayRaw>().add(field).read_unaligned() };
    }
    let scalars = if source.strings == 3 {
        let record = unsafe { source.at(index).cast::<FPreferencesInfo>().read_unaligned() };
        Some((record.category, record.immediate))
    } else { None };
    let destination = unsafe { slots(result) };
    let old_num = unsafe { destination.num.read() };
    let old_max = unsafe { destination.max.read() };
    let index = match unsafe { array::add(destination, 1, source.width, TAG.as_ptr().cast(), arrays) } {
        Ok(index) => index,
        Err(status) => { unsafe { destination.num.write(old_num); destination.max.write(old_max); } return Err(status); }
    };
    unsafe { destination.data.read_unaligned().cast::<u8>().add(index as usize * source.width as usize).write_bytes(0, source.width as usize); }
    for (field, header) in headers.iter_mut().enumerate().take(source.strings) {
        let mut output = EMPTY;
        let operation = unsafe { string_storage::assign_with_allocator(slots(&raw mut output), Some(slots(header)), ptr::null(), true, arrays, TAG.as_ptr().cast()) };
        if operation.is_err() { output.num = 0; if output.data.is_null() { output.max = 0; } }
        // Reacquire after allocation: the result array may have relocated.
        unsafe { destination.data.read_unaligned().cast::<u8>().add(index as usize * source.width as usize).cast::<TArrayRaw>().add(field).write_unaligned(output); }
        operation?;
    }
    if let Some((category, immediate)) = scalars {
        let record = unsafe { destination.data.read_unaligned().cast::<FPreferencesInfo>().add(index as usize) };
        unsafe { ptr::addr_of_mut!((*record).category).write_unaligned(category); ptr::addr_of_mut!((*record).immediate).write_unaligned(immediate); }
    }
    Ok(())
}
/// # Safety
/// Engine-thread serialized. Native arguments and guarded callbacks live for the
/// synchronous call. Result headers must remain live across reentrant callbacks.
/// Language input must be NUL-terminated and fit the historical 64-TCHAR buffer.
pub unsafe fn run(operation: Hp2ObjectRegistryOperation, args: &Hp2ObjectRegistryArgs, context: *mut c_void, services: &Hp2ObjectRegistryServices) -> Result {
    let f = Flow { context, services };
    use Hp2ObjectRegistryOperation as O;
    let mut cleanup_error = 0;
    let previous = unsafe { ptr::replace(&raw mut DROP_STATUS, &raw mut cleanup_error) };
    let result = (|| {
    unsafe {
        match operation {
            O::Cache => { cache(&f, args.force_refresh, array::published_allocator()?)?; }
            O::Objects => {
                f.call(L::CheckClass, 0, 0)?;
                f.call(L::CheckMetaClass, 0, 0)?;
                let arrays = array::published_allocator()?;
                let cache = cache(&f, args.force_refresh, arrays)?;
                let class = f.text(L::ClassName, 0)?;
                let meta = f.text(L::MetaClassName, 0)?;
                let mut index = 0;
                while index < cache.drivers.len() {
                    let record = cache.drivers.at(index).cast::<FRegistryObjectInfo>();
                    if equal(text(ptr::addr_of!((*record).class)), class.as_ptr()) && equal(text(ptr::addr_of!((*record).meta_class)), meta.as_ptr()) {
                        append_result(args.results.cast(), &cache.drivers, index, arrays)?;
                    }
                    index += 1;
                }
            }
            O::Preferences => {
                let arrays = array::published_allocator()?;
                let cache = cache(&f, args.force_refresh, arrays)?;
                let parent = snapshot(args.text.cast())?;
                clear_table(args.results.cast(), size_of::<FPreferencesInfo>() as i32, 3, arrays)?;
                let mut index = 0;
                while index < cache.preferences.len() {
                    let record = cache.preferences.at(index).cast::<FPreferencesInfo>();
                    if equal(text(ptr::addr_of!((*record).parent_caption)), parent.as_ptr()) { append_result(args.results.cast(), &cache.preferences, index, arrays)?; }
                    index += 1;
                }
            }
            O::Language => {
                if equal(args.text.cast(), current_language()) { return Ok(()); }
                ansi::strcpy((&raw mut LANGUAGE).cast(), args.text.cast());
                for word in 0..5 { f.call(L::LocalizeWord, word, 0)?; }
                let mut i = f.call(L::NextObjectIndex, (-1isize) as usize, 0)?;
                while i < f.call(L::ObjectCount, 0, 0)? {
                    let object = f.call(L::ObjectAt, i, 0)?;
                    f.call(L::LanguageChange, object, 0)?;
                    i = f.call(L::NextObjectIndex, i, 0)?;
                }
            }
        }
    }
        Ok(())
    })();
    unsafe { (&raw mut DROP_STATUS).write(previous); }
    result.and(if cleanup_error == 0 { Ok(()) } else { Err(cleanup_error) })
}
