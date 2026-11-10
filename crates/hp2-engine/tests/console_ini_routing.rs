// Reuse the real VM-call and canonical world construction helpers.
include!("console_vm_native_owner_contracts.rs");

#[test]
fn player_console_routes_ini_audio_and_viewport_properties() {
    use hp2_abi::engine::generated::UClient;
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join("console-ini-engine-routing");
    std::fs::create_dir_all(root.join("System")).unwrap();
    let system = root.join("System/Default.ini");
    std::fs::write(&system, "[Engine.Engine]\nCanvas=Engine.Canvas\nInput=Engine.Input\nAudioDevice=ALAudio.ALAudioSubsystem\nViewportManager=SDLDrv.SDLClient\n").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: system.clone(),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_scriptable: true, ..access.process_mode() });
    unsafe {
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<UGameEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine.cast::<UEngine>()).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let pawn = construct(access, "Engine.PlayerPawn", level.cast()).cast::<APlayerPawn>();
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &[info.cast::<AActor>(), pawn.cast()]);
        for actor in [info.cast::<AActor>(), pawn.cast()] { (*actor).x_level = level.cast(); (*actor).level = info.cast(); }
        (*engine).g_level = level.cast();
        let client = construct(access, "SDLDrv.SDLClient", engine.cast()).cast::<UClient>();
        (*engine.cast::<UEngine>()).client = client.cast();
        access.construct_object(access.native_class("Core.Enum").unwrap(), access.native_class("Engine.Actor").unwrap().cast(), access.intern_str("EInputKey", FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap();
        let viewport = construct(access, "SDLDrv.SDLViewport", client.cast()).cast::<UViewport>();
        (*viewport).actor = pawn.cast(); (*pawn).player = viewport.cast();
        let console = construct(access, "Engine.Console", client.cast()).cast::<UConsole>();
        (*console).viewport = viewport.cast(); (*viewport).console = console.cast();
        let audio = construct(access, "ALAudio.ALAudioSubsystem", engine.cast()).cast::<hp2_abi::engine::generated::UALAudioSubsystem>();
        for (configured, direct) in [
            ("GET ini:Engine.Engine.AudioDevice MusicVolume", "GET ALAudio.ALAudioSubsystem MusicVolume"),
            ("GET ini:Engine.Engine.ViewportManager Brightness", "GET SDLDrv.SDLClient Brightness"),
        ] {
            let value = string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg(configured));
            let expected = string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg(direct));
            value.parse::<f32>().unwrap();
            assert_eq!(value, expected);
        }
        // A property outside the scoped transactional set retains the prior
        // default-only SET behavior instead of becoming an unsupported error.
        string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("SET ini:Engine.Engine.AudioDevice UseEAX True"));
        let eax = string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("GET ini:Engine.Engine.AudioDevice UseEAX"));
        assert_eq!(eax.to_ascii_lowercase(), "true");
        assert!(!std::fs::read_to_string(&system).unwrap().contains("UseEAX="));
        access.config_set_units_from(Some(&wide("User")), &wide("Other"), &wide("Pending"), &wide("not-yet-published")).unwrap();
        string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("SET ini:Engine.Engine.AudioDevice MusicVolume 0.375"));
        assert_eq!(std::fs::read_to_string(root.join("System/DefUser.ini")).unwrap(), "");
        assert_eq!((*audio).music_volume, 0.375);
        string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("SET ini:Engine.Engine.AudioDevice SoundVolume 0.75"));
        assert_eq!((*audio).sound_volume, 0.75);
        string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("SET ini:Engine.Engine.ViewportManager Brightness 0.625"));
        assert_eq!((*client).brightness, 0.625);
        string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("SET ini:Engine.Engine.ViewportManager TextureDetail Low"));
        assert_eq!((*client).texture_lod_set[1], 2);
        let detail = string_result(access, pawn.cast(), "Engine.PlayerPawn", "ConsoleCommand", &string_arg("GET ini:Engine.Engine.ViewportManager TextureDetail"));
        assert_eq!(detail, "Low");
        let saved = std::fs::read_to_string(&system).unwrap();
        assert!(saved.contains("MusicVolume=0.375"), "{saved}");
        assert!(saved.contains("SoundVolume=0.75"), "{saved}");
        assert!(saved.contains("Brightness=0.625"), "{saved}");
        assert!(saved.contains("TextureDetail=Low"), "{saved}");
        let invalid = wide("SET ini:Engine.Engine.AudioDevice MusicVolume invalid");
        assert!(access.execute_static_command(invalid.as_ptr(), &mut |_| Ok(())).is_err());
        assert_eq!((*audio).music_volume, 0.375);
        assert_eq!(std::fs::read_to_string(&system).unwrap(), saved);
        for text in ["SET ini:Engine.Engine.AudioDevice MusicVolume 1.5", "SET ini:Engine.Engine.ViewportManager TextureDetail 256", "SET ini:Engine.Engine.ViewportManager TextureDetail Bogus"] {
            assert!(access.execute_static_command(wide(text).as_ptr(), &mut |_| Ok(())).is_err(), "{text}");
        }
        assert_eq!(std::fs::read_to_string(&system).unwrap(), saved);
        (*audio).sound_volume = 1.5;
        let side_effect = wide("SET ini:Engine.Engine.AudioDevice MusicVolume 0.75");
        assert!(access.execute_static_command(side_effect.as_ptr(), &mut |_| Ok(())).is_err());
        assert_eq!((*audio).sound_volume, 1.5, "notification must not clamp another field before publication");
        assert_eq!((*audio).music_volume, 0.375);
        (*audio).sound_volume = 0.75;
        (*audio.cast::<UObject>()).object_flags |= 1;
        assert!(access.execute_static_command(side_effect.as_ptr(), &mut |_| Ok(())).is_err());
        (*audio.cast::<UObject>()).object_flags &= !1;
        assert_eq!((*audio).music_volume, 0.375);
        assert_eq!(std::fs::read_to_string(&system).unwrap(), saved);
        std::fs::write(&system, "external change\n").unwrap();
        let failed = wide("SET ini:Engine.Engine.AudioDevice MusicVolume 0.75");
        assert!(access.execute_static_command(failed.as_ptr(), &mut |_| Ok(())).is_err());
        assert_eq!((*audio).music_volume, 0.375);
        assert_eq!(std::fs::read_to_string(&system).unwrap(), "external change\n");
        let mut lines = Vec::new();
        let get = wide("GET ini:Engine.Engine.AudioDevice MusicVolume");
        access.execute_static_command(get.as_ptr(), &mut |value| { lines.push(value.to_vec()); Ok(()) }).unwrap();
        assert_eq!(lines[0].iter().copied().filter_map(char::from_u32).collect::<String>().parse::<f32>().unwrap(), 0.375);
        std::fs::write(&system, &saved).unwrap();
        access.config_set_units(&wide("Other"), &wide("Pending"), &wide("stay-cached")).unwrap();
        let dirty = wide("SET ini:Engine.Engine.AudioDevice MusicVolume 0.75");
        assert!(access.execute_static_command(dirty.as_ptr(), &mut |_| Ok(())).is_err());
        assert_eq!((*audio).music_volume, 0.375);
        assert_eq!(std::fs::read_to_string(&system).unwrap(), saved);
    }
    owner.shutdown().unwrap();
}
