// Exercise the same native PlayerPawn.ConsoleCommand VM seam as the menu.
include!("console_vm_native_owner_contracts.rs");

fn copy_system(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() { copy_system(&entry.path(), &target.join(entry.file_name())); }
        else { std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap(); }
    }
}
#[test]
fn loaded_console_function_preserves_inherited_reflection() {
    let data = std::env::var_os("HP2_CORE_DATA_ROOT").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-hgame-loaded-get-{}", std::process::id()));
    copy_system(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(std::fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let system = root.join("System/Default.ini");
    let mut ini = std::fs::read_to_string(&system).unwrap();
    // Selector entries are isolated test fixtures, not retail-data claims.
    ini.push_str("\n[HGame]\nHarry=HGame.Harry\n[Engine]\nPlayerPawn=Engine.PlayerPawn\n");
    std::fs::write(&system, ini).unwrap();
    let user = root.join("System/DefUser.ini");
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: system, user_ini: user.clone(), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: false, is_scriptable: true, lazy_load: false });
    unsafe {
        for package in ["Engine.u", "HGame.u"] { access.load_package(&root.join("System").join(package), 1).unwrap(); }
        let harry_class = access.static_load_class(access.native_class("Engine.PlayerPawn").unwrap(), ptr::null_mut(), &wide("HGame.Harry"), 2).unwrap();
        assert!(!harry_class.is_null());
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<UGameEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine.cast::<UEngine>()).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let name = access.intern_str("VmRouteHarry", FindName::Add).unwrap();
        let pawn = access.construct_object(harry_class, level.cast(), name, 0, ptr::null_mut()).unwrap().cast::<APlayerPawn>();
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &[info.cast::<AActor>(), pawn.cast()]);
        for actor in [info.cast::<AActor>(), pawn.cast()] { (*actor).x_level = level.cast(); (*actor).level = info.cast(); }
        (*engine).g_level = level.cast();
        let client = construct(access, "SDLDrv.SDLClient", engine.cast());
        (*engine.cast::<UEngine>()).client = client.cast();
        let viewport = construct(access, "SDLDrv.SDLViewport", client).cast::<UViewport>();
        (*viewport).actor = pawn.cast(); (*pawn).player = viewport.cast();
        let console = construct(access, "Engine.Console", client).cast::<UConsole>();
        (*console).viewport = viewport.cast(); (*viewport).console = console.cast();
        let direct = real_console(access, pawn.cast(), &string_arg("GET ini:HGame.Harry ObjectDetail"));
        assert_eq!(direct, "ObjectDetailVeryHigh");
        let name = access.intern_str("ObjectDetail", FindName::Find).unwrap();
        assert!(!access.find_field(harry_class.cast(), name, access.native_class("Core.Property").unwrap()).is_null());
    }
    owner.shutdown().unwrap();
}

// Reuse the loaded package function; constructing a duplicate ConsoleCommand
// under PlayerPawn would replace the real UFunction and damage its field chain.
unsafe fn real_console(access: CoreAccess<'_>, object: *mut UObject, args: &[u8]) -> String {
    unsafe {
        let class = access.native_class("Engine.PlayerPawn").unwrap();
        let name = access.intern_str("ConsoleCommand", FindName::Find).unwrap();
        let function = access.find_field(class.cast(), name, access.native_class("Core.Function").unwrap()).cast::<UFunction>();
        assert!(!function.is_null(), "real loaded PlayerPawn.ConsoleCommand");
        let caller = construct(access, "Core.Function", class.cast()).cast::<UStruct>();
        let mut code = vec![0x1c];
        code.extend((*function.cast::<UObject>()).index.to_le_bytes());
        code.extend(args); code.push(0x16);
        set_array(ptr::addr_of_mut!((*caller).script).cast(), &code);
        let start = (*caller).script.data.cast::<u8>();
        let mut frame = FFrame { node: caller.cast(), object: object.cast(), code: start, locals: ptr::null_mut(), parent: ptr::null_mut() };
        let mut result=TArrayRaw { data:ptr::null_mut(), num:0, max:0 };
        access.execute_state_frame_step(&mut frame, (&mut result as *mut TArrayRaw).cast()).unwrap();
        assert_eq!(frame.code, start.add(code.len()));
        let text = hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(&result)).iter().filter_map(|&unit| char::from_u32(unit)).collect();
        access.destroy_owned_fstring(&mut result).unwrap();
        text
    }
}
