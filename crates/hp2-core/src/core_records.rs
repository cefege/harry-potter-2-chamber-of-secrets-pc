//! Measured, field-complete raw records for native Core classes.
//!
//! The secondary-base vtable words are inert zero-initialized storage in Rust-owned
//! objects; no Rust record is given a C++ vtable or dispatched through C++.

use hp2_abi::engine::generated::{FArchive, FName, UField, ULinker, UObject, UProperty, UState, TArrayRaw};
use std::ffi::c_void;

/// ABI storage of the secondary `FExec` base subobject (measured 8-byte vptr slot).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FExecRaw { pub inert_vptr: *mut c_void }

/// ABI storage of the secondary `FOutputDevice` base subobject.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FOutputDeviceRaw { pub inert_vptr: *mut c_void }

/// Native `UPackage` record from `UnCorObj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PackageRecord {
    pub base: UObject,
    pub dll_handle: *mut c_void,
    pub attempted_bind: i32,
    pub package_flags: u32,
}

/// Native `USubsystem` record from `UnCorObj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SubsystemRecord { pub base: UObject, pub exec: FExecRaw }

/// Native `ULanguage` record from `UnCorObj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LanguageRecord { pub base: UObject, pub super_language: *mut LanguageRecord }

/// Native `UTextBuffer` record from `UnCorObj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextBufferRecord {
    pub base: UObject,
    pub output_device: FOutputDeviceRaw,
    pub pos: i32,
    pub top: i32,
    pub text: TArrayRaw,
}

/// Native `USystem` record from `UnCorObj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SystemRecord {
    pub base: SubsystemRecord,
    pub licensee_mode: i32,
    pub purge_cache_days: i32,
    pub save_path: TArrayRaw,
    pub save_slot_path: TArrayRaw,
    pub map_path: TArrayRaw,
    pub cache_path: TArrayRaw,
    pub cache_ext: TArrayRaw,
    pub paths: TArrayRaw,
    pub suppress: TArrayRaw,
}

/// Native `UFactory` record from `UFactory.h`; bitfields share `factory_bits`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FactoryRecord {
    pub base: UObject,
    pub supported_class: *mut c_void,
    pub context_class: *mut c_void,
    pub description: TArrayRaw,
    pub in_context_command: TArrayRaw,
    pub out_of_context_command: TArrayRaw,
    pub formats: TArrayRaw,
    pub factory_bits: u32,
    pub auto_priority: i32,
}

/// Native `UExporter` record from `UExporter.h`; bitfields share `exporter_bits`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ExporterRecord {
    pub base: UObject,
    pub supported_class: *mut c_void,
    pub formats: TArrayRaw,
    pub text_indent: i32,
    pub exporter_bits: u32,
}

/// Native `UTextBufferFactory` adds no fields to `UFactory`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextBufferFactoryRecord { pub base: FactoryRecord }

/// Native `UConst` record from `UnClass.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConstRecord { pub base: UField, pub value: TArrayRaw }

/// Native `UMapProperty` record from `UnType.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MapPropertyRecord { pub base: UProperty, pub key: *mut UProperty, pub value: *mut UProperty }
/// Native `TMapBase` storage header; pair elements remain owned by the map kernel.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MapStorageRaw {
    pub pairs: TArrayRaw,
    pub hash: *mut i32,
    pub hash_count: i32,
}

/// Native `UPackageMap` record from `UnCoreNet.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PackageMapRecord {
    pub base: UObject,
    pub list: TArrayRaw,
    pub linker_map: MapStorageRaw,
    pub class_field_indices: MapStorageRaw,
    pub name_indices: TArrayRaw,
    pub max_object_index: u32,
    pub max_name_index: u32,
}

const _: () = {
    assert!(size_of::<PackageMapRecord>() == 176 && align_of::<PackageMapRecord>() == 8);
    assert!(size_of::<PackageMapRecord>() == size_of::<hp2_abi::engine::generated::UPackageMap>());
    assert!(std::mem::offset_of!(PackageMapRecord, list) == 72);
    assert!(std::mem::offset_of!(PackageMapRecord, linker_map) == 88);
    assert!(std::mem::offset_of!(PackageMapRecord, class_field_indices) == 120);
    assert!(std::mem::offset_of!(PackageMapRecord, name_indices) == 152);
    assert!(std::mem::offset_of!(PackageMapRecord, max_object_index) == 168);
    assert!(std::mem::offset_of!(PackageMapRecord, max_name_index) == 172);
    assert!(std::mem::offset_of!(MapStorageRaw, pairs) == 0);
    assert!(std::mem::offset_of!(MapStorageRaw, hash) == 16);
    assert!(std::mem::offset_of!(MapStorageRaw, hash_count) == 24);
};

/// Native `ULinkerSave` record from `UnLinker.h`; second base is `FArchive`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinkerSaveRecord {
    pub base: ULinker,
    pub archive: FArchive,
    pub saver: *mut FArchive,
    pub object_indices: TArrayRaw,
    pub name_indices: TArrayRaw,
}

/// Native `UCommandlet` record; seven native bitfields share the final word.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CommandletRecord {
    pub base: UObject,
    pub help_cmd: TArrayRaw,
    pub help_one_liner: TArrayRaw,
    pub help_usage: TArrayRaw,
    pub help_web_link: TArrayRaw,
    pub help_parm: [TArrayRaw; 16],
    pub help_desc: [TArrayRaw; 16],
    pub commandlet_bits: u32,
}

/// Native `UClass` record with all fields named, based on frozen `UState` prefix.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ClassRecord {
    pub base: UState,
    pub class_flags: u32,
    pub class_unique: i32,
    pub class_guid: [u32; 4],
    pub class_within: *mut c_void,
    pub class_config_name: FName,
    pub class_crc: u32,
    pub class_reps: ClassRepArrayRaw,
    pub net_fields: TArrayRaw,
    pub dependencies: TArrayRaw,
    pub package_imports: TArrayRaw,
    pub defaults: TArrayRaw,
    pub class_constructor: *mut c_void,
    pub class_static_constructor: *mut c_void,
    pub default_prop_text: TArrayRaw,
}

/// Native `FClassRepArray` field sequence used in `UClass`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ClassRepArrayRaw { pub data: *mut c_void, pub num: i32, pub max: i32 }

// Measured from the native Core class declarations for the macOS arm64 ABI.
const _: () = {
    assert!(size_of::<PackageRecord>() == 88 && align_of::<PackageRecord>() == 8);
    assert!(size_of::<SubsystemRecord>() == 80 && align_of::<SubsystemRecord>() == 8);
    assert!(size_of::<LanguageRecord>() == 80 && align_of::<LanguageRecord>() == 8);
    assert!(size_of::<SystemRecord>() == 200 && align_of::<SystemRecord>() == 8);
    assert!(size_of::<FactoryRecord>() == 160 && align_of::<FactoryRecord>() == 8);
    assert!(size_of::<ExporterRecord>() == 104 && align_of::<ExporterRecord>() == 8);
    assert!(size_of::<TextBufferFactoryRecord>() == 160 && align_of::<TextBufferFactoryRecord>() == 8);
    assert!(size_of::<ConstRecord>() == 112 && align_of::<ConstRecord>() == 8);
    assert!(size_of::<MapPropertyRecord>() == 176 && align_of::<MapPropertyRecord>() == 8);
    assert!(size_of::<LinkerSaveRecord>() == 328 && align_of::<LinkerSaveRecord>() == 8);
    assert!(size_of::<TextBufferRecord>() == 104 && align_of::<TextBufferRecord>() == 8);
    assert!(size_of::<CommandletRecord>() == 656 && align_of::<CommandletRecord>() == 8);
    assert!(size_of::<ClassRecord>() == 2416 && align_of::<ClassRecord>() == 8);
    assert!(std::mem::offset_of!(SystemRecord, save_path) == 88);
    assert!(std::mem::offset_of!(SystemRecord, paths) == 168);
    assert!(std::mem::offset_of!(SystemRecord, suppress) == 184);
    assert!(std::mem::offset_of!(FactoryRecord, factory_bits) == 152);
    assert!(std::mem::offset_of!(FactoryRecord, auto_priority) == 156);
    assert!(std::mem::offset_of!(ExporterRecord, exporter_bits) == 100);
    assert!(std::mem::offset_of!(LinkerSaveRecord, archive) == 232);
    assert!(std::mem::offset_of!(LinkerSaveRecord, saver) == 288);
    assert!(std::mem::offset_of!(CommandletRecord, commandlet_bits) == 648);
};

