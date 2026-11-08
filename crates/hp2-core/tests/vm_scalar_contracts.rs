use hp2_abi::engine::generated::FFrame;
use hp2_core::vm_frame::{Hp2VmFrameInitializationServices, Hp2VmNativeTraceServices};
use hp2_core::vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT};
use hp2_core::{ansi_math, vm_scalar as vm};
use std::cell::RefCell;
use std::ffi::{c_char, c_void};

#[test]
fn booleans_normalize_all_nonzero_bits() {
    for a in [0, 1, 2, 0x8000_0000, u32::MAX] {
        assert_eq!(vm::boolean(129, a, 0), u32::from(a == 0));
        for b in [0, 1, 2, 0x8000_0000, u32::MAX] {
            assert_eq!(vm::boolean(242, a, b), u32::from((a == 0) == (b == 0)));
            for op in [131, 243] {
                assert_eq!(vm::boolean(op, a, b), u32::from((a == 0) != (b == 0)));
            }
        }
    }
}

#[test]
fn integer_machine_boundaries_division_shifts_and_selection() {
    let cases = [
        (141, i32::MIN, 0, i32::MAX),
        (143, i32::MIN, 0, i32::MIN),
        (144, i32::MAX, 2, -2),
        (145, i32::MIN, -1, i32::MIN),
        (145, i32::MIN, 0, 0),
        (145, -7, 3, -2),
        (145, 7, -3, -2),
        (146, i32::MAX, 1, i32::MIN),
        (147, i32::MIN, 1, i32::MAX),
        (148, 1, -1, i32::MIN),
        (148, 7, 32, 7),
        (149, i32::MIN, 31, -1),
        (149, i32::MIN, 32, i32::MIN),
        (196, i32::MIN, -1, 1),
        (196, -1, 32, -1),
        (156, 0x55, 0x33, 0x11),
        (157, 0x55, 0x33, 0x66),
        (158, 0x55, 0x33, 0x77),
        (249, i32::MIN, i32::MAX, i32::MIN),
        (250, i32::MIN, i32::MAX, i32::MAX),
    ];
    for (op, a, b, expected) in cases {
        assert_eq!(vm::integer(op, a, b, 0), expected, "native {op}");
    }
    for a in [i32::MIN, -1, 0, 1, i32::MAX] {
        for b in [i32::MIN, -1, 0, 1, i32::MAX] {
            for (op, expected) in [
                (150, a < b),
                (151, a > b),
                (152, a <= b),
                (153, a >= b),
                (154, a == b),
                (155, a != b),
            ] {
                assert_eq!(vm::integer(op, a, b, 0), i32::from(expected));
            }
        }
    }
    // Clamp is nested comparisons, not a validated/sorted range.
    assert_eq!(vm::integer(251, 4, 10, 2), 10);
    assert_eq!(vm::integer(251, 12, 10, 2), 2);
    assert_eq!(vm::integer(251, 5, 0, 10), 5);
}

#[test]
fn byte_mutations_narrow_and_publish_after_lvalue() {
    for (op, old, rhs, new, returned) in [
        (133, 200, 2, 144, 144),
        (134, 255, 2, 127, 127),
        (134, 255, 0, 255, 0),
        (135, 250, 10, 4, 4),
        (136, 0, 1, 255, 255),
        (137, 255, 0, 0, 0),
        (138, 0, 0, 255, 255),
        (139, 255, 0, 0, 255),
        (140, 0, 0, 255, 0),
    ] {
        let mut a = old;
        let mut result = 99;
        unsafe { vm::byte_assign(op, &mut a, rhs, &mut result) };
        assert_eq!((a, result), (new, returned), "native {op}");
        let mut aliased = old;
        let p = &raw mut aliased;
        unsafe { vm::byte_assign(op, p, rhs, p) };
        assert_eq!(aliased, returned, "publication wins alias for native {op}");
    }
    // A zero denominator does not touch the lvalue at all.
    let mut result = 99;
    unsafe { vm::byte_assign(134, std::ptr::null_mut(), 0, &mut result) };
    assert_eq!(result, 0);
}

#[test]
fn integer_lvalue_rounding_zero_division_and_aliases() {
    for (op, old, b, f, expected, returned) in [
        (159, 16_777_217, 0, 1.0, 16_777_216, 16_777_216),
        (159, 3, 0, 1.5, 4, 4),
        (159, i32::MAX, 0, 2.0, i32::MAX, i32::MAX),
        (159, -2, 0, f32::INFINITY, i32::MIN, i32::MIN),
        (159, 5, 0, f32::NAN, 0, 0),
        (160, 17, 0, -0.0, 0, 0),
        (160, -7, 0, 2.0, -3, -3),
        (160, 17, 0, f32::NAN, 0, 0),
        (161, i32::MAX, 1, 0.0, i32::MIN, i32::MIN),
        (162, i32::MIN, 1, 0.0, i32::MAX, i32::MAX),
        (163, i32::MAX, 0, 0.0, i32::MIN, i32::MIN),
        (164, i32::MIN, 0, 0.0, i32::MAX, i32::MAX),
        (165, i32::MAX, 0, 0.0, i32::MIN, i32::MAX),
        (166, i32::MIN, 0, 0.0, i32::MAX, i32::MIN),
    ] {
        let mut a = old;
        let mut result = 99;
        unsafe { vm::integer_assign(op, &mut a, b, f, &mut result) };
        assert_eq!((a, result), (expected, returned), "native {op}");
        let mut a = old;
        let p = &raw mut a;
        unsafe { vm::integer_assign(op, p, b, f, p) };
        assert_eq!(a, returned, "native {op} alias");
    }
    // A captured address, not its pre-parser value: RHS/debug callbacks may
    // have changed the lvalue by the time the arithmetic native is entered.
    let mut a = 7;
    let p = &raw mut a;
    let rhs = a;
    a = 90;
    let mut result = 0;
    unsafe { vm::integer_assign(161, p, rhs, 0.0, &mut result) };
    assert_eq!((a, result), (97, 97));
}

#[test]
fn float_operations_preserve_ieee_selection_and_precision() {
    let negative_nan = f32::from_bits(0xffc1_2345);
    assert_eq!(vm::float(169, -0.0, 0.0, 0.0).to_bits(), 0);
    assert_eq!(vm::float(186, -0.0, 0.0, 0.0).to_bits(), 0x8000_0000);
    assert_eq!(
        vm::float(186, negative_nan, 0.0, 0.0).to_bits(),
        0x7fc1_2345
    );
    assert_eq!(vm::float(171, -0.0, 3.0, 0.0).to_bits(), 0x8000_0000);
    assert_eq!(vm::float(172, 1.0, -0.0, 0.0), f32::NEG_INFINITY);
    assert!(vm::float(172, 0.0, 0.0, 0.0).is_nan());
    assert_eq!(vm::float(173, -5.0, 2.0, 0.0), -1.0);
    assert!(vm::float(173, 5.0, 0.0, 0.0).is_nan());
    assert_eq!(vm::float(174, 16_777_216.0, 1.0, 0.0), 16_777_216.0);
    assert_eq!(vm::float(175, 9.0, 2.0, 0.0), 7.0);
    assert_eq!(vm::float(194, -3.0, 0.0, 0.0), 9.0);
    for op in [244, 245] {
        assert_eq!(vm::float(op, -0.0, 0.0, 0.0).to_bits(), 0x8000_0000);
        assert_eq!(vm::float(op, negative_nan, 2.0, 0.0), 2.0);
        assert_eq!(
            vm::float(op, 2.0, negative_nan, 0.0).to_bits(),
            negative_nan.to_bits()
        );
    }
    assert_eq!(vm::float(246, negative_nan, 0.0, 1.0), 1.0);
    assert_eq!(vm::float(246, 0.5, 2.0, 1.0), 2.0);
    for op in [176, 177, 178, 179, 180, 210] {
        assert_eq!(vm::float_compare(op, negative_nan, 1.0), 0);
    }
    assert_eq!(vm::float_compare(181, negative_nan, negative_nan), 1);
    assert_eq!(vm::float_compare(180, -0.0, 0.0), 1);
    assert_eq!(vm::float_compare(210, 0.0, 1.0e-4_f32), 1);
    assert_eq!(
        vm::float_compare(210, 0.0, f32::from_bits(1.0e-4_f32.to_bits() + 1)),
        0
    );
    for (op, old, rhs, expected) in [
        (182, 1.5, 2.0, 3.0),
        (183, 1.0, -0.0, f32::NEG_INFINITY),
        (184, 16_777_216.0, 1.0, 16_777_216.0),
        (185, -0.0, 0.0, -0.0),
    ] {
        let mut a = old;
        let mut result = 0.0;
        unsafe { vm::float_assign(op, &mut a, rhs, &mut result) };
        assert_eq!(a.to_bits(), expected.to_bits());
        assert_eq!(result.to_bits(), expected.to_bits());
        let p = &raw mut a;
        unsafe { vm::float_assign(op, p, rhs, p) };
        let arithmetic_op = match op {
            182 => 171,
            183 => 172,
            184 => 174,
            185 => 175,
            _ => unreachable!(),
        };
        assert_eq!(
            a.to_bits(),
            vm::float(arithmetic_op, expected, rhs, 0.0).to_bits()
        );
    }
}

#[test]
fn float_libm_uses_double_arguments_then_narrows() {
    for a in [-123.456_f32, -0.0, 0.0, 0.125, 0.9, 12.345] {
        for (op, function) in [
            (187, ansi_math::sin as fn(f64) -> f64),
            (188, ansi_math::cos),
            (189, ansi_math::tan),
            (190, ansi_math::atan),
            (191, ansi_math::exp),
            (192, ansi_math::loge),
            (193, ansi_math::sqrt),
        ] {
            let expected = function(a as f64) as f32;
            assert_eq!(
                vm::float(op, a, 0.0, 0.0).to_bits(),
                expected.to_bits(),
                "native {op}"
            );
        }
        assert_eq!(
            vm::float(170, a, 1.25, 0.0).to_bits(),
            (ansi_math::pow(a as f64, 1.25) as f32).to_bits()
        );
    }
}

#[test]
fn interpolation_keeps_native_fused_rounding() {
    // Product rounds to exactly one without contraction; FMADD retains -2^-46.
    let v = f32::from_bits(0x3f80_0001);
    let a = -1.0;
    let b = f32::from_bits(0xb400_0000);
    assert_eq!(
        vm::float(247, v, a, b).to_bits(),
        v.mul_add(b - a, a).to_bits()
    );
    assert_ne!(
        vm::float(247, v, a, b).to_bits(),
        (a + v * (b - a)).to_bits()
    );
    assert_eq!(vm::float(248, 0.0, 10.0, 20.0), 10.0);
    assert_eq!(vm::float(248, 1.0, 10.0, 20.0), 20.0);
    assert_eq!(vm::float(248, 0.25, 0.0, 1.0), 0.15625);
    assert_eq!(vm::float(248, 2.0, 0.0, 1.0), -4.0);
}

#[derive(Default)]
struct RandCtx {
    raw: i32,
    value: f32,
    rand_error: i32,
    trace_error: i32,
    traced: Option<(usize, usize, i32, i32, i32)>,
    events: Vec<&'static str>,
}

thread_local! {
    static RAND_CTX: RefCell<RandCtx> = RefCell::new(RandCtx::default());
}

unsafe extern "C" fn mock_rand(result: *mut i32) -> i32 {
    RAND_CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.events.push("rand");
        unsafe { result.write(c.raw) };
        c.rand_error
    })
}
unsafe extern "C" fn mock_frand(result: *mut f32) -> i32 {
    RAND_CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.events.push("frand");
        unsafe { result.write(c.value) };
        c.rand_error
    })
}
unsafe extern "C" fn mock_trace(
    object: *mut c_void,
    stack: *mut c_void,
    raw: i32,
    bound: i32,
    index: i32,
) -> i32 {
    RAND_CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.events.push("trace");
        c.traced = Some((object as usize, stack as usize, raw, bound, index));
        c.trace_error
    })
}
static RAND_SERVICES: vm::Hp2VmScalarRandServices = vm::Hp2VmScalarRandServices {
    rand: mock_rand,
    frand: mock_frand,
    trace: mock_trace,
};

#[test]
fn random_draw_computes_modulo_and_orders_draw_before_trace_errors() {
    for (rand_error, trace_error, raw, bound) in [(0, 0, 23, 10), (7, 0, 23, 10), (0, 9, 23, 10)] {
        RAND_CTX.with(|c| {
            *c.borrow_mut() = RandCtx {
                raw,
                rand_error,
                trace_error,
                ..Default::default()
            }
        });
        let mut result = 99;
        let object = 0x10usize as *mut c_void;
        let stack = 0x20usize as *mut c_void;
        let status = unsafe { vm::random_draw(object, stack, bound, &mut result, &RAND_SERVICES) };
        if rand_error == 0 {
            assert_eq!(
                status,
                if trace_error == 0 {
                    Ok(())
                } else {
                    Err(trace_error)
                }
            );
            assert_eq!(result, raw % bound);
            RAND_CTX.with(|c| {
                let c = c.borrow();
                assert_eq!(c.events, ["rand", "trace"]);
                assert_eq!(
                    c.traced,
                    Some((object as usize, stack as usize, raw, bound, raw % bound))
                );
            });
        } else {
            assert_eq!(status, Err(rand_error));
            assert_eq!(result, 99);
            RAND_CTX.with(|c| assert_eq!(c.borrow().events, ["rand"]));
        }
    }
}

#[test]
fn random_float_writes_frand_output_and_propagates_errors() {
    for rand_error in [0, 7] {
        RAND_CTX.with(|c| {
            *c.borrow_mut() = RandCtx {
                value: -0.0,
                rand_error,
                ..Default::default()
            }
        });
        let mut result = 99.0_f32;
        let status = unsafe { vm::random_float(&mut result, &RAND_SERVICES) };
        assert_eq!(
            status,
            if rand_error == 0 {
                Ok(())
            } else {
                Err(rand_error)
            }
        );
        assert_eq!(
            result.to_bits(),
            if rand_error == 0 {
                (-0.0_f32).to_bits()
            } else {
                99.0_f32.to_bits()
            }
        );
        RAND_CTX.with(|c| assert_eq!(c.borrow().events, ["frand"]));
    }
}

#[repr(C)]
struct RandInvocation {
    frame: FFrame,
    code: [u8; 8],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    bound: i32,
    events: Vec<&'static str>,
    scope_native: String,
    scope_native_slot: i32,
    scope_callsite_offset: i32,
    scope_begin_error: i32,
    scope_end_error: i32,
}

impl RandInvocation {
    fn new(code: [u8; 8], bound: i32) -> Self {
        Self {
            frame: FFrame {
                node: std::ptr::null_mut(),
                object: std::ptr::null_mut(),
                code: std::ptr::null_mut(),
                locals: std::ptr::null_mut(),
                parent: std::ptr::null_mut(),
            },
            code,
            table: [rand_unexpected; NATIVE_COUNT],
            bound,
            events: Vec::new(),
            scope_native: String::new(),
            scope_native_slot: 0,
            scope_callsite_offset: 0,
            scope_begin_error: 0,
            scope_end_error: 0,
        }
    }

    fn prepare(&mut self) {
        self.frame.code = self.code.as_mut_ptr();
        self.frame.node = (self as *mut Self).cast();
        self.table[0x80] = rand_operand;
        self.table[0x38] = rand_debug;
    }
}

unsafe extern "C" fn rand_unexpected(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    99
}
unsafe extern "C" fn rand_operand(_: *mut c_void, frame: *mut c_void, result: *mut c_void) -> i32 {
    unsafe {
        let p = frame.cast::<RandInvocation>();
        (*p).events.push("operand");
        result.cast::<i32>().write((*p).bound);
        0
    }
}
unsafe extern "C" fn rand_debug(_: *mut c_void, frame: *mut c_void, _: *mut c_void) -> i32 {
    unsafe {
        frame
            .cast::<RandInvocation>()
            .as_mut()
            .unwrap()
            .events
            .push("debug");
        0
    }
}
unsafe extern "C" fn rand_object_class(_: *mut c_void) -> *mut c_void {
    panic!("object_class must not be called by callsite_offset");
}
unsafe extern "C" fn rand_script_code(node: *mut c_void, offset: i32, out: *mut *mut u8) -> i32 {
    unsafe {
        let p = node.cast::<RandInvocation>();
        (*p).events.push("script_code");
        assert_eq!(offset, 0);
        out.write((*p).code.as_mut_ptr());
        0
    }
}
unsafe extern "C" fn rand_scope_begin(
    stack: *mut c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let p = stack.cast::<RandInvocation>();
        (*p).events.push("scope_begin");
        (*p).scope_native = std::ffi::CStr::from_ptr(native)
            .to_str()
            .unwrap()
            .to_string();
        (*p).scope_native_slot = native_slot;
        (*p).scope_callsite_offset = callsite_offset;
        out.write(stack);
        (*p).scope_begin_error
    }
}
unsafe extern "C" fn rand_scope_end(handle: *mut c_void) -> i32 {
    unsafe {
        let p = handle.cast::<RandInvocation>();
        (*p).events.push("scope_end");
        (*p).scope_end_error
    }
}
fn trace_services() -> Hp2VmNativeTraceServices {
    Hp2VmNativeTraceServices {
        frame: Hp2VmFrameInitializationServices {
            object_class: rand_object_class,
            script_code: rand_script_code,
        },
        scope_begin: rand_scope_begin,
        scope_end: rand_scope_end,
    }
}

#[test]
fn execute_rand_skips_the_scope_when_bound_is_not_positive() {
    for bound in [i32::MIN, -1, 0] {
        let mut invocation = RandInvocation::new([0x80, 0x16, 0x42, 0, 0, 0, 0, 0], bound);
        invocation.prepare();
        RAND_CTX.with(|c| *c.borrow_mut() = RandCtx::default());
        let trace = trace_services();
        let p = &raw mut invocation;
        let mut result = 0x1234_5678_i32;
        let status = unsafe {
            vm::execute_rand(
                std::ptr::null_mut(),
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &RAND_SERVICES,
                &trace,
            )
        };
        assert_eq!(status, Ok(()));
        assert_eq!(result, 0);
        assert_eq!(invocation.events, ["script_code", "operand"]);
        RAND_CTX.with(|c| assert!(c.borrow().events.is_empty()));
    }
}

#[test]
fn execute_rand_opens_the_scope_after_finish_and_orders_draw_before_end_errors() {
    for (draw_error, end_error, frame_events, ctx_events, status) in [
        (
            0,
            0,
            vec![
                "script_code",
                "operand",
                "debug",
                "scope_begin",
                "scope_end",
            ],
            vec!["rand", "trace"],
            Ok(()),
        ),
        (
            7,
            0,
            vec![
                "script_code",
                "operand",
                "debug",
                "scope_begin",
                "scope_end",
            ],
            vec!["rand"],
            Err(7),
        ),
        (
            0,
            9,
            vec![
                "script_code",
                "operand",
                "debug",
                "scope_begin",
                "scope_end",
            ],
            vec!["rand", "trace"],
            Err(9),
        ),
    ] {
        let mut invocation = RandInvocation::new([0x80, 0x16, 0x38, 0, 0, 0, 0, 0], 10);
        invocation.scope_end_error = end_error;
        invocation.prepare();
        RAND_CTX.with(|c| {
            *c.borrow_mut() = RandCtx {
                raw: 123,
                rand_error: draw_error,
                ..Default::default()
            }
        });
        let trace = trace_services();
        let p = &raw mut invocation;
        let object = 0x55usize as *mut c_void;
        let mut result = 99_i32;
        let status_got = unsafe {
            vm::execute_rand(
                object,
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &RAND_SERVICES,
                &trace,
            )
        };
        assert_eq!(status_got, status);
        assert_eq!(invocation.events, frame_events);
        assert_eq!(invocation.scope_native, "Core.Object.Rand");
        assert_eq!(invocation.scope_native_slot, 167);
        assert_eq!(invocation.scope_callsite_offset, 0);
        RAND_CTX.with(|c| assert_eq!(c.borrow().events, ctx_events));
        if draw_error == 0 {
            assert_eq!(result, 123 % 10);
            RAND_CTX.with(|c| {
                assert_eq!(
                    c.borrow().traced,
                    Some((object as usize, p as usize, 123, 10, 123 % 10))
                )
            });
        } else {
            assert_eq!(result, 99);
        }
    }
}

#[test]
fn execute_rand_propagates_a_scope_open_failure_without_drawing_or_closing() {
    let mut invocation = RandInvocation::new([0x80, 0x16, 0x42, 0, 0, 0, 0, 0], 10);
    invocation.scope_begin_error = 5;
    invocation.prepare();
    RAND_CTX.with(|c| *c.borrow_mut() = RandCtx::default());
    let trace = trace_services();
    let p = &raw mut invocation;
    let mut result = 99_i32;
    let status = unsafe {
        vm::execute_rand(
            std::ptr::null_mut(),
            std::ptr::addr_of_mut!((*p).frame),
            &raw mut result,
            std::ptr::addr_of!((*p).table).cast(),
            &RAND_SERVICES,
            &trace,
        )
    };
    assert_eq!(status, Err(5));
    assert_eq!(result, 99);
    assert_eq!(invocation.events, ["script_code", "operand", "scope_begin"]);
    RAND_CTX.with(|c| assert!(c.borrow().events.is_empty()));
}

#[test]
fn execute_frand_always_opens_the_scope_and_orders_draw_before_end_errors() {
    for (draw_error, end_error, ctx_events, status) in [
        (0, 0, vec!["frand"], Ok(())),
        (7, 0, vec!["frand"], Err(7)),
        (0, 9, vec!["frand"], Err(9)),
    ] {
        let mut invocation = RandInvocation::new([0x16, 0x42, 0, 0, 0, 0, 0, 0], 0);
        invocation.scope_end_error = end_error;
        invocation.prepare();
        RAND_CTX.with(|c| {
            *c.borrow_mut() = RandCtx {
                value: -0.5,
                rand_error: draw_error,
                ..Default::default()
            }
        });
        let trace = trace_services();
        let p = &raw mut invocation;
        let mut result = 99.0_f32;
        let status_got = unsafe {
            vm::execute_frand(
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &RAND_SERVICES,
                &trace,
            )
        };
        assert_eq!(status_got, status);
        assert_eq!(
            invocation.events,
            ["script_code", "scope_begin", "scope_end"]
        );
        assert_eq!(invocation.scope_native, "Core.Object.FRand");
        assert_eq!(invocation.scope_native_slot, 195);
        RAND_CTX.with(|c| assert_eq!(c.borrow().events, ctx_events));
        assert_eq!(
            result.to_bits(),
            if draw_error == 0 {
                (-0.5_f32).to_bits()
            } else {
                99.0_f32.to_bits()
            }
        );
    }
}

#[repr(C)]
struct ScalarInvocation {
    frame: FFrame,
    code: [u8; 8],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    slots: [*mut u32; 3],
    events: Vec<(u8, usize, usize)>,
    fail_at: usize,
    mutate: bool,
}

impl ScalarInvocation {
    fn new(code: [u8; 8]) -> Self {
        Self {
            frame: FFrame {
                node: std::ptr::null_mut(),
                object: 1usize as *mut c_void,
                code: std::ptr::null_mut(),
                locals: std::ptr::null_mut(),
                parent: std::ptr::null_mut(),
            },
            code,
            table: [scalar_unexpected; NATIVE_COUNT],
            slots: [std::ptr::null_mut(); 3],
            events: Vec::new(),
            fail_at: 0,
            mutate: false,
        }
    }

    fn prepare(&mut self) {
        self.frame.code = self.code.as_mut_ptr();
        self.table[0x80] = scalar_first;
        self.table[0x16] = scalar_omitted;
        self.table[0x38] = scalar_debug;
    }
}

unsafe extern "C" fn scalar_unexpected(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    99
}

unsafe fn scalar_record(p: *mut ScalarInvocation, object: *mut c_void, event: u8) -> i32 {
    unsafe {
        let offset = (*p)
            .frame
            .code
            .offset_from(std::ptr::addr_of!((*p).code).cast()) as usize;
        (*p).events.push((event, offset, object as usize));
        let count = (*p).events.len();
        if count == (*p).fail_at {
            70 + count as i32
        } else {
            0
        }
    }
}

unsafe extern "C" fn scalar_first(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ScalarInvocation>();
        (*p).slots[0] = result.cast();
        result.cast::<u32>().write(2);
        // The next occurrence of the same opcode must load this replacement.
        (*p).table[0x80] = scalar_later;
        (*p).frame.object = 2usize as *mut c_void;
        scalar_record(p, object, 1)
    }
}

unsafe extern "C" fn scalar_later(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ScalarInvocation>();
        let index = (*p).events.len();
        (*p).slots[index] = result.cast();
        result.cast::<u32>().write(3);
        if (*p).mutate {
            (*p).slots[0].write(40);
        }
        (*p).frame.object = (index + 2) as *mut c_void;
        scalar_record(p, object, 2)
    }
}

unsafe extern "C" fn scalar_debug(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ScalarInvocation>();
        if !result.is_null() {
            return 98;
        }
        let status = scalar_record(p, object, 3);
        if (*p).mutate {
            (*p).slots[0].write(50);
            (*p).slots[1].write(7);
        }
        // The caller must preserve even a failing debug callback's cursor edit.
        (*p).frame.code = (*p).frame.code.add(1);
        status
    }
}

unsafe extern "C" fn scalar_omitted(
    object: *mut c_void,
    frame: *mut c_void,
    _: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ScalarInvocation>();
        let status = scalar_record(p, object, 4);
        (*p).frame.code = (*p).frame.code.sub(1);
        status
    }
}

#[test]
fn direct_scalar_uses_live_arguments_object_and_registry_through_debug() {
    for debug in [false, true] {
        let token = if debug { 0x38 } else { 0x42 };
        let mut invocation = ScalarInvocation::new([0x80, 0x80, 0x16, token, 0, 0, 0, 0]);
        invocation.mutate = true;
        invocation.prepare();
        let p = &raw mut invocation;
        let mut output = [0xdead_beef_u32; 3];
        let status = unsafe {
            vm::execute_integer::<146, 2>(
                std::ptr::addr_of_mut!((*p).frame),
                output.as_mut_ptr().add(1).cast(),
                std::ptr::addr_of!((*p).table).cast(),
            )
        };
        assert_eq!(status, Ok(()));
        assert_eq!(
            output,
            [0xdead_beef, if debug { 57 } else { 43 }, 0xdead_beef]
        );
        let all_events = [(1, 1, 1), (2, 2, 2), (3, 4, 3)];
        assert_eq!(invocation.events, all_events[..if debug { 3 } else { 2 }]);
        let offset = if debug { 5 } else { 3 };
        assert_eq!(invocation.frame.code, unsafe {
            invocation.code.as_mut_ptr().add(offset)
        });
    }
}

#[test]
fn direct_scalar_steps_every_omitted_fixed_operand_and_ignores_non_debug_token() {
    // 0x42 is not the native DebugInfo opcode (0x38).
    let mut invocation = ScalarInvocation::new([0x16, 0x42, 0, 0, 0, 0, 0, 0]);
    invocation.prepare();
    let p = &raw mut invocation;
    let mut result = 99_i32;
    let status = unsafe {
        vm::execute_integer::<251, 3>(
            std::ptr::addr_of_mut!((*p).frame),
            &raw mut result,
            std::ptr::addr_of!((*p).table).cast(),
        )
    };
    assert_eq!(status, Ok(()));
    assert_eq!(result, 0);
    assert_eq!(invocation.events, [(4, 1, 1), (4, 1, 1), (4, 1, 1)]);
    assert_eq!(invocation.frame.code, unsafe {
        invocation.code.as_mut_ptr().add(1)
    });
}

#[test]
fn direct_scalar_first_errors_preserve_only_the_executed_prefix_and_never_publish() {
    for fail_at in 1..=4 {
        let mut invocation = ScalarInvocation::new([0x80, 0x80, 0x80, 0x16, 0x38, 0, 0, 0]);
        invocation.fail_at = fail_at;
        invocation.prepare();
        let p = &raw mut invocation;
        let mut result = 0x1234_5678_i32;
        let status = unsafe {
            vm::execute_integer::<251, 3>(
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
            )
        };
        assert_eq!(status, Err(70 + fail_at as i32));
        assert_eq!(result, 0x1234_5678);
        let all_events = [(1, 1, 1), (2, 2, 2), (2, 3, 3), (3, 5, 4)];
        assert_eq!(invocation.events, all_events[..fail_at]);
        let offset = if fail_at == 4 { 6 } else { fail_at };
        assert_eq!(invocation.frame.code, unsafe {
            invocation.code.as_mut_ptr().add(offset)
        });
    }
}

#[repr(C)]
struct BoolInvocation {
    frame: FFrame,
    code: [u8; 32],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    values: [u32; 2],
    calls: usize,
    fail_at: usize,
    nested: bool,
    output: u32,
}

impl BoolInvocation {
    fn new(lhs: u32, rhs: u32) -> Self {
        let mut code = [0; 32];
        code[..7].copy_from_slice(&[0x80, 0x18, 8, 0, 0x80, 0x16, 0x38]);
        code[12..16].copy_from_slice(&[0x81, 0x18, 4, 0]);
        Self {
            frame: FFrame {
                node: std::ptr::null_mut(),
                object: std::ptr::null_mut(),
                code: std::ptr::null_mut(),
                locals: std::ptr::null_mut(),
                parent: std::ptr::null_mut(),
            },
            code,
            table: [scalar_unexpected; NATIVE_COUNT],
            values: [lhs, rhs],
            calls: 0,
            fail_at: 0,
            nested: false,
            output: 99,
        }
    }

    fn prepare(&mut self) {
        self.frame.code = self.code.as_mut_ptr();
        self.table[0x80] = bool_operand;
        self.table[0x81] = bool_nested_operand;
    }
}

unsafe extern "C" fn bool_nested_operand(
    _: *mut c_void,
    _: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe { result.cast::<u32>().write(0x8000_0000) };
    0
}

unsafe extern "C" fn bool_operand(_: *mut c_void, frame: *mut c_void, result: *mut c_void) -> i32 {
    unsafe {
        let p = frame.cast::<BoolInvocation>();
        let index = (*p).calls;
        (*p).calls += 1;
        if (*p).output != 99 {
            return 98;
        }
        result.cast::<u32>().write((*p).values[index]);
        if index == 1 && (*p).nested {
            (*p).frame.code = std::ptr::addr_of_mut!((*p).code).cast::<u8>().add(12);
            let mut nested_result = 99;
            if let Err(error) = vm::execute_short_circuit::<132>(
                frame.cast(),
                &raw mut nested_result,
                std::ptr::addr_of!((*p).table).cast(),
            ) {
                return error;
            }
            if nested_result != 1 {
                return 97;
            }
        }
        if (*p).calls == (*p).fail_at { 37 } else { 0 }
    }
}

#[test]
fn direct_short_circuit_keeps_all_bool_bits_and_skips_rhs_side_effects_and_debug() {
    for or in [false, true] {
        for lhs in [0, 0x8000_0000] {
            for rhs in [0, 0x8000_0000] {
                let mut invocation = BoolInvocation::new(lhs, rhs);
                invocation.prepare();
                let p = &raw mut invocation;
                let status = unsafe {
                    let frame = std::ptr::addr_of_mut!((*p).frame);
                    let result = std::ptr::addr_of_mut!((*p).output);
                    let table = std::ptr::addr_of!((*p).table).cast();
                    if or {
                        vm::execute_short_circuit::<132>(frame, result, table)
                    } else {
                        vm::execute_short_circuit::<130>(frame, result, table)
                    }
                };
                let evaluates = (lhs == 0) == or;
                assert_eq!(status, Ok(()));
                assert_eq!(invocation.calls, if evaluates { 2 } else { 1 });
                assert_eq!(
                    invocation.output,
                    u32::from(if evaluates { rhs != 0 } else { lhs != 0 })
                );
                assert_eq!(invocation.frame.code, unsafe {
                    invocation
                        .code
                        .as_mut_ptr()
                        .add(if evaluates { 6 } else { 12 })
                });
            }
        }
    }
}

#[test]
fn direct_short_circuit_reentry_and_errors_preserve_live_cursor_and_unwritten_result() {
    for nested in [false, true] {
        for fail_at in 0..=2 {
            let mut invocation = BoolInvocation::new(2, 0);
            invocation.nested = nested;
            invocation.fail_at = fail_at;
            invocation.prepare();
            let p = &raw mut invocation;
            let status = unsafe {
                vm::execute_short_circuit::<130>(
                    std::ptr::addr_of_mut!((*p).frame),
                    std::ptr::addr_of_mut!((*p).output),
                    std::ptr::addr_of!((*p).table).cast(),
                )
            };
            assert_eq!(status, if fail_at == 0 { Ok(()) } else { Err(37) });
            assert_eq!(invocation.output, if fail_at == 0 { 0 } else { 99 });
            assert_eq!(invocation.calls, if fail_at == 1 { 1 } else { 2 });
            let offset = match (fail_at, nested) {
                (1, _) => 1,
                (2, false) => 5,
                (2, true) => 20,
                (_, false) => 6,
                (_, true) => 21,
            };
            assert_eq!(invocation.frame.code, unsafe {
                invocation.code.as_mut_ptr().add(offset)
            });
        }
    }
    let mut invocation = BoolInvocation::new(0, 2);
    invocation.code[2] = 0;
    invocation.fail_at = 2;
    invocation.prepare();
    let p = &raw mut invocation;
    unsafe {
        vm::execute_short_circuit::<130>(
            std::ptr::addr_of_mut!((*p).frame),
            std::ptr::addr_of_mut!((*p).output),
            std::ptr::addr_of!((*p).table).cast(),
        )
        .unwrap();
    }
    assert_eq!(invocation.calls, 1);
    assert_eq!(invocation.output, 0);
    assert_eq!(invocation.frame.code, unsafe {
        invocation.code.as_mut_ptr().add(4)
    });
}

// Adversarial GNatives callbacks exercise the parsing boundary, not a reflected
// property implementation. Real property-lvalue evidence remains native-mixed.
#[repr(C)]
struct ReferenceInvocation {
    frame: FFrame,
    code: [u8; 8],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    target: [u32; 3],
    decoy: i32,
    temporary: *mut c_void,
    rhs_slot: *mut c_void,
    rhs: u32,
    debug_lhs: u32,
    debug_rhs: u32,
    byte: bool,
    fallback: bool,
    calls: usize,
    fail_at: usize,
}

impl ReferenceInvocation {
    fn new(byte: bool, fallback: bool, lhs: u32, rhs: u32) -> Self {
        Self {
            frame: FFrame {
                node: std::ptr::null_mut(),
                object: std::ptr::null_mut(),
                code: std::ptr::null_mut(),
                locals: std::ptr::null_mut(),
                parent: std::ptr::null_mut(),
            },
            code: [0x80, 0x81, 0x16, 0x38, 0, 0, 0, 0],
            table: [scalar_unexpected; NATIVE_COUNT],
            target: [0xdead_beef, lhs, 0xdead_beef],
            decoy: 100,
            temporary: std::ptr::null_mut(),
            rhs_slot: std::ptr::null_mut(),
            rhs,
            debug_lhs: 40,
            debug_rhs: 7,
            byte,
            fallback,
            calls: 0,
            fail_at: 0,
        }
    }

    fn prepare(&mut self) {
        self.frame.code = self.code.as_mut_ptr();
        self.table[0x80] = reference_first;
        self.table[0x81] = reference_rhs;
        self.table[0x82] = reference_nested;
        self.table[0x38] = reference_debug;
    }
}

unsafe fn reference_write(byte: bool, slot: *mut c_void, value: u32) {
    unsafe {
        if byte {
            slot.cast::<u8>().write(value as u8)
        } else {
            slot.cast::<u32>().write(value)
        }
    }
}

unsafe fn reference_status(p: *mut ReferenceInvocation) -> i32 {
    unsafe {
        (*p).calls += 1;
        if (*p).calls == (*p).fail_at {
            70 + (*p).calls as i32
        } else {
            0
        }
    }
}

unsafe extern "C" fn reference_first(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ReferenceInvocation>();
        if !hp2_core::vm_variable::address_slot().read().is_null() {
            return 98;
        }
        (*p).temporary = result;
        if !(*p).fallback {
            let target = std::ptr::addr_of_mut!((*p).target).cast::<u32>().add(1);
            reference_write((*p).byte, result, target.read());
            hp2_core::vm_variable::address_slot().write(target.cast());
        }
        reference_status(p)
    }
}

unsafe extern "C" fn reference_nested(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = (*frame.cast::<FFrame>())
            .locals
            .cast::<ReferenceInvocation>();
        let decoy = std::ptr::addr_of_mut!((*p).decoy);
        result.cast::<i32>().write(decoy.read());
        hp2_core::vm_variable::address_slot().write(decoy.cast());
    }
    0
}

unsafe extern "C" fn reference_rhs(_: *mut c_void, frame: *mut c_void, result: *mut c_void) -> i32 {
    unsafe {
        let p = frame.cast::<ReferenceInvocation>();
        (*p).rhs_slot = result;
        reference_write((*p).byte, result, (*p).rhs);
        let mut code = [0x82, 0x16, 0];
        let mut nested = FFrame {
            node: std::ptr::null_mut(),
            object: std::ptr::null_mut(),
            code: code.as_mut_ptr(),
            locals: p.cast(),
            parent: frame.cast(),
        };
        let mut output = 0;
        if let Err(error) = vm::execute_integer_assign::<163>(
            &raw mut nested,
            &raw mut output,
            std::ptr::addr_of!((*p).table).cast(),
        ) {
            return error;
        }
        reference_status(p)
    }
}

unsafe extern "C" fn reference_debug(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ReferenceInvocation>();
        if !result.is_null() {
            return 97;
        }
        let target = if (*p).fallback {
            (*p).temporary
        } else {
            std::ptr::addr_of_mut!((*p).target)
                .cast::<u32>()
                .add(1)
                .cast()
        };
        reference_write((*p).byte, target, (*p).debug_lhs);
        if !(*p).rhs_slot.is_null() {
            reference_write((*p).byte, (*p).rhs_slot, (*p).debug_rhs);
        }
        (*p).frame.code = (*p).frame.code.add(1);
        reference_status(p)
    }
}

#[test]
fn direct_references_capture_before_reentry_finish_before_reads_and_preserve_aliases() {
    // One test serializes every use of the process-global interpreter lvalue slot.
    for kind in 0..3 {
        for fallback in [false, true] {
            for debug in [false, true] {
                for fail_at in 0..=if debug { 3 } else { 2 } {
                    let float = kind == 2;
                    let bits = |n: u32| if float { (n as f32).to_bits() } else { n };
                    let mut invocation =
                        ReferenceInvocation::new(kind == 0, fallback, bits(10), bits(3));
                    invocation.debug_lhs = bits(40);
                    invocation.debug_rhs = bits(7);
                    invocation.fail_at = fail_at;
                    if !debug {
                        invocation.code[3] = 0;
                    }
                    invocation.prepare();
                    let p = &raw mut invocation;
                    let mut output = [0xdead_beef_u32; 3];
                    let status = unsafe {
                        // A stale published address must be cleared before the LHS.
                        hp2_core::vm_variable::address_slot()
                            .write(std::ptr::addr_of_mut!((*p).decoy).cast());
                        let frame = std::ptr::addr_of_mut!((*p).frame);
                        let result = output.as_mut_ptr().add(1);
                        let table = std::ptr::addr_of!((*p).table).cast();
                        match kind {
                            0 => vm::execute_byte_assign::<135>(frame, result.cast(), table),
                            1 => vm::execute_integer_assign::<161>(frame, result.cast(), table),
                            _ => vm::execute_float_assign::<184>(frame, result.cast(), table),
                        }
                    };
                    assert_eq!(
                        status,
                        if fail_at == 0 {
                            Ok(())
                        } else {
                            Err(70 + fail_at as i32)
                        }
                    );
                    let value = bits(if debug {
                        47
                    } else if fallback {
                        3
                    } else {
                        13
                    });
                    let published = if fail_at != 0 {
                        0xdead_beef
                    } else if kind == 0 {
                        0xdead_be00 | value
                    } else {
                        value
                    };
                    assert_eq!(output, [0xdead_beef, published, 0xdead_beef]);
                    let target = if fallback {
                        bits(10)
                    } else if fail_at == 0 {
                        value
                    } else if fail_at == 3 {
                        bits(40)
                    } else {
                        bits(10)
                    };
                    assert_eq!(invocation.target, [0xdead_beef, target, 0xdead_beef]);
                    assert_eq!(invocation.decoy, if fail_at == 1 { 100 } else { 101 });
                    let offset = match fail_at {
                        1 => 1,
                        2 => 2,
                        _ if debug => 5,
                        _ => 3,
                    };
                    assert_eq!(invocation.frame.code, unsafe {
                        invocation.code.as_mut_ptr().add(offset)
                    });
                }
            }
        }
    }
    for byte in [false, true] {
        for post in [false, true] {
            let mut invocation = ReferenceInvocation::new(byte, false, 10, 0);
            invocation.code = [0x80, 0x16, 0x38, 0, 0, 0, 0, 0];
            invocation.prepare();
            let p = &raw mut invocation;
            unsafe {
                let frame = std::ptr::addr_of_mut!((*p).frame);
                let result = std::ptr::addr_of_mut!((*p).target).cast::<u32>().add(1);
                let table = std::ptr::addr_of!((*p).table).cast();
                match (byte, post) {
                    (true, true) => vm::execute_byte_assign::<139>(frame, result.cast(), table),
                    (true, false) => vm::execute_byte_assign::<137>(frame, result.cast(), table),
                    (false, true) => vm::execute_integer_assign::<165>(frame, result.cast(), table),
                    (false, false) => {
                        vm::execute_integer_assign::<163>(frame, result.cast(), table)
                    }
                }
                .unwrap();
            }
            assert_eq!(
                invocation.target,
                [0xdead_beef, if post { 40 } else { 41 }, 0xdead_beef]
            );
            assert_eq!(invocation.calls, 2);
        }
    }
    // An INT reference takes a FLOAT RHS; debug changes that same typed slot.
    let mut invocation = ReferenceInvocation::new(false, false, 10, 1.5_f32.to_bits());
    invocation.debug_lhs = 7;
    invocation.debug_rhs = 2.5_f32.to_bits();
    invocation.prepare();
    let p = &raw mut invocation;
    unsafe {
        let result = std::ptr::addr_of_mut!((*p).target)
            .cast::<u32>()
            .add(1)
            .cast();
        vm::execute_integer_assign::<159>(
            std::ptr::addr_of_mut!((*p).frame),
            result,
            std::ptr::addr_of!((*p).table).cast(),
        )
        .unwrap();
        hp2_core::vm_variable::address_slot().write(std::ptr::null_mut());
    }
    assert_eq!(invocation.target, [0xdead_beef, 17, 0xdead_beef]);
}
