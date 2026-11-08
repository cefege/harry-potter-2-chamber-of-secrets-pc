//! UObject payload initialization, constructor-link traversal and default-object policy.
//!
//! Native code supplies layout reads, checked diagnostics and typed virtual hooks.
//! All traversal uses live links; no native object is borrowed over a callback.

use std::ffi::c_void;

#[repr(C)]
pub struct Hp2ObjectPropertiesServices {
    pub host_size: unsafe extern "C" fn(*mut i32) -> i32,
    /// Kind 0: DataCount >= UObjectSize; 1: DefaultsCount >= Inited;
    /// 2: DefaultsCount <= DataCount. The latter two retain checkSlow semantics.
    pub check_bounds: unsafe extern "C" fn(i32, i32, i32) -> i32,
    pub defaults_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub defaults_data: unsafe extern "C" fn(*mut c_void, *mut *mut u8) -> i32,
    pub constructor_link: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub constructor_next: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub offset: unsafe extern "C" fn(*mut c_void) -> i32,
    pub size: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub invalid_offset: unsafe extern "C" fn(*mut c_void) -> i32,
    pub copy_complete: unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
}

/// Failure location, written before work, so the native shell can reconstruct
/// the original guardSlow/InitProperty diagnostics after unwinding Rust.
#[repr(C)]
pub struct Hp2ObjectPropertiesTrace {
    /// 0: entry, 1: FindDefaults, 2: DefaultsFill, 3: ZeroFill, 4: ConstructProperties.
    pub phase: i32,
    pub property: *mut c_void,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// # Safety
/// Data/defaults must satisfy the native byte-storage and property-layout
/// contract. Services and trace must remain valid through reentry. Callbacks
/// capture exceptions instead of unwinding. Legacy invalid sizes/pointers are
/// not made valid when the native check/checkSlow configuration disables checks.
pub unsafe fn init_properties(
    data: *mut u8,
    data_count: i32,
    defaults_class: *mut c_void,
    mut defaults: *mut u8,
    mut defaults_count: i32,
    services: &Hp2ObjectPropertiesServices,
    trace: *mut Hp2ObjectPropertiesTrace,
) -> Result<(), i32> {
    unsafe {
        (*trace).phase = 0;
        (*trace).property = std::ptr::null_mut();
        let mut host_size = 0;
        status((services.host_size)(&mut host_size))?;
        status((services.check_bounds)(0, data_count, host_size))?;
        let mut inited = host_size;

        (*trace).phase = 1;
        if defaults.is_null()
            && !defaults_class.is_null()
            && (services.defaults_count)(defaults_class) != 0
        {
            status((services.defaults_data)(defaults_class, &mut defaults))?;
            // The accessor may reenter and change the live array count.
            defaults_count = (services.defaults_count)(defaults_class);
        }

        (*trace).phase = 2;
        if !defaults.is_null() {
            status((services.check_bounds)(1, defaults_count, inited))?;
            status((services.check_bounds)(2, defaults_count, data_count))?;
            // Use the same primitive as appMemcpy, including its legacy alias
            // contract, without materializing a payload snapshot.
            crate::ansi_math::memcpy(
                data.wrapping_offset(inited as isize).cast(),
                defaults.wrapping_offset(inited as isize).cast(),
                defaults_count.wrapping_sub(inited),
            );
            inited = defaults_count;
        }

        (*trace).phase = 3;
        if inited < data_count {
            std::ptr::write_bytes(
                data.wrapping_offset(inited as isize),
                0,
                data_count.wrapping_sub(inited) as usize,
            );
        }

        (*trace).phase = 4;
        if !defaults_class.is_null() {
            let mut property = (services.constructor_link)(defaults_class);
            while !property.is_null() {
                (*trace).property = property;
                if (services.offset)(property) < host_size {
                    status((services.invalid_offset)(property))?;
                }
                // Do not gate this on defaults being nonnull: the original
                // explicit count and diagnostic ordering are intentional.
                if (services.offset)(property) < defaults_count {
                    let mut size = 0;
                    status((services.size)(property, &mut size))?;
                    std::ptr::write_bytes(
                        data.wrapping_offset((services.offset)(property) as isize),
                        0,
                        size as usize,
                    );
                    status((services.copy_complete)(
                        property,
                        data.wrapping_offset((services.offset)(property) as isize),
                        defaults.wrapping_offset((services.offset)(property) as isize),
                    ))?;
                }
                // Hooks may replace the current node's successor.
                property = (services.constructor_next)(property);
            }
        }
        Ok(())
    }
}

/// # Safety
/// Class, data and linked properties must remain live as required by the native
/// traversal. Current is a writable diagnostic slot, not borrowed engine data.
pub unsafe fn exit_properties(
    data: *mut u8,
    class: *mut c_void,
    services: &Hp2ObjectPropertiesServices,
    current: *mut *mut c_void,
) -> Result<(), i32> {
    unsafe {
        let mut property = (services.constructor_link)(class);
        *current = property;
        while !property.is_null() {
            if (services.offset)(property) != 0 {
                status((services.destroy)(
                    property,
                    data.wrapping_offset((services.offset)(property) as isize),
                ))?;
            }
            property = (services.constructor_next)(property);
            *current = property;
        }
        Ok(())
    }
}

/// Typed Core property initialization shared by the legacy service adapter and
/// the direct Rust owner. The direct owner does not byte-copy the property tail:
/// dynamic values are initialized exclusively by `copy_complete`.
///
/// # Safety
/// The supplied pointers and property links remain valid for their individual
/// operations. Callbacks may reenter and replace the current property's successor.
pub(crate) unsafe fn init_typed_properties<E>(
    data: *mut u8,
    data_count: i32,
    defaults: *const u8,
    defaults_count: i32,
    host_size: i32,
    mut first_property: impl FnMut() -> *mut c_void,
    mut offset: impl FnMut(*mut c_void) -> i32,
    mut extent: impl FnMut(*mut c_void) -> i32,
    mut next: impl FnMut(*mut c_void) -> *mut c_void,
    mut copy_complete: impl FnMut(*mut c_void, *mut c_void, *mut c_void) -> Result<(), E>,
    mut invalid: impl FnMut(&'static str) -> E,
) -> Result<(), E> {
    if data.is_null() || data_count < host_size || host_size < 0 || defaults_count < 0 {
        return Err(invalid("Invalid property initialization storage"));
    }
    if !defaults.is_null() && defaults_count > data_count {
        return Err(invalid("Invalid property initialization size"));
    }
    unsafe {
        std::ptr::write_bytes(data.add(host_size as usize), 0, (data_count - host_size) as usize);
    }
    let mut check = first_property();
    while !check.is_null() {
        let property_offset = offset(check);
        let property_extent = extent(check);
        if property_offset < host_size || property_extent < 0
            || property_offset.saturating_add(property_extent) > data_count
        {
            return Err(invalid("Invalid reflected property offset"));
        }
        check = next(check);
    }
    if !defaults.is_null() {
        let raw_end = defaults_count.min(data_count);
        let mut cursor = host_size;
        while cursor < raw_end {
            let mut covered_end = cursor;
            let mut next_start = raw_end;
            loop {
                let previous_end = covered_end;
                let mut property = first_property();
                while !property.is_null() {
                    let property_offset = offset(property);
                    let property_end = property_offset
                        .saturating_add(extent(property))
                        .min(raw_end);
                    if property_offset <= covered_end && property_end > covered_end {
                        covered_end = property_end;
                    } else if property_offset > cursor && property_offset < next_start {
                        next_start = property_offset;
                    }
                    property = next(property);
                }
                if covered_end == previous_end {
                    break;
                }
            }
            if covered_end > cursor {
                cursor = covered_end;
            } else {
                let copy_end = next_start.max(cursor + 1).min(raw_end);
                unsafe {
                    std::ptr::copy(
                        defaults.add(cursor as usize),
                        data.add(cursor as usize),
                        (copy_end - cursor) as usize,
                    );
                }
                cursor = copy_end;
            }
        }
    }
    let mut property = first_property();
    while !property.is_null() {
        let property_offset = offset(property);
        let property_extent = extent(property);
        if property_offset < host_size || property_extent < 0
            || property_offset.saturating_add(property_extent) > data_count
        {
            return Err(invalid("Invalid reflected property offset"));
        }
        if !defaults.is_null() && property_offset < defaults_count {
            if property_offset.saturating_add(property_extent) > defaults_count {
                return Err(invalid("Incomplete reflected property defaults"));
            }
            copy_complete(
                property,
                unsafe { data.add(property_offset as usize).cast() },
                unsafe { defaults.add(property_offset as usize).cast_mut().cast() },
            )?;
        }
        property = next(property);
    }
    Ok(())
}

/// Typed Core property destruction shared by direct owner implementations.
///
/// # Safety
/// Data spans the property storage and the supplied callbacks obey the property
/// layout and destruction contracts.
pub(crate) unsafe fn exit_typed_properties<E>(
    data: *mut u8,
    mut first_property: impl FnMut() -> *mut c_void,
    mut offset: impl FnMut(*mut c_void) -> i32,
    mut next: impl FnMut(*mut c_void) -> *mut c_void,
    mut destroy: impl FnMut(*mut c_void, *mut c_void) -> Result<(), E>,
    mut invalid: impl FnMut(&'static str) -> E,
) -> Result<(), E> {
    if data.is_null() { return Err(invalid("Invalid property destruction storage")); }
    let mut property = first_property();
    while !property.is_null() {
        let property_offset = offset(property);
        if property_offset != 0 {
            destroy(property, unsafe { data.wrapping_add(property_offset as usize).cast() })?;
        }
        property = next(property);
    }
    Ok(())
}

#[repr(C)]
pub struct Hp2ClassDefaultObjectServices {
    /// Zero the native UObject header, then install vtable/class/INDEX_NONE.
    pub initialize_header: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub properties_size: unsafe extern "C" fn(*mut c_void) -> i32,
    pub superclass: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    /// Enter the native InitProperties guard; the entry routes back to Rust.
    pub initialize_properties: unsafe extern "C" fn(*mut u8, i32, *mut c_void, *mut u8, i32) -> i32,
}

/// Executes class-default initialization for an owned byte buffer, not a registered object.
///
/// # Safety
/// The buffer spans the class size and remains live across the supplied operations.
pub(crate) unsafe fn init_class_default_object_with<E>(
    object: *mut c_void,
    class: *mut c_void,
    mut initialize_header: impl FnMut(*mut c_void, *mut c_void) -> Result<(), E>,
    mut properties_size: impl FnMut(*mut c_void) -> i32,
    mut superclass: impl FnMut(*mut c_void) -> *mut c_void,
    mut initialize_properties: impl FnMut(*mut u8, i32, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    initialize_header(object, class)?;
    let count = properties_size(class);
    let parent = superclass(class);
    initialize_properties(object.cast(), count, parent)
}

/// # Safety
/// Object must have enough storage for Class. Header initialization is a typed
/// native layout leaf; callbacks must retain the original object lifetimes.
pub unsafe fn init_class_default_object(
    object: *mut c_void,
    class: *mut c_void,
    services: &Hp2ClassDefaultObjectServices,
) -> Result<(), i32> {
    unsafe {
        init_class_default_object_with(
            object,
            class,
            |object, class| status((services.initialize_header)(object, class)),
            |class| (services.properties_size)(class),
            |class| (services.superclass)(class),
            |data, count, superclass| status((services.initialize_properties)(
                data, count, superclass, std::ptr::null_mut(), 0,
            )),
        )
    }
}
