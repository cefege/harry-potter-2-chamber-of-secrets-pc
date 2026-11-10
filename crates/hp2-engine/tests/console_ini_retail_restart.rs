// Reuse the real package VM fixture and loaded ConsoleCommand helper.
include!("console_ini_retail_selectors.rs");

unsafe fn boot_restart_world(root: &std::path::Path) -> (EngineOwner, *mut APlayerPawn, *mut hp2_abi::engine::generated::UClass) {
    let owner = EngineOwner::new(CoreOptions { data_root: root.to_path_buf(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: false, is_scriptable: true, lazy_load: false });
    unsafe {
        for package in ["Engine.u", "HGame.u"] { access.load_package(&root.join("System").join(package), 1).unwrap(); }
        let class = access.static_load_class(access.native_class("Engine.PlayerPawn").unwrap(), ptr::null_mut(), &wide("HGame.Harry"), 2).unwrap();
        assert!(!class.is_null());
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<UGameEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine.cast::<UEngine>()).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let name = access.intern_str("RestartHarry", FindName::Add).unwrap();
        let pawn = access.construct_object(class, level.cast(), name, 0, ptr::null_mut()).unwrap().cast::<APlayerPawn>();
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &[info.cast::<AActor>(), pawn.cast()]);
        for actor in [info.cast::<AActor>(), pawn.cast()] { (*actor).x_level = level.cast(); (*actor).level = info.cast(); }
        (*engine).g_level = level.cast();
        let client = construct(access, "SDLDrv.SDLClient", engine.cast());
        (*engine.cast::<UEngine>()).client = client.cast();
        let viewport = construct(access, "SDLDrv.SDLViewport", client).cast::<UViewport>();
        (*viewport).actor = pawn.cast(); (*pawn).player = viewport.cast();
        let console = construct(access, "Engine.Console", client).cast::<UConsole>();
        (*console).viewport = viewport.cast(); (*viewport).console = console.cast();
        (owner, pawn, class)
    }
}

#[test]
fn all_six_retail_menu_controls_survive_new_owner_and_package_reload() {
    let data = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("verified prepared retail data root"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-retail-restart-{}", std::process::id()));
    copy_system(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(std::fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let controls = [("ini:HGame.Harry", "bAutoCenterCamera", "False"), ("ini:HGame.Harry", "bAutoQuaff", "False"),
        ("ini:HGame.Harry", "bMoveWhileCasting", "True"), ("ini:HGame.Harry", "ObjectDetail", "ObjectDetailLow"),
        ("ini:HGame.Harry", "Difficulty", "DifficultyHard"), ("ini:Engine.PlayerPawn", "bInvertMouse", "True")];
    let (first, pawn, _) = unsafe { boot_restart_world(&root) };
    let access = first.access();
    assert!(access.config_get_units_from(Some(&wide("System")), &wide("HGame"), &wide("Harry")).unwrap().is_none());
    assert!(access.config_get_units_from(Some(&wide("System")), &wide("Engine"), &wide("PlayerPawn")).unwrap().is_none());
    for (selector, key, value) in controls {
        unsafe { real_console(access, pawn.cast(), &string_arg(&format!("SET {selector} {key} {value}"))) };
        let got = unsafe { real_console(access, pawn.cast(), &string_arg(&format!("GET {selector} {key}"))) };
        assert!(got.eq_ignore_ascii_case(value), "before restart {key}: {got}");
    }
    first.shutdown().unwrap();
    let (second, pawn, harry) = unsafe { boot_restart_world(&root) };
    let access = second.access();
    for (selector, key, value) in controls {
        let got = unsafe { real_console(access, pawn.cast(), &string_arg(&format!("GET {selector} {key}"))) };
        assert!(got.eq_ignore_ascii_case(value), "after restart {key}: {got}");
        let name = access.intern_str(key, FindName::Find).unwrap();
        let property = unsafe { access.find_field(harry.cast(), name, access.native_class("Core.Property").unwrap()).cast::<UProperty>() };
        assert!(!property.is_null());
        let address = unsafe { access.property_address(pawn.cast(), property) }.unwrap();
        match key {
            "ObjectDetail" => assert_eq!(unsafe { address.read() }, 3, "live {key}"),
            "Difficulty" => assert_eq!(unsafe { address.read() }, 2, "live {key}"),
            _ => {
                let mask = unsafe { (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as u32 };
                assert_eq!(unsafe { address.cast::<u32>().read_unaligned() } & mask != 0, value == "True", "live {key}");
            }
        }
    }
    second.shutdown().unwrap();
}
