//! LevelCore: Core `ULevelBase` / `ULevel` functions from
//! `HarryPotter2/Unreal/Engine/Src/UnLevel.cpp` (oracle ref `7ddb1d3`).
//!
//! The C++ shells in `UnLevel.cpp` forward to the exports in
//! `crates/hp2-ffi/src/engine_level.rs`, which call the functions here with the
//! [`Hp2LevelCoreServices`] table defined in `Hp2LevelServices.cpp`. Control flow
//! follows the original line by line. Each service wraps one original C++ call or
//! expression, in original order.
//!
//! Scalar fields of `ULevel`, `AActor` and `FCheckResult` are read and written
//! through the generated mirrors in `hp2_abi::engine::generated`.
//!
//! Divergences (also marked `// Divergence:` at the call sites):
//! - Nested `guard(...)` labels inside Core bodies (StartCollision, EndCollision,
//!   MatchIdentical, MatchEditorOther, KillViews) are not reproduced. They only
//!   label crash reports; the outer labels stay in the C++ shells.
//! - `FCheckResult` has a C++ constructor, so the C++ shell owns the `FirstHit`
//!   and `Hit` locals and passes pointers.
//! - `check(...)` failures are captured in C++ and propagated back through the export status.
//! - `ZoneDist[Zone1][Zone2]` is read through raw pointer arithmetic with no bounds
//!   check, as in the original.
//! - `appRound` is `as i32`; out-of-range floats saturate in Rust where C is UB.

use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FCheckResult, ULevel};

/// Opaque engine or C++ object pointer.
pub type Obj = *mut c_void;

/// Mirror of `FActorSlotCompactIndexTrace` (`Core/Inc/UnArc.h:17`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FActorSlotCompactIndexTrace {
    pub index: i32,
    pub offset_after: i32,
    pub captured: i32,
}

/// C++ services used by the LevelCore port. Field order matches
/// `MakeCoreServices()` in `Hp2LevelServices.cpp`.
#[repr(C)]
pub struct Hp2LevelCoreServices {
    // FString handles (C++ `FString` locals and temporaries).
    /// `new FString()`; the handle is released with `fstring_delete`.
    pub fstring_new: unsafe extern "C" fn() -> Obj,
    /// `delete` an FString handle.
    pub fstring_delete: unsafe extern "C" fn(s: Obj),
    /// `dst = src.Caps()`. `dst` may equal `src`.
    pub fstring_caps: unsafe extern "C" fn(dst: Obj, src: Obj),
    /// `s.Len()`.
    pub fstring_len: unsafe extern "C" fn(s: Obj) -> i32,
    /// `*s` (pointer to the NUL-terminated TCHAR buffer).
    pub fstring_chars: unsafe extern "C" fn(s: Obj) -> *const u32,
    /// `dst = src` for a TCHAR literal or buffer.
    pub fstring_assign_chars: unsafe extern "C" fn(dst: Obj, src: *const u32),
    /// `dst = src.Mid(start, count)`. `dst` may equal `src`.
    pub fstring_mid: unsafe extern "C" fn(dst: Obj, src: Obj, start: i32, count: i32),
    /// `s.InStr(sub)` with a TCHAR literal.
    pub fstring_instr_chars: unsafe extern "C" fn(s: Obj, sub: *const u32) -> i32,
    /// `s.InStr(sub)` with an FString.
    pub fstring_instr_fstring: unsafe extern "C" fn(s: Obj, sub: Obj) -> i32,
    /// `s == TEXT(literal)`.
    pub fstring_eq_chars: unsafe extern "C" fn(s: Obj, lit: *const u32) -> i32,
    /// `a == b`.
    pub fstring_eq_fstring: unsafe extern "C" fn(a: Obj, b: Obj) -> i32,

    // Diagnostics and globals.
    /// `debugf(kind, TEXT("%s"), msg)`. `kind`: 0 = `NAME_Log`, 1 = `NAME_Error`.
    /// Named `debug_log` because `debugf` is a macro in the engine headers.
    pub debug_log: unsafe extern "C" fn(kind: i32, msg: *const u32),
    /// `GIsEditor`.
    pub is_editor: unsafe extern "C" fn() -> i32,
    /// `check(expr)` failure at oracle `line`; C++ captures engine exceptions and returns status.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    /// `dst = obj->GetName()`.
    pub object_get_name: unsafe extern "C" fn(dst: Obj, obj: Obj),
    /// `appStricmp(*actor->Tag, viewport->GetName())`.
    pub actor_tag_stricmp_viewport_name: unsafe extern "C" fn(actor: Obj, viewport: Obj) -> i32,
    /// `appSystemTime(Info->Year, Month, DayOfWeek, Day, Hour, Minute, Second, Millisecond)`.
    pub app_system_time_from_level_info: unsafe extern "C" fn(info: Obj),
    /// `appSprintf(Result, TEXT("Script=%05.1f ... Net=%04.1f"), vals[0..9], NumMoves)`.
    /// `vals` order: Script, Actor, Path, See, Spawn, Audio, Un, Move, Net.
    pub app_sprintf_stats: unsafe extern "C" fn(result: *mut u32, vals: *const f32, num_moves: i32),
    /// `GSecondsPerCycle`.
    pub gseconds_per_cycle: unsafe extern "C" fn() -> f32,
    /// `static_cast<FLOAT>(GScriptCycles)`.
    pub script_cycles_f32: unsafe extern "C" fn() -> f32,
    /// `GScriptCycles = 0`.
    pub script_cycles_clear: unsafe extern "C" fn(),
    /// `GScriptEntryTag = 0`.
    pub script_entry_tag_clear: unsafe extern "C" fn(),
    /// `new FAppRandTracePhaseScope(name)`; released with `app_rand_trace_phase_scope_leave`.
    pub app_rand_trace_phase_scope_enter: unsafe extern "C" fn(name: *const c_char) -> Obj,
    /// `delete` the scope from `app_rand_trace_phase_scope_enter`.
    pub app_rand_trace_phase_scope_leave: unsafe extern "C" fn(scope: Obj),

    // Level actor list and level-info access.
    /// `obj->Actors.Num()`.
    pub actors_num: unsafe extern "C" fn(obj: Obj) -> i32,
    /// `obj->Actors(i)`.
    pub actors_get: unsafe extern "C" fn(obj: Obj, i: i32) -> Obj,
    /// `obj->Actors.Empty(n)`.
    pub actors_empty: unsafe extern "C" fn(obj: Obj, n: i32),
    /// `obj->Actors.Add(n)`.
    pub actors_add: unsafe extern "C" fn(obj: Obj, n: i32),
    /// `obj->Actors.CountBytes(Ar)`.
    pub actors_count_bytes: unsafe extern "C" fn(obj: Obj, ar: Obj),
    /// `ALevelInfo::StaticClass()`.
    pub alevel_info_static_class: unsafe extern "C" fn() -> Obj,
    /// `obj->SpawnActor(class)`.
    pub level_spawn_actor: unsafe extern "C" fn(obj: Obj, cls: Obj),
    /// `obj->SpawnBrush()`.
    pub level_spawn_brush: unsafe extern "C" fn(obj: Obj) -> Obj,
    /// `obj->GetLevelInfo()`.
    pub level_get_level_info: unsafe extern "C" fn(obj: Obj) -> Obj,
    /// `obj->GetLevelInfo()->bHighDetailMode = v`.
    pub level_info_set_high_detail: unsafe extern "C" fn(obj: Obj, v: i32),
    /// `obj->GetLevelInfo()->Game != NULL`.
    pub level_info_has_game: unsafe extern "C" fn(obj: Obj) -> i32,
    /// `obj->GetLevelInfo()->Game->eventDetailChange()`.
    pub level_info_game_event_detail_change: unsafe extern "C" fn(obj: Obj),
    /// `obj->DestroyActor(actor)`.
    pub level_destroy_actor: unsafe extern "C" fn(obj: Obj, actor: Obj),
    /// `obj->SpawnViewActor(viewport)`.
    pub level_spawn_view_actor: unsafe extern "C" fn(obj: Obj, viewport: Obj),
    /// `obj->SingleLineCheck(hit, source, end, start, flags, extent, node_flags)`.
    /// `hit` is an `FCheckResult`; `end`, `start` and `extent` are `FVector`s.
    pub level_single_line_check: unsafe extern "C" fn(
        obj: Obj,
        hit: *mut c_void,
        source: Obj,
        end: *const c_void,
        start: *const c_void,
        flags: u32,
        extent: *const c_void,
        node_flags: u8,
    ) -> i32,
    /// `obj->ReachSpecs.Shrink()`.
    pub level_reach_specs_shrink: unsafe extern "C" fn(obj: Obj),
    /// `Engine->Client` is non-null.
    pub level_has_client: unsafe extern "C" fn(obj: Obj) -> i32,
    /// `Engine->Client->Viewports.Num()`.
    pub level_client_viewports_num: unsafe extern "C" fn(obj: Obj) -> i32,
    /// `Engine->Client->Viewports(i)`.
    pub level_client_viewport: unsafe extern "C" fn(obj: Obj, i: i32) -> Obj,
    /// `#if ENGINE_VERSION>230` body of `ULevel::PostLoad`: for every AActor
    /// whose outer is this level's outer, `It->XLevel = this`.
    pub level_retarget_actor_xlevels: unsafe extern "C" fn(obj: Obj),
    /// `obj->Time.GetFloat()` (`TimeSeconds.GetFloat()`).
    pub level_time_seconds_get_float: unsafe extern "C" fn(obj: Obj) -> f32,

    // Collision hash and brush tracker.
    /// `GNewCollisionHash()`.
    pub collision_hash_new: unsafe extern "C" fn() -> Obj,
    /// `hash->AddActor(actor)`.
    pub collision_hash_add_actor: unsafe extern "C" fn(hash: Obj, actor: Obj),
    /// `hash->RemoveActor(actor, true)`.
    pub collision_hash_remove_actor: unsafe extern "C" fn(hash: Obj, actor: Obj),
    /// `delete hash`.
    pub collision_hash_delete: unsafe extern "C" fn(hash: Obj),
    /// `tracker->CountBytes(Ar)`.
    pub brush_tracker_count_bytes: unsafe extern "C" fn(tracker: Obj, ar: Obj),
    /// `delete tracker`.
    pub brush_tracker_delete: unsafe extern "C" fn(tracker: Obj),
    /// `delete net_driver` (UNetDriver, virtual destructor).
    pub net_driver_delete: unsafe extern "C" fn(driver: Obj),

    // AActor and derived-class access.
    /// `actor->bCollideActors`.
    pub actor_collide_actors: unsafe extern "C" fn(actor: Obj) -> i32,
    /// `actor->IsPlayer()`.
    pub actor_is_player: unsafe extern "C" fn(actor: Obj) -> i32,
    /// `*actor->Group` (TCHAR pointer for the FName).
    pub actor_group_chars: unsafe extern "C" fn(actor: Obj) -> *const u32,
    /// `actor->ExcludeGameStates` (FString handle, not a copy).
    pub actor_exclude_game_states: unsafe extern "C" fn(actor: Obj) -> Obj,
    /// `actor->bHiddenEd = v`.
    pub actor_set_hidden_ed: unsafe extern "C" fn(actor: Obj, v: i32),
    /// `actor->bInCurrentGameState = v`.
    pub actor_set_in_current_game_state: unsafe extern "C" fn(actor: Obj, v: i32),
    /// `actor->eventOnResolveGameState()`.
    pub actor_event_on_resolve_game_state: unsafe extern "C" fn(actor: Obj),
    /// `actor->IsOwnedBy(other)`.
    pub actor_is_owned_by: unsafe extern "C" fn(actor: Obj, other: Obj) -> i32,
    /// `actor->IsA(ALevelInfo::StaticClass())`.
    pub actor_is_a_alevel_info: unsafe extern "C" fn(actor: Obj) -> i32,
    /// `actor->IsA(APlayerPawn::StaticClass())`.
    pub actor_is_a_aplayer_pawn: unsafe extern "C" fn(actor: Obj) -> i32,
    /// `actor->IsA(ACamera::StaticClass())`.
    pub actor_is_a_camera: unsafe extern "C" fn(actor: Obj) -> i32,
    /// `Cast<ACamera>(actor)`.
    pub actor_cast_camera: unsafe extern "C" fn(actor: Obj) -> Obj,
    /// `actor->GetClass()->IsChildOf(parent)`.
    pub actor_class_is_child_of: unsafe extern "C" fn(actor: Obj, parent: Obj) -> i32,
    /// `actor->XLevel->Model->Nodes(item).iZone[1]`.
    pub model_node_izone1_of_actor: unsafe extern "C" fn(actor: Obj, item: i32) -> u8,

    // APlayerPawn access.
    /// `pawn->CurrentGameState` (FString handle, not a copy).
    pub pawn_current_game_state: unsafe extern "C" fn(pawn: Obj) -> Obj,
    /// `pawn->GameStateMasterList` (FString handle, not a copy).
    pub pawn_game_state_master_list: unsafe extern "C" fn(pawn: Obj) -> Obj,
    /// `pawn->GameStateTokenLen`.
    pub pawn_game_state_token_len: unsafe extern "C" fn(pawn: Obj) -> i32,
    /// `pawn->IsZoneVisible(zone)`.
    pub pawn_is_zone_visible: unsafe extern "C" fn(pawn: Obj, zone: u8) -> i32,
    /// `pawn->Player`.
    pub pawn_get_player: unsafe extern "C" fn(pawn: Obj) -> Obj,
    /// `pawn->Player = player`.
    pub pawn_set_player: unsafe extern "C" fn(pawn: Obj, player: Obj),

    // ACamera and UViewport access.
    /// `Cast<UViewport>(view->Player)`.
    pub camera_player_viewport: unsafe extern "C" fn(view: Obj) -> Obj,
    /// `view->OrthoZoom = vp->SavedOrthoZoom` and the five other `Saved*` copies
    /// (`FovAngle`, `ShowFlags`, `RendMap`, `Misc1`, `Misc2`).
    pub camera_restore_view: unsafe extern "C" fn(view: Obj, viewport: Obj),
    /// `viewport->Actor`.
    pub viewport_get_actor: unsafe extern "C" fn(viewport: Obj) -> Obj,
    /// `viewport->Actor = (APlayerPawn*)actor; viewport->Actor->Player = viewport;`.
    pub viewport_match_actor: unsafe extern "C" fn(viewport: Obj, actor: Obj),
    /// `viewport->Saved* = viewport->Actor->*; viewport->Actor = NULL;`
    /// (`OrthoZoom`, `FovAngle`, `ShowFlags`, `RendMap`, `Misc1`, `Misc2`).
    pub viewport_remember_actor: unsafe extern "C" fn(viewport: Obj),

    // FCheckResult and Model.
    /// `*dst = *src` (FCheckResult copy assignment).
    pub check_result_assign: unsafe extern "C" fn(dst: *mut c_void, src: *const c_void),
    /// `new(outer) UModel(NULL, root_outside)`.
    pub model_new_root: unsafe extern "C" fn(outer: Obj, root_outside: i32) -> Obj,
    /// `new(outer, TEXT("Brush")) UModel(owner, 1)`.
    pub model_new_brush: unsafe extern "C" fn(outer: Obj, owner: Obj) -> Obj,
    /// `model->Modify()`.
    pub model_modify: unsafe extern "C" fn(model: Obj),
    /// `model->ShrinkModel()`.
    pub model_shrink_model: unsafe extern "C" fn(model: Obj),
    /// `model->FastLineCheck(end, start)`; both point at `FVector`.
    pub model_fast_line_check:
        unsafe extern "C" fn(model: Obj, end: *const c_void, start: *const c_void) -> i32,
    /// `actor->Brush = brush` (ABrush::Brush).
    pub brush_actor_set_brush: unsafe extern "C" fn(actor: Obj, brush: Obj),

    // UObject bases and flags.
    /// `obj->GetOuter()`.
    pub uobject_get_outer: unsafe extern "C" fn(obj: Obj) -> Obj,
    /// `obj->SetFlags(flags)`.
    pub uobject_set_flags: unsafe extern "C" fn(obj: Obj, flags: u32),
    /// `obj->ClearFlags(flags)`.
    pub uobject_clear_flags: unsafe extern "C" fn(obj: Obj, flags: u32),
    /// `UObject::Serialize(Ar)` (the `Super::Serialize` of ULevelBase).
    pub uobject_serialize: unsafe extern "C" fn(obj: Obj, ar: Obj),
    /// `UObject::Destroy()` (the `Super::Destroy` of ULevelBase).
    pub uobject_destroy: unsafe extern "C" fn(obj: Obj),
    /// `UObject::Modify()`.
    pub uobject_modify: unsafe extern "C" fn(obj: Obj),
    /// `Super::PostLoad()` of ULevel (resolves to UObject::PostLoad).
    pub uobject_post_load: unsafe extern "C" fn(obj: Obj),

    // Archive.
    /// `Ar.IsLoading()`.
    pub archive_is_loading: unsafe extern "C" fn(ar: Obj) -> i32,
    /// `Ar.IsSaving()`.
    pub archive_is_saving: unsafe extern "C" fn(ar: Obj) -> i32,
    /// `Ar.IsTrans()`.
    pub archive_is_trans: unsafe extern "C" fn(ar: Obj) -> i32,
    /// `Ar.Ver()`.
    pub archive_ver: unsafe extern "C" fn(ar: Obj) -> i32,
    /// `Ar.Tell()`.
    pub archive_tell: unsafe extern "C" fn(ar: Obj) -> i32,
    /// `Ar.SupportsActorSlotCompactIndexTrace()`.
    pub archive_supports_compact_index_trace: unsafe extern "C" fn(ar: Obj) -> i32,
    /// `Ar.SetActorSlotCompactIndexTrace(trace)`; returns the previous trace.
    pub archive_set_compact_index_trace:
        unsafe extern "C" fn(ar: Obj, trace: *mut c_void) -> *mut c_void,
    /// `Ar << *(INT*)slot`.
    pub archive_serialize_i32: unsafe extern "C" fn(ar: Obj, value: *mut i32),
    /// `Ar << *(FLOAT*)slot`.
    pub archive_serialize_f32: unsafe extern "C" fn(ar: Obj, value: *mut f32),
    /// `Ar << obj->Actors` (TTransArray).
    pub archive_serialize_actors: unsafe extern "C" fn(ar: Obj, obj: Obj),
    /// `Ar << obj->Actors(i)`.
    pub archive_serialize_actor_slot: unsafe extern "C" fn(ar: Obj, obj: Obj, i: i32),
    /// `Ar << obj->URL`.
    pub archive_serialize_url: unsafe extern "C" fn(ar: Obj, obj: Obj),
    /// `Ar << *(UNetDriver**)slot`.
    pub archive_serialize_net_driver: unsafe extern "C" fn(ar: Obj, slot: *mut c_void),
    /// `Ar << *(UModel**)slot`.
    pub archive_serialize_model: unsafe extern "C" fn(ar: Obj, slot: *mut c_void),
    /// `Ar << obj->ReachSpecs` (TArray<FReachSpec>).
    pub archive_serialize_reach_specs: unsafe extern "C" fn(ar: Obj, obj: Obj),
    /// `Ar << *(AActor**)slot`.
    pub archive_serialize_actor_ptr: unsafe extern "C" fn(ar: Obj, slot: *mut c_void),
    /// `Ar << *(UTextBuffer**)slot`.
    pub archive_serialize_text_buffer: unsafe extern "C" fn(ar: Obj, slot: *mut c_void),
    /// `Ar << obj->TravelInfo` (TMap<FString,FString>, Ver > 62).
    pub archive_serialize_travel_info: unsafe extern "C" fn(ar: Obj, obj: Obj),
    /// Ver 61-62 branch: `TArray<FString> Names, Items; Ar << Names << Items;`
    /// `TravelInfo = TMap<>(); TravelInfo.Set(*Names(i), *Items(i))`.
    pub archive_serialize_travel_info_legacy: unsafe extern "C" fn(ar: Obj, obj: Obj),
    /// `Ar.Preload(model)`.
    pub archive_preload: unsafe extern "C" fn(ar: Obj, model: Obj),

    // HP2_ACTOR_SLOT_DUMP support. The getenv call and the weak-hook presence
    // checks live in C++; the capture decision and call order are in Rust.
    /// `getenv("HP2_ACTOR_SLOT_DUMP")`.
    pub actor_slot_dump_env_path: unsafe extern "C" fn() -> *const c_char,
    /// The three weak `HP2ActorSlotDump*` hooks are all linked in.
    pub actor_slot_dump_hooks_linked: unsafe extern "C" fn() -> i32,
    /// `HP2ActorSlotDumpBeginRawLevelActorTable(this)`.
    pub actor_slot_dump_begin: unsafe extern "C" fn(obj: Obj),
    /// `HP2ActorSlotDumpRawLevelActor(this, slot, actor, before, after, index, captured, supported)`.
    pub actor_slot_dump_raw_actor: unsafe extern "C" fn(
        obj: Obj,
        slot: i32,
        actor: Obj,
        reference_before: i32,
        reference_after: i32,
        compact_index: i32,
        captured: i32,
        supported: i32,
    ),
    /// `HP2ActorSlotDumpEndRawLevelActorTable(this)`.
    pub actor_slot_dump_end: unsafe extern "C" fn(obj: Obj),
}

const RF_TRANSACTIONAL: u32 = 0x0000_0001;
const RF_NOT_FOR_CLIENT: u32 = 0x0010_0000;
const RF_NOT_FOR_SERVER: u32 = 0x0020_0000;
const INDEX_NONE: i32 = -1;
const MAXINT: i32 = 0x7fff_ffff;
const NUM_LEVEL_TEXT_BLOCKS: usize = 16;
const TRACE_VIS_BLOCKING: u32 = 0x06;
const ZONE_FACTOR: f64 = 0.85;
const ZONE_DIST_DIM: isize = 64;
const OCCLUSION_DEFAULT: i32 = 0;
const OCCLUSION_BSP: i32 = 2;
const OCCLUSION_STATIC_MESHES: i32 = 3;
const DEBUGF_LOG: i32 = 0;
const DEBUGF_ERROR: i32 = 1;
const ZERO: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

fn lv(obj: Obj) -> *mut ULevel {
    obj.cast()
}

fn tcs(text: &str) -> Vec<u32> {
    text.bytes().map(u32::from).collect()
}

fn nul_terminated(mut text: Vec<u32>) -> Vec<u32> {
    text.push(0);
    text
}

/// # Safety
/// `p` must point to a NUL-terminated TCHAR buffer.
unsafe fn read_tchars(mut p: *const u32) -> Vec<u32> {
    let mut out = Vec::new();
    unsafe {
        while *p != 0 {
            out.push(*p);
            p = p.add(1);
        }
    }
    out
}

/// Owned `FString` handle; released with `fstring_delete` at scope end.
struct FStr<'a> {
    s: &'a Hp2LevelCoreServices,
    h: Obj,
}

impl<'a> FStr<'a> {
    /// # Safety
    /// Calls the C++ service table.
    unsafe fn new(s: &'a Hp2LevelCoreServices) -> Self {
        Self {
            s,
            h: unsafe { (s.fstring_new)() },
        }
    }

    /// # Safety
    /// The handle must be live.
    unsafe fn chars(&self) -> Vec<u32> {
        unsafe { read_tchars((self.s.fstring_chars)(self.h)) }
    }
}

impl Drop for FStr<'_> {
    fn drop(&mut self) {
        unsafe { (self.s.fstring_delete)(self.h) }
    }
}

/// `debugf(kind, TEXT("%s"), msg)`; `msg` excludes the terminator.
///
/// # Safety
/// Calls the C++ service table.
unsafe fn debug_text(s: &Hp2LevelCoreServices, kind: i32, msg: Vec<u32>) {
    let buf = nul_terminated(msg);
    unsafe { (s.debug_log)(kind, buf.as_ptr()) }
}

/// `FDist(V1, V2)` = `appSqrt(Square(dx)+Square(dy)+Square(dz))` (UnMath.h:2161).
fn fdist(v1: FVector, v2: FVector) -> f32 {
    let dx = v2.x - v1.x;
    let dy = v2.y - v1.y;
    let dz = v2.z - v1.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// `Start + V` (operator+ adds component-wise, `X + V.X`).
fn vec_add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

/// `FLOAT * FVector` (operator*(FLOAT, V) returns `V.X * Scale` per component).
fn vec_scale(scale: f32, v: FVector) -> FVector {
    FVector {
        x: v.x * scale,
        y: v.y * scale,
        z: v.z * scale,
    }
}

/// `ULevelBase::Serialize` (UnLevel.cpp:36).
///
/// # Safety
/// `obj` must be a live ULevelBase; `ar` a live FArchive.
pub unsafe fn level_base_serialize(s: &Hp2LevelCoreServices, obj: Obj, ar: Obj) {
    unsafe {
        (s.uobject_serialize)(obj, ar);
        if (s.archive_is_trans)(ar) != 0 {
            (s.archive_serialize_actors)(ar, obj);
        } else {
            // oldver: old-format actor list.
            let mut db_num = (s.actors_num)(obj);
            let mut db_max = db_num;
            (s.actors_count_bytes)(obj, ar);
            (s.archive_serialize_i32)(ar, &mut db_num);
            (s.archive_serialize_i32)(ar, &mut db_max);
            if (s.archive_is_loading)(ar) != 0 {
                (s.actors_empty)(obj, db_num);
                (s.actors_add)(obj, db_num);
            }
            let slot_dump_path = (s.actor_slot_dump_env_path)();
            let capture = (s.archive_is_loading)(ar) != 0
                && !slot_dump_path.is_null()
                && *slot_dump_path != 0
                && (s.actor_slot_dump_hooks_linked)() != 0;
            if capture {
                (s.actor_slot_dump_begin)(obj);
            }
            let mut i = 0;
            while i < (s.actors_num)(obj) {
                let mut trace = FActorSlotCompactIndexTrace {
                    index: INDEX_NONE,
                    offset_after: INDEX_NONE,
                    captured: 0,
                };
                let reference_before = if capture {
                    (s.archive_tell)(ar)
                } else {
                    INDEX_NONE
                };
                let capture_compact = capture && (s.archive_supports_compact_index_trace)(ar) != 0;
                let mut previous: *mut c_void = ptr::null_mut();
                if capture_compact {
                    previous =
                        (s.archive_set_compact_index_trace)(ar, ptr::addr_of_mut!(trace).cast());
                }
                (s.archive_serialize_actor_slot)(ar, obj, i);
                if capture_compact {
                    (s.archive_set_compact_index_trace)(ar, previous);
                }
                if capture {
                    (s.actor_slot_dump_raw_actor)(
                        obj,
                        i,
                        (s.actors_get)(obj, i),
                        reference_before,
                        trace.offset_after,
                        trace.index,
                        trace.captured,
                        capture_compact as i32,
                    );
                }
                i += 1;
            }
            if capture {
                (s.actor_slot_dump_end)(obj);
            }
        }
        // Level variables (after both branches, as in the original).
        (s.archive_serialize_url)(ar, obj);
        if (s.archive_is_loading)(ar) == 0 && (s.archive_is_saving)(ar) == 0 {
            (s.archive_serialize_net_driver)(ar, ptr::addr_of_mut!((*lv(obj)).net_driver).cast());
            (s.archive_serialize_net_driver)(
                ar,
                ptr::addr_of_mut!((*lv(obj)).demo_rec_driver).cast(),
            );
        }
    }
}

/// `ULevelBase::Destroy` (UnLevel.cpp:91).
///
/// # Safety
/// `obj` must be a live ULevelBase.
pub unsafe fn level_base_destroy(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        let level = lv(obj);
        if !(*level).net_driver.is_null() {
            (s.net_driver_delete)((*level).net_driver);
            (*level).net_driver = ptr::null_mut();
        }
        if !(*level).demo_rec_driver.is_null() {
            (s.net_driver_delete)((*level).demo_rec_driver);
            (*level).demo_rec_driver = ptr::null_mut();
        }
        (s.uobject_destroy)(obj);
    }
}

/// `ULevel::ULevel` (UnLevel.cpp:123). The C++ constructor body runs `ULevelBase(InEngine)` first.
///
/// # Safety
/// `obj` must be a freshly constructed ULevel.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn level_init(s: &Hp2LevelCoreServices, obj: Obj, root_outside: i32) -> Result<(), ()> {
    unsafe {
        // Allocate subobjects.
        (s.uobject_set_flags)(obj, RF_TRANSACTIONAL);
        let model = (s.model_new_root)((s.uobject_get_outer)(obj), root_outside);
        (*lv(obj)).model = model;
        (s.uobject_set_flags)(model, RF_TRANSACTIONAL);
        // Spawn the level info.
        (s.level_spawn_actor)(obj, (s.alevel_info_static_class)());
        if (s.level_get_level_info)(obj).is_null()
            && (s.check_failed)(c"GetLevelInfo()".as_ptr(), 135) != 0
        {
            return Err(());
        }
        // Spawn the default brush.
        let temp = (s.level_spawn_brush)(obj);
        if temp != (s.actors_get)(obj, 1) && (s.check_failed)(c"Temp==Actors(1)".as_ptr(), 139) != 0
        {
            return Err(());
        }
        let brush = (s.model_new_brush)((s.uobject_get_outer)(obj), temp);
        (s.brush_actor_set_brush)(temp, brush);
        (s.uobject_set_flags)(
            temp,
            RF_NOT_FOR_CLIENT | RF_NOT_FOR_SERVER | RF_TRANSACTIONAL,
        );
        (s.uobject_set_flags)(
            brush,
            RF_NOT_FOR_CLIENT | RF_NOT_FOR_SERVER | RF_TRANSACTIONAL,
        );
        Ok(())
    }
}

/// `ULevel::ScreenActorsByGameState` (UnLevel.cpp:154).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_screen_actors_by_game_state(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        let cur_gstate = FStr::new(s);
        let buffer = FStr::new(s);
        let token = FStr::new(s);
        let gstate_key = nul_terminated(tcs("GSTATE"));
        let comma = nul_terminated(tcs(","));
        let none = tcs("NONE");
        let mut pawn: Obj = ptr::null_mut();

        // *** Get PlayerPawn: the player pawn has the CurrentGameState.
        let mut i = 0;
        while i < (s.actors_num)(obj) {
            let act = (s.actors_get)(obj, i);
            if !act.is_null() && (s.actor_is_player)(act) != 0 {
                pawn = act;
                (s.fstring_caps)(cur_gstate.h, (s.pawn_current_game_state)(pawn));
                break;
            }
            i += 1;
        }

        // *** Error checking: if the current state is not set then return.
        if (s.fstring_eq_chars)(cur_gstate.h, nul_terminated(none).as_ptr()) != 0 {
            return;
        }
        // If we didn't find the player or its CurGState is invalid, return.
        if pawn.is_null()
            || (s.fstring_instr_fstring)((s.pawn_game_state_master_list)(pawn), cur_gstate.h) < 0
        {
            if pawn.is_null() {
                debug_text(s, DEBUGF_LOG, tcs("**** Can't find a valid player!!!! "));
            } else {
                debug_text(
                    s,
                    DEBUGF_LOG,
                    tcs(
                        "**** Caution: The player pawn has an Invalid CurrentGameState for this package Load.",
                    ),
                );
            }
            return;
        }

        // *** Screen Actors: screen them according to the current GameState.
        let mut i = 0;
        while i < (s.actors_num)(obj) {
            let act = (s.actors_get)(obj, i);
            i += 1;
            if act.is_null() {
                continue;
            }
            // Reset local vars for the next token.
            (s.fstring_assign_chars)(buffer.h, (s.actor_group_chars)(act));
            (s.fstring_caps)(buffer.h, buffer.h);
            let mut has_gstate = false;
            let mut in_cur_gstate = false;
            loop {
                // See if this actor has an excludeGameState list containing CurGState.
                let exclude = (s.actor_exclude_game_states)(act);
                if (s.fstring_len)(exclude) != 0 {
                    let excl_caps = FStr::new(s);
                    (s.fstring_caps)(excl_caps.h, exclude);
                    if (s.fstring_instr_fstring)(excl_caps.h, cur_gstate.h) >= 0 {
                        // CurGState is in the actor's *exclude* list.
                        has_gstate = true;
                        in_cur_gstate = false;
                        break;
                    }
                }
                // Is the GState *keyword* in the group name?
                let j = (s.fstring_instr_chars)(buffer.h, gstate_key.as_ptr());
                if j < 0 {
                    break;
                }
                // This actor has a GState.
                has_gstate = true;
                if (s.is_editor)() != 0 {
                    (s.actor_set_hidden_ed)(act, 0);
                }
                // Start the buffer at the "GS" keyword.
                (s.fstring_mid)(buffer.h, buffer.h, j, MAXINT);
                // Current token.
                let mut len = (s.fstring_instr_chars)(buffer.h, comma.as_ptr());
                if len < 0 {
                    len = (s.fstring_len)(buffer.h);
                }
                (s.fstring_mid)(token.h, buffer.h, 0, len);
                if (s.fstring_len)(token.h) == (s.pawn_game_state_token_len)(pawn)
                    && (s.fstring_instr_fstring)((s.pawn_game_state_master_list)(pawn), token.h)
                        >= 0
                {
                    // Is this actor's GState the *current* GState?
                    if (s.fstring_eq_fstring)(token.h, cur_gstate.h) != 0
                        && (s.fstring_instr_fstring)(
                            (s.actor_exclude_game_states)(act),
                            cur_gstate.h,
                        ) < 0
                    {
                        in_cur_gstate = true;
                        break;
                    }
                } else {
                    // ERROR: the GState wasn't in the master list.
                    (s.object_get_name)(buffer.h, act);
                    let mut msg = tcs("*** GameState ERROR Actor: ");
                    msg.extend(buffer.chars());
                    msg.extend(tcs(" has an invalid GameState Token: "));
                    msg.extend(token.chars());
                    msg.extend(tcs("!!!"));
                    debug_text(s, DEBUGF_ERROR, msg);
                }
                // Cut out the token.
                (s.fstring_mid)(buffer.h, buffer.h, (s.fstring_len)(token.h), MAXINT);
            }
            // Set bInCurrentGameState and resolve the GameState if this actor has one.
            if has_gstate {
                (s.actor_set_in_current_game_state)(act, in_cur_gstate as i32);
                if (s.is_editor)() == 0 {
                    (s.actor_event_on_resolve_game_state)(act);
                }
            }
        }
    }
}

/// `ULevel::ShrinkLevel` (UnLevel.cpp:930).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_shrink(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        (s.model_shrink_model)((*lv(obj)).model);
        (s.level_reach_specs_shrink)(obj);
    }
}

/// `ULevel::DetailChange` (UnLevel.cpp:939).
///
/// # Safety
/// `obj` must be a live ULevel with a level info.
pub unsafe fn level_detail_change(s: &Hp2LevelCoreServices, obj: Obj, new_detail: i32) {
    unsafe {
        (s.level_info_set_high_detail)(obj, new_detail);
        if (s.level_info_has_game)(obj) != 0 {
            (s.level_info_game_event_detail_change)(obj);
        }
    }
}

/// `ULevel::Modify` (UnLevel.cpp:955). `DoTransArrays` is unused by the original body.
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_modify(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        (s.uobject_modify)(obj);
        (s.model_modify)((*lv(obj)).model);
    }
}

/// `ULevel::PostLoad` (UnLevel.cpp:962).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_post_load(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        (s.uobject_post_load)(obj);
        (s.level_retarget_actor_xlevels)(obj);
    }
}

/// `ULevel::SetActorCollision` (UnLevel.cpp:973).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_set_actor_collision(s: &Hp2LevelCoreServices, obj: Obj, collision: i32) {
    unsafe {
        let level = lv(obj);
        // Init collision if first time through.
        if collision != 0 && (*level).hash.is_null() {
            let scope = (s.app_rand_trace_phase_scope_enter)(c"collision_hash".as_ptr());
            // Divergence: nested guard(StartCollision) label is not reproduced.
            (*level).hash = (s.collision_hash_new)();
            let mut i = 0;
            while i < (s.actors_num)(obj) {
                let act = (s.actors_get)(obj, i);
                if !act.is_null() && (s.actor_collide_actors)(act) != 0 {
                    (s.collision_hash_add_actor)((*level).hash, act);
                }
                i += 1;
            }
            (s.app_rand_trace_phase_scope_leave)(scope);
        } else if !(*level).hash.is_null() && collision == 0 {
            // Destroy hash.
            // Divergence: nested guard(EndCollision) label is not reproduced.
            let mut i = 0;
            while i < (s.actors_num)(obj) {
                let act = (s.actors_get)(obj, i);
                if !act.is_null() && (s.actor_collide_actors)(act) != 0 {
                    (s.collision_hash_remove_actor)((*level).hash, act);
                }
                i += 1;
            }
            (s.collision_hash_delete)((*level).hash);
            (*level).hash = ptr::null_mut();
        }
    }
}

/// `ULevel::Serialize` (UnLevel.cpp:1008).
///
/// # Safety
/// `obj` must be a live ULevel; `ar` a live FArchive.
pub unsafe fn level_serialize(s: &Hp2LevelCoreServices, obj: Obj, ar: Obj) {
    unsafe {
        let level = lv(obj);
        level_base_serialize(s, obj, ar);
        let mut approx_time = (s.level_time_seconds_get_float)(obj);
        (s.archive_serialize_model)(ar, ptr::addr_of_mut!((*level).model).cast());
        (s.archive_serialize_reach_specs)(ar, obj);
        (s.archive_serialize_f32)(ar, &mut approx_time);
        (s.archive_serialize_actor_ptr)(ar, ptr::addr_of_mut!((*level).first_deleted).cast());
        for i in 0..NUM_LEVEL_TEXT_BLOCKS {
            (s.archive_serialize_text_buffer)(
                ar,
                ptr::addr_of_mut!((*level).text_blocks[i]).cast(),
            );
        }
        if (s.archive_ver)(ar) > 62 {
            (s.archive_serialize_travel_info)(ar, obj);
        } else if (s.archive_ver)(ar) >= 61 {
            (s.archive_serialize_travel_info_legacy)(ar, obj);
        }
        if !(*level).model.is_null() && (s.archive_is_trans)(ar) == 0 {
            (s.archive_preload)(ar, (*level).model);
        }
        if !(*level).brush_tracker.is_null() {
            (s.brush_tracker_count_bytes)((*level).brush_tracker, ar);
        }
    }
}

/// `ULevel::Destroy` (UnLevel.cpp:1041).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_destroy(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        let level = lv(obj);
        if !(*level).hash.is_null() {
            (s.collision_hash_delete)((*level).hash);
            (*level).hash = ptr::null_mut();
        }
        if !(*level).brush_tracker.is_null() {
            (s.brush_tracker_delete)((*level).brush_tracker);
            (*level).brush_tracker = ptr::null_mut();
        }
        level_base_destroy(s, obj);
    }
}

/// `ULevel::RememberActors` (UnLevel.cpp:1074).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_remember_actors(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        if (s.level_has_client)(obj) != 0 {
            let mut i = 0;
            while i < (s.level_client_viewports_num)(obj) {
                let viewport = (s.level_client_viewport)(obj, i);
                (s.viewport_remember_actor)(viewport);
                i += 1;
            }
        }
    }
}

/// `ULevel::ReconcileActors` (UnLevel.cpp:1099).
///
/// # Safety
/// `obj` must be a live ULevel with an engine client.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn level_reconcile_actors(s: &Hp2LevelCoreServices, obj: Obj) -> Result<(), ()> {
    unsafe {
        if (s.is_editor)() == 0 && (s.check_failed)(c"GIsEditor".as_ptr(), 1102) != 0 {
            return Err(());
        }

        // Dissociate all actor Viewports and remember their view properties.
        let mut i = 0;
        while i < (s.actors_num)(obj) {
            let act = (s.actors_get)(obj, i);
            if !act.is_null()
                && (s.actor_is_a_aplayer_pawn)(act) != 0
                && !(s.pawn_get_player)(act).is_null()
            {
                (s.pawn_set_player)(act, ptr::null_mut());
            }
            i += 1;
        }

        // Match Viewports and Viewport-actors with identical names.
        // Divergence: nested guard(MatchIdentical) label is not reproduced.
        let mut i = 0;
        while i < (s.level_client_viewports_num)(obj) {
            let viewport = (s.level_client_viewport)(obj, i);
            if !(s.viewport_get_actor)(viewport).is_null()
                && (s.check_failed)(c"Viewport->Actor==NULL".as_ptr(), 1116) != 0
            {
                return Err(());
            }
            let mut j = 0;
            while j < (s.actors_num)(obj) {
                let act = (s.actors_get)(obj, j);
                if !act.is_null()
                    && (s.actor_is_a_camera)(act) != 0
                    && (s.actor_tag_stricmp_viewport_name)(act, viewport) == 0
                {
                    let name = FStr::new(s);
                    (s.object_get_name)(name.h, viewport);
                    let mut msg = tcs("Matched Viewport ");
                    msg.extend(name.chars());
                    debug_text(s, DEBUGF_LOG, msg);
                    (s.viewport_match_actor)(viewport, act);
                    break;
                }
                j += 1;
            }
            i += 1;
        }

        // Match up all remaining Viewports to actors.
        // Divergence: nested guard(MatchEditorOther) label is not reproduced.
        let mut i = 0;
        while i < (s.level_client_viewports_num)(obj) {
            let viewport = (s.level_client_viewport)(obj, i);
            if (s.viewport_get_actor)(viewport).is_null() {
                (s.level_spawn_view_actor)(obj, viewport);
            }
            i += 1;
        }

        // Handle remaining unassociated view actors.
        // Divergence: nested guard(KillViews) label is not reproduced.
        let mut i = 0;
        while i < (s.actors_num)(obj) {
            let view = (s.actor_cast_camera)((s.actors_get)(obj, i));
            if !view.is_null() {
                let viewport = (s.camera_player_viewport)(view);
                if !viewport.is_null() {
                    (s.uobject_clear_flags)(view, RF_TRANSACTIONAL);
                    (s.camera_restore_view)(view, viewport);
                } else {
                    (s.level_destroy_actor)(obj, view);
                }
            }
            i += 1;
        }
        Ok(())
    }
}

/// `ULevel::InitStats` (UnLevel.cpp:1615).
///
/// # Safety
/// `obj` must be a live ULevel.
pub unsafe fn level_init_stats(s: &Hp2LevelCoreServices, obj: Obj) {
    unsafe {
        let level = lv(obj);
        (*level).net_tick_cycles = 0;
        (*level).net_diff_cycles = 0;
        (*level).actor_tick_cycles = 0;
        (*level).audio_tick_cycles = 0;
        (*level).find_path_cycles = 0;
        (*level).move_cycles = 0;
        (*level).num_moves = 0;
        (*level).num_reps = 0;
        (*level).num_pv = 0;
        (*level).get_relevant_cycles = 0;
        (*level).num_rpc = 0;
        (*level).see_player = 0;
        (*level).spawning = 0;
        (*level).unused = 0;
        (s.script_cycles_clear)();
        (s.script_entry_tag_clear)();
    }
}

/// `ULevel::GetStats` (UnLevel.cpp:1624). The format literal stays in C++.
///
/// # Safety
/// `obj` must be a live ULevel; `result` must point to a TCHAR buffer large enough for the format.
pub unsafe fn level_get_stats(s: &Hp2LevelCoreServices, obj: Obj, result: *mut u32) {
    unsafe {
        let level = lv(obj);
        // `GSecondsPerCycle*1000 * X` evaluates as `(GSecondsPerCycle*1000) * X`.
        let k = (s.gseconds_per_cycle)() * 1000.0f32;
        let vals: [f32; 9] = [
            k * (s.script_cycles_f32)(),
            k * ((*level).actor_tick_cycles as f32),
            k * ((*level).find_path_cycles as f32),
            k * ((*level).see_player as f32),
            k * ((*level).spawning as f32),
            k * ((*level).audio_tick_cycles as f32),
            k * ((*level).unused as f32),
            k * ((*level).move_cycles as f32),
            k * ((*level).net_tick_cycles as f32),
        ];
        (s.app_sprintf_stats)(result, vals.as_ptr(), (*level).num_moves);
    }
}

/// `ULevel::UpdateTime` (UnLevel.cpp:1649).
///
/// # Safety
/// `info` must be a live ALevelInfo.
pub unsafe fn level_update_time(s: &Hp2LevelCoreServices, info: Obj) {
    unsafe { (s.app_system_time_from_level_info)(info) }
}

/// `ULevel::IsAudibleAt` (UnLevel.cpp:1658). `hit` is the C++-owned `Hit` local.
///
/// # Safety
/// `obj` must be a live ULevel; pointers must be valid for the call.
pub unsafe fn level_is_audible_at(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    sound_location: *const FVector,
    listener_location: *const FVector,
    sound_actor: Obj,
    sound_occlusion: i32,
    hit: *mut FCheckResult,
) -> i32 {
    unsafe {
        match sound_occlusion {
            OCCLUSION_STATIC_MESHES => (s.level_single_line_check)(
                obj,
                hit.cast(),
                sound_actor,
                sound_location.cast(),
                listener_location.cast(),
                TRACE_VIS_BLOCKING,
                ptr::from_ref(&ZERO).cast(),
                0,
            ),
            OCCLUSION_DEFAULT | OCCLUSION_BSP => (s.model_fast_line_check)(
                (*lv(obj)).model,
                sound_location.cast(),
                listener_location.cast(),
            ),
            // OCCLUSION_NONE (1) and any other value.
            _ => 1,
        }
    }
}

/// `ULevel::CalculateRadiusMultiplier` (UnLevel.cpp:1683).
///
/// # Safety
/// `obj` must be a live ULevel. `zone1`/`zone2` are unchecked, as in the original.
pub unsafe fn level_calculate_radius_multiplier(obj: Obj, zone1: i32, zone2: i32) -> f32 {
    unsafe {
        // ZoneDist is BYTE[64][64] (UnLevel.h:156); index without bounds checks.
        let base = ptr::addr_of!((*lv(obj)).zone_dist).cast::<u8>();
        let distance = i32::from(*base.offset(zone1 as isize * ZONE_DIST_DIM + zone2 as isize));
        // `appPow(ZONE_FACTOR, Distance * Distance)`: int product widened to double.
        ZONE_FACTOR.powf(f64::from(distance * distance)) as f32
    }
}

/// Returns `SourceActor` when it is an APlayerPawn, else null (the `Player` local).
///
/// # Safety
/// `source` must be a live AActor.
unsafe fn visible_player(s: &Hp2LevelCoreServices, source: Obj) -> Obj {
    unsafe {
        if (s.actor_is_a_aplayer_pawn)(source) != 0 {
            source
        } else {
            ptr::null_mut()
        }
    }
}

/// Searches the `FirstHit` chain for a visible hit (the inner `for` of TraceVisible).
/// `None` means the original `break` or loop exhaustion.
///
/// # Safety
/// `first_hit` must head a valid FCheckResult chain.
unsafe fn find_visible_check(
    s: &Hp2LevelCoreServices,
    source: Obj,
    player: Obj,
    first_hit: *mut FCheckResult,
) -> Option<*mut FCheckResult> {
    unsafe {
        let mut check = first_hit;
        while !check.is_null() {
            let actor = (*check).actor;
            // Skip owned actors, but return the one nearest actor or level data.
            if source.is_null() || (s.actor_is_owned_by)(source, actor) == 0 {
                if (s.actor_is_a_alevel_info)(actor) != 0 {
                    // If we're in the rock (node 0), skip the test, then try again.
                    if (*check).item == 0 {
                        return None;
                    }
                    // Make sure the node we hit is in a visible zone.
                    let zone = (s.model_node_izone1_of_actor)(actor, (*check).item);
                    if player.is_null() || (s.pawn_is_zone_visible)(player, zone) != 0 {
                        return Some(check);
                    }
                } else if !actor.is_null() {
                    // Make sure the actor we hit is in a visible zone.
                    let zone = (*actor.cast::<AActor>()).region.zone_number;
                    if player.is_null() || (s.pawn_is_zone_visible)(player, zone) != 0 {
                        return Some(check);
                    }
                }
            }
            check = (*check).next.cast();
        }
        None
    }
}

/// `ULevel::TraceVisible` (UnLevel.cpp:1697). `first_hit` is the C++-owned `FirstHit` local.
///
/// # Safety
/// `obj` must be a live ULevel; all pointers must be valid; `first_hit` is written by the trace.
#[allow(clippy::too_many_arguments)] // Mirrors the C++ signature (UnLevel.cpp:1697).
pub unsafe fn level_trace_visible(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    trace_direction: *const FVector,
    hit: *mut FCheckResult,
    source: Obj,
    start: *const FVector,
    trace_flags: u32,
    distance: i32,
    first_hit: *mut FCheckResult,
) {
    unsafe {
        let direction = *trace_direction;
        let start_location = *start;
        let player = visible_player(s, source);

        // Trace the entire distance looking for a "selected" zone.
        let mut start_trace = start_location;
        let end = vec_add(start_location, vec_scale(distance as f32, direction));
        while (fdist(start_location, start_trace) as i32) < distance {
            // Get list of hit actors.
            if (s.level_single_line_check)(
                obj,
                first_hit.cast(),
                source,
                ptr::from_ref(&end).cast(),
                ptr::from_ref(&start_trace).cast(),
                trace_flags,
                ptr::from_ref(&ZERO).cast(),
                0,
            ) != 0
            {
                break;
            }
            if let Some(check) = find_visible_check(s, source, player, first_hit) {
                (s.check_result_assign)(hit.cast(), check.cast_const().cast());
                return;
            }
            // Found a room, but it's the wrong one: move forward one foot, then try again.
            start_trace = vec_add((*first_hit).location, vec_scale(16.0, direction));
        }

        // Missed section.
        (*hit).time = 1.0;
        (*hit).actor = ptr::null_mut();
    }
}

/// `ULevel::TraceVisibleObjects` (UnLevel.cpp:1773). `first_hit` is the C++-owned `FirstHit` local.
///
/// # Safety
/// `obj` must be a live ULevel; all pointers must be valid; `first_hit` is written by the trace.
#[allow(clippy::too_many_arguments)] // Mirrors the C++ signature (UnLevel.cpp:1773).
pub unsafe fn level_trace_visible_objects(
    s: &Hp2LevelCoreServices,
    obj: Obj,
    parent_class: Obj,
    trace_direction: *const FVector,
    hit: *mut FCheckResult,
    source: Obj,
    start: *const FVector,
    trace_flags: u32,
    distance: i32,
    first_hit: *mut FCheckResult,
) {
    unsafe {
        let direction = *trace_direction;
        let start_location = *start;
        let player = visible_player(s, source);

        // Trace the entire distance looking for a matching object in the "selected" zone.
        let mut start_trace = start_location;
        let end = vec_add(start_location, vec_scale(distance as f32, direction));
        while fdist(start_location, start_trace) < distance as f32 {
            // Get list of hit actors.
            if (s.level_single_line_check)(
                obj,
                first_hit.cast(),
                source,
                ptr::from_ref(&end).cast(),
                ptr::from_ref(&start_trace).cast(),
                trace_flags,
                ptr::from_ref(&ZERO).cast(),
                0,
            ) != 0
            {
                break;
            }
            // Skip owned actors, but return the nearest actor matching the ParentClass.
            let mut check = first_hit;
            while !check.is_null() {
                let actor = (*check).actor;
                if (source.is_null() || (s.actor_is_owned_by)(source, actor) == 0)
                    && (s.actor_class_is_child_of)(actor, parent_class) != 0
                {
                    // Make sure the actor we hit is in a visible zone.
                    let zone = (*actor.cast::<AActor>()).region.zone_number;
                    if player.is_null() || (s.pawn_is_zone_visible)(player, zone) != 0 {
                        (s.check_result_assign)(hit.cast(), check.cast_const().cast());
                        return;
                    }
                }
                check = (*check).next.cast();
            }
            // Found a room, but it's the wrong one: move forward 1 foot, then try again.
            start_trace = vec_add((*first_hit).location, vec_scale(16.0, direction));
        }

        // Missed section.
        (*hit).time = 1.0;
        (*hit).actor = ptr::null_mut();
    }
}
