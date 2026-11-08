//! Versioned package summaries over compiler-probed layouts and core array storage.

use std::ffi::c_void;
use std::ptr::{self, addr_of, addr_of_mut};

use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_abi::engine::generated::{FGenerationInfo, FPackageFileSummary, TArrayRaw};

use crate::package_archive::{GenerationInfo, PackageSummary};

/// Direct Rust reader contract for the versioned package summary wire fields.
pub trait PackageSummaryInput {
    /// Error returned by the archive owner.
    type Error;
    /// Constructs a format error without changing archive error transport.
    fn error(&self, message: &str) -> Self::Error;
    /// Archive position before reading.
    fn tell(&self) -> usize;
    /// Archive seek preserving its first failure.
    fn seek(&mut self, position: usize) -> Result<(), Self::Error>;
    /// Reads a little-endian archive integer.
    fn read_i32(&mut self) -> Result<i32, Self::Error>;
    /// Reads a little-endian archive DWORD.
    fn read_u32(&mut self) -> Result<u32, Self::Error>;
}

/// Scalar fields in the common package-summary wire prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummaryIntField { Tag, FileVersion, NameCount, NameOffset, ExportCount, ExportOffset, ImportCount, ImportOffset }

/// Per-operation summary traversal. Each method accesses one live field only;
/// version-specific tails retain their owning archive's allocation/reentry rules.
pub trait PackageSummaryOps {
    type Error;
    fn int(&mut self, field: SummaryIntField) -> Result<(), Self::Error>;
    fn dword(&mut self) -> Result<(), Self::Error>;
    fn version(&self) -> u16;
    fn prepare_modern_generations(&mut self) -> Result<(), Self::Error> { Ok(()) }
    fn guid(&mut self) -> Result<(), Self::Error>;
    fn modern_generations(&mut self) -> Result<(), Self::Error>;
    fn legacy_heritage(&mut self) -> Result<(), Self::Error>;
}
 
/// Shared versioned field ordering for direct and legacy package summaries.
pub fn walk_package_summary<O: PackageSummaryOps>(ops: &mut O) -> Result<(), O::Error> {
    use SummaryIntField::*;
    ops.int(Tag)?;
    ops.int(FileVersion)?;
    ops.dword()?;
    ops.int(NameCount)?;
    ops.int(NameOffset)?;
    ops.int(ExportCount)?;
    ops.int(ExportOffset)?;
    ops.int(ImportCount)?;
    ops.int(ImportOffset)?;
    if ops.version() >= 68 {
        ops.prepare_modern_generations()?;
        ops.guid()?;
        ops.modern_generations()
    } else {
        ops.legacy_heritage()
    }
}


struct ReadSummary<'a, I> { input: &'a mut I, result: PackageSummary }
impl<I: PackageSummaryInput> PackageSummaryOps for ReadSummary<'_, I> {
    type Error = I::Error;
    fn int(&mut self, field: SummaryIntField) -> Result<(), Self::Error> {
        let value = self.input.read_i32()?;
        match field {
            SummaryIntField::Tag => {
                if value as u32 != 0x9e2a_83c1 { return Err(self.input.error("invalid package tag")); }
                self.result.tag = value as u32;
            }
            SummaryIntField::FileVersion => {
                self.result.version = value as u16;
                self.result.licensee_version = (value as u32 >> 16) as u16;
            }
            SummaryIntField::NameCount | SummaryIntField::ExportCount | SummaryIntField::ImportCount if value < 0 => {
                return Err(self.input.error("negative package table count"));
            }
            SummaryIntField::NameOffset | SummaryIntField::ExportOffset | SummaryIntField::ImportOffset if value < 0 => {
                return Err(self.input.error("negative package table offset"));
            }
            SummaryIntField::NameCount => self.result.name_count = value as u32,
            SummaryIntField::NameOffset => self.result.name_offset = value as u32,
            SummaryIntField::ExportCount => self.result.export_count = value as u32,
            SummaryIntField::ExportOffset => self.result.export_offset = value as u32,
            SummaryIntField::ImportCount => self.result.import_count = value as u32,
            SummaryIntField::ImportOffset => self.result.import_offset = value as u32,
        }
        Ok(())
    }
    fn dword(&mut self) -> Result<(), Self::Error> { self.result.package_flags = self.input.read_u32()?; Ok(()) }
    fn version(&self) -> u16 { self.result.version }
    fn guid(&mut self) -> Result<(), Self::Error> {
        for word in &mut self.result.guid { *word = self.input.read_u32()?; }
        Ok(())
    }
    fn modern_generations(&mut self) -> Result<(), Self::Error> {
        let count = self.input.read_i32()?;
        if count < 0 { return Err(self.input.error("negative package generation count")); }
        for _ in 0..count {
            self.result.generations.push(GenerationInfo { export_count: self.input.read_i32()?, name_count: self.input.read_i32()? });
        }
        Ok(())
    }
    fn legacy_heritage(&mut self) -> Result<(), Self::Error> {
        let count = self.input.read_i32()?;
        let offset = self.input.read_i32()?;
        if count < 0 || offset < 0 { return Err(self.input.error("invalid package heritage range")); }
        let resume = self.input.tell();
        if count != 0 {
            self.input.seek(offset as usize)?;
            for _ in 0..count { self.guid()?; }
            self.input.seek(resume)?;
        }
        self.result.generations.push(GenerationInfo { export_count: self.result.export_count as i32, name_count: self.result.name_count as i32 });
        Ok(())
    }
}

/// Reads a package summary through the shared direct/legacy field traversal.
pub fn read_owned_summary<I: PackageSummaryInput>(input: &mut I) -> Result<PackageSummary, I::Error> {
    let mut ops = ReadSummary {
        input,
        result: PackageSummary {
            tag: 0, version: 0, licensee_version: 0, package_flags: 0,
            name_count: 0, name_offset: 0, export_count: 0, export_offset: 0,
            import_count: 0, import_offset: 0, guid: [0; 4], generations: Vec::new(),
        },
    };
    walk_package_summary(&mut ops)?;
    Ok(ops.result)
}

/// Direct Rust writer contract for package summary fields.
pub trait PackageSummaryOutput {
    type Error;
    fn error(&self, message: &str) -> Self::Error;
    fn write_i32(&mut self, value: i32) -> Result<(), Self::Error>;
    fn write_u32(&mut self, value: u32) -> Result<(), Self::Error>;
}

struct WriteSummary<'a, W> { output: &'a mut W, summary: &'a PackageSummary }
impl<W: PackageSummaryOutput> PackageSummaryOps for WriteSummary<'_, W> {
    type Error = W::Error;
    fn int(&mut self, field: SummaryIntField) -> Result<(), Self::Error> {
        let value = match field {
            SummaryIntField::Tag => self.summary.tag as i32,
            SummaryIntField::FileVersion => ((u32::from(self.summary.licensee_version) << 16) | u32::from(self.summary.version)) as i32,
            SummaryIntField::NameCount => i32::try_from(self.summary.name_count).map_err(|_| self.output.error("package summary field overflow"))?,
            SummaryIntField::NameOffset => i32::try_from(self.summary.name_offset).map_err(|_| self.output.error("package summary field overflow"))?,
            SummaryIntField::ExportCount => i32::try_from(self.summary.export_count).map_err(|_| self.output.error("package summary field overflow"))?,
            SummaryIntField::ExportOffset => i32::try_from(self.summary.export_offset).map_err(|_| self.output.error("package summary field overflow"))?,
            SummaryIntField::ImportCount => i32::try_from(self.summary.import_count).map_err(|_| self.output.error("package summary field overflow"))?,
            SummaryIntField::ImportOffset => i32::try_from(self.summary.import_offset).map_err(|_| self.output.error("package summary field overflow"))?,
        };
        self.output.write_i32(value)
    }
    fn dword(&mut self) -> Result<(), Self::Error> { self.output.write_u32(self.summary.package_flags) }
    fn version(&self) -> u16 { self.summary.version }
    fn guid(&mut self) -> Result<(), Self::Error> {
        for word in self.summary.guid { self.output.write_u32(word)?; }
        Ok(())
    }
    fn modern_generations(&mut self) -> Result<(), Self::Error> {
        let count = i32::try_from(self.summary.generations.len()).map_err(|_| self.output.error("generation count overflow"))?;
        self.output.write_i32(count)?;
        for generation in &self.summary.generations {
            self.output.write_i32(generation.export_count)?;
            self.output.write_i32(generation.name_count)?;
        }
        Ok(())
    }
    fn legacy_heritage(&mut self) -> Result<(), Self::Error> {
        self.output.write_i32(0)?;
        self.output.write_i32(0)
    }
}

/// Writes a package summary through the shared field traversal.
pub fn write_owned_summary<W: PackageSummaryOutput>(output: &mut W, summary: &PackageSummary) -> Result<(), W::Error> {
    walk_package_summary(&mut WriteSummary { output, summary })
}
/// Native leaves retain typed archive operations, allocator pairing and errors.
#[repr(C)]
pub struct Hp2PackageSummaryArchiveServices {
    pub archive_is_loading: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    pub archive_serialize_int: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub archive_serialize_dword: unsafe extern "C" fn(ar: *mut c_void, value: *mut u32) -> i32,
    pub archive_serialize_guid: unsafe extern "C" fn(ar: *mut c_void, guid: *mut c_void) -> i32,
    pub archive_tell: unsafe extern "C" fn(ar: *mut c_void, position: *mut i32) -> i32,
    pub archive_seek: unsafe extern "C" fn(ar: *mut c_void, position: i32) -> i32,
    pub arrays: *const Hp2ArrayServices,
    pub array_tag: *const c_void,
}

unsafe fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    // SAFETY: The caller supplies a live canonical header, possibly packed.
    unsafe {
        Hp2ArraySlots {
            data: addr_of_mut!((*array).data),
            num: addr_of_mut!((*array).num),
            max: addr_of_mut!((*array).max),
        }
    }
}

unsafe fn replace_generations(
    array: *mut TArrayRaw,
    count: i32,
    svc: &Hp2PackageSummaryArchiveServices,
) -> Result<(), i32> {
    if svc.arrays.is_null() {
        return Err(1);
    }
    // SAFETY: Resource callbacks use the native array allocator. Generation
    // records have no destructor effects, including after a partial load.
    unsafe {
        let allocator = svc.arrays.read_unaligned();
        let mut replacement = TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        };
        let replacement_slots = array_slots(&raw mut replacement);
        array_storage::set_exact(
            replacement_slots,
            count,
            size_of::<FGenerationInfo>() as i32,
            svc.array_tag,
            allocator,
        )?;
        let slots = array_slots(array);
        if let Err(code) = array_storage::release(slots, allocator) {
            let _ = array_storage::release(replacement_slots, allocator);
            return Err(code);
        }
        // Publish only after allocation succeeds; scalar storage intentionally
        // remains uninitialized until the typed loading leaves write it.
        slots.data.write_unaligned(replacement.data);
        slots.max.write(count);
        slots.num.write(count);
    }
    Ok(())
}

unsafe fn set_legacy_generation(
    summary: *mut FPackageFileSummary,
    svc: &Hp2PackageSummaryArchiveServices,
) -> Result<(), i32> {
    if svc.arrays.is_null() {
        return Err(1);
    }
    // SAFETY: The canonical header remains live over resource leaves. Only
    // individual raw slots are retained; no native state is exclusively borrowed.
    unsafe {
        let slots = array_slots(addr_of_mut!((*summary).generations));
        let allocator = svc.arrays.read_unaligned();
        array_storage::empty(
            slots,
            1,
            size_of::<FGenerationInfo>() as i32,
            svc.array_tag,
            allocator,
        )?;
        array_storage::add(
            slots,
            1,
            size_of::<FGenerationInfo>() as i32,
            svc.array_tag,
            allocator,
        )?;
        slots.data.read_unaligned().cast::<FGenerationInfo>().write(FGenerationInfo {
            export_count: addr_of!((*summary).export_count).read(),
            name_count: addr_of!((*summary).name_count).read(),
        });
    }
    Ok(())
}

/// Serializes one generation record as `ExportCount`, then `NameCount`.
///
/// # Safety
/// The archive, canonical record storage and services remain live for this call.
/// Save fields are initialized; loading leaves initialize fields without reading
/// them. No record reference is held across reentrant archive callbacks.
pub unsafe fn serialize_generation(
    ar: *mut c_void,
    generation: *mut c_void,
    svc: &Hp2PackageSummaryArchiveServices,
) -> i32 {
    if ar.is_null() || generation.is_null() {
        return 1;
    }
    let generation = generation.cast::<FGenerationInfo>();
    // SAFETY: Only raw addresses of possibly uninitialized scalar fields cross
    // the typed leaves; no Rust reference to the generation is constructed.
    unsafe {
        for value in [
            addr_of_mut!((*generation).export_count),
            addr_of_mut!((*generation).name_count),
        ] {
            let status = (svc.archive_serialize_int)(ar, value);
            if status != 0 {
                return status;
            }
        }
    }
    0
}

/// Serializes one GUID as four archive-order DWORDs.
///
/// # Safety
/// The archive and four consecutive, `u32`-aligned GUID words remain valid.
/// Loading callbacks initialize each word without reading its prior value.
pub unsafe fn serialize_guid(
    ar: *mut c_void,
    guid: *mut c_void,
    svc: &Hp2PackageSummaryArchiveServices,
) -> i32 {
    if ar.is_null() || guid.is_null() {
        return 1;
    }
    let words = guid.cast::<u32>();
    for index in 0..4 {
        // SAFETY: Each of the four words is writable canonical GUID storage.
        let status = unsafe { (svc.archive_serialize_dword)(ar, words.add(index)) };
        if status != 0 {
            return status;
        }
    }
    0
}

/// Preserves version-68 generation tables and pre-68 heritage GUID seek/restore.
///
/// Old-version saving writes zero heritage count/offset, rather than the
/// uninitialized locals used by the original C++ implementation.
///
/// Modern replacement allocation failure leaves prior generation storage intact.
/// Once allocation succeeds, the full count is published before record loading;
/// archive errors retain that count and any fields already loaded. Legacy
/// allocation failure retains `Empty(1)` semantics: zero count and live storage.
///
/// # Safety
/// `ar`, canonical `summary` storage and services remain live for this call.
/// The array header and save input fields are initialized; loading leaves write
/// scalar fields before they are read. Archive callbacks may reenter, but must
/// preserve the storage used after they return. Array resources pair with the
/// native `TArray` destructor. No whole-summary/array reference crosses a leaf.
struct LegacySummaryOps<'a> {
    ar: *mut c_void,
    summary: *mut FPackageFileSummary,
    svc: &'a Hp2PackageSummaryArchiveServices,
    generation_count: i32,
}

impl PackageSummaryOps for LegacySummaryOps<'_> {
    type Error = i32;
    fn int(&mut self, field: SummaryIntField) -> Result<(), i32> {
        let value = unsafe {
            match field {
                SummaryIntField::Tag => addr_of_mut!((*self.summary).tag),
                SummaryIntField::FileVersion => addr_of_mut!((*self.summary).file_version),
                SummaryIntField::NameCount => addr_of_mut!((*self.summary).name_count),
                SummaryIntField::NameOffset => addr_of_mut!((*self.summary).name_offset),
                SummaryIntField::ExportCount => addr_of_mut!((*self.summary).export_count),
                SummaryIntField::ExportOffset => addr_of_mut!((*self.summary).export_offset),
                SummaryIntField::ImportCount => addr_of_mut!((*self.summary).import_count),
                SummaryIntField::ImportOffset => addr_of_mut!((*self.summary).import_offset),
            }
        };
        let status = unsafe { (self.svc.archive_serialize_int)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn dword(&mut self) -> Result<(), i32> {
        let value = unsafe { addr_of_mut!((*self.summary).package_flags) };
        let status = unsafe { (self.svc.archive_serialize_dword)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn version(&self) -> u16 { unsafe { addr_of!((*self.summary).file_version).read() as u32 as u16 } }
    fn prepare_modern_generations(&mut self) -> Result<(), i32> {
        self.generation_count = unsafe { addr_of!((*self.summary).generations.num).read_unaligned() };
        Ok(())
    }
    fn guid(&mut self) -> Result<(), i32> {
        let value = unsafe { addr_of_mut!((*self.summary).guid) }.cast();
        let status = unsafe { (self.svc.archive_serialize_guid)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn modern_generations(&mut self) -> Result<(), i32> {
        let mut count = self.generation_count;
        let status = unsafe { (self.svc.archive_serialize_int)(self.ar, &raw mut count) };
        if status != 0 { return Err(status); }
        if unsafe { (self.svc.archive_is_loading)(self.ar) } != 0 {
            unsafe { replace_generations(addr_of_mut!((*self.summary).generations), count, self.svc) }
                .map_err(|code| code)?;
        }
        for index in 0..count {
            let length = unsafe { addr_of!((*self.summary).generations.num).read_unaligned() };
            if index >= length { return Err(1); }
            let data = unsafe { addr_of!((*self.summary).generations.data).read_unaligned() };
            if data.is_null() { return Err(1); }
            let record = unsafe { data.cast::<FGenerationInfo>().add(index as usize).cast() };
            let status = unsafe { serialize_generation(self.ar, record, self.svc) };
            if status != 0 { return Err(status); }
        }
        Ok(())
    }
    fn legacy_heritage(&mut self) -> Result<(), i32> {
        let mut heritage_count = 0;
        let mut heritage_offset = 0;
        let status = unsafe { (self.svc.archive_serialize_int)(self.ar, &raw mut heritage_count) };
        if status != 0 { return Err(status); }
        let status = unsafe { (self.svc.archive_serialize_int)(self.ar, &raw mut heritage_offset) };
        if status != 0 { return Err(status); }
        let mut saved = 0;
        let status = unsafe { (self.svc.archive_tell)(self.ar, &raw mut saved) };
        if status != 0 { return Err(status); }
        if heritage_count != 0 {
            let status = unsafe { (self.svc.archive_seek)(self.ar, heritage_offset) };
            if status != 0 { return Err(status); }
            for _ in 0..heritage_count { self.guid()?; }
        }
        let status = unsafe { (self.svc.archive_seek)(self.ar, saved) };
        if status != 0 { return Err(status); }
        if unsafe { (self.svc.archive_is_loading)(self.ar) } != 0 {
            unsafe { set_legacy_generation(self.summary, self.svc) }.map_err(|code| code)?;
        }
        Ok(())
    }
}

/// Serializes a package summary through the shared live-field traversal.
///
/// # Safety
/// The archive and canonical summary remain valid; callbacks may reenter. The
/// adapter retains no field reference across archive operations.
pub unsafe fn serialize(ar: *mut c_void, summary: *mut c_void, svc: &Hp2PackageSummaryArchiveServices) -> i32 {
    if ar.is_null() || summary.is_null() { return 1; }
    let mut ops = LegacySummaryOps { ar, summary: summary.cast(), svc, generation_count: 0 };
    walk_package_summary(&mut ops).err().unwrap_or(0)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    unsafe extern "C" {
        fn realloc(data: *mut c_void, bytes: usize) -> *mut c_void;
        fn free(data: *mut c_void);
    }

    #[derive(Default)]
    struct Allocator {
        fail_next: bool,
    }

    unsafe extern "C" fn allocate(
        context: *mut c_void,
        data: *mut c_void,
        bytes: usize,
        _max: *const i32,
        _width: i32,
        _tag: *const c_void,
        output: *mut *mut c_void,
    ) -> i32 {
        // SAFETY: The fixture owns the context and pairs every buffer with free.
        unsafe {
            let allocator = &mut *context.cast::<Allocator>();
            if std::mem::take(&mut allocator.fail_next) || bytes > i32::MAX as usize {
                return 17;
            }
            if bytes == 0 {
                free(data);
                output.write(ptr::null_mut());
                return 0;
            }
            let next = realloc(data, bytes);
            if next.is_null() {
                return 17;
            }
            output.write(next);
        }
        0
    }

    unsafe extern "C" fn checked_size(
        _context: *mut c_void,
        bytes: usize,
        output: *mut i32,
    ) -> i32 {
        if bytes > i32::MAX as usize {
            return 17;
        }
        // SAFETY: The array owner supplies a writable scalar output.
        unsafe { output.write(bytes as i32) };
        0
    }

    unsafe extern "C" fn release(_context: *mut c_void, data: *mut c_void) -> i32 {
        // SAFETY: All fixture buffers were obtained from the matching allocator.
        unsafe { free(data) };
        0
    }

    struct Summary {
        value: FPackageFileSummary,
        // Raw callback context avoids Box uniqueness retags when the fixture moves.
        allocator: *mut Allocator,
        arrays: Hp2ArrayServices,
    }

    impl Summary {
        fn new(version: i32, generations: &[(i32, i32)]) -> Self {
            let allocator = Box::into_raw(Box::<Allocator>::default());
            let arrays = Hp2ArrayServices {
                context: allocator.cast(),
                realloc: allocate,
                checked_int_size: checked_size,
                free: release,
            };
            // SAFETY: Canonical summary scalar/header fields accept zero values.
            let mut value: FPackageFileSummary = unsafe { std::mem::zeroed() };
            value.tag = 0x9e2a_83c1u32 as i32;
            value.file_version = version;
            value.package_flags = 0x1234_5678;
            value.name_count = 3;
            value.name_offset = 64;
            value.export_count = 5;
            value.export_offset = 96;
            value.import_count = 2;
            value.import_offset = 128;
            value.guid = [11, 22, 33, 44];
            let mut summary = Self { value, allocator, arrays };
            let services = summary.services();
            // SAFETY: The fixture uses the same raw header/allocator contract
            // as the native owner, then initializes every save-input record.
            unsafe {
                assert!(replace_generations(
                    addr_of_mut!(summary.value.generations),
                    generations.len() as i32,
                    &services,
                ).is_ok());
                for (index, &(export_count, name_count)) in generations.iter().enumerate() {
                    summary.value.generations.data.cast::<FGenerationInfo>().add(index).write(
                        FGenerationInfo { export_count, name_count },
                    );
                }
            }
            summary
        }

        fn services(&self) -> Hp2PackageSummaryArchiveServices {
            services(&raw const self.arrays)
        }

        fn serialize(&mut self, archive: &mut Archive) -> i32 {
            let services = self.services();
            // SAFETY: Fields and resources satisfy the summary owner contract.
            unsafe { serialize((archive as *mut Archive).cast(), (&raw mut self.value).cast(), &services) }
        }

        fn generations(&self) -> Vec<(i32, i32)> {
            (0..self.value.generations.num as usize).map(|index| {
                // SAFETY: Called only for completely initialized records.
                let value = unsafe { self.value.generations.data.cast::<FGenerationInfo>().add(index).read() };
                (value.export_count, value.name_count)
            }).collect()
        }
    }

    impl Drop for Summary {
        fn drop(&mut self) {
            // SAFETY: Trivial generation destruction does not read scalar
            // fields, even when an archive failure left some uninitialized.
            unsafe {
                let _ = array_storage::release(array_slots(addr_of_mut!(self.value.generations)), self.arrays);
                // All synchronous callbacks are finished before reclaiming it.
                drop(Box::from_raw(self.allocator));
            }
        }
    }

    #[derive(Default)]
    struct Archive {
        loading: bool,
        bytes: Vec<u8>,
        position: usize,
        fail_seek: Option<i32>,
        tell_status: i32,
        tell_position: Option<i32>,
        seek_count: usize,
        observe_summary: *mut FPackageFileSummary,
        observed_generation_count: Option<i32>,
    }

    fn transfer(ar: *mut c_void, data: *mut u8, size: usize) -> i32 {
        // SAFETY: The callback receives live archive state and a writable field.
        // Loading copies into raw storage without reading uninitialized bytes.
        unsafe {
            let ar = &mut *ar.cast::<Archive>();
            if ar.position >= 56 && !ar.observe_summary.is_null() {
                ar.observed_generation_count = Some(
                    addr_of!((*ar.observe_summary).generations.num).read_unaligned(),
                );
                addr_of_mut!((*ar.observe_summary).package_flags).write(0xfeed_beef);
            }
            let end = ar.position + size;
            if ar.loading {
                if end > ar.bytes.len() {
                    return 9;
                }
                ptr::copy_nonoverlapping(ar.bytes.as_ptr().add(ar.position), data, size);
            } else {
                if end > ar.bytes.len() {
                    ar.bytes.resize(end, 0);
                }
                ptr::copy_nonoverlapping(data, ar.bytes.as_mut_ptr().add(ar.position), size);
            }
            ar.position = end;
        }
        0
    }

    unsafe extern "C" fn is_loading(ar: *mut c_void) -> i32 {
        unsafe { (*ar.cast::<Archive>()).loading as i32 }
    }
    unsafe extern "C" fn serialize_int(ar: *mut c_void, value: *mut i32) -> i32 {
        transfer(ar, value.cast(), 4)
    }
    unsafe extern "C" fn serialize_dword(ar: *mut c_void, value: *mut u32) -> i32 {
        transfer(ar, value.cast(), 4)
    }
    unsafe extern "C" fn serialize_guid(ar: *mut c_void, guid: *mut c_void) -> i32 {
        // SAFETY: The native typed GUID leaf reenters the DWORD owner likewise.
        unsafe { super::serialize_guid(ar, guid, &services(ptr::null())) }
    }
    unsafe extern "C" fn tell(ar: *mut c_void, position: *mut i32) -> i32 {
        // SAFETY: The owner supplies live archive state and writable output.
        unsafe {
            let ar = &*ar.cast::<Archive>();
            if ar.tell_status != 0 {
                return ar.tell_status;
            }
            position.write(ar.tell_position.unwrap_or(ar.position as i32));
        }
        0
    }
    unsafe extern "C" fn seek(ar: *mut c_void, position: i32) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        ar.seek_count += 1;
        if ar.fail_seek == Some(position) || position < 0 || position as usize > ar.bytes.len() {
            return 19;
        }
        ar.position = position as usize;
        0
    }

    fn services(arrays: *const Hp2ArrayServices) -> Hp2PackageSummaryArchiveServices {
        Hp2PackageSummaryArchiveServices {
            archive_is_loading: is_loading,
            archive_serialize_int: serialize_int,
            archive_serialize_dword: serialize_dword,
            archive_serialize_guid: serialize_guid,
            archive_tell: tell,
            archive_seek: seek,
            arrays,
            array_tag: ptr::null(),
        }
    }

    fn modern_bytes() -> Vec<u8> {
        let mut summary = Summary::new(68, &[(5, 3), (8, 6)]);
        let mut saving = Archive::default();
        assert_eq!(summary.serialize(&mut saving), 0);
        saving.bytes
    }

    fn legacy_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in [0x9e2a_83c1u32, 67, 0, 3, 64, 5, 96, 2, 128, 2, 128] {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        bytes.resize(128, 0);
        for value in [1u32, 2, 3, 4, 5, 6, 7, 8] {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        bytes
    }

    #[test]
    fn guid_serializes_each_dword_in_field_order() {
        let mut guid = [0x0102_0304u32, 0x0506_0708, 0x090a_0b0c, 0x0d0e_0f10];
        let expected = guid.iter().flat_map(|word| word.to_ne_bytes()).collect::<Vec<_>>();
        let mut archive = Archive::default();
        assert_eq!(unsafe {
            super::serialize_guid((&raw mut archive).cast(), guid.as_mut_ptr().cast(), &services(ptr::null()))
        }, 0);
        assert_eq!(archive.bytes, expected);
    }

    #[test]
    fn guid_load_initializes_uninitialized_words() {
        let expected = [0x0102_0304u32, 0x0506_0708, 0x090a_0b0c, 0x0d0e_0f10];
        let mut archive = Archive {
            loading: true,
            bytes: expected.iter().flat_map(|word| word.to_ne_bytes()).collect(),
            ..Archive::default()
        };
        let mut guid = MaybeUninit::<[u32; 4]>::uninit();
        assert_eq!(unsafe {
            super::serialize_guid((&raw mut archive).cast(), guid.as_mut_ptr().cast(), &services(ptr::null()))
        }, 0);
        assert_eq!(unsafe { guid.assume_init() }, expected);
        assert_eq!(archive.position, 16);
    }

    #[test]
    fn generation_load_initializes_uninitialized_fields_in_order() {
        let mut generation = MaybeUninit::<FGenerationInfo>::uninit();
        let mut archive = Archive {
            loading: true,
            bytes: [37i32, 19].into_iter().flat_map(i32::to_ne_bytes).collect(),
            ..Archive::default()
        };
        assert_eq!(unsafe {
            serialize_generation((&raw mut archive).cast(), generation.as_mut_ptr().cast(), &services(ptr::null()))
        }, 0);
        let generation = unsafe { generation.assume_init() };
        assert_eq!((generation.export_count, generation.name_count), (37, 19));
        assert_eq!(archive.position, 8);
    }

    #[test]
    fn modern_summary_round_trips_generations_at_version_68() {
        // The high licensee-version bits must not affect the version-68 cutoff.
        let mut source = Summary::new((7 << 16) | 68, &[(5, 3), (8, 6)]);
        let mut saving = Archive::default();
        assert_eq!(source.serialize(&mut saving), 0);
        let expected_words = [
            0x9e2a_83c1, (7 << 16) | 68, 0x1234_5678, 3, 64, 5, 96, 2, 128,
            11, 22, 33, 44, 2, 5, 3, 8, 6,
        ];
        assert_eq!(saving.bytes, expected_words.into_iter().flat_map(u32::to_ne_bytes).collect::<Vec<_>>());

        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive { loading: true, bytes: saving.bytes, ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 0);
        assert_eq!(loading.position, 72);
        assert_eq!(loaded.value.tag, source.value.tag);
        assert_eq!(loaded.value.file_version, source.value.file_version);
        assert_eq!(loaded.value.package_flags, source.value.package_flags);
        assert_eq!(loaded.value.guid, source.value.guid);
        assert_eq!(loaded.generations(), source.generations());
    }

    #[test]
    fn modern_load_publishes_count_before_reentrant_generation_callbacks() {
        let mut loaded = Summary::new(0, &[]);
        let services = loaded.services();
        let summary = &raw mut loaded.value;
        let mut loading = Archive {
            loading: true,
            bytes: modern_bytes(),
            observe_summary: summary,
            ..Archive::default()
        };
        // SAFETY: The observer and owner use the same raw summary pointer,
        // without creating a competing whole-summary borrow during reentry.
        assert_eq!(unsafe {
            serialize((&raw mut loading).cast(), summary.cast(), &services)
        }, 0);
        assert_eq!(loading.observed_generation_count, Some(2));
        assert_eq!(loaded.value.package_flags, 0xfeed_beef);
        assert_eq!(loaded.generations(), [(5, 3), (8, 6)]);
    }

    #[test]
    fn modern_allocation_failure_preserves_old_generation_storage() {
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let old_data = loaded.value.generations.data;
        // SAFETY: This fixture owns the context and no callback is active.
        unsafe { (*loaded.allocator).fail_next = true };
        let mut loading = Archive { loading: true, bytes: modern_bytes(), ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 17);
        assert_eq!(loading.position, 56);
        assert_eq!(loaded.value.guid, [11, 22, 33, 44]);
        assert_eq!(loaded.value.generations.data, old_data);
        assert_eq!(loaded.generations(), [(99, 88)]);
    }

    #[test]
    fn modern_partial_generation_error_preserves_initialized_fields_and_full_count() {
        let mut bytes = modern_bytes();
        bytes.truncate(68);
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive { loading: true, bytes, ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 9);
        assert_eq!(loading.position, 68);
        assert_eq!(loaded.value.generations.num, 2);
        // Only these fields have been initialized. In particular, never read
        // the second record wholesale after its NameCount failed to load.
        let data = loaded.value.generations.data.cast::<FGenerationInfo>();
        unsafe {
            assert_eq!(addr_of!((*data).export_count).read(), 5);
            assert_eq!(addr_of!((*data).name_count).read(), 3);
            assert_eq!(addr_of!((*data.add(1)).export_count).read(), 8);
        }
    }

    #[test]
    fn modern_guid_or_count_error_does_not_replace_generations() {
        for length in [40, 52] {
            let mut bytes = modern_bytes();
            bytes.truncate(length);
            let mut loaded = Summary::new(0, &[(99, 88)]);
            let old_data = loaded.value.generations.data;
            let mut loading = Archive { loading: true, bytes, ..Archive::default() };
            assert_eq!(loaded.serialize(&mut loading), 9, "truncated at {length}");
            assert_eq!(loading.position, length);
            assert_eq!(loaded.value.generations.data, old_data);
            assert_eq!(loaded.generations(), [(99, 88)]);
        }
    }

    #[test]
    fn modern_invalid_generation_count_propagates_allocator_error_without_replacement() {
        let mut bytes = modern_bytes();
        bytes[52..56].copy_from_slice(&(-1i32).to_ne_bytes());
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive { loading: true, bytes, ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 17);
        assert_eq!(loading.position, 56);
        assert_eq!(loaded.generations(), [(99, 88)]);
    }

    #[test]
    fn modern_zero_count_replaces_existing_generations_with_empty_storage() {
        let mut source = Summary::new(68, &[]);
        let mut saving = Archive::default();
        assert_eq!(source.serialize(&mut saving), 0);
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive { loading: true, bytes: saving.bytes, ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 0);
        assert_eq!(loading.position, 56);
        assert!(loaded.generations().is_empty());
    }

    #[test]
    fn legacy_summary_reads_heritage_guids_then_restores_header_position() {
        let mut loaded = Summary::new(0, &[(99, 88), (77, 66)]);
        let mut loading = Archive { loading: true, bytes: legacy_bytes(), ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 0);
        assert_eq!(loading.position, 44);
        assert_eq!(loaded.value.guid, [5, 6, 7, 8]);
        assert_eq!(loaded.generations(), [(5, 3)]);
    }

    #[test]
    fn legacy_summary_saving_uses_zero_heritage_count_and_offset() {
        let mut summary = Summary::new((9 << 16) | 67, &[(99, 88)]);
        let mut saving = Archive::default();
        assert_eq!(summary.serialize(&mut saving), 0);
        assert_eq!(saving.position, 44);
        assert_eq!(&saving.bytes[36..], &[0; 8]);
        assert_eq!(summary.generations(), [(99, 88)]);
    }

    #[test]
    fn legacy_without_heritage_keeps_guid_and_builds_current_generation() {
        let mut saving = Archive::default();
        assert_eq!(Summary::new(67, &[]).serialize(&mut saving), 0);
        let mut loaded = Summary::new(0, &[(99, 88)]);
        loaded.value.guid = [1, 2, 3, 4];
        let mut loading = Archive { loading: true, bytes: saving.bytes, ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 0);
        assert_eq!(loading.position, 44);
        assert_eq!(loaded.value.guid, [1, 2, 3, 4]);
        assert_eq!(loaded.generations(), [(5, 3)]);
    }

    #[test]
    fn legacy_heritage_read_error_returns_without_restoring_or_replacing_generations() {
        let mut bytes = legacy_bytes();
        bytes.truncate(144);
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive { loading: true, bytes, ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 9);
        assert_eq!(loading.position, 144);
        assert_eq!(loaded.value.guid, [1, 2, 3, 4]);
        assert_eq!(loaded.generations(), [(99, 88)]);
    }

    #[test]
    fn legacy_seek_errors_stop_before_generation_replacement() {
        for (fail_seek, expected_position) in [(128, 44), (44, 160)] {
            let mut loaded = Summary::new(0, &[(99, 88)]);
            let mut loading = Archive {
                loading: true,
                bytes: legacy_bytes(),
                fail_seek: Some(fail_seek),
                ..Archive::default()
            };
            assert_eq!(loaded.serialize(&mut loading), 19);
            assert_eq!(loading.position, expected_position);
            assert_eq!(loaded.generations(), [(99, 88)]);
        }
    }

    #[test]
    fn legacy_tell_error_stops_before_seek_guid_reads_or_generation_replacement() {
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive {
            loading: true,
            bytes: legacy_bytes(),
            tell_status: 23,
            ..Archive::default()
        };
        assert_eq!(loaded.serialize(&mut loading), 23);
        assert_eq!(loading.position, 44);
        assert_eq!(loading.seek_count, 0);
        assert_eq!(loaded.value.guid, [11, 22, 33, 44]);
        assert_eq!(loaded.generations(), [(99, 88)]);
    }

    #[test]
    fn legacy_negative_tell_position_is_passed_to_restore_seek_not_used_as_status() {
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let mut loading = Archive {
            loading: true,
            bytes: legacy_bytes(),
            tell_position: Some(-7),
            ..Archive::default()
        };
        assert_eq!(loaded.serialize(&mut loading), 19);
        assert_eq!(loading.position, 160);
        assert_eq!(loading.seek_count, 2);
        assert_eq!(loaded.value.guid, [5, 6, 7, 8]);
        assert_eq!(loaded.generations(), [(99, 88)]);
    }

    #[test]
    fn legacy_allocation_error_leaves_empty_count_and_live_native_storage() {
        let mut loaded = Summary::new(0, &[(99, 88)]);
        let old_data = loaded.value.generations.data;
        // SAFETY: This fixture owns the context and no callback is active.
        unsafe { (*loaded.allocator).fail_next = true };
        let mut loading = Archive { loading: true, bytes: legacy_bytes(), ..Archive::default() };
        assert_eq!(loaded.serialize(&mut loading), 17);
        assert_eq!(loading.position, 44);
        assert_eq!(loaded.value.guid, [5, 6, 7, 8]);
        assert_eq!(loaded.value.generations.num, 0);
        assert_eq!(loaded.value.generations.data, old_data);
    }
}
