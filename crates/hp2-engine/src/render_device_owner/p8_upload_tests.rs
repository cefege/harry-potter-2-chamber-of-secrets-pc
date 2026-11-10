use super::*;
use hp2_core::core_access::CoreOptions;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

#[test]
#[ignore = "requires a real GPU adapter and HP2_ARTIFACT_DIR"]
fn engine_p8_upload_policy_cache_and_errors() {
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
        .join("engine-p8-upload-policy");
    std::fs::create_dir_all(&root).unwrap();
    let owner = crate::EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("Game.ini"),
        user_ini: root.join("User.ini"),
        command_line: vec![],
        output,
        log_file: None,
    })
    .unwrap();
    let access = owner.access();
    // SAFETY: UWgpuRenderDevice and its URenderDevice/TArrayRaw/FPlane fields
    // contain only integers, floats, raw pointers and byte arrays; zero is valid.
    let mut device: UWgpuRenderDevice = unsafe { std::mem::zeroed() };
    // SAFETY: FTextureInfo contains integers, floats, FVector, raw pointers and
    // byte arrays; zero is valid for all of those fields.
    let mut info: FTextureInfo = unsafe { std::mem::zeroed() };
    // SAFETY: FMipmapBase contains only raw pointers, integers and byte arrays;
    // zero is valid for each field.
    let mut mip: FMipmapBase = unsafe { std::mem::zeroed() };
    let device_ptr = ptr::addr_of_mut!(device);
    let info_ptr = ptr::addr_of_mut!(info);
    let mip_ptr = ptr::addr_of_mut!(mip);
    let owner_state = state(access).unwrap();
    owner_state.register(device_ptr as usize).unwrap();
    owner_state.global_device.set(device_ptr.cast());
    let runtime = owner_state.runtime(device_ptr as usize).unwrap();
    let gpu = hp2_wgpu::create_device_sync(
        hp2_wgpu::BackendChoice::Auto,
        None::<hp2_wgpu::HeadlessSurfaceCreator>,
    )
    .expect("a real GPU adapter is required");
    std::fs::write(
        root.join("gpu-adapter.txt"),
        format!(
            "adapter: {}\nbackend: {:?}\n",
            gpu.adapter_name, gpu.backend
        ),
    )
    .unwrap();
    {
        let mut lease = runtime.lease().unwrap();
        lease.resources().created = Some(gpu);
    }

    let mut indices = [0u8, 255];
    let mut palette = [hp2_wgpu::texture::FColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    }; 256];
    let indices_ptr = ptr::addr_of_mut!(indices);
    let palette_ptr = ptr::addr_of_mut!(palette);
    // SAFETY: These pointers originate from live, aligned stack values. Their
    // storage remains live through every policy call and is not accessed in parallel.
    let original_palette = unsafe {
        palette_ptr
            .cast::<hp2_wgpu::texture::FColor>()
            .write(hp2_wgpu::texture::FColor {
                r: 1,
                g: 2,
                b: 3,
                a: 4,
            });
        palette_ptr
            .cast::<hp2_wgpu::texture::FColor>()
            .add(255)
            .write(hp2_wgpu::texture::FColor {
                r: 251,
                g: 252,
                b: 253,
                a: 254,
            });
        (*mip_ptr).data_ptr = indices_ptr.cast();
        (*mip_ptr).u_size = 2;
        (*mip_ptr).v_size = 1;
        (*info_ptr).cache_id = 0x120;
        (*info_ptr).format = 0; // TEXF_P8
        (*info_ptr).u_size = 2;
        (*info_ptr).v_size = 1;
        (*info_ptr).num_mips = 1;
        (*info_ptr).palette = palette_ptr.cast();
        ptr::addr_of_mut!((*info_ptr).mips)
            .cast::<*mut c_void>()
            .write(mip_ptr.cast());
        *palette_ptr
    };
    let adapter = policy(access, device_ptr, ptr::null_mut(), 1, 1).unwrap();
    adapter.call.borrow_mut().info = info_ptr;
    let action = hp2_wgpu::driver_policy::Hp2WgpuDriverAction::UploadTexture;

    for (flags, expected_id) in [(0u32, 0x120u64), (2, 0x124)] {
        adapter.call.borrow_mut().upload_poly_flags = flags;
        // SAFETY: info_ptr remains live and no policy call is active.
        unsafe { (*info_ptr).set_b_realtime_changed(true) };
        assert_eq!(
            with_policy(adapter.clone(), action).unwrap() as u64,
            expected_id
        );
        // SAFETY: The policy call has returned and info_ptr remains live.
        assert!(!unsafe { (*info_ptr).b_realtime_changed() });
        let mut lease = runtime.lease().unwrap();
        assert!(lease.resources().loop_state.is_texture_cached(expected_id));
    }
    // SAFETY: palette_ptr still points to the live 256-entry array.
    assert_eq!(
        unsafe { *palette_ptr },
        original_palette,
        "conversion must not mutate the palette"
    );

    // Direct leaf validates its byte count before dereferencing the palette.
    // The 2-byte index array remains live even in the error cases.
    adapter.call.borrow_mut().current_mip = mip_ptr;
    adapter.call.borrow_mut().cache_id = 0x130;
    // SAFETY: info_ptr remains live and no policy call is active.
    unsafe { (*info_ptr).palette = ptr::null_mut() };
    assert_eq!(
        adapter.upload_uncompressed(1, false).unwrap_err(),
        RuntimeError::new("Canonical texture mip byte count is invalid")
    );
    assert_eq!(
        adapter.upload_uncompressed(2, false).unwrap_err(),
        RuntimeError::new("Canonical P8 texture has no palette")
    );
    assert!(
        !runtime
            .lease()
            .unwrap()
            .resources()
            .loop_state
            .is_texture_cached(0x130)
    );

    // The full policy resets dirty before its null-palette skip; it does not
    // call the direct leaf in this case. No invalid pointer is dereferenced.
    // SAFETY: info_ptr remains live and no policy call is active.
    unsafe {
        (*info_ptr).cache_id = 0x140;
        (*info_ptr).set_b_realtime_changed(true);
    }
    adapter.call.borrow_mut().upload_poly_flags = 0;
    assert_eq!(with_policy(adapter.clone(), action).unwrap() as u64, 0x140);
    // SAFETY: The policy call has returned and info_ptr remains live.
    assert!(!unsafe { (*info_ptr).b_realtime_changed() });
    assert!(
        !runtime
            .lease()
            .unwrap()
            .resources()
            .loop_state
            .is_texture_cached(0x140)
    );

    drop(adapter);
    {
        let mut lease = runtime.lease().unwrap();
        lease.resources().release_device();
    }
    owner_state.unregister(device_ptr as usize).unwrap();
    drop(runtime);
    drop(owner_state);
    owner.shutdown().unwrap();
}
