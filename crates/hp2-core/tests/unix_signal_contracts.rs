use hp2_core::unix_signal::{exit_requested, initialize, set_exit_requested};
use std::{
    cell::Cell,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::ExitStatusExt,
    },
    process::{Command, Output},
};

fn child(mode: &str) -> Output {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "signal_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HP2_SIGNAL_CONTRACT_CHILD", mode)
        .env_remove("HP2_RNG_SEED")
        .output()
        .unwrap();
    if let Some(directory) = std::env::var_os("HP2_ARTIFACT_DIR") {
        let path = std::path::PathBuf::from(directory);
        let mut log = output.stdout.clone();
        log.extend_from_slice(&output.stderr);
        std::fs::write(path.join(format!("unix-signal-{mode}.log")), log).unwrap();
    }
    output
}

#[test]
fn exit_signals_return_through_live_rust_cleanup() {
    for mode in ["term", "hup", "quit"] {
        let output = child(mode);
        assert!(output.status.success(), "{mode}: {output:?}");
    }
}

#[test]
fn repeated_exit_signals_preserve_forced_and_default_termination() {
    let different = child("second-different");
    assert_eq!(different.status.code(), Some(1), "{different:?}");
    let same = child("second-same");
    assert_eq!(same.status.signal(), Some(libc::SIGTERM), "{same:?}");
}

#[test]
fn fatal_signal_terminates_instead_of_recovering() {
    let output = child("fatal-trap");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
}

#[test]
fn closed_pipe_reports_epipe_without_requesting_exit() {
    let output = child("broken-pipe");
    assert!(output.status.success(), "{output:?}");
}

struct Cleanup<'a>(&'a Cell<u32>);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
#[ignore = "fresh-process helper invoked by the signal contracts"]
fn signal_child() {
    let mode = std::env::var("HP2_SIGNAL_CONTRACT_CHILD").unwrap();
    // Rust itself ignores SIGPIPE on startup: reset it so this test observes
    // the engine initializer's behavior, not the test harness's disposition.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        initialize(libc::RAND_MAX as u32);
    }
    assert!(!exit_requested());
    match mode.as_str() {
        "term" | "hup" | "quit" => {
            let signal = match mode.as_str() {
                "term" => libc::SIGTERM,
                "hup" => libc::SIGHUP,
                _ => libc::SIGQUIT,
            };
            let drops = Cell::new(0);
            {
                let _cleanup = Cleanup(&drops);
                assert_eq!(unsafe { libc::raise(signal) }, 0);
                assert!(exit_requested());
            }
            assert_eq!(drops.get(), 1, "a signal skipped live Rust cleanup");
            set_exit_requested(false);
            assert!(
                !exit_requested(),
                "explicit session reset did not clear exit state"
            );
            println!("Rust cleanup count={} exit request reset", drops.get());
        }
        "second-different" => unsafe {
            libc::raise(libc::SIGHUP);
            libc::raise(libc::SIGTERM);
            panic!("second distinct exit signal returned");
        },
        "second-same" => unsafe {
            libc::raise(libc::SIGTERM);
            libc::raise(libc::SIGTERM);
            panic!("second identical exit signal returned");
        },
        "fatal-trap" => unsafe {
            libc::raise(libc::SIGTRAP);
            panic!("fatal signal returned");
        },
        "broken-pipe" => {
            let mut fds = [0; 2];
            assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
            let reader = unsafe { OwnedFd::from_raw_fd(fds[0]) };
            let writer = unsafe { OwnedFd::from_raw_fd(fds[1]) };
            drop(reader);
            assert_eq!(
                unsafe { libc::write(writer.as_raw_fd(), b"x".as_ptr().cast(), 1) },
                -1
            );
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EPIPE)
            );
            assert!(!exit_requested());
        }
        _ => panic!("unknown signal contract child mode: {mode}"),
    }
}
