//! Native Core identities are canonical objects, not a second class registry.

use crate::class_lifecycle;
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_class_specs::{NATIVE_CLASSES, NativeClassSpec, NativeRecordKind, StartupDefault, StartupPropertyKind, StartupScalarKind, StartupStructField};
use crate::core_records::{ClassRecord, FactoryRecord};
use crate::name_owner::FindName;
use crate::object_owner::{ClassData, OwnedClassRecord, OwnedObject};
use crate::object_storage;
use hp2_abi::engine::generated::{
    FName, TArrayRaw, UArrayProperty, UBoolProperty, UByteProperty, UClass, UClassProperty, UEnum, UObject,
    UObjectProperty, UProperty, UStruct, UStructProperty,
};
use std::alloc::Layout;
use std::ptr;

/// Publish the native class graph before registration runs constructors or IO.
///
/// All fallible name/layout work is staged before publication. Staged records
/// contain only zeroed raw members and copied static metadata, so their ordinary
/// allocation drop is complete until they enter the canonical object table.
///
/// # Safety
/// Access denotes a fresh CoreOwner with no previously published records.
pub(crate) unsafe fn publish_native_classes(
    access: CoreAccess<'_>, declarations: &'static [NativeClassSpec],
) -> RuntimeResult<()> {
    if access.object_count() != 0 {
        return Err(RuntimeError::new("Native classes require fresh Core storage"));
    }
    let count = NATIVE_CLASSES.len() + declarations.len();
    for spec in NATIVE_CLASSES.iter().chain(declarations.iter()) {
        let layout = spec.layout.unwrap_or_else(|| spec.kind.layout());
        if layout.size() < std::mem::size_of::<UObject>()
            || layout.size() > i32::MAX as usize
            || layout.align() > i32::MAX as usize
        {
            return Err(RuntimeError::new("Invalid native class record layout")
                .with_context(&format!("{}.{}", spec.package, spec.short_name)));
        }
    }
    for (index, spec) in NATIVE_CLASSES.iter().chain(declarations.iter()).enumerate() {
        if NATIVE_CLASSES.iter().chain(declarations.iter()).take(index)
            .any(|prior| prior.package == spec.package && prior.short_name == spec.short_name)
        {
            return Err(RuntimeError::new("Duplicate native class identity")
                .with_context(&format!("{}.{}", spec.package, spec.short_name)));
        }
    }
    let class_index = spec_index("Core.Class", "Core", declarations)?;
    let package_index = spec_index("Core.Package", "Core", declarations)?;
    let mut staged = Vec::with_capacity(count);
    for spec in NATIVE_CLASSES.iter().chain(declarations.iter()) {
        let name = access.intern_str(spec.short_name, FindName::Intrinsic)?;
        staged.push(OwnedObject::allocate(
            Layout::new::<OwnedClassRecord>(), ptr::null_mut(), ptr::null_mut(), name, spec.object_flags,
        )?);
    }
    let meta = staged[class_index].as_ptr().cast::<UClass>();
    let package_class = staged[package_index].as_ptr();
    let mut packages = Vec::with_capacity(count);
    let core_name = access.intern_str("Core", FindName::Add)?;
    packages.push(("Core", OwnedObject::allocate(
        NativeRecordKind::Package.layout(), package_class.cast(), ptr::null_mut(), core_name, 4,
    )?));
    for spec in NATIVE_CLASSES.iter().chain(declarations.iter()) {
        if !packages.iter().any(|(package, _)| *package == spec.package) {
            let name = access.intern_str(spec.package, FindName::Add)?;
            packages.push((spec.package, OwnedObject::allocate(
                NativeRecordKind::Package.layout(), package_class.cast(), ptr::null_mut(), name, 4,
            )?));
        }
    }
    let package_pointer = |name: &str| packages.iter().find(|(package, _)| *package == name)
        .map(|(_, owner)| owner.as_ptr()).ok_or_else(|| RuntimeError::new("Native class package is missing").with_context(name));
    let core_pointer = package_pointer("Core")?;
    let transient_name = access.intern_str("Transient", FindName::Add)?;
    let transient = OwnedObject::allocate(
        NativeRecordKind::Package.layout(), package_class.cast(), ptr::null_mut(), transient_name, 0,
    )?;
    let transient_pointer = transient.as_ptr();

    for (index, spec) in NATIVE_CLASSES.iter().chain(declarations.iter()).enumerate() {
        let class = staged[index].as_ptr().cast::<ClassRecord>();
        let superclass = match spec.super_name {
            Some(name) => staged[spec_index(name, spec.package, declarations)?].as_ptr(),
            None => ptr::null_mut(),
        };
        let within = staged[spec_index(spec.within, spec.package, declarations)?].as_ptr();
        let config_name = access.intern_str(spec.config, FindName::Add)?;
        let identity = spec as *const NativeClassSpec;
        let layout = spec.layout.unwrap_or_else(|| spec.kind.layout());
        let properties_align = spec.layout.map_or(4, |layout| layout.align() as i32);
        let ops = spec.operations.unwrap_or_else(|| spec.kind.operations());
        let has_static_constructor = ops.static_constructor as usize
            != NativeRecordKind::Object.operations().static_constructor as usize;
        unsafe {
            (*class.cast::<UObject>()).class = meta.cast();
            (*class.cast::<UObject>()).outer = package_pointer(spec.package)?.cast();
            class_lifecycle::initialize_field_registration_data(class.cast(), superclass.cast(), true);
            class_lifecycle::initialize_struct_registration_data(
                class.cast::<UStruct>(), layout.size() as i32, properties_align,
            );
            class_lifecycle::initialize_state_registration_data(class.cast());
            (*class).class_flags = spec.flags | 0x00002 | 0x00010;
            (*class).class_guid = spec.guid;
            (*class).class_within = within.cast();
            (*class).class_config_name.index = config_name;
            (*class).class_constructor = identity.cast_mut().cast();
            if !spec.properties.is_empty() || !spec.defaults.is_empty() || has_static_constructor {
                (*class).class_static_constructor = identity.cast_mut().cast();
            }
            OwnedClassRecord::initialize(class.cast(), ClassData {
                kind: spec.kind,
                ops,
                constructor_identity: identity as usize,
                native_spec: Some(spec),
            });
        }
    }
    for (_, package) in packages {
        let pointer = package.as_ptr();
        unsafe { object_storage::add_owned(access.storage(), package, -1)?; }
        access.register_loaded_native_module(pointer);
    }
    for record in staged {
        let class = record.as_ptr();
        unsafe {
            object_storage::add_owned(access.storage(), record, -1)?;
            // Native classes are owner-lifetime records. Trace their packages,
            // metadata and defaults even when a caller supplies no keep flags.
            access.add_root(class);
        }
    }
    unsafe {
        object_storage::add_owned(access.storage(), transient, -1)?;
        access.set_bootstrap_roots(core_pointer, meta, transient_pointer);
        access.add_root(transient_pointer);
    }
    Ok(())
}

fn spec_index(
    name: &str, package: &str, declarations: &[NativeClassSpec],
) -> RuntimeResult<usize> {
    let (qualified_package, short_name) = match name.split_once('.') {
        Some((package, short_name)) => (Some(package), short_name),
        None => (None, name),
    };
    let requested_package = qualified_package.unwrap_or(package);
    let find = |package: &str| {
        NATIVE_CLASSES.iter().chain(declarations.iter()).position(|spec| {
            spec.package == package && spec.short_name == short_name
        })
    };
    find(requested_package)
        .or_else(|| (qualified_package.is_none() && requested_package != "Core").then(|| find("Core")).flatten())
        .ok_or_else(|| RuntimeError::new("Unknown native class declaration").with_context(name))
}

unsafe fn startup_default_declaration(
    access: CoreAccess<'_>, mut class: *mut UClass, field: &str,
) -> RuntimeResult<Option<&'static crate::core_class_specs::StartupProperty>> {
    while !class.is_null() {
        if let Some(spec) = unsafe { access.class_metadata(class)? }.native_spec {
            if let Some(declaration) = spec.properties.iter()
                .find(|property| property.name == field && property.array_dim == 1)
            {
                return Ok(Some(declaration));
            }
        }
        class = unsafe { (*class).super_field.cast() };
    }
    Ok(None)
}

/// Apply the actual native declaration effects to this class's own default.
///
/// # Safety
/// The class and default buffer are initialized and remain live through object
/// construction, linking, and the ordinary output/config callbacks they invoke.
pub(crate) unsafe fn static_constructor(
    access: CoreAccess<'_>, class: *mut UClass, default: *mut UObject,
) -> RuntimeResult<()> {
    let metadata = unsafe { access.class_metadata(class)? };
    let spec = metadata.native_spec
        .ok_or_else(|| RuntimeError::new("Static constructor has no native declaration"))?;
    (metadata.ops.static_constructor)(access, class, default)?;
    // Native class constructors establish referenced structs before enums, and
    // establish all such types before appending this class's property list.
    for declaration in spec.properties {
        if let StartupPropertyKind::Struct { name, fields } = declaration.kind {
            unsafe { create_startup_struct(access, name, fields, declaration.object_flags)?; }
        }
    }
    for declaration in spec.properties {
        if let StartupPropertyKind::ByteEnum { name, values } = declaration.kind {
            unsafe { create_startup_enum(access, class, name, values)?; }
        }
    }
    for declaration in spec.properties {
        let property_class = access.native_class(match declaration.kind {
            StartupPropertyKind::Byte | StartupPropertyKind::ByteEnum { .. } => "ByteProperty",
            StartupPropertyKind::Float => "FloatProperty",
            StartupPropertyKind::Int => "IntProperty",
            StartupPropertyKind::Bool { .. } => "BoolProperty",
            StartupPropertyKind::String => "StrProperty",
            StartupPropertyKind::Name => "NameProperty",
            StartupPropertyKind::Object { .. } => "ObjectProperty",
            StartupPropertyKind::Class { .. } => "ClassProperty",
            StartupPropertyKind::Struct { .. } => "StructProperty",
            StartupPropertyKind::Array | StartupPropertyKind::ArrayOfString | StartupPropertyKind::ArrayOfName => "ArrayProperty",
        })?;
        let name = access.intern_str(declaration.name, FindName::Add)?;
        let property = unsafe {
            access.construct_object(property_class, class.cast(), name, declaration.object_flags, ptr::null_mut())?
        }.cast::<UProperty>();
        let category = access.intern_str(declaration.category, FindName::Add)?;
        unsafe {
            crate::native_property_offsets::initialize_cpp_prefix_data(property, declaration.property_flags);
            (*property).category = category.to_ne_bytes();
            (*property).offset = declaration.offset as i32;
            (*property).array_dim = declaration.array_dim as i32;
            (*property).cpp_property = 1;
            if let StartupPropertyKind::Bool { bit_mask } = declaration.kind {
                (*property.cast::<UBoolProperty>()).bit_mask = bit_mask;
            }
            crate::native_property_offsets::add_cpp_property(class.cast(), property.cast());
        }
        match declaration.kind {
            StartupPropertyKind::Object { class: target } => {
                let target = access.native_class(target)?;
                unsafe { (*property.cast::<UObjectProperty>()).property_class = target.cast() };
            }
            StartupPropertyKind::Class { meta_class } => {
                let class_type = access.native_class("Class")?;
                let constraint = access.native_class(meta_class)?;
                unsafe {
                    (*property.cast::<UObjectProperty>()).property_class = class_type.cast();
                    (*property.cast::<UClassProperty>()).meta_class = constraint.cast();
                }
            }
            StartupPropertyKind::ByteEnum { name, values } => {
                let enumeration = unsafe { create_startup_enum(access, class, name, values)? };
                unsafe { (*property.cast::<UByteProperty>()).r#enum = enumeration.cast() };
            }
            StartupPropertyKind::Struct { name, fields } => {
                let structure = unsafe { create_startup_struct(access, name, fields, declaration.object_flags)? };
                unsafe { (*property.cast::<UStructProperty>()).r#struct = structure.cast() };
            }
            _ => {}
        }
        let inner_class = match declaration.kind {
            StartupPropertyKind::ArrayOfString => Some("StrProperty"),
            StartupPropertyKind::ArrayOfName => Some("NameProperty"),
            _ => None,
        };
        if let Some(inner_class) = inner_class {
            let inner_name = declaration.inner_name
                .ok_or_else(|| RuntimeError::new("Native array declaration has no inner name"))?;
            let inner_class = access.native_class(inner_class)?;
            let inner_name = access.intern_str(inner_name, FindName::Add)?;
            let inner = unsafe {
                access.construct_object(inner_class, property.cast(), inner_name, declaration.object_flags, ptr::null_mut())?
            };
            unsafe { (*property.cast::<UArrayProperty>()).inner = inner.cast() };
        }
    }
    for effect in spec.defaults {
        match *effect {
            StartupDefault::SetClass { field: "SupportedClass", class: target } => {
                let target = access.native_class(target)?;
                unsafe { (*default.cast::<FactoryRecord>()).supported_class = target.cast() };
            }
            StartupDefault::SetBool { field, value } => {
                let (offset, mask) = if let Some(declaration) =
                    unsafe { startup_default_declaration(access, class, field)? }
                {
                    let StartupPropertyKind::Bool { bit_mask } = declaration.kind else {
                        return Err(RuntimeError::new("Native boolean default has no scalar boolean declaration")
                            .with_context(field));
                    };
                    (declaration.offset, bit_mask)
                } else {
                    // UFactory's non-reflected native bits retain their original defaults.
                    let mask = match field {
                        "bCreateNew" => 1,
                        "bText" => 1 << 3,
                        _ => return Err(RuntimeError::new("Unknown native boolean default").with_context(field)),
                    };
                    (std::mem::offset_of!(FactoryRecord, factory_bits), mask)
                };
                unsafe {
                    let bits = default.cast::<u8>().add(offset).cast::<u32>();
                    bits.write_unaligned((bits.read_unaligned() & !mask) | if value { mask } else { 0 });
                }
            }
            StartupDefault::SetByte { field, value } => {
                let declaration = unsafe { startup_default_declaration(access, class, field)? }
                    .filter(|property| matches!(property.kind, StartupPropertyKind::Byte | StartupPropertyKind::ByteEnum { .. }))
                    .ok_or_else(|| RuntimeError::new("Native byte default has no scalar byte declaration")
                        .with_context(field))?;
                unsafe { default.cast::<u8>().add(declaration.offset).write(value); }
            }
            StartupDefault::SetFloat { field, value } => {
                let declaration = unsafe { startup_default_declaration(access, class, field)? }
                    .filter(|property| property.kind == StartupPropertyKind::Float)
                    .ok_or_else(|| RuntimeError::new("Native float default has no scalar float declaration")
                        .with_context(field))?;
                unsafe {
                    default.cast::<u8>().add(declaration.offset).cast::<f32>().write_unaligned(value);
                }
            }
            StartupDefault::SetInt { field, value } => {
                let declaration = unsafe { startup_default_declaration(access, class, field)? }
                    .filter(|property| property.kind == StartupPropertyKind::Int)
                    .ok_or_else(|| RuntimeError::new("Native integer default has no scalar integer declaration")
                        .with_context(field))?;
                unsafe {
                    default.cast::<u8>().add(declaration.offset).cast::<i32>().write_unaligned(value);
                }
            }
            StartupDefault::AppendString { field: "Formats", value } => {
                unsafe {
                    crate::core_array::append_string_utf8(
                        ptr::addr_of_mut!((*default.cast::<FactoryRecord>()).formats), value,
                    )?;
                }
            }
            StartupDefault::SetClass { field, .. } | StartupDefault::AppendString { field, .. } => {
                return Err(RuntimeError::new("Unknown native default field").with_context(field));
            }
        }
    }
    Ok(())
}
unsafe fn create_startup_enum(
    access: CoreAccess<'_>,
    owner_class: *mut UClass,
    name: &str,
    values: &[&str],
) -> RuntimeResult<*mut UEnum> {
    let class = access.native_class("Enum")?;
    let name_index = access.intern_str(name, FindName::Add)?;
    let existing = unsafe { access.find_object(class.cast(), owner_class.cast(), name_index, true) };
    if !existing.is_null() { return Ok(existing.cast()); }
    let enumeration = unsafe {
        access.construct_object(class, owner_class.cast(), name_index, 4, ptr::null_mut())?
    }.cast::<UEnum>();
    for value in values {
        let index = access.intern_str(value, FindName::Add)?;
        let names = unsafe { ptr::addr_of_mut!((*enumeration).names) };
        let slots = crate::array_storage::Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*names).data) },
            num: unsafe { ptr::addr_of_mut!((*names).num) },
            max: unsafe { ptr::addr_of_mut!((*names).max) },
        };
        let item = unsafe {
            crate::array_storage::add(
                slots, 1, std::mem::size_of::<FName>() as i32, ptr::null(),
                crate::array_storage::system_allocator(),
            )
        }.map_err(|_| RuntimeError::new("Failed to extend native enum names"))?;
        let data = unsafe { ptr::addr_of!((*names).data).read_unaligned().cast::<FName>() };
        unsafe { data.add(item as usize).write_unaligned(FName { index }) };
    }
    Ok(enumeration)
}

unsafe fn create_startup_struct(
    access: CoreAccess<'_>,
    name: &str,
    fields: &'static [StartupStructField],
    object_flags: u32,
) -> RuntimeResult<*mut UStruct> {
    let class = access.native_class("Struct")?;
    let outer = access.native_class("Object")?;
    let name_index = access.intern_str(name, FindName::Add)?;
    let existing = unsafe { access.find_object(class.cast(), outer.cast(), name_index, true) };
    if !existing.is_null() {
        access.register_startup_struct(existing.cast(), fields)?;
        return Ok(existing.cast());
    }
    let structure = unsafe {
        access.construct_object(class, outer.cast(), name_index, object_flags, ptr::null_mut())?
    }.cast::<UStruct>();
    access.register_startup_struct(structure.cast(), fields)?;
    for field in fields {
        let (property_class_name, width) = match field.kind {
            StartupScalarKind::Byte => ("ByteProperty", 1),
            StartupScalarKind::Int => ("IntProperty", 4),
            StartupScalarKind::Float => ("FloatProperty", 4),
        };
        let property_class = access.native_class(property_class_name)?;
        let field_name = access.intern_str(field.name, FindName::Add)?;
        let property = unsafe {
            access.construct_object(property_class, structure.cast(), field_name, object_flags, ptr::null_mut())?
        }.cast::<UProperty>();
        unsafe {
            crate::native_property_offsets::initialize_cpp_prefix_data(property, 0);
            (*property).offset = field.offset as i32;
            (*property).array_dim = 1;
            (*property).cpp_property = 1;
            (*property).element_size = width;
            crate::native_property_offsets::add_cpp_property(structure, property.cast());
        }
    }
    unsafe { access.link_struct(structure, None, true)?; }
    Ok(structure)
}
