include!("console_ini_retail_restart.rs");

#[test]
fn retail_menu_numeric_enum_values_use_loaded_vm_and_survive_reload() {
    let data = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prepared retail data"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-retail-numeric-enum-{}", std::process::id()));
    copy_system(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(std::fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let (first, pawn, class) = unsafe { boot_restart_world(&root) };
    let access = first.access();
    for (selector, key, number) in [("ini:HGame.Harry", "ObjectDetail", 0u8), ("ini:HGame.Harry", "Difficulty", 2), ("ini:Engine.Engine.ViewportManager", "TextureDetail", 2)] {
        unsafe { real_console(access, pawn.cast(), &string_arg(&format!("SET {selector} {key} {number}"))); }
        if selector == "ini:HGame.Harry" {
            let name = access.intern_str(key, FindName::Find).unwrap();
            let property = unsafe { access.find_field(class.cast(), name, access.native_class("Core.Property").unwrap()).cast::<UProperty>() };
            let address = unsafe { access.property_address(pawn.cast(), property) }.unwrap();
            assert_eq!(unsafe { address.read() }, number, "live {key}");
        }
    }
    let before = std::fs::read(root.join("System/DefUser.ini")).unwrap();
    for value in ["255", "256", "0junk", "bogus"] {
        let command = wide(&format!("SET ini:HGame.Harry ObjectDetail {value}"));
        assert!(unsafe { access.execute_static_command(command.as_ptr(), &mut |_| Ok(())) }.is_err(), "accepted {value}");
        assert_eq!(std::fs::read(root.join("System/DefUser.ini")).unwrap(), before);
    }
    first.shutdown().unwrap();
    let (second, pawn, class) = unsafe { boot_restart_world(&root) };
    let access = second.access();
    for (key, expected) in [("ObjectDetail", 0u8), ("Difficulty", 2)] {
        let property = unsafe { access.find_field(class.cast(), access.intern_str(key, FindName::Find).unwrap(), access.native_class("Core.Property").unwrap()).cast::<UProperty>() };
        let address = unsafe { access.property_address(pawn.cast(), property) }.unwrap();
        assert_eq!(unsafe { address.read() }, expected, "after reload {key}");
        let child = unsafe { real_console(access, pawn.cast(), &string_arg(&format!("GET ini:HGame.Harry {key}"))) };
        let parent = unsafe { real_console(access, pawn.cast(), &string_arg(&format!("GET ini:Engine.PlayerPawn {key}"))) };
        assert_eq!(child, parent, "inherited globalconfig {key}");
    }
    second.shutdown().unwrap();
}
