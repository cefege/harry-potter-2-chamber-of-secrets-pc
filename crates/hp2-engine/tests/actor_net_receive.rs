static RECEIVE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use std::cell::Cell;
use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, APlayerReplicationInfo, ULevel};
use hp2_engine::actor::{Hp2ActorServices, class, post_net_receive, pre_net_receive};

thread_local! {
    static SMOOTH_CALLS: Cell<u32> = const { Cell::new(0) };
    static CLASS_MASK: Cell<u32> = const { Cell::new(1 << class::APAWN) };
    static DEMO_HAS_CONNECTION: Cell<i32> = const { Cell::new(0) };
    static VIEWPORT_PRI: Cell<usize> = const { Cell::new(0) };
    static AVERAGE_FRAME_TIME: Cell<f32> = const { Cell::new(0.0) };
    static FAR_MOVE_CALLS: Cell<u32> = const { Cell::new(0) };
}

unsafe extern "C" fn level_has_hash(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn hash_actor(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn actor_is_a(_: *const c_void, class_id: u32) -> i32 {
    CLASS_MASK.with(|mask| i32::from(mask.get() & (1 << class_id) != 0))
}
unsafe extern "C" fn far_move(
    _: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
    _: i32,
    _: i32,
) -> i32 {
    let dest = unsafe { ptr::read(dest.cast::<FVector>()) };
    FAR_MOVE_CALLS.with(|n| n.set(n.get() + 1));
    unsafe { (*actor.cast::<AActor>()).location = dest };
    1
}
unsafe extern "C" fn move_rotation(_: *mut c_void, _: *mut c_void, _: *const c_void) {}
unsafe extern "C" fn move_smooth(_: *mut c_void, _: *const c_void) -> i32 {
    SMOOTH_CALLS.with(|n| n.set(n.get() + 1));
    1
}
unsafe extern "C" fn set_physics(_: *mut c_void, _: u8) {}
unsafe extern "C" fn event_two(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn event_one(_: *mut c_void) {}
unsafe extern "C" fn demo_server(_: *mut c_void) -> i32 {
    DEMO_HAS_CONNECTION.with(Cell::get)
}
unsafe extern "C" fn viewport_pri(_: *mut c_void) -> *mut c_void {
    VIEWPORT_PRI.with(Cell::get) as *mut c_void
}
unsafe extern "C" fn average(_: *mut c_void) -> f32 {
    AVERAGE_FRAME_TIME.with(Cell::get)
}
unsafe extern "C" fn check_failed(_: *const c_char, _: i32) {}
unsafe extern "C" fn is_editor() -> i32 {
    0
}
unsafe extern "C" fn flush() {}
unsafe extern "C" fn engine_primitive(_: *mut c_void, _: u8) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn validate(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn process_event(_: *mut c_void, _: *mut c_void, _: *mut c_void, _: *mut c_void) {
}
unsafe extern "C" fn destroy(_: *mut c_void) {}
unsafe extern "C" fn actor_flag(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn load(_: *mut c_void) {}
unsafe extern "C" fn object_pointer(_: *mut c_void) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn player_set(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn extent(_: *mut c_void, _: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn render_box(_: *mut c_void, _: *mut c_void, _: i32, _: *mut c_void) {}
unsafe extern "C" fn class_static(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn archive_preload(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn archive_state(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn serialize(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn properties_size(_: *mut c_void) -> usize {
    0
}
unsafe extern "C" fn count_bytes(_: *mut c_void, _: usize) {}
unsafe extern "C" fn trace(
    _: *mut c_void,
    _: *const c_void,
    _: *const c_void,
    _: *mut c_void,
) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn surface_pointer(_: *mut c_void) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn surface_u32(_: *mut c_void) -> u32 {
    0
}
unsafe extern "C" fn surface_i32(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn model_item(_: *mut c_void, _: i32, _: *mut c_void) {}
unsafe extern "C" fn decal_pointer(_: *mut c_void, _: i32) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn decal_vertices(_: *mut c_void, _: *mut c_void) {}
unsafe extern "C" fn object_valid(_: *mut c_void) -> i32 {
    1
}

fn services() -> Hp2ActorServices {
    Hp2ActorServices {
        level_has_hash,
        level_hash_add_actor: hash_actor,
        level_hash_remove_actor: hash_actor,
        actor_is_a,
        level_far_move_actor: far_move,
        level_move_actor_to_rotation: move_rotation,
        actor_move_smooth: move_smooth,
        actor_set_physics: set_physics,
        actor_event_bump: event_two,
        actor_event_detach: event_two,
        actor_event_attach: event_two,
        actor_event_base_change: event_one,
        actor_event_lost_child: event_two,
        actor_event_gained_child: event_two,
        level_demo_rec_has_server_connection: demo_server,
        level_viewport0_actor_pri: viewport_pri,
        level_net_average_frame_time: average,
        check_failed,
        actor_event_touch: event_two,
        actor_event_untouch: event_two,
        is_editor,
        gcache_flush: flush,
        level_engine_primitive: engine_primitive,
        primitive_validate_actor: validate,
        level_begun_play: actor_flag,
        uobject_process_event: process_event,
        uobject_destroy: destroy,
        render_interface_destroy: destroy,
        object_conditional_destroy: destroy,
        object_set_transactional: destroy,
        actor_class_is_localized: actor_flag,
        actor_load_localized: load,
        uobject_post_load: destroy,
        model_polys: object_pointer,
        player_actor: object_pointer,
        player_set_actor: player_set,
        actor_event_possess: event_one,
        actor_log_possessed: event_one,
        mesh_collision_extent: extent,
        brush_collision_extent: extent,
        mesh_render_bounding_box: render_box,
        brush_render_bounding_box_coords: render_box,
        object_set_debug_serialize: destroy,
        object_class: object_pointer,
        class_is_uclass_static: class_static,
        archive_preload,
        archive_is_loading: archive_state,
        archive_is_saving: archive_state,
        archive_is_trans: archive_state,
        object_serialize_tagged_persistent: serialize,
        object_serialize_bin: serialize,
        object_properties_size: properties_size,
        archive_count_bytes: count_bytes,
        trace_line_texture_hit: trace,
        actor_x_level_model: object_pointer,
        bsp_surf_texture: surface_pointer,
        bsp_surf_poly_flags: surface_u32,
        texture_poly_flags: surface_u32,
        bsp_surf_decal_count: surface_i32,
        bsp_surf_v_normal: surface_i32,
        bsp_surf_p_base: surface_i32,
        model_vector: model_item,
        model_point: model_item,
        bsp_surf_decal: decal_pointer,
        decal_vertices,
        decal_actor_texture: surface_pointer,
        actor_get_primitive: object_pointer,
        object_is_valid: object_valid,
    }
}

#[test]
fn post_net_receive_uses_fused_size_squared_at_location_limit() {
    let _guard = RECEIVE_TEST_LOCK.lock().unwrap();
    let svc = services();
    let mut pawn: APlayerPawn = unsafe { std::mem::zeroed() };
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let actor = (&mut pawn.base.base) as *mut AActor;
    unsafe {
        (*actor).x_level = (&mut level as *mut ULevel).cast();
        (*actor).role = 2;
        (*actor).velocity = FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        pre_net_receive(&svc, actor);
        (*actor).location = FVector {
            x: 34.52091,
            y: 26.25897,
            z: 90.10423,
        };
        post_net_receive(&svc, actor);
    }
    let loc = unsafe { (*actor).location };
    assert_eq!(loc.x.to_bits(), 34.52091_f32.mul_add(0.5, 0.0).to_bits());
    assert_eq!(loc.y.to_bits(), 26.25897_f32.mul_add(0.5, 0.0).to_bits());
    assert_eq!(loc.z.to_bits(), 90.10423_f32.mul_add(0.5, 0.0).to_bits());
    assert_eq!(SMOOTH_CALLS.with(Cell::get), 1);
    let fixture = include_str!("fixtures/actor_unactor.txt");
    assert!(fixture.contains("case net_receive 3 pawn_fma"));
}
#[test]
fn post_net_receive_applies_demo_view_rotation() {
    let _guard = RECEIVE_TEST_LOCK.lock().unwrap();
    let svc = services();
    CLASS_MASK.with(|mask| mask.set((1 << class::APAWN) | (1 << class::APLAYERPAWN)));
    DEMO_HAS_CONNECTION.with(|has_connection| has_connection.set(1));
    let mut pawn: APlayerPawn = unsafe { std::mem::zeroed() };
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let actor = (&mut pawn.base.base) as *mut AActor;
    unsafe {
        level.demo_rec_driver = ptr::dangling_mut();
        (*actor).x_level = (&mut level as *mut ULevel).cast();
        (*actor).role = 2;
        pre_net_receive(&svc, actor);
        pawn.demo_view_pitch = 1234;
        pawn.demo_view_yaw = -5678;
        post_net_receive(&svc, actor);
    }
    assert_eq!(pawn.base.view_rotation.pitch, 1234);
    assert_eq!(pawn.base.view_rotation.yaw, -5678);
}

#[test]
fn post_net_receive_updates_client_replication_ping() {
    let _guard = RECEIVE_TEST_LOCK.lock().unwrap();
    let svc = services();
    CLASS_MASK.with(|mask| mask.set(1 << class::APLAYERREPLICATIONINFO));
    AVERAGE_FRAME_TIME.with(|average| average.set(0.011));
    let mut info: APlayerReplicationInfo = unsafe { std::mem::zeroed() };
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let mut level_info: ALevelInfo = unsafe { std::mem::zeroed() };
    let actor = (&mut info.base.base.base) as *mut AActor;
    unsafe {
        level.net_driver = ptr::dangling_mut();
        level_info.net_mode = 3;
        (*actor).x_level = (&mut level as *mut ULevel).cast();
        (*actor).level = (&mut level_info as *mut ALevelInfo).cast();
        info.ping = 10;
    }
    VIEWPORT_PRI.with(|pri| pri.set(actor as usize));
    unsafe { post_net_receive(&svc, actor) };
    assert_eq!(info.ping, 5);
    VIEWPORT_PRI.with(|pri| pri.set(0));
}
#[test]
fn post_net_receive_uses_far_move_at_or_above_10000_without_smoothing() {
    let _guard = RECEIVE_TEST_LOCK.lock().unwrap();
    let svc = services();
    CLASS_MASK.with(|mask| mask.set(1 << class::APAWN));
    FAR_MOVE_CALLS.with(|n| n.set(0));
    let mut pawn: APlayerPawn = unsafe { std::mem::zeroed() };
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let actor = (&mut pawn.base.base) as *mut AActor;
    unsafe {
        (*actor).x_level = (&mut level as *mut ULevel).cast();
        (*actor).role = 2;
        (*actor).velocity.x = 1.0;
        pre_net_receive(&svc, actor);
        (*actor).location.x = 101.0;
        post_net_receive(&svc, actor);
    }
    assert_eq!(unsafe { (*actor).location.x }, 101.0);
    assert_eq!(FAR_MOVE_CALLS.with(Cell::get), 1);
    assert_eq!(SMOOTH_CALLS.with(Cell::get), 0);
}

#[test]
fn post_net_receive_skips_demo_view_without_driver_or_server_connection() {
    let _guard = RECEIVE_TEST_LOCK.lock().unwrap();
    let svc = services();
    CLASS_MASK.with(|mask| mask.set((1 << class::APAWN) | (1 << class::APLAYERPAWN)));
    for (driver_present, server_connection) in [(false, true), (true, false)] {
        DEMO_HAS_CONNECTION.with(|has_connection| has_connection.set(i32::from(server_connection)));
        let mut pawn: APlayerPawn = unsafe { std::mem::zeroed() };
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        let actor = (&mut pawn.base.base) as *mut AActor;
        unsafe {
            if driver_present {
                level.demo_rec_driver = ptr::dangling_mut();
            }
            (*actor).x_level = (&mut level as *mut ULevel).cast();
            (*actor).role = 2;
            pre_net_receive(&svc, actor);
            pawn.demo_view_pitch = 1234;
            pawn.demo_view_yaw = -5678;
            post_net_receive(&svc, actor);
        }
        assert_eq!(pawn.base.view_rotation.pitch, 0);
        assert_eq!(pawn.base.view_rotation.yaw, 0);
    }
}

#[test]
fn fixture_traces_cover_receive_branch_families() {
    let fixture = include_str!("fixtures/actor_unactor.txt");
    let record = |index: usize| {
        let marker = format!("case net_receive {index} ");
        fixture
            .split_once(&marker)
            .unwrap_or_else(|| panic!("missing {marker}"))
            .1
            .lines()
            .take_while(|line| !line.starts_with("case "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let c0 = record(0);
    assert!(c0.contains("hook isa self APawn 1"));
    assert!(c0.contains("loop=1 min=3f000000"));
    let c1 = record(1);
    assert!(c1.contains("hook move_smooth self"));
    assert!(c1.contains("hook far_move level"));
    let c2 = record(2);
    assert!(c2.contains("hook set_physics self 9"));
    assert!(c2.contains("key=1,2"));
    let c3 = record(3);
    assert!(c3.contains("hook move_smooth self"));
    assert!(c3.contains("dest=418a1569 =4152125f =4234355e"));
    let c4 = record(4);
    assert!(c4.contains("hook move_actor level self"));
    assert!(c4.contains("hook event self Bump base"));
    assert!(c4.contains("hook event self BaseChange null"));
    assert!(record(5).contains("viewrot=1234,-5678"));
    assert!(record(6).contains("hook far_move level  dest=42c80000"));
    assert!(record(7).contains("ping=5"));
    assert!(record(8).contains("hook far_move level  dest=42ca0000"));
    for (index, case_name) in [
        (9, "demo_driver_absent"),
        (10, "demo_server_connection_absent"),
    ] {
        let case = record(index);
        assert!(fixture.contains(&format!("case net_receive {index} {case_name}")));
        assert!(case.contains("hook isa self APlayerPawn 1"));
        assert!(case.contains("viewrot=0,0"));
    }
}
