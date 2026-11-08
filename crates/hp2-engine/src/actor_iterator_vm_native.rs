//! Canonical actor foreach arguments and live world-query leaves.

use std::{cell::RefCell, ffi::c_void, ptr};
use hp2_abi::{FVector, engine::generated::{AActor, FFrame, ULevel, UObject}};
use hp2_core::{
    canonical_vm_native::{finish_arguments, run_iterator, step_argument, step_argument_out, VmNativeSpec},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
};
use crate::actor_iterator::{self, Hp2ActorIteratorServices};

struct Context<'a> {
    access: CoreAccess<'a>,
    error: RefCell<Option<RuntimeError>>,
}
impl Context<'_> {
    unsafe fn output<T>(&self, out: *mut T, value: RuntimeResult<T>) -> i32 {
        match value {
            Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
            Err(error) => {
                let mut first = self.error.borrow_mut();
                if first.is_none() { *first = Some(error); }
                1
            }
        }
    }
    fn checked(&self, value: Result<(), i32>) -> RuntimeResult<()> {
        value.map_err(|_| self.error.borrow_mut().take()
            .unwrap_or_else(|| RuntimeError::new("Canonical actor iterator selection failed")))
    }
}
unsafe fn context<'a>(raw: *mut c_void) -> &'a Context<'a> { unsafe { &*raw.cast() } }
unsafe fn level(actor: *mut c_void) -> RuntimeResult<*mut ULevel> {
    if actor.is_null() { return Err(RuntimeError::new("Null canonical iterator receiver")); }
    let level = unsafe { ptr::addr_of!((*actor.cast::<AActor>()).x_level).read_unaligned().cast::<ULevel>() };
    if level.is_null() { return Err(RuntimeError::new("Canonical iterator receiver has no Level")); }
    Ok(level)
}
unsafe extern "C" fn count(raw: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32 {
    unsafe { context(raw).output(out, level(actor).and_then(|level| crate::level_owner::actor_count(level))) }
}
unsafe extern "C" fn at(raw: *mut c_void, actor: *mut c_void, index: i32, out: *mut *mut c_void) -> i32 {
    let value = (|| unsafe {
        let level = level(actor)?;
        let count = crate::level_owner::actor_count(level)?;
        if index < 0 || index >= count { return Err(RuntimeError::new("Canonical iterator actor index is outside Level")); }
        Ok(crate::level_owner::actor_at(level, index).cast())
    })();
    unsafe { context(raw).output(out, value) }
}
unsafe extern "C" fn default_class(raw: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { context(raw) };
    unsafe { context.output(out, context.access.native_class("Engine.Actor").map(|class| class.cast())) }
}
unsafe extern "C" fn is_a(raw: *mut c_void, actor: *mut c_void, class: *mut c_void, out: *mut i32) -> i32 {
    unsafe { context(raw).output(out, Ok(i32::from(hp2_core::object_model::is_a(actor, class)))) }
}
unsafe extern "C" fn hidden(raw: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32 {
    unsafe { context(raw).output(out, Ok(i32::from((*actor.cast::<AActor>()).b_hidden()))) }
}
unsafe extern "C" fn fast_line(raw: *mut c_void, actor: *mut c_void, candidate: *mut c_void, trace: *const c_void, out: *mut i32) -> i32 {
    let value = (|| unsafe {
        let level = level(actor)?;
        let model = ptr::addr_of!((*level).model).read_unaligned().cast();
        let end = ptr::addr_of!((*candidate.cast::<AActor>()).location).read_unaligned();
        let start = trace.cast::<FVector>().read_unaligned();
        crate::collision_owner::model_fast_line_check(model, end, start).map(i32::from)
    })();
    unsafe { context(raw).output(out, value) }
}
unsafe extern "C" fn tag_none(raw: *mut c_void, tag: *const c_void, out: *mut i32) -> i32 {
    unsafe { context(raw).output(out, Ok(i32::from(tag.cast::<i32>().read_unaligned() == 0))) }
}
unsafe extern "C" fn tag_matches(raw: *mut c_void, actor: *mut c_void, tag: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let actual = ptr::addr_of!((*actor.cast::<AActor>()).tag).cast::<i32>().read_unaligned();
        context(raw).output(out, Ok(i32::from(actual == tag.cast::<i32>().read_unaligned())))
    }
}
unsafe extern "C" fn in_zone(raw: *mut c_void, actor: *mut c_void, zone: *mut c_void, out: *mut i32) -> i32 {
    unsafe { context(raw).output(out, Ok(i32::from(crate::actor::is_in_zone(actor.cast(), zone)))) }
}
fn services(context: &mut Context<'_>) -> Hp2ActorIteratorServices {
    Hp2ActorIteratorServices {
        context: (context as *mut Context<'_>).cast(),
        actor_count: count, actor_at: at, default_actor_class: default_class,
        actor_is_a: is_a, actor_is_hidden: hidden, actor_fast_line_check: fast_line,
        tag_is_none: tag_none, actor_tag_matches: tag_matches, actor_is_in_zone: in_zone,
    }
}

#[derive(Clone, Copy)]
enum Kind { Child, Based, Touching, Zone, Radius, Visible }

unsafe fn iterate(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, kind: Kind) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical actor iterator receiver")); }
    let mut base_class = ptr::null_mut::<c_void>();
    let mut scratch = ptr::null_mut::<c_void>();
    let mut radius = 0_f32;
    let mut trace = FVector { x: 0., y: 0., z: 0. };
    let out = unsafe {
        step_argument(access, frame, (&raw mut base_class).cast())?;
        let out = step_argument_out(access, frame, (&raw mut scratch).cast())?.cast::<*mut c_void>();
        if matches!(kind, Kind::Radius | Kind::Visible) {
            step_argument(access, frame, (&raw mut radius).cast())?;
            trace = ptr::addr_of!((*object.cast::<AActor>()).location).read_unaligned();
            step_argument(access, frame, (&raw mut trace).cast())?;
        }
        finish_arguments(access, frame)?;
        out
    };
    let mut context = Context { access, error: RefCell::new(None) };
    let services = services(&mut context);
    let mut cursor = 0;
    unsafe { run_iterator(access, frame, || {
        let mut index = -1;
        let selected = match kind {
            Kind::Child => actor_iterator::child_actors_next(object.cast(), &raw mut base_class, &raw mut cursor, out, &raw mut index, &services),
            Kind::Based => actor_iterator::based_actors_next(object.cast(), &raw mut base_class, &raw mut cursor, out, &raw mut index, &services),
            Kind::Touching => actor_iterator::touching_actors_next(object.cast(), &raw mut base_class, &raw mut cursor, out, &services),
            Kind::Zone => actor_iterator::zone_actors_next(object.cast(), &raw mut base_class, &raw mut cursor, out, &services),
            Kind::Radius => actor_iterator::radius_actors_next(object.cast(), &raw mut base_class, radius, &trace, &raw mut cursor, out, &raw mut index, &services),
            Kind::Visible => actor_iterator::visible_actors_next(object.cast(), &raw mut base_class, radius, &trace, &raw mut cursor, out, &raw mut index, &services),
        };
        context.checked(selected)?;
        Ok(!out.read_unaligned().is_null())
    }) }
}
unsafe fn child(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { iterate(access, object, frame, Kind::Child) }
}
unsafe fn based(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { iterate(access, object, frame, Kind::Based) }
}
unsafe fn touching(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { iterate(access, object, frame, Kind::Touching) }
}
unsafe fn zone(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { iterate(access, object, frame, Kind::Zone) }
}
unsafe fn radius(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { iterate(access, object, frame, Kind::Radius) }
}
unsafe fn visible(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { iterate(access, object, frame, Kind::Visible) }
}
unsafe fn trace_actors(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical TraceActors receiver")); }
    let mut base_class = ptr::null_mut::<c_void>();
    let mut actor_scratch = ptr::null_mut::<c_void>();
    let mut location_scratch = FVector { x: 0., y: 0., z: 0. };
    let mut normal_scratch = location_scratch;
    let mut end = location_scratch;
    let mut extent = location_scratch;
    unsafe {
        // BaseClass is evaluated by the source but does not filter TraceActors hits.
        step_argument(access, frame, (&raw mut base_class).cast())?;
        let out_actor = step_argument_out(access, frame, (&raw mut actor_scratch).cast())?.cast::<*mut c_void>();
        let out_location = step_argument_out(access, frame, (&raw mut location_scratch).cast())?.cast::<FVector>();
        let out_normal = step_argument_out(access, frame, (&raw mut normal_scratch).cast())?.cast::<FVector>();
        step_argument(access, frame, (&raw mut end).cast())?;
        let mut start = ptr::addr_of!((*object.cast::<AActor>()).location).read_unaligned();
        step_argument(access, frame, (&raw mut start).cast())?;
        step_argument(access, frame, (&raw mut extent).cast())?;
        finish_arguments(access, frame)?;
        let level = level(object.cast())?;
        let level_info = ptr::addr_of!((*object.cast::<AActor>()).level).read_unaligned().cast();
        let memory = access.scratch_memory();
        let mark = memory.mark();
        let mut hit = crate::level_trace_owner::multi_line_check(
            access, level, level_info, end, start, extent, true, 0,
        )?.cast::<c_void>();
        let outcome = run_iterator(access, frame, || {
            let has_hit = !hit.is_null();
            actor_iterator::trace_actors_next(&raw mut hit, out_actor, out_location, out_normal)
                .map_err(|_| RuntimeError::new("Canonical TraceActors selection failed"))?;
            Ok(has_hit)
        });
        // Native FMemMark has no destructor: checked errors deliberately do not pop.
        if outcome.is_ok() { memory.pop(&mark); }
        outcome
    }
}

unsafe fn visible_colliding(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical VisibleCollidingActors receiver")); }
    let mut base_class = ptr::null_mut::<c_void>();
    let mut actor_scratch = ptr::null_mut::<c_void>();
    let mut radius = 0_f32;
    let mut ignore_hidden = 0_u32;
    unsafe {
        step_argument(access, frame, (&raw mut base_class).cast())?;
        let out = step_argument_out(access, frame, (&raw mut actor_scratch).cast())?.cast::<*mut c_void>();
        step_argument(access, frame, (&raw mut radius).cast())?;
        let mut trace = ptr::addr_of!((*object.cast::<AActor>()).location).read_unaligned();
        step_argument(access, frame, (&raw mut trace).cast())?;
        step_argument(access, frame, (&raw mut ignore_hidden).cast())?;
        finish_arguments(access, frame)?;
        if radius == 0. { radius = 1000.; }
        let level = level(object.cast())?;
        let memory = access.scratch_memory();
        let mark = memory.mark();
        let mut link = crate::level_owner::hash_radius_check(access, level, trace, radius)?.cast::<c_void>();
        let mut context = Context { access, error: RefCell::new(None) };
        let services = services(&mut context);
        let outcome = run_iterator(access, frame, || {
            context.checked(actor_iterator::visible_colliding_actors_next(
                &raw mut link, object.cast(), &raw mut base_class, ignore_hidden as i32,
                &trace, out, &services,
            ))?;
            Ok(!out.read_unaligned().is_null())
        });
        if outcome.is_ok() { memory.pop(&mark); }
        outcome
    }
}

unsafe fn all_actors(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    use std::ptr::NonNull;
    use crate::core_owner::{ActorAllActorsEvent, EngineState};
    let requestor = NonNull::new(object.cast::<AActor>())
        .ok_or_else(|| RuntimeError::new("Null canonical AllActors receiver"))?;
    let mut base_class = ptr::null_mut::<c_void>();
    let mut actor_scratch = ptr::null_mut::<c_void>();
    let mut tag = 0_i32;
    unsafe {
        step_argument(access, frame, (&raw mut base_class).cast())?;
        let out = step_argument_out(access, frame, (&raw mut actor_scratch).cast())?.cast::<*mut c_void>();
        step_argument(access, frame, (&raw mut tag).cast())?;
        finish_arguments(access, frame)?;
        let mut context = Context { access, error: RefCell::new(None) };
        let services = services(&mut context);
        let mut cursor = 0;
        let mut order = 0_i32;
        run_iterator(access, frame, || {
            let mut slot = -1;
            context.checked(actor_iterator::all_actors_next(
                object.cast(), &raw mut base_class, (&raw const tag).cast(),
                &raw mut cursor, out, &raw mut slot, &services,
            ))?;
            let exhausted = out.read_unaligned().is_null();
            if !exhausted {
                let current_order = order;
                order = order.wrapping_add(1);
                hp2_core::ansi_trace_patrol::record_point_iterator_owned(
                    access, object, base_class.cast(), tag, slot,
                    out.read_unaligned().cast(), current_order,
                )?;
            }
            let state = access.owner_context::<EngineState>()
                .ok_or_else(|| RuntimeError::new("AllActors has no canonical Engine owner"))?;
            // Fetch at every yield: callbacks may replace the binding or reenter iteration.
            let observer = state.actor_all_actors_observer.get();
            if let Some(observer) = observer {
                let requested_class = NonNull::new(base_class.cast())
                    .ok_or_else(|| RuntimeError::new("AllActors has no resolved requested class"))?;
                observer(access, ActorAllActorsEvent {
                    requestor, requested_class, tag,
                    yielded: NonNull::new(out.read_unaligned().cast()),
                    slot, exhausted,
                })?;
            }
            Ok(!exhausted)
        })
    }
}

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 9] = [
    VmNativeSpec { class_name: "Engine.Actor", function_name: "ChildActors", native_index: Some(305), handler: child },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "BasedActors", native_index: Some(306), handler: based },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "TouchingActors", native_index: Some(307), handler: touching },
    VmNativeSpec { class_name: "Engine.ZoneInfo", function_name: "ZoneActors", native_index: Some(308), handler: zone },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "RadiusActors", native_index: Some(310), handler: radius },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "VisibleActors", native_index: Some(311), handler: visible },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "TraceActors", native_index: Some(309), handler: trace_actors },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "VisibleCollidingActors", native_index: Some(312), handler: visible_colliding },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "AllActors", native_index: Some(304), handler: all_actors },
];
