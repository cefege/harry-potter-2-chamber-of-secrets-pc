//! VM conversion policy and checked single-expression primitive handlers.
//! Results publish only after operand decoding or fallible parse/lookup succeeds.
use crate::{
    ansi::{self, Tchar},
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    math::{self, FRotator},
    name::PublishedNames,
    object_identity::{self, Hp2IdentityLeaf},
    object_storage::ObjectStorage,
    string_storage, vm_flow, vm_frame,
    vm_registry::Hp2VmNativeHandler,
    vm_state::NAME_NONE,
};
use hp2_abi::{
    FVector,
    engine::generated::{FFrame, TArrayRaw},
};
use std::ffi::c_void;
type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmConvertServices {
    pub is_class: unsafe extern "C" fn(Object, *mut i32) -> i32,
}

static mut CONVERT_SERVICES: Option<Hp2VmConvertServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmConvertServices) {
    unsafe {
        (&raw mut CONVERT_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmConvertServices> {
    unsafe { (&raw const CONVERT_SERVICES).read().ok_or(1) }
}
fn status(value: i32) -> Result {
    if value == 0 { Ok(()) } else { Err(value) }
}
const NONE: [Tchar; 5] = [78, 111, 110, 101, 0];
const TRUE: [Tchar; 5] = [84, 114, 117, 101, 0];
const FALSE: [Tchar; 6] = [70, 97, 108, 115, 101, 0];
const COMMA: [Tchar; 2] = [44, 0];

const fn boolean_text<const N: usize>(text: [Tchar; N]) -> [Tchar; 64] {
    let mut output = [0; 64];
    let mut index = 0;
    while index < N {
        output[index] = text[index];
        index += 1;
    }
    output
}

// These are the actual exported native localization buffers, not snapshots.
pub static mut GTrue: [Tchar; 64] = boolean_text(TRUE);
pub static mut GFalse: [Tchar; 64] = boolean_text(FALSE);

static EMPTY: [Tchar; 1] = [0];
static STRING_TAG: [Tchar; 8] = [70, 83, 116, 114, 105, 110, 103, 0];

unsafe fn string_slots(value: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: std::ptr::addr_of_mut!((*value).data),
            num: std::ptr::addr_of_mut!((*value).num),
            max: std::ptr::addr_of_mut!((*value).max),
        }
    }
}

unsafe fn string_text(value: *mut TArrayRaw) -> *const Tchar {
    unsafe {
        let slots = string_slots(value);
        if slots.num.read() == 0 {
            EMPTY.as_ptr()
        } else {
            slots.data.read_unaligned().cast()
        }
    }
}

unsafe fn formatted<I>(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    mut value: I,
    format: impl FnOnce(I, &mut [std::mem::MaybeUninit<Tchar>; 512]),
) -> Result {
    unsafe {
        vm_frame::step_argument(frame, (&raw mut value).cast(), native_table)?;
        let mut text = [std::mem::MaybeUninit::uninit(); 512];
        format(value, &mut text);
        // FString::Printf constructs a temporary before assigning the result.
        let mut temporary = TArrayRaw {
            data: std::ptr::null_mut(),
            num: 0,
            max: 0,
        };
        let slots = string_slots(&raw mut temporary);
        let operation = string_storage::assign_with_allocator(
            slots,
            None,
            text.as_ptr().cast(),
            true,
            arrays,
            STRING_TAG.as_ptr().cast(),
        )
        .and_then(|()| {
            string_storage::assign_with_allocator(
                string_slots(result.cast()),
                Some(slots),
                std::ptr::null(),
                false,
                arrays,
                STRING_TAG.as_ptr().cast(),
            )
        });
        let cleanup = array_storage::release(slots, arrays);
        operation.and(cleanup)
    }
}

/// # Safety
/// Frame/table satisfy step_argument; result is an initialized owning FString.
/// Arrays is the live canonical allocator, copied before operand evaluation.
pub unsafe fn execute_byte_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe {
        formatted(frame, result, table, arrays, 0_u8, |v, out| {
            ansi::format_int(v as i32, out)
        })
    }
}

/// # Safety
/// See execute_byte_to_string; the operand slot is an initialized INT.
pub unsafe fn execute_int_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe { formatted(frame, result, table, arrays, 0_i32, ansi::format_int) }
}

/// # Safety
/// See execute_byte_to_string; the operand slot is an initialized FLOAT.
pub unsafe fn execute_float_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe { formatted(frame, result, table, arrays, 0_f32, ansi::format_float) }
}

/// # Safety
/// See execute_byte_to_string; the operand slot is an initialized FVector.
pub unsafe fn execute_vector_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe {
        formatted(
            frame,
            result,
            table,
            arrays,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            |value, out| {
                let mut x = [std::mem::MaybeUninit::uninit(); 512];
                let mut y = [std::mem::MaybeUninit::uninit(); 512];
                let mut z = [std::mem::MaybeUninit::uninit(); 512];
                ansi::format_float(value.x, &mut x);
                ansi::format_float(value.y, &mut y);
                ansi::format_float(value.z, &mut z);
                let dest = out.as_mut_ptr().cast::<Tchar>();
                ansi::strcpy(dest, x.as_ptr().cast());
                ansi::strcat(dest, COMMA.as_ptr());
                ansi::strcat(dest, y.as_ptr().cast());
                ansi::strcat(dest, COMMA.as_ptr());
                ansi::strcat(dest, z.as_ptr().cast());
            },
        )
    }
}

/// # Safety
/// See execute_byte_to_string; the operand slot is an initialized FRotator.
pub unsafe fn execute_rotator_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe {
        formatted(
            frame,
            result,
            table,
            arrays,
            FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            |value, out| {
                let pitch = value.pitch & 65535;
                let yaw = value.yaw & 65535;
                let roll = value.roll & 65535;
                let mut p = [std::mem::MaybeUninit::uninit(); 512];
                let mut y = [std::mem::MaybeUninit::uninit(); 512];
                let mut r = [std::mem::MaybeUninit::uninit(); 512];
                ansi::format_int(pitch, &mut p);
                ansi::format_int(yaw, &mut y);
                ansi::format_int(roll, &mut r);
                let dest = out.as_mut_ptr().cast::<Tchar>();
                ansi::strcpy(dest, p.as_ptr().cast());
                ansi::strcat(dest, COMMA.as_ptr());
                ansi::strcat(dest, y.as_ptr().cast());
                ansi::strcat(dest, COMMA.as_ptr());
                ansi::strcat(dest, r.as_ptr().cast());
            },
        )
    }
}

/// # Safety
/// See execute_byte_to_string. Localization writes are engine-thread serialized.
pub unsafe fn execute_bool_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe {
        let mut value = 0_u32;
        vm_frame::step_argument(frame, (&raw mut value).cast(), table)?;
        let text = if value != 0 {
            (&raw const GTrue).cast()
        } else {
            (&raw const GFalse).cast()
        };
        string_storage::assign_with_allocator(
            string_slots(result.cast()),
            None,
            text,
            false,
            arrays,
            STRING_TAG.as_ptr().cast(),
        )
    }
}

unsafe fn parsed<O>(
    frame: *mut FFrame,
    result: *mut O,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    parse: impl FnOnce(*const Tchar) -> Result<O>,
) -> Result {
    unsafe {
        let mut value = TArrayRaw {
            data: std::ptr::null_mut(),
            num: 0,
            max: 0,
        };
        let value = &raw mut value;
        let operation = (|| {
            vm_frame::step_argument(frame, value.cast(), table)?;
            let output = parse(string_text(value))?;
            result.write(output);
            Ok(())
        })();
        let cleanup = array_storage::release(string_slots(value), arrays);
        operation.and(cleanup)
    }
}

/// # Safety
/// Frame/table satisfy step_argument; result is an initialized owning FString.
/// Names and allocator are copies of the live canonical providers acquired
/// before decoding. Only the operand is consumed, never P_FINISH.
pub unsafe fn execute_name_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    names: PublishedNames,
) -> Result {
    unsafe {
        let mut value = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut value).cast(), table)?;
        string_storage::assign_with_allocator(
            string_slots(result.cast()),
            None,
            names.text(value),
            false,
            arrays,
            STRING_TAG.as_ptr().cast(),
        )
    }
}

unsafe fn object_path(
    object: Object,
    stop: Object,
    text: *mut Tchar,
    names: PublishedNames,
) -> Result {
    use Hp2IdentityLeaf::*;
    unsafe {
        object_identity::path(object as usize, stop as usize, |leaf, a, b| {
            let object = a as *const hp2_abi::engine::generated::UObject;
            match leaf {
                Outer => return Ok((&raw const (*object).outer).read() as usize),
                Clear => text.write(0),
                None => {
                    ansi::strcpy(text, NONE.as_ptr());
                }
                AppendName => {
                    let index = (&raw const (*object).name).cast::<i32>().read_unaligned();
                    ansi::strcat(text, names.text(index));
                }
                AppendDot => {
                    ansi::strcat(text, [46, 0].as_ptr());
                }
                PathCall => object_path(a as Object, b as Object, text, names)?,
                _ => unreachable!("non-path identity leaf"),
            }
            Ok(0)
        })
    }
}

/// # Safety
/// See execute_name_to_string. Non-null operands are live compiler-probed
/// UObject headers, including every outer. Native's 1024-TCHAR scratch contract
/// and canonical ring lifetime apply to the resulting path.
pub unsafe fn execute_object_to_string(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    names: PublishedNames,
) -> Result {
    unsafe {
        let mut value: Object = std::ptr::null_mut();
        vm_frame::step_argument(frame, (&raw mut value).cast(), table)?;
        let text = if value.is_null() {
            NONE.as_ptr()
        } else {
            let text = ansi::static_string_1024();
            object_path(value, std::ptr::null_mut(), text, names)?;
            text.cast_const()
        };
        string_storage::assign_with_allocator(
            string_slots(result.cast()),
            None,
            text,
            false,
            arrays,
            STRING_TAG.as_ptr().cast(),
        )
    }
}

/// # Safety
/// Frame/table satisfy step_argument; result is a writable native FName.
/// The initialized owning FString operand is released on success or failure;
/// lookup uses FNAME_Find, never interns an absent spelling or consumes P_FINISH.
pub unsafe fn execute_string_to_name(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    names: PublishedNames,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<i32>(), table, arrays, |text| {
            names.find(text)
        })
    }
}

unsafe fn parse_integer(text: *const Tchar, ansi: ansi::PublishedAnsi) -> Result<i32> {
    unsafe {
        ansi::atoi(
            &ansi.services,
            text,
            ansi.int_parse_file,
            ansi.int_parse_line,
        )
        .map_err(|_| 1)
    }
}

/// Parses one already-decoded Core string using the same CRT-backed conversion as the VM kernel.
///
/// # Safety
/// `text` points to readable, NUL-terminated TCHAR storage and `ansi` is the published Core ANSI
/// owner for this engine thread.
pub(crate) unsafe fn parse_string_to_int(
    text: *const Tchar,
    ansi: ansi::PublishedAnsi,
) -> Result<i32> {
    unsafe { parse_integer(text, ansi) }
}

/// Parses a Core string to FLOAT using the VM kernel's `appAtof` policy.
///
/// # Safety
/// `text` points to readable, NUL-terminated TCHAR storage and `ansi` is the published Core ANSI
/// owner for this engine thread.
pub(crate) unsafe fn parse_string_to_float(
    text: *const Tchar,
    ansi: ansi::PublishedAnsi,
) -> Result<f32> {
    unsafe { ansi::atof(&ansi.services, text).map_err(|_| 1) }
}

/// Parses a Core string to FVector using the VM kernel's comma-prefix policy.
///
/// # Safety
/// `text` points to readable, NUL-terminated TCHAR storage and `ansi` is the published Core ANSI
/// owner for this engine thread.
pub(crate) unsafe fn parse_string_to_vector(
    text: *const Tchar,
    ansi: ansi::PublishedAnsi,
) -> Result<FVector> {
    unsafe {
        let mut stream = text;
        let mut values = [0.0_f32; 3];
        for (index, value) in values.iter_mut().enumerate() {
            *value = ansi::atof(&ansi.services, stream).map_err(|_| 1)?;
            if index == 2 { break; }
            stream = ansi::strstr(stream, COMMA.as_ptr());
            if stream.is_null() { break; }
            stream = stream.add(1);
        }
        Ok(FVector { x: values[0], y: values[1], z: values[2] })
    }
}

/// Parses a Core string to FRotator using the VM kernel's comma-prefix policy.
///
/// # Safety
/// `text` points to readable, NUL-terminated TCHAR storage and `ansi` is the published Core ANSI
/// owner for this engine thread.
pub(crate) unsafe fn parse_string_to_rotator(
    text: *const Tchar,
    ansi: ansi::PublishedAnsi,
) -> Result<FRotator> {
    unsafe {
        let mut stream = text;
        let mut values = [0_i32; 3];
        for (index, value) in values.iter_mut().enumerate() {
            *value = parse_integer(stream, ansi)?;
            if index == 2 { break; }
            stream = ansi::strstr(stream, COMMA.as_ptr());
            if stream.is_null() { break; }
            stream = stream.add(1);
        }
        Ok(FRotator { pitch: values[0], yaw: values[1], roll: values[2] })
    }
}

/// Parses a Core string to BOOL with localized truth/falsity and integer fallback.
///
/// # Safety
/// `text` points to readable, NUL-terminated TCHAR storage and `ansi` is the published Core ANSI
/// owner for this engine thread.
pub(crate) unsafe fn parse_string_to_bool(
    text: *const Tchar,
    ansi: ansi::PublishedAnsi,
) -> Result<i32> {
    unsafe {
        if ansi::stricmp(text, TRUE.as_ptr()) == 0
            || ansi::stricmp(text, (&raw const GTrue).cast()) == 0
        {
            Ok(1)
        } else if ansi::stricmp(text, FALSE.as_ptr()) == 0
            || ansi::stricmp(text, (&raw const GFalse).cast()) == 0
        {
            Ok(0)
        } else {
            parse_integer(text, ansi).map(|value| i32::from(value != 0))
        }
    }
}

/// # Safety
/// Frame/table satisfy step_argument; result is a writable BYTE. Arrays owns
/// the initialized FString operand; ANSI leaves preserve native exceptions.
pub unsafe fn execute_string_to_byte(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    ansi: ansi::PublishedAnsi,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<u8>(), table, arrays, |text| {
            parse_integer(text, ansi).map(|v| v as u8)
        })
    }
}

/// # Safety
/// See execute_string_to_byte; result is a writable INT.
pub unsafe fn execute_string_to_int(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    ansi: ansi::PublishedAnsi,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<i32>(), table, arrays, |text| {
            parse_integer(text, ansi)
        })
    }
}

/// # Safety
/// See execute_string_to_byte; result is a writable FLOAT.
pub unsafe fn execute_string_to_float(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    ansi: ansi::PublishedAnsi,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<f32>(), table, arrays, |text| {
            ansi::atof(&ansi.services, text).map_err(|_| 1)
        })
    }
}

/// # Safety
/// See execute_string_to_byte; result is a writable FVector. Each component
/// parses via atof; a missing trailing comma leaves later components at
/// their zero-initialized default, matching the comma-prefix contract.
pub unsafe fn execute_string_to_vector(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    ansi: ansi::PublishedAnsi,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<FVector>(), table, arrays, |text| {
            let mut stream = text;
            let mut values = [0.0_f32; 3];
            for (index, value) in values.iter_mut().enumerate() {
                *value = ansi::atof(&ansi.services, stream).map_err(|_| 1)?;
                if index == 2 {
                    break;
                }
                stream = ansi::strstr(stream, COMMA.as_ptr());
                if stream.is_null() {
                    break;
                }
                stream = stream.add(1);
            }
            Ok(FVector {
                x: values[0],
                y: values[1],
                z: values[2],
            })
        })
    }
}

/// # Safety
/// See execute_string_to_vector; result is a writable FRotator and each
/// component parses via the atoi range-check contract.
pub unsafe fn execute_string_to_rotator(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    ansi: ansi::PublishedAnsi,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<FRotator>(), table, arrays, |text| {
            let mut stream = text;
            let mut values = [0_i32; 3];
            for (index, value) in values.iter_mut().enumerate() {
                *value = parse_integer(stream, ansi)?;
                if index == 2 {
                    break;
                }
                stream = ansi::strstr(stream, COMMA.as_ptr());
                if stream.is_null() {
                    break;
                }
                stream = stream.add(1);
            }
            Ok(FRotator {
                pitch: values[0],
                yaw: values[1],
                roll: values[2],
            })
        })
    }
}

/// # Safety
/// See execute_string_to_byte; result is a writable UBOOL and localization
/// writes are engine-thread serialized. English true precedes localized true,
/// both preceding either false spelling and the decimal numeric fallback.
pub unsafe fn execute_string_to_bool(
    frame: *mut FFrame,
    result: Object,
    table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    ansi: ansi::PublishedAnsi,
) -> Result {
    unsafe {
        parsed(frame, result.cast::<i32>(), table, arrays, |text| {
            if ansi::stricmp(text, TRUE.as_ptr()) == 0
                || ansi::stricmp(text, (&raw const GTrue).cast()) == 0
            {
                Ok(1)
            } else if ansi::stricmp(text, FALSE.as_ptr()) == 0
                || ansi::stricmp(text, (&raw const GFalse).cast()) == 0
            {
                Ok(0)
            } else {
                parse_integer(text, ansi).map(|v| i32::from(v != 0))
            }
        })
    }
}
pub(crate) fn byte_to_int(value: u8) -> i32 {
    value as i32
}

pub(crate) fn byte_to_bool(value: u8) -> u32 {
    u32::from(value != 0)
}

pub(crate) fn byte_to_float(value: u8) -> f32 {
    value as f32
}

pub(crate) fn int_to_byte(value: i32) -> u8 {
    value as u8
}

pub(crate) fn int_to_bool(value: i32) -> i32 {
    i32::from(value != 0)
}

pub(crate) fn int_to_float(value: i32) -> f32 {
    value as f32
}

pub(crate) fn bool_to_byte(value: u32) -> u8 {
    (value & 1) as u8
}

pub(crate) fn bool_to_int(value: u32) -> i32 {
    (value & 1) as i32
}

pub(crate) fn bool_to_float(value: u32) -> f32 {
    (value & 1) as f32
}

// Legacy arm64 stores narrow signed FCVTZS results to a byte.
// Preserve its negative wrapping and signed saturation boundaries.
pub(crate) fn float_to_byte(value: f32) -> u8 {
    (value as i32) as u8
}

pub(crate) fn float_to_int(value: f32) -> i32 {
    value as i32
}

pub(crate) fn float_to_bool(value: f32) -> u32 {
    u32::from(value != 0.0)
}

pub(crate) fn object_to_bool(value: Object) -> u32 {
    u32::from(!value.is_null())
}

pub(crate) fn vector_to_bool(value: FVector) -> u32 {
    u32::from(value.x != 0.0 || value.y != 0.0 || value.z != 0.0)
}

pub(crate) fn rotator_to_bool(value: FRotator) -> u32 {
    u32::from((value.pitch | value.yaw | value.roll) & 65535 != 0)
}

pub(crate) fn name_to_bool(value: i32, none_name: i32) -> u32 {
    u32::from(value != none_name)
}

// The initialized parameter is the stable operand slot for the single Step.
// No frame/table borrow survives the callback and no terminator is consumed.
#[inline]
unsafe fn expression<I, O>(
    frame: *mut FFrame,
    result: *mut O,
    native_table: *const Hp2VmNativeHandler,
    mut value: I,
    convert: impl FnOnce(I) -> O,
) -> Result {
    unsafe {
        vm_frame::step_argument(frame, (&raw mut value).cast(), native_table)?;
        result.write(convert(value));
        Ok(())
    }
}

/// Decode one initialized u8 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for i32; operand callbacks honor the u8 slot representation.
pub unsafe fn execute_byte_to_int(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, byte_to_int) }
}

/// Decode one initialized u8 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u32; operand callbacks honor the u8 slot representation.
pub unsafe fn execute_byte_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, byte_to_bool) }
}

/// Decode one initialized u8 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for f32; operand callbacks honor the u8 slot representation.
pub unsafe fn execute_byte_to_float(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, byte_to_float) }
}

/// Decode one initialized i32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u8; operand callbacks honor the i32 slot representation.
pub unsafe fn execute_int_to_byte(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, int_to_byte) }
}

/// Decode one initialized i32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for i32; operand callbacks honor the i32 slot representation.
pub unsafe fn execute_int_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, int_to_bool) }
}

/// Decode one initialized i32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for f32; operand callbacks honor the i32 slot representation.
pub unsafe fn execute_int_to_float(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, int_to_float) }
}

/// Decode one initialized u32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u8; operand callbacks honor the u32 slot representation.
pub unsafe fn execute_bool_to_byte(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, bool_to_byte) }
}

/// Decode one initialized u32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for i32; operand callbacks honor the u32 slot representation.
pub unsafe fn execute_bool_to_int(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, bool_to_int) }
}

/// Decode one initialized u32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for f32; operand callbacks honor the u32 slot representation.
pub unsafe fn execute_bool_to_float(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0, bool_to_float) }
}

/// Decode one initialized f32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u8; operand callbacks honor the f32 slot representation.
pub unsafe fn execute_float_to_byte(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0.0, float_to_byte) }
}

/// Decode one initialized f32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for i32; operand callbacks honor the f32 slot representation.
pub unsafe fn execute_float_to_int(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0.0, float_to_int) }
}

/// Decode one initialized f32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u32; operand callbacks honor the f32 slot representation.
pub unsafe fn execute_float_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe { expression(frame, result.cast(), native_table, 0.0, float_to_bool) }
}

/// Decode one initialized Object operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u32; operand callbacks honor the Object slot representation.
pub unsafe fn execute_object_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe {
        expression(
            frame,
            result.cast(),
            native_table,
            std::ptr::null_mut(),
            object_to_bool,
        )
    }
}

/// Decode one initialized i32 operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u32; operand callbacks honor the i32 slot representation.
pub unsafe fn execute_name_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe {
        expression(frame, result.cast(), native_table, NAME_NONE, |value| {
            name_to_bool(value, NAME_NONE)
        })
    }
}

/// Decode one initialized FVector operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u32; operand callbacks honor the FVector slot representation.
pub unsafe fn execute_vector_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe {
        expression(
            frame,
            result.cast(),
            native_table,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            vector_to_bool,
        )
    }
}

/// Decode one initialized FVector operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for FRotator; operand callbacks honor the FVector slot representation.
pub unsafe fn execute_vector_to_rotator(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe {
        expression(
            frame,
            result.cast(),
            native_table,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            math::vector_rotation,
        )
    }
}

/// Decode one initialized FRotator operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for u32; operand callbacks honor the FRotator slot representation.
pub unsafe fn execute_rotator_to_bool(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe {
        expression(
            frame,
            result.cast(),
            native_table,
            FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            rotator_to_bool,
        )
    }
}

/// Decode one initialized FRotator operand and publish its conversion only on success.
///
/// # Safety
/// Frame and live native table satisfy vm_frame::step_raw. Result is writable
/// and aligned for FVector; operand callbacks honor the FRotator slot representation.
pub unsafe fn execute_rotator_to_vector(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
) -> Result {
    unsafe {
        expression(
            frame,
            result.cast(),
            native_table,
            FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            |r| math::rotator_vector(r, math::trig_table()),
        )
    }
}
/// Apply the source dynamic/metaclass cast policy over a direct class query.
///
/// # Safety
/// Object/class pointers are live and `result` is a writable object-pointer slot.
pub unsafe fn cast_with<E>(
    object: Object,
    class: Object,
    meta: bool,
    result: *mut Object,
    is_class: impl FnOnce(Object) -> std::result::Result<bool, E>,
) -> std::result::Result<(), E> {
    let matches = if object.is_null() {
        false
    } else if meta {
        is_class(object)? && unsafe { crate::object_model::is_child_of(object, class) }
    } else {
        unsafe { crate::object_model::is_a(object, class) }
    };
    unsafe { result.write(if matches { object } else { std::ptr::null_mut() }); }
    Ok(())
}

/// # Safety
/// Native object/class pointers are live; result is a writable object-pointer
/// slot. `IsA`/`IsChildOf` are plain `SuperField`-chain reads with no native
/// exceptions; only the remaining `is_class` leaf (needs `UClass::StaticClass()`)
/// crosses into C++ and can fail.
pub unsafe fn cast(
    object: Object,
    class: Object,
    meta: bool,
    result: *mut Object,
    s: &Hp2VmConvertServices,
) -> Result {
    unsafe {
        cast_with(object, class, meta, result, |object| {
            let mut matches = 0;
            status((s.is_class)(object, &mut matches))?;
            Ok(matches != 0)
        })
    }
}

/// Decode the literal class operand, step the castee expression, and publish
/// the cast result only after both operands and the query succeed.
///
/// # Safety
/// Frame, live native table and object storage are live; result is a writable
/// object-pointer slot. Convert services catch native exceptions.
pub unsafe fn execute_dynamic_cast(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
    storage: *const ObjectStorage,
    services: &Hp2VmConvertServices,
) -> Result {
    unsafe {
        let cursor = std::ptr::addr_of_mut!((*frame).code);
        let class = vm_flow::read_object_operand(cursor, storage);
        let mut castee: Object = std::ptr::null_mut();
        vm_frame::step_argument(frame, (&raw mut castee).cast(), native_table)?;
        cast(castee, class, false, result.cast(), services)
    }
}

/// Decode the literal metaclass operand, step the castee expression, and
/// publish the cast result only after both operands and the query succeed.
///
/// # Safety
/// Frame, live native table and object storage are live; result is a writable
/// object-pointer slot. Convert services catch native exceptions.
pub unsafe fn execute_meta_cast(
    frame: *mut FFrame,
    result: Object,
    native_table: *const Hp2VmNativeHandler,
    storage: *const ObjectStorage,
    services: &Hp2VmConvertServices,
) -> Result {
    unsafe {
        let cursor = std::ptr::addr_of_mut!((*frame).code);
        let class = vm_flow::read_object_operand(cursor, storage);
        let mut castee: Object = std::ptr::null_mut();
        vm_frame::step_argument(frame, (&raw mut castee).cast(), native_table)?;
        cast(castee, class, true, result.cast(), services)
    }
}
