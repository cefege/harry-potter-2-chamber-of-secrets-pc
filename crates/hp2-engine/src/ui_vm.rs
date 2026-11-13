//! Calls UnrealScript functions with typed arguments from Rust, for the engine-drawn menus.
//! Arguments are matched to the function's reflected parameters by name and layout.
use std::ffi::c_void;

use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UFunction, UObject};
use hp2_core::core_access::{CoreAccess, RuntimeResult};

use crate::stock_graphics_menu::{field, function};

/// One argument value.
pub(crate) enum V<'a> {
    Obj(*mut UObject),
    F(f32),
    I(i32),
    /// A `byte` parameter (enum values such as notify events).
    U8(u8),
    Bool(bool),
    S(&'a str),
}

/// Calls `receiver.name(args...)`. `out_floats` names `out float` parameters whose values are
/// returned in order after the call. `result` receives the return value like `execute_event`.
///
/// # Safety
/// `receiver` must be a live object; object arguments must be live objects of the parameter class.
pub(crate) unsafe fn invoke(a: CoreAccess<'_>, receiver: *mut UObject, name: &str, args: &[(&str, V<'_>)], out_floats: &[&str], result: *mut c_void) -> RuntimeResult<Vec<f32>> {
    let f = unsafe { function(a, receiver, name)? };
    unsafe { invoke_function(a, receiver, f, args, out_floats, result) }
}

/// Calls a state-first virtual function without an active bytecode frame.
///
/// # Safety
/// The receiver and object arguments must remain live through synchronous reentry.
pub(crate) unsafe fn invoke_virtual(a: CoreAccess<'_>, receiver: *mut UObject, name: &str, args: &[(&str, V<'_>)], out_floats: &[&str], result: *mut c_void) -> RuntimeResult<Vec<f32>> {
    let name = a.intern_str(name, hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { a.find_object_function_by_name(receiver, name, false)? };
    if function.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Missing virtual function"));
    }
    unsafe { invoke_resolved(a, receiver, function, args, out_floats, result) }
}

/// Calls the exact declared/super function, without virtual redispatch.
///
/// # Safety
/// Function metadata, receiver and object arguments must remain live through reentry.
pub(crate) unsafe fn invoke_resolved(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, args: &[(&str, V<'_>)], out_floats: &[&str], result: *mut c_void) -> RuntimeResult<Vec<f32>> {
    unsafe { invoke_function(a, receiver, function, args, out_floats, result) }
}

unsafe fn invoke_function(a: CoreAccess<'_>, receiver: *mut UObject, f: *mut UFunction, args: &[(&str, V<'_>)], out_floats: &[&str], result: *mut c_void) -> RuntimeResult<Vec<f32>> {
    let size = usize::from(unsafe { (*f).parms_size });
    let mut frame = vec![0u64; size.div_ceil(8).max(1)];
    let base = frame.as_mut_ptr().cast::<u8>();
    let mut strings = Vec::new();
    let outcome = (|| -> RuntimeResult<Vec<f32>> {
        for (arg_name, value) in args {
            let (kind, width) = match value {
                V::Obj(_) => ("Core.ObjectProperty", size_of::<*mut c_void>()),
                V::F(_) => ("Core.FloatProperty", 4),
                V::I(_) => ("Core.IntProperty", 4),
                V::U8(_) => ("Core.ByteProperty", 1),
                V::Bool(_) => ("Core.BoolProperty", 4),
                V::S(_) => ("Core.StrProperty", size_of::<TArrayRaw>()),
            };
            let offset = unsafe { field(a, f.cast(), arg_name, kind, width, size, true)? };
            unsafe {
                match value {
                    V::Obj(o) => base.add(offset).cast::<*mut UObject>().write_unaligned(*o),
                    V::F(v) => base.add(offset).cast::<f32>().write_unaligned(*v),
                    V::I(v) => base.add(offset).cast::<i32>().write_unaligned(*v),
                    V::U8(v) => base.add(offset).write(*v),
                    V::Bool(value) => {
                        let property = a.find_field(
                            f.cast(),
                            a.intern_str(arg_name, hp2_core::name_owner::FindName::Find)?,
                            a.native_class("Core.BoolProperty")?,
                        ).cast::<UBoolProperty>();
                        if property.is_null() || (*property).bit_mask.count_ones() != 1 {
                            return Err(hp2_core::core_access::RuntimeError::new("Invalid boolean argument mask"));
                        }
                        let mask = (*property).bit_mask;
                        let slot = base.add(offset).cast::<u32>();
                        let old = slot.read_unaligned();
                        slot.write_unaligned(if *value { old | mask } else { old & !mask });
                    }
                    V::S(s) => {
                        let mut array: TArrayRaw = std::mem::zeroed();
                        a.assign_fstring(&mut array, &s.chars().map(u32::from).chain([0]).collect::<Vec<_>>())?;
                        base.add(offset).cast::<TArrayRaw>().write_unaligned(array);
                        strings.push(offset);
                    }
                }
            }
        }
        let mut out_offsets = Vec::new();
        for out in out_floats {
            out_offsets.push(unsafe { field(a, f.cast(), out, "Core.FloatProperty", 4, size, true)? });
        }
        unsafe { a.execute_event(receiver, f, base.cast(), result)? };
        Ok(out_offsets.into_iter().map(|offset| unsafe { base.add(offset).cast::<f32>().read_unaligned() }).collect())
    })();
    let mut cleanup = None;
    for offset in strings {
        let mut array = unsafe { base.add(offset).cast::<TArrayRaw>().read_unaligned() };
        if let Err(error) = unsafe { a.destroy_owned_fstring(&mut array) } {
            cleanup = Some(error);
        }
    }
    let outs = outcome?;
    if let Some(error) = cleanup {
        return Err(error);
    }
    Ok(outs)
}
