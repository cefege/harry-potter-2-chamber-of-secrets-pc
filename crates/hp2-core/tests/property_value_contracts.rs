use hp2_abi::engine::generated::{
    TArrayRaw, UArrayProperty, UBoolProperty, UByteProperty, UClass, UEnum, UField,
    UFixedArrayProperty, UObject, UProperty, UStruct, UStructProperty,
};
use hp2_core::array_storage::Hp2ArrayServices;
use hp2_core::property_value::*;
use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ffi::c_void;
use std::ops::Deref;
use std::ptr::null_mut;
type Object = *mut c_void;
fn obj<T>(p: &mut T) -> Object {
    (p as *mut T).cast()
}
#[repr(C)]
struct Node {
    storage: UStruct,
    excluded: bool,
    enumeration: UEnum,
}
impl Default for Node {
    fn default() -> Self {
        // SAFETY: These compiler-probed views contain only integers/raw pointers.
        unsafe { std::mem::zeroed() }
    }
}
macro_rules! view {
    ($name:ident, $type:ty) => {
        fn $name(&mut self) -> &mut $type {
            // SAFETY: UStruct-sized aligned fixture storage covers every property view.
            unsafe { &mut *(&raw mut self.storage).cast::<$type>() }
        }
    };
}
impl Node {
    view!(property, UProperty);
    view!(boolean, UBoolProperty);
    view!(fixed, UFixedArrayProperty);
    view!(array, UArrayProperty);
    view!(structure, UStruct);
    view!(structure_property, UStructProperty);
    view!(field, UField);
    view!(object, UObject);

    fn name_index(&mut self) -> &mut i32 {
        // SAFETY: The probed FName slot is an aligned four-byte integer.
        unsafe { &mut *(&raw mut self.object().name).cast::<i32>() }
    }

    fn set_enum_count(&mut self, count: i32) {
        self.enumeration.names.num = count;
        let enumeration = if count == -1 {
            null_mut()
        } else {
            obj(&mut self.enumeration)
        };
        // SAFETY: The byte-property view fits in storage.
        unsafe { (*(&raw mut self.storage).cast::<UByteProperty>()).r#enum = enumeration };
    }
}
static PROPERTY_CLASS: [u64; std::mem::size_of::<UClass>() / 8] =
    [0; std::mem::size_of::<UClass>() / 8];
fn scalar() -> Node {
    let mut node = Node::default();
    node.property().array_dim = 1;
    node.property().element_size = 4;
    node.object().class = (&raw const PROPERTY_CLASS).cast_mut().cast();
    node
}

// The fixture owns real probed array headers and allocator-paired raw storage.
#[repr(transparent)]
struct Array(TArrayRaw);
impl Array {
    fn new(values: &[i32]) -> Self {
        let mut array = Self(TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        });
        unsafe {
            let max = values.len() as i32;
            let mut data = null_mut();
            assert_eq!(
                reallocate(
                    null_mut(),
                    null_mut(),
                    values.len() * 4,
                    &max,
                    4,
                    std::ptr::null(),
                    &mut data
                ),
                0
            );
            if !values.is_empty() {
                std::ptr::copy_nonoverlapping(values.as_ptr(), data.cast(), values.len());
            }
            array.0 = TArrayRaw {
                data,
                num: max,
                max,
            };
        }
        array
    }
}
impl Deref for Array {
    type Target = [i32];
    fn deref(&self) -> &[i32] {
        if self.0.num == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.0.data.cast(), self.0.num as usize) }
        }
    }
}
impl std::fmt::Debug for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}
impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}
impl<const N: usize> PartialEq<[i32; N]> for Array {
    fn eq(&self, other: &[i32; N]) -> bool {
        self.deref() == other
    }
}
impl Drop for Array {
    fn drop(&mut self) {
        unsafe {
            free_buffer(null_mut(), self.0.data);
        }
    }
}
unsafe extern "C" fn reallocate(
    _: Object,
    old: Object,
    bytes: usize,
    _: *const i32,
    _: i32,
    _: *const c_void,
    out: *mut Object,
) -> i32 {
    unsafe {
        let header = std::mem::size_of::<usize>();
        let data = if bytes == 0 {
            null_mut()
        } else {
            let base = alloc_zeroed(Layout::from_size_align(bytes + header, 8).unwrap());
            assert!(!base.is_null(), "fixture allocation failed");
            base.cast::<usize>().write(bytes);
            base.add(header).cast::<c_void>()
        };
        if !old.is_null() && bytes != 0 {
            let old_bytes = old.cast::<usize>().sub(1).read();
            std::ptr::copy_nonoverlapping(
                old.cast::<u8>(),
                data.cast::<u8>(),
                old_bytes.min(bytes),
            );
        }
        free_buffer(null_mut(), old);
        out.write(data);
    }
    0
}
unsafe extern "C" fn checked_size(_: Object, bytes: usize, out: *mut i32) -> i32 {
    let Ok(bytes) = i32::try_from(bytes) else {
        return 73;
    };
    unsafe { out.write(bytes) };
    0
}
unsafe extern "C" fn free_buffer(_: Object, data: Object) -> i32 {
    if !data.is_null() {
        unsafe {
            let base = data.cast::<usize>().sub(1);
            let bytes = base.read();
            dealloc(
                base.cast(),
                Layout::from_size_align(bytes + std::mem::size_of::<usize>(), 8).unwrap(),
            );
        }
    }
    0
}
const ARRAYS: Hp2ArrayServices = Hp2ArrayServices {
    context: null_mut(),
    realloc: reallocate,
    checked_int_size: checked_size,
    free: free_buffer,
};
unsafe extern "C" fn array_check_add(count: i32, num: i32, max: i32) -> i32 {
    if count >= 0 && num >= 0 && max >= num {
        0
    } else {
        74
    }
}
#[derive(Default)]
struct Context {
    loading: bool,
    wire: Vec<(Hp2ValueWire, u32, u32)>,
    input: Vec<u32>,
    fail_wire: usize,
    calls: usize,
    fail_call: usize,
    visited: Vec<(Object, usize)>,
    shrink_node: Object,
    shrink_array: Object,
    replace_next: Option<(Object, Object)>,
    publication: Vec<i32>,
    preloaded: bool,
    binary: bool,
    replace_struct: Option<(Object, Object)>,
    change_size: Option<(Object, i32)>,
    change_mask: Option<(Object, u32)>,
}
unsafe extern "C" fn integer(p: Object, f: Hp2ValueInteger) -> i32 {
    match f {
        // SAFETY: Loading queries receive the live fixture context.
        Hp2ValueInteger::ValueLoading => unsafe { i32::from((*p.cast::<Context>()).loading) },
    }
}
unsafe extern "C" fn pointer(p: Object, f: Hp2ValuePointer, out: *mut Object) -> i32 {
    // SAFETY: The callback receives valid native-view fixture storage/output.
    unsafe {
        *out = match f {
            Hp2ValuePointer::ValueSuper => (*p.cast::<UField>()).super_field,
            Hp2ValuePointer::ValuePropertyClass => (&raw const PROPERTY_CLASS).cast_mut().cast(),
        };
    }
    0
}
unsafe extern "C" fn call(
    p: Object,
    c: Object,
    op: Hp2ValueCall,
    a: Object,
    b: Object,
    out: *mut i32,
) -> i32 {
    unsafe {
        use Hp2ValueCall::*;
        let context = &mut *(c as *mut Context);
        if matches!(op, ValueCallMapIndex) {
            *out = if (*(p as *mut Node)).excluded { -1 } else { 7 };
            return 0;
        }
        if matches!(op, ValueCallPreload) {
            context.preloaded = true;
            if let Some((property, structure)) = context.replace_struct {
                (*(property as *mut Node)).structure_property().r#struct = structure;
            }
            return 0;
        }
        if matches!(op, ValueCallBinary) {
            context.binary = context.preloaded;
            context.visited.push((p, 0));
            return 0;
        }
        context.calls += 1;
        context.visited.push((p, a as usize));
        if context.fail_call == context.calls {
            return 71;
        }
        match op {
            ValueCallIdentical => {
                *out = i32::from(
                    a.cast::<i32>().read()
                        == if b.is_null() {
                            0
                        } else {
                            b.cast::<i32>().read()
                        },
                )
            }
            ValueCallCopySingle | ValueCallCopyComplete => {
                a.cast::<i32>().write(b.cast::<i32>().read())
            }
            ValueCallDestroy => {
                context.publication.push(a.cast::<i32>().read());
                a.cast::<i32>().write(-99);
            }
            ValueCallSerialize => {
                a.cast::<i32>().write(context.calls as i32 * 10);
            }
            ValueCallNetSerialize => {
                context.publication.push(a.cast::<i32>().read());
                *out = 0;
            }
            ValueCallObjectNet => *out = 0,
            _ => unreachable!(),
        }
        if !context.shrink_node.is_null() {
            let node = &mut *(context.shrink_node as *mut Node);
            node.property().array_dim = 1;
            node.fixed().count = 1;
        }
        if !context.shrink_array.is_null() {
            (*(context.shrink_array as *mut TArrayRaw)).num = 1;
        }
        if let Some((field, next)) = context.replace_next {
            (*(field as *mut Node)).field().next = next;
        }
        if let Some((field, size)) = context.change_size {
            (*(field as *mut Node)).property().element_size = size;
        }
    }
    0
}
unsafe extern "C" fn storage(op: Hp2ValueStorage, a: Object) -> i32 {
    unsafe {
        use Hp2ValueStorage::*;
        match op {
            ValueArrayDestroy => {
                let array = &mut *a.cast::<TArrayRaw>();
                free_buffer(null_mut(), array.data);
                array.data = null_mut();
                array.num = 0;
                array.max = 0;
            }
            ValueStringDestroy => (*(a as *mut Vec<i32>)).clear(),
        }
    }
    0
}
unsafe extern "C" fn wire(c: Object, kind: Hp2ValueWire, data: Object, bound: u32) -> i32 {
    unsafe {
        let context = &mut *(c as *mut Context);
        let value = match kind {
            Hp2ValueWire::ValueWireByte | Hp2ValueWire::ValueWireBits => {
                data.cast::<u8>().read() as u32
            }
            Hp2ValueWire::ValueWireShort => data.cast::<u16>().read() as u32,
            _ => data.cast::<u32>().read(),
        };
        let i = context.wire.len();
        context.wire.push((kind, bound, value));
        if context.loading {
            let value = context.input[i];
            match kind {
                Hp2ValueWire::ValueWireByte | Hp2ValueWire::ValueWireBits => {
                    data.cast::<u8>().write(value as u8)
                }
                Hp2ValueWire::ValueWireShort => data.cast::<u16>().write(value as u16),
                _ => data.cast::<u32>().write(value),
            }
        }
        if let Some((property, mask)) = context.change_mask {
            (*property.cast::<UBoolProperty>()).bit_mask = mask;
        }
        if context.fail_wire == i + 1 {
            return 72;
        }
    }
    0
}
const SERVICES: Hp2PropertyValueServices = Hp2PropertyValueServices {
    none_name: 19,
    vector_name: 1,
    rotator_name: 2,
    plane_name: 3,
    arrays: &ARRAYS,
    array_tag: std::ptr::null(),
    string_tag: std::ptr::null(),
    array_check_add,
    integer,
    pointer,
    call,
    storage,
    wire,
};
unsafe fn execute(
    kind: Hp2ValueKind,
    op: Hp2ValueOperation,
    node: &mut Node,
    context: &mut Context,
    a: Object,
    b: Object,
) -> Result<i32, i32> {
    unsafe { run(kind, op, obj(node), obj(context), a, b, 0, &SERVICES) }
}

#[test]
fn scalar_defaults_masks_nan_and_float_zero() {
    unsafe {
        let mut n = scalar();
        let mut c = Context::default();
        n.boolean().bit_mask = 0x20;
        let mut a = 0x80u32;
        let mut b = 0x21u32;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueCopySingle,
                &mut n,
                &mut c,
                obj(&mut a),
                obj(&mut b)
            ),
            Ok(1)
        );
        assert_eq!(a, 0xa0);
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut a),
                obj(&mut b)
            ),
            Ok(1)
        );
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut a),
                null_mut()
            ),
            Ok(0)
        );
        a = 0x80;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut a),
                null_mut()
            ),
            Ok(1)
        );
        let mut f = f32::NAN;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueFloat,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut f),
                obj(&mut f)
            ),
            Ok(0)
        );
        f = -0.0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueFloat,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut f),
                null_mut()
            ),
            Ok(1)
        );
        let mut object: Object = null_mut();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueObject,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut object),
                null_mut()
            ),
            Ok(1)
        );
        let mut name = 19i32;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueName,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut name),
                null_mut()
            ),
            Ok(1)
        );
        let mut x = Array::new(&[65i32, 98, 0]);
        let mut y = Array::new(&[97i32, 66, 0]);
        assert_eq!(
            execute(
                Hp2ValueKind::ValueString,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut x),
                obj(&mut y)
            ),
            Ok(1)
        );
        *x.0.data.cast::<i32>() = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueString,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut x),
                null_mut()
            ),
            Ok(1)
        );
    }
}
#[test]
fn bool_wire_is_canonical_and_failed_local_payload_does_not_publish() {
    unsafe {
        let mut n = scalar();
        n.boolean().bit_mask = 8;
        let mut c = Context {
            loading: true,
            input: vec![0],
            ..Context::default()
        };
        let mut value = 0xabu32;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut value),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(value, 0xa3);
        assert_eq!(c.wire, vec![(Hp2ValueWire::ValueWireBits, 1, 1)]);
        c.wire.clear();
        c.input = vec![9];
        c.fail_wire = 1;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut value),
                null_mut()
            ),
            Err(72)
        );
        assert_eq!(value, 0xa3);
        c.wire.clear();
        c.fail_wire = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut value),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(value, 0xab);
    }
}
#[test]
fn enum_bit_counts_and_scalar_archive_partial_publication() {
    unsafe {
        let mut n = scalar();
        let mut value = 0xffu8;
        for (count, bits) in [(-1, 8), (0, 0), (1, 0), (2, 1), (3, 2), (256, 8), (257, 9)] {
            n.set_enum_count(count);
            let mut c = Context::default();
            assert_eq!(
                execute(
                    Hp2ValueKind::ValueByte,
                    Hp2ValueOperation::ValueNetSerialize,
                    &mut n,
                    &mut c,
                    obj(&mut value),
                    null_mut()
                ),
                Ok(1)
            );
            assert_eq!(c.wire, vec![(Hp2ValueWire::ValueWireBits, bits, 255)]);
        }
        let mut c = Context {
            loading: true,
            input: vec![123],
            fail_wire: 1,
            ..Context::default()
        };
        let mut value = 7i32;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueInt,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut value),
                null_mut()
            ),
            Err(72)
        );
        assert_eq!(value, 123);
    }
}
#[test]
fn fixed_and_complete_loops_observe_live_bounds_and_matches_offsets() {
    unsafe {
        let mut inner = scalar();
        let mut n = scalar();
        n.fixed().count = 3;
        n.property().array_dim = 3;
        n.array().inner = obj(&mut inner);
        let mut c = Context {
            shrink_node: obj(&mut n),
            ..Context::default()
        };
        let mut dst = [0i32; 3];
        let mut src = [4, 5, 6];
        assert_eq!(
            execute(
                Hp2ValueKind::ValueFixed,
                Hp2ValueOperation::ValueCopySingle,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut src)
            ),
            Ok(1)
        );
        assert_eq!(dst, [4, 0, 0]);
        n.property().array_dim = 3;
        c.calls = 0;
        dst = [0; 3];
        assert_eq!(
            execute(
                Hp2ValueKind::ValueGeneric,
                Hp2ValueOperation::ValueCopyComplete,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut src)
            ),
            Ok(1)
        );
        assert_eq!(dst, [4, 0, 0]);
        n.property().offset = 4;
        c.shrink_node = null_mut();
        let mut zeros = [99i32, 98, 0];
        assert_eq!(
            run(
                Hp2ValueKind::ValueGeneric,
                Hp2ValueOperation::ValueMatches,
                obj(&mut n),
                obj(&mut c),
                obj(&mut zeros),
                null_mut(),
                1,
                &SERVICES
            ),
            Ok(1)
        );
    }
}
#[test]
fn array_load_publishes_resize_and_successful_prefix_only() {
    unsafe {
        let mut inner = scalar();
        let mut n = scalar();
        n.array().inner = obj(&mut inner);
        let mut values = Array::new(&[91, 92]);
        let mut c = Context {
            loading: true,
            input: vec![3],
            fail_call: 2,
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Err(71)
        );
        assert_eq!(values, [10, 0, 0]);
        assert_eq!(c.calls, 2);
        assert_eq!(c.wire[0].0, Hp2ValueWire::ValueWireIndex);
        c = Context::default();
        let mut empty = Array::new(&[]);
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut empty),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(0)
        );
    }
}
#[test]
fn array_comparison_captures_count_but_ctor_copy_and_destruction_use_live_count() {
    unsafe {
        let mut inner = scalar();
        inner.property().property_flags = 0x00400000;
        let mut n = scalar();
        n.array().inner = obj(&mut inner);
        let mut dst = Array::new(&[8, 9, 10]);
        let mut src = Array::new(&[8, 9, 10]);
        let mut c = Context {
            shrink_array: obj(&mut dst),
            ..Context::default()
        };
        // Shrinking only the live count keeps captured storage/count available to comparison.
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut src)
            ),
            Ok(1)
        );
        assert_eq!(c.calls, 3);
        dst = Array::new(&[77]);
        c.calls = 0;
        c.visited.clear();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueCopySingle,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut src)
            ),
            Ok(1)
        );
        assert_eq!(dst, [8]);
        assert_eq!(c.calls, 1);
        assert!(c.publication.is_empty());
        dst = Array::new(&[31, 32, 33]);
        c.calls = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueDestroy,
                &mut n,
                &mut c,
                obj(&mut dst),
                null_mut()
            ),
            Ok(1)
        );
        assert!(dst.is_empty());
        assert_eq!(c.publication, [31]);
        inner.property().property_flags = 0;
        c.shrink_array = null_mut();
        c.calls = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueCopySingle,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut src)
            ),
            Ok(1)
        );
        assert_eq!(dst, src);
        assert_eq!(c.calls, 0);
    }
}
#[test]
fn inherited_fields_are_live_and_network_filters_ignore_child_false() {
    unsafe {
        let mut first = scalar();
        let mut skipped = scalar();
        skipped.property().offset = 4;
        skipped.excluded = true;
        let mut last = scalar();
        last.property().offset = 8;
        let mut base = {
            let mut node = Node::default();
            node.structure().children = obj(&mut last);
            node
        };
        let mut ty = {
            let mut node = Node::default();
            node.structure().children = obj(&mut first);
            node.field().super_field = obj(&mut base);
            node
        };
        let mut n = {
            let mut node = scalar();
            node.structure_property().r#struct = obj(&mut ty);
            node
        };
        let mut c = Context {
            replace_next: Some((obj(&mut first), obj(&mut skipped))),
            ..Context::default()
        };
        let mut values = [11i32, 22, 33];
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(c.publication, [11, 33]);
        c = Context::default();
        let mut dst = [0i32; 3];
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueCopySingle,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut values)
            ),
            Ok(1)
        );
        assert_eq!(dst, values);
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut values)
            ),
            Ok(1)
        );
        dst[2] = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut dst),
                obj(&mut values)
            ),
            Ok(0)
        );
    }
}
#[test]
fn constructor_chain_and_preload_follow_reentrant_metadata() {
    unsafe {
        let mut field = scalar();
        field.property().offset = 4;
        let mut ty = {
            let mut node = Node::default();
            node.structure().constructor_link = obj(&mut field);
            node
        };
        let mut n = {
            let mut node = scalar();
            node.structure_property().r#struct = obj(&mut ty);
            node.property().array_dim = 2;
            node.property().element_size = 8;
            node
        };
        let mut values = [1i32, 2, 3, 4];
        let mut c = Context::default();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueDestroy,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(values, [1, -99, 3, -99]);
        assert_eq!(c.publication, [2, 4]);
        let mut replacement = Node::default();
        let replacement_ptr = obj(&mut replacement);
        c = Context {
            replace_struct: Some((obj(&mut n), replacement_ptr)),
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        assert!(c.binary);
        assert_eq!(c.visited, [(replacement_ptr, 0)]);
    }
}
#[test]
fn vector_compression_boundaries_truncate_and_load_publish_atomically() {
    unsafe {
        let mut ty = {
            let mut node = Node::default();
            *node.name_index() = 1;
            node
        };
        let mut n = {
            let mut node = scalar();
            node.structure_property().r#struct = obj(&mut ty);
            node
        };
        for (value, bits, bound, encoded) in [
            (0.9, 0, 4, 2),
            (1.9, 0, 4, 3),
            (2.9, 1, 8, 6),
            (-2.9, 1, 8, 2),
            (32767.0, 14, 65536, 65535),
            (32768.0, 15, 131072, 98304),
            (65536.0, 15, 131072, 131072),
        ] {
            let mut values = [value, 0.0f32, 0.0];
            let mut c = Context::default();
            assert_eq!(
                execute(
                    Hp2ValueKind::ValueStruct,
                    Hp2ValueOperation::ValueNetSerialize,
                    &mut n,
                    &mut c,
                    obj(&mut values),
                    null_mut()
                ),
                Ok(1)
            );
            assert_eq!(c.wire[0], (Hp2ValueWire::ValueWireBounded, 16, bits));
            assert_eq!(c.wire[1], (Hp2ValueWire::ValueWireBounded, bound, encoded));
            assert_eq!(c.wire.len(), 4);
            assert_eq!(values[0], value);
        }
        let mut values = [91.0f32, 92.0, 93.0];
        let mut c = Context {
            loading: true,
            input: vec![1, 2, 4, 7],
            fail_wire: 3,
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Err(72)
        );
        assert_eq!(values, [91.0, 92.0, 93.0]);
        assert_eq!(c.wire.len(), 3);
        c.wire.clear();
        c.fail_wire = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(values, [-2.0, 0.0, 3.0]);
    }
}
#[test]
fn rotator_absent_bytes_retain_initial_values_and_plane_uses_signed_shorts() {
    unsafe {
        let mut ty = {
            let mut node = Node::default();
            *node.name_index() = 2;
            node
        };
        let mut n = {
            let mut node = scalar();
            node.structure_property().r#struct = obj(&mut ty);
            node
        };
        let mut rotation = [0x1234i32, 0x0055, -1];
        let mut c = Context::default();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut rotation),
                null_mut()
            ),
            Ok(1)
        );
        use Hp2ValueWire::*;
        assert_eq!(
            c.wire,
            [
                (ValueWireBits, 1, 1),
                (ValueWireByte, 0, 0x12),
                (ValueWireBits, 1, 0),
                (ValueWireBits, 1, 1),
                (ValueWireByte, 0, 255)
            ]
        );
        c = Context {
            loading: true,
            input: vec![0, 1, 7, 0],
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut rotation),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(rotation, [0x1200, 0x0700, 0xff00]);
        *ty.name_index() = 3;
        let mut plane = [32768.9f32, -32769.9, 1.9, -1.9];
        c = Context::default();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut plane),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(
            c.wire,
            [
                (ValueWireShort, 0, 32768),
                (ValueWireShort, 0, 32767),
                (ValueWireShort, 0, 1),
                (ValueWireShort, 0, 65535)
            ]
        );
        c = Context {
            loading: true,
            input: vec![65535, 32768, 0, 32767],
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueNetSerialize,
                &mut n,
                &mut c,
                obj(&mut plane),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(plane, [-1.0, -32768.0, 0.0, 32767.0]);
    }
}

#[test]
fn fixed_stride_is_live_but_array_archive_stride_is_captured() {
    unsafe {
        let mut inner = scalar();
        let mut n = {
            let mut node = scalar();
            node.array().inner = obj(&mut inner);
            node.fixed().count = 3;
            node
        };
        let mut values = [0i32; 5];
        let mut c = Context {
            change_size: Some((obj(&mut inner), 8)),
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueFixed,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(values, [10, 0, 20, 0, 30]);
        inner.property().element_size = 4;
        let mut values = Array::new(&[0i32; 3]);
        c.calls = 0;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueSerialize,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(values, [10, 20, 30]);
    }
}

#[test]
fn constructor_failure_keeps_successful_prefix_and_string_destroy_visits_all_dimensions() {
    unsafe {
        let mut field = scalar();
        field.property().offset = 4;
        let mut ty = {
            let mut node = Node::default();
            node.structure().constructor_link = obj(&mut field);
            node
        };
        let mut n = {
            let mut node = scalar();
            node.structure_property().r#struct = obj(&mut ty);
            node.property().array_dim = 3;
            node.property().element_size = 8;
            node
        };
        let mut c = Context {
            fail_call: 2,
            ..Context::default()
        };
        let mut values = [1i32, 2, 3, 4, 5, 6];
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueDestroy,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Err(71)
        );
        assert_eq!(values, [1, -99, 3, 4, 5, 6]);
        assert_eq!(c.calls, 2);
        n.structure_property().r#struct = null_mut();
        c = Context::default();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueStruct,
                Hp2ValueOperation::ValueIdentical,
                &mut n,
                &mut c,
                obj(&mut values),
                null_mut()
            ),
            Ok(1)
        );
        let mut strings = [vec![65i32, 0], vec![66, 0], vec![67, 0]];
        n.property().element_size = std::mem::size_of::<Vec<i32>>() as i32;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueString,
                Hp2ValueOperation::ValueDestroy,
                &mut n,
                &mut c,
                obj(&mut strings),
                null_mut()
            ),
            Ok(1)
        );
        assert!(strings.iter().all(Vec::is_empty));
    }
}

#[test]
fn scalar_complete_copy_preserves_forward_overlap_and_live_metadata_dimension() {
    unsafe {
        let mut n = {
            let mut node = scalar();
            node.property().array_dim = 3;
            node
        };
        let mut c = Context::default();
        let mut values = [1i32, 2, 3, 4];
        let source = values.as_mut_ptr();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueInt,
                Hp2ValueOperation::ValueCopyComplete,
                &mut n,
                &mut c,
                source.add(1).cast(),
                source.cast()
            ),
            Ok(1)
        );
        assert_eq!(values, [1, 1, 1, 1]);

        let mut source = [1i32, 99, 98];
        let property = (&raw mut n).cast();
        let destination = (&raw mut n.property().array_dim).cast();
        assert_eq!(
            run(
                Hp2ValueKind::ValueInt,
                Hp2ValueOperation::ValueCopyComplete,
                property,
                obj(&mut c),
                destination,
                source.as_mut_ptr().cast(),
                0,
                &SERVICES
            ),
            Ok(1)
        );
        assert_eq!(
            (
                n.property().array_dim,
                n.property().element_size,
                n.property().offset
            ),
            (1, 4, 0)
        );
    }
}

#[test]
fn direct_string_assignment_uses_allocator_failure_and_self_identity_contracts() {
    unsafe {
        let mut node = scalar();
        let mut context = Context::default();
        let mut source = Array::new(&[65, 66, 0]);
        let mut destination = Array::new(&[90, 0]);
        let old_data = destination.0.data;
        let mut state = AllocationFailure {
            calls: 0,
            fail_at: 1,
        };
        let arrays = Hp2ArrayServices {
            context: obj(&mut state),
            realloc: failing_reallocate,
            ..ARRAYS
        };
        let services = Hp2PropertyValueServices {
            arrays: &arrays,
            ..SERVICES
        };
        let destination_ptr = obj(&mut destination);
        assert_eq!(
            run(
                Hp2ValueKind::ValueString,
                Hp2ValueOperation::ValueCopySingle,
                obj(&mut node),
                obj(&mut context),
                destination_ptr,
                destination_ptr,
                0,
                &services
            ),
            Ok(1)
        );
        assert_eq!(state.calls, 0, "self-assignment must not allocate");
        assert_eq!(
            run(
                Hp2ValueKind::ValueString,
                Hp2ValueOperation::ValueCopySingle,
                obj(&mut node),
                obj(&mut context),
                destination_ptr,
                obj(&mut source),
                0,
                &services
            ),
            Err(75)
        );
        assert_eq!((destination.0.num, destination.0.max), (3, 3));
        assert_eq!(
            destination.0.data, old_data,
            "failed realloc retains its old allocation"
        );
        assert_eq!(old_data.cast::<i32>().read(), 90);
        state.fail_at = 0;
        assert_eq!(
            run(
                Hp2ValueKind::ValueString,
                Hp2ValueOperation::ValueCopySingle,
                obj(&mut node),
                obj(&mut context),
                destination_ptr,
                obj(&mut source),
                0,
                &services
            ),
            Ok(1)
        );
        assert_eq!(destination, [65, 66, 0]);
    }
}

struct AllocationFailure {
    calls: usize,
    fail_at: usize,
}
unsafe extern "C" fn failing_reallocate(
    context: Object,
    old: Object,
    bytes: usize,
    max: *const i32,
    width: i32,
    tag: *const c_void,
    out: *mut Object,
) -> i32 {
    unsafe {
        let state = &mut *context.cast::<AllocationFailure>();
        state.calls += 1;
        if state.calls == state.fail_at {
            return 75;
        }
        reallocate(null_mut(), old, bytes, max, width, tag, out)
    }
}

#[test]
fn array_load_failure_preserves_resize_publication_and_native_diagnostic_order() {
    unsafe {
        let mut inner = scalar();
        let mut node = scalar();
        node.array().inner = obj(&mut inner);
        let mut values = Array::new(&[7, 8]);
        let mut context = Context {
            loading: true,
            input: vec![u32::MAX],
            ..Context::default()
        };
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueSerialize,
                &mut node,
                &mut context,
                obj(&mut values),
                null_mut()
            ),
            Err(74)
        );
        assert_eq!((values.0.num, values.0.max), (0, 0));
        assert!(values.0.data.is_null());
        assert_eq!(
            context.calls, 0,
            "invalid count stops before element dispatch"
        );
        let mut state = AllocationFailure {
            calls: 0,
            fail_at: 2,
        };
        let arrays = Hp2ArrayServices {
            context: obj(&mut state),
            realloc: failing_reallocate,
            ..ARRAYS
        };
        let services = Hp2PropertyValueServices {
            arrays: &arrays,
            ..SERVICES
        };
        context = Context {
            loading: true,
            input: vec![3],
            ..Context::default()
        };
        assert_eq!(
            run(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueSerialize,
                obj(&mut node),
                obj(&mut context),
                obj(&mut values),
                null_mut(),
                0,
                &services
            ),
            Err(75)
        );
        assert_eq!(state.calls, 2, "empty precedes add allocation");
        assert_eq!((values.0.num, values.0.max), (3, 36));
        assert!(values.0.data.is_null());
        assert_eq!(context.calls, 0);
    }
}

#[test]
fn raw_copy_overlap_and_packed_container_traversal_preserve_native_values() {
    unsafe {
        let mut node = scalar();
        node.property().element_size = 12;
        let mut context = Context::default();
        let mut values = [1i32, 2, 3, 4];
        let data = values.as_mut_ptr();
        assert_eq!(
            execute(
                Hp2ValueKind::ValueGeneric,
                Hp2ValueOperation::ValueCopySingle,
                &mut node,
                &mut context,
                data.add(1).cast(),
                data.cast()
            ),
            Ok(1)
        );
        assert_eq!(values, [1, 1, 2, 3]);
        let mut inner = scalar();
        node.array().inner = obj(&mut inner);
        let mut left = [11i32, 22];
        let mut right = [11i32, 22];
        let mut packed_left = [0u64; 3];
        let mut packed_right = [0u64; 3];
        let a = packed_left
            .as_mut_ptr()
            .cast::<u8>()
            .add(4)
            .cast::<TArrayRaw>();
        let b = packed_right
            .as_mut_ptr()
            .cast::<u8>()
            .add(4)
            .cast::<TArrayRaw>();
        a.write_unaligned(TArrayRaw {
            data: left.as_mut_ptr().cast(),
            num: 2,
            max: 2,
        });
        b.write_unaligned(TArrayRaw {
            data: right.as_mut_ptr().cast(),
            num: 2,
            max: 2,
        });
        assert_eq!(
            execute(
                Hp2ValueKind::ValueArray,
                Hp2ValueOperation::ValueIdentical,
                &mut node,
                &mut context,
                a.cast(),
                b.cast()
            ),
            Ok(1)
        );
        assert_eq!(context.calls, 2);
    }
}

#[test]
fn bool_archive_reentry_rereads_live_probed_mask() {
    unsafe {
        let mut node = scalar();
        node.boolean().bit_mask = 1;
        let mut context = Context {
            loading: true,
            input: vec![1],
            change_mask: Some((obj(&mut node), 8)),
            ..Context::default()
        };
        let mut value = 2u32;
        assert_eq!(
            execute(
                Hp2ValueKind::ValueBool,
                Hp2ValueOperation::ValueSerialize,
                &mut node,
                &mut context,
                obj(&mut value),
                null_mut()
            ),
            Ok(1)
        );
        assert_eq!(
            value, 10,
            "archive mutation changes the mask used for publication"
        );
        assert_eq!(context.wire, [(Hp2ValueWire::ValueWireByte, 1, 0)]);
    }
}
