//! Canonical `ULevel::Exec` dispatch over Rust-owned network records.

use crate::exec_output::ExecOutput;
use hp2_abi::engine::generated::ULevel;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};

/// Dispatches command input through the live Level network and demo drivers.
///
/// The Level policy checks the network driver first, then the demo-recording
/// driver, before handling Level-local commands. Commands not consumed here are
/// left for the caller's source-ordered GameInfo and Engine fallback.
///
/// # Safety
/// `level` must be a live canonical `ULevel`; `cmd` must be a NUL-terminated
/// UTF-32 command string. The output sink and all referenced driver records must
/// remain live for the synchronous call.
pub unsafe fn exec(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    cmd: *const u32,
    output: &mut ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if level.is_null() || cmd.is_null() {
        return Err(RuntimeError::new("Invalid canonical Level Exec arguments"));
    }

    let net_driver = unsafe { (*level).base.net_driver.cast::<hp2_abi::engine::generated::UNetDriver>() };
    if !net_driver.is_null() && unsafe { crate::network_owner::exec(access, net_driver, cmd, output)? } {
        return Ok(true);
    }

    let demo_driver = unsafe { (*level).base.demo_rec_driver.cast::<hp2_abi::engine::generated::UDemoRecDriver>() };
    if !demo_driver.is_null() && unsafe { crate::demo_owner::exec(access, demo_driver, cmd, output)? } {
        return Ok(true);
    }

    let mut command = cmd;
    if unsafe { hp2_core::command::parse_command((&mut command as *mut *const u32).cast(), hp2_core::lit!("DEMOREC").as_ptr().cast()) } != 0 {
        return unsafe { start_demo_record(access, level, command, output) };
    }
    let mut command = cmd;
    if unsafe { hp2_core::command::parse_command((&mut command as *mut *const u32).cast(), hp2_core::lit!("DEMOPLAY").as_ptr().cast()) } != 0 {
        return unsafe { start_demo_play(access, level, command, output) };
    }

    Ok(false)
}

unsafe fn start_demo_record(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    mut command: *const u32,
    output: &mut ExecOutput<'_>,
) -> RuntimeResult<bool> {
    use hp2_abi::engine::generated::{FURL, TArrayRaw, UDemoRecDriver};
    use std::{mem::MaybeUninit, ptr};

    let capacity = unsafe { token_capacity(command)? };
    let mut map = vec![0u32; capacity];
    if !unsafe {
        hp2_core::tokenize::parse_token((&mut command as *mut *const u32).cast(), map.as_mut_ptr().cast(), capacity as i32, false)
    } {
        output(hp2_core::lit!("You must specify a filename"))?;
        return Ok(true);
    }

    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut url = MaybeUninit::<FURL>::uninit();
    unsafe { crate::url_owner::UrlOwner::construct_default(access, &defaults, url.as_mut_ptr())?; }
    let url = unsafe { url.assume_init_mut() };
    let mut map_len = map.iter().position(|unit| *unit == 0).unwrap_or(map.len());
    if map_len < 4
        || unsafe { hp2_core::ansi::strnicmp(map.as_ptr().add(map_len - 4).cast(), hp2_core::lit!(".dem").as_ptr().cast(), 4) } != 0
    {
        map.resize(map_len + 5, 0);
        map[map_len] = u32::from(b'.');
        map[map_len + 1] = u32::from(b'd');
        map[map_len + 2] = u32::from(b'e');
        map[map_len + 3] = u32::from(b'm');
        map[map_len + 4] = 0;
        map_len += 4;
    }
    let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let mut driver = ptr::null_mut();
    let result = (|| {
        unsafe { access.assign_fstring(ptr::addr_of_mut!((*url).map), &map[..=map_len])?; }
        let mut attempt = hp2_core::lit!("Attempting to record demo ").to_vec();
        attempt.extend_from_slice(&map[..map_len]);
        let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
        access.emit_log(category, &attempt)?;
        let base = access.native_class("Engine.NetDriver")?;
        let class_name = hp2_core::lit!("ini:Engine.Engine.DemoRecordingDevice");
        let class = unsafe { access.static_load_class(base, ptr::null_mut(), class_name, 1)? };
        driver = unsafe { access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())? }
            .cast::<UDemoRecDriver>();
        unsafe { (*level).base.demo_rec_driver = driver.cast(); }
        let started = unsafe {
            crate::demo_owner::init_listen(
                access, driver, level.cast(), url, ptr::addr_of_mut!(error),
            )?
        };
        if !started {
            let error = unsafe { fstring_units(ptr::addr_of!(error))? };
            let mut message = hp2_core::lit!("Demo recording failed: ").to_vec();
            message.extend_from_slice(&error);
            output(&message)?;
            unsafe {
                access.delete_object(driver.cast())?;
                (*level).base.demo_rec_driver = ptr::null_mut();
            }
            driver = ptr::null_mut();
        } else {
            let map = unsafe { fstring_units(ptr::addr_of!((*url).map))? };
            let mut message = hp2_core::lit!("Demo recording started to ").to_vec();
            message.extend_from_slice(&map);
            output(&message)?;
        }
        Ok(())
    })();
    if result.is_err() && !driver.is_null() {
        unsafe {
            let _ = access.delete_object(driver.cast());
            (*level).base.demo_rec_driver = ptr::null_mut();
        }
    }
    let cleanup_error = unsafe { crate::url_owner::UrlOwner::destroy(access, url) };
    let error_cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    result?;
    cleanup_error?;
    error_cleanup?;
    Ok(true)
}


unsafe fn start_demo_play(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    mut command: *const u32,
    output: &mut ExecOutput<'_>,
) -> RuntimeResult<bool> {
    use hp2_abi::engine::generated::{FURL, UDemoPlayPendingLevel, UGameEngine};
    use std::{mem::MaybeUninit, ptr};

    let capacity = unsafe { token_capacity(command)? };
    let mut token = vec![0u32; capacity];
    if !unsafe {
        hp2_core::tokenize::parse_token(
            (&mut command as *mut *const u32).cast(),
            token.as_mut_ptr().cast(),
            capacity as i32,
            false,
        )
    } {
        output(hp2_core::lit!("You must specify a filename"))?;
        return Ok(true);
    }

    let mut token_len = token.iter().position(|unit| *unit == 0).unwrap_or(token.len());
    let mut remove_at = None;
    for index in 0..token_len.saturating_sub(3) {
        if unsafe { hp2_core::ansi::strnicmp(token.as_ptr().add(index).cast(), hp2_core::lit!(".DEM").as_ptr().cast(), 4) } == 0 {
            remove_at = Some(index);
            break;
        }
    }
    if let Some(index) = remove_at {
        token.copy_within(index + 4..token_len, index);
        token_len -= 4;
    }
    let mut url_text = Vec::with_capacity(token_len + 5);
    url_text.extend_from_slice(&token[..token_len]);
    url_text.extend(hp2_core::lit!(".dem").iter().copied());

    let engine = unsafe { (*level).base.engine.cast::<UGameEngine>() };
    if engine.is_null() { return Err(RuntimeError::new("Canonical Level has no GameEngine")); }
    if !unsafe { (*engine).g_pending_level }.is_null() {
        unsafe { crate::network_owner::cancel_game_pending(access, engine)?; }
    }

    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut url = MaybeUninit::<FURL>::uninit();
    unsafe { crate::url_owner::UrlOwner::construct_default(access, &defaults, url.as_mut_ptr())?; }
    let url = unsafe { url.assume_init_mut() };
    let result = (|| -> RuntimeResult<bool> {
    unsafe {
        crate::url_owner::UrlOwner::parse_update(
            access, &defaults, url, ptr::null_mut(), url_text.as_ptr(), 0,
        )?;
    }
    let map = unsafe { fstring_units(ptr::addr_of!((*url).map))? };
    let mut attempt = hp2_core::lit!("Attempting to play demo ").to_vec();
    attempt.extend_from_slice(&map);
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
    access.emit_log(category, &attempt)?;

    let class = access.native_class("Engine.DemoPlayPendingLevel")?;
    let pending = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?
    }.cast::<UDemoPlayPendingLevel>();
    let pending_base = unsafe { ptr::addr_of_mut!((*pending).base) };
    let pending_url = unsafe { ptr::addr_of_mut!((*pending_base).url) };
        if let Err(error) = unsafe { crate::url_owner::UrlOwner::assign_copy(access, pending_url, url) } {
            let _ = unsafe { access.delete_object(pending.cast()) };
            return Err(error);
        }
    unsafe {
        (*pending_base).engine = engine.cast();
        (*pending_base).success = 0;
    }

    unsafe {
        crate::demo_pending::constructor_with_ops(
            &mut CoreDemoPendingConstructor(access),
            pending.cast(),
            pending_url.cast(),
        )?;
    }
    if unsafe { (*pending_base).demo_rec_driver.is_null() } {
        let error = unsafe { fstring_units(ptr::addr_of!((*pending_base).error))? };
        let mut message = hp2_core::lit!("Demo playback failed: ").to_vec();
        message.extend_from_slice(&error);
        output(&message)?;
        unsafe { access.delete_object(pending.cast())?; }
    } else {
        unsafe { (*engine).g_pending_level = pending.cast(); }
    }
        Ok(true)
    })();
    let cleanup = unsafe { crate::url_owner::UrlOwner::destroy(access, url) };
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

struct CoreDemoPendingConstructor<'a>(CoreAccess<'a>);

impl crate::demo_pending::PendingConstructorOps for CoreDemoPendingConstructor<'_> {
    type Error = RuntimeError;

    unsafe fn load_recording_driver_class(&mut self) -> RuntimeResult<*mut std::ffi::c_void> {
        let base = self.0.native_class("Engine.NetDriver")?;
        let name = hp2_core::lit!("ini:Engine.Engine.DemoRecordingDevice");
        unsafe { self.0.static_load_class(base, std::ptr::null_mut(), name, 1).map(|class| class.cast()) }
    }

    unsafe fn construct_recording_driver(&mut self, class: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        unsafe { self.0.construct_object(class.cast(), self.0.transient_package(), 0, 0, std::ptr::null_mut()).map(|object| object.cast()) }
    }

    unsafe fn init_connect(
        &mut self,
        driver: *mut std::ffi::c_void,
        pending: *mut std::ffi::c_void,
        url: *mut std::ffi::c_void,
        error: *mut std::ffi::c_void,
    ) -> RuntimeResult<bool> {
        unsafe {
            crate::demo_owner::init_connect(
                self.0,
                driver.cast(),
                pending,
                url.cast(),
                error.cast(),
            )
        }
    }

    unsafe fn delete_driver(&mut self, driver: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { self.0.delete_object(driver.cast()) }
    }
}

unsafe fn token_capacity(command: *const u32) -> RuntimeResult<usize> {
    if command.is_null() { return Err(RuntimeError::new("Null command text")); }
    let mut length = 0usize;
    while unsafe { *command.add(length) } != 0 {
        length = length.checked_add(1)
            .ok_or_else(|| RuntimeError::new("Command length overflow"))?;
    }
    let capacity = length.checked_add(1)
        .ok_or_else(|| RuntimeError::new("Command buffer size overflow"))?;
    i32::try_from(capacity)
        .map_err(|_| RuntimeError::new("Command exceeds the TCHAR parser capacity"))?;
    Ok(capacity)
}

unsafe fn fstring_units(string: *const hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<Vec<u32>> {
    if string.is_null() { return Err(RuntimeError::new("Null canonical FString")); }
    let string = unsafe { &*string };
    if string.num < 0 || (string.num > 0 && string.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical FString storage"));
    }
    if string.num == 0 { return Ok(Vec::new()); }
    let units = unsafe { std::slice::from_raw_parts(string.data.cast::<u32>(), string.num as usize) };
    let units = if units.last() == Some(&0) { &units[..units.len() - 1] } else { units };
    Ok(units.to_vec())
}
