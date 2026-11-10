//! Each fixture runs in a fresh process because bootstrap reads HOME and cwd.

use super::{run, runtime_configuration_paths};
use std::{
    fs::{self, File, FileTimes},
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

const CASE_ENV: &str = "HP2_RUNTIME_CONFIGURATION_CHILD";
const CASES: [&str; 7] = [
    "profile_and_active_files_are_separate",
    "quoted_relative_first_overrides",
    "absolute_system_override",
    "system_refresh_forces_explicit_user_refresh",
    "user_failure_keeps_earlier_system_copy",
    "empty_user_override_precedes_copy",
    "non_token_comma_and_nul_parse",
];
const DEFAULT_SYSTEM: &[u8] = b"[Sentinel]\nSource=system-default\n";
const DEFAULT_USER: &[u8] = b"[Sentinel]\nSource=user-default\n";
const ACTIVE_SYSTEM: &[u8] = b"[Sentinel]\nSource=active-system\n";
const ACTIVE_USER: &[u8] = b"[Sentinel]\nSource=active-user\n";

fn put(path: &Path, bytes: &[u8], seconds: u64) {
    fs::create_dir_all(path.parent().expect("fixture file has a parent")).unwrap();
    fs::write(path, bytes).unwrap();
    File::open(path)
        .unwrap()
        .set_times(
            FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
        )
        .unwrap();
}

fn fixture_user() -> PathBuf {
    let outcome = run(4, b"", b"", &[], |_, _| 0);
    assert!(outcome.success, "user lookup: {:?}", outcome.text);
    PathBuf::from(std::ffi::OsStr::from_bytes(&outcome.text[0]))
}

fn checked_child_root(case: &str) -> PathBuf {
    assert!(CASES.contains(&case), "unknown fixture case {case}");
    let supplied = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap());
    assert!(
        supplied.is_absolute(),
        "child artifact root must be absolute"
    );
    let root = fs::canonicalize(&supplied).expect("child artifact root must exist");
    assert_eq!(supplied, root, "parent must supply the canonical case root");
    assert_eq!(root.file_name().and_then(|s| s.to_str()), Some(case));
    let marker = root.join("fixture-authorization.txt");
    assert!(fs::symlink_metadata(&marker).unwrap().file_type().is_file());
    assert_eq!(
        fs::read_to_string(marker).unwrap(),
        format!("runtime-configuration-fixture-v1\ncase={case}\n")
    );
    for (key, sub) in [
        ("HOME", "home"),
        ("TMPDIR", "tmp"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_DATA_HOME", "xdg-data"),
    ] {
        let path = PathBuf::from(std::env::var_os(key).unwrap());
        assert!(path.is_absolute(), "child {key} must be absolute");
        let expected = root.join(sub);
        assert_eq!(fs::canonicalize(path).unwrap(), expected, "child {key}");
        assert!(
            fs::symlink_metadata(&expected)
                .unwrap()
                .file_type()
                .is_dir()
        );
        assert!(
            fs::read_dir(&expected).unwrap().next().is_none(),
            "child {key} must be empty"
        );
    }
    let cwd = root.join("cwd");
    assert_eq!(
        fs::canonicalize(std::env::current_dir().unwrap()).unwrap(),
        cwd
    );
    assert!(fs::symlink_metadata(&cwd).unwrap().file_type().is_dir());
    assert!(
        fs::read_dir(&cwd).unwrap().next().is_none(),
        "child cwd must be empty"
    );
    assert_eq!(
        fs::symlink_metadata(root.join("data")).unwrap_err().kind(),
        std::io::ErrorKind::NotFound,
        "fixture data must not exist before child validation"
    );
    root
}

fn child_case(case: &str) {
    // Validate the complete child environment before writing or consulting HOME.
    let root = checked_child_root(case);
    let data = root.join("data");
    put(
        &data.join("System/Default.ini"),
        DEFAULT_SYSTEM,
        1_000_000_000,
    );
    put(
        &data.join("System/DefUser.ini"),
        DEFAULT_USER,
        1_000_000_000,
    );
    let user = fixture_user();
    assert!(user.starts_with(&root), "fixture HOME escaped: {user:?}");
    let system = user.join("HP.ini");
    let user_ini = user.join("User.ini");
    let cwd = std::env::current_dir().unwrap();
    assert!(cwd.starts_with(&root), "fixture cwd escaped: {cwd:?}");

    match case {
        "profile_and_active_files_are_separate" => {
            let profile = user.join("Profiles/Retail");
            let profile_system = profile.join("Game.ini");
            let profile_user = profile.join("User.ini");
            put(&profile_system, b"profile-system", 2_000_000_000);
            put(&profile_user, b"profile-user", 2_000_000_000);
            put(&system, ACTIVE_SYSTEM, 2_000_000_000);
            put(&user_ini, ACTIVE_USER, 2_000_000_000);
            let mut published = None;
            let installed = run(
                3,
                data.as_os_str().as_bytes(),
                profile.as_os_str().as_bytes(),
                &[],
                |base, user| {
                    published = Some((base.to_vec(), user.to_vec()));
                    1
                },
            );
            assert!(
                installed.success,
                "profile installation: {:?}",
                installed.text
            );
            let (base, selected) = published.expect("installer must run");
            let mut expected_base = data.join("System").as_os_str().as_bytes().to_vec();
            expected_base.extend_from_slice(b"/\0");
            let mut expected_user = profile.as_os_str().as_bytes().to_vec();
            expected_user.extend_from_slice(b"/\0");
            assert_eq!(base, expected_base);
            assert_eq!(selected, expected_user);
            let paths = runtime_configuration_paths(&data, b"").unwrap();
            assert_eq!(paths.user_root, user);
            assert_eq!(paths.system_ini, system);
            assert_eq!(paths.user_ini, user_ini);
            assert_eq!(fs::read(&system).unwrap(), ACTIVE_SYSTEM);
            assert_eq!(fs::read(&user_ini).unwrap(), ACTIVE_USER);
            assert_eq!(fs::read(profile_system).unwrap(), b"profile-system");
            assert_eq!(fs::read(profile_user).unwrap(), b"profile-user");
        }
        "quoted_relative_first_overrides" => {
            let first = cwd.join("first system.ini");
            let chosen_user = cwd.join("chosen user.ini");
            put(&first, ACTIVE_SYSTEM, 2_000_000_000);
            put(&chosen_user, ACTIVE_USER, 2_000_000_000);
            let paths = runtime_configuration_paths(
                &data,
                b"-UsErInI=\"chosen user.ini\" -iNi=\"first system.ini\" -INI=later.ini",
            )
            .unwrap();
            assert_eq!(paths.system_ini, first);
            assert_eq!(paths.user_ini, chosen_user);
            assert_eq!(fs::read(&paths.system_ini).unwrap(), ACTIVE_SYSTEM);
            assert_eq!(fs::read(&paths.user_ini).unwrap(), ACTIVE_USER);
            assert!(!cwd.join("later.ini").exists());
            assert!(!system.exists());
            assert!(!user_ini.exists());
        }
        "absolute_system_override" => {
            let selected = root.join("explicit-system.ini");
            put(&selected, ACTIVE_SYSTEM, 2_000_000_000);
            put(&user_ini, ACTIVE_USER, 2_000_000_000);
            let command = format!("-INI={}", selected.display());
            let paths = runtime_configuration_paths(&data, command.as_bytes()).unwrap();
            assert_eq!(paths.system_ini, selected);
            assert_eq!(paths.user_ini, user_ini);
            assert_eq!(fs::read(&paths.system_ini).unwrap(), ACTIVE_SYSTEM);
            assert_eq!(fs::read(&paths.user_ini).unwrap(), ACTIVE_USER);
            assert!(!system.exists());
        }
        "system_refresh_forces_explicit_user_refresh" => {
            let selected_user = root.join("explicit-user.ini");
            put(&system, ACTIVE_SYSTEM, 500_000_000);
            put(&selected_user, ACTIVE_USER, 2_000_000_000);
            let command = format!("-USERINI={}", selected_user.display());
            let paths = runtime_configuration_paths(&data, command.as_bytes()).unwrap();
            assert_eq!(paths.system_ini, system);
            assert_eq!(paths.user_ini, selected_user);
            assert_eq!(fs::read(&system).unwrap(), DEFAULT_SYSTEM);
            assert_eq!(fs::read(&paths.user_ini).unwrap(), DEFAULT_USER);
            assert!(!user_ini.exists());
        }
        "user_failure_keeps_earlier_system_copy" => {
            let missing = root.join("absent-parent/User.ini");
            let command = format!("-USERINI={}", missing.display());
            let error = runtime_configuration_paths(&data, command.as_bytes()).unwrap_err();
            assert!(
                error.starts_with(&format!(
                    "failed to initialize configuration '{}': ",
                    missing.display()
                )),
                "unexpected partial-copy error: {error}"
            );
            assert_eq!(fs::read(&system).unwrap(), DEFAULT_SYSTEM);
            assert!(!missing.exists());
            assert!(!missing.parent().unwrap().exists());
        }
        "empty_user_override_precedes_copy" => {
            let error = runtime_configuration_paths(&data, b"-USERINI=").unwrap_err();
            assert_eq!(error, "empty USERINI= command-line value");
            assert!(!system.exists());
            assert!(!user_ini.exists());
        }
        "non_token_comma_and_nul_parse" => {
            let first = cwd.join("first.ini");
            put(&first, ACTIVE_SYSTEM, 2_000_000_000);
            put(&user_ini, ACTIVE_USER, 2_000_000_000);
            let paths = runtime_configuration_paths(
                &data,
                b"anything-INI=first.ini,ignored\0 -INI=later.ini",
            )
            .unwrap();
            assert_eq!(paths.system_ini, first);
            assert_eq!(paths.user_ini, user_ini);
            assert_eq!(fs::read(&paths.system_ini).unwrap(), ACTIVE_SYSTEM);
            assert!(!cwd.join("later.ini").exists());
        }
        _ => unreachable!("case was checked before creating files"),
    }
    fs::write(
        root.join("result.txt"),
        format!("case={case}\nstatus=passed\n"),
    )
    .unwrap();
}

#[test]
fn isolated_active_ini_cases() {
    let _serial = super::tests::serial();
    if let Some(case) = std::env::var_os(CASE_ENV) {
        child_case(case.to_str().expect("fixture case must be UTF-8"));
        return;
    }
    let artifacts = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("isolated bootstrap test needs HP2_ARTIFACT_DIR"),
    );
    assert!(artifacts.is_absolute(), "HP2_ARTIFACT_DIR must be absolute");
    let artifacts = fs::canonicalize(artifacts).expect("HP2_ARTIFACT_DIR must exist");
    let root = artifacts.join(format!("runtime-configuration-{}", std::process::id()));
    // Private fresh directories prevent unrelated users from changing the fixture.
    // Path checks do not defend against a concurrent hostile writer as this user.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .expect("fixture directory must be new");
    for case in CASES {
        let case_root = root.join(case);
        for sub in ["home", "cwd", "tmp", "config", "cache", "xdg-data"] {
            fs::create_dir_all(case_root.join(sub)).unwrap();
        }
        fs::write(
            case_root.join("fixture-authorization.txt"),
            format!("runtime-configuration-fixture-v1\ncase={case}\n"),
        )
        .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bootstrap_paths::runtime_configuration_contracts::isolated_active_ini_cases",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env(CASE_ENV, case)
            .env("HP2_ARTIFACT_DIR", &case_root)
            .env("HOME", case_root.join("home"))
            .env("TMPDIR", case_root.join("tmp"))
            .env("XDG_CONFIG_HOME", case_root.join("config"))
            .env("XDG_CACHE_HOME", case_root.join("cache"))
            .env("XDG_DATA_HOME", case_root.join("xdg-data"))
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .current_dir(case_root.join("cwd"))
            .output()
            .unwrap();
        fs::write(case_root.join("child.stdout"), &output.stdout).unwrap();
        fs::write(case_root.join("child.stderr"), &output.stderr).unwrap();
        assert!(output.status.success(), "case {case}: {output:?}");
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
            "case {case} did not run its exact child test: {stdout}"
        );
        assert_eq!(
            fs::read_to_string(case_root.join("result.txt")).unwrap(),
            format!("case={case}\nstatus=passed\n")
        );
    }
}
