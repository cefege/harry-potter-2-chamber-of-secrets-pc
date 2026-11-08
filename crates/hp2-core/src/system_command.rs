//! Unix USystem console routing; native leaves retain OS calls and typed lifetimes.

use std::ffi::c_void;
use std::ptr::null;

use crate::ansi::{Tchar, Threw, wide};
use crate::command::parse_command;

/// cbindgen:prefix-with-name
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum Hp2SystemCommandLeaf {
    Memstat,
    App,
    LogRelaunch,
    FlushConfig,
    LaunchServer,
    RequestExit,
    ArgumentTooLarge,
    Crash,
    LogGpf,
    Gpf,
    LogRecurse,
    Recurse,
    LogEatMemory,
    EatMemoryChunk,
}

#[repr(C)]
pub struct Hp2SystemCommandServices {
    pub opaque: *mut c_void,
    /// Nonzero captures the first native exception; no subsequent leaf may run.
    pub invoke: unsafe extern "C" fn(*mut c_void, u32, *const c_void, usize) -> i32,
}

impl Hp2SystemCommandServices {
    unsafe fn call(
        &self,
        leaf: Hp2SystemCommandLeaf,
        text: *const Tchar,
        length: usize,
    ) -> Result<(), Threw> {
        if unsafe { (self.invoke)(self.opaque, leaf as u32, text.cast(), length) } == 0 {
            Ok(())
        } else {
            Err(Threw)
        }
    }
}

/// Operations needed by the shared `USystem::Exec` command policy.
pub trait SystemCommandHost {
    /// Error returned by a host operation.
    type Error;

    /// Runs one typed system command leaf. `text` and `length` are borrowed
    /// synchronously and are meaningful only for leaves that consume arguments.
    ///
    /// # Safety
    /// The pointer must refer to the active NUL-terminated command buffer whenever
    /// the corresponding leaf reads it.
    unsafe fn invoke(
        &mut self,
        leaf: Hp2SystemCommandLeaf,
        text: *const Tchar,
        length: usize,
    ) -> Result<(), Self::Error>;
}

struct NativeHost<'a>(&'a Hp2SystemCommandServices);

impl SystemCommandHost for NativeHost<'_> {
    type Error = Threw;

    unsafe fn invoke(
        &mut self,
        leaf: Hp2SystemCommandLeaf,
        text: *const Tchar,
        length: usize,
    ) -> Result<(), Self::Error> {
        unsafe { self.0.call(leaf, text, length) }
    }
}

const MEMSTAT: [Tchar; 8] = wide(b"MEMSTAT\0");
const APP: [Tchar; 4] = wide(b"APP\0");
const RELAUNCH: [Tchar; 9] = wide(b"RELAUNCH\0");
const DEBUG: [Tchar; 6] = wide(b"DEBUG\0");
const CRASH: [Tchar; 6] = wide(b"CRASH\0");
const GPF: [Tchar; 4] = wide(b"GPF\0");
const RECURSE: [Tchar; 8] = wide(b"RECURSE\0");
const EATMEM: [Tchar; 7] = wide(b"EATMEM\0");

/// Executes the shared system command policy against a typed host.
///
/// # Safety
/// `command` must remain a readable NUL-terminated TCHAR string throughout the
/// synchronous host callbacks. Callbacks must not retain its pointer.
pub unsafe fn exec_with<H: SystemCommandHost>(
    host: &mut H,
    mut command: *const Tchar,
) -> Result<bool, H::Error> {
    // SAFETY: keywords and command are NUL-terminated; host calls are synchronous.
    unsafe {
        if parse_command(&mut command, MEMSTAT.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::Memstat, null(), 0)?;
            return Ok(true);
        }
        if parse_command(&mut command, APP.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::App, null(), 0)?;
            return Ok(true);
        }
        if parse_command(&mut command, RELAUNCH.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::LogRelaunch, command, 0)?;
            host.invoke(Hp2SystemCommandLeaf::FlushConfig, null(), 0)?;
            let mut length = 0usize;
            while *command.add(length) != 0 && *command.add(length) != b' ' as Tchar {
                length += 1;
            }
            if length > usize::MAX / std::mem::size_of::<Tchar>() - 1 {
                host.invoke(Hp2SystemCommandLeaf::ArgumentTooLarge, null(), 0)?;
                return Ok(true);
            }
            host.invoke(Hp2SystemCommandLeaf::LaunchServer, command, length)?;
            host.invoke(Hp2SystemCommandLeaf::RequestExit, null(), 0)?;
            return Ok(true);
        }
        if parse_command(&mut command, DEBUG.as_ptr()) == 0 {
            return Ok(false);
        }
        if parse_command(&mut command, CRASH.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::Crash, null(), 0)?;
        } else if parse_command(&mut command, GPF.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::LogGpf, null(), 0)?;
            host.invoke(Hp2SystemCommandLeaf::Gpf, null(), 0)?;
        } else if parse_command(&mut command, RECURSE.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::LogRecurse, null(), 0)?;
            host.invoke(Hp2SystemCommandLeaf::Recurse, null(), 0)?;
        } else if parse_command(&mut command, EATMEM.as_ptr()) != 0 {
            host.invoke(Hp2SystemCommandLeaf::LogEatMemory, null(), 0)?;
            loop {
                host.invoke(Hp2SystemCommandLeaf::EatMemoryChunk, null(), 65536)?;
            }
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}

/// # Safety
/// The immutable services table and NUL-terminated command remain live throughout
/// the call. Native callbacks return status instead of unwinding through Rust.
/// No native object borrow or lock is retained across callbacks.
pub unsafe fn exec(
    services: &Hp2SystemCommandServices,
    command: *const Tchar,
) -> Result<bool, Threw> {
    let mut host = NativeHost(services);
    unsafe { exec_with(&mut host, command) }
}

struct CoreHost<'a, 'output> {
    access: crate::core_access::CoreAccess<'a>,
    output: &'output mut dyn FnMut(&[u32]) -> crate::core_access::RuntimeResult<()>,
}

impl SystemCommandHost for CoreHost<'_, '_> {
    type Error = crate::core_access::RuntimeError;

    unsafe fn invoke(
        &mut self,
        leaf: Hp2SystemCommandLeaf,
        text: *const Tchar,
        length: usize,
    ) -> crate::core_access::RuntimeResult<()> {
        use crate::core_access::RuntimeError;
        let access = self.access;
        // SAFETY: the parsed command remains live through this synchronous Core operation.
        match leaf {
            Hp2SystemCommandLeaf::Memstat => (self.output)(&"MEMSTAT command not available.".chars().map(u32::from).collect::<Vec<_>>()),
            Hp2SystemCommandLeaf::App => (self.output)(&"APP command not available.".chars().map(u32::from).collect::<Vec<_>>()),
            Hp2SystemCommandLeaf::LogRelaunch => {
                let suffix = unsafe { core_text(text) };
                let mut message: Vec<u32> = "Relaunch: ".chars().map(u32::from).collect();
                message.extend_from_slice(&suffix);
                (self.output)(&message)
            }
            Hp2SystemCommandLeaf::FlushConfig => access.config_flush_units_from(None, false),
            Hp2SystemCommandLeaf::LaunchServer => {
                let mut url = String::new();
                for &unit in unsafe { std::slice::from_raw_parts(text, length) } {
                    url.push(char::from_u32(unit as u32).unwrap_or(char::REPLACEMENT_CHARACTER));
                }
                let url = std::ffi::CString::new(url)
                    .map_err(|_| RuntimeError::new("Server URL contains a null byte"))?;
                // SAFETY: CString is NUL-terminated and remains live through fork/exec.
                let normalized = unsafe { crate::unix_path::unix_path(url.as_ptr()) }
                    .ok_or_else(|| RuntimeError::new("Unable to normalize server URL"))?;
                // SAFETY: `normalized` is the process-global NUL-terminated Unix path.
                if unsafe { crate::unix_process::launch_server(normalized) } < 0 {
                    Err(RuntimeError::new("Failed to launch process."))
                } else {
                    Ok(())
                }
            }
            Hp2SystemCommandLeaf::RequestExit => {
                access.set_running(false);
                Ok(())
            }
            Hp2SystemCommandLeaf::ArgumentTooLarge => Err(RuntimeError::new("Relaunch argument is too large.")),
            Hp2SystemCommandLeaf::Crash => Err(RuntimeError::new("Unreal crashed at your request")),
            Hp2SystemCommandLeaf::LogGpf => (self.output)(&"Unreal crashing with voluntary GPF".chars().map(u32::from).collect::<Vec<_>>()),
            Hp2SystemCommandLeaf::Gpf => std::process::abort(),
            Hp2SystemCommandLeaf::LogRecurse => (self.output)(&"Recursing".chars().map(u32::from).collect::<Vec<_>>()),
            Hp2SystemCommandLeaf::Recurse => recurse(),
            Hp2SystemCommandLeaf::LogEatMemory => (self.output)(&"Eating up all available memory".chars().map(u32::from).collect::<Vec<_>>()),
            Hp2SystemCommandLeaf::EatMemoryChunk => {
                let bytes = unsafe { crate::unix_alloc::system_malloc(length) };
                if bytes.is_null() {
                    return Err(RuntimeError::new("Unable to allocate EatMem memory"));
                }
                unsafe { std::ptr::write_bytes(bytes, 0, length) };
                Ok(())
            }
        }
    }
}

#[inline(never)]
fn recurse() -> ! {
    std::hint::black_box(recurse as fn() -> !)()
}

unsafe fn core_text(mut text: *const Tchar) -> Vec<u32> {
    let mut output = Vec::new();
    while unsafe { *text } != 0 {
        output.push(unsafe { *text } as u32);
        text = unsafe { text.add(1) };
    }
    output
}

/// Executes `USystem::Exec` through canonical Core state and a borrowed output sink.
///
/// # Safety
/// `system` is a live canonical `USystem`; `command` is readable and NUL-terminated.
pub unsafe fn execute_system_command(
    access: crate::core_access::CoreAccess<'_>,
    system: *mut hp2_abi::engine::generated::UObject,
    command: *const u32,
    output: &mut dyn FnMut(&[u32]) -> crate::core_access::RuntimeResult<()>,
) -> crate::core_access::RuntimeResult<bool> {
    if system.is_null() || !(0..access.object_count()).any(|index| access.object_at(index) == system) {
        return Err(crate::core_access::RuntimeError::new("USystem command object is not owned by Core"));
    }
    let system_class = access.native_class("Core.System")?;
    // SAFETY: both identities belong to this Core owner; object_model reads only headers.
    if !unsafe { crate::object_model::is_a(system.cast(), system_class.cast()) } {
        return Err(crate::core_access::RuntimeError::new("Command object is not a USystem"));
    }
    let mut host = CoreHost { access, output };
    // SAFETY: command is NUL-terminated and system is an owned canonical USystem.
    unsafe { exec_with(&mut host, command.cast()) }
}

