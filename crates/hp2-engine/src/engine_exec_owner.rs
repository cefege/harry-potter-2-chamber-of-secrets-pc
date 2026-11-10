//! Canonical Engine command dispatch without native UObject vptrs.

#[cfg(test)]
#[path = "object_detail_command_tests.rs"]
mod object_detail_command_tests;
#[cfg(test)]
#[path = "object_detail_menu_vm_tests.rs"]
mod object_detail_menu_vm_tests;

use hp2_abi::engine::generated::{FURL, TArrayRaw, UEngine, UFunction, UObject, URender, UALAudioSubsystem};
use hp2_core::{
    cache_owner::CacheOwner,
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    object_model,
};
use std::{cell::RefCell, ffi::c_void, ptr};

/// Genuine optional Rust counterpart of the process's FExec hook.
/// The engine retains no native object or output-device pointer.
pub type ExecHook = for<'owner, 'borrow, 'output> unsafe fn(
    CoreAccess<'owner>, *const u32, &'borrow mut crate::exec_output::ExecOutput<'output>,
) -> RuntimeResult<bool>;

/// Runs UObject::ScriptConsoleExec with the selected canonical ProcessEvent override.
///
/// # Safety
/// Subject, executor when present, and command belong to this live canonical graph.
/// The output may reenter the owner; no owner-state borrow spans dispatch.
pub unsafe fn process_console_exec(
    access: CoreAccess<'_>, object: *mut UObject, executor: *mut UObject,
    command: *const u32, output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if object.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid canonical ScriptConsoleExec arguments"));
    }
    let actor = access.native_class("Engine.Actor")?;
    let mut process = |subject: *mut UObject, function: *mut UFunction, parameters: *mut c_void| {
        if unsafe { object_model::is_a(subject.cast(), actor.cast()) } {
            unsafe { crate::actor_owner::process_event(access, subject.cast(), function, parameters, ptr::null_mut()) }
        } else {
            unsafe { access.execute_event(subject, function, parameters, ptr::null_mut()) }
        }
    };
    unsafe { access.execute_console_with(object, executor, command, output, &mut process) }
}

struct CacheLease<'a> {
    slot: &'a RefCell<Option<CacheOwner>>,
    cache: Option<CacheOwner>,
}

impl<'a> CacheLease<'a> {
    fn acquire(slot: &'a RefCell<Option<CacheOwner>>) -> RuntimeResult<Self> {
        let cache = slot.try_borrow_mut()
            .map_err(|_| RuntimeError::new("Engine cache command reentered"))?
            .take().ok_or_else(|| RuntimeError::new("Engine cache is already in use"))?;
        Ok(Self { slot, cache: Some(cache) })
    }
}

impl Drop for CacheLease<'_> {
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.cache.take();
    }
}

/// Executes cache commands against the same record used by Engine initialization,
/// resource consumers, Flush and teardown. Restores ownership on error or unwind.
///
/// # Safety
/// Command is NUL terminated and output stays live through synchronous reentry.
pub(crate) unsafe fn execute_cache_command_actual(
    access: CoreAccess<'_>, command: *const u32,
    output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
) -> RuntimeResult<bool> {
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Cache command has no canonical Engine owner"))?;
    let mut lease = CacheLease::acquire(&state.cache)?;
    unsafe {
        lease.cache.as_mut().expect("A live lease owns the cache")
            .exec_with_output(access, command.cast(), output)
    }
}

struct CoreEngineExecOps<'owner, 'borrow, 'output> {
    access: CoreAccess<'owner>,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
}

impl crate::uengine::EngineExecOps for CoreEngineExecOps<'_, '_, '_> {
    type Command = *const u32;
    type Error = RuntimeError;

    unsafe fn system(&mut self, command: &mut Self::Command) -> RuntimeResult<bool> {
        let system = self.access.system()?;
        unsafe { self.access.execute_system_command(system.cast(), *command, self.output) }
    }
    unsafe fn static_exec(&mut self, command: &mut Self::Command) -> RuntimeResult<bool> {
        unsafe { self.access.execute_static_command(*command, self.output) }
    }
    unsafe fn cache(&mut self, command: &mut Self::Command) -> RuntimeResult<bool> {
        unsafe { self.access.execute_cache_command(*command, self.output) }
    }
    unsafe fn global_exec(&mut self, command: &mut Self::Command) -> RuntimeResult<bool> {
        let hook = self.access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine command has no canonical owner"))?
            .exec_hook.get();
        match hook {
            Some(hook) => unsafe { hook(self.access, *command, self.output) },
            None => Ok(false),
        }
    }
    unsafe fn client(&mut self, engine: *mut c_void, command: &mut Self::Command) -> RuntimeResult<bool> {
        let client = unsafe { (*engine.cast::<UEngine>()).client };
        if client.is_null() { return Ok(false); }
        unsafe { crate::platform_owner::exec(self.access, client.cast(), *command, self.output) }
    }
    unsafe fn render(&mut self, engine: *mut c_void, command: &mut Self::Command) -> RuntimeResult<bool> {
        let render = unsafe { (*engine.cast::<UEngine>()).render };
        if render.is_null() { return Ok(false); }
        // RenderBase inherits Subsystem's false Exec. Only Render's native
        // implementation owns the renderer tail consumed by the typed adapter.
        let class = self.access.native_class("Render.Render")?;
        if !unsafe { object_model::is_a(render.cast(), class.cast()) } { return Ok(false); }
        unsafe { crate::render_owner::exec(self.access, render.cast::<URender>(), *command, &mut *self.output) }
    }
    unsafe fn audio(&mut self, engine: *mut c_void, command: &mut Self::Command) -> RuntimeResult<bool> {
        let audio = unsafe { (*engine.cast::<UEngine>()).audio };
        if audio.is_null() { return Ok(false); }
        let class = self.access.native_class("ALAudio.ALAudioSubsystem")?;
        if !unsafe { object_model::is_a(audio.cast(), class.cast()) } { return Ok(false); }
        unsafe { crate::audio_owner::exec(self.access, audio.cast::<UALAudioSubsystem>(), *command, self.output) }
    }
    unsafe fn flush(&mut self, engine: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::engine_owner::flush(self.access, engine.cast(), true) }
    }
    unsafe fn flushed(&mut self, _: &mut Self::Command) -> RuntimeResult<()> {
        (self.output)(hp2_core::lit!("Flushed engine caches"))
    }
    unsafe fn crack_url(&mut self, command: &mut Self::Command) -> RuntimeResult<()> {
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
        // Empty owned headers are valid destruction targets. parse_update
        // constructs the source defaults once before replacing this empty URL.
        let mut scope = CrackUrlScope {
            access: self.access, url: unsafe { std::mem::zeroed() }, live: true,
        };
        unsafe {
            crate::url_owner::UrlOwner::parse_update(
                self.access, &defaults, &mut scope.url, ptr::null_mut(), *command,
                0, // Source TRAVEL_Absolute.
            )?;
        }
        let result = unsafe { crate::uengine::crack_url_with(&mut CoreCrackUrlOps {
            access: self.access, url: &scope.url, defaults: &defaults,
            output: self.output, scratch: Vec::new(),
        }) };
        let cleanup = unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut scope.url) };
        scope.live = false;
        result.and(cleanup)
    }
    unsafe fn parse_command(&mut self, command: &mut Self::Command, keyword: &[u32]) -> RuntimeResult<bool> {
        Ok(unsafe {
            hp2_core::command::parse_command(
                (command as *mut *const u32).cast(), keyword.as_ptr().cast(),
            ) != 0
        })
    }
}

/// Dispatches the native Engine Exec override rather than a qualified UEngine call.
///
/// # Safety
/// Engine, command and output satisfy `exec`'s canonical graph contract.
pub(crate) unsafe fn exec_virtual(
    access: CoreAccess<'_>, engine: *mut UEngine, command: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    let game = access.native_class("Engine.GameEngine")?;
    if unsafe { hp2_core::object_model::is_a(engine.cast(), game.cast()) } {
        unsafe { crate::game_engine_owner::exec(access, engine.cast(), command, output) }
    } else {
        unsafe { exec(access, engine, command, output) }
    }
}

/// Executes the complete shared UEngine command policy against canonical owners.
///
/// # Safety
/// Engine belongs to this live Core graph; command is NUL terminated and the
/// output is live for synchronous reentry. No native virtual tables are used.
pub(crate) unsafe fn exec(
    access: CoreAccess<'_>, engine: *mut UEngine, command: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if engine.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid canonical UEngine Exec arguments"));
    }
    let mut command = command;
    unsafe { crate::uengine::exec_with_ops(
        &mut CoreEngineExecOps { access, output }, engine.cast(), &mut command,
    ) }
}

struct CrackUrlScope<'a> {
    access: CoreAccess<'a>,
    url: FURL,
    live: bool,
}
impl Drop for CrackUrlScope<'_> {
    fn drop(&mut self) {
        if self.live { let _ = unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut self.url) }; }
    }
}

struct CoreCrackUrlOps<'owner, 'url, 'borrow, 'output> {
    access: CoreAccess<'owner>,
    url: &'url FURL,
    defaults: &'url crate::url_owner::UrlDefaults,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
    scratch: Vec<u32>,
}

fn append_decimal(output: &mut Vec<u32>, value: i32) {
    let mut buffer = [0; 11];
    output.extend_from_slice(crate::exec_output::decimal(value, &mut buffer));
}

unsafe fn string_units(raw: &TArrayRaw) -> RuntimeResult<&[u32]> {
    if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical URL string storage"));
    }
    if raw.num == 0 { return Ok(&[]); }
    let units = unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) };
    Ok(units.strip_suffix(&[0]).unwrap_or(units))
}

impl crate::uengine::CrackUrlOps for CoreCrackUrlOps<'_, '_, '_, '_> {
    type Error = RuntimeError;
    unsafe fn valid(&mut self) -> RuntimeResult<bool> { Ok(self.url.valid != 0) }
    unsafe fn option_count(&mut self) -> RuntimeResult<i32> { Ok(self.url.op.num) }
    unsafe fn emit(&mut self, field: crate::uengine::CrackUrlField, index: i32) -> RuntimeResult<()> {
        use crate::uengine::CrackUrlField;
        let (label, text, number) = match field {
            CrackUrlField::Protocol => (hp2_core::lit!("     Protocol: "), Some(&self.url.protocol), None),
            CrackUrlField::Host => (hp2_core::lit!("         Host: "), Some(&self.url.host), None),
            CrackUrlField::Port => (hp2_core::lit!("         Port: "), None, Some(self.url.port)),
            CrackUrlField::Map => (hp2_core::lit!("          Map: "), Some(&self.url.map), None),
            CrackUrlField::NumOptions => (hp2_core::lit!("   NumOptions: "), None, Some(self.url.op.num)),
            CrackUrlField::Portal => (hp2_core::lit!("       Portal: "), Some(&self.url.portal), None),
            CrackUrlField::Option => {
                if index < 0 || index >= self.url.op.num || self.url.op.data.is_null() {
                    return Err(RuntimeError::new("Invalid canonical URL option index"));
                }
                self.scratch.clear();
                self.scratch.extend_from_slice(hp2_core::lit!("     Option "));
                append_decimal(&mut self.scratch, index);
                self.scratch.extend_from_slice(hp2_core::lit!(": "));
                let option = unsafe { &*self.url.op.data.cast::<TArrayRaw>().add(index as usize) };
                self.scratch.extend_from_slice(unsafe { string_units(option)? });
                return (self.output)(&self.scratch);
            }
            CrackUrlField::String => {
                let mut text = unsafe {
                    crate::url_owner::UrlOwner::string(self.access, self.defaults, self.url, false)?
                };
                // Keep owned FString lifetime across formatting and the output
                // callback, including unwind. Normal cleanup errors propagate.
                struct StringScope<'a> { access: CoreAccess<'a>, value: Option<TArrayRaw> }
                impl Drop for StringScope<'_> {
                    fn drop(&mut self) {
                        if let Some(mut value) = self.value.take() {
                            let _ = unsafe { self.access.destroy_owned_fstring(&mut value) };
                        }
                    }
                }
                let mut scope = StringScope { access: self.access, value: Some(text) };
                self.scratch.clear();
                self.scratch.extend_from_slice(hp2_core::lit!("       String: '"));
                self.scratch.extend_from_slice(unsafe { string_units(scope.value.as_ref().unwrap())? });
                self.scratch.push(b'\'' as u32);
                let result = (self.output)(&self.scratch);
                text = scope.value.take().unwrap();
                let cleanup = unsafe { self.access.destroy_owned_fstring(&mut text) };
                return result.and(cleanup);
            }
            CrackUrlField::BadUrl => return (self.output)(hp2_core::lit!("BAD URL")),
        };
        self.scratch.clear();
        self.scratch.extend_from_slice(label);
        if let Some(text) = text { self.scratch.extend_from_slice(unsafe { string_units(text)? }); }
        if let Some(number) = number { append_decimal(&mut self.scratch, number); }
        (self.output)(&self.scratch)
    }
}
