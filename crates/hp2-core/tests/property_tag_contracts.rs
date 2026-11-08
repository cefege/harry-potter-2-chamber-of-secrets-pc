use hp2_core::{
    property_archive::Hp2PropertyArchiveTag as Tag,
    property_tag::{
        self, Hp2PropertyTagCountContinuation, Hp2PropertyTagInteger as Integer,
        Hp2PropertyTagKind as Kind, Hp2PropertyTagServices,
    },
};
use std::{ffi::c_void, mem::MaybeUninit, ptr};
type Object = *mut c_void;
const BOOL: i32 = 7;
const STRUCT: i32 = 11;
#[derive(Clone, Copy)]
enum Flavor {
    Scalar,
    Struct,
    Bool,
    DerivedBool,
}
struct Property {
    flavor: Flavor,
    size: i32,
    mask: u32,
}
struct Archive {
    loading: bool,
    counting: bool,
    counted: i32,
    bytes: Vec<u8>,
    position: usize,
    fail_after: Option<usize>,
    item_error: i32,
}
impl Archive {
    fn saving() -> Self {
        Self {
            loading: false,
            counting: false,
            counted: 0,
            bytes: Vec::new(),
            position: 0,
            fail_after: None,
            item_error: 0,
        }
    }
    fn loading(bytes: Vec<u8>) -> Self {
        Self {
            loading: true,
            bytes,
            ..Self::saving()
        }
    }
    fn object(&mut self) -> Object {
        (self as *mut Self).cast()
    }
}
fn tag(info: u8, size: i32, index: i32) -> Tag {
    Tag {
        property_type: info & 15,
        info,
        name: 24,
        item_name: 39,
        size,
        array_index: index,
    }
}
fn property(flavor: Flavor, size: i32) -> Property {
    Property {
        flavor,
        size,
        mask: 0x8000_0000,
    }
}
unsafe fn transfer(archive: Object, value: *mut u8, size: usize) -> i32 {
    let a = unsafe { &mut *archive.cast::<Archive>() };
    if a.counting {
        a.counted = a.counted.wrapping_add(size as i32);
        return 0;
    }
    for index in 0..size {
        if a.loading {
            let Some(byte) = a.bytes.get(a.position).copied() else {
                return 91;
            };
            unsafe {
                value.add(index).write(byte);
            }
        } else {
            a.bytes.push(unsafe { *value.add(index) });
        }
        a.position += 1;
        if a.fail_after == Some(a.position) {
            return 77;
        }
    }
    0
}
unsafe extern "C" fn integer(p: Object, field: Integer, result: *mut i32) -> i32 {
    let p = unsafe { &*p.cast::<Property>() };
    unsafe {
        *result = match field {
            Integer::TagPropertyType => match p.flavor {
                Flavor::Scalar => 5,
                Flavor::Struct => STRUCT,
                _ => BOOL,
            },
            Integer::TagPropertyName => 24,
            Integer::TagStructName => 39,
            Integer::TagBitMask => p.mask as i32,
        };
    }
    0
}
unsafe extern "C" fn kind(p: Object, kind: Kind) -> i32 {
    let p = unsafe { &*p.cast::<Property>() };
    (match kind {
        Kind::TagIsStruct => matches!(p.flavor, Flavor::Struct),
        Kind::TagIsBool => matches!(p.flavor, Flavor::Bool | Flavor::DerivedBool),
        Kind::TagIsExactBool => matches!(p.flavor, Flavor::Bool),
    }) as i32
}
unsafe extern "C" fn count_scope(
    archive: Object,
    body: Hp2PropertyTagCountContinuation,
    user: Object,
    size: *mut i32,
) -> i32 {
    let item_error = unsafe { (*archive.cast::<Archive>()).item_error };
    let mut count = Archive {
        counting: true,
        item_error,
        ..Archive::saving()
    };
    let code = unsafe { body(user, count.object()) };
    unsafe {
        *size = count.counted;
    }
    code
}
// Name mapping belongs to the supplied archive. This fixture uses a 32-bit name primitive.
unsafe extern "C" fn name(archive: Object, value: *mut i32, previous: Object) -> i32 {
    let code = unsafe { transfer(archive, value.cast(), 4) };
    if code == 0 && !previous.is_null() && unsafe { *value } != 0 {
        unsafe {
            *previous.cast::<i32>() = *value;
        }
    }
    code
}
unsafe extern "C" fn byte(a: Object, value: *mut u8) -> i32 {
    unsafe { transfer(a, value, 1) }
}
unsafe extern "C" fn word(a: Object, value: *mut u16) -> i32 {
    unsafe { transfer(a, value.cast(), 2) }
}
unsafe extern "C" fn int(a: Object, value: *mut i32) -> i32 {
    unsafe { transfer(a, value.cast(), 4) }
}
unsafe extern "C" fn loading(a: Object) -> i32 {
    unsafe { (*a.cast::<Archive>()).loading as i32 }
}
unsafe extern "C" fn check(condition: i32) -> i32 {
    if condition == 0 { 63 } else { 0 }
}
unsafe extern "C" fn item(p: Object, a: Object, value: Object) -> i32 {
    let size = unsafe { (*p.cast::<Property>()).size };
    let a_ref = unsafe { &mut *a.cast::<Archive>() };
    if a_ref.item_error != 0 {
        return a_ref.item_error;
    }
    if a_ref.counting {
        a_ref.counted = a_ref.counted.wrapping_add(size);
        0
    } else {
        unsafe { transfer(a, value.cast(), size as usize) }
    }
}
static SERVICES: Hp2PropertyTagServices = Hp2PropertyTagServices {
    bool_property: BOOL,
    struct_property: STRUCT,
    integer,
    kind,
    count_scope,
    name,
    byte,
    word,
    int,
    loading,
    check,
    serialize_item: item,
};
unsafe fn construct(
    t: *mut Tag,
    a: &mut Archive,
    p: &mut Property,
    index: i32,
    value: *mut u32,
) -> Result<(), i32> {
    unsafe {
        property_tag::construct(
            t,
            a.object(),
            (p as *mut Property).cast(),
            index,
            value.cast(),
            &SERVICES,
        )
    }
}
unsafe fn wire(t: *mut Tag, a: &mut Archive) -> Result<(), i32> {
    unsafe { property_tag::wire(t, a.object(), ptr::null_mut(), &SERVICES) }
}
unsafe fn payload(
    t: *const Tag,
    a: &mut Archive,
    p: &mut Property,
    value: *mut u32,
) -> Result<(), i32> {
    unsafe {
        property_tag::payload(
            t,
            a.object(),
            (p as *mut Property).cast(),
            value.cast(),
            &SERVICES,
        )
    }
}

#[test]
fn constructed_sizes_preserve_all_legacy_codes_including_65536_truncation() {
    for (size, code, encoded_size) in [
        (0, 0x50, 0),
        (1, 0, 1),
        (2, 0x10, 2),
        (4, 0x20, 4),
        (12, 0x30, 12),
        (16, 0x40, 16),
        (255, 0x50, 255),
        (256, 0x60, 256),
        (65535, 0x60, 65535),
        (65536, 0x60, 0),
        (65537, 0x70, 65537),
    ] {
        let mut p = property(Flavor::Scalar, size);
        let mut value = 0u32;
        let mut a = Archive::saving();
        let mut t = MaybeUninit::<Tag>::uninit();
        unsafe {
            construct(t.as_mut_ptr(), &mut a, &mut p, 0, &mut value).unwrap();
        }
        let mut t = unsafe { t.assume_init() };
        assert_eq!((t.info, t.size), (5 | code, size));
        unsafe {
            wire(&mut t, &mut a).unwrap();
        }
        assert_eq!(t.size, encoded_size);
        let mut input = Archive::loading(a.bytes);
        let mut decoded = tag(0, 99, 99);
        unsafe {
            wire(&mut decoded, &mut input).unwrap();
        }
        assert_eq!(
            (decoded.property_type, decoded.size, decoded.array_index),
            (5, encoded_size, 0)
        );
        assert_eq!(input.position, input.bytes.len());
    }
}
#[test]
fn array_indices_have_exact_one_two_and_four_byte_boundaries() {
    for (index, encoded) in [
        (0, vec![]),
        (127, vec![127]),
        (128, vec![0x80, 128]),
        (16383, vec![0xbf, 255]),
        (16384, vec![0xc0, 0, 0x40, 0]),
        (0x3fff_ffff, vec![0xff, 255, 255, 255]),
    ] {
        let mut t = tag(5 | if index == 0 { 0 } else { 0x80 }, 1, index);
        let mut output = Archive::saving();
        unsafe {
            wire(&mut t, &mut output).unwrap();
        }
        assert_eq!(&output.bytes[5..], encoded.as_slice());
        let mut input = Archive::loading(output.bytes);
        let mut decoded = tag(0, 99, -1);
        unsafe {
            wire(&mut decoded, &mut input).unwrap();
        }
        assert_eq!(decoded.array_index, index);
    }
}
#[test]
fn struct_name_precedes_extended_size_and_array_index() {
    let mut t = tag(0, 0, 0);
    let mut a = Archive::saving();
    let mut p = property(Flavor::Struct, 9);
    let mut value = 0u32;
    unsafe {
        construct(&mut t, &mut a, &mut p, 128, &mut value).unwrap();
        wire(&mut t, &mut a).unwrap();
    }
    let mut expected = 24i32.to_le_bytes().to_vec();
    expected.push(0xdb);
    expected.extend_from_slice(&39i32.to_le_bytes());
    expected.extend_from_slice(&[9, 0x80, 128]);
    assert_eq!(a.bytes, expected);
    let mut input = Archive::loading(a.bytes);
    let mut decoded = tag(0, 0, 0);
    unsafe {
        wire(&mut decoded, &mut input).unwrap();
    }
    assert_eq!(
        (decoded.item_name, decoded.size, decoded.array_index),
        (39, 9, 128)
    );
}
#[test]
fn none_name_ends_without_reading_uninitialized_tag_fields() {
    let mut a = Archive::loading(vec![0, 0, 0, 0, 0xaa]);
    let mut t = MaybeUninit::<Tag>::uninit();
    unsafe {
        wire(t.as_mut_ptr(), &mut a).unwrap();
    }
    assert_eq!(a.position, 4);
    assert_eq!(unsafe { (*t.as_ptr()).name }, 0);
}
#[test]
fn loader_initializes_present_fields_without_reading_absent_item_name() {
    let mut bytes = 24i32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0x95, 0xbf, 255]);
    let mut a = Archive::loading(bytes);
    let mut t = MaybeUninit::<Tag>::uninit();
    unsafe {
        wire(t.as_mut_ptr(), &mut a).unwrap();
    }
    let p = t.as_ptr();
    assert_eq!(
        unsafe {
            (
                (*p).name,
                (*p).property_type,
                (*p).info,
                (*p).size,
                (*p).array_index,
            )
        },
        (24, 5, 0x95, 2, 16383)
    );
}
#[test]
fn packed_bool_changes_only_its_bit_and_consumes_no_payload() {
    for enabled in [false, true] {
        let mut p = property(Flavor::Bool, 1);
        let mut t = tag(BOOL as u8 | if enabled { 0x80 } else { 0 }, 1, 21);
        let mut output = Archive::saving();
        unsafe {
            wire(&mut t, &mut output).unwrap();
        }
        assert_eq!((t.array_index, output.bytes.len()), (0, 5));
        let mut input = Archive::loading(Vec::new());
        let mut bits = if enabled { 0x1234_5678 } else { 0x9234_5678 };
        unsafe {
            payload(&t, &mut input, &mut p, &mut bits).unwrap();
        }
        assert_eq!(bits, if enabled { 0x9234_5678 } else { 0x1234_5678 });
        assert_eq!(input.position, 0);
        let before = bits;
        unsafe {
            payload(&t, &mut output, &mut p, &mut bits).unwrap();
        }
        assert_eq!((bits, output.bytes.len()), (before, 5));
    }
}
#[test]
fn bool_constructor_keeps_zero_size_and_legacy_nonzero_index_flag() {
    for (bits, index, flag) in [(0u32, 0, 0), (0x8000_0000, 0, 0x80), (0, 3, 0x80)] {
        let mut bits = bits;
        let mut p = property(Flavor::Bool, 1);
        let mut a = Archive::saving();
        let mut t = tag(0, 99, 99);
        unsafe {
            construct(&mut t, &mut a, &mut p, index, &mut bits).unwrap();
            wire(&mut t, &mut a).unwrap();
        }
        assert_eq!(
            (t.info, t.size, t.array_index),
            (BOOL as u8 | 0x50 | flag, 0, 0)
        );
        assert_eq!(a.bytes.len(), 6);
    }
}
#[test]
fn derived_bool_uses_virtual_payload_and_count_instead_of_exact_bool_path() {
    let mut p = property(Flavor::DerivedBool, 4);
    let mut value = 0x8000_0012u32;
    let mut a = Archive::saving();
    let mut t = tag(0, 0, 0);
    unsafe {
        construct(&mut t, &mut a, &mut p, 0, &mut value).unwrap();
        payload(&t, &mut a, &mut p, &mut value).unwrap();
    }
    assert_eq!((t.size, t.info), (4, BOOL as u8 | 0xa0));
    assert_eq!(a.bytes, value.to_le_bytes());
    let mut input = Archive::loading(0x1234_5678u32.to_le_bytes().to_vec());
    unsafe {
        payload(&t, &mut input, &mut p, &mut value).unwrap();
    }
    assert_eq!(value, 0x1234_5678);
}
#[test]
fn failed_size_read_keeps_size_and_index_but_publishes_prior_header_fields() {
    let mut bytes = 24i32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0xe5, 0x34, 0x12, 0xc0, 0, 0x40, 0]);
    let mut a = Archive::loading(bytes);
    a.fail_after = Some(6);
    let mut t = tag(0, 999, 4321);
    assert_eq!(unsafe { wire(&mut t, &mut a) }, Err(77));
    assert_eq!(
        (t.name, t.info, t.property_type, t.size, t.array_index),
        (24, 0xe5, 5, 999, 4321)
    );
    assert_eq!(a.position, 6);
}
#[test]
fn failed_four_byte_index_never_publishes_a_partial_index() {
    let mut bytes = 24i32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0x85, 0xc0, 0, 0x40, 0]);
    let mut a = Archive::loading(bytes);
    a.fail_after = Some(8);
    let mut t = tag(0, 999, 4321);
    assert_eq!(unsafe { wire(&mut t, &mut a) }, Err(77));
    assert_eq!((t.size, t.array_index, a.position), (1, 4321, 8));
}
#[test]
fn truncated_four_byte_index_is_an_error_without_partial_publication() {
    let mut bytes = 24i32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0x85, 0xc0, 0, 0x40]);
    let mut a = Archive::loading(bytes);
    let mut t = tag(0, 999, 4321);
    assert_eq!(unsafe { wire(&mut t, &mut a) }, Err(91));
    assert_eq!((t.size, t.array_index, a.position), (1, 4321, 8));
}
#[test]
fn mask_assertion_and_virtual_payload_failure_do_not_publish_value_mutations() {
    let mut t = tag(BOOL as u8 | 0x80, 0, 0);
    let mut p = property(Flavor::Bool, 1);
    p.mask = 0;
    let mut bits = 0x1234_5678u32;
    let mut a = Archive::loading(Vec::new());
    assert_eq!(unsafe { payload(&t, &mut a, &mut p, &mut bits) }, Err(63));
    assert_eq!(bits, 0x1234_5678);
    p = property(Flavor::Scalar, 4);
    a.item_error = 59;
    assert_eq!(unsafe { payload(&t, &mut a, &mut p, &mut bits) }, Err(59));
    assert_eq!(bits, 0x1234_5678);
    assert_eq!(
        unsafe { construct(&mut t, &mut a, &mut p, 0, &mut bits) },
        Err(59)
    );
    assert_eq!(a.position, 0);
}
