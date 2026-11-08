//! Immutable native Core startup descriptions; this is not a live class registry.

use crate::core_records;
use hp2_abi::engine::generated;
use std::alloc::Layout;

/// Raw record shape used when allocating a Core native class instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeRecordKind {
    Object,
    Field,
    Struct,
    State,
    Class,
    Function,
    Enum,
    Const,
    Property,
    ByteProperty,
    IntProperty,
    BoolProperty,
    FloatProperty,
    ObjectProperty,
    ClassProperty,
    NameProperty,
    StringProperty,
    FixedArrayProperty,
    ArrayProperty,
    MapProperty,
    StructProperty,
    Linker,
    LinkerLoad,
    LinkerSave,
    Commandlet,
    Package,
    Subsystem,
    Language,
    TextBuffer,
    System,
    Factory,
    Exporter,
    PackageMap,
    TextBufferFactory,
}

impl NativeRecordKind {
    /// Returns this Core record kind's ordinary Rust lifecycle and archive operations.
    #[must_use]
    pub const fn operations(self) -> crate::object_owner::ClassOps {
        crate::core_lifetime::class_ops(self)
    }

    /// Returns the complete record layout, including its opaque UObject prefix.
    #[must_use]
    pub fn layout(self) -> Layout {
        let layout = match self {
            Self::Object => Layout::new::<generated::UObject>(),
            Self::Field => Layout::new::<generated::UField>(),
            Self::Struct => Layout::new::<generated::UStruct>(),
            Self::State => Layout::new::<generated::UState>(),
            Self::Class => Layout::new::<core_records::ClassRecord>(),
            Self::Function => Layout::new::<generated::UFunction>(),
            Self::Enum => Layout::new::<generated::UEnum>(),
            Self::Const => Layout::new::<core_records::ConstRecord>(),
            Self::Property | Self::IntProperty | Self::FloatProperty | Self::NameProperty | Self::StringProperty =>
                Layout::new::<generated::UProperty>(),
            Self::ByteProperty => Layout::new::<generated::UByteProperty>(),
            Self::BoolProperty => Layout::new::<generated::UBoolProperty>(),
            Self::ObjectProperty => Layout::new::<generated::UObjectProperty>(),
            Self::ClassProperty => Layout::new::<generated::UClassProperty>(),
            Self::StructProperty => Layout::new::<generated::UStructProperty>(),
            Self::FixedArrayProperty => Layout::new::<generated::UFixedArrayProperty>(),
            Self::ArrayProperty => Layout::new::<generated::UArrayProperty>(),
            Self::MapProperty => Layout::new::<core_records::MapPropertyRecord>(),
            Self::Linker => Layout::new::<generated::ULinker>(),
            Self::LinkerLoad => Layout::new::<generated::ULinkerLoad>(),
            Self::LinkerSave => crate::core_save::allocation_layout(),
            Self::Commandlet => Layout::new::<core_records::CommandletRecord>(),
            Self::Package => Layout::new::<core_records::PackageRecord>(),
            Self::Subsystem => Layout::new::<core_records::SubsystemRecord>(),
            Self::Language => Layout::new::<core_records::LanguageRecord>(),
            Self::TextBuffer => Layout::new::<core_records::TextBufferRecord>(),
            Self::System => Layout::new::<core_records::SystemRecord>(),
            Self::Factory => Layout::new::<core_records::FactoryRecord>(),
            Self::Exporter => Layout::new::<core_records::ExporterRecord>(),
            Self::PackageMap => Layout::new::<core_records::PackageMapRecord>(),
            Self::TextBufferFactory => Layout::new::<core_records::TextBufferFactoryRecord>(),
        };
        layout.align_to(std::mem::align_of::<generated::UObject>())
            .expect("static native record layouts have a valid UObject alignment")
    }
}

/// Native Core property category emitted by a static constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupPropertyKind {
    Byte,
    ByteEnum { name: &'static str, values: &'static [&'static str] },
    Int,
    Float,
    Bool { bit_mask: u32 },
    String,
    Name,
    Object { class: &'static str },
    Class { meta_class: &'static str },
    Struct { name: &'static str, fields: &'static [StartupStructField] },
    Array,
    ArrayOfString,
    ArrayOfName,
}

/// A field in a struct property type created by a native static constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupStructField {
    pub name: &'static str,
    pub kind: StartupScalarKind,
    pub offset: usize,
}

/// Scalar leaf kinds supported by native startup struct declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupScalarKind {
    Byte,
    Int,
    Float,
}

/// One immutable property declaration from a native StaticConstructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupProperty {
    pub name: &'static str,
    pub kind: StartupPropertyKind,
    pub category: &'static str,
    pub property_flags: u32,
    pub object_flags: u32,
    pub inner_name: Option<&'static str>,
    pub offset: usize,
    pub array_dim: usize,
}

/// Default effect applied by a native static constructor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StartupDefault {
    SetBool { field: &'static str, value: bool },
    SetByte { field: &'static str, value: u8 },
    SetClass { field: &'static str, class: &'static str },
    SetFloat { field: &'static str, value: f32 },
    SetInt { field: &'static str, value: i32 },
    AppendString { field: &'static str, value: &'static str },
}

/// Immutable identity and startup behavior for one registered native class.
#[derive(Clone, Copy, Debug)]
pub struct NativeClassSpec {
    pub short_name: &'static str,
    pub package: &'static str,
    pub super_name: Option<&'static str>,
    pub within: &'static str,
    pub flags: u32,
    pub object_flags: u32,
    pub config: &'static str,
    pub guid: [u32; 4],
    pub kind: NativeRecordKind,
    /// Complete record layout; `None` uses the Core kind's layout.
    /// Allocation strengthens packed-record alignment for the UObject prefix
    /// without changing native field offsets or reflected property alignment.
    pub layout: Option<Layout>,
    /// Ordinary Rust lifecycle, archive, GC and callback operations.
    /// `None` selects the Core operations for `kind`.
    pub operations: Option<crate::object_owner::ClassOps>,
    pub properties: &'static [StartupProperty],
    pub defaults: &'static [StartupDefault],
}

const PUBLIC_STARTUP: u32 = 4;

const NATIVE_CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const TRANSIENT: u32 = 0x00008;
const ABSTRACT: u32 = 0x00001;
const LOCALIZED: u32 = 0x00020;
const CONFIG: u32 = 0x00004;
const CPF_CONFIG: u32 = 0x00004000;
const NONE: &[StartupProperty] = &[];
const NO_DEFAULTS: &[StartupDefault] = &[];
const FACTORY_PROPERTIES: &[StartupProperty] = &[
    StartupProperty { name: "Description", kind: StartupPropertyKind::String, category: "", property_flags: 0, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::FactoryRecord, description), array_dim: 1 },
    StartupProperty { name: "InContextCommand", kind: StartupPropertyKind::String, category: "", property_flags: 0, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::FactoryRecord, in_context_command), array_dim: 1 },
    StartupProperty { name: "OutOfContextCommand", kind: StartupPropertyKind::String, category: "", property_flags: 0, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::FactoryRecord, out_of_context_command), array_dim: 1 },
    StartupProperty { name: "Formats", kind: StartupPropertyKind::ArrayOfString, category: "", property_flags: 0, object_flags: PUBLIC_STARTUP, inner_name: Some("StrProperty0"), offset: std::mem::offset_of!(core_records::FactoryRecord, formats), array_dim: 1 },
];
const EXPORTER_PROPERTIES: &[StartupProperty] = &[
    StartupProperty { name: "Formats", kind: StartupPropertyKind::ArrayOfString, category: "", property_flags: 0, object_flags: PUBLIC_STARTUP, inner_name: Some("StrProperty0"), offset: std::mem::offset_of!(core_records::ExporterRecord, formats), array_dim: 1 },
];
const SYSTEM_PROPERTIES: &[StartupProperty] = &[
    StartupProperty { name: "PurgeCacheDays", kind: StartupPropertyKind::Int, category: "Options", property_flags: CPF_CONFIG, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::SystemRecord, purge_cache_days), array_dim: 1 },
    StartupProperty { name: "SavePath", kind: StartupPropertyKind::String, category: "Options", property_flags: CPF_CONFIG, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::SystemRecord, save_path), array_dim: 1 },
    StartupProperty { name: "CachePath", kind: StartupPropertyKind::String, category: "Options", property_flags: CPF_CONFIG, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::SystemRecord, cache_path), array_dim: 1 },
    StartupProperty { name: "CacheExt", kind: StartupPropertyKind::String, category: "Options", property_flags: CPF_CONFIG, object_flags: PUBLIC_STARTUP, inner_name: None, offset: std::mem::offset_of!(core_records::SystemRecord, cache_ext), array_dim: 1 },
    StartupProperty { name: "Paths", kind: StartupPropertyKind::ArrayOfString, category: "Options", property_flags: CPF_CONFIG, object_flags: PUBLIC_STARTUP, inner_name: Some("StrProperty0"), offset: std::mem::offset_of!(core_records::SystemRecord, paths), array_dim: 1 },
    StartupProperty { name: "Suppress", kind: StartupPropertyKind::ArrayOfName, category: "Options", property_flags: CPF_CONFIG, object_flags: PUBLIC_STARTUP, inner_name: Some("NameProperty0"), offset: std::mem::offset_of!(core_records::SystemRecord, suppress), array_dim: 1 },
];
const TEXT_FACTORY_DEFAULTS: &[StartupDefault] = &[
    StartupDefault::SetClass { field: "SupportedClass", class: "TextBuffer" },
    StartupDefault::SetBool { field: "bCreateNew", value: false },
    StartupDefault::SetBool { field: "bText", value: true },
    StartupDefault::AppendString { field: "Formats", value: "txt;Text files" },
];

const fn spec(
    short_name: &'static str, super_name: Option<&'static str>, within: &'static str,
    flags: u32, kind: NativeRecordKind,
) -> NativeClassSpec {
    NativeClassSpec { short_name, package: "Core", super_name, within, flags,
        object_flags: NATIVE_CLASS_OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind, layout: None, operations: None, properties: NONE, defaults: NO_DEFAULTS }
}

/// Core native registrations in source order within each translation unit.
/// Cross-translation-unit static constructor order is intentionally not claimed.
pub static NATIVE_CLASSES: &[NativeClassSpec] = &[
    // UnObj.cpp
    spec("Object", None, "Object", ABSTRACT, NativeRecordKind::Object),
    spec("TextBuffer", Some("Object"), "Object", 0, NativeRecordKind::TextBuffer),
    spec("Enum", Some("Field"), "Struct", 0, NativeRecordKind::Enum),
    spec("Linker", Some("Object"), "Object", TRANSIENT, NativeRecordKind::Linker),
    spec("LinkerLoad", Some("Linker"), "Object", TRANSIENT, NativeRecordKind::LinkerLoad),
    spec("LinkerSave", Some("Linker"), "Object", TRANSIENT, NativeRecordKind::LinkerSave),
    spec("Subsystem", Some("Object"), "Object", TRANSIENT | ABSTRACT, NativeRecordKind::Subsystem),
    spec("Commandlet", Some("Object"), "Object", TRANSIENT | ABSTRACT | LOCALIZED, NativeRecordKind::Commandlet),
    spec("Package", Some("Object"), "Object", 0, NativeRecordKind::Package),
    NativeClassSpec { defaults: TEXT_FACTORY_DEFAULTS, ..spec("TextBufferFactory", Some("Factory"), "Object", 0, NativeRecordKind::TextBufferFactory) },
    spec("Language", Some("Object"), "Object", TRANSIENT | ABSTRACT, NativeRecordKind::Language),
    // UnClass.cpp
    spec("Field", Some("Object"), "Object", ABSTRACT, NativeRecordKind::Field),
    spec("Struct", Some("Field"), "Object", 0, NativeRecordKind::Struct),
    spec("State", Some("Struct"), "Object", 0, NativeRecordKind::State),
    spec("Class", Some("State"), "Package", 0, NativeRecordKind::Class),
    spec("Function", Some("Struct"), "State", 0, NativeRecordKind::Function),
    spec("Const", Some("Field"), "Struct", 0, NativeRecordKind::Const),
    spec("Property", Some("Field"), "Field", ABSTRACT, NativeRecordKind::Property),
    // UnProp.cpp. Property subclasses inherit UProperty::WithinClass (UField).
    spec("ByteProperty", Some("Property"), "Field", 0, NativeRecordKind::ByteProperty),
    spec("IntProperty", Some("Property"), "Field", 0, NativeRecordKind::IntProperty),
    spec("BoolProperty", Some("Property"), "Field", 0, NativeRecordKind::BoolProperty),
    spec("FloatProperty", Some("Property"), "Field", 0, NativeRecordKind::FloatProperty),
    spec("ObjectProperty", Some("Property"), "Field", 0, NativeRecordKind::ObjectProperty),
    spec("ClassProperty", Some("ObjectProperty"), "Field", 0, NativeRecordKind::ClassProperty),
    spec("NameProperty", Some("Property"), "Field", 0, NativeRecordKind::NameProperty),
    spec("StrProperty", Some("Property"), "Field", 0, NativeRecordKind::StringProperty),
    spec("FixedArrayProperty", Some("Property"), "Field", 0, NativeRecordKind::FixedArrayProperty),
    spec("ArrayProperty", Some("Property"), "Field", 0, NativeRecordKind::ArrayProperty),
    spec("MapProperty", Some("Property"), "Field", 0, NativeRecordKind::MapProperty),
    spec("StructProperty", Some("Property"), "Field", 0, NativeRecordKind::StructProperty),
    // UnUnix.cpp
    NativeClassSpec { properties: SYSTEM_PROPERTIES, ..spec("System", Some("Subsystem"), "Object", CONFIG, NativeRecordKind::System) },
    // UFactory.cpp, UExporter.cpp, then UnCoreNet.cpp, each in TU order.
    NativeClassSpec { properties: FACTORY_PROPERTIES, ..spec("Factory", Some("Object"), "Object", ABSTRACT, NativeRecordKind::Factory) },
    NativeClassSpec { properties: EXPORTER_PROPERTIES, ..spec("Exporter", Some("Object"), "Object", ABSTRACT, NativeRecordKind::Exporter) },
    spec("PackageMap", Some("Object"), "Object", TRANSIENT, NativeRecordKind::PackageMap),
];

/// Iterates immutable Core registration descriptions.
#[must_use]
pub fn iter() -> impl Iterator<Item = &'static NativeClassSpec> { NATIVE_CLASSES.iter() }
