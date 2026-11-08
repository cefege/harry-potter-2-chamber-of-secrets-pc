//! Scalar UnrealScript natives (129–251, through Smerp). Operation numbers are
//! GNatives slots. Scalar handlers own argument stepping and publication;
//! only RNG retains its native scope contract.

use crate::vm_registry::Hp2VmNativeHandler;
use crate::{ansi_math, vm_frame};
use hp2_abi::engine::generated::FFrame;
use std::ffi::c_void;

// Every fixed argument is stepped, even when EndFunctionParms rewinds Code.
// Raw local slots let later parameter/debug handlers mutate earlier arguments;
// none of those values may be staged before P_FINISH has completed.

/// Step native zero-initialized operands, finish, then publish four bytes.
///
/// # Safety
/// OP is a boolean slot with COUNT fixed operands (1..=2).
/// Frame/code/result and initialized table satisfy vm_frame::step_raw. Result
/// is a writable aligned u32 slot. Callbacks may retain argument pointers only
/// until this invocation returns; no frame/argument references span reentry.
#[inline]
pub unsafe fn execute_boolean<const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut u32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    let mut execute = unsafe { legacy_dispatcher(native_table) };
    unsafe { execute_boolean_with::<i32, OP, COUNT>(frame, result, &mut execute) }
}

/// Step native zero-initialized operands, finish, then publish four bytes.
///
/// # Safety
/// OP is a integer slot with COUNT fixed operands (1..=3).
/// Frame/code/result and initialized table satisfy vm_frame::step_raw. Result
/// is a writable aligned i32 slot. Callbacks may retain argument pointers only
/// until this invocation returns; no frame/argument references span reentry.
#[inline]
pub unsafe fn execute_integer<const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    let mut execute = unsafe { legacy_dispatcher(native_table) };
    unsafe { execute_integer_with::<i32, OP, COUNT>(frame, result, &mut execute) }
}

/// Step native zero-initialized operands, finish, then publish four bytes.
///
/// # Safety
/// OP is a float slot with COUNT fixed operands (1..=3).
/// Frame/code/result and initialized table satisfy vm_frame::step_raw. Result
/// is a writable aligned f32 slot. Callbacks may retain argument pointers only
/// until this invocation returns; no frame/argument references span reentry.
#[inline]
pub unsafe fn execute_float<const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut f32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    let mut execute = unsafe { legacy_dispatcher(native_table) };
    unsafe { execute_float_with::<i32, OP, COUNT>(frame, result, &mut execute) }
}

/// Step native zero-initialized operands, finish, then publish four bytes.
///
/// # Safety
/// OP is a float_compare slot with COUNT fixed operands (2..=2).
/// Frame/code/result and initialized table satisfy vm_frame::step_raw. Result
/// is a writable aligned u32 slot. Callbacks may retain argument pointers only
/// until this invocation returns; no frame/argument references span reentry.
#[inline]
pub unsafe fn execute_float_compare<const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut u32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    let mut execute = unsafe { legacy_dispatcher(native_table) };
    unsafe { execute_float_compare_with::<i32, OP, COUNT>(frame, result, &mut execute) }
}

/// Adapt the established native table to the generic frame kernel.
///
/// # Safety
/// `native_table` is live and contains every opcode reached by the operation.
unsafe fn legacy_dispatcher(
    native_table: *const Hp2VmNativeHandler,
) -> impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), i32> {
    move |opcode, object, frame, result| {
        let handler = unsafe { native_table.add(opcode as usize).read() };
        status(unsafe { handler(object, frame.cast(), result) })
    }
}
unsafe fn arguments_with<T, E, const COUNT: usize>(
    frame: *mut FFrame,
    values: *mut T,
    execute: &mut impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        for index in 0..COUNT {
            vm_frame::step_argument_with(frame, values.add(index).cast(), |opcode, object, frame, result| {
                execute(opcode, object, frame, result)
            })?;
        }
        vm_frame::finish_arguments_with(frame, |opcode, object, frame, result| {
            execute(opcode, object, frame, result)
        })
    }
}

/// Execute a fixed-arity boolean operation with the canonical direct bytecode dispatcher.
///
/// # Safety
/// `OP` is a boolean native and `COUNT` is one or two. Frame operands and result
/// satisfy the native four-byte contract; callbacks may mutate the staged slots.
pub unsafe fn execute_boolean_with<E, const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut u32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0_u32; COUNT];
        arguments_with::<u32, E, COUNT>(frame, (&raw mut values).cast(), &mut execute)?;
        result.write(boolean(
            OP,
            values[0],
            if COUNT > 1 { values[1] } else { 0 },
        ));
        Ok(())
    }
}

/// Execute a fixed-arity integer operation with the canonical direct dispatcher.
///
/// # Safety
/// `OP` is an integer native and `COUNT` is one through three. Frame operands
/// and result satisfy the native four-byte contract.
pub unsafe fn execute_integer_with<E, const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut i32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0_i32; COUNT];
        arguments_with::<i32, E, COUNT>(frame, (&raw mut values).cast(), &mut execute)?;
        result.write(integer(
            OP,
            values[0],
            if COUNT > 1 { values[1] } else { 0 },
            if COUNT > 2 { values[2] } else { 0 },
        ));
        Ok(())
    }
}

/// Execute a fixed-arity float operation with the canonical direct dispatcher.
///
/// # Safety
/// `OP` is a float native and `COUNT` is one through three. Frame operands and
/// result satisfy the native four-byte contract.
pub unsafe fn execute_float_with<E, const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut f32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0.0_f32; COUNT];
        arguments_with::<f32, E, COUNT>(frame, (&raw mut values).cast(), &mut execute)?;
        result.write(float(
            OP,
            values[0],
            if COUNT > 1 { values[1] } else { 0.0 },
            if COUNT > 2 { values[2] } else { 0.0 },
        ));
        Ok(())
    }
}

/// Execute a fixed-arity float comparison with the canonical direct dispatcher.
///
/// # Safety
/// `OP` is a float-comparison native and `COUNT` is two. Frame operands and
/// result satisfy the native four-byte contract.
pub unsafe fn execute_float_compare_with<E, const OP: u32, const COUNT: usize>(
    frame: *mut FFrame,
    result: *mut u32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0.0_f32; COUNT];
        arguments_with::<f32, E, COUNT>(frame, (&raw mut values).cast(), &mut execute)?;
        result.write(float_compare(
            OP,
            values[0],
            if COUNT > 1 { values[1] } else { 0.0 },
        ));
        Ok(())
    }
}

unsafe fn arguments_count_with<T, E>(
    frame: *mut FFrame,
    values: *mut T,
    count: usize,
    execute: &mut impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        for index in 0..count {
            vm_frame::step_argument_with(frame, values.add(index).cast(), |opcode, object, frame, result| {
                execute(opcode, object, frame, result)
            })?;
        }
        vm_frame::finish_arguments_with(frame, |opcode, object, frame, result| {
            execute(opcode, object, frame, result)
        })
    }
}

/// Run a runtime-selected boolean kernel through a direct Rust frame dispatcher.
///
/// # Safety
/// `opcode` selects a boolean kernel and `count` matches its fixed operands.
pub unsafe fn execute_boolean_opcode_with<E>(
    opcode: u32,
    count: usize,
    frame: *mut FFrame,
    result: *mut u32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0_u32; 3];
        arguments_count_with(frame, (&raw mut values).cast::<u32>(), count, &mut execute)?;
        result.write(boolean(opcode, values[0], values[1]));
        Ok(())
    }
}

/// Run a runtime-selected integer kernel through a direct Rust frame dispatcher.
///
/// # Safety
/// `opcode` selects an integer kernel and `count` is one through three.
pub unsafe fn execute_integer_opcode_with<E>(
    opcode: u32,
    count: usize,
    frame: *mut FFrame,
    result: *mut i32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0_i32; 3];
        arguments_count_with(frame, (&raw mut values).cast::<i32>(), count, &mut execute)?;
        result.write(integer(opcode, values[0], values[1], values[2]));
        Ok(())
    }
}

/// Run a runtime-selected float kernel through a direct Rust frame dispatcher.
///
/// # Safety
/// `opcode` selects a float kernel and `count` is one through three.
pub unsafe fn execute_float_opcode_with<E>(
    opcode: u32,
    count: usize,
    frame: *mut FFrame,
    result: *mut f32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0.0_f32; 3];
        arguments_count_with(frame, (&raw mut values).cast::<f32>(), count, &mut execute)?;
        result.write(float(opcode, values[0], values[1], values[2]));
        Ok(())
    }
}

/// Run a runtime-selected float comparison through a direct Rust dispatcher.
///
/// # Safety
/// `opcode` selects a binary float-comparison kernel.
pub unsafe fn execute_float_compare_opcode_with<E>(
    opcode: u32,
    frame: *mut FFrame,
    result: *mut u32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut values = [0.0_f32; 2];
        arguments_count_with(frame, (&raw mut values).cast::<f32>(), 2, &mut execute)?;
        result.write(float_compare(opcode, values[0], values[1]));
        Ok(())
    }
}


#[inline]
unsafe fn scalar_reference<T>(
    frame: *mut FFrame,
    temporary: *mut T,
    native_table: *const Hp2VmNativeHandler,
) -> Result<*mut T, i32> {
    unsafe {
        let slot = crate::vm_variable::address_slot();
        slot.write(std::ptr::null_mut());
        vm_frame::step_argument(frame, temporary.cast(), native_table)?;
        let address = slot.read();
        Ok(if address.is_null() {
            temporary
        } else {
            address.cast()
        })
    }
}

/// Evaluate a byte reference operation and publish exactly one byte.
///
/// # Safety
/// OP is 133..=140. Frame/code/table satisfy vm_frame::step_raw. Result and
/// captured lvalue are writable byte slots and may alias. Argument slots stay
/// live through finish/debug reentry; canonical lvalue access is serialized.
#[inline]
pub unsafe fn execute_byte_assign<const OP: u32>(
    frame: *mut FFrame,
    result: *mut u8,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut temporary = 0_u8;
        let mut rhs = 0_u8;
        let target = scalar_reference(frame, &raw mut temporary, native_table)?;
        if OP <= 136 {
            vm_frame::step_argument(frame, (&raw mut rhs).cast(), native_table)?;
        }
        vm_frame::finish_arguments(frame, native_table)?;
        byte_assign(OP, target, rhs, result);
        Ok(())
    }
}

/// Evaluate an integer reference operation, retaining its native RHS width.
///
/// # Safety
/// OP is 159..=166. Frame/code/table satisfy vm_frame::step_raw. Result and
/// captured lvalue are aligned writable INT slots and may alias. Argument slots
/// stay live through finish/debug reentry; canonical lvalue access is serialized.
#[inline]
pub unsafe fn execute_integer_assign<const OP: u32>(
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut temporary = 0_i32;
        let mut integer_rhs = 0_i32;
        let mut float_rhs = 0_f32;
        let target = scalar_reference(frame, &raw mut temporary, native_table)?;
        if OP <= 160 {
            vm_frame::step_argument(frame, (&raw mut float_rhs).cast(), native_table)?;
        } else if OP <= 162 {
            vm_frame::step_argument(frame, (&raw mut integer_rhs).cast(), native_table)?;
        }
        vm_frame::finish_arguments(frame, native_table)?;
        integer_assign(OP, target, integer_rhs, float_rhs, result);
        Ok(())
    }
}

/// Evaluate a float reference operation and publish exactly four bytes.
///
/// # Safety
/// OP is 182..=185. Frame/code/table satisfy vm_frame::step_raw. Result and
/// captured lvalue are aligned writable FLOAT slots and may alias. Argument slots
/// stay live through finish/debug reentry; canonical lvalue access is serialized.
#[inline]
pub unsafe fn execute_float_assign<const OP: u32>(
    frame: *mut FFrame,
    result: *mut f32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let mut temporary = 0_f32;
        let mut rhs = 0_f32;
        let target = scalar_reference(frame, &raw mut temporary, native_table)?;
        vm_frame::step_argument(frame, (&raw mut rhs).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        float_assign(OP, target, rhs, result);
        Ok(())
    }
}

/// Evaluate native short-circuit operands without invoking P_FINISH/debug.
///
/// # Safety
/// OP is 130 or 132. Frame/code/table satisfy vm_frame::step_raw, including an
/// EX_Skip followed by a readable unaligned native-endian WORD after the first
/// operand. Result is a writable aligned DWORD. Both skip destinations are live.
#[inline]
pub unsafe fn execute_short_circuit_with<E, const OP: u32>(
    frame: *mut FFrame,
    result: *mut u32,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut lhs = 0_u32;
        vm_frame::step_argument_with(frame, (&raw mut lhs).cast(), |opcode, context, frame, value| {
            execute(opcode, context, frame, value)
        })?;
        let cursor = std::ptr::addr_of_mut!((*frame).code);
        cursor.write(cursor.read().add(1)); // P_GET_SKIP_OFFSET consumes EX_Skip.
        let skip = crate::vm_bytecode::read::<u16>(cursor);
        let evaluate = match OP {
            130 => lhs != 0,
            132 => lhs == 0,
            _ => unreachable!("invalid short-circuit native"),
        };
        if evaluate {
            let mut rhs = 0_u32;
            vm_frame::step_argument_with(frame, (&raw mut rhs).cast(), |opcode, context, frame, value| {
                execute(opcode, context, frame, value)
            })?;
            result.write(u32::from(rhs != 0));
            cursor.write(cursor.read().add(1));
        } else {
            result.write(u32::from(lhs != 0));
            cursor.write(cursor.read().add(usize::from(skip)));
        }
        Ok(())
    }
}

/// Legacy native-table adapter for the shared short-circuit kernel.
///
/// # Safety
/// OP is 130 or 132. Frame, result and table satisfy
/// [`execute_short_circuit_with`]; handlers preserve the native status contract.
#[inline]
pub unsafe fn execute_short_circuit<const OP: u32>(
    frame: *mut FFrame,
    result: *mut u32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        execute_short_circuit_with::<i32, OP>(frame, result, |opcode, context, frame, value| {
            let handler = native_table.add(opcode as usize).read();
            let status = handler(context, frame.cast(), value);
            if status == 0 { Ok(()) } else { Err(status) }
        })
    }
}

pub fn boolean(op: u32, a: u32, b: u32) -> u32 {
    u32::from(match op {
        129 => a == 0,
        242 => (a != 0) == (b != 0),
        243 | 131 => (a != 0) != (b != 0),
        _ => unreachable!("invalid boolean native"),
    })
}

pub fn integer(op: u32, a: i32, b: i32, c: i32) -> i32 {
    match op {
        141 => !a,
        143 => a.wrapping_neg(),
        144 => a.wrapping_mul(b),
        // ARM64 SDIV returns MIN for MIN/-1; zero is explicitly guarded.
        145 => {
            if b == 0 {
                0
            } else {
                a.wrapping_div(b)
            }
        }
        146 => a.wrapping_add(b),
        147 => a.wrapping_sub(b),
        // LSLV/ASRV/LSRV mask the count to five bits, including negative counts.
        148 => a.wrapping_shl(b as u32),
        149 => a.wrapping_shr(b as u32),
        196 => (a as u32).wrapping_shr(b as u32) as i32,
        150 => i32::from(a < b),
        151 => i32::from(a > b),
        152 => i32::from(a <= b),
        153 => i32::from(a >= b),
        154 => i32::from(a == b),
        155 => i32::from(a != b),
        156 => a & b,
        157 => a ^ b,
        158 => a | b,
        249 => {
            if a <= b {
                a
            } else {
                b
            }
        }
        250 => {
            if a >= b {
                a
            } else {
                b
            }
        }
        251 => {
            if a < b {
                b
            } else if a < c {
                a
            } else {
                c
            }
        }
        _ => unreachable!("invalid integer native"),
    }
}

/// # Safety
/// Both pointers are writable bytes and may alias. The native parser has already
/// finished; no borrowed lvalue may survive a VM callback.
pub unsafe fn byte_assign(op: u32, a: *mut u8, b: u8, result: *mut u8) {
    unsafe {
        // Divide-by-zero does not even read/write the lvalue, but publishes zero.
        if op == 134 && b == 0 {
            result.write(0);
            return;
        }
        let old = a.read();
        let new = match op {
            133 => old.wrapping_mul(b),
            134 => old / b,
            135 => old.wrapping_add(b),
            136 => old.wrapping_sub(b),
            137 | 139 => old.wrapping_add(1),
            138 | 140 => old.wrapping_sub(1),
            _ => unreachable!("invalid byte assignment native"),
        };
        a.write(new);
        result.write(if op == 139 || op == 140 { old } else { new });
    }
}

/// # Safety
/// Pointers are writable INT slots, possibly identical; arguments are parsed.
pub unsafe fn integer_assign(op: u32, a: *mut i32, b: i32, f: f32, result: *mut i32) {
    unsafe {
        // FCVTZS on supported ARM64 saturates infinities/range overflow and maps
        // NaN to zero. Rust's float cast has precisely that behavior.
        let new = match op {
            159 => (a.read() as f32 * f) as i32,
            160 => {
                if f != 0.0 {
                    (a.read() as f32 / f) as i32
                } else {
                    0
                }
            }
            161 => a.read().wrapping_add(b),
            162 => a.read().wrapping_sub(b),
            163 | 165 => a.read().wrapping_add(1),
            164 | 166 => a.read().wrapping_sub(1),
            _ => unreachable!("invalid integer assignment native"),
        };
        let value = if op == 165 || op == 166 {
            a.read()
        } else {
            new
        };
        a.write(new);
        result.write(value);
    }
}

fn abs(a: f32) -> f32 {
    // UnTemplate::Abs deliberately preserves -0 and negates a NaN's sign.
    if a >= 0.0 { a } else { -a }
}

pub fn float_compare(op: u32, a: f32, b: f32) -> u32 {
    u32::from(match op {
        176 => a < b,
        177 => a > b,
        178 => a <= b,
        179 => a >= b,
        180 => a == b,
        181 => a != b,
        // Subtraction/Abs are FLOAT; the literal comparison promotes to DOUBLE.
        210 => (abs(a - b) as f64) < 1.0e-4_f64,
        _ => unreachable!("invalid float comparison native"),
    })
}

pub fn float(op: u32, a: f32, b: f32, c: f32) -> f32 {
    match op {
        169 => -a,
        170 => ansi_math::pow(a as f64, b as f64) as f32,
        171 => a * b,
        172 => a / b,
        173 => ansi_math::fmod(a as f64, b as f64) as f32,
        174 => a + b,
        175 => a - b,
        186 => abs(a),
        187 => ansi_math::sin(a as f64) as f32,
        188 => ansi_math::cos(a as f64) as f32,
        189 => ansi_math::tan(a as f64) as f32,
        190 => ansi_math::atan(a as f64) as f32,
        191 => ansi_math::exp(a as f64) as f32,
        192 => ansi_math::loge(a as f64) as f32,
        193 => ansi_math::sqrt(a as f64) as f32,
        194 => a * a,
        244 => {
            if a <= b {
                a
            } else {
                b
            }
        }
        245 => {
            if a >= b {
                a
            } else {
                b
            }
        }
        246 => {
            if a < b {
                b
            } else if a < c {
                a
            } else {
                c
            }
        }
        // Arguments are V,A,B. The existing ARM64 UnCorSc object uses FMADD.
        247 => a.mul_add(c - b, b),
        248 => smerp(a, b, c),
        _ => unreachable!("invalid float native"),
    }
}

// Match the existing clang ARM64 instruction order, including FNMUL rather
// than negating an already-rounded product (the NaN sign differs).
#[cfg(target_arch = "aarch64")]
fn smerp(v: f32, a: f32, b: f32) -> f32 {
    let result;
    unsafe {
        std::arch::asm!(
            "fmov {t:s}, #3.0",
            "fmul {t:s}, {v:s}, {t:s}",
            "fadd {u:s}, {v:s}, {v:s}",
            "fmul {u:s}, {v:s}, {u:s}",
            "fnmul {u:s}, {v:s}, {u:s}",
            "fmadd {t:s}, {t:s}, {v:s}, {u:s}",
            "fsub {u:s}, {b:s}, {a:s}",
            "fmadd {result:s}, {t:s}, {u:s}, {a:s}",
            v = in(vreg) v, a = in(vreg) a, b = in(vreg) b,
            t = out(vreg) _, u = out(vreg) _, result = out(vreg) result,
            options(pure, nomem, nostack),
        );
    }
    result
}

#[cfg(not(target_arch = "aarch64"))]
fn smerp(v: f32, a: f32, b: f32) -> f32 {
    (v * 3.0).mul_add(v, -(v * (v * (v + v)))).mul_add(b - a, a)
}

/// # Safety
/// Pointers are writable FLOAT slots, possibly identical; arguments are parsed.
pub unsafe fn float_assign(op: u32, a: *mut f32, b: f32, result: *mut f32) {
    unsafe {
        let old = a.read();
        let new = match op {
            182 => old * b,
            183 => old / b,
            184 => old + b,
            185 => old - b,
            _ => unreachable!("invalid float assignment native"),
        };
        a.write(new);
        result.write(new);
    }
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmScalarRandServices {
    pub rand: unsafe extern "C" fn(*mut i32) -> i32,
    pub frand: unsafe extern "C" fn(*mut f32) -> i32,
    /// Native leaf forwarding to `hp2_rs_launcher_creature_generator_rand(object, raw, bound, index)`;
    /// the stack argument is unused by the reporter.
    pub trace: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32, i32) -> i32,
}

static mut RAND_SERVICES: Option<Hp2VmScalarRandServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmScalarRandServices) {
    unsafe {
        (&raw mut RAND_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmScalarRandServices, i32> {
    unsafe { (&raw const RAND_SERVICES).read().ok_or(1) }
}

/// # Safety
/// Called inside the native RNG scope with positive bound and a writable result.
pub unsafe fn random_draw(
    object: *mut c_void,
    stack: *mut c_void,
    bound: i32,
    result: *mut i32,
    s: &Hp2VmScalarRandServices,
) -> Result<(), i32> {
    unsafe {
        let mut raw = 0;
        status((s.rand)(&mut raw))?;
        let index = raw % bound;
        result.write(index);
        status((s.trace)(object, stack, raw, bound, index))
    }
}

/// # Safety
/// Called inside the native FRand scope. Output remains live across callbacks.
pub unsafe fn random_float(result: *mut f32, s: &Hp2VmScalarRandServices) -> Result<(), i32> {
    unsafe {
        let mut value = 0.0;
        status((s.frand)(&mut value))?;
        result.write(value);
        Ok(())
    }
}

/// Compute CallsiteOffset, decode the bound operand, then draw and trace
/// inside the native RNG scope only when bound is positive. The scope closes
/// on every path once opened, including a failed draw.
///
/// # Safety
/// Frame, live native table and services are live; result is a writable i32 slot.
pub unsafe fn execute_rand(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
    rand: &Hp2VmScalarRandServices,
    trace: &vm_frame::Hp2VmNativeTraceServices,
) -> Result<(), i32> {
    unsafe {
        let offset = vm_frame::callsite_offset(frame, &trace.frame)?;
        let mut bound = 0i32;
        vm_frame::step_argument(frame, (&raw mut bound).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        if bound > 0 {
            let node = vm_frame::open_native_trace_scope(
                frame,
                c"Core.Object.Rand".as_ptr(),
                167,
                offset,
                trace,
            )?;
            let stack = frame.cast::<c_void>();
            let draw = random_draw(object, stack, bound, result, rand);
            let end = vm_frame::close_native_trace_scope(node, trace);
            draw?;
            end
        } else {
            result.write(0);
            Ok(())
        }
    }
}

/// Compute CallsiteOffset before P_FINISH, then draw inside the native RNG
/// scope, which always opens unconditionally and closes on every path.
///
/// # Safety
/// Frame, live native table and services are live; result is a writable f32 slot.
pub unsafe fn execute_frand(
    frame: *mut FFrame,
    result: *mut f32,
    native_table: *const Hp2VmNativeHandler,
    rand: &Hp2VmScalarRandServices,
    trace: &vm_frame::Hp2VmNativeTraceServices,
) -> Result<(), i32> {
    unsafe {
        let offset = vm_frame::callsite_offset(frame, &trace.frame)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let node = vm_frame::open_native_trace_scope(
            frame,
            c"Core.Object.FRand".as_ptr(),
            195,
            offset,
            trace,
        )?;
        let draw = random_float(result, rand);
        let end = vm_frame::close_native_trace_scope(node, trace);
        draw?;
        end
    }
}
