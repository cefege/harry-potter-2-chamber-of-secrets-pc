//! Fast-path `Hp2LevelSpawnActorServices` mock for consumer tests whose code
//! under test now calls the real Rust `ULevel::SpawnActor`
//! (`hp2_engine::level_spawn_actor`) directly, with every optional argument
//! defaulted exactly like the `*Services.cpp` leaves this replaces
//! (`SpawnActor(Class)` / `SpawnActor(Class, NAME_None, NULL, NULL,
//! FVector(0,0,0), FRotator(0,0,0), NULL, 0, 0)`). Every policy step except
//! `Construct` takes the success path that call always took; `construct`
//! receives the real context (so it can read the caller's `Class`) and must
//! write the spawned actor through `out`, exactly like the leaf it replaces.

use std::ffi::c_void;
use std::sync::{LazyLock, Once, OnceLock};

use hp2_engine::level_spawn_actor::{self, Hp2LevelSpawnActorServices, Hp2SpawnActorContext, Op};
use hp2_abi::FVector;
use hp2_abi::engine::generated::{FName, FRotator};

pub type Construct = unsafe extern "C" fn(*const Hp2SpawnActorContext, usize, *mut usize) -> i32;

static CONSTRUCT: OnceLock<Construct> = OnceLock::new();

/// Publish once per test binary; the first caller's `construct` callback
/// serves every test, like the `pawn_moves::publish` pilot.
pub fn publish(construct: Construct) {
    static TABLE: LazyLock<Hp2LevelSpawnActorServices> = LazyLock::new(|| {
        Hp2LevelSpawnActorServices {
            call,
            virtual_spawn_actor,
        }
    });
    static PUBLISHED: Once = Once::new();
    CONSTRUCT.set(construct).ok();
    PUBLISHED.call_once(|| {
        // SAFETY: published once, before any reader in this process.
        unsafe { level_spawn_actor::publish_services(&TABLE) };
    });
}

unsafe extern "C" fn call(
    context: *mut c_void,
    op: u32,
    a: usize,
    _b: usize,
    out: *mut usize,
) -> i32 {
    let ctx = context.cast::<Hp2SpawnActorContext>();
    if op == Op::Construct as u32 {
        let construct = CONSTRUCT
            .get()
            .expect("level_spawn_actor_mock::publish first");
        return unsafe { construct(ctx, a, out) };
    }
    let value: usize = if op == Op::Class as u32 {
        unsafe { (*ctx).class as usize }
    } else if op == Op::ClassIsActor as u32
        || op == Op::Editor as u32
        || op == Op::Template as u32
        || op == Op::Base as u32
    {
        1
    } else {
        0
    };
    unsafe { *out = value };
    0
}

/// Mimics stock (non-overridden) `ULevel::SpawnActor` by calling straight
/// back into [`level_spawn_actor::spawn_actor`], matching what the real
/// `Hp2SpawnActorVirtualSpawn` C++ leaf reaches via
/// `((ULevel*)Level)->SpawnActor(...)` when nothing overrides it.
unsafe extern "C" fn virtual_spawn_actor(
    level: *mut c_void,
    class: *mut c_void,
    in_name_index: i32,
    owner: *mut c_void,
    instigator: *mut c_void,
    location: *const c_void,
    rotation: *const c_void,
    template: *mut c_void,
    no_collision_fail: i32,
    remote_owned: i32,
    out: *mut usize,
) -> i32 {
    let location = unsafe { *location.cast::<FVector>() };
    let rotation = unsafe { *rotation.cast::<FRotator>() };
    match unsafe {
        level_spawn_actor::spawn_actor(
            level,
            class,
            FName { index: in_name_index },
            owner,
            instigator,
            location,
            rotation,
            template,
            no_collision_fail,
            remote_owned,
        )
    } {
        Ok(value) => {
            unsafe { *out = value };
            0
        }
        Err(status) => status,
    }
}
