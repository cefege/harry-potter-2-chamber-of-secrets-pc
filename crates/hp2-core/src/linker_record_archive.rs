//! `FObjectExport` and `FObjectImport` archive field ordering from `UnLinker.h`.

use std::ffi::c_void;

use hp2_abi::engine::generated::{FObjectExport, FObjectImport, UObject};
use std::ptr::{addr_of, addr_of_mut};

/// Only typed archive operators remain native; record layout and mutations use
/// the compiler-probed canonical mirrors.
#[repr(C)]
pub struct Hp2LinkerRecordArchiveServices {
    pub archive_is_loading: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    pub archive_compact_index: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub archive_int: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub archive_dword: unsafe extern "C" fn(ar: *mut c_void, value: *mut u32) -> i32,
    pub archive_name: unsafe extern "C" fn(ar: *mut c_void, name: *mut c_void) -> i32,
}

/// Generic direct-reader operations for package import/export fields.
pub trait LinkerRecordInput {
    /// Error preserved from the underlying archive.
    type Error;
    /// Reads a compact index.
    fn compact_index(&mut self) -> Result<i32, Self::Error>;
    /// Reads an FName/local name index; direct packages use compact indices.
    fn name_index(&mut self) -> Result<i32, Self::Error> { self.compact_index() }
    /// Reads a signed archive integer.
    fn int(&mut self) -> Result<i32, Self::Error>;
    /// Reads an archive DWORD.
    fn dword(&mut self) -> Result<u32, Self::Error>;
}

/// Fully decoded serialized export fields, excluding runtime-only object pointers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportFields {
    pub class_index: i32,
    pub super_index: i32,
    pub package_index: i32,
    pub object_name: i32,
    pub object_flags: u32,
    pub serial_size: i32,
    pub serial_offset: i32,
}

/// Fully decoded serialized import fields, excluding runtime-only linker pointers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportFields {
    pub class_package: i32,
    pub class_name: i32,
    pub package_index: i32,
    pub object_name: i32,
}

/// Serialized export fields traversed in their package order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportField { ClassIndex, SuperIndex, PackageIndex, ObjectName, ObjectFlags, SerialSize, SerialOffset }

/// Serialized import fields traversed in their package order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportField { ClassPackage, ClassName, PackageIndex, ObjectName }

/// Operations on one live export field at a time.
pub trait ExportRecordOps {
    type Error;
    fn compact(&mut self, field: ExportField) -> Result<(), Self::Error>;
    fn int(&mut self, field: ExportField) -> Result<(), Self::Error>;
    fn name(&mut self, field: ExportField) -> Result<(), Self::Error>;
    fn dword(&mut self, field: ExportField) -> Result<(), Self::Error>;
    fn serial_size(&self) -> i32;
}

/// Operations on one live import field at a time.
pub trait ImportRecordOps {
    type Error;
    fn compact(&mut self, field: ImportField) -> Result<(), Self::Error>;
    fn int(&mut self, field: ImportField) -> Result<(), Self::Error>;
    fn name(&mut self, field: ImportField) -> Result<(), Self::Error>;
}

/// Shared export traversal. Each operation completes before the next field is
/// queried, preserving archive reentry and partially-loaded record state.
pub fn walk_export<O: ExportRecordOps>(ops: &mut O) -> Result<(), O::Error> {
    ops.compact(ExportField::ClassIndex)?;
    ops.compact(ExportField::SuperIndex)?;
    ops.int(ExportField::PackageIndex)?;
    ops.name(ExportField::ObjectName)?;
    ops.dword(ExportField::ObjectFlags)?;
    ops.compact(ExportField::SerialSize)?;
    if ops.serial_size() != 0 { ops.compact(ExportField::SerialOffset)?; }
    Ok(())
}

/// Shared import traversal in package field order.
pub fn walk_import<O: ImportRecordOps>(ops: &mut O) -> Result<(), O::Error> {
    ops.name(ImportField::ClassPackage)?;
    ops.name(ImportField::ClassName)?;
    ops.int(ImportField::PackageIndex)?;
    ops.name(ImportField::ObjectName)
}

struct ReadExport<'a, R> { archive: &'a mut R, fields: ExportFields }
impl<R: LinkerRecordInput> ExportRecordOps for ReadExport<'_, R> {
    type Error = R::Error;
    fn compact(&mut self, field: ExportField) -> Result<(), Self::Error> {
        let value = self.archive.compact_index()?;
        match field {
            ExportField::ClassIndex => self.fields.class_index = value,
            ExportField::SuperIndex => self.fields.super_index = value,
            ExportField::SerialSize => self.fields.serial_size = value,
            ExportField::SerialOffset => self.fields.serial_offset = value,
            _ => unreachable!("noncompact export field"),
        }
        Ok(())
    }
    fn int(&mut self, field: ExportField) -> Result<(), Self::Error> {
        if field != ExportField::PackageIndex { unreachable!("noninteger export field"); }
        self.fields.package_index = self.archive.int()?;
        Ok(())
    }
    fn name(&mut self, field: ExportField) -> Result<(), Self::Error> {
        if field != ExportField::ObjectName { unreachable!("non-name export field"); }
        self.fields.object_name = self.archive.name_index()?;
        Ok(())
    }
    fn dword(&mut self, field: ExportField) -> Result<(), Self::Error> {
        if field != ExportField::ObjectFlags { unreachable!("non-DWORD export field"); }
        self.fields.object_flags = self.archive.dword()?;
        Ok(())
    }
    fn serial_size(&self) -> i32 { self.fields.serial_size }
}

struct ReadImport<'a, R> { archive: &'a mut R, fields: ImportFields }
impl<R: LinkerRecordInput> ImportRecordOps for ReadImport<'_, R> {
    type Error = R::Error;
    fn compact(&mut self, _: ImportField) -> Result<(), Self::Error> { unreachable!("imports have no compact-only fields") }
    fn int(&mut self, field: ImportField) -> Result<(), Self::Error> {
        if field != ImportField::PackageIndex { unreachable!("noninteger import field"); }
        self.fields.package_index = self.archive.int()?;
        Ok(())
    }
    fn name(&mut self, field: ImportField) -> Result<(), Self::Error> {
        let value = self.archive.name_index()?;
        match field {
            ImportField::ClassPackage => self.fields.class_package = value,
            ImportField::ClassName => self.fields.class_name = value,
            ImportField::ObjectName => self.fields.object_name = value,
            ImportField::PackageIndex => unreachable!("package index is not a name"),
        }
        Ok(())
    }
}

/// Reads export fields using the shared ordered traversal.
pub fn read_export_fields<R: LinkerRecordInput>(archive: &mut R) -> Result<ExportFields, R::Error> {
    let mut ops = ReadExport { archive, fields: ExportFields { class_index: 0, super_index: 0, package_index: 0, object_name: 0, object_flags: 0, serial_size: 0, serial_offset: 0 } };
    walk_export(&mut ops)?;
    Ok(ops.fields)
}

/// Reads import fields using the shared ordered traversal.
pub fn read_import_fields<R: LinkerRecordInput>(archive: &mut R) -> Result<ImportFields, R::Error> {
    let mut ops = ReadImport { archive, fields: ImportFields { class_package: 0, class_name: 0, package_index: 0, object_name: 0 } };
    walk_import(&mut ops)?;
    Ok(ops.fields)
}

/// Generic direct-writer operations for package linker fields.
pub trait LinkerRecordOutput {
    type Error;
    fn compact_index(&mut self, value: i32) -> Result<(), Self::Error>;
    fn name_index(&mut self, value: i32) -> Result<(), Self::Error> { self.compact_index(value) }
    fn int(&mut self, value: i32) -> Result<(), Self::Error>;
    fn dword(&mut self, value: u32) -> Result<(), Self::Error>;
}

struct WriteExport<'a, W> { archive: &'a mut W, fields: &'a ExportFields }
impl<W: LinkerRecordOutput> ExportRecordOps for WriteExport<'_, W> {
    type Error = W::Error;
    fn compact(&mut self, field: ExportField) -> Result<(), Self::Error> {
        let value = match field {
            ExportField::ClassIndex => self.fields.class_index,
            ExportField::SuperIndex => self.fields.super_index,
            ExportField::SerialSize => self.fields.serial_size,
            ExportField::SerialOffset => self.fields.serial_offset,
            _ => unreachable!("noncompact export field"),
        };
        self.archive.compact_index(value)
    }
    fn int(&mut self, field: ExportField) -> Result<(), Self::Error> {
        if field != ExportField::PackageIndex { unreachable!("noninteger export field"); }
        self.archive.int(self.fields.package_index)
    }
    fn name(&mut self, field: ExportField) -> Result<(), Self::Error> {
        if field != ExportField::ObjectName { unreachable!("non-name export field"); }
        self.archive.name_index(self.fields.object_name)
    }
    fn dword(&mut self, field: ExportField) -> Result<(), Self::Error> {
        if field != ExportField::ObjectFlags { unreachable!("non-DWORD export field"); }
        self.archive.dword(self.fields.object_flags)
    }
    fn serial_size(&self) -> i32 { self.fields.serial_size }
}

struct WriteImport<'a, W> { archive: &'a mut W, fields: &'a ImportFields }
impl<W: LinkerRecordOutput> ImportRecordOps for WriteImport<'_, W> {
    type Error = W::Error;
    fn compact(&mut self, _: ImportField) -> Result<(), Self::Error> { unreachable!("imports have no compact-only fields") }
    fn int(&mut self, field: ImportField) -> Result<(), Self::Error> {
        if field != ImportField::PackageIndex { unreachable!("noninteger import field"); }
        self.archive.int(self.fields.package_index)
    }
    fn name(&mut self, field: ImportField) -> Result<(), Self::Error> {
        let value = match field {
            ImportField::ClassPackage => self.fields.class_package,
            ImportField::ClassName => self.fields.class_name,
            ImportField::ObjectName => self.fields.object_name,
            ImportField::PackageIndex => unreachable!("package index is not a name"),
        };
        self.archive.name_index(value)
    }
}

/// Writes export fields using the shared ordered traversal.
pub fn write_export_fields<W: LinkerRecordOutput>(archive: &mut W, fields: &ExportFields) -> Result<(), W::Error> {
    walk_export(&mut WriteExport { archive, fields })
}

/// Writes import fields using the shared ordered traversal.
pub fn write_import_fields<W: LinkerRecordOutput>(archive: &mut W, fields: &ImportFields) -> Result<(), W::Error> {
    walk_import(&mut WriteImport { archive, fields })
}

/// Initializes only the two fields written by the default export constructor.
///
/// # Safety
/// `record` is writable, correctly aligned storage for an `FObjectExport`; it
/// need not contain an initialized Rust value. All other bytes remain untouched.
pub unsafe fn initialize_export_default(record: *mut c_void) -> i32 {
    if record.is_null() {
        return 1;
    }
    let record = record.cast::<FObjectExport>();
    unsafe {
        addr_of_mut!((*record)._object).write(std::ptr::null_mut());
        addr_of_mut!((*record)._i_hash_next).write(-1);
    }
    0
}

/// Initializes the object-taking export constructor, leaving `_iHashNext` and
/// padding unspecified exactly as the native constructor did.
///
/// # Safety
/// `record` is writable export storage. A nonnull `object` is a live `UObject`.
/// `load_mask` and `name_none` are the engine's `RF_Load` and `NAME_None`.
pub unsafe fn initialize_export(
    record: *mut c_void,
    object: *mut c_void,
    load_mask: u32,
    name_none: i32,
) -> i32 {
    if record.is_null() {
        return 1;
    }
    let record = record.cast::<FObjectExport>();
    let object_fields = object.cast::<UObject>();
    unsafe {
        addr_of_mut!((*record).class_index).write(0);
        addr_of_mut!((*record).super_index).write(0);
        addr_of_mut!((*record).package_index).write(0);
        addr_of_mut!((*record).object_name).write(if object.is_null() {
            name_none.to_ne_bytes()
        } else {
            addr_of!((*object_fields).name).read()
        });
        addr_of_mut!((*record).object_flags).write(if object.is_null() {
            0
        } else {
            addr_of!((*object_fields).object_flags).read() & load_mask
        });
        addr_of_mut!((*record).serial_size).write(0);
        addr_of_mut!((*record).serial_offset).write(0);
        addr_of_mut!((*record)._object).write(object);
    }
    0
}

/// Initializes an import's data before the caller performs the existing
/// `ImportConstructed` package-policy operation.
///
/// # Safety
/// `record` is writable import storage; `object`, its class and class outer
/// are live canonical objects, as required by the native constructor.
pub unsafe fn initialize_import(record: *mut c_void, object: *mut c_void) -> i32 {
    if record.is_null() || object.is_null() {
        return 1;
    }
    let record = record.cast::<FObjectImport>();
    let object_fields = object.cast::<UObject>();
    unsafe {
        let class = addr_of!((*object_fields).class).read().cast::<UObject>();
        let outer = addr_of!((*class).outer).read().cast::<UObject>();
        addr_of_mut!((*record).class_package).write(addr_of!((*outer).name).read());
        addr_of_mut!((*record).class_name).write(addr_of!((*class).name).read());
        addr_of_mut!((*record).package_index).write(0);
        addr_of_mut!((*record).object_name).write(addr_of!((*object_fields).name).read());
        addr_of_mut!((*record).x_object).write(object);
        addr_of_mut!((*record).source_linker).write(std::ptr::null_mut());
        addr_of_mut!((*record).source_index).write(-1);
    }
    0
}

/// Serializes one export record, including `SerialOffset` only when the
/// resulting `SerialSize` is nonzero (on load, this uses the decoded size).
///
/// # Safety
/// `ar` is a live archive and `record` is writable canonical export storage.
/// Save input fields must be initialized; successful loading leaves must fully
/// initialize their field. No record reference is created or held over a leaf,
/// allowing native archive reentry and partially initialized load records.
struct LegacyExportOps<'a> { ar: *mut c_void, record: *mut FObjectExport, svc: &'a Hp2LinkerRecordArchiveServices }
impl ExportRecordOps for LegacyExportOps<'_> {
    type Error = i32;
    fn compact(&mut self, field: ExportField) -> Result<(), i32> {
        let value = unsafe {
            match field {
                ExportField::ClassIndex => addr_of_mut!((*self.record).class_index),
                ExportField::SuperIndex => addr_of_mut!((*self.record).super_index),
                ExportField::SerialSize => addr_of_mut!((*self.record).serial_size),
                ExportField::SerialOffset => addr_of_mut!((*self.record).serial_offset),
                _ => unreachable!("noncompact export field"),
            }
        };
        let status = unsafe { (self.svc.archive_compact_index)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn int(&mut self, _: ExportField) -> Result<(), i32> {
        let value = unsafe { addr_of_mut!((*self.record).package_index) };
        let status = unsafe { (self.svc.archive_int)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn name(&mut self, _: ExportField) -> Result<(), i32> {
        let value = unsafe { addr_of_mut!((*self.record).object_name) }.cast();
        let status = unsafe { (self.svc.archive_name)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn dword(&mut self, _: ExportField) -> Result<(), i32> {
        let value = unsafe { addr_of_mut!((*self.record).object_flags) };
        let status = unsafe { (self.svc.archive_dword)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn serial_size(&self) -> i32 { unsafe { addr_of!((*self.record).serial_size).read() } }
}

struct LegacyImportOps<'a> { ar: *mut c_void, record: *mut FObjectImport, svc: &'a Hp2LinkerRecordArchiveServices }
impl ImportRecordOps for LegacyImportOps<'_> {
    type Error = i32;
    fn compact(&mut self, _: ImportField) -> Result<(), i32> { unreachable!("imports have no compact-only fields") }
    fn int(&mut self, _: ImportField) -> Result<(), i32> {
        let value = unsafe { addr_of_mut!((*self.record).package_index) };
        let status = unsafe { (self.svc.archive_int)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn name(&mut self, field: ImportField) -> Result<(), i32> {
        let value = unsafe {
            match field {
                ImportField::ClassPackage => addr_of_mut!((*self.record).class_package),
                ImportField::ClassName => addr_of_mut!((*self.record).class_name),
                ImportField::ObjectName => addr_of_mut!((*self.record).object_name),
                ImportField::PackageIndex => unreachable!("package index is not a name"),
            }
        }.cast();
        let status = unsafe { (self.svc.archive_name)(self.ar, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}

/// Serializes one export using the shared live-field traversal.
///
/// # Safety
/// `ar` and `record` remain valid throughout serialization; archive errors are
/// returned unchanged and no field reference crosses a callback.
pub unsafe fn serialize_export(ar: *mut c_void, record: *mut c_void, svc: &Hp2LinkerRecordArchiveServices) -> i32 {
    if ar.is_null() || record.is_null() { return 1; }
    let mut ops = LegacyExportOps { ar, record: record.cast(), svc };
    walk_export(&mut ops).err().unwrap_or(0)
}

/// Serializes an import using shared ordering and restores runtime fields after
/// successful loading, matching the original C++ operator.
///
/// # Safety
/// `ar` and `record` remain valid throughout serialization; archive errors are
/// returned unchanged and no field reference crosses a callback.
pub unsafe fn serialize_import(ar: *mut c_void, record: *mut c_void, svc: &Hp2LinkerRecordArchiveServices) -> i32 {
    if ar.is_null() || record.is_null() { return 1; }
    let mut ops = LegacyImportOps { ar, record: record.cast(), svc };
    if let Err(status) = walk_import(&mut ops) { return status; }
    if unsafe { (svc.archive_is_loading)(ar) } != 0 {
        let record = record.cast::<FObjectImport>();
        unsafe {
            addr_of_mut!((*record).source_index).write(-1);
            addr_of_mut!((*record).x_object).write(std::ptr::null_mut());
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    type Export = FObjectExport;
    type Import = FObjectImport;

    struct Archive {
        loading: bool,
        compact_input: [i32; 4],
        compact_input_count: usize,
        compact_read: usize,
        compact_output: [i32; 4],
        compact_written: usize,
        int_input: i32,
        int_output: i32,
        dword_input: u32,
        dword_output: u32,
        name_input: [i32; 3],
        name_read: usize,
        name_output: [i32; 3],
        name_written: usize,
    }

    unsafe extern "C" fn is_loading(ar: *mut c_void) -> i32 {
        unsafe { (*ar.cast::<Archive>()).loading as i32 }
    }

    unsafe extern "C" fn compact_index(ar: *mut c_void, value: *mut i32) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        if ar.loading {
            if ar.compact_read >= ar.compact_input_count {
                return 9;
            }
            unsafe { *value = ar.compact_input[ar.compact_read] };
            ar.compact_read += 1;
        } else if ar.compact_written < ar.compact_output.len() {
            ar.compact_output[ar.compact_written] = unsafe { *value };
            ar.compact_written += 1;
        } else {
            return 8;
        }
        0
    }

    unsafe extern "C" fn archive_int(ar: *mut c_void, value: *mut i32) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        if ar.loading {
            unsafe { *value = ar.int_input };
        } else {
            ar.int_output = unsafe { *value };
        }
        0
    }

    unsafe extern "C" fn archive_dword(ar: *mut c_void, value: *mut u32) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        if ar.loading {
            unsafe { *value = ar.dword_input };
        } else {
            ar.dword_output = unsafe { *value };
        }
        0
    }

    unsafe extern "C" fn archive_name(ar: *mut c_void, name: *mut c_void) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        let name = name.cast::<i32>();
        if ar.loading {
            if ar.name_read >= ar.name_input.len() {
                return 9;
            }
            unsafe { name.write(ar.name_input[ar.name_read]) };
            ar.name_read += 1;
        } else if ar.name_written < ar.name_output.len() {
            ar.name_output[ar.name_written] = unsafe { name.read() };
            ar.name_written += 1;
        } else {
            return 8;
        }
        0
    }

    const SERVICES: Hp2LinkerRecordArchiveServices = Hp2LinkerRecordArchiveServices {
        archive_is_loading: is_loading,
        archive_compact_index: compact_index,
        archive_int,
        archive_dword,
        archive_name,
    };

    fn archive(loading: bool) -> Archive {
        Archive {
            loading,
            compact_input: [0; 4],
            compact_input_count: 0,
            compact_read: 0,
            compact_output: [0; 4],
            compact_written: 0,
            int_input: 0,
            int_output: 0,
            dword_input: 0,
            dword_output: 0,
            name_input: [0; 3],
            name_read: 0,
            name_output: [0; 3],
            name_written: 0,
        }
    }

    #[test]
    fn export_save_omits_offset_for_zero_size() {
        let mut ar = archive(false);
        let mut export: Export = unsafe { std::mem::zeroed() };
        export.class_index = 2;
        export.super_index = -1;
        export.package_index = 5;
        export.object_name = 23i32.to_ne_bytes();
        export.object_flags = 0xaabb_ccddu32;
        export.serial_size = 0;
        export.serial_offset = 91;
        let status = unsafe {
            serialize_export(
                (&mut ar as *mut Archive).cast(),
                (&mut export as *mut Export).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(&ar.compact_output[..3], &[2, -1, 0]);
        assert_eq!(ar.compact_written, 3);
        assert_eq!(ar.int_output, 5);
        assert_eq!(ar.name_output[0], 23);
        assert_eq!(ar.dword_output, 0xaabb_ccddu32);
    }

    #[test]
    fn export_load_reads_offset_after_nonzero_decoded_size() {
        let mut ar = archive(true);
        ar.compact_input = [2, -1, 41, 88];
        ar.compact_input_count = 4;
        ar.int_input = -3;
        ar.dword_input = 0x1234;
        ar.name_input[0] = 7;
        let mut export: Export = unsafe { std::mem::zeroed() };
        let status = unsafe {
            serialize_export(
                (&mut ar as *mut Archive).cast(),
                (&mut export as *mut Export).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(export.class_index, 2);
        assert_eq!(export.super_index, -1);
        assert_eq!(export.package_index, -3);
        assert_eq!(i32::from_ne_bytes(export.object_name), 7);
        assert_eq!(export.object_flags, 0x1234);
        assert_eq!(export.serial_size, 41);
        assert_eq!(export.serial_offset, 88);
        assert_eq!(ar.compact_read, 4);
    }

    #[test]
    fn import_load_resets_runtime_only_fields_after_serializing() {
        let mut ar = archive(true);
        ar.int_input = -6;
        ar.name_input = [10, 11, 12];
        let mut import: Import = unsafe { std::mem::zeroed() };
        import.class_package = 1i32.to_ne_bytes();
        import.class_name = 2i32.to_ne_bytes();
        import.object_name = 3i32.to_ne_bytes();
        import.x_object = 1usize as *mut c_void;
        import.source_linker = 2usize as *mut c_void;
        import.source_index = 88;
        let status = unsafe {
            serialize_import(
                (&mut ar as *mut Archive).cast(),
                (&mut import as *mut Import).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(i32::from_ne_bytes(import.class_package), 10);
        assert_eq!(i32::from_ne_bytes(import.class_name), 11);
        assert_eq!(import.package_index, -6);
        assert_eq!(i32::from_ne_bytes(import.object_name), 12);
        assert!(import.x_object.is_null());
        assert_eq!(import.source_index, -1);
        assert_eq!(import.source_linker, 2usize as *mut c_void);
    }

    #[test]
    fn export_constructors_preserve_unspecified_bytes() {
        let mut storage = std::mem::MaybeUninit::<Export>::uninit();
        let record = storage.as_mut_ptr();
        unsafe {
            record
                .cast::<u8>()
                .write_bytes(0xa5, std::mem::size_of::<Export>());
            assert_eq!(initialize_export_default(record.cast()), 0);
            assert!(addr_of!((*record)._object).read().is_null());
            assert_eq!(addr_of!((*record)._i_hash_next).read(), -1);
            let object = std::mem::offset_of!(Export, _object);
            let hash = std::mem::offset_of!(Export, _i_hash_next);
            for offset in 0..std::mem::size_of::<Export>() {
                if !(object..object + std::mem::size_of::<*mut c_void>()).contains(&offset)
                    && !(hash..hash + std::mem::size_of::<i32>()).contains(&offset)
                {
                    assert_eq!(record.cast::<u8>().add(offset).read(), 0xa5);
                }
            }
            addr_of_mut!((*record)._i_hash_next).write(71);
            let mut object: UObject = std::mem::zeroed();
            object.name = 29i32.to_ne_bytes();
            object.object_flags = 0x1234;
            let object = (&mut object as *mut UObject).cast();
            assert_eq!(initialize_export(record.cast(), object, 0x1030, 0), 0);
            assert_eq!(addr_of!((*record).object_name).read(), 29i32.to_ne_bytes());
            assert_eq!(addr_of!((*record).object_flags).read(), 0x1030);
            assert_eq!(addr_of!((*record)._object).read(), object);
            assert_eq!(addr_of!((*record)._i_hash_next).read(), 71);
            assert_eq!(
                initialize_export(record.cast(), std::ptr::null_mut(), 0x1030, 0),
                0
            );
            assert_eq!(addr_of!((*record).object_name).read(), 0i32.to_ne_bytes());
            assert_eq!(addr_of!((*record).object_flags).read(), 0);
            assert_eq!(addr_of!((*record)._i_hash_next).read(), 71);
        }
    }

    #[test]
    fn import_constructor_reads_canonical_object_class_and_outer() {
        unsafe {
            let mut package: UObject = std::mem::zeroed();
            package.name = 10i32.to_ne_bytes();
            let mut class: UObject = std::mem::zeroed();
            class.name = 11i32.to_ne_bytes();
            class.outer = (&mut package as *mut UObject).cast();
            let mut object: UObject = std::mem::zeroed();
            object.name = 12i32.to_ne_bytes();
            object.class = (&mut class as *mut UObject).cast();
            let object = (&mut object as *mut UObject).cast();
            let mut storage = std::mem::MaybeUninit::<Import>::uninit();
            let record = storage.as_mut_ptr();
            assert_eq!(initialize_import(record.cast(), object), 0);
            assert_eq!(
                addr_of!((*record).class_package).read(),
                10i32.to_ne_bytes()
            );
            assert_eq!(addr_of!((*record).class_name).read(), 11i32.to_ne_bytes());
            assert_eq!(addr_of!((*record).object_name).read(), 12i32.to_ne_bytes());
            assert_eq!(addr_of!((*record).package_index).read(), 0);
            assert_eq!(addr_of!((*record).x_object).read(), object);
            assert!(addr_of!((*record).source_linker).read().is_null());
            assert_eq!(addr_of!((*record).source_index).read(), -1);
        }
    }

    #[test]
    fn export_error_preserves_earlier_mutations_without_reading_failed_size() {
        let mut ar = archive(true);
        ar.compact_input = [2, -1, 0, 0];
        ar.compact_input_count = 2;
        ar.int_input = -3;
        ar.dword_input = 0x1234;
        ar.name_input[0] = 7;
        let mut storage = std::mem::MaybeUninit::<Export>::uninit();
        let record = storage.as_mut_ptr();
        unsafe {
            addr_of_mut!((*record).serial_offset).write(91);
            assert_eq!(
                serialize_export((&mut ar as *mut Archive).cast(), record.cast(), &SERVICES),
                9
            );
            assert_eq!(addr_of!((*record).class_index).read(), 2);
            assert_eq!(addr_of!((*record).super_index).read(), -1);
            assert_eq!(addr_of!((*record).package_index).read(), -3);
            assert_eq!(addr_of!((*record).object_name).read(), 7i32.to_ne_bytes());
            assert_eq!(addr_of!((*record).object_flags).read(), 0x1234);
            assert_eq!(addr_of!((*record).serial_offset).read(), 91);
        }
    }

    #[test]
    fn import_error_keeps_runtime_fields_and_prior_name_mutations() {
        let mut ar = archive(true);
        ar.name_input = [10, 11, 12];
        ar.name_read = 1;
        ar.int_input = -6;
        let mut import: Import = unsafe { std::mem::zeroed() };
        import.object_name = 3i32.to_ne_bytes();
        import.x_object = 1usize as *mut c_void;
        import.source_linker = 2usize as *mut c_void;
        import.source_index = 88;
        let status = unsafe {
            serialize_import(
                (&mut ar as *mut Archive).cast(),
                (&mut import as *mut Import).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 9);
        assert_eq!(i32::from_ne_bytes(import.class_package), 11);
        assert_eq!(i32::from_ne_bytes(import.class_name), 12);
        assert_eq!(import.package_index, -6);
        assert_eq!(i32::from_ne_bytes(import.object_name), 3);
        assert_eq!(import.x_object, 1usize as *mut c_void);
        assert_eq!(import.source_linker, 2usize as *mut c_void);
        assert_eq!(import.source_index, 88);
    }
}
