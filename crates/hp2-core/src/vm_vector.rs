//! Vector/rotator script arithmetic, axis publication, and random draw policy.
//! By-value handlers, compound assignments and axes own raw-frame evaluation.
//! Other reference operations and RNG retain their native parsing/scope contracts.

use crate::math::{self, FRotator, NUM_ANGLES, add_rn, dot3, fmadd_rn, mul_rn, scale, sub};
use crate::{vm_bytecode, vm_frame, vm_registry::Hp2VmNativeHandler};
use hp2_abi::{FCoords, FVector, engine::generated::FFrame};
use std::{ffi::c_void, mem::MaybeUninit};

/// Evaluate axes arguments against the canonical interpreter lvalue slots.
///
/// # Safety
/// Frame/code/table satisfy vm_frame::step_raw. The required rotator expression
/// initializes all twelve input bytes. Referenced vectors remain live through
/// argument/debug reentry; they may alias. Discarded outputs use local,
/// uninitialized native-compatible struct storage and are only ever written.
pub unsafe fn execute_axes<const UNAXES: bool>(
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut rotation = MaybeUninit::<FRotator>::uninit();
        let mut x_temp = MaybeUninit::<FVector>::uninit();
        let mut y_temp = MaybeUninit::<FVector>::uninit();
        let mut z_temp = MaybeUninit::<FVector>::uninit();
        vm_frame::step_argument(frame, rotation.as_mut_ptr().cast(), native_table)?;
        let x = vector_reference(frame, x_temp.as_mut_ptr(), native_table)?;
        let y = vector_reference(frame, y_temp.as_mut_ptr(), native_table)?;
        let z = vector_reference(frame, z_temp.as_mut_ptr(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let coordinates = axes(rotation.assume_init(), UNAXES, math::trig_table());
        write_axes(coordinates, x, y, z);
        Ok(())
    }
}

#[inline]
unsafe fn vector_reference<T>(
    frame: *mut FFrame,
    temporary: *mut T,
    native_table: *const Hp2VmNativeHandler,
) -> Result<*mut T, i32> {
    unsafe {
        let slot = crate::vm_variable::address_slot();
        slot.write(std::ptr::null_mut());
        vm_frame::step_argument(frame, temporary.cast(), native_table)?;
        // Read after nested execution, not from a pre-Step state snapshot.
        let address = slot.read();
        Ok(if address.is_null() {
            temporary
        } else {
            address.cast()
        })
    }
}

/// Evaluate a vector compound assignment, retaining the first expression's lvalue.
///
/// # Safety
/// OP is 221, 297, 222, 223 or 224. Frame/code/table satisfy vm_frame::step_raw.
/// Required struct expressions initialize all twelve temporary bytes when they
/// do not supply a live canonical lvalue. Lvalues and argument slots remain live
/// through argument/debug reentry. Result is aligned and writable for twelve
/// bytes and may overlap operands or the lvalue. No references span callbacks.
#[inline]
pub unsafe fn execute_vector_assign<const OP: u32>(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut temporary = MaybeUninit::<FVector>::uninit();
        let mut b = MaybeUninit::<FVector>::uninit();
        let mut scalar = 0.0_f32;
        let a = vector_reference(frame, temporary.as_mut_ptr(), native_table)?;
        let b = if OP == 221 || OP == 222 {
            vm_frame::step_argument(frame, (&raw mut scalar).cast(), native_table)?;
            std::ptr::null()
        } else {
            vm_frame::step_argument(frame, b.as_mut_ptr().cast(), native_table)?;
            b.as_ptr()
        };
        // RHS/debug execution can overwrite the canonical address and mutate A.
        // Keep its original address, but defer all value reads until after finish.
        vm_frame::finish_arguments(frame, native_table)?;
        vector_assign(OP, a, b, scalar, result.cast());
        Ok(())
    }
}

/// Evaluate a rotator compound assignment with native reference/write ordering.
///
/// # Safety
/// OP is 290, 291, 318 or 319. Frame, arguments, lvalue and twelve-byte result
/// satisfy execute_vector_assign, with rotator slots in place of vectors.
#[inline]
pub unsafe fn execute_rotator_assign<const OP: u32>(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut temporary = MaybeUninit::<FRotator>::uninit();
        let mut b = MaybeUninit::<FRotator>::uninit();
        let mut scalar = 0.0_f32;
        let a = vector_reference(frame, temporary.as_mut_ptr(), native_table)?;
        let b = if OP == 290 || OP == 291 {
            vm_frame::step_argument(frame, (&raw mut scalar).cast(), native_table)?;
            std::ptr::null()
        } else {
            vm_frame::step_argument(frame, b.as_mut_ptr().cast(), native_table)?;
            b.as_ptr()
        };
        vm_frame::finish_arguments(frame, native_table)?;
        rotator_assign(OP, a, b, scalar, result.cast());
        Ok(())
    }
}

/// Evaluate the selected by-value vector native, then publish its exact result width.
///
/// # Safety
/// OP is one of the selected by-value vector slots. Frame/code/table satisfy
/// vm_frame::step_raw; result is four-byte aligned and writable for four bytes
/// (comparison/float) or twelve bytes (vector). Callbacks may retain argument
/// pointers only until this invocation returns. No references span reentry.
/// As with native P_GET_STRUCT, every required vector argument must be fully
/// initialized before a successful finish; an omitted struct has no default.
#[inline]
pub unsafe fn execute_vector<const OP: u32>(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    let mut execute = unsafe { legacy_dispatcher(native_table) };
    unsafe { execute_vector_opcode_with(OP, frame, result, &mut execute) }
}

/// Evaluate the selected by-value rotator native, then publish its exact result width.
///
/// # Safety
/// OP is one of the selected by-value rotator slots. Frame/code/table satisfy
/// vm_frame::step_raw; result is four-byte aligned and writable for four bytes
/// (comparison) or twelve bytes (rotator). Callbacks may retain argument pointers
/// only until this invocation returns. No frame or argument references span reentry.
/// As with native P_GET_STRUCT, every required rotator argument must be fully
/// initialized before a successful finish; an omitted struct has no default.
#[inline]
pub unsafe fn execute_rotator<const OP: u32>(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    let mut execute = unsafe { legacy_dispatcher(native_table) };
    unsafe { execute_rotator_opcode_with(OP, frame, result, &mut execute) }
}

/// Adapt the established native table to the shared vector frame kernel.
///
/// # Safety
/// `native_table` is live and contains every opcode reached by these operands.
unsafe fn legacy_dispatcher(
    native_table: *const Hp2VmNativeHandler,
) -> impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), i32> {
    move |opcode, object, frame, result| {
        let handler = unsafe { native_table.add(opcode as usize).read() };
        status(unsafe { handler(object, frame.cast(), result) })
    }
}

/// Evaluate any registered by-value vector native with direct expression dispatch.
///
/// # Safety
/// `opcode` is one of the registered vector value operations. Frame operands and
/// result satisfy that operation's native width/alignment contract. Staged slots
/// remain stable through expression and debug reentry.
pub unsafe fn execute_vector_opcode_with<E>(
    opcode: u32,
    frame: *mut FFrame,
    result: *mut c_void,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut a = MaybeUninit::<FVector>::zeroed();
        let mut b = MaybeUninit::<FVector>::zeroed();
        let mut scalar = MaybeUninit::<f32>::zeroed();
        if opcode == 213 {
            vm_frame::step_argument_with(frame, scalar.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
            vm_frame::step_argument_with(frame, a.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
        } else {
            vm_frame::step_argument_with(frame, a.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
            match opcode {
                211 | 225 | 226 => {}
                212 | 214 => {
                    vm_frame::step_argument_with(frame, scalar.as_mut_ptr().cast(), |op, object, frame, out| {
                        execute(op, object, frame, out)
                    })?;
                }
                _ => {
                    vm_frame::step_argument_with(frame, b.as_mut_ptr().cast(), |op, object, frame, out| {
                        execute(op, object, frame, out)
                    })?;
                }
            }
        }
        vm_frame::finish_arguments_with(frame, |op, object, frame, out| {
            execute(op, object, frame, out)
        })?;
        let a = a.assume_init();
        let b = b.assume_init();
        if matches!(opcode, 217 | 218) {
            result
                .cast::<u32>()
                .write_unaligned(vector_compare(opcode, a, b));
        } else if matches!(opcode, 219 | 225) {
            result.cast::<f32>().write_unaligned(vector_float(opcode, a, b));
        } else {
            result.cast::<FVector>().write_unaligned(vector(
                opcode,
                a,
                b,
                scalar.assume_init(),
            ));
        }
        Ok(())
    }
}

/// Evaluate any registered by-value rotator native with direct expression dispatch.
///
/// # Safety
/// `opcode` is one of the registered rotator value operations. Frame operands and
/// result satisfy that operation's native width/alignment contract.
pub unsafe fn execute_rotator_opcode_with<E>(
    opcode: u32,
    frame: *mut FFrame,
    result: *mut c_void,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut a = MaybeUninit::<FRotator>::zeroed();
        let mut b = MaybeUninit::<FRotator>::zeroed();
        let mut scalar = MaybeUninit::<f32>::zeroed();
        if matches!(opcode, 142 | 203 | 316 | 317) {
            vm_frame::step_argument_with(frame, a.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
            vm_frame::step_argument_with(frame, b.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
        } else if opcode == 288 {
            vm_frame::step_argument_with(frame, scalar.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
            vm_frame::step_argument_with(frame, a.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
        } else {
            vm_frame::step_argument_with(frame, a.as_mut_ptr().cast(), |op, object, frame, out| {
                execute(op, object, frame, out)
            })?;
            if matches!(opcode, 287 | 289) {
                vm_frame::step_argument_with(frame, scalar.as_mut_ptr().cast(), |op, object, frame, out| {
                    execute(op, object, frame, out)
                })?;
            }
        }
        vm_frame::finish_arguments_with(frame, |op, object, frame, out| {
            execute(op, object, frame, out)
        })?;
        let a = a.assume_init();
        let b = b.assume_init();
        if matches!(opcode, 142 | 203) {
            result
                .cast::<u32>()
                .write_unaligned(rotator_compare(opcode, a, b));
        } else {
            result.cast::<FRotator>().write_unaligned(rotator(
                opcode,
                a,
                b,
                scalar.assume_init(),
            ));
        }
        Ok(())
    }
}

/// Read either three-component constant without finishing function arguments.
///
/// # Safety
/// Frame has a live writable Code slot. Before each component read its live
/// cursor addresses four initialized bytes; result addresses twelve writable,
/// four-byte-aligned bytes. Result may overlap source bytes or frame fields.
#[inline]
pub unsafe fn execute_constant(frame: *mut FFrame, result: *mut u32) {
    unsafe {
        let cursor = std::ptr::addr_of_mut!((*frame).code);
        for component in 0..3 {
            // Reload and advance Code before each store: an earlier component
            // can change the next source word or the cursor itself.
            let word = vm_bytecode::read::<u32>(cursor);
            result.add(component).write(word);
        }
    }
}

fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

// Unlike the other cross-product callsite in math.rs, UnCorSc's ARM64 object
// negates B before multiplying (fneg/fmul/fmla), not the rounded product.
fn cross(a: FVector, b: FVector) -> FVector {
    v(
        fmadd_rn(b.z, a.y, mul_rn(a.z, -b.y)),
        fmadd_rn(b.x, a.z, mul_rn(a.x, -b.z)),
        fmadd_rn(a.x, b.y, mul_rn(-b.x, a.y)),
    )
}

pub fn vector(op: u32, a: FVector, b: FVector, s: f32) -> FVector {
    match op {
        211 => v(-a.x, -a.y, -a.z),
        212 | 213 | 221 => v(mul_rn(a.x, s), mul_rn(a.y, s), mul_rn(s, a.z)),
        296 => v(mul_rn(a.x, b.x), mul_rn(a.y, b.y), mul_rn(a.z, b.z)),
        297 => v(mul_rn(b.x, a.x), mul_rn(b.y, a.y), mul_rn(b.z, a.z)),
        214 | 222 => {
            let r = 1.0f32 / s;
            v(mul_rn(a.x, r), mul_rn(a.y, r), mul_rn(r, a.z))
        }
        215 => v(add_rn(a.x, b.x), add_rn(a.y, b.y), add_rn(a.z, b.z)),
        223 => v(add_rn(b.x, a.x), add_rn(b.y, a.y), add_rn(b.z, a.z)),
        216 | 224 => sub(a, b),
        220 => cross(a, b),
        226 => math::vector_safe_normal(a),
        300 => {
            let n = math::vector_safe_normal(b);
            // The script intrinsic doubles the normal first. MirrorByVector
            // doubles the dot instead and is observably different on overflow.
            let twice = v(add_rn(n.x, n.x), add_rn(n.y, n.y), add_rn(n.z, n.z));
            sub(a, scale(twice, dot3(n, a)))
        }
        _ => panic!("invalid vector native {op}"),
    }
}

pub fn vector_compare(op: u32, a: FVector, b: FVector) -> u32 {
    let equal = a.x == b.x && a.y == b.y && a.z == b.z;
    u32::from(match op {
        217 => equal,
        218 => !equal,
        _ => panic!("invalid vector comparison {op}"),
    })
}

pub fn vector_float(op: u32, a: FVector, b: FVector) -> f32 {
    match op {
        219 => dot3(a, b),
        225 => (dot3(a, a) as f64).sqrt() as f32,
        _ => panic!("invalid vector scalar native {op}"),
    }
}

/// # Safety
/// Operands/results are live aligned vector slots; they may overlap. All reads
/// finish before assignment, and the result is published after the lvalue.
pub unsafe fn vector_assign(
    op: u32,
    a: *mut FVector,
    b: *const FVector,
    s: f32,
    result: *mut FVector,
) {
    unsafe {
        let old = a.read();
        let rhs = if b.is_null() {
            v(0.0, 0.0, 0.0)
        } else {
            b.read()
        };
        let value = vector(op, old, rhs, s);
        a.write(value);
        result.write(value);
    }
}

pub fn rotator(op: u32, a: FRotator, b: FRotator, s: f32) -> FRotator {
    match op {
        287 | 288 | 289 | 290 | 291 => {
            let s = if op == 289 || op == 291 {
                1.0f32 / s
            } else {
                s
            };
            // UnGnuG appRound is truncation; ARM64 SCVTF/FMUL/FCVTZS
            // saturates overflow/infinity and converts NaN to zero.
            FRotator {
                pitch: (s * a.pitch as f32) as i32,
                yaw: (s * a.yaw as f32) as i32,
                roll: (s * a.roll as f32) as i32,
            }
        }
        316 | 318 => FRotator {
            pitch: a.pitch.wrapping_add(b.pitch),
            yaw: a.yaw.wrapping_add(b.yaw),
            roll: a.roll.wrapping_add(b.roll),
        },
        317 | 319 => FRotator {
            pitch: a.pitch.wrapping_sub(b.pitch),
            yaw: a.yaw.wrapping_sub(b.yaw),
            roll: a.roll.wrapping_sub(b.roll),
        },
        1031 => FRotator {
            pitch: a.pitch as i16 as i32,
            yaw: a.yaw as i16 as i32,
            roll: a.roll as i16 as i32,
        },
        _ => panic!("invalid rotator native {op}"),
    }
}

pub fn rotator_compare(op: u32, a: FRotator, b: FRotator) -> u32 {
    u32::from(match op {
        142 => a == b,
        203 => a != b,
        _ => panic!("invalid rotator comparison {op}"),
    })
}

/// # Safety
/// Operands/result are aligned live rotator slots, possibly overlapping.
pub unsafe fn rotator_assign(
    op: u32,
    a: *mut FRotator,
    b: *const FRotator,
    s: f32,
    result: *mut FRotator,
) {
    unsafe {
        let old = a.read();
        let rhs = if b.is_null() {
            FRotator::default()
        } else {
            b.read()
        };
        let value = rotator(op, old, rhs, s);
        a.write(value);
        result.write(value);
    }
}

// ARM FMSUB computes c - a*b. Keeping the instruction explicit preserves NaN
// sign and zero behavior in the compiled FCoords::operator*=(FRotator).
#[cfg(target_arch = "aarch64")]
fn fmsub(a: f32, b: f32, c: f32) -> f32 {
    let r;
    unsafe {
        std::arch::asm!("fmsub {r:s}, {a:s}, {b:s}, {c:s}", r = lateout(vreg) r, a = in(vreg) a, b = in(vreg) b, c = in(vreg) c, options(pure, nomem, nostack));
    }
    r
}
#[cfg(not(target_arch = "aarch64"))]
fn fmsub(a: f32, b: f32, c: f32) -> f32 {
    (-a).mul_add(b, c)
}

pub fn axes(rot: FRotator, unaxes: bool, trig: &[f32; NUM_ANGLES]) -> FCoords {
    if !unaxes {
        return math::coords_div_rotator(&math::unit_coords(), rot, trig);
    }
    let sin = |r: i32| trig[((r as u32 >> 2) & 16383) as usize];
    let cos = |r: i32| sin(r.wrapping_add(16384));
    let (sy, cy, sp, cp, sr, cr) = (
        sin(rot.yaw),
        cos(rot.yaw),
        sin(rot.pitch),
        cos(rot.pitch),
        sin(rot.roll),
        cos(rot.roll),
    );
    let transform = |a: FVector| {
        // Yaw, then pitch, then roll. Do not simplify zero products: the
        // original full coordinate multiplications preserve signed zero/NaN.
        let y = v(
            fmadd_rn(a.z, 0.0, fmadd_rn(a.x, cy, mul_rn(sy, a.y))),
            fmadd_rn(a.z, 0.0, fmsub(a.x, sy, mul_rn(cy, a.y))),
            add_rn(a.z, fmadd_rn(a.x, 0.0, mul_rn(a.y, 0.0))),
        );
        let t = mul_rn(y.y, 0.0);
        let p = v(
            fmadd_rn(y.z, sp, fmadd_rn(y.x, cp, t)),
            fmadd_rn(y.z, 0.0, fmadd_rn(y.x, 0.0, y.y)),
            fmadd_rn(y.z, cp, fmsub(y.x, sp, t)),
        );
        v(
            fmadd_rn(p.z, 0.0, add_rn(mul_rn(p.y, 0.0), p.x)),
            fmsub(p.z, sr, fmadd_rn(p.x, 0.0, mul_rn(p.y, cr))),
            fmadd_rn(p.z, cr, fmadd_rn(p.x, 0.0, mul_rn(p.y, sr))),
        )
    };
    let unit = math::unit_coords();
    FCoords {
        origin: unit.origin,
        x_axis: transform(unit.x_axis),
        y_axis: transform(unit.y_axis),
        z_axis: transform(unit.z_axis),
    }
}

pub fn rotate(a: FVector, rot: FRotator, unrotate: bool, trig: &[f32; NUM_ANGLES]) -> FVector {
    let c = axes(rot, unrotate, trig);
    v(dot3(a, c.x_axis), dot3(a, c.y_axis), dot3(a, c.z_axis))
}

/// # Safety
/// Output slots are live aligned vectors and may alias; X, Y, Z publish in order.
pub unsafe fn write_axes(c: FCoords, x: *mut FVector, y: *mut FVector, z: *mut FVector) {
    unsafe {
        x.write(c.x_axis);
        y.write(c.y_axis);
        z.write(c.z_axis);
    }
}

/// # Safety
/// X/Y/Z are readable/writable aligned vectors, possibly aliased. Snapshot all
/// three rows before publishing any part of the inverse.
pub unsafe fn invert(x: *mut FVector, y: *mut FVector, z: *mut FVector) {
    unsafe {
        let c = FCoords {
            origin: v(0.0, 0.0, 0.0),
            x_axis: x.read(),
            y_axis: y.read(),
            z_axis: z.read(),
        };
        write_axes(math::coords_inverse(&c), x, y, z);
    }
}

pub fn ortho_rotation(x: FVector, y: FVector, z: FVector, trig: &[f32; NUM_ANGLES]) -> FRotator {
    math::coords_ortho_rotation(
        &FCoords {
            origin: v(0.0, 0.0, 0.0),
            x_axis: x,
            y_axis: y,
            z_axis: z,
        },
        trig,
    )
}

/// Evaluate a vector-rotate-by-rotator expression, then publish the result.
///
/// # Safety
/// Frame/code/table satisfy vm_frame::step_raw. Result is twelve-byte aligned
/// and writable for an FVector. No references span argument/debug reentry.
pub unsafe fn execute_vector_rotate<const UNROTATE: bool>(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut a = MaybeUninit::<FVector>::uninit();
        let mut r = MaybeUninit::<FRotator>::uninit();
        vm_frame::step_argument(frame, a.as_mut_ptr().cast(), native_table)?;
        vm_frame::step_argument(frame, r.as_mut_ptr().cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        result.cast::<FVector>().write(rotate(
            a.assume_init(),
            r.assume_init(),
            UNROTATE,
            math::trig_table(),
        ));
        Ok(())
    }
}

/// Evaluate OrthoRotation's three vector axes, then publish the derived rotator.
///
/// # Safety
/// Frame/code/table satisfy vm_frame::step_raw. Result is twelve-byte aligned
/// and writable for an FRotator. No references span argument/debug reentry.
pub unsafe fn execute_ortho_rotation(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut x = MaybeUninit::<FVector>::uninit();
        let mut y = MaybeUninit::<FVector>::uninit();
        let mut z = MaybeUninit::<FVector>::uninit();
        vm_frame::step_argument(frame, x.as_mut_ptr().cast(), native_table)?;
        vm_frame::step_argument(frame, y.as_mut_ptr().cast(), native_table)?;
        vm_frame::step_argument(frame, z.as_mut_ptr().cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        result.cast::<FRotator>().write(ortho_rotation(
            x.assume_init(),
            y.assume_init(),
            z.assume_init(),
            math::trig_table(),
        ));
        Ok(())
    }
}

/// Evaluate Invert's three referenced vector axes in place; there is no result.
///
/// # Safety
/// Frame/code/table satisfy vm_frame::step_raw. Referenced vectors remain live
/// through argument/debug reentry; they may alias. Discarded outputs use
/// local, uninitialized native-compatible storage and are only ever written.
pub unsafe fn execute_invert(
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut x_temp = MaybeUninit::<FVector>::uninit();
        let mut y_temp = MaybeUninit::<FVector>::uninit();
        let mut z_temp = MaybeUninit::<FVector>::uninit();
        let x = vector_reference(frame, x_temp.as_mut_ptr(), native_table)?;
        let y = vector_reference(frame, y_temp.as_mut_ptr(), native_table)?;
        let z = vector_reference(frame, z_temp.as_mut_ptr(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        invert(x, y, z);
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmVectorRandServices {
    pub rand: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub frand: unsafe extern "C" fn(*mut c_void, *mut f32) -> i32,
}
fn status(s: i32) -> Result<(), i32> {
    if s == 0 { Ok(()) } else { Err(s) }
}

/// # Safety
/// Called inside the native RNG trace scope. Services catch native exceptions;
/// result stays live across callbacks. On failure no output is published.
pub unsafe fn random_vector(
    context: *mut c_void,
    result: *mut FVector,
    services: Hp2VmVectorRandServices,
) -> Result<(), i32> {
    unsafe {
        loop {
            let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
            status((services.frand)(context, &mut x))?;
            status((services.frand)(context, &mut y))?;
            status((services.frand)(context, &mut z))?;
            let a = v(
                x.mul_add(2.0, -1.0),
                y.mul_add(2.0, -1.0),
                z.mul_add(2.0, -1.0),
            );
            let sq = dot3(a, a);
            if sq > 1.0 {
                continue;
            }
            // UnsafeNormal, deliberately accepting zero and unordered NaNs.
            let s = (1.0f64 / (sq as f64).sqrt()) as f32;
            result.write(scale(a, s));
            return Ok(());
        }
    }
}

/// # Safety
/// Same scope/error contract as random_vector. Draw order is yaw, pitch, roll;
/// false bRoll consumes only two draws. Native trace occurs inside each draw.
pub unsafe fn random_rotator(
    context: *mut c_void,
    roll: u32,
    result: *mut FRotator,
    services: Hp2VmVectorRandServices,
) -> Result<(), i32> {
    unsafe {
        let (mut yaw, mut pitch, mut r) = (0, 0, 0);
        status((services.rand)(context, &mut yaw))?;
        status((services.rand)(context, &mut pitch))?;
        if roll != 0 {
            status((services.rand)(context, &mut r))?;
        }
        let angle = |n: i32| n.wrapping_mul(2) % 65535;
        result.write(FRotator {
            pitch: angle(pitch),
            yaw: angle(yaw),
            roll: angle(r),
        });
        Ok(())
    }
}

static mut VECTOR_RAND_SERVICES: Option<Hp2VmVectorRandServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmVectorRandServices) {
    unsafe {
        (&raw mut VECTOR_RAND_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmVectorRandServices, i32> {
    unsafe { (&raw const VECTOR_RAND_SERVICES).read().ok_or(1) }
}

/// Compute CallsiteOffset before P_FINISH, then draw a uniform unit vector
/// inside the native RNG scope, which always opens and closes on every path.
///
/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_vrand(
    frame: *mut FFrame,
    result: *mut FVector,
    native_table: *const Hp2VmNativeHandler,
    rand: Hp2VmVectorRandServices,
    trace: &vm_frame::Hp2VmNativeTraceServices,
) -> Result<(), i32> {
    unsafe {
        let offset = vm_frame::callsite_offset(frame, &trace.frame)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let node = vm_frame::open_native_trace_scope(
            frame,
            c"Core.Object.VRand".as_ptr(),
            252,
            offset,
            trace,
        )?;
        let draw = random_vector(std::ptr::null_mut(), result, rand);
        let end = vm_frame::close_native_trace_scope(node, trace);
        draw?;
        end
    }
}

/// Compute CallsiteOffset, decode the optional bRoll operand, then draw a
/// random rotator inside the native RNG scope, which always opens and
/// closes on every path.
///
/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_rotrand(
    frame: *mut FFrame,
    result: *mut FRotator,
    native_table: *const Hp2VmNativeHandler,
    rand: Hp2VmVectorRandServices,
    trace: &vm_frame::Hp2VmNativeTraceServices,
) -> Result<(), i32> {
    unsafe {
        let offset = vm_frame::callsite_offset(frame, &trace.frame)?;
        let mut b_roll: u32 = 0;
        vm_frame::step_argument(frame, (&raw mut b_roll).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let node = vm_frame::open_native_trace_scope(
            frame,
            c"Core.Object.RotRand".as_ptr(),
            320,
            offset,
            trace,
        )?;
        let draw = random_rotator(std::ptr::null_mut(), b_roll, result, rand);
        let end = vm_frame::close_native_trace_scope(node, trace);
        draw?;
        end
    }
}
