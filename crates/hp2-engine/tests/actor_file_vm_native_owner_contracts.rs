use hp2_abi::engine::generated::{
    AActor, FFrame, FMipmap, TArrayRaw, UFunction, ULevel, UObject, UPalette, UStruct, UTexture,
};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, RuntimeResult},
    name_owner::FindName,
};
use hp2_engine::EngineOwner;
use std::{ffi::c_void, fs, path::PathBuf, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe {
        access
            .construct_object(
                access.native_class(class).unwrap(),
                outer,
                0,
                0,
                ptr::null_mut(),
            )
            .unwrap()
    }
}
fn string_arg(text: &str) -> Vec<u8> {
    let mut value = vec![0x34];
    for unit in text.encode_utf16().chain([0]) {
        value.extend(unit.to_le_bytes());
    }
    value
}
unsafe fn call(
    access: CoreAccess<'_>,
    object: *mut UObject,
    class: &str,
    name: &str,
    index: Option<u32>,
    args: &[u8],
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        let class = access.native_class(class)?;
        let function = access
            .construct_object(
                access.native_class("Core.Function")?,
                class.cast(),
                access.intern_str(name, FindName::Add)?,
                0,
                ptr::null_mut(),
            )?
            .cast::<UFunction>();
        (*function).i_native = index.unwrap_or(0) as u16;
        (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast())?;
        let caller = construct(access, "Core.Function", class.cast()).cast::<UStruct>();
        let mut code = Vec::new();
        if index.is_none() {
            code.push(0x1c);
            code.extend((*function.cast::<UObject>()).index.to_le_bytes());
        }
        code.extend(args);
        code.push(0x16);
        let script = ptr::addr_of_mut!((*caller).script).cast::<TArrayRaw>();
        array_storage::set(
            Hp2ArraySlots {
                data: &raw mut (*script).data,
                num: &raw mut (*script).num,
                max: &raw mut (*script).max,
            },
            code.len() as i32,
            1,
            ptr::null(),
            array_storage::system_allocator(),
        )
        .unwrap();
        ptr::copy_nonoverlapping(code.as_ptr(), (*script).data.cast(), code.len());
        let start = (*script).data.cast::<u8>();
        let mut frame = FFrame {
            node: caller.cast(),
            object: object.cast(),
            code: start,
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        if let Some(index) = index {
            access.execute_vm_native_index(object.cast(), index, &raw mut frame, result)?;
        } else {
            access.execute_state_frame_step(&raw mut frame, result)?;
        }
        assert_eq!(
            frame.code,
            start.add(code.len()),
            "{class:p} {name} consumes P_FINISH"
        );
        Ok(())
    }
}
fn owner(label: &str) -> (EngineOwner, PathBuf) {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("{label}-{}", std::process::id()));
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
    (owner, root)
}
fn bmp_header(width: u32, height: u32, bits: u16, offset: usize, length: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&(length as u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&(offset as u32).to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&width.to_le_bytes());
    bytes[22..26].copy_from_slice(&height.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&bits.to_le_bytes());
    bytes
}
unsafe fn import(
    access: CoreAccess<'_>,
    actor: *mut UObject,
    name: &str,
    path: &std::path::Path,
) -> RuntimeResult<*mut UTexture> {
    let mut args = string_arg(name);
    args.extend(string_arg(path.to_str().unwrap()));
    let mut result = ptr::null_mut::<UTexture>();
    unsafe {
        call(
            access,
            actor,
            "Engine.Actor",
            "CreateTextureFromBMP",
            Some(321),
            &args,
            (&raw mut result).cast(),
        )?;
    }
    Ok(result)
}
unsafe fn mip(texture: *mut UTexture) -> FMipmap {
    unsafe {
        let mips = ptr::addr_of!((*texture).mips).read_unaligned();
        assert_eq!(mips.num, 1);
        mips.data.cast::<FMipmap>().read_unaligned()
    }
}
unsafe fn mip_bytes(texture: *mut UTexture) -> Vec<u8> {
    unsafe {
        let mip = mip(texture);
        std::slice::from_raw_parts(
            mip.data_array.data.cast::<u8>(),
            mip.data_array.array_num as usize,
        )
        .to_vec()
    }
}
#[test]
fn imported_bmp_has_real_pixels_palette_masking_and_animation_links() {
    let (owner, root) = owner("actor-image");
    let access = owner.access();
    let mut indexed = bmp_header(2, 2, 8, 1078, 1086);
    for i in 0..256usize {
        let color = if i == 0 {
            [255, 0, 255, 77]
        } else {
            [(i * 3) as u8, (255 - i) as u8, i as u8, 77]
        };
        indexed[54 + i * 4..58 + i * 4].copy_from_slice(&color);
    }
    indexed[1078..].copy_from_slice(&[3, 0, 0, 0, 1, 2, 0, 0]);
    fs::write(root.join("indexed.bmp"), indexed).unwrap();
    let mut rgb = bmp_header(4, 2, 24, 54, 78);
    rgb[54..].copy_from_slice(&[
        30, 20, 10, 60, 50, 40, 90, 80, 70, 120, 110, 100, 150, 140, 130, 180, 170, 160, 210, 200,
        190, 240, 230, 220,
    ]);
    fs::write(root.join("rgb.bmp"), rgb).unwrap();
    unsafe {
        let actor = construct(access, "Engine.Actor", access.transient_package());
        let first = import(access, actor, "Saved_A00", &root.join("indexed.bmp")).unwrap();
        assert!(!first.is_null());
        assert_eq!(mip_bytes(first), [1, 2, 3, 0]);
        let first_mip = mip(first);
        assert_eq!(
            (
                first_mip.base.u_size,
                first_mip.base.v_size,
                first_mip.base.u_bits,
                first_mip.base.v_bits
            ),
            (2, 2, 1, 1)
        );
        assert_eq!(ptr::addr_of!((*first).base.format).read_unaligned(), 0);
        assert_eq!(
            ptr::addr_of!((*first).poly_flags).read_unaligned() & 2,
            2,
            "magenta palette zero with a zero texel is masked"
        );
        let palette = ptr::addr_of!((*first).base.palette)
            .read_unaligned()
            .cast::<UPalette>();
        let colors = ptr::addr_of!((*palette).colors).read_unaligned();
        assert_eq!(colors.num, 256);
        assert_eq!(colors.data.cast::<u32>().read_unaligned(), 0xffff00ff);
        assert_eq!(
            colors.data.cast::<u32>().add(1).read_unaligned(),
            0xff03fe01,
            "reserved BMP alpha is replaced by opaque source colors"
        );
        let second = import(access, actor, "Saved_A01", &root.join("indexed.bmp")).unwrap();
        assert_eq!(
            ptr::addr_of!((*second).base.palette).read_unaligned(),
            palette.cast(),
            "identical SavePics palettes reuse real canonical ownership"
        );
        assert_eq!(
            ptr::addr_of!((*first).anim_next).read_unaligned(),
            second.cast(),
            "adjacent source animation names link the previous frame"
        );
        assert_eq!(
            (*first.cast::<UObject>()).outer,
            (*second.cast::<UObject>()).outer
        );
        let package = (*first.cast::<UObject>()).outer.cast::<UObject>();
        assert_eq!(
            i32::from_ne_bytes((*package).name),
            access.intern_str("SavePics", FindName::Find).unwrap()
        );
        let rgba = import(access, actor, "TrueColor", &root.join("rgb.bmp")).unwrap();
        assert!(!rgba.is_null());
        assert_eq!(ptr::addr_of!((*rgba).base.format).read_unaligned(), 5);
        assert_eq!(
            mip_bytes(rgba),
            [
                150, 140, 130, 255, 180, 170, 160, 255, 210, 200, 190, 255, 240, 230, 220, 255, 30,
                20, 10, 255, 60, 50, 40, 255, 90, 80, 70, 255, 120, 110, 100, 255
            ]
        );
        let rgba_mip = mip(rgba);
        assert_eq!((rgba_mip.base.u_bits, rgba_mip.base.v_bits), (2, 1));
        let relative = format!("system-relative-{}/image.bmp", std::process::id());
        let physical = root.join("System").join(&relative);
        fs::create_dir_all(physical.parent().unwrap()).unwrap();
        fs::copy(root.join("rgb.bmp"), &physical).unwrap();
        let resolved = import(
            access,
            actor,
            "SystemRelative",
            std::path::Path::new(&relative),
        )
        .unwrap();
        assert!(!resolved.is_null());
        assert_eq!(
            mip_bytes(resolved),
            mip_bytes(rgba),
            "relative image import resolves beneath System, not the data root or process cwd"
        );
    }
}
#[test]
fn image_import_distinguishes_source_rejections_and_truncated_storage_errors() {
    let (owner, root) = owner("actor-image-boundaries");
    let access = owner.access();
    fs::write(root.join("nonpower.bmp"), bmp_header(3, 2, 24, 54, 78)).unwrap();
    fs::write(root.join("truncated.bmp"), bmp_header(2, 2, 8, 1078, 54)).unwrap();
    let mut pcx = vec![0u8; 128];
    pcx[0] = 10;
    pcx[3] = 8;
    pcx[8..10].copy_from_slice(&1u16.to_le_bytes());
    pcx[10..12].copy_from_slice(&1u16.to_le_bytes());
    pcx[65] = 1;
    pcx[66..68].copy_from_slice(&2u16.to_le_bytes());
    fs::write(root.join("truncated.pcx"), pcx).unwrap();
    fs::write(root.join("empty.bmp"), []).unwrap();
    fs::File::create(root.join("oversized.bmp"))
        .unwrap()
        .set_len(i32::MAX as u64 + 1)
        .unwrap();
    unsafe {
        let actor = construct(access, "Engine.Actor", access.transient_package());
        assert!(
            import(access, actor, "Missing", &root.join("missing.bmp"))
                .unwrap()
                .is_null()
        );
        assert!(
            import(access, actor, "NonPower", &root.join("nonpower.bmp"))
                .unwrap()
                .is_null()
        );
        assert!(
            import(access, actor, "Oversized", &root.join("oversized.bmp"))
                .unwrap()
                .is_null(),
            "source Unix reader refuses sizes outside INT without allocating the file body"
        );
        fs::remove_file(root.join("oversized.bmp")).unwrap();
        assert!(
            import(access, actor, "Empty", &root.join("empty.bmp")).is_err(),
            "source appLoadFileToArray accesses element zero after sizing its binary array"
        );
        assert!(import(access, actor, "TruncatedBmp", &root.join("truncated.bmp")).is_err());
        assert!(import(access, actor, "TruncatedPcx", &root.join("truncated.pcx")).is_err());
    }
}
#[test]
fn platform_and_network_queries_use_real_configuration_and_driver_dispatch() {
    let (owner, _root) = owner("actor-file-queries");
    let access = owner.access();
    unsafe {
        let actor = construct(access, "Engine.Actor", access.transient_package());
        for (native, name) in [(327, "IsOSVer2kOrXP"), (329, "IsSoftwareRendering")] {
            let mut result = [0x5a, 0xa5, 0xc3, 0xd4, 0xe5];
            call(
                access,
                actor,
                "Engine.Actor",
                name,
                Some(native),
                &[],
                result.as_mut_ptr().add(1).cast(),
            )
            .unwrap();
            assert_eq!(
                result,
                [0x5a, 0, 0xc3, 0xd4, 0xe5],
                "query writes exactly its one-byte VM bool"
            );
        }
        for (setting, expected) in [
            ("sOfTdRv.SoFtWaReReNdErDeViCe", 1u8),
            ("WgpuDrv.WgpuRenderDevice", 0),
        ] {
            let value: Vec<u32> = setting.chars().map(u32::from).collect();
            access
                .config_set_units(
                    hp2_core::lit!("Engine.Engine"),
                    hp2_core::lit!("GameRenderDevice"),
                    &value,
                )
                .unwrap();
            let mut result = 0xa5u8;
            call(
                access,
                actor,
                "Engine.Actor",
                "IsSoftwareRendering",
                Some(329),
                &[],
                (&raw mut result).cast(),
            )
            .unwrap();
            assert_eq!(result, expected);
        }
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        let game = construct(access, "Engine.GameInfo", level.cast()).cast::<AActor>();
        (*game).x_level = level.cast();
        for driver in [
            ptr::null_mut(),
            construct(access, "Engine.DemoRecDriver", level.cast()),
        ] {
            (*level).base.net_driver = driver.cast();
            let mut result = TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            };
            call(
                access,
                game.cast(),
                "Engine.GameInfo",
                "GetNetworkNumber",
                None,
                &[],
                (&raw mut result).cast(),
            )
            .unwrap();
            assert!(hp2_core::native_text::fstring_units(&raw const result).is_empty());
            access.destroy_owned_fstring(&raw mut result).unwrap();
        }
        (*level).base.net_driver = ptr::null_mut();
        let mut texture = ptr::null_mut::<UTexture>();
        call(
            access,
            actor,
            "Engine.Actor",
            "CreateTextureFromScreenShot",
            Some(322),
            &[0x2a],
            (&raw mut texture).cast(),
        )
        .unwrap();
        assert!(texture.is_null());
    }
}
