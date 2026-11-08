//! Typed lifecycle and base serialization for Rust-owned Core records.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_class_specs::NativeRecordKind;
use crate::object_archive::ObjectArchive;
use crate::object_owner::ClassOps;
use crate::object_serialization::{self, Hp2ObjectSerializationLeaf as Leaf};
use crate::core_records;
use hp2_abi::engine::generated::{FFrame, FStateFrame, UClass, UFunction, UObject, UStruct};
use std::ffi::c_void;
use std::ptr;

// Values are the frozen EObjectFlags definitions in Core/Inc/UnObjBas.h.
const RF_DEBUG_SERIALIZE: u32 = 0x4000_0000;
const RF_HAS_STACK: u32 = 0x0200_0000;
const RF_DEBUG_DESTROY: u32 = 0x8000_0000;
const NAME_DEV_KILL: &str = "DevKill";

unsafe fn post_edit_change(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { access.modify_object(object) }
}
unsafe fn static_constructor(
    _access: CoreAccess<'_>,
    _class: *mut UClass,
    _default: *mut UObject,
) -> RuntimeResult<()> {
    Ok(())
}
unsafe fn object_is_pending_kill(_access: CoreAccess<'_>, _object: *mut UObject) -> RuntimeResult<bool> {
    Ok(false)
}

unsafe fn object_process_remote_function(
    _access: CoreAccess<'_>, _object: *mut UObject, _function: *mut UFunction,
    _parameters: *mut c_void, _stack: *mut FFrame,
) -> RuntimeResult<bool> {
    Ok(false)
}

unsafe fn finalize_members(_access: CoreAccess<'_>, _object: *mut UObject) -> RuntimeResult<()> {
    Ok(())
}


/// Returns the ordinary Rust operations for one registered Core record kind.
pub const fn class_ops(kind: NativeRecordKind) -> ClassOps {
    macro_rules! ops {
        ($kind:ident, $ctor:ident, $destroy:ident, $serialize:ident, $post_load:ident, $visit:ident, $event:ident) => {{
            unsafe fn $ctor(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
                construct(access, object, NativeRecordKind::$kind)
            }
            unsafe fn $destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
                destroy(access, object, NativeRecordKind::$kind)
            }
            unsafe fn $serialize(
                access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
            ) -> RuntimeResult<()> {
                serialize_record(access, object, archive, NativeRecordKind::$kind)
            }
            unsafe fn $post_load(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
                post_load(access, object, NativeRecordKind::$kind)
            }
            unsafe fn $visit(
                access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
            ) -> RuntimeResult<()> {
                visit_references(access, object, archive, NativeRecordKind::$kind)
            }
            unsafe fn $event(
                access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction,
                parameters: *mut c_void, result: *mut c_void,
            ) -> RuntimeResult<()> {
                access.execute_vm_event(object, function, parameters, result)
            }
            ClassOps { construct: $ctor, destroy: $destroy, finalize_members, serialize: $serialize,
                static_constructor, post_load: $post_load, initialize_execution, post_edit_change,
                visit_references: $visit, is_pending_kill: object_is_pending_kill,
                process_remote_function: object_process_remote_function, execute_event: $event }
        }};
    }
    match kind {
        NativeRecordKind::Object => ops!(Object, object_construct, object_destroy, object_serialize, object_post_load, object_visit, object_event),
        NativeRecordKind::Field => ops!(Field, field_construct, field_destroy, field_serialize, field_post_load, field_visit, field_event),
        NativeRecordKind::Struct => ops!(Struct, struct_construct, struct_destroy, struct_serialize, struct_post_load, struct_visit, struct_event),
        NativeRecordKind::State => ops!(State, state_construct, state_destroy, state_serialize, state_post_load, state_visit, state_event),
        NativeRecordKind::Class => ops!(Class, class_construct, class_destroy, class_serialize, class_post_load, class_visit, class_event),
        NativeRecordKind::Function => ops!(Function, function_construct, function_destroy, function_serialize, function_post_load, function_visit, function_event),
        NativeRecordKind::Enum => ops!(Enum, enum_construct, enum_destroy, enum_serialize, enum_post_load, enum_visit, enum_event),
        NativeRecordKind::Const => ops!(Const, const_construct, const_destroy, const_serialize, const_post_load, const_visit, const_event),
        NativeRecordKind::Property => ops!(Property, property_construct, property_destroy, property_serialize, property_post_load, property_visit, property_event),
        NativeRecordKind::ByteProperty => ops!(ByteProperty, byte_property_construct, byte_property_destroy, byte_property_serialize, byte_property_post_load, byte_property_visit, byte_property_event),
        NativeRecordKind::IntProperty => ops!(IntProperty, int_property_construct, int_property_destroy, int_property_serialize, int_property_post_load, int_property_visit, int_property_event),
        NativeRecordKind::BoolProperty => ops!(BoolProperty, bool_property_construct, bool_property_destroy, bool_property_serialize, bool_property_post_load, bool_property_visit, bool_property_event),
        NativeRecordKind::FloatProperty => ops!(FloatProperty, float_property_construct, float_property_destroy, float_property_serialize, float_property_post_load, float_property_visit, float_property_event),
        NativeRecordKind::ObjectProperty => ops!(ObjectProperty, object_property_construct, object_property_destroy, object_property_serialize, object_property_post_load, object_property_visit, object_property_event),
        NativeRecordKind::ClassProperty => ops!(ClassProperty, class_property_construct, class_property_destroy, class_property_serialize, class_property_post_load, class_property_visit, class_property_event),
        NativeRecordKind::NameProperty => ops!(NameProperty, name_property_construct, name_property_destroy, name_property_serialize, name_property_post_load, name_property_visit, name_property_event),
        NativeRecordKind::StringProperty => ops!(StringProperty, string_property_construct, string_property_destroy, string_property_serialize, string_property_post_load, string_property_visit, string_property_event),
        NativeRecordKind::FixedArrayProperty => ops!(FixedArrayProperty, fixed_array_property_construct, fixed_array_property_destroy, fixed_array_property_serialize, fixed_array_property_post_load, fixed_array_property_visit, fixed_array_property_event),
        NativeRecordKind::ArrayProperty => ops!(ArrayProperty, array_property_construct, array_property_destroy, array_property_serialize, array_property_post_load, array_property_visit, array_property_event),
        NativeRecordKind::MapProperty => ops!(MapProperty, map_property_construct, map_property_destroy, map_property_serialize, map_property_post_load, map_property_visit, map_property_event),
        NativeRecordKind::StructProperty => ops!(StructProperty, struct_property_construct, struct_property_destroy, struct_property_serialize, struct_property_post_load, struct_property_visit, struct_property_event),
        NativeRecordKind::Linker => ops!(Linker, linker_construct, linker_destroy, linker_serialize, linker_post_load, linker_visit, linker_event),
        NativeRecordKind::LinkerLoad => ops!(LinkerLoad, linker_load_construct, linker_load_destroy, linker_load_serialize, linker_load_post_load, linker_load_visit, linker_load_event),
        NativeRecordKind::LinkerSave => ops!(LinkerSave, linker_save_construct, linker_save_destroy, linker_save_serialize, linker_save_post_load, linker_save_visit, linker_save_event),
        NativeRecordKind::Commandlet => ops!(Commandlet, commandlet_construct, commandlet_destroy, commandlet_serialize, commandlet_post_load, commandlet_visit, commandlet_event),
        NativeRecordKind::Package => ops!(Package, package_construct, package_destroy, package_serialize, package_post_load, package_visit, package_event),
        NativeRecordKind::Subsystem => ops!(Subsystem, subsystem_construct, subsystem_destroy, subsystem_serialize, subsystem_post_load, subsystem_visit, subsystem_event),
        NativeRecordKind::Language => ops!(Language, language_construct, language_destroy, language_serialize, language_post_load, language_visit, language_event),
        NativeRecordKind::TextBuffer => ops!(TextBuffer, text_buffer_construct, text_buffer_destroy, text_buffer_serialize, text_buffer_post_load, text_buffer_visit, text_buffer_event),
        NativeRecordKind::System => ops!(System, system_construct, system_destroy, system_serialize, system_post_load, system_visit, system_event),
        NativeRecordKind::Factory => ops!(Factory, factory_construct, factory_destroy, factory_serialize, factory_post_load, factory_visit, factory_event),
        NativeRecordKind::Exporter => ops!(Exporter, exporter_construct, exporter_destroy, exporter_serialize, exporter_post_load, exporter_visit, exporter_event),
        NativeRecordKind::PackageMap => ops!(PackageMap, package_map_construct, package_map_destroy, package_map_serialize, package_map_post_load, package_map_visit, package_map_event),
        NativeRecordKind::TextBufferFactory => ops!(TextBufferFactory, text_buffer_factory_construct, text_buffer_factory_destroy, text_buffer_factory_serialize, text_buffer_factory_post_load, text_buffer_factory_visit, text_buffer_factory_event),
    }
}
unsafe fn initialize_execution(
    _access: CoreAccess<'_>, object: *mut UObject,
) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Cannot initialize a null object")); }
    let previous = unsafe { (*object).state_frame.cast::<FStateFrame>() };
    let class = unsafe { (*object).class.cast::<UClass>() };
    if class.is_null() { return Err(RuntimeError::new("Cannot initialize an object without a class")); }
    // SAFETY: dispatcher validation ensures canonical ownership; the class and
    // any existing frame remain live for the shared native initialization.
    unsafe {
        crate::object_instance::initialize_state_frame(
            object.cast(), class.cast(), previous,
            |frame| {
                (*object).state_frame = frame.cast();
                Ok(())
            },
            || {
                (*object).object_flags |= RF_HAS_STACK;
                Ok(())
            },
        )
    }
}

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject, kind: NativeRecordKind) -> RuntimeResult<()> {
    if matches!(kind,
        NativeRecordKind::Property | NativeRecordKind::ByteProperty | NativeRecordKind::IntProperty
        | NativeRecordKind::BoolProperty | NativeRecordKind::FloatProperty | NativeRecordKind::ObjectProperty
        | NativeRecordKind::ClassProperty | NativeRecordKind::NameProperty | NativeRecordKind::StringProperty
        | NativeRecordKind::FixedArrayProperty | NativeRecordKind::ArrayProperty | NativeRecordKind::MapProperty
        | NativeRecordKind::StructProperty
    ) {
        unsafe {
            crate::class_lifecycle::initialize_field_instance_data(object.cast(), ptr::null_mut());
            crate::native_property_offsets::initialize_default_data(object.cast());
        }
    }
    if matches!(kind, NativeRecordKind::Struct | NativeRecordKind::State | NativeRecordKind::Class | NativeRecordKind::Function) {
        // The ordinary native constructors enter UField(NULL), then UStruct(NULL).
        // Registration constructors have a separate path in core_bootstrap.
        unsafe {
            crate::class_lifecycle::initialize_field_instance_data(object.cast(), ptr::null_mut());
            crate::class_lifecycle::initialize_struct_instance_data(object.cast(), ptr::null_mut(), 4);
        }
        if kind == NativeRecordKind::Class {
            unsafe {
                crate::class_lifecycle::initialize_class_instance_data(
                    object.cast(), access.native_class("Object")?.cast(),
                );
            }
        }
    }
    if kind == NativeRecordKind::Function {
        unsafe { access.bind_function(object.cast::<UFunction>())?; }
    }
    if kind == NativeRecordKind::PackageMap {
        unsafe { initialize_package_map(object.cast())?; }
    }
    if kind == NativeRecordKind::LinkerSave {
        unsafe { crate::core_save::initialize(object)?; }
    }
    Ok(())
}

unsafe fn initialize_package_map(
    record: *mut core_records::PackageMapRecord,
) -> RuntimeResult<()> {
    // TMapBase<T>::TMapBase initializes HashCount to eight and allocates the
    // empty bucket table through Rehash; pair storage remains an empty TArray.
    let linker_map = unsafe { ptr::addr_of_mut!((*record).linker_map) };
    unsafe { ptr::addr_of_mut!((*linker_map).hash_count).write(8); }
    if let Err(error) = unsafe { crate::core_array::initialize_map_storage(linker_map) } {
        // Cleanup is best-effort; preserve the allocation failure.
        let _ = unsafe { destroy_package_map(record) };
        return Err(error);
    }

    let class_field_indices = unsafe { ptr::addr_of_mut!((*record).class_field_indices) };
    unsafe { ptr::addr_of_mut!((*class_field_indices).hash_count).write(8); }
    if let Err(error) = unsafe { crate::core_array::initialize_map_storage(class_field_indices) } {
        // Release both maps without replacing the allocation error with a
        // cleanup diagnostic; destroy_package_map is best-effort here.
        let _ = unsafe { destroy_package_map(record) };
        return Err(error);
    }
    Ok(())
}

unsafe fn post_load(access: CoreAccess<'_>, object: *mut UObject, kind: NativeRecordKind) -> RuntimeResult<()> {
    if kind == NativeRecordKind::Class
        && unsafe { (*object.cast::<UClass>()).class_within }.is_null()
    {
        return Err(RuntimeError::new("Class has no ClassWithin"));
    }
    // Every native override routes through UObject::PostLoad before binding.
    unsafe { (*object).object_flags |= crate::object_lifecycle::RF_DEBUG_POST_LOAD; }
    match kind {
        NativeRecordKind::Function => unsafe { access.bind_function(object.cast::<UFunction>())?; },
        NativeRecordKind::Class => {
            let superclass = unsafe { (*object.cast::<UClass>()).super_field.cast::<UClass>() };
            if !superclass.is_null() {
                unsafe { access.conditional_post_load(superclass.cast())?; }
            }
        }
        _ => {}
    }
    Ok(())
}

unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject, kind: NativeRecordKind) -> RuntimeResult<()> {
    if kind == NativeRecordKind::Class {
        return unsafe { destroy_class(access, object.cast::<UClass>()) };
    }
    let mut first_error = None;
    match kind {
        // Native subclass members are destroyed before UObject::Destroy.
        NativeRecordKind::Struct | NativeRecordKind::State | NativeRecordKind::Function => {
            let structure = object.cast::<UStruct>();
            unsafe { release_raw_array(ptr::addr_of_mut!((*structure).script)); }
        }
        NativeRecordKind::TextBuffer => {
            let record = object.cast::<core_records::TextBufferRecord>();
            unsafe { release_raw_array(ptr::addr_of_mut!((*record).text)); }
        }
        NativeRecordKind::Enum => {
            let record = object.cast::<hp2_abi::engine::generated::UEnum>();
            unsafe { release_raw_array(ptr::addr_of_mut!((*record).names)); }
        }
        NativeRecordKind::Exporter => {
            let record = object.cast::<core_records::ExporterRecord>();
            remember(&mut first_error, unsafe { destroy_fstring_array(ptr::addr_of_mut!((*record).formats)) });
        }
        _ => {}
    }
    if kind == NativeRecordKind::LinkerLoad {
        // The native LinkerLoad override tears down its owner state before UObject::Destroy.
        remember(&mut first_error, unsafe { crate::core_loader::loader_destroy(access, object) });
    }
    if kind == NativeRecordKind::LinkerSave {
        remember(&mut first_error, unsafe { crate::core_save::destroy(object) });
    }
    remember(&mut first_error, unsafe { destroy_uobject_base(access, object) });
    if kind == NativeRecordKind::PackageMap {
        remember(&mut first_error, unsafe { destroy_package_map(object.cast()) });
    }
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

fn remember(first: &mut Option<RuntimeError>, result: RuntimeResult<()>) {
    if let Err(error) = result {
        if first.is_none() { *first = Some(error); }
    }
}

unsafe fn destroy_class(access: CoreAccess<'_>, class: *mut UClass) -> RuntimeResult<()> {
    use crate::class_lifecycle::ClassDestroyStep::*;
    let mut first_error = None;
    let result = unsafe {
        crate::class_lifecycle::destroy_class_with(class, |step, class| match step {
            ClearNetFields => { release_raw_array(ptr::addr_of_mut!((*class).net_fields)); Ok(()) }
            ClearDependencies => { release_raw_array(ptr::addr_of_mut!((*class).dependencies)); Ok(()) }
            ClearPackageImports => { release_raw_array(ptr::addr_of_mut!((*class).package_imports)); Ok(()) }
            ExitDefaultProperties => {
                if (*class).defaults.num == 0 {
                    Ok(())
                } else {
                    let data = ptr::addr_of!((*class).defaults.data).read_unaligned().cast::<u8>();
                    access.exit_properties(data, class.cast())
                }
            }
            ClearDefaults => { release_raw_array(ptr::addr_of_mut!((*class).defaults)); Ok(()) }
            ClearDefaultText => {
                release_raw_array(ptr::addr_of_mut!((*class.cast::<core_records::ClassRecord>()).default_prop_text));
                Ok(())
            }
            DestroySuper => {
                release_raw_array(ptr::addr_of_mut!((*class.cast::<UStruct>()).script));
                destroy_uobject_base(access, class.cast())
            }
        })
    };
    remember(&mut first_error, result);
    // C++ destroys the replication-array member after UClass::Destroy returns.
    let reps = unsafe { ptr::addr_of_mut!((*class).class_reps) };
    unsafe {
        crate::array_storage::release_owned(
            ptr::addr_of_mut!((*reps).data),
            ptr::addr_of_mut!((*reps).num),
            ptr::addr_of_mut!((*reps).max),
        );
    }
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

unsafe fn destroy_package_map(record: *mut core_records::PackageMapRecord) -> RuntimeResult<()> {
    // Native `UPackageMap::Destroy` first calls UObject::Destroy. Its owner
    // dispatch performs that base sequence before these cache/value destructors.
    let mut first_error = None;
    let caches = unsafe { ptr::addr_of_mut!((*record).class_field_indices) };
    let pairs = unsafe { ptr::addr_of!((*caches).pairs).read_unaligned() };
    if pairs.num < 0 {
        remember(&mut first_error, Err(RuntimeError::new("Invalid class cache count")));
    } else if !pairs.data.is_null() {
        let entries = pairs.data.cast::<ClassCachePair>();
        for index in 0..pairs.num as usize {
            let cache = unsafe { ptr::addr_of!((*entries.add(index)).value).read_unaligned() };
            if cache.is_null() { continue; }
            unsafe {
                release_raw_array(ptr::addr_of_mut!((*cache).rep_properties));
                release_raw_array(ptr::addr_of_mut!((*cache).fields));
                release_map_storage(fclass_cache_field_map(cache));
                libc::free(cache.cast());
            }
        }
    }
    unsafe { release_map_storage(caches); }
    unsafe { release_raw_array(ptr::addr_of_mut!((*record).name_indices)); }
    unsafe { release_map_storage(ptr::addr_of_mut!((*record).linker_map)); }
    let list = unsafe { ptr::addr_of_mut!((*record).list) };
    let list_raw = unsafe { ptr::read_unaligned(list) };
    if list_raw.num < 0 {
        remember(&mut first_error, Err(RuntimeError::new("Invalid package list count")));
    } else if !list_raw.data.is_null() {
        let packages = list_raw.data.cast::<hp2_abi::engine::generated::FPackageInfo>();
        for index in 0..list_raw.num as usize {
            unsafe { release_raw_array(ptr::addr_of_mut!((*packages.add(index)).url)); }
        }
    }
    unsafe { release_raw_array(list); }
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

unsafe fn fclass_cache_field_map(
    cache: *mut hp2_abi::engine::generated::FClassNetCache,
) -> *mut core_records::MapStorageRaw {
    // The native FClassNetCache declaration places FieldMap immediately after
    // its measured Fields TArray. The generated prefix marks that tail opaque.
    let offset = std::mem::offset_of!(hp2_abi::engine::generated::FClassNetCache, fields)
        + std::mem::size_of::<hp2_abi::engine::generated::TArrayRaw>();
    unsafe { cache.cast::<u8>().add(offset).cast() }
}

pub(crate) unsafe fn release_map_storage(map: *mut core_records::MapStorageRaw) {
    unsafe {
        let pairs = ptr::addr_of_mut!((*map).pairs);
        release_raw_array(pairs);
        let hash = ptr::addr_of!((*map).hash).read_unaligned();
        if !hash.is_null() { libc::free(hash.cast()); }
        ptr::addr_of_mut!((*map).hash).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*map).hash_count).write(0);
    }
}
struct ClassCachePair {
    hash_next: i32,
    key: *mut c_void,
    value: *mut hp2_abi::engine::generated::FClassNetCache,
}

unsafe fn destroy_fstring_array(array: *mut hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<()> {
    let raw = unsafe { ptr::read_unaligned(array) };
    if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
        unsafe { release_raw_array(array); }
        return Err(RuntimeError::new("Invalid FString array storage"));
    }
    let values = raw.data.cast::<hp2_abi::engine::generated::TArrayRaw>();
    for index in 0..raw.num as usize {
        unsafe { release_raw_array(values.add(index)); }
    }
    unsafe { release_raw_array(array); }
    Ok(())
}
unsafe fn destroy_uobject_base(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let mut first_error = None;
    unsafe { (*object).object_flags |= RF_DEBUG_DESTROY; }
    let class = unsafe { ptr::addr_of!((*object).class).read() };
    remember(&mut first_error, unsafe { access.exit_properties(object.cast::<u8>(), class.cast()) });

    match access.intern_str(NAME_DEV_KILL, crate::name_owner::FindName::Find) {
        Ok(category) => match object_full_name(access, object) {
            Ok(full_name) => {
                let mut message = "Destroying ".chars().map(u32::from).collect::<Vec<_>>();
                message.extend(full_name);
                remember(&mut first_error, access.emit_log(category, &message));
            }
            Err(error) => remember(&mut first_error, Err(error)),
        },
        Err(error) => remember(&mut first_error, Err(error)),
    }

    let outer_index = unsafe {
        let outer = ptr::addr_of!((*object).outer).read().cast::<UObject>();
        if outer.is_null() { 0 } else { (*outer).index }
    };
    remember(&mut first_error, unsafe { access.unlink_object(object) });
    unsafe { (*object)._linker_index = outer_index; }
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

unsafe fn release_raw_array(array: *mut hp2_abi::engine::generated::TArrayRaw) {
    let data = unsafe { ptr::addr_of!((*array).data).read_unaligned() };
    if !data.is_null() {
        // SAFETY: Rust-owned Core arrays use the same libc-backed allocator and
        // the array contains no live nontrivial elements at this call site.
        unsafe { libc::free(data); }
    }
    unsafe { ptr::addr_of_mut!((*array).data).write_unaligned(ptr::null_mut()); }
    unsafe { ptr::addr_of_mut!((*array).num).write(0); }
    unsafe { ptr::addr_of_mut!((*array).max).write(0); }
}

unsafe fn serialize_record(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>, kind: NativeRecordKind,
) -> RuntimeResult<()> {
    if matches!(kind,
        NativeRecordKind::Property | NativeRecordKind::ByteProperty | NativeRecordKind::IntProperty
        | NativeRecordKind::BoolProperty | NativeRecordKind::FloatProperty | NativeRecordKind::ObjectProperty
        | NativeRecordKind::ClassProperty | NativeRecordKind::NameProperty | NativeRecordKind::StringProperty
        | NativeRecordKind::FixedArrayProperty | NativeRecordKind::ArrayProperty | NativeRecordKind::MapProperty
        | NativeRecordKind::StructProperty
    ) {
        return unsafe { crate::core_property::serialize_record(access, object.cast(), archive, kind) };
    }
    match metadata_kind(kind) {
        Some(metadata_kind) => unsafe { access.serialize_metadata(object, archive, metadata_kind)?; },
        None => unsafe { serialize_object_base(access, object, archive)?; },
    }

    match kind {
        NativeRecordKind::TextBuffer => {
            let record = object.cast::<core_records::TextBufferRecord>();
            let (mut pos, mut top) = unsafe { ((*record).pos, (*record).top) };
            archive.serialize_i32(&mut pos)?;
            archive.serialize_i32(&mut top)?;
            unsafe { (*record).pos = pos; (*record).top = top; }
            unsafe { access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*record).text))?; }
        }
        NativeRecordKind::Enum => {
            let record = object.cast::<hp2_abi::engine::generated::UEnum>();
            serialize_name_array(archive, unsafe { ptr::addr_of_mut!((*record).names) })?;
        }
        NativeRecordKind::Factory => {
            if !archive.is_loading() && !archive.is_saving() {
                let record = object.cast::<core_records::FactoryRecord>();
                unsafe {
                    archive.serialize_object_slot(ptr::addr_of_mut!((*record).supported_class).cast())?;
                    archive.serialize_object_slot(ptr::addr_of_mut!((*record).context_class).cast())?;
                }
            }
        }
        NativeRecordKind::Exporter => {
            let record = object.cast::<core_records::ExporterRecord>();
            unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*record).supported_class).cast())?; }
            unsafe { access.serialize_owned_fstring_array(archive, ptr::addr_of_mut!((*record).formats))?; }
        }
        NativeRecordKind::LinkerLoad => unsafe { crate::core_loader::loader_serialize(access, object, archive)?; },
        NativeRecordKind::PackageMap => unsafe { crate::core_array::serialize_package_map(archive, object.cast())?; },
        NativeRecordKind::Const => {
            let record = object.cast::<core_records::ConstRecord>();
            unsafe { access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*record).value))?; }
        }
        _ => {}
    }
    Ok(())
}

/// Serializes the UObject identity, mapped references, stack frame and properties.
/// Metadata owners call this only for their native UObject superclass step.
///
/// # Safety
/// `object`, its reflected class, state frame and archive mapping remain live.
pub(crate) unsafe fn serialize_object_base(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    let mut stack_guard = 0;
    object_serialization::serialize_with_runtime(
        RF_DEBUG_SERIALIZE, RF_HAS_STACK, true, &mut stack_guard,
        |leaf, a, b| unsafe { serialize_leaf(access, object, archive, leaf, a, b) },
    )
}

unsafe fn serialize_leaf(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
    leaf: Leaf, a: usize, b: usize,
) -> RuntimeResult<usize> {
    use Leaf::*;
    match leaf {
        SetFlags => { unsafe { (*object).object_flags |= a as u32; } Ok(0) }
        Class => Ok(unsafe { ptr::addr_of!((*object).class).read() } as usize),
        Object => Ok(object as usize),
        StaticClass => Ok(access.native_class("Class")? as usize),
        Preload => {
            if archive.is_trans() {
                unsafe { archive.preload_transaction_object(a as *mut UObject)?; }
            } else {
                unsafe { crate::core_loader::preload(access, a as *mut UObject)?; }
            }
            Ok(0)
        }
        IsLoading => Ok(usize::from(archive.is_loading())),
        IsSaving => Ok(usize::from(archive.is_saving())),
        IsTrans => Ok(usize::from(archive.is_trans())),
        Name => { unsafe { archive.serialize_name_slot(ptr::addr_of_mut!((*object).name).cast())?; } Ok(0) }
        Outer | SerializeClass => {
            let slot = if leaf == Outer { unsafe { ptr::addr_of_mut!((*object).outer) } }
                else { unsafe { ptr::addr_of_mut!((*object).class) } };
            unsafe { archive.serialize_object_slot(slot.cast())?; }
            Ok(0)
        }
        Linker => { unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*object)._linker).cast())?; } Ok(0) }
        Flags => Ok(unsafe { ptr::addr_of!((*object).object_flags).read() } as usize),
        Frame => Ok(unsafe { ptr::addr_of!((*object).state_frame).read() } as usize),
        PublishFrame => { unsafe { ptr::addr_of_mut!((*object).state_frame).write(a as *mut c_void); } Ok(0) }
        Node | StateNode | ProbeMask | LatentAction => {
            let frame = unsafe { ptr::addr_of!((*object).state_frame).read().cast::<FStateFrame>() };
            if frame.is_null() { return Err(RuntimeError::new("Object has no execution frame")); }
            match leaf {
                Node => { unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*frame).node).cast())?; } }
                StateNode => { unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*frame).state_node).cast())?; } }
                ProbeMask => {
                    let mut value = unsafe { ptr::addr_of!((*frame).probe_mask).read() };
                    serialize_u64(archive, &mut value)?;
                    unsafe { ptr::addr_of_mut!((*frame).probe_mask).write(value); }
                }
                LatentAction => {
                    let mut value = unsafe { ptr::addr_of!((*frame).latent_action).read() };
                    archive.serialize_i32(&mut value)?;
                    unsafe { ptr::addr_of_mut!((*frame).latent_action).write(value); }
                }
                _ => unreachable!(),
            }
            Ok(0)
        }
        FrameNode => {
            let frame = unsafe { ptr::addr_of!((*object).state_frame).read().cast::<FStateFrame>() };
            if frame.is_null() { return Ok(0); }
            Ok(unsafe { ptr::addr_of!((*frame).node).read() } as usize)
        }
        Code => {
            let frame = unsafe { ptr::addr_of!((*object).state_frame).read().cast::<FStateFrame>() };
            if frame.is_null() { return Ok(0); }
            Ok(unsafe { ptr::addr_of!((*frame).code).read() } as usize)
        }
        PublishCode => {
            let frame = unsafe { ptr::addr_of!((*object).state_frame).read().cast::<FStateFrame>() };
            if frame.is_null() { return Err(RuntimeError::new("Object has no execution frame")); }
            unsafe { ptr::addr_of_mut!((*frame).code).write(a as *mut u8); }
            Ok(0)
        }
        ScriptCount => {
            let structure = (a as *mut UObject).cast::<UStruct>();
            let script = unsafe { ptr::addr_of!((*structure).script).read() };
            Ok(script.num.max(0) as usize)
        }
        ScriptAt => {
            let structure = (a as *mut UObject).cast::<UStruct>();
            let script = unsafe { ptr::addr_of!((*structure).script).read() };
            if script.data.is_null() { return Err(RuntimeError::new("Null script bytecode")); }
            Ok(script.data.cast::<u8>().wrapping_add(b) as usize)
        }
        CheckSaveCode => {
            if a == 0 && true { return Err(RuntimeError::new("StateFrame code is outside the node script")); }
            Ok(0)
        }
        CheckNonnegativeOffset => {
            if a == 0 { return Err(RuntimeError::new("Negative state frame code offset")); }
            Ok(0)
        }
        CheckedOffset => i32::try_from(a).map(|value| value as usize)
            .map_err(|_| RuntimeError::new("State frame code offset exceeds INT")),
        Offset => {
            let value = a as *mut i32;
            if value.is_null() { return Err(RuntimeError::new("Null state frame offset")); }
            unsafe { archive.serialize_compact_index(&mut *value)?; }
            Ok(0)
        }
        InvalidOffset => Err(RuntimeError::new("State frame script offset mismatch")),
        TaggedProperties => {
            let class = unsafe { (*object).class.cast::<UClass>() };
            let defaults = unsafe { access.default_object(class)? };
            let size = unsafe { (*class.cast::<UStruct>()).properties_size };
            unsafe { access.tagged_properties(class.cast(), archive, object.cast(), defaults.cast(), size)?; }
            Ok(0)
        }
        BinaryProperties => {
            let class = unsafe { (*object).class.cast::<UClass>() };
            unsafe { access.binary_properties(class.cast(), archive, object.cast())?; }
            Ok(0)
        }
        PropertiesSize => {
            let class = (a as *mut UObject).cast::<UStruct>();
            Ok(unsafe { (*class).properties_size.max(0) as usize })
        }
        CountBytes => { archive.count_bytes(a, a)?; Ok(0) }
    }
}

unsafe fn serialize_u64(archive: &ObjectArchive<'_>, value: &mut u64) -> RuntimeResult<()> {
    if archive.is_loading() {
        let mut bytes = [0; 8];
        archive.read(&mut bytes)?;
        *value = u64::from_le_bytes(bytes);
    } else if archive.is_saving() {
        archive.write(&value.to_le_bytes())?;
    }
    Ok(())
}

unsafe fn serialize_name_array(archive: &ObjectArchive<'_>, array: *mut hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<()> {
    unsafe { crate::core_array::serialize_names(archive, array) }
}

unsafe fn visit_references(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>, kind: NativeRecordKind,
) -> RuntimeResult<()> {
    unsafe { serialize_record(access, object, archive, kind) }
}

fn metadata_kind(kind: NativeRecordKind) -> Option<crate::class_metadata::Hp2MetadataKind> {
    use crate::class_metadata::Hp2MetadataKind::*;
    Some(match kind {
        NativeRecordKind::Field | NativeRecordKind::Enum | NativeRecordKind::Const => MetadataField,
        NativeRecordKind::Struct => MetadataStruct,
        NativeRecordKind::State => MetadataState,
        NativeRecordKind::Class => MetadataClass,
        NativeRecordKind::Function => MetadataFunction,
        _ => return None,
    })
}

pub(crate) fn object_full_name(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
    if object.is_null() { return Err(RuntimeError::new("Null UObject in full-name lookup")); }
    let class = unsafe { (*object).class.cast::<UObject>() };
    let mut result = if class.is_null() { Vec::new() }
        else { access.name_text(i32::from_ne_bytes(unsafe { (*class).name }))? };
    result.push(' ' as u32);
    let mut chain = Vec::new();
    let mut cursor = object;
    while !cursor.is_null() {
        let name = i32::from_ne_bytes(unsafe { (*cursor).name });
        chain.push(access.name_text(name)?);
        cursor = unsafe { (*cursor).outer.cast::<UObject>() };
    }
    for (index, name) in chain.iter().rev().enumerate() {
        if index != 0 { result.push('.' as u32); }
        result.extend_from_slice(name);
    }
    Ok(result)
}
