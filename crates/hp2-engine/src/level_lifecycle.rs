//! LevelLifecycle: `ULevel` actor lifecycle functions from
//! `HarryPotter2/Unreal/Engine/Src/UnLevAct.cpp` (oracle ref `da56c30`) —
//! `DestroyActor`, `CompactActors`, `CleanupDestroyed`, `SpawnViewActor`,
//! `SpawnBrush`, `DropToFloor`.
//!
//! The C++ shells in `UnLevAct.cpp` forward to the exports in
//! `crates/hp2-ffi/src/engine_level_lifecycle.rs`, which call the functions
//! here with the [`Hp2LevelLifecycleServices`] table published from the same
//! translation unit. Control flow follows the original line by line; each
//! service wraps one original C++ call or expression, in original order.
//! `check()`/`is_editor()` reuse the already-published `LevActServices`
//! table (`crate::levact`), since both files are the same `ULevel` and the
//! same translation unit publishes both tables at static-init time.
//!
//! Scalar fields of `ULevel`, `AActor`, `UObject`, `APlayerPawn`,
//! `UViewport` and `UNetConnection` are read and written through the
//! generated mirrors in `hp2_abi::engine::generated`. `ULevel::Actors` is
//! read/written directly through `crate::levact::{actor_at, actor_count,
//! set_actor_at}` (no leaf: plain element access into a live `TArrayRaw`,
//! not a transactional `Add`/`Remove`).

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APlayerPawn, FCheckResult, FName, FRotator, FStateFrame, UNetConnection, UObject,
    UViewport,
};

use crate::level_spawn_actor;
use crate::levact::{self, LevelCtx, check};
use crate::native_guard::guarded;

/// `ENetRole::ROLE_Authority` (`EngineClasses.h`).
const ROLE_AUTHORITY: u8 = 4;
/// `EConnectionState::USOCK_Closed` (`UnConn.h`).
const USOCK_CLOSED: i32 = 1;
/// `RF_Transactional` (`UnObjBas.h`).
const RF_TRANSACTIONAL: u32 = 0x0000_0001;
/// `RF_NotForClient` (`UnObjBas.h`).
const RF_NOT_FOR_CLIENT: u32 = 0x0010_0000;
/// `RF_NotForServer` (`UnObjBas.h`).
const RF_NOT_FOR_SERVER: u32 = 0x0020_0000;
/// `SHOW_Frame` (`UnCamera.h`).
const SHOW_FRAME: i32 = 0x0000_0001;
/// `SHOW_Actors` (`UnCamera.h`).
const SHOW_ACTORS: i32 = 0x0000_0008;
/// `SHOW_Brush` (`UnCamera.h`).
const SHOW_BRUSH: i32 = 0x0000_0040;
/// `SHOW_MovingBrushes` (`UnCamera.h`).
const SHOW_MOVING_BRUSHES: i32 = 0x0000_0400;
/// `REN_DynLight` (`UnCamera.h`).
const REN_DYN_LIGHT: i32 = 5;

/// Source policy for the pre-mutation `DestroyActor` rejection/channel-close path.
/// Both the canonical owner and the native compatibility adapter execute this
/// same ordered state machine; only the field/type queries and channel action vary.
pub(crate) trait DestroyActorRuleOps {
    type Error;
    unsafe fn is_editor(&mut self) -> Result<bool, Self::Error>;
    unsafe fn is_static(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn is_no_delete(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn is_delete_me(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn role(&mut self, actor: *mut c_void) -> Result<u8, Self::Error>;
    unsafe fn is_net_temporary(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn connected_player_channel(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn close_player_channel(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn destroy_actor_early_rules<O: DestroyActorRuleOps>(
    ops: &mut O, actor: *mut c_void, net_force: bool,
) -> Result<Option<bool>, O::Error> {
    if !unsafe { ops.is_editor()? } {
        if unsafe { ops.is_static(actor)? } || unsafe { ops.is_no_delete(actor)? } {
            return Ok(Some(false));
        }
        if unsafe { ops.is_delete_me(actor)? } {
            return Ok(Some(true));
        }
        if unsafe { ops.role(actor)? } != ROLE_AUTHORITY
            && !net_force
            && !unsafe { ops.is_net_temporary(actor)? }
        {
            return Ok(Some(false));
        }
        if unsafe { ops.connected_player_channel(actor)? } {
            unsafe { ops.close_player_channel(actor)?; }
            return Ok(Some(false));
        }
    }
    Ok(None)
}

#[derive(Clone, Copy)]
pub(crate) enum DestroyActorStage {
    ModifyActor,
    EndState,
    Debase,
    Unhash,
    ProcessDestroyed,
    CleanupStandardRefs,
    Disown,
    NotifyDrivers,
    UnlinkAndDestroy,
}

/// Stage implementation for the complete source-ordered DestroyActor algorithm.
/// Implementations own only engine effects; this sequencer owns order and the
/// source's early-return boundaries.
pub(crate) trait DestroyActorOps {
    type Error;
    unsafe fn early_result(&mut self) -> Result<Option<bool>, Self::Error>;
    unsafe fn stage(&mut self, stage: DestroyActorStage) -> Result<Option<bool>, Self::Error>;
}

pub(crate) unsafe fn destroy_actor_with_ops<O: DestroyActorOps>(
    ops: &mut O,
) -> Result<bool, O::Error> {
    if let Some(early) = unsafe { ops.early_result()? } {
        return Ok(early);
    }
    const STAGES: &[DestroyActorStage] = &[
        DestroyActorStage::ModifyActor,
        DestroyActorStage::EndState,
        DestroyActorStage::Debase,
        DestroyActorStage::Unhash,
        DestroyActorStage::ProcessDestroyed,
        DestroyActorStage::CleanupStandardRefs,
        DestroyActorStage::Disown,
        DestroyActorStage::NotifyDrivers,
        DestroyActorStage::UnlinkAndDestroy,
    ];
    for stage in STAGES {
        if let Some(early) = unsafe { ops.stage(*stage)? } {
            return Ok(early);
        }
    }
    Ok(true)
}

/// Checked native leaves for the UnrealScript VM, UObject reflection, Core
/// transaction system, and out-of-scope-file methods that `DestroyActor`,
/// `CompactActors`, `CleanupDestroyed` and `SpawnViewActor` call. Every
/// status is inspected before any output is read.
#[repr(C)]
pub struct Hp2LevelLifecycleServices {
    pub actor_is_valid: unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub actor_not_found: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub cast_player_pawn: unsafe extern "C" fn(actor: *mut c_void, result: *mut *mut c_void) -> i32,
    pub cast_net_connection: unsafe extern "C" fn(obj: *mut c_void, result: *mut *mut c_void) -> i32,
    pub channel_close: unsafe extern "C" fn(channel: *mut c_void) -> i32,
    pub actors_modify_item: unsafe extern "C" fn(level: *mut c_void, index: i32) -> i32,
    pub actor_modify: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_is_probing_end_state: unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub event_end_state: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub set_base_null: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub hash_remove_actor_thorough: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void) -> i32,
    pub hash_check_actor_not_referenced: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void) -> i32,
    pub event_destroyed: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub set_owner_null: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub end_touch: unsafe extern "C" fn(this_actor: *mut c_void, other: *mut c_void) -> i32,
    pub event_lost_child: unsafe extern "C" fn(owner: *mut c_void, child: *mut c_void) -> i32,
    pub net_driver_notify_actor_destroyed: unsafe extern "C" fn(driver: *mut c_void, actor: *mut c_void) -> i32,
    pub demo_rec_driver_server_connection: unsafe extern "C" fn(driver: *mut c_void, result: *mut *mut c_void) -> i32,
    pub actor_lifecycle_observer: unsafe extern "C" fn(result: *mut *mut c_void) -> i32,
    pub observer_on_actor_destroying: unsafe extern "C" fn(observer: *mut c_void, actor: *mut c_void) -> i32,
    pub engine_client: unsafe extern "C" fn(engine: *mut c_void, result: *mut *mut c_void) -> i32,
    pub engine_audio: unsafe extern "C" fn(engine: *mut c_void, result: *mut *mut c_void) -> i32,
    pub audio_note_destroy: unsafe extern "C" fn(audio: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_conditional_destroy: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_get_deleted: unsafe extern "C" fn(actor: *mut c_void, result: *mut *mut c_void) -> i32,
    pub actor_set_deleted: unsafe extern "C" fn(actor: *mut c_void, value: *mut c_void) -> i32,
    pub destroy_actor_unwind: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub warn_undeleted: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub class_cleanup_destroyed: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub delete_actor: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actors_remove_tail: unsafe extern "C" fn(level: *mut c_void, from: i32, count: i32) -> i32,
    pub abrush_static_class: unsafe extern "C" fn(result: *mut *mut c_void) -> i32,
    pub acamera_static_class: unsafe extern "C" fn(result: *mut *mut c_void) -> i32,
    pub cast_camera: unsafe extern "C" fn(actor: *mut c_void, result: *mut *mut c_void) -> i32,
    pub set_b_admin: unsafe extern "C" fn(actor: *mut c_void) -> i32,
}

impl Hp2LevelLifecycleServices {
    unsafe fn actor_is_valid(&self, actor: *mut c_void) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_is_valid)(actor, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn actor_not_found(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_not_found)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn cast_player_pawn(&self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.cast_player_pawn)(actor, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn cast_net_connection(&self, obj: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.cast_net_connection)(obj, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn channel_close(&self, channel: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.channel_close)(channel) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actors_modify_item(&self, level: *mut c_void, index: i32) -> Result<(), i32> {
        let status = unsafe { (self.actors_modify_item)(level, index) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_modify(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_modify)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_is_probing_end_state(&self, actor: *mut c_void) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_is_probing_end_state)(actor, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn event_end_state(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.event_end_state)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn set_base_null(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.set_base_null)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn hash_remove_actor_thorough(&self, hash: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.hash_remove_actor_thorough)(hash, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn hash_check_actor_not_referenced(&self, hash: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.hash_check_actor_not_referenced)(hash, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn event_destroyed(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.event_destroyed)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn set_owner_null(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.set_owner_null)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn end_touch(&self, this_actor: *mut c_void, other: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.end_touch)(this_actor, other) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn event_lost_child(&self, owner: *mut c_void, child: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.event_lost_child)(owner, child) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn net_driver_notify_actor_destroyed(&self, driver: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.net_driver_notify_actor_destroyed)(driver, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn demo_rec_driver_server_connection(&self, driver: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.demo_rec_driver_server_connection)(driver, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn actor_lifecycle_observer(&self) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.actor_lifecycle_observer)(result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn observer_on_actor_destroying(&self, observer: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.observer_on_actor_destroying)(observer, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn engine_client(&self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.engine_client)(engine, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn engine_audio(&self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.engine_audio)(engine, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn audio_note_destroy(&self, audio: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.audio_note_destroy)(audio, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_conditional_destroy(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_conditional_destroy)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_get_deleted(&self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.actor_get_deleted)(actor, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn actor_set_deleted(&self, actor: *mut c_void, value: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_set_deleted)(actor, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn destroy_actor_unwind(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.destroy_actor_unwind)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn warn_undeleted(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.warn_undeleted)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn class_cleanup_destroyed(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.class_cleanup_destroyed)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn delete_actor(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.delete_actor)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actors_remove_tail(&self, level: *mut c_void, from: i32, count: i32) -> Result<(), i32> {
        let status = unsafe { (self.actors_remove_tail)(level, from, count) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn abrush_static_class(&self) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.abrush_static_class)(result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn acamera_static_class(&self) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.acamera_static_class)(result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn cast_camera(&self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.cast_camera)(actor, result.as_mut_ptr()) };
        if status == 0 { Ok(unsafe { result.assume_init() }) } else { Err(status) }
    }
    unsafe fn set_b_admin(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.set_b_admin)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}

/// Stored once by [`publish_services`].
static mut SERVICES: Option<&'static Hp2LevelLifecycleServices> = None;

/// Publishes the process-lifetime `GHp2LevelLifecycleServices` table.
/// Called once from `UnLevAct.cpp`'s static initializer.
///
/// # Safety
/// `services` is the real, immutable `GHp2LevelLifecycleServices` table.
/// Publication happens once, before any engine call, and invokes no
/// callbacks.
pub unsafe fn publish_services(services: &'static Hp2LevelLifecycleServices) {
    unsafe { (&raw mut SERVICES).write(Some(services)) };
}

/// The table stored by [`publish_services`]; `Err(1)` before publication.
///
/// # Safety
/// No concurrent [`publish_services`] call.
unsafe fn published_services() -> Result<&'static Hp2LevelLifecycleServices, i32> {
    unsafe { (&raw const SERVICES).read() }.ok_or(1)
}

/// `ULevel::GetActorIndex`. Linear search, then the original's
/// `appErrorf`-and-never-return fallback, reproduced via a leaf (the engine
/// aborts here in practice; the fallback cannot be reached by live Rust
/// control flow).
unsafe fn actor_index_of(
    ctx: LevelCtx<'_>,
    lsvc: &Hp2LevelLifecycleServices,
    actor: *mut c_void,
) -> Result<usize, i32> {
    unsafe {
        let count = levact::actor_count(ctx);
        let mut i = 0usize;
        while i < count {
            if levact::actor_at(ctx, i) == actor {
                return Ok(i);
            }
            i += 1;
        }
        lsvc.actor_not_found(actor)?;
        Err(1)
    }
}

/// `ULevel::DestroyActor`. The original's `guard(ULevel::DestroyActor)` /
/// `unguardf(("(%s)", ThisActor->GetFullName()))` pair carries a dynamic
/// suffix, so it is not a [`guarded`] call: on the error path,
/// `destroy_actor_unwind` reproduces both `appUnwindf` calls in one leaf,
/// matching the `Hp2SpawnActorOp::Unwind` precedent.
///
/// # Safety
/// `this_actor`, `ctx.ulevel`, and every non-null engine service pointer
/// must be live.
pub unsafe fn destroy_actor(
    ctx: LevelCtx<'_>,
    this_actor: *mut c_void,
    net_force: bool,
) -> Result<bool, i32> {
    unsafe {
        let lsvc = published_services()?;
        let result = destroy_actor_body(ctx, lsvc, this_actor, net_force);
        if result.is_err() {
            let _ = lsvc.destroy_actor_unwind(this_actor);
        }
        result
    }
}

struct NativeDestroyActorRules<'a> {
    ctx: LevelCtx<'a>,
    services: &'a Hp2LevelLifecycleServices,
    channel: *mut c_void,
}

impl DestroyActorRuleOps for NativeDestroyActorRules<'_> {
    type Error = i32;

    unsafe fn is_editor(&mut self) -> Result<bool, i32> {
        unsafe { self.ctx.svc.is_editor().map(|value| value != 0) }
    }
    unsafe fn is_static(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_static() })
    }
    unsafe fn is_no_delete(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_no_delete() })
    }
    unsafe fn is_delete_me(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_delete_me() })
    }
    unsafe fn role(&mut self, actor: *mut c_void) -> Result<u8, i32> {
        Ok(unsafe { (*actor.cast::<AActor>()).role })
    }
    unsafe fn is_net_temporary(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_net_temporary() })
    }
    unsafe fn connected_player_channel(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        self.channel = ptr::null_mut();
        let pawn = unsafe { self.services.cast_player_pawn(actor)? };
        let player = if pawn.is_null() {
            ptr::null_mut()
        } else {
            unsafe { (*pawn.cast::<APlayerPawn>()).player }
        };
        let connection = unsafe { self.services.cast_net_connection(player)? };
        if connection.is_null() {
            return Ok(false);
        }
        let connection = connection.cast::<UNetConnection>();
        self.channel = unsafe { (*connection).channels[0] };
        Ok(unsafe { !self.channel.is_null() && (*connection).state != USOCK_CLOSED })
    }
    unsafe fn close_player_channel(&mut self, _actor: *mut c_void) -> Result<(), i32> {
        if !self.channel.is_null() {
            unsafe { self.services.channel_close(self.channel)?; }
        }
        Ok(())
    }
}

struct NativeDestroyActorOps<'a> {
    ctx: LevelCtx<'a>,
    services: &'a Hp2LevelLifecycleServices,
    actor: *mut c_void,
    actor_index: usize,
    net_force: bool,
}

impl DestroyActorOps for NativeDestroyActorOps<'_> {
    type Error = i32;

    unsafe fn early_result(&mut self) -> Result<Option<bool>, i32> {
        let mut rules = NativeDestroyActorRules {
            ctx: self.ctx,
            services: self.services,
            channel: ptr::null_mut(),
        };
        guarded(c"DestroyRules", || unsafe {
            destroy_actor_early_rules(&mut rules, self.actor, self.net_force)
        })
    }

    unsafe fn stage(&mut self, stage: DestroyActorStage) -> Result<Option<bool>, i32> {
        let actor = self.actor;
        let context = self.ctx;
        let services = self.services;
        match stage {
            DestroyActorStage::ModifyActor => {
                self.actor_index = actor_index_of(context, services, actor)?;
                guarded(c"ModifyActor", || -> Result<(), i32> {
                    services.actors_modify_item(
                        context.ulevel.cast(),
                        self.actor_index as i32,
                    )?;
                    services.actor_modify(actor)?;
                    Ok(())
                })?;
            }
            DestroyActorStage::EndState => {
                if let Some(early) = guarded(c"EndState", || -> Result<Option<bool>, i32> {
                    let object = actor.cast::<UObject>();
                    let state_frame = (*object).state_frame;
                    let has_state_node = !state_frame.is_null()
                        && !(*state_frame.cast::<FStateFrame>()).state_node.is_null();
                    if has_state_node && services.actor_is_probing_end_state(actor)? != 0 {
                        services.event_end_state(actor)?;
                        if (*actor.cast::<AActor>()).b_delete_me() {
                            return Ok(Some(true));
                        }
                    }
                    Ok(None)
                })? {
                    return Ok(Some(early));
                }
            }
            DestroyActorStage::Debase => {
                if let Some(early) = guarded(c"Debase", || -> Result<Option<bool>, i32> {
                    let current = actor.cast::<AActor>();
                    if !(*current).base.is_null() {
                        crate::actor::set_base(
                            &crate::actor::published_services()?,
                            current,
                            ptr::null_mut(),
                            true,
                        )?;
                        if (*current).b_delete_me() {
                            return Ok(Some(true));
                        }
                    }
                    if (*current).standing_count > 0 {
                        let mut index = 0usize;
                        while index < levact::actor_count(context) {
                            let other = levact::actor_at(context, index);
                            if !other.is_null()
                                && (*other.cast::<AActor>()).base == actor
                            {
                                crate::actor::set_base(
                                    &crate::actor::published_services()?,
                                    other.cast(),
                                    ptr::null_mut(),
                                    true,
                                )?;
                            }
                            index += 1;
                        }
                    }
                    Ok(None)
                })? {
                    return Ok(Some(early));
                }
            }
            DestroyActorStage::Unhash => {
                guarded(c"Unhash", || -> Result<(), i32> {
                    let hash = levact::level_hash(context);
                    if !hash.is_null() {
                        if (*actor.cast::<AActor>()).b_collide_actors() {
                            services.hash_remove_actor_thorough(hash, actor)?;
                        }
                        services.hash_check_actor_not_referenced(hash, actor)?;
                    }
                    Ok(())
                })?;
            }
            DestroyActorStage::ProcessDestroyed => {
                if let Some(early) =
                    guarded(c"ProcessDestroyed", || -> Result<Option<bool>, i32> {
                        services.event_destroyed(actor)?;
                        if (*actor.cast::<AActor>()).b_delete_me() {
                            return Ok(Some(true));
                        }
                        Ok(None)
                    })?
                {
                    return Ok(Some(early));
                }
            }
            DestroyActorStage::CleanupStandardRefs => {
                if let Some(early) =
                    guarded(c"CleanupStandardRefs", || -> Result<Option<bool>, i32> {
                        let mut index = 0usize;
                        while index < levact::actor_count(context) {
                            let other = levact::actor_at(context, index);
                            if !other.is_null() {
                                let other_actor = other.cast::<AActor>();
                                if (*other_actor).tick_parent == actor {
                                    (*other_actor).tick_parent = ptr::null_mut();
                                }
                                if (*other_actor).tick_parent2 == actor {
                                    (*other_actor).tick_parent2 = ptr::null_mut();
                                }
                                if (*other_actor).owner == actor {
                                    crate::actor::set_owner(
                                        &crate::actor::published_services()?,
                                        other_actor,
                                        ptr::null_mut(),
                                    )?;
                                    if (*actor.cast::<AActor>()).b_delete_me() {
                                        return Ok(Some(true));
                                    }
                                } else {
                                    let touching =
                                        ptr::addr_of!((*other_actor).touching).read_unaligned();
                                    for touched in touching {
                                        if touched == actor {
                                            crate::actor::end_touch(
                                                &crate::actor::published_services()?,
                                                actor.cast(),
                                                other.cast(),
                                                1,
                                            )?;
                                            if (*actor.cast::<AActor>()).b_delete_me() {
                                                return Ok(Some(true));
                                            }
                                        }
                                    }
                                }
                            }
                            index += 1;
                        }
                        Ok(None)
                    })?
                {
                    return Ok(Some(early));
                }
            }
            DestroyActorStage::Disown => {
                if let Some(early) = guarded(c"Disown", || -> Result<Option<bool>, i32> {
                    let current = actor.cast::<AActor>();
                    if !(*current).owner.is_null() {
                        services.event_lost_child((*current).owner, actor)?;
                        if (*current).b_delete_me() {
                            return Ok(Some(true));
                        }
                    }
                    Ok(None)
                })? {
                    return Ok(Some(early));
                }
            }
            DestroyActorStage::NotifyDrivers => {
                let net_driver =
                    ptr::addr_of!((*context.ulevel).base.net_driver).read_unaligned();
                if !net_driver.is_null() {
                    services.net_driver_notify_actor_destroyed(net_driver, actor)?;
                }
                let demo_driver =
                    ptr::addr_of!((*context.ulevel).base.demo_rec_driver).read_unaligned();
                if !demo_driver.is_null() {
                    let server_connection =
                        services.demo_rec_driver_server_connection(demo_driver)?;
                    if server_connection.is_null() {
                        services.net_driver_notify_actor_destroyed(demo_driver, actor)?;
                    }
                }
            }
            DestroyActorStage::UnlinkAndDestroy => {
                guarded(c"Unlist", || -> Result<(), i32> {
                    check(
                        context.svc,
                        levact::actor_at(context, self.actor_index) == actor,
                        c"Actors(iActor)==ThisActor",
                        337,
                    )?;
                    levact::set_actor_at(context, self.actor_index, ptr::null_mut());
                    (*actor.cast::<AActor>()).set_b_delete_me(true);
                    let observer = services.actor_lifecycle_observer()?;
                    if !observer.is_null() {
                        services.observer_on_actor_destroying(observer, actor)?;
                    }
                    Ok(())
                })?;
                guarded(c"ShutupSound", || -> Result<(), i32> {
                    let engine =
                        ptr::addr_of!((*context.ulevel).base.engine).read_unaligned();
                    if !engine.is_null() {
                        let audio = services.engine_audio(engine)?;
                        if !audio.is_null() {
                            services.audio_note_destroy(audio, actor)?;
                        }
                    }
                    services.actor_conditional_destroy(actor)?;
                    Ok(())
                })?;
                guarded(c"Cleanup", || -> Result<(), i32> {
                    if context.svc.is_editor()? == 0 {
                        let first_deleted =
                            ptr::addr_of!((*context.ulevel).first_deleted).read_unaligned();
                        services.actor_set_deleted(actor, first_deleted)?;
                        ptr::addr_of_mut!((*context.ulevel).first_deleted).write_unaligned(actor);
                    } else {
                        cleanup_destroyed_body(context, services, true)?;
                    }
                    Ok(())
                })?;
                return Ok(Some(true));
            }
        }
        Ok(None)
    }
}

unsafe fn destroy_actor_body(
    ctx: LevelCtx<'_>,
    lsvc: &Hp2LevelLifecycleServices,
    this_actor: *mut c_void,
    net_force: bool,
) -> Result<bool, i32> {
    unsafe {
        check(ctx.svc, !this_actor.is_null(), c"ThisActor", 200)?;
        check(
            ctx.svc,
            lsvc.actor_is_valid(this_actor)? != 0,
            c"ThisActor->IsValid()",
            201,
        )?;
        let mut ops = NativeDestroyActorOps {
            ctx,
            services: lsvc,
            actor: this_actor,
            actor_index: 0,
            net_force,
        };
        destroy_actor_with_ops(&mut ops)
    }
}

/// `ULevel::CompactActors`.
///
/// # Safety
/// `ctx.ulevel` and every non-null engine service pointer must be live.
pub unsafe fn compact_actors(ctx: LevelCtx<'_>) -> Result<(), i32> {
    unsafe {
        let lsvc = published_services()?;
        guarded(c"ULevel::CompactActors", || compact_actors_body(ctx, lsvc))
    }
}

unsafe fn compact_actors_body(ctx: LevelCtx<'_>, lsvc: &Hp2LevelLifecycleServices) -> Result<(), i32> {
    unsafe {
        let first_dynamic = ptr::addr_of!((*ctx.ulevel).i_first_dynamic_actor).read_unaligned() as usize;
        let mut c = first_dynamic;
        let mut i = first_dynamic;
        while i < levact::actor_count(ctx) {
            let a = levact::actor_at(ctx, i);
            if !a.is_null() {
                if !(*a.cast::<AActor>()).b_delete_me() {
                    levact::set_actor_at(ctx, c, a);
                    c += 1;
                } else {
                    lsvc.warn_undeleted(a)?;
                }
            }
            i += 1;
        }
        let total = levact::actor_count(ctx);
        if c != total {
            lsvc.actors_remove_tail(ctx.ulevel.cast(), c as i32, (total - c) as i32)?;
        }
        Ok(())
    }
}

/// `ULevel::CleanupDestroyed`.
///
/// # Safety
/// `ctx.ulevel` and every non-null engine service pointer must be live.
pub unsafe fn cleanup_destroyed(ctx: LevelCtx<'_>, force: bool) -> Result<(), i32> {
    unsafe {
        let lsvc = published_services()?;
        guarded(c"ULevel::CleanupDestroyed", || cleanup_destroyed_body(ctx, lsvc, force))
    }
}

unsafe fn cleanup_destroyed_body(
    ctx: LevelCtx<'_>,
    lsvc: &Hp2LevelLifecycleServices,
    force: bool,
) -> Result<(), i32> {
    unsafe {
        if ctx.svc.is_editor()? == 0 && !force {
            compact_actors_body(ctx, lsvc)?;
        }
        if ptr::addr_of!((*ctx.ulevel).first_deleted).read_unaligned() == (ptr::null_mut::<c_void>()) {
            return Ok(());
        }

        let proceed = guarded(c"CheckDeleted", || -> Result<bool, i32> {
            let mut count = 0i32;
            let mut a = ptr::addr_of!((*ctx.ulevel).first_deleted).read_unaligned();
            while !a.is_null() {
                count += 1;
                a = lsvc.actor_get_deleted(a)?;
            }
            if count < 128 && !force {
                return Ok(false);
            }
            Ok(true)
        })?;
        if !proceed {
            return Ok(());
        }

        guarded(c"CleanupRefs", || -> Result<(), i32> {
            let mut i = 0usize;
            while i < levact::actor_count(ctx) {
                let actor = levact::actor_at(ctx, i);
                if !actor.is_null() {
                    // `checkSlow(!Actor->bDeleteMe)` is debug-only; a no-op
                    // in this release configuration, so it is not ported.
                    lsvc.class_cleanup_destroyed(actor)?;
                }
                i += 1;
            }
            Ok(())
        })?;

        if ctx.svc.is_editor()? != 0 {
            return Ok(());
        }

        guarded(c"FinishDestroyedActors", || -> Result<(), i32> {
            let mut first_deleted = ptr::addr_of!((*ctx.ulevel).first_deleted).read_unaligned();
            while !first_deleted.is_null() {
                check(ctx.svc, (*first_deleted.cast::<AActor>()).b_delete_me(), c"FirstDeleted->bDeleteMe", 439)?;
                let actor_to_kill = first_deleted;
                first_deleted = lsvc.actor_get_deleted(first_deleted)?;
                check(ctx.svc, (*actor_to_kill.cast::<AActor>()).b_delete_me(), c"ActorToKill->bDeleteMe", 442)?;
                lsvc.delete_actor(actor_to_kill)?;
            }
            ptr::addr_of_mut!((*ctx.ulevel).first_deleted).write_unaligned(first_deleted);
            Ok(())
        })?;
        Ok(())
    }
}

/// `ULevel::SpawnBrush`.
///
/// # Safety
/// `ctx.ulevel` and every non-null engine service pointer must be live.
pub unsafe fn spawn_brush(ctx: LevelCtx<'_>) -> Result<*mut c_void, i32> {
    unsafe {
        let lsvc = published_services()?;
        guarded(c"ULevel::SpawnBrush", || -> Result<*mut c_void, i32> {
            let class = lsvc.abrush_static_class()?;
            let result = level_spawn_actor::virtual_spawn_actor(
                ctx.ulevel.cast(),
                class,
                FName { index: 0 },
                ptr::null_mut(),
                ptr::null_mut(),
                FVector { x: 0.0, y: 0.0, z: 0.0 },
                FRotator { pitch: 0, yaw: 0, roll: 0 },
                ptr::null_mut(),
                0,
                0,
            )? as *mut c_void;
            check(ctx.svc, !result.is_null(), c"Result", 176)?;
            Ok(result)
        })
    }
}

/// `ULevel::SpawnViewActor`.
///
/// # Safety
/// `viewport`, `ctx.ulevel`, and every non-null engine service pointer must
/// be live.
pub unsafe fn spawn_view_actor(ctx: LevelCtx<'_>, viewport: *mut c_void) -> Result<(), i32> {
    unsafe {
        let lsvc = published_services()?;
        guarded(c"ULevel::SpawnViewActor", || -> Result<(), i32> {
            let engine = ptr::addr_of!((*ctx.ulevel).base.engine).read_unaligned();
            let client = lsvc.engine_client(engine)?;
            check(ctx.svc, !client.is_null(), c"Engine->Client", 464)?;
            let vp = viewport.cast::<UViewport>();
            check(ctx.svc, (*vp).actor.is_null(), c"Viewport->Actor==NULL", 465)?;

            guarded(c"FindExisting", || -> Result<(), i32> {
                let viewport_name = (*viewport.cast::<UObject>()).name;
                let mut i = 0usize;
                while i < levact::actor_count(ctx) {
                    let a = levact::actor_at(ctx, i);
                    if !a.is_null() {
                        let camera = lsvc.cast_camera(a)?;
                        if !camera.is_null() {
                            let player = (*camera.cast::<APlayerPawn>()).player;
                            if player.is_null() && (*camera.cast::<AActor>()).tag == viewport_name {
                                (*vp).actor = camera;
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                Ok(())
            })?;

            guarded(c"SpawnNew", || -> Result<(), i32> {
                if (*vp).actor.is_null() {
                    let class = lsvc.acamera_static_class()?;
                    let location = FVector { x: -500.0, y: -300.0, z: 300.0 };
                    let rotation = FRotator { pitch: 0, yaw: 0, roll: 0 };
                    let result = level_spawn_actor::virtual_spawn_actor(
                        ctx.ulevel.cast(),
                        class,
                        FName { index: 0 },
                        ptr::null_mut(),
                        ptr::null_mut(),
                        location,
                        rotation,
                        ptr::null_mut(),
                        1,
                        0,
                    )?;
                    (*vp).actor = result as *mut c_void;
                    check(ctx.svc, !(*vp).actor.is_null(), c"Viewport->Actor", 485)?;
                    (*(*vp).actor.cast::<AActor>()).tag = (*viewport.cast::<UObject>()).name;
                }
                Ok(())
            })?;

            guarded(c"SetProperties", || -> Result<(), i32> {
                let actor_ptr = (*vp).actor;
                let obj = actor_ptr.cast::<UObject>();
                let flags = (*obj).object_flags;
                (*obj).object_flags = flags | RF_NOT_FOR_CLIENT | RF_NOT_FOR_SERVER;
                (*obj).object_flags &= !RF_TRANSACTIONAL;
                let pp = actor_ptr.cast::<APlayerPawn>();
                (*pp).player = viewport;
                (*pp).show_flags = SHOW_FRAME | SHOW_MOVING_BRUSHES | SHOW_ACTORS | SHOW_BRUSH;
                (*pp).rend_map = REN_DYN_LIGHT;
                lsvc.set_b_admin(actor_ptr)?;
                Ok(())
            })?;

            levact::set_actor_zone(ctx, (*vp).actor, false, true)?;
            Ok(())
        })
    }
}

/// `ULevel::DropToFloor`. Owns the guard and the `check(Actor!=NULL)`
/// precondition.
///
/// # Safety
/// `actor`, `ctx.ulevel`, and every non-null engine service pointer must be
/// live.
pub unsafe fn drop_to_floor(ctx: LevelCtx<'_>, actor: *mut c_void) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::DropToFloor", || -> Result<bool, i32> {
            check(ctx.svc, !actor.is_null(), c"Actor!=NULL", 823)?;
            let mut hit = levact::check_result(1.0);
            let rotation = (*actor.cast::<AActor>()).rotation;
            let delta = FVector { x: 0.0, y: 0.0, z: -1000.0 };
            levact::move_actor(ctx, actor, delta, rotation, &mut hit as *mut FCheckResult, false, false, false, false)?;
            Ok(hit.time < 1.0)
        })
    }
}
