use hp2_abi::engine::generated::{
    AActor, ALevelInfo, FName, FRotator, TArrayRaw, ULevel, UModel, UObject,
};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_engine::{EngineOwner, level_spawn_actor_owner};
use std::{path::PathBuf, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe {
        access.construct_object(
            access
                .native_class(class)
                .expect("registered canonical class"),
            outer,
            0,
            0,
            ptr::null_mut(),
        )
    }
    .expect("construct fixture in canonical object storage")
}

#[test]
fn rejected_static_class_does_not_publish_and_spawned_actor_retains_world_identity() {
    let artifact =
        PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated test artifacts"));
    assert!(
        artifact.is_dir(),
        "artifact directory must exist before the consumer starts"
    );
    let root = artifact.join(format!("spawn-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).expect("create data-none fixture");
    std::fs::write(root.join("System/Default.ini"), "").expect("empty configuration");
    std::fs::write(root.join("System/DefUser.ini"), "").expect("empty user configuration");
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("start canonical runtime without game data");
    let access = owner.access();
    let level =
        unsafe { construct(access, "Engine.Level", access.transient_package()) }.cast::<ULevel>();
    let info = unsafe { construct(access, "Engine.LevelInfo", level.cast()) }.cast::<ALevelInfo>();
    let model = unsafe { construct(access, "Engine.Model", level.cast()) }.cast::<UModel>();
    let template = unsafe { construct(access, "Engine.Trigger", level.cast()) }.cast::<AActor>();
    unsafe {
        (*level).model = model.cast();
        (*model).root_outside = 1;
        (*info.cast::<AActor>()).level = info.cast();
        (*info.cast::<AActor>()).x_level = level.cast();
        let array = ptr::addr_of_mut!((*level).base.actors).cast::<TArrayRaw>();
        let slots = Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        };
        array_storage::add_zeroed(
            slots,
            1,
            std::mem::size_of::<*mut AActor>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
        .expect("allocate actual Level actor slots");
        (*array).data.cast::<*mut AActor>().write(info.cast());
        // Explicit data-none template: production defaults normally come from Engine.u.
        (*template).role = 4; // ROLE_Authority.
        (*template).set_b_no_delete(false);
        (*template).set_b_static(false);
        (*template).set_b_collide_world(false);
        (*template).set_b_collide_when_placing(false);
        (*template).set_b_collide_actors(false);
    }
    let class = access
        .native_class("Engine.Trigger")
        .expect("canonical concrete Actor class");
    let default_actor = unsafe { access.default_object(class) }
        .expect("canonical class defaults")
        .cast::<AActor>();
    unsafe {
        (*default_actor).set_b_no_delete(false);
        (*default_actor).set_b_static(true);
    }
    let spawn = || unsafe {
        level_spawn_actor_owner::spawn_actor(
            access,
            level,
            class,
            FName { index: 0 },
            ptr::null_mut(),
            ptr::null_mut(),
            hp2_abi::FVector {
                x: 11.0,
                y: 13.0,
                z: 17.0,
            },
            FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            template,
            false,
            false,
        )
    };
    assert!(
        spawn()
            .expect("static class rejection is not a runtime error")
            .is_null()
    );
    assert_eq!(
        unsafe { (*level).base.actors.array_num },
        1,
        "rejection must not reserve a Level slot"
    );
    unsafe {
        (*default_actor).set_b_static(false);
    }
    let actor = spawn().expect("spawn through canonical world operations");
    assert!(
        !actor.is_null(),
        "a fitting concrete non-static template must spawn"
    );
    unsafe {
        assert_eq!((*level).base.actors.array_num, 2);
        assert_eq!(
            (*level)
                .base
                .actors
                .data
                .cast::<*mut AActor>()
                .add(1)
                .read(),
            actor
        );
        assert_eq!({ (*actor).x_level }, level.cast());
        assert_eq!({ (*actor).level }, info.cast());
        assert_eq!({ (*actor).region.zone }, info.cast());
        assert_eq!(
            (*actor).location,
            hp2_abi::FVector {
                x: 11.0,
                y: 13.0,
                z: 17.0
            }
        );
        assert_eq!((*actor.cast::<UObject>()).class, class.cast());
        assert!(
            actor.cast::<*mut std::ffi::c_void>().read().is_null(),
            "spawn must not construct a C++ UObject"
        );
        assert!(
            !(*actor.cast::<UObject>()).state_frame.is_null(),
            "positive Actor execution must initialize its real frame"
        );
    }
    owner.shutdown().expect("destroy the canonical world graph");
}
