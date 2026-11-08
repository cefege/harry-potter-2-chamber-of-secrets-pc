//! Complete macOS speech process lifecycle. The native viewport embeds this
//! Rust-defined state without an allocation; its FString-to-UTF8 callback owns
//! the conversion buffer until the call returns. No child is killed or waited
//! for on replacement/destruction. Native viewport storage itself remains part
//! of the object-foundation cutover.

use std::ffi::{c_char, c_void};

/// Inline per-viewport state. Native code initializes it through the Rust
/// initializer and otherwise only passes its address back to Rust.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hp2PlatformSpeechState {
    pub pid: i32,
}

impl Default for Hp2PlatformSpeechState {
    fn default() -> Self {
        Self { pid: -1 }
    }
}

/// Checked FString-to-UTF8 typed leaf. Success writes a NUL-terminated pointer
/// whose storage survives the complete speech call; failure returns the exact
/// native status. No exception may unwind through this callback.
pub type Hp2PlatformSpeechText =
    unsafe extern "C" fn(context: *mut c_void, text: *mut *const c_char) -> i32;

#[cfg(any(target_vendor = "apple", test))]
trait Processes {
    fn poll(&mut self, pid: i32) -> bool;
    unsafe fn spawn(&mut self, text: *const c_char) -> i32;
}

#[cfg(any(target_vendor = "apple", test))]
fn update_with(state: &mut Hp2PlatformSpeechState, processes: &mut impl Processes) {
    if state.pid > 0 && processes.poll(state.pid) {
        state.pid = -1;
    }
}

// Take a raw pointer rather than retaining a Rust exclusive borrow across the
// native conversion callback. Mutations before a failing callback stay visible.
#[cfg(any(target_vendor = "apple", test))]
unsafe fn start_with(
    state: *mut Hp2PlatformSpeechState,
    enabled: bool,
    text: impl FnOnce() -> Result<*const c_char, i32>,
    processes: &mut impl Processes,
) -> Result<(), i32> {
    if !enabled {
        return Ok(());
    }
    update_with(unsafe { &mut *state }, processes);
    let text = text()?;
    let child = unsafe { processes.spawn(text) };
    if child > 0 {
        unsafe { (*state).pid = child };
    }
    Ok(())
}

#[cfg(all(unix, any(target_vendor = "apple", test)))]
fn finished(pid: i32, result: i32, error: i32) -> bool {
    result == pid || (result < 0 && error == libc::ECHILD)
}

#[cfg(target_vendor = "apple")]
struct NativeProcesses;

#[cfg(target_vendor = "apple")]
impl Processes for NativeProcesses {
    fn poll(&mut self, pid: i32) -> bool {
        let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
        let error = if result < 0 {
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
        } else {
            0
        };
        finished(pid, result, error)
    }

    unsafe fn spawn(&mut self, text: *const c_char) -> i32 {
        let child = unsafe { libc::fork() };
        if child == 0 {
            // Only async-signal-safe C calls in the child: no Rust allocation,
            // callback, formatting, lock, destructor, panic, or unwinding.
            unsafe {
                libc::execl(c"/usr/bin/say".as_ptr(), c"say".as_ptr(), text, std::ptr::null::<c_char>());
                libc::_exit(127);
            }
        }
        child
    }
}

/// Runs the complete supported speech path; non-Apple targets are no-ops,
/// including no callback and no state update, matching the native shell.
///
/// # Safety
/// `state` points to live writable state. `convert` obeys its documented pointer
/// lifetime and exception contract. The native context must not destroy state.
pub unsafe fn start(
    state: *mut Hp2PlatformSpeechState,
    enabled: bool,
    context: *mut c_void,
    convert: Hp2PlatformSpeechText,
) -> Result<(), i32> {
    #[cfg(target_vendor = "apple")]
    {
        unsafe {
            start_with(state, enabled, || {
                let mut text = std::ptr::null();
                let status = convert(context, &mut text);
                if status == 0 { Ok(text) } else { Err(status) }
            }, &mut NativeProcesses)
        }
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        let _ = (state, enabled, context, convert);
        Ok(())
    }
}

/// Performs one nonblocking wait, only for a positive tracked PID. No retry.
pub fn update(state: &mut Hp2PlatformSpeechState) {
    #[cfg(target_vendor = "apple")]
    update_with(state, &mut NativeProcesses);
    #[cfg(not(target_vendor = "apple"))]
    let _ = state;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Script<'a> {
        calls: &'a RefCell<Vec<&'static str>>,
        done: bool,
        child: i32,
        expected_pid: i32,
    }

    impl Processes for Script<'_> {
        fn poll(&mut self, pid: i32) -> bool {
            assert_eq!(pid, self.expected_pid);
            self.calls.borrow_mut().push("wait");
            self.done
        }
        unsafe fn spawn(&mut self, text: *const c_char) -> i32 {
            assert_eq!(unsafe { std::ffi::CStr::from_ptr(text) }, c"utterance");
            self.calls.borrow_mut().push("fork");
            self.child
        }
    }

    #[test]
    fn disabled_does_not_reap_convert_or_spawn() {
        let calls = RefCell::new(Vec::new());
        let mut process = Script { calls: &calls, done: true, child: 99, expected_pid: 41 };
        let mut state = Hp2PlatformSpeechState { pid: 41 };
        assert_eq!(unsafe { start_with(&mut state, false, || panic!("conversion"), &mut process) }, Ok(()));
        assert_eq!(state.pid, 41);
        assert!(calls.borrow().is_empty());
    }

    #[test]
    fn replacement_and_fork_failure_preserve_exact_wait_conversion_order() {
        for old_pid in [-1, 0, 41] {
            for done in [false, true] {
                for child in [-1, 99] {
                    let calls = RefCell::new(Vec::new());
                    let mut process = Script { calls: &calls, done, child, expected_pid: old_pid };
                    let mut state = Hp2PlatformSpeechState { pid: old_pid };
                    let result = unsafe { start_with(&mut state, true, || {
                        calls.borrow_mut().push("convert");
                        Ok(c"utterance".as_ptr())
                    }, &mut process) };
                    assert_eq!(result, Ok(()));
                    let expected = if child > 0 { child } else if old_pid > 0 && done { -1 } else { old_pid };
                    assert_eq!(state.pid, expected);
                    let expected_calls = if old_pid > 0 { vec!["wait", "convert", "fork"] } else { vec!["convert", "fork"] };
                    assert_eq!(*calls.borrow(), expected_calls);
                }
            }
        }
    }

    #[test]
    fn failed_conversion_stops_after_reaping_and_keeps_partial_mutation() {
        let calls = RefCell::new(Vec::new());
        let mut process = Script { calls: &calls, done: true, child: 99, expected_pid: 41 };
        let mut state = Hp2PlatformSpeechState { pid: 41 };
        assert_eq!(unsafe { start_with(&mut state, true, || Err(17), &mut process) }, Err(17));
        assert_eq!(state.pid, -1);
        assert_eq!(*calls.borrow(), ["wait"]);
    }

    #[cfg(unix)]
    #[test]
    fn wait_status_distinguishes_reaped_missing_running_and_unrelated_errors() {
        assert!(finished(41, 41, 0));
        assert!(finished(41, -1, libc::ECHILD));
        assert!(!finished(41, 0, libc::ECHILD));
        assert!(!finished(41, 42, 0));
        for error in [libc::EINTR, libc::EINVAL, libc::EAGAIN] {
            assert!(!finished(41, -1, error));
        }
    }

    #[cfg(not(target_vendor = "apple"))]
    #[test]
    fn unsupported_platform_does_not_touch_state_or_convert() {
        unsafe extern "C" fn fail(_: *mut c_void, _: *mut *const c_char) -> i32 { 17 }
        let mut state = Hp2PlatformSpeechState { pid: 41 };
        assert_eq!(unsafe { start(&mut state, true, std::ptr::null_mut(), fail) }, Ok(()));
        update(&mut state);
        assert_eq!(state.pid, 41);
    }
}
