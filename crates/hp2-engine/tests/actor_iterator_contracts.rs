use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FCheckResult};
use hp2_engine::actor_iterator::{self, Hp2ActorIteratorServices};
use std::ffi::c_void;

struct Harness {
    actors: [*mut c_void; 4],
    class_match: [bool; 4],
    hidden: [bool; 4],
    line_visible: [bool; 4],
    tag_match: [bool; 4],
    zone_match: [bool; 4],
    tag_is_none: bool,
    default_class: *mut c_void,
}

fn actor_index(harness: &Harness, actor: *mut c_void) -> Option<usize> {
    harness
        .actors
        .iter()
        .position(|candidate| *candidate == actor)
}

unsafe extern "C" fn actor_count(context: *mut c_void, _actor: *mut c_void, out: *mut i32) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    unsafe { *out = harness.actors.len() as i32 };
    0
}

unsafe extern "C" fn actor_at(
    context: *mut c_void,
    _actor: *mut c_void,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    unsafe { *out = harness.actors[index as usize] };
    0
}

unsafe extern "C" fn default_actor_class(context: *mut c_void, out: *mut *mut c_void) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    unsafe { *out = harness.default_class };
    0
}

unsafe extern "C" fn actor_is_a(
    context: *mut c_void,
    candidate: *mut c_void,
    base_class: *mut c_void,
    out: *mut i32,
) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    let index = actor_index(harness, candidate);
    unsafe {
        *out = (base_class == harness.default_class
            && index.is_some_and(|i| harness.class_match[i])) as i32
    };
    0
}

unsafe extern "C" fn actor_is_hidden(
    context: *mut c_void,
    candidate: *mut c_void,
    out: *mut i32,
) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    let index = actor_index(harness, candidate);
    unsafe { *out = index.is_some_and(|i| harness.hidden[i]) as i32 };
    0
}

unsafe extern "C" fn actor_fast_line_check(
    context: *mut c_void,
    _actor: *mut c_void,
    candidate: *mut c_void,
    _trace_location: *const c_void,
    out: *mut i32,
) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    let index = actor_index(harness, candidate);
    unsafe { *out = index.is_some_and(|i| harness.line_visible[i]) as i32 };
    0
}

unsafe extern "C" fn tag_is_none(context: *mut c_void, _tag: *const c_void, out: *mut i32) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    unsafe { *out = harness.tag_is_none as i32 };
    0
}

unsafe extern "C" fn actor_tag_matches(
    context: *mut c_void,
    candidate: *mut c_void,
    _tag: *const c_void,
    out: *mut i32,
) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    let index = actor_index(harness, candidate);
    unsafe { *out = index.is_some_and(|i| harness.tag_match[i]) as i32 };
    0
}

unsafe extern "C" fn actor_is_in_zone(
    context: *mut c_void,
    candidate: *mut c_void,
    _zone: *mut c_void,
    out: *mut i32,
) -> i32 {
    let harness = unsafe { &*context.cast::<Harness>() };
    let index = actor_index(harness, candidate);
    unsafe { *out = index.is_some_and(|i| harness.zone_match[i]) as i32 };
    0
}

fn services(harness: &mut Harness) -> Hp2ActorIteratorServices {
    Hp2ActorIteratorServices {
        context: (harness as *mut Harness).cast(),
        actor_count,
        actor_at,
        default_actor_class,
        actor_is_a,
        actor_is_hidden,
        actor_fast_line_check,
        tag_is_none,
        actor_tag_matches,
        actor_is_in_zone,
    }
}

#[test]
fn all_actors_resolves_class_filters_tag_and_exhausts_cursor() {
    let mut bytes = [0_u8; 7];
    let (owner, first, second, match_actor, class, tag, _unused) = (
        &mut bytes[0] as *mut u8,
        std::ptr::null_mut::<u8>(),
        &mut bytes[1] as *mut u8,
        &mut bytes[2] as *mut u8,
        &mut bytes[3] as *mut u8,
        &mut bytes[4] as *mut u8,
        &mut bytes[5] as *mut u8,
    );
    let mut harness = Harness {
        actors: [
            first.cast(),
            second.cast(),
            match_actor.cast(),
            std::ptr::null_mut(),
        ],
        class_match: [false, false, true, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false, false, true, false],
        zone_match: [false; 4],
        tag_is_none: false,
        default_class: class.cast(),
    };
    let services = services(&mut harness);
    let mut base_class = std::ptr::null_mut();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;
    let result = unsafe {
        actor_iterator::all_actors_next(
            owner.cast(),
            &mut base_class,
            tag.cast(),
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(base_class, class.cast());
    assert_eq!(out, match_actor.cast());
    assert_eq!(index, 2);
    assert_eq!(cursor, 3);

    let result = unsafe {
        actor_iterator::all_actors_next(
            owner.cast(),
            &mut base_class,
            tag.cast(),
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert!(out.is_null());
    assert_eq!(index, -1);
    assert_eq!(cursor, 4);
}
#[test]
fn all_actors_ignores_tag_filter_for_name_none() {
    let mut bytes = [0_u8; 5];
    let (owner, candidate, class, tag) = (
        &mut bytes[0] as *mut u8,
        &mut bytes[1] as *mut u8,
        &mut bytes[2] as *mut u8,
        &mut bytes[3] as *mut u8,
    );
    let mut harness = Harness {
        actors: [
            candidate.cast(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ],
        class_match: [true, false, false, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: class.cast(),
    };
    let services = services(&mut harness);
    let mut base_class = class.cast();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;
    let result = unsafe {
        actor_iterator::all_actors_next(
            owner.cast(),
            &mut base_class,
            tag.cast(),
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(out, candidate.cast());
    assert_eq!(index, 0);
    assert_eq!(cursor, 1);
}

#[test]
fn child_actors_selects_owned_actor_and_skips_unowned_candidates() {
    let mut parent: AActor = unsafe { std::mem::zeroed() };
    let mut child: AActor = unsafe { std::mem::zeroed() };
    let mut unrelated: AActor = unsafe { std::mem::zeroed() };
    child.owner = (&mut parent as *mut AActor).cast();
    let parent_ptr = (&mut parent as *mut AActor).cast::<c_void>();
    let child_ptr = (&mut child as *mut AActor).cast::<c_void>();
    let unrelated_ptr = (&mut unrelated as *mut AActor).cast::<c_void>();
    let mut class_storage = 0_u8;
    let mut harness = Harness {
        actors: [
            unrelated_ptr,
            child_ptr,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ],
        class_match: [true, true, false, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: (&mut class_storage as *mut u8).cast(),
    };
    let services = services(&mut harness);
    let mut base_class = harness.default_class;
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;
    let result = unsafe {
        actor_iterator::child_actors_next(
            parent_ptr,
            &mut base_class,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(out, child_ptr);
    assert_eq!(index, 1);
    assert_eq!(cursor, 2);
}

#[test]
fn based_actors_uses_direct_base_equality_after_class_filtering() {
    let mut owner: AActor = unsafe { std::mem::zeroed() };
    let mut unbased: AActor = unsafe { std::mem::zeroed() };
    let mut based: AActor = unsafe { std::mem::zeroed() };
    let mut class_storage = 0_u8;
    let owner_ptr = (&mut owner as *mut AActor).cast::<c_void>();
    let unbased_ptr = (&mut unbased as *mut AActor).cast::<c_void>();
    let based_ptr = (&mut based as *mut AActor).cast::<c_void>();
    based.base = owner_ptr;
    let mut harness = Harness {
        actors: [
            unbased_ptr,
            based_ptr,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ],
        class_match: [true, true, false, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: (&mut class_storage as *mut u8).cast(),
    };
    let services = services(&mut harness);
    let mut base_class = std::ptr::null_mut();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;

    let result = unsafe {
        actor_iterator::based_actors_next(
            owner_ptr,
            &mut base_class,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(base_class, harness.default_class);
    assert_eq!(out, based_ptr);
    assert_eq!(index, 1);
    assert_eq!(cursor, 2);
}

#[test]
fn touching_actors_preserves_fixed_slot_order_class_fallback_and_exhaustion() {
    let mut owner: AActor = unsafe { std::mem::zeroed() };
    let mut rejected: AActor = unsafe { std::mem::zeroed() };
    let mut accepted: AActor = unsafe { std::mem::zeroed() };
    let mut class_storage = 0_u8;
    let rejected_ptr = (&mut rejected as *mut AActor).cast::<c_void>();
    let accepted_ptr = (&mut accepted as *mut AActor).cast::<c_void>();
    owner.touching = [
        std::ptr::null_mut(),
        rejected_ptr,
        accepted_ptr,
        std::ptr::null_mut(),
    ];
    let mut harness = Harness {
        actors: [
            rejected_ptr,
            accepted_ptr,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ],
        class_match: [false, true, false, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: (&mut class_storage as *mut u8).cast(),
    };
    let services = services(&mut harness);
    let owner_ptr = (&mut owner as *mut AActor).cast::<c_void>();
    let mut base_class = std::ptr::null_mut();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();

    let result = unsafe {
        actor_iterator::touching_actors_next(
            owner_ptr,
            &mut base_class,
            &mut cursor,
            &mut out,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(base_class, harness.default_class);
    assert_eq!(out, accepted_ptr);
    assert_eq!(cursor, 3);

    let result = unsafe {
        actor_iterator::touching_actors_next(
            owner_ptr,
            &mut base_class,
            &mut cursor,
            &mut out,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert!(out.is_null());
    assert_eq!(cursor, 4);
}

#[test]
fn radius_actors_uses_strict_distance_and_adds_collision_radius() {
    let mut boundary: AActor = unsafe { std::mem::zeroed() };
    let mut expanded_match: AActor = unsafe { std::mem::zeroed() };
    let mut class_storage = 0_u8;
    boundary.location = FVector {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    };
    expanded_match.location = FVector {
        x: 14.0,
        y: 0.0,
        z: 0.0,
    };
    expanded_match.collision_radius = 5.0;
    let boundary_ptr = (&mut boundary as *mut AActor).cast::<c_void>();
    let expanded_ptr = (&mut expanded_match as *mut AActor).cast::<c_void>();
    let mut harness = Harness {
        actors: [
            boundary_ptr,
            expanded_ptr,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ],
        class_match: [true, true, false, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: (&mut class_storage as *mut u8).cast(),
    };
    let services = services(&mut harness);
    let center = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let mut base_class = std::ptr::null_mut();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;
    let result = unsafe {
        actor_iterator::radius_actors_next(
            boundary_ptr,
            &mut base_class,
            10.0,
            &center,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(base_class, harness.default_class);
    assert_eq!(out, expanded_ptr);
    assert_eq!(index, 1);
    assert_eq!(cursor, 2);

    let result = unsafe {
        actor_iterator::radius_actors_next(
            boundary_ptr,
            &mut base_class,
            10.0,
            &center,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert!(out.is_null());
    assert_eq!(index, -1);
    assert_eq!(cursor, 4);
}

#[test]
fn visible_actors_zero_radius_skips_distance_but_keeps_hidden_and_line_checks() {
    let mut hidden: AActor = unsafe { std::mem::zeroed() };
    let mut blocked: AActor = unsafe { std::mem::zeroed() };
    let mut distant_visible: AActor = unsafe { std::mem::zeroed() };
    let mut class_storage = 0_u8;
    distant_visible.location = FVector {
        x: 1000.0,
        y: 0.0,
        z: 0.0,
    };
    let hidden_ptr = (&mut hidden as *mut AActor).cast::<c_void>();
    let blocked_ptr = (&mut blocked as *mut AActor).cast::<c_void>();
    let distant_ptr = (&mut distant_visible as *mut AActor).cast::<c_void>();
    let mut harness = Harness {
        actors: [hidden_ptr, blocked_ptr, distant_ptr, std::ptr::null_mut()],
        class_match: [true, true, true, false],
        hidden: [true, false, false, false],
        line_visible: [true, false, true, false],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: (&mut class_storage as *mut u8).cast(),
    };
    let services = services(&mut harness);
    let center = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let mut base_class = std::ptr::null_mut();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;

    let result = unsafe {
        actor_iterator::visible_actors_next(
            hidden_ptr,
            &mut base_class,
            0.0,
            &center,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(out, distant_ptr);
    assert_eq!(index, 2);
    assert_eq!(cursor, 3);

    let result = unsafe {
        actor_iterator::visible_actors_next(
            hidden_ptr,
            &mut base_class,
            0.0,
            &center,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert!(out.is_null());
    assert_eq!(cursor, 4);
}

#[test]
fn visible_actors_uses_strict_distance_limit_when_radius_is_nonzero() {
    let mut boundary: AActor = unsafe { std::mem::zeroed() };
    let mut inside: AActor = unsafe { std::mem::zeroed() };
    let mut class_storage = 0_u8;
    boundary.location = FVector {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    };
    inside.location = FVector {
        x: 9.0,
        y: 0.0,
        z: 0.0,
    };
    let boundary_ptr = (&mut boundary as *mut AActor).cast::<c_void>();
    let inside_ptr = (&mut inside as *mut AActor).cast::<c_void>();
    let mut harness = Harness {
        actors: [
            boundary_ptr,
            inside_ptr,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ],
        class_match: [true, true, false, false],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: (&mut class_storage as *mut u8).cast(),
    };
    let services = services(&mut harness);
    let center = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let mut base_class = harness.default_class;
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();
    let mut index = -1;
    let result = unsafe {
        actor_iterator::visible_actors_next(
            boundary_ptr,
            &mut base_class,
            10.0,
            &center,
            &mut cursor,
            &mut out,
            &mut index,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(out, inside_ptr);
    assert_eq!(index, 1);
    assert_eq!(cursor, 2);
}

#[test]
fn trace_actors_copies_hits_in_chain_order_and_preserves_outputs_on_exhaustion() {
    let mut first_actor = 0_u8;
    let mut second_actor = 0_u8;
    let mut second: FCheckResult = unsafe { std::mem::zeroed() };
    let mut first: FCheckResult = unsafe { std::mem::zeroed() };
    first.next = (&mut second as *mut FCheckResult).cast::<c_void>();
    first.actor = (&mut first_actor as *mut u8).cast::<c_void>();
    first.location = FVector {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    first.normal = FVector {
        x: 4.0,
        y: 5.0,
        z: 6.0,
    };
    second.actor = (&mut second_actor as *mut u8).cast::<c_void>();
    second.location = FVector {
        x: 7.0,
        y: 8.0,
        z: 9.0,
    };
    second.normal = FVector {
        x: 10.0,
        y: 11.0,
        z: 12.0,
    };

    let mut hit = (&mut first as *mut FCheckResult).cast::<c_void>();
    let mut out_actor = std::ptr::null_mut();
    let mut location = FVector {
        x: -1.0,
        y: -1.0,
        z: -1.0,
    };
    let mut normal = location;

    unsafe {
        actor_iterator::trace_actors_next(&mut hit, &mut out_actor, &mut location, &mut normal)
    }
    .unwrap();
    assert_eq!(out_actor, first.actor);
    assert_eq!(hit, first.next);
    assert_eq!((location.x, location.y, location.z), (1.0, 2.0, 3.0));
    assert_eq!((normal.x, normal.y, normal.z), (4.0, 5.0, 6.0));

    unsafe {
        actor_iterator::trace_actors_next(&mut hit, &mut out_actor, &mut location, &mut normal)
    }
    .unwrap();
    assert_eq!(out_actor, second.actor);
    assert!(hit.is_null());
    assert_eq!((location.x, location.y, location.z), (7.0, 8.0, 9.0));
    assert_eq!((normal.x, normal.y, normal.z), (10.0, 11.0, 12.0));

    unsafe {
        actor_iterator::trace_actors_next(&mut hit, &mut out_actor, &mut location, &mut normal)
    }
    .unwrap();
    assert!(out_actor.is_null());
    assert_eq!((location.x, location.y, location.z), (7.0, 8.0, 9.0));
    assert_eq!((normal.x, normal.y, normal.z), (10.0, 11.0, 12.0));
}

#[test]
fn visible_colliding_actors_filters_linked_candidates_and_preserves_ignore_hidden() {
    let mut bytes = [0_u8; 6];
    let actors = [
        (&mut bytes[0] as *mut u8).cast::<c_void>(),
        (&mut bytes[1] as *mut u8).cast::<c_void>(),
        (&mut bytes[2] as *mut u8).cast::<c_void>(),
        (&mut bytes[3] as *mut u8).cast::<c_void>(),
    ];
    let actor = (&mut bytes[4] as *mut u8).cast::<c_void>();
    let class = (&mut bytes[5] as *mut u8).cast::<c_void>();
    let mut harness = Harness {
        actors,
        class_match: [false, true, true, true],
        hidden: [false, true, false, false],
        line_visible: [true, true, false, true],
        tag_match: [false; 4],
        zone_match: [false; 4],
        tag_is_none: true,
        default_class: class,
    };
    let services = services(&mut harness);
    let mut hits: [FCheckResult; 5] = unsafe { std::mem::zeroed() };
    hits[1].actor = actors[0];
    hits[2].actor = actors[1];
    hits[3].actor = actors[2];
    hits[4].actor = actors[3];
    for index in 0..4 {
        hits[index].next = unsafe { hits.as_mut_ptr().add(index + 1).cast::<c_void>() };
    }
    let start = hits.as_mut_ptr().cast::<c_void>();
    let mut link = start;
    let mut base_class = std::ptr::null_mut();
    let mut out = std::ptr::null_mut();
    let trace_location = FVector {
        x: 4.0,
        y: 5.0,
        z: 6.0,
    };

    let result = unsafe {
        actor_iterator::visible_colliding_actors_next(
            &mut link,
            actor,
            &mut base_class,
            1,
            &trace_location,
            &mut out,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(base_class, class);
    assert_eq!(out, actors[3]);
    assert!(link.is_null());

    link = start;
    let result = unsafe {
        actor_iterator::visible_colliding_actors_next(
            &mut link,
            actor,
            &mut base_class,
            0,
            &trace_location,
            &mut out,
            &services,
        )
    };
    assert_eq!(result, Ok(()));
    assert_eq!(out, actors[1]);
    assert_eq!(link, unsafe { hits.as_mut_ptr().add(3).cast::<c_void>() });
}

#[test]
fn zone_actors_filters_class_and_zone_membership_in_actor_array_order() {
    let mut bytes = [0_u8; 5];
    let actors = [
        std::ptr::null_mut(),
        (&mut bytes[0] as *mut u8).cast::<c_void>(),
        (&mut bytes[1] as *mut u8).cast::<c_void>(),
        (&mut bytes[2] as *mut u8).cast::<c_void>(),
    ];
    let zone = (&mut bytes[3] as *mut u8).cast::<c_void>();
    let class = (&mut bytes[4] as *mut u8).cast::<c_void>();
    let mut harness = Harness {
        actors,
        class_match: [false, false, true, true],
        hidden: [false; 4],
        line_visible: [true; 4],
        tag_match: [false; 4],
        zone_match: [false, false, false, true],
        tag_is_none: true,
        default_class: class,
    };
    let services = services(&mut harness);
    let mut base_class = std::ptr::null_mut();
    let mut cursor = 0;
    let mut out = std::ptr::null_mut();

    let result = unsafe {
        actor_iterator::zone_actors_next(zone, &mut base_class, &mut cursor, &mut out, &services)
    };
    assert_eq!(result, Ok(()));
    assert_eq!(base_class, class);
    assert_eq!(out, actors[3]);
    assert_eq!(cursor, 4);

    let result = unsafe {
        actor_iterator::zone_actors_next(zone, &mut base_class, &mut cursor, &mut out, &services)
    };
    assert_eq!(result, Ok(()));
    assert!(out.is_null());
    assert_eq!(cursor, 4);
}
