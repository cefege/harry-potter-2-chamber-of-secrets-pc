use hp2_abi::{FCoords, FVector, engine::generated::FFrame};
use hp2_core::{
    math::{self, FRotator, NUM_ANGLES},
    vm_frame::{Hp2VmFrameInitializationServices, Hp2VmNativeTraceServices},
    vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT},
    vm_vector::{self as vm, Hp2VmVectorRandServices},
};
use std::{
    cell::RefCell,
    ffi::{c_char, c_void},
    ptr,
    sync::{LazyLock, Mutex},
};

fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
fn r(pitch: i32, yaw: i32, roll: i32) -> FRotator {
    FRotator { pitch, yaw, roll }
}
fn bits(a: FVector) -> [u32; 3] {
    [a.x.to_bits(), a.y.to_bits(), a.z.to_bits()]
}
fn close(a: FVector, b: FVector) {
    for (a, b) in [(a.x, b.x), (a.y, b.y), (a.z, b.z)] {
        assert!((a - b).abs() < 0.000002, "{a} != {b}");
    }
}
static TRIG: LazyLock<[f32; NUM_ANGLES]> = LazyLock::new(|| {
    let mut trig = [0.0; NUM_ANGLES];
    let mut sqrt = [0.0; 2 << math::APPROX_MAN_BITS];
    let mut inv_sqrt = [0.0; 1 << math::APPROX_MAN_BITS];
    let mut inv = [0.0; 1 << math::APPROX_MAN_BITS];
    let mut inv_sqrt_exp = [0.0; 1 << math::APPROX_EXP_BITS];
    let mut inv_exp = [0.0; 1 << math::APPROX_EXP_BITS];
    math::fill_tables(
        &mut trig,
        &mut sqrt,
        &mut inv_sqrt,
        &mut inv,
        &mut inv_sqrt_exp,
        &mut inv_exp,
    );
    trig
});
fn trig() -> &'static [f32; NUM_ANGLES] {
    &TRIG
}

#[test]
fn vector_arithmetic_keeps_native_rounding_and_ieee_boundaries() {
    let zero = v(0.0, 0.0, 0.0);
    assert_eq!(
        bits(vm::vector(211, v(0.0, -0.0, 1.0), zero, 0.0)),
        [0x80000000, 0, (-1.0f32).to_bits()]
    );
    for op in [212, 213, 221] {
        assert_eq!(
            bits(vm::vector(op, v(2.0, -3.0, 4.0), zero, -2.0)),
            bits(v(-4.0, 6.0, -8.0))
        );
    }
    for op in [296, 297] {
        assert_eq!(
            bits(vm::vector(op, v(2.0, -3.0, 4.0), v(3.0, 2.0, -1.0), 0.0)),
            bits(v(6.0, -6.0, -4.0))
        );
    }
    for op in [215, 223] {
        assert_eq!(
            bits(vm::vector(op, v(2.0, -3.0, 4.0), v(3.0, 2.0, -1.0), 0.0)),
            bits(v(5.0, -1.0, 3.0))
        );
    }
    for op in [216, 224] {
        assert_eq!(
            bits(vm::vector(op, v(2.0, -3.0, 4.0), v(3.0, 2.0, -1.0), 0.0)),
            bits(v(-1.0, -5.0, 5.0))
        );
    }
    for op in [214, 222] {
        let divided = vm::vector(op, v(0.0, 1.0, -1.0), zero, 0.0);
        assert!(divided.x.is_nan());
        assert_eq!(divided.y, f32::INFINITY);
        assert_eq!(divided.z, f32::NEG_INFINITY);
        // Reciprocal multiplication, not direct component division: reciprocal
        // of a subnormal overflows before the finite component can divide it.
        assert_eq!(
            vm::vector(op, v(f32::from_bits(1), 1.0, 1.0), zero, f32::from_bits(1)).x,
            f32::INFINITY
        );
    }
    assert_eq!(
        vm::vector_float(219, v(4097.0, -4096.0, 0.0), v(4097.0, 4098.0, 0.0)),
        1.0
    );
    assert_eq!(
        vm::vector(220, v(0.0, 4097.0, 4096.0), v(0.0, 4098.0, 4097.0), 0.0).x,
        1.0
    );
    assert_eq!(vm::vector_float(225, v(3.0, 4.0, 0.0), zero), 5.0);
    assert_eq!(
        bits(vm::vector(226, v(0.00005, 0.0, 0.0), zero, 0.0)),
        bits(zero)
    );
    close(
        vm::vector(226, v(0.0002, 0.0, 0.0), zero, 0.0),
        v(1.0, 0.0, 0.0),
    );
    assert!(vm::vector(226, v(f32::NAN, 0.0, 0.0), zero, 0.0).x.is_nan());
    assert_eq!(vm::vector_compare(217, zero, v(-0.0, 0.0, -0.0)), 1);
    assert_eq!(vm::vector_compare(217, v(f32::NAN, 0.0, 0.0), zero), 0);
    assert_eq!(vm::vector_compare(218, v(f32::NAN, 0.0, 0.0), zero), 1);
}

#[test]
#[cfg(target_arch = "aarch64")]
fn vector_nan_payloads_follow_native_operand_lane_order() {
    let a_nan = f32::from_bits(0x7fc01234);
    let b_nan = f32::from_bits(0xffc05678);
    let a = v(a_nan, a_nan, a_nan);
    let b = v(b_nan, b_nan, b_nan);
    for op in [212, 213, 221] {
        assert_eq!(
            bits(vm::vector(op, a, b, b_nan)),
            [a_nan.to_bits(), a_nan.to_bits(), b_nan.to_bits()]
        );
    }
    assert_eq!(bits(vm::vector(296, a, b, 0.0)), bits(a));
    assert_eq!(bits(vm::vector(297, a, b, 0.0)), bits(b));
    assert_eq!(bits(vm::vector(215, a, b, 0.0)), bits(a));
    assert_eq!(bits(vm::vector(223, a, b, 0.0)), bits(b));
}

#[test]
fn mirror_normalizes_and_doubles_normal_before_dot_product_scaling() {
    close(
        vm::vector(300, v(2.0, -3.0, 4.0), v(0.0, 5.0, 0.0), 0.0),
        v(2.0, 3.0, 4.0),
    );
    close(
        vm::vector(300, v(2.0, -3.0, 4.0), v(0.0, 0.0, 0.0), 0.0),
        v(2.0, -3.0, 4.0),
    );
    let reflected = vm::vector(300, v(f32::MAX, 0.0, 0.0), v(1.0, 0.0, 0.0), 0.0);
    assert_eq!(reflected.x, f32::NEG_INFINITY);
    // Doubling the dot first would produce 0*infinity => NaN in these lanes.
    assert_eq!(reflected.y, 0.0);
    assert_eq!(reflected.z, 0.0);
}

#[test]
fn compound_assignments_snapshot_aliases_before_result_publication() {
    let mut a = v(2.0, -3.0, 4.0);
    let p = ptr::addr_of_mut!(a);
    unsafe {
        vm::vector_assign(297, p, p, 0.0, p);
    }
    assert_eq!(bits(a), bits(v(4.0, 9.0, 16.0)));
    let mut slots = [1.0f32, 2.0, 3.0, 4.0];
    let p = slots.as_mut_ptr().cast::<FVector>();
    // Result overlaps lvalue by two components: result is the final write.
    unsafe {
        vm::vector_assign(221, p, ptr::null(), 2.0, p.cast::<f32>().add(1).cast());
    }
    assert_eq!(slots, [2.0, 2.0, 4.0, 6.0]);
    let mut rot = r(i32::MAX, i32::MIN, -1);
    let p = ptr::addr_of_mut!(rot);
    unsafe {
        vm::rotator_assign(318, p, p, 0.0, p);
    }
    assert_eq!(rot, r(-2, 0, -2));
    unsafe {
        vm::rotator_assign(319, p, p, 0.0, p);
    }
    assert_eq!(rot, r(0, 0, 0));
    unsafe {
        vm::rotator_assign(290, p, ptr::null(), f32::INFINITY, p);
    }
    assert_eq!(rot, r(0, 0, 0));
}

#[test]
fn compound_assignment_operand_and_result_partial_overlap_keeps_snapshots() {
    // B starts one lane into A; result starts one lane into B. Both input
    // snapshots must precede the lvalue store, and result must be the last store.
    let mut vector = [1.0_f32, 2.0, 3.0, 4.0, 99.0, 77.0];
    let p = vector.as_mut_ptr();
    unsafe {
        vm::vector_assign(223, p.cast(), p.add(1).cast(), 0.0, p.add(2).cast());
    }
    assert_eq!(vector, [3.0, 5.0, 3.0, 5.0, 7.0, 77.0]);
    let mut rotator = [i32::MAX, 1, -2, i32::MIN, 99, 77];
    let p = rotator.as_mut_ptr();
    unsafe {
        vm::rotator_assign(318, p.cast(), p.add(1).cast(), 0.0, p.add(2).cast());
    }
    assert_eq!(rotator, [i32::MIN, -1, i32::MIN, -1, i32::MAX - 1, 77]);
}

#[test]
fn rotator_wrapping_truncation_saturation_and_normalization() {
    let zero = r(0, 0, 0);
    assert_eq!(
        vm::rotator(316, r(i32::MAX, i32::MIN, -1), r(1, -1, 1), 0.0),
        r(i32::MIN, i32::MAX, 0)
    );
    assert_eq!(
        vm::rotator(317, r(i32::MIN, i32::MAX, 0), r(1, -1, 1), 0.0),
        r(i32::MAX, i32::MIN, -1)
    );
    for op in [287, 288, 290] {
        assert_eq!(vm::rotator(op, r(3, -3, 1), zero, 0.5), r(1, -1, 0));
        assert_eq!(
            vm::rotator(op, r(1, -1, 0), zero, f32::INFINITY),
            r(i32::MAX, i32::MIN, 0)
        );
        assert_eq!(
            vm::rotator(op, r(i32::MAX, i32::MIN, 5), zero, f32::NAN),
            zero
        );
    }
    for op in [289, 291] {
        assert_eq!(vm::rotator(op, r(3, -3, 1), zero, 2.0), r(1, -1, 0));
    }
    assert_eq!(
        vm::rotator(1031, r(32768, 65535, i32::MIN), zero, 0.0),
        r(-32768, -1, 0)
    );
    assert_eq!(
        vm::rotator(1031, r(32767, -32769, i32::MAX), zero, 0.0),
        r(32767, 32767, -1)
    );
    // Equality compares raw integers, not their equivalent normalized angles.
    assert_eq!(vm::rotator_compare(142, zero, r(65536, 0, 0)), 0);
    assert_eq!(vm::rotator_compare(203, zero, r(65536, 0, 0)), 1);
}

#[test]
fn rotation_axes_mask_angles_and_keep_rotation_direction() {
    let t = trig();
    let yaw = r(0, 16384, 0);
    close(
        vm::rotate(v(1.0, 0.0, 0.0), yaw, false, t),
        v(0.0, -1.0, 0.0),
    );
    close(vm::rotate(v(1.0, 0.0, 0.0), yaw, true, t), v(0.0, 1.0, 0.0));
    let a = v(0.25, -0.5, 0.75);
    let rot = r(12348, -23456, 34564);
    close(vm::rotate(vm::rotate(a, rot, false, t), rot, true, t), a);
    for unaxes in [false, true] {
        let c = vm::axes(rot, unaxes, t);
        let masked = vm::axes(
            r(rot.pitch + 65536, rot.yaw - 65536, rot.roll + 65536),
            unaxes,
            t,
        );
        assert_eq!(bits(c.x_axis), bits(masked.x_axis));
        assert_eq!(bits(c.y_axis), bits(masked.y_axis));
        assert_eq!(bits(c.z_axis), bits(masked.z_axis));
        let narrowed = vm::axes(r(rot.pitch + 3, rot.yaw + 3, rot.roll + 3), unaxes, t);
        assert_eq!(bits(c.x_axis), bits(narrowed.x_axis));
        let mut shared = v(99.0, 99.0, 99.0);
        let p = ptr::addr_of_mut!(shared);
        unsafe {
            vm::write_axes(c, p, p, p);
        }
        assert_eq!(bits(shared), bits(c.z_axis));
    }
    // Exercise yaw, pitch and roll without numerical idealization of table sin.
    for rot in [
        r(8192, 0, 0),
        r(0, 8192, 0),
        r(0, 0, 8192),
        r(4096, 8192, -4096),
    ] {
        let c = vm::axes(rot, false, t);
        let recovered = vm::ortho_rotation(c.x_axis, c.y_axis, c.z_axis, t);
        let c2 = vm::axes(recovered, false, t);
        for (a, b) in [
            (c.x_axis, c2.x_axis),
            (c.y_axis, c2.y_axis),
            (c.z_axis, c2.z_axis),
        ] {
            assert!(
                (a.x - b.x).abs() < 0.002 && (a.y - b.y).abs() < 0.002 && (a.z - b.z).abs() < 0.002
            );
        }
    }
}

#[test]
fn invert_handles_nonorthogonal_rows_and_aliased_publication() {
    let (mut x, mut y, mut z) = (v(2.0, 1.0, 0.0), v(0.0, 3.0, 1.0), v(0.0, 0.0, 4.0));
    unsafe {
        vm::invert(&mut x, &mut y, &mut z);
    }
    close(x, v(0.5, -1.0 / 6.0, 1.0 / 24.0));
    close(y, v(0.0, 1.0 / 3.0, -1.0 / 12.0));
    close(z, v(0.0, 0.0, 0.25));
    let (mut shared, mut z) = (v(1.0, 2.0, 3.0), v(2.0, 4.0, 6.0));
    let expected = math::coords_inverse(&FCoords {
        origin: v(0.0, 0.0, 0.0),
        x_axis: shared,
        y_axis: shared,
        z_axis: z,
    });
    let p = ptr::addr_of_mut!(shared);
    unsafe {
        vm::invert(p, p, &mut z);
    }
    assert_eq!(bits(shared), bits(expected.y_axis));
    assert_eq!(bits(z), bits(expected.z_axis));
}

#[derive(Debug, PartialEq)]
enum Event {
    Draw(usize),
    Trace(usize),
}
struct Random {
    ints: Vec<i32>,
    floats: Vec<f32>,
    events: Vec<Event>,
    draws: usize,
    fail_draw: Option<usize>,
    fail_trace: Option<usize>,
}
impl Random {
    fn floats(values: &[f32]) -> Self {
        Self {
            ints: vec![],
            floats: values.to_vec(),
            events: vec![],
            draws: 0,
            fail_draw: None,
            fail_trace: None,
        }
    }
    fn ints(values: &[i32]) -> Self {
        Self {
            ints: values.to_vec(),
            ..Self::floats(&[])
        }
    }
    fn begin(&mut self) -> Result<usize, i32> {
        let i = self.draws;
        self.draws += 1;
        self.events.push(Event::Draw(i));
        if self.fail_draw == Some(i) {
            return Err(71);
        }
        // Native appRand/appFrand emit their trace before returning the draw.
        self.events.push(Event::Trace(i));
        if self.fail_trace == Some(i) {
            return Err(72);
        }
        Ok(i)
    }
}
unsafe extern "C" fn rand(context: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        let c = &mut *context.cast::<Random>();
        match c.begin() {
            Ok(i) => {
                result.write(c.ints[i]);
                0
            }
            Err(e) => e,
        }
    }
}
unsafe extern "C" fn frand(context: *mut c_void, result: *mut f32) -> i32 {
    unsafe {
        let c = &mut *context.cast::<Random>();
        match c.begin() {
            Ok(i) => {
                result.write(c.floats[i]);
                0
            }
            Err(e) => e,
        }
    }
}
const RNG: Hp2VmVectorRandServices = Hp2VmVectorRandServices { rand, frand };
fn context(c: &mut Random) -> *mut c_void {
    ptr::from_mut(c).cast()
}

#[test]
fn random_vector_rejects_whole_triples_and_accepts_zero_and_nan() {
    let mut draws = Random::floats(&[1.0, 1.0, 1.0, 0.5, 0.5, 1.0]);
    let mut out = v(99.0, 99.0, 99.0);
    assert_eq!(
        unsafe { vm::random_vector(context(&mut draws), &mut out, RNG) },
        Ok(())
    );
    assert_eq!(bits(out), bits(v(0.0, 0.0, 1.0)));
    assert_eq!(draws.draws, 6);
    assert_eq!(
        draws.events,
        (0..6)
            .flat_map(|i| [Event::Draw(i), Event::Trace(i)])
            .collect::<Vec<_>>()
    );
    for input in [[0.5, 0.5, 0.5], [f32::NAN, 0.5, 0.5]] {
        let mut draws = Random::floats(&input);
        assert_eq!(
            unsafe { vm::random_vector(context(&mut draws), &mut out, RNG) },
            Ok(())
        );
        assert_eq!(draws.draws, 3);
        assert!(out.x.is_nan() && out.y.is_nan() && out.z.is_nan());
    }
}

#[test]
fn random_rotator_draws_yaw_then_pitch_and_only_optional_roll() {
    for roll in [0, 1, 7] {
        let mut draws = Random::ints(&[i32::MAX, i32::MIN, 32768]);
        let mut out = r(99, 99, 99);
        assert_eq!(
            unsafe { vm::random_rotator(context(&mut draws), roll, &mut out, RNG) },
            Ok(())
        );
        assert_eq!(out, r(0, -2, if roll == 0 { 0 } else { 1 }));
        assert_eq!(draws.draws, if roll == 0 { 2 } else { 3 });
    }
}

#[test]
fn random_first_error_stops_draws_and_preserves_unpublished_result() {
    for fail in 0..6 {
        for trace_failure in [false, true] {
            let mut draws = Random::floats(&[1.0, 1.0, 1.0, 0.5, 0.5, 1.0]);
            if trace_failure {
                draws.fail_trace = Some(fail);
            } else {
                draws.fail_draw = Some(fail);
            }
            let mut out = v(17.0, 23.0, 42.0);
            let error = if trace_failure { 72 } else { 71 };
            assert_eq!(
                unsafe { vm::random_vector(context(&mut draws), &mut out, RNG) },
                Err(error)
            );
            assert_eq!(bits(out), bits(v(17.0, 23.0, 42.0)));
            assert_eq!(draws.draws, fail + 1);
            assert_eq!(
                draws.events.last(),
                Some(&if trace_failure {
                    Event::Trace(fail)
                } else {
                    Event::Draw(fail)
                })
            );
        }
    }
    for fail in 0..3 {
        let mut draws = Random::ints(&[10, 20, 30]);
        draws.fail_trace = Some(fail);
        let mut out = r(17, 23, 42);
        assert_eq!(
            unsafe { vm::random_rotator(context(&mut draws), 1, &mut out, RNG) },
            Err(72)
        );
        assert_eq!(out, r(17, 23, 42));
        assert_eq!(draws.draws, fail + 1);
    }
}

#[derive(Default)]
struct VRandCtx {
    ints: Vec<i32>,
    floats: Vec<f32>,
    draws: usize,
    fail_draw: Option<usize>,
    fail_code: i32,
    events: Vec<&'static str>,
}

thread_local! {
    static VRAND_CTX: RefCell<VRandCtx> = RefCell::new(VRandCtx::default());
}

unsafe extern "C" fn vrand_rand(_context: *mut c_void, result: *mut i32) -> i32 {
    VRAND_CTX.with(|c| {
        let mut c = c.borrow_mut();
        let i = c.draws;
        c.draws += 1;
        c.events.push("rand");
        if c.fail_draw == Some(i) {
            return c.fail_code;
        }
        unsafe { result.write(c.ints[i]) };
        0
    })
}
unsafe extern "C" fn vrand_frand(_context: *mut c_void, result: *mut f32) -> i32 {
    VRAND_CTX.with(|c| {
        let mut c = c.borrow_mut();
        let i = c.draws;
        c.draws += 1;
        c.events.push("frand");
        if c.fail_draw == Some(i) {
            return c.fail_code;
        }
        unsafe { result.write(c.floats[i]) };
        0
    })
}
const VRNG: Hp2VmVectorRandServices = Hp2VmVectorRandServices {
    rand: vrand_rand,
    frand: vrand_frand,
};

#[repr(C)]
struct VectorInvocation {
    frame: FFrame,
    code: [u8; 8],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    b_roll: u32,
    events: Vec<&'static str>,
    scope_native: String,
    scope_native_slot: i32,
    scope_callsite_offset: i32,
    scope_begin_error: i32,
    scope_end_error: i32,
}

impl VectorInvocation {
    fn new(code: [u8; 8]) -> Self {
        Self {
            frame: FFrame {
                node: ptr::null_mut(),
                object: ptr::null_mut(),
                code: ptr::null_mut(),
                locals: ptr::null_mut(),
                parent: ptr::null_mut(),
            },
            code,
            table: [vector_unexpected; NATIVE_COUNT],
            b_roll: 0,
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
        self.table[0x80] = vector_operand;
    }
}

unsafe extern "C" fn vector_unexpected(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    99
}
unsafe extern "C" fn vector_operand(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<VectorInvocation>();
        (*p).events.push("operand");
        result.cast::<u32>().write((*p).b_roll);
        0
    }
}
unsafe extern "C" fn vector_object_class(_: *mut c_void) -> *mut c_void {
    panic!("object_class must not be called by callsite_offset");
}
unsafe extern "C" fn vector_script_code(node: *mut c_void, offset: i32, out: *mut *mut u8) -> i32 {
    unsafe {
        let p = node.cast::<VectorInvocation>();
        (*p).events.push("script_code");
        assert_eq!(offset, 0);
        out.write((*p).code.as_mut_ptr());
        0
    }
}
unsafe extern "C" fn vector_scope_begin(
    stack: *mut c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let p = stack.cast::<VectorInvocation>();
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
unsafe extern "C" fn vector_scope_end(handle: *mut c_void) -> i32 {
    unsafe {
        let p = handle.cast::<VectorInvocation>();
        (*p).events.push("scope_end");
        (*p).scope_end_error
    }
}
fn vector_trace_services() -> Hp2VmNativeTraceServices {
    Hp2VmNativeTraceServices {
        frame: Hp2VmFrameInitializationServices {
            object_class: vector_object_class,
            script_code: vector_script_code,
        },
        scope_begin: vector_scope_begin,
        scope_end: vector_scope_end,
    }
}

#[test]
fn execute_vrand_always_opens_the_scope_and_orders_draw_before_end_errors() {
    for (draw_error, end_error, ctx_events, status) in [
        (0, 0, vec!["frand", "frand", "frand"], Ok(())),
        (7, 0, vec!["frand"], Err(7)),
        (0, 9, vec!["frand", "frand", "frand"], Err(9)),
        (7, 9, vec!["frand"], Err(7)),
    ] {
        let mut invocation = VectorInvocation::new([0x16, 0x42, 0, 0, 0, 0, 0, 0]);
        invocation.scope_end_error = end_error;
        invocation.prepare();
        VRAND_CTX.with(|c| {
            *c.borrow_mut() = VRandCtx {
                floats: vec![0.5, 0.5, 1.0],
                fail_draw: if draw_error != 0 { Some(0) } else { None },
                fail_code: draw_error,
                ..Default::default()
            }
        });
        let trace = vector_trace_services();
        let p = &raw mut invocation;
        let mut result = v(99.0, 99.0, 99.0);
        let status_got = unsafe {
            vm::execute_vrand(
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                VRNG,
                &trace,
            )
        };
        assert_eq!(status_got, status);
        assert_eq!(
            invocation.events,
            ["script_code", "scope_begin", "scope_end"]
        );
        assert_eq!(invocation.scope_native, "Core.Object.VRand");
        assert_eq!(invocation.scope_native_slot, 252);
        assert_eq!(invocation.scope_callsite_offset, 0);
        VRAND_CTX.with(|c| assert_eq!(c.borrow().events, ctx_events));
        if draw_error == 0 {
            assert_eq!(bits(result), bits(v(0.0, 0.0, 1.0)));
        } else {
            assert_eq!(bits(result), bits(v(99.0, 99.0, 99.0)));
        }
    }
}

#[test]
fn execute_rotrand_steps_the_roll_operand_before_the_scope_and_threads_it_through() {
    for (b_roll, ints, yaw, pitch, roll) in [
        (0_u32, vec![10, 20, 30], 20, 40, 0),
        (5_u32, vec![10, 20, 30], 20, 40, 60),
    ] {
        let mut invocation = VectorInvocation::new([0x80, 0x16, 0x42, 0, 0, 0, 0, 0]);
        invocation.b_roll = b_roll;
        invocation.prepare();
        VRAND_CTX.with(|c| {
            *c.borrow_mut() = VRandCtx {
                ints,
                ..Default::default()
            }
        });
        let trace = vector_trace_services();
        let p = &raw mut invocation;
        let mut result = r(99, 99, 99);
        let status = unsafe {
            vm::execute_rotrand(
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                VRNG,
                &trace,
            )
        };
        assert_eq!(status, Ok(()));
        assert_eq!(
            invocation.events,
            ["script_code", "operand", "scope_begin", "scope_end"]
        );
        assert_eq!(invocation.scope_native, "Core.Object.RotRand");
        assert_eq!(invocation.scope_native_slot, 320);
        assert_eq!(invocation.scope_callsite_offset, 0);
        VRAND_CTX.with(|c| assert_eq!(c.borrow().draws, if b_roll == 0 { 2 } else { 3 }));
        assert_eq!(result, r(pitch, yaw, roll));
    }
}
#[repr(C)]
struct ValueInvocation {
    frame: FFrame,
    code: [u8; 8],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    slots: [*mut u32; 2],
    widths: [usize; 2],
    values: [[u32; 3]; 2],
    late: [[u32; 3]; 2],
    events: Vec<(usize, usize)>,
    fail_at: usize,
    mutate: bool,
}

impl ValueInvocation {
    fn new(widths: [usize; 2], values: [[u32; 3]; 2], debug: bool) -> Self {
        Self {
            frame: FFrame {
                node: ptr::null_mut(),
                object: 1usize as *mut c_void,
                code: ptr::null_mut(),
                locals: ptr::null_mut(),
                parent: ptr::null_mut(),
            },
            code: [
                0x80,
                0x80,
                0x16,
                if debug { 0x38 } else { 0x42 },
                0,
                0,
                0,
                0,
            ],
            table: [value_unexpected; NATIVE_COUNT],
            slots: [ptr::null_mut(); 2],
            widths,
            values,
            late: [[0; 3]; 2],
            events: Vec::new(),
            fail_at: 0,
            mutate: false,
        }
    }

    fn prepare(&mut self) {
        self.frame.code = self.code.as_mut_ptr();
        self.table[0x80] = value_argument;
        self.table[0x16] = value_omitted;
        self.table[0x38] = value_debug;
    }
}

unsafe extern "C" fn value_unexpected(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    99
}

unsafe fn value_record(p: *mut ValueInvocation, object: *mut c_void) -> i32 {
    unsafe {
        let offset = (*p).frame.code.offset_from(ptr::addr_of!((*p).code).cast()) as usize;
        (*p).events.push((offset, object as usize));
        let count = (*p).events.len();
        if count == (*p).fail_at {
            70 + count as i32
        } else {
            0
        }
    }
}

unsafe extern "C" fn value_argument(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ValueInvocation>();
        let index = (*p).events.len();
        (*p).slots[index] = result.cast();
        // Struct slots are native-uninitialized. A failing callback deliberately
        // writes only one component; the executor must not read the remainder.
        let words = if (*p).fail_at == index + 1 {
            1
        } else {
            (*p).widths[index]
        };
        for word in 0..words {
            if (*p).widths[index] == 1 && result.cast::<u32>().read() != 0 {
                return 98;
            }
            result
                .cast::<u32>()
                .add(word)
                .write((*p).values[index][word]);
        }
        if index == 1 && (*p).mutate {
            for word in 0..(*p).widths[0] {
                (*p).slots[0].add(word).write((*p).late[0][word]);
            }
        }
        (*p).frame.object = (index + 2) as *mut c_void;
        value_record(p, object)
    }
}

unsafe extern "C" fn value_debug(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ValueInvocation>();
        if !result.is_null() {
            return 97;
        }
        if (*p).mutate {
            for index in 0..2 {
                for word in 0..(*p).widths[index] {
                    (*p).slots[index].add(word).write((*p).late[index][word]);
                }
            }
        }
        let status = value_record(p, object);
        (*p).frame.code = (*p).frame.code.add(1);
        status
    }
}

unsafe extern "C" fn value_omitted(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ValueInvocation>();
        if (*p).widths[(*p).events.len()] != 1 || result.cast::<u32>().read() != 0 {
            return 96;
        }
        let status = value_record(p, object);
        (*p).frame.code = (*p).frame.code.sub(1);
        status
    }
}

type ValueHandler =
    unsafe fn(*mut FFrame, *mut c_void, *const Hp2VmNativeHandler) -> Result<(), i32>;

#[test]
fn raw_value_arguments_keep_heterogeneous_order_and_late_debug_mutations() {
    let scalar = [2.0_f32.to_bits(), 0, 0];
    let late_scalar = [4.0_f32.to_bits(), 0, 0];
    let vector = bits(v(1.0, 2.0, 3.0));
    let late_vector = bits(v(5.0, 6.0, 7.0));
    let rotator = [1, 2, 3];
    let late_rotator = [5, 6, 7];
    let cases: [(ValueHandler, bool, bool); 4] = [
        (vm::execute_vector::<212>, false, false),
        (vm::execute_vector::<213>, true, false),
        (vm::execute_rotator::<287>, false, true),
        (vm::execute_rotator::<288>, true, true),
    ];
    for (handler, scalar_first, integer) in cases {
        for debug in [false, true] {
            let triple = if integer { rotator } else { vector };
            let late_triple = if integer { late_rotator } else { late_vector };
            let (widths, values, late) = if scalar_first {
                ([1, 3], [scalar, triple], [late_scalar, late_triple])
            } else {
                ([3, 1], [triple, scalar], [late_triple, late_scalar])
            };
            let mut invocation = ValueInvocation::new(widths, values, debug);
            invocation.mutate = true;
            invocation.late = late;
            invocation.prepare();
            let p = &raw mut invocation;
            let mut output = [0xdead_beef_u32; 5];
            assert_eq!(
                unsafe {
                    handler(
                        ptr::addr_of_mut!((*p).frame),
                        output.as_mut_ptr().add(1).cast(),
                        ptr::addr_of!((*p).table).cast(),
                    )
                },
                Ok(())
            );
            let expected = if debug {
                [20, 24, 28]
            } else if scalar_first {
                [4, 8, 12]
            } else {
                [10, 12, 14]
            };
            let expected = if integer {
                expected
            } else {
                expected.map(|n| (n as f32).to_bits())
            };
            assert_eq!(
                output,
                [
                    0xdead_beef,
                    expected[0],
                    expected[1],
                    expected[2],
                    0xdead_beef
                ]
            );
            let events = [(1, 1), (2, 2), (4, 3)];
            assert_eq!(invocation.events, events[..if debug { 3 } else { 2 }]);
            assert_eq!(invocation.frame.code, unsafe {
                invocation.code.as_mut_ptr().add(if debug { 5 } else { 3 })
            });
        }
    }
}

#[test]
fn raw_value_results_publish_only_the_native_four_byte_width() {
    let cases: [(ValueHandler, [u32; 3], [u32; 3], u32); 5] = [
        (
            vm::execute_vector::<217>,
            bits(v(1.0, 2.0, 3.0)),
            bits(v(1.0, 2.0, 3.0)),
            1,
        ),
        (
            vm::execute_vector::<218>,
            bits(v(1.0, 2.0, 3.0)),
            bits(v(1.0, 2.0, 4.0)),
            1,
        ),
        (
            vm::execute_vector::<219>,
            bits(v(1.0, 2.0, 3.0)),
            bits(v(4.0, 5.0, 6.0)),
            32.0_f32.to_bits(),
        ),
        (vm::execute_rotator::<142>, [1, 2, 3], [1, 2, 3], 1),
        (vm::execute_rotator::<203>, [1, 2, 3], [1, 2, 4], 1),
    ];
    for (handler, a, b, expected) in cases {
        let mut invocation = ValueInvocation::new([3, 3], [a, b], false);
        invocation.prepare();
        let p = &raw mut invocation;
        let mut output = [0xdead_beef_u32; 5];
        assert_eq!(
            unsafe {
                handler(
                    ptr::addr_of_mut!((*p).frame),
                    output.as_mut_ptr().add(1).cast(),
                    ptr::addr_of!((*p).table).cast(),
                )
            },
            Ok(())
        );
        assert_eq!(
            output,
            [0xdead_beef, expected, 0xdead_beef, 0xdead_beef, 0xdead_beef]
        );
    }
}

#[test]
fn raw_value_omitted_trailing_floats_keep_the_native_zero_default() {
    let cases: [(ValueHandler, [u32; 3], u32); 4] = [
        (vm::execute_vector::<212>, bits(v(1.0, 2.0, 3.0)), 0),
        (
            vm::execute_vector::<214>,
            bits(v(1.0, 2.0, 3.0)),
            f32::INFINITY.to_bits(),
        ),
        (vm::execute_rotator::<287>, [1, 2, 3], 0),
        (vm::execute_rotator::<289>, [1, 2, 3], i32::MAX as u32),
    ];
    for (handler, value, expected) in cases {
        let mut invocation = ValueInvocation::new([3, 1], [value, [0; 3]], false);
        invocation.code = [0x80, 0x16, 0x42, 0, 0, 0, 0, 0];
        invocation.prepare();
        let p = &raw mut invocation;
        let mut output = [0xdead_beef_u32; 5];
        assert_eq!(
            unsafe {
                handler(
                    ptr::addr_of_mut!((*p).frame),
                    output.as_mut_ptr().add(1).cast(),
                    ptr::addr_of!((*p).table).cast(),
                )
            },
            Ok(())
        );
        assert_eq!(
            output,
            [0xdead_beef, expected, expected, expected, 0xdead_beef]
        );
        assert_eq!(invocation.events, [(1, 1), (2, 2)]);
        assert_eq!(invocation.frame.code, unsafe {
            invocation.code.as_mut_ptr().add(2)
        });
    }
}

#[test]
fn raw_value_first_errors_preserve_exact_prefix_without_publication() {
    let cases: [(ValueHandler, [usize; 2]); 3] = [
        (vm::execute_vector::<213>, [1, 3]),
        (vm::execute_vector::<219>, [3, 3]),
        (vm::execute_rotator::<287>, [3, 1]),
    ];
    for (handler, widths) in cases {
        for fail_at in 1..=3 {
            let mut invocation = ValueInvocation::new(widths, [[0; 3]; 2], true);
            invocation.fail_at = fail_at;
            invocation.prepare();
            let p = &raw mut invocation;
            let mut output = [0xdead_beef_u32; 5];
            assert_eq!(
                unsafe {
                    handler(
                        ptr::addr_of_mut!((*p).frame),
                        output.as_mut_ptr().add(1).cast(),
                        ptr::addr_of!((*p).table).cast(),
                    )
                },
                Err(70 + fail_at as i32)
            );
            assert_eq!(output, [0xdead_beef; 5]);
            assert_eq!(invocation.events, [(1, 1), (2, 2), (4, 3)][..fail_at]);
            assert_eq!(invocation.frame.code, unsafe {
                invocation
                    .code
                    .as_mut_ptr()
                    .add(if fail_at == 3 { 5 } else { fail_at })
            });
        }
    }
}

#[test]
fn raw_three_component_constants_store_between_reads_and_preserve_bits() {
    let mut invocation = ValueInvocation::new([0; 2], [[0; 3]; 2], false);
    let mut words = [
        0x7fc1_2345_u32,
        0x8000_0000,
        0xffff_ffff,
        0xdead_beef,
        0xdead_beef,
    ];
    invocation.frame.code = words.as_mut_ptr().cast();
    unsafe { vm::execute_constant(&raw mut invocation.frame, words.as_mut_ptr().add(1)) };
    assert_eq!(
        words,
        [
            0x7fc1_2345,
            0x7fc1_2345,
            0x7fc1_2345,
            0x7fc1_2345,
            0xdead_beef
        ]
    );
    assert_eq!(invocation.frame.code, unsafe {
        words.as_mut_ptr().add(3).cast()
    });

    let mut wire = [0_u8; 13];
    let expected = [0x8000_0000_u32, 0x7fc1_2345, 0xffff_ffff];
    for (index, word) in expected.into_iter().enumerate() {
        wire[1 + index * 4..5 + index * 4].copy_from_slice(&word.to_ne_bytes());
    }
    invocation.frame.code = unsafe { wire.as_mut_ptr().add(1) };
    let mut output = [0xdead_beef_u32; 5];
    unsafe { vm::execute_constant(&raw mut invocation.frame, output.as_mut_ptr().add(1)) };
    assert_eq!(
        output,
        [
            0xdead_beef,
            expected[0],
            expected[1],
            expected[2],
            0xdead_beef
        ]
    );
    assert_eq!(invocation.frame.code, unsafe { wire.as_mut_ptr().add(13) });
}

#[test]
#[cfg(all(target_pointer_width = "64", target_endian = "little"))]
fn raw_constant_component_store_can_redirect_the_live_frame_cursor() {
    #[repr(align(32))]
    struct Wire([u32; 8]);
    let mut wire = Wire([0; 8]);
    // Alignment keeps both source addresses in the same high 32-bit region.
    let redirected = unsafe { wire.0.as_mut_ptr().add(4) };
    wire.0[0] = redirected as usize as u32;
    wire.0[4] = (redirected as usize >> 32) as u32;
    wire.0[5] = 0x7654_3210;
    let mut invocation = ValueInvocation::new([0; 2], [[0; 3]; 2], false);
    invocation.frame.code = wire.0.as_mut_ptr().cast();
    let frame = &raw mut invocation.frame;
    unsafe {
        // The first component overwrites Code's low half. The second preserves
        // its high half; the third lands in the following Locals field.
        vm::execute_constant(frame, ptr::addr_of_mut!((*frame).code).cast());
        assert_eq!((*frame).code, redirected.add(2).cast());
        assert_eq!(
            ptr::addr_of!((*frame).locals).cast::<u32>().read(),
            0x7654_3210
        );
    }
}

// The interpreter lvalue slot is process-global, matching native single-threaded
// VM execution. Only compound-handler tests in this binary access that slot.
static REFERENCE_LOCK: Mutex<()> = Mutex::new(());

fn compound_handlers() -> [(ValueHandler, bool, bool, [u32; 3]); 9] {
    [
        (vm::execute_vector_assign::<221>, false, true, [40, 60, 80]),
        (
            vm::execute_vector_assign::<297>,
            false,
            false,
            [80, 150, 320],
        ),
        (vm::execute_vector_assign::<222>, false, true, [10, 15, 20]),
        (vm::execute_vector_assign::<223>, false, false, [24, 35, 48]),
        (vm::execute_vector_assign::<224>, false, false, [16, 25, 32]),
        (vm::execute_rotator_assign::<290>, true, true, [40, 60, 80]),
        (vm::execute_rotator_assign::<291>, true, true, [10, 15, 20]),
        (vm::execute_rotator_assign::<318>, true, false, [24, 35, 48]),
        (vm::execute_rotator_assign::<319>, true, false, [16, 25, 32]),
    ]
}

#[test]
fn compound_handlers_read_temporaries_after_debug_and_publish_twelve_bytes() {
    let _lock = REFERENCE_LOCK.lock().unwrap();
    for (handler, integer, scalar, expected) in compound_handlers() {
        let words = |values: [u32; 3]| {
            if integer {
                values
            } else {
                values.map(|n| (n as f32).to_bits())
            }
        };
        let mut invocation = ValueInvocation::new(
            [3, if scalar { 1 } else { 3 }],
            [
                words([8, 12, 16]),
                if scalar {
                    [4.0_f32.to_bits(), 0, 0]
                } else {
                    words([2, 3, 4])
                },
            ],
            true,
        );
        invocation.mutate = true;
        invocation.late = [
            words([20, 30, 40]),
            if scalar {
                [2.0_f32.to_bits(), 0, 0]
            } else {
                words([4, 5, 8])
            },
        ];
        invocation.prepare();
        let p = &raw mut invocation;
        let mut output = [0xdead_beef_u32; 5];
        assert_eq!(
            unsafe {
                handler(
                    ptr::addr_of_mut!((*p).frame),
                    output.as_mut_ptr().add(1).cast(),
                    ptr::addr_of!((*p).table).cast(),
                )
            },
            Ok(())
        );
        let expected = words(expected);
        assert_eq!(
            output,
            [
                0xdead_beef,
                expected[0],
                expected[1],
                expected[2],
                0xdead_beef
            ]
        );
        assert_eq!(invocation.events, [(1, 1), (2, 2), (4, 3)]);
        assert_eq!(invocation.frame.code, unsafe {
            invocation.code.as_mut_ptr().add(5)
        });
    }
}

#[test]
fn compound_handlers_preserve_first_argument_rhs_and_debug_errors() {
    let _lock = REFERENCE_LOCK.lock().unwrap();
    for (handler, _, scalar, _) in compound_handlers() {
        for fail_at in 1..=3 {
            let mut invocation =
                ValueInvocation::new([3, if scalar { 1 } else { 3 }], [[0; 3]; 2], true);
            invocation.fail_at = fail_at;
            invocation.prepare();
            let p = &raw mut invocation;
            let mut output = [0xdead_beef_u32; 5];
            assert_eq!(
                unsafe {
                    handler(
                        ptr::addr_of_mut!((*p).frame),
                        output.as_mut_ptr().add(1).cast(),
                        ptr::addr_of!((*p).table).cast(),
                    )
                },
                Err(70 + fail_at as i32)
            );
            assert_eq!(output, [0xdead_beef; 5]);
            assert_eq!(invocation.events, [(1, 1), (2, 2), (4, 3)][..fail_at]);
            assert_eq!(invocation.frame.code, unsafe {
                invocation
                    .code
                    .as_mut_ptr()
                    .add(if fail_at == 3 { 5 } else { fail_at })
            });
        }
    }
}

unsafe extern "C" fn compound_nested_rhs(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ValueInvocation>();
        if (*p).events.is_empty() {
            return value_argument(object, frame, result);
        }
        // Reenter the real compound evaluator while its caller's first temporary
        // remains live. No synthetic property descriptor or lvalue provider.
        let mut nested = ValueInvocation::new(
            [3, 3],
            [bits(v(2.0, 3.0, 4.0)), bits(v(5.0, 6.0, 7.0))],
            false,
        );
        nested.prepare();
        let n = &raw mut nested;
        if let Err(error) = vm::execute_vector_assign::<223>(
            ptr::addr_of_mut!((*n).frame),
            result,
            ptr::addr_of!((*n).table).cast(),
        ) {
            return error;
        }
        value_record(p, object)
    }
}

#[test]
fn compound_rhs_reentry_keeps_the_outer_first_temporary() {
    let _lock = REFERENCE_LOCK.lock().unwrap();
    let mut invocation = ValueInvocation::new([3, 3], [bits(v(10.0, 20.0, 30.0)), [0; 3]], false);
    invocation.prepare();
    invocation.table[0x80] = compound_nested_rhs;
    let p = &raw mut invocation;
    let mut output = [0xdead_beef_u32; 5];
    assert_eq!(
        unsafe {
            vm::execute_vector_assign::<224>(
                ptr::addr_of_mut!((*p).frame),
                output.as_mut_ptr().add(1).cast(),
                ptr::addr_of!((*p).table).cast(),
            )
        },
        Ok(())
    );
    let expected = bits(v(3.0, 11.0, 19.0));
    assert_eq!(
        output,
        [
            0xdead_beef,
            expected[0],
            expected[1],
            expected[2],
            0xdead_beef
        ]
    );
    assert_eq!(invocation.events, [(1, 1), (2, 2)]);
    assert_eq!(invocation.frame.code, unsafe {
        invocation.code.as_mut_ptr().add(3)
    });
}
