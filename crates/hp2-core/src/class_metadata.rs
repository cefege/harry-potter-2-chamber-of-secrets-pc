//! Live reflection metadata serialization, class registration and linking policy.
//! Rust owns compiler-probed fields; native leaves retain typed/virtual operations.

use crate::{
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    class_lifecycle,
    vm_registry::Hp2VmNativeHandler,
};
use hp2_abi::engine::generated::{
    TArrayRaw, UClass, UField, UFunction, UObject, UProperty, UState, UStruct,
};
use std::{ffi::c_void, ptr};

type Object = *mut c_void;
type Result<T = (), E = i32> = std::result::Result<T, E>;

pub const METADATA_RF_PUBLIC_STANDALONE: u32 = 0x0008_0004;
pub const METADATA_RF_NATIVE: u32 = 0x0400_0000;
pub const METADATA_FUNC_NET: u32 = 0x40;
pub const METADATA_FUNC_NATIVE: u32 = 0x400;
pub const METADATA_CPF_PARM: u32 = 0x80;
pub const METADATA_CPF_RETURN: u32 = 0x400;
pub const METADATA_HASH_COUNT: i32 = 256;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2MetadataKind {
    MetadataField,
    MetadataStruct,
    MetadataState,
    MetadataClass,
    MetadataFunction,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2MetadataPointer {
    MetadataInheritanceSuper,
    MetadataSuperState,
    MetadataSuperClass,
    MetadataRootClass,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2MetadataInteger {
    MetadataArchiveVersion,
    MetadataArchiveLoading,
    MetadataArchiveSaving,
    MetadataIsProperty,
    MetadataIsStruct,
}

/// Actual native `FArchive` operator type, never a metadata field or field group.
/// Reference, name and compound values retain native archive interception.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2MetadataArchiveValue {
    MetadataByte,
    MetadataWord,
    MetadataInt,
    MetadataDword,
    MetadataQword,
    MetadataFieldReference,
    MetadataTextBufferReference,
    MetadataClassReference,
    MetadataName,
    MetadataGuid,
    MetadataDependencyArray,
    MetadataNameArray,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2MetadataCheck {
    MetadataFriendlyNameValid,
    MetadataDefaultSizeValid,
    MetadataSuperclassLoaded,
    MetadataDefaultsMatch,
    MetadataScriptNativeZero,
    MetadataNativeIndexValid,
    MetadataNativeFunctionPresent,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ClassMetadataServices {
    pub pointer: unsafe extern "C" fn(Object, Hp2MetadataPointer, *mut Object) -> i32,
    pub integer: unsafe extern "C" fn(Object, Hp2MetadataInteger) -> i32,
    pub set_flags: unsafe extern "C" fn(Object, u32) -> i32,
    pub super_serialize: unsafe extern "C" fn(Object, Object, Hp2MetadataKind) -> i32,
    /// Serializes the supplied live field address; an error keeps its partial
    /// mutation and prevents Rust from visiting subsequent fields.
    pub serialize_value: unsafe extern "C" fn(Object, Hp2MetadataArchiveValue, Object) -> i32,
    pub serialize_expr: unsafe extern "C" fn(Object, Object, *mut i32) -> i32,
    pub script_mismatch: unsafe extern "C" fn(i32, i32) -> i32,
    pub check: unsafe extern "C" fn(Object, Hp2MetadataCheck) -> i32,
    pub link: unsafe extern "C" fn(Object, Object, i32) -> i32,
    pub array_allocator: unsafe extern "C" fn() -> *const Hp2ArrayServices,
    pub initialize_default: unsafe extern "C" fn(Object) -> i32,
    pub tagged_defaults: unsafe extern "C" fn(Object, Object) -> i32,
    pub load_config: unsafe extern "C" fn(Object) -> i32,
    pub load_localized: unsafe extern "C" fn(Object) -> i32,
    pub flush_localized: unsafe extern "C" fn() -> i32,
    pub count_defaults: unsafe extern "C" fn(Object, Object) -> i32,
    pub binary_defaults: unsafe extern "C" fn(Object, Object) -> i32,
    pub system_config: unsafe extern "C" fn(Object) -> i32,
    pub link_struct: unsafe extern "C" fn(Object, Object, i32) -> i32,
    pub lookup_dynamic: unsafe extern "C" fn(Object, *mut Object) -> i32,
    pub script_handler: Hp2VmNativeHandler,
    pub native_handlers: *const Hp2VmNativeHandler,
}

/// Operations required to execute the canonical reflection archive state machine.
///
/// Direct Rust owners implement this with ordinary `Result` propagation. The
/// service-table adapter below exists only for mixed native callers.
///
/// # Safety
/// Implementations must not retain object, archive, field, or storage references
/// across callbacks. Pointer arguments identify live fields only for the duration
/// of each call; callbacks may reenter and relocate any backing storage.
pub unsafe trait MetadataOps {
    type Error;

    /// Reads archive mode/version or property classification without stashing status.
    unsafe fn integer(&self, object: Object, selector: Hp2MetadataInteger) -> Result<i32, Self::Error>;
    /// Runs superclass serialization before this kind's fields.
    unsafe fn super_serialize(&self, object: Object, archive: Object, kind: Hp2MetadataKind) -> Result<(), Self::Error>;
    /// Direct owners may serialize the source-ordered base payload chain in one
    /// state-machine frame; mixed-runtime adapters retain superclass callbacks.
    fn flatten_superclasses(&self) -> bool {
        false
    }
    /// Serializes one live field location and preserves partial mutation on error.
    unsafe fn serialize_value(&self, archive: Object, value: Hp2MetadataArchiveValue, field: Object) -> Result<(), Self::Error>;
    /// Object-reference variant kept separate so direct GC recursion does not
    /// retain the full scalar/array serializer frame.
    unsafe fn serialize_reference(
        &self,
        archive: Object,
        value: Hp2MetadataArchiveValue,
        field: Object,
    ) -> Result<(), Self::Error> {
        unsafe { self.serialize_value(archive, value, field) }
    }
    unsafe fn check(&self, object: Object, check: Hp2MetadataCheck) -> Result<(), Self::Error>;
    /// Serializes one script expression, updating its live relocation cursor.
    unsafe fn serialize_expr(&self, object: Object, archive: Object, code: *mut i32) -> Result<(), Self::Error>;
    unsafe fn script_mismatch(&self, actual: i32, expected: i32) -> Result<(), Self::Error>;
    /// Reallocates/resets the owned script array using the direct allocator and
    /// typed lifetime rules; direct implementations must not call legacy services.
    unsafe fn reset_script(&self, object: Object, size: i32) -> Result<(), Self::Error>;
    unsafe fn link(&self, object: Object, archive: Object, props: i32) -> Result<(), Self::Error>;
    unsafe fn set_flags(&self, object: Object, flags: u32) -> Result<(), Self::Error>;
    unsafe fn system_config(&self, object: Object) -> Result<(), Self::Error>;
    unsafe fn initialize_default(&self, object: Object) -> Result<(), Self::Error>;
    unsafe fn tagged_defaults(&self, object: Object, archive: Object) -> Result<(), Self::Error>;
    unsafe fn load_config(&self, object: Object) -> Result<(), Self::Error>;
    unsafe fn load_localized(&self, object: Object) -> Result<(), Self::Error>;
    unsafe fn flush_localized(&self) -> Result<(), Self::Error>;
    unsafe fn count_defaults(&self, object: Object, archive: Object) -> Result<(), Self::Error>;
    unsafe fn binary_defaults(&self, object: Object, archive: Object) -> Result<(), Self::Error>;
    unsafe fn pointer(&self, object: Object, field: Hp2MetadataPointer) -> Result<Object, Self::Error>;
    /// Resets class defaults through the Rust owner and its typed field lifetimes.
    unsafe fn reset_class_defaults(&self, object: Object) -> Result<(), Self::Error>;
    /// Copies the current parameter metadata and next location before returning.
    unsafe fn function_parameter(&self, property: Object) -> Result<Option<FunctionParameterInfo>, Self::Error>;
}
/// Stable metadata copied from one reflected function parameter before reentry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionParameterInfo {
    pub flags: u32,
    pub offset: i32,
    pub array_dim: i32,
    pub element_size: i32,
    pub next: Object,
}
/// Direct class-registration operations for the established registration order.
///
/// Hooks must re-fetch mutable metadata after reentry; no object or store borrow
/// may be held while a hook runs.
pub unsafe trait ClassRegistrationOps {
    type Error;
    unsafe fn super_register(&self, class: Object) -> Result<(), Self::Error>;
    unsafe fn restore_config_name(&self, class: Object) -> Result<(), Self::Error>;
    unsafe fn reset_defaults(&self, class: Object) -> Result<(), Self::Error>;
    unsafe fn initialize_default(&self, class: Object) -> Result<(), Self::Error>;
    unsafe fn superclass(&self, class: Object) -> Object;
    unsafe fn constructor_identity(&self, class: Object) -> usize;
    unsafe fn default_object(&self, class: Object) -> Result<Object, Self::Error>;
    unsafe fn run_static_constructor(&self, class: Object, default: Object) -> Result<(), Self::Error>;
    unsafe fn class_flags(&self, class: Object) -> u32;
    unsafe fn add_class_flags(&self, class: Object, flags: u32) -> Result<(), Self::Error>;
    unsafe fn link_defaults(&self, class: Object) -> Result<(), Self::Error>;
    unsafe fn load_config(&self, class: Object) -> Result<(), Self::Error>;
    unsafe fn load_localized(&self, class: Object) -> Result<(), Self::Error>;
}
struct LegacyMetadataOps(Hp2ClassMetadataServices);

unsafe impl MetadataOps for LegacyMetadataOps {
    type Error = i32;

    unsafe fn integer(&self, object: Object, selector: Hp2MetadataInteger) -> Result<i32, i32> {
        Ok(unsafe { (self.0.integer)(object, selector) })
    }

    unsafe fn super_serialize(&self, object: Object, archive: Object, kind: Hp2MetadataKind) -> Result<(), i32> {
        status(unsafe { (self.0.super_serialize)(object, archive, kind) })
    }

    unsafe fn serialize_value(&self, archive: Object, value: Hp2MetadataArchiveValue, field: Object) -> Result<(), i32> {
        status(unsafe { (self.0.serialize_value)(archive, value, field) })
    }

    unsafe fn check(&self, object: Object, check: Hp2MetadataCheck) -> Result<(), i32> {
        status(unsafe { (self.0.check)(object, check) })
    }

    unsafe fn serialize_expr(&self, object: Object, archive: Object, code: *mut i32) -> Result<(), i32> {
        status(unsafe { (self.0.serialize_expr)(object, archive, code) })
    }

    unsafe fn script_mismatch(&self, actual: i32, expected: i32) -> Result<(), i32> {
        status(unsafe { (self.0.script_mismatch)(actual, expected) })
    }

    unsafe fn reset_script(&self, object: Object, size: i32) -> Result<(), i32> {
        let arrays = unsafe { (self.0.array_allocator)().read_unaligned() };
        const EMPTY_TAG: [u32; 1] = [0];
        unsafe {
            array_storage::empty(script_slots(object), 0, 1, EMPTY_TAG.as_ptr().cast(), arrays)?;
            array_storage::add(script_slots(object), size, 1, EMPTY_TAG.as_ptr().cast(), arrays)?;
        }
        Ok(())
    }

    unsafe fn link(&self, object: Object, archive: Object, props: i32) -> Result<(), i32> {
        status(unsafe { (self.0.link)(object, archive, props) })
    }

    unsafe fn set_flags(&self, object: Object, flags: u32) -> Result<(), i32> {
        status(unsafe { (self.0.set_flags)(object, flags) })
    }

    unsafe fn system_config(&self, object: Object) -> Result<(), i32> {
        status(unsafe { (self.0.system_config)(object) })
    }

    unsafe fn initialize_default(&self, object: Object) -> Result<(), i32> {
        status(unsafe { (self.0.initialize_default)(object) })
    }

    unsafe fn tagged_defaults(&self, object: Object, archive: Object) -> Result<(), i32> {
        status(unsafe { (self.0.tagged_defaults)(object, archive) })
    }

    unsafe fn load_config(&self, object: Object) -> Result<(), i32> {
        status(unsafe { (self.0.load_config)(object) })
    }

    unsafe fn load_localized(&self, object: Object) -> Result<(), i32> {
        status(unsafe { (self.0.load_localized)(object) })
    }

    unsafe fn flush_localized(&self) -> Result<(), i32> {
        status(unsafe { (self.0.flush_localized)() })
    }

    unsafe fn count_defaults(&self, object: Object, archive: Object) -> Result<(), i32> {
        status(unsafe { (self.0.count_defaults)(object, archive) })
    }

    unsafe fn binary_defaults(&self, object: Object, archive: Object) -> Result<(), i32> {
        status(unsafe { (self.0.binary_defaults)(object, archive) })
    }

    unsafe fn reset_class_defaults(&self, object: Object) -> Result<(), i32> {
        let arrays = unsafe { (self.0.array_allocator)().read_unaligned() };
        unsafe { class_lifecycle::reset_defaults(object.cast(), arrays) }
    }

    unsafe fn pointer(&self, object: Object, field: Hp2MetadataPointer) -> Result<Object, i32> {
        let mut value = ptr::null_mut();
        status(unsafe { (self.0.pointer)(object, field, &mut value) })?;
        Ok(value)
    }

    unsafe fn function_parameter(&self, property: Object) -> Result<Option<FunctionParameterInfo>, i32> {
        if unsafe { (self.0.integer)(property, Hp2MetadataInteger::MetadataIsProperty) } == 0 {
            return Ok(None);
        }
        let field = property.cast::<UProperty>();
        Ok(Some(unsafe {
            FunctionParameterInfo {
                flags: ptr::addr_of!((*field).property_flags).read_unaligned(),
                offset: ptr::addr_of!((*field).offset).read_unaligned(),
                array_dim: ptr::addr_of!((*field).array_dim).read_unaligned(),
                element_size: ptr::addr_of!((*field).element_size).read_unaligned(),
                next: ptr::addr_of!((*property.cast::<UField>()).next).read_unaligned(),
            }
        }))
    }
}


/// Loading-layout scope shared only by calls using this storage instance.
pub struct MetadataStorage {
    pub loading_layout: Object,
}

impl MetadataStorage {
    pub const fn new() -> Self {
        Self {
            loading_layout: ptr::null_mut(),
        }
    }
}

impl Default for MetadataStorage {
    fn default() -> Self {
        Self::new()
    }
}

unsafe fn script_slots(object: Object) -> Hp2ArraySlots {
    unsafe {
        let script: *mut TArrayRaw = ptr::addr_of_mut!((*object.cast::<UStruct>()).script);
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*script).data),
            num: ptr::addr_of_mut!((*script).num),
            max: ptr::addr_of_mut!((*script).max),
        }
    }
}

pub type Hp2ClassStaticConstructor = unsafe extern "C" fn(*mut c_void) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ClassRegistrationServices {
    pub super_register: unsafe extern "C" fn(Object) -> i32,
    pub restore_config_name: unsafe extern "C" fn(Object) -> i32,
    pub array_allocator: unsafe extern "C" fn() -> *const Hp2ArrayServices,
    pub initialize_default: unsafe extern "C" fn(Object) -> i32,
    pub superclass: unsafe extern "C" fn(Object) -> Object,
    pub static_constructor: unsafe extern "C" fn(Object) -> Hp2ClassStaticConstructor,
    pub default_object: unsafe extern "C" fn(Object, *mut Object) -> i32,
    pub class_flags: unsafe extern "C" fn(Object) -> u32,
    pub add_class_flags: unsafe extern "C" fn(Object, u32) -> i32,
    pub link_defaults: unsafe extern "C" fn(Object) -> i32,
    pub load_config: unsafe extern "C" fn(Object) -> i32,
    pub load_localized: unsafe extern "C" fn(Object) -> i32,
}

/// Legacy service-table entry point for mixed native callers.
///
/// # Safety
/// Objects, defaults and callbacks must remain live through reentry. Callbacks
/// must capture native exceptions and return status codes.
pub unsafe fn register_class(
    object: Object,
    inherit_mask: u32,
    services: &Hp2ClassRegistrationServices,
) -> Result {
    let ops = LegacyClassRegistrationOps(*services);
    unsafe { register_class_with(object, inherit_mask, &ops) }
}
/// Registers a class through direct owner operations in native-observable order.
///
/// # Safety
/// The class and all state read or changed by hooks must remain live through
/// reentry. Hooks must not retain object/store borrows across their calls.
pub unsafe fn register_class_with<O: ClassRegistrationOps>(
    object: Object,
    inherit_mask: u32,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    unsafe {
        ops.super_register(object)?;
        ops.restore_config_name(object)?;
        ops.reset_defaults(object)?;
        ops.initialize_default(object)?;
        let parent = ops.superclass(object);
        if parent.is_null() || ops.constructor_identity(object) != ops.constructor_identity(parent) {
            let default = ops.default_object(object)?;
            // Default creation can replace the registered constructor.
            ops.run_static_constructor(object, default)?;
        }
        let parent = ops.superclass(object);
        if !parent.is_null() {
            let inherited = ops.class_flags(parent) & inherit_mask;
            ops.add_class_flags(object, inherited)?;
        }
        ops.link_defaults(object)?;
        ops.load_config(object)?;
        ops.load_localized(object)
    }
}
struct LegacyClassRegistrationOps(Hp2ClassRegistrationServices);

unsafe impl ClassRegistrationOps for LegacyClassRegistrationOps {
    type Error = i32;
    unsafe fn super_register(&self, class: Object) -> Result<(), i32> {
        status(unsafe { (self.0.super_register)(class) })
    }
    unsafe fn restore_config_name(&self, class: Object) -> Result<(), i32> {
        status(unsafe { (self.0.restore_config_name)(class) })
    }
    unsafe fn reset_defaults(&self, class: Object) -> Result<(), i32> {
        let arrays = unsafe { (self.0.array_allocator)().read_unaligned() };
        unsafe { class_lifecycle::reset_defaults(class.cast(), arrays) }
    }
    unsafe fn initialize_default(&self, class: Object) -> Result<(), i32> {
        status(unsafe { (self.0.initialize_default)(class) })
    }
    unsafe fn superclass(&self, class: Object) -> Object {
        unsafe { (self.0.superclass)(class) }
    }
    unsafe fn constructor_identity(&self, class: Object) -> usize {
        unsafe { (self.0.static_constructor)(class) as usize }
    }
    unsafe fn default_object(&self, class: Object) -> Result<Object, i32> {
        let mut value = ptr::null_mut();
        status(unsafe { (self.0.default_object)(class, &mut value) })?;
        Ok(value)
    }
    unsafe fn run_static_constructor(&self, class: Object, default: Object) -> Result<(), i32> {
        let constructor = unsafe { (self.0.static_constructor)(class) };
        status(unsafe { constructor(default) })
    }
    unsafe fn class_flags(&self, class: Object) -> u32 {
        unsafe { (self.0.class_flags)(class) }
    }
    unsafe fn add_class_flags(&self, class: Object, flags: u32) -> Result<(), i32> {
        status(unsafe { (self.0.add_class_flags)(class, flags) })
    }
    unsafe fn link_defaults(&self, class: Object) -> Result<(), i32> {
        status(unsafe { (self.0.link_defaults)(class) })
    }
    unsafe fn load_config(&self, class: Object) -> Result<(), i32> {
        status(unsafe { (self.0.load_config)(class) })
    }
    unsafe fn load_localized(&self, class: Object) -> Result<(), i32> {
        status(unsafe { (self.0.load_localized)(class) })
    }
}


fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}

unsafe fn pointer(
    s: &Hp2ClassMetadataServices,
    object: Object,
    field: Hp2MetadataPointer,
) -> Result<Object> {
    let mut value = std::ptr::null_mut();
    status(unsafe { (s.pointer)(object, field, &mut value) })?;
    Ok(value)
}

// Keep kind-specific wire policy out of frames held by synchronous object-reference
// visits during GC.
#[inline(never)]
unsafe fn serialize_field_payload<O: MetadataOps>(
    object: Object,
    ar: Object,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataArchiveValue::MetadataFieldReference;
    use Hp2MetadataInteger::MetadataArchiveLoading;
    unsafe {
        let field = object.cast::<UField>();
        ops.serialize_reference(ar, MetadataFieldReference, ptr::addr_of_mut!((*field).super_field).cast())?;
        ops.serialize_reference(ar, MetadataFieldReference, ptr::addr_of_mut!((*field).next).cast())?;
        if ops.integer(ar, MetadataArchiveLoading)? != 0 {
            ptr::addr_of_mut!((*field).hash_next).write_unaligned(ptr::null_mut());
        }
    }
    Ok(())
}

#[inline(never)]
unsafe fn serialize_struct_payload<O: MetadataOps>(
    storage: *mut MetadataStorage,
    object: Object,
    ar: Object,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataArchiveValue::*;
    use Hp2MetadataCheck::*;
    use Hp2MetadataInteger::*;
    unsafe {
        let structure = object.cast::<UStruct>();
        ops.serialize_reference(ar, MetadataTextBufferReference, ptr::addr_of_mut!((*structure).script_text).cast())?;
        ops.serialize_reference(ar, MetadataFieldReference, ptr::addr_of_mut!((*structure).children).cast())?;
        ops.serialize_value(ar, MetadataName, ptr::addr_of_mut!((*structure).friendly_name).cast())?;
        ops.check(object, MetadataFriendlyNameValid)?;
        ops.serialize_value(ar, MetadataInt, ptr::addr_of_mut!((*structure).line).cast())?;
        ops.serialize_value(ar, MetadataInt, ptr::addr_of_mut!((*structure).text_pos).cast())?;
        let mut size = script_slots(object).num.read();
        ops.serialize_value(ar, MetadataInt, ptr::addr_of_mut!(size).cast())?;
        if ops.integer(ar, MetadataArchiveLoading)? != 0 {
            ops.reset_script(object, size)?;
        }
        let mut code = 0;
        while code < size {
            ops.serialize_expr(object, ar, &mut code)?;
        }
        if code != size {
            ops.script_mismatch(code, size)?;
        }
        if ops.integer(ar, MetadataArchiveLoading)? != 0 {
            let native_layout = object == ptr::addr_of!((*storage).loading_layout).read();
            let registered_size = ptr::addr_of!((*structure).properties_size).read_unaligned();
            let registered_align = ptr::addr_of!((*structure).properties_align).read_unaligned();
            ops.link(object, ar, 1)?;
            if native_layout {
                let size = ptr::addr_of!((*structure).properties_size)
                    .read_unaligned()
                    .max(registered_size);
                ptr::addr_of_mut!((*structure).properties_size).write_unaligned(size);
                let align = ptr::addr_of!((*structure).properties_align)
                    .read_unaligned()
                    .max(registered_align);
                ptr::addr_of_mut!((*structure).properties_align).write_unaligned(align);
                ptr::addr_of_mut!((*structure).properties_size).write_unaligned(
                    size.wrapping_add(align.wrapping_sub(1)) & !align.wrapping_sub(1),
                );
            }
        }
    }
    Ok(())
}

#[inline(never)]
unsafe fn serialize_state_payload<O: MetadataOps>(
    object: Object,
    ar: Object,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataArchiveValue::*;
    unsafe {
        let state = object.cast::<UState>();
        ops.serialize_value(ar, MetadataQword, ptr::addr_of_mut!((*state).probe_mask).cast())?;
        ops.serialize_value(ar, MetadataQword, ptr::addr_of_mut!((*state).ignore_mask).cast())?;
        ops.serialize_value(ar, MetadataWord, ptr::addr_of_mut!((*state).label_table_offset).cast())?;
        ops.serialize_value(ar, MetadataDword, ptr::addr_of_mut!((*state).state_flags).cast())?;
    }
    Ok(())
}

#[inline(never)]
unsafe fn serialize_class_payload<O: MetadataOps>(
    object: Object,
    ar: Object,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataArchiveValue::*;
    use Hp2MetadataCheck::*;
    use Hp2MetadataInteger::*;
    use Hp2MetadataPointer::*;
    unsafe {
        let class = object.cast::<UClass>();
        if ops.integer(ar, MetadataArchiveVersion)? <= 61 {
            let mut old_class_record_size = 0i32;
            ops.serialize_value(ar, MetadataInt, ptr::addr_of_mut!(old_class_record_size).cast())?;
            ops.set_flags(object, METADATA_RF_PUBLIC_STANDALONE)?;
        }
        ops.serialize_value(ar, MetadataDword, ptr::addr_of_mut!((*class).class_flags).cast())?;
        ops.serialize_value(ar, MetadataGuid, ptr::addr_of_mut!((*class).class_guid).cast())?;
        ops.serialize_value(ar, MetadataDependencyArray, ptr::addr_of_mut!((*class).dependencies).cast())?;
        ops.serialize_value(ar, MetadataNameArray, ptr::addr_of_mut!((*class).package_imports).cast())?;
        if ops.integer(ar, MetadataArchiveVersion)? >= 62 {
            ops.serialize_reference(ar, MetadataClassReference, ptr::addr_of_mut!((*class).class_within).cast())?;
            ops.serialize_value(ar, MetadataName, ptr::addr_of_mut!((*class).class_config_name).cast())?;
        } else {
            ops.system_config(object)?;
        }
        if ops.integer(ar, MetadataArchiveLoading)? != 0 {
            ops.check(object, MetadataDefaultSizeValid)?;
            ops.check(object, MetadataSuperclassLoaded)?;
            ops.reset_class_defaults(object)?;
            ops.initialize_default(object)?;
            ops.tagged_defaults(object, ar)?;
            ops.load_config(object)?;
            ops.load_localized(object)?;
            ptr::addr_of_mut!((*class).class_unique).write_unaligned(0);
            if ops.integer(ar, MetadataArchiveVersion)? <= 61 {
                let root = ops.pointer(object, MetadataRootClass)?;
                ptr::addr_of_mut!((*class).class_within).write_unaligned(root);
            }
            ops.flush_localized()?;
        } else if ops.integer(ar, MetadataArchiveSaving)? != 0 {
            ops.check(object, MetadataDefaultsMatch)?;
            ops.tagged_defaults(object, ar)?;
        } else {
            ops.check(object, MetadataDefaultsMatch)?;
            ops.count_defaults(object, ar)?;
            ops.binary_defaults(object, ar)?;
        }
    }
    Ok(())
}

#[inline(never)]
unsafe fn serialize_function_payload<O: MetadataOps>(
    object: Object,
    ar: Object,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataArchiveValue::*;
    use Hp2MetadataInteger::*;
    unsafe {
        let function = object.cast::<UFunction>();
        if ops.integer(ar, MetadataArchiveVersion)? <= 63 {
            ops.serialize_value(ar, MetadataWord, ptr::addr_of_mut!((*function).parms_size).cast())?;
        }
        ops.serialize_value(ar, MetadataWord, ptr::addr_of_mut!((*function).i_native).cast())?;
        if ops.integer(ar, MetadataArchiveVersion)? <= 63 {
            ops.serialize_value(ar, MetadataByte, ptr::addr_of_mut!((*function).num_parms).cast())?;
        }
        ops.serialize_value(ar, MetadataByte, ptr::addr_of_mut!((*function).oper_precedence).cast())?;
        if ops.integer(ar, MetadataArchiveVersion)? <= 63 {
            ops.serialize_value(ar, MetadataWord, ptr::addr_of_mut!((*function).return_value_offset).cast())?;
        }
        ops.serialize_value(ar, MetadataDword, ptr::addr_of_mut!((*function).function_flags).cast())?;
        if ptr::addr_of!((*function).function_flags).read_unaligned() & METADATA_FUNC_NET != 0 {
            ops.serialize_value(ar, MetadataWord, ptr::addr_of_mut!((*function).rep_offset).cast())?;
        }
        if ops.integer(ar, MetadataArchiveLoading)? != 0 {
            ptr::addr_of_mut!((*function).num_parms).write_unaligned(0);
            ptr::addr_of_mut!((*function).parms_size).write_unaligned(0);
            ptr::addr_of_mut!((*function).return_value_offset).write_unaligned(u16::MAX);
            let mut property = ptr::addr_of!((*object.cast::<UStruct>()).children).read_unaligned();
            while !property.is_null() {
                let Some(info) = ops.function_parameter(property)? else {
                    break;
                };
                if info.flags & METADATA_CPF_PARM == 0 {
                    break;
                }
                let count = ptr::addr_of!((*function).num_parms).read_unaligned().wrapping_add(1);
                ptr::addr_of_mut!((*function).num_parms).write_unaligned(count);
                let size = info.array_dim.wrapping_mul(info.element_size);
                ptr::addr_of_mut!((*function).parms_size)
                    .write_unaligned(info.offset.wrapping_add(size) as u16);
                if info.flags & METADATA_CPF_RETURN != 0 {
                    ptr::addr_of_mut!((*function).return_value_offset)
                        .write_unaligned(info.offset as u16);
                }
                property = info.next;
            }
        }
    }
    Ok(())
}

#[inline(never)]
unsafe fn serialize_kind_fields<O: MetadataOps>(
    storage: *mut MetadataStorage,
    object: Object,
    ar: Object,
    kind: Hp2MetadataKind,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataKind::*;
    match kind {
        MetadataField => unsafe { serialize_field_payload(object, ar, ops) },
        MetadataStruct => unsafe { serialize_struct_payload(storage, object, ar, ops) },
        MetadataState => unsafe { serialize_state_payload(object, ar, ops) },
        MetadataClass => unsafe { serialize_class_payload(object, ar, ops) },
        MetadataFunction => unsafe { serialize_function_payload(object, ar, ops) },
    }
}

/// Runs the canonical reflection metadata archive state machine with direct Rust operations.
///
/// # Safety
/// `storage`, object, archive, and all locations passed to `ops` must remain live
/// under callback reentry. No references into them may be retained across hooks.
pub unsafe fn serialize_with<O: MetadataOps>(
    storage: *mut MetadataStorage,
    object: Object,
    ar: Object,
    kind: Hp2MetadataKind,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2MetadataInteger::MetadataArchiveLoading;
    use Hp2MetadataKind::*;
    unsafe {
        // The original loading-layout scope is deliberately NOT restored on error.
        let saved_layout = if kind == MetadataClass {
            let saved = ptr::addr_of!((*storage).loading_layout).read();
            if ops.integer(ar, MetadataArchiveLoading)? != 0
                && (ptr::addr_of!((*object.cast::<UObject>()).object_flags).read_unaligned()
                    & METADATA_RF_NATIVE
                    != 0
                    || !ptr::addr_of!((*object.cast::<UClass>()).class_constructor)
                        .read_unaligned()
                        .is_null())
            {
                ptr::addr_of_mut!((*storage).loading_layout).write(object);
            }
            saved
        } else {
            std::ptr::null_mut()
        };
        if ops.flatten_superclasses() {
            ops.super_serialize(object, ar, MetadataField)?;
            let hierarchy: &[Hp2MetadataKind] = match kind {
                MetadataField => &[MetadataField],
                MetadataStruct => &[MetadataField, MetadataStruct],
                MetadataState => &[MetadataField, MetadataStruct, MetadataState],
                MetadataClass => &[MetadataField, MetadataStruct, MetadataState, MetadataClass],
                MetadataFunction => &[MetadataField, MetadataStruct, MetadataFunction],
            };
            for serialized_kind in hierarchy {
                if *serialized_kind == MetadataClass {
                    ptr::addr_of_mut!((*storage).loading_layout).write(saved_layout);
                }
                serialize_kind_fields(storage, object, ar, *serialized_kind, ops)?;
            }
        } else {
            ops.super_serialize(object, ar, kind)?;
            if kind == MetadataClass {
                ptr::addr_of_mut!((*storage).loading_layout).write(saved_layout);
            }
            serialize_kind_fields(storage, object, ar, kind, ops)?;
        }
        Ok(())
    }
}
/// Preserves the C-compatible status boundary for mixed native callers.
///
/// # Safety
/// Native storage, archive and service callbacks must remain valid through
/// reentry; callbacks must capture exceptions and return status codes.
pub unsafe fn serialize(
    storage: *mut MetadataStorage,
    object: Object,
    ar: Object,
    kind: Hp2MetadataKind,
    services: &Hp2ClassMetadataServices,
) -> Result {
    let ops = LegacyMetadataOps(*services);
    unsafe { serialize_with(storage, object, ar, kind, &ops) }
}

/// # Safety
/// State fields remain live across native superclass linking and hash writes.
pub unsafe fn state_link(
    object: Object,
    ar: Object,
    props: i32,
    s: &Hp2ClassMetadataServices,
) -> Result {
    use Hp2MetadataInteger::*;
    use Hp2MetadataPointer::*;
    let services = *s;
    let s = &services;
    unsafe {
        // Qualified UStruct::Link, not virtual redispatch to UState::Link.
        status((s.link_struct)(object, ar, props))?;
        let parent = pointer(s, object, MetadataSuperState)?;
        let hash = ptr::addr_of_mut!((*object.cast::<UState>()).vf_hash).cast::<Object>();
        if parent.is_null() {
            ptr::write_bytes(hash, 0, METADATA_HASH_COUNT as usize);
        } else {
            let source = ptr::addr_of!((*parent.cast::<UState>()).vf_hash).cast::<Object>();
            // `parent` may alias the destination in a reentrant native leaf.
            ptr::copy(source, hash, METADATA_HASH_COUNT as usize);
        }
        let mut owner = object;
        let mut field = ptr::addr_of!((*owner.cast::<UStruct>()).children).read_unaligned();
        loop {
            // Match TFieldIterator<UStruct>, including its increment scan before
            // the next owned-field condition is evaluated.
            while field.is_null() || (s.integer)(field, MetadataIsStruct) == 0 {
                if field.is_null() {
                    owner = pointer(s, owner, MetadataInheritanceSuper)?;
                    if owner.is_null() {
                        return Ok(());
                    }
                    field = ptr::addr_of!((*owner.cast::<UStruct>()).children).read_unaligned();
                } else {
                    field = ptr::addr_of!((*field.cast::<UField>()).next).read_unaligned();
                }
            }
            if ptr::addr_of!((*field.cast::<UObject>()).outer).read_unaligned() != object {
                return Ok(());
            }
            let index =
                i32::from_ne_bytes(ptr::addr_of!((*field.cast::<UObject>()).name).read_unaligned())
                    & (METADATA_HASH_COUNT - 1);
            let bucket = hash.add(index as usize);
            ptr::addr_of_mut!((*field.cast::<UField>()).hash_next)
                .write_unaligned(bucket.read_unaligned());
            bucket.write_unaligned(field);
            field = ptr::addr_of!((*field.cast::<UField>()).next).read_unaligned();
        }
    }
}
/// # Safety
/// Function bindings and callbacks must remain live, including export lookup.
pub unsafe fn function_bind(object: Object, s: &Hp2ClassMetadataServices) -> Result {
    use Hp2MetadataCheck::*;
    let services = *s;
    let s = &services;
    unsafe {
        let function = object.cast::<UFunction>();
        let handler = ptr::addr_of_mut!((*function).func).cast::<Option<Hp2VmNativeHandler>>();
        if ptr::addr_of!((*function).function_flags).read_unaligned() & METADATA_FUNC_NATIVE == 0 {
            status((s.check)(object, MetadataScriptNativeZero))?;
            handler.write_unaligned(Some(s.script_handler));
        } else if ptr::addr_of!((*function).i_native).read_unaligned() != 0 {
            status((s.check)(object, MetadataNativeIndexValid))?;
            status((s.check)(object, MetadataNativeFunctionPresent))?;
            let index = ptr::addr_of!((*function).i_native).read_unaligned();
            handler.write_unaligned(
                s.native_handlers
                    .cast::<Option<Hp2VmNativeHandler>>()
                    .add(index as usize)
                    .read_unaligned(),
            );
        } else {
            let mut address = std::ptr::null_mut();
            status((s.lookup_dynamic)(object, &mut address))?;
            if !address.is_null() {
                handler.write_unaligned(
                    address
                        .cast::<Option<Hp2VmNativeHandler>>()
                        .read_unaligned(),
                );
            }
        }
        Ok(())
    }
}
