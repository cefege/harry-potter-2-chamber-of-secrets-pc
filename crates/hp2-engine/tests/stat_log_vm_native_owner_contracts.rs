use hp2_abi::engine::generated::{
    AActor, AGameInfo, ALevelInfo, APawn, APlayerPawn, APlayerReplicationInfo, AStatLog,
    AStatLogFile, FFrame, TArrayRaw, UField, UFunction, ULevel, UObject, UProperty, UStruct,
};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult},
    name_owner::FindName,
};
use hp2_engine::EngineOwner;
use std::{
    ffi::c_void,
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    ptr,
    time::{Duration, Instant},
};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
fn wide(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe {
        access.construct_object(
            access.native_class(class).unwrap(),
            outer,
            0,
            0,
            ptr::null_mut(),
        )
    }
    .unwrap_or_else(|error| panic!("construct {class}: {error:?}"))
}
unsafe fn set_array<T: Copy>(raw: *mut TArrayRaw, values: &[T]) {
    unsafe {
        array_storage::set(
            Hp2ArraySlots {
                data: &raw mut (*raw).data,
                num: &raw mut (*raw).num,
                max: &raw mut (*raw).max,
            },
            values.len() as i32,
            size_of::<T>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
        .unwrap();
        ptr::copy_nonoverlapping(values.as_ptr(), (*raw).data.cast(), values.len());
    }
}
unsafe fn native_function(access: CoreAccess<'_>, class: &str, name: &str) -> *mut UFunction {
    unsafe {
        let class = access.native_class(class).unwrap();
        let function = access
            .construct_object(
                access.native_class("Core.Function").unwrap(),
                class.cast(),
                access.intern_str(name, FindName::Add).unwrap(),
                0,
                ptr::null_mut(),
            )
            .unwrap()
            .cast::<UFunction>();
        (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast()).unwrap();
        function
    }
}
unsafe fn call(
    access: CoreAccess<'_>,
    receiver: *mut UObject,
    class: &str,
    name: &str,
    args: &[u8],
    result: *mut c_void,
    locals: *mut u8,
) -> RuntimeResult<()> {
    unsafe {
        let function = native_function(access, class, name);
        let caller = construct(access, "Core.Function", (*receiver).class.cast()).cast::<UStruct>();
        let mut code = vec![0x1c];
        code.extend((*function.cast::<UObject>()).index.to_le_bytes());
        code.extend(args);
        code.push(0x16);
        set_array(ptr::addr_of_mut!((*caller).script).cast(), &code);
        let start = (*caller).script.data.cast::<u8>();
        let mut frame = FFrame {
            node: caller.cast(),
            object: receiver.cast(),
            code: start,
            locals,
            parent: ptr::null_mut(),
        };
        let operation = access.execute_state_frame_step(&raw mut frame, result);
        if operation.is_ok() {
            assert_eq!(
                frame.code,
                start.add(code.len()),
                "{class}.{name} consumes P_FINISH"
            );
        }
        operation
    }
}
unsafe fn call_void(
    access: CoreAccess<'_>,
    receiver: *mut UObject,
    class: &str,
    name: &str,
    args: &[u8],
) {
    unsafe {
        call(
            access,
            receiver,
            class,
            name,
            args,
            ptr::null_mut(),
            ptr::null_mut(),
        )
        .unwrap()
    }
}
fn string_arg(value: &str) -> Vec<u8> {
    let mut code = vec![0x34];
    for unit in value.encode_utf16().chain([0]) {
        code.extend(unit.to_le_bytes());
    }
    code
}
unsafe fn object_arg(value: *mut UObject) -> Vec<u8> {
    if value.is_null() {
        vec![0x2a]
    } else {
        let mut code = vec![0x20];
        code.extend(unsafe { (*value).index }.to_le_bytes());
        code
    }
}
unsafe fn property(
    access: CoreAccess<'_>,
    parent: *mut UStruct,
    name: &str,
    offset: i32,
) -> *mut UProperty {
    unsafe {
        let property = access
            .construct_object(
                access.native_class("Core.StrProperty").unwrap(),
                parent.cast(),
                access.intern_str(name, FindName::Add).unwrap(),
                0,
                ptr::null_mut(),
            )
            .unwrap()
            .cast::<UProperty>();
        (*property).offset = offset;
        (*property).element_size = size_of::<TArrayRaw>() as i32;
        (*property).array_dim = 1;
        (*property).property_flags = 0x80;
        (*property.cast::<UField>()).next = (*parent).children;
        (*parent).children = property.cast();
        property
    }
}
unsafe fn out_string(
    access: CoreAccess<'_>,
    receiver: *mut UObject,
    class: &str,
    name: &str,
    prefix_args: &[u8],
    initial: &str,
) -> String {
    unsafe {
        let source = construct(access, "Core.Function", (*receiver).class.cast()).cast::<UStruct>();
        let property = property(access, source, "Checksum", 0);
        let mut value = TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        };
        access
            .assign_fstring(&raw mut value, &wide(initial))
            .unwrap();
        let mut args = prefix_args.to_vec();
        args.push(0x00);
        args.extend((*property.cast::<UObject>()).index.to_le_bytes());
        let operation = call(
            access,
            receiver,
            class,
            name,
            &args,
            ptr::null_mut(),
            (&raw mut value).cast(),
        );
        let text = text(&raw const value);
        access.destroy_owned_fstring(&raw mut value).unwrap();
        operation.unwrap();
        text
    }
}
unsafe fn text(raw: *const TArrayRaw) -> String {
    unsafe { hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(raw)) }
        .iter()
        .map(|&unit| char::from_u32(unit).unwrap())
        .collect()
}
unsafe fn string_result(access: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> String {
    unsafe {
        let mut value = TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        };
        let operation = call(
            access,
            receiver,
            "Engine.StatLog",
            name,
            &[],
            (&raw mut value).cast(),
            ptr::null_mut(),
        );
        let text = text(&raw const value);
        access.destroy_owned_fstring(&raw mut value).unwrap();
        operation.unwrap();
        text
    }
}
unsafe fn file_event(access: CoreAccess<'_>, class: *mut UStruct, name: &str, parameters: usize) {
    unsafe {
        let function = access
            .construct_object(
                access.native_class("Core.Function").unwrap(),
                class.cast(),
                access.intern_str(name, FindName::Add).unwrap(),
                0,
                ptr::null_mut(),
            )
            .unwrap()
            .cast::<UFunction>();
        (*function.cast::<UField>()).next = (*class).children;
        (*class).children = function.cast();
        (*function.cast::<UStruct>()).properties_size =
            (parameters * size_of::<TArrayRaw>()) as i32;
        (*function.cast::<UStruct>()).properties_align = 4;
        (*function).parms_size = (parameters * size_of::<TArrayRaw>()) as u16;
        (*function).num_parms = parameters as u8;
        (*function).function_flags = 0x800;
        (*function).return_value_offset = u16::MAX;
        let state = class.cast::<hp2_abi::engine::generated::UState>();
        let hash = (i32::from_ne_bytes((*function.cast::<UObject>()).name) as usize) & 255;
        (*function.cast::<UField>()).hash_next = (*state).vf_hash[hash];
        (*state).vf_hash[hash] = function.cast();
        let file_log = native_function(access, "Engine.StatLogFile", "FileLog");
        let mut code = Vec::new();
        for index in 0..parameters {
            let name = match index {
                0 => "SpecialID",
                1 => "SpecialParam",
                _ => "SpecialParam2",
            };
            let parameter = property(
                access,
                function.cast(),
                name,
                (index * size_of::<TArrayRaw>()) as i32,
            );
            code.push(0x1c);
            code.extend((*file_log.cast::<UObject>()).index.to_le_bytes());
            code.push(0x00);
            code.extend((*parameter.cast::<UObject>()).index.to_le_bytes());
            code.push(0x16);
        }
        code.extend([0x04, 0x0b]);
        set_array(
            ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(),
            &code,
        );
    }
}
fn encoded(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .chain([13, 10])
        .flat_map(u16::to_le_bytes)
        .collect()
}
fn await_marker(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "real batcher did not create {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn stat_log_natives_write_source_bytes_hashes_script_events_and_real_batch_effects() {
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("artifacts"))
        .join(format!("statlog-vm-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).unwrap();
    fs::write(root.join("System/Default.ini"), "[Engine.Engine]\n").unwrap();
    fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: Some(fs::File::create(root.join("runtime.log")).unwrap()),
    })
    .unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode {
        is_client: true,
        is_server: true,
        is_scriptable: true,
        ..access.process_mode()
    });
    unsafe {
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let game = construct(access, "Engine.GameInfo", level.cast()).cast::<AGameInfo>();
        let log = construct(access, "Engine.StatLogFile", level.cast()).cast::<AStatLogFile>();
        let pawn = construct(access, "Engine.PlayerPawn", level.cast()).cast::<APlayerPawn>();
        let pri = construct(access, "Engine.PlayerReplicationInfo", level.cast())
            .cast::<APlayerReplicationInfo>();
        let mutator = construct(access, "Engine.Mutator", level.cast());
        let actors = [
            info.cast::<AActor>(),
            game.cast(),
            log.cast(),
            pawn.cast(),
            pri.cast(),
            mutator.cast(),
        ];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors {
            (*actor).level = info.cast();
            (*actor).x_level = level.cast();
        }
        (*info).game = game.cast();
        (*info).set_b_begun_play(true);
        access
            .assign_fstring(
                ptr::addr_of_mut!((*level).base.url.map),
                &wide("B6Entry.unr"),
            )
            .unwrap();
        assert_eq!(
            string_result(access, log.cast(), "GetMapFileName"),
            "B6Entry.unr"
        );
        assert_eq!(
            string_result(access, log.cast(), "GetGMTRef"),
            "0.0",
            "CTest TZ=UTC produces the source numeric timezone reference"
        );

        let local = root.join("local.tmp");
        let final_local = root.join("local.log");
        fs::write(&local, b"old").unwrap();
        fs::set_permissions(&local, fs::Permissions::from_mode(0o400)).unwrap();
        fs::write(&final_local, b"replace").unwrap();
        fs::set_permissions(&final_local, fs::Permissions::from_mode(0o400)).unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log).stat_log_file),
                &wide(local.to_str().unwrap()),
            )
            .unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log).stat_log_final),
                &wide(final_local.to_str().unwrap()),
            )
            .unwrap();
        call_void(access, log.cast(), "Engine.StatLogFile", "OpenLog", &[]);
        call_void(
            access,
            log.cast(),
            "Engine.StatLogFile",
            "FileLog",
            &string_arg("frag Ω"),
        );
        call_void(access, log.cast(), "Engine.StatLogFile", "FileFlush", &[]);
        assert_eq!(
            fs::read(&local).unwrap(),
            encoded("frag Ω"),
            "Flush exposes exact UTF-16LE CRLF bytes before CloseLog"
        );
        call_void(access, log.cast(), "Engine.StatLogFile", "CloseLog", &[]);
        assert!(!local.exists());
        assert_eq!(
            fs::read(&final_local).unwrap(),
            encoded("frag Ω"),
            "CloseLog replaces the read-only destination with source bytes"
        );

        (*log.cast::<AStatLog>()).set_b_world(true);
        let world = root.join("world.tmp");
        let final_world = root.join("world.log");
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log).stat_log_file),
                &wide(world.to_str().unwrap()),
            )
            .unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log).stat_log_final),
                &wide(final_world.to_str().unwrap()),
            )
            .unwrap();
        assert!(
            call(
                access,
                log.cast(),
                "Engine.StatLogFile",
                "Watermark",
                &string_arg("before-open"),
                ptr::null_mut(),
                ptr::null_mut()
            )
            .is_err()
        );
        call_void(access, log.cast(), "Engine.StatLogFile", "OpenLog", &[]);
        call_void(
            access,
            log.cast(),
            "Engine.StatLogFile",
            "Watermark",
            &string_arg("mark Ω"),
        );
        assert_eq!(
            out_string(
                access,
                log.cast(),
                "Engine.StatLogFile",
                "GetChecksum",
                &[],
                "prefix:"
            ),
            "prefix:8d8f9c3a89d83eec762801a7c2862763"
        );
        call_void(
            access,
            log.cast(),
            "Engine.StatLogFile",
            "FileLog",
            &string_arg("frag Ω"),
        );
        call_void(access, log.cast(), "Engine.StatLogFile", "CloseLog", &[]);
        let expected: Vec<u8> = encoded("frag Ω")
            .into_iter()
            .map(|byte| byte ^ 0xa7)
            .collect();
        assert_eq!(fs::read(&final_world).unwrap(), expected);

        access
            .assign_fstring(ptr::addr_of_mut!((*pawn).ng_world_secret), &wide("sëcret"))
            .unwrap();
        access
            .assign_fstring(ptr::addr_of_mut!((*pri).player_name), &wide("Harry"))
            .unwrap();
        (*pawn.cast::<APawn>()).player_replication_info = pri.cast();
        assert_eq!(
            out_string(
                access,
                log.cast(),
                "Engine.StatLog",
                "GetPlayerChecksum",
                &object_arg(pawn.cast()),
                "overwrite"
            ),
            "82390281e05529fbc969bf8db9ff81f4"
        );
        (*pawn.cast::<APawn>()).player_replication_info = ptr::null_mut();
        assert_eq!(
            out_string(
                access,
                log.cast(),
                "Engine.StatLog",
                "GetPlayerChecksum",
                &object_arg(pawn.cast()),
                "overwrite"
            ),
            "5ba5057cc91d2e7c72caf90151ae32b2"
        );
        assert_eq!(
            out_string(
                access,
                log.cast(),
                "Engine.StatLog",
                "GetPlayerChecksum",
                &object_arg(ptr::null_mut()),
                "overwrite"
            ),
            "NoChecksum"
        );

        (*log.cast::<AStatLog>()).set_b_world(false);
        let events = root.join("events.tmp");
        let final_events = root.join("events.log");
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log).stat_log_file),
                &wide(events.to_str().unwrap()),
            )
            .unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log).stat_log_final),
                &wide(final_events.to_str().unwrap()),
            )
            .unwrap();
        let log_class = access
            .native_class("Engine.StatLogFile")
            .unwrap()
            .cast::<UStruct>();
        file_event(access, log_class, "LogGameSpecial", 2);
        file_event(access, log_class, "LogGameSpecial2", 3);
        // These files are only sized by InitialCheck, never parsed as packages.
        fs::write(root.join("System/Core.u"), b"123456789").unwrap();
        fs::write(root.join("System/Engine.u"), b"1234567").unwrap();
        call_void(access, log.cast(), "Engine.StatLogFile", "OpenLog", &[]);
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "InitialCheck",
            &object_arg(game.cast()),
        );
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "LogMutator",
            &object_arg(mutator),
        );
        call_void(access, log.cast(), "Engine.StatLogFile", "CloseLog", &[]);
        let expected = [
            "GameClass",
            "Class Engine.GameInfo",
            "CodePackageChecksum",
            "Core.u",
            "2c72c94bbd84b3a21998f68864b159a4",
            "CodePackageChecksum",
            "Engine.u",
            "b44a52e6dd9723ce2ab147cd855c6b54",
            "GameMutator",
            "Class Engine.Mutator",
        ]
        .into_iter()
        .flat_map(encoded)
        .collect::<Vec<_>>();
        assert_eq!(
            fs::read(&final_events).unwrap(),
            expected,
            "real ProcessEvent reenters native FileLog with source class/package iteration order"
        );
        fs::remove_file(root.join("System/Core.u")).unwrap();
        fs::remove_file(root.join("System/Engine.u")).unwrap();

        let local_marker = root.join("local-batcher");
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log.cast::<AStatLog>()).local_batcher_url),
                &wide("/usr/bin/touch"),
            )
            .unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*game).local_log_file_name),
                &wide(local_marker.to_str().unwrap()),
            )
            .unwrap();
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "ExecuteLocalLogBatcher",
            &[],
        );
        await_marker(&local_marker);
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log.cast::<AStatLog>()).local_batcher_url),
                &wide("/usr/bin/true"),
            )
            .unwrap();
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "ExecuteSilentLogBatcher",
            &[],
        );
        let defaults = access
            .default_object(access.native_class("Engine.StatLogFile").unwrap())
            .unwrap()
            .cast::<AStatLog>();
        let default_marker = root.join("class-default-batcher");
        access
            .assign_fstring(
                ptr::addr_of_mut!((*defaults).local_batcher_url),
                &wide("/usr/bin/touch"),
            )
            .unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*defaults).local_log_dir),
                &wide(default_marker.to_str().unwrap()),
            )
            .unwrap();
        call_void(access, log.cast(), "Engine.StatLog", "BatchLocal", &[]);
        await_marker(&default_marker);
        access
            .assign_fstring(
                ptr::addr_of_mut!((*log.cast::<AStatLog>()).world_batcher_url),
                &wide("/usr/bin/true"),
            )
            .unwrap();
        (*log.cast::<AStatLog>()).set_b_world_batcher_error(true);
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "ExecuteWorldLogBatcher",
            &[],
        );
        let static_defaults = access
            .default_object(access.native_class("Engine.StatLog").unwrap())
            .unwrap()
            .cast::<AStatLog>();
        assert!(
            (*static_defaults).b_world_batcher_error(),
            "world batcher writes its resulting error state back to the StatLog CDO"
        );
        (*game).set_b_external_batcher(true);
        (*log.cast::<AStatLog>()).set_b_world_batcher_error(false);
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "ExecuteWorldLogBatcher",
            &[],
        );
        assert!(
            (*static_defaults).b_world_batcher_error(),
            "external batching leaves the prior CDO state unchanged"
        );
        (*game).set_b_external_batcher(false);
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "ExecuteWorldLogBatcher",
            &[],
        );
        assert!(
            !(*static_defaults).b_world_batcher_error(),
            "the next real batcher poll preserves and saves a cleared error state"
        );
        let persisted = access
            .config_get_units(
                hp2_core::lit!("Engine.StatLog"),
                hp2_core::lit!("bWorldBatcherError"),
            )
            .unwrap()
            .expect("SaveConfig writes the global property");
        let persisted: String = persisted
            .into_iter()
            .take_while(|&unit| unit != 0)
            .map(|unit| char::from_u32(unit).unwrap())
            .collect();
        assert_eq!(persisted, "False");
        call_void(
            access,
            log.cast(),
            "Engine.StatLog",
            "BrowseRelativeLocalURL",
            &string_arg("stats/index.html"),
        );
        (*info).game = ptr::null_mut();
        assert!(
            call(
                access,
                log.cast(),
                "Engine.StatLog",
                "ExecuteLocalLogBatcher",
                &[],
                ptr::null_mut(),
                ptr::null_mut()
            )
            .is_err()
        );
    }
    owner.shutdown().unwrap();
    fs::write(root.join("contract.txt"), "data_profile=data-none\nnatives=16\nlocal=UTF16LE+CRLF\nworld=XOR-A7\nchecksums=source-MD5\nevents=real-ProcessEvent-reentry\nprocesses=real-Unix-kernel\n").unwrap();
}
