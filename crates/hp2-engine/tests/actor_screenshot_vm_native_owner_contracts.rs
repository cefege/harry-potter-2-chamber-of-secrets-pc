//! Process-main-thread SDL/WGPU contract for Actor.CreateTextureFromScreenShot.
//!
//! Run with `HP2_ARTIFACT_DIR=/absolute/writable/path cargo test -p hp2-engine --features 'sdl renderer-wgpu' --test actor_screenshot_vm_native_owner_contracts`.
//! Requires macOS arm64, an interactive Aqua session, SDL2, and a working wgpu adapter.
use hp2_abi::{
    FColor, FPlane,
    engine::generated::{
        AActor, ALevelInfo, APlayerPawn, FFrame, FMipmap, UEngine, UFunction, UGameEngine, ULevel,
        UObject, USDLClient, UStruct, UTexture, UViewport,
    },
};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult},
    name_owner::FindName,
};
use hp2_engine::EngineOwner;
use std::{path::PathBuf, ptr};

thread_local! {
    static LOG: std::cell::RefCell<Option<std::io::BufWriter<std::fs::File>>> = const { std::cell::RefCell::new(None) };
}
fn output(_: CoreAccess<'_>, category: i32, text: &[u32]) -> RuntimeResult<()> {
    use std::io::Write;
    LOG.with(|log| {
        let mut log = log.borrow_mut();
        let log = log
            .as_mut()
            .expect("artifact logger initialized before EngineOwner");
        let record = (|| -> std::io::Result<()> {
            write!(log, "{category}: ")?;
            for &unit in text {
                let scalar = char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER);
                log.write_all(scalar.encode_utf8(&mut [0; 4]).as_bytes())?;
            }
            log.write_all(b"\n")?;
            log.flush()
        })();
        record.map_err(|error| {
            hp2_core::core_access::RuntimeError::new("Screenshot artifact logging failed")
                .with_context(&error.to_string())
        })
    })
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

unsafe fn set_array<T: Copy>(raw: *mut hp2_abi::engine::generated::TArrayRaw, values: &[T]) {
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

fn run() {
    let artifact_root = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR must name a writable artifact directory"),
    );
    std::fs::create_dir_all(&artifact_root).unwrap();
    // The production capture path renders real GPU frames even when Aqua
    // reports Occluded; it does not claim a window frame was presented.
    unsafe {
        std::env::set_var(
            "HP2_CAPTURE_FRAMES",
            artifact_root.join("screenshot-frame-capture"),
        );
    }
    let root = artifact_root.join(format!("screenshot-vm-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    let system_ini = root.join("System/Default.ini");
    let user_ini = root.join("System/DefUser.ini");
    std::fs::write(&system_ini, "[Engine.Engine]\nCanvas=Engine.Canvas\nInput=Engine.Input\nGameRenderDevice=WgpuDrv.WgpuRenderDevice\n").unwrap();
    std::fs::write(&user_ini, "").unwrap();
    LOG.with(|log| {
        *log.borrow_mut() = Some(std::io::BufWriter::new(
            std::fs::File::create(root.join("runtime.log")).unwrap(),
        ))
    });

    // This binary has harness=false, so main is the process's actual main
    // thread required by SDL's Cocoa video backend.
    assert_eq!(
        hp2_platform::launcher_boot_video::init_video_and_joystick(),
        0,
        "SDL video initialization must succeed on the process main thread"
    );
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini,
        user_ini,
        command_line: vec![0],
        output,
        log_file: None,
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
        let engine = construct(access, "Engine.GameEngine", access.transient_package())
            .cast::<UGameEngine>();
        hp2_engine::engine_owner::init(
            access,
            ptr::NonNull::new(engine.cast::<UEngine>()).unwrap(),
        )
        .unwrap();
        let level = construct(access, "Engine.Level", engine.cast()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        (*level).model = construct(access, "Engine.Model", level.cast()).cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let pawn = construct(access, "Engine.PlayerPawn", level.cast()).cast::<APlayerPawn>();
        set_array(
            ptr::addr_of_mut!((*level).base.actors).cast(),
            &[info.cast::<AActor>(), pawn.cast()],
        );
        for actor in [info.cast::<AActor>(), pawn.cast()] {
            (*actor).x_level = level.cast();
            (*actor).level = info.cast();
        }
        (*engine).g_level = level.cast();
        let client = construct(access, "SDLDrv.SDLClient", engine.cast()).cast::<USDLClient>();
        (*engine.cast::<UEngine>()).client = client.cast();
        // UInput construction resolves this native enum even in data-none mode.
        access
            .construct_object(
                access.native_class("Core.Enum").unwrap(),
                access.native_class("Engine.Actor").unwrap().cast(),
                access.intern_str("EInputKey", FindName::Add).unwrap(),
                0,
                ptr::null_mut(),
            )
            .unwrap();
        hp2_engine::platform_owner::init_client(access, client, engine.cast()).unwrap();
        (*client).base.windowed_viewport_x = 512;
        (*client).base.windowed_viewport_y = 256;
        let viewport = hp2_engine::platform_owner::new_viewport(access, client, 0).unwrap();
        (*viewport).base.actor = pawn.cast();
        (*pawn).player = viewport.cast();
        hp2_engine::platform_owner::open_viewport_window(access, viewport, false, -1, -1, -1, -1)
            .unwrap();
        let viewport_base = viewport.cast::<UViewport>();
        assert_eq!((*viewport_base).size_x, 512);
        assert_eq!((*viewport_base).size_y, 256);
        let device = (*viewport_base).ren_dev;
        assert!(
            !device.is_null(),
            "canonical SDL viewport owns a real render device"
        );

        // Submit a genuine wgpu frame through the canonical viewport lock,
        // with an unmistakable clear plane; no fake SnapBuff/provider exists.
        let scale = FPlane {
            x: 1.0,
            y: 1.0,
            z: 1.0,
            w: 1.0,
        };
        let fog = FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        let clear = FPlane {
            x: 0.125,
            y: 0.5,
            z: 0.875,
            w: 1.0,
        };
        assert!(
            hp2_engine::platform_owner::lock_viewport(
                access,
                viewport_base,
                &raw const scale,
                &raw const fog,
                &raw const clear,
                0,
                ptr::null_mut(),
                ptr::null_mut()
            )
            .unwrap()
        );
        hp2_engine::platform_owner::unlock_viewport(access, viewport_base, true).unwrap();
        let mut source = vec![
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            };
            512 * 256
        ];
        hp2_engine::render_device_owner::read_pixels(access, device.cast(), &mut source).unwrap();
        let expected_clear = FColor {
            r: 32,
            g: 128,
            b: 223,
            a: 255,
        };
        assert!(
            source.iter().all(|pixel| *pixel == expected_clear),
            "known linear clear color, gamma-1 postprocess, and non-sRGB RGBA8 readback"
        );
        let source_bytes = unsafe {
            std::slice::from_raw_parts(
                source.as_ptr().cast::<u8>(),
                source.len() * size_of::<FColor>(),
            )
        };
        std::fs::write(
            artifact_root.join("screenshot-source-rgba.bin"),
            source_bytes,
        )
        .unwrap();

        // Build a real Core function frame and step the real viewport object
        // argument through native 322, then check its actual owned mip storage.
        let class = access.native_class("Engine.Actor").unwrap();
        let function = access
            .construct_object(
                access.native_class("Core.Function").unwrap(),
                class.cast(),
                access
                    .intern_str("CreateTextureFromScreenShot", FindName::Add)
                    .unwrap(),
                0,
                ptr::null_mut(),
            )
            .unwrap()
            .cast::<UFunction>();
        (*function).i_native = 322;
        (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast()).unwrap();
        let caller = construct(access, "Core.Function", class.cast()).cast::<UStruct>();
        let mut script = vec![0x20]; // EX_ObjectConst
        script.extend_from_slice(&(*viewport.cast::<UObject>()).index.to_le_bytes());
        script.push(0x16);
        set_array(ptr::addr_of_mut!((*caller).script).cast(), &script);
        let code = (*caller).script.data.cast::<u8>();
        let mut frame = FFrame {
            node: caller.cast(),
            object: ptr::null_mut(),
            code,
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        let actor = construct(access, "Engine.Actor", access.transient_package());
        let mut texture = ptr::null_mut::<UObject>();
        access
            .execute_vm_native_index(actor.cast(), 322, &raw mut frame, (&raw mut texture).cast())
            .unwrap();
        assert_eq!(
            frame.code,
            code.add(script.len()),
            "native argument stepping and P_FINISH advance the script cursor"
        );
        // The source SNAP argument is an exponent: 2^8 = 256, not divisor 8.
        assert_eq!(
            ((*viewport_base).snap_u, (*viewport_base).snap_v),
            (2, 1),
            "native executed the real viewport Exec snap 8 path"
        );
        assert_eq!(
            (*viewport_base).snap_buff.num,
            2,
            "SNAP populated its owned viewport pixel buffer through read_pixels"
        );
        assert!(
            !texture.is_null(),
            "native returns a real canonical texture"
        );
        let texture = texture.cast::<UTexture>();
        assert_eq!(ptr::addr_of!((*texture).base.format).read_unaligned(), 5);
        assert_eq!(ptr::addr_of!((*texture).base.u_size).read_unaligned(), 128);
        assert_eq!(ptr::addr_of!((*texture).base.v_size).read_unaligned(), 64);
        let mips = ptr::addr_of!((*texture).mips).read_unaligned();
        assert_eq!(mips.num, 1, "LoadFromSnap produced the owned base mip");
        assert!(!mips.data.is_null());
        let mip = &*mips.data.cast::<FMipmap>();
        let texture_bytes = &mip.data_array;
        assert_eq!(
            texture_bytes.array_num,
            128 * 64 * 4,
            "texture pixels live in its owned lazy byte array"
        );
        assert!(!texture_bytes.data.is_null());
        let mip_bytes = std::slice::from_raw_parts(
            texture_bytes.data.cast::<u8>(),
            texture_bytes.array_num as usize,
        );
        let snap = std::slice::from_raw_parts((*viewport_base).snap_buff.data.cast::<u8>(), 2 * 4);
        assert_eq!(
            snap,
            [32, 128, 223, 255, 32, 128, 223, 255],
            "source 256x256 box filter preserves the actual uniform GPU color"
        );
        for y in 0..64 {
            let row = &mip_bytes[y * 128 * 4..(y + 1) * 128 * 4];
            if y < 1 {
                let copied = snap;
                assert_eq!(
                    &row[..copied.len()],
                    copied,
                    "LoadFromSnap copies the source row into the texture's top-left"
                );
                assert!(
                    row[copied.len()..].iter().all(|&byte| byte == 0),
                    "LoadFromSnap leaves row padding zero"
                );
            } else {
                assert!(
                    row.iter().all(|&byte| byte == 0),
                    "LoadFromSnap leaves lower texture rows zero"
                );
            }
        }
        assert!(
            (*texture).b_realtime_changed(),
            "LoadFromSnap marks texture storage changed"
        );
        std::fs::write(artifact_root.join("screenshot-texture-rgba.bin"), mip_bytes).unwrap();
    }
    owner.shutdown().unwrap();
    hp2_platform::launcher_boot_video::quit();
    std::fs::write(artifact_root.join("screenshot-vm-contract.txt"), "native=322\nviewport=512x256\nsnap=2x1\ntexture=128x64\nformat=5\nsource=real-wgpu-readback\ncapture=HP2_CAPTURE_FRAMES\n").unwrap();
}

fn main() {
    run();
}
