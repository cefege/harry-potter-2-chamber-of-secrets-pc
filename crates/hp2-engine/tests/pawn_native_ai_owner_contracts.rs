use hp2_abi::{FVector, engine::generated::{AActor, AGameInfo, ALevelInfo, APawn, APlayerReplicationInfo, FFrame, TArrayRaw, UFunction, ULevel, UObject, UProperty, UStruct}};
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
unsafe fn call(access: CoreAccess<'_>, pawn: *mut APawn, native: u16, aim: f32, distance: f32) -> (*mut AActor, [f32; 2]) {
    unsafe {
        let function = construct(access, "Core.Function", (*pawn.cast::<UObject>()).class.cast()).cast::<UFunction>();
        let mut code = Vec::new();
        if native == 531 || native == 534 {
            for (name, offset) in [("Aim", 0), ("Distance", 4)] {
                let property = access.construct_object(access.native_class("Core.FloatProperty").unwrap(), function.cast(), access.intern_str(name, FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap().cast::<UProperty>();
                (*property).offset = offset; (*property).element_size = 4; (*property).array_dim = 1;
                code.push(0); code.extend((*property.cast::<UObject>()).index.to_le_bytes());
            }
            for vector in [[1_f32, 0., 0.], [0., 0., 0.]] {
                code.push(0x23); for value in vector { code.extend(value.to_le_bytes()); }
            }
        }
        code.push(0x16);
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let script = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut locals = [aim, distance];
        let mut result = pawn.cast::<AActor>();
        let mut frame = FFrame { node: function.cast(), object: pawn.cast(), code: script, locals: locals.as_mut_ptr().cast(), parent: ptr::null_mut() };
        access.execute_vm_native_index(pawn.cast(), native.into(), &raw mut frame, (&raw mut result).cast()).unwrap();
        assert_eq!(frame.code, script.add(code.len()));
        (result, locals)
    }
}

unsafe fn boolean_call(access: CoreAccess<'_>, pawn: *mut APawn, native: u16, other: *mut AActor) -> u32 {
    unsafe {
        let function = construct(access, "Core.Function", (*pawn.cast::<UObject>()).class.cast()).cast::<UFunction>();
        let mut code = Vec::new();
        if native != 526 {
            if other.is_null() { code.push(0x2a); } else { code.push(0x20); code.extend((*other.cast::<UObject>()).index.to_le_bytes()); }
        }
        code.push(0x16);
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let script = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut result = u32::MAX;
        let mut frame = FFrame { node: function.cast(), object: pawn.cast(), code: script, locals: ptr::null_mut(), parent: ptr::null_mut() };
        access.execute_vm_native_index(pawn.cast(), native.into(), &raw mut frame, (&raw mut result).cast()).unwrap();
        assert_eq!(frame.code, script.add(code.len()));
        result
    }
}

pub fn exercise_target_selection() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("pawn-ai-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    let access = owner.access();
    unsafe {
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let shooter = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let pawn_target = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let oblique = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let aligned = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let equal_aim_later = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let behind = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let far = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let actors = [info.cast::<AActor>(), shooter.cast(), pawn_target.cast(), oblique, aligned, equal_aim_later, behind, far];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors { (*actor).x_level = level.cast(); (*actor).level = info.cast(); (*actor).set_b_proj_target(true); (*actor).location = FVector { x: 0., y: 0., z: 0. }; }
        (*shooter).base.set_b_proj_target(false);
        (*info.cast::<AActor>()).set_b_proj_target(false);
        (*pawn_target).health = 100; (*pawn_target).base.location.x = 100.;
        (*oblique).location = FVector { x: 50., y: 50., z: 0. };
        (*aligned).location.x = 125.; (*equal_aim_later).location.x = 10.;
        (*behind).location.x = -10.; (*far).location.x = 2001.;
        let model = construct(access, "Engine.Model", level.cast()); (*level).model = model.cast();
        call(access, shooter, 529, 0., 0.);
        call(access, pawn_target, 529, 0., 0.);
        call(access, pawn_target, 529, 0., 0.);
        assert_eq!({ (*info).pawn_list }, pawn_target.cast());
        assert_eq!({ (*pawn_target).next_pawn }, shooter.cast()); assert!((*shooter).next_pawn.is_null(), "duplicate AddPawn must not create a cycle");
        let (selected, values) = call(access, shooter, 534, 0., 777.);
        assert_eq!(selected, aligned); assert_eq!(values, [1., 125.], "highest aim wins; equal aim does not switch to the closer later actor");
        let (selected, values) = call(access, shooter, 534, 1., 777.);
        assert!(selected.is_null()); assert_eq!(values, [1., 777.], "strict aim threshold leaves caller out values unchanged when no candidate improves");
        let (selected, values) = call(access, shooter, 531, 0., 777.);
        assert_eq!(selected, pawn_target.cast()); assert_eq!(values, [1., 100.]);
        (*pawn_target).health = 0;
        let (selected, values) = call(access, shooter, 531, 0.5, 777.);
        assert!(selected.is_null()); assert_eq!(values, [0.5, 777.]);
        (*pawn_target).health = 100;
        let game = construct(access, "Engine.GameInfo", level.cast()).cast::<AGameInfo>();
        (*game.cast::<AActor>()).x_level = level.cast(); (*game.cast::<AActor>()).level = info.cast();
        (*game).set_b_team_game(true); (*info).game = game.cast();
        let own_info = construct(access, "Engine.PlayerReplicationInfo", level.cast()).cast::<APlayerReplicationInfo>();
        let other_info = construct(access, "Engine.PlayerReplicationInfo", level.cast()).cast::<APlayerReplicationInfo>();
        for pri in [own_info, other_info] { (*pri.cast::<AActor>()).x_level = level.cast(); (*pri.cast::<AActor>()).level = info.cast(); (*pri).team = 1; }
        (*shooter).player_replication_info = own_info.cast(); (*pawn_target).player_replication_info = other_info.cast();
        assert!(call(access, shooter, 531, 0., 777.).0.is_null(), "same-team target excluded by live GameInfo and replication metadata");
        (*other_info).team = 2;
        assert_eq!(call(access, shooter, 531, 0., 777.).0, pawn_target.cast());
        call(access, pawn_target, 530, 0., 0.);
        assert_eq!({ (*info).pawn_list }, shooter.cast()); assert_eq!({ (*pawn_target).next_pawn }, shooter.cast(), "RemovePawn retains removed node's next pointer");
        call(access, pawn_target, 529, 0., 0.);
        call(access, shooter, 530, 0., 0.);
        assert_eq!({ (*info).pawn_list }, pawn_target.cast()); assert!((*pawn_target).next_pawn.is_null(), "interior removal relinks its predecessor");
        call(access, pawn_target, 530, 0., 0.);
        assert!((*info).pawn_list.is_null());
        (*shooter).view_rotation.pitch = 40000;
        assert_eq!(hp2_engine::pawn_native_ai_owner::find_stair_rotation(access, shooter, 1.).unwrap(), 40000,
            "long frame returns before pitch normalization or trace");
        for actor in actors { (*actor).set_b_collide_actors(false); }
        hp2_engine::level_owner::set_actor_collision(access, level, true).unwrap();
        (*shooter).base.collision_height = 20.; (*shooter).base.collision_radius = 10.; (*shooter).base_eye_height = 5.;
        assert_eq!(hp2_engine::pawn_native_ai_owner::find_stair_rotation(access, shooter, 0.125).unwrap(), 0,
            "real empty-world visibility traces ease pitch to level");
        assert_eq!((*shooter).view_rotation.pitch, -25536, "short frame publishes source pitch normalization");
        (*shooter).base.physics = 3;
        (*shooter).destination = FVector { x: 15., y: 0., z: 0. };
        assert_eq!(boolean_call(access, shooter, 526, ptr::null_mut()), 1,
            "swimming adjustment must use canonical cylinder extents, not a C++-published actor table");
        assert_eq!((*shooter).destination, FVector { x: 0., y: 0., z: 20. });
        (*shooter).base.physics = 2;
        assert_eq!(boolean_call(access, shooter, 526, ptr::null_mut()), 0);
        assert_eq!((*shooter).destination, FVector { x: 0., y: 0., z: 20. }, "falling rejection does not change the destination");
        (*shooter).base.physics = 1;
        (*shooter).sight_radius = 1000.; (*shooter).peripheral_vision = 0.;
        (*shooter).set_b_lo_sflag(false);
        assert_eq!(boolean_call(access, shooter, 514, aligned), 1);
        assert!(!(*shooter).b_lo_sflag(), "LineOfSightTo does not perform show-self toggling");
        assert_eq!(boolean_call(access, shooter, 533, aligned), 0,
            "source CanSee skips the non-pawn head trace on the toggled-on lap");
        assert!((*shooter).b_lo_sflag());
        assert_eq!(boolean_call(access, shooter, 533, aligned), 1);
        assert!(!(*shooter).b_lo_sflag(), "the next CanSee lap restores the source head trace");
        assert_eq!(boolean_call(access, shooter, 514, ptr::null_mut()), 0);
        hp2_engine::level_owner::set_actor_collision(access, level, false).unwrap();
        access.scratch_memory().tick().unwrap();
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nreal_target_VM_out_arguments=ok\nreal_model_sight=ok\nnonpawn_filter_and_strict_aim_precedence=ok\nno_improvement_preserves_out_values=ok\nhealth_and_live_team_filters=ok\npawn_list_duplicate_head_and_interior_transitions=ok\nretained_removed_next_pointer=ok\nstair_pitch_short_and_long_frames_and_real_visibility_traces=ok\nwall_adjust_canonical_shape_and_trace_producers=ok\nsight_null_and_show_self_semantics=ok\nshutdown=ok\n").unwrap();
}
#[test]
fn canonical_target_selection_and_pawn_list_transitions() { exercise_target_selection(); }

pub fn exercise_landing() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("pawn-landing-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()),
    }).unwrap();
    let access = owner.access();
    unsafe {
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let pawn = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &[info.cast::<AActor>(), pawn.cast()]);
        for actor in [info.cast::<AActor>(), pawn.cast()] {
            (*actor).x_level = level.cast(); (*actor).level = info.cast();
        }
        (*pawn).base.physics = 0;
        call(access, pawn, 527, 0., 0.);
        assert_eq!((*pawn).base.latent_float, 2.5);
        assert!((*pawn.cast::<UObject>()).state_frame.is_null(), "grounded WaitForLanding does not require or allocate an execution frame");
        access.initialize_execution(pawn.cast()).unwrap();
        let frame = (*pawn.cast::<UObject>()).state_frame.cast::<hp2_abi::engine::generated::FStateFrame>();
        (*frame).latent_action = 384;
        (*pawn).base.latent_float = 11.;
        hp2_engine::pawn_native_ai_owner::stop_waiting(pawn).unwrap();
        assert_eq!((*pawn).base.latent_float, -1.);
        assert_eq!((*frame).latent_action, 384, "StopWaiting cancels the timer, not the poll identity");
        (*frame).latent_action = 528; (*pawn).base.latent_float = 11.;
        hp2_engine::pawn_native_ai_owner::stop_waiting(pawn).unwrap();
        assert_eq!((*pawn).base.latent_float, 11., "non-Sleep latent action is not cancelled");
        (*pawn).base.physics = 2;
        call(access, pawn, 527, 0., 0.);
        assert_eq!((*frame).latent_action, 528);
        let mut delta = 1_f32;
        access.execute_vm_native_index(pawn.cast(), 528, frame.cast(), (&raw mut delta).cast()).unwrap();
        assert_eq!((*pawn).base.latent_float, 1.5);
        delta = 1.5;
        access.execute_vm_native_index(pawn.cast(), 528, frame.cast(), (&raw mut delta).cast()).unwrap();
        assert_eq!((*pawn).base.latent_float, 0., "zero timer does not invoke LongFall");
        delta = 0.25;
        let error = access.execute_vm_native_index(pawn.cast(), 528, frame.cast(), (&raw mut delta).cast()).unwrap_err();
        assert!(error.to_string().contains("LongFall event metadata is unavailable"), "missing event cannot become a fake successful producer: {error}");
        assert_eq!((*pawn).base.latent_float, -0.25, "timer write precedes the checked event failure");
        assert_eq!((*frame).latent_action, 528);
        (*pawn).base.physics = 0;
        access.execute_vm_native_index(pawn.cast(), 528, frame.cast(), ptr::null_mut()).unwrap();
        assert_eq!((*frame).latent_action, 0, "grounded poll clears without reading a delta");
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nreal_execution_frame_and_VM_poll=ok\ngrounded_wait_does_not_require_stack=ok\nsleep_only_timer_cancellation=ok\nlanding_timer_zero_and_negative_boundaries=ok\nmissing_LongFall_checked_error_preserves_timer=ok\ngrounded_poll_does_not_read_delta=ok\nshutdown=ok\n").unwrap();
}

#[test]
fn canonical_landing_timer_and_poll_boundaries() { exercise_landing(); }
