use hp2_abi::{FVector, engine::generated::{AActor, ALevelInfo, FFrame, FMeshAnimSeq, FMeshBone, FStateFrame, TArrayRaw, UAnimation, UFunction, ULevel, UMesh, UObject, USkeletalMesh, UStruct}};
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
fn vector(x: f32, y: f32, z: f32) -> FVector { FVector { x, y, z } }
fn name_arg(name: i32) -> Vec<u8> { let mut code = vec![0x21]; code.extend(name.to_le_bytes()); code }
fn float_arg(value: f32) -> Vec<u8> { let mut code = vec![0x1e]; code.extend(value.to_le_bytes()); code }
fn int_arg(value: i32) -> Vec<u8> { let mut code = vec![0x1d]; code.extend(value.to_le_bytes()); code }
unsafe fn object_arg(object: *mut UObject) -> Vec<u8> {
    if object.is_null() { vec![0x2a] } else { let mut code = vec![0x20]; code.extend(unsafe { (*object).index }.to_le_bytes()); code }
}
unsafe fn call(access: CoreAccess<'_>, actor: *mut AActor, native: u32, mut code: Vec<u8>, poll: bool, mut result: [u128; 2]) -> [u128; 2] {
    unsafe {
        let function = construct(access, "Core.Function", (*actor.cast::<UObject>()).class.cast()).cast::<UFunction>();
        if !poll { code.push(0x16); }
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let start = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut frame = FFrame { node: function.cast(), object: actor.cast(), code: start, locals: ptr::null_mut(), parent: ptr::null_mut() };
        access.execute_vm_native_index(actor.cast(), native, &raw mut frame, result.as_mut_ptr().cast()).unwrap();
        assert_eq!(frame.code, start.add(if poll { 0 } else { code.len() }), "native {native}: latent polls must not consume the next instruction");
        result
    }
}
unsafe fn invoke(access: CoreAccess<'_>, actor: *mut AActor, native: u32, code: Vec<u8>) -> [u128; 2] {
    unsafe { call(access, actor, native, code, false, [u128::MAX; 2]) }
}
unsafe fn poll(access: CoreAccess<'_>, actor: *mut AActor, native: u32, seconds: f32) {
    let mut input = [0_u128; 2]; unsafe { input.as_mut_ptr().cast::<f32>().write(seconds); call(access, actor, native, vec![0xff], true, input); }
}
fn word(result: [u128; 2]) -> u32 { result[0] as u32 }
unsafe fn sequence(access: CoreAccess<'_>, name: &str, group: &str, frames: i32, rate: f32) -> FMeshAnimSeq {
    let mut seq: FMeshAnimSeq = unsafe { std::mem::zeroed() };
    seq.name = access.intern_str(name, FindName::Add).unwrap().to_ne_bytes();
    seq.group = access.intern_str(group, FindName::Add).unwrap().to_ne_bytes(); seq.num_frames = frames; seq.rate = rate; seq
}
unsafe fn new_channel(access: CoreAccess<'_>, actor: *mut AActor, bone: i32, transient: bool, protected: bool) -> *mut AActor {
    unsafe {
        let mut args = object_arg(access.native_class("Engine.Pawn").unwrap().cast()); args.extend([0x24, 1]); args.extend(name_arg(bone));
        args.push(if transient { 0x27 } else { 0x28 }); args.push(if protected { 0x27 } else { 0x28 });
        let result = invoke(access, actor, 265, args); result[0] as usize as *mut AActor
    }
}
// A real Core-owned, preloaded script subclass; no Engine.u or mocked loader.
unsafe fn author_anim_channel_class(access: CoreAccess<'_>) {
    unsafe {
        let parent = access.native_class("Engine.Pawn").unwrap();
        let outer = (*parent.cast::<UObject>()).outer.cast();
        let class = access.construct_object(access.native_class("Core.Class").unwrap(), outer,
            access.intern_str("AnimChannel", FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap()
            .cast::<hp2_abi::engine::generated::UClass>();
        (*class).super_field = parent.cast();
        (*class).properties_size = (*parent).properties_size;
        (*class).class_within = (*parent).class_within;
        (*class).class_constructor = (*parent).class_constructor;
        let defaults = ptr::addr_of!((*parent).defaults).read_unaligned();
        let bytes = std::slice::from_raw_parts(defaults.data.cast::<u8>(), defaults.num as usize);
        set_array(ptr::addr_of_mut!((*class).defaults), bytes);
        (*(*class).defaults.data.cast::<UObject>()).class = class.cast();
    }
}
#[test]
fn canonical_script_natives_drive_latent_actions_animation_channels_and_skeletal_pose_queries() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("actor-script-vm-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap(); std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    let access = owner.access();
    unsafe {
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<hp2_abi::engine::generated::UEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        (*level).model = construct(access, "Engine.Model", level.cast()).cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let brush = construct(access, "Engine.Brush", level.cast()).cast::<AActor>();
        let actor = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let actors = [info.cast::<AActor>(), brush, actor]; set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for p in actors { (*p).x_level = level.cast(); (*p).level = info.cast(); (*p).region.zone = info.cast(); }
        access.initialize_execution(actor.cast()).unwrap();
        let state = (*actor.cast::<UObject>()).state_frame.cast::<FStateFrame>();
        invoke(access, actor, 256, float_arg(0.375)); assert_eq!((*state).latent_action, 384); assert_eq!((*actor).latent_float, 0.375);
        poll(access, actor, 384, 0.25); assert_eq!((*actor).latent_float, 0.125); assert_eq!((*state).latent_action, 384, "half-tick equality does not wake Sleep");
        poll(access, actor, 384, 0.125); assert_eq!((*state).latent_action, 0);
        (*actor).set_b_interpolating(true); invoke(access, actor, 301, vec![]); poll(access, actor, 302, 0.);
        assert_eq!((*state).latent_action, 302); (*actor).set_b_interpolating(false); poll(access, actor, 302, 0.); assert_eq!((*state).latent_action, 0);
        (*actor).location = vector(10., 20., 30.); (*actor).rotation.pitch = 100; (*actor).rotation.yaw = 200; (*actor).rotation.roll = 300;
        assert_eq!(word(invoke(access, actor, 268, name_arg(0))), 0); assert_eq!(word(invoke(access, actor, 269, int_arg(7))), 0);
        let pos = invoke(access, actor, 257, name_arg(0)); assert_eq!(pos.as_ptr().cast::<FVector>().read_unaligned(), (*actor).location);
        let rot = invoke(access, actor, 328, name_arg(0)); assert_eq!(rot.as_ptr().cast::<[i32; 3]>().read_unaligned(), [100, 200, 300]);
        assert_eq!(word(invoke(access, actor, 263, name_arg(0))), u32::MAX, "HasAnim without a mesh preserves the caller's result");
        assert_eq!(word(invoke(access, actor, 293, name_arg(0))), 0);
        let mut no_channel = vec![0x2a, 0x24, 0]; no_channel.extend(name_arg(0)); no_channel.extend([0x0b, 0x0b]);
        assert_eq!(invoke(access, actor, 265, no_channel)[0] as usize, 0, "no mesh means no channel");
        let mesh = construct(access, "Engine.Mesh", access.transient_package()).cast::<UMesh>();
        let seq = sequence(access, "Run", "Locomotion", 8, 16.); set_array(ptr::addr_of_mut!((*mesh).anim_seqs).cast(), &[seq]); (*actor).mesh = mesh.cast();
        let run = i32::from_ne_bytes(seq.name); let group = i32::from_ne_bytes(seq.group);
        let mut args = name_arg(run); args.extend([0x0b; 4]); invoke(access, actor, 259, args);
        assert_eq!((*actor).anim_sequence, seq.name); assert_eq!((*actor).anim_rate, 2.); assert_eq!((*actor).anim_last, 0.875); assert!(!(*actor).b_anim_loop());
        assert_eq!(word(invoke(access, actor, 282, vec![0x0b])), 1);
        assert_eq!(word(invoke(access, actor, 293, name_arg(run))), group as u32); assert_eq!(word(invoke(access, actor, 263, name_arg(run))), 1);
        let absent = access.intern_str("Absent", FindName::Add).unwrap();
        assert_eq!(word(invoke(access, actor, 263, name_arg(absent))), 0); assert_eq!(word(invoke(access, actor, 293, name_arg(absent))), 0);
        let mut args = name_arg(run); args.extend(float_arg(0.5)); args.extend([0x0b, 0x0b, 0x0b, 0x0b]); invoke(access, actor, 260, args);
        assert_eq!((*actor).anim_rate, 1.); assert!((*actor).b_anim_loop());
        (*actor).anim_frame = 0.25; invoke(access, actor, 261, vec![0x0b]); assert_eq!((*state).latent_action, 385); assert!(!(*actor).b_anim_loop());
        poll(access, actor, 385, 0.); assert_eq!((*state).latent_action, 385); (*actor).set_b_anim_finished(true); poll(access, actor, 385, 0.); assert_eq!((*state).latent_action, 0);
        let mut args = name_arg(run); args.extend(float_arg(0.5)); invoke(access, actor, 294, args);
        assert_eq!((*actor).anim_rate, 0.); assert_eq!((*actor).tween_rate, 2.); assert!(!(*actor).b_anim_loop());
        // FName equality is case-insensitive, including the special Move root.
        let move_root = access.intern_str("mOvE", FindName::Add).unwrap(); let mut args = name_arg(run); args.extend([0x0b, 0x0b, 0x0b]); args.extend(name_arg(move_root)); invoke(access, actor, 259, args);
        assert!((*actor).b_anim_move(), "Move root selection must use FName equality, not case-sensitive text");
        let skeletal = construct(access, "Engine.SkeletalMesh", access.transient_package()).cast::<USkeletalMesh>();
        let root_name = access.intern_str("root", FindName::Add).unwrap(); let child_name = access.intern_str("hand", FindName::Add).unwrap();
        let mut root_bone: FMeshBone = std::mem::zeroed(); root_bone.name = root_name.to_ne_bytes(); root_bone.bone_pos.place.quat = [0., 0., 0., 1.]; root_bone.parent_index = 0; root_bone.num_children = 1;
        let mut child_bone = root_bone; child_bone.name = child_name.to_ne_bytes(); child_bone.num_children = 0; child_bone.depth = 1; child_bone.bone_pos.place.pos = vector(5., 0., 0.); child_bone.bone_pos.place.quat = [0., 0., std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2];
        set_array(ptr::addr_of_mut!((*skeletal).ref_skeleton).cast(), &[root_bone, child_bone]);
        (*skeletal).scale = vector(1., 1., 1.); (*skeletal).origin = vector(0., 0., 0.); (*actor).draw_scale = 1.; (*actor).wideness = 128;
        (*actor).rotation.pitch = 0; (*actor).rotation.yaw = 0; (*actor).rotation.roll = 0; (*actor).set_b_anim_move(false);
        let animation = construct(access, "Engine.Animation", access.transient_package()).cast::<UAnimation>(); set_array(ptr::addr_of_mut!((*animation).anim_seqs).cast(), &[seq]);
        (*skeletal).default_animation = animation.cast(); (*actor).mesh = skeletal.cast(); (*actor).skel_anim = ptr::null_mut();
        assert_eq!(word(invoke(access, actor, 263, name_arg(run))), 1, "skeletal default animation is selected when no linked animation exists");
        let linked = construct(access, "Engine.Animation", access.transient_package()).cast::<UAnimation>(); let linked_seq = sequence(access, "LinkedRun", "LinkedGroup", 4, 12.);
        set_array(ptr::addr_of_mut!((*linked).anim_seqs).cast(), &[linked_seq]); (*actor).skel_anim = linked.cast();
        assert_eq!(word(invoke(access, actor, 263, name_arg(run))), 0, "an explicit animation replaces rather than supplements the default");
        assert_eq!(word(invoke(access, actor, 293, name_arg(i32::from_ne_bytes(linked_seq.name)))), i32::from_ne_bytes(linked_seq.group) as u32);
        assert_eq!(word(invoke(access, actor, 268, name_arg(child_name))), 1); assert_eq!(word(invoke(access, actor, 268, name_arg(absent))), u32::MAX);
        assert_eq!(word(invoke(access, actor, 269, int_arg(1))), child_name as u32);
        for index in [-1, 2] { assert_eq!(word(invoke(access, actor, 269, int_arg(index))), 0); }
        // Evaluate the real renderer's CPU skeletal pose, without a GPU backend.
        (*actor).anim_sequence = [0; 4]; (*actor).anim_frame = 0.; (*actor).tween_alpha = 0.;
        let pos = invoke(access, actor, 257, name_arg(child_name)); assert_eq!(pos.as_ptr().cast::<FVector>().read_unaligned(), vector(15., 20., 30.));
        let rot = invoke(access, actor, 328, name_arg(child_name)); let angles = rot.as_ptr().cast::<[i32; 3]>().read_unaligned();
        assert_eq!(angles[0], 0); assert!(((angles[1] & 65535) - 16384).abs() <= 1, "authored quarter-turn bone must not return the actor's zero rotation: {angles:?}");
        // No script defaults are loaded in data-none: author an authority channel class.
        let channel_defaults = access.default_object(access.native_class("Engine.Pawn").unwrap()).unwrap().cast::<AActor>();
        (*channel_defaults).role = 4;
        let persistent = new_channel(access, actor, child_name, false, false); assert!(!persistent.is_null());
        let transient = new_channel(access, actor, child_name, true, false); assert!(!transient.is_null());
        let protected = new_channel(access, actor, child_name, true, true); assert!(!protected.is_null());
        assert_ne!(persistent, transient); assert_ne!(transient, protected);
        for channel in [persistent, transient, protected] { assert_eq!({ (*channel).owner }, actor.cast()); (*channel).anim_sequence = linked_seq.name; (*channel).anim_rate = 1.; (*channel).anim_frame = 0.; (*channel).anim_last = 0.75; }
        assert_eq!(word(invoke(access, actor, 282, name_arg(child_name))), 1, "bone IsAnimating sees an auxiliary channel");
        access.initialize_execution(protected.cast()).unwrap(); let channel_state = (*protected.cast::<UObject>()).state_frame.cast::<FStateFrame>();
        (*state).latent_action = 91; (*protected).set_b_anim_loop(true); invoke(access, actor, 261, name_arg(child_name));
        assert_eq!((*channel_state).latent_action, 385); assert_eq!((*state).latent_action, 91, "FinishAnim suspends its selected channel");
        (*protected).set_b_anim_finished(true); poll(access, protected, 385, 0.); assert_eq!((*channel_state).latent_action, 0);
        let mut args = name_arg(i32::from_ne_bytes(linked_seq.name)); args.extend([0x0b; 5]); invoke(access, actor, 260, args);
        let aux = ptr::addr_of!((*actor).aux_anims).read_unaligned(); let retained = std::slice::from_raw_parts(aux.data.cast::<*mut AActor>(), aux.num as usize);
        assert_eq!(retained, &[protected, persistent]); assert!((*transient).b_delete_me()); assert_eq!((*persistent).anim_sequence, [0; 4]); assert_eq!((*protected).anim_sequence, linked_seq.name);
        // Root-specific playback creates transient channels, never reuses the persistent one.
        author_anim_channel_class(access);
        let mut args = name_arg(i32::from_ne_bytes(linked_seq.name)); args.extend([0x0b, 0x0b, 0x0b]); args.extend(name_arg(child_name));
        invoke(access, actor, 259, args);
        let aux = ptr::addr_of!((*actor).aux_anims).read_unaligned();
        let rooted = aux.data.cast::<*mut AActor>().add((aux.num - 1) as usize).read_unaligned();
        assert_ne!(rooted, persistent, "root playback must not reuse a persistent channel");
        assert!(!(*persistent).b_delete_me()); assert!((*rooted).b_anim_transient());
        let mut args = name_arg(i32::from_ne_bytes(linked_seq.name)); args.extend([0x0b; 5]); invoke(access, actor, 260, args);
        let aux = ptr::addr_of!((*actor).aux_anims).read_unaligned();
        assert_eq!(std::slice::from_raw_parts(aux.data.cast::<*mut AActor>(), aux.num as usize), &[protected, persistent]);
        assert!((*rooted).b_delete_me(), "whole-skeleton replacement destroys its transient root channel");
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nall_seventeen_actor_script_VM_natives=ok\nSleep_half_tick_boundary_and_poll_cursors=ok\nFinishInterpolation_and_FinishAnim_transitions=ok\nclassic_and_linked_skeletal_animation_selection=ok\nmissing_mesh_and_missing_sequence_results=ok\nFName_Move_root_case_insensitivity=ok\nreal_CPU_skeletal_bone_pose_and_index_bounds=ok\nauxiliary_channel_finish_and_replacement_lifetimes=ok\nshutdown=ok\n").unwrap();
}

#[test]
fn canonical_spawn_keeps_tags_separate_from_unique_object_names() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("actor-spawn-tag-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output,
        log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()),
    }).unwrap();
    let access = owner.access();
    unsafe {
        let engine = construct(access, "Engine.GameEngine", access.transient_package())
            .cast::<hp2_abi::engine::generated::UEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast();
        (*level).model = construct(access, "Engine.Model", level.cast()).cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let receiver = construct(access, "Engine.Actor", level.cast()).cast::<AActor>();
        let actors = [info.cast::<AActor>(), receiver];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors {
            (*actor).x_level = level.cast();
            (*actor).level = info.cast();
            (*actor).region.zone = info.cast();
        }
        let class = access.native_class("Engine.Actor").unwrap();
        // This data-none fixture authors the authority default normally loaded from Engine.u.
        (*(*class).defaults.data.cast::<AActor>()).role = 4;
        let shared_tag = access.intern_str("SharedSpawnTag", FindName::Add).unwrap();
        let mut spawned = [ptr::null_mut::<AActor>(); 3];
        for (slot, tag) in spawned.iter_mut().zip([shared_tag, shared_tag, 0]) {
            let mut arguments = object_arg(class.cast());
            arguments.extend(object_arg(ptr::null_mut()));
            arguments.extend(name_arg(tag));
            arguments.extend([0x0b, 0x0b]);
            *slot = invoke(access, receiver, 278, arguments)[0] as usize as *mut AActor;
            assert!(!slot.is_null());
            assert_eq!(i32::from_ne_bytes((**slot).tag), tag, "SpawnTag belongs to Actor.Tag, including None");
        }
        assert_ne!(spawned[0], spawned[1], "a shared tag must not replace the first actor");
        let first_name = (*spawned[0].cast::<UObject>()).name;
        let second_name = (*spawned[1].cast::<UObject>()).name;
        assert_ne!(first_name, second_name, "spawned objects retain distinct automatic names");
        assert_ne!(i32::from_ne_bytes(first_name), shared_tag);
        assert_ne!(i32::from_ne_bytes(second_name), shared_tag);
    }
    owner.shutdown().unwrap();
}
