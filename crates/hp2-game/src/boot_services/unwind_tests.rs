use super::BootHost;
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeError, RuntimeResult};
use hp2_engine::{EngineOwner, SpawnInstrumentation, SpawnInstrumentationEvent};
use std::{
    cell::RefCell,
    fs, panic,
    path::{Path, PathBuf},
    rc::Rc,
};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn fixture(name: &str) -> PathBuf {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("artifact directory"))
        .join(format!("boot-unwind-{name}-{}", std::process::id()));
    fs::create_dir(&root).expect("create new fixture without replacing evidence");
    fs::create_dir(root.join("System")).expect("create System directory");
    fs::write(root.join("System/Default.ini"), b"").expect("write system ini");
    fs::write(root.join("System/DefUser.ini"), b"").expect("write user ini");
    root
}

fn owner(root: &Path) -> EngineOwner {
    EngineOwner::new(CoreOptions {
        data_root: root.to_path_buf(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("create data-none EngineOwner")
}

fn attached(root: &Path) -> Box<BootHost> {
    let mut host = Box::new(BootHost::new(Vec::new()));
    host.owner = Some(owner(root));
    // SAFETY: the boxed host will not move its owner while reporters are attached;
    // its declaration order drops reporters before the owner on unwind.
    host.reporters = Some(
        unsafe {
            crate::reporters::RuntimeReporters::new(
                host.owner.as_ref().unwrap() as *const EngineOwner
            )
        }
        .expect("attach real runtime reporters"),
    );
    host
}

fn dirty(host: &BootHost) {
    let units = |s: &str| s.chars().map(u32::from).collect::<Vec<_>>();
    host.owner
        .as_ref()
        .unwrap()
        .access()
        .config_set_units(
            &units("UnwindContract"),
            &units("Marker"),
            &units("present"),
        )
        .expect("dirty real config cache");
}

struct Probe {
    path: PathBuf,
    observed: Rc<RefCell<Vec<Result<Vec<u8>, String>>>>,
}
impl SpawnInstrumentation for Probe {
    fn on_spawn_instrumentation(
        &self,
        _: CoreAccess<'_>,
        _: SpawnInstrumentationEvent,
    ) -> RuntimeResult<()> {
        Ok(())
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        self.observed
            .borrow_mut()
            .push(fs::read(&self.path).map_err(|e| e.to_string()));
    }
}

fn probe(host: &BootHost, root: &Path, observed: &Rc<RefCell<Vec<Result<Vec<u8>, String>>>>) {
    host.owner
        .as_ref()
        .unwrap()
        .publish_spawn_instrumentation(Rc::new(Probe {
            path: root.join("System/Default.ini"),
            observed: Rc::clone(observed),
        }))
        .expect("install owner-local drop probe");
}

fn assert_flushed(root: &Path) {
    let bytes = fs::read(root.join("System/Default.ini")).expect("read flushed ini");
    assert!(
        bytes
            .windows(b"Marker=present".len())
            .any(|w| w == b"Marker=present"),
        "the ASCII fixture key must be flushed: {bytes:?}"
    );
}

fn assert_probe(observed: &Rc<RefCell<Vec<Result<Vec<u8>, String>>>>) {
    assert_eq!(
        &*observed.borrow(),
        &[Ok(Vec::new())],
        "probe must drop exactly once before config flush"
    );
}

const PREFIX: &str = r#"use hp2_abi::engine::generated::UGameEngine;
use hp2_core::core_access::{CoreOptions, CoreProcessMode, RuntimeError, RuntimeResult};
use hp2_engine::EngineOwner;
use hp2_launcher::boot::{
    Hp2LauncherBootLeaf as Leaf, Hp2LauncherBootServices, Hp2LauncherBootValue,
};
use std::{ffi::c_void, fs, io::Write, path::PathBuf, ptr};

#[cfg(test)]
mod unwind_tests;

"#;
const DECLARATION: &str = r#"static LOG_TO_STDOUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) struct BootHost {
    pub(super) args: Vec<String>,
    pub(super) data_root: Option<PathBuf>,
    pub(super) user_root: Option<PathBuf>,
    pub(super) system_ini: Option<PathBuf>,
    pub(super) user_ini: Option<PathBuf>,
    pub(super) override_root: Option<String>,
    pub(super) forwarded: String,
    pub(super) engine_command_line: String,
    // Reporters must detach before their owner is dropped during unwinding.
    pub(super) reporters: Option<crate::reporters::RuntimeReporters>,
    pub(super) owner: Option<EngineOwner>,
    pub(super) engine: *mut UGameEngine,
    pub(super) splash: Option<hp2_platform::launcher_splash::SplashHandle>,
    pub(super) is_client: bool,
    pub(super) lazy_load: bool,
    pub(super) is_started: bool,
    pub(super) is_guarded: bool,
    pub(super) exit_requested: bool,
    pub(super) fixed_delta: f32,
    pub(super) test_ticks: i32,
    pub(super) first_error: Option<RuntimeError>,
}

impl BootHost {"#;
const REPORTER: &str = "    pub(super) reporters: Option<crate::reporters::RuntimeReporters>,\n";
const OWNER: &str = "    pub(super) owner: Option<EngineOwner>,\n";

fn valid_source(source: &str) -> bool {
    // Exact shape intentionally blocks unreviewed syntax, comments and Drop changes.
    source
        .strip_prefix(PREFIX)
        .is_some_and(|rest| rest.starts_with(DECLARATION))
        && source.matches(DECLARATION).count() == 1
        && source.matches("BootHost").count() == 3
        && !source.contains("/*")
        && !source.contains("*/")
        && !source.contains("Drop")
        && source.matches(REPORTER).count() == 1
        && source.matches(OWNER).count() == 1
}

#[test]
fn boot_unwind_source_gate_negative_fixtures() {
    let source = include_str!("../boot_services.rs");
    assert!(valid_source(source), "baseline source shape must match");
    let both = format!("{REPORTER}{OWNER}");
    let cases = [
        (
            "reordered",
            source.replace(&both, &format!("{OWNER}{REPORTER}")),
        ),
        ("missing", source.replace(REPORTER, "")),
        (
            "duplicate",
            source.replace(REPORTER, &format!("{REPORTER}{REPORTER}")),
        ),
        (
            "comment fake",
            source.replace(REPORTER, &format!("    // {}", REPORTER.trim_start())),
        ),
        (
            "block comment fake",
            source.replace(REPORTER, &format!("    /*\n{REPORTER}    */\n")),
        ),
        (
            "field cfg",
            source.replace(REPORTER, &format!("    #[cfg(test)]\n{REPORTER}")),
        ),
        (
            "cfg attr",
            source.replace(
                REPORTER,
                &format!("    #[cfg_attr(test, allow(dead_code))]\n{REPORTER}"),
            ),
        ),
        (
            "struct cfg",
            source.replace(
                "pub(super) struct BootHost {",
                "#[cfg(test)]\npub(super) struct BootHost {",
            ),
        ),
        (
            "changed representation",
            source.replace(REPORTER, &REPORTER.replace("Option<", "Box<")),
        ),
        ("duplicate declaration", format!("{source}\n{DECLARATION}")),
        (
            "custom drop",
            format!("{source}\nimpl Drop for BootHost {{ fn drop(&mut self) {{}} }}\n"),
        ),
    ];
    for (name, candidate) in cases {
        assert_ne!(candidate, source, "fixture {name} must change source");
        assert!(!valid_source(&candidate), "fixture {name} must fail closed");
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ExpectedPanic {
    Empty,
    OwnerOnly,
    Attached,
}

fn expect_panic(result: std::thread::Result<()>, expected: ExpectedPanic) {
    match result {
        Err(payload) if payload.downcast_ref::<ExpectedPanic>() == Some(&expected) => {}
        Err(payload) => panic::resume_unwind(payload),
        Ok(()) => panic!("expected deliberate {expected:?} unwind"),
    }
}

#[test]
fn boot_unwind_real_owner_and_finish_contracts() {
    assert!(
        valid_source(include_str!("../boot_services.rs")),
        "unreviewed BootHost source shape; do not construct attached host"
    );
    let empty = panic::catch_unwind(|| {
        let _host = Box::new(BootHost::new(Vec::new()));
        panic::panic_any(ExpectedPanic::Empty);
    });
    expect_panic(empty, ExpectedPanic::Empty);
    let partial_root = fixture("partial");
    let partial = panic::catch_unwind(|| {
        let mut host = Box::new(BootHost::new(Vec::new()));
        host.owner = Some(owner(&partial_root));
        panic::panic_any(ExpectedPanic::OwnerOnly);
    });
    expect_panic(partial, ExpectedPanic::OwnerOnly);

    let root = fixture("attached");
    let observed = Rc::new(RefCell::new(Vec::new()));
    let caught = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let host = attached(&root);
        dirty(&host);
        assert_eq!(fs::read(root.join("System/Default.ini")).unwrap(), b"");
        probe(&host, &root, &observed);
        panic::panic_any(ExpectedPanic::Attached);
    }));
    expect_panic(caught, ExpectedPanic::Attached);
    assert_probe(&observed);
    assert_flushed(&root);

    let reuse = fixture("reuse");
    let mut host = attached(&reuse);
    let other_root = fixture("occupied");
    let other = owner(&other_root);
    // SAFETY: `other` remains live and unmoved; the already active context rejects attachment.
    let result = unsafe { crate::reporters::RuntimeReporters::new(&other as *const EngineOwner) };
    match result {
        Err(error) => assert_eq!(
            error.to_string(),
            "A canonical reporter owner is already attached"
        ),
        Ok(reporters) => {
            drop(reporters);
            panic!("a second active reporter must be rejected");
        }
    }
    other.shutdown().expect("shutdown rejected owner");
    dirty(&host);
    let observed = Rc::new(RefCell::new(Vec::new()));
    probe(&host, &reuse, &observed);
    assert!(!host.finish(), "normal finish succeeds");
    assert!(host.owner.is_none() && host.reporters.is_none());
    assert_probe(&observed);
    assert_flushed(&reuse);

    for (name, prior) in [("reporter-error", false), ("first-error", true)] {
        let root = fixture(name);
        let mut host = attached(&root);
        dirty(&host);
        let observed = Rc::new(RefCell::new(Vec::new()));
        probe(&host, &root, &observed);
        if prior {
            host.retain(RuntimeError::new("first failure"));
        }
        crate::reporters::with_owner((), |_| Err(RuntimeError::new("injected reporter failure")));
        assert!(host.finish(), "finish must report a retained error");
        assert_eq!(
            host.first_error.as_ref().unwrap().to_string(),
            if prior {
                "first failure"
            } else {
                "injected reporter failure"
            }
        );
        assert!(host.owner.is_none() && host.reporters.is_none());
        assert_probe(&observed);
        assert_flushed(&root);
    }
    let root = fixture("final-reuse");
    let mut host = attached(&root);
    assert!(
        !host.finish(),
        "reporters can attach again after error cleanup"
    );
}
