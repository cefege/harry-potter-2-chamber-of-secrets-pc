//! Reflected metadata storage policy over compiler-probed native fields.
//! Rust owns raw class arrays; native leaves retain typed property/object work.

use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_abi::engine::generated::{
    FDependency, TArrayRaw, UClass, UField, UObject, UState, UStruct,
};
use std::{ffi::c_void, mem::size_of, ptr};

const EMPTY_TAG: [u32; 1] = [0];

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ClassLifecycleServices {
    pub bind: unsafe extern "C" fn(*mut c_void) -> i32,
    pub exit_properties: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub super_destroy: unsafe extern "C" fn(*mut c_void) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

unsafe fn slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

/// Release replication records at the native class member's destruction boundary.
///
/// # Safety
/// `array` must address the initialized, compiler-checked raw slots of a
/// `FClassRepArray`. Callbacks must capture native exceptions and retain valid
/// slots through reentry. `width` must equal the native record size.
pub unsafe fn destroy_class_reps(
    array: *mut TArrayRaw,
    width: i32,
    arrays: Hp2ArrayServices,
    check_after_remove: unsafe extern "C" fn(*mut c_void) -> i32,
) -> Result<(), i32> {
    unsafe {
        let storage = slots(array);
        let count = storage.num.read_unaligned();
        array_storage::remove(storage, 0, count, width, EMPTY_TAG.as_ptr().cast(), arrays)?;
        status(check_after_remove(array.cast()))?;
        array_storage::release(storage, arrays)
    }
}

/// Initialize native/static registration links without touching the object base.
/// # Safety
/// `field` must address writable compiler-probed fields. Static registration
/// preserves the existing superclass; no whole-object representation is read.
pub unsafe fn initialize_field_registration_data(
    field: *mut UField,
    super_field: *mut c_void,
    native: bool,
) {
    unsafe {
        if native {
            ptr::addr_of_mut!((*field).super_field).write_unaligned(super_field);
        }
        ptr::addr_of_mut!((*field).next).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*field).hash_next).write_unaligned(ptr::null_mut());
    }
}

/// Initialize only the ordinary constructor's superclass link.
/// # Safety
/// `field` must address the writable compiler-probed superclass slot.
pub unsafe fn initialize_field_instance_data(field: *mut UField, super_field: *mut c_void) {
    unsafe {
        ptr::addr_of_mut!((*field).super_field).write_unaligned(super_field);
    }
}

/// Initialize registration scalars and links, leaving Script's typed lifetime
/// and the empty native FName default constructor's FriendlyName bytes alone.
/// # Safety
/// `structure` must address writable compiler-probed fields; `property_alignment`
/// must be the native PROPERTY_ALIGNMENT constant.
pub unsafe fn initialize_struct_registration_data(
    structure: *mut UStruct,
    size: i32,
    property_alignment: i32,
) {
    unsafe {
        ptr::addr_of_mut!((*structure).script_text).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).children).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).properties_size).write_unaligned(size);
        ptr::addr_of_mut!((*structure).properties_align).write_unaligned(property_alignment);
        ptr::addr_of_mut!((*structure).text_pos).write_unaligned(0);
        ptr::addr_of_mut!((*structure).line).write_unaligned(0);
        ptr::addr_of_mut!((*structure).ref_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).struct_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).property_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).config_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).constructor_link).write_unaligned(ptr::null_mut());
    }
}

/// Inherit ordinary structure size/alignment and copy the already-initialized
/// root UObject name, preserving all other fields including typed members.
/// # Safety
/// `structure` must address writable compiler-probed fields and initialized
/// UObject name bytes. A nonnull `superclass` must have readable size/alignment
/// fields and may alias `structure`. `property_alignment` is PROPERTY_ALIGNMENT.
pub unsafe fn initialize_struct_instance_data(
    structure: *mut UStruct,
    superclass: *mut UStruct,
    property_alignment: i32,
) {
    unsafe {
        let (size, alignment) = if superclass.is_null() {
            (0, property_alignment)
        } else {
            (
                ptr::addr_of!((*superclass).properties_size).read_unaligned(),
                ptr::addr_of!((*superclass).properties_align).read_unaligned(),
            )
        };
        let name = ptr::addr_of!((*structure.cast::<UObject>()).name).read_unaligned();
        ptr::addr_of_mut!((*structure).properties_size).write_unaligned(size);
        ptr::addr_of_mut!((*structure).properties_align).write_unaligned(alignment);
        ptr::addr_of_mut!((*structure).friendly_name).write_unaligned(name);
    }
}

/// Initialize registration state scalars without touching VfHash.
/// # Safety
/// `state` must address writable compiler-probed fields.
pub unsafe fn initialize_state_registration_data(state: *mut UState) {
    unsafe {
        ptr::addr_of_mut!((*state).probe_mask).write_unaligned(0);
        ptr::addr_of_mut!((*state).ignore_mask).write_unaligned(0);
        ptr::addr_of_mut!((*state).state_flags).write_unaligned(0);
        ptr::addr_of_mut!((*state).label_table_offset).write_unaligned(0);
    }
}

/// Initialize registration class data without constructing typed members.
/// # Safety
/// `class` must address writable compiler-probed fields. `guid` must contain four
/// readable words (it may alias ClassGuid). `parsed_compiled_mask` is the native
/// CLASS_Parsed | CLASS_Compiled mask. The config slot must accommodate a whole
/// pointer, including the native overlap with ClassCRC. Callbacks are nullable
/// checked native leaves and are stored, never invoked here.
pub unsafe fn initialize_class_registration_data(
    class: *mut UClass,
    native: bool,
    class_flags: u32,
    parsed_compiled_mask: u32,
    within: *mut c_void,
    guid: *const u32,
    config: *const u32,
    constructor: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    static_constructor: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
) {
    unsafe {
        let guid = guid.cast::<[u32; 4]>().read_unaligned();
        ptr::addr_of_mut!((*class).class_flags).write_unaligned(class_flags | parsed_compiled_mask);
        ptr::addr_of_mut!((*class).class_unique).write_unaligned(0);
        ptr::addr_of_mut!((*class).class_guid).write_unaligned(guid);
        if native {
            ptr::addr_of_mut!((*class).class_within).write_unaligned(within);
            ptr::addr_of_mut!((*class).class_crc).write_unaligned(0);
        }
        ptr::addr_of_mut!((*class).class_constructor)
            .write_unaligned(constructor.map_or(ptr::null_mut(), |leaf| leaf as *mut c_void));
        ptr::addr_of_mut!((*class).class_static_constructor).write_unaligned(
            static_constructor.map_or(ptr::null_mut(), |leaf| leaf as *mut c_void),
        );
        // The native registration protocol stashes a TCHAR pointer across the
        // four-byte FName slot and, on 64-bit hosts, the adjacent ClassCRC.
        ptr::addr_of_mut!((*class).class_config_name)
            .cast::<*const u32>()
            .write_unaligned(config);
    }
}

/// Initialize only the fields shared by the ordinary and superclass constructors.
/// # Safety
/// `class` must address writable compiler-probed fields; no whole-object Rust
/// reference or initialized C++ object representation is required.
pub unsafe fn initialize_class_instance_data(class: *mut UClass, root_class: *mut c_void) {
    unsafe {
        ptr::addr_of_mut!((*class).class_within).write_unaligned(root_class);
        ptr::addr_of_mut!((*class).class_crc).write_unaligned(0);
    }
}

/// Empty a structure's script byte array through the shared array owner.
///
/// # Safety
/// `structure` must address an initialized `UStruct::script` slot. `arrays`
/// must be the allocator paired with the script buffer's allocation.
pub unsafe fn destroy_struct_script(
    structure: *mut UStruct,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        array_storage::empty(
            slots(ptr::addr_of_mut!((*structure).script)),
            0,
            1,
            EMPTY_TAG.as_ptr().cast(),
            arrays,
        )
    }
}

/// Replace raw default storage, without initializing its object header.
/// # Safety
/// Class fields and allocator must satisfy `array_storage`'s live-slot contract.
/// Callbacks may reenter and change PropertiesSize, counts, or buffers.
pub unsafe fn reset_defaults(class: *mut UClass, arrays: Hp2ArrayServices) -> Result<(), i32> {
    unsafe {
        let defaults = slots(ptr::addr_of_mut!((*class).defaults));
        array_storage::empty(
            defaults,
            ptr::addr_of!((*class).properties_size).read_unaligned(),
            1,
            EMPTY_TAG.as_ptr().cast(),
            arrays,
        )?;
        array_storage::add(
            defaults,
            ptr::addr_of!((*class).properties_size).read_unaligned(),
            1,
            EMPTY_TAG.as_ptr().cast(),
            arrays,
        )?;
    }
    Ok(())
}

/// Reproduce byte TArray assignment, coalescing only callback-free insertions.
unsafe fn copy_defaults(
    destination: *mut TArrayRaw,
    source: *mut TArrayRaw,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    if destination == source {
        return Ok(());
    }
    unsafe {
        let dst = slots(destination);
        let src = slots(source);
        array_storage::empty(dst, src.num.read(), 1, EMPTY_TAG.as_ptr().cast(), arrays)?;
        let mut i = 0;
        while i < src.num.read() {
            let count = (src.num.read() - i).min(dst.max.read() - dst.num.read());
            if count > 0 {
                // No callback can observe the intermediate byte placements. Keep
                // the native committed count before the next allocation/bind.
                let index = dst.num.read();
                dst.num.write(index + count);
                ptr::copy_nonoverlapping(
                    src.data.read_unaligned().cast::<u8>().add(i as usize),
                    dst.data.read_unaligned().cast::<u8>().add(index as usize),
                    count as usize,
                );
                i += count;
            } else {
                // Placement-new allocates before evaluating Other(i). Reentry
                // may replace either buffer, either count, or the destination's
                // prefix. Retain only the returned placement index across it.
                let index = array_storage::add(dst, 1, 1, EMPTY_TAG.as_ptr().cast(), arrays)?;
                let value = src
                    .data
                    .read_unaligned()
                    .cast::<u8>()
                    .add(i as usize)
                    .read();
                dst.data
                    .read_unaligned()
                    .cast::<u8>()
                    .add(index as usize)
                    .write(value);
                i += 1;
            }
        }
    }
    Ok(())
}

/// Inherit the live superclass's within/default fields and then bind the class.
/// # Safety
/// Superclass and both byte arrays must remain valid through callbacks. Distinct
/// arrays must have independent storage, as required by native TArray assignment.
/// All native leaves capture exceptions and report their status.
pub unsafe fn inherit_defaults(
    class: *mut UClass,
    arrays: Hp2ArrayServices,
    lifecycle: Hp2ClassLifecycleServices,
) -> Result<(), i32> {
    unsafe {
        let parent = ptr::addr_of!((*class).super_field)
            .read_unaligned()
            .cast::<UClass>();
        if !parent.is_null() {
            ptr::addr_of_mut!((*class).class_within)
                .write_unaligned(ptr::addr_of!((*parent).class_within).read_unaligned());
            copy_defaults(
                ptr::addr_of_mut!((*class).defaults),
                ptr::addr_of_mut!((*parent).defaults),
                arrays,
            )?;
            status((lifecycle.bind)(class.cast()))?;
        }
    }
    Ok(())
}

/// One ordered action in `UClass::Destroy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClassDestroyStep {
    ClearNetFields,
    ClearDependencies,
    ClearPackageImports,
    ExitDefaultProperties,
    ClearDefaults,
    ClearDefaultText,
    DestroySuper,
}

/// Executes the shared UClass teardown order, stopping at the first error.
///
/// Neither adapter may free default storage or destroy the superclass after
/// property destruction fails. Independent owner-level cleanup can still
/// collect the returned error without advancing these dependent steps.
///
/// # Safety
/// `class` is a live layout-compatible class allocation. The operation closure
/// must obey each step's element-lifetime and reentry contract.
pub(crate) unsafe fn destroy_class_with<E>(
    class: *mut UClass,
    mut operation: impl FnMut(ClassDestroyStep, *mut UClass) -> Result<(), E>,
) -> Result<(), E> {
    use ClassDestroyStep::*;
    for step in [
        ClearNetFields,
        ClearDependencies,
        ClearPackageImports,
        ExitDefaultProperties,
        ClearDefaults,
        ClearDefaultText,
        DestroySuper,
    ] {
        operation(step, class)?;
    }
    Ok(())
}

/// Legacy native adapter to the shared `UClass::Destroy` ordering.
///
/// # Safety
/// See [`destroy_class_with`]. Service callbacks must not unwind.
pub unsafe fn destroy_class(
    class: *mut UClass,
    arrays: Hp2ArrayServices,
    lifecycle: Hp2ClassLifecycleServices,
) -> Result<(), i32> {
    use ClassDestroyStep::*;
    unsafe {
        destroy_class_with(class, |step, class| {
            let tag = EMPTY_TAG.as_ptr().cast();
            match step {
                ClearNetFields => array_storage::empty(
                    slots(ptr::addr_of_mut!((*class).net_fields)), 0,
                    size_of::<*mut c_void>() as i32, tag, arrays,
                ),
                ClearDependencies => array_storage::empty(
                    slots(ptr::addr_of_mut!((*class).dependencies)), 0,
                    size_of::<FDependency>() as i32, tag, arrays,
                ),
                ClearPackageImports => array_storage::empty(
                    slots(ptr::addr_of_mut!((*class).package_imports)), 0, 4, tag, arrays,
                ),
                ExitDefaultProperties => status((lifecycle.exit_properties)(
                    ptr::addr_of!((*class).defaults.data).read_unaligned(), class.cast(),
                )),
                ClearDefaults => array_storage::empty(
                    slots(ptr::addr_of_mut!((*class).defaults)), 0, 1, tag, arrays,
                ),
                ClearDefaultText => array_storage::empty(
                    slots(ptr::addr_of_mut!((*class.cast::<crate::core_records::ClassRecord>()).default_prop_text)),
                    0,
                    size_of::<crate::ansi::Tchar>() as i32,
                    tag,
                    arrays,
                ),
                DestroySuper => status((lifecycle.super_destroy)(class.cast())),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn struct_destroy_frees_script_storage_and_resets_the_array_header() {
        let mut structure = MaybeUninit::<UStruct>::zeroed();
        let structure_ptr = structure.as_mut_ptr();
        let script = unsafe { ptr::addr_of_mut!((*structure_ptr).script) };
        unsafe { script.write(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
        let arrays = array_storage::system_allocator();
        unsafe {
            array_storage::add(slots(script), 8, 1, EMPTY_TAG.as_ptr().cast(), arrays)
                .expect("script storage allocation");
            assert!(!(*script).data.is_null());
            destroy_struct_script(structure_ptr, arrays).expect("script storage destruction");
            assert_eq!((*script).data, ptr::null_mut());
            assert_eq!(((*script).num, (*script).max), (0, 0));
        }
    }

    #[test]
    fn reset_defaults_allocates_the_registered_class_size() {
        let mut class = MaybeUninit::<UClass>::zeroed();
        let class_ptr = class.as_mut_ptr();
        unsafe {
            ptr::addr_of_mut!((*class_ptr).properties_size).write(12);
            ptr::addr_of_mut!((*class_ptr).defaults).write(TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            });
        }
        let arrays = array_storage::system_allocator();
        unsafe {
            reset_defaults(class_ptr, arrays).expect("default storage allocation");
            let defaults = slots(ptr::addr_of_mut!((*class_ptr).defaults));
            assert!(!defaults.data.read_unaligned().is_null());
            assert_eq!((defaults.num.read(), defaults.max.read()), (12, 12));
            array_storage::release(defaults, arrays).expect("default storage cleanup");
        }
    }


    unsafe extern "C" fn lifecycle_success(_: *mut c_void) -> i32 { 0 }
    unsafe extern "C" fn exit_properties_success(_: *mut c_void, _: *mut c_void) -> i32 { 0 }

    #[test]
    fn class_destroy_releases_default_text_as_one_character_array() {
        let mut class = MaybeUninit::<UClass>::zeroed();
        let class_ptr = class.as_mut_ptr();
        let default_text = unsafe {
            ptr::addr_of_mut!((*class_ptr.cast::<crate::core_records::ClassRecord>()).default_prop_text)
        };
        unsafe { default_text.write(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
        let arrays = array_storage::system_allocator();
        unsafe {
            array_storage::add(
                slots(default_text),
                4,
                size_of::<crate::ansi::Tchar>() as i32,
                EMPTY_TAG.as_ptr().cast(),
                arrays,
            )
            .expect("default text allocation");
            destroy_class(
                class_ptr,
                arrays,
                Hp2ClassLifecycleServices {
                    bind: lifecycle_success,
                    exit_properties: exit_properties_success,
                    super_destroy: lifecycle_success,
                },
            )
            .expect("class destruction");
            let text = slots(default_text);
            assert!(text.data.read_unaligned().is_null());
            assert_eq!((text.num.read(), text.max.read()), (0, 0));
        }
    }
    #[test]
    fn class_destroy_stops_before_dependent_cleanup_on_the_first_error() {
        use ClassDestroyStep::*;
        let mut steps = Vec::new();
        let result = unsafe {
            destroy_class_with(ptr::null_mut(), |step, _| {
                steps.push(step);
                match step {
                    ClearDependencies => Err(11),
                    DestroySuper => Err(22),
                    _ => Ok(()),
                }
            })
        };
        assert_eq!(
            steps,
            [
                ClearNetFields,
                ClearDependencies,
            ],
        );
        assert_eq!(result, Err(11));
    }
}
