//! Engine-thread property construction, registration and UProperty archive ordering.
//!
//! Like vm_registry, this storage is usable before appInit and is never reset.
//! It deliberately has no allocation, lazy initializer, or synchronization.

use hp2_abi::engine::generated::{
    UBoolProperty, UByteProperty, UClassProperty, UField, UObjectProperty, UProperty, UStruct,
    UStructProperty,
};
use std::{ffi::c_void, ptr};

type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;
const CAPACITY: usize = 16384;
const CPF_NET: u32 = 0x20;

#[derive(Clone, Copy)]
struct NativeOffset {
    owner: Object,
    name: i32,
    offset: i32,
}

static mut OFFSETS: [NativeOffset; CAPACITY] = [NativeOffset {
    owner: std::ptr::null_mut(),
    name: 0,
    offset: 0,
}; CAPACITY];
static mut COUNT: usize = 0;

/// Append without deduplication: the earliest owner/name registration wins.
/// False means the caller MUST invoke the native capacity assertion, not ignore
/// the registration. In particular AddCppProperty must already have happened.
///
/// # Safety
/// Calls and consumers must be serialized on the engine thread. The owner is
/// an opaque identity, never dereferenced. No registry references may survive
/// a native callback. Valid before appInit; there is intentionally no reset.
pub unsafe fn register(owner: Object, name: i32, offset: i32) -> bool {
    unsafe {
        let count = COUNT;
        if count == CAPACITY {
            return false;
        }
        (&raw mut OFFSETS)
            .cast::<NativeOffset>()
            .add(count)
            .write(NativeOffset {
                owner,
                name,
                offset,
            });
        COUNT = count + 1;
        true
    }
}

unsafe fn lookup(owner: Object, name: i32) -> Option<i32> {
    unsafe {
        for index in 0..COUNT {
            let entry = (&raw const OFFSETS)
                .cast::<NativeOffset>()
                .add(index)
                .read();
            if entry.owner == owner && entry.name == name {
                return Some(entry.offset);
            }
        }
        None
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PropertyConstructionServices {
    pub add_cpp_property: unsafe extern "C" fn(Object) -> i32,
    pub registration_key: unsafe extern "C" fn(Object, *mut Object, *mut i32) -> i32,
    pub capacity_assert: unsafe extern "C" fn() -> i32,
}

/// Initialize only the default constructor's explicitly owned scalar slots.
/// # Safety
/// `property` addresses writable compiler-probed UProperty storage. Its base and
/// typed members retain their native lifetimes; other bytes may be uninitialized.
pub unsafe fn initialize_default_data(property: *mut UProperty) {
    unsafe {
        ptr::addr_of_mut!((*property).array_dim).write_unaligned(1);
        ptr::addr_of_mut!((*property).offset).write_unaligned(0);
        ptr::addr_of_mut!((*property).cpp_property).write_unaligned(0);
    }
}

/// Commit the C++ constructor prefix before native typed Category construction.
/// # Safety
/// `property` addresses writable compiler-probed fields. Category construction
/// must occur after this call and before `finish_cpp_construction`.
pub unsafe fn initialize_cpp_prefix_data(property: *mut UProperty, flags: u32) {
    unsafe {
        ptr::addr_of_mut!((*property).array_dim).write_unaligned(1);
        ptr::addr_of_mut!((*property).property_flags).write_unaligned(flags);
    }
}

/// Finish construction, publish through the native virtual leaf, then register
/// the original input offset under the post-publication owner/name.
/// # Safety
/// Property and callback targets remain live through reentry; callbacks contain
/// native exceptions. All registry access is engine-thread serialized. The table
/// is passed by value so no reference into caller storage survives a native leaf.
pub unsafe fn finish_cpp_construction(
    property: *mut UProperty,
    original_offset: i32,
    services: Hp2PropertyConstructionServices,
) -> Result {
    unsafe {
        ptr::addr_of_mut!((*property).offset).write_unaligned(original_offset);
        ptr::addr_of_mut!((*property).cpp_property).write_unaligned(1);
        status((services.add_cpp_property)(property.cast()))?;
        let mut owner = ptr::null_mut();
        let mut name = 0;
        status((services.registration_key)(
            property.cast(),
            &mut owner,
            &mut name,
        ))?;
        if !register(owner, name, original_offset) {
            status((services.capacity_assert)())?;
        }
        Ok(())
    }
}

/// Prepend a property, preserving the old head before publishing the new head.
/// # Safety
/// Both pointers address writable compiler-probed storage, and `children` is
/// initialized. Access is engine-thread serialized; no native leaf is invoked.
pub unsafe fn add_cpp_property(structure: *mut UStruct, property: *mut UField) {
    unsafe {
        let children = ptr::addr_of!((*structure).children).read_unaligned();
        ptr::addr_of_mut!((*property).next).write_unaligned(children);
        ptr::addr_of_mut!((*structure).children).write_unaligned(property.cast());
    }
}

/// Initialize the C++ byte constructor's Enum slot, including a null input.
/// # Safety
/// `property` addresses writable compiler-probed UByteProperty storage.
pub unsafe fn initialize_byte_data(property: *mut UByteProperty, enumeration: Object) {
    unsafe { ptr::addr_of_mut!((*property).r#enum).write_unaligned(enumeration) };
}

/// Initialize only the C++ bool constructor's BitMask slot.
/// # Safety
/// `property` addresses writable compiler-probed UBoolProperty storage.
pub unsafe fn initialize_bool_data(property: *mut UBoolProperty) {
    unsafe { ptr::addr_of_mut!((*property).bit_mask).write_unaligned(1) };
}

/// Initialize PropertyClass without changing NextReference or base storage.
/// # Safety
/// `property` addresses writable compiler-probed UObjectProperty storage.
pub unsafe fn initialize_object_data(property: *mut UObjectProperty, class: Object) {
    unsafe { ptr::addr_of_mut!((*property).property_class).write_unaligned(class) };
}

/// Initialize MetaClass without changing the UObjectProperty base's class.
/// # Safety
/// `property` addresses writable compiler-probed UClassProperty storage.
pub unsafe fn initialize_class_data(property: *mut UClassProperty, metaclass: Object) {
    unsafe { ptr::addr_of_mut!((*property).meta_class).write_unaligned(metaclass) };
}

/// Initialize Struct without changing NextStruct or base storage.
/// # Safety
/// `property` addresses writable compiler-probed UStructProperty storage.
pub unsafe fn initialize_struct_data(property: *mut UStructProperty, structure: Object) {
    unsafe { ptr::addr_of_mut!((*property).r#struct).write_unaligned(structure) };
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2NativePropertyInteger {
    Offset,
    CppProperty,
    Flags,
    Loading,
    Version,
    Name,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2NativePropertyWire {
    Super,
    ArrayDim,
    Flags,
    Category,
    RepOffset,
}

#[repr(C)]
pub struct Hp2NativePropertyServices {
    pub integer: unsafe extern "C" fn(Object, Object, Hp2NativePropertyInteger, *mut i32) -> i32,
    pub owner: unsafe extern "C" fn(Object, *mut Object) -> i32,
    pub archive: unsafe extern "C" fn(Object, Object, Hp2NativePropertyWire) -> i32,
    pub set_flags: unsafe extern "C" fn(Object, u32) -> i32,
    pub publish: unsafe extern "C" fn(Object, i32, i32) -> i32,
}

fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}

/// Archive leaves shared by legacy UProperty and Rust-owned property records.
pub(crate) trait PropertyRecordOps {
    type Error;
    fn integer(&self, field: Hp2NativePropertyInteger) -> std::result::Result<i32, Self::Error>;
    fn archive(&self, field: Hp2NativePropertyWire) -> std::result::Result<(), Self::Error>;
    fn registered_offset(&self) -> std::result::Result<Option<i32>, Self::Error>;
    fn set_flags(&self, flags: u32) -> std::result::Result<(), Self::Error>;
    fn publish(&self, offset: i32, cpp: i32) -> std::result::Result<(), Self::Error>;
}

pub(crate) fn serialize_with<O: PropertyRecordOps>(ops: &O) -> std::result::Result<(), O::Error> {
    use Hp2NativePropertyInteger as I;
    use Hp2NativePropertyWire as W;
    let saved_offset = ops.integer(I::Offset)?;
    let was_cpp = ops.integer(I::CppProperty)?;
    ops.archive(W::Super)?;
    ops.archive(W::ArrayDim)?;
    ops.archive(W::Flags)?;
    ops.archive(W::Category)?;
    let registered = if ops.integer(I::Loading)? != 0 { ops.registered_offset()? } else { None };
    if ops.integer(I::Flags)? as u32 & CPF_NET != 0 {
        ops.archive(W::RepOffset)?;
    }
    if ops.integer(I::Version)? <= 61 {
        let flags = ops.integer(I::Flags)? as u32;
        ops.set_flags(flags & !0x0008_0040)?;
    }
    if ops.integer(I::Loading)? != 0 {
        let offset = registered.unwrap_or(if was_cpp != 0 { saved_offset } else { 0 });
        ops.publish(offset, i32::from(registered.is_some() || was_cpp != 0))?;
    }
    Ok(())
}

struct LegacyPropertyRecordOps<'a> {
    property: Object,
    archive: Object,
    services: &'a Hp2NativePropertyServices,
}

impl PropertyRecordOps for LegacyPropertyRecordOps<'_> {
    type Error = i32;
    fn integer(&self, field: Hp2NativePropertyInteger) -> Result<i32> {
        let mut value = 0;
        unsafe { status((self.services.integer)(self.property, self.archive, field, &mut value))?; }
        Ok(value)
    }
    fn archive(&self, field: Hp2NativePropertyWire) -> Result {
        unsafe { status((self.services.archive)(self.property, self.archive, field)) }
    }
    fn registered_offset(&self) -> Result<Option<i32>> {
        if unsafe { COUNT } == 0 { return Ok(None); }
        let mut owner = ptr::null_mut();
        unsafe { status((self.services.owner)(self.property, &mut owner))?; }
        let name = self.integer(Hp2NativePropertyInteger::Name)?;
        Ok(unsafe { lookup(owner, name) })
    }
    fn set_flags(&self, flags: u32) -> Result {
        unsafe { status((self.services.set_flags)(self.property, flags)) }
    }
    fn publish(&self, offset: i32, cpp: i32) -> Result {
        unsafe { status((self.services.publish)(self.property, offset, cpp)) }
    }
}

/// Serialize live fields, retaining the native partial-state/error boundaries.
///
/// # Safety
/// Property/archive/services stay live throughout recursive native callbacks.
/// Callbacks contain native exceptions and return the first failure status.
/// Registry access is engine-thread serialized, including nested registration.
/// No Rust reference into property or registry storage is held across a leaf.
pub unsafe fn serialize(
    property: Object,
    archive: Object,
    services: &Hp2NativePropertyServices,
) -> Result {
    serialize_with(&LegacyPropertyRecordOps { property, archive, services })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Property {
        name: i32,
        offset: i32,
        cpp: i32,
        flags: u32,
        constructor_link: bool,
        registration_stage: i32,
        registered_offset: i32,
        fail_rep: bool,
        publications: i32,
    }

    unsafe extern "C" fn integer(
        p: Object,
        _: Object,
        field: Hp2NativePropertyInteger,
        result: *mut i32,
    ) -> i32 {
        let p = unsafe { &*p.cast::<Property>() };
        unsafe {
            result.write(match field {
                Hp2NativePropertyInteger::Offset => p.offset,
                Hp2NativePropertyInteger::CppProperty => p.cpp,
                Hp2NativePropertyInteger::Flags => p.flags as i32,
                Hp2NativePropertyInteger::Loading => 1,
                Hp2NativePropertyInteger::Version => 61,
                Hp2NativePropertyInteger::Name => p.name,
            });
        }
        0
    }

    unsafe extern "C" fn owner(p: Object, result: *mut Object) -> i32 {
        unsafe { result.write(p) };
        0
    }

    unsafe extern "C" fn archive(p: Object, _: Object, field: Hp2NativePropertyWire) -> i32 {
        let property = unsafe { &mut *p.cast::<Property>() };
        let stage = match field {
            Hp2NativePropertyWire::Super => {
                property.offset = 999;
                property.cpp = 0;
                0
            }
            Hp2NativePropertyWire::Category => 1,
            Hp2NativePropertyWire::RepOffset => 2,
            _ => 0,
        };
        if stage != 0 && stage == property.registration_stage {
            // A later duplicate must not replace even a zero first offset.
            if !unsafe { register(p, property.name, property.registered_offset) }
                || !unsafe { register(p, property.name, 787) }
            {
                return 91;
            }
        }
        if stage == 2 {
            property.flags = 0x0008_0060;
            if property.fail_rep {
                return 73;
            }
        }
        0
    }

    unsafe extern "C" fn set_flags(p: Object, flags: u32) -> i32 {
        unsafe { (*p.cast::<Property>()).flags = flags };
        0
    }

    unsafe extern "C" fn publish(p: Object, offset: i32, cpp: i32) -> i32 {
        let p = unsafe { &mut *p.cast::<Property>() };
        p.offset = offset;
        p.cpp = cpp;
        p.constructor_link = false;
        p.publications += 1;
        0
    }

    // One test owns all global-registry access, with distinct names and no
    // reset. Other unit tests do not call this engine-thread-only registry.
    #[test]
    fn archive_lookup_timing_and_failure_preserve_native_partial_state() {
        let services = Hp2NativePropertyServices {
            integer,
            owner,
            archive,
            set_flags,
            publish,
        };
        for (name, stage, offset, fail) in [
            (-901, 1, 0, false),
            (-902, 1, -37, false),
            (-903, 2, -48, false),
            (-904, 2, -59, true),
        ] {
            let mut property = Property {
                name,
                offset: -83,
                cpp: 1,
                flags: CPF_NET,
                constructor_link: true,
                registration_stage: stage,
                registered_offset: offset,
                fail_rep: fail,
                publications: 0,
            };
            let object = (&raw mut property).cast();
            let result = unsafe { serialize(object, std::ptr::null_mut(), &services) };
            if fail {
                assert_eq!(result, Err(73));
                assert_eq!((property.offset, property.cpp), (999, 0));
                assert_eq!(property.flags, 0x0008_0060);
                assert!(property.constructor_link);
                assert_eq!(property.publications, 0);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(property.offset, if stage == 1 { offset } else { -83 });
                assert_eq!(property.cpp, 1);
                assert_eq!(property.flags, CPF_NET);
                assert!(!property.constructor_link);
                assert_eq!(property.publications, 1);
            }
            assert_eq!(unsafe { lookup(object, name) }, Some(offset));
        }
    }
}
