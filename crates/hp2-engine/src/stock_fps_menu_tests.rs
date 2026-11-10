use super::*;
use hp2_core::core_access::{CoreOptions, CoreProcessMode};
use std::{fs, path::{Path, PathBuf}};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
#[test]
fn checkbox_bits_preserve_neighboring_flags() {
    let mut bits: u32 = 0b1010;
    let object = (&mut bits as *mut u32).cast::<UObject>();
    unsafe { write_bool(object, (0, 1), true); }
    assert_eq!(bits, 0b1011);
    assert!(unsafe { read_bool(object, (0, 1)) });
    unsafe { write_bool(object, (0, 1), false); }
    assert_eq!(bits, 0b1010);
}
#[test]
fn fps_tab_moves_past_labels_without_reordering_stock_controls() {
    let order = [1, 2, 3, 4, 5, 99];
    let fixed = tab_order(&order, 99, |id| id == 3 || id == 5).unwrap();
    assert_eq!(fixed, [1, 2, 99, 3, 4, 5]);
    assert_eq!(fixed.iter().copied().filter(|&v| v != 99).collect::<Vec<_>>(), order[..5]);
    assert_eq!(tab_order(&[99, 3, 1], 99, |id| id == 3), Some(vec![99, 3, 1]));
    assert_eq!(tab_order(&order, 99, |_| false), None);
    assert_eq!(tab_order(&[99, 1, 99], 99, |_| true), None);
}
fn config_fixture(name: &str) -> (PathBuf, hp2_core::core_access::CoreOwner) {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"))
        .join(format!("stock-fps-{name}-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("Game.ini"), b"[SDLDrv.SDLClient]\nShowFPS=False\nOther=keep\n").unwrap();
    let owner = hp2_core::core_access::CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("Game.ini"), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
    (root, owner)
}
#[test]
fn checkbox_change_saves_then_updates_live_client() {
    let (root, owner) = config_fixture("save");
    let mut client: UClient = unsafe { std::mem::zeroed() };
    let mut bits = 1u32;
    let control = (&mut bits as *mut u32).cast();
    unsafe { commit_selection(owner.access(), &mut client, control, (0, 1)).unwrap(); }
    assert!(client.show_fps());
    assert_eq!(bits, 1);
    assert!(fs::read_to_string(root.join("Game.ini")).unwrap().contains("ShowFPS=True"));
    owner.shutdown().unwrap();
}
#[test]
fn save_conflict_restores_checkbox_and_keeps_live_and_file() {
    let (root, owner) = config_fixture("conflict");
    let mut client: UClient = unsafe { std::mem::zeroed() };
    settings_config::read(owner.access()).unwrap();
    let external = b"[SDLDrv.SDLClient]\nShowFPS=False\nOther=external\n";
    fs::write(root.join("Game.ini"), external).unwrap();
    let mut bits = 1u32;
    let control = (&mut bits as *mut u32).cast();
    assert!(unsafe { commit_selection(owner.access(), &mut client, control, (0, 1)) }.is_err());
    assert!(!client.show_fps());
    assert_eq!(bits, 0);
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
    owner.shutdown().unwrap();
}
#[test]
fn checkbox_disable_persists_and_survives_config_restart() {
    let (root, first) = config_fixture("restart");
    let mut client: UClient = unsafe { std::mem::zeroed() };
    let mut bits = 1u32;
    unsafe { commit_selection(first.access(), &mut client, (&mut bits as *mut u32).cast(), (0, 1)).unwrap(); }
    first.shutdown().unwrap();
    let reopen = || hp2_core::core_access::CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("Game.ini"), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
    let second = reopen();
    let value = |owner: &hp2_core::core_access::CoreOwner| settings_config::read(owner.access()).unwrap().values.into_iter()
        .find(|(key, _)| key.section == "SDLDrv.SDLClient" && key.key == "ShowFPS")
        .and_then(|(_, value)| value).unwrap();
    assert_eq!(value(&second), "True");
    bits = 0;
    unsafe { commit_selection(second.access(), &mut client, (&mut bits as *mut u32).cast(), (0, 1)).unwrap(); }
    assert!(!client.show_fps());
    second.shutdown().unwrap();
    let third = reopen();
    assert_eq!(value(&third), "False");
    third.shutdown().unwrap();
}
#[test]
fn failed_disable_restores_the_checked_state() {
    let (root, owner) = config_fixture("conflict-disable");
    let mut client: UClient = unsafe { std::mem::zeroed() };
    client.set_show_fps(true);
    settings_config::read(owner.access()).unwrap();
    let external = b"[SDLDrv.SDLClient]\nShowFPS=True\nOther=external\n";
    fs::write(root.join("Game.ini"), external).unwrap();
    let mut bits = 0b100u32;
    assert!(unsafe { commit_selection(owner.access(), &mut client, (&mut bits as *mut u32).cast(), (0, 1)) }.is_err());
    assert!(client.show_fps());
    assert_eq!(bits, 0b101);
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
    owner.shutdown().unwrap();
}
#[test]
fn fps_label_uses_selected_language_and_international_fallback() {
    let (root, owner) = config_fixture("localization");
    fs::create_dir(root.join("System")).unwrap();
    fs::write(root.join("System/HP2Menu.int"), "[StockMenu]\nShowFPS=International FPS\n").unwrap();
    fs::write(root.join("System/HP2Menu.fra"), "[StockMenu]\nShowFPS=FPS francais\n").unwrap();
    owner.access().set_language(hp2_core::lit!("fra")).unwrap();
    assert_eq!(label(owner.access()).unwrap(), hp2_core::lit!("FPS francais"));
    owner.access().set_language(hp2_core::lit!("deu")).unwrap();
    assert_eq!(label(owner.access()).unwrap(), hp2_core::lit!("International FPS"));
    owner.shutdown().unwrap();
}
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() { copy_tree(&entry.path(), &target); }
        else { fs::copy(entry.path(), target).unwrap(); }
    }
}
#[test]
#[ignore = "requires retail data and HP2_ARTIFACT_DIR"]
fn retail_layout_supports_adapter() {
    let source = PathBuf::from(std::env::var_os("HP2_TEST_DATA_ROOT").unwrap());
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("stock-fps-layout-{}", std::process::id()));
    assert!(!root.exists());
    copy_tree(&source.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let path = source.join(dir);
        if path.is_dir() { std::os::unix::fs::symlink(fs::canonicalize(path).unwrap(), root.join(dir)).unwrap(); }
    }
    let owner = crate::EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: vec![], output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
    unsafe { access.load_package(&root.join("System/Engine.u"), 1).unwrap(); access.load_package(&root.join("System/HGame.u"), 1).unwrap(); }
    let b = unsafe { resolve(access) }.unwrap();
    assert_eq!(label(access).unwrap(), hp2_core::lit!("Show FPS"));
    let guard = unsafe { install(access) }.expect("hook installed");
    drop(guard);
    // Use the real retail reflected classes and field offsets. The stock ring
    // starts with a non-focusable label and ends with the added checkbox.
    let make = |class| unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let page = make(b.page);
    let label_control = make(b.label);
    let stock = make(b.checkbox);
    let fps = make(b.checkbox);
    let set_ptr = |object: *mut UObject, offset, value: *mut UObject| unsafe { ptr::write_unaligned(object.cast::<u8>().add(offset).cast::<*mut UObject>(), value) };
    let get_ptr = |object: *mut UObject, offset| unsafe { ptr::read_unaligned(object.cast::<u8>().add(offset).cast::<*mut UObject>()) };
    let ring = [label_control, stock, fps];
    for (i, &node) in ring.iter().enumerate() {
        set_ptr(node, b.notify_window, page);
        set_ptr(node, b.tab_prev, ring[(i + ring.len() - 1) % ring.len()]);
        set_ptr(node, b.tab_next, ring[(i + 1) % ring.len()]);
        unsafe { write_bool(node, b.visible, true); }
    }
    unsafe { write_bool(label_control, b.accepts_focus, false); write_bool(stock, b.accepts_focus, true); }
    set_ptr(page, b.tab_last, fps);
    unsafe { fix_tab_chain(access, b, page, fps).unwrap(); }
    assert_eq!(get_ptr(page, b.tab_last), stock);
    assert_eq!(get_ptr(label_control, b.tab_next), fps);
    assert_eq!(get_ptr(fps, b.tab_prev), label_control);
    assert_eq!(get_ptr(fps, b.tab_next), stock);
    assert_eq!(get_ptr(stock, b.tab_prev), fps);
    assert_eq!(get_ptr(stock, b.tab_next), label_control);
}
