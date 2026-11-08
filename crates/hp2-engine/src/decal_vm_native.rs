//! Canonical `Engine.Decal` script-native argument decoders.

use crate::decal_owner;
use hp2_abi::{
    FVector,
    engine::generated::{ADecal, FFrame, UObject},
};
use hp2_core::{
    canonical_vm_native::{VmNativeSpec, finish_arguments, step_argument},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
};
use std::{ffi::c_void, ptr};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 2] = [
    VmNativeSpec {
        class_name: "Engine.Decal",
        function_name: "AttachDecal",
        native_index: None,
        handler: attach_decal,
    },
    VmNativeSpec {
        class_name: "Engine.Decal",
        function_name: "DetachDecal",
        native_index: None,
        handler: detach_decal,
    },
];

unsafe fn attach_decal(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        let mut distance = 0.0_f32;
        step_argument(access, frame, (&raw mut distance).cast())?;
        // `P_GET_VECTOR_OPTX(DecalDir,FVector(0,0,0))`.
        let mut direction = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        step_argument(access, frame, (&raw mut direction).cast())?;
        finish_arguments(access, frame)?;
        if result.is_null() {
            return Err(RuntimeError::new(
                "Null canonical ADecal.AttachDecal result",
            ));
        }
        // The source initializes the UObject result before any callback can fail.
        result
            .cast::<*mut c_void>()
            .write_unaligned(ptr::null_mut());
        let texture = decal_owner::attach(access, object.cast::<ADecal>(), distance, direction)?;
        result.cast::<*mut c_void>().write_unaligned(texture);
        Ok(())
    }
}

unsafe fn detach_decal(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        decal_owner::detach(access, object.cast::<ADecal>())
    }
}
