//! Canonical Engine file, texture import, decal and platform query natives.
use hp2_abi::engine::generated::{AActor, FFrame, TArrayRaw, ULevel, UObject, UViewport};
use hp2_core::{
    canonical_vm_native::{VmNativeSpec, finish_arguments, step_argument},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    native_text,
};
use std::{ffi::c_void, ptr};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 11] = [
    VmNativeSpec {
        class_name: "Engine.GameInfo",
        function_name: "GetNetworkNumber",
        native_index: None,
        handler: network_number,
    },
    crate::decal_vm_native::NATIVE_FUNCTIONS[0],
    crate::decal_vm_native::NATIVE_FUNCTIONS[1],
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "CreateTextureFromScreenShot",
        native_index: Some(322),
        handler: screen_shot,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "CreateTextureFromBMP",
        native_index: Some(321),
        handler: from_bmp,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SaveObjectAsFile",
        native_index: Some(323),
        handler: save_object,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "LoadObjectAsFile",
        native_index: Some(324),
        handler: load_object,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SaveGameSaveInfo",
        native_index: Some(325),
        handler: save_info,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "LoadGameSaveInfo",
        native_index: Some(326),
        handler: load_info,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "IsOSVer2kOrXP",
        native_index: Some(327),
        handler: is_windows_version,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "IsSoftwareRendering",
        native_index: Some(329),
        handler: is_software_rendering,
    },
];

unsafe fn network_number(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        let level = (*object.cast::<AActor>()).x_level.cast::<ULevel>();
        if level.is_null() {
            return Err(RuntimeError::new("GetNetworkNumber GameInfo has no Level"));
        }
        let driver = (*level)
            .base
            .net_driver
            .cast::<hp2_abi::engine::generated::UNetDriver>();
        let text = if driver.is_null() {
            &[][..]
        } else {
            crate::network_owner::low_level_network_number(access, driver)?
        };
        access.write_vm_fstring_result(result.cast(), native_text::tchar_units(text))
    }
}
unsafe fn is_windows_version(
    access: CoreAccess<'_>,
    _: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        result.cast::<bool>().write_unaligned(false);
    }
    // UnScript.cpp's non-MSVC branch is unconditionally false.
    Ok(())
}
unsafe fn is_software_rendering(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
    }
    let value = access.config_get_units(
        hp2_core::lit!("Engine.Engine"),
        hp2_core::lit!("GameRenderDevice"),
    )?;
    let software = if let Some(mut value) = value {
        value.push(0);
        unsafe {
            hp2_core::ansi::stricmp(
                value.as_ptr().cast(),
                hp2_core::lit!("SoftDrv.SoftwareRenderDevice")
                    .as_ptr()
                    .cast(),
            ) == 0
        }
    } else {
        false
    };
    hp2_core::ansi_trace::with_process_state(|state| unsafe {
        hp2_core::ansi_trace::record_software_rendering(state, object.cast(), software)
    });
    let observer = {
        let state = access
            .owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("Software rendering query has no Engine owner"))?;
        state.spawn_instrumentation.borrow().clone()
    };
    if let Some(observer) = observer {
        observer.on_spawn_instrumentation(
            access,
            crate::core_owner::SpawnInstrumentationEvent::SoftwareRendering {
                actor: object.cast(),
                software,
            },
        )?;
    }
    unsafe {
        result.cast::<bool>().write_unaligned(software);
    }
    Ok(())
}
unsafe fn screen_shot(
    access: CoreAccess<'_>,
    _: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        let mut viewport = ptr::null_mut::<UViewport>();
        step_argument(access, frame, (&raw mut viewport).cast())?;
        finish_arguments(access, frame)?;
        let texture = crate::texture_owner::image::from_screen_shot(access, frame, viewport)?;
        result.cast::<*mut UObject>().write_unaligned(texture);
        Ok(())
    }
}
unsafe fn from_bmp(
    access: CoreAccess<'_>,
    _: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut name = TArrayRaw {
        data: ptr::null_mut(),
        num: 0,
        max: 0,
    };
    let mut text = name;
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut name).cast())?;
            step_argument(access, frame, (&raw mut text).cast())?;
            finish_arguments(access, frame)?;
            let texture =
                crate::texture_owner::image::from_bmp(access, &raw const name, &raw const text)?;
            result.cast::<*mut UObject>().write_unaligned(texture);
            Ok(())
        })()
    };
    let release_name = unsafe { access.release_vm_fstring_argument(&raw mut name) };
    let release_text = unsafe { access.release_vm_fstring_argument(&raw mut text) };
    operation.and(release_name).and(release_text)
}
unsafe fn transfer(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    result: *mut c_void,
    operation: crate::actor_files::FileOperation,
) -> RuntimeResult<()> {
    let mut path = TArrayRaw {
        data: ptr::null_mut(),
        num: 0,
        max: 0,
    };
    let transfer = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut path).cast())?;
            let mut object = ptr::null_mut::<UObject>();
            step_argument(access, frame, (&raw mut object).cast())?;
            finish_arguments(access, frame)?;
            let success = crate::actor_files_owner::transfer_file(
                access,
                operation,
                &raw const path,
                object,
            )?;
            result.cast::<bool>().write_unaligned(success);
            Ok(())
        })()
    };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut path) };
    transfer.and(release)
}
unsafe fn save_object(
    a: CoreAccess<'_>,
    _: *mut UObject,
    f: *mut FFrame,
    r: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { transfer(a, f, r, crate::actor_files::FileOperation::SaveObject) }
}
unsafe fn load_object(
    a: CoreAccess<'_>,
    _: *mut UObject,
    f: *mut FFrame,
    r: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { transfer(a, f, r, crate::actor_files::FileOperation::LoadObject) }
}
unsafe fn save_info(
    a: CoreAccess<'_>,
    _: *mut UObject,
    f: *mut FFrame,
    r: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { transfer(a, f, r, crate::actor_files::FileOperation::SaveInfo) }
}
unsafe fn load_info(
    a: CoreAccess<'_>,
    _: *mut UObject,
    f: *mut FFrame,
    r: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { transfer(a, f, r, crate::actor_files::FileOperation::LoadInfo) }
}
