use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert};
use hp2_abi::{
    FPlane, FVector,
    engine::generated::{
        self, AActor, ADecal, ALevelInfo, FFrame, FRotator, TArrayRaw, UClient, UEngine, UFunction,
        UGameEngine, ULevel, UModel, UObject, UStruct, UTexture,
    },
};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult},
    name_owner::FindName,
};
use hp2_engine::EngineOwner;
use std::{ffi::c_void, mem::size_of, path::PathBuf, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
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

unsafe fn set_array<T: Copy>(array: *mut TArrayRaw, values: &[T]) {
    unsafe {
        let slots = Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        };
        array_storage::set(
            slots,
            values.len() as i32,
            size_of::<T>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
        .unwrap();
        if !values.is_empty() {
            ptr::copy_nonoverlapping(values.as_ptr(), (*array).data.cast(), values.len());
        }
    }
}

fn float_arg(value: f32) -> Vec<u8> {
    let mut result = vec![0x1e];
    result.extend(value.to_le_bytes());
    result
}

fn vector_arg(value: FVector) -> Vec<u8> {
    let mut result = vec![0x23];
    result.extend(value.x.to_le_bytes());
    result.extend(value.y.to_le_bytes());
    result.extend(value.z.to_le_bytes());
    result
}

unsafe fn call(
    access: CoreAccess<'_>,
    object: *mut UObject,
    class_name: &str,
    function_name: &str,
    args: &[u8],
    result: *mut c_void,
) {
    unsafe {
        let class = access.native_class(class_name).unwrap();
        let function = access
            .construct_object(
                access.native_class("Core.Function").unwrap(),
                class.cast(),
                access.intern_str(function_name, FindName::Add).unwrap(),
                0,
                ptr::null_mut(),
            )
            .unwrap()
            .cast::<UFunction>();
        (*function).i_native = 0;
        (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast()).unwrap();

        let caller = access
            .construct_object(
                access.native_class("Core.Function").unwrap(),
                class.cast(),
                0,
                0,
                ptr::null_mut(),
            )
            .unwrap()
            .cast::<UStruct>();
        let mut code = vec![0x1c];
        code.extend((*function.cast::<UObject>()).index.to_le_bytes());
        code.extend_from_slice(args);
        code.push(0x16);
        set_array(ptr::addr_of_mut!((*caller).script).cast(), &code);
        let start = (*caller).script.data.cast::<u8>();
        let mut frame = FFrame {
            node: caller.cast(),
            object: object.cast(),
            code: start,
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        access
            .execute_state_frame_step(&raw mut frame, result)
            .unwrap_or_else(|error| panic!("{class_name}.{function_name}: {error:?}"));
        assert_eq!(
            frame.code,
            start.add(code.len()),
            "{class_name}.{function_name} consumes its argument terminator"
        );
    }
}

fn options(root: &std::path::Path) -> CoreOptions {
    CoreOptions {
        data_root: root.to_path_buf(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: Some(std::fs::File::create(root.join("decal-runtime.log")).unwrap()),
    }
}

fn vec3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

#[test]
fn actual_bsp_attach_clips_to_boundary_and_detach_preserves_unowned_surface_decals() {
    let artifacts =
        PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifact directory"));
    let root = artifacts.join(format!("decal-vm-native-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(
        root.join("System/Default.ini"),
        "[Engine.Engine]\nCanvas=Engine.Canvas\nInput=Engine.Input\n",
    )
    .unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(options(&root)).unwrap();
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
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        ptr::addr_of_mut!((*level).base.engine).write_unaligned(engine.cast::<UEngine>().cast());
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        (*info.cast::<AActor>()).x_level = level.cast();
        (*info.cast::<AActor>()).level = info.cast();
        (*engine).g_level = level.cast();

        let client = construct(access, "SDLDrv.SDLClient", engine.cast()).cast::<UClient>();
        (*client).set_decals(true);
        (*engine.cast::<UEngine>()).client = client.cast();

        let model = construct(access, "Engine.Model", level.cast()).cast::<UModel>();
        (*level).model = model.cast();
        let decal_texture =
            construct(access, "Engine.Texture", access.transient_package()).cast::<UTexture>();
        let surface_texture =
            construct(access, "Engine.Texture", access.transient_package()).cast::<UTexture>();
        let unrelated_texture =
            construct(access, "Engine.Texture", access.transient_package()).cast::<UTexture>();
        ptr::addr_of_mut!((*decal_texture).base.u_size).write_unaligned(16);
        ptr::addr_of_mut!((*surface_texture).base.u_size).write_unaligned(64);
        ptr::addr_of_mut!((*unrelated_texture).base.u_size).write_unaligned(24);

        let decal = construct(access, "Engine.Decal", level.cast()).cast::<ADecal>();
        (*decal.cast::<AActor>()).level = info.cast();
        (*decal.cast::<AActor>()).x_level = level.cast();
        (*decal.cast::<AActor>()).texture = decal_texture.cast();
        (*decal.cast::<AActor>()).location = vec3(20.0, 0.0, 0.0);
        (*decal.cast::<AActor>()).rotation = FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        };
        (*decal.cast::<AActor>()).draw_scale = 1.0;

        let unrelated = construct(access, "Engine.Decal", level.cast()).cast::<ADecal>();
        (*unrelated.cast::<AActor>()).texture = unrelated_texture.cast();
        let existing = generated::FDecal {
            vertices: [vec3(0.0, 0.0, 0.0); 4],
            actor: unrelated.cast(),
            nodes: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
        };
        let mut surface: FBspSurf = std::mem::zeroed();
        surface.texture = surface_texture.cast();
        surface.p_base = 0;
        surface.v_normal = 0;
        set_array(ptr::addr_of_mut!(surface.nodes).cast(), &[0_i32]);
        set_array(ptr::addr_of_mut!(surface.decals).cast(), &[existing]);

        let points = [
            vec3(10.0, -8.0, -8.0),
            vec3(10.0, 8.0, -8.0),
            vec3(10.0, 8.0, 8.0),
            vec3(10.0, -8.0, 8.0),
        ];
        let vectors = [vec3(1.0, 0.0, 0.0)];
        let verts = [0, 1, 2, 3].map(|p_vertex| FVert {
            p_vertex,
            i_side: 0,
        });
        let mut node: FBspNode = std::mem::zeroed();
        node.plane = FPlane {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 10.0,
        };
        node.i_vert_pool = 0;
        node.i_surf = 0;
        node.i_back = -1;
        node.i_front = -1;
        node.i_plane = -1;
        node.i_collision_bound = -1;
        node.i_render_bound = -1;
        node.num_vertices = 4;
        set_array(ptr::addr_of_mut!((*model).points).cast(), &points);
        set_array(ptr::addr_of_mut!((*model).vectors).cast(), &vectors);
        set_array(ptr::addr_of_mut!((*model).verts).cast(), &verts);
        set_array(ptr::addr_of_mut!((*model).nodes).cast(), &[node]);
        set_array(ptr::addr_of_mut!((*model).surfs).cast(), &[surface]);
        (*model.cast::<hp2_abi::engine::generated::UPrimitive>())
            .bounding_box
            .is_valid = 1;
        (*model).root_outside = 1;

        // Omitted optional FVector is decoded as zero, causing the real canonical
        // VRand callback; the result is the BSP surface's texture, not the decal's.
        let mut result = 1_usize as *mut c_void;
        call(
            access,
            decal.cast(),
            "Engine.Decal",
            "AttachDecal",
            &float_arg(20.0),
            (&raw mut result).cast(),
        );
        assert_eq!(
            result,
            surface_texture.cast(),
            "successful AttachDecal returns the hit BSP texture"
        );
        assert_eq!(
            ptr::addr_of!((*decal).multi_decal_level).read_unaligned(),
            0
        );
        let surfs = ptr::addr_of!((*model).surfs)
            .cast::<TArrayRaw>()
            .read_unaligned();
        let surface = &*surfs.data.cast::<FBspSurf>();
        assert_eq!(
            surface.decals.array_num, 2,
            "the unrelated pre-existing decal remains beside the new decal"
        );
        let decals = std::slice::from_raw_parts(surface.decals.data.cast::<generated::FDecal>(), 2);
        assert_eq!(decals[0].actor, unrelated.cast());
        assert_eq!(decals[1].actor, decal.cast());
        assert_eq!(
            decals[1].nodes.num, 1,
            "the real BSP node is attached to FDecal.Nodes"
        );
        assert_eq!(*decals[1].nodes.data.cast::<i32>(), 0);
        assert!(
            decals[1].vertices.iter().all(|vertex| vertex.x == 0.0),
            "stored vertices are relative to the BSP surface base"
        );
        let surf_list = ptr::addr_of!((*decal).surf_list).read_unaligned();
        assert_eq!(surf_list.num, 1);
        assert_eq!(*surf_list.data.cast::<i32>(), 0);

        call(
            access,
            decal.cast(),
            "Engine.Decal",
            "DetachDecal",
            &[],
            ptr::null_mut(),
        );
        let surface_after = &*surfs.data.cast::<FBspSurf>();
        assert_eq!(
            surface_after.decals.array_num, 1,
            "detach removes only this actor's FDecal"
        );
        let remaining = &*surface_after.decals.data.cast::<generated::FDecal>();
        assert_eq!(
            remaining.actor,
            unrelated.cast(),
            "detach preserves another decal on the same surface"
        );
        assert_eq!(ptr::addr_of!((*decal).surf_list).read_unaligned().num, 0);

        // Exact polygon-vertex contact is inside the legacy coplanar boundary.
        (*decal.cast::<AActor>()).location = vec3(20.0, 8.0, 8.0);
        let mut args = float_arg(20.0);
        args.extend(vector_arg(vec3(0.0, 1.0, 0.0)));
        let mut boundary_result = ptr::null_mut::<UTexture>();
        call(
            access,
            decal.cast(),
            "Engine.Decal",
            "AttachDecal",
            &args,
            (&raw mut boundary_result).cast(),
        );
        assert_eq!(
            boundary_result,
            surface_texture.cast(),
            "a trace at the BSP polygon corner remains a valid inclusive hit"
        );
        let surface_at_boundary = &*surfs.data.cast::<FBspSurf>();
        assert_eq!(surface_at_boundary.decals.array_num, 2);
        let entries = std::slice::from_raw_parts(
            surface_at_boundary.decals.data.cast::<generated::FDecal>(),
            2,
        );
        assert_eq!(entries[0].actor, unrelated.cast());
        assert_eq!(entries[1].actor, decal.cast());
        assert_eq!(entries[1].nodes.num, 1);
        call(
            access,
            decal.cast(),
            "Engine.Decal",
            "DetachDecal",
            &[],
            ptr::null_mut(),
        );
        let final_surface = &*surfs.data.cast::<FBspSurf>();
        assert_eq!(final_surface.decals.array_num, 1);
        assert_eq!(
            (*final_surface.decals.data.cast::<generated::FDecal>()).actor,
            unrelated.cast()
        );

        // The native writes NULL before early return when the client disables decals.
        (*client).set_decals(false);
        let mut disabled_result = 2_usize as *mut c_void;
        call(
            access,
            decal.cast(),
            "Engine.Decal",
            "AttachDecal",
            &float_arg(20.0),
            (&raw mut disabled_result).cast(),
        );
        assert!(disabled_result.is_null());
        assert_eq!(
            (*final_surface.decals.data.cast::<generated::FDecal>()).actor,
            unrelated.cast()
        );
        (*client).set_decals(true);

        // A finite segment that cannot reach the plane returns NULL without
        // mutating either live BSP array.
        let mut missed_result = 3_usize as *mut c_void;
        call(
            access,
            decal.cast(),
            "Engine.Decal",
            "AttachDecal",
            &float_arg(1.0),
            (&raw mut missed_result).cast(),
        );
        assert!(missed_result.is_null());
        let final_surface = &*surfs.data.cast::<FBspSurf>();
        assert_eq!(final_surface.decals.array_num, 1);
        assert_eq!(ptr::addr_of!((*decal).surf_list).read_unaligned().num, 0);
    }
    owner
        .shutdown()
        .expect("destroy decal owner and all typed BSP storage");
}
