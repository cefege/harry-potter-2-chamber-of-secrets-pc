use hp2_core::object_properties::*;
use std::ffi::c_void;
use std::ptr::{null_mut, read_unaligned, write_unaligned};

const HEADER: usize = 8;
const FAILURE: i32 = 73;

struct Class {
    defaults: Vec<u8>,
    count: i32,
    count_after_data: Option<i32>,
    first: *mut c_void,
    superclass: *mut c_void,
    size: i32,
    marker: u8,
    header_error: bool,
}
impl Class {
    fn new(defaults: Vec<u8>) -> Self {
        Self {
            count: defaults.len() as i32,
            defaults,
            count_after_data: None,
            first: null_mut(),
            superclass: null_mut(),
            size: 32,
            marker: 0x42,
            header_error: false,
        }
    }
    fn ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
}

struct Property {
    offset: i32,
    size: i32,
    owned_elements: usize,
    next: *mut c_void,
    replace_next: Option<*mut c_void>,
    fail_copy: bool,
    fail_destroy: bool,
    reentry: *mut Reentry,
}
impl Property {
    fn bytes(offset: i32, size: i32) -> Self {
        Self {
            offset,
            size,
            owned_elements: 0,
            next: null_mut(),
            replace_next: None,
            fail_copy: false,
            fail_destroy: false,
            reentry: null_mut(),
        }
    }
    fn owned(offset: i32, elements: usize) -> Self {
        let mut p = Self::bytes(offset, (elements * size_of::<usize>()) as i32);
        p.owned_elements = elements;
        p
    }
    fn ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
}
struct Reentry {
    data: Vec<u8>,
    class: *mut c_void,
    result: Option<Result<(), i32>>,
}

unsafe extern "C" fn host_size(out: *mut i32) -> i32 {
    unsafe {
        *out = HEADER as i32;
    }
    0
}
unsafe extern "C" fn bounds(kind: i32, actual: i32, bound: i32) -> i32 {
    if (kind == 2 && actual <= bound) || (kind != 2 && actual >= bound) {
        0
    } else {
        61 + kind
    }
}
unsafe extern "C" fn defaults_count(class: *mut c_void) -> i32 {
    unsafe { (*class.cast::<Class>()).count }
}
unsafe extern "C" fn defaults_data(class: *mut c_void, out: *mut *mut u8) -> i32 {
    unsafe {
        let class = class.cast::<Class>();
        *out = (*class).defaults.as_mut_ptr();
        if let Some(count) = (*class).count_after_data.take() {
            (*class).count = count;
        }
    }
    0
}
unsafe extern "C" fn first(class: *mut c_void) -> *mut c_void {
    unsafe { (*class.cast::<Class>()).first }
}
unsafe extern "C" fn next(property: *mut c_void) -> *mut c_void {
    unsafe { (*property.cast::<Property>()).next }
}
unsafe extern "C" fn offset(property: *mut c_void) -> i32 {
    unsafe { (*property.cast::<Property>()).offset }
}
unsafe extern "C" fn size(property: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = (*property.cast::<Property>()).size;
    }
    0
}
unsafe extern "C" fn invalid_offset(_: *mut c_void) -> i32 {
    69
}
unsafe extern "C" fn copy(property: *mut c_void, data: *mut u8, source: *mut u8) -> i32 {
    unsafe {
        let property = property.cast::<Property>();
        if let Some(replacement) = (*property).replace_next.take() {
            (*property).next = replacement;
        }
        let reentry = (*property).reentry;
        if !reentry.is_null() {
            // Do not retain a borrow into the callback-owned state over reentry.
            (*property).reentry = null_mut();
            let mut trace = fresh_trace();
            let result = init_properties(
                (*reentry).data.as_mut_ptr(),
                (*reentry).data.len() as i32,
                (*reentry).class,
                null_mut(),
                0,
                &SERVICES,
                &mut trace,
            );
            (*reentry).result = Some(result);
        }
        if (*property).fail_copy {
            *data = 0xe7;
            return FAILURE;
        }
        let elements = (*property).owned_elements;
        if elements == 0 {
            std::ptr::copy(source, data, (*property).size as usize);
        } else {
            for index in 0..elements {
                let at = index * size_of::<usize>();
                // Owning native values require zeroed destination storage before
                // deep assignment; reject a shallow alias instead of freeing it.
                if !read_unaligned(data.add(at).cast::<*mut Vec<u8>>()).is_null() {
                    return 95;
                }
                let source_value = read_unaligned(source.add(at).cast::<*mut Vec<u8>>());
                let copied = if source_value.is_null() {
                    null_mut()
                } else {
                    Box::into_raw(Box::new((*source_value).clone()))
                };
                write_unaligned(data.add(at).cast::<*mut Vec<u8>>(), copied);
            }
        }
    }
    0
}
unsafe extern "C" fn destroy(property: *mut c_void, data: *mut u8) -> i32 {
    unsafe {
        let property = property.cast::<Property>();
        if let Some(replacement) = (*property).replace_next.take() {
            (*property).next = replacement;
        }
        for index in 0..(*property).owned_elements {
            let slot = data.add(index * size_of::<usize>()).cast::<*mut Vec<u8>>();
            let value = read_unaligned(slot);
            if !value.is_null() {
                drop(Box::from_raw(value));
            }
            write_unaligned(slot, null_mut());
        }
        if (*property).owned_elements == 0 {
            std::ptr::write_bytes(data, 0xdd, (*property).size as usize);
        }
        if (*property).fail_destroy {
            return FAILURE;
        }
    }
    0
}
static SERVICES: Hp2ObjectPropertiesServices = Hp2ObjectPropertiesServices {
    host_size,
    check_bounds: bounds,
    defaults_count,
    defaults_data,
    constructor_link: first,
    constructor_next: next,
    offset,
    size,
    invalid_offset,
    copy_complete: copy,
    destroy,
};
fn fresh_trace() -> Hp2ObjectPropertiesTrace {
    Hp2ObjectPropertiesTrace {
        phase: -1,
        property: null_mut(),
    }
}
fn initialize(
    data: &mut [u8],
    class: *mut c_void,
    source: *mut u8,
    count: i32,
) -> (Result<(), i32>, Hp2ObjectPropertiesTrace) {
    let mut trace = fresh_trace();
    let result = unsafe {
        init_properties(
            data.as_mut_ptr(),
            data.len() as i32,
            class,
            source,
            count,
            &SERVICES,
            &mut trace,
        )
    };
    (result, trace)
}

#[test]
fn absent_defaults_preserve_header_and_zero_only_payload() {
    let mut data = [0xa5; 24];
    assert_eq!(initialize(&mut data, null_mut(), null_mut(), 0).0, Ok(()));
    assert_eq!(&data[..HEADER], &[0xa5; HEADER]);
    assert_eq!(&data[HEADER..], &[0; 16]);
}

#[test]
fn explicit_template_wins_over_nonempty_class_defaults() {
    let mut class = Class::new(vec![0x33; 20]);
    let mut explicit = [0x71; 12];
    let mut data = [0xa5; 24];
    assert_eq!(
        initialize(&mut data, class.ptr(), explicit.as_mut_ptr(), 12).0,
        Ok(())
    );
    assert_eq!(&data[..8], &[0xa5; 8]);
    assert_eq!(&data[8..12], &[0x71; 4]);
    assert_eq!(&data[12..], &[0; 12]);
    assert_eq!(class.defaults, vec![0x33; 20]);
}

#[test]
fn selected_class_defaults_replace_incoming_count_and_zero_tail() {
    let mut class = Class::new(vec![0x31; 16]);
    let mut data = [0xa5; 24];
    assert_eq!(
        initialize(&mut data, class.ptr(), null_mut(), 123).0,
        Ok(())
    );
    assert_eq!(&data[..8], &[0xa5; 8]);
    assert_eq!(&data[8..16], &[0x31; 8]);
    assert_eq!(&data[16..], &[0; 8]);
}

#[test]
fn empty_native_defaults_do_not_attempt_payload_copy() {
    let mut class = Class::new(vec![]);
    let mut data = [0xa5; 16];
    assert_eq!(initialize(&mut data, class.ptr(), null_mut(), 0).0, Ok(()));
    assert_eq!(&data[8..], &[0; 8]);
}

#[test]
fn default_count_is_read_again_after_default_data_accessor() {
    let mut class = Class::new(vec![0x67; 20]);
    class.count_after_data = Some(12);
    let mut data = [0xa5; 24];
    assert_eq!(initialize(&mut data, class.ptr(), null_mut(), 0).0, Ok(()));
    assert_eq!(&data[8..12], &[0x67; 4]);
    assert_eq!(&data[12..], &[0; 12]);
}

#[test]
fn bad_payload_bounds_fail_before_any_copy_or_zero() {
    for (count, expected) in [(7, 62), (25, 63)] {
        let mut source = [0x22; 32];
        let mut data = [0xa5; 24];
        let (result, trace) = initialize(&mut data, null_mut(), source.as_mut_ptr(), count);
        assert_eq!(result, Err(expected));
        assert_eq!(trace.phase, 2);
        assert_eq!(data, [0xa5; 24]);
    }
}

#[test]
fn short_host_fails_before_default_access_or_mutation() {
    let mut class = Class::new(vec![0x99; 16]);
    class.count_after_data = Some(8);
    let mut data = [0xa5; 7];
    let (result, trace) = initialize(&mut data, class.ptr(), null_mut(), 0);
    assert_eq!(result, Err(61));
    assert_eq!(trace.phase, 0);
    assert_eq!(data, [0xa5; 7]);
    assert_eq!(class.count, 16);
}

#[test]
fn header_only_object_accepts_exact_boundary_without_touching_identity() {
    let mut data = [0xa5; HEADER];
    let mut defaults = [0x21; HEADER];
    assert_eq!(
        initialize(&mut data, null_mut(), defaults.as_mut_ptr(), HEADER as i32).0,
        Ok(())
    );
    assert_eq!(data, [0xa5; HEADER]);
}

#[test]
fn property_at_defaults_end_is_not_deep_initialized() {
    let mut property = Property::bytes(16, 4);
    property.fail_copy = true;
    let mut class = Class::new(vec![0x11; 16]);
    class.first = property.ptr();
    let mut data = [0xa5; 24];
    assert_eq!(initialize(&mut data, class.ptr(), null_mut(), 0).0, Ok(()));
    assert_eq!(&data[16..20], &[0; 4]);
}

#[test]
fn invalid_host_offset_errors_even_when_outside_defaults_range() {
    let mut property = Property::bytes(4, 4);
    let mut class = Class::new(vec![]);
    class.first = property.ptr();
    let mut data = [0xa5; 24];
    let (result, trace) = initialize(&mut data, class.ptr(), null_mut(), 0);
    assert_eq!(result, Err(69));
    assert_eq!(trace.phase, 4);
    assert_eq!(trace.property, property.ptr());
    assert_eq!(&data[..8], &[0xa5; 8]);
    assert_eq!(&data[8..], &[0; 16]);
}

#[test]
fn deep_typed_array_values_are_independent_and_destroyed_at_packed_offsets() {
    let mut property = Property::owned(9, 2);
    let mut class = Class::new(vec![0; 32]);
    class.first = property.ptr();
    let source_a = Box::into_raw(Box::new(vec![1u8, 2, 3]));
    let source_b = Box::into_raw(Box::new(vec![4u8, 5]));
    unsafe {
        write_unaligned(class.defaults.as_mut_ptr().add(9).cast(), source_a);
        write_unaligned(
            class
                .defaults
                .as_mut_ptr()
                .add(9 + size_of::<usize>())
                .cast(),
            source_b,
        );
    }
    let mut data = [0xa5; 40];
    assert_eq!(initialize(&mut data, class.ptr(), null_mut(), 0).0, Ok(()));
    unsafe {
        let copy_a = read_unaligned(data.as_ptr().add(9).cast::<*mut Vec<u8>>());
        let copy_b = read_unaligned(
            data.as_ptr()
                .add(9 + size_of::<usize>())
                .cast::<*mut Vec<u8>>(),
        );
        assert_ne!(copy_a, source_a);
        assert_ne!(copy_b, source_b);
        (*source_a).push(9);
        assert_eq!(*copy_a, vec![1, 2, 3]);
        assert_eq!(*copy_b, vec![4, 5]);
        let mut current = null_mut();
        assert_eq!(
            exit_properties(data.as_mut_ptr(), class.ptr(), &SERVICES, &mut current),
            Ok(())
        );
        assert!(current.is_null());
        assert_eq!(
            &data[9..9 + 2 * size_of::<usize>()],
            &[0; 2 * size_of::<usize>()]
        );
        drop(Box::from_raw(source_a));
        drop(Box::from_raw(source_b));
    }
    assert_eq!(&data[..8], &[0xa5; 8]);
}

#[test]
fn explicit_template_aliasing_class_defaults_keeps_explicit_count() {
    let mut data = [0xa5; 32];
    let mut property = Property::bytes(8, 4);
    let mut class = Class::new(vec![0x22; 24]);
    class.first = property.ptr();
    let source = class.defaults.as_mut_ptr();
    let (result, _) = initialize(&mut data, class.ptr(), source, 12);
    assert_eq!(result, Ok(()));
    assert_eq!(&data[..8], &[0xa5; 8]);
    assert_eq!(&data[8..12], &[0x22; 4]);
    assert_eq!(&data[12..], &[0; 20]);
    assert_eq!(class.defaults, vec![0x22; 24]);
}

#[test]
fn constructor_successor_is_live_after_copy_and_reentry() {
    let mut skipped = Property::bytes(12, 4);
    skipped.fail_copy = true;
    let mut replacement = Property::bytes(16, 4);
    replacement.fail_copy = true;
    let mut first_property = Property::bytes(8, 4);
    first_property.next = skipped.ptr();
    first_property.replace_next = Some(replacement.ptr());
    let mut nested_class = Class::new(vec![0x62; 12]);
    let mut reentry = Reentry {
        data: vec![0xb1; 20],
        class: nested_class.ptr(),
        result: None,
    };
    first_property.reentry = &mut reentry;
    let mut class = Class::new(vec![0x31; 24]);
    class.first = first_property.ptr();
    let mut data = [0xa5; 32];
    let (result, trace) = initialize(&mut data, class.ptr(), null_mut(), 0);
    assert_eq!(result, Err(FAILURE));
    assert_eq!(trace.property, replacement.ptr());
    assert_eq!(&data[8..12], &[0x31; 4]);
    assert_eq!(&data[12..16], &[0x31; 4]);
    assert_eq!(&data[16..20], &[0xe7, 0, 0, 0]);
    assert_eq!(reentry.result, Some(Ok(())));
    assert_eq!(&reentry.data[..8], &[0xb1; 8]);
    assert_eq!(&reentry.data[8..12], &[0x62; 4]);
    assert_eq!(&reentry.data[12..], &[0; 8]);
}

#[test]
fn first_copy_error_retains_partial_mutation_and_stops_later_properties() {
    let mut later = Property::bytes(16, 4);
    let mut failed = Property::bytes(12, 4);
    failed.fail_copy = true;
    failed.next = later.ptr();
    let mut class = Class::new(vec![0x51; 24]);
    class.first = failed.ptr();
    let mut data = [0xa5; 32];
    let (result, trace) = initialize(&mut data, class.ptr(), null_mut(), 0);
    assert_eq!(result, Err(FAILURE));
    assert_eq!(trace.property, failed.ptr());
    assert_eq!(&data[12..16], &[0xe7, 0, 0, 0]);
    assert_eq!(&data[16..24], &[0x51; 8]);
    assert_eq!(&data[24..], &[0; 8]);
}

#[test]
fn destruction_skips_zero_offset_and_uses_mutated_successor() {
    let mut skipped = Property::bytes(12, 4);
    let mut replacement = Property::bytes(16, 4);
    let mut active = Property::bytes(8, 4);
    active.next = skipped.ptr();
    active.replace_next = Some(replacement.ptr());
    let mut zero = Property::bytes(0, 4);
    zero.fail_destroy = true;
    zero.next = active.ptr();
    let mut class = Class::new(vec![]);
    class.first = zero.ptr();
    let mut data = [0xa5; 24];
    let mut current = null_mut();
    unsafe {
        assert_eq!(
            exit_properties(data.as_mut_ptr(), class.ptr(), &SERVICES, &mut current),
            Ok(())
        );
    }
    assert!(current.is_null());
    assert_eq!(&data[..8], &[0xa5; 8]);
    assert_eq!(&data[8..12], &[0xdd; 4]);
    assert_eq!(&data[12..16], &[0xa5; 4]);
    assert_eq!(&data[16..20], &[0xdd; 4]);
}

#[test]
fn destruction_error_keeps_current_diagnostic_and_prior_mutations() {
    let mut later = Property::bytes(16, 4);
    let mut failed = Property::bytes(12, 4);
    failed.fail_destroy = true;
    failed.next = later.ptr();
    let mut first_property = Property::bytes(8, 4);
    first_property.next = failed.ptr();
    let mut class = Class::new(vec![]);
    class.first = first_property.ptr();
    let mut data = [0xa5; 24];
    let mut current = null_mut();
    unsafe {
        assert_eq!(
            exit_properties(data.as_mut_ptr(), class.ptr(), &SERVICES, &mut current),
            Err(FAILURE)
        );
    }
    assert_eq!(current, failed.ptr());
    assert_eq!(&data[8..16], &[0xdd; 8]);
    assert_eq!(&data[16..], &[0xa5; 8]);
}

unsafe extern "C" fn initialize_header(object: *mut c_void, class: *mut c_void) -> i32 {
    unsafe {
        let data = object.cast::<u8>();
        std::ptr::write_bytes(data, 0, HEADER);
        *data = (*class.cast::<Class>()).marker;
        std::ptr::write_bytes(data.add(4), 0xff, 4);
        if (*class.cast::<Class>()).header_error {
            return FAILURE;
        }
    }
    0
}
unsafe extern "C" fn properties_size(class: *mut c_void) -> i32 {
    unsafe { (*class.cast::<Class>()).size }
}
unsafe extern "C" fn superclass(class: *mut c_void) -> *mut c_void {
    unsafe { (*class.cast::<Class>()).superclass }
}
unsafe extern "C" fn initialize_properties(
    data: *mut u8,
    count: i32,
    class: *mut c_void,
    defaults: *mut u8,
    defaults_count: i32,
) -> i32 {
    let mut trace = fresh_trace();
    unsafe {
        init_properties(
            data,
            count,
            class,
            defaults,
            defaults_count,
            &SERVICES,
            &mut trace,
        )
        .err()
        .unwrap_or(0)
    }
}
static CLASS_SERVICES: Hp2ClassDefaultObjectServices = Hp2ClassDefaultObjectServices {
    initialize_header,
    properties_size,
    superclass,
    initialize_properties,
};

#[test]
fn class_default_header_precedes_superclass_payload_not_own_defaults() {
    let mut superclass = Class::new(vec![0x29; 16]);
    let mut class = Class::new(vec![0x91; 32]);
    class.superclass = superclass.ptr();
    let mut data = [0xa5u8; 32];
    unsafe {
        assert_eq!(
            init_class_default_object(data.as_mut_ptr().cast(), class.ptr(), &CLASS_SERVICES),
            Ok(())
        );
    }
    assert_eq!(&data[..8], &[0x42, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(&data[8..16], &[0x29; 8]);
    assert_eq!(&data[16..], &[0; 16]);
}

#[test]
fn class_default_header_error_prevents_payload_initialization() {
    let mut class = Class::new(vec![0x91; 32]);
    class.header_error = true;
    let mut data = [0xa5u8; 32];
    unsafe {
        assert_eq!(
            init_class_default_object(data.as_mut_ptr().cast(), class.ptr(), &CLASS_SERVICES),
            Err(FAILURE)
        );
    }
    assert_eq!(&data[..8], &[0x42, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(&data[8..], &[0xa5; 24]);
}

#[test]
fn class_default_payload_error_preserves_initialized_identity() {
    let mut invalid = Property::bytes(0, 4);
    let mut superclass = Class::new(vec![0x29; 16]);
    superclass.first = invalid.ptr();
    let mut class = Class::new(vec![0x91; 32]);
    class.superclass = superclass.ptr();
    let mut data = [0xa5u8; 32];
    unsafe {
        assert_eq!(
            init_class_default_object(data.as_mut_ptr().cast(), class.ptr(), &CLASS_SERVICES),
            Err(69)
        );
    }
    assert_eq!(&data[..8], &[0x42, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(&data[8..16], &[0x29; 8]);
    assert_eq!(&data[16..], &[0; 16]);
}
