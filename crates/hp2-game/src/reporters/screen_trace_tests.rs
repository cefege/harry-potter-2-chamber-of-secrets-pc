//! Runtime lifecycle checks. Each case gets its own environment and process.
#![cfg(unix)]
use super::*;
use hp2_core::core_access::CoreOptions;
use std::{fs, panic, process::Command};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

// RLIMIT_FSIZE and signal dispositions are process-wide. This is used only in
// the dedicated child, never in the parent test process. No descriptors close.
struct FileSizeLimit {
    previous: libc::rlimit,
    previous_signal: libc::sighandler_t,
}
impl FileSizeLimit {
    fn at(bytes: u64) -> Self {
        let mut previous = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: getrlimit initializes the supplied writable record on success.
        assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, previous.as_mut_ptr()) }, 0);
        let previous = unsafe { previous.assume_init() };
        let limit = libc::rlim_t::try_from(bytes).expect("header length fits rlim_t");
        assert!(limit <= previous.rlim_max, "hard file-size limit is too small");
        // SAFETY: SIG_IGN is a valid signal disposition. This child owns the change.
        let previous_signal = unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_IGN) };
        assert_ne!(previous_signal, libc::SIG_ERR, "ignore SIGXFSZ");
        let temporary = libc::rlimit { rlim_cur: limit, rlim_max: previous.rlim_max };
        let guard = Self { previous, previous_signal };
        // SAFETY: the limit record is initialized and its hard limit is unchanged.
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &temporary) }, 0);
        guard
    }
}
impl Drop for FileSizeLimit {
    fn drop(&mut self) {
        // SAFETY: restore the values obtained from this child process above.
        let limit_result = unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &self.previous) };
        let signal_result = unsafe { libc::signal(libc::SIGXFSZ, self.previous_signal) };
        if limit_result != 0 || signal_result == libc::SIG_ERR {
            eprintln!("failed to restore child RLIMIT_FSIZE or SIGXFSZ");
            if !std::thread::panicking() { panic!("child limit restoration failed"); }
        }
    }
}

#[test]
fn isolated_lifecycle_cases() {
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    for case in ["normal", "error", "panic", "stale", "write-failure"] {
        let dir = root.join(format!("screen-trace-{case}-{}", std::process::id()));
        fs::create_dir(&dir).expect("create isolated case directory");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "reporters::screen_trace_tests::child", "--nocapture"])
            .env("HP2_SCREEN_TRACE_CHILD", case)
            .env("HP2_SCREEN_TRACE_ROOT", &dir)
            .env("HP2_GAME_STATE_TRACE", dir.join("trace.log"))
            .env("HP2_GAME_STATE_TRACE_ID", "0123456789abcdef0123456789abcdef");
        if case == "error" || case == "panic" {
            command.env("HP2_GAME_STATE_TRACE_TEST_SETUP", case);
        } else {
            command.env_remove("HP2_GAME_STATE_TRACE_TEST_SETUP");
        }
        let result = command.status().expect("start isolated child");
        assert!(result.success(), "{case} child failed: {result}");
    }
}

#[test]
fn child() {
    let Ok(case) = std::env::var("HP2_SCREEN_TRACE_CHILD") else { return };
    let root = std::path::PathBuf::from(std::env::var_os("HP2_SCREEN_TRACE_ROOT").unwrap());
    fs::create_dir(root.join("System")).unwrap();
    fs::write(root.join("System/Default.ini"), b"").unwrap();
    fs::write(root.join("System/DefUser.ini"), b"").unwrap();
    let trace = root.join("trace.log");
    if case == "stale" { fs::write(&trace, b"old evidence\n").unwrap(); }
    let owner = Box::new(EngineOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }).unwrap());
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| unsafe {
        RuntimeReporters::new(&*owner as *const EngineOwner)
    }));
    match case.as_str() {
        "normal" => {
            let mut reporters = result.unwrap().unwrap();
            assert!(reporters.cutscene_guard.is_some());
            assert!(unsafe { owner.access().install_resolved_script_scope_observer(
                "game-state-cutscene-blocked", select_cutscene_blocked, observe_cutscene_blocked,
            ) }.is_err());
            reporters.detach().unwrap();
            reporters.detach().unwrap();
            assert!(reporters.cutscene_guard.is_none());
            let replacement = unsafe { owner.access().install_resolved_script_scope_observer(
                "game-state-cutscene-blocked", select_cutscene_blocked, observe_cutscene_blocked,
            ) }.expect("guard was removed while owner remains live");
            drop(replacement);
            drop(reporters);
            let text = fs::read_to_string(&trace).unwrap();
            assert!(text.contains("event=trace_end "));
            assert!(text.contains("capture_selection=blocked"));
        }
        "write-failure" => {
            let mut reporters = result.unwrap().unwrap();
            assert!(reporters.game_state_trace_active);
            assert!(reporters.cutscene_guard.is_some());
            let header_len = fs::metadata(&trace).unwrap().len();
            assert!(header_len > 0);
            {
                // This is a separate test process. Only its soft file-size limit
                // and SIGXFSZ disposition change, and both are restored on unwind.
                let _limit = FileSizeLimit::at(header_len);
                reporters.detach().expect("trace failure must not change game result");
            }
            assert!(!reporters.game_state_trace_active);
            assert!(reporters.cutscene_guard.is_none());
            let bytes = fs::read(&trace).unwrap();
            assert_eq!(bytes.len() as u64, header_len);
            assert!(!bytes.windows(b"event=trace_end".len()).any(|w| w == b"event=trace_end"));
            let consumer = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../Tests/GameStateScreenTraceSourceTests.py");
            let output = Command::new("python3").arg("-B").arg(consumer)
                .arg("--validate").arg(&trace)
                .arg("0123456789abcdef0123456789abcdef")
                .output().expect("run trace consumer");
            assert!(!output.status.success(), "consumer accepted incomplete trace");
            assert!(String::from_utf8_lossy(&output.stderr).contains("missing or invalid completion record"));
        }
        "error" => assert!(result.unwrap().is_err()),
        "panic" => assert!(result.is_err()),
        "stale" => {
            let mut reporters = result.unwrap().unwrap();
            assert!(reporters.cutscene_guard.is_none());
            reporters.detach().unwrap();
            assert_eq!(fs::read(&trace).unwrap(), b"old evidence\n");
        }
        _ => panic!("unknown case"),
    }
    if case == "error" || case == "panic" {
        let text = fs::read_to_string(&trace).unwrap();
        assert!(!text.contains("event=trace_end "));
        let replacement = unsafe { owner.access().install_resolved_script_scope_observer(
            "game-state-cutscene-blocked", select_cutscene_blocked, observe_cutscene_blocked,
        ) }.expect("partial setup removed guard before owner teardown");
        drop(replacement);
    }
    drop(owner);
}
