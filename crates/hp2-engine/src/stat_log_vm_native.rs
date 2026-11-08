//! Canonical StatLog native argument decoding; policy stays in `stat_log`.

use hp2_abi::engine::generated::{FFrame, TArrayRaw, UObject};
use hp2_core::{
    canonical_vm_native::{VmNativeSpec, finish_arguments, step_argument, step_argument_out},
    core_access::{CoreAccess, RuntimeResult},
};
use std::{ffi::c_void, ptr};

use crate::stat_log::{self, StatLogResult, StatLogServices};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 16] = [
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "ExecuteLocalLogBatcher",
        native_index: None,
        handler: execute_local,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "ExecuteSilentLogBatcher",
        native_index: None,
        handler: execute_silent,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "BatchLocal",
        native_index: None,
        handler: batch_local,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "BrowseRelativeLocalURL",
        native_index: None,
        handler: browse_relative,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "ExecuteWorldLogBatcher",
        native_index: None,
        handler: execute_world,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "InitialCheck",
        native_index: None,
        handler: initial_check,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "LogMutator",
        native_index: None,
        handler: log_mutator,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "GetPlayerChecksum",
        native_index: None,
        handler: player_checksum,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "GetGMTRef",
        native_index: None,
        handler: gmt_ref,
    },
    VmNativeSpec {
        class_name: "Engine.StatLog",
        function_name: "GetMapFileName",
        native_index: None,
        handler: map_file_name,
    },
    VmNativeSpec {
        class_name: "Engine.StatLogFile",
        function_name: "OpenLog",
        native_index: None,
        handler: open_log,
    },
    VmNativeSpec {
        class_name: "Engine.StatLogFile",
        function_name: "CloseLog",
        native_index: None,
        handler: close_log,
    },
    VmNativeSpec {
        class_name: "Engine.StatLogFile",
        function_name: "Watermark",
        native_index: None,
        handler: watermark,
    },
    VmNativeSpec {
        class_name: "Engine.StatLogFile",
        function_name: "GetChecksum",
        native_index: None,
        handler: checksum,
    },
    VmNativeSpec {
        class_name: "Engine.StatLogFile",
        function_name: "FileFlush",
        native_index: None,
        handler: file_flush,
    },
    VmNativeSpec {
        class_name: "Engine.StatLogFile",
        function_name: "FileLog",
        native_index: None,
        handler: file_log,
    },
];

unsafe fn no_arguments(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    operation: impl FnOnce(&StatLogServices) -> StatLogResult,
) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)? };
    crate::stat_log_owner::with_services(&access, operation)
}

unsafe fn string_argument(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    by_reference: bool,
    operation: impl FnOnce(&StatLogServices, *mut c_void) -> StatLogResult,
) -> RuntimeResult<()> {
    let mut temporary = TArrayRaw {
        data: ptr::null_mut(),
        num: 0,
        max: 0,
    };
    let operation = unsafe {
        (|| {
            let text = if by_reference {
                step_argument_out(access, frame, (&raw mut temporary).cast())?
            } else {
                step_argument(access, frame, (&raw mut temporary).cast())?;
                (&raw mut temporary).cast()
            };
            finish_arguments(access, frame)?;
            crate::stat_log_owner::with_services(&access, |services| operation(services, text))
        })()
    };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut temporary) };
    operation.and(release)
}

unsafe fn execute_local(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::execute_local_log_batcher(services, object.cast())
        })
    }
}

unsafe fn execute_silent(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::execute_silent_log_batcher(services, object.cast())
        })
    }
}

unsafe fn batch_local(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::batch_local(services, object.cast());
            Ok(())
        })
    }
}

unsafe fn browse_relative(
    access: CoreAccess<'_>,
    _: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        string_argument(access, frame, false, |services, url| {
            stat_log::browse_relative_local_url(services, url);
            Ok(())
        })
    }
}

unsafe fn execute_world(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::execute_world_log_batcher(services, object.cast())
        })
    }
}

unsafe fn initial_check(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        let mut game = ptr::null_mut::<c_void>();
        step_argument(access, frame, (&raw mut game).cast())?;
        finish_arguments(access, frame)?;
        crate::stat_log_owner::with_services(&access, |services| {
            stat_log::initial_check(services, object.cast(), game)
        })
    }
}

unsafe fn log_mutator(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        let mut mutator = ptr::null_mut::<c_void>();
        step_argument(access, frame, (&raw mut mutator).cast())?;
        finish_arguments(access, frame)?;
        crate::stat_log_owner::with_services(&access, |services| {
            stat_log::log_mutator(services, object.cast(), mutator);
            Ok(())
        })
    }
}

unsafe fn player_checksum(
    access: CoreAccess<'_>,
    _: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    let mut temporary = TArrayRaw {
        data: ptr::null_mut(),
        num: 0,
        max: 0,
    };
    let operation = unsafe {
        (|| {
            let mut pawn = ptr::null_mut::<c_void>();
            step_argument(access, frame, (&raw mut pawn).cast())?;
            let checksum = step_argument_out(access, frame, (&raw mut temporary).cast())?;
            finish_arguments(access, frame)?;
            crate::stat_log_owner::with_services(&access, |services| {
                stat_log::get_player_checksum(services, pawn, checksum)
            })
        })()
    };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut temporary) };
    operation.and(release)
}

unsafe fn gmt_ref(
    access: CoreAccess<'_>,
    _: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::get_gmt_ref(services, result);
            Ok(())
        })
    }
}

unsafe fn map_file_name(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::get_map_file_name(services, object.cast(), result);
            Ok(())
        })
    }
}

unsafe fn open_log(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::open_log(
                services,
                object.cast(),
                hp2_core::lit!("../Logs").as_ptr().cast(),
            );
            Ok(())
        })
    }
}

unsafe fn close_log(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::close_log(services, object.cast());
            Ok(())
        })
    }
}

unsafe fn watermark(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        string_argument(access, frame, false, |services, event| {
            stat_log::watermark(services, object.cast(), event)
        })
    }
}

unsafe fn checksum(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        string_argument(access, frame, true, |services, checksum| {
            stat_log::get_checksum(services, object.cast(), checksum)
        })
    }
}

unsafe fn file_flush(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        no_arguments(access, frame, |services| {
            stat_log::file_flush(services, object.cast());
            Ok(())
        })
    }
}

unsafe fn file_log(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        string_argument(access, frame, false, |services, event| {
            stat_log::file_log(services, object.cast(), event)
        })
    }
}
