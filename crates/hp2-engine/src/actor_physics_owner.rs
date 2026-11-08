//! Canonical Rust owner for actor physics transitions.

use hp2_abi::engine::generated::{AActor, FCheckResult, ULevel};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use crate::physic::{Hp2PhysicServices, PhysicsError, PhysicsResult};
const TRACE_ALL_BLOCKING: u32 = crate::levact::TRACE_ALL_BLOCKING;

/// Backend operations used by the single FindBase policy.
pub(crate) trait FindBaseOps {
    type Error;

    unsafe fn cylinder_extent(&mut self, actor: *mut AActor) -> Result<FVector, Self::Error>;
    unsafe fn single_line_check(
        &mut self,
        actor: *mut AActor,
        hit: *mut FCheckResult,
        end: FVector,
        start: FVector,
        trace: u32,
        extent: FVector,
    ) -> Result<(), Self::Error>;
    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> Result<(), Self::Error>;
}

/// Runs the native FindBase order and geometry without backend-specific policy.
///
/// # Safety
/// `actor` must be a live actor, and every actor reached by the backend must remain live.
pub(crate) unsafe fn find_base_with_ops<O: FindBaseOps>(
    ops: &mut O,
    actor: *mut AActor,
) -> Result<(), O::Error> {
    let location = unsafe { (*actor).location };
    let end = FVector { x: location.x, y: location.y, z: location.z + -8.0 };
    let extent = unsafe { ops.cylinder_extent(actor)? };
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = 1.0;
    hit.item = -1;
    unsafe { ops.single_line_check(actor, &mut hit, end, location, TRACE_ALL_BLOCKING, extent)?; }
    if unsafe { (*actor).base } != hit.actor {
        unsafe { ops.set_base(actor, hit.actor.cast())?; }
    }
    Ok(())
}

/// Finds the blocking Actor eight units beneath a canonical Actor and updates its base.
///
/// # Safety
/// `actor` must be a live canonical `AActor` owned by `access`.
pub unsafe fn find_base(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<()> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in FindBase"));
    }
    let mut ops = CanonicalFindBaseOps { access };
    unsafe { find_base_with_ops(&mut ops, actor) }
}

struct CanonicalFindBaseOps<'a> {
    access: CoreAccess<'a>,
}

impl FindBaseOps for CanonicalFindBaseOps<'_> {
    type Error = RuntimeError;

    unsafe fn cylinder_extent(&mut self, actor: *mut AActor) -> RuntimeResult<FVector> {
        unsafe { crate::collision_owner::actor_cylinder_extent(self.access, actor) }
    }

    unsafe fn single_line_check(
        &mut self,
        actor: *mut AActor,
        hit: *mut FCheckResult,
        end: FVector,
        start: FVector,
        trace: u32,
        extent: FVector,
    ) -> RuntimeResult<()> {
        let level = unsafe { (*actor).x_level.cast::<ULevel>() };
        if level.is_null() {
            return Err(RuntimeError::new("Canonical Actor has no Level in FindBase"));
        }
        let _ = unsafe {
            crate::level_trace_owner::single_line_check(
                self.access,
                level,
                hit,
                actor,
                end,
                start,
                trace,
                extent,
                0,
            )?
        };
        Ok(())
    }
    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> RuntimeResult<()> {
        unsafe { crate::level_lifecycle_owner::set_base(self.access, actor, new_base, true) }
    }
}

/// Backend operations for the shared AActor::MoveSmooth transition.
pub(crate) trait MoveSmoothOps {
    type Error;

    unsafe fn move_actor(
        &mut self,
        actor: *mut AActor,
        delta: FVector,
        rotation: hp2_abi::engine::generated::FRotator,
        hit: *mut FCheckResult,
    ) -> Result<bool, Self::Error>;
    unsafe fn event_hit_wall(
        &mut self,
        actor: *mut AActor,
        normal: FVector,
        hit_actor: *mut std::ffi::c_void,
    ) -> Result<(), Self::Error>;
}

/// Shared native MoveSmooth slide-and-two-wall response policy.
///
/// # Safety
/// `actor` and `hit` must be live and every backend-reached object must remain live.
pub(crate) unsafe fn move_smooth_with_ops<O: MoveSmoothOps>(
    ops: &mut O,
    actor: *mut AActor,
    delta: FVector,
) -> Result<bool, O::Error> {
    let mut hit = crate::levact::check_result(1.0);
    let rotation = unsafe { (*actor).rotation };
    let did_hit = unsafe { ops.move_actor(actor, delta, rotation, &mut hit)? };
    if hit.time < 1.0 {
        let d = dot(delta, hit.normal);
        let mut adjusted = slide_scaled(delta, hit.normal, d, 1.0 - hit.time);
        if dot(delta, adjusted) >= 0.0 {
            let old_hit_normal = hit.normal;
            let desired_dir = safe_normal(delta);
            unsafe { ops.event_hit_wall(actor, hit.normal, hit.actor)?; }
            let rotation = unsafe { (*actor).rotation };
            unsafe { ops.move_actor(actor, adjusted, rotation, &mut hit)?; }
            if hit.time < 1.0 {
                two_wall_adjust(desired_dir, &mut adjusted, hit.normal, old_hit_normal, hit.time);
                let rotation = unsafe { (*actor).rotation };
                unsafe { ops.move_actor(actor, adjusted, rotation, &mut hit)?; }
            }
        }
    }
    Ok(did_hit)
}

/// Canonical MoveSmooth entry point. MoveActor owns the full collision/movement policy.
///
/// # Safety
/// `actor` must be a live canonical Actor owned by `access`.
pub unsafe fn move_smooth(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    delta: FVector,
) -> RuntimeResult<bool> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in MoveSmooth"));
    }
    let mut ops = CanonicalMoveSmoothOps { access };
    unsafe { move_smooth_with_ops(&mut ops, actor, delta) }
}

struct CanonicalMoveSmoothOps<'a> {
    access: CoreAccess<'a>,
}

#[repr(C, packed(4))]
struct HitWallParams {
    normal: FVector,
    hit_actor: *mut std::ffi::c_void,
}

const _: () = {
    assert!(std::mem::offset_of!(HitWallParams, hit_actor) == 12);
    assert!(std::mem::size_of::<HitWallParams>() == 20);
};

impl MoveSmoothOps for CanonicalMoveSmoothOps<'_> {
    type Error = RuntimeError;

    unsafe fn move_actor(
        &mut self,
        actor: *mut AActor,
        delta: FVector,
        rotation: hp2_abi::engine::generated::FRotator,
        hit: *mut FCheckResult,
    ) -> RuntimeResult<bool> {
        let level = unsafe { (*actor).x_level.cast::<ULevel>() };
        if level.is_null() {
            return Err(RuntimeError::new("Canonical Actor has no Level in MoveSmooth"));
        }
        unsafe {
            crate::level_move_owner::move_actor(
                self.access, level, actor, delta, rotation, hit, false, false, false, false,
            )
        }
    }

    unsafe fn event_hit_wall(
        &mut self,
        actor: *mut AActor,
        normal: FVector,
        hit_actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        let mut params = HitWallParams { normal, hit_actor };
        unsafe {
            execute_actor_event(
                self.access,
                actor.cast(),
                "HitWall",
                (&mut params as *mut HitWallParams).cast(),
                std::mem::size_of::<HitWallParams>(),
            )
        }
    }
}

fn dot(a: FVector, b: FVector) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn safe_normal(a: FVector) -> FVector {
    let squared = crate::vec::size_squared(a);
    if squared < 1.0e-8 {
        FVector { x: 0.0, y: 0.0, z: 0.0 }
    } else {
        let length = crate::math::app_sqrt(squared as f64) as f32;
        crate::vec::scale(a, 1.0 / length)
    }
}

fn slide_scaled(delta: FVector, normal: FVector, d: f32, scale: f32) -> FVector {
    FVector {
        x: (delta.x - normal.x * d) * scale,
        y: (delta.y - normal.y * d) * scale,
        z: (delta.z - normal.z * d) * scale,
    }
}

fn two_wall_adjust(
    desired_dir: FVector,
    delta: &mut FVector,
    hit_normal: FVector,
    old_hit_normal: FVector,
    hit_time: f32,
) {
    if dot(old_hit_normal, hit_normal) <= 0.0 {
        let new_dir = safe_normal(FVector {
            x: hit_normal.y * old_hit_normal.z - hit_normal.z * old_hit_normal.y,
            y: hit_normal.z * old_hit_normal.x - hit_normal.x * old_hit_normal.z,
            z: hit_normal.x * old_hit_normal.y - hit_normal.y * old_hit_normal.x,
        });
        let along = dot(*delta, new_dir) * (1.0 - hit_time);
        *delta = crate::vec::scale(new_dir, along);
        if dot(desired_dir, *delta) < 0.0 {
            *delta = crate::vec::scale(*delta, -1.0);
        }
    } else {
        let slide = crate::vec::sub(*delta, crate::vec::scale(hit_normal, dot(*delta, hit_normal)));
        *delta = crate::vec::scale(slide, 1.0 - hit_time);
        if dot(*delta, desired_dir) <= 0.0 {
            *delta = FVector { x: 0.0, y: 0.0, z: 0.0 };
        }
    }
}

/// Returns whether `object`'s class chain contains `short_name`.
///
/// This is Core's pointer-free equivalent of `UObject::IsA`: classes are compared
/// by their canonical FName, which is the identity `IsA` uses inside one package.
pub(crate) unsafe fn is_a_named(
    access: CoreAccess<'_>,
    object: *mut c_void,
    short_name: &str,
) -> RuntimeResult<bool> {
    if object.is_null() {
        return Ok(false);
    }
    let wanted = access.intern_str(short_name, hp2_core::name_owner::FindName::Find)?;
    let wanted = wanted.to_ne_bytes();
    let mut class = unsafe { (*object.cast::<hp2_abi::engine::generated::UObject>()).class }
        .cast::<hp2_abi::engine::generated::UClass>();
    while !class.is_null() {
        let name = unsafe {
            (*class.cast::<hp2_abi::engine::generated::UObject>()).name
        };
        if name == wanted {
            return Ok(true);
        }
        class = unsafe { (*class).super_field }.cast();
    }
    Ok(false)
}

/// Borrowed canonical state for the shared physics-policy call table.
struct CanonicalPhysicContext {
    access: CoreAccess<'static>,
    error: RefCell<Option<RuntimeError>>,
}

impl CanonicalPhysicContext {
    fn run<T>(
        &self,
        body: impl FnOnce(CoreAccess<'_>) -> RuntimeResult<T>,
    ) -> PhysicsResult<T> {
        match body(self.access) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.set_error(error);
                Err(PhysicsError(1))
            }
        }
    }

    fn set_error(&self, error: RuntimeError) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    fn ensure_error(&self, message: &'static str) {
        self.set_error(RuntimeError::new(message));
    }
}

thread_local! {
    static CANONICAL_PHYSIC_CONTEXT: Cell<*mut CanonicalPhysicContext> =
        const { Cell::new(std::ptr::null_mut()) };
}

struct PhysicContextScope {
    previous: *mut CanonicalPhysicContext,
}

impl PhysicContextScope {
    fn enter(context: &mut CanonicalPhysicContext) -> Self {
        let previous = CANONICAL_PHYSIC_CONTEXT.with(|slot| {
            let previous = slot.get();
            slot.set(context);
            previous
        });
        Self { previous }
    }
}

impl Drop for PhysicContextScope {
    fn drop(&mut self) {
        CANONICAL_PHYSIC_CONTEXT.with(|slot| slot.set(self.previous));
    }
}

fn current_physic_context() -> Option<*mut CanonicalPhysicContext> {
    CANONICAL_PHYSIC_CONTEXT.with(|slot| {
        let context = slot.get();
        (!context.is_null()).then_some(context)
    })
}

/// Returns the active canonical Core access while a shared-physics call runs.
pub(crate) unsafe fn canonical_context_access() -> Option<CoreAccess<'static>> {
    let context = current_physic_context()?;
    Some(unsafe { (*context).access })
}

/// Runs `AActor::performPhysics`/`APawn::performPhysics` through the shared policy.
///
/// # Safety
/// `actor` is a live canonical Actor and every object reached by physics remains
/// live and owned by `access` for the synchronous call.
pub(crate) unsafe fn perform_physics(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    delta: f32,
) -> RuntimeResult<()> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in performPhysics"));
    }
    let pawn = unsafe { is_a_named(access, actor.cast(), "Pawn")? };
    let interpolation_manager = !pawn && unsafe {
        is_a_named(access, actor.cast(), "InterpolationManager")?
    };
    with_physic_context(access, |services| unsafe {
        if pawn {
            crate::physic::dispatch::perform_physics_pawn(actor.cast(), services, delta)
        } else if interpolation_manager {
            crate::physic::interp::perform_physics(actor.cast(), services, delta)
        } else {
            crate::physic::dispatch::perform_physics_actor(actor, services, delta)
        }
    })
}

fn with_physic_context<T>(
    access: CoreAccess<'_>, body: impl FnOnce(&Hp2PhysicServices) -> PhysicsResult<T>,
) -> RuntimeResult<T> {
    let mut context = CanonicalPhysicContext {
        // The synchronous scope is popped before the borrowed Core access can expire.
        access: unsafe { std::mem::transmute::<CoreAccess<'_>, CoreAccess<'static>>(access) },
        error: RefCell::new(None),
    };
    let _scope = PhysicContextScope::enter(&mut context);
    let result = body(&CANONICAL_PHYSIC_SERVICES);
    match (result, context.error.into_inner()) {
        (Ok(value), _) => Ok(value),
        (Err(_), Some(error)) => Err(error),
        (Err(error), None) => Err(RuntimeError::new(&format!(
            "Canonical physics policy failed ({})",
            error.0
        ))),
    }
}

/// Runs the shared water-boundary policy outside a physics tick with real Core producers.
///
/// # Safety
/// Pawn/world/regions and writable endpoint remain live through the synchronous query.
pub(crate) unsafe fn find_water_line(
    access: CoreAccess<'_>, pawn: *mut hp2_abi::engine::generated::APawn,
    start: FVector, end: *mut FVector,
) -> RuntimeResult<()> {
    with_physic_context(access, |services| unsafe {
        crate::physic::swim::find_water_line(pawn, services, start, end)
    })
}

struct CanonicalSetPhysicsOps {
    access: CoreAccess<'static>,
}

impl crate::physic::dispatch::SetPhysicsOps for CanonicalSetPhysicsOps {
    type Error = RuntimeError;

    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> RuntimeResult<()> {
        unsafe { crate::level_lifecycle_owner::set_base(self.access, actor, new_base, true) }
    }

    unsafe fn find_base(&mut self, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { find_base(self.access, actor) }
    }
}

/// Runs `AActor::setPhysics` with canonical base/floor ownership.
pub(crate) unsafe fn set_physics(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    new_physics: u8,
    new_floor: *mut AActor,
) -> RuntimeResult<()> {
    let mut ops = CanonicalSetPhysicsOps {
        access: unsafe { std::mem::transmute::<CoreAccess<'_>, CoreAccess<'static>>(access) },
    };
    unsafe {
        crate::physic::dispatch::set_physics_with_ops(&mut ops, actor, new_physics, new_floor)
    }
}

/// Canonical route used by the shared `Hp2PhysicServices::call_set_physics`.
pub(crate) unsafe fn canonical_set_physics_if_active(
    actor: *mut AActor,
    new_physics: u8,
    new_floor: *mut AActor,
) -> Option<PhysicsResult<()>> {
    let context = current_physic_context()?;
    let context = unsafe { &*context };
    Some(context.run(|access| unsafe { set_physics(access, actor, new_physics, new_floor) }))
}

/// Canonical cylinder extent used by shared physics policy.
pub(crate) unsafe fn canonical_cylinder_extent_if_active(
    actor: *mut AActor,
    out: *mut FVector,
) -> Option<RuntimeResult<()>> {
    let context = current_physic_context()?;
    let context = unsafe { &*context };
    Some(
        context
            .run(|access| unsafe {
                let extent = crate::collision_owner::actor_cylinder_extent(access, actor)?;
                if !out.is_null() {
                    unsafe { out.write(extent); }
                }
                Ok(())
            })
            .map_err(|_| RuntimeError::new("Canonical cylinder extent failed")),
    )
}

/// Canonical `SetBase` used by shared physics policy.
pub(crate) unsafe fn canonical_set_base_if_active(
    actor: *mut AActor,
    base: *mut AActor,
) -> Option<RuntimeResult<()>> {
    let context = current_physic_context()?;
    let context = unsafe { &*context };
    Some(
        context
            .run(|access| unsafe {
                crate::level_lifecycle_owner::set_base(access, actor, base, true)
            })
            .map_err(|_| RuntimeError::new("Canonical SetBase failed")),
    )
}

const PHYS_CLASS_PAWN: i32 = 0;
const PHYS_CLASS_PLAYER_PAWN: i32 = 1;
const PHYS_CLASS_DECORATION: i32 = 2;
const PHYS_CLASS_CARCASS: i32 = 3;
const PHYS_CLASS_PROJECTILE: i32 = 4;
const PHYS_CLASS_MOVER: i32 = 5;
const PHYS_CLASS_INVENTORY: i32 = 6;

fn physic_class_name(class: i32) -> Option<&'static str> {
    match class {
        PHYS_CLASS_PAWN => Some("Pawn"),
        PHYS_CLASS_PLAYER_PAWN => Some("PlayerPawn"),
        PHYS_CLASS_DECORATION => Some("Decoration"),
        PHYS_CLASS_CARCASS => Some("Carcass"),
        PHYS_CLASS_PROJECTILE => Some("Projectile"),
        PHYS_CLASS_MOVER => Some("Mover"),
        PHYS_CLASS_INVENTORY => Some("Inventory"),
        _ => None,
    }
}

unsafe fn canonical_single_line_check(
    access: CoreAccess<'_>,
    level: *mut c_void,
    source: *mut AActor,
    hit: *mut FCheckResult,
    end: FVector,
    start: FVector,
    trace: i32,
    extent: FVector,
) -> RuntimeResult<bool> {
    let flags = if trace == 1 {
        TRACE_ALL_BLOCKING
    } else {
        crate::levact::TRACE_LEVEL | crate::levact::TRACE_MOVERS
    };
    unsafe {
        crate::level_trace_owner::single_line_check(
            access,
            level.cast::<ULevel>(),
            hit,
            source,
            end,
            start,
            flags,
            extent,
            0,
        )
    }
}

unsafe fn canonical_debugf(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    text: &str,
) -> RuntimeResult<()> {
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
    let mut message: Vec<u32> = text.chars().map(u32::from).collect();
    if !actor.is_null() {
        let name = unsafe { access.object_full_name(actor.cast())? };
        message.push(' ' as u32);
        message.extend(name);
    }
    access.emit_log(category, &message)
}

unsafe fn execute_actor_event(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    event: &str,
    params: *mut c_void,
    parameter_size: usize,
) -> RuntimeResult<()> {
    let object = actor.cast::<hp2_abi::engine::generated::UObject>();
    let name = access.intern_str(event, hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(object, name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("Canonical physics event is missing").with_context(event));
    }
    let declared = usize::from(unsafe { (*function).parms_size });
    if declared != parameter_size || (declared != 0 && params.is_null()) {
        return Err(RuntimeError::new("Canonical physics event parameter layout does not match metadata")
            .with_context(&format!("{event}: declared={declared}, supplied={parameter_size}")));
    }
    unsafe {
        crate::actor_owner::process_event(
            access,
            actor.cast::<AActor>(),
            function,
            params,
            std::ptr::null_mut(),
        )
    }
}

#[repr(C, packed(4))]
struct InterpolateEndParams {
    manager: *mut c_void,
    forward: i32,
}

const _: () = {
    assert!(std::mem::offset_of!(InterpolateEndParams, forward) == 8);
    assert!(std::mem::size_of::<InterpolateEndParams>() == 12);
};

unsafe extern "C" fn phys_is_a(obj: *mut c_void, class: i32, out: *mut i32) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let Some(name) = physic_class_name(class) else {
        context.ensure_error("Canonical physics is_a has an unknown class id");
        return 1;
    };
    match context.run(|access| unsafe { is_a_named(access, obj, name) }) {
        Ok(value) => {
            if !out.is_null() {
                unsafe { *out = i32::from(value); }
            }
            0
        }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_is_probing_hit_wall(actor: *mut c_void, out: *mut i32) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    match context.run(|access| {
        let name = access.intern_str("HitWall", hp2_core::name_owner::FindName::Find)?;
        unsafe { access.is_probing(actor.cast(), name) }.map(i32::from)
    }) {
        Ok(value) => {
            if !out.is_null() { unsafe { *out = value; } }
            0
        }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_app_frand(out: *mut f32) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    match context.run(|access| Ok(access.rand_range_float(0.0, 1.0))) {
        Ok(value) => { if !out.is_null() { unsafe { *out = value; } } 0 }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_bsp_surf_special(
    hit: *const FCheckResult,
    radius: f32,
    out: *mut i32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    match context.run(|access| unsafe {
        if hit.is_null() {
            return Ok(false);
        }
        let primitive = (*hit).primitive;
        if primitive.is_null() || !is_a_named(access, primitive, "Model")? {
            return Ok(false);
        }
        let model = primitive.cast::<hp2_abi::engine::generated::UModel>();
        let view = crate::collision_owner::model_view(model)?;
        let mover = if (*hit).actor.is_null() || !is_a_named(access, (*hit).actor, "Mover")? {
            None
        } else {
            Some(crate::actor::to_local((*hit).actor.cast::<AActor>()))
        };
        let surface = crate::prim::bsp_surf(
            Some(&view.bsp),
            (*hit).item,
            (*hit).location,
            (*hit).normal,
            radius,
            mover.as_ref(),
        );
        Ok(surface
            .and_then(|index| view.bsp.surfs.get(index))
            .is_some_and(|surface| surface.poly_flags & hp2_abi::poly_flags::SPECIAL_POLY != 0))
    }) {
        Ok(value) => { if !out.is_null() { unsafe { *out = i32::from(value); } } 0 }
        Err(error) => error.0,
    }
}

fn core_rotator(rotation: hp2_abi::engine::generated::FRotator) -> hp2_core::math::FRotator {
    hp2_core::math::FRotator {
        pitch: rotation.pitch,
        yaw: rotation.yaw,
        roll: rotation.roll,
    }
}

fn abi_rotator(rotation: hp2_core::math::FRotator) -> hp2_abi::engine::generated::FRotator {
    hp2_abi::engine::generated::FRotator {
        pitch: rotation.pitch,
        yaw: rotation.yaw,
        roll: rotation.roll,
    }
}

unsafe extern "C" fn phys_rotation_vector(
    rot: *const hp2_abi::engine::generated::FRotator,
    out: *mut FVector,
) -> i32 {
    if rot.is_null() || out.is_null() {
        return 1;
    }
    unsafe {
        out.write(hp2_core::math::rotator_vector(core_rotator(*rot), hp2_core::math::trig_table()));
    }
    0
}

unsafe extern "C" fn phys_move_actor(
    actor: *mut c_void,
    delta: *const FVector,
    rotation: *const hp2_abi::engine::generated::FRotator,
    hit: *mut FCheckResult,
    result: *mut i32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if delta.is_null() || rotation.is_null() || hit.is_null() {
        context.ensure_error("Canonical physics MoveActor has null arguments");
        return 1;
    }
    let actor = actor.cast::<AActor>();
    match context.run(|access| unsafe {
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() {
            return Err(RuntimeError::new("Canonical physics MoveActor has no Level"));
        }
        crate::level_move_owner::move_actor(
            access, level, actor, *delta, *rotation, hit, false, false, false, false,
        )
    }) {
        Ok(value) => { if !result.is_null() { unsafe { *result = i32::from(value); } } 0 }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_single_line_check(
    actor: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    trace: i32,
    extent: *const FVector,
    result: *mut i32,
) -> i32 {
    phys_single_line_check_impl(actor, hit, end, start, trace, extent, result, true)
}

unsafe extern "C" fn phys_single_line_check_no_source(
    actor: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    trace: i32,
    extent: *const FVector,
    result: *mut i32,
) -> i32 {
    phys_single_line_check_impl(actor, hit, end, start, trace, extent, result, false)
}

#[allow(clippy::too_many_arguments)]
unsafe fn phys_single_line_check_impl(
    actor: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    trace: i32,
    extent: *const FVector,
    result: *mut i32,
    with_source: bool,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if hit.is_null() || end.is_null() || start.is_null() || extent.is_null() {
        context.ensure_error("Canonical physics SingleLineCheck has null arguments");
        return 1;
    }
    let actor = actor.cast::<AActor>();
    match context.run(|access| unsafe {
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() {
            return Err(RuntimeError::new("Canonical physics SingleLineCheck has no Level"));
        }
        let source = if with_source { actor } else { std::ptr::null_mut() };
        canonical_single_line_check(access, level.cast(), source, hit, *end, *start, trace, *extent)
    }) {
        Ok(value) => { if !result.is_null() { unsafe { *result = i32::from(value); } } 0 }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_mount(actor: *mut c_void, dir: *const FVector) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if dir.is_null() { context.ensure_error("Canonical Mount event has a null direction"); return 1; }
    let mut parameter = unsafe { *dir };
    match context.run(|access| unsafe {
        execute_actor_event(access, actor, "Mount", (&mut parameter as *mut FVector).cast(), std::mem::size_of::<FVector>())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_hit_wall(
    actor: *mut c_void,
    normal: *const FVector,
    hit_actor: *mut c_void,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if normal.is_null() { context.ensure_error("Canonical HitWall event has a null normal"); return 1; }
    let mut parameter = HitWallParams { normal: unsafe { *normal }, hit_actor };
    match context.run(|access| unsafe {
        execute_actor_event(access, actor, "HitWall", (&mut parameter as *mut HitWallParams).cast(), std::mem::size_of::<HitWallParams>())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_landed(actor: *mut c_void, normal: *const FVector) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if normal.is_null() { context.ensure_error("Canonical Landed event has a null normal"); return 1; }
    let mut parameter = unsafe { *normal };
    match context.run(|access| unsafe {
        execute_actor_event(access, actor, "Landed", (&mut parameter as *mut FVector).cast(), std::mem::size_of::<FVector>())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_point_region_zone(
    actor: *mut c_void,
    location: *const FVector,
    out: *mut *mut c_void,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if location.is_null() { context.ensure_error("Canonical PointRegion has a null location"); return 1; }
    let actor = actor.cast::<AActor>();
    match context.run(|access| unsafe {
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() { return Err(RuntimeError::new("Canonical PointRegion has no Level")); }
        let model = (*level).model.cast::<hp2_abi::engine::generated::UModel>();
        if model.is_null() { return Err(RuntimeError::new("Canonical PointRegion has no Model")); }
        let region = crate::collision_owner::CollisionOwner::model_point_region(model, (*actor).level, *location)?;
        Ok(region.zone)
    }) {
        Ok(zone) => { if !out.is_null() { unsafe { *out = zone; } } 0 }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_debugf_flew_out(actor: *mut c_void) -> i32 {
    phys_debugf(actor, "flew out of the world!")
}

unsafe extern "C" fn phys_debugf_fell_out(actor: *mut c_void) -> i32 {
    phys_debugf(actor, "fell out of the world!")
}

unsafe fn phys_debugf(actor: *mut c_void, text: &str) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    match context.run(|access| unsafe { canonical_debugf(access, actor.cast::<AActor>(), text) }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_destroy_actor(actor: *mut c_void) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let actor = actor.cast::<AActor>();
    match context.run(|access| unsafe {
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() { return Err(RuntimeError::new("Canonical DestroyActor has no Level")); }
        crate::level_lifecycle_owner::destroy_actor(access, level, actor, false).map(|_| ())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_falling(actor: *mut c_void) -> i32 {
    phys_simple_event(actor, "Falling", 0)
}

unsafe extern "C" fn phys_log_fell_out_of_world(actor: *mut c_void) -> i32 {
    phys_debugf(actor, "fell out of the world!")
}

unsafe extern "C" fn phys_event_fell_out_of_world(actor: *mut c_void) -> i32 {
    phys_simple_event(actor, "FellOutOfWorld", 0)
}

unsafe fn phys_simple_event(actor: *mut c_void, event: &str, parameter_size: usize) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    match context.run(|access| unsafe {
        execute_actor_event(access, actor, event, std::ptr::null_mut(), parameter_size)
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_rotation_from_vector(
    vector: *const FVector,
    out: *mut hp2_abi::engine::generated::FRotator,
) -> i32 {
    if vector.is_null() || out.is_null() { return 1; }
    unsafe { out.write(abi_rotator(hp2_core::math::vector_rotation(*vector))); }
    0
}

unsafe extern "C" fn phys_find_view_target(
    actor: *mut c_void,
    tag: *const [u8; 4],
    out: *mut *mut c_void,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if tag.is_null() { context.ensure_error("Canonical FindViewTarget has a null tag"); return 1; }
    let actor = actor.cast::<AActor>();
    match context.run(|access| unsafe {
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() { return Err(RuntimeError::new("Canonical FindViewTarget has no Level")); }
        let count = crate::level_owner::actor_count(level)?;
        for index in 0..count {
            let candidate = crate::level_owner::actor_at(level, index);
            if !candidate.is_null() && (*candidate.cast::<AActor>()).tag == *tag {
                return Ok(candidate.cast());
            }
        }
        Ok(std::ptr::null_mut())
    }) {
        Ok(found) => { if !out.is_null() { unsafe { *out = found; } } 0 }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_update_camera(manager: *mut c_void, old_alpha: f32) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let mut parameter = old_alpha;
    match context.run(|access| unsafe {
        execute_actor_event(access, manager, "UpdateCamera", (&mut parameter as *mut f32).cast(), std::mem::size_of::<f32>())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_interpolate_end(
    point: *mut c_void,
    manager: *mut c_void,
    forward: i32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let mut parameter = InterpolateEndParams { manager, forward };
    match context.run(|access| unsafe {
        execute_actor_event(
            access,
            point,
            "InterpolateEnd",
            (&mut parameter as *mut InterpolateEndParams).cast(),
            std::mem::size_of::<InterpolateEndParams>(),
        )
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_finished_interpolation(manager: *mut c_void) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let mut parameter = std::ptr::null_mut::<c_void>();
    match context.run(|access| unsafe {
        execute_actor_event(
            access,
            manager,
            "FinishedInterpolation",
            (&mut parameter as *mut *mut c_void).cast(),
            std::mem::size_of::<*mut c_void>(),
        )
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_key_frame_reached(actor: *mut c_void) -> i32 {
    phys_simple_event(actor, "KeyFrameReached", 0)
}

unsafe extern "C" fn phys_mover_freeze_log(
    actor: *mut c_void,
    phys_alpha: f32,
    prev_key_num: i32,
    key_num: i32,
    hit_actor: *mut c_void,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let mut text = format!(
        "MoverFreeze: alpha={phys_alpha} key {prev_key_num}->{key_num} hit"
    );
    if hit_actor.is_null() {
        text.push_str(" world");
    } else {
        text.push_str(" actor");
    }
    match context.run(|access| unsafe { canonical_debugf(access, actor.cast::<AActor>(), &text) }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_post_touch(touched: *mut c_void, toucher: *mut c_void) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let mut parameter = toucher;
    match context.run(|access| unsafe {
        execute_actor_event(
            access,
            touched,
            "PostTouch",
            (&mut parameter as *mut *mut c_void).cast(),
            std::mem::size_of::<*mut c_void>(),
        )
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_is_probing_ended_rotation(actor: *mut c_void, out: *mut i32) -> i32 {
    phys_is_probing_named(actor, "EndedRotation", out)
}

unsafe extern "C" fn phys_is_probing_may_fall(actor: *mut c_void, out: *mut i32) -> i32 {
    phys_is_probing_named(actor, "MayFall", out)
}

unsafe fn phys_is_probing_named(actor: *mut c_void, name: &str, out: *mut i32) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    match context.run(|access| {
        let name = access.intern_str(name, hp2_core::name_owner::FindName::Find)?;
        unsafe { access.is_probing(actor.cast(), name) }.map(i32::from)
    }) {
        Ok(value) => { if !out.is_null() { unsafe { *out = value; } } 0 }
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_event_ended_rotation(actor: *mut c_void) -> i32 {
    phys_simple_event(actor, "EndedRotation", 0)
}

unsafe extern "C" fn phys_sin_tab(index: i32) -> f32 {
    hp2_core::math::sin_tab_inline(index, hp2_core::math::trig_table())
}

unsafe extern "C" fn phys_cos_tab(index: i32) -> f32 {
    hp2_core::math::cos_tab_inline(index, hp2_core::math::trig_table())
}

unsafe extern "C" fn phys_event_may_fall(actor: *mut c_void) -> i32 {
    phys_simple_event(actor, "MayFall", 0)
}

unsafe extern "C" fn phys_event_do_jump(actor: *mut c_void, value: f32) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    let mut parameter = value;
    match context.run(|access| unsafe {
        execute_actor_event(access, actor, "DoJump", (&mut parameter as *mut f32).cast(), std::mem::size_of::<f32>())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_far_move_actor(actor: *mut c_void, location: *const FVector) -> i32 {
    phys_far_move_impl(actor, location, false)
}

unsafe extern "C" fn phys_trailer_far_move_actor(actor: *mut c_void, location: *const FVector) -> i32 {
    phys_far_move_impl(actor, location, true)
}

unsafe extern "C" fn phys_far_move_impl(actor: *mut c_void, location: *const FVector, no_check: bool) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if location.is_null() { context.ensure_error("Canonical FarMoveActor has a null location"); return 1; }
    let actor = actor.cast::<AActor>();
    match context.run(|access| unsafe {
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() { return Err(RuntimeError::new("Canonical FarMoveActor has no Level")); }
        crate::level_move_owner::far_move_actor(access, level, actor, *location, false, no_check).map(|_| ())
    }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_mesh_bone_coords(
    mesh: *mut c_void,
    owner: *mut c_void,
    bone: i32,
    out: *mut c_void,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let context = unsafe { &*context };
    if out.is_null() { context.ensure_error("Canonical GetBoneCoords has a null output"); return 1; }
    match context.run(|access| unsafe {
        crate::render_owner::bone_coords(
            access,
            mesh,
            owner.cast::<AActor>(),
            bone,
        )
    }) {
        Ok(coords) => { unsafe { out.cast::<hp2_abi::FCoords>().write(coords); } 0 }
        Err(error) => error.0,
    }
}

unsafe fn shared_services(services: *const Hp2PhysicServices) -> &'static Hp2PhysicServices {
    if services.is_null() {
        &CANONICAL_PHYSIC_SERVICES
    } else {
        unsafe { &*services }
    }
}

macro_rules! phys_status_route {
    ($context:expr, $body:expr) => {{
        let context = unsafe { &*$context };
        match $body {
            Ok(()) => 0,
            Err(error) => {
                context.ensure_error("Canonical physics route failed");
                error.0
            }
        }
    }};
}

unsafe extern "C" fn phys_call_calc_velocity(
    pawn: *mut hp2_abi::engine::generated::APawn,
    accel_dir: *const FVector,
    delta_time: f32,
    max_speed: f32,
    friction: f32,
    b_fluid: i32,
    b_brake: i32,
    b_buoyant: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if accel_dir.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe {
        crate::physic::calc_velocity(pawn, svc, *accel_dir, delta_time, max_speed, friction, b_fluid, b_brake, b_buoyant)
    })
}

unsafe extern "C" fn phys_call_mount(
    pawn: *mut hp2_abi::engine::generated::APawn,
    mount_dir: *const FVector,
    hit: *mut FCheckResult,
    services: *const Hp2PhysicServices,
    result: *mut i32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if mount_dir.is_null() || hit.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe { crate::physic::mount(pawn, svc, *mount_dir, hit) } {
        Ok(value) => { if !result.is_null() { unsafe { *result = i32::from(value); } } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical Mount route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_step_up(
    pawn: *mut hp2_abi::engine::generated::APawn,
    grav_dir: *const FVector,
    desired_dir: *const FVector,
    delta: *const FVector,
    hit: *mut FCheckResult,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if grav_dir.is_null() || desired_dir.is_null() || delta.is_null() || hit.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe {
        crate::physic::step_up(pawn, svc, *grav_dir, *desired_dir, *delta, hit)
    })
}

unsafe extern "C" fn phys_call_process_hit_wall(
    this: *mut AActor,
    hit_normal: *const FVector,
    hit_actor: *mut c_void,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if hit_normal.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe {
        crate::physic::process_hit_wall(this, svc, *hit_normal, hit_actor)
    })
}

unsafe extern "C" fn phys_call_process_landed(
    this: *mut AActor,
    hit_normal: *const FVector,
    hit_actor: *mut c_void,
    remaining_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if hit_normal.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe {
        crate::physic::process_landed(this, svc, *hit_normal, hit_actor, remaining_time, iterations)
    })
}

unsafe extern "C" fn phys_call_find_base(this: *mut AActor, _services: *const Hp2PhysicServices) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    match unsafe { &*context }.run(|access| unsafe { find_base(access, this) }) {
        Ok(()) => 0,
        Err(error) => error.0,
    }
}

unsafe extern "C" fn phys_call_phys_trailer(
    this: *mut AActor,
    delta_time: f32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::dispatch::phys_trailer(this, svc, delta_time) })
}

unsafe extern "C" fn phys_call_phys_moving_brush(
    this: *mut AActor,
    delta_time: f32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::dispatch::phys_moving_brush(this, svc, delta_time) })
}

unsafe extern "C" fn phys_call_physics_rotation_actor(
    this: *mut AActor,
    delta_time: f32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::dispatch::physics_rotation_actor(this, svc, delta_time) })
}

unsafe extern "C" fn phys_call_physics_rotation_pawn(
    this: *mut hp2_abi::engine::generated::APawn,
    delta_time: f32,
    old_velocity: *const FVector,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if old_velocity.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe {
        crate::physic::dispatch::physics_rotation_pawn(this, svc, delta_time, *old_velocity)
    })
}

unsafe extern "C" fn phys_call_get_desired_rotation_at_pause(
    this: *mut hp2_abi::engine::generated::AInterpolationPoint,
    pause_num: i32,
    new_location: *const FVector,
    services: *const Hp2PhysicServices,
    result: *mut hp2_abi::engine::generated::FRotator,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if new_location.is_null() || result.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe {
        crate::physic::interp::get_desired_rotation_at_pause(this, svc, pause_num, *new_location)
    } {
        Ok(value) => { unsafe { result.write(value); } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical interpolation route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_get_desired_rotation_at_position(
    this: *mut hp2_abi::engine::generated::AInterpolationPoint,
    pos_offset: i32,
    pause_num: i32,
    new_location: *const FVector,
    services: *const Hp2PhysicServices,
    result: *mut hp2_abi::engine::generated::FRotator,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if new_location.is_null() || result.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe {
        crate::physic::interp::get_desired_rotation_at_position(this, svc, pos_offset, pause_num, *new_location)
    } {
        Ok(value) => { unsafe { result.write(value); } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical interpolation route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_get_interpolated_position(
    this: *mut hp2_abi::engine::generated::AInterpolationPoint,
    old_coords: *const hp2_abi::FCoords,
    start_control: *const FVector,
    phys_alpha: f32,
    pause_num: i32,
    services: *const Hp2PhysicServices,
    result: *mut hp2_abi::FCoords,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if old_coords.is_null() || start_control.is_null() || result.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe {
        crate::physic::interp::get_interpolated_position(this, svc, *old_coords, *start_control, phys_alpha, pause_num)
    } {
        Ok(value) => { unsafe { result.write(value); } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical interpolation route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_start_swimming(
    this: *mut hp2_abi::engine::generated::APawn,
    old_velocity: *const FVector,
    time_tick: f32,
    remaining_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if old_velocity.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe {
        crate::physic::swim::start_swimming(this, svc, *old_velocity, time_tick, remaining_time, iterations)
    })
}

unsafe extern "C" fn phys_call_phys_flying(
    this: *mut hp2_abi::engine::generated::APawn,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::swim::phys_flying(this, svc, delta_time, iterations) })
}

unsafe extern "C" fn phys_call_swim(
    this: *mut hp2_abi::engine::generated::APawn,
    delta: *const FVector,
    hit: *mut FCheckResult,
    services: *const Hp2PhysicServices,
    result: *mut f32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if delta.is_null() || hit.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe { crate::physic::swim::swim(this, svc, *delta, hit) } {
        Ok(value) => { if !result.is_null() { unsafe { *result = value; } } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical Swim route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_find_water_line(
    this: *mut hp2_abi::engine::generated::APawn,
    start: *const FVector,
    end: *mut FVector,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if start.is_null() || end.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::swim::find_water_line(this, svc, *start, end) })
}

unsafe extern "C" fn phys_call_phys_swimming(
    this: *mut hp2_abi::engine::generated::APawn,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::swim::phys_swimming(this, svc, delta_time, iterations) })
}

unsafe extern "C" fn phys_call_phys_falling(
    this: *mut AActor,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::fall::phys_falling(this, svc, delta_time, iterations) })
}

unsafe extern "C" fn phys_call_phys_projectile(
    this: *mut AActor,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::motion::phys_projectile(this, svc, delta_time, iterations) })
}

unsafe extern "C" fn phys_call_phys_rolling(
    this: *mut AActor,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::motion::phys_rolling(this, svc, delta_time, iterations) })
}

unsafe extern "C" fn phys_call_check_floor(
    this: *mut hp2_abi::engine::generated::APawn,
    dir: *const FVector,
    hit: *mut FCheckResult,
    services: *const Hp2PhysicServices,
    result: *mut i32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if dir.is_null() || hit.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe { crate::physic::motion::check_floor(this, svc, *dir, hit) } {
        Ok(value) => { if !result.is_null() { unsafe { *result = value; } } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical CheckFloor route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_find_new_floor(
    this: *mut hp2_abi::engine::generated::APawn,
    old_location: *const FVector,
    delta_time: f32,
    remaining_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
    result: *mut i32,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    if old_location.is_null() { return 1; }
    let svc = unsafe { shared_services(services) };
    match unsafe {
        crate::physic::motion::find_new_floor(this, svc, *old_location, delta_time, remaining_time, iterations)
    } {
        Ok(value) => { if !result.is_null() { unsafe { *result = value; } } 0 }
        Err(error) => { unsafe { &*context }.ensure_error("Canonical FindNewFloor route failed"); error.0 }
    }
}

unsafe extern "C" fn phys_call_phys_spider(
    this: *mut hp2_abi::engine::generated::APawn,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::motion::phys_spider(this, svc, delta_time, iterations) })
}

unsafe extern "C" fn phys_call_phys_walking(
    this: *mut hp2_abi::engine::generated::APawn,
    delta_time: f32,
    iterations: i32,
    services: *const Hp2PhysicServices,
) -> i32 {
    let Some(context) = current_physic_context() else { return 1 };
    let svc = unsafe { shared_services(services) };
    phys_status_route!(context, unsafe { crate::physic::walk::phys_walking(this, svc, delta_time, iterations) })
}

static CANONICAL_PHYSIC_SERVICES: Hp2PhysicServices = Hp2PhysicServices {
    is_a: phys_is_a,
    is_probing_hit_wall: phys_is_probing_hit_wall,
    app_frand: phys_app_frand,
    bsp_surf_special: phys_bsp_surf_special,
    rotation_vector: phys_rotation_vector,
    move_actor: phys_move_actor,
    single_line_check: phys_single_line_check,
    event_mount: phys_event_mount,
    event_hit_wall: phys_event_hit_wall,
    event_landed: phys_event_landed,
    point_region_zone: phys_point_region_zone,
    debugf_flew_out: phys_debugf_flew_out,
    destroy_actor: phys_destroy_actor,
    event_falling: phys_event_falling,
    log_fell_out_of_world: phys_log_fell_out_of_world,
    event_fell_out_of_world: phys_event_fell_out_of_world,
    rotation_from_vector: phys_rotation_from_vector,
    find_view_target: phys_find_view_target,
    event_update_camera: phys_event_update_camera,
    event_interpolate_end: phys_event_interpolate_end,
    event_finished_interpolation: phys_event_finished_interpolation,
    event_key_frame_reached: phys_event_key_frame_reached,
    mover_freeze_log: phys_mover_freeze_log,
    event_post_touch: phys_event_post_touch,
    is_probing_ended_rotation: phys_is_probing_ended_rotation,
    event_ended_rotation: phys_event_ended_rotation,
    sin_tab: phys_sin_tab,
    cos_tab: phys_cos_tab,
    is_probing_may_fall: phys_is_probing_may_fall,
    event_may_fall: phys_event_may_fall,
    event_do_jump: phys_event_do_jump,
    far_move_actor: phys_far_move_actor,
    debugf_fell_out: phys_debugf_fell_out,
    single_line_check_no_source: phys_single_line_check_no_source,
    mesh_bone_coords: phys_mesh_bone_coords,
    trailer_far_move_actor: phys_trailer_far_move_actor,
    call_calc_velocity: phys_call_calc_velocity,
    call_mount: phys_call_mount,
    call_step_up: phys_call_step_up,
    call_process_hit_wall: phys_call_process_hit_wall,
    call_process_landed: phys_call_process_landed,
    call_find_base: phys_call_find_base,
    call_phys_trailer: phys_call_phys_trailer,
    call_phys_moving_brush: phys_call_phys_moving_brush,
    call_physics_rotation_actor: phys_call_physics_rotation_actor,
    call_physics_rotation_pawn: phys_call_physics_rotation_pawn,
    call_get_desired_rotation_at_pause: phys_call_get_desired_rotation_at_pause,
    call_get_desired_rotation_at_position: phys_call_get_desired_rotation_at_position,
    call_get_interpolated_position: phys_call_get_interpolated_position,
    call_start_swimming: phys_call_start_swimming,
    call_phys_flying: phys_call_phys_flying,
    call_swim: phys_call_swim,
    call_find_water_line: phys_call_find_water_line,
    call_phys_swimming: phys_call_phys_swimming,
    call_phys_falling: phys_call_phys_falling,
    call_phys_projectile: phys_call_phys_projectile,
    call_phys_rolling: phys_call_phys_rolling,
    call_check_floor: phys_call_check_floor,
    call_find_new_floor: phys_call_find_new_floor,
    call_phys_spider: phys_call_phys_spider,
    call_phys_walking: phys_call_phys_walking,
};


#[cfg(test)]
mod tests;
