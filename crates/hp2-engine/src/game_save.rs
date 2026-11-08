//! Checked save, travel export, server-map and command-line load-slot policy.
//! Native exceptions are captured by each service; the first failure returns
//! without executing cleanup absent from the original. The property iterator
//! lives in its original typed native stack scope, never on the heap.
use std::ffi::{c_char, c_void};

pub type Result<T = ()> = std::result::Result<T, i32>;
const INDEX_NONE: i32 = -1;
const MAXINT: i32 = i32::MAX;
const LEVACT_NONE: i32 = 0;
const LEVACT_SAVING: i32 = 2;
const QUOTE: u32 = b'"' as u32;
// Slot, hub-copy and queued-save filenames share the platform filesystem path limit.
pub(crate) const PATH_CAPACITY: usize = libc::PATH_MAX as usize;
const VALUE_CAPACITY: usize = 1024;
const LOAD_URL_MIN_CAPACITY: i32 = 18;

/// Immutable callbacks; all engine objects and native stack state remain raw.
/// Every callback returns zero on success and leaves native failures pending.
#[repr(C)]
pub struct Hp2GameSaveServices {
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    pub paint_progress: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    pub make_dir_save_path: unsafe extern "C" fn() -> i32,
    pub make_dir_save_slot_path: unsafe extern "C" fn() -> i32,
    pub format_slot_file: unsafe extern "C" fn(out: *mut u32, position: i32) -> i32,
    pub format_game_file: unsafe extern "C" fn(out: *mut u32, index: i32) -> i32,
    pub format_save_hub_file: unsafe extern "C" fn(out: *mut u32, position: i32, index: i32) -> i32,
    pub debugf_saving: unsafe extern "C" fn(filename: *const u32) -> i32,
    pub level_info_set_action: unsafe extern "C" fn(engine: *mut c_void, action: i32) -> i32,
    pub level_info_hub_stack_level:
        unsafe extern "C" fn(engine: *mut c_void, result: *mut i32) -> i32,
    pub begin_slow_task_saving: unsafe extern "C" fn() -> i32,
    pub end_slow_task: unsafe extern "C" fn() -> i32,
    pub level_has_brush_tracker: unsafe extern "C" fn(engine: *mut c_void, result: *mut i32) -> i32,
    pub level_delete_brush_tracker: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    pub level_cleanup_destroyed: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    pub level_save_package:
        unsafe extern "C" fn(engine: *mut c_void, filename: *const u32, result: *mut i32) -> i32,
    pub file_copy: unsafe extern "C" fn(first: *const u32, second: *const u32) -> i32,
    pub file_size: unsafe extern "C" fn(filename: *const u32, result: *mut i32) -> i32,
    pub file_delete: unsafe extern "C" fn(filename: *const u32) -> i32,
    pub level_actor_count: unsafe extern "C" fn(engine: *mut c_void, result: *mut i32) -> i32,
    pub level_actor:
        unsafe extern "C" fn(engine: *mut c_void, index: i32, result: *mut *mut c_void) -> i32,
    pub actor_is_mover: unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub mover_reset_saved_pos: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub level_new_brush_tracker: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    pub gcache_flush: unsafe extern "C" fn() -> i32,
    pub save_persistent_actor_cache: unsafe extern "C" fn() -> i32,
    pub debugf_export_class: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_b_travel: unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub out_logf_header: unsafe extern "C" fn(out: *mut c_void, actor: *mut c_void) -> i32,
    pub prop_iter_valid: unsafe extern "C" fn(it: *mut c_void, result: *mut i32) -> i32,
    pub prop_iter_next: unsafe extern "C" fn(it: *mut c_void) -> i32,
    pub prop_array_dim: unsafe extern "C" fn(it: *mut c_void, result: *mut i32) -> i32,
    pub prop_travel_flag: unsafe extern "C" fn(it: *mut c_void, result: *mut i32) -> i32,
    pub prop_export_text: unsafe extern "C" fn(
        it: *mut c_void,
        index: i32,
        value: *mut u32,
        actor: *mut c_void,
        result: *mut i32,
    ) -> i32,
    pub out_log_prop_name: unsafe extern "C" fn(out: *mut c_void, it: *mut c_void) -> i32,
    pub out_logf_index: unsafe extern "C" fn(out: *mut c_void, index: i32) -> i32,
    pub out_log_equals: unsafe extern "C" fn(out: *mut c_void) -> i32,
    pub prop_is_actor_ref: unsafe extern "C" fn(it: *mut c_void, result: *mut i32) -> i32,
    pub out_logf_object_ref: unsafe extern "C" fn(
        out: *mut c_void,
        it: *mut c_void,
        actor: *mut c_void,
        index: i32,
    ) -> i32,
    pub out_logf_value: unsafe extern "C" fn(out: *mut c_void, value: *const u32) -> i32,
    pub out_logf_footer: unsafe extern "C" fn(out: *mut c_void) -> i32,
    pub begin_load: unsafe extern "C" fn() -> i32,
    pub end_load: unsafe extern "C" fn() -> i32,
    pub level_linker: unsafe extern "C" fn(level: *mut c_void, result: *mut *mut c_void) -> i32,
    pub level_level_info: unsafe extern "C" fn(level: *mut c_void, result: *mut *mut c_void) -> i32,
    pub level_info_game: unsafe extern "C" fn(info: *mut c_void, result: *mut *mut c_void) -> i32,
    pub game_class_linker: unsafe extern "C" fn(game: *mut c_void, result: *mut *mut c_void) -> i32,
    pub master_map_add_linker:
        unsafe extern "C" fn(net_driver: *mut c_void, linker: *mut c_void, result: *mut i32) -> i32,
    pub master_map_compute: unsafe extern "C" fn(net_driver: *mut c_void) -> i32,
    pub server_packages_count: unsafe extern "C" fn(engine: *mut c_void, result: *mut i32) -> i32,
    pub server_package_debugf: unsafe extern "C" fn(engine: *mut c_void, index: i32) -> i32,
    pub server_package_linker:
        unsafe extern "C" fn(engine: *mut c_void, index: i32, result: *mut *mut c_void) -> i32,
    pub debugf_server_side_only: unsafe extern "C" fn() -> i32,
    pub strnicmp:
        unsafe extern "C" fn(a: *const u32, b: *const u32, count: i32, result: *mut i32) -> i32,
    pub with_properties: unsafe extern "C" fn(out: *mut c_void, actor: *mut c_void) -> i32,
}

impl Hp2GameSaveServices {
    unsafe fn check_failed(&self, expr: *const c_char, line: i32) -> Result<()> {
        let status = unsafe { (self.check_failed)(expr, line) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn paint_progress(&self, engine: *mut c_void) -> Result<()> {
        let status = unsafe { (self.paint_progress)(engine) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn make_dir_save_path(&self) -> Result<()> {
        let status = unsafe { (self.make_dir_save_path)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn make_dir_save_slot_path(&self) -> Result<()> {
        let status = unsafe { (self.make_dir_save_slot_path)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn format_slot_file(&self, out: *mut u32, position: i32) -> Result<()> {
        let status = unsafe { (self.format_slot_file)(out, position) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn format_game_file(&self, out: *mut u32, index: i32) -> Result<()> {
        let status = unsafe { (self.format_game_file)(out, index) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn format_save_hub_file(&self, out: *mut u32, position: i32, index: i32) -> Result<()> {
        let status = unsafe { (self.format_save_hub_file)(out, position, index) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn debugf_saving(&self, filename: *const u32) -> Result<()> {
        let status = unsafe { (self.debugf_saving)(filename) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_info_set_action(&self, engine: *mut c_void, action: i32) -> Result<()> {
        let status = unsafe { (self.level_info_set_action)(engine, action) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_info_hub_stack_level(&self, engine: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_info_hub_stack_level)(engine, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn begin_slow_task_saving(&self) -> Result<()> {
        let status = unsafe { (self.begin_slow_task_saving)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn end_slow_task(&self) -> Result<()> {
        let status = unsafe { (self.end_slow_task)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_has_brush_tracker(&self, engine: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_has_brush_tracker)(engine, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn level_delete_brush_tracker(&self, engine: *mut c_void) -> Result<()> {
        let status = unsafe { (self.level_delete_brush_tracker)(engine) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_cleanup_destroyed(&self, engine: *mut c_void) -> Result<()> {
        let status = unsafe { (self.level_cleanup_destroyed)(engine) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_save_package(&self, engine: *mut c_void, filename: *const u32) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_save_package)(engine, filename, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn file_copy(&self, first: *const u32, second: *const u32) -> Result<()> {
        let status = unsafe { (self.file_copy)(first, second) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn file_size(&self, filename: *const u32) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.file_size)(filename, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn file_delete(&self, filename: *const u32) -> Result<()> {
        let status = unsafe { (self.file_delete)(filename) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_actor_count(&self, engine: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_actor_count)(engine, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn level_actor(&self, engine: *mut c_void, index: i32) -> Result<*mut c_void> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_actor)(engine, index, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn actor_is_mover(&self, actor: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_is_mover)(actor, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn mover_reset_saved_pos(&self, actor: *mut c_void) -> Result<()> {
        let status = unsafe { (self.mover_reset_saved_pos)(actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_new_brush_tracker(&self, engine: *mut c_void) -> Result<()> {
        let status = unsafe { (self.level_new_brush_tracker)(engine) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn gcache_flush(&self) -> Result<()> {
        let status = unsafe { (self.gcache_flush)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn save_persistent_actor_cache(&self) -> Result<()> {
        let status = unsafe { (self.save_persistent_actor_cache)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn debugf_export_class(&self, actor: *mut c_void) -> Result<()> {
        let status = unsafe { (self.debugf_export_class)(actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn actor_b_travel(&self, actor: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_b_travel)(actor, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn out_logf_header(&self, out: *mut c_void, actor: *mut c_void) -> Result<()> {
        let status = unsafe { (self.out_logf_header)(out, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn prop_iter_valid(&self, it: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.prop_iter_valid)(it, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn prop_iter_next(&self, it: *mut c_void) -> Result<()> {
        let status = unsafe { (self.prop_iter_next)(it) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn prop_array_dim(&self, it: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.prop_array_dim)(it, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn prop_travel_flag(&self, it: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.prop_travel_flag)(it, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn prop_export_text(
        &self,
        it: *mut c_void,
        index: i32,
        value: *mut u32,
        actor: *mut c_void,
    ) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status =
            unsafe { (self.prop_export_text)(it, index, value, actor, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn out_log_prop_name(&self, out: *mut c_void, it: *mut c_void) -> Result<()> {
        let status = unsafe { (self.out_log_prop_name)(out, it) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn out_logf_index(&self, out: *mut c_void, index: i32) -> Result<()> {
        let status = unsafe { (self.out_logf_index)(out, index) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn out_log_equals(&self, out: *mut c_void) -> Result<()> {
        let status = unsafe { (self.out_log_equals)(out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn prop_is_actor_ref(&self, it: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.prop_is_actor_ref)(it, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn out_logf_object_ref(
        &self,
        out: *mut c_void,
        it: *mut c_void,
        actor: *mut c_void,
        index: i32,
    ) -> Result<()> {
        let status = unsafe { (self.out_logf_object_ref)(out, it, actor, index) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn out_logf_value(&self, out: *mut c_void, value: *const u32) -> Result<()> {
        let status = unsafe { (self.out_logf_value)(out, value) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn out_logf_footer(&self, out: *mut c_void) -> Result<()> {
        let status = unsafe { (self.out_logf_footer)(out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn begin_load(&self) -> Result<()> {
        let status = unsafe { (self.begin_load)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn end_load(&self) -> Result<()> {
        let status = unsafe { (self.end_load)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn level_linker(&self, level: *mut c_void) -> Result<*mut c_void> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_linker)(level, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn level_level_info(&self, level: *mut c_void) -> Result<*mut c_void> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_level_info)(level, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn level_info_game(&self, info: *mut c_void) -> Result<*mut c_void> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_info_game)(info, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn game_class_linker(&self, game: *mut c_void) -> Result<*mut c_void> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.game_class_linker)(game, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn master_map_add_linker(
        &self,
        net_driver: *mut c_void,
        linker: *mut c_void,
    ) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status =
            unsafe { (self.master_map_add_linker)(net_driver, linker, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn master_map_compute(&self, net_driver: *mut c_void) -> Result<()> {
        let status = unsafe { (self.master_map_compute)(net_driver) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn server_packages_count(&self, engine: *mut c_void) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.server_packages_count)(engine, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn server_package_debugf(&self, engine: *mut c_void, index: i32) -> Result<()> {
        let status = unsafe { (self.server_package_debugf)(engine, index) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn server_package_linker(&self, engine: *mut c_void, index: i32) -> Result<*mut c_void> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.server_package_linker)(engine, index, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
    unsafe fn debugf_server_side_only(&self) -> Result<()> {
        let status = unsafe { (self.debugf_server_side_only)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    unsafe fn strnicmp(&self, a: *const u32, b: *const u32, count: i32) -> Result<i32> {
        let mut result = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.strnicmp)(a, b, count, result.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { result.assume_init() })
    }
}

/// Builds a NUL-free `TCHAR` string constant (for prefix compares).
const fn wide<const N: usize>(text: &str) -> [u32; N] {
    let bytes = text.as_bytes();
    assert!(bytes.len() == N);
    let mut out = [0u32; N];
    let mut i = 0;
    while i < N {
        out[i] = bytes[i] as u32;
        i += 1;
    }
    out
}

const REPAIR_SAVE_PREFIX: &[u32] = &wide::<12>("-REPAIRSAVE=");
const LOAD_PREFIX: &[u32] = &wide::<6>("-LOAD=");

#[inline]
fn is_separator(c: u32) -> bool {
    matches!(c, 0x20 | 0x09 | 0x0D | 0x0A | 0x2C)
}

/// `ParseCommandLineSaveSlot` (`UnGame.cpp:110`). Keeps the original quirks:
/// `-LOAD="` yields slot 0, and a quote-only token is checked via `TokenEnd[-1]`.
///
/// # Safety
/// `cmd` must be a NUL-terminated `TCHAR` string, or null.
unsafe fn parse_command_line_save_slot(
    s: &Hp2GameSaveServices,
    cmd: *const u32,
    prefix: &[u32],
) -> Result<Option<i32>> {
    if cmd.is_null() {
        return Ok(None);
    }
    // SAFETY: every index below is at or before the NUL terminator; no loop
    // steps past it.
    let at = |i: usize| unsafe { *cmd.add(i) };
    let prefix_len = prefix.len();
    let mut p = 0usize;
    while at(p) != 0 {
        while is_separator(at(p)) {
            p += 1;
        }
        if at(p) == 0 {
            break;
        }
        let quoted = at(p) == QUOTE;
        let token = p + usize::from(quoted);
        let mut token_end = token;
        if quoted {
            while at(token_end) != 0 && at(token_end) != QUOTE {
                token_end += 1;
            }
            if at(token_end) == 0 {
                return Ok(None);
            }
            p = token_end + 1;
        } else {
            while at(token_end) != 0 && !is_separator(at(token_end)) {
                token_end += 1;
            }
            p = token_end;
        }

        // SAFETY: `token + prefix_len <= token_end` is checked first, so the
        // compare reads only within the token.
        if token_end - token < prefix_len
            || unsafe { s.strnicmp(cmd.add(token), prefix.as_ptr(), prefix_len as i32)? } != 0
        {
            continue;
        }

        let mut value = token + prefix_len;
        if value < token_end && at(value) == QUOTE {
            if at(token_end - 1) != QUOTE {
                return Ok(None);
            }
            value += 1;
            token_end -= 1;
        }
        if value == token_end {
            return Ok(None);
        }

        let mut parsed: i32 = 0;
        while value < token_end {
            let c = at(value);
            if !(0x30..=0x39).contains(&c) {
                return Ok(None);
            }
            value += 1;
            let digit = (c - 0x30) as i32;
            if parsed > (MAXINT - digit) / 10 {
                return Ok(None);
            }
            parsed = parsed * 10 + digit;
        }
        return Ok(Some(parsed));
    }
    Ok(None)
}

/// `appConsumeCommandLineLoadSlot` (`UnGame.cpp:178`). Returns the slot only on
/// the first successful call. `consumed` is the shell's `static UBOOL Consumed`,
/// and `repair_slot` is `GRepairSaveSlot`. Both stay C++ statics; this writes them.
///
/// # Safety
/// `cmd` must be a NUL-terminated `TCHAR` string, or null. `consumed` and
/// `repair_slot` must point to live `INT`/`UBOOL` objects.
pub unsafe fn consume_load_slot(
    s: &Hp2GameSaveServices,
    cmd: *const u32,
    consumed: *mut i32,
    repair_slot: *mut i32,
) -> Result<Option<i32>> {
    // SAFETY: caller contract.
    if unsafe { *consumed } != 0 {
        return Ok(None);
    }
    // SAFETY: caller contract.
    let repair = unsafe { parse_command_line_save_slot(s, cmd, REPAIR_SAVE_PREFIX)? };
    let parsed = match repair {
        Some(slot) => slot,
        // SAFETY: caller contract.
        None => match unsafe { parse_command_line_save_slot(s, cmd, LOAD_PREFIX)? } {
            Some(slot) => slot,
            None => return Ok(None),
        },
    };
    // SAFETY: caller contract.
    unsafe {
        *consumed = 1;
        *repair_slot = if repair.is_some() { parsed } else { INDEX_NONE };
    }
    Ok(Some(parsed))
}

/// `appFormatLoadGameURL` (`UnGame.cpp:188`). Writes `?load=%i` plus NUL.
///
/// # Safety
/// `out` must be valid for `capacity` `TCHAR`s, or null (then returns false).
pub unsafe fn format_load_game_url(slot: i32, out: *mut u32, capacity: i32) -> bool {
    if slot < 0 || out.is_null() || capacity < LOAD_URL_MIN_CAPACITY {
        return false;
    }
    let mut len = 0usize;
    // SAFETY: `capacity >= 18`, and the longest output is 6 + 10 + NUL = 17.
    let mut put = |c: u8| unsafe {
        *out.add(len) = u32::from(c);
        len += 1;
    };
    for &b in b"?load=" {
        put(b);
    }
    let mut digits = [0u8; 10];
    let mut n = 0usize;
    let mut v = slot as u32;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while n > 0 {
        n -= 1;
        put(digits[n]);
    }
    // SAFETY: the NUL lands at index <= 16, within capacity.
    unsafe { *out.add(len) = 0 };
    true
}

pub(crate) trait SaveGameOps {
    type Error;

    unsafe fn make_dir_save_path(&self) -> std::result::Result<(), Self::Error>;
    unsafe fn make_dir_save_slot_path(&self) -> std::result::Result<(), Self::Error>;
    unsafe fn format_slot_file(&self, out: *mut u32, position: i32) -> std::result::Result<(), Self::Error>;
    unsafe fn debugf_saving(&self, filename: *const u32) -> std::result::Result<(), Self::Error>;
    unsafe fn level_info_set_action(&self, engine: *mut c_void, action: i32) -> std::result::Result<(), Self::Error>;
    unsafe fn paint_progress(&self, engine: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn begin_slow_task_saving(&self) -> std::result::Result<(), Self::Error>;
    unsafe fn level_has_brush_tracker(&self, engine: *mut c_void) -> std::result::Result<i32, Self::Error>;
    unsafe fn level_delete_brush_tracker(&self, engine: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn level_cleanup_destroyed(&self, engine: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn level_save_package(&self, engine: *mut c_void, filename: *const u32) -> std::result::Result<i32, Self::Error>;
    unsafe fn level_info_hub_stack_level(&self, engine: *mut c_void) -> std::result::Result<i32, Self::Error>;
    unsafe fn format_game_file(&self, out: *mut u32, index: i32) -> std::result::Result<(), Self::Error>;
    unsafe fn format_save_hub_file(&self, out: *mut u32, position: i32, index: i32) -> std::result::Result<(), Self::Error>;
    unsafe fn file_copy(&self, first: *const u32, second: *const u32) -> std::result::Result<(), Self::Error>;
    unsafe fn file_size(&self, filename: *const u32) -> std::result::Result<i32, Self::Error>;
    unsafe fn file_delete(&self, filename: *const u32) -> std::result::Result<(), Self::Error>;
    unsafe fn level_actor_count(&self, engine: *mut c_void) -> std::result::Result<i32, Self::Error>;
    unsafe fn level_actor(&self, engine: *mut c_void, index: i32) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn actor_is_mover(&self, actor: *mut c_void) -> std::result::Result<i32, Self::Error>;
    unsafe fn mover_reset_saved_pos(&self, actor: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn level_new_brush_tracker(&self, engine: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn end_slow_task(&self) -> std::result::Result<(), Self::Error>;
    unsafe fn gcache_flush(&self) -> std::result::Result<(), Self::Error>;
    unsafe fn save_persistent_actor_cache(&self) -> std::result::Result<(), Self::Error>;
}

impl SaveGameOps for Hp2GameSaveServices {
    type Error = i32;

    unsafe fn make_dir_save_path(&self) -> Result<()> { unsafe { Hp2GameSaveServices::make_dir_save_path(self) } }
    unsafe fn make_dir_save_slot_path(&self) -> Result<()> { unsafe { Hp2GameSaveServices::make_dir_save_slot_path(self) } }
    unsafe fn format_slot_file(&self, out: *mut u32, position: i32) -> Result<()> { unsafe { Hp2GameSaveServices::format_slot_file(self, out, position) } }
    unsafe fn debugf_saving(&self, filename: *const u32) -> Result<()> { unsafe { Hp2GameSaveServices::debugf_saving(self, filename) } }
    unsafe fn level_info_set_action(&self, engine: *mut c_void, action: i32) -> Result<()> { unsafe { Hp2GameSaveServices::level_info_set_action(self, engine, action) } }
    unsafe fn paint_progress(&self, engine: *mut c_void) -> Result<()> { unsafe { Hp2GameSaveServices::paint_progress(self, engine) } }
    unsafe fn begin_slow_task_saving(&self) -> Result<()> { unsafe { Hp2GameSaveServices::begin_slow_task_saving(self) } }
    unsafe fn level_has_brush_tracker(&self, engine: *mut c_void) -> Result<i32> { unsafe { Hp2GameSaveServices::level_has_brush_tracker(self, engine) } }
    unsafe fn level_delete_brush_tracker(&self, engine: *mut c_void) -> Result<()> { unsafe { Hp2GameSaveServices::level_delete_brush_tracker(self, engine) } }
    unsafe fn level_cleanup_destroyed(&self, engine: *mut c_void) -> Result<()> { unsafe { Hp2GameSaveServices::level_cleanup_destroyed(self, engine) } }
    unsafe fn level_save_package(&self, engine: *mut c_void, filename: *const u32) -> Result<i32> { unsafe { Hp2GameSaveServices::level_save_package(self, engine, filename) } }
    unsafe fn level_info_hub_stack_level(&self, engine: *mut c_void) -> Result<i32> { unsafe { Hp2GameSaveServices::level_info_hub_stack_level(self, engine) } }
    unsafe fn format_game_file(&self, out: *mut u32, index: i32) -> Result<()> { unsafe { Hp2GameSaveServices::format_game_file(self, out, index) } }
    unsafe fn format_save_hub_file(&self, out: *mut u32, position: i32, index: i32) -> Result<()> { unsafe { Hp2GameSaveServices::format_save_hub_file(self, out, position, index) } }
    unsafe fn file_copy(&self, first: *const u32, second: *const u32) -> Result<()> { unsafe { Hp2GameSaveServices::file_copy(self, first, second) } }
    unsafe fn file_size(&self, filename: *const u32) -> Result<i32> { unsafe { Hp2GameSaveServices::file_size(self, filename) } }
    unsafe fn file_delete(&self, filename: *const u32) -> Result<()> { unsafe { Hp2GameSaveServices::file_delete(self, filename) } }
    unsafe fn level_actor_count(&self, engine: *mut c_void) -> Result<i32> { unsafe { Hp2GameSaveServices::level_actor_count(self, engine) } }
    unsafe fn level_actor(&self, engine: *mut c_void, index: i32) -> Result<*mut c_void> { unsafe { Hp2GameSaveServices::level_actor(self, engine, index) } }
    unsafe fn actor_is_mover(&self, actor: *mut c_void) -> Result<i32> { unsafe { Hp2GameSaveServices::actor_is_mover(self, actor) } }
    unsafe fn mover_reset_saved_pos(&self, actor: *mut c_void) -> Result<()> { unsafe { Hp2GameSaveServices::mover_reset_saved_pos(self, actor) } }
    unsafe fn level_new_brush_tracker(&self, engine: *mut c_void) -> Result<()> { unsafe { Hp2GameSaveServices::level_new_brush_tracker(self, engine) } }
    unsafe fn end_slow_task(&self) -> Result<()> { unsafe { Hp2GameSaveServices::end_slow_task(self) } }
    unsafe fn gcache_flush(&self) -> Result<()> { unsafe { Hp2GameSaveServices::gcache_flush(self) } }
    unsafe fn save_persistent_actor_cache(&self) -> Result<()> { unsafe { Hp2GameSaveServices::save_persistent_actor_cache(self) } }
}
/// `UGameEngine::SaveGame` (`UnGame.cpp:2205`).
///
/// # Safety
/// `engine` must be a live `UGameEngine*` (or the call is UB as in the original).
pub(crate) unsafe fn save_game_with<O: SaveGameOps>(s: &O, engine: *mut c_void, position: i32) -> std::result::Result<(), O::Error> {
    let mut filename = [0u32; PATH_CAPACITY];
    // SAFETY: every adapter performs the original call on the live engine.
    unsafe {
        s.make_dir_save_path()?;
        s.make_dir_save_slot_path()?;
        s.format_slot_file(filename.as_mut_ptr(), position)?;
        s.debugf_saving(filename.as_ptr())?;
        s.level_info_set_action(engine, LEVACT_SAVING)?;
        s.paint_progress(engine)?;
        s.begin_slow_task_saving()?;
        if s.level_has_brush_tracker(engine)? != 0 {
            s.level_delete_brush_tracker(engine)?;
        }
        s.level_cleanup_destroyed(engine)?;

        if s.level_save_package(engine, filename.as_ptr())? != 0 {
            let mut i = 0;
            while i < s.level_info_hub_stack_level(engine)? {
                let mut src = [0u32; PATH_CAPACITY];
                let mut dest = [0u32; PATH_CAPACITY];
                s.format_game_file(src.as_mut_ptr(), i)?;
                s.format_save_hub_file(dest.as_mut_ptr(), position, i)?;
                // Native SaveFileCopy passes (Src, Dest) to GFileManager::Copy(Dest, Src),
                // so the existing slot's hub file is copied over Game%i.usa.
                s.file_copy(src.as_ptr(), dest.as_ptr())?;
                i += 1;
            }
            loop {
                s.format_save_hub_file(filename.as_mut_ptr(), position, i)?;
                if s.file_size(filename.as_ptr())? <= 0 {
                    break;
                }
                s.file_delete(filename.as_ptr())?;
                i += 1;
            }
        }

        let mut i = 0;
        while i < s.level_actor_count(engine)? {
            let actor = s.level_actor(engine, i)?;
            if s.actor_is_mover(actor)? != 0 {
                s.mover_reset_saved_pos(actor)?;
            }
            i += 1;
        }

        s.level_new_brush_tracker(engine)?;
        s.end_slow_task()?;
        s.level_info_set_action(engine, LEVACT_NONE)?;
        s.gcache_flush()?;
        s.save_persistent_actor_cache()?;
    }
    Ok(())
}

/// Native adapter entry point; canonical Core owners use [`save_game_with`]
/// with their typed operations instead of constructing this callback table.
///
/// # Safety
/// `s` must remain valid and `engine` must be a live native `UGameEngine*`.
pub unsafe fn save_game(s: &Hp2GameSaveServices, engine: *mut c_void, position: i32) -> Result {
    unsafe { save_game_with(s, engine, position) }
}

/// `ExportTravel` (`UnGame.cpp:1841`).
///
/// # Safety
/// `out` must be a live `FOutputDevice*`, and `actor` a live `AActor*`.
pub unsafe fn export_travel(
    s: &Hp2GameSaveServices,
    out: *mut c_void,
    actor: *mut c_void,
) -> Result {
    unsafe {
        s.debugf_export_class(actor)?;
        if actor.is_null() {
            s.check_failed(c"Actor".as_ptr(), 1895)?;
        }
        if s.actor_b_travel(actor)? == 0 {
            return Ok(());
        }
        s.out_logf_header(out, actor)?;
        let status = (s.with_properties)(out, actor);
        if status != 0 {
            return Err(status);
        }
        s.out_logf_footer(out)?;
    }
    Ok(())
}

/// Runs only while the native stack TFieldIterator is alive.
/// # Safety
/// `it` is the native continuation's live iterator; output and actor are live.
pub unsafe fn export_properties(
    s: &Hp2GameSaveServices,
    out: *mut c_void,
    actor: *mut c_void,
    it: *mut c_void,
) -> Result {
    unsafe {
        while s.prop_iter_valid(it)? != 0 {
            let mut index = 0;
            while index < s.prop_array_dim(it)? {
                let mut value = [0u32; VALUE_CAPACITY];
                if s.prop_travel_flag(it)? != 0
                    && s.prop_export_text(it, index, value.as_mut_ptr(), actor)? != 0
                {
                    s.out_log_prop_name(out, it)?;
                    if s.prop_array_dim(it)? != 1 {
                        s.out_logf_index(out, index)?;
                    }
                    s.out_log_equals(out)?;
                    if s.prop_is_actor_ref(it)? != 0 {
                        s.out_logf_object_ref(out, it, actor, index)?;
                    }
                    s.out_logf_value(out, value.as_ptr())?;
                }
                index += 1;
            }
            s.prop_iter_next(it)?;
        }
    }
    Ok(())
}

/// Generic operations used by native and Core `BuildServerMasterMap` adapters.
pub(crate) trait ServerMasterMapOps {
    type Error;

    unsafe fn check_failed(&mut self, expression: &'static [u8], line: i32) -> std::result::Result<(), Self::Error>;
    unsafe fn begin_load(&mut self) -> std::result::Result<(), Self::Error>;
    unsafe fn end_load(&mut self) -> std::result::Result<(), Self::Error>;
    unsafe fn level_linker(&mut self, level: *mut c_void) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn master_map_add_linker(
        &mut self,
        driver: *mut c_void,
        linker: *mut c_void,
    ) -> std::result::Result<i32, Self::Error>;
    unsafe fn server_packages_count(&mut self, engine: *mut c_void) -> std::result::Result<i32, Self::Error>;
    unsafe fn server_package_debugf(
        &mut self,
        engine: *mut c_void,
        index: i32,
    ) -> std::result::Result<(), Self::Error>;
    unsafe fn server_package_linker(
        &mut self,
        engine: *mut c_void,
        index: i32,
    ) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn debugf_server_side_only(&mut self) -> std::result::Result<(), Self::Error>;
    unsafe fn level_level_info(&mut self, level: *mut c_void) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn level_info_game(&mut self, info: *mut c_void) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn game_class_linker(&mut self, game: *mut c_void) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn master_map_compute(&mut self, driver: *mut c_void) -> std::result::Result<(), Self::Error>;
}

/// Shared policy for `UGameEngine::BuildServerMasterMap`.
///
/// # Safety
/// The supplied operations must preserve their backend's object and load-state
/// invariants; pointers must identify the expected live engine records.
pub(crate) unsafe fn build_server_master_map_with<O: ServerMasterMapOps>(
    ops: &mut O,
    engine: *mut c_void,
    net_driver: *mut c_void,
    level: *mut c_void,
) -> std::result::Result<(), O::Error> {
    // SAFETY: caller guarantees that non-null pointers are valid for this backend.
    unsafe {
        if net_driver.is_null() {
            ops.check_failed(c"NetDriver".to_bytes_with_nul(), 81)?;
        }
        if level.is_null() {
            ops.check_failed(c"InLevel".to_bytes_with_nul(), 82)?;
        }
        ops.begin_load()?;

        let linker = ops.level_linker(level)?;
        if linker.is_null() {
            ops.check_failed(c"InLevel->GetLinker()".to_bytes_with_nul(), 86)?;
        }
        ops.master_map_add_linker(net_driver, linker)?;

        let mut i = 0;
        while i < ops.server_packages_count(engine)? {
            ops.server_package_debugf(engine, i)?;
            let pkg_linker = ops.server_package_linker(engine, i)?;
            if ops.master_map_add_linker(net_driver, pkg_linker)? == INDEX_NONE {
                ops.debugf_server_side_only()?;
            }
            i += 1;
        }

        let info = ops.level_level_info(level)?;
        if info.is_null() {
            ops.check_failed(c"InLevel->GetLevelInfo()".to_bytes_with_nul(), 99)?;
        }
        let game = ops.level_info_game(info)?;
        if game.is_null() {
            ops.check_failed(c"InLevel->GetLevelInfo()->Game".to_bytes_with_nul(), 100)?;
        }
        let game_linker = ops.game_class_linker(game)?;
        if game_linker.is_null() {
            ops.check_failed(
                c"InLevel->GetLevelInfo()->Game->GetClass()->GetLinker()".to_bytes_with_nul(),
                101,
            )?;
        }
        ops.master_map_add_linker(net_driver, game_linker)?;
        ops.master_map_compute(net_driver)?;
        ops.end_load()?;
    }
    Ok(())
}

struct NativeServerMasterMapOps<'a>(&'a Hp2GameSaveServices);

impl ServerMasterMapOps for NativeServerMasterMapOps<'_> {
    type Error = i32;

    unsafe fn check_failed(&mut self, expression: &'static [u8], line: i32) -> Result<()> {
        // SAFETY: expression is a static NUL-terminated C string.
        unsafe { self.0.check_failed(expression.as_ptr().cast(), line) }
    }
    unsafe fn begin_load(&mut self) -> Result<()> { unsafe { self.0.begin_load() } }
    unsafe fn end_load(&mut self) -> Result<()> { unsafe { self.0.end_load() } }
    unsafe fn level_linker(&mut self, level: *mut c_void) -> Result<*mut c_void> {
        unsafe { self.0.level_linker(level) }
    }
    unsafe fn master_map_add_linker(&mut self, driver: *mut c_void, linker: *mut c_void) -> Result<i32> {
        unsafe { self.0.master_map_add_linker(driver, linker) }
    }
    unsafe fn server_packages_count(&mut self, engine: *mut c_void) -> Result<i32> {
        unsafe { self.0.server_packages_count(engine) }
    }
    unsafe fn server_package_debugf(&mut self, engine: *mut c_void, index: i32) -> Result<()> {
        unsafe { self.0.server_package_debugf(engine, index) }
    }
    unsafe fn server_package_linker(&mut self, engine: *mut c_void, index: i32) -> Result<*mut c_void> {
        unsafe { self.0.server_package_linker(engine, index) }
    }
    unsafe fn debugf_server_side_only(&mut self) -> Result<()> {
        unsafe { self.0.debugf_server_side_only() }
    }
    unsafe fn level_level_info(&mut self, level: *mut c_void) -> Result<*mut c_void> {
        unsafe { self.0.level_level_info(level) }
    }
    unsafe fn level_info_game(&mut self, info: *mut c_void) -> Result<*mut c_void> {
        unsafe { self.0.level_info_game(info) }
    }
    unsafe fn game_class_linker(&mut self, game: *mut c_void) -> Result<*mut c_void> {
        unsafe { self.0.game_class_linker(game) }
    }
    unsafe fn master_map_compute(&mut self, driver: *mut c_void) -> Result<()> {
        unsafe { self.0.master_map_compute(driver) }
    }
}

/// `UGameEngine::BuildServerMasterMap` (`UnGame.cpp:71`).
///
/// # Safety
/// `engine`, `net_driver`, and `level` must be live objects of the original
/// types (null is checked, and then dereferenced by the original path).
pub unsafe fn build_server_master_map(
    s: &Hp2GameSaveServices,
    engine: *mut c_void,
    net_driver: *mut c_void,
    level: *mut c_void,
) -> Result {
    // SAFETY: forwarded from this function's caller.
    unsafe {
        build_server_master_map_with(
            &mut NativeServerMasterMapOps(s),
            engine,
            net_driver,
            level,
        )
    }
}
