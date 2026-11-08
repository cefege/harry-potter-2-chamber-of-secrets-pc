//! Port of `appCreateProc`, `appGetProcReturnCode`, `HandleChild`'s SIGCHLD
//! reaping, `Hp2SystemCommandInvoke`'s `LaunchServer` fork/exec, and the
//! trivial empty-body platform hooks (`Core/Src/UnUnix.cpp`).
//! `fork`/`system`/`_exit` in [`create_proc`]'s child branch call only raw
//! `libc` functions with no allocation or `Drop` between `fork` and
//! `_exit`, preserving the original's "fork child stays entirely native"
//! async-signal-safety invariant -- now from Rust instead of C++, issuing
//! the exact same libc calls in the exact same order.

use std::ffi::c_char;

use crate::ansi::Tchar;
use crate::unix_alloc;
use crate::unix_text::{self, Hp2UnixTextServices};

/// `appCreateProc`'s `FUnixUtf8String URLUtf8/ParmsUtf8` encode-plus-concat
/// step: `"<url> <parms>\0"` in one `system_malloc`'d buffer, matching the
/// original byte-for-byte (only the URL half is path-normalized; `Parms` is
/// not). `url`/`parms` may be null, treated as empty (matching the
/// original's `URL ? URL : TEXT("")` ternary -- [`unix_text::to_utf8`]
/// already treats a null TCHAR pointer as empty).
///
/// Returns `Ok(buffer)` on success, owned by the native system allocator
/// (release with native libc `free`). `Err(2)` means the combined command
/// line overflows `usize` (the original's unreachable-in-practice
/// `appErrorf` path); the caller must raise that error itself. Every other
/// `Err(status)` is a `unix_text` callback status, propagated unchanged.
///
/// # Safety
/// `url`/`parms` must each be null or a readable NUL-terminated TCHAR
/// string. `services` must satisfy its documented leaf contract.
pub unsafe fn build_proc_command(
    services: &Hp2UnixTextServices,
    url: *const Tchar,
    parms: *const Tchar,
) -> Result<*mut c_char, i32> {
    // SAFETY: `url`/`services` are valid by contract.
    let url_utf8 = unsafe { unix_text::to_utf8(services, url) }?;
    // SAFETY: `url_utf8` is a fresh NUL-terminated native allocation.
    unsafe { unix_text::normalize_path(url_utf8) };
    // SAFETY: `parms`/`services` are valid by contract.
    let parms_utf8 = match unsafe { unix_text::to_utf8(services, parms) } {
        Ok(bytes) => bytes,
        Err(status) => {
            // SAFETY: `url_utf8` came from the native system allocator.
            unsafe { libc::free(url_utf8.cast()) };
            return Err(status);
        }
    };
    // SAFETY: both buffers are NUL-terminated native allocations.
    let url_length = unsafe { libc::strlen(url_utf8.cast()) };
    let parms_length = unsafe { libc::strlen(parms_utf8.cast()) };
    if url_length > usize::MAX - parms_length - 2 {
        // SAFETY: both came from the native system allocator.
        unsafe {
            libc::free(url_utf8.cast());
            libc::free(parms_utf8.cast());
        }
        return Err(2);
    }
    let total = url_length + parms_length + 2;
    // SAFETY: the system allocator contract.
    let buffer = unsafe { unix_alloc::system_malloc(total) }.cast::<u8>();
    // SAFETY: `buffer` has room for `total` bytes; `url_utf8`/`parms_utf8`
    // are each readable for their own length (`parms_utf8`'s copy includes
    // its trailing NUL).
    unsafe {
        std::ptr::copy_nonoverlapping(url_utf8, buffer, url_length);
        *buffer.add(url_length) = b' ';
        std::ptr::copy_nonoverlapping(parms_utf8, buffer.add(url_length + 1), parms_length + 1);
        libc::free(url_utf8.cast());
        libc::free(parms_utf8.cast());
    }
    Ok(buffer.cast())
}

/// Fixed capacity for the SIGCHLD exit-code table, replacing the original's
/// unbounded `TMap<pid_t,int>`. `appCreateProc` launches processes only for
/// rare, explicit, user-initiated actions (e.g. a dedicated server), so this
/// generously exceeds any realistic number of simultaneously-unreaped
/// children; see [`record_child_exit`] for the overflow behavior.
const MAX_PENDING_CHILDREN: usize = 64;

struct ChildSlot {
    pid: std::sync::atomic::AtomicI32,
    code: std::sync::atomic::AtomicI32,
}

/// `pid_t` 0 is never a real child pid (reserved for the scheduler), so it
/// is a safe "slot is empty" sentinel.
static EXIT_CODES: [ChildSlot; MAX_PENDING_CHILDREN] = {
    const EMPTY: ChildSlot = ChildSlot {
        pid: std::sync::atomic::AtomicI32::new(0),
        code: std::sync::atomic::AtomicI32::new(0),
    };
    [EMPTY; MAX_PENDING_CHILDREN]
};

/// Claims an empty slot for `pid`/`code` via a single `compare_exchange`
/// (no locks, no allocation: sound to call from a signal handler). If every
/// slot is already claimed, the exit code is silently dropped -- the same
/// outcome as the original hitting `OOM` inside `TMap::Set` would have had,
/// just without the crash, and unreachable at this engine's real-world
/// `appCreateProc` call volume.
fn record_child_exit(pid: libc::pid_t, code: i32) {
    use std::sync::atomic::Ordering;
    for slot in EXIT_CODES.iter() {
        if slot
            .pid
            .compare_exchange(0, pid, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            slot.code.store(code, Ordering::SeqCst);
            return;
        }
    }
}

/// `HandleChild`: reaps every finished child via `waitpid(-1,&Status,WNOHANG)`
/// and records its exit status. Async-signal-safe (atomics only).
///
/// # Safety
/// Must only run as an installed signal handler.
extern "C" fn handle_child(_signal: std::os::raw::c_int) {
    loop {
        let mut status: std::os::raw::c_int = 0;
        // SAFETY: a plain `waitpid` call; async-signal-safe.
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid <= 0 {
            break;
        }
        record_child_exit(pid, libc::WEXITSTATUS(status));
    }
}

/// `appPlatformInit`'s `sigaction(SIGCHLD, ...)` install step: `handle_child`
/// with an empty mask and no flags, matching the original exactly.
pub fn install_sigchld_handler() {
    // SAFETY: a plain, fully-initialized local `sigaction`; `sigaction` with
    // a null old-disposition out-param is always sound to call.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = handle_child as *const () as libc::sighandler_t;
        libc::sigemptyset(&mut action.sa_mask);
        action.sa_flags = 0;
        libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut());
    }
}

/// `appCreateProc`'s `fork()`/`system()`/`_exit()` dance. `local_command`
/// must be the already-built, NUL-terminated native command line from
/// [`build_proc_command`]; freed here in the parent exactly where the
/// original freed it (unconditionally, after the fork, regardless of
/// whether `fork` itself succeeded). Returns the child pid, or a negative
/// value if `fork` failed (matching the original's `Pid<0` check).
///
/// # Safety
/// `local_command` must be a readable NUL-terminated native command line,
/// owned by the native system allocator.
pub unsafe fn create_proc(local_command: *mut c_char) -> i32 {
    // SAFETY: a plain `fork` call.
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        // SAFETY: the child branch only ever calls `system`/`_exit`, both
        // raw libc calls; `local_command` is readable and NUL-terminated by
        // contract, fully built in the parent before this call.
        unsafe {
            let status = libc::system(local_command);
            libc::_exit(status);
        }
    }
    // SAFETY: `local_command` came from the native system allocator.
    unsafe { libc::free(local_command.cast()) };
    pid
}

/// `Hp2SystemCommandInvoke`'s `Hp2SystemCommandLeaf_LaunchServer` fork/exec.
/// `server_url` must be the already-encoded, NUL-terminated, path-normalized
/// native command-line argument (the original's `**Context.ServerUrl`); the
/// child calls only `sleep`/`execl`/`_exit`, all raw libc calls, matching
/// the original's own "fork child stays entirely native" invariant byte for
/// byte. Returns the child pid, or a negative value if `fork` failed
/// (matching the original's `Pid<0` check).
///
/// # Safety
/// `server_url` must be a readable NUL-terminated native string, valid for
/// the duration of this call.
pub unsafe fn launch_server(server_url: *const c_char) -> i32 {
    // SAFETY: a plain `fork` call.
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        // SAFETY: the child branch only ever calls raw libc functions;
        // `server_url` is readable and NUL-terminated by contract.
        unsafe {
            libc::sleep(3);
            libc::execl(
                c"./ucc".as_ptr(),
                c"ucc".as_ptr(),
                c"server".as_ptr(),
                server_url,
                std::ptr::null::<c_char>(),
            );
            libc::_exit(127);
        }
    }
    pid
}

/// `appGetProcReturnCode`'s `MAXINT` range-check/cast/lookup/remove steps.
pub fn get_proc_return_code(proc_handle: u64) -> Option<i32> {
    use std::sync::atomic::Ordering;
    if proc_handle > i32::MAX as u64 {
        return None;
    }
    let pid = proc_handle as libc::pid_t;
    for slot in EXIT_CODES.iter() {
        if slot.pid.load(Ordering::SeqCst) == pid {
            let code = slot.code.load(Ordering::SeqCst);
            slot.pid.store(0, Ordering::SeqCst);
            return Some(code);
        }
    }
    None
}

/// `appLaunchURL`: the original's body is empty (the `Error` out-param is
/// never written). Kept as a Rust export purely so the behavior -- doing
/// nothing -- is nominally Rust-owned like every other platform hook here.
pub fn launch_url() {}

/// `appEnableFastMath`: the original's body is empty.
pub fn enable_fast_math() {}

/// `appPlatformPreExit`: the original's body is empty.
pub fn platform_pre_exit() {}

/// `appPlatformExit`: the original's body is empty.
pub fn platform_exit() {}

/// `appClipboardCopy`: the original's body is empty (clipboard support was
/// disabled; the original text is commented out alongside it).
pub fn clipboard_copy() {}

/// `appClipboardPaste`: the original's body always returns an empty string.
/// `FString`'s default constructor is a C++-only ABI operation, so the
/// caller still builds the empty return value itself.
pub fn clipboard_paste() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `EXIT_CODES` is one process-global static; serialize tests against it
    // so they do not observe each other's slots (matching the precedent in
    // `rng_private.rs`'s tests).
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn records_and_takes_a_single_child_exit() {
        let _guard = TEST_LOCK.lock().unwrap();
        record_child_exit(123_456, 7);
        assert_eq!(get_proc_return_code(123_456), Some(7));
        // Taken once: the slot is released and a second read finds nothing.
        assert_eq!(get_proc_return_code(123_456), None);
    }

    #[test]
    fn real_fork_exit_code_round_trips_through_the_installed_sigchld_handler() {
        let _guard = TEST_LOCK.lock().unwrap();
        install_sigchld_handler();
        // SAFETY: a plain `fork` call, serialized against the other tests in
        // this module by `TEST_LOCK` (no other test in this binary forks).
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // SAFETY: the child only ever calls `_exit`.
            unsafe { libc::_exit(42) };
        }
        assert!(pid > 0, "fork failed");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut found = None;
        while std::time::Instant::now() < deadline {
            if let Some(code) = get_proc_return_code(pid as u64) {
                found = Some(code);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(found, Some(42), "exit code never arrived via SIGCHLD");
    }

    #[test]
    fn unknown_pid_returns_none() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(get_proc_return_code(987_654), None);
    }

    #[test]
    fn rejects_a_handle_above_maxint() {
        let _guard = TEST_LOCK.lock().unwrap();
        record_child_exit(55, 1);
        // `appGetProcReturnCode`'s `RawPid>static_cast<UPTRINT>(MAXINT)` guard:
        // a handle above `i32::MAX` is always rejected, even if a (fake, for
        // this test) slot's pid would otherwise match after truncation.
        assert_eq!(get_proc_return_code(i32::MAX as u64 + 1), None);
        assert_eq!(get_proc_return_code(55), Some(1));
    }

    #[test]
    fn distinct_children_keep_independent_exit_codes() {
        let _guard = TEST_LOCK.lock().unwrap();
        record_child_exit(111, 1);
        record_child_exit(222, 2);
        record_child_exit(333, 3);
        assert_eq!(get_proc_return_code(222), Some(2));
        assert_eq!(get_proc_return_code(111), Some(1));
        assert_eq!(get_proc_return_code(333), Some(3));
    }

    #[test]
    fn table_overflow_silently_drops_excess_entries() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Fill every slot, including any left over from other tests in this
        // run (all use small pids below this range), then one more.
        let base = 10_000;
        for offset in 0..MAX_PENDING_CHILDREN as i32 {
            record_child_exit(base + offset, offset);
        }
        record_child_exit(base + MAX_PENDING_CHILDREN as i32, 999);
        assert_eq!(
            get_proc_return_code((base + MAX_PENDING_CHILDREN as i32) as u64),
            None
        );
        for offset in 0..MAX_PENDING_CHILDREN as i32 {
            assert_eq!(get_proc_return_code((base + offset) as u64), Some(offset));
        }
    }
}
