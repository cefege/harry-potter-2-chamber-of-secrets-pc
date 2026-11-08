//! Checked policy for ULevel::SpawnActor. Native values and storage remain opaque.
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, Ordering};
use hp2_abi::FVector;
use hp2_abi::engine::generated::{FName, FRotator};

#[repr(C)]
pub struct Hp2LevelSpawnActorServices {
    pub call: unsafe extern "C" fn(*mut c_void, u32, usize, usize, *mut usize) -> i32,
    /// Real virtual dispatch for `ULevel::SpawnActor`
    /// (`((ULevel*)Level)->SpawnActor(...)`), used by Rust callers that
    /// spawn actors outside the `ULevel::SpawnActor` shell itself, so an
    /// overridden `SpawnActor` is honored exactly as if those callers held
    /// a `ULevel*` and called through its vtable.
    #[allow(clippy::type_complexity)]
    pub virtual_spawn_actor: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        i32,
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut c_void,
        i32,
        i32,
        *mut usize,
    ) -> i32,
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Op {
    TransitionRequest,
    CreatureRequest,
    Class,
    ClassAbstract,
    ClassIsActor,
    Editor,
    DefaultActor,
    Static,
    NoDelete,
    WarnMissingClass,
    WarnAbstract,
    WarnNotActor,
    WarnStatic,
    WarnFit,
    TransitionResult,
    CreatureResult,
    Template,
    CheckTemplate,
    CollideWorld,
    CollideWhenPlacing,
    NetIsClient,
    NoCollisionFail,
    FindSpot,
    AddSlot,
    Construct,
    PublishSlot,
    Transactional,
    SetTag,
    SetRegion,
    SetLevel,
    Ticked,
    SetTicked,
    SetXLevel,
    CheckAuthority,
    RemoteOwned,
    SwapRoles,
    Brush,
    ClearBrush,
    SetLocation,
    SetOldLocation,
    SetRotation,
    CollideActors,
    Hash,
    HashAdd,
    IsPawn,
    SetPawnRegions,
    SetOwner,
    SetInstigator,
    InitExecution,
    Spawned,
    EventSpawned,
    Observer,
    ObserveSpawned,
    PreBeginBefore,
    PreBeginPlay,
    ObservePreBegin,
    PreBeginAfter,
    BeginPlay,
    ObserveBegin,
    DeleteMe,
    TransitionPublished,
    CreaturePublished,
    SetZone,
    PostBeginPlay,
    ObservePostBegin,
    HashRemove,
    Encroachment,
    Destroy,
    InitialState,
    ObserveInitialState,
    Base,
    IsDecoration,
    IsInventory,
    PhysicsNone,
    PhysicsRotating,
    FindBase,
    InTick,
    LinkNewlySpawned,
    FirstNotify,
    NotifyClass,
    IsA,
    Notify,
    NextNotify,
    ObservePublished,
    Unwind,
}

// This is deliberately a process-wide latch, like the original function-static
// UBOOL. In particular, a failed notification does NOT reset it. No lock or
// native object borrow is held across a callback, including recursive spawns.
static INSIDE_NOTIFICATION: AtomicBool = AtomicBool::new(false);

type Result = core::result::Result<usize, i32>;
type PolicyResult<E> = core::result::Result<usize, E>;

pub(crate) trait SpawnActorOps {
    type Error;
    unsafe fn call(&self, context: *mut c_void, op: Op, a: usize, b: usize) -> core::result::Result<usize, Self::Error>;
}

struct NativeSpawnActorOps<'a>(&'a Hp2LevelSpawnActorServices);
impl SpawnActorOps for NativeSpawnActorOps<'_> {
    type Error = i32;
    unsafe fn call(&self, context: *mut c_void, op: Op, a: usize, b: usize) -> Result {
        let mut out = 0;
        let status = unsafe { (self.0.call)(context, op as u32, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
}

struct Policy<'a, O: SpawnActorOps> {
    ops: &'a O,
    context: *mut c_void,
}
impl<O: SpawnActorOps> Policy<'_, O> {
    fn call(&self, op: Op, a: usize, b: usize) -> PolicyResult<O::Error> {
        unsafe { self.ops.call(self.context, op, a, b) }
    }
    fn get(&self, op: Op) -> PolicyResult<O::Error> {
        self.call(op, 0, 0)
    }
    fn one(&self, op: Op, a: usize) -> PolicyResult<O::Error> {
        self.call(op, a, 0)
    }
    fn result(&self, actor: usize) -> PolicyResult<O::Error> {
        self.one(Op::TransitionResult, actor)?;
        self.one(Op::CreatureResult, actor)
    }
    fn published(&self, actor: usize) -> PolicyResult<O::Error> {
        self.one(Op::TransitionPublished, actor)?;
        self.one(Op::CreaturePublished, actor)
    }
    fn reject(&self, warning: Op) -> PolicyResult<O::Error> {
        self.get(warning)?;
        self.result(0)?;
        Ok(0)
    }
    fn observe(&self, op: Op, actor: usize) -> PolicyResult<O::Error> {
        let observer = self.get(Op::Observer)?;
        if observer != 0 {
            self.call(op, actor, observer)?;
        }
        Ok(0)
    }
    fn hash(&self, op: Op, actor: usize) -> PolicyResult<O::Error> {
        if self.one(Op::CollideActors, actor)? != 0 {
            let hash = self.get(Op::Hash)?;
            if hash != 0 {
                self.call(op, actor, hash)?;
            }
        }
        Ok(0)
    }
    fn run(&self) -> PolicyResult<O::Error> {
        self.get(Op::TransitionRequest)?;
        self.get(Op::CreatureRequest)?;
        if self.get(Op::Class)? == 0 {
            return self.reject(Op::WarnMissingClass);
        }
        if self.get(Op::ClassAbstract)? != 0 {
            return self.reject(Op::WarnAbstract);
        }
        if self.get(Op::ClassIsActor)? == 0 {
            return self.reject(Op::WarnNotActor);
        }
        if self.get(Op::Editor)? == 0
            && (self.one(Op::Static, self.get(Op::DefaultActor)?)? != 0
                || self.one(Op::NoDelete, self.get(Op::DefaultActor)?)? != 0)
        {
            return self.reject(Op::WarnStatic);
        }
        let mut template = self.get(Op::Template)?;
        if template == 0 {
            template = self.get(Op::DefaultActor)?;
        }
        self.one(Op::CheckTemplate, template)?;
        if (self.one(Op::CollideWorld, template)? != 0
            || (self.one(Op::CollideWhenPlacing, template)? != 0
                && self.get(Op::NetIsClient)? == 0))
            && self.get(Op::NoCollisionFail)? == 0
            && self.one(Op::FindSpot, template)? == 0
        {
            return self.reject(Op::WarnFit);
        }
        let slot = self.get(Op::AddSlot)?;
        let mut actor = self.one(Op::Construct, template)?;
        self.call(Op::PublishSlot, actor, slot)?;
        self.one(Op::Transactional, actor)?;
        self.result(actor)?;
        self.one(Op::SetTag, actor)?;
        self.one(Op::SetRegion, actor)?;
        self.one(Op::SetLevel, actor)?;
        self.call(
            Op::SetTicked,
            actor,
            usize::from(self.get(Op::Ticked)? == 0),
        )?;
        self.one(Op::SetXLevel, actor)?;
        self.one(Op::CheckAuthority, actor)?;
        if self.get(Op::RemoteOwned)? != 0 {
            self.one(Op::SwapRoles, actor)?;
        }
        if self.one(Op::Brush, actor)? != 0 {
            self.one(Op::ClearBrush, actor)?;
        }
        self.one(Op::SetLocation, actor)?;
        self.one(Op::SetOldLocation, actor)?;
        self.one(Op::SetRotation, actor)?;
        self.hash(Op::HashAdd, actor)?;
        self.one(Op::SetRegion, actor)?;
        if self.one(Op::IsPawn, actor)? != 0 {
            self.one(Op::SetPawnRegions, actor)?;
        }
        self.one(Op::SetOwner, actor)?;
        self.one(Op::SetInstigator, actor)?;
        self.one(Op::InitExecution, actor)?;
        self.one(Op::Spawned, actor)?;
        self.one(Op::EventSpawned, actor)?;
        self.observe(Op::ObserveSpawned, actor)?;
        self.one(Op::PreBeginBefore, actor)?;
        self.one(Op::PreBeginPlay, actor)?;
        self.observe(Op::ObservePreBegin, actor)?;
        self.one(Op::PreBeginAfter, actor)?;
        self.one(Op::BeginPlay, actor)?;
        self.observe(Op::ObserveBegin, actor)?;
        if self.one(Op::DeleteMe, actor)? != 0 {
            self.published(0)?;
            return Ok(0);
        }
        self.call(Op::SetZone, actor, usize::from(slot == 0))?;
        self.one(Op::PostBeginPlay, actor)?;
        self.observe(Op::ObservePostBegin, actor)?;
        if self.get(Op::NoCollisionFail)? == 0 {
            self.hash(Op::HashRemove, actor)?;
            if self.one(Op::Encroachment, actor)? != 0 {
                self.one(Op::Destroy, actor)?;
                self.published(0)?;
                return Ok(0);
            }
            self.hash(Op::HashAdd, actor)?;
        }
        self.one(Op::InitialState, actor)?;
        self.observe(Op::ObserveInitialState, actor)?;
        if self.one(Op::Base, actor)? == 0
            && self.one(Op::CollideWorld, actor)? != 0
            && (self.one(Op::IsDecoration, actor)? != 0
                || self.one(Op::IsInventory, actor)? != 0
                || self.one(Op::IsPawn, actor)? != 0)
            && (self.one(Op::PhysicsNone, actor)? != 0
                || self.one(Op::PhysicsRotating, actor)? != 0)
        {
            self.one(Op::FindBase, actor)?;
        }
        if self.get(Op::InTick)? != 0 {
            self.one(Op::LinkNewlySpawned, actor)?;
        }
        if INSIDE_NOTIFICATION
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let mut notify = self.get(Op::FirstNotify)?;
            while notify != 0 {
                let class = self.one(Op::NotifyClass, notify)?;
                if class != 0 && self.call(Op::IsA, actor, class)? != 0 {
                    actor = self.call(Op::Notify, notify, actor)?;
                }
                notify = self.one(Op::NextNotify, notify)?;
            }
            INSIDE_NOTIFICATION.store(false, Ordering::Relaxed);
        }
        self.observe(Op::ObservePublished, actor)?;
        self.published(actor)?;
        Ok(actor)
    }
}

/// The native callback context. Exposed so test harnesses that must script
/// `Op::Class` / `Op::Construct` faithfully (matching the real class/template
/// pointer threaded through a call) can decode it from the opaque `context`
/// their mock `call` receives.
#[repr(C)]
pub struct Hp2SpawnActorContext {
    pub level: *mut c_void,
    pub class: *mut c_void,
    pub in_name: FName,
    pub owner: *mut c_void,
    pub instigator: *mut c_void,
    pub location: *const FVector,
    pub rotation: *const FRotator,
    pub template: *mut c_void,
    pub no_collision_fail: i32,
    pub remote_owned: i32,
}

static mut SERVICES: Option<&'static Hp2LevelSpawnActorServices> = None;

/// Publishes the native vtable once, from `UnLevAct.cpp`'s static
/// initializer, mirroring the `pawn_reach` pilot's publish/published_services
/// shape.
///
/// # Safety
/// Must be called at most once, before any engine call to [`spawn_actor`],
/// with a `services` reference that outlives the process.
pub unsafe fn publish_services(services: &'static Hp2LevelSpawnActorServices) {
    unsafe { (&raw mut SERVICES).write(Some(services)) };
}

/// # Safety
/// Must only be read after [`publish_services`] has run.
unsafe fn published_services() -> core::result::Result<&'static Hp2LevelSpawnActorServices, i32> {
    unsafe { (&raw const SERVICES).read() }.ok_or(1)
}

/// Runs the shared source `ULevel::SpawnActor` policy over canonical Core-owned
/// operations. The context is an initialized `Hp2SpawnActorContext`.
///
/// # Safety
/// `ops` implements all source operations for `context`; neither may unwind.
pub(crate) unsafe fn spawn_actor_with_ops<O: SpawnActorOps>(
    ops: &O,
    context: *mut c_void,
) -> core::result::Result<usize, O::Error> {
    let policy = Policy { ops, context };
    let result = policy.run();
    if result.is_err() { let _ = policy.call(Op::Unwind, 0, 0); }
    result
}

/// Checked `ULevel::SpawnActor`, retaining the legacy forwarding entrypoint.
///
/// # Safety
/// The supplied native objects are live and callbacks do not unwind.
#[allow(clippy::too_many_arguments)]
pub unsafe fn spawn_actor(
    level: *mut c_void, class: *mut c_void, in_name: FName, owner: *mut c_void,
    instigator: *mut c_void, location: FVector, rotation: FRotator, template: *mut c_void,
    no_collision_fail: i32, remote_owned: i32,
) -> Result {
    let services = unsafe { published_services() }?;
    let context = Hp2SpawnActorContext {
        level, class, in_name, owner, instigator, location: &location, rotation: &rotation,
        template, no_collision_fail, remote_owned,
    };
    unsafe {
        spawn_actor_with_ops(
            &NativeSpawnActorOps(services),
            (&context as *const Hp2SpawnActorContext as *mut Hp2SpawnActorContext).cast(),
        )
    }
}

/// Real virtual dispatch for `ULevel::SpawnActor`
/// (`((ULevel*)Level)->SpawnActor(...)`). Rust callers that previously
/// called [`spawn_actor`] directly from outside the `ULevel::SpawnActor`
/// shell (net channel actor replication, path scouting) go through this
/// instead, so an overridden `SpawnActor` is honored exactly as if those
/// callers held a `ULevel*` and called through its vtable. `level_init`
/// (`ULevel::ULevel`, which runs before any subclass vtable could matter)
/// keeps calling [`spawn_actor`] directly.
///
/// # Safety
/// Same requirements as [`spawn_actor`]; `level` must additionally be a
/// live `ULevel*`.
#[allow(clippy::too_many_arguments)]
pub unsafe fn virtual_spawn_actor(
    level: *mut c_void,
    class: *mut c_void,
    in_name: FName,
    owner: *mut c_void,
    instigator: *mut c_void,
    location: FVector,
    rotation: FRotator,
    template: *mut c_void,
    no_collision_fail: i32,
    remote_owned: i32,
) -> Result {
    let services = unsafe { published_services() }?;
    let mut out: usize = 0;
    let status = unsafe {
        (services.virtual_spawn_actor)(
            level,
            class,
            in_name.index,
            owner,
            instigator,
            (&location as *const FVector).cast(),
            (&rotation as *const FRotator).cast(),
            template,
            no_collision_fail,
            remote_owned,
            &mut out,
        )
    };
    if status == 0 { Ok(out) } else { Err(status) }
}
