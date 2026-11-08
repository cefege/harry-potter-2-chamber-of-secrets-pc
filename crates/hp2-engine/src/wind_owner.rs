//! Canonical Rust ownership for Engine's AWind registry and noise state.

use crate::actor_owner;
use hp2_abi::{FVector, engine::generated::{AActor, AWind, ULevel, UObject}};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, cell::{RefCell, UnsafeCell}, ptr::NonNull};

/// Engine-owned wind registry and process-local `FIntRandom` state.
///
/// Registry entries are non-owning: UObject storage remains owned by CoreOwner.
pub(crate) struct WindState {
    actors: RefCell<Vec<NonNull<AWind>>>,
    random: UnsafeCell<crate::wind::IntRandom>,
}

impl Default for WindState {
    fn default() -> Self {
        Self { actors: RefCell::new(Vec::new()), random: UnsafeCell::new(crate::wind::IntRandom::new()) }
    }
}

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Wind",
    package: "Engine",
    super_name: Some("Actor"),
    within: "Object",
    flags: 0,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<AWind>()),
    operations: Some(WIND_OPERATIONS),
    properties: &[],
    defaults: &[],
}];

const WIND_OPERATIONS: ClassOps = {
    let mut operations = actor_owner::ACTOR_OPERATIONS;
    operations.construct = construct_wind;
    operations.destroy = destroy_wind;
    operations
};

/// Adds an already-constructed wind to native-order global sampling membership.
unsafe fn construct_wind(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (actor_owner::ACTOR_OPERATIONS.construct)(access, object)?; }
    let state = wind_state(access)?;
    let wind = NonNull::new(object.cast::<AWind>())
        .ok_or_else(|| RuntimeError::new("Cannot construct null AWind"))?;
    state.actors.borrow_mut().push(wind);
    Ok(())
}

/// Removes registry membership before running AActor destruction.
unsafe fn destroy_wind(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let state = wind_state(access)?;
    if let Some(wind) = NonNull::new(object.cast::<AWind>()) {
        state.actors.borrow_mut().retain(|registered| *registered != wind);
    }
    unsafe { (actor_owner::ACTOR_OPERATIONS.destroy)(access, object) }
}

fn wind_state<'a>(access: CoreAccess<'a>) -> RuntimeResult<&'a WindState> {
    access
        .owner_context::<crate::core_owner::EngineState>()
        .map(|engine| &engine.wind)
        .ok_or_else(|| RuntimeError::new("Canonical EngineState is missing wind state"))
}

/// Reads the current native registry length without retaining a borrow.
pub fn wind_count(access: CoreAccess<'_>) -> RuntimeResult<usize> {
    Ok(wind_state(access)?.actors.borrow().len())
}

/// Reads the current wind at an index without retaining a borrow across policy callbacks.
pub(crate) fn wind_at(access: CoreAccess<'_>, index: usize) -> RuntimeResult<Option<NonNull<AWind>>> {
    Ok(wind_state(access)?.actors.borrow().get(index).copied())
}
///
/// The engine is serialized on its owner thread. `IntRandom::init` uses raw
/// table accesses and publishes initialization before invoking canonical RNG,
/// so reentry never creates overlapping Rust references to this value.
pub(crate) fn random_storage(access: CoreAccess<'_>) -> RuntimeResult<*mut crate::wind::IntRandom> {
    let state = wind_state(access)?;
    Ok(state.random.get())
}

/// Samples a canonical wind using the caller's genuine world line-check.
///
/// # Safety
/// `wind` remains a live canonical actor in `access` through any world callback.
/// The callback must line-check its canonical level's model without native vptrs.
pub unsafe fn get_wind(
    access: CoreAccess<'_>,
    wind: *mut AWind,
    location: &FVector,
    mut line_check: impl FnMut(NonNull<ULevel>, &FVector, &FVector) -> RuntimeResult<bool>,
) -> RuntimeResult<FVector> {
    unsafe {
        crate::wind::get_wind_with(
            random_storage(access)?,
            wind,
            location,
            || Ok(access.rand()),
            |level, start, end| {
                let level = NonNull::new(level)
                    .ok_or_else(|| RuntimeError::new("Non-permeating Wind has no canonical level"))?;
                line_check(level, start, end)
            },
            |rotation| Ok(hp2_core::math::rotator_vector(
                hp2_core::math::FRotator {
                    pitch: rotation.pitch,
                    yaw: rotation.yaw,
                    roll: rotation.roll,
                },
                hp2_core::math::trig_table(),
            )),
        )
    }
}

/// Samples the live owner registry in native order, rereading it after callbacks.
///
/// # Safety
/// Registered actors remain live during sampling; the line-check contract is
/// the same as `get_wind`.
pub unsafe fn get_total_wind(
    access: CoreAccess<'_>,
    location: &FVector,
    mut line_check: impl FnMut(NonNull<ULevel>, &FVector, &FVector) -> RuntimeResult<bool>,
) -> RuntimeResult<FVector> {
    crate::wind::get_total_wind_with(
        || wind_count(access),
        |index| {
            let wind = wind_at(access, index)?
                .ok_or_else(|| RuntimeError::new("Wind registry changed before sampling"))?;
            unsafe { get_wind(access, wind.as_ptr(), location, &mut line_check) }
        },
    )
}

/// Runs genuine superclass ticking before the shared gust integration.
///
/// # Safety
/// `wind` remains canonical and live. `super_tick` must perform actual canonical
/// AActor ticking, not only script Tick dispatch or a native-object bridge.
pub unsafe fn tick(
    access: CoreAccess<'_>,
    wind: *mut AWind,
    delta_time: f32,
    tick_type: i32,
    super_tick: impl FnOnce(*mut AActor, f32, i32) -> RuntimeResult<bool>,
) -> RuntimeResult<bool> {
    super_tick(wind.cast(), delta_time, tick_type)?;
    unsafe {
        crate::wind::tick_with(wind, delta_time, || Ok(access.rand_range_float(0.0, 1.0)))?;
    }
    Ok(true)
}

