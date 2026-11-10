//! Typed Core operations for the shared `UGameEngine::SaveGame` policy.

use crate::game_save::SaveGameOps;

use hp2_abi::engine::generated::{AMover, ALevelInfo, TArrayRaw, UGameEngine, ULevel, UObject, UProperty};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{cell::RefCell, ffi::c_void, fs, path::{Path, PathBuf}, ptr};

struct CoreSaveGameOps<'a> {
    access: CoreAccess<'a>,
    slow_task: RefCell<Option<hp2_core::runtime_globals::SlowTaskScope>>,
    /// A forked save child must not touch the GPU or the shared UI task state, so it skips
    /// the "Saving" progress screen and the slow-task scope.
    quiet: bool,
}

impl<'a> CoreSaveGameOps<'a> {
    fn new(access: CoreAccess<'a>) -> Self {
        Self { access, slow_task: RefCell::new(None), quiet: false }
    }

    fn quiet(access: CoreAccess<'a>) -> Self {
        Self { access, slow_task: RefCell::new(None), quiet: true }
    }


    fn level(&self, engine: *mut c_void) -> RuntimeResult<*mut ULevel> {
        if engine.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine")); }
        let level = unsafe { (*engine.cast::<UGameEngine>()).g_level.cast::<ULevel>() };
        if level.is_null() { return Err(RuntimeError::new("SaveGame requires a canonical ULevel")); }
        Ok(level)
    }

    fn level_info(&self, engine: *mut c_void) -> RuntimeResult<*mut ALevelInfo> {
        let level = self.level(engine)?;
        let info = unsafe { crate::package_map_level_owner::level_info(level)? };
        if info.is_null() { return Err(RuntimeError::new("SaveGame level has no canonical ALevelInfo")); }
        Ok(info)
    }

    fn field(&self, object: *mut UObject, field: &str) -> RuntimeResult<*mut u8> {
        if object.is_null() { return Err(RuntimeError::new("Null canonical object for SaveGame field")); }
        let name = self.access.intern_str(field, FindName::Find)?;
        let property_class = self.access.native_class("Property")?;
        let class = unsafe { ptr::addr_of!((*object).class).read_unaligned().cast() };
        let property = unsafe { self.access.find_field(class, name, property_class) }.cast::<UProperty>();
        if property.is_null() {
            return Err(RuntimeError::new("SaveGame reflected field is missing").with_context(field));
        }
        let offset = unsafe { ptr::addr_of!((*property).offset).read_unaligned() };
        let offset = usize::try_from(offset).map_err(|_| RuntimeError::new("Invalid SaveGame reflected field offset").with_context(field))?;
        Ok(unsafe { object.cast::<u8>().add(offset) })
    }

    fn slot_path(&self) -> RuntimeResult<PathBuf> {
        crate::game_init_owner::system_path(self.access, true)
    }

    fn output_path(&self, out: *mut u32, filename: &str) -> RuntimeResult<()> {
        let path = self.slot_path()?.join(filename);
        // SaveGame's source-local TCHAR filename buffers are 256 units.
        unsafe { write_wide_path(out, &path) }
    }

    fn create_save_directory(&self, slot: bool) -> RuntimeResult<()> {
        let path = crate::game_init_owner::system_path(self.access, slot)?;
        // GFileManager::MakeDirectory(path, 0) reports failure as a boolean; the
        // source ignores that result and lets the subsequent save operation fail.
        let _ = fs::create_dir(path);
        Ok(())
    }

    fn log(&self, message: &str) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        let units = message.chars().map(u32::from).collect::<Vec<_>>();
        self.access.emit_log(category, &units)
    }

}


unsafe fn write_wide_path(out: *mut u32, path: &Path) -> RuntimeResult<()> {
    if out.is_null() { return Err(RuntimeError::new("Null SaveGame filename buffer")); }
    let text = path.to_string_lossy();
    let mut length = 0;
    for character in text.chars() {
        if length + 1 >= crate::game_save::PATH_CAPACITY {
            return Err(RuntimeError::new("SaveGame filename exceeds TCHAR buffer capacity"));
        }
        unsafe { out.add(length).write_unaligned(u32::from(character)); }
        length += 1;
    }
    unsafe { out.add(length).write_unaligned(0); }
    Ok(())
}

pub(crate) unsafe fn path_from_wide(text: *const u32) -> RuntimeResult<PathBuf> {
    if text.is_null() { return Err(RuntimeError::new("Null SaveGame path")); }
    let mut path = String::new();
    for index in 0..crate::game_save::PATH_CAPACITY {
        let unit = unsafe { text.add(index).read_unaligned() };
        if unit == 0 { return Ok(PathBuf::from(path)); }
        path.push(char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid UTF-32 SaveGame path"))?);
    }
    Err(RuntimeError::new("Unterminated SaveGame path"))
}

impl crate::game_save::SaveGameOps for CoreSaveGameOps<'_> {
    type Error = RuntimeError;

    unsafe fn make_dir_save_path(&self) -> RuntimeResult<()> { self.create_save_directory(false) }
    unsafe fn make_dir_save_slot_path(&self) -> RuntimeResult<()> { self.create_save_directory(true) }

    unsafe fn format_slot_file(&self, out: *mut u32, position: i32) -> RuntimeResult<()> {
        self.output_path(out, &format!("Save{position}.usa"))
    }

    unsafe fn debugf_saving(&self, filename: *const u32) -> RuntimeResult<()> {
        let path = unsafe { path_from_wide(filename)? };
        self.log(&format!("Saving game... filename: {}", path.display()))
    }

    unsafe fn level_info_set_action(&self, engine: *mut c_void, action: i32) -> RuntimeResult<()> {
        let info = self.level_info(engine)?;
        unsafe { ptr::addr_of_mut!((*info).level_action).write_unaligned(action as u8); }
        Ok(())
    }

    unsafe fn paint_progress(&self, engine: *mut c_void) -> RuntimeResult<()> {
        if self.quiet { return Ok(()); }
        unsafe { crate::game_engine_owner::paint_progress(self.access, engine.cast()) }
    }

    unsafe fn begin_slow_task_saving(&self) -> RuntimeResult<()> {
        if self.quiet { return Ok(()); }
        let mut task = self.slow_task.try_borrow_mut()
            .map_err(|_| RuntimeError::new("SaveGame slow-task state reentered"))?;
        if task.is_some() {
            return Err(RuntimeError::new("SaveGame slow task is already active"));
        }
        // SAFETY: canonical Exec and package operations run on the application thread.
        *task = Some(unsafe { hp2_core::runtime_globals::slow_task_scope() });
        Ok(())
    }

    unsafe fn level_has_brush_tracker(&self, engine: *mut c_void) -> RuntimeResult<i32> {
        let level = self.level(engine)?;
        Ok(i32::from(!unsafe { ptr::addr_of!((*level).brush_tracker).read_unaligned() }.is_null()))
    }

    unsafe fn level_delete_brush_tracker(&self, engine: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::dynbsp_owner::destroy(self.access, self.level(engine)?) }
    }

    unsafe fn level_cleanup_destroyed(&self, engine: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::level_lifecycle_owner::cleanup_destroyed(self.access, self.level(engine)?) }
    }

    unsafe fn level_save_package(&self, engine: *mut c_void, filename: *const u32) -> RuntimeResult<i32> {
        let level = self.level(engine)?;
        let path = unsafe { path_from_wide(filename)? };
        let outer = unsafe { ptr::addr_of!((*level).base.base.outer).read_unaligned().cast::<UObject>() };
        let saved = unsafe { self.access.save_package(outer, level.cast(), 0, &path, None)? };
        Ok(i32::from(saved))
    }

    unsafe fn level_info_hub_stack_level(&self, engine: *mut c_void) -> RuntimeResult<i32> {
        let info = self.level_info(engine)?;
        let field = self.field(info.cast(), "HubStackLevel")?;
        Ok(unsafe { field.cast::<i32>().read_unaligned() })
    }

    unsafe fn format_game_file(&self, out: *mut u32, index: i32) -> RuntimeResult<()> {
        self.output_path(out, &format!("Game{index}.usa"))
    }

    unsafe fn format_save_hub_file(&self, out: *mut u32, position: i32, index: i32) -> RuntimeResult<()> {
        self.output_path(out, &format!("Save{position}{index}.usa"))
    }

    unsafe fn file_copy(&self, first: *const u32, second: *const u32) -> RuntimeResult<()> {
        let first = unsafe { path_from_wide(first)? };
        let second = unsafe { path_from_wide(second)? };
        // The native callback passes (Src, Dest) directly to GFileManager::Copy,
        // whose API is (Dest, Src); preserve that source argument ordering.
        let _ = fs::copy(second, first);
        Ok(())
    }

    unsafe fn file_size(&self, filename: *const u32) -> RuntimeResult<i32> {
        let path = unsafe { path_from_wide(filename)? };
        let size = fs::metadata(path).map(|metadata| i32::try_from(metadata.len()).unwrap_or(-1)).unwrap_or(-1);
        Ok(size)
    }

    unsafe fn file_delete(&self, filename: *const u32) -> RuntimeResult<()> {
        let path = unsafe { path_from_wide(filename)? };
        let _ = fs::remove_file(path);
        Ok(())
    }

    unsafe fn level_actor_count(&self, engine: *mut c_void) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(self.level(engine)?) }
    }

    unsafe fn level_actor(&self, engine: *mut c_void, index: i32) -> RuntimeResult<*mut c_void> {
        let level = self.level(engine)?;
        let count = unsafe { crate::level_owner::actor_count(level)? };
        if index < 0 || index >= count { return Err(RuntimeError::new("SaveGame actor index is out of range")); }
        Ok(unsafe { crate::level_owner::actor_at(level, index) }.cast())
    }

    unsafe fn actor_is_mover(&self, actor: *mut c_void) -> RuntimeResult<i32> {
        if actor.is_null() { return Ok(0); }
        let class = self.access.native_class("Engine.Mover")?;
        Ok(i32::from(unsafe { hp2_core::object_model::is_a(actor, class.cast()) }))
    }

    unsafe fn mover_reset_saved_pos(&self, actor: *mut c_void) -> RuntimeResult<()> {
        if actor.is_null() { return Err(RuntimeError::new("Null mover in SaveGame")); }
        unsafe {
            ptr::addr_of_mut!((*actor.cast::<AMover>()).saved_pos).write_unaligned(FVector { x: -1.0, y: -1.0, z: -1.0 });
        }
        Ok(())
    }

    unsafe fn level_new_brush_tracker(&self, engine: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::dynbsp_owner::initialize(self.access, self.level(engine)?) }
    }

    unsafe fn end_slow_task(&self) -> RuntimeResult<()> {
        if self.quiet { return Ok(()); }
        let mut task = self.slow_task.try_borrow_mut()
            .map_err(|_| RuntimeError::new("SaveGame slow-task state reentered"))?;
        let scope = task.take()
            .ok_or_else(|| RuntimeError::new("SaveGame slow task is not active"))?;
        drop(scope);
        Ok(())
    }

    unsafe fn gcache_flush(&self) -> RuntimeResult<()> {
        crate::render_owner::flush_cache(self.access)
    }

    unsafe fn save_persistent_actor_cache(&self) -> RuntimeResult<()> {
        hp2_core::native_runtime_init::save_persistent_cache(self.access)
    }
}

/// Creates the two source SaveGame directories, ignoring ordinary filesystem
/// failure just as `GFileManager::MakeDirectory` returns an unchecked boolean.
pub(crate) fn make_save_directories(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let ops = CoreSaveGameOps::new(access);
    ops.create_save_directory(false)?;
    ops.create_save_directory(true)
}

/// Returns `SaveSlotPath/Save<position>.usa` in NUL-terminated UTF-32.
pub(crate) fn slot_filename(access: CoreAccess<'_>, position: i32) -> RuntimeResult<Vec<u32>> {
    let ops = CoreSaveGameOps::new(access);
    let mut buffer = [0u32; crate::game_save::PATH_CAPACITY];
    unsafe { ops.format_slot_file(buffer.as_mut_ptr(), position)?; }
    let length = buffer.iter().position(|unit| *unit == 0)
        .ok_or_else(|| RuntimeError::new("Unterminated SaveGame filename"))?;
    Ok(buffer[..=length].to_vec())
}

/// Parses an Exec slot with the Core `wcstol` implementation used by `appAtoi`.
///
/// # Safety
/// `text` is NUL-terminated canonical UTF-32 command-tail storage.
pub(crate) unsafe fn app_atoi(text: *const u32) -> RuntimeResult<i32> {
    if text.is_null() { return Err(RuntimeError::new("Null appAtoi input")); }
    unsafe { hp2_core::ansi::atoi_checked(text.cast()) }
        .map_err(|value| RuntimeError::new("Save slot exceeds INT range").with_context(&value.to_string()))
}

/// Returns the canonical `ALevelInfo::LevelEnterText` FString for persistent actor saving.
///
/// # Safety
/// `info` is a live canonical LevelInfo record with initialized reflected properties.
pub(crate) unsafe fn level_enter_text(access: CoreAccess<'_>, info: *mut ALevelInfo) -> RuntimeResult<*const u32> {
    static EMPTY: [u32; 1] = [0];
    let ops = CoreSaveGameOps::new(access);
    let field = ops.field(info.cast(), "LevelEnterText")?;
    let text = unsafe { field.cast::<TArrayRaw>().read_unaligned() };
    if text.num < 0 || text.max < text.num || (text.num > 0 && text.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical LevelEnterText FString"));
    }
    if text.num == 0 {
        return Ok(EMPTY.as_ptr());
    }
    let units = unsafe { std::slice::from_raw_parts(text.data.cast::<u32>(), text.num as usize) };
    if units.last() != Some(&0) {
        return Err(RuntimeError::new("Canonical LevelEnterText FString is not NUL-terminated"));
    }
    Ok(text.data.cast())
}

/// Converts a checked UTF-32 filename to the human-readable form used by Exec logs.
pub(crate) fn filename_text(filename: &[u32]) -> RuntimeResult<String> {
    let mut text = String::with_capacity(filename.len().saturating_sub(1));
    for &unit in filename.iter().take_while(|&&unit| unit != 0) {
        text.push(char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid UTF-32 SaveGame filename"))?);
    }
    Ok(text)
}

/// Resolves a script file beneath Engine's configured SaveSlotPath using the
/// same System-relative base as the canonical save-game path operations.
pub(crate) fn save_slot_file_path(access: CoreAccess<'_>, leaf: &Path) -> RuntimeResult<PathBuf> {
    let system = access.system()?;
    let raw = unsafe { ptr::addr_of!((*system).save_slot_path).read_unaligned() };
    if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical SaveSlotPath FString"));
    }
    let mut path = if raw.num == 0 { PathBuf::new() } else { unsafe { path_from_wide(raw.data.cast())? } };
    // FString's path operator appends the right operand verbatim; unlike
    // PathBuf::push, an absolute-looking leaf never replaces a nonempty base.
    if !path.as_os_str().is_empty() && !path.as_os_str().as_encoded_bytes().ends_with(b"/") {
        path.as_mut_os_string().push("/");
    }
    path.as_mut_os_string().push(leaf.as_os_str());
    Ok(if path.is_absolute() { path } else { access.data_root().join("System").join(path) })
}

/// Runs the complete shared save policy with canonical Core-owned operations.
///
/// # Safety
/// `engine` is a live canonical `UGameEngine` record.
pub(crate) unsafe fn save_game(access: CoreAccess<'_>, engine: *mut UGameEngine, position: i32) -> RuntimeResult<()> {
    if engine.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine")); }
    #[cfg(unix)]
    if std::env::var_os("HP2_SYNC_SAVE").is_none() {
        return unsafe { forked::save_game(access, engine, position) };
    }
    unsafe { save_game_sync(access, engine, position) }
}

/// The original blocking save: serialize the level, write the slot, return.
///
/// # Safety
/// `engine` is a live canonical `UGameEngine` record.
unsafe fn save_game_sync(access: CoreAccess<'_>, engine: *mut UGameEngine, position: i32) -> RuntimeResult<()> {
    let ops = CoreSaveGameOps::new(access);
    unsafe { crate::game_save::save_game_with(&ops, engine.cast(), position) }
}

/// Waits for a background save (see `forked`) before something reads or replaces its files.
pub(crate) fn finish_pending_save(access: CoreAccess<'_>) -> RuntimeResult<()> {
    #[cfg(unix)]
    return forked::finish(access, true);
    #[cfg(not(unix))]
    { let _ = access; Ok(()) }
}

/// Non-blocking check for a finished background save; call once per tick.
pub(crate) fn poll_pending_save(access: CoreAccess<'_>) -> RuntimeResult<()> {
    #[cfg(unix)]
    return forked::finish(access, false);
    #[cfg(not(unix))]
    { let _ = access; Ok(()) }
}

/// Background save by `fork`: the child is a copy-on-write snapshot of the whole process, so it
/// can run the unchanged blocking save on a frozen world while the game keeps running.
///
/// The child runs the exact original `save_game_with` sequence (so the files are byte-identical
/// to a blocking save) with the progress screen and slow-task scope disabled, logging detached,
/// crash handlers reset and no GPU access, then `_exit`s. The parent applies only the state
/// changes the original made to the live game (brush tracker rebuild, dead-reference cleanup,
/// mover saved positions, cache flush) and returns at once. Anything that reads or replaces the
/// slot files first waits for the child: the next save, map load, and shutdown. A child that
/// fails or hangs (30 s) is replaced by a blocking save of the then-current world.
#[cfg(unix)]
mod forked {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    #[derive(Clone, Copy)]
    struct Pending { pid: libc::pid_t, started: Instant, engine: usize, position: i32 }

    static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
    /// Pid of the running child, 0 when none: lets the per-tick poll skip the mutex.
    static PENDING_PID: AtomicI32 = AtomicI32::new(0);
    const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

    pub(super) unsafe fn save_game(access: CoreAccess<'_>, engine: *mut UGameEngine, position: i32) -> RuntimeResult<()> {
        finish(access, true)?;
        let ops = CoreSaveGameOps::new(access);
        // SAFETY: the original save also formats and logs the slot name first.
        unsafe {
            ops.make_dir_save_path()?;
            ops.make_dir_save_slot_path()?;
            let mut filename = [0u32; crate::game_save::PATH_CAPACITY];
            ops.format_slot_file(filename.as_mut_ptr(), position)?;
            ops.debugf_saving(filename.as_ptr())?;
        }
        // SAFETY: fork has no preconditions here; the child only runs `child` below.
        match unsafe { libc::fork() } {
            -1 => unsafe { save_game_sync(access, engine, position) },
            0 => unsafe { child(access, engine, position) },
            pid => {
                *PENDING.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(Pending { pid, started: Instant::now(), engine: engine as usize, position });
                PENDING_PID.store(pid, Ordering::Release);
                unsafe { parent_state_updates(&ops, engine) }
            }
        }
    }

    unsafe fn child(access: CoreAccess<'_>, engine: *mut UGameEngine, position: i32) -> ! {
        // Default crash behavior (the parent's reporter writes into the shared working directory),
        // and no inherited buffered log bytes may be flushed from this copy.
        for signal in [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGABRT, libc::SIGFPE, libc::SIGTRAP, libc::SIGTERM] {
            unsafe { libc::signal(signal, libc::SIG_DFL) };
        }
        // Nothing the child prints may interleave with the parent's console output.
        unsafe {
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
            if null >= 0 {
                libc::dup2(null, 1);
                libc::dup2(null, 2);
            }
        }
        access.detach_logging_after_fork();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let ops = CoreSaveGameOps::quiet(access);
            unsafe { crate::game_save::save_game_with(&ops, engine.cast(), position) }
        }));
        let code = match result { Ok(Ok(())) => 0, Ok(Err(_)) => 3, Err(_) => 4 };
        unsafe { libc::_exit(code) }
    }

    /// What the blocking save leaves behind in the live game, minus the serialization itself, in
    /// the original order.
    unsafe fn parent_state_updates(ops: &CoreSaveGameOps<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
        let engine = engine.cast::<c_void>();
        unsafe {
            if ops.level_has_brush_tracker(engine)? != 0 {
                ops.level_delete_brush_tracker(engine)?;
            }
            ops.level_cleanup_destroyed(engine)?;
            let mut i = 0;
            while i < ops.level_actor_count(engine)? {
                let actor = ops.level_actor(engine, i)?;
                if ops.actor_is_mover(actor)? != 0 {
                    ops.mover_reset_saved_pos(actor)?;
                }
                i += 1;
            }
            ops.level_new_brush_tracker(engine)?;
            ops.level_info_set_action(engine, crate::game_save::LEVACT_NONE)?;
            ops.gcache_flush()
        }
    }

    /// Reaps the background child. With `block`, waits for it (30 s at most); otherwise only
    /// checks. A failed child triggers the blocking save the child was meant to do.
    pub(super) fn finish(access: CoreAccess<'_>, block: bool) -> RuntimeResult<()> {
        if PENDING_PID.load(Ordering::Acquire) == 0 {
            return Ok(());
        }
        let Some(pending) = *PENDING.lock().unwrap_or_else(|e| e.into_inner()) else { return Ok(()) };
        let mut status = 0;
        let ok = loop {
            // SAFETY: plain waitpid on our own child.
            let reaped = unsafe { libc::waitpid(pending.pid, &mut status, libc::WNOHANG) };
            if reaped == pending.pid {
                break libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;
            }
            if reaped < 0 {
                if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) { continue; }
                break false; // child vanished (for example reaped elsewhere): do not trust the file
            }
            if pending.started.elapsed() > CHILD_TIMEOUT {
                unsafe { libc::kill(pending.pid, libc::SIGKILL); libc::waitpid(pending.pid, &mut status, 0); }
                break false;
            }
            if !block {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = None;
        PENDING_PID.store(0, Ordering::Release);
        if ok {
            return Ok(());
        }
        let ops = CoreSaveGameOps::new(access);
        let _ = ops.log(&format!("Warning: background save of slot {} failed (status {status}); saving again in the foreground", pending.position));
        // SAFETY: the engine pointer was live when the save started and the owner outlives it.
        unsafe { save_game_sync(access, pending.engine as *mut UGameEngine, pending.position) }
    }
}
