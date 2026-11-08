use hp2_abi::engine::generated::{AActor, FFrame, UObject};
use hp2_core::{canonical_vm_native::{finish_arguments, step_argument, step_argument_out, VmNativeSpec}, core_access::{CoreAccess, RuntimeResult}};
use std::{ffi::c_void, ptr};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 1] = [VmNativeSpec {
    class_name: "Engine.Actor", function_name: "TraceTexture", native_index: Some(285), handler: trace_texture,
}];

unsafe fn trace_texture(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut end = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut start = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut flags_temp = 0_i32;
    let mut trace_decals = 0_u32;
    unsafe {
        step_argument(access, frame, (&raw mut end).cast())?;
        step_argument(access, frame, (&raw mut start).cast())?;
        let flags = step_argument_out(access, frame, (&raw mut flags_temp).cast())?.cast::<i32>();
        step_argument(access, frame, (&raw mut trace_decals).cast())?;
        finish_arguments(access, frame)?;
        // The initialized value is the legacy optional-bool default when absent.
        let flags = if flags.is_null() { &raw mut flags_temp } else { flags };
        let texture = crate::trace_texture_owner::trace_texture(access, object.cast::<AActor>(), end, start, flags, trace_decals != 0)?;
        result.cast::<*mut c_void>().write_unaligned(texture);
    }
    Ok(())
}
