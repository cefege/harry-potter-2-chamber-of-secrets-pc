//! Checked string argument evaluation, owning lifetimes and result publication.
//! Name/object identity kernels share the original UnrealScript operator policy.
//! TCHAR comparison/search use the same host CRT as appStrcmp/appStricmp;
//! FString::Caps, unlike appStrupr, only uppercases ASCII.

use crate::ansi::{self, Tchar};
use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use crate::{vm_frame, vm_registry::Hp2VmNativeHandler, vm_state::NAME_NONE};
use hp2_abi::engine::generated::{FFrame, TArrayRaw};
use std::ffi::c_void;
use std::ptr;

/// FString storage includes its terminator; an empty FString may have num == 0.
/// data always points to at least a readable NUL, including for num == 0.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmStringView {
    pub data: *const c_void,
    pub num: i32,
}

static EMPTY: [Tchar; 1] = [0];
const WIDTH: i32 = std::mem::size_of::<Tchar>() as i32;

pub unsafe fn slots(value: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*value).data),
            num: ptr::addr_of_mut!((*value).num),
            max: ptr::addr_of_mut!((*value).max),
        }
    }
}

pub unsafe fn view(value: *mut TArrayRaw) -> Hp2VmStringView {
    unsafe {
        let value = slots(value);
        let num = value.num.read();
        Hp2VmStringView {
            data: if num == 0 {
                EMPTY.as_ptr().cast()
            } else {
                value.data.read_unaligned()
            },
            num,
        }
    }
}
fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

pub trait VmStringStorage {
    type Error;
    fn set(&self, slots: Hp2ArraySlots, count: i32, width: i32, tag: *const c_void)
        -> Result<(), Self::Error>;
    fn shrink(&self, slots: Hp2ArraySlots, width: i32, tag: *const c_void)
        -> Result<(), Self::Error>;
    fn checked_int_size(&self, bytes: usize) -> Result<i32, Self::Error>;
    fn release(&self, slots: Hp2ArraySlots) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy)]
struct LegacyStringStorage(Hp2ArrayServices);
impl VmStringStorage for LegacyStringStorage {
    type Error = i32;
    fn set(&self, slots: Hp2ArraySlots, count: i32, width: i32, tag: *const c_void)
        -> Result<(), i32> {
        unsafe { array_storage::set(slots, count, width, tag, self.0) }
    }
    fn shrink(&self, slots: Hp2ArraySlots, width: i32, tag: *const c_void)
        -> Result<(), i32> {
        unsafe { array_storage::shrink(slots, width, tag, self.0) }
    }
    fn checked_int_size(&self, bytes: usize) -> Result<i32, i32> {
        let mut checked = 0;
        let status = unsafe {
            (self.0.checked_int_size)(self.0.context, bytes, &mut checked)
        };
        if status == 0 { Ok(checked) } else { Err(status) }
    }
    fn release(&self, slots: Hp2ArraySlots) -> Result<(), i32> {
        unsafe { array_storage::release(slots, self.0) }
    }
}

/// Execute the shared FString argument/operation kernel with a direct VM stepper.
///
/// # Safety
/// OP and raw storage obey [`execute`]'s contract. `step` consumes one direct bytecode expression.
pub unsafe fn execute_with<const OP: u32, S, E>(
    frame: *mut FFrame,
    result: *mut c_void,
    storage: &S,
    mut step: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E>
where
    S: VmStringStorage<Error = E>,
{
    unsafe {
        let mut a = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        let mut b = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        let mut index = 0_i32;
        let mut count = if OP == 127 { 65535_i32 } else { 0_i32 };
        let a = &raw mut a;
        let b = &raw mut b;
        let index = &raw mut index;
        let count = &raw mut count;
        let operation = (|| {
            if OP == 236 {
                vm_frame::step_argument_with(frame, index.cast(), &mut step)?;
            } else {
                vm_frame::step_argument_with(frame, a.cast(), &mut step)?;
                match OP {
                    112 | 168 | 115 | 116 | 120 | 121 | 122 | 123 | 124 | 126 => {
                        vm_frame::step_argument_with(frame, b.cast(), &mut step)?;
                    }
                    127 => {
                        vm_frame::step_argument_with(frame, index.cast(), &mut step)?;
                        vm_frame::step_argument_with(frame, count.cast(), &mut step)?;
                    }
                    128 | 234 => vm_frame::step_argument_with(frame, count.cast(), &mut step)?,
                    125 | 235 | 237 => {}
                    _ => unreachable!("invalid string native"),
                }
            }
            vm_frame::finish_arguments_with(frame, &mut step)?;
            let a_view = view(a);
            let b_view = view(b);
            match OP {
                112 | 168 | 127 | 128 | 234 | 235 | 236 => {
                    string_with(OP, a_view, b_view, index.read(), count.read(), result.cast(), storage)
                }
                _ => {
                    scalar(OP, a_view, b_view, result.cast());
                    Ok(())
                }
            }
        })();
        let destroy_b = storage.release(slots(b));
        let destroy_a = storage.release(slots(a));
        preserve_first(operation, destroy_b, destroy_a)
    }
}

/// Evaluate and discard one owning FString with the direct VM stepper.
///
/// # Safety
/// The frame, storage and result ownership follow [`eat`]'s contract.
pub unsafe fn eat_with<S, E>(
    object: *mut c_void,
    frame: *mut FFrame,
    storage: &S,
    mut step: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E>
where
    S: VmStringStorage<Error = E>,
{
    unsafe {
        let mut value = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        let value = &raw mut value;
        let operation = vm_frame::step_with(frame, object, value.cast(), |opcode, context, frame, result| {
            step(opcode, context, frame, result)
        });
        let destroy = storage.release(slots(value));
        match (operation, destroy) {
            (Err(error), _) => Err(error),
            (Ok(()), result) => result,
        }
    }
}

fn preserve_first<E>(first: Result<(), E>, second: Result<(), E>, third: Result<(), E>) -> Result<(), E> {
    match first {
        Err(error) => { let _ = second; let _ = third; Err(error) }
        Ok(()) => match second {
            Err(error) => { let _ = third; Err(error) }
            Ok(()) => third,
        },
    }
}
/// Evaluate owning FString arguments, finish/debug, publish, then destroy in
/// reverse declaration order. Every cleanup runs, preserving the first error.
///
/// # Safety
/// OP selects one of the 17 string operators. Frame/table satisfy step_raw;
/// result is a writable INT or initialized FString according to OP. Allocation
/// leaves own all argument/result storage and cannot unwind. Argument slots
/// remain live through debug and cleanup reentry; no references span callbacks.
pub unsafe fn execute<const OP: u32>(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        execute_with::<OP, _, i32>(frame, result, &LegacyStringStorage(arrays), |opcode, context, frame, result| {
            let handler = native_table.add(opcode as usize).read();
            status(handler(context, frame.cast(), result))
        })
    }
}

/// Evaluate and discard one owning string, without consuming P_FINISH.
///
/// # Safety
/// Object/frame/table satisfy step_raw and arrays owns all expression storage.
pub unsafe fn eat(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        eat_with(object, frame, &LegacyStringStorage(arrays), |opcode, context, frame, result| {
            let handler = native_table.add(opcode as usize).read();
            status(handler(context, frame.cast(), result))
        })
    }
}


fn len(value: Hp2VmStringView) -> i32 {
    if value.num == 0 { 0 } else { value.num - 1 }
}

unsafe fn nul_len(data: *const Tchar) -> i32 {
    // A valid FString's terminated prefix fits its signed 32-bit array count.
    unsafe { ansi::strchr(data, 0).offset_from(data) as i32 }
}

/// # Safety
/// Views are valid terminated FString storage. result is a writable INT slot;
/// it may alias source storage because all reads finish before publication.
pub unsafe fn scalar(op: u32, a: Hp2VmStringView, b: Hp2VmStringView, result: *mut i32) {
    unsafe {
        let a_data = a.data.cast::<Tchar>();
        let b_data = b.data.cast::<Tchar>();
        let value = match op {
            115 | 116 | 120 | 121 | 122 | 123 => {
                let ordering = ansi::strcmp(a_data, b_data);
                i32::from(match op {
                    115 => ordering < 0,
                    116 => ordering > 0,
                    120 => ordering <= 0,
                    121 => ordering >= 0,
                    122 => ordering == 0,
                    123 => ordering != 0,
                    _ => unreachable!(),
                })
            }
            124 => i32::from(ansi::stricmp(a_data, b_data) == 0),
            125 => len(a),
            126 => {
                let found = ansi::strstr(a_data, b_data);
                if found.is_null() {
                    -1
                } else {
                    found.offset_from(a_data) as i32
                }
            }
            237 => a_data.read() as i32,
            _ => unreachable!("invalid string scalar native"),
        };
        result.write(value);
    }
}

/// Name indices and object pointer identities are passed without narrowing.
pub fn identity(op: u32, a: usize, b: usize) -> u32 {
    u32::from(match op {
        254 | 114 => a == b,
        255 | 119 => a != b,
        _ => unreachable!("invalid identity native"),
    })
}

/// Evaluate non-owning name/object operands, finish/debug, then publish a DWORD.
///
/// # Safety
/// OP is 254/255 (32-bit name indices) or 114/119 (full-width object pointers).
/// Frame/table satisfy step_raw and result is a writable DWORD. Operand storage
/// remains live through callbacks; debug may update previously evaluated slots.
pub unsafe fn execute_identity<const OP: u32>(
    frame: *mut FFrame,
    result: *mut u32,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        execute_identity_with::<OP, i32>(frame, result, |opcode, context, frame, value| {
            let handler = native_table.add(opcode as usize).read();
            status(handler(context, frame.cast(), value))
        })
    }
}

/// Evaluate identity operands through the same frame stepping and comparison kernel.
///
/// # Safety
/// OP is 254/255 or 114/119; result and operand storage meet `execute_identity`'s contract.
pub unsafe fn execute_identity_with<const OP: u32, E>(
    frame: *mut FFrame,
    result: *mut u32,
    mut step: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let value = match OP {
            254 | 255 => {
                let mut a = NAME_NONE;
                let mut b = NAME_NONE;
                let a = &raw mut a;
                let b = &raw mut b;
                vm_frame::step_argument_with(frame, a.cast(), &mut step)?;
                vm_frame::step_argument_with(frame, b.cast(), &mut step)?;
                vm_frame::finish_arguments_with(frame, &mut step)?;
                identity(OP, a.read() as usize, b.read() as usize)
            }
            114 | 119 => {
                let mut a: *mut c_void = ptr::null_mut();
                let mut b: *mut c_void = ptr::null_mut();
                let a = &raw mut a;
                let b = &raw mut b;
                vm_frame::step_argument_with(frame, a.cast(), &mut step)?;
                vm_frame::step_argument_with(frame, b.cast(), &mut step)?;
                vm_frame::finish_arguments_with(frame, &mut step)?;
                identity(OP, a.read() as usize, b.read() as usize)
            }
            _ => unreachable!("invalid identity native"),
        };
        result.write(value);
        Ok(())
    }
}

/// # Safety
/// Views describe owning argument copies, disjoint from the initialized result.
/// They remain valid across allocation callbacks. All storage belongs to arrays,
/// whose leaves cannot unwind. No references to native storage span callbacks.
/// Counts satisfy native FString's signed allocation/copy-size contracts.
pub unsafe fn string(
    op: u32,
    a: Hp2VmStringView,
    b: Hp2VmStringView,
    index: i32,
    count: i32,
    result: *mut TArrayRaw,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe { string_with(op, a, b, index, count, result, &LegacyStringStorage(arrays)) }
}

/// # Safety
/// `a` and `b` point to live, terminated FString storage, and `result` points
/// to an initialized writable `TArrayRaw` owned by `arrays`. Source storage
/// remains valid across allocation/reentry callbacks; no references into native
/// storage may be retained across those calls. `op` must be one of the string
/// result operations handled below.
pub unsafe fn string_with<S: VmStringStorage>(
    op: u32,
    a: Hp2VmStringView,
    b: Hp2VmStringView,
    index: i32,
    count: i32,
    result: *mut TArrayRaw,
    arrays: &S,
) -> Result<(), S::Error> {
    unsafe {
        let mut source = a.data.cast::<Tchar>();
        let mut right_len = 0;
        let mut left_len = 0;
        let output_num = match op {
            112 | 168 => {
                // operator+ copies the complete left array, but operator+=
                // measures only the NUL-terminated prefix of its right operand.
                left_len = len(a);
                right_len = nul_len(b.data.cast());
                let length = left_len
                    .wrapping_add(right_len)
                    .wrapping_add(i32::from(op == 168));
                if a.num != 0 || length != 0 {
                    length.wrapping_add(1)
                } else {
                    0
                }
            }
            127 => {
                // End is calculated BEFORE the unsigned Start clamp. Negative
                // counts/indices are not the usual signed substring policy.
                let start = (index as u32).min(len(a) as u32);
                let end = (index.wrapping_add(count) as u32).clamp(start, len(a) as u32);
                source = source.add(start as usize);
                left_len = (end - start) as i32;
                if left_len == 0 { 0 } else { left_len + 1 }
            }
            128 => {
                left_len = count.clamp(0, len(a));
                if left_len == 0 { 0 } else { left_len + 1 }
            }
            234 => {
                source = source.add((len(a) - count.clamp(0, len(a))) as usize);
                left_len = nul_len(source);
                if left_len == 0 { 0 } else { left_len + 1 }
            }
            235 => {
                left_len = nul_len(source);
                if left_len == 0 { 0 } else { left_len + 1 }
            }
            236 => {
                if index == 0 {
                    0
                } else {
                    2
                }
            }
            _ => unreachable!("invalid string result native"),
        };
        let output_slots = slots(result);
        arrays.set(output_slots, output_num, WIDTH, EMPTY.as_ptr().cast())?;
        arrays.shrink(output_slots, WIDTH, EMPTY.as_ptr().cast())?;
        let _checked =
            arrays.checked_int_size((output_num as usize).wrapping_mul(WIDTH as usize))?;
        if output_num == 0 {
            return Ok(());
        }
        let output = output_slots.data.read_unaligned().cast::<Tchar>();
        match op {
            112 | 168 => {
                // No intermediate FString allocation, and no callbacks per TCHAR.
                std::ptr::copy_nonoverlapping(source, output, left_len as usize);
                let mut offset = left_len as usize;
                if op == 168 {
                    output.add(offset).write(b' ' as Tchar);
                    offset += 1;
                }
                std::ptr::copy_nonoverlapping(
                    b.data.cast::<Tchar>(),
                    output.add(offset),
                    right_len as usize + 1,
                );
            }
            127 | 128 => {
                // The counted FString constructor uses wcsncpy, including NUL
                // padding, rather than copying bytes after an embedded NUL.
                ansi::strncpy(output, source, output_num);
            }
            234 => {
                std::ptr::copy_nonoverlapping(source, output, output_num as usize);
            }
            235 => {
                // Valid Caps input has no embedded NUL before its terminator:
                // the legacy loop indexed the original ArrayNum in its shorter
                // NUL-constructed temporary, making other inputs out of bounds.
                for i in 0..left_len as usize {
                    let c = source.add(i).read();
                    output
                        .add(i)
                        .write(if (b'a' as Tchar..=b'z' as Tchar).contains(&c) {
                            c - (b'a' - b'A') as Tchar
                        } else {
                            c
                        });
                }
                output.add(left_len as usize).write(0);
            }
            236 => {
                output.write(index as Tchar);
                output.add(1).write(0);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
