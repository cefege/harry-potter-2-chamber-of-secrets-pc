use Hp2VmVariableOp::*;
use hp2_core::vm_variable::*;
use std::{cell::Cell, collections::VecDeque, ffi::c_void, ptr};
type Object = *mut c_void;
type Step = Box<dyn FnMut(*mut Fixture, Object, Object) -> i32>;
thread_local! { static ACTIVE: Cell<*mut Fixture> = const { Cell::new(ptr::null_mut()) }; }
#[derive(Default)]
struct Property {
    offset: i32,
    dim: i32,
    size: i32,
    mask: u32,
    is_bool: bool,
    is_array: bool,
    inner: Object,
    outer: Object,
    constructor: Object,
    next: Object,
}
impl Property {
    fn scalar(offset: i32) -> Box<Self> {
        Box::new(Self {
            offset,
            dim: 1,
            size: 4,
            ..Self::default()
        })
    }
    fn object(&mut self) -> Object {
        (self as *mut Self).cast()
    }
}
struct Array {
    count: i32,
    data: [u32; 16],
}
struct Fixture {
    script: [u8; 128],
    cursor: *mut u8,
    property: Object,
    address: *mut u8,
    locals: [u32; 16],
    defaults: [u32; 16],
    sink: [u8; 32],
    receiver: Object,
    table: Vec<Object>,
    steps: VecDeque<Step>,
    events: Vec<String>,
    failure: Option<&'static str>,
    trace_hook: Option<fn(*mut Fixture)>,
    warning_hook: Option<fn(*mut Fixture)>,
    resolve_hook: Option<fn(*mut Fixture)>,
    destroy_hook: Option<fn(Object)>,
    default_context: Object,
    compared: u32,
}
impl Fixture {
    fn new() -> Box<Self> {
        let mut f = Box::new(Self {
            script: [0; 128],
            cursor: ptr::null_mut(),
            property: ptr::null_mut(),
            address: ptr::null_mut(),
            locals: [0; 16],
            defaults: [0; 16],
            sink: [0xa5; 32],
            receiver: ptr::null_mut(),
            table: Vec::new(),
            steps: VecDeque::new(),
            events: Vec::new(),
            failure: None,
            trace_hook: None,
            warning_hook: None,
            resolve_hook: None,
            destroy_hook: None,
            default_context: ptr::null_mut(),
            compared: 1,
        });
        f.cursor = f.script.as_mut_ptr();
        f.receiver = f.defaults.as_mut_ptr().cast();
        f
    }
    fn position(&self) -> usize {
        unsafe { self.cursor.offset_from(self.script.as_ptr()) as usize }
    }
    fn index(&mut self, index: i32) {
        self.script[..4].copy_from_slice(&index.to_ne_bytes());
    }
    fn step(&mut self, step: impl FnMut(*mut Fixture, Object, Object) -> i32 + 'static) {
        self.steps.push_back(Box::new(step));
    }
    unsafe fn run(&mut self, op: Hp2VmVariableOp, out: Object) -> Result<(), i32> {
        ACTIVE.with(|a| a.set(self));
        let e = Hp2VmVariableExecution {
            frame: (self as *mut Self).cast(),
            object: self.locals.as_mut_ptr().cast(),
            code: &raw mut self.cursor,
            property: &raw mut self.property,
            address: &raw mut self.address,
        };
        let s = self.services();
        unsafe { execute(op, &e, out, &s) }
    }
    fn services(&mut self) -> Hp2VmVariableServices {
        Hp2VmVariableServices {
            frame_object,
            locals,
            resolve,
            field,
            link,
            step,
            dispatch,
            opcode_offset,
            trace,
            copy,
            warning,
            default_address,
            default_object,
            array_num,
            array_data,
            array_add,
            struct_outer,
            scratch: scratch_leaf,
            destroy,
            compare,
            invalid_bool,
            assignment_sink: self.sink.as_mut_ptr(),
            assignment_zero_size: 8,
            unwind_guard,
        }
    }
}
unsafe fn event(f: *mut Fixture, name: &str) -> i32 {
    unsafe {
        (*f).events.push(name.into());
        if (*f).failure == Some(name) { 37 } else { 0 }
    }
}
fn active() -> *mut Fixture {
    ACTIVE.with(Cell::get)
}
unsafe extern "C" fn frame_object(f: Object) -> Object {
    unsafe { (*f.cast::<Fixture>()).receiver }
}
unsafe extern "C" fn locals(f: Object) -> *mut u8 {
    unsafe { (*f.cast::<Fixture>()).locals.as_mut_ptr().cast() }
}
unsafe extern "C" fn resolve(index: i32, out: *mut Object) -> i32 {
    unsafe {
        let f = active();
        let status = event(f, &format!("resolve:{index}"));
        if status != 0 {
            return status;
        }
        if let Some(hook) = (*f).resolve_hook {
            hook(f);
        }
        out.write((&(*f).table)[index as usize]);
        0
    }
}
unsafe extern "C" fn field(p: Object, kind: Hp2VmVariableField) -> i32 {
    unsafe {
        let p = &*p.cast::<Property>();
        match kind {
            Hp2VmVariableField::VmVariableOffset => p.offset,
            Hp2VmVariableField::VmVariableArrayDim => p.dim,
            Hp2VmVariableField::VmVariableElementSize => p.size,
            Hp2VmVariableField::VmVariableBitMask => p.mask as i32,
            Hp2VmVariableField::VmVariableStructSize => p.size,
            Hp2VmVariableField::VmVariableIsBool => i32::from(p.is_bool),
        }
    }
}
unsafe extern "C" fn link(p: Object, kind: Hp2VmVariableLink) -> Object {
    unsafe {
        if p.is_null() {
            return ptr::null_mut();
        }
        let property = &*p.cast::<Property>();
        match kind {
            Hp2VmVariableLink::VmVariableArrayCast => {
                if property.is_array {
                    p
                } else {
                    ptr::null_mut()
                }
            }
            Hp2VmVariableLink::VmVariableInner => property.inner,
            Hp2VmVariableLink::VmVariableConstructor => property.constructor,
            Hp2VmVariableLink::VmVariableConstructorNext => property.next,
        }
    }
}
unsafe extern "C" fn step(f: Object, receiver: Object, out: Object) -> i32 {
    unsafe {
        let f = f.cast::<Fixture>();
        let status = event(f, "step");
        if status != 0 {
            return status;
        }
        let mut action = (*f).steps.pop_front().expect("unexpected evaluation");
        action(f, receiver, out)
    }
}
unsafe extern "C" fn dispatch(f: Object, receiver: Object, opcode: u8) -> i32 {
    unsafe {
        let status = event(f.cast(), &format!("dispatch:{opcode}"));
        if status != 0 {
            return status;
        }
        step(f, receiver, ptr::null_mut())
    }
}
unsafe extern "C" fn opcode_offset(f: Object, out: *mut i32) -> i32 {
    unsafe {
        let f = f.cast::<Fixture>();
        out.write((*f).position() as i32 - 1);
    }
    0
}
unsafe extern "C" fn trace(f: Object, _: Object, _: Hp2VmVariableOp, offset: i32) -> i32 {
    unsafe {
        let f = f.cast::<Fixture>();
        let status = event(f, &format!("trace:{offset}"));
        if let Some(hook) = (*f).trace_hook {
            hook(f);
        }
        status
    }
}
unsafe extern "C" fn copy(p: Object, out: Object, source: *mut u8, kind: Hp2VmVariableCopy) -> i32 {
    unsafe {
        let status = event(active(), "copy");
        if status != 0 {
            return status;
        }
        let property = &*p.cast::<Property>();
        let size = property.size
            * if matches!(kind, Hp2VmVariableCopy::VmVariableCopySingle) {
                1
            } else {
                property.dim
            };
        ptr::copy_nonoverlapping(source, out.cast(), size as usize);
    }
    0
}
unsafe extern "C" fn warning(f: Object, kind: Hp2VmVariableWarning, index: i32, bound: i32) -> i32 {
    unsafe {
        let f = f.cast::<Fixture>();
        let name = match kind {
            Hp2VmVariableWarning::VmVariableNullClass => "class",
            Hp2VmVariableWarning::VmVariableNullContext => "context",
            Hp2VmVariableWarning::VmVariableBounds => "bounds",
            Hp2VmVariableWarning::VmVariableNullAssignment => "assignment",
        };
        let status = event(f, &format!("warning:{name}:{index}:{bound}"));
        if let Some(hook) = (*f).warning_hook {
            hook(f);
        }
        status
    }
}
unsafe extern "C" fn default_address(object: Object, offset: i32, out: *mut *mut u8) -> i32 {
    unsafe {
        let f = active();
        let status = event(f, "default_address");
        if status != 0 {
            return status;
        }
        assert_eq!(object, (*f).locals.as_mut_ptr().cast());
        out.write(
            (*f).defaults
                .as_mut_ptr()
                .cast::<u8>()
                .offset(offset as isize),
        );
    }
    0
}
unsafe extern "C" fn default_object(_: Object, out: *mut Object) -> i32 {
    unsafe {
        let f = active();
        let status = event(f, "default_object");
        if status != 0 {
            return status;
        }
        out.write((*f).default_context);
    }
    0
}
unsafe extern "C" fn array_num(p: Object) -> i32 {
    unsafe { (*p.cast::<Array>()).count }
}
unsafe extern "C" fn array_data(p: Object) -> *mut u8 {
    unsafe { (*p.cast::<Array>()).data.as_mut_ptr().cast() }
}
unsafe extern "C" fn array_add(p: Object, size: i32, count: i32) -> i32 {
    unsafe {
        let f = active();
        let status = event(f, &format!("grow:{size}:{count}"));
        if status != 0 {
            return status;
        }
        let a = &mut *p.cast::<Array>();
        for i in a.count..a.count + count {
            a.data[i as usize] = 0;
        }
        a.count += count;
        (*f).property = ptr::null_mut();
        (*f).address = ptr::null_mut();
    }
    0
}
unsafe extern "C" fn struct_outer(p: Object, out: *mut Object) -> i32 {
    unsafe {
        let status = event(active(), "outer");
        if status != 0 {
            return status;
        }
        out.write((*p.cast::<Property>()).outer);
    }
    0
}
unsafe extern "C" fn scratch_leaf(
    e: *const Hp2VmVariableExecution,
    op: Hp2VmVariableOp,
    p: Object,
    structure: Object,
    size: i32,
    second_size: i32,
    out: Object,
) -> i32 {
    let mut first = [0xa5a5a5a5u32; 16];
    let mut second = [0xa5a5a5a5u32; 16];
    unsafe {
        assert!(size <= 64 && second_size <= 64);
        let s = (*active()).services();
        scratch(
            op,
            &*e,
            p,
            structure,
            first.as_mut_ptr().cast(),
            second.as_mut_ptr().cast(),
            out,
            &s,
        )
        .err()
        .unwrap_or(0)
    }
}
unsafe extern "C" fn destroy(p: Object, address: *mut u8) -> i32 {
    unsafe {
        let status = event(
            active(),
            &format!("destroy:{}", (*p.cast::<Property>()).offset),
        );
        if status != 0 {
            return status;
        }
        address.cast::<u32>().write(0);
        if let Some(hook) = (*active()).destroy_hook {
            hook(p);
        }
    }
    0
}
unsafe extern "C" fn compare(_: Object, a: *mut u8, b: *mut u8, out: *mut u32) -> i32 {
    unsafe {
        let status = event(active(), "compare");
        if status != 0 {
            return status;
        }
        assert_eq!(a.cast::<u32>().read(), 11);
        assert_eq!(b.cast::<u32>().read(), 22);
        out.write((*active()).compared);
    }
    0
}
unsafe extern "C" fn invalid_bool() -> i32 {
    unsafe { event(active(), "invalid_bool") };
    91
}
unsafe extern "C" fn unwind_guard(f: Object, which: i32) -> i32 {
    unsafe { event(f.cast::<Fixture>(), &format!("unwind_guard:{which}")) }
}
fn ptr_out<T>(v: &mut T) -> Object {
    (v as *mut T).cast()
}
fn variable(f: &mut Fixture, p: Object, address: *mut u8) {
    f.step(move |f, _, out| unsafe {
        assert!(out.is_null());
        (*f).property = p;
        (*f).address = address;
        0
    });
}
fn index_step(f: &mut Fixture, index: i32) {
    f.step(move |f, receiver, out| unsafe {
        assert_eq!(receiver, (*f).receiver);
        out.cast::<i32>().write(index);
        0
    });
}
fn struct_steps(f: &mut Fixture) {
    for value in [11u32, 22] {
        f.step(move |_, _, out| unsafe {
            assert_eq!(out.cast::<u32>().read(), 0);
            out.cast::<u32>().write(value);
            0
        });
    }
}
#[test]
fn local_uses_four_byte_index_and_publishes_without_result() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(4);
        f.table.push(p.object());
        f.index(0);
        f.locals[1] = 73;
        f.run(VmVariableLocal, ptr::null_mut()).unwrap();
        assert_eq!(f.position(), 4);
        assert_eq!(f.property, p.object());
        assert_eq!(f.address, f.locals.as_mut_ptr().cast::<u8>().add(4));
        f.cursor = f.script.as_mut_ptr();
        let mut out = 0;
        f.run(VmVariableLocal, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 73);
    }
}
#[test]
fn operand_failure_keeps_cursor_and_existing_publication() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        f.property = p.object();
        f.address = f.locals.as_mut_ptr().cast();
        f.failure = Some("resolve:0");
        let mut out = 123u32;
        assert_eq!(f.run(VmVariableLocal, ptr_out(&mut out)), Err(37));
        assert_eq!(f.position(), 0);
        assert_eq!(f.property, p.object());
        assert_eq!(out, 123);
    }
}
#[test]
fn resolver_reentry_advances_live_cursor_not_saved_cursor() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        f.table.push(p.object());
        f.resolve_hook = Some(|f| unsafe {
            (*f).cursor = (*f).script.as_mut_ptr().add(20);
        });
        f.run(VmVariableLocal, ptr::null_mut()).unwrap();
        assert_eq!(f.position(), 24);
    }
}
#[test]
fn instance_trace_reentry_controls_copy_and_failure_keeps_publication() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        let mut other = Property::scalar(0);
        f.table = vec![p.object(), other.object()];
        f.defaults[2] = 89;
        f.trace_hook = Some(|f| unsafe {
            (*f).property = (&(*f).table)[1];
            (*f).address = (*f).defaults.as_mut_ptr().add(2).cast();
        });
        let mut out = 0;
        f.run(VmVariableInstance, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 89);
        assert_eq!(f.property, other.object());
        assert_eq!(f.events, ["resolve:0", "trace:-1", "copy"]);
        f.cursor = f.script.as_mut_ptr();
        f.failure = Some("trace:-1");
        out = 17;
        assert_eq!(f.run(VmVariableInstance, ptr_out(&mut out)), Err(37));
        assert_eq!(out, 17);
        assert_eq!(f.position(), 4);
    }
}
#[test]
fn defaults_use_class_storage_and_trace_original_callsite() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(8);
        f.table.push(p.object());
        f.defaults[2] = 51;
        let mut out = 0;
        f.run(VmVariableDefault, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 51);
        assert_eq!(
            f.events,
            ["resolve:0", "default_address", "trace:-1", "copy"]
        );
    }
}
#[test]
fn native_parameter_never_changes_property_and_null_result_leaves_address() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(4);
        let mut existing = Property::scalar(0);
        f.table.push(p.object());
        f.property = existing.object();
        f.address = f.defaults.as_mut_ptr().cast();
        let old = f.address;
        f.run(VmVariableNativeParm, ptr::null_mut()).unwrap();
        assert_eq!(f.address, old);
        assert_eq!(f.property, existing.object());
        f.cursor = f.script.as_mut_ptr();
        f.locals[1] = 41;
        let mut out = 0;
        f.run(VmVariableNativeParm, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 41);
        assert_eq!(f.property, existing.object());
        assert_ne!(f.address, old);
    }
}
#[test]
fn null_context_skips_unsigned_word_and_zeros_only_declared_width() {
    unsafe {
        for op in [VmVariableContext, VmVariableClassContext] {
            let mut f = Fixture::new();
            let mut p = Property::scalar(0);
            f.property = p.object();
            f.address = f.locals.as_mut_ptr().cast();
            f.script[..3].copy_from_slice(&[7, 0, 3]);
            f.step(|_, _, out| unsafe {
                out.cast::<Object>().write(ptr::null_mut());
                0
            });
            let mut out = [0xa5u8; 8];
            f.run(op, out.as_mut_ptr().cast()).unwrap();
            assert_eq!(f.position(), 10);
            assert_eq!(out, [0, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5]);
            assert!(f.property.is_null() && f.address.is_null());
        }
    }
}
#[test]
fn context_warning_failure_does_not_skip_or_clear_live_state() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        f.property = p.object();
        f.address = f.locals.as_mut_ptr().cast();
        f.failure = Some("warning:context:0:0");
        f.step(|_, _, _| 0);
        let mut out = 73;
        assert_eq!(f.run(VmVariableContext, ptr_out(&mut out)), Err(37));
        assert_eq!(out, 73);
        assert_eq!(f.position(), 0);
        assert_eq!(f.property, p.object());
        assert!(!f.address.is_null());
    }
}
#[test]
fn context_warning_reentry_changes_skip_operand_location() {
    unsafe {
        let mut f = Fixture::new();
        f.script[20..23].copy_from_slice(&[5, 0, 1]);
        f.warning_hook = Some(|f| unsafe {
            (*f).cursor = (*f).script.as_mut_ptr().add(20);
        });
        f.step(|_, _, _| 0);
        let mut out = [9u8; 2];
        f.run(VmVariableContext, out.as_mut_ptr().cast()).unwrap();
        assert_eq!(f.position(), 28);
        assert_eq!(out, [0, 9]);
    }
}
#[test]
fn successful_context_uses_receiver_and_default_failure_keeps_header_advance() {
    unsafe {
        for op in [VmVariableContext, VmVariableClassContext] {
            let mut f = Fixture::new();
            let target = f.defaults.as_mut_ptr().cast();
            f.default_context = target;
            f.step(move |f, receiver, out| unsafe {
                assert_eq!(
                    receiver,
                    if op == VmVariableContext {
                        (*f).locals.as_mut_ptr().cast()
                    } else {
                        (*f).receiver
                    }
                );
                out.cast::<Object>().write(target);
                0
            });
            f.step(move |f, receiver, out| unsafe {
                assert_eq!(receiver, target);
                assert_eq!((*f).position(), 3);
                out.cast::<u32>().write(77);
                0
            });
            let mut out = 0;
            f.run(op, ptr_out(&mut out)).unwrap();
            assert_eq!(out, 77);
        }
        let mut f = Fixture::new();
        let target = f.locals.as_mut_ptr().cast();
        f.step(move |_, _, out| unsafe {
            out.cast::<Object>().write(target);
            0
        });
        f.failure = Some("default_object");
        assert_eq!(f.run(VmVariableClassContext, ptr::null_mut()), Err(37));
        assert_eq!(f.position(), 3);
    }
}
#[test]
fn fixed_array_clamps_both_edges_and_rereads_property_after_warning() {
    unsafe {
        for index in [-4, 9] {
            let mut f = Fixture::new();
            let mut p = Property::scalar(0);
            p.dim = 3;
            let mut replacement = Property::scalar(0);
            replacement.dim = 2;
            f.table.push(replacement.object());
            f.defaults[..3].copy_from_slice(&[17, 29, 99]);
            index_step(&mut f, index);
            let base = f.locals.as_mut_ptr().cast();
            variable(&mut f, p.object(), base);
            f.warning_hook = Some(|f| unsafe {
                (*f).property = (&(*f).table)[0];
                (*f).address = (*f).defaults.as_mut_ptr().cast();
            });
            let mut out = 0;
            f.run(VmVariableArrayElement, ptr_out(&mut out)).unwrap();
            assert_eq!(out, if index < 0 { 17 } else { 29 });
            assert_eq!(f.property, replacement.object());
        }
    }
}
#[test]
fn dynamic_array_negative_does_not_grow_and_positive_growth_uses_saved_array() {
    unsafe {
        for index in [-1, 3] {
            let mut f = Fixture::new();
            let mut inner = Property::scalar(0);
            let mut p = Property::scalar(0);
            p.is_array = true;
            p.size = std::mem::size_of::<hp2_abi::engine::generated::TArrayRaw>() as i32;
            p.inner = inner.object();
            let mut array = Array {
                count: 1,
                data: [91; 16],
            };
            index_step(&mut f, index);
            variable(&mut f, p.object(), (&raw mut array).cast());
            let mut out = 77;
            f.run(VmVariableDynArrayElement, ptr_out(&mut out)).unwrap();
            assert_eq!(out, if index < 0 { 91 } else { 0 });
            assert_eq!(array.count, if index < 0 { 1 } else { 4 });
            assert_eq!(
                f.address,
                array.data.as_mut_ptr().add(index.max(0) as usize).cast()
            );
            if index >= 0 {
                assert!(f.property.is_null());
                assert!(f.events.iter().any(|e| e == "grow:4:3"));
            }
        }
    }
}
#[test]
fn array_growth_error_preserves_base_and_never_copies() {
    unsafe {
        let mut f = Fixture::new();
        let mut inner = Property::scalar(0);
        let mut p = Property::scalar(0);
        p.is_array = true;
        p.size = std::mem::size_of::<hp2_abi::engine::generated::TArrayRaw>() as i32;
        p.inner = inner.object();
        let mut array = Array {
            count: 1,
            data: [4; 16],
        };
        index_step(&mut f, 2);
        let base = (&raw mut array).cast();
        variable(&mut f, p.object(), base);
        f.failure = Some("grow:4:2");
        let mut out = 77;
        assert_eq!(f.run(VmVariableDynArrayElement, ptr_out(&mut out)), Err(37));
        assert_eq!(out, 77);
        assert_eq!(f.address, base);
        assert_eq!(array.count, 1);
    }
}
#[test]
fn dynamic_count_requires_both_live_slots_and_self_returns_actual_receiver() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        let mut array = Array {
            count: 7,
            data: [0; 16],
        };
        variable(&mut f, p.object(), (&raw mut array).cast());
        let mut out = 99;
        f.run(VmVariableDynArrayCount, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 7);
        variable(&mut f, ptr::null_mut(), (&raw mut array).cast());
        out = 99;
        f.run(VmVariableDynArrayCount, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 99);
        let mut object = ptr::null_mut::<c_void>();
        f.run(VmVariableSelf, ptr_out(&mut object)).unwrap();
        assert_eq!(object, f.locals.as_mut_ptr().cast());
    }
}
#[test]
fn bool_dispatch_restores_peeked_property_but_trace_can_replace_live_mask() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        p.mask = 1;
        let mut other = Property::scalar(0);
        other.mask = 8;
        f.table = vec![p.object(), other.object()];
        f.script[0] = 42;
        f.script[1..5].copy_from_slice(&0i32.to_ne_bytes());
        f.step(|f, _, _| unsafe {
            assert_eq!((*f).position(), 1);
            (*f).cursor = (*f).cursor.add(4);
            (*f).property = ptr::null_mut();
            (*f).address = (*f).locals.as_mut_ptr().cast();
            0
        });
        f.trace_hook = Some(|f| unsafe {
            assert_eq!((*f).property, (&(*f).table)[0]);
            (*f).property = (&(*f).table)[1];
            (*f).address = (*f).defaults.as_mut_ptr().cast();
        });
        f.defaults[0] = 8;
        let mut out = 99;
        f.run(VmVariableBool, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 1);
        assert_eq!(f.position(), 5);
        assert_eq!(f.property, other.object());
    }
}
#[test]
fn bool_dispatch_failure_does_not_restore_property_or_publish_result() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        f.table.push(p.object());
        f.script[0] = 42;
        f.failure = Some("dispatch:42");
        let mut out = 99;
        assert_eq!(f.run(VmVariableBool, ptr_out(&mut out)), Err(37));
        assert_eq!(f.position(), 1);
        assert!(f.property.is_null());
        assert_eq!(out, 99);
    }
}
#[test]
fn ordinary_assignment_rhs_receives_target_even_when_reentry_changes_globals() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        let target = f.locals.as_mut_ptr().cast();
        variable(&mut f, p.object(), target);
        f.step(move |f, receiver, out| unsafe {
            assert_eq!(receiver, (*f).receiver);
            assert_eq!(out, target.cast());
            (*f).address = (*f).defaults.as_mut_ptr().cast();
            out.cast::<u32>().write(123);
            0
        });
        f.run(VmVariableLet, ptr::null_mut()).unwrap();
        assert_eq!(f.locals[0], 123);
        assert_eq!(f.defaults[0], 0);
        assert_eq!(f.address, f.defaults.as_mut_ptr().cast());
    }
}
#[test]
fn null_assignment_zeros_only_string_width_and_warning_failure_stops_rhs() {
    unsafe {
        let mut f = Fixture::new();
        f.step(|_, _, _| 0);
        f.step(|f, _, out| unsafe {
            assert_eq!(out, (*f).sink.as_mut_ptr().cast());
            assert_eq!(&(&(*f).sink)[..8], &[0; 8]);
            assert_eq!((*f).sink[8], 0xa5);
            out.cast::<u32>().write(17);
            0
        });
        f.run(VmVariableLet, ptr::null_mut()).unwrap();
        assert_eq!(f.sink[0], 17);
        let mut f = Fixture::new();
        f.step(|_, _, _| 0);
        f.failure = Some("warning:assignment:0:0");
        assert_eq!(f.run(VmVariableLet, ptr::null_mut()), Err(37));
        assert_eq!(f.sink, [0xa5; 32]);
        assert!(f.address.is_null());
    }
}
#[test]
fn boolean_assignment_snapshots_target_and_property_but_reads_mask_after_rhs() {
    unsafe {
        for value in [0u32, 2] {
            let mut f = Fixture::new();
            let mut p = Property::scalar(0);
            p.is_bool = true;
            p.mask = 1;
            f.locals[0] = 0b1010;
            let target = f.locals.as_mut_ptr().cast();
            let property = p.object();
            variable(&mut f, property, target);
            f.step(move |f, _, out| unsafe {
                (*property.cast::<Property>()).mask = 2;
                (*f).property = ptr::null_mut();
                (*f).address = (*f).defaults.as_mut_ptr().cast();
                out.cast::<u32>().write(value);
                0
            });
            f.run(VmVariableLetBool, ptr::null_mut()).unwrap();
            assert_eq!(f.locals[0], if value == 0 { 8 } else { 10 });
            assert!(f.property.is_null());
            assert_eq!(f.defaults[0], 0);
        }
    }
}
#[test]
fn boolean_assignment_rhs_failure_leaves_target_and_invalid_type_fails_after_rhs() {
    unsafe {
        let mut f = Fixture::new();
        let mut p = Property::scalar(0);
        p.is_bool = true;
        p.mask = 1;
        f.locals[0] = 8;
        let target = f.locals.as_mut_ptr().cast();
        variable(&mut f, p.object(), target);
        f.step(|_, _, _| 37);
        assert_eq!(f.run(VmVariableLetBool, ptr::null_mut()), Err(37));
        assert_eq!(f.locals[0], 8);
        variable(&mut f, p.object(), target);
        p.is_bool = false;
        f.step(|_, _, out| unsafe {
            out.cast::<u32>().write(1);
            0
        });
        assert_eq!(f.run(VmVariableLetBool, ptr::null_mut()), Err(91));
        assert_eq!(f.locals[0], 8);
        assert_eq!(f.events.last().unwrap(), "invalid_bool");
    }
}
#[test]
fn struct_member_copies_scratch_not_lvalue_and_follows_live_destructor_links() {
    unsafe {
        let mut f = Fixture::new();
        let mut structure = Property::scalar(0);
        structure.size = 12;
        let mut member = Property::scalar(4);
        member.outer = structure.object();
        let mut first = Property::scalar(0);
        let mut second = Property::scalar(4);
        let mut replacement = Property::scalar(8);
        first.next = second.object();
        first.inner = replacement.object();
        structure.constructor = first.object();
        f.table.push(member.object());
        f.step(|f, _, out| unsafe {
            let words = out.cast::<u32>();
            assert_eq!(words.add(1).read(), 0);
            words.add(1).write(71);
            (*f).locals[1] = 99;
            (*f).address = (*f).locals.as_mut_ptr().cast();
            0
        });
        f.destroy_hook = Some(|p| unsafe {
            (*p.cast::<Property>()).next = (*p.cast::<Property>()).inner;
        });
        let mut out = 0;
        f.run(VmVariableStructMember, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 71);
        assert_eq!(f.address, f.locals.as_mut_ptr().add(1).cast());
        assert_eq!(f.property, member.object());
        assert_eq!(
            f.events,
            [
                "resolve:0",
                "outer",
                "step",
                "copy",
                "destroy:0",
                "destroy:8"
            ]
        );
    }
}
#[test]
fn struct_member_copy_and_destructor_errors_stop_cleanup_at_first_failure() {
    unsafe {
        for failure in ["copy", "destroy:0"] {
            let mut f = Fixture::new();
            let mut structure = Property::scalar(0);
            structure.size = 8;
            let mut member = Property::scalar(0);
            member.outer = structure.object();
            let mut first = Property::scalar(0);
            let mut second = Property::scalar(4);
            first.next = second.object();
            structure.constructor = first.object();
            f.table.push(member.object());
            f.failure = Some(failure);
            f.step(|_, _, out| unsafe {
                out.cast::<u32>().write(73);
                0
            });
            let mut out = 99;
            assert_eq!(f.run(VmVariableStructMember, ptr_out(&mut out)), Err(37));
            assert_eq!(out, if failure == "copy" { 99 } else { 73 });
            assert_eq!(f.property, member.object());
            assert_eq!(f.events.last().unwrap(), failure);
            assert!(!f.events.iter().any(|e| e == "destroy:4"));
        }
    }
}
#[test]
fn struct_member_null_result_still_destroys_and_step_error_does_not_publish_property() {
    unsafe {
        let mut f = Fixture::new();
        let mut structure = Property::scalar(0);
        let mut member = Property::scalar(0);
        member.outer = structure.object();
        let mut destructor = Property::scalar(0);
        structure.constructor = destructor.object();
        f.table.push(member.object());
        f.step(|_, _, _| 0);
        f.run(VmVariableStructMember, ptr::null_mut()).unwrap();
        assert_eq!(f.events.last().unwrap(), "destroy:0");
        assert!(!f.events.iter().any(|e| e == "copy"));
        f.cursor = f.script.as_mut_ptr();
        f.property = ptr::null_mut();
        f.address = f.locals.as_mut_ptr().cast();
        f.failure = Some("step");
        assert_eq!(f.run(VmVariableStructMember, ptr::null_mut()), Err(37));
        assert!(f.property.is_null() && f.address.is_null());
    }
}
#[test]
fn struct_comparisons_preserve_native_truth_value_and_never_destroy_scratch() {
    unsafe {
        for (op, comparison, expected) in [
            (VmVariableStructCmpEq, 7, 7),
            (VmVariableStructCmpNe, 7, 0),
            (VmVariableStructCmpNe, 0, 1),
        ] {
            let mut f = Fixture::new();
            let mut structure = Property::scalar(0);
            f.table.push(structure.object());
            f.compared = comparison;
            struct_steps(&mut f);
            let mut out = 99;
            f.run(op, ptr_out(&mut out)).unwrap();
            assert_eq!(out, expected);
            assert_eq!(f.events, ["resolve:0", "step", "step", "compare"]);
        }
        let mut f = Fixture::new();
        let mut structure = Property::scalar(0);
        f.table.push(structure.object());
        struct_steps(&mut f);
        f.failure = Some("compare");
        let mut out = 99;
        assert_eq!(f.run(VmVariableStructCmpEq, ptr_out(&mut out)), Err(37));
        assert_eq!(out, 99);
        assert_eq!(f.position(), 4);
    }
}

#[test]
fn trace_can_reenter_variable_execution_and_outer_copy_observes_nested_publication() {
    unsafe {
        let mut f = Fixture::new();
        let mut outer = Property::scalar(0);
        let mut nested = Property::scalar(8);
        f.table = vec![outer.object(), nested.object()];
        f.locals[0] = 11;
        f.locals[2] = 62;
        f.script[20..24].copy_from_slice(&1i32.to_ne_bytes());
        f.trace_hook = Some(|f| unsafe {
            (*f).cursor = (*f).script.as_mut_ptr().add(20);
            (*f).run(VmVariableLocal, ptr::null_mut()).unwrap();
        });
        let mut out = 0;
        f.run(VmVariableInstance, ptr_out(&mut out)).unwrap();
        assert_eq!(out, 62);
        assert_eq!(f.position(), 24);
        assert_eq!(f.property, nested.object());
        assert_eq!(f.events, ["resolve:0", "trace:-1", "resolve:1", "copy"]);
    }
}

#[test]
fn missing_array_base_leaves_result_and_failed_index_does_not_clear_property() {
    unsafe {
        for op in [VmVariableArrayElement, VmVariableDynArrayElement] {
            let mut f = Fixture::new();
            let mut p = Property::scalar(0);
            f.property = p.object();
            index_step(&mut f, 4);
            variable(&mut f, ptr::null_mut(), ptr::null_mut());
            let mut out = 91;
            f.run(op, ptr_out(&mut out)).unwrap();
            assert_eq!(out, 91);
            f.property = p.object();
            f.step(|_, _, _| 37);
            assert_eq!(f.run(op, ptr_out(&mut out)), Err(37));
            assert_eq!(f.property, p.object());
            assert_eq!(out, 91);
        }
    }
}

#[test]
fn missing_boolean_target_still_evaluates_rhs_without_reusing_nested_target() {
    unsafe {
        let mut f = Fixture::new();
        f.step(|_, _, _| 0);
        f.step(|f, _, out| unsafe {
            (*f).address = (*f).locals.as_mut_ptr().cast();
            (*f).locals[0] = 32;
            out.cast::<u32>().write(1);
            0
        });
        f.run(VmVariableLetBool, ptr::null_mut()).unwrap();
        assert_eq!(f.locals[0], 32);
        assert_eq!(f.events, ["step", "step"]);
    }
}

#[test]
fn comparison_second_expression_failure_keeps_first_expression_effects_and_result() {
    unsafe {
        let mut f = Fixture::new();
        let mut structure = Property::scalar(0);
        f.table.push(structure.object());
        f.step(|f, _, out| unsafe {
            out.cast::<u32>().write(11);
            (*f).locals[0] = 71;
            0
        });
        f.step(|f, _, out| unsafe {
            assert_eq!(out.cast::<u32>().read(), 0);
            (*f).locals[1] = 82;
            37
        });
        let mut out = 99;
        assert_eq!(f.run(VmVariableStructCmpNe, ptr_out(&mut out)), Err(37));
        assert_eq!(out, 99);
        assert_eq!(&f.locals[..2], &[71, 82]);
        assert_eq!(f.events, ["resolve:0", "step", "step"]);
    }
}
