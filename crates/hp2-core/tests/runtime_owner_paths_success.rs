//! Actual `CoreOwner` + `unix_identity` path publication with synthetic files.
//!
//! One test per integration binary because the base/user directories are
//! process-global. This is not the private game `BootHost` callback; it mirrors
//! its order (base, then user) with the same public setters. All data is
//! synthetic and lives under a fresh directory below `HP2_ARTIFACT_DIR`.
use hp2_core::ansi::Tchar;
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, CoreProcessMode, RuntimeResult};
use hp2_core::lit;
use hp2_core::unix_identity;
use hp2_core::unix_text::{self, Hp2UnixTextFailure, Hp2UnixTextServices};
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};

unsafe extern "C" fn allocate(_: *mut c_void, size: usize, out: *mut *mut c_void) -> i32 {
    // SAFETY: the callback contract supplies a valid out pointer.
    unsafe { out.write(libc::malloc(size.max(1))) };
    0
}
unsafe extern "C" fn fatal(_: *mut c_void, _: Hp2UnixTextFailure) -> i32 {
    1
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn fresh_root() -> PathBuf {
    let artifacts = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    assert!(artifacts.is_absolute() && artifacts.is_dir(), "artifact dir must be an existing absolute directory");
    let root = artifacts.join(format!("runtime-owner-paths-success-{}", std::process::id()));
    fs::create_dir(&root).expect("fresh fixture root must not already exist");
    fs::canonicalize(root).expect("canonical fixture root")
}

fn publish(path: &Path, set: unsafe fn(&Hp2UnixTextServices, *const Tchar) -> Result<(), i32>) {
    let services = Hp2UnixTextServices { opaque: std::ptr::null_mut(), allocate, fatal };
    let mut bytes = path.to_str().expect("utf-8 fixture path").as_bytes().to_vec();
    bytes.push(0);
    // SAFETY: `bytes` is NUL-terminated; the services use malloc/free.
    let text = unsafe { unix_text::from_utf8(&services, bytes.as_ptr()) }.expect("decode path");
    // SAFETY: `text` is a NUL-terminated TCHAR allocation.
    unsafe { set(&services, text) }.expect("publish path");
    // SAFETY: `text` came from the callback's malloc.
    unsafe { libc::free(text.cast()) };
}

fn read_user_dir() -> String {
    let mut pointer = unix_identity::user_dir();
    let mut text = String::new();
    // SAFETY: published roots are NUL-terminated process-lifetime allocations.
    unsafe {
        while *pointer != 0 {
            text.push(char::from_u32(*pointer as u32).expect("scalar"));
            pointer = pointer.add(1);
        }
    }
    text
}

fn trimmed(value: Option<Vec<u32>>) -> String {
    let value = value.expect("configured value");
    value.into_iter().take_while(|unit| *unit != 0).map(|u| char::from_u32(u).unwrap()).collect()
}

#[test]
fn global_profile_save_root_is_separate_from_selected_core_ini_files() {
    let root = fresh_root();
    let data = root.join("data");
    let profile = root.join("profile");
    let base = root.join("base");
    for directory in [data.join("System"), profile.clone(), base.clone()] {
        fs::create_dir_all(directory).unwrap();
    }
    let files = [
        (data.join("System/Default.ini"), "[Core.System]\n".to_string()),
        (data.join("System/DefUser.ini"), String::new()),
        (profile.join("Game.ini"), "[Sentinel]\nWho=profile-game\n".to_string()),
        (profile.join("User.ini"), "[Sentinel]\nWho=profile-user\n".to_string()),
        (
            base.join("HP.ini"),
            "[Core.System]\nPaths=../System/*.u\nPaths=../System/*.u\nPaths=../System/*.u\nPaths=../System/*.u\nPaths=../System/*.u\nPaths=../Maps/*.unr\n[Sentinel]\nWho=base-system\n".to_string(),
        ),
        (base.join("User.ini"), "[Sentinel]\nWho=base-user\n".to_string()),
    ];
    for (path, content) in &files {
        fs::write(path, content).unwrap();
    }
    let profile_before: Vec<_> = ["Game.ini", "User.ini"].iter().map(|n| fs::read(profile.join(n)).unwrap()).collect();
    let base_user_before = fs::read(base.join("User.ini")).unwrap();

    // Same order as the game's publisher: base directory, then user directory.
    let system_dir = data.join("System/");
    publish(&system_dir, unix_identity::set_base_dir);
    publish(&profile.join(""), unix_identity::set_user_dir);
    let expected_user = format!("{}/", profile.display());
    assert_eq!(read_user_dir(), expected_user);

    let owner = CoreOwner::new(CoreOptions {
        data_root: data.clone(),
        system_ini: base.join("HP.ini"),
        user_ini: base.join("User.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("Core owner");
    owner.access().set_process_mode(CoreProcessMode { is_client: true, ..CoreProcessMode::default() });
    let access = owner.access();
    access.initialize_application_system().expect("application system");

    // Selected Core INI files are the explicit base files, not the profile files.
    assert_eq!(trimmed(access.config_get_units_from(Some(lit!("User")), lit!("Sentinel"), lit!("Who")).unwrap()), "base-user");
    assert_eq!(trimmed(access.config_get_units(lit!("Sentinel"), lit!("Who")).unwrap()), "base-system");
    // Save policy follows the published global user directory.
    let save = profile.join("Save");
    assert!(save.is_dir(), "profile Save directory must be created");
    assert!(!base.join("Save").exists(), "base User must not receive Save");
    assert_eq!(trimmed(access.config_get_units(lit!("Core.System"), lit!("SavePath")).unwrap()), format!("{}", save.display()));

    owner.shutdown().expect("shutdown");
    for (name, before) in ["Game.ini", "User.ini"].iter().zip(&profile_before) {
        assert_eq!(&fs::read(profile.join(name)).unwrap(), before, "profile {name} must be unchanged");
    }
    assert_eq!(fs::read(base.join("User.ini")).unwrap(), base_user_before);
    let flushed = fs::read_to_string(base.join("HP.ini")).unwrap();
    assert!(flushed.contains("SavePath="), "system INI receives the flushed SavePath: {flushed}");
}
