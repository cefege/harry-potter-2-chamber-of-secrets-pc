use hp2_abi::{FVector, engine::generated::{AActor, ALevelInfo, APawn, FFrame, FStateFrame, TArrayRaw, UField, UFunction, ULevel, UObject, UProperty, UStruct}};
use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult}, name_owner::FindName};
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
fn vector(x: f32, y: f32, z: f32) -> FVector { FVector { x, y, z } }
fn vector_arg(v: FVector) -> Vec<u8> {
    let mut code = vec![0x23]; for value in [v.x, v.y, v.z] { code.extend(value.to_le_bytes()); } code
}
fn float_arg(value: f32) -> Vec<u8> { let mut code = vec![0x1e]; code.extend(value.to_le_bytes()); code }
unsafe fn object_arg(object: *mut UObject) -> Vec<u8> {
    if object.is_null() { vec![0x2a] } else { let mut code = vec![0x20]; code.extend(unsafe { (*object).index }.to_le_bytes()); code }
}
unsafe fn call(access: CoreAccess<'_>, pawn: *mut APawn, native: u32, mut code: Vec<u8>, poll: bool) {
    unsafe {
        let function = construct(access, "Core.Function", (*pawn.cast::<UObject>()).class.cast()).cast::<UFunction>();
        if !poll { code.push(0x16); }
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let start = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut frame = FFrame { node: function.cast(), object: pawn.cast(), code: start, locals: ptr::null_mut(), parent: ptr::null_mut() };
        let mut delta = 0.016_f32;
        access.execute_vm_native_index(pawn.cast(), native, &raw mut frame, (&raw mut delta).cast()).unwrap();
        assert_eq!(frame.code, start.add(if poll { 0 } else { code.len() }), "native {native}: polls must not consume the next script opcode");
    }
}
unsafe fn poll(access: CoreAccess<'_>, pawn: *mut APawn, native: u32) { unsafe { call(access, pawn, native, vec![0xff], true) } }
unsafe fn property(access: CoreAccess<'_>, parent: *mut UStruct, class: &str, name: &str, offset: i32, size: i32, flags: u32) -> *mut UProperty {
    unsafe {
        let p = access.construct_object(access.native_class(class).unwrap(), parent.cast(), access.intern_str(name, FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap().cast::<UProperty>();
        (*p).offset = offset; (*p).element_size = size; (*p).array_dim = 1; (*p).property_flags = flags;
        (*p.cast::<UField>()).next = (*parent).children; (*parent).children = p.cast(); p
    }
}
unsafe fn hear_noise_script(access: CoreAccess<'_>, pawn: *mut APawn) {
    unsafe {
        let class = (*pawn.cast::<UObject>()).class.cast::<UStruct>();
        let speed = property(access, class, "Core.FloatProperty", "DesiredSpeed", std::mem::offset_of!(APawn, desired_speed) as i32, 4, 0);
        let target = property(access, class, "Core.ObjectProperty", "MoveTarget", std::mem::offset_of!(APawn, move_target) as i32, 8, 0);
        let f = access.construct_object(access.native_class("Core.Function").unwrap(), class.cast(), access.intern_str("HearNoise", FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap().cast::<UFunction>();
        (*f.cast::<UField>()).next = (*class).children; (*class).children = f.cast();
        (*f.cast::<UStruct>()).properties_size = 12; (*f).parms_size = 12; (*f).num_parms = 2; (*f).return_value_offset = u16::MAX;
        let loud = property(access, f.cast(), "Core.FloatProperty", "Loudness", 0, 4, 0x80);
        let noise = property(access, f.cast(), "Core.ObjectProperty", "NoiseMaker", 4, 8, 0x80);
        let mut code = Vec::new();
        for (destination, source) in [(speed, loud), (target, noise)] {
            code.extend([0x0f, 0x01]); code.extend((*destination.cast::<UObject>()).index.to_le_bytes());
            code.push(0x00); code.extend((*source.cast::<UObject>()).index.to_le_bytes());
        }
        code.extend([0x04, 0x0b]); set_array(ptr::addr_of_mut!((*f.cast::<UStruct>()).script).cast(), &code);
    }
}
#[test]
fn canonical_movement_natives_preserve_latent_transitions_arguments_and_noise_events() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("pawn-move-vm-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap(); std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    let access = owner.access(); access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_scriptable: true, ..access.process_mode() });
    unsafe {
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let pawn = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let goal = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let listener = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let actors = [info.cast::<AActor>(), pawn.cast(), goal.cast(), listener.cast()];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors { (*actor).x_level = level.cast(); (*actor).level = info.cast(); (*actor).region.zone = info.cast(); }
        for p in [pawn, goal, listener] { access.initialize_execution(p.cast()).unwrap(); (*p).base.collision_radius = 20.; (*p).base.collision_height = 40.; }
        let state = (*pawn.cast::<UObject>()).state_frame.cast::<FStateFrame>();
        (*pawn).base.physics = 0; (*pawn).ground_speed = 240.; (*pawn).accel_rate = 300.; (*pawn).max_desired_speed = 0.75; (*pawn).base.location = vector(0., 0., 0.);
        (*goal).base.location = vector(1000., 0., 10.);
        let dest = vector(800., 100., 0.);
        let mut args = vector_arg(dest); args.extend(float_arg(4.)); call(access, pawn, 500, args, false);
        assert_eq!((*state).latent_action, 501); assert_eq!((*pawn).desired_speed, 0.75); assert_eq!((*pawn).destination, dest); assert_eq!((*pawn).focus, dest); assert!((*pawn).move_target.is_null());
        (*pawn).base.old_location = (*pawn).base.location; (*pawn).base.location = vector(dest.x + 1., dest.y, dest.z);
        poll(access, pawn, 501); assert_eq!((*state).latent_action, 0, "arrival clears MoveTo");
        (*pawn).base.location = vector(0., 0., 0.); (*pawn).base.old_location = (*pawn).base.location;
        let mut args = object_arg(goal.cast()); args.push(0x0b); call(access, pawn, 502, args, false);
        assert_eq!((*state).latent_action, 503); assert_eq!((*pawn).move_timer, 1.2); assert_eq!({ (*pawn).move_target }, goal.cast()); assert_eq!((*pawn).desired_speed, 0.75);
        (*pawn).move_target = ptr::null_mut(); poll(access, pawn, 503); assert_eq!((*state).latent_action, 0, "lost target cancels MoveToward");
        let focus = vector(0., 500., 30.); let mut args = vector_arg(dest); args.extend(vector_arg(focus)); call(access, pawn, 504, args, false);
        assert_eq!((*state).latent_action, 505); assert_eq!((*pawn).destination, dest); assert_eq!((*pawn).focus, focus);
        (*pawn).base.location = vector(dest.x + 1., dest.y, dest.z); poll(access, pawn, 505); assert_eq!((*state).latent_action, 0, "arrival clears StrafeTo");
        (*pawn).base.location = vector(0., 0., 0.); (*pawn).base.old_location = (*pawn).base.location;
        let mut args = vector_arg(dest); args.extend(object_arg(goal.cast())); call(access, pawn, 506, args, false);
        assert_eq!((*state).latent_action, 507); assert_eq!({ (*pawn).face_target }, goal.cast());
        (*goal).base.location = vector(1000., 300., 30.); poll(access, pawn, 507);
        assert_eq!((*pawn).focus, (*goal).base.location); assert_eq!((*pawn).destination, dest, "moving focus does not replace the strafe destination");
        (*pawn).face_target = ptr::null_mut(); poll(access, pawn, 507); assert_eq!((*state).latent_action, 0);
        (*pawn).base.physics = 1; (*pawn).base.location.z = 17.;
        call(access, pawn, 508, vector_arg(vector(500., 0., 900.)), false);
        assert_eq!((*state).latent_action, 509); assert_eq!((*pawn).focus.z, 17., "walking TurnTo keeps current height");
        (*pawn).base.rotation.yaw = (*pawn).base.desired_rotation.yaw;
        poll(access, pawn, 509); assert_eq!((*state).latent_action, 0, "completed rotation clears TurnTo");
        (*pawn).base.physics = 4; (*pawn).set_b_can_strafe(false); (*pawn).base.rotation.pitch = 0; (*pawn).base.rotation.yaw = 0; (*pawn).base.rotation.roll = 0;
        call(access, pawn, 510, object_arg(goal.cast()), false);
        assert_eq!((*state).latent_action, 511); assert_eq!({ (*pawn).face_target }, goal.cast()); assert_eq!((*pawn).base.acceleration, vector(300., 0., 0.), "airborne turns retain forward acceleration");
        (*pawn).base.rotation.yaw = (*pawn).base.desired_rotation.yaw; poll(access, pawn, 511); assert_eq!((*state).latent_action, 0);
        for native in [502, 506, 510] {
            (*state).latent_action = 77; let old_dest = (*pawn).destination; let old_face = (*pawn).face_target;
            let args = match native { 502 => vec![0x2a, 0x0b], 506 => { let mut a = vector_arg(dest); a.push(0x2a); a }, _ => vec![0x2a] };
            call(access, pawn, native, args, false); assert_eq!((*state).latent_action, 77); assert_eq!((*pawn).destination, old_dest); assert_eq!({ (*pawn).face_target }, old_face);
        }
        // Low gravity compares the live zone with its class default, not itself.
        let default = access.default_object((*info.cast::<UObject>()).class.cast()).unwrap().cast::<ALevelInfo>();
        (*default).base.zone_gravity.z = -950.; (*info).base.zone_gravity.z = -100.;
        (*pawn).base.physics = 2; (*pawn).base.location = vector(0., 0., 0.); (*pawn).base.velocity = vector(0., 0., -20.);
        let mut args = vector_arg(vector(300., 0., 200.)); args.push(0x0b); call(access, pawn, 500, args, false);
        poll(access, pawn, 501); assert_eq!((*state).latent_action, 0, "falling low-gravity pawn cannot rise to the destination"); assert_eq!((*pawn).base.acceleration, vector(300., 0., 0.));
        (*info).time_seconds = 1.; (*level).time_seconds = hp2_core::time::from_float(1.).to_ne_bytes();
        (*pawn).base.instigator = pawn.cast(); (*pawn).base.location = vector(25., 35., 0.);
        (*info).net_mode = 3; call(access, pawn, 512, float_arg(0.6), false); assert_eq!((*pawn).noise1time, 0., "clients do not publish noise");
        (*info).net_mode = 0; call(access, pawn, 512, float_arg(0.6), false);
        assert_eq!((*pawn).noise1time, 1.); assert_eq!((*pawn).noise1loudness, 0.6); assert_eq!((*pawn).noise1spot, (*pawn).base.location);
        // Consumer-visible packed (FLOAT, Actor*) parameters, not just the noise producer's state.
        hear_noise_script(access, listener); (*info).set_b_begun_play(true); (*info).pawn_list = listener.cast();
        (*listener).base.location = (*pawn).base.location; (*listener).desired_speed = -4.; (*listener).move_target = ptr::null_mut();
        (*(*listener.cast::<UObject>()).state_frame.cast::<FStateFrame>()).probe_mask = u64::MAX;
        (*info).time_seconds = 2.; (*level).time_seconds = hp2_core::time::from_float(2.).to_ne_bytes();
        call(access, pawn, 512, float_arg(0.7), false);
        assert_eq!((*listener).desired_speed, 0.7); assert_eq!({ (*listener).move_target }, pawn.cast(), "HearNoise receives its source actor at packed offset 4");
        (*listener).desired_speed = -4.; call(access, pawn, 512, float_arg(0.6), false); assert_eq!((*listener).desired_speed, -4., "recent quieter noise is throttled before event dispatch");
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nall_thirteen_movement_VM_natives=ok\nlatent_poll_cursors_and_transitions=ok\nnull_goals_and_speed_clamping=ok\nstrafe_destination_and_turn_focus=ok\nlow_gravity_compares_class_default=ok\nMakeNoise_client_and_throttle=ok\nHearNoise_script_packed_parameters=ok\nshutdown=ok\n").unwrap();
}
