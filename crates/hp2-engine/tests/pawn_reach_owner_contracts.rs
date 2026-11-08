use hp2_abi::{FVector, engine::generated::{AActor, ALevelInfo, APawn, FFrame, TArrayRaw, UEngine, UFunction, ULevel, UModel, UObject, UStruct}, render_scene::FBspNode};
use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_access::{CoreAccess, CoreOptions, RuntimeResult}};
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
unsafe fn point_call(access: CoreAccess<'_>, pawn: *mut APawn, point: FVector) -> i32 {
    unsafe {
        let function = construct(access, "Core.Function", (*pawn.cast::<UObject>()).class.cast()).cast::<UFunction>();
        let mut code = vec![0x23];
        for value in [point.x, point.y, point.z] { code.extend(value.to_le_bytes()); }
        code.push(0x16);
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let script = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut frame = FFrame { node: function.cast(), object: pawn.cast(), code: script, locals: ptr::null_mut(), parent: ptr::null_mut() };
        let mut result = -1_i32;
        access.execute_vm_native_index(pawn.cast(), 521, &raw mut frame, (&raw mut result).cast()).unwrap();
        assert_eq!(frame.code, script.add(code.len()));
        result
    }
}

unsafe fn jump_call(access: CoreAccess<'_>, pawn: *mut APawn) -> FVector {
    unsafe {
        let function = construct(access, "Core.Function", (*pawn.cast::<UObject>()).class.cast()).cast::<UFunction>();
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &[0x16_u8]);
        let script = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut frame = FFrame { node: function.cast(), object: pawn.cast(), code: script, locals: ptr::null_mut(), parent: ptr::null_mut() };
        let mut result = FVector { x: -1., y: -1., z: -1. };
        access.execute_vm_native_index(pawn.cast(), 523, &raw mut frame, (&raw mut result).cast()).unwrap();
        assert_eq!(frame.code, script.add(1));
        result
    }
}

pub fn exercise_reach() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("pawn-reach-owner-{}", std::process::id()));
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
        let pawn = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let target = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let actors = [info.cast::<AActor>(), pawn.cast(), target];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors { (*actor).x_level = level.cast(); (*actor).level = info.cast(); (*actor).region.zone = info.cast(); (*actor).set_b_collide_actors(false); (*actor).set_b_collide_world(false); (*actor).set_b_collide_when_placing(false); (*actor).set_b_static(false); (*actor).set_b_movable(true); }
        let model = construct(access, "Engine.Model", level.cast()).cast::<UModel>(); (*level).model = model.cast();
        // The tracker expands Nodes.Num into its spare-node pool. A source-valid
        // BSP needs a real root before that expansion, not an unlinked zero node.
        let mut partition: FBspNode = std::mem::zeroed();
        partition.plane.x = 1.;
        partition.i_front = -1; partition.i_back = -1; partition.i_plane = -1;
        partition.i_leaf = [-1, -1];
        partition.i_collision_bound = -1; partition.i_render_bound = -1;
        partition.node_flags = hp2_engine::trace::NF_NOT_CSG;
        set_array(ptr::addr_of_mut!((*model).nodes).cast(), &[partition]);
        (*model).root_outside = 1;
        hp2_engine::level_owner::set_actor_collision(access, level, true).unwrap();
        hp2_engine::dynbsp_owner::initialize(access, level).unwrap();
        (*pawn).base.location = FVector { x: 0., y: 0., z: 0. };
        (*pawn).base.velocity = FVector { x: 3., y: -4., z: 5. };
        (*pawn).base.physics = 4; (*pawn).base.collision_radius = 10.; (*pawn).base.collision_height = 20.;
        (*pawn).base_eye_height = 5.; (*pawn).foot_region.zone = info.cast(); (*pawn).head_region.zone = info.cast();
        let original_location = (*pawn).base.location;
        let original_velocity = (*pawn).base.velocity;
        assert_eq!(point_call(access, pawn, FVector { x: 100., y: 0., z: 0. }), hp2_engine::reach::R_FLY,
            "real flying reachability must simulate canonical MoveActor and restore the pose");
        assert_eq!((*pawn).base.location, original_location); assert_eq!((*pawn).base.velocity, original_velocity);
        assert_eq!(point_call(access, pawn, FVector { x: 801., y: 0., z: 0. }), 0, "source non-editor planar 800-unit limit");
        assert_eq!((*pawn).base.location, original_location);
        (*target).location = FVector { x: 125., y: 0., z: 0. };
        assert_eq!(hp2_engine::pawn_reach_owner::actor_reachable(access, pawn, target, 0).unwrap(), hp2_engine::reach::R_FLY);
        assert_eq!((*pawn).base.location, original_location); assert_eq!((*pawn).base.velocity, original_velocity);
        assert_eq!(hp2_engine::pawn_reach_owner::actor_reachable(access, pawn, ptr::null_mut(), 0).unwrap(), 0);
        (*pawn).ground_speed = 200.;
        (*pawn).destination = FVector { x: 10_000., y: 0., z: 0. };
        (*info).base.zone_gravity.z = 0.;
        assert_eq!(jump_call(access, pawn), FVector { x: 200., y: 0., z: 5. },
            "nonnegative gravity uses the source low-gravity policy, caps speed, and preserves vertical input");
        assert_eq!((*pawn).base.velocity, original_velocity);
        (*info).base.zone_gravity.z = -950.;
        (*pawn).destination = FVector { x: 0., y: 0., z: 100. };
        assert_eq!(jump_call(access, pawn), FVector { x: 0., y: 0., z: 5. });
        assert_eq!((*pawn).base.location, original_location);
        assert_eq!((*pawn).base.velocity, original_velocity);
        // This consumer owns the explicit tracker initialization. Restore its
        // model pools before Core's reverse-order object teardown frees Model.
        hp2_engine::dynbsp_owner::destroy(access, level).unwrap();
        assert_eq!({ (*model).nodes.array_num }, 1);
        hp2_engine::level_owner::set_actor_collision(access, level, false).unwrap();
        access.scratch_memory().tick().unwrap();
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nactual_point_reach_VM_and_canonical_movement=ok\nreal_actor_reach_and_class_queries=ok\nplanar_range_limit_and_null_target=ok\nlocation_and_velocity_restoration=ok\njump_VM_speed_cap_gravity_and_vertical_preservation=ok\nexplicit_tracker_restoration=ok\nscratch_released=ok\nshutdown=ok\n").unwrap();
}
#[test]
fn canonical_point_and_actor_reach_simulate_and_restore_real_poses() { exercise_reach(); }
