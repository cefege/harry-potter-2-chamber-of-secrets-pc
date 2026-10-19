//! Record-level replay of the UnActor.cpp actor slice.
//!
//! `fixtures/actor_unactor.txt` is generated from the oracle
//! (`git show 90d1e8d:HarryPotter2/Unreal/Engine/Src/UnActor.cpp`) by
//! `Build/actor_fixture_harness/gen_actor.sh`. Each case rebuilds the object
//! graph the harness builds, drives the Rust entry points, and records every
//! hook the services table sees plus the result lines. The whole log is then
//! compared line by line against the fixture, with exact per-family counts.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, AMover, APlayerPawn, APlayerReplicationInfo, FRotator, ULevel,
};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_engine::actor::{self, Hp2ActorServices, class};

const FIXTURE: &str = include_str!("fixtures/actor_unactor.txt");

/// Large enough for the biggest generated actor type (APlayerPawn and friends).
const SLOT: usize = 32 * 1024;

/// Oracle `ENetRole::ROLE_SimulatedProxy` (EngineClasses.h:563).
const ROLE_SIMULATED_PROXY: u8 = 2;
/// Oracle `NM_Client`.
const NM_CLIENT: u8 = 3;

#[repr(C, align(16))]
struct Slot([u8; SLOT]);

/// Owns every zeroed object for one case.
#[derive(Default)]
struct Rig {
    mem: Vec<Box<Slot>>,
}

impl Rig {
    fn obj(&mut self, name: &'static str, isa: &'static str) -> usize {
        let mut slot = Box::new(Slot([0; SLOT]));
        let p = slot.0.as_mut_ptr() as usize;
        self.mem.push(slot);
        reg(p, name, isa);
        p
    }
}

#[derive(Default)]
struct St {
    ents: Vec<(usize, &'static str, &'static str)>,
    editor: i32,
    level_begun: i32,
    localized: i32,
    polys: usize,
    player_actor: usize,
    /// Engine primitives: cylinder, oval cylinder, box, oval, oval-oval.
    prims: [usize; 5],
    avg_frame: f32,
    viewport_pri: usize,
    demo_conn: i32,
    class_ptr: usize,
    uclass_static: usize,
    ar_loading: i32,
    ar_saving: i32,
    ar_trans: i32,
    smooth_noop: bool,
    trace_kind: i32,
    surf: usize,
    surf_texture: usize,
    surf_flags: u32,
    tex_flags: u32,
    decal_count: i32,
    decal_ptr: usize,
    decal_tex: usize,
    decal_verts: [[f32; 3]; 4],
    model_vec: [f32; 3],
    model_pt: [f32; 3],
    hit_loc: [f32; 3],
}

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static ST: RefCell<St> = RefCell::new(St::default());
}

fn emit(line: impl Into<String>) {
    LOG.with(|l| l.borrow_mut().push(line.into()));
}

fn with_st<R>(f: impl FnOnce(&mut St) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}

fn reset() {
    with_st(|s| *s = St::default());
}

fn reg(p: usize, name: &'static str, isa: &'static str) {
    with_st(|s| match s.ents.iter().position(|e| e.0 == p) {
        Some(i) => {
            s.ents[i].1 = name;
            s.ents[i].2 = isa;
        }
        None => s.ents.push((p, name, isa)),
    });
}

/// Harness `nm()`: null prints `null`, unregistered prints `?`.
fn nm(p: usize) -> String {
    if p == 0 {
        return "null".to_owned();
    }
    with_st(|s| {
        s.ents
            .iter()
            .find(|e| e.0 == p)
            .map_or("?", |e| e.1)
            .to_owned()
    })
}

/// Harness `isa_of`: substring check of ` cls ` inside ` isa `.
fn isa_has(p: usize, cls: &str) -> i32 {
    with_st(|s| {
        let isa = s.ents.iter().find(|e| e.0 == p).map_or("", |e| e.2);
        i32::from(format!(" {isa} ").contains(&format!(" {cls} ")))
    })
}

/// Harness `bits()`: ` label=%08x`.
fn bits(lbl: &str, f: f32) -> String {
    format!(" {lbl}={:08x}", f.to_bits())
}

/// Harness `vec()`: ` lbl=X =Y =Z`.
fn vec3(lbl: &str, v: [f32; 3]) -> String {
    format!("{}{}{}", bits(lbl, v[0]), bits("", v[1]), bits("", v[2]))
}

fn arr(v: &FVector) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn fv3(a: [f32; 3]) -> FVector {
    FVector {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}

fn zvec() -> FVector {
    fv3([0.0; 3])
}

fn zcoords() -> FCoords {
    FCoords {
        origin: zvec(),
        x_axis: zvec(),
        y_axis: zvec(),
        z_axis: zvec(),
    }
}

fn case(fam: &str, idx: usize, knobs: &str) {
    emit(format!("case {fam} {idx} {knobs}"));
}

fn class_name(id: u32) -> &'static str {
    match id {
        class::AMOVER => "AMover",
        class::APAWN => "APawn",
        class::APLAYERPAWN => "APlayerPawn",
        class::APLAYERREPLICATIONINFO => "APlayerReplicationInfo",
        class::APROJECTILE => "AProjectile",
        class::ATRIGGER => "ATrigger",
        class::ABRUSH => "ABrush",
        _ => "?",
    }
}

/// Engine primitive slot (collide type) to `prims` index.
fn prim_index(slot: u8) -> usize {
    match slot {
        1 => 1,
        2 => 2,
        4 => 3,
        5 => 4,
        _ => 0,
    }
}

// ---- Services stubs: print hook lines in harness format. -------------------

fn event(obj: *mut c_void, name: &str, arg: *mut c_void) {
    emit(format!(
        "hook event {} {name} {}",
        nm(obj as usize),
        nm(arg as usize)
    ));
}

unsafe extern "C" fn s_level_has_hash(level: *mut c_void) -> i32 {
    unsafe { i32::from(!(*level.cast::<ULevel>()).hash.is_null()) }
}

unsafe extern "C" fn s_level_hash_add_actor(_level: *mut c_void, actor: *mut c_void) {
    emit(format!("hook hash {} AddActor", nm(actor as usize)));
}

unsafe extern "C" fn s_level_hash_remove_actor(_level: *mut c_void, actor: *mut c_void) {
    emit(format!("hook hash {} RemoveActor", nm(actor as usize)));
}

unsafe extern "C" fn s_actor_is_a(obj: *const c_void, class_id: u32) -> i32 {
    let p = obj as usize;
    if class_id == class::ABRUSH {
        // Oracle IsBrush reads Brush first; with no brush the hook never fires.
        let brush = unsafe { (*obj.cast::<AActor>()).brush };
        if brush.is_null() {
            return 0;
        }
        let r = isa_has(p, "ABrush");
        emit(format!("hook is_brush {} {r}", nm(p)));
        return r;
    }
    let cls = class_name(class_id);
    let r = isa_has(p, cls);
    emit(format!("hook isa {} {cls} {r}", nm(p)));
    r
}

unsafe extern "C" fn s_level_far_move_actor(
    level: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
    test: i32,
    no_check: i32,
) -> i32 {
    let d = unsafe { ptr::read(dest.cast::<FVector>()) };
    emit(format!(
        "hook far_move {} {} test={test} no_check={no_check}",
        nm(level as usize),
        vec3("dest", arr(&d))
    ));
    unsafe { (*actor.cast::<AActor>()).location = d };
    1
}

unsafe extern "C" fn s_level_move_actor_to_rotation(
    level: *mut c_void,
    actor: *mut c_void,
    rot: *const c_void,
) {
    let r = unsafe { ptr::read(rot.cast::<FRotator>()) };
    emit(format!(
        "hook move_actor {} {}{} rot={},{},{} flags=0,0,0,1",
        nm(level as usize),
        nm(actor as usize),
        vec3("delta", [0.0; 3]),
        r.pitch,
        r.yaw,
        r.roll
    ));
    unsafe { (*actor.cast::<AActor>()).rotation = r };
}

unsafe extern "C" fn s_actor_move_smooth(actor: *mut c_void, delta: *const c_void) -> i32 {
    let d = unsafe { ptr::read(delta.cast::<FVector>()) };
    emit(format!(
        "hook move_smooth {}{}",
        nm(actor as usize),
        vec3("delta", arr(&d))
    ));
    if !with_st(|s| s.smooth_noop) {
        let a = unsafe { &mut *actor.cast::<AActor>() };
        a.location = fv3([a.location.x + d.x, a.location.y + d.y, a.location.z + d.z]);
    }
    1
}

unsafe extern "C" fn s_actor_set_physics(actor: *mut c_void, phys: u8) {
    emit(format!("hook set_physics {} {phys}", nm(actor as usize)));
    unsafe { (*actor.cast::<AActor>()).physics = phys };
}

unsafe extern "C" fn s_actor_event_bump(actor: *mut c_void, other: *mut c_void) {
    event(actor, "Bump", other);
}

unsafe extern "C" fn s_actor_event_detach(base: *mut c_void, actor: *mut c_void) {
    event(base, "Detach", actor);
}

unsafe extern "C" fn s_actor_event_attach(base: *mut c_void, actor: *mut c_void) {
    event(base, "Attach", actor);
}

unsafe extern "C" fn s_actor_event_base_change(actor: *mut c_void) {
    event(actor, "BaseChange", ptr::null_mut());
}

unsafe extern "C" fn s_actor_event_lost_child(owner: *mut c_void, actor: *mut c_void) {
    event(owner, "LostChild", actor);
}

unsafe extern "C" fn s_actor_event_gained_child(owner: *mut c_void, actor: *mut c_void) {
    event(owner, "GainedChild", actor);
}

unsafe extern "C" fn s_level_demo_rec_has_server_connection(_level: *mut c_void) -> i32 {
    with_st(|s| s.demo_conn)
}

unsafe extern "C" fn s_level_viewport0_actor_pri(_level: *mut c_void) -> *mut c_void {
    with_st(|s| s.viewport_pri) as *mut c_void
}

unsafe extern "C" fn s_level_net_average_frame_time(_level: *mut c_void) -> f32 {
    with_st(|s| s.avg_frame)
}

unsafe extern "C" fn s_check_failed(expr: *const c_char, line: i32) {
    let e = unsafe { CStr::from_ptr(expr) }
        .to_string_lossy()
        .into_owned();
    emit(format!("hook check_failed {e} {line}"));
}

unsafe extern "C" fn s_actor_event_touch(actor: *mut c_void, other: *mut c_void) {
    event(actor, "Touch", other);
}

unsafe extern "C" fn s_actor_event_untouch(actor: *mut c_void, other: *mut c_void) {
    event(actor, "UnTouch", other);
}

unsafe extern "C" fn s_is_editor() -> i32 {
    with_st(|s| s.editor)
}

unsafe extern "C" fn s_gcache_flush() {
    emit("hook cache_flush");
}

unsafe extern "C" fn s_level_engine_primitive(_level: *mut c_void, slot: u8) -> *mut c_void {
    let idx = prim_index(slot);
    with_st(|s| s.prims[idx]) as *mut c_void
}

unsafe extern "C" fn s_primitive_validate_actor(prim: *mut c_void, actor: *mut c_void) {
    emit(format!(
        "hook validate {} {}",
        nm(prim as usize),
        nm(actor as usize)
    ));
}

unsafe extern "C" fn s_level_begun_play(_level_info: *mut c_void) -> i32 {
    with_st(|s| s.level_begun)
}

unsafe extern "C" fn s_uobject_process_event(
    obj: *mut c_void,
    func: *mut c_void,
    parms: *mut c_void,
    result: *mut c_void,
) {
    emit(format!(
        "hook super_process_event {} {} {} {}",
        nm(obj as usize),
        nm(func as usize),
        nm(parms as usize),
        nm(result as usize)
    ));
}

unsafe extern "C" fn s_uobject_destroy(obj: *mut c_void) {
    emit(format!("hook uobject_destroy {}", nm(obj as usize)));
}

unsafe extern "C" fn s_render_interface_destroy(ri: *mut c_void) {
    let n = nm(ri as usize);
    emit(format!("hook remove_from_root {n}"));
    emit(format!("hook cond_destroy {n}"));
    emit(format!("hook delete_ri {n}"));
}

unsafe extern "C" fn s_object_conditional_destroy(obj: *mut c_void) {
    emit(format!("hook cond_destroy {}", nm(obj as usize)));
}

unsafe extern "C" fn s_object_set_transactional(obj: *mut c_void) {
    emit(format!("hook set_flags {} 1", nm(obj as usize)));
}

unsafe extern "C" fn s_actor_class_is_localized(_actor: *mut c_void) -> i32 {
    with_st(|s| s.localized)
}

unsafe extern "C" fn s_actor_load_localized(actor: *mut c_void) {
    emit(format!("hook load_localized {}", nm(actor as usize)));
}

unsafe extern "C" fn s_uobject_post_load(obj: *mut c_void) {
    emit(format!("hook uobject_postload {}", nm(obj as usize)));
}

unsafe extern "C" fn s_model_polys(_model: *mut c_void) -> *mut c_void {
    with_st(|s| s.polys) as *mut c_void
}

unsafe extern "C" fn s_player_actor(_player: *mut c_void) -> *mut c_void {
    with_st(|s| s.player_actor) as *mut c_void
}

unsafe extern "C" fn s_player_set_actor(_player: *mut c_void, actor: *mut c_void) {
    with_st(|s| s.player_actor = actor as usize);
}

unsafe extern "C" fn s_actor_event_possess(pawn: *mut c_void) {
    event(pawn, "Possess", ptr::null_mut());
}

unsafe extern "C" fn s_actor_log_possessed(pawn: *mut c_void) {
    emit(format!("hook possessed {}", nm(pawn as usize)));
}

unsafe extern "C" fn s_mesh_collision_extent(
    mesh: *mut c_void,
    actor: *mut c_void,
    out: *mut c_void,
) {
    emit(format!(
        "hook collision_extent {} {}",
        nm(mesh as usize),
        nm(actor as usize)
    ));
    unsafe { ptr::write(out.cast::<FVector>(), fv3([16.0, 24.0, 32.0])) };
}

unsafe extern "C" fn s_brush_collision_extent(
    brush: *mut c_void,
    actor: *mut c_void,
    out: *mut c_void,
) {
    emit(format!(
        "hook collision_extent {} {}",
        nm(brush as usize),
        nm(actor as usize)
    ));
    unsafe { ptr::write(out.cast::<FVector>(), fv3([16.0, 24.0, 32.0])) };
}

unsafe extern "C" fn s_mesh_render_bounding_box(
    mesh: *mut c_void,
    actor: *mut c_void,
    exact: i32,
    out: *mut c_void,
) {
    emit(format!(
        "hook mesh_render_box {} {} exact={exact}",
        nm(mesh as usize),
        nm(actor as usize)
    ));
    unsafe { ptr::write(out.cast::<FCoords>(), zcoords()) };
}

unsafe extern "C" fn s_brush_render_bounding_box_coords(
    brush: *mut c_void,
    actor: *mut c_void,
    exact: i32,
    out: *mut c_void,
) {
    emit(format!(
        "hook brush_render_box {} {} exact={exact}",
        nm(brush as usize),
        nm(actor as usize)
    ));
    unsafe { ptr::write(out.cast::<FCoords>(), zcoords()) };
}

unsafe extern "C" fn s_object_set_debug_serialize(obj: *mut c_void) {
    emit(format!("hook set_flags {} 1073741824", nm(obj as usize)));
}

unsafe extern "C" fn s_object_class(_obj: *mut c_void) -> *mut c_void {
    with_st(|s| s.class_ptr) as *mut c_void
}

unsafe extern "C" fn s_class_is_uclass_static(cls: *mut c_void) -> i32 {
    emit("hook uclass_static");
    i32::from(with_st(|s| s.uclass_static) == cls as usize)
}

unsafe extern "C" fn s_archive_preload(ar: *mut c_void, obj: *mut c_void) {
    emit(format!(
        "hook preload {} {}",
        nm(ar as usize),
        nm(obj as usize)
    ));
}

unsafe extern "C" fn s_archive_is_loading(ar: *mut c_void) -> i32 {
    emit(format!("hook is_loading {}", nm(ar as usize)));
    with_st(|s| s.ar_loading)
}

unsafe extern "C" fn s_archive_is_saving(ar: *mut c_void) -> i32 {
    emit(format!("hook is_saving {}", nm(ar as usize)));
    with_st(|s| s.ar_saving)
}

unsafe extern "C" fn s_archive_is_trans(ar: *mut c_void) -> i32 {
    emit(format!("hook is_trans {}", nm(ar as usize)));
    with_st(|s| s.ar_trans)
}

unsafe extern "C" fn s_object_serialize_tagged_persistent(obj: *mut c_void, ar: *mut c_void) {
    let cls = with_st(|s| s.class_ptr);
    emit(format!(
        "hook serialize_tagged {} {} {}",
        nm(cls),
        nm(ar as usize),
        nm(obj as usize)
    ));
}

unsafe extern "C" fn s_object_serialize_bin(obj: *mut c_void, ar: *mut c_void) {
    let cls = with_st(|s| s.class_ptr);
    emit(format!(
        "hook serialize_bin {} {} {}",
        nm(cls),
        nm(ar as usize),
        nm(obj as usize)
    ));
}

unsafe extern "C" fn s_object_properties_size(_obj: *mut c_void) -> usize {
    let cls = with_st(|s| s.class_ptr);
    emit(format!("hook properties_size {}", nm(cls)));
    96
}

unsafe extern "C" fn s_archive_count_bytes(ar: *mut c_void, size: usize) {
    emit(format!("hook count_bytes {} {size}", nm(ar as usize)));
}

unsafe extern "C" fn s_trace_line_texture_hit(
    actor: *mut c_void,
    _trace_end: *const c_void,
    _trace_start: *const c_void,
    out_location: *mut c_void,
) -> *mut c_void {
    let lvl = unsafe { (*actor.cast::<AActor>()).x_level } as usize;
    emit(format!(
        "hook single_line_check {} {} flags=4",
        nm(lvl),
        nm(actor as usize)
    ));
    let (kind, hit, surf) = with_st(|s| (s.trace_kind, s.hit_loc, s.surf));
    unsafe { ptr::write(out_location.cast::<FVector>(), fv3(hit)) };
    emit(format!("hook bsp_surf kind={kind}"));
    if kind != 0 {
        surf as *mut c_void
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn s_actor_x_level_model(actor: *mut c_void) -> *mut c_void {
    unsafe {
        let level = (*actor.cast::<AActor>()).x_level.cast::<ULevel>();
        (*level).model
    }
}

unsafe extern "C" fn s_bsp_surf_texture(_surf: *mut c_void) -> *mut c_void {
    with_st(|s| s.surf_texture) as *mut c_void
}

unsafe extern "C" fn s_bsp_surf_poly_flags(_surf: *mut c_void) -> u32 {
    with_st(|s| s.surf_flags)
}

unsafe extern "C" fn s_texture_poly_flags(_texture: *mut c_void) -> u32 {
    with_st(|s| s.tex_flags)
}

unsafe extern "C" fn s_bsp_surf_decal_count(_surf: *mut c_void) -> i32 {
    with_st(|s| s.decal_count)
}

unsafe extern "C" fn s_bsp_surf_v_normal(_surf: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn s_bsp_surf_p_base(_surf: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn s_model_vector(_model: *mut c_void, _index: i32, out: *mut c_void) {
    let v = with_st(|s| s.model_vec);
    unsafe { ptr::write(out.cast::<FVector>(), fv3(v)) };
}

unsafe extern "C" fn s_model_point(_model: *mut c_void, _index: i32, out: *mut c_void) {
    let v = with_st(|s| s.model_pt);
    unsafe { ptr::write(out.cast::<FVector>(), fv3(v)) };
}

unsafe extern "C" fn s_bsp_surf_decal(_surf: *mut c_void, _index: i32) -> *mut c_void {
    with_st(|s| s.decal_ptr) as *mut c_void
}

unsafe extern "C" fn s_decal_vertices(_decal: *mut c_void, out: *mut c_void) {
    let vs = with_st(|s| s.decal_verts);
    for (k, v) in vs.iter().enumerate() {
        unsafe { ptr::write(out.cast::<FVector>().add(k), fv3(*v)) };
    }
}

unsafe extern "C" fn s_decal_actor_texture(_decal: *mut c_void) -> *mut c_void {
    with_st(|s| s.decal_tex) as *mut c_void
}

/// Oracle `AActor::GetPrimitive`: a shape uses its mesh, then its brush;
/// every other collide type maps to an engine primitive.
unsafe extern "C" fn s_actor_get_primitive(actor: *mut c_void) -> *mut c_void {
    let a = actor.cast::<AActor>();
    let (ct, mesh, brush) = unsafe { ((*a).collide_type, (*a).mesh, (*a).brush) };
    if ct == actor::CT_SHAPE {
        if !mesh.is_null() {
            return mesh;
        }
        if !brush.is_null() {
            return brush;
        }
    }
    let idx = prim_index(ct);
    with_st(|s| s.prims[idx]) as *mut c_void
}

unsafe extern "C" fn s_object_is_valid(obj: *mut c_void) -> i32 {
    emit(format!("hook is_valid {}", nm(obj as usize)));
    i32::from(!obj.is_null())
}

fn services() -> Hp2ActorServices {
    Hp2ActorServices {
        level_has_hash: s_level_has_hash,
        level_hash_add_actor: s_level_hash_add_actor,
        level_hash_remove_actor: s_level_hash_remove_actor,
        actor_is_a: s_actor_is_a,
        level_far_move_actor: s_level_far_move_actor,
        level_move_actor_to_rotation: s_level_move_actor_to_rotation,
        actor_move_smooth: s_actor_move_smooth,
        actor_set_physics: s_actor_set_physics,
        actor_event_bump: s_actor_event_bump,
        actor_event_detach: s_actor_event_detach,
        actor_event_attach: s_actor_event_attach,
        actor_event_base_change: s_actor_event_base_change,
        actor_event_lost_child: s_actor_event_lost_child,
        actor_event_gained_child: s_actor_event_gained_child,
        level_demo_rec_has_server_connection: s_level_demo_rec_has_server_connection,
        level_viewport0_actor_pri: s_level_viewport0_actor_pri,
        level_net_average_frame_time: s_level_net_average_frame_time,
        check_failed: s_check_failed,
        actor_event_touch: s_actor_event_touch,
        actor_event_untouch: s_actor_event_untouch,
        is_editor: s_is_editor,
        gcache_flush: s_gcache_flush,
        level_engine_primitive: s_level_engine_primitive,
        primitive_validate_actor: s_primitive_validate_actor,
        level_begun_play: s_level_begun_play,
        uobject_process_event: s_uobject_process_event,
        uobject_destroy: s_uobject_destroy,
        render_interface_destroy: s_render_interface_destroy,
        object_conditional_destroy: s_object_conditional_destroy,
        object_set_transactional: s_object_set_transactional,
        actor_class_is_localized: s_actor_class_is_localized,
        actor_load_localized: s_actor_load_localized,
        uobject_post_load: s_uobject_post_load,
        model_polys: s_model_polys,
        player_actor: s_player_actor,
        player_set_actor: s_player_set_actor,
        actor_event_possess: s_actor_event_possess,
        actor_log_possessed: s_actor_log_possessed,
        mesh_collision_extent: s_mesh_collision_extent,
        brush_collision_extent: s_brush_collision_extent,
        mesh_render_bounding_box: s_mesh_render_bounding_box,
        brush_render_bounding_box_coords: s_brush_render_bounding_box_coords,
        object_set_debug_serialize: s_object_set_debug_serialize,
        object_class: s_object_class,
        class_is_uclass_static: s_class_is_uclass_static,
        archive_preload: s_archive_preload,
        archive_is_loading: s_archive_is_loading,
        archive_is_saving: s_archive_is_saving,
        archive_is_trans: s_archive_is_trans,
        object_serialize_tagged_persistent: s_object_serialize_tagged_persistent,
        object_serialize_bin: s_object_serialize_bin,
        object_properties_size: s_object_properties_size,
        archive_count_bytes: s_archive_count_bytes,
        trace_line_texture_hit: s_trace_line_texture_hit,
        actor_x_level_model: s_actor_x_level_model,
        bsp_surf_texture: s_bsp_surf_texture,
        bsp_surf_poly_flags: s_bsp_surf_poly_flags,
        texture_poly_flags: s_texture_poly_flags,
        bsp_surf_decal_count: s_bsp_surf_decal_count,
        bsp_surf_v_normal: s_bsp_surf_v_normal,
        bsp_surf_p_base: s_bsp_surf_p_base,
        model_vector: s_model_vector,
        model_point: s_model_point,
        bsp_surf_decal: s_bsp_surf_decal,
        decal_vertices: s_decal_vertices,
        decal_actor_texture: s_decal_actor_texture,
        actor_get_primitive: s_actor_get_primitive,
        object_is_valid: s_object_is_valid,
    }
}

// ---- Object graph (harness zero_all + world). ------------------------------

#[derive(Clone, Copy)]
struct W {
    self_: usize,
    other: usize,
    owner: usize,
    base: usize,
    touch: usize,
    zone: usize,
    carcass: usize,
    pawn: usize,
    level: usize,
    li: usize,
    engine: usize,
    class: usize,
    uclass_static: usize,
    brush: usize,
    polys: usize,
    mesh: usize,
    player: usize,
    ri: usize,
    ar: usize,
    surf: usize,
    hash: usize,
    func: usize,
    parms: usize,
    driver: usize,
    prims: [usize; 5],
    tex: [usize; 8],
}

/// Fresh zeroed world for one case. Harness `zero_all(); world(hash)`.
fn fresh(rig: &mut Rig, hash: bool) -> W {
    reset();
    let w = W {
        self_: rig.obj("self", " AActor "),
        other: rig.obj("other", " AActor "),
        owner: rig.obj("owner", " AActor "),
        base: rig.obj("base", " AActor "),
        touch: rig.obj("touch", " AActor "),
        zone: rig.obj("zone", " AActor AZoneInfo "),
        carcass: rig.obj("carcass", " AActor ACarcass "),
        pawn: rig.obj("pawn", " AActor APlayerPawn APawn "),
        level: rig.obj("level", ""),
        li: rig.obj("levelinfo", ""),
        engine: rig.obj("engine", ""),
        class: rig.obj("class", ""),
        uclass_static: rig.obj("uclass_static", ""),
        brush: rig.obj("brush_model", ""),
        polys: rig.obj("polys", ""),
        mesh: rig.obj("mesh", ""),
        player: rig.obj("player", ""),
        ri: rig.obj("render_iface", ""),
        ar: rig.obj("ar", ""),
        surf: rig.obj("surf", ""),
        hash: rig.obj("hash", ""),
        func: rig.obj("func", ""),
        parms: rig.obj("parms", ""),
        driver: rig.obj("driver", ""),
        prims: [
            rig.obj("prim_cyl", ""),
            rig.obj("prim_ocyl", ""),
            rig.obj("prim_box", ""),
            rig.obj("prim_oval", ""),
            rig.obj("prim_ooval", ""),
        ],
        tex: [
            rig.obj("tex0", ""),
            rig.obj("tex1", ""),
            rig.obj("tex2", ""),
            rig.obj("tex3", ""),
            rig.obj("tex4", ""),
            rig.obj("tex5", ""),
            rig.obj("tex6", ""),
            rig.obj("tex7", ""),
        ],
    };
    unsafe {
        let l = w.level as *mut ULevel;
        (*l).engine = w.engine as _;
        (*l).hash = if hash { w.hash as _ } else { ptr::null_mut() };
        for a in [
            w.self_, w.other, w.owner, w.base, w.touch, w.zone, w.carcass, w.pawn,
        ] {
            let ac = a as *mut AActor;
            (*ac).x_level = w.level as _;
            (*ac).level = w.li as _;
        }
    }
    with_st(|s| {
        s.prims = w.prims;
        s.class_ptr = w.class;
        s.uclass_static = w.uclass_static;
    });
    w
}

// ---- Families (harness fam_* order). ---------------------------------------

fn drive_get_skin() {
    for fill in 0..2usize {
        for index in 0..8usize {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let a = w.self_ as *mut AActor;
            unsafe {
                if fill == 1 {
                    let mut skins = (*a).multi_skins;
                    for (dst, src) in skins.iter_mut().zip(w.tex) {
                        *dst = src as _;
                    }
                    (*a).multi_skins = skins;
                }
                case(
                    "get_skin",
                    fill * 8 + index,
                    &format!("fill={fill} index={index}"),
                );
                let r = actor::get_skin(a, index as i32);
                emit(format!("res skin={}", nm(r as usize)));
            }
        }
    }
}

fn drive_get_net_priority() {
    let mut idx = 0;
    for ar in 0..2u32 {
        for &time in &[0.0f32, 0.5, 2.0] {
            for &nufs in &[0u32, 5, 20] {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, false);
                let a = w.self_ as *mut AActor;
                unsafe {
                    (*a).set_b_always_relevant(ar == 1);
                    (*a).net_priority = 1.5;
                    (*a).net_update_frequency = nufs as f32;
                    case(
                        "get_net_priority",
                        idx,
                        &format!("always_relevant={ar} time={time} net_update_freq={nufs}"),
                    );
                    let r = actor::get_net_priority(a, time);
                    emit(bits("res_priority", r));
                }
                idx += 1;
            }
        }
    }
}

fn drive_set_collision(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for old in 0..2u32 {
        for hash in 0..2u32 {
            for nc in ["000", "111", "101"] {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, hash == 1);
                let a = w.self_ as *mut AActor;
                let bs = nc.as_bytes();
                unsafe {
                    (*a).set_b_collide_actors(old == 1);
                    (*a).set_b_block_actors(old == 1);
                    case(
                        "set_collision",
                        idx,
                        &format!("old_collide={old} hash={hash} new={nc}"),
                    );
                    actor::set_collision(svc, a, bs[0] == b'1', bs[1] == b'1', bs[2] == b'1');
                    emit(format!(
                        "res collide={} block={} players={}",
                        (*a).b_collide_actors() as i32,
                        (*a).b_block_actors() as i32,
                        (*a).b_block_players() as i32
                    ));
                }
                idx += 1;
            }
        }
    }
}

fn drive_set_collision_size(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for old in 0..2u32 {
        for hash in 0..2u32 {
            for (r, h) in [(20, 40), (0, 0), (-1, 5)] {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, hash == 1);
                let a = w.self_ as *mut AActor;
                unsafe {
                    (*a).set_b_collide_actors(old == 1);
                    case(
                        "set_collision_size",
                        idx,
                        &format!("old_collide={old} hash={hash} radius={r} height={h}"),
                    );
                    actor::set_collision_size(svc, a, r as f32, h as f32);
                    emit(format!(
                        "{}{} collide={}",
                        bits("res_radius", (*a).collision_radius),
                        bits("height", (*a).collision_height),
                        (*a).b_collide_actors() as i32
                    ));
                }
                idx += 1;
            }
        }
    }
}

fn drive_set_owner(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for old in 0..2u32 {
        for new in 0..2u32 {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let a = w.self_ as *mut AActor;
            unsafe {
                (*a).owner = if old == 1 {
                    w.owner as _
                } else {
                    ptr::null_mut()
                };
                case(
                    "set_owner",
                    idx,
                    &format!("old_owner={old} new_owner={new}"),
                );
                let target = if new == 1 { w.owner } else { 0 };
                actor::set_owner(svc, a, target as _);
                emit(format!("res owner={}", nm((*a).owner as usize)));
            }
            idx += 1;
        }
    }
}

fn drive_set_base(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for old in 0..2u32 {
        for nw in 0..3u32 {
            for notify in 0..2u32 {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, false);
                let a = w.self_ as *mut AActor;
                unsafe {
                    (*a).base = if old == 1 {
                        w.base as _
                    } else {
                        ptr::null_mut()
                    };
                    case(
                        "set_base",
                        idx,
                        &format!("old_base={old} new_base={nw} notify={notify}"),
                    );
                    let target = match nw {
                        0 => 0,
                        1 => w.base,
                        _ => w.self_,
                    };
                    actor::set_base(svc, a, target as _, notify == 1);
                    emit(format!("res base={}", nm((*a).base as usize)));
                }
                idx += 1;
            }
        }
    }
}

const ROLE_NAMES: [&str; 3] = ["plain", "projectile", "trigger"];
const ROLE_ISA: [&str; 3] = [" AActor ", " AActor AProjectile ", " AActor ATrigger "];

fn drive_begin_touch(svc: &Hp2ActorServices) {
    let roles: [(usize, usize); 6] = [(1, 2), (2, 1), (0, 0), (0, 2), (1, 0), (2, 0)];
    let mut idx = 0;
    for (sr, or) in roles {
        for full in 0..2u32 {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            reg(w.self_, "self", ROLE_ISA[sr]);
            reg(w.other, "other", ROLE_ISA[or]);
            let a = w.self_ as *mut AActor;
            let o = w.other as *mut AActor;
            unsafe {
                if full == 1 {
                    let mut touching = (*a).touching;
                    for t in touching.iter_mut() {
                        *t = w.touch as _;
                    }
                    (*a).touching = touching;
                }
                case(
                    "begin_touch",
                    idx,
                    &format!(
                        "self_role={} other_role={} touching_full={full}",
                        ROLE_NAMES[sr], ROLE_NAMES[or]
                    ),
                );
                actor::begin_touch(svc, a, o);
                emit(format!(
                    "res touching0={} touching1={}",
                    nm((*a).touching[0] as usize),
                    nm((*a).touching[1] as usize)
                ));
            }
            idx += 1;
        }
    }
}

fn drive_end_touch(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for ts in 0..2u32 {
        for to in 0..2u32 {
            for nn in 0..2u32 {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, false);
                let a = w.self_ as *mut AActor;
                let o = w.other as *mut AActor;
                unsafe {
                    if ts == 1 {
                        (*a).touching[0] = w.other as _;
                    }
                    if to == 1 {
                        (*o).touching[0] = w.self_ as _;
                    }
                    case(
                        "end_touch",
                        idx,
                        &format!("self_touching={ts} other_touching={to} no_notify_self={nn}"),
                    );
                    actor::end_touch(svc, a, o, nn as i32);
                    emit(format!(
                        "res self_touch0={} other_touch0={}",
                        nm((*a).touching[0] as usize),
                        nm((*o).touching[0] as usize)
                    ));
                }
                idx += 1;
            }
        }
    }
}

fn drive_is_overlapping(svc: &Hp2ActorServices) {
    const OTHER: [&str; 3] = ["near", "far", "brush"];
    let mut idx = 0;
    for sb in 0..2u32 {
        for (ok, other_name) in OTHER.iter().enumerate() {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let a = w.self_ as *mut AActor;
            let o = w.other as *mut AActor;
            if sb == 1 {
                reg(w.self_, "self", " AActor ABrush ");
                unsafe { (*a).brush = w.brush as _ };
            }
            if ok == 2 {
                reg(w.other, "other", " AActor ABrush ");
                unsafe { (*o).brush = w.brush as _ };
            }
            let far = if ok == 1 { 10000.0 } else { 5.0 };
            unsafe {
                (*a).location = zvec();
                (*o).location = fv3([far, 0.0, 0.0]);
                (*a).collision_radius = 10.0;
                (*a).collision_height = 10.0;
                (*o).collision_radius = 10.0;
                (*o).collision_height = 10.0;
                case(
                    "is_overlapping",
                    idx,
                    &format!("self_brush={sb} other={other_name}"),
                );
                let r = actor::is_overlapping(svc, a, o);
                emit(format!("res overlapping={r}"));
            }
            idx += 1;
        }
    }
}

fn drive_get_visibility_box() {
    for (i, &(r, h)) in [(0, 0), (100, 50), (-1, 2)].iter().enumerate() {
        let mut rig = Rig::default();
        let w = fresh(&mut rig, false);
        let a = w.self_ as *mut AActor;
        unsafe {
            (*a).location = fv3([10.0, 20.0, 30.0]);
            (*a).visibility_radius = r as f32;
            (*a).visibility_height = h as f32;
            case("get_visibility_box", i, &format!("radius={r} height={h}"));
            let b = actor::get_visibility_box(a);
            emit(format!(
                "{}{}",
                vec3("res_min", arr(&b.min)),
                vec3(" max", arr(&b.max))
            ));
        }
    }
}

fn drive_carcass_update_frequency() {
    let mut idx = 0;
    for phys in 0..2u32 {
        for anim in 0..2u32 {
            for nuf in [2u32, 4, 9] {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, false);
                let c = w.carcass as *mut AActor;
                unsafe {
                    (*c).physics = phys as u8;
                    (*c).anim_rate = if anim == 1 { 1.0 } else { 0.0 };
                    if anim == 1 {
                        (*c).anim_sequence = 1u32.to_le_bytes();
                    }
                    (*c).net_update_frequency = nuf as f32;
                    case(
                        "carcass_update_frequency",
                        idx,
                        &format!("physics={phys} anim={anim} net_update_freq={nuf}"),
                    );
                    let r = actor::carcass_update_frequency(c);
                    emit(bits("res_freq", r));
                }
                idx += 1;
            }
        }
    }
}

fn drive_get_hit_actor() {
    for h in 0..2u32 {
        let mut rig = Rig::default();
        let w = fresh(&mut rig, false);
        let a = w.self_ as *mut AActor;
        unsafe {
            (*a).hit_actor = if h == 1 {
                w.other as _
            } else {
                ptr::null_mut()
            };
            case("get_hit_actor", h as usize, &format!("hit_actor={h}"));
            let r = actor::get_hit_actor(a);
            emit(format!("res hit={}", nm(r as usize)));
        }
    }
}

fn drive_post_edit_change(svc: &Hp2ActorServices) {
    let _ = svc;
    for e in 0..2i32 {
        let mut rig = Rig::default();
        let w = fresh(&mut rig, false);
        let a = w.self_ as *mut AActor;
        with_st(|s| s.editor = e);
        case("post_edit_change", e as usize, &format!("editor={e}"));
        emit("hook uobject_post_edit_change self");
        unsafe {
            actor::actor_post_edit_change(svc, a);
            emit(format!(
                "res light_changed={}",
                (*a).b_light_changed() as i32
            ));
        }
    }
}

fn drive_zone_info_post_edit_change(svc: &Hp2ActorServices) {
    for e in 0..2i32 {
        let mut rig = Rig::default();
        let _w = fresh(&mut rig, false);
        with_st(|s| s.editor = e);
        case(
            "zone_info_post_edit_change",
            e as usize,
            &format!("editor={e}"),
        );
        emit("hook uobject_post_edit_change zone");
        unsafe { actor::zone_info_post_edit_change(svc) };
        emit("res done=1");
    }
}

fn drive_get_primitive(svc: &Hp2ActorServices) {
    for ct in 0..6u8 {
        for mesh in 0..2u32 {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let a = w.self_ as *mut AActor;
            unsafe {
                (*a).collide_type = ct;
                if mesh == 1 {
                    (*a).mesh = w.mesh as _;
                }
                case(
                    "get_primitive",
                    ct as usize * 2 + mesh as usize,
                    &format!("collide_type={ct} mesh={mesh}"),
                );
                let p = actor::get_primitive(svc, a);
                emit(format!("res prim={}", nm(p as usize)));
            }
        }
    }
}

fn drive_spawned(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for ct in [3u8, 0] {
        for mesh in 0..2u32 {
            for brush in 0..2u32 {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, false);
                let a = w.self_ as *mut AActor;
                unsafe {
                    (*a).collide_type = ct;
                    if mesh == 1 {
                        (*a).mesh = w.mesh as _;
                    }
                    if brush == 1 {
                        (*a).brush = w.brush as _;
                    }
                    case(
                        "spawned",
                        idx,
                        &format!("collide_type={ct} mesh={mesh} brush={brush}"),
                    );
                    actor::spawned(svc, a);
                }
                emit("res done=1");
                idx += 1;
            }
        }
    }
}

fn drive_process_event(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for begun in 0..2i32 {
        for fnf in 0..2u32 {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let a = w.self_ as *mut AActor;
            with_st(|s| s.level_begun = begun);
            case(
                "process_event",
                idx,
                &format!("begun={begun} function={fnf}"),
            );
            let func = if fnf == 1 { w.func } else { 0 };
            unsafe {
                actor::process_event(svc, a, func as _, w.parms as _, ptr::null_mut());
            }
            emit("res done=1");
            idx += 1;
        }
    }
}

fn drive_destroy(svc: &Hp2ActorServices) {
    for ri in 0..2u32 {
        let mut rig = Rig::default();
        let w = fresh(&mut rig, false);
        let a = w.self_ as *mut AActor;
        unsafe {
            if ri == 1 {
                (*a).render_interface = w.ri as _;
            }
            case("destroy", ri as usize, &format!("render_iface={ri}"));
            actor::destroy(svc, a);
            emit(format!(
                "res render_iface={}",
                nm((*a).render_interface as usize)
            ));
        }
    }
}

fn drive_post_load(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for loc in 0..2i32 {
        for brush in 0..2u32 {
            for polys in 0..2u32 {
                for ct in [3u8, 0] {
                    let mut rig = Rig::default();
                    let w = fresh(&mut rig, false);
                    let a = w.self_ as *mut AActor;
                    with_st(|s| {
                        s.localized = loc;
                        s.polys = if brush == 1 && polys == 1 { w.polys } else { 0 };
                    });
                    unsafe {
                        (*a).collide_type = ct;
                        if brush == 1 {
                            (*a).brush = w.brush as _;
                        }
                        case(
                            "post_load",
                            idx,
                            &format!(
                                "localized={loc} brush={brush} polys={polys} collide_type={ct}"
                            ),
                        );
                        actor::post_load(svc, a);
                    }
                    emit("res done=1");
                    idx += 1;
                }
            }
        }
    }
}

fn drive_set_player(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for prev in 0..2u32 {
        for pp in 0..2u32 {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let p = w.pawn as *mut APlayerPawn;
            with_st(|s| s.player_actor = if prev == 1 { w.other } else { 0 });
            unsafe {
                if pp == 1 {
                    (*p).player = w.player as _;
                }
                case(
                    "set_player",
                    idx,
                    &format!("prev_actor={prev} pawn_player={pp}"),
                );
                actor::set_player(svc, p, w.player as _);
                emit(format!("res pawn_player={}", nm((*p).player as usize)));
            }
            idx += 1;
        }
    }
}

fn drive_get_cylinder_extent(svc: &Hp2ActorServices) {
    for ct in 0..6u8 {
        for wd in 0..2u32 {
            let idx = ct as usize * 2 + wd as usize;
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            let a = w.self_ as *mut AActor;
            unsafe {
                (*a).collide_type = ct;
                (*a).collision_width = if wd == 1 { 7.0 } else { 0.0 };
                (*a).collision_radius = 12.0;
                (*a).collision_height = 24.0;
                let width = if wd == 1 { 7 } else { 0 };
                case(
                    "get_cylinder_extent",
                    idx,
                    &format!("collide_type={ct} width={width}"),
                );
                let mut out = zvec();
                actor::get_cylinder_extent(svc, a, &mut out);
                emit(vec3("res", arr(&out)));
            }
        }
    }
    // Shape with a mesh, then shape with a brush.
    let mut rig = Rig::default();
    let w = fresh(&mut rig, false);
    let a = w.self_ as *mut AActor;
    unsafe {
        (*a).collide_type = 3;
        (*a).mesh = w.mesh as _;
        case("get_cylinder_extent", 12, "collide_type=3 mesh=1 brush=0");
        let mut out = zvec();
        actor::get_cylinder_extent(svc, a, &mut out);
        emit(vec3("res", arr(&out)));
    }
    let mut rig = Rig::default();
    let w = fresh(&mut rig, false);
    let a = w.self_ as *mut AActor;
    unsafe {
        (*a).collide_type = 3;
        (*a).brush = w.brush as _;
        case("get_cylinder_extent", 13, "collide_type=3 mesh=0 brush=1");
        let mut out = zvec();
        actor::get_cylinder_extent(svc, a, &mut out);
        emit(vec3("res", arr(&out)));
    }
}

fn drive_get_render_bounding_box(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for exact in 0..2i32 {
        for mesh in 0..2u32 {
            for brush in 0..2u32 {
                let mut rig = Rig::default();
                let w = fresh(&mut rig, false);
                let a = w.self_ as *mut AActor;
                unsafe {
                    if mesh == 1 {
                        (*a).mesh = w.mesh as _;
                    }
                    if brush == 1 {
                        (*a).brush = w.brush as _;
                    }
                    case(
                        "get_render_bounding_box",
                        idx,
                        &format!("exact={exact} mesh={mesh} brush={brush}"),
                    );
                    let mut out = zcoords();
                    actor::get_render_bounding_box(svc, a, exact, &mut out);
                    emit(vec3("res_origin", arr(&out.origin)));
                }
                idx += 1;
            }
        }
    }
}

fn drive_serialize_persistence(svc: &Hp2ActorServices) {
    const COMBOS: [(i32, i32, i32); 6] = [
        (0, 0, 0),
        (0, 0, 1),
        (1, 0, 0),
        (1, 0, 1),
        (0, 1, 0),
        (0, 1, 1),
    ];
    for (idx, (loading, saving, trans)) in COMBOS.into_iter().enumerate() {
        let mut rig = Rig::default();
        let w = fresh(&mut rig, false);
        let a = w.self_ as *mut AActor;
        with_st(|s| {
            s.ar_loading = loading;
            s.ar_saving = saving;
            s.ar_trans = trans;
        });
        unsafe {
            case(
                "serialize_persistence",
                idx,
                &format!("loading={loading} saving={saving} trans={trans}"),
            );
            actor::serialize_persistence(svc, a, w.ar as _);
        }
        emit("res done=1");
    }
}

/// Points the level at the brush model and sets the surf/texture knobs.
fn set_trace_world(w: &W, kind: i32) {
    unsafe { (*(w.level as *mut ULevel)).model = w.brush as _ };
    with_st(|s| {
        s.trace_kind = kind;
        s.surf = w.surf;
        if kind >= 1 {
            s.surf_texture = w.tex[0];
            s.surf_flags = if kind == 2 { 0x10 } else { 0x1 };
        }
        if kind == 2 {
            s.tex_flags = 0x100;
        }
    });
}

fn drive_trace_texture(svc: &Hp2ActorServices) {
    let mut idx = 0;
    for kind in 0..3i32 {
        for dec in 0..2u32 {
            let mut rig = Rig::default();
            let w = fresh(&mut rig, false);
            set_trace_world(&w, kind);
            let a = w.self_ as *mut AActor;
            let end = fv3([0.0, 0.0, -100.0]);
            let start = fv3([0.0, 0.0, 100.0]);
            let mut flags: i32 = 0;
            case(
                "trace_texture",
                idx,
                &format!("surf={kind} trace_decals={dec}"),
            );
            let out = unsafe { actor::trace_texture(svc, a, &end, &start, &mut flags, dec == 1) };
            emit(format!(
                "res texture={} flags={}",
                nm(out as usize),
                flags as u32
            ));
            idx += 1;
        }
    }
}

/// The 8 decal-flip cases, read from the fixture so the inputs are the
/// recorded ones (same approach as `actor_decal_trace.rs`).
fn decal_flip_inputs() -> Vec<(usize, [u32; 18])> {
    FIXTURE
        .lines()
        .filter_map(|l| l.strip_prefix("case trace_decal_flip "))
        .map(|rest| {
            let (idx, knobs) = rest.split_once(' ').expect("case index");
            let hex = knobs
                .strip_prefix("decal_flip bits=")
                .expect("decal_flip knob");
            let parts: Vec<&str> = hex.split(',').collect();
            assert_eq!(parts.len(), 18, "decal_flip word count");
            let mut words = [0u32; 18];
            for (w, p) in words.iter_mut().zip(parts) {
                *w = u32::from_str_radix(p, 16).expect("hex word");
            }
            (idx.parse().expect("case index"), words)
        })
        .collect()
}

fn drive_trace_decal_flip(svc: &Hp2ActorServices) {
    for (idx, words) in decal_flip_inputs() {
        let knobs = format!(
            "decal_flip bits={}",
            words
                .iter()
                .map(|w| format!("{w:08x}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut rig = Rig::default();
        let w = fresh(&mut rig, false);
        let decal = rig.obj("decal", "");
        set_trace_world(&w, 1);
        let f = |i: usize| f32::from_bits(words[i]);
        let vert = |k: usize| [f(3 * k), f(3 * k + 1), f(3 * k + 2)];
        with_st(|s| {
            s.surf_flags = 1;
            s.tex_flags = 0;
            s.surf_texture = w.tex[0];
            s.decal_count = 1;
            s.decal_ptr = decal;
            s.decal_tex = w.tex[1];
            s.decal_verts = [vert(0), vert(1), vert(2), vert(3)];
            s.model_vec = [f(12), f(13), f(14)];
            s.model_pt = [0.0; 3];
            s.hit_loc = [f(15), f(16), f(17)];
        });
        let a = w.self_ as *mut AActor;
        let end = fv3([0.0, 0.0, -100.0]);
        let start = fv3([0.0, 0.0, 100.0]);
        let mut flags: i32 = 0;
        case("trace_decal_flip", idx, &knobs);
        let out = unsafe { actor::trace_texture(svc, a, &end, &start, &mut flags, true) };
        emit(format!(
            "res texture={} flags={}",
            nm(out as usize),
            flags as u32
        ));
    }
}

const NET_KINDS: [&str; 11] = [
    "pawn_small",
    "pawn_smooth",
    "mover",
    "pawn_fma",
    "generic_state",
    "demo_view",
    "pawn_teleport",
    "pri_ping",
    "pawn_over_10000",
    "demo_driver_absent",
    "demo_server_connection_absent",
];

fn drive_net_receive(svc: &Hp2ActorServices) {
    for (c, kind) in NET_KINDS.iter().enumerate() {
        let mut rig = Rig::default();
        let w = fresh(&mut rig, true);
        let isa = match c {
            2 => " AActor ABrush AMover ",
            4 => " AActor ",
            5 | 9 | 10 => " AActor APlayerPawn APawn ",
            7 => " AActor APlayerReplicationInfo ",
            _ => " AActor APawn ",
        };
        reg(w.self_, "self", isa);
        with_st(|s| s.smooth_noop = c == 1 || c == 3);
        let a = w.self_ as *mut AActor;
        let l = w.level as *mut ULevel;
        let li = w.li as *mut ALevelInfo;
        let pawn = w.pawn as *mut APlayerPawn;
        unsafe {
            (*a).set_b_collide_actors(c == 4);
            if c == 2 {
                let m = a as *mut AMover;
                (*m).sim_interpolate = fv3([1.0, 2.0, 3.0]);
                (*m).sim_old_pos = fv3([7.0, 8.0, 9.0]);
                (*m).sim_old_rot_pitch = 10;
                (*m).sim_old_rot_yaw = 20;
                (*m).sim_old_rot_roll = 30;
            }
            if c != 2 && c != 5 && c != 7 {
                (*a).role = ROLE_SIMULATED_PROXY;
                (*a).velocity = fv3([1.0, 0.0, 0.0]);
                (*a).anim_min_rate = 0.2;
            }
            if c == 5 {
                with_st(|s| s.demo_conn = 1);
            }
            if c == 5 || c == 10 {
                (*l).demo_rec_driver = w.driver as _;
            }
            if c == 7 {
                (*li).net_mode = NM_CLIENT;
                (*l).net_driver = w.driver as _;
                with_st(|s| {
                    s.avg_frame = 0.011;
                    s.viewport_pri = w.self_;
                });
                (*pawn).base.player_replication_info = w.self_ as _;
                (*a.cast::<APlayerReplicationInfo>()).ping = 10;
            }
            case("net_receive", c, kind);
            actor::pre_net_receive(svc, a);
            match c {
                0 => {
                    (*a).location = fv3([10.0, 0.0, 0.0]);
                    (*a).sim_anim = FPlane {
                        x: 1000.0,
                        y: 2000.0,
                        z: 3000.0,
                        w: -4000.0,
                    };
                }
                1 => (*a).location = fv3([50.0, 0.0, 0.0]),
                2 => (*(a as *mut AMover)).sim_interpolate = fv3([4.0, 5.0, 513.5]),
                3 => {
                    (*a).location = fv3([
                        f32::from_bits(0x420a_1569),
                        f32::from_bits(0x41d2_125f),
                        f32::from_bits(0x42b4_355e),
                    ]);
                }
                4 => {
                    (*a).set_b_collide_actors(false);
                    (*a).base = w.base as _;
                    (*a).rotation = FRotator {
                        pitch: 123,
                        yaw: -456,
                        roll: 789,
                    };
                    (*a).collision_radius = 3.0;
                    (*a).collision_height = 4.0;
                    (*a).set_b_block_actors(true);
                    (*a).set_b_block_players(false);
                }
                5 | 9 | 10 => {
                    (*(a as *mut APlayerPawn)).demo_view_pitch = 1234;
                    (*(a as *mut APlayerPawn)).demo_view_yaw = -5678;
                }
                6 => {
                    (*a).location = fv3([100.0, 0.0, 0.0]);
                    (*a).sim_anim = FPlane {
                        x: 1000.0,
                        y: 2000.0,
                        z: 3000.0,
                        w: 4000.0,
                    };
                }
                8 => (*a).location = fv3([101.0, 0.0, 0.0]),
                _ => {}
            }
            actor::post_net_receive(svc, a);

            let mv = a as *mut AMover;
            let mut line = format!("res loc={}", vec3("", arr(&(*a).location)));
            line += &format!(
                " anim={:08x},{:08x},{:08x},{:08x} loop={} min={:08x} tele={}",
                (*a).anim_frame.to_bits(),
                (*a).anim_rate.to_bits(),
                (*a).tween_rate.to_bits(),
                (*a).anim_last.to_bits(),
                (*a).b_anim_loop() as i32,
                (*a).anim_min_rate.to_bits(),
                (*a).b_just_teleported() as i32
            );
            if c == 2 {
                line += &format!(
                    " key={},{} alpha={:08x} rate={:08x} interpolating={}",
                    (*mv).key_num,
                    (*mv).prev_key_num,
                    (*mv).phys_alpha.to_bits(),
                    (*mv).phys_rate.to_bits(),
                    (*mv).base.b_interpolating() as i32
                );
            }
            if c == 4 {
                line += &format!(
                    " base={} collide={} block={},{} radius={:08x} height={:08x} rot={},{},{}",
                    nm((*a).base as usize),
                    (*a).b_collide_actors() as i32,
                    (*a).b_block_actors() as i32,
                    (*a).b_block_players() as i32,
                    (*a).collision_radius.to_bits(),
                    (*a).collision_height.to_bits(),
                    (*a).rotation.pitch,
                    (*a).rotation.yaw,
                    (*a).rotation.roll
                );
            }
            if matches!(c, 5 | 9 | 10) {
                line += &format!(
                    " viewrot={},{}",
                    (*(a as *mut APlayerPawn)).base.view_rotation.pitch,
                    (*(a as *mut APlayerPawn)).base.view_rotation.yaw
                );
            }
            if c == 7 {
                line += &format!(" ping={}", (*a.cast::<APlayerReplicationInfo>()).ping);
            }
            emit(line);
        }
    }
}

// ---- Driver and comparison. -------------------------------------------------

fn drive_all() {
    let svc = services();
    drive_get_skin();
    drive_get_net_priority();
    drive_set_collision(&svc);
    drive_set_collision_size(&svc);
    drive_set_owner(&svc);
    drive_set_base(&svc);
    drive_begin_touch(&svc);
    drive_end_touch(&svc);
    drive_is_overlapping(&svc);
    drive_get_visibility_box();
    drive_carcass_update_frequency();
    drive_get_hit_actor();
    drive_post_edit_change(&svc);
    drive_zone_info_post_edit_change(&svc);
    drive_get_primitive(&svc);
    drive_spawned(&svc);
    drive_process_event(&svc);
    drive_destroy(&svc);
    drive_post_load(&svc);
    drive_set_player(&svc);
    drive_get_cylinder_extent(&svc);
    drive_get_render_bounding_box(&svc);
    drive_serialize_persistence(&svc);
    drive_trace_texture(&svc);
    drive_trace_decal_flip(&svc);
    drive_net_receive(&svc);
}

#[test]
fn unactor_records_match_oracle_fixture() {
    LOG.with(|l| l.borrow_mut().clear());
    drive_all();
    let got: Vec<String> = LOG.with(|l| l.borrow().clone());
    let want: Vec<&str> = FIXTURE.lines().collect();

    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(
            g,
            w,
            "record line {} differs from the oracle fixture",
            i + 1
        );
    }
    assert_eq!(got.len(), want.len(), "record line count differs");

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for line in &got {
        if let Some(rest) = line.strip_prefix("case ") {
            let fam = rest.split(' ').next().unwrap_or("");
            *counts.entry(fam).or_default() += 1;
        }
    }
    let expected: [(&str, usize); 26] = [
        ("get_skin", 16),
        ("get_net_priority", 18),
        ("set_collision", 12),
        ("set_collision_size", 12),
        ("set_owner", 4),
        ("set_base", 12),
        ("begin_touch", 12),
        ("end_touch", 8),
        ("is_overlapping", 6),
        ("get_visibility_box", 3),
        ("carcass_update_frequency", 12),
        ("get_hit_actor", 2),
        ("post_edit_change", 2),
        ("zone_info_post_edit_change", 2),
        ("get_primitive", 12),
        ("spawned", 8),
        ("process_event", 4),
        ("destroy", 2),
        ("post_load", 16),
        ("set_player", 4),
        ("get_cylinder_extent", 14),
        ("get_render_bounding_box", 8),
        ("serialize_persistence", 6),
        ("trace_texture", 6),
        ("trace_decal_flip", 8),
        ("net_receive", 11),
    ];
    for (fam, n) in expected {
        assert_eq!(
            counts.get(fam).copied().unwrap_or(0),
            n,
            "case count for {fam}"
        );
    }
    assert_eq!(counts.len(), expected.len(), "family count");
    assert_eq!(counts.values().sum::<usize>(), 220, "total case count");
    assert!(
        !got.iter().any(|l| l.contains("STUB")),
        "stub hit in replay"
    );
}
