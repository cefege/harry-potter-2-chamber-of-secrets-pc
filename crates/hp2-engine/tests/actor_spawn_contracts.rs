use hp2_engine::actor_spawn::{self, Hp2ActorSpawnServices};
use std::ffi::c_void;

struct Harness {
    events: [u8; 8],
    event_count: usize,
    fail_at: u8,
    spawned: *mut c_void,
    destroyed: u32,
    observed: [usize; 7],
}

impl Harness {
    fn new(spawned: *mut c_void) -> Self {
        Self {
            events: [0; 8],
            event_count: 0,
            fail_at: 0,
            spawned,
            destroyed: 0x8000_0001,
            observed: [0; 7],
        }
    }

    fn events(&self) -> &[u8] {
        &self.events[..self.event_count]
    }
}

unsafe fn record(context: *mut c_void, event: u8) -> i32 {
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.events[harness.event_count] = event;
    harness.event_count += 1;
    if harness.fail_at == event { 19 } else { 0 }
}

unsafe extern "C" fn trace_class(
    context: *mut c_void,
    class: *mut c_void,
    actor: *mut c_void,
) -> i32 {
    let status = unsafe { record(context, 1) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[0] = class as usize;
    harness.observed[1] = actor as usize;
    status
}

unsafe extern "C" fn trace_request(
    context: *mut c_void,
    class: *mut c_void,
    actor: *mut c_void,
) -> i32 {
    let status = unsafe { record(context, 2) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[0] = class as usize;
    harness.observed[1] = actor as usize;
    status
}

unsafe extern "C" fn spawn(
    context: *mut c_void,
    actor: *mut c_void,
    class: *mut c_void,
    owner: *mut c_void,
    location: *const c_void,
    rotation: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    let status = unsafe { record(context, 3) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[..5].copy_from_slice(&[
        actor as usize,
        class as usize,
        owner as usize,
        location as usize,
        rotation as usize,
    ]);
    if status == 0 {
        unsafe { *out = harness.spawned };
    }
    status
}

unsafe extern "C" fn trace_result(
    context: *mut c_void,
    actor: *mut c_void,
    spawned: *mut c_void,
) -> i32 {
    let status = unsafe { record(context, 4) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[0] = actor as usize;
    harness.observed[1] = spawned as usize;
    status
}

unsafe extern "C" fn trace_published(
    context: *mut c_void,
    actor: *mut c_void,
    spawned: *mut c_void,
) -> i32 {
    let status = unsafe { record(context, 5) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[0] = actor as usize;
    harness.observed[1] = spawned as usize;
    status
}

unsafe extern "C" fn set_tag(
    context: *mut c_void,
    spawned: *mut c_void,
    name: *const c_void,
) -> i32 {
    let status = unsafe { record(context, 6) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[0] = spawned as usize;
    harness.observed[1] = name as usize;
    status
}

unsafe extern "C" fn destroy(context: *mut c_void, actor: *mut c_void, out: *mut u32) -> i32 {
    let status = unsafe { record(context, 7) };
    let harness = unsafe { &mut *(context.cast::<Harness>()) };
    harness.observed[0] = actor as usize;
    if status == 0 {
        unsafe { *out = harness.destroyed };
    }
    status
}

fn services(harness: &mut Harness) -> Hp2ActorSpawnServices {
    Hp2ActorSpawnServices {
        context: (harness as *mut Harness).cast(),
        trace_class_resolution: trace_class,
        trace_spawn_request: trace_request,
        spawn,
        trace_spawn_result: trace_result,
        trace_spawn_published: trace_published,
        set_tag,
        destroy,
    }
}

#[test]
fn spawn_preserves_optional_defaults_and_applies_tag_after_publication() {
    let mut bytes = [0_u8; 7];
    let (actor, class, _owner, name, location, rotation, spawned) = (
        &mut bytes[0] as *mut u8,
        &mut bytes[1] as *mut u8,
        &mut bytes[2] as *mut u8,
        &mut bytes[3] as *mut u8,
        &mut bytes[4] as *mut u8,
        &mut bytes[5] as *mut u8,
        &mut bytes[6] as *mut u8,
    );
    let mut harness = Harness::new(spawned.cast());
    let services = services(&mut harness);
    let mut out = std::ptr::null_mut();
    let result = unsafe {
        actor_spawn::exec_spawn(
            actor.cast(),
            class.cast(),
            std::ptr::null_mut(),
            name.cast(),
            location.cast(),
            rotation.cast(),
            &services,
            &mut out,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(harness.events(), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(out, spawned.cast());
    assert_eq!(
        harness.observed,
        [
            spawned as usize,
            name as usize,
            0,
            location as usize,
            rotation as usize,
            0,
            0
        ]
    );
}

#[test]
fn null_class_traces_resolution_without_spawning() {
    let mut bytes = [0_u8; 3];
    let actor = &mut bytes[0] as *mut u8;
    let name = &mut bytes[1] as *mut u8;
    let location = &mut bytes[2] as *mut u8;
    let mut harness = Harness::new(std::ptr::null_mut());
    let services = services(&mut harness);
    let mut out = 1_usize as *mut c_void;
    let result = unsafe {
        actor_spawn::exec_spawn(
            actor.cast(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name.cast(),
            location.cast(),
            std::ptr::null(),
            &services,
            &mut out,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(harness.events(), &[1]);
    assert!(out.is_null());
}

#[test]
fn spawn_failure_stops_later_effects_and_destroy_preserves_result_bits() {
    let mut bytes = [0_u8; 4];
    let actor = &mut bytes[0] as *mut u8;
    let class = &mut bytes[1] as *mut u8;
    let location = &mut bytes[2] as *mut u8;
    let rotation = &mut bytes[3] as *mut u8;
    let mut harness = Harness::new(std::ptr::null_mut());
    harness.fail_at = 3;
    let services = services(&mut harness);
    let mut out = 1_usize as *mut c_void;
    let result = unsafe {
        actor_spawn::exec_spawn(
            actor.cast(),
            class.cast(),
            std::ptr::null_mut(),
            std::ptr::null(),
            location.cast(),
            rotation.cast(),
            &services,
            &mut out,
        )
    };
    assert_eq!(result, Err(19));
    assert_eq!(harness.events(), &[1, 2, 3]);
    assert_eq!(out, 1_usize as *mut c_void);

    harness.fail_at = 0;
    harness.events = [0; 8];
    harness.event_count = 0;
    let mut destroyed = 0_u32;
    let result = unsafe { actor_spawn::exec_destroy(actor.cast(), &services, &mut destroyed) };
    assert_eq!(result, Ok(()));
    assert_eq!(harness.events(), &[7]);
    assert_eq!(destroyed, 0x8000_0001);
}
