use hp2_abi::engine::generated::{AActor, ALevelInfo, FFrame, TArrayRaw, UEngine, UFunction, ULevel, UObject, UStruct};
use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_access::{CoreAccess, CoreOptions, RuntimeResult}, name_owner::FindName};
use hp2_engine::EngineOwner;
use std::{path::PathBuf, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe { access.construct_object(access.native_class(class).unwrap(), outer, 0, 0, ptr::null_mut()) }.unwrap()
}
unsafe fn set_array<T: Copy>(raw: *mut TArrayRaw, values: &[T]) {
    unsafe {
        array_storage::set(Hp2ArraySlots { data: &raw mut (*raw).data, num: &raw mut (*raw).num, max: &raw mut (*raw).max }, values.len() as i32, size_of::<T>() as i32, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(values.as_ptr(), (*raw).data.cast(), values.len());
    }
}
unsafe fn call(access: CoreAccess<'_>, actor: *mut AActor, name: &str, index: Option<u32>, args: &[u8]) -> i32 {
    unsafe {
        let class = access.native_class("Engine.Actor").unwrap();
        let function = access.construct_object(access.native_class("Core.Function").unwrap(), class.cast(),
            access.intern_str(name, FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap().cast::<UFunction>();
        (*function).i_native = index.unwrap_or(0) as u16;
        (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast()).unwrap();
        let caller = construct(access, "Core.Function", class.cast()).cast::<UStruct>();
        let mut code = Vec::new();
        if index.is_none() {
            code.push(0x1c); // EX_FinalFunction uses the canonical object index, not a pointer.
            code.extend((*function.cast::<UObject>()).index.to_le_bytes());
        }
        code.extend(args);
        code.push(0x16);
        set_array(ptr::addr_of_mut!((*caller).script).cast(), &code);
        let script = (*caller).script.data.cast::<u8>();
        let mut frame = FFrame { node: caller.cast(), object: actor.cast(), code: script, locals: ptr::null_mut(), parent: ptr::null_mut() };
        let mut result = -19_i32;
        if let Some(index) = index {
            access.execute_vm_native_index(actor.cast(), index, &raw mut frame, (&raw mut result).cast()).unwrap();
        } else {
            access.execute_state_frame_step(&raw mut frame, (&raw mut result).cast()).unwrap();
        }
        assert_eq!(frame.code, script.add(code.len()), "{name} consumes all parameters and P_FINISH");
        result
    }
}

#[test]
fn audio_natives_consume_real_script_arguments_and_preserve_no_device_semantics() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("actor-audio-vm-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    let access = owner.access();
    unsafe {
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<UEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let actor = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &[info.cast::<AActor>(), actor]);
        for value in [info.cast::<AActor>(), actor] { (*value).x_level = level.cast(); (*value).level = info.cast(); }
        // A real sound record forces routing through Level.DemoRecDriver and
        // Level.Engine.Client, rather than only testing the null-sound exit.
        let sound = construct(access, "Engine.Sound", access.transient_package());
        let mut sound_args = vec![0x20]; // EX_ObjectConst
        sound_args.extend((*sound).index.to_le_bytes());
        sound_args.extend([0x0b; 7]); // EX_Nothing: optional arguments retain actor defaults.
        for (name, index) in [("PlaySound", Some(264)), ("PlayOwnedSound", None), ("DemoPlaySound", None)] {
            call(access, actor, name, index, &sound_args);
            call(access, actor, name, index, &[0x2a, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b]);
        }
        // FString constants own VM temporary storage; a pointer-sized receiver
        // corrupts this path even when the engine has no audio device.
        let mut music = vec![0x1f];
        music.extend(b"music/theme.ogg\0");
        music.push(0x0b);
        assert_eq!(call(access, actor, "PlayMusic", None, &music), 0, "no device returns the source's invalid music handle");
        call(access, actor, "StopMusic", None, &[0x26, 0x0b]); // EX_IntOne
        call(access, actor, "StopAllMusic", None, &[0x0b]);
        assert_eq!(call(access, actor, "ModifySound", Some(567), &[0x24, 2, 0x1e, 0, 0, 0x80, 0x3f, 0x2a, 0x0b]), 0,
            "no device cannot report a modified channel");
        call(access, actor, "StopSound", Some(568), &[0x2a, 0x0b, 0x0b]);
        assert!((*engine).audio.is_null(), "script calls must not create an audio device implicitly");
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nall_eight_audio_VM_natives=ok\nindexed_and_named_script_cursor=ok\nreal_sound_no_hearers_and_null_sound=ok\nFString_music_cleanup=ok\nno_device_results=ok\nshutdown=ok\n").unwrap();
}
