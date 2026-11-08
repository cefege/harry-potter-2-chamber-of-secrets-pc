use super::{execute_actor_event, HitWallParams, InterpolateEndParams};
use crate::EngineOwner;
use hp2_abi::{FVector, engine::generated::{
    AActor, ALevelInfo, FStateFrame, TArrayRaw, UBoolProperty, UClass, UField, UFunction,
    ULevel, UObject, UObjectProperty, UProperty, UState, UStruct, UStructProperty,
}};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult},
    name_owner::FindName,
};
use std::{fs, io::Write, path::{Path, PathBuf}, ptr};

fn output(access: CoreAccess<'_>, category: i32, text: &[u32]) -> RuntimeResult<()> {
    access.write_log_record(&access.name_text(category)?, text)
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn owner(label: &str) -> EngineOwner {
    let data = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype data"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("physics-events-{label}-{}", std::process::id()));
    copy_tree(&data.join("System"), &root.join("System"));
    for directory in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(directory);
        if source.is_dir() {
            std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(directory)).unwrap();
        }
    }
    let mut log = fs::File::create(root.join("runtime.log")).unwrap();
    log.write_all(&[0xff, 0xfe]).unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output,
        log_file: Some(log),
    }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode {
        is_client: true, is_server: true, is_scriptable: true, ..access.process_mode()
    });
    unsafe { access.load_package(&root.join("System/Engine.u"), 1) }.unwrap();
    owner
}

unsafe fn reflected_class(access: CoreAccess<'_>, name: &str) -> *mut UClass {
    let text: Vec<u32> = name.chars().map(u32::from).chain([0]).collect();
    unsafe { access.static_load_class(access.native_class("Core.Object").unwrap(), ptr::null_mut(), &text, 1) }
        .unwrap_or_else(|error| panic!("load prototype class {name}: {error}"))
}

unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject, name: &str) -> *mut UObject {
    unsafe {
        access.construct_object(reflected_class(access, class), outer,
            access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut())
    }.unwrap()
}

unsafe fn attach_actor(level: *mut ULevel, info: *mut ALevelInfo, actor: *mut AActor) {
    unsafe {
        (*actor).x_level = level.cast();
        (*actor).level = info.cast();
        (*actor).region.zone = info.cast();
        let raw = ptr::addr_of_mut!((*level).base.actors).cast::<TArrayRaw>();
        let index = array_storage::add(Hp2ArraySlots {
            data: &raw mut (*raw).data, num: &raw mut (*raw).num, max: &raw mut (*raw).max,
        }, 1, size_of::<*mut AActor>() as i32, ptr::null(), array_storage::system_allocator()).unwrap();
        (*raw).data.cast::<*mut AActor>().add(index as usize).write(actor);
    }
}

unsafe fn world(access: CoreAccess<'_>) -> (*mut ULevel, *mut ALevelInfo) {
    unsafe {
        let level = construct(access, "Engine.Level", access.transient_package(), "World").cast::<ULevel>();
        let info = construct(access, "Engine.LevelInfo", level.cast(), "WorldInfo").cast::<ALevelInfo>();
        (*info).set_b_begun_play(true);
        attach_actor(level, info, info.cast());
        (level, info)
    }
}

unsafe fn world_actor(access: CoreAccess<'_>, level: *mut ULevel, info: *mut ALevelInfo, class: &str, name: &str) -> *mut AActor {
    unsafe {
        let actor = construct(access, class, level.cast(), name).cast::<AActor>();
        attach_actor(level, info, actor);
        actor
    }
}

unsafe fn field(access: CoreAccess<'_>, owner: *mut UStruct, name: &str, kind: &str) -> *mut UObject {
    let found = unsafe { access.find_field(owner, access.intern_str(name, FindName::Find).unwrap(),
        access.native_class(kind).unwrap()) };
    assert!(!found.is_null(), "actual prototype field {name}");
    found.cast()
}

unsafe fn script_event(access: CoreAccess<'_>, owner: *mut UStruct, source: *mut UFunction) -> *mut UFunction {
    unsafe {
        let name = i32::from_ne_bytes((*source.cast::<UObject>()).name);
        let function = access.construct_object(access.native_class("Core.Function").unwrap(),
            owner.cast(), name, 0x4000, ptr::null_mut()).unwrap().cast::<UFunction>();
        (*function.cast::<UField>()).next = (*owner).children;
        (*owner).children = function.cast();
        (*function).function_flags = 2;
        (*function).parms_size = (*source).parms_size;
        (*function).num_parms = (*source).num_parms;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_size = i32::from((*source).parms_size);
        (*function.cast::<UStruct>()).properties_align = 4;
        let mut child = (*source.cast::<UStruct>()).children.cast::<UField>();
        while !child.is_null() {
            if hp2_core::object_model::is_a(child.cast(), access.native_class("Core.Property").unwrap().cast()) {
                let original = child.cast::<UProperty>();
                if (*original).property_flags & 0x80 == 0 { break; }
                let property = access.construct_object((*child.cast::<UObject>()).class.cast(),
                    function.cast(), i32::from_ne_bytes((*child.cast::<UObject>()).name),
                    0x4000, ptr::null_mut()).unwrap().cast::<UProperty>();
                (*property).offset = (*original).offset;
                (*property).element_size = (*original).element_size;
                (*property).array_dim = (*original).array_dim;
                (*property).property_flags = (*original).property_flags;
                if hp2_core::object_model::is_a(original.cast(), access.native_class("Core.StructProperty").unwrap().cast()) {
                    (*property.cast::<UStructProperty>()).r#struct = (*original.cast::<UStructProperty>()).r#struct;
                } else if hp2_core::object_model::is_a(original.cast(), access.native_class("Core.ObjectProperty").unwrap().cast()) {
                    (*property.cast::<UObjectProperty>()).property_class = (*original.cast::<UObjectProperty>()).property_class;
                } else if hp2_core::object_model::is_a(original.cast(), access.native_class("Core.BoolProperty").unwrap().cast()) {
                    (*property.cast::<UBoolProperty>()).bit_mask = (*original.cast::<UBoolProperty>()).bit_mask;
                } else {
                    panic!("unexpected actual physics parameter kind");
                }
                (*property.cast::<UField>()).next = (*function.cast::<UStruct>()).children;
                (*function.cast::<UStruct>()).children = property.cast();
            }
            child = (*child).next.cast();
        }
        function
    }
}

unsafe fn script(access: CoreAccess<'_>, function: *mut UFunction, code: &[u8]) {
    unsafe {
        let raw = ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast::<TArrayRaw>();
        array_storage::set(Hp2ArraySlots { data: &raw mut (*raw).data, num: &raw mut (*raw).num,
            max: &raw mut (*raw).max }, code.len() as i32, 1, ptr::null(),
            array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(code.as_ptr(), (*raw).data.cast::<u8>(), code.len());
    }
}

unsafe fn assignment(code: &mut Vec<u8>, destination: *mut UObject, source: *mut UObject, boolean: bool) {
    code.extend(if boolean { &[0x14, 0x2d, 0x01][..] } else { &[0x0f, 0x01][..] });
    code.extend(unsafe { (*destination).index }.to_le_bytes());
    if boolean { code.push(0x2d); }
    code.push(0x00);
    code.extend(unsafe { (*source).index }.to_le_bytes());
}

unsafe fn set_owner_call(code: &mut Vec<u8>, function: *mut UObject, source: *mut UObject) {
    code.push(0x1c); // FinalFunction: Actor.SetOwner is final.
    code.extend(unsafe { (*function).index }.to_le_bytes());
    code.push(0x00); // LocalVariable actor argument.
    code.extend(unsafe { (*source).index }.to_le_bytes());
    code.push(0x16);
}

#[test]
fn startup_rebase_preserves_none_physics_until_base_change_is_requested() {
    let owner = owner("startup-rebase");
    let access = owner.access();
    unsafe {
        let (level, info) = world(access);
        let pawn = world_actor(access, level, info, "Engine.Scout", "RebasedPawn");
        (*pawn).physics = crate::lev_tic_actor::PHYS_NONE;

        crate::level_lifecycle_owner::set_base(access, pawn, info.cast(), false).unwrap();
        crate::level_lifecycle_owner::set_base(access, pawn, ptr::null_mut(), false).unwrap();
        assert!((*pawn).base.is_null());
        assert_eq!((*pawn).physics, crate::lev_tic_actor::PHYS_NONE);

        // The same real Pawn.BaseChange script must still run for normal rebases.
        crate::level_lifecycle_owner::set_base(access, pawn, info.cast(), false).unwrap();
        crate::level_lifecycle_owner::set_base(access, pawn, ptr::null_mut(), true).unwrap();
        assert!((*pawn).base.is_null());
        assert_eq!((*pawn).physics, crate::lev_tic_actor::PHYS_FALLING);
    }
}

#[test]
fn hit_wall_delivers_both_packed_arguments_to_the_active_state_override() {
    let owner = owner("hit-wall");
    let access = owner.access();
    unsafe {
        let class = reflected_class(access, "Engine.InterpolationPoint");
        let (level, info) = world(access);
        let receiver = world_actor(access, level, info, "Engine.InterpolationPoint", "Receiver");
        let other = world_actor(access, level, info, "Engine.InterpolationPoint", "Wall");
        let original = field(access, class.cast(), "HitWall", "Core.Function").cast::<UFunction>();
        let fallback = script_event(access, class.cast(), original);
        script(access, fallback, &[0x04, 0x0b]);
        let state = construct(access, "Core.State", class.cast(), "CollisionState").cast::<UState>();
        let function = script_event(access, state.cast(), original);
        let velocity = field(access, class.cast(), "Velocity", "Core.StructProperty");
        let set_owner = field(access, class.cast(), "SetOwner", "Core.Function");
        let normal = field(access, function.cast(), "HitNormal", "Core.StructProperty");
        let wall = field(access, function.cast(), "HitWall", "Core.ObjectProperty");
        let mut code = Vec::new();
        assignment(&mut code, velocity, normal, false);
        set_owner_call(&mut code, set_owner, wall);
        code.extend([0x04, 0x0b]);
        script(access, function, &code);
        access.initialize_execution(receiver.cast()).unwrap();
        (*(*receiver.cast::<UObject>()).state_frame.cast::<FStateFrame>()).state_node = state.cast();
        let expected = FVector { x: 0.25, y: -0.5, z: 1.0 };
        let mut parameters = HitWallParams { normal: expected, hit_actor: other.cast() };
        execute_actor_event(access, receiver.cast(), "HitWall", (&raw mut parameters).cast(), size_of::<HitWallParams>()).unwrap();
        assert_eq!((*receiver).velocity, expected);
        assert_eq!(ptr::addr_of!((*receiver).owner).read_unaligned(), other.cast());
        (*function).parms_size += 4;
        (*function.cast::<UStruct>()).properties_size += 4;
        parameters.normal = FVector { x: 8.0, y: 9.0, z: 10.0 };
        let mut backing = [0_u128; 2];
        ptr::copy_nonoverlapping((&raw const parameters).cast::<u8>(),
            backing.as_mut_ptr().cast::<u8>(), size_of::<HitWallParams>());
        assert!(execute_actor_event(access, receiver.cast(), "HitWall",
            backing.as_mut_ptr().cast(), size_of::<HitWallParams>()).is_err());
        assert_eq!((*receiver).velocity, expected, "invalid extents fail before VM copying or execution");
        (*function).parms_size -= 4;
        (*function.cast::<UStruct>()).properties_size -= 4;
    }
    owner.shutdown().unwrap();
}

#[test]
fn interpolate_end_delivers_manager_and_bool_without_host_tail_padding() {
    let owner = owner("interpolate-end");
    let access = owner.access();
    unsafe {
        let class = reflected_class(access, "Engine.InterpolationPoint");
        let (level, info) = world(access);
        let receiver = world_actor(access, level, info, "Engine.InterpolationPoint", "Point");
        let manager = world_actor(access, level, info, "Engine.InterpolationManager", "Manager");
        let original = field(access, class.cast(), "InterpolateEnd", "Core.Function").cast::<UFunction>();
        let state = construct(access, "Core.State", class.cast(), "InterpolationState").cast::<UState>();
        let function = script_event(access, state.cast(), original);
        let set_owner = field(access, class.cast(), "SetOwner", "Core.Function");
        let visibility = field(access, class.cast(), "bHidden", "Core.BoolProperty");
        let other = field(access, function.cast(), "Other", "Core.ObjectProperty");
        let forward = field(access, function.cast(), "bForward", "Core.BoolProperty");
        let mut code = Vec::new();
        set_owner_call(&mut code, set_owner, other);
        assignment(&mut code, visibility, forward, true);
        code.extend([0x04, 0x0b]);
        script(access, function, &code);
        access.initialize_execution(receiver.cast()).unwrap();
        (*(*receiver.cast::<UObject>()).state_frame.cast::<FStateFrame>()).state_node = state.cast();
        (*receiver).set_b_hidden(false);
        let mut parameters = InterpolateEndParams { manager: manager.cast(), forward: 1 };
        execute_actor_event(access, receiver.cast(), "InterpolateEnd", (&raw mut parameters).cast(), size_of::<InterpolateEndParams>()).unwrap();
        assert_eq!(ptr::addr_of!((*receiver).owner).read_unaligned(), manager.cast());
        assert!((*receiver).b_hidden());
        parameters.forward = 0;
        execute_actor_event(access, receiver.cast(), "InterpolateEnd", (&raw mut parameters).cast(), size_of::<InterpolateEndParams>()).unwrap();
        assert!(!(*receiver).b_hidden(), "the second reflected bool value must clear the destination mask");
    }
    owner.shutdown().unwrap();
}

#[test]
fn interpolation_manager_virtual_physics_moves_its_real_owner() {
    use hp2_abi::engine::generated::{AInterpolationManager, AInterpolationPoint};

    let engine = owner("interpolation-manager-motion");
    let access = engine.access();
    unsafe {
        let (level, info) = world(access);
        (*level).model = construct(access, "Engine.Model", level.cast(), "SplineModel").cast();
        let moving = world_actor(access, level, info, "Engine.Scout", "SplineOwner");
        (*moving).set_b_collide_actors(false);
        (*moving).set_b_collide_world(false);
        (*moving).set_b_interpolating(true);
        (*moving).location = FVector { x: 0.0, y: 0.0, z: 0.0 };
        (*moving).velocity = FVector { x: 0.0, y: 0.0, z: 0.0 };
        (*moving).ip_speed = 128.0;
        let destination = world_actor(
            access, level, info, "Engine.InterpolationPoint", "SplineDestination",
        ).cast::<AInterpolationPoint>();
        (*destination.cast::<AActor>()).location = FVector { x: 128.0, y: 0.0, z: 0.0 };
        (*destination).path_dist = 128.0;
        let manager = world_actor(
            access, level, info, "Engine.InterpolationManager", "SplineManager",
        ).cast::<AInterpolationManager>();
        (*manager.cast::<AActor>()).owner = moving.cast();
        (*manager.cast::<AActor>()).physics = 8;
        #[repr(C)]
        struct InitParams {
            destination: *mut AInterpolationPoint,
            rate: f32,
            jump_to_start: u32,
        }
        let mut init = InitParams { destination, rate: 1.0, jump_to_start: 0 };
        execute_actor_event(
            access, manager.cast(), "Init", (&mut init as *mut InitParams).cast(),
            size_of::<InitParams>(),
        ).unwrap();

        super::perform_physics(access, manager.cast(), 1.0).unwrap();

        let endpoint = FVector { x: 128.0, y: 0.0, z: 0.0 };
        assert_eq!((*moving).location, endpoint, "manager physics moves its owner, not itself");
        assert_eq!((*moving).velocity, endpoint, "the one-second segment publishes owner velocity");
        assert_eq!((*moving).acceleration, endpoint, "owner acceleration uses the previous velocity");
    }
    engine.shutdown().unwrap();
}
