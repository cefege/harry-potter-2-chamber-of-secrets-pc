use hp2_engine::binding_config::{BindingConfig, ConfigBindingError};
use hp2_core::CoreConfig;
use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn setup(contents: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("CTest must provide HP2_ARTIFACT_DIR"))
        .join(format!("binding-config-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&root).unwrap();
    let user = root.join("User.ini");
    fs::write(&user, contents).unwrap();
    (root, user)
}
fn open(root: &std::path::Path, user: &std::path::Path) -> BindingConfig {
    BindingConfig::open(root, &root.join("Game.ini"), user, &["A", "B", "Hidden"]).unwrap()
}
fn w(s: &str) -> Vec<u32> { s.chars().map(u32::from).collect() }

#[test]
fn edits_compound_binding_and_persists_without_losing_hidden_values() {
    let (root, user) = setup("[Engine.Input]\nA=Button Jump|Say \"A|B\"|Button JumpHigher\nHidden=Axis aForward Speed=2\nAliases[0]=(Command=\"foo\",Alias=Bar)\n[Other]\nValue=Keep\n");
    let mut adapter = open(&root, &user);
    assert!(adapter.edit("A", &["Button Jump"], &["Button Duck"]).unwrap());
    assert_eq!(adapter.read("A").unwrap(), "Say \"A|B\"|Button JumpHigher|Button Duck");
    adapter.commit().unwrap();
    let mut reload = open(&root, &user);
    assert_eq!(reload.read("a").unwrap(), "Say \"A|B\"|Button JumpHigher|Button Duck");
    assert_eq!(reload.read("Hidden").unwrap(), "Axis aForward Speed=2");
    let mut core = CoreConfig::new(root.clone(), root.join("Game.ini"), user);
    assert_eq!(core.get(Some(&w("User.ini")), &w("Engine.Input"), &w("Aliases[0]")).unwrap(), Some(w("(Command=\"foo\",Alias=Bar)")));
    assert_eq!(core.get(Some(&w("User.ini")), &w("Other"), &w("Value")).unwrap(), Some(w("Keep")));
}

#[test]
fn rejects_missing_duplicate_unapproved_and_unsupported_without_changes() {
    let original = "[Engine.Input]\nA=Jump|Other\nA=Jump|Custom\nHidden=\"unclosed\n";
    let (root, user) = setup(original);
    let mut adapter = open(&root, &user);
    assert!(matches!(adapter.edit("A", &["Jump"], &[]), Err(ConfigBindingError::MissingOrDuplicateKey)));
    assert!(matches!(adapter.edit("B", &[], &["Jump"]), Err(ConfigBindingError::MissingOrDuplicateKey)));
    assert!(matches!(adapter.read("Aliases[0]"), Err(ConfigBindingError::InvalidKey)));
    assert!(matches!(adapter.edit("Hidden", &["Jump"], &[]), Err(ConfigBindingError::Binding(_))));
    adapter.commit().unwrap();
    assert_eq!(fs::read_to_string(user).unwrap(), original);
}

#[test]
fn external_change_fails_instead_of_overwriting() {
    let (root, user) = setup("[Engine.Input]\nA=Jump|Other\n");
    let mut adapter = open(&root, &user);
    adapter.edit("A", &["Jump"], &["Duck"]).unwrap();
    let external = "[Engine.Input]\nA=External\n";
    fs::write(&user, external).unwrap();
    assert!(matches!(adapter.commit(), Err(ConfigBindingError::Config(_))));
    assert_eq!(fs::read_to_string(user).unwrap(), external);
}

#[test]
fn missing_user_file_and_noncanonical_key_are_rejected() {
    let (root, user) = setup("[Engine.Input]\nA=Jump\n");
    assert!(matches!(BindingConfig::open(&root, &root.join("Game.ini"), &root.join("missing.ini"), &["A"]), Err(ConfigBindingError::InvalidPath)));
    assert!(matches!(BindingConfig::open(&root, &root.join("Game.ini"), &user, &["Aliases[0]"]), Err(ConfigBindingError::InvalidKey)));
}
