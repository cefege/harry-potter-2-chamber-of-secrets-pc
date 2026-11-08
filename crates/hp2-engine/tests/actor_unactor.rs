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
    AActor, ALevelInfo, AMover, APlayerPawn, APlayerReplicationInfo, FArchive, FBox, FBspSurf,
    FDecal, FRotator, UClass, UDemoRecDriver, UGameEngine, ULevel, UModel, UNetConnection,
    UNetDriver, UObject, UPlayer, USkeletalMesh, UTexture,
};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_engine::actor::{self, Hp2ActorServices, class};
use hp2_engine::levact;

const FIXTURE: &str = include_str!("fixtures/actor_unactor.txt");

/// Large enough for the biggest generated actor type (APlayerPawn and friends).
const SLOT: usize = 32 * 1024;

/// Oracle `ENetRole::ROLE_SimulatedProxy` (EngineClasses.h:563).
const ROLE_SIMULATED_PROXY: u8 = 2;
/// Oracle `NM_Client`.
const NM_CLIENT: u8 = 3;
/// Oracle `CLASS_Localized` (`UnObjBas.h`).
const CLASS_LOCALIZED: u32 = 0x0000_0020;
/// Oracle `RF_Transactional` (`UnObjBas.h`).
const RF_TRANSACTIONAL: u32 = 0x0000_0001;
/// Oracle `RF_DebugSerialize` (`UnObjBas.h`).
const RF_DEBUG_SERIALIZE: u32 = 0x4000_0000;

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
    /// Engine primitives: cylinder, oval cylinder, box, oval, oval-oval.
    prims: [usize; 5],
    viewport_pri: usize,
    class_ptr: usize,
    uclass_static: usize,
    smooth_noop: bool,
    trace_kind: i32,
    surf: usize,
    decal_count: i32,
    decal_ptr: usize,
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

unsafe extern "C" fn s_level_hash_add_actor(_level: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook hash {} AddActor", nm(actor as usize)));
    })();
    0
}

unsafe extern "C" fn s_level_hash_remove_actor(_level: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook hash {} RemoveActor", nm(actor as usize)));
    })();
    0
}

unsafe extern "C" fn s_actor_is_a(obj: *const c_void, class_id: u32, out_value: *mut i32) -> i32 {
    unsafe {
        out_value.write((|| {
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
        })());
    }
    0
}

unsafe extern "C" fn s_level_far_move_actor(
    level: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
    test: i32,
    no_check: i32,
    out_value: *mut i32,
) -> i32 {
    unsafe {
        out_value.write((|| {
            let d = unsafe { ptr::read(dest.cast::<FVector>()) };
            emit(format!(
                "hook far_move {} {} test={test} no_check={no_check}",
                nm(level as usize),
                vec3("dest", arr(&d))
            ));
            unsafe { (*actor.cast::<AActor>()).location = d };
            1
        })());
    }
    0
}

unsafe extern "C" fn s_level_move_actor_to_rotation(
    level: *mut c_void,
    actor: *mut c_void,
    rot: *const c_void,
) -> i32 {
    (|| {
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
    })();
    0
}

unsafe extern "C" fn s_actor_move_smooth(
    actor: *mut c_void,
    delta: *const c_void,
    out_value: *mut i32,
) -> i32 {
    unsafe {
        out_value.write((|| {
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
        })());
    }
    0
}

unsafe extern "C" fn s_actor_set_physics(actor: *mut c_void, phys: u8) -> i32 {
    (|| {
        emit(format!("hook set_physics {} {phys}", nm(actor as usize)));
        unsafe { (*actor.cast::<AActor>()).physics = phys };
    })();
    0
}

unsafe extern "C" fn s_actor_perform_physics(_actor: *mut c_void, _delta: f32) -> i32 {
    0
}

unsafe extern "C" fn s_actor_event_bump(actor: *mut c_void, other: *mut c_void) -> i32 {
    (|| {
        event(actor, "Bump", other);
    })();
    0
}

unsafe extern "C" fn s_actor_event_detach(base: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        event(base, "Detach", actor);
    })();
    0
}

unsafe extern "C" fn s_actor_event_attach(base: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        event(base, "Attach", actor);
    })();
    0
}

unsafe extern "C" fn s_actor_event_base_change(actor: *mut c_void) -> i32 {
    (|| {
        event(actor, "BaseChange", ptr::null_mut());
    })();
    0
}

unsafe extern "C" fn s_actor_event_lost_child(owner: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        event(owner, "LostChild", actor);
    })();
    0
}

unsafe extern "C" fn s_actor_event_gained_child(owner: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        event(owner, "GainedChild", actor);
    })();
    0
}

unsafe extern "C" fn s_level_viewport0_actor_pri(
    _level: *mut c_void,
    out_value: *mut *mut c_void,
) -> i32 {
    unsafe {
        out_value.write((|| with_st(|s| s.viewport_pri) as *mut c_void)());
    }
    0
}

unsafe extern "C" fn s_check_failed(expr: *const c_char, line: i32) -> i32 {
    (|| {
        let e = unsafe { CStr::from_ptr(expr) }
            .to_string_lossy()
            .into_owned();
        emit(format!("hook check_failed {e} {line}"));
    })();
    0
}

unsafe extern "C" fn s_actor_event_touch(actor: *mut c_void, other: *mut c_void) -> i32 {
    (|| {
        event(actor, "Touch", other);
    })();
    0
}

unsafe extern "C" fn s_actor_event_untouch(actor: *mut c_void, other: *mut c_void) -> i32 {
    (|| {
        event(actor, "UnTouch", other);
    })();
    0
}

unsafe extern "C" fn s_is_editor(out_value: *mut i32) -> i32 {
    unsafe {
        out_value.write((|| with_st(|s| s.editor))());
    }
    0
}

unsafe extern "C" fn s_gcache_flush() -> i32 {
    (|| {
        emit("hook cache_flush");
    })();
    0
}

unsafe extern "C" fn s_primitive_validate_actor(prim: *mut c_void, actor: *mut c_void) -> i32 {
    (|| {
        emit(format!(
            "hook validate {} {}",
            nm(prim as usize),
            nm(actor as usize)
        ));
    })();
    0
}

unsafe extern "C" fn s_uobject_process_event(
    obj: *mut c_void,
    func: *mut c_void,
    parms: *mut c_void,
    result: *mut c_void,
) -> i32 {
    (|| {
        emit(format!(
            "hook super_process_event {} {} {} {}",
            nm(obj as usize),
            nm(func as usize),
            nm(parms as usize),
            nm(result as usize)
        ));
    })();
    0
}

unsafe extern "C" fn s_uobject_destroy(obj: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook uobject_destroy {}", nm(obj as usize)));
    })();
    0
}

unsafe extern "C" fn s_render_interface_destroy(ri: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook delete_ri {}", nm(ri as usize)));
    })();
    0
}

unsafe extern "C" fn s_object_conditional_destroy(obj: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook cond_destroy {}", nm(obj as usize)));
    })();
    0
}

unsafe extern "C" fn s_actor_load_localized(actor: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook load_localized {}", nm(actor as usize)));
    })();
    0
}

unsafe extern "C" fn s_uobject_post_load(obj: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook uobject_postload {}", nm(obj as usize)));
    })();
    0
}

unsafe extern "C" fn s_actor_event_possess(pawn: *mut c_void) -> i32 {
    (|| {
        event(pawn, "Possess", ptr::null_mut());
    })();
    0
}

unsafe extern "C" fn s_actor_log_possessed(pawn: *mut c_void) -> i32 {
    (|| {
        emit(format!("hook possessed {}", nm(pawn as usize)));
    })();
    0
}

unsafe extern "C" fn s_mesh_collision_extent(
    mesh: *mut c_void,
    actor: *mut c_void,
    out: *mut c_void,
) -> i32 {
    (|| {
        emit(format!(
            "hook collision_extent {} {}",
            nm(mesh as usize),
            nm(actor as usize)
        ));
        unsafe { ptr::write(out.cast::<FVector>(), fv3([16.0, 24.0, 32.0])) };
    })();
    0
}

unsafe extern "C" fn s_brush_collision_extent(
    brush: *mut c_void,
    actor: *mut c_void,
    out: *mut c_void,
) -> i32 {
    (|| {
        emit(format!(
            "hook collision_extent {} {}",
            nm(brush as usize),
            nm(actor as usize)
        ));
        unsafe { ptr::write(out.cast::<FVector>(), fv3([16.0, 24.0, 32.0])) };
    })();
    0
}

unsafe extern "C" fn s_mesh_render_bounding_box(
    mesh: *mut c_void,
    actor: *mut c_void,
    exact: i32,
    out: *mut c_void,
) -> i32 {
    (|| {
        emit(format!(
            "hook mesh_render_box {} {} exact={exact}",
            nm(mesh as usize),
            nm(actor as usize)
        ));
        unsafe { ptr::write(out.cast::<FCoords>(), zcoords()) };
    })();
    0
}

unsafe extern "C" fn s_brush_render_bounding_box_coords(
    brush: *mut c_void,
    actor: *mut c_void,
    exact: i32,
    out: *mut c_void,
) -> i32 {
    (|| {
        emit(format!(
            "hook brush_render_box {} {} exact={exact}",
            nm(brush as usize),
            nm(actor as usize)
        ));
        unsafe { ptr::write(out.cast::<FCoords>(), zcoords()) };
    })();
    0
}

unsafe extern "C" fn s_class_is_uclass_static(cls: *mut c_void, out_value: *mut i32) -> i32 {
    unsafe {
        out_value.write((|| {
            emit("hook uclass_static");
            i32::from(with_st(|s| s.uclass_static) == cls as usize)
        })());
    }
    0
}

unsafe extern "C" fn s_archive_preload(ar: *mut c_void, obj: *mut c_void) -> i32 {
    (|| {
        emit(format!(
            "hook preload {} {}",
            nm(ar as usize),
            nm(obj as usize)
        ));
    })();
    0
}

unsafe extern "C" fn s_object_serialize_tagged_persistent(
    obj: *mut c_void,
    ar: *mut c_void,
) -> i32 {
    (|| {
        let cls = with_st(|s| s.class_ptr);
        emit(format!(
            "hook serialize_tagged {} {} {}",
            nm(cls),
            nm(ar as usize),
            nm(obj as usize)
        ));
    })();
    0
}

unsafe extern "C" fn s_object_serialize_bin(obj: *mut c_void, ar: *mut c_void) -> i32 {
    (|| {
        let cls = with_st(|s| s.class_ptr);
        emit(format!(
            "hook serialize_bin {} {} {}",
            nm(cls),
            nm(ar as usize),
            nm(obj as usize)
        ));
    })();
    0
}

unsafe extern "C" fn s_archive_count_bytes(ar: *mut c_void, size: usize) -> i32 {
    (|| {
        emit(format!("hook count_bytes {} {size}", nm(ar as usize)));
    })();
    0
}

unsafe extern "C" fn s_trace_line_texture_hit(
    actor: *mut c_void,
    _trace_end: *const c_void,
    _trace_start: *const c_void,
    out_location: *mut c_void,
    out_value: *mut *mut c_void,
) -> i32 {
    unsafe {
        out_value.write((|| {
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
        })());
    }
    0
}

unsafe extern "C" fn s_bsp_surf_decal_count(_surf: *mut c_void, out_value: *mut i32) -> i32 {
    unsafe {
        out_value.write((|| with_st(|s| s.decal_count))());
    }
    0
}

unsafe extern "C" fn s_model_vector(
    _model: *mut c_void,
    _index: i32,
    out_value: *mut *const c_void,
) -> i32 {
    unsafe { out_value.write(with_st(|s| s.model_vec.as_ptr().cast())) };
    0
}

unsafe extern "C" fn s_model_point(
    _model: *mut c_void,
    _index: i32,
    out_value: *mut *const c_void,
) -> i32 {
    unsafe { out_value.write(with_st(|s| s.model_pt.as_ptr().cast())) };
    0
}

unsafe extern "C" fn s_bsp_surf_decal(
    _surf: *mut c_void,
    _index: i32,
    out_value: *mut *mut c_void,
) -> i32 {
    unsafe {
        out_value.write((|| with_st(|s| s.decal_ptr) as *mut c_void)());
    }
    0
}

/// Oracle `AActor::GetPrimitive`: a shape uses its mesh, then its brush;
/// every other collide type maps to an engine primitive.
unsafe extern "C" fn s_actor_get_primitive(actor: *mut c_void, out_value: *mut *mut c_void) -> i32 {
    unsafe {
        out_value.write((|| {
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
        })());
    }
    0
}

unsafe extern "C" fn s_object_is_valid(obj: *mut c_void, out_value: *mut i32) -> i32 {
    unsafe {
        out_value.write((|| {
            emit(format!("hook is_valid {}", nm(obj as usize)));
            i32::from(!obj.is_null())
        })());
    }
    0
}

fn services() -> Hp2ActorServices {
    Hp2ActorServices {
        actor_end_touch: checked_end_touch,
        actor_touch_to: checked_touch_to,
        render_interface_remove_root: checked_remove_root,
        level_hash_add_actor: s_level_hash_add_actor,
        level_hash_remove_actor: s_level_hash_remove_actor,
        actor_is_a: s_actor_is_a,
        level_far_move_actor: s_level_far_move_actor,
        level_move_actor_to_rotation: s_level_move_actor_to_rotation,
        actor_move_smooth: s_actor_move_smooth,
        actor_set_physics: s_actor_set_physics,
        actor_perform_physics: s_actor_perform_physics,
        actor_event_bump: s_actor_event_bump,
        actor_event_detach: s_actor_event_detach,
        actor_event_attach: s_actor_event_attach,
        actor_event_base_change: s_actor_event_base_change,
        actor_event_lost_child: s_actor_event_lost_child,
        actor_event_gained_child: s_actor_event_gained_child,
        level_viewport0_actor_pri: s_level_viewport0_actor_pri,
        check_failed: s_check_failed,
        actor_event_touch: s_actor_event_touch,
        actor_event_untouch: s_actor_event_untouch,
        is_editor: s_is_editor,
        gcache_flush: s_gcache_flush,
        primitive_validate_actor: s_primitive_validate_actor,
        uobject_process_event: s_uobject_process_event,
        uobject_destroy: s_uobject_destroy,
        render_interface_destroy: s_render_interface_destroy,
        object_conditional_destroy: s_object_conditional_destroy,
        actor_load_localized: s_actor_load_localized,
        uobject_post_load: s_uobject_post_load,
        actor_event_possess: s_actor_event_possess,
        actor_log_possessed: s_actor_log_possessed,
        mesh_collision_extent: s_mesh_collision_extent,
        brush_collision_extent: s_brush_collision_extent,
        mesh_render_bounding_box: s_mesh_render_bounding_box,
        brush_render_bounding_box_coords: s_brush_render_bounding_box_coords,
        class_is_uclass_static: s_class_is_uclass_static,
        archive_preload: s_archive_preload,
        object_serialize_tagged_persistent: s_object_serialize_tagged_persistent,
        object_serialize_bin: s_object_serialize_bin,
        archive_count_bytes: s_archive_count_bytes,
        trace_line_texture_hit: s_trace_line_texture_hit,
        bsp_surf_decal_count: s_bsp_surf_decal_count,
        model_vector: s_model_vector,
        model_point: s_model_point,
        bsp_surf_decal: s_bsp_surf_decal,
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
    conn: usize,
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
        conn: rig.obj("conn", ""),
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
        (*l).base.engine = w.engine as _;
        (*l).hash = if hash { w.hash as _ } else { ptr::null_mut() };
        let ge = w.engine as *mut UGameEngine;
        (*ge).cylinder = w.prims[0] as _;
        (*ge).o_cylinder = w.prims[1] as _;
        (*ge).r#box = w.prims[2] as _;
        (*ge).oval_cylinder = w.prims[3] as _;
        (*ge).o_oval_cylinder = w.prims[4] as _;
        for a in [
            w.self_, w.other, w.owner, w.base, w.touch, w.zone, w.carcass, w.pawn,
        ] {
            let ac = a as *mut AActor;
            (*ac).x_level = w.level as _;
            (*ac).level = w.li as _;
            (*ac.cast::<UObject>()).class = w.class as _;
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
                    actor::set_collision(svc, a, bs[0] == b'1', bs[1] == b'1', bs[2] == b'1')
                        .expect("checked actor call");
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
                    actor::set_collision_size(svc, a, r as f32, h as f32)
                        .expect("checked actor call");
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
                actor::set_owner(svc, a, target as _).expect("checked actor call");
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
                    actor::set_base(svc, a, target as _, notify == 1).expect("checked actor call");
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
                actor::begin_touch(svc, a, o).expect("checked actor call");
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
                    actor::end_touch(svc, a, o, nn as i32).expect("checked actor call");
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
                let r = actor::is_overlapping(svc, a, o).expect("checked actor call");
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
            actor::actor_post_edit_change(svc, a).expect("checked actor call");
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
        unsafe { actor::zone_info_post_edit_change(svc).expect("checked actor call") };
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
                let p = actor::get_primitive(svc, a).expect("checked actor call");
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
                    actor::spawned(svc, a).expect("checked actor call");
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
            unsafe {
                (*(w.li as *mut ALevelInfo)).set_b_begun_play(begun != 0);
            }
            case(
                "process_event",
                idx,
                &format!("begun={begun} function={fnf}"),
            );
            let func = if fnf == 1 { w.func } else { 0 };
            unsafe {
                actor::process_event(svc, a, func as _, w.parms as _, ptr::null_mut())
                    .expect("checked actor call");
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
            actor::destroy(svc, a).expect("checked actor call");
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
                    let poly_ptr = if brush == 1 && polys == 1 { w.polys } else { 0 };
                    unsafe {
                        (*(w.class as *mut UClass)).class_flags =
                            if loc == 1 { CLASS_LOCALIZED } else { 0 };
                        (*a).collide_type = ct;
                        if brush == 1 {
                            (*a).brush = w.brush as _;
                            (*(w.brush as *mut UModel)).polys = poly_ptr as _;
                        }
                        case(
                            "post_load",
                            idx,
                            &format!(
                                "localized={loc} brush={brush} polys={polys} collide_type={ct}"
                            ),
                        );
                        let before = LOG.with(|l| l.borrow().len());
                        actor::post_load(svc, a).expect("checked actor call");
                        // `object_set_transactional` no longer calls through the hook table;
                        // splice its former log lines back in right where they used to sit
                        // (before the first `hook validate` line from `primitive_validate_actor`,
                        // or at the end if that line is absent). Check the *actual* post-call
                        // `object_flags` state rather than re-deriving the call condition, so a
                        // regression in `object_set_transactional` itself is still caught.
                        let mut inject = Vec::new();
                        if (*(w.brush as *mut UObject)).object_flags & RF_TRANSACTIONAL != 0 {
                            inject.push(format!("hook set_flags {} 1", nm(w.brush)));
                        }
                        if poly_ptr != 0
                            && (*(poly_ptr as *mut UObject)).object_flags & RF_TRANSACTIONAL != 0
                        {
                            inject.push(format!("hook set_flags {} 1", nm(poly_ptr)));
                        }
                        if !inject.is_empty() {
                            LOG.with(|l| {
                                let mut l = l.borrow_mut();
                                let at = l[before..]
                                    .iter()
                                    .position(|line| line.starts_with("hook validate "))
                                    .map_or(l.len(), |p| before + p);
                                l.splice(at..at, inject);
                            });
                        }
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
            unsafe {
                (*(w.player as *mut UPlayer)).actor = if prev == 1 {
                    w.other as _
                } else {
                    ptr::null_mut()
                };
            }
            unsafe {
                if pp == 1 {
                    (*p).player = w.player as _;
                }
                case(
                    "set_player",
                    idx,
                    &format!("prev_actor={prev} pawn_player={pp}"),
                );
                actor::set_player(svc, p, w.player as _).expect("checked actor call");
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
                actor::get_cylinder_extent(svc, a, &mut out).expect("checked actor call");
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
        actor::get_cylinder_extent(svc, a, &mut out).expect("checked actor call");
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
        actor::get_cylinder_extent(svc, a, &mut out).expect("checked actor call");
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
                    actor::get_render_bounding_box(svc, a, exact, &mut out)
                        .expect("checked actor call");
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
        unsafe {
            (*(w.ar as *mut FArchive)).ar_is_loading = loading;
            (*(w.ar as *mut FArchive)).ar_is_saving = saving;
            (*(w.ar as *mut FArchive)).ar_is_trans = trans;
            (*(w.class as *mut UClass)).properties_size = 96;
            case(
                "serialize_persistence",
                idx,
                &format!("loading={loading} saving={saving} trans={trans}"),
            );
            let before = LOG.with(|l| l.borrow().len());
            actor::serialize_persistence(svc, a, w.ar as _).expect("checked actor call");
            // `object_set_debug_serialize`, `archive_is_loading/is_saving/is_trans` and
            // `object_properties_size` no longer call through the hook table; splice their
            // former log lines back in at the positions they used to occupy. The debug-serialize
            // line is only spliced if the real `object_flags` bit was actually observed set
            // post-call, so a regression in `object_set_debug_serialize` itself is still caught.
            // The `is_loading`/`is_saving`/`is_trans` and `properties_size` lines are plain
            // reads with no flag to check directly, but their correctness is still verified
            // indirectly: a wrong field read would flip the `serialize_tagged`/`serialize_bin`
            // choice (still a real hook below) or change the size fed into the still-real
            // `hook count_bytes` line, either of which would fail the fixture comparison.
            LOG.with(|l| {
                let mut l = l.borrow_mut();
                if (*(w.self_ as *mut UObject)).object_flags & RF_DEBUG_SERIALIZE != 0 {
                    let p1 = l[before..]
                        .iter()
                        .position(|line| line.starts_with("hook uclass_static"))
                        .map_or(l.len(), |p| before + p);
                    l.insert(p1, format!("hook set_flags {} 1073741824", nm(w.self_)));
                }
                let p2 = l[before..]
                    .iter()
                    .position(|line| line.starts_with("hook serialize_"))
                    .map_or(l.len(), |p| before + p);
                let mut inject = vec![format!("hook is_loading {}", nm(w.ar))];
                if loading == 0 {
                    inject.push(format!("hook is_saving {}", nm(w.ar)));
                }
                if loading != 0 || saving != 0 {
                    inject.push(format!("hook is_trans {}", nm(w.ar)));
                }
                l.splice(p2..p2, inject);
                let p3 = l[before..]
                    .iter()
                    .position(|line| line.starts_with("hook count_bytes"))
                    .map_or(l.len(), |p| before + p);
                l.insert(p3, format!("hook properties_size {}", nm(w.class)));
            });
        }
        emit("res done=1");
    }
}

/// Points the level at the brush model and sets the surf/texture knobs.
fn set_trace_world(w: &W, kind: i32) {
    unsafe {
        (*(w.level as *mut ULevel)).model = w.brush as _;
        if kind >= 1 {
            (*(w.surf as *mut FBspSurf)).texture = w.tex[0] as _;
            (*(w.surf as *mut FBspSurf)).poly_flags = if kind == 2 { 0x10 } else { 0x1 };
        }
        if kind == 2 {
            (*(w.tex[0] as *mut UTexture)).poly_flags = 0x100;
        }
    }
    with_st(|s| {
        s.trace_kind = kind;
        s.surf = w.surf;
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
            let out = unsafe {
                actor::trace_texture(svc, a, &end, &start, &mut flags, dec == 1)
                    .expect("checked actor call")
            };
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
        let decal_owner = rig.obj("decal_owner", "");
        set_trace_world(&w, 1);
        let f = |i: usize| f32::from_bits(words[i]);
        let vert = |k: usize| [f(3 * k), f(3 * k + 1), f(3 * k + 2)];
        unsafe {
            (*(decal_owner as *mut AActor)).texture = w.tex[1] as _;
            (*(decal as *mut FDecal)).actor = decal_owner as _;
            (*(decal as *mut FDecal)).vertices =
                [fv3(vert(0)), fv3(vert(1)), fv3(vert(2)), fv3(vert(3))];
        }
        with_st(|s| {
            s.decal_count = 1;
            s.decal_ptr = decal;
            s.model_vec = [f(12), f(13), f(14)];
            s.model_pt = [0.0; 3];
            s.hit_loc = [f(15), f(16), f(17)];
        });
        let a = w.self_ as *mut AActor;
        let end = fv3([0.0, 0.0, -100.0]);
        let start = fv3([0.0, 0.0, 100.0]);
        let mut flags: i32 = 0;
        case("trace_decal_flip", idx, &knobs);
        let out = unsafe {
            actor::trace_texture(svc, a, &end, &start, &mut flags, true)
                .expect("checked actor call")
        };
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
            if c == 5 || c == 10 {
                (*l).base.demo_rec_driver = w.driver as _;
            }
            if c == 5 {
                (*(w.driver as *mut UDemoRecDriver)).base.server_connection = w.driver as _;
            }
            if c == 7 {
                (*li).net_mode = NM_CLIENT;
                (*l).base.net_driver = w.driver as _;
                (*(w.driver as *mut UNetDriver)).server_connection = w.conn as _;
                (*(w.conn as *mut UNetConnection)).average_frame_time = 0.011;
                with_st(|s| {
                    s.viewport_pri = w.self_;
                });
                (*pawn).base.player_replication_info = w.self_ as _;
                (*a.cast::<APlayerReplicationInfo>()).ping = 10;
            }
            case("net_receive", c, kind);
            actor::pre_net_receive(svc, a).expect("checked actor call");
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
            actor::post_net_receive(svc, a).expect("checked actor call");

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

#[test]
fn exec_set_collision_size_sets_width_and_success_result() {
    let svc = services();
    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.set_b_collide_actors(false);
    let mut result = 0;
    assert_eq!(
        unsafe {
            actor::exec_set_collision_size(&svc, &mut actor, 12.0, 24.0, 7.0, &mut result)
                .expect("checked actor call")
        },
        0
    );
    let (radius, height, width) = (
        actor.collision_radius,
        actor.collision_height,
        actor.collision_width,
    );
    assert_eq!((radius, height, width, result), (12.0, 24.0, 7.0, 1));
}

#[derive(Default)]
struct CollisionBoxContext {
    primitive: usize,
    actor: usize,
    world: i32,
    skeletal: bool,
}

unsafe extern "C" fn collision_box(
    context: *mut c_void,
    primitive: *mut c_void,
    actor: *mut c_void,
    world: i32,
    out: *mut c_void,
) -> i32 {
    let context = unsafe { &mut *context.cast::<CollisionBoxContext>() };
    context.primitive = primitive as usize;
    context.actor = actor as usize;
    context.world = world;
    let mut bounds: FBox = unsafe { std::mem::zeroed() };
    bounds.min = FVector {
        x: -1.0,
        y: -2.0,
        z: -3.0,
    };
    bounds.max = FVector {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    bounds.is_valid = 1;
    unsafe { out.cast::<FBox>().write_unaligned(bounds) };
    0
}

unsafe extern "C" fn mesh_is_skeletal(
    context: *mut c_void,
    _mesh: *mut c_void,
    out: *mut i32,
) -> i32 {
    let context = unsafe { &mut *context.cast::<CollisionBoxContext>() };
    unsafe { *out = i32::from(context.skeletal) };
    0
}

#[test]
fn world_collision_box_selects_visual_overrides_or_actor_primitive() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let svc = services();
    let mut context = CollisionBoxContext::default();
    let box_services = actor::Hp2ActorWorldCollisionBoxServices {
        context: (&mut context as *mut CollisionBoxContext).cast(),
        primitive_collision_bounding_box: collision_box,
        mesh_is_skeletal,
    };
    let actor = world.self_ as *mut AActor;
    unsafe {
        (*actor).mesh = world.mesh as _;
        (*actor).brush = world.brush as _;
    }
    let mut bounds: FBox = unsafe { std::mem::zeroed() };

    assert_eq!(
        unsafe {
            actor::get_world_collision_box(
                &svc,
                actor,
                1,
                (&mut bounds as *mut FBox).cast(),
                &box_services,
            )
            .expect("checked actor call")
        },
        0
    );
    assert_eq!(context.primitive, world.mesh);
    assert_eq!(context.actor, world.self_);
    assert_eq!(context.world, 1);

    unsafe { (*actor).mesh = ptr::null_mut() };
    assert_eq!(
        unsafe {
            actor::get_world_collision_box(
                &svc,
                actor,
                1,
                (&mut bounds as *mut FBox).cast(),
                &box_services,
            )
            .expect("checked actor call")
        },
        0
    );
    assert_eq!(context.primitive, world.brush);

    unsafe {
        (*actor).collide_type = actor::CT_ALIGNED_CYLINDER;
        (*actor).mesh = world.mesh as _;
    }
    assert_eq!(
        unsafe {
            actor::get_world_collision_box(
                &svc,
                actor,
                0,
                (&mut bounds as *mut FBox).cast(),
                &box_services,
            )
            .expect("checked actor call")
        },
        0
    );
    assert_eq!(context.primitive, world.prims[0]);
    assert_eq!(bounds.is_valid, 1);
}

#[test]
fn render_extent_uses_skeletal_sum_or_local_primitive_bounds() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let svc = services();
    let mut context = CollisionBoxContext::default();
    let box_services = actor::Hp2ActorWorldCollisionBoxServices {
        context: (&mut context as *mut CollisionBoxContext).cast(),
        mesh_is_skeletal,
        primitive_collision_bounding_box: collision_box,
    };
    let actor = world.self_ as *mut AActor;
    let mut extent: FVector = unsafe { std::mem::zeroed() };

    unsafe { (*actor).mesh = world.mesh as _ };
    assert_eq!(
        unsafe {
            actor::get_render_extent(
                &svc,
                actor,
                (&mut extent as *mut FVector).cast(),
                &box_services,
            )
            .expect("checked actor call")
        },
        0
    );
    assert_eq!((extent.x, extent.y, extent.z), (2.0, 4.0, 6.0));
    assert_eq!(context.primitive, world.mesh);
    assert_eq!(context.world, 0);

    unsafe {
        (*actor).mesh = ptr::null_mut();
        (*actor).brush = world.brush as _;
    }
    assert_eq!(
        unsafe {
            actor::get_render_extent(
                &svc,
                actor,
                (&mut extent as *mut FVector).cast(),
                &box_services,
            )
            .expect("checked actor call")
        },
        0
    );
    assert_eq!(context.primitive, world.brush);

    let mut skeletal: USkeletalMesh = unsafe { std::mem::zeroed() };
    skeletal.bounding_box_sum.min = FVector {
        x: -2.0,
        y: -3.0,
        z: -4.0,
    };
    skeletal.bounding_box_sum.max = FVector {
        x: 2.0,
        y: 3.0,
        z: 4.0,
    };
    assert!(!context.skeletal);
    context.skeletal = true;
    unsafe { (*actor).mesh = (&mut skeletal as *mut USkeletalMesh).cast() };
    assert_eq!(
        unsafe {
            actor::get_render_extent(
                &svc,
                actor,
                (&mut extent as *mut FVector).cast(),
                &box_services,
            )
            .expect("checked actor call")
        },
        0
    );
    assert_eq!((extent.x, extent.y, extent.z), (4.0, 6.0, 8.0));
    assert!(context.skeletal);
}

#[test]
fn script_set_base_default_notifies_actor() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let svc = services();
    let actor = world.self_ as *mut AActor;
    unsafe {
        actor::set_base(&svc, actor, world.base as *mut AActor, true).expect("checked actor call")
    };

    let events = LOG.with(|log| log.borrow().clone());
    assert!(
        events
            .iter()
            .any(|line| line == "hook event self BaseChange null"),
        "default script SetBase must notify the actor: {events:?}"
    );
}

#[derive(Default)]
struct FastTraceContext {
    actor: usize,
    trace_end: [f32; 3],
    trace_start: [f32; 3],
}

unsafe extern "C" fn script_fast_line_check(
    context: *mut c_void,
    actor: *mut c_void,
    trace_end: *const c_void,
    trace_start: *const c_void,
    out_result: *mut c_void,
) -> i32 {
    let context = unsafe { &mut *context.cast::<FastTraceContext>() };
    let trace_end = unsafe { trace_end.cast::<FVector>().read_unaligned() };
    let trace_start = unsafe { trace_start.cast::<FVector>().read_unaligned() };
    context.actor = actor as usize;
    context.trace_end = [trace_end.x, trace_end.y, trace_end.z];
    context.trace_start = [trace_start.x, trace_start.y, trace_start.z];
    unsafe { out_result.cast::<u32>().write_unaligned(0) };
    0
}

#[test]
fn fast_trace_forwards_end_and_start_and_preserves_blocked_result() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let actor = world.self_ as *mut AActor;
    let mut context = FastTraceContext::default();
    let services = actor::Hp2ActorFastTraceServices {
        context: (&mut context as *mut FastTraceContext).cast(),
        fast_line_check: script_fast_line_check,
    };
    let trace_end = FVector {
        x: 23.0,
        y: -8.0,
        z: 11.0,
    };
    let trace_start = FVector {
        x: -4.0,
        y: 6.0,
        z: 9.0,
    };
    let mut result = 1u32;
    assert_eq!(
        unsafe {
            actor::exec_fast_trace(
                actor,
                trace_end,
                trace_start,
                (&mut result as *mut u32).cast(),
                &services,
            )
        },
        0
    );
    assert_eq!(result, 0);
    assert_eq!(context.actor, world.self_);
    assert_eq!(context.trace_end, [23.0, -8.0, 11.0]);
    assert_eq!(context.trace_start, [-4.0, 6.0, 9.0]);
}

#[derive(Default)]
struct SoundDurationContext {
    calls: usize,
    sound: usize,
    duration: f32,
}

unsafe extern "C" fn script_get_sound_duration(
    context: *mut c_void,
    sound: *mut c_void,
    out_duration: *mut c_void,
) -> i32 {
    let context = unsafe { &mut *context.cast::<SoundDurationContext>() };
    context.calls += 1;
    context.sound = sound as usize;
    unsafe { out_duration.cast::<f32>().write_unaligned(context.duration) };
    0
}

#[test]
fn sound_duration_is_zero_without_sound_and_queried_with_sound() {
    let mut context = SoundDurationContext {
        duration: 7.75,
        ..SoundDurationContext::default()
    };
    let services = actor::Hp2ActorSoundDurationServices {
        context: (&mut context as *mut SoundDurationContext).cast(),
        get_duration: script_get_sound_duration,
    };
    let mut result = -1.0f32;
    assert_eq!(
        unsafe {
            actor::exec_get_sound_duration(
                std::ptr::null_mut(),
                (&mut result as *mut f32).cast(),
                &services,
            )
        },
        0
    );
    assert_eq!(result, 0.0);
    assert_eq!(context.calls, 0);

    let sound = 0x1000usize as *mut c_void;
    assert_eq!(
        unsafe {
            actor::exec_get_sound_duration(sound, (&mut result as *mut f32).cast(), &services)
        },
        0
    );
    assert_eq!(result, 7.75);
    assert_eq!(context.calls, 1);
    assert_eq!(context.sound, sound as usize);
}

#[test]
fn script_set_timer_resets_counter_and_replaces_timer_state() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let actor = world.self_ as *mut AActor;
    unsafe {
        (*actor).timer_counter = 12.5;
        (*actor).timer_rate = 3.0;
        (*actor).set_b_timer_loop(false);
        actor::exec_set_timer(actor, 2.25, 1);
        assert_eq!((*actor).timer_counter, 0.0);
        assert_eq!((*actor).timer_rate, 2.25);
        assert!((*actor).b_timer_loop());
        actor::exec_set_timer(actor, 0.0, 0);
        assert_eq!((*actor).timer_counter, 0.0);
        assert_eq!((*actor).timer_rate, 0.0);
        assert!(!(*actor).b_timer_loop());
    }
}

#[test]
fn is_owned_by_walks_self_and_ancestors_but_never_matches_null() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let self_actor = world.self_ as *mut AActor;
    let base = world.base as *mut AActor;
    let owner = world.owner as *mut AActor;
    let unrelated = world.other as *mut AActor;
    unsafe {
        (*self_actor).owner = base.cast();
        (*base).owner = owner.cast();

        assert_eq!(actor::is_owned_by(self_actor, self_actor), 1);
        assert_eq!(actor::is_owned_by(self_actor, base), 1);
        assert_eq!(actor::is_owned_by(self_actor, owner), 1);
        assert_eq!(actor::is_owned_by(self_actor, unrelated), 0);
        assert_eq!(actor::is_owned_by(self_actor, std::ptr::null()), 0);
        assert_eq!(actor::is_owned_by(std::ptr::null(), std::ptr::null()), 0);
    }
}

#[derive(Default)]
struct LatentContext {
    actor: usize,
    action: i32,
    calls: usize,
}

unsafe extern "C" fn set_latent_action(
    context: *mut c_void,
    actor: *mut c_void,
    action: i32,
) -> i32 {
    let context = unsafe { &mut *context.cast::<LatentContext>() };
    context.actor = actor as usize;
    context.action = action;
    context.calls += 1;
    0
}

#[test]
fn sleep_sets_latent_state_and_poll_awakens_only_below_half_a_tick() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let actor_ptr = world.self_ as *mut AActor;
    let mut context = LatentContext::default();
    let services = actor::Hp2ActorLatentServices {
        context: (&mut context as *mut LatentContext).cast(),
        set_latent_action,
    };

    unsafe {
        assert_eq!(actor::exec_sleep(actor_ptr, 1.25, 17, &services), 0);
        assert_eq!((*actor_ptr).latent_float, 1.25);
        assert_eq!(
            (context.actor, context.action, context.calls),
            (actor_ptr as usize, 17, 1)
        );

        (*actor_ptr).latent_float = 0.75;
        assert_eq!(actor::poll_sleep(actor_ptr, 0.5, &services), 0);
        assert_eq!((*actor_ptr).latent_float, 0.25);
        assert_eq!(context.calls, 1); // equality does not awaken: comparison is strict.

        assert_eq!(actor::poll_sleep(actor_ptr, 0.2, &services), 0);
        assert!(((*actor_ptr).latent_float - 0.05).abs() < f32::EPSILON);
        assert_eq!(
            (context.actor, context.action, context.calls),
            (actor_ptr as usize, 0, 2)
        );
    }
}

#[test]
fn finish_interpolation_sets_and_clears_latent_state_on_completion() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let actor_ptr = world.self_ as *mut AActor;
    let mut context = LatentContext::default();
    let services = actor::Hp2ActorLatentServices {
        context: (&mut context as *mut LatentContext).cast(),
        set_latent_action,
    };

    unsafe {
        assert_eq!(
            actor::exec_finish_interpolation(actor_ptr, 29, &services),
            0
        );
        assert_eq!(
            (context.actor, context.action, context.calls),
            (actor_ptr as usize, 29, 1)
        );

        (*actor_ptr).set_b_interpolating(true);
        assert_eq!(actor::poll_finish_interpolation(actor_ptr, &services), 0);
        assert_eq!(context.calls, 1);

        (*actor_ptr).set_b_interpolating(false);
        assert_eq!(actor::poll_finish_interpolation(actor_ptr, &services), 0);
        assert_eq!(
            (context.actor, context.action, context.calls),
            (actor_ptr as usize, 0, 2)
        );
    }
}

#[test]
fn poll_finish_anim_clears_latent_action_only_after_animation_is_finished() {
    let mut rig = Rig::default();
    let world = fresh(&mut rig, false);
    let actor_ptr = world.self_ as *mut AActor;
    let mut context = LatentContext::default();
    let services = actor::Hp2ActorLatentServices {
        context: (&mut context as *mut LatentContext).cast(),
        set_latent_action,
    };

    unsafe {
        (*actor_ptr).set_b_anim_finished(false);
        assert_eq!(actor::poll_finish_anim(actor_ptr, &services), 0);
        assert_eq!(context.calls, 0);

        (*actor_ptr).set_b_anim_finished(true);
        assert_eq!(actor::poll_finish_anim(actor_ptr, &services), 0);
        assert_eq!(
            (context.actor, context.action, context.calls),
            (actor_ptr as usize, 0, 1)
        );
    }
}

#[derive(Default)]
struct FinishAnimContext {
    bone_queries: usize,
    bone_index: i32,
}

unsafe extern "C" fn finish_mesh_is_skeletal(
    _context: *mut c_void,
    _mesh: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe { out.write(1) };
    0
}

unsafe extern "C" fn finish_bone_index(
    context: *mut c_void,
    _mesh: *mut c_void,
    _bone_name: *mut c_void,
    out: *mut i32,
) -> i32 {
    let context = unsafe { &mut *context.cast::<FinishAnimContext>() };
    context.bone_queries += 1;
    unsafe { out.write(context.bone_index) };
    0
}

#[test]
fn finish_anim_selects_matching_auxiliary_channel_and_schedules_completion() {
    let mut rig = Rig::default();
    let root = rig.obj("root", "AActor") as *mut AActor;
    let mesh = rig.obj("mesh", "USkeletalMesh") as *mut c_void;
    let channel = rig.obj("channel", "AAnimChannel") as *mut AActor;
    let mut channels = [channel.cast::<c_void>()];
    let mut bone = 1u32;
    let mut bone_context = FinishAnimContext {
        bone_index: 4,
        ..FinishAnimContext::default()
    };
    let bone_services = hp2_engine::actor_anim::Hp2ActorBoneNumberServices {
        context: (&mut bone_context as *mut FinishAnimContext).cast(),
        mesh_is_skeletal: finish_mesh_is_skeletal,
        bone_index: finish_bone_index,
    };
    let mut latent_context = LatentContext::default();
    let latent_services = actor::Hp2ActorLatentServices {
        context: (&mut latent_context as *mut LatentContext).cast(),
        set_latent_action,
    };

    unsafe {
        (*root).mesh = mesh;
        (*root).aux_anims.data = channels.as_mut_ptr().cast();
        (*root).aux_anims.num = channels.len() as i32;
        (*root).aux_anims.max = channels.len() as i32;

        (*channel).anim_bone = 4;
        (*channel).anim_sequence = [1, 0, 0, 0];
        (*channel).anim_rate = 1.0;
        (*channel).anim_frame = 0.0;
        (*channel).anim_last = 1.0;
        (*channel).set_b_anim_loop(true);
        (*channel).set_b_anim_finished(true);

        assert_eq!(
            hp2_engine::actor_anim::exec_finish_anim(
                root.cast(),
                (&mut bone as *mut u32).cast(),
                47,
                &bone_services,
                &latent_services,
            ),
            0
        );
        assert_eq!(bone_context.bone_queries, 1);
        assert!(!(*channel).b_anim_loop());
        assert!(!(*channel).b_anim_finished());
        assert_eq!(
            (
                latent_context.actor,
                latent_context.action,
                latent_context.calls
            ),
            (channel as usize, 47, 1)
        );

        let mut missing_bone_context = FinishAnimContext {
            bone_index: 9,
            ..FinishAnimContext::default()
        };
        let missing_bone_services = hp2_engine::actor_anim::Hp2ActorBoneNumberServices {
            context: (&mut missing_bone_context as *mut FinishAnimContext).cast(),
            mesh_is_skeletal: finish_mesh_is_skeletal,
            bone_index: finish_bone_index,
        };
        assert_eq!(
            hp2_engine::actor_anim::exec_finish_anim(
                root.cast(),
                (&mut bone as *mut u32).cast(),
                48,
                &missing_bone_services,
                &latent_services,
            ),
            0
        );
        assert_eq!(latent_context.calls, 1);
    }
}

/// Scripted `LevActServices` for `AActor::execMove`/`execSetLocation`/
/// `execSetRotation`/`execTrace`, which reach only the three `virtual_*`
/// leaves below (`hp2_engine::actor::exec_move`/`exec_set_location`/
/// `exec_set_rotation`/`exec_trace` call `levact::published_services()`
/// directly, bypassing `ULevel`). The other 41 table slots are never
/// reached by those four leaves and fail loudly (status 1) if they are.
mod levact_script {
    use std::cell::RefCell;
    use std::ffi::{c_char, c_void};
    use std::ptr;

    use hp2_abi::FVector;
    use hp2_abi::engine::generated::{FCheckResult, FPointRegion, FRotator};
    use hp2_engine::levact::{self, LevActServices};

    #[derive(Clone, Copy)]
    pub struct MoveActorCall {
        pub level: usize,
        pub actor: usize,
        pub delta: FVector,
        pub rotation: FRotator,
        pub test: i32,
        pub ignore_pawns: i32,
        pub ignore_bases: i32,
        pub no_fail: i32,
    }

    #[derive(Clone, Copy)]
    pub struct FarMoveCall {
        pub level: usize,
        pub actor: usize,
        pub destination: FVector,
        pub test: i32,
        pub no_check: i32,
    }

    #[derive(Clone, Copy)]
    pub struct SingleLineCall {
        pub level: usize,
        pub source: usize,
        pub end: FVector,
        pub start: FVector,
        pub trace_flags: u32,
        pub extent: FVector,
        pub extra_node_flags: u8,
    }

    pub struct Script {
        pub move_actor_calls: Vec<MoveActorCall>,
        pub move_actor_result: i32,
        pub far_move_calls: Vec<FarMoveCall>,
        pub far_move_result: i32,
        pub single_line_calls: Vec<SingleLineCall>,
        pub single_line_result: i32,
        pub single_line_actor: *mut c_void,
        pub single_line_location: FVector,
        pub single_line_normal: FVector,
    }

    impl Default for Script {
        fn default() -> Self {
            let zero = FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            Script {
                move_actor_calls: Vec::new(),
                move_actor_result: 0,
                far_move_calls: Vec::new(),
                far_move_result: 0,
                single_line_calls: Vec::new(),
                single_line_result: 0,
                single_line_actor: ptr::null_mut(),
                single_line_location: zero,
                single_line_normal: zero,
            }
        }
    }

    thread_local! {
        static SCRIPT: RefCell<Option<Script>> = const { RefCell::new(None) };
    }

    /// Resets this thread's script; every test must call this first so a
    /// previous test's leftover calls/results on the same worker thread
    /// never leak in.
    pub fn begin() {
        SCRIPT.with(|s| *s.borrow_mut() = Some(Script::default()));
    }

    pub fn with_script<R>(f: impl FnOnce(&mut Script) -> R) -> R {
        SCRIPT.with(|s| {
            f(s.borrow_mut()
                .as_mut()
                .expect("levact_script::begin() first"))
        })
    }

    unsafe extern "C" fn svc_move_actor(
        level: *mut c_void,
        actor: *mut c_void,
        delta: *const FVector,
        new_rotation: *const FRotator,
        _hit: *mut FCheckResult,
        test: i32,
        ignore_pawns: i32,
        ignore_bases: i32,
        no_fail: i32,
        result: *mut i32,
    ) -> i32 {
        with_script(|s| {
            s.move_actor_calls.push(MoveActorCall {
                level: level as usize,
                actor: actor as usize,
                delta: unsafe { *delta },
                rotation: unsafe { *new_rotation },
                test,
                ignore_pawns,
                ignore_bases,
                no_fail,
            });
            unsafe { *result = s.move_actor_result };
        });
        0
    }

    unsafe extern "C" fn svc_far_move_actor(
        level: *mut c_void,
        actor: *mut c_void,
        destination: *const FVector,
        test: i32,
        no_check: i32,
        result: *mut i32,
    ) -> i32 {
        with_script(|s| {
            s.far_move_calls.push(FarMoveCall {
                level: level as usize,
                actor: actor as usize,
                destination: unsafe { *destination },
                test,
                no_check,
            });
            unsafe { *result = s.far_move_result };
        });
        0
    }

    unsafe extern "C" fn svc_single_line_check(
        level: *mut c_void,
        hit: *mut FCheckResult,
        source: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        trace_flags: u32,
        extent: *const FVector,
        extra_node_flags: u8,
        result: *mut i32,
    ) -> i32 {
        with_script(|s| {
            s.single_line_calls.push(SingleLineCall {
                level: level as usize,
                source: source as usize,
                end: unsafe { *end },
                start: unsafe { *start },
                trace_flags,
                extent: unsafe { *extent },
                extra_node_flags,
            });
            unsafe {
                (*hit).actor = s.single_line_actor;
                (*hit).location = s.single_line_location;
                (*hit).normal = s.single_line_normal;
                *result = s.single_line_result;
            }
        });
        0
    }

    // The remaining 41 table slots are unreachable from
    // `exec_move`/`exec_set_location`/`exec_set_rotation`/`exec_trace`;
    // each fails with status 1 so an unexpected call is loud, not silent.
    unsafe extern "C" fn unused_pair(_: *mut c_void, _: *mut c_void) -> i32 {
        1
    }
    unsafe extern "C" fn unused_pair_const_result(
        _: *const c_void,
        _: *const c_void,
        result: *mut i32,
    ) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_pair_result(
        _: *mut c_void,
        _: *mut c_void,
        result: *mut i32,
    ) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_tab(_: i32) -> f32 {
        0.0
    }
    unsafe extern "C" fn unused_alloc(_: *mut c_void, _: i32, _: *mut *mut FCheckResult) -> i32 {
        1
    }
    unsafe extern "C" fn unused_model_point_check(
        _: *mut c_void,
        _: *mut FCheckResult,
        _: *const FVector,
        _: *const FVector,
        _: *mut i32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_model_line_check(
        _: *mut c_void,
        _: *mut FCheckResult,
        _: *const FVector,
        _: *const FVector,
        _: *const FVector,
        _: u32,
        _: *mut i32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_hash_point(
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *const FVector,
        _: u32,
        _: *mut *mut FCheckResult,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_hash_line(
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *const FVector,
        _: *const FVector,
        _: u32,
        _: *mut *mut FCheckResult,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_is_a(_: *const c_void, _: i32, result: *mut i32) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_check_failed(_: *const c_char, _: i32) -> i32 {
        1
    }
    unsafe extern "C" fn unused_u32_result(result: *mut u32) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_i32_result(result: *mut i32) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_point_region(
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *mut FPointRegion,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_encroachment_check(
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *const FRotator,
        _: u32,
        _: *mut *mut FCheckResult,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_end_touch(_: *mut c_void, _: *mut c_void, _: i32) -> i32 {
        1
    }
    unsafe extern "C" fn unused_collision_box(
        _: *mut c_void,
        _: *mut FVector,
        _: *mut FVector,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_move_smooth(
        _: *mut c_void,
        _: *const FVector,
        result: *mut i32,
    ) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_net_mode(_: *mut c_void, result: *mut u8) -> i32 {
        unsafe { *result = 0 };
        1
    }
    unsafe extern "C" fn unused_set_actor_zone(
        _: *mut c_void,
        _: *mut c_void,
        _: i32,
        _: i32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_multi_line_check(
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *const FVector,
        _: *const FVector,
        _: i32,
        _: *mut c_void,
        _: u8,
        _: *mut *mut FCheckResult,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_multi_point_check(
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *const FVector,
        _: u32,
        _: *mut c_void,
        _: i32,
        _: *mut *mut FCheckResult,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_single_point_check(
        _: *mut c_void,
        _: *mut FCheckResult,
        _: *const FVector,
        _: *const FVector,
        _: u32,
        _: *mut c_void,
        _: i32,
        _: *mut i32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_check_encroachment(
        _: *mut c_void,
        _: *mut c_void,
        _: *const FVector,
        _: *const FRotator,
        _: i32,
        _: *mut i32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_find_spot(
        _: *mut c_void,
        _: *const FVector,
        _: *mut FVector,
        _: i32,
        _: i32,
        _: *mut i32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn unused_adjust_spot(
        _: *mut c_void,
        _: *mut FVector,
        _: *const FVector,
        _: f32,
        _: *mut FCheckResult,
    ) -> i32 {
        1
    }

    static TABLE: LevActServices = LevActServices {
        alloc_check_results: unused_alloc,
        model_point_check: unused_model_point_check,
        model_line_check: unused_model_line_check,
        hash_actor_point_check: unused_hash_point,
        hash_actor_line_check: unused_hash_line,
        actor_is_a: unused_is_a,
        actor_is_owned_by: unused_pair_const_result,
        actor_is_blocked_by: unused_pair_const_result,
        check_failed: unused_check_failed,
        app_cycles: unused_u32_result,
        is_editor: unused_i32_result,
        bump_observer: unused_pair,
        sin_tab: unused_tab,
        cos_tab: unused_tab,
        model_point_region: unused_point_region,
        hash_add_actor: unused_pair,
        hash_remove_actor: unused_pair,
        hash_actor_encroachment_check: unused_encroachment_check,
        tracker_update: unused_pair,
        actor_begin_touch: unused_pair,
        actor_end_touch: unused_end_touch,
        actor_is_overlapping: unused_pair_result,
        actor_collision_box: unused_collision_box,
        actor_move_smooth: unused_move_smooth,
        event_bump: unused_pair,
        event_encroaching_on: unused_pair_result,
        event_encroached_by: unused_pair,
        event_zone_change: unused_pair,
        zone_event_actor_leaving: unused_pair,
        zone_event_actor_entered: unused_pair,
        pawn_event_foot_zone_change: unused_pair,
        pawn_event_head_zone_change: unused_pair,
        pri_set_player_zone: unused_pair,
        level_net_mode: unused_net_mode,
        virtual_set_actor_zone: unused_set_actor_zone,
        virtual_multi_line_check: unused_multi_line_check,
        virtual_multi_point_check: unused_multi_point_check,
        virtual_single_line_check: svc_single_line_check,
        virtual_single_point_check: unused_single_point_check,
        virtual_check_encroachment: unused_check_encroachment,
        virtual_move_actor: svc_move_actor,
        virtual_far_move_actor: svc_far_move_actor,
        virtual_find_spot: unused_find_spot,
        virtual_adjust_spot: unused_adjust_spot,
    };

    /// Publishes `TABLE` once per process; every scripted test reads/writes
    /// only its own thread's [`Script`], so one process-wide table is safe
    /// under default parallel test execution.
    pub fn ensure_published() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe { levact::publish_services(&TABLE, ptr::null_mut()) });
    }
}

/// Oracle: `git show 9e65d0a:HarryPotter2/Unreal/Engine/Src/UnScript.cpp`
/// `AActor::execMove`:
/// ```cpp
/// FCheckResult Hit(1.0f);
/// *(DWORD*)Result = GetLevel()->MoveActor( this, Delta, Rotation, Hit );
/// ```
/// `MoveActor`'s declared defaults (`UnLevel.h`) are
/// `Test=0, IgnorePawns=0, bIgnoreBases=0, bNoFail=0`.
#[test]
fn script_move_uses_move_actor_defaults() {
    levact_script::begin();
    levact_script::ensure_published();

    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.x_level = 0x1000 as *mut c_void;
    actor.rotation = FRotator {
        pitch: 11,
        yaw: 22,
        roll: 33,
    };
    let delta = FVector {
        x: 4.0,
        y: -5.0,
        z: 6.0,
    };
    levact_script::with_script(|s| s.move_actor_result = 1);

    let mut out_result: u32 = 0xffff_ffff;
    let status =
        unsafe { actor::exec_move(&mut actor, delta, (&mut out_result as *mut u32).cast()) };
    assert_eq!(status, 0);
    assert_eq!(out_result, 1, "UBOOL result must mirror MoveActor's return");

    levact_script::with_script(|s| {
        assert_eq!(s.move_actor_calls.len(), 1, "exactly one MoveActor call");
        let call = s.move_actor_calls[0];
        assert_eq!(call.level, 0x1000, "GetLevel() forwarded as the level arg");
        assert_eq!(call.actor, &actor as *const AActor as usize);
        assert_eq!((call.delta.x, call.delta.y, call.delta.z), (4.0, -5.0, 6.0));
        assert_eq!(
            (call.rotation.pitch, call.rotation.yaw, call.rotation.roll),
            (11, 22, 33),
            "execMove passes the actor's own Rotation, not a zero rotation"
        );
        assert_eq!(
            (
                call.test,
                call.ignore_pawns,
                call.ignore_bases,
                call.no_fail
            ),
            (0, 0, 0, 0),
            "MoveActor's declared defaults"
        );
    });
}

/// Oracle: `AActor::execSetLocation`:
/// ```cpp
/// *(DWORD*)Result = GetLevel()->FarMoveActor( this, NewLocation );
/// ```
/// `FarMoveActor`'s declared defaults (`UnLevel.h`) are
/// `Test=0, bNoCheck=0`.
#[test]
fn script_set_location_uses_far_move_defaults() {
    levact_script::begin();
    levact_script::ensure_published();

    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.x_level = 0x2000 as *mut c_void;
    let destination = FVector {
        x: -1.0,
        y: 2.0,
        z: -3.0,
    };
    levact_script::with_script(|s| s.far_move_result = 1);

    let mut out_result: u32 = 0xffff_ffff;
    let status = unsafe { actor::exec_set_location(&mut actor, &destination, &mut out_result) }
        .expect("checked actor call");
    assert_eq!(status, 0);
    assert_eq!(
        out_result, 1,
        "UBOOL result must mirror FarMoveActor's return"
    );

    levact_script::with_script(|s| {
        assert_eq!(s.far_move_calls.len(), 1, "exactly one FarMoveActor call");
        let call = s.far_move_calls[0];
        assert_eq!(call.level, 0x2000);
        assert_eq!(call.actor, &actor as *const AActor as usize);
        assert_eq!(
            (call.destination.x, call.destination.y, call.destination.z),
            (-1.0, 2.0, -3.0)
        );
        assert_eq!(
            (call.test, call.no_check),
            (0, 0),
            "FarMoveActor's declared defaults"
        );
    });
}

/// Oracle: `AActor::execSetRotation`:
/// ```cpp
/// FCheckResult Hit(1.0f);
/// *(DWORD*)Result = GetLevel()->MoveActor( this, FVector(0,0,0), NewRotation, Hit );
/// ```
/// Same `MoveActor` defaults as `execMove` (`Test=0, IgnorePawns=0,
/// bIgnoreBases=0, bNoFail=0`), but the delta is a zero vector and the
/// rotation is the script argument, not the actor's current `Rotation`.
#[test]
fn script_set_rotation_preserves_move_actor_defaults() {
    levact_script::begin();
    levact_script::ensure_published();

    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.x_level = 0x3000 as *mut c_void;
    actor.rotation = FRotator {
        pitch: 99,
        yaw: 98,
        roll: 97,
    };
    let new_rotation = FRotator {
        pitch: 1,
        yaw: 2,
        roll: 3,
    };
    levact_script::with_script(|s| s.move_actor_result = 1);

    let mut out_result: u32 = 0xffff_ffff;
    let status = unsafe {
        actor::exec_set_rotation(
            &mut actor,
            new_rotation,
            (&mut out_result as *mut u32).cast(),
        )
    };
    assert_eq!(status, 0);
    assert_eq!(out_result, 1, "UBOOL result must mirror MoveActor's return");

    levact_script::with_script(|s| {
        assert_eq!(s.move_actor_calls.len(), 1, "exactly one MoveActor call");
        let call = s.move_actor_calls[0];
        assert_eq!(call.level, 0x3000);
        assert_eq!(call.actor, &actor as *const AActor as usize);
        assert_eq!(
            (call.delta.x, call.delta.y, call.delta.z),
            (0.0, 0.0, 0.0),
            "execSetRotation always passes a zero delta"
        );
        assert_eq!(
            (call.rotation.pitch, call.rotation.yaw, call.rotation.roll),
            (1, 2, 3),
            "execSetRotation passes NewRotation, not the actor's current Rotation"
        );
        assert_eq!(
            (
                call.test,
                call.ignore_pawns,
                call.ignore_bases,
                call.no_fail
            ),
            (0, 0, 0, 0),
            "same MoveActor defaults as execMove"
        );
    });
}

/// Oracle: `AActor::execTrace`:
/// ```cpp
/// FCheckResult Hit(1.0f);
/// DWORD TraceFlags;
/// if( bTraceActors )
///     TraceFlags = TRACE_AllColliding;
/// else
///     TraceFlags = TRACE_VisBlocking;
///
/// GetLevel()->SingleLineCheck( Hit, this, TraceEnd, TraceStart, TraceFlags, TraceExtent );
/// *(AActor**)Result = Hit.Actor;
/// *HitLocation      = Hit.Location;
/// *HitNormal        = Hit.Normal;
/// ```
/// `TRACE_VisBlocking = TRACE_Level | TRACE_Movers` and
/// `TRACE_AllColliding = TRACE_BlockActors | TRACE_Movers | TRACE_Level |
/// TRACE_Others` (`UnLevel.h`).
#[test]
fn trace_selects_visibility_or_all_colliding_flags() {
    levact_script::begin();
    levact_script::ensure_published();

    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.x_level = 0x4000 as *mut c_void;
    let trace_end = FVector {
        x: 10.0,
        y: 20.0,
        z: 30.0,
    };
    let trace_start = FVector {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let trace_extent = FVector {
        x: 0.5,
        y: 0.5,
        z: 0.5,
    };
    let scripted_actor = 0x5000 as *mut c_void;
    let scripted_location = FVector {
        x: 7.0,
        y: 8.0,
        z: 9.0,
    };
    let scripted_normal = FVector {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };
    levact_script::with_script(|s| {
        s.single_line_result = 0;
        s.single_line_actor = scripted_actor;
        s.single_line_location = scripted_location;
        s.single_line_normal = scripted_normal;
    });

    // bTraceActors == 0 => TRACE_VisBlocking (TRACE_Level | TRACE_Movers).
    let mut out_actor: *mut c_void = ptr::null_mut();
    let mut out_location: FVector = unsafe { std::mem::zeroed() };
    let mut out_normal: FVector = unsafe { std::mem::zeroed() };
    let status = unsafe {
        actor::exec_trace(
            &mut actor,
            trace_end,
            trace_start,
            0,
            trace_extent,
            (&mut out_actor as *mut *mut c_void).cast(),
            (&mut out_location as *mut FVector).cast(),
            (&mut out_normal as *mut FVector).cast(),
        )
    };
    assert_eq!(status, 0);
    assert_eq!(
        out_actor, scripted_actor,
        "Hit.Actor must be written to Result"
    );
    assert_eq!(
        (out_location.x, out_location.y, out_location.z),
        (7.0, 8.0, 9.0),
        "Hit.Location must be written to *HitLocation"
    );
    assert_eq!(
        (out_normal.x, out_normal.y, out_normal.z),
        (0.0, 0.0, 1.0),
        "Hit.Normal must be written to *HitNormal"
    );

    // bTraceActors != 0 => TRACE_AllColliding.
    let status = unsafe {
        actor::exec_trace(
            &mut actor,
            trace_end,
            trace_start,
            1,
            trace_extent,
            (&mut out_actor as *mut *mut c_void).cast(),
            (&mut out_location as *mut FVector).cast(),
            (&mut out_normal as *mut FVector).cast(),
        )
    };
    assert_eq!(status, 0);

    levact_script::with_script(|s| {
        assert_eq!(s.single_line_calls.len(), 2, "one SingleLineCheck per call");
        let visibility = &s.single_line_calls[0];
        let all_colliding = &s.single_line_calls[1];
        assert_eq!(
            visibility.trace_flags,
            levact::TRACE_LEVEL | levact::TRACE_MOVERS,
            "bTraceActors == 0 selects TRACE_VisBlocking"
        );
        assert_eq!(
            all_colliding.trace_flags,
            levact::TRACE_ALL_COLLIDING,
            "bTraceActors != 0 selects TRACE_AllColliding"
        );
        for call in [visibility, all_colliding] {
            assert_eq!(call.level, 0x4000);
            assert_eq!(call.source, &actor as *const AActor as usize);
            assert_eq!((call.end.x, call.end.y, call.end.z), (10.0, 20.0, 30.0));
            assert_eq!((call.start.x, call.start.y, call.start.z), (1.0, 2.0, 3.0));
            assert_eq!(
                (call.extent.x, call.extent.y, call.extent.z),
                (0.5, 0.5, 0.5)
            );
            assert_eq!(
                call.extra_node_flags, 0,
                "execTrace never sets extra node flags"
            );
        }
    });
}

mod play_anim_contracts {
    use super::*;
    use hp2_abi::engine::generated::{FMeshAnimSeq, FMeshBone, TArrayRaw};
    use hp2_engine::actor_anim::{Hp2ActorAnimServices, Hp2ActorPlayAnimServices, play_anim};

    const SEQUENCE: u32 = 17;
    const MOVE: u32 = 23;
    const BONE: u32 = 29;

    #[derive(Default)]
    struct Context {
        skeletal: bool,
        sequence: *mut c_void,
        channel: *mut c_void,
        protected: Vec<usize>,
        bone: i32,
        events: Vec<&'static str>,
        destroyed: Vec<i32>,
        created: Option<(usize, i32, u32)>,
        played: Option<(usize, u32, i32, f32, f32, f32)>,
        channel_result: i32,
        logs: Vec<(usize, u32, i32)>,
        fail: &'static str,
    }

    unsafe fn context<'a>(p: *mut c_void, event: &'static str) -> (&'a mut Context, i32) {
        let c = unsafe { &mut *p.cast::<Context>() };
        c.events.push(event);
        let status = if c.fail == event { 91 } else { 0 };
        (c, status)
    }

    unsafe extern "C" fn skeletal(p: *mut c_void, _: *mut c_void, out: *mut i32) -> i32 {
        let (c, status) = unsafe { context(p, "skeletal") };
        unsafe {
            out.write(i32::from(c.skeletal));
        }
        status
    }

    unsafe extern "C" fn animation(p: *mut c_void, _: *mut c_void, out: *mut *mut c_void) -> i32 {
        let (_, status) = unsafe { context(p, "animation") };
        unsafe {
            out.write(ptr::null_mut());
        }
        status
    }

    unsafe extern "C" fn default_animation(
        p: *mut c_void,
        _: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        let (_, status) = unsafe { context(p, "default") };
        unsafe {
            out.write(0x4000usize as *mut c_void);
        }
        status
    }

    unsafe extern "C" fn find(
        p: *mut c_void,
        _: *mut c_void,
        name: *mut c_void,
        _: i32,
        out: *mut *mut c_void,
    ) -> i32 {
        let (c, status) = unsafe { context(p, "find") };
        unsafe {
            out.write(if name.cast::<u32>().read_unaligned() == SEQUENCE {
                c.sequence
            } else {
                ptr::null_mut()
            });
        }
        status
    }

    unsafe extern "C" fn root_move(p: *mut c_void, name: *mut c_void, out: *mut i32) -> i32 {
        let (_, status) = unsafe { context(p, "move") };
        unsafe {
            out.write(i32::from(name.cast::<u32>().read_unaligned() == MOVE));
        }
        status
    }

    unsafe extern "C" fn bone(
        p: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let (c, status) = unsafe { context(p, "bone") };
        unsafe {
            out.write(c.bone);
        }
        status
    }

    unsafe extern "C" fn protected(p: *mut c_void, channel: *mut c_void, out: *mut i32) -> i32 {
        let (c, status) = unsafe { context(p, "protected") };
        unsafe {
            out.write(i32::from(c.protected.contains(&(channel as usize))));
        }
        status
    }

    unsafe extern "C" fn load(p: *mut c_void) -> i32 {
        unsafe { context(p, "load").1 }
    }

    unsafe extern "C" fn create(
        p: *mut c_void,
        actor: *mut c_void,
        kind: i32,
        name: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        let (c, status) = unsafe { context(p, "create") };
        unsafe {
            c.created = Some((actor as usize, kind, name.cast::<u32>().read_unaligned()));
            out.write(c.channel);
        }
        status
    }

    unsafe extern "C" fn destroy(p: *mut c_void, actor: *mut c_void, index: i32) -> i32 {
        let (c, status) = unsafe { context(p, "destroy") };
        if status != 0 {
            return status;
        }
        c.destroyed.push(index);
        unsafe {
            let actor = actor.cast::<AActor>();
            let mut aux = ptr::addr_of!((*actor).aux_anims).read_unaligned();
            let data = aux.data.cast::<*mut AActor>();
            ptr::copy(
                data.add(index as usize + 1),
                data.add(index as usize),
                (aux.num - index - 1) as usize,
            );
            aux.num -= 1;
            (*actor).aux_anims = aux;
        }
        0
    }

    unsafe extern "C" fn play(
        p: *mut c_void,
        channel: *mut c_void,
        name: *mut c_void,
        looping: i32,
        rate: f32,
        tween: f32,
        min: f32,
        out: *mut i32,
    ) -> i32 {
        let (c, status) = unsafe { context(p, "play") };
        unsafe {
            c.played = Some((
                channel as usize,
                name.cast::<u32>().read_unaligned(),
                looping,
                rate,
                tween,
                min,
            ));
            out.write(c.channel_result);
        }
        status
    }

    unsafe extern "C" fn log(
        p: *mut c_void,
        actor: *mut c_void,
        name: *mut c_void,
        missing_mesh: i32,
    ) -> i32 {
        let (c, status) = unsafe { context(p, "log") };
        c.logs.push((
            actor as usize,
            unsafe { name.cast::<u32>().read_unaligned() },
            missing_mesh,
        ));
        status
    }

    struct Case {
        actor: Box<AActor>,
        sequence: Box<FMeshAnimSeq>,
        context: Context,
        channels: Vec<Box<AActor>>,
        pointers: Vec<*mut AActor>,
        _mesh: Box<USkeletalMesh>,
        _bones: Box<[FMeshBone; 3]>,
    }

    impl Case {
        fn new() -> Self {
            // `bones[2].num_children = 3` reproduces this harness's former
            // mock `bone_is_subset` range (`child >= parent && child <
            // parent + 3`) for `Context::bone`'s default (2), now that
            // `bone_is_subset` reads a real `USkeletalMesh::RefSkeleton`
            // TArray instead of a callback.
            let mut bones: Box<[FMeshBone; 3]> = Box::new(unsafe { std::mem::zeroed() });
            bones[2].num_children = 3;
            let mut mesh: Box<USkeletalMesh> = Box::new(unsafe { std::mem::zeroed() });
            mesh.ref_skeleton = TArrayRaw {
                data: bones.as_mut_ptr().cast(),
                num: bones.len() as i32,
                max: bones.len() as i32,
            };
            let mut actor: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
            actor.mesh = (&mut *mesh as *mut USkeletalMesh).cast();
            let mut sequence: Box<FMeshAnimSeq> = Box::new(unsafe { std::mem::zeroed() });
            sequence.num_frames = 4;
            sequence.rate = 8.0;
            sequence.notifys.num = 1;
            let context = Context {
                sequence: (&mut *sequence as *mut FMeshAnimSeq).cast(),
                channel_result: 1,
                bone: 2,
                ..Context::default()
            };
            Self {
                actor,
                sequence,
                context,
                channels: Vec::new(),
                pointers: Vec::new(),
                _mesh: mesh,
                _bones: bones,
            }
        }

        fn channel(&mut self, bone: u8, transient: bool, protected: bool) {
            let mut channel: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
            channel.anim_sequence = SEQUENCE.to_ne_bytes();
            channel.anim_bone = bone;
            channel.set_b_anim_transient(transient);
            let p = &mut *channel as *mut AActor;
            if protected {
                self.context.protected.push(p as usize);
            }
            self.channels.push(channel);
            self.pointers.push(p);
            self.actor.aux_anims = TArrayRaw {
                data: self.pointers.as_mut_ptr().cast(),
                num: self.pointers.len() as i32,
                max: self.pointers.capacity() as i32,
            };
        }

        fn run(
            &mut self,
            mut name: u32,
            looping: i32,
            rate: f32,
            tween: f32,
            min: f32,
            kind: i32,
            mut root: u32,
        ) -> (i32, i32) {
            let p = (&mut self.context as *mut Context).cast();
            let animation = Hp2ActorAnimServices {
                context: p,
                mesh_is_skeletal: skeletal,
                actor_skeletal_animation: animation,
                mesh_default_animation: default_animation,
                find_sequence: find,
            };
            let services = Hp2ActorPlayAnimServices {
                context: p,
                animation: &animation,
                root_is_move: root_move,
                bone_index: bone,
                channel_not_replaceable: protected,
                load_channel_class: load,
                create_channel: create,
                destroy_channel: destroy,
                play_channel: play,
                log,
            };
            let mut result = -1;
            let status = unsafe {
                play_anim(
                    (&mut *self.actor as *mut AActor).cast(),
                    (&mut name as *mut u32).cast(),
                    looping,
                    rate,
                    tween,
                    min,
                    kind,
                    (&mut root as *mut u32).cast(),
                    &mut result,
                    &services,
                )
            };
            (status, result)
        }

        fn sim(&self) -> FPlane {
            unsafe { ptr::addr_of!(self.actor.sim_anim).read_unaligned() }
        }
    }

    #[test]
    fn play_anim_defaults_tween_modes_and_replication_sentinel() {
        for (tween, tween_rate, alpha) in [
            (-1.0, 2.0, 0.0),
            (0.25, 4.0, 0.0),
            (0.0, 0.0, 1.0),
            (f32::NAN, 0.0, 1.0),
        ] {
            let mut c = Case::new();
            c.actor.set_b_anim_finished(true);
            c.actor.set_b_anim_by_owner(true);
            c.actor.anim_frame = 0.7;
            assert_eq!(c.run(SEQUENCE, 0, 1.0, tween, 0.0, 0, 0), (0, 1));
            assert_eq!(c.actor.anim_sequence, SEQUENCE.to_ne_bytes());
            assert_eq!(
                (c.actor.anim_rate, c.actor.anim_last, c.actor.anim_min_rate),
                (2.0, 0.75, 0.0)
            );
            assert_eq!(
                (c.actor.anim_frame, c.actor.tween_rate, c.actor.tween_alpha),
                (0.0, tween_rate, alpha)
            );
            assert!(c.actor.b_anim_notify());
            assert!(!c.actor.b_anim_loop());
            assert!(!c.actor.b_anim_finished());
            assert!(c.actor.b_anim_by_owner());
            assert_eq!(
                c.sim(),
                FPlane {
                    x: 0.0,
                    y: 10000.0,
                    z: 1000.0 * tween_rate,
                    w: -7500.0
                }
            );
            assert_eq!(c.run(SEQUENCE, 0, 1.0, tween, 0.0, 0, 0), (0, 1));
            assert_eq!(c.sim().w, -7499.0);
            assert_eq!(c.run(SEQUENCE, 0, 1.0, tween, 0.0, 0, 0), (0, 1));
            assert_eq!(c.sim().w, -7500.0);
        }
    }

    #[test]
    fn play_anim_static_none_and_tween_only_preserve_legacy_bits() {
        let mut c = Case::new();
        c.sequence.num_frames = 1;
        assert_eq!(c.run(SEQUENCE, 1, 3.0, 0.5, 0.2, 1, 0), (0, 1));
        assert_eq!(
            (c.actor.anim_last, c.actor.anim_rate, c.actor.anim_min_rate),
            (0.0, 0.0, 0.0)
        );
        assert!(c.actor.b_anim_loop());
        assert!(!c.actor.b_anim_notify());
        assert_eq!((c.actor.tween_rate, c.actor.tween_alpha), (2.0, 0.0));
        // Even a one-frame loop takes the fast path while still tweening.
        assert_eq!(c.run(SEQUENCE, 1, 3.0, 9.0, 0.25, 1, 0), (0, 1));
        assert_eq!(
            (c.actor.anim_rate, c.actor.anim_min_rate, c.actor.tween_rate),
            (24.0, 2.0, 2.0)
        );
        c.actor.set_b_anim_notify(true);
        assert_eq!(c.run(0, 1, 7.0, 0.0, 3.0, 1, 0), (0, 1));
        assert_eq!(c.actor.anim_sequence, [0; 4]);
        assert!(!c.actor.b_anim_loop());
        assert!(!c.actor.b_anim_notify());
        assert_eq!(
            (c.actor.anim_rate, c.actor.tween_rate, c.actor.tween_alpha),
            (0.0, 0.0, 1.0)
        );
        assert_eq!(c.run(0, 1, 7.0, 0.0, 3.0, 1, 0), (0, 1));
        assert_eq!(c.sim().w, 0.0); // Input bLoop, not the cleared bAnimLoop bit.
        c.sequence.num_frames = 4;
        assert_eq!(c.run(SEQUENCE, 0, 0.0, 0.5, 0.0, 0, 0), (0, 1));
        assert_eq!(
            (c.actor.anim_rate, c.actor.anim_last, c.actor.tween_rate),
            (0.0, 0.75, 2.0)
        );
        assert!(c.actor.b_anim_notify());
    }

    #[test]
    fn play_anim_existing_loop_keeps_progress_and_does_not_clamp_replication() {
        let mut c = Case::new();
        assert_eq!(c.run(SEQUENCE, 1, 10.0, 0.25, 0.5, 0, 0), (0, 1));
        assert_eq!(c.sim().y, 32767.0);
        c.actor.anim_frame = 0.4;
        c.actor.anim_last = 0.6;
        c.actor.tween_alpha = 0.3;
        c.actor.set_b_anim_notify(false);
        c.actor.set_b_anim_finished(true);
        c.actor.sim_anim = FPlane {
            x: 9.0,
            y: 100000.0,
            z: 11.0,
            w: -7500.0,
        };
        assert_eq!(c.run(SEQUENCE, 1, 10.0, 9.0, 0.25, 0, 0), (0, 1));
        assert_eq!(
            (
                c.actor.anim_frame,
                c.actor.anim_last,
                c.actor.tween_rate,
                c.actor.tween_alpha
            ),
            (0.4, 0.6, 4.0, 0.3)
        );
        assert_eq!((c.actor.anim_rate, c.actor.anim_min_rate), (20.0, 0.5));
        assert!(!c.actor.b_anim_notify());
        assert!(!c.actor.b_anim_finished());
        assert_eq!(
            c.sim(),
            FPlane {
                x: 9.0,
                y: 100000.0,
                z: 11.0,
                w: -7499.0
            }
        );
        // A stopped loop must restart, rather than take the fast path.
        c.actor.anim_rate = 0.0;
        c.actor.tween_rate = 0.0;
        assert_eq!(c.run(SEQUENCE, 1, 1.0, -1.0, 0.0, 1, 0), (0, 1));
        assert_eq!(
            (c.actor.anim_frame, c.actor.anim_last, c.actor.tween_alpha),
            (0.0, 0.75, 0.0)
        );
        assert!(c.actor.b_anim_notify());
        // Negative replication rates are not clamped from below.
        assert_eq!(c.run(SEQUENCE, 0, -10.0, 0.0, -0.5, 1, 0), (0, 1));
        assert_eq!((c.sim().y, c.actor.anim_min_rate), (-100000.0, -1.0));
    }

    #[test]
    fn play_anim_move_and_classic_bones_use_whole_skeleton() {
        let mut c = Case::new();
        c.context.skeletal = true;
        assert_eq!(c.run(SEQUENCE, 0, 1.0, -1.0, 0.0, 1, MOVE), (0, 1));
        assert!(c.actor.b_anim_move());
        assert!(!c.context.events.contains(&"load"));
        assert_eq!(
            c.context.events,
            [
                "skeletal",
                "move",
                "skeletal",
                "animation",
                "default",
                "find"
            ]
        );
        c.context.skeletal = false;
        assert_eq!(c.run(SEQUENCE, 0, 1.0, -1.0, 0.0, 1, BONE), (0, 1));
        assert!(!c.actor.b_anim_move());
        assert!(!c.context.events.contains(&"bone"));
        assert!(!c.context.events.contains(&"create"));
    }

    #[test]
    fn play_anim_channel_dispatch_preserves_arguments_and_boolean_result() {
        for result in [0, 1] {
            let mut c = Case::new();
            c.context.skeletal = true;
            c.context.channel = 0x2000usize as *mut c_void;
            c.context.channel_result = result;
            c.actor.anim_frame = 0.6;
            assert_eq!(c.run(SEQUENCE, 1, 1.5, 0.25, 0.1, 1, BONE), (0, result));
            assert_eq!(
                c.context.created,
                Some((&*c.actor as *const AActor as usize, 1, BONE))
            );
            assert_eq!(
                c.context.played,
                Some((0x2000, SEQUENCE, 1, 1.5, 0.25, 0.1))
            );
            assert_eq!(
                c.context.events,
                ["skeletal", "move", "load", "create", "play"]
            );
            assert_eq!({ c.actor.anim_frame }, 0.6);
        }
        let mut c = Case::new();
        c.context.skeletal = true;
        c.channel(2, true, false);
        assert_eq!(c.run(SEQUENCE, 0, 1.0, -1.0, 0.0, 0, BONE), (0, 1));
        assert_eq!(c.context.destroyed, [0]);
        assert_eq!(c.actor.anim_sequence, SEQUENCE.to_ne_bytes());
        assert!(
            c.context
                .events
                .iter()
                .position(|e| *e == "create")
                .unwrap()
                < c.context
                    .events
                    .iter()
                    .position(|e| *e == "destroy")
                    .unwrap()
        );
    }

    #[test]
    fn play_anim_subset_removal_honors_protection_persistence_and_bounds() {
        let mut c = Case::new();
        c.context.skeletal = true;
        c.channel(1, true, false); // Below the subset.
        c.channel(2, true, false);
        c.channel(3, false, false);
        c.channel(4, true, true);
        c.channel(5, true, false); // At the exclusive upper bound.
        c.actor.anim_sequence = 99u32.to_ne_bytes();
        assert_eq!(c.run(0, 0, 1.0, -1.0, 0.0, 0, BONE), (0, 1));
        assert_eq!(c.context.destroyed, [1]);
        assert_eq!({ c.actor.aux_anims.num }, 4);
        assert_eq!(c.channels[2].anim_sequence, [0; 4]);
        for i in [0, 3, 4] {
            assert_eq!(c.channels[i].anim_sequence, SEQUENCE.to_ne_bytes());
        }
        assert_eq!(c.actor.anim_sequence, 99u32.to_ne_bytes());
        assert!(!c.context.events.contains(&"find"));
        let expected = [0, 2, 3, 4].map(|i| (&*c.channels[i] as *const AActor).cast_mut());
        assert_eq!(c.pointers[..4], expected);
        c.context.bone = -1;
        c.context.events.clear();
        assert_eq!(c.run(0, 0, 1.0, -1.0, 0.0, 0, BONE), (0, 0));
        assert_eq!(c.context.events, ["skeletal", "move", "bone"]);
    }

    #[test]
    fn play_anim_replace_cleanup_precedes_lookup_and_combine_keeps_channels() {
        let mut c = Case::new();
        c.channel(0, true, false);
        c.channel(1, false, false);
        c.channel(2, true, true);
        c.channel(3, false, true);
        c.channel(4, true, false);
        assert_eq!(c.run(SEQUENCE, 0, 1.0, -1.0, 0.0, 1, 0), (0, 1));
        assert!(!c.context.events.contains(&"protected"));
        c.context.sequence = ptr::null_mut();
        c.actor.anim_frame = 0.4;
        assert_eq!(c.run(SEQUENCE, 0, 1.0, -1.0, 0.0, 0, 0), (0, 0));
        assert_eq!(c.context.destroyed, [4, 0]);
        assert_eq!({ c.actor.aux_anims.num }, 3);
        assert_eq!(c.channels[1].anim_sequence, [0; 4]);
        assert_eq!(c.channels[2].anim_sequence, SEQUENCE.to_ne_bytes());
        assert_eq!(c.channels[3].anim_sequence, SEQUENCE.to_ne_bytes());
        assert_eq!({ c.actor.anim_frame }, 0.4);
        assert_eq!(
            c.context.logs,
            [(&*c.actor as *const AActor as usize, SEQUENCE, 0)]
        );
        c.context.events.clear();
        c.actor.mesh = ptr::null_mut();
        c.actor.set_b_anim_move(true);
        assert_eq!(c.run(SEQUENCE, 0, 1.0, -1.0, 0.0, 0, 0), (0, 0));
        assert!(c.actor.b_anim_move());
        assert_eq!(c.context.events, ["log"]);
        assert_eq!(c.context.logs.last().unwrap().2, 1);
    }

    #[test]
    fn play_anim_callback_failures_stop_at_the_failing_effect() {
        for fail in [
            "skeletal",
            "move",
            "animation",
            "default",
            "find",
            "load",
            "create",
            "play",
            "bone",
            "protected",
            "destroy",
            "log",
        ] {
            let mut c = Case::new();
            c.context.skeletal = true;
            c.context.fail = fail;
            c.channel(2, true, false);
            let (name, root) = match fail {
                "load" | "create" | "play" => {
                    c.context.channel = 0x2000usize as *mut c_void;
                    (SEQUENCE, BONE)
                }
                "bone" => (0, BONE),
                "log" => {
                    c.actor.mesh = ptr::null_mut();
                    (SEQUENCE, 0)
                }
                _ => (SEQUENCE, 0),
            };
            let (status, _) = c.run(name, 0, 1.0, -1.0, 0.0, 0, root);
            assert_eq!(status, 91, "{fail}");
            assert_eq!(c.context.events.last(), Some(&fail), "{fail}");
            assert_eq!(c.actor.anim_sequence, [0; 4], "{fail}");
        }
    }

    #[test]
    fn play_anim_restart_boundaries_and_unchecked_frame_arithmetic() {
        for (same_sequence, previous_loop) in [(false, true), (true, false)] {
            let mut c = Case::new();
            c.actor.anim_sequence = if same_sequence { SEQUENCE } else { 99 }.to_ne_bytes();
            c.actor.set_b_anim_loop(previous_loop);
            c.actor.anim_rate = 1.0;
            c.actor.anim_frame = 0.6;
            c.sequence.notifys.num = 0;
            assert_eq!(c.run(SEQUENCE, 1, 1.0, 0.0, 0.0, 1, 0), (0, 1));
            assert_eq!((c.actor.anim_frame, c.actor.tween_alpha), (0.0, 1.0));
            assert!(!c.actor.b_anim_notify());
        }
        let mut c = Case::new();
        c.sequence.num_frames = 0;
        assert_eq!(c.run(SEQUENCE, 0, 1.0, 0.0, 0.0, 1, 0), (0, 1));
        assert_eq!({ c.actor.anim_rate }, f32::INFINITY);
        assert_eq!({ c.actor.anim_last }, f32::NEG_INFINITY);
        assert_eq!(c.sim().y, 32767.0);
        assert_eq!(c.sim().w, f32::INFINITY);
    }
}

unsafe extern "C" fn checked_end_touch(
    actor: *mut c_void,
    other: *mut c_void,
    no_notify: i32,
) -> i32 {
    unsafe { hp2_engine::actor::end_touch(&services(), actor.cast(), other.cast(), no_notify) }
        .map_or_else(|status| status, |()| 0)
}
unsafe extern "C" fn checked_touch_to(
    actor: *mut c_void,
    other: *mut c_void,
    out_value: *mut i32,
) -> i32 {
    match unsafe { hp2_engine::actor::touch_to(&services(), actor.cast(), other.cast()) } {
        Ok(value) => {
            unsafe { out_value.write(value) };
            0
        }
        Err(status) => status,
    }
}
unsafe extern "C" fn checked_remove_root(ri: *mut c_void) -> i32 {
    emit(format!("hook remove_from_root {}", nm(ri as usize)));
    0
}
