//! Canonical world/service composition for the shared route policy.
use std::{cell::{Cell, RefCell}, ffi::c_void, mem::MaybeUninit, ptr};
use hp2_abi::{FVector, engine::generated::{APawn, ULevel, UObject, UProperty, UStruct}};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use crate::route::{Hp2RouteServices, Hp2SortedPathList};

struct Context { access: CoreAccess<'static>, error: RefCell<Option<RuntimeError>> }
thread_local! { static ACTIVE: Cell<*mut Context> = const { Cell::new(ptr::null_mut()) }; }
struct Scope(*mut Context);
impl Drop for Scope { fn drop(&mut self) { ACTIVE.with(|slot| slot.set(self.0)); } }
pub(crate) fn with_services<T>(access: CoreAccess<'_>, body: impl FnOnce(&'static Hp2RouteServices) -> RuntimeResult<T>) -> RuntimeResult<T> {
    let mut context = Context {
        // The synchronous/reentrant scope ends before CoreAccess can expire.
        access: unsafe { std::mem::transmute::<CoreAccess<'_>, CoreAccess<'static>>(access) },
        error: RefCell::new(None),
    };
    let _scope = Scope(ACTIVE.with(|slot| slot.replace(&raw mut context)));
    match body(&SERVICES) {
        Ok(value) => Ok(value),
        Err(error) => Err(context.error.into_inner().unwrap_or(error)),
    }
}
pub(crate) fn output<T>(out: *mut T, body: impl FnOnce(CoreAccess<'_>) -> RuntimeResult<T>) -> i32 {
    ACTIVE.with(|slot| {
        let raw = slot.get();
        if raw.is_null() || out.is_null() { return 1; }
        let context = unsafe { &*raw };
        match body(context.access) {
            Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
            Err(error) => { context.error.borrow_mut().get_or_insert(error); 1 }
        }
    })
}
pub(crate) fn status(body: impl FnOnce(CoreAccess<'_>) -> RuntimeResult<()>) -> i32 { let mut value = (); output(&raw mut value, body) }
unsafe extern "C" fn random(out: *mut f32) -> i32 { output(out, |access| Ok(access.rand_range_float(0., 1.))) }
unsafe extern "C" fn far_move(level: *mut c_void, actor: *mut c_void, point: *const c_void, test: i32, no_check: i32, out: *mut i32) -> i32 {
    output(out, |access| unsafe { crate::level_move_owner::far_move_actor(access, level.cast(), actor.cast(), point.cast::<FVector>().read_unaligned(), test != 0, no_check != 0).map(i32::from) })
}
unsafe extern "C" fn point_reachable(pawn: *mut c_void, point: *const c_void, know: i32, out: *mut i32) -> i32 {
    output(out, |access| unsafe { crate::pawn_reach_owner::point_reachable(access, pawn.cast(), point.cast::<FVector>().read_unaligned(), know) })
}
unsafe extern "C" fn actor_reachable(pawn: *mut c_void, actor: *mut c_void, know: i32, out: *mut i32) -> i32 {
    output(out, |access| unsafe { crate::pawn_reach_owner::actor_reachable(access, pawn.cast(), actor.cast(), know) })
}
unsafe extern "C" fn fast_line(model: *mut c_void, end: *const c_void, start: *const c_void, out: *mut u8) -> i32 {
    output(out, |_| unsafe { crate::collision_owner::model_fast_line_check(model.cast(), end.cast::<FVector>().read_unaligned(), start.cast::<FVector>().read_unaligned()).map(u8::from) })
}
unsafe extern "C" fn hit_actor(level: *mut c_void, source: *mut c_void, end: *const c_void, start: *const c_void, out: *mut *mut c_void) -> i32 {
    output(out, |access| unsafe {
        let mut hit = crate::levact::check_result(1.);
        crate::level_trace_owner::single_line_check(access, level.cast(), &raw mut hit, source.cast(), end.cast::<FVector>().read_unaligned(), start.cast::<FVector>().read_unaligned(), 0x06, FVector { x: 0., y: 0., z: 0. }, 0)?;
        Ok(hit.actor)
    })
}
unsafe extern "C" fn navigation_list(level: *mut c_void, out: *mut *mut c_void) -> i32 {
    output(out, |_| unsafe { crate::package_map_level_owner::level_info(level.cast::<ULevel>()).map(|info| (*info).navigation_point_list) })
}
unsafe extern "C" fn is_a(actor: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    output(out, |access| unsafe {
        let name = match kind { 0 => "NavigationPoint", 1 => "Pawn", 2 => "InventorySpot", 3 => "Mover", _ => return Err(RuntimeError::new("Invalid route class selector")) };
        crate::actor_physics_owner::is_a_named(access, actor, name).map(i32::from)
    })
}
unsafe extern "C" fn player_only(mover: *mut c_void, out: *mut i32) -> i32 {
    output(out, |access| unsafe {
        let class = (*mover.cast::<UObject>()).class.cast();
        let name = access.intern_str("bPlayerOnly", FindName::Find)?;
        let property = access.find_field(class, name, access.native_class("Core.BoolProperty")?).cast::<hp2_abi::engine::generated::UBoolProperty>();
        if property.is_null() { return Err(RuntimeError::new("Mover bPlayerOnly metadata is unavailable")); }
        let offset = (*property.cast::<UProperty>()).offset;
        if offset < 0 { return Err(RuntimeError::new("Negative Mover bPlayerOnly offset")); }
        let bits = mover.cast::<u8>().add(offset as usize).cast::<u32>().read_unaligned();
        Ok(i32::from(bits & (*property).bit_mask != 0))
    })
}

/// Source Pawn-argument events use reflected offsets, not host-ABI struct padding.
pub(crate) unsafe fn pawn_event<T: Copy + Default>(access: CoreAccess<'_>, receiver: *mut UObject, event: &str, argument: &str, pawn: *mut c_void) -> RuntimeResult<T> {
    unsafe {
        let function = access.find_object_function_by_name(receiver, access.intern_str(event, FindName::Find)?, false)?;
        if function.is_null() { return Err(RuntimeError::new("Missing canonical route event").with_context(event)); }
        let property = access.find_field(function.cast::<UStruct>(), access.intern_str(argument, FindName::Find)?, access.native_class("Core.ObjectProperty")?).cast::<UProperty>();
        if property.is_null() { return Err(RuntimeError::new("Missing canonical route event argument").with_context(argument)); }
        let offset = (*property).offset;
        let result = usize::from((*function).return_value_offset);
        let size = usize::from((*function).parms_size);
        let mut storage = [0_u128; 2];
        if offset < 0 || offset as usize + size_of::<*mut c_void>() > size || result + size_of::<T>() > size || size > size_of_val(&storage) {
            return Err(RuntimeError::new("Invalid canonical route event layout").with_context(event));
        }
        let bytes = storage.as_mut_ptr().cast::<u8>();
        bytes.add(offset as usize).cast::<*mut c_void>().write_unaligned(pawn);
        bytes.add(result).cast::<T>().write_unaligned(T::default());
        crate::actor_owner::process_event(access, receiver.cast(), function, bytes.cast(), ptr::null_mut())?;
        Ok(bytes.add(result).cast::<T>().read_unaligned())
    }
}
unsafe extern "C" fn special_cost(node: *mut c_void, pawn: *mut c_void, out: *mut i32) -> i32 {
    output(out, |access| unsafe { pawn_event(access, node.cast(), "SpecialCost", "Seeker", pawn) })
}
unsafe extern "C" fn probing_touch(item: *mut c_void, out: *mut i32) -> i32 {
    output(out, |access| unsafe { access.is_probing(item.cast(), access.intern_str("Touch", FindName::Find)?).map(i32::from) })
}
unsafe extern "C" fn desirability(item: *mut c_void, pawn: *mut c_void, out: *mut f32) -> i32 {
    output(out, |access| unsafe { pawn_event(access, item.cast(), "BotDesireability", "Bot", pawn) })
}
unsafe extern "C" fn name_index(actor: *mut c_void, out: *mut i32) -> i32 { output(out, |_| Ok(i32::from_ne_bytes(unsafe { (*actor.cast::<UObject>()).name }))) }
unsafe extern "C" fn class(actor: *mut c_void, out: *mut *mut c_void) -> i32 { output(out, |_| Ok(unsafe { (*actor.cast::<UObject>()).class.cast() })) }
unsafe extern "C" fn log(kind: i32, actor: *mut c_void) -> i32 {
    status(|access| {
        let index = i32::from_ne_bytes(unsafe { (*actor.cast::<UObject>()).name });
        let name: String = access.name_text(index)?.into_iter().filter_map(char::from_u32).collect();
        let message = match kind { 0 => format!("startname is {name}"), 1 => format!("endname is {name}"), 2 => format!("Breadth path list overflow from {name}"), 3 => format!("1000 navigation nodes searched from {name}!"), _ => return Err(RuntimeError::new("Invalid route log selector")) };
        access.emit_log(access.intern_str(if kind == 3 { "DevPath" } else { "Log" }, FindName::Find)?, &message.chars().map(u32::from).collect::<Vec<_>>())
    })
}
unsafe extern "C" fn lists(count: i32, context: *mut c_void, run: unsafe extern "C" fn(*mut c_void, *mut Hp2SortedPathList, *mut Hp2SortedPathList) -> i32) -> i32 {
    let mut first = MaybeUninit::<Hp2SortedPathList>::uninit();
    let mut second = MaybeUninit::<Hp2SortedPathList>::uninit();
    unsafe {
        ptr::addr_of_mut!((*first.as_mut_ptr()).num_points).write(0);
        if count == 2 {
            ptr::addr_of_mut!((*second.as_mut_ptr()).num_points).write(0);
            run(context, first.as_mut_ptr(), second.as_mut_ptr())
        } else { run(context, first.as_mut_ptr(), ptr::null_mut()) }
    }
}
pub(crate) static SERVICES: Hp2RouteServices = Hp2RouteServices {
    app_frand: random, far_move_actor: far_move, point_reachable, actor_reachable,
    model_fast_line_check: fast_line, single_line_hit_actor: hit_actor,
    navigation_point_list: navigation_list, actor_is_a: is_a, mover_player_only: player_only,
    event_special_cost: special_cost, item_is_probing_touch: probing_touch, item_bot_desirability: desirability,
    actor_name_index: name_index, actor_class: class, log_actor: log, lists,
};

/// # Safety
/// Pawn, world and navigation nodes remain live through synchronous script callbacks.
pub unsafe fn clear_paths(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    with_services(access, |services| unsafe { crate::route::clear_paths(services, pawn).map_err(|_| RuntimeError::new("Canonical ClearPaths failed")) })
}
