use Hp2MetadataArchiveValue::*;
use Hp2MetadataCheck::*;
use Hp2MetadataInteger::*;
use Hp2MetadataKind::*;
use Hp2MetadataPointer::*;
use hp2_abi::engine::generated::{TArrayRaw, UClass, UField, UFunction, UObject, UProperty, UState, UStruct};
use hp2_core::array_storage::Hp2ArrayServices;
use hp2_core::class_metadata::{self as metadata, *};
use hp2_core::vm_registry::Hp2VmNativeHandler;
use std::{cell::Cell, ffi::c_void, ptr};

type P = *mut c_void;
macro_rules! get {
    ($p:expr, $ty:ty, $field:ident) => {
        ptr::addr_of!((*($p).cast::<$ty>()).$field).read_unaligned()
    };
}
macro_rules! put {
    ($p:expr, $ty:ty, $field:ident, $value:expr) => {
        ptr::addr_of_mut!((*($p).cast::<$ty>()).$field).write_unaligned($value)
    };
}

// Initialized ABI storage, not a live C++ object. All metadata lives in `raw`;
// the remaining fields configure typed leaves or own allocated byte buffers.
#[repr(C)]
struct Object {
    raw: UClass,
    script_allocation: Vec<u8>,
    default_allocation: Vec<u8>,
    is_property: bool,
    is_struct: bool,
    link_size: i32,
    link_align: i32,
    link_error: i32,
    super_error: i32,
    allocation_error: i32,
    initialize_error: i32,
    default_initialized: bool,
    localized_error: i32,
    expr_stride: i32,
    expr_error: i32,
    linked: bool,
    system_config: bool,
    export: P,
    export_side_effect: Option<Hp2VmNativeHandler>,
    export_error: i32,
    hardcoded_present: bool,
    checked_index: Option<u16>,
    property_mutation: bool,
    recurse_struct: bool,
    nested_class: P,
    empty_reentry: bool,
}
impl Default for Object {
    fn default() -> Self {
        Self {
            raw: unsafe { std::mem::zeroed() },
            script_allocation: vec![], default_allocation: vec![],
            is_property: false, is_struct: false,
            link_size: 8, link_align: 8, link_error: 0, super_error: 0,
            allocation_error: 0, initialize_error: 0, default_initialized: false,
            localized_error: 0, expr_stride: 1, expr_error: 0,
            linked: false, system_config: false, export: ptr::null_mut(),
            export_side_effect: None, export_error: 0, hardcoded_present: true,
            checked_index: None, property_mutation: false, recurse_struct: false,
            nested_class: ptr::null_mut(), empty_reentry: false,
        }
    }
}
struct Archive {
    version: i32,
    loading: bool,
    saving: bool,
    bytes: Vec<u8>,
    at: usize,
    defaults: Vec<u8>,
    counted: usize,
}
impl Archive {
    fn new(version: i32, loading: bool, saving: bool) -> Self {
        Self { version, loading, saving, bytes: vec![], at: 0,
            defaults: vec![1, 2, 3, 4, 0, 0, 0, 0], counted: 0 }
    }
    fn pointer(&mut self) -> P { (self as *mut Self).cast() }
    // Local primitive fixture: references are process-local pointer tokens and
    // names are raw indices, not a package archive's object/name index mapping.
    unsafe fn value(&mut self, value: P, width: usize) -> i32 {
        unsafe {
            if self.loading {
                let available = width.min(self.bytes.len().saturating_sub(self.at));
                ptr::copy_nonoverlapping(self.bytes.as_ptr().add(self.at), value.cast(), available);
                self.at += available;
                if available != width { return 93; }
            } else if self.saving {
                self.bytes.extend_from_slice(std::slice::from_raw_parts(value.cast::<u8>(), width));
            }
        }
        0
    }
    fn scalar(&mut self, value: &mut i32, width: usize) {
        if self.loading {
            let mut bytes = [0u8; 4];
            bytes[..width].copy_from_slice(&self.bytes[self.at..self.at + width]);
            *value = i32::from_le_bytes(bytes);
            self.at += width;
        } else if self.saving {
            self.bytes.extend_from_slice(&value.to_le_bytes()[..width]);
        }
    }
}
struct Host {
    storage: MetadataStorage,
    objects: Vec<Box<Object>>,
    root: P,
    flushed: usize,
}
thread_local! { static ACTIVE: Cell<*mut Host> = const { Cell::new(ptr::null_mut()) }; }
struct Fixture { host: *mut Host, old: *mut Host }
impl Fixture {
    fn new() -> Self {
        let host = Box::into_raw(Box::new(Host {
            storage: MetadataStorage::new(), objects: vec![],
            root: 1usize as P, flushed: 0,
        }));
        let old = ACTIVE.with(|active| active.replace(host));
        Self { host, old }
    }
    fn storage(&self) -> *mut MetadataStorage {
        unsafe { ptr::addr_of_mut!((*self.host).storage) }
    }
    fn add(&mut self) -> P {
        let mut value = Box::<Object>::default();
        // Initialize padding too: base/derived mirrors name overlapping bytes.
        unsafe { ptr::write_bytes(ptr::addr_of_mut!(value.raw), 0, 1); }
        let p: P = (&mut *value as *mut Object).cast();
        unsafe {
            put!(p, UStruct, properties_size, 8);
            put!(p, UStruct, properties_align, 8);
            (*self.host).objects.push(value);
        }
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.old));
        unsafe { drop(Box::from_raw(self.host)); }
    }
}
unsafe fn o(p: P) -> &'static mut Object { unsafe { &mut *p.cast::<Object>() } }
unsafe fn a(p: P) -> &'static mut Archive { unsafe { &mut *p.cast::<Archive>() } }
fn host() -> *mut Host { ACTIVE.with(Cell::get) }
unsafe fn bytes(array: TArrayRaw) -> Vec<u8> {
    if array.num == 0 { vec![] }
    else { unsafe { std::slice::from_raw_parts(array.data.cast::<u8>(), array.num as usize).to_vec() } }
}
unsafe fn defaults(p: P) -> Vec<u8> { unsafe { bytes(get!(p, UClass, defaults)) } }
unsafe fn script(p: P) -> Vec<u8> { unsafe { bytes(get!(p, UStruct, script)) } }
unsafe fn set_defaults(p: P, values: &[u8]) {
    unsafe {
        o(p).default_allocation = values.to_vec();
        put!(p, UClass, defaults, TArrayRaw {
            data: o(p).default_allocation.as_mut_ptr().cast(),
            num: values.len() as i32, max: values.len() as i32,
        });
    }
}
unsafe extern "C" fn pointer(p: P, field: Hp2MetadataPointer, out: *mut P) -> i32 {
    unsafe { out.write(match field {
        MetadataRootClass => (*host()).root,
        MetadataInheritanceSuper | MetadataSuperState | MetadataSuperClass => get!(p, UField, super_field),
    }); }
    0
}
unsafe extern "C" fn integer(p: P, field: Hp2MetadataInteger) -> i32 {
    unsafe { match field {
        MetadataArchiveVersion => a(p).version,
        MetadataArchiveLoading => a(p).loading as i32,
        MetadataArchiveSaving => a(p).saving as i32,
        MetadataIsProperty => {
            if o(p).property_mutation {
                o(p).property_mutation = false;
                put!(p, UProperty, offset, 27);
                put!(p, UProperty, array_dim, 3);
                put!(p, UProperty, element_size, 5);
                put!(p, UProperty, property_flags, METADATA_CPF_PARM | METADATA_CPF_RETURN);
            }
            o(p).is_property as i32
        }
        MetadataIsStruct => o(p).is_struct as i32,
    } }
}
unsafe extern "C" fn set_flags(p: P, flags: u32) -> i32 {
    unsafe { put!(p, UObject, object_flags, get!(p, UObject, object_flags) | flags); }
    0
}
unsafe extern "C" fn super_serialize(p: P, ar: P, kind: Hp2MetadataKind) -> i32 {
    unsafe {
        if o(p).super_error != 0 { return o(p).super_error; }
        let storage = ptr::addr_of_mut!((*host()).storage);
        if kind == MetadataClass && !o(p).nested_class.is_null() {
            let nested = o(p).nested_class;
            o(p).nested_class = ptr::null_mut();
            if let Err(error) = metadata::serialize(storage, nested, ar, MetadataClass, &S) { return error; }
        }
        if kind == MetadataClass && o(p).recurse_struct {
            if let Err(error) = metadata::serialize(storage, p, ar, MetadataStruct, &S) { return error; }
        }
        0
    }
}
unsafe extern "C" fn serialize_value(ar: P, kind: Hp2MetadataArchiveValue, value: P) -> i32 {
    unsafe {
        let width = match kind {
            MetadataByte => 1,
            MetadataWord => 2,
            MetadataInt | MetadataDword | MetadataName => 4,
            MetadataQword => 8,
            MetadataFieldReference | MetadataTextBufferReference | MetadataClassReference => std::mem::size_of::<P>(),
            MetadataGuid => 16,
            MetadataDependencyArray | MetadataNameArray => {
                // These contracts exercise empty arrays only. Nonempty typed
                // compound operators belong to the native archive contracts.
                assert_eq!(ptr::addr_of!((*value.cast::<TArrayRaw>()).num).read_unaligned(), 0);
                let mut count = 0u8;
                let result = a(ar).value(ptr::addr_of_mut!(count).cast(), 1);
                assert_eq!(count, 0);
                return result;
            }
        };
        a(ar).value(value, width)
    }
}
unsafe extern "C" fn serialize_expr(p: P, ar: P, code: *mut i32) -> i32 {
    unsafe {
        let array = get!(p, UStruct, script);
        let mut value = array.data.cast::<u8>().add(*code as usize).read() as i32;
        a(ar).scalar(&mut value, 1);
        array.data.cast::<u8>().add(*code as usize).write(value as u8);
        *code += o(p).expr_stride;
        o(p).expr_error
    }
}
unsafe extern "C" fn script_mismatch(_: i32, _: i32) -> i32 { 77 }
unsafe extern "C" fn check(p: P, condition: Hp2MetadataCheck) -> i32 {
    unsafe { match condition {
        MetadataDefaultsMatch if get!(p, UClass, defaults).num != get!(p, UStruct, properties_size) => 74,
        MetadataScriptNativeZero if get!(p, UFunction, i_native) != 0 => 71,
        MetadataNativeIndexValid if get!(p, UFunction, i_native) >= 4096 => 72,
        MetadataNativeFunctionPresent => {
            if !o(p).hardcoded_present { return 73; }
            if let Some(index) = o(p).checked_index.take() { put!(p, UFunction, i_native, index); }
            0
        }
        _ => 0,
    } }
}
unsafe extern "C" fn link(p: P, _: P, _: i32) -> i32 {
    unsafe {
        o(p).linked = true;
        put!(p, UStruct, properties_size, o(p).link_size);
        put!(p, UStruct, properties_align, o(p).link_align);
        o(p).link_error
    }
}
unsafe extern "C" fn reallocate(
    _: P, _: P, size: usize, max: *const i32, width: i32,
    _: *const c_void, out: *mut P,
) -> i32 {
    unsafe {
        assert_eq!(width, 1);
        let owner = (*host()).objects.iter_mut().find_map(|object| {
            let p: P = (&mut **object as *mut Object).cast();
            if max == ptr::addr_of!((*p.cast::<UClass>()).defaults.max) { Some((p, false)) }
            else if max == ptr::addr_of!((*p.cast::<UStruct>()).script.max) { Some((p, true)) }
            else { None }
        }).unwrap();
        let (p, is_script) = owner;
        if o(p).allocation_error != 0 { return o(p).allocation_error; }
        if is_script {
            o(p).script_allocation.resize(size, 0xa5);
            out.write(if size == 0 { ptr::null_mut() } else { o(p).script_allocation.as_mut_ptr().cast() });
            if size == 0 && o(p).empty_reentry {
                o(p).empty_reentry = false;
                // Empty's allocator may reenter and publish a new live count.
                ptr::addr_of_mut!((*p.cast::<UStruct>()).script.num).write(2);
            }
        } else {
            o(p).default_allocation.resize(size, 0xa5);
            out.write(o(p).default_allocation.as_mut_ptr().cast());
        }
        0
    }
}
unsafe extern "C" fn checked_array_size(_: P, size: usize, out: *mut i32) -> i32 {
    unsafe { out.write(size as i32); }
    0
}
unsafe extern "C" fn free(_: P, _: P) -> i32 { 0 }
const ARRAYS: Hp2ArrayServices = Hp2ArrayServices {
    context: ptr::null_mut(), realloc: reallocate, checked_int_size: checked_array_size, free,
};
unsafe extern "C" fn array_allocator() -> *const Hp2ArrayServices { &ARRAYS }
unsafe extern "C" fn initialize_default(p: P) -> i32 {
    unsafe {
        let array = get!(p, UClass, defaults);
        assert_eq!(array.data, o(p).default_allocation.as_mut_ptr().cast());
        assert_eq!(array.num, get!(p, UStruct, properties_size));
        ptr::write_bytes(array.data, 0, array.num as usize);
        o(p).default_initialized = true;
        o(p).initialize_error
    }
}
unsafe extern "C" fn tagged_defaults(p: P, ar: P) -> i32 {
    unsafe {
        if a(ar).loading {
            let array = get!(p, UClass, defaults);
            ptr::copy_nonoverlapping(a(ar).defaults.as_ptr(), array.data.cast(), array.num as usize);
        } else { a(ar).bytes.extend_from_slice(&defaults(p)); }
    }
    0
}
unsafe extern "C" fn load_config(p: P) -> i32 {
    unsafe {
        let data = get!(p, UClass, defaults).data.cast::<u8>();
        data.write(data.read() + 10);
    }
    0
}
unsafe extern "C" fn load_localized(p: P) -> i32 {
    unsafe {
        if o(p).localized_error != 0 { return o(p).localized_error; }
        let data = get!(p, UClass, defaults).data.cast::<u8>();
        data.add(1).write(data.read() + 20);
    }
    0
}
unsafe extern "C" fn flush_localized() -> i32 { unsafe { (*host()).flushed += 1; } 0 }
unsafe extern "C" fn count_defaults(p: P, ar: P) -> i32 {
    unsafe { a(ar).counted += get!(p, UClass, defaults).num as usize; }
    0
}
unsafe extern "C" fn binary_defaults(p: P, ar: P) -> i32 {
    unsafe { a(ar).bytes.extend_from_slice(&defaults(p)); }
    0
}
unsafe extern "C" fn system_config(p: P) -> i32 { unsafe { o(p).system_config = true; } 0 }
unsafe extern "C" fn script_handler(_: P, _: P, result: P) -> i32 {
    unsafe { result.cast::<u32>().write(11); } 0
}
unsafe extern "C" fn native_handler(_: P, _: P, result: P) -> i32 {
    unsafe { result.cast::<u32>().write(22); } 0
}
unsafe extern "C" fn dynamic_handler(_: P, _: P, result: P) -> i32 {
    unsafe { result.cast::<u32>().write(33); } 0
}
const HANDLERS: [Option<Hp2VmNativeHandler>; 4] = [None, Some(script_handler), Some(dynamic_handler), Some(native_handler)];
unsafe fn invoke(p: P) -> u32 {
    unsafe {
        let handler = ptr::addr_of!((*p.cast::<UFunction>()).func)
            .cast::<Option<Hp2VmNativeHandler>>().read_unaligned().unwrap();
        let mut result = 0u32;
        assert_eq!(handler(p, ptr::null_mut(), (&mut result as *mut u32).cast()), 0);
        result
    }
}
unsafe extern "C" fn lookup_dynamic(p: P, result: *mut P) -> i32 {
    unsafe {
        if let Some(handler) = o(p).export_side_effect {
            put!(p, UFunction, func, handler as *const () as P);
        }
        result.write(o(p).export);
        o(p).export_error
    }
}
const S: Hp2ClassMetadataServices = Hp2ClassMetadataServices {
    pointer, integer, set_flags, super_serialize, serialize_value,
    serialize_expr, script_mismatch, check, link, array_allocator, initialize_default,
    tagged_defaults, load_config, load_localized, flush_localized, count_defaults,
    binary_defaults, system_config, link_struct: link, lookup_dynamic,
    script_handler, native_handlers: HANDLERS.as_ptr().cast(),
};

#[test]
fn function_wire_boundary_and_conditional_replication_field() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        put!(p, UFunction, parms_size, 0x1122);
        put!(p, UFunction, i_native, 0x3344);
        put!(p, UFunction, num_parms, 3);
        put!(p, UFunction, oper_precedence, 5);
        put!(p, UFunction, rep_offset, 0x1234);
        put!(p, UFunction, return_value_offset, 0x5566);
        for version in [63, 64] {
            for flags in [0, METADATA_FUNC_NET] {
                put!(p, UFunction, function_flags, flags);
                let mut ar = Archive::new(version, false, true);
                metadata::serialize(f.storage(), p, ar.pointer(), MetadataFunction, &S).unwrap();
                let mut expected = if version == 63 {
                    vec![0x22, 0x11, 0x44, 0x33, 3, 5, 0x66, 0x55]
                } else { vec![0x44, 0x33, 5] };
                expected.extend_from_slice(&flags.to_le_bytes());
                if flags != 0 { expected.extend_from_slice(&[0x34, 0x12]); }
                assert_eq!(ar.bytes, expected);
            }
        }
    }
}

#[test]
fn loaded_parameter_layout_overrides_legacy_values_and_stops_at_first_local() {
    let mut f = Fixture::new();
    let p = f.add();
    let first = f.add();
    let ret = f.add();
    let local = f.add();
    let ignored = f.add();
    unsafe {
        put!(p, UStruct, children, first);
        for (current, next, offset, flags, size) in [
            (first, ret, 8, METADATA_CPF_PARM, 8),
            (ret, local, 24, METADATA_CPF_PARM | METADATA_CPF_RETURN, 4),
            (local, ignored, 128, 0, 8),
            (ignored, ptr::null_mut(), 144, METADATA_CPF_PARM | METADATA_CPF_RETURN, 8),
        ] {
            put!(current, UField, next, next);
            o(current).is_property = true;
            put!(current, UProperty, property_flags, flags);
            put!(current, UProperty, offset, offset);
            put!(current, UProperty, array_dim, 1);
            put!(current, UProperty, element_size, size);
        }
        let mut ar = Archive::new(63, true, false);
        ar.bytes = vec![0xff, 0xff, 0, 0, 0xee, 5, 0xfe, 0xff, 0, 0, 0, 0];
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataFunction, &S).unwrap();
        assert_eq!((get!(p, UFunction, num_parms), get!(p, UFunction, parms_size), get!(p, UFunction, return_value_offset)), (2, 28, 24));
        assert_eq!(ar.at, 12);
    }
}

#[test]
fn parameter_count_and_size_keep_native_byte_word_narrowing_and_adjacent_bytes() {
    let mut f = Fixture::new();
    let p = f.add();
    let mut previous: P = ptr::null_mut();
    unsafe {
        put!(p, UFunction, rep_offset, 0xabcd);
        put!(p, UFunction, func, dynamic_handler as *const () as P);
        for _ in 0..257 {
            let prop = f.add();
            o(prop).is_property = true;
            put!(prop, UProperty, property_flags, METADATA_CPF_PARM);
            put!(prop, UProperty, offset, 65535);
            put!(prop, UProperty, array_dim, 2);
            put!(prop, UProperty, element_size, 2);
            if previous.is_null() { put!(p, UStruct, children, prop); }
            else { put!(previous, UField, next, prop); }
            previous = prop;
        }
        let mut ar = Archive::new(64, true, false);
        ar.bytes = vec![0x34, 0x12, 0xab, 0, 0, 0, 0];
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataFunction, &S).unwrap();
        assert_eq!((get!(p, UFunction, num_parms), get!(p, UFunction, parms_size), get!(p, UFunction, return_value_offset)), (1, 3, 65535));
        assert_eq!(get!(p, UFunction, i_native), 0x1234);
        assert_eq!(get!(p, UFunction, oper_precedence), 0xab);
        assert_eq!(get!(p, UFunction, rep_offset), 0xabcd);
        assert_eq!(invoke(p), 33);
    }
}

#[test]
fn property_type_leaf_mutation_is_read_from_live_fields() {
    let mut f = Fixture::new();
    let p = f.add();
    let prop = f.add();
    unsafe {
        put!(p, UStruct, children, prop);
        o(prop).is_property = true;
        o(prop).property_mutation = true;
        let mut ar = Archive::new(64, true, false);
        ar.bytes = vec![0; 7];
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataFunction, &S).unwrap();
        assert_eq!((get!(p, UFunction, num_parms), get!(p, UFunction, parms_size), get!(p, UFunction, return_value_offset)), (1, 42, 27));
    }
}

#[test]
fn native_layout_preserves_unreflected_tail_and_stricter_registered_alignment() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        (*f.storage()).loading_layout = p;
        put!(p, UStruct, properties_size, 101);
        o(p).link_size = 100;
        o(p).link_align = 4;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 28];
        ar.bytes.extend_from_slice(&[3, 0, 0, 0, 7, 8, 9]);
        ar.bytes[20..24].copy_from_slice(&17i32.to_le_bytes());
        ar.bytes[24..28].copy_from_slice(&29i32.to_le_bytes());
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S).unwrap();
        assert_eq!((get!(p, UStruct, properties_size), get!(p, UStruct, properties_align)), (104, 8));
        assert_eq!(script(p), [7, 8, 9]);
        assert_eq!((get!(p, UStruct, line), get!(p, UStruct, text_pos)), (17, 29));
        (*f.storage()).loading_layout = ptr::null_mut();
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 32];
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S).unwrap();
        assert_eq!((get!(p, UStruct, properties_size), get!(p, UStruct, properties_align)), (100, 4));
        assert!(script(p).is_empty());
    }
}

#[test]
fn script_mismatch_and_expression_error_stop_before_property_linking() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        o(p).expr_stride = 2;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 28];
        ar.bytes.extend_from_slice(&[1, 0, 0, 0, 42]);
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S), Err(77));
        assert_eq!(script(p), [42]);
        assert!(!o(p).linked);
        o(p).expr_error = 78;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 28];
        ar.bytes.extend_from_slice(&[3, 0, 0, 0, 43]);
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S), Err(78));
        assert_eq!(script(p), [43, 0xa5, 0xa5]);
        assert!(!o(p).linked);
    }
}

#[test]
fn script_empty_add_observes_reentrant_live_count_and_preserves_failure_prefix() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        o(p).empty_reentry = true;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 28];
        ar.bytes.extend_from_slice(&[1, 0, 0, 0, 42]);
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S).unwrap();
        assert_eq!(script(p), [42, 0xa5, 0xa5]);
        let data = get!(p, UStruct, script).data;
        o(p).linked = false;
        o(p).allocation_error = 91;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 28];
        ar.bytes.extend_from_slice(&[2, 0, 0, 0]);
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S), Err(91));
        let array = get!(p, UStruct, script);
        assert_eq!((array.num, array.max, array.data), (0, 0, data));
        assert!(!o(p).linked);
    }
}

#[test]
fn failed_native_link_keeps_partial_layout_instead_of_applying_registered_maxima() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        (*f.storage()).loading_layout = p;
        put!(p, UStruct, properties_size, 100);
        o(p).link_size = 40;
        o(p).link_align = 4;
        o(p).link_error = 81;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 32];
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataStruct, &S), Err(81));
        assert_eq!((get!(p, UStruct, properties_size), get!(p, UStruct, properties_align)), (40, 4));
    }
}

#[test]
fn old_class_loading_applies_config_then_localization_before_publishing_completion() {
    let mut f = Fixture::new();
    let p = f.add();
    let old = f.add();
    unsafe {
        (*f.storage()).loading_layout = old;
        put!(p, UClass, class_unique, 99);
        let mut ar = Archive::new(61, true, false);
        ar.bytes = vec![0; 26];
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataClass, &S).unwrap();
        assert_eq!(defaults(p), [11, 31, 3, 4, 0, 0, 0, 0]);
        assert!(o(p).system_config);
        assert_eq!(get!(p, UObject, object_flags), METADATA_RF_PUBLIC_STANDALONE);
        assert_eq!(get!(p, UClass, class_unique), 0);
        assert_eq!(get!(p, UClass, class_within), (*f.host).root);
        assert_eq!(((*f.storage()).loading_layout, (*f.host).flushed), (old, 1));
    }
}

#[test]
fn class_errors_preserve_loading_scope_and_first_error_default_state() {
    let mut f = Fixture::new();
    let p = f.add();
    let old = f.add();
    unsafe {
        (*f.storage()).loading_layout = old;
        put!(p, UObject, object_flags, METADATA_RF_NATIVE);
        o(p).super_error = 82;
        let mut ar = Archive::new(79, true, false);
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataClass, &S), Err(82));
        assert_eq!((*f.storage()).loading_layout, p);
        assert!(defaults(p).is_empty());
        (*f.storage()).loading_layout = old;
        o(p).super_error = 0;
        o(p).localized_error = 83;
        put!(p, UClass, class_unique, 99);
        ar.bytes = vec![0; 34];
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataClass, &S), Err(83));
        assert_eq!(defaults(p), [11, 2, 3, 4, 0, 0, 0, 0]);
        assert_eq!((get!(p, UClass, class_unique), (*f.host).flushed, (*f.storage()).loading_layout), (99, 0, old));
    }
}

#[test]
fn nested_loading_scope_restores_before_class_fields_but_not_on_nested_error() {
    let mut f = Fixture::new();
    let outer = f.add();
    let inner = f.add();
    let old = f.add();
    unsafe {
        (*f.storage()).loading_layout = old;
        put!(outer, UClass, class_constructor, 1usize as P);
        put!(inner, UObject, object_flags, METADATA_RF_NATIVE);
        o(outer).nested_class = inner;
        o(outer).recurse_struct = true;
        o(outer).link_size = 4;
        o(outer).link_align = 4;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 34 + 32 + 34];
        metadata::serialize(f.storage(), outer, ar.pointer(), MetadataClass, &S).unwrap();
        assert_eq!(get!(outer, UStruct, properties_size), 8);
        assert_eq!((*f.storage()).loading_layout, old);
        assert_eq!(defaults(outer), [11, 31, 3, 4, 0, 0, 0, 0]);
        o(outer).nested_class = inner;
        o(inner).super_error = 82;
        assert_eq!(metadata::serialize(f.storage(), outer, ar.pointer(), MetadataClass, &S), Err(82));
        assert_eq!((*f.storage()).loading_layout, inner);
        let independent = MetadataStorage::default();
        assert!(independent.loading_layout.is_null());
    }
}

#[test]
fn class_save_and_reference_archives_preserve_defaults_and_reject_wrong_size() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        set_defaults(p, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let mut save = Archive::new(79, false, true);
        metadata::serialize(f.storage(), p, save.pointer(), MetadataClass, &S).unwrap();
        let mut expected = vec![0; 34];
        expected.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(save.bytes, expected);
        assert_eq!(save.counted, 0);
        let mut refs = Archive::new(79, false, false);
        metadata::serialize(f.storage(), p, refs.pointer(), MetadataClass, &S).unwrap();
        assert_eq!(refs.bytes, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(refs.counted, 8);
        set_defaults(p, &[1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(metadata::serialize(f.storage(), p, refs.pointer(), MetadataClass, &S), Err(74));
        assert_eq!(refs.counted, 8);
    }
}

#[test]
fn state_hash_overrides_in_order_without_rehashing_inherited_or_variable_fields() {
    let mut f = Fixture::new();
    let state = f.add();
    let parent = f.add();
    let inherited = f.add();
    let variable = f.add();
    let first = f.add();
    let second = f.add();
    unsafe {
        put!(state, UField, super_field, parent);
        ptr::addr_of_mut!((*parent.cast::<UState>()).vf_hash).cast::<P>().write(inherited);
        put!(parent, UStruct, children, inherited);
        o(inherited).is_struct = true;
        put!(inherited, UObject, outer, parent);
        put!(state, UStruct, children, variable);
        put!(variable, UField, next, first);
        for (field, next, name) in [(first, second, 256i32), (second, ptr::null_mut(), 512)] {
            o(field).is_struct = true;
            put!(field, UObject, name, name.to_ne_bytes());
            put!(field, UObject, outer, state);
            put!(field, UField, next, next);
        }
        metadata::state_link(state, ptr::null_mut(), 1, &S).unwrap();
        assert_eq!(get!(state, UState, vf_hash)[0], second);
        assert_eq!(get!(second, UField, hash_next), first);
        assert_eq!(get!(first, UField, hash_next), inherited);
        assert!(get!(inherited, UField, hash_next).is_null());
        assert_eq!(get!(parent, UState, vf_hash)[0], inherited);
        assert!(get!(variable, UField, hash_next).is_null());
    }
}

#[test]
fn state_hash_clears_without_parent_but_not_after_failed_super_link() {
    let mut f = Fixture::new();
    let state = f.add();
    let poison = f.add();
    unsafe {
        ptr::addr_of_mut!((*state.cast::<UState>()).vf_hash).cast::<P>().add(5).write(poison);
        o(state).link_error = 84;
        assert_eq!(metadata::state_link(state, ptr::null_mut(), 1, &S), Err(84));
        assert_eq!(get!(state, UState, vf_hash)[5], poison);
        o(state).link_error = 0;
        metadata::state_link(state, ptr::null_mut(), 1, &S).unwrap();
        assert!(get!(state, UState, vf_hash).iter().all(|field| field.is_null()));
    }
}

#[test]
fn function_binding_installs_callable_handlers_and_keeps_error_side_effects() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        put!(p, UFunction, func, dynamic_handler as *const () as P);
        put!(p, UFunction, i_native, 1);
        assert_eq!(metadata::function_bind(p, &S), Err(71));
        assert_eq!(invoke(p), 33);
        put!(p, UFunction, i_native, 0);
        metadata::function_bind(p, &S).unwrap();
        assert_eq!(invoke(p), 11);
        put!(p, UFunction, function_flags, METADATA_FUNC_NATIVE);
        put!(p, UFunction, i_native, 4096);
        assert_eq!(metadata::function_bind(p, &S), Err(72));
        assert_eq!(invoke(p), 11);
        put!(p, UFunction, i_native, 3);
        o(p).hardcoded_present = false;
        assert_eq!(metadata::function_bind(p, &S), Err(73));
        assert_eq!(invoke(p), 11);
        o(p).hardcoded_present = true;
        metadata::function_bind(p, &S).unwrap();
        assert_eq!(invoke(p), 22);
        o(p).checked_index = Some(2);
        metadata::function_bind(p, &S).unwrap();
        assert_eq!(invoke(p), 33);
        put!(p, UFunction, i_native, 0);
        o(p).export_side_effect = Some(script_handler);
        metadata::function_bind(p, &S).unwrap();
        assert_eq!(invoke(p), 11);
        let mut export: Option<Hp2VmNativeHandler> = Some(dynamic_handler);
        o(p).export = (&mut export as *mut Option<Hp2VmNativeHandler>).cast();
        metadata::function_bind(p, &S).unwrap();
        assert_eq!(invoke(p), 33);
        export = Some(native_handler);
        std::hint::black_box(&export);
        metadata::function_bind(p, &S).unwrap();
        assert_eq!(invoke(p), 22);
        o(p).export_error = 85;
        assert_eq!(metadata::function_bind(p, &S), Err(85));
        assert_eq!(invoke(p), 11);
    }
}

#[test]
fn class_load_storage_and_header_failures_stop_before_tagged_publication_and_recover() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        put!(p, UClass, class_unique, 99);
        o(p).allocation_error = 91;
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; 34];
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataClass, &S), Err(91));
        let array = get!(p, UClass, defaults);
        assert_eq!((array.num, array.max), (0, 8));
        assert!(array.data.is_null());
        assert!(!o(p).default_initialized);
        assert!(defaults(p).is_empty());
        assert_eq!(get!(p, UClass, class_unique), 99);
        assert_eq!((*f.host).flushed, 0);
        o(p).allocation_error = 0;
        o(p).initialize_error = 92;
        ar.at = 0;
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataClass, &S), Err(92));
        let array = get!(p, UClass, defaults);
        assert_eq!((array.num, array.max), (8, 8));
        assert!(o(p).default_initialized);
        assert_eq!(defaults(p), [0; 8]);
        assert_eq!(get!(p, UClass, class_unique), 99);
        assert_eq!((*f.host).flushed, 0);
        o(p).initialize_error = 0;
        ar.at = 0;
        metadata::serialize(f.storage(), p, ar.pointer(), MetadataClass, &S).unwrap();
        assert_eq!(defaults(p), [11, 31, 3, 4, 0, 0, 0, 0]);
        assert_eq!(get!(p, UClass, defaults).data, o(p).default_allocation.as_mut_ptr().cast());
        assert_eq!(get!(p, UClass, class_unique), 0);
        assert_eq!((*f.host).flushed, 1);
    }
}

#[test]
fn field_loading_clears_only_field_hash_not_object_hash_or_neighbors() {
    let mut f = Fixture::new();
    let p = f.add();
    let marker = f.add();
    unsafe {
        put!(p, UObject, hash_next, marker);
        put!(p, UField, hash_next, marker);
        put!(p, UField, next, marker);
        let mut save = Archive::new(79, false, true);
        metadata::serialize(f.storage(), p, save.pointer(), MetadataField, &S).unwrap();
        assert_eq!(get!(p, UField, hash_next), marker);
        let mut load = Archive::new(79, true, false);
        load.bytes = save.bytes;
        metadata::serialize(f.storage(), p, load.pointer(), MetadataField, &S).unwrap();
        assert!(get!(p, UField, hash_next).is_null());
        assert_eq!(get!(p, UObject, hash_next), marker);
        assert_eq!(get!(p, UField, next), marker);
    }
}

#[test]
fn state_archive_failure_keeps_scalar_prefix_and_does_not_visit_later_fields() {
    let mut f = Fixture::new();
    let p = f.add();
    unsafe {
        put!(p, UState, probe_mask, u64::MAX);
        put!(p, UState, ignore_mask, 0x8877665544332211);
        put!(p, UState, label_table_offset, 0x1234);
        put!(p, UState, state_flags, 0x55667788);
        let mut ar = Archive::new(79, true, false);
        ar.bytes = 0x1020304050607080u64.to_le_bytes().to_vec();
        ar.bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc]);
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataState, &S), Err(93));
        assert_eq!(get!(p, UState, probe_mask), 0x1020304050607080);
        assert_eq!(get!(p, UState, ignore_mask), 0x8877665544ccbbaa);
        assert_eq!(get!(p, UState, label_table_offset), 0x1234);
        assert_eq!(get!(p, UState, state_flags), 0x55667788);
        assert_eq!(ar.at, 11);
    }
}

#[test]
fn field_archive_failure_preserves_hash_until_both_references_complete() {
    let mut f = Fixture::new();
    let p = f.add();
    let marker = f.add();
    unsafe {
        put!(p, UField, super_field, marker);
        put!(p, UField, next, marker);
        put!(p, UField, hash_next, marker);
        let mut ar = Archive::new(79, true, false);
        ar.bytes = vec![0; std::mem::size_of::<P>()];
        assert_eq!(metadata::serialize(f.storage(), p, ar.pointer(), MetadataField, &S), Err(93));
        assert!(get!(p, UField, super_field).is_null());
        assert_eq!(get!(p, UField, next), marker);
        assert_eq!(get!(p, UField, hash_next), marker);
        assert_eq!(ar.at, std::mem::size_of::<P>());
    }
}
