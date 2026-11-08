//! Shared lighting-manager storage and lifetime policy. Native code only borrows typed
//! SDK views; no per-actor snapshots or nesting restoration are introduced.
#![allow(non_snake_case)]
use crate::{light, light_actor, services::Hp2RenderServices, services_light::Hp2LightServices};
use hp2_abi::{
    FVector,
    engine::generated::{AActor, ALevelInfo},
};
use std::{
    cell::{Cell, UnsafeCell},
    ffi::{c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    ptr,
};
thread_local! {
    static ACTIVE_STATE: Cell<*mut State> = const { Cell::new(ptr::null_mut()) };
    static CORE_FILTER_FAILED: Cell<bool> = const { Cell::new(false) };
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Effect {
    pub SpatialFxFunc: *mut c_void,
    pub IsSpatialDynamic: i32,
    pub IsMergeDynamic: i32,
}
#[repr(C)]
pub struct LightInfo {
    pub Actor: *mut c_void,
    pub Opt: i32,
    pub Location: [f32; 3],
    pub LightDir: [f32; 3],
    pub MaxSpecDir: [f32; 3],
    pub Radius: f32,
    pub RRadius: f32,
    pub RadiusInner: f32,
    pub RRadiusMult: f32,
    pub Brightness: f32,
    pub LightDistTab: *mut f32,
    pub Diffuse: f32,
    pub IlluminationMap: *mut u8,
    pub ShadowBits: *mut u8,
    pub IsVolumetric: i32,
    pub MinU: i32,
    pub MaxU: i32,
    pub MinV: i32,
    pub MaxV: i32,
    pub VolRadius: f32,
    pub VolRadiusSquared: f32,
    pub VolBrightness: f32,
    pub LocationSizeSquared: f32,
    pub RVolRadius: f32,
    pub RVolRadiusSquared: f32,
    pub VolInside: i32,
    pub Effect: Effect,
    pub FloatColor: [f32; 4],
    pub VolumetricColor: [f32; 4],
    pub ScaledColor: [f32; 4],
    pub Palette: *mut c_void,
    pub VolPalette: *mut c_void,
}
// SDK objects are constructed in place by the SDK adapter. Their size/alignment
// and all surrounding offsets are asserted against this view in UnLight.cpp.
#[repr(C)]
pub struct State {
    pub MapCoords: *mut c_void,
    pub MapUncoords: [f32; 12],
    pub VertexBase: [f32; 3],
    pub VertexDU: [f32; 3],
    pub VertexDV: [f32; 3],
    pub Frame: *mut c_void,
    pub Level: *mut c_void,
    pub Mark: [usize; 3],
    pub ShadowMaskU: i32,
    pub ShadowMaskSpace: i32,
    pub ShadowSkip: i32,
    pub StaticLights: i32,
    pub DynamicLights: i32,
    pub MovingLights: i32,
    pub StaticLightingChanged: i32,
    pub LightMap: [u64; 25],
    pub FogMap: [u64; 25],
    pub LightMip: [u64; 8],
    pub FogMip: [u64; 8],
    pub LastLight: *mut LightInfo,
    pub LastVtric: *mut *mut LightInfo,
    pub FinalLight: *mut LightInfo,
    pub FirstLight: [LightInfo; 256],
    pub FirstVtric: [*mut LightInfo; 256],
    pub LevelInfo: *mut c_void,
    pub Zone: *mut c_void,
    pub AmbientVector: [f32; 4],
    pub Diffuse: f32,
    pub Specular: f32,
    pub SpecMinCos: f32,
    pub TemporaryTablesBuilt: i32,
    pub BackdropBrightness: f32,
    pub LightDist: [f32; 4096],
    pub LightDistInc: [f32; 4096],
    pub FilterTab: [[u32; 4]; 128],
    pub ByteFog: [u8; 0x4000],
    pub Actor: *mut c_void,
    pub Effects: [Effect; 20],
    pub ItemsToUnlock: [*mut c_void; 256],
    pub TopItemToUnlock: *mut *mut c_void,
    pub SurfManager: *mut c_void,
    pub SurfLocalMark: [usize; 3],
    pub FogRejectionMethod: i32,
    pub StrobeLastUpdateTime: f32,
    pub StrobeToggle: i32,
    pub Consider: light_actor::LightConsiderSet,
}
struct Shared(UnsafeCell<State>);
// The SDK renderer is single-threaded, including reentrant calls. Raw access is
// intentional: never retain a Rust borrow across a native engine callback.
unsafe impl Sync for Shared {}
static STATE: Shared = Shared(UnsafeCell::new(unsafe { std::mem::zeroed() }));
/// Returns the renderer state selected for this synchronous policy call.
pub fn state() -> *mut State {
    ACTIVE_STATE.with(|active| {
        let selected = active.get();
        if selected.is_null() { STATE.0.get() } else { selected }
    })
}

struct StateScope(*mut State);

impl Drop for StateScope {
    fn drop(&mut self) {
        ACTIVE_STATE.with(|active| active.set(self.0));
    }
}

/// Runs light-manager algorithms against a renderer-owned state record.
///
/// # Safety
/// `state` must remain uniquely accessible until `operation` returns; the operation must
/// not retain references to the selected state after returning.
pub unsafe fn with_state<R>(state: *mut State, operation: impl FnOnce() -> R) -> R {
    assert!(!state.is_null(), "light-manager state must be non-null");
    let previous = ACTIVE_STATE.with(|active| active.replace(state));
    let _scope = StateScope(previous);
    operation()
}

unsafe extern "C" fn core_filter_failed(_expression: *const c_char, _line: i32) {
    CORE_FILTER_FAILED.with(|failed| failed.set(true));
}

/// Initializes a renderer-owned light-manager state without native SDK lifecycle leaves.
///
/// # Safety
/// `state` must point to exclusively accessible, correctly aligned `State` storage.
pub unsafe fn initialize_core_state(state: *mut State, initialize_static_fields: bool) -> bool {
    if state.is_null() {
        return false;
    }
    CORE_FILTER_FAILED.with(|failed| failed.set(false));
    unsafe { initialize_tables(state, initialize_static_fields, core_filter_failed) };
    !CORE_FILTER_FAILED.with(Cell::get)
}

unsafe fn initialize_tables(
    state: *mut State,
    initialize_static_fields: bool,
    check_failed: unsafe extern "C" fn(*const c_char, i32),
) {
    unsafe {
        if initialize_static_fields {
            (*state).FinalLight = ptr::addr_of_mut!((*state).FirstLight)
                .cast::<LightInfo>()
                .add(256);
            let kinds = [0, 0, 0, 0, 1, 3, 4, 0, 8, 5, 6, 0, 8, 10, 11, 0, 7, 9, 2, 0];
            for (index, kind) in kinds.into_iter().enumerate() {
                (*state).Effects[index] = Effect {
                    SpatialFxFunc: spatial_fn_address(kind),
                    IsSpatialDynamic: i32::from(matches!(index, 4 | 5 | 6 | 7 | 9 | 10 | 16 | 18)),
                    IsMergeDynamic: i32::from(matches!(index, 1 | 2 | 3)),
                };
            }
        }
        ptr::write_bytes(ptr::addr_of_mut!((*state).LightMap), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*state).FogMap), 0, 1);
        light::init_light_dist(&mut (*state).LightDist, &mut (*state).LightDistInc);
        (*state).FilterTab = light::init_filter_tab(check_failed);
        light::init_byte_fog(&mut (*state).ByteFog);
        (*state).TopItemToUnlock = ptr::addr_of_mut!((*state).ItemsToUnlock).cast();
    }
}

struct Bindings {
    failed: Option<unsafe extern "C" fn() -> i32>,
    constructed: bool,
    // Cached once by `init()`: the native `LIGHT_SPATIAL_FUNC` signature
    // (`FTextureInfo&, FLightInfo*, BYTE*, BYTE*`) has no room for a
    // services-table argument, so the spatial dispatch entry points below
    // read these instead of receiving them per call.
    render_services: Option<Hp2RenderServices>,
    light_services: Option<Hp2LightServices>,
}
struct SharedBindings(UnsafeCell<Bindings>);
unsafe impl Sync for SharedBindings {}
static BINDINGS: SharedBindings = SharedBindings(UnsafeCell::new(Bindings {
    failed: None,
    constructed: false,
    render_services: None,
    light_services: None,
}));

/// Private unwind token never crosses an extern-C boundary. Native exceptions
/// remain in the native stack's exception_ptr; Rust unwinds only its own frames.
struct CallbackFailure;
pub fn check_callback() {
    if unsafe { (*BINDINGS.0.get()).failed.is_some_and(|f| f() != 0) } {
        resume_unwind(Box::new(CallbackFailure));
    }
}
pub fn boundary<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(error) if error.is::<CallbackFailure>() => fallback,
        Err(error) => resume_unwind(error),
    }
}
#[inline]
pub fn callback<T>(f: impl FnOnce() -> T) -> T {
    let value = f();
    check_callback();
    value
}

/// Native leaves: construct SDK subobjects and emit the SDK lifecycle log.
/// Each leaf captures exceptions itself.
#[repr(C)]
pub struct Lifecycle {
    pub construct: unsafe extern "C" fn(),
    pub log: unsafe extern "C" fn(u32),
    pub failed: unsafe extern "C" fn() -> i32,
    pub destroy: unsafe extern "C" fn(),
}
unsafe extern "C" {
    fn atexit(callback: unsafe extern "C" fn()) -> i32;
}
pub unsafe fn init(svc: &Hp2LightServices, leaves: &Lifecycle, render: &Hp2RenderServices) {
    unsafe {
        (*BINDINGS.0.get()).failed = Some(leaves.failed);
        (*BINDINGS.0.get()).render_services = Some(*render);
        (*BINDINGS.0.get()).light_services = Some(*svc);
        let s = state();
        let initialize_static_fields = !(*BINDINGS.0.get()).constructed;
        if initialize_static_fields {
            callback(|| (leaves.construct)());
            // SDK subobjects have process lifetime, not Init/Exit lifetime.
            // Register their native destructor directly: no Rust frame is live
            // when the C runtime invokes it at shutdown.
            atexit(leaves.destroy);
            (*BINDINGS.0.get()).constructed = true;
        }
        initialize_tables(s, initialize_static_fields, svc.check_failed);
        check_callback();
        callback(|| (leaves.log)(0));
    }
}
pub unsafe fn exit(leaves: &Lifecycle) {
    unsafe {
        callback(|| (leaves.log)(1));
    }
}

// ---------------------------------------------------------------------------
// Spatial kernel dispatch (`FLightManager::spatial_*` / `LightSpatialAddress`
// / `MAKE_SPATIAL_FUNC`, deleted from `UnLight.cpp`). Each entry point below
// replaces one of the 12 deleted C++ statics: its address is taken once in
// `init()` and stored in `Effects[]`; native code later calls through it
// directly as `FLightInfo::Effect::SpatialFxFunc`, with the fixed
// `LIGHT_SPATIAL_FUNC` signature (`FTextureInfo&, FLightInfo*, BYTE*,
// BYTE*`). Each must stay a real, individually addressable `extern "C"`
// symbol, exactly as the deleted C++ statics were. A panic here aborts (no
// unwinding across `extern "C"` since Rust 1.81): acceptable, matching the
// guard-free shell policy already used for still-C++ virtual dispatch.
// ---------------------------------------------------------------------------

/// Shared body for every `hp2_rs_light_spatial_*` entry point below: the
/// `Hp2LightSpatialParams` marshal `LightSpatialForward` used to perform in
/// C++. `map` is always `&LightState().LightMap`; `UClamp`/`VClamp` sit at
/// byte offsets 68/72 of `FTextureInfo` (verified via `offsetof` against the
/// real engine header). `info` is always `&FirstLight[i]`, i.e. Rust's own
/// `LightInfo` allocation, not a foreign mirror; same for `Info->Actor`
/// (`AActor`, golden-asserted in `HP2EngineAbiAsserts.cpp`) and
/// `LightState().LevelInfo` (`ALevelInfo`, same). `render_services`/
/// `light_services` are the copies `init()` cached in `Bindings`: the fixed
/// native call site has no services-table argument to pass per call.
/// `TimeSeconds` is read only for the Cycle-kind kernels (SearchLight through
/// Interference) and the view vector only for Spotlight, exactly as
/// `Effect_Cycle::Init` and `Effect_Spotlight::Init` did.
unsafe fn spatial_marshal(
    kind: light::Spatial,
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe {
        let bindings = &*BINDINGS.0.get();
        let render = bindings.render_services.expect("light spatial dispatch before init");
        let light_services = bindings.light_services.expect("light spatial dispatch before init");
        let tex = map.cast::<u8>();
        let uclamp = *tex.add(68).cast::<i32>();
        let vclamp = *tex.add(72).cast::<i32>();
        let manager = state();
        let src_len = ((*manager).ShadowMaskSpace * 8).max(0) as usize;
        let dest_len = (uclamp * vclamp).max(0) as usize;
        let mut view = FVector { x: 0.0, y: 0.0, z: 0.0 };
        if kind == light::Spatial::Spotlight {
            callback(|| (light_services.view_vector)((*info.cast::<LightInfo>()).Actor, (&mut view as *mut FVector).cast()));
        }
        spatial_apply_with_gmath(
            &light::GMath::new(&render),
            kind,
            map,
            info,
            std::slice::from_raw_parts(src, src_len),
            std::slice::from_raw_parts_mut(dest, dest_len),
            view,
        );
    }
}

type SpatialFn = unsafe extern "C" fn(*const c_void, *const c_void, *const u8, *mut u8);

/// Resolves an `Effects[]` slot's `SpatialFxFunc`, replacing the native
/// `LightSpatialAddress` switch. `kind` is the same `LSK_*`-style code
/// 0..=11 the `Effects[]` build table in `init()` already used; an unlisted
/// code (there are none today) falls back to `None`, matching
/// `LightSpatialAddress`'s `default`.
fn spatial_fn_address(kind: u32) -> *mut c_void {
    let f: SpatialFn = match kind {
        1 => hp2_rs_light_spatial_search_light,
        2 => hp2_rs_light_spatial_rotor,
        3 => hp2_rs_light_spatial_slow_wave,
        4 => hp2_rs_light_spatial_fast_wave,
        5 => hp2_rs_light_spatial_shock,
        6 => hp2_rs_light_spatial_disco,
        7 => hp2_rs_light_spatial_interference,
        8 => hp2_rs_light_spatial_spotlight,
        9 => hp2_rs_light_spatial_cylinder,
        10 => hp2_rs_light_spatial_non_incidence,
        11 => hp2_rs_light_spatial_shell,
        _ => hp2_rs_light_spatial_none,
    };
    f as *mut c_void
}

/// Resolves the selected `Effects[]` spatial function to its shared Rust kernel.
///
/// # Safety
/// `effect` must be a valid index in the selected manager's `Effects` array.
pub unsafe fn spatial_for_effect(effect: u8) -> light::Spatial {
    let address = unsafe { (*state()).Effects[effect as usize].SpatialFxFunc };
    let kind = (0..=11).find(|kind| spatial_fn_address(*kind) == address).unwrap_or(0);
    match kind {
        1 => light::Spatial::SearchLight,
        2 => light::Spatial::Rotor,
        3 => light::Spatial::SlowWave,
        4 => light::Spatial::FastWave,
        5 => light::Spatial::Shock,
        6 => light::Spatial::Disco,
        7 => light::Spatial::Interference,
        8 => light::Spatial::Spotlight,
        9 => light::Spatial::Cylinder,
        10 => light::Spatial::NonIncidence,
        11 => light::Spatial::Shell,
        _ => light::Spatial::None,
    }
}

pub unsafe extern "C" fn hp2_rs_light_spatial_none(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::None, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_search_light(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::SearchLight, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_rotor(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Rotor, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_slow_wave(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::SlowWave, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_fast_wave(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::FastWave, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_shock(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Shock, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_disco(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Disco, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_interference(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Interference, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_spotlight(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Spotlight, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_cylinder(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Cylinder, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_non_incidence(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::NonIncidence, map, info, src, dest) }
}
pub unsafe extern "C" fn hp2_rs_light_spatial_shell(
    map: *const c_void,
    info: *const c_void,
    src: *const u8,
    dest: *mut u8,
) {
    unsafe { spatial_marshal(light::Spatial::Shell, map, info, src, dest) }
}

#[repr(C)]
pub struct ActorInput {
    pub frame: *mut c_void,
    pub actor: *mut c_void,
    pub leaf_lights: *mut c_void,
    pub volumetrics: *mut c_void,
    pub level: *mut c_void,
    pub level_info: *mut c_void,
    pub zone: *mut c_void,
    pub zone_number: i32,
    pub begin: light_actor::ActorBeginIo,
    pub unlit: u32,
    pub leaf: i32,
    pub leaves: i32,
    pub rend_map: i32,
    pub no_lighting: u32,
    pub cache_id: u64,
    pub candidates: light_actor::CandidateIo,
    pub delta_time: f32,
}
#[repr(C)]
pub struct ActorLeaves {
    pub read_context: unsafe extern "C" fn(*mut ActorInput, u32),
    pub set_zone: unsafe extern "C" fn(),
    pub read_begin: unsafe extern "C" fn(*mut ActorInput),
    pub read_gate: unsafe extern "C" fn(*mut ActorInput, u32),
    pub read_cache: unsafe extern "C" fn(*mut ActorInput),
    pub read_candidates: unsafe extern "C" fn(*mut ActorInput),
}

pub trait ActorSetupOps {
    type Error;
    type Candidate: light_actor::CandidateOps<Error = Self::Error>;
    type Tail: light_actor::SetupTailOps<Error = Self::Error>;

    fn mark_begin(&mut self) -> Result<(), Self::Error>;
    fn read_context(&mut self, input: *mut ActorInput, field: u32) -> Result<(), Self::Error>;
    fn set_zone(&mut self) -> Result<(), Self::Error>;
    fn read_begin(&mut self, input: *mut ActorInput) -> Result<(), Self::Error>;
    fn read_gate(&mut self, input: *mut ActorInput, field: u32) -> Result<(), Self::Error>;
    fn read_cache(&mut self, input: *mut ActorInput) -> Result<(), Self::Error>;
    fn cache_get(&mut self, id: u64) -> Result<*mut c_void, Self::Error>;
    fn cache_create(&mut self, id: u64, size: u64) -> Result<std::ptr::NonNull<c_void>, Self::Error>;
    fn read_candidates(&mut self, input: *mut ActorInput) -> Result<(), Self::Error>;
    fn candidate_ops(&mut self) -> &mut Self::Candidate;
    fn tail_ops(&mut self) -> &mut Self::Tail;
}

pub unsafe fn setup_actor_with_ops<O: ActorSetupOps>(
    ops: &mut O,
    input: *mut ActorInput,
) -> Result<u32, O::Error> {
    unsafe {
        let state = state();
        ops.mark_begin()?;
        (*state).Frame = (*input).frame;
        ops.read_context(input, 0)?;
        (*state).Level = (*input).level;
        ops.read_context(input, 1)?;
        (*state).LevelInfo = (*input).level_info;
        (*state).Actor = (*input).actor;
        ops.read_context(input, 2)?;
        (*state).Zone = (*input).zone;
        (*state).MapCoords = ptr::null_mut();
        (*state).LastLight = ptr::addr_of_mut!((*state).FirstLight).cast();
        if (*input).zone_number == 0 {
            ops.set_zone()?;
        }
        ops.read_begin(input)?;
        light_actor::actor_begin(&mut (*input).begin);
        (*state).Diffuse = (*input).begin.diffuse;
        (*state).Specular = (*input).begin.specular;
        (*state).SpecMinCos = (*input).begin.spec_min_cos;
        (*state).AmbientVector = (*input).begin.ambient;
        ops.read_gate(input, 0)?;
        if (*input).unlit != 0 { return Ok(0); }
        ops.read_gate(input, 1)?;
        if (*input).leaf == -1 { return Ok(0); }
        ops.read_gate(input, 2)?;
        if (*input).leaves == 0 { return Ok(0); }
        ops.read_gate(input, 3)?;
        if (*input).rend_map != 5 { return Ok(0); }
        ops.read_gate(input, 4)?;
        if (*input).no_lighting != 0 { return Ok(0); }
        ops.read_cache(input)?;
        (*input).candidates.delta = (((*input).delta_time * 768.0) as i32).clamp(0, 255);
        let mut block = ops.cache_get((*input).cache_id)?;
        let first = block.is_null();
        if first {
            block = ops.cache_create(
                (*input).cache_id,
                ((light_actor::MAX_ACTOR_LIGHTS + 1)
                    * std::mem::size_of::<light_actor::SenderEntry>()) as u64,
            )?.as_ptr();
            block.cast::<i32>().write(0);
        }
        ops.read_candidates(input)?;
        (*input).candidates.first_see = u32::from(first);
        light_actor::collect_candidates_with(
            ops.candidate_ops(),
            &mut (*state).Consider,
            &(*input).candidates,
            std::slice::from_raw_parts_mut(
                block.cast::<u8>().add(16).cast(),
                light_actor::MAX_ACTOR_LIGHTS,
            ),
            &mut *block.cast::<i32>(),
        )?;
        let mut tail = light_actor::SetupTailIo {
            frame: (*state).Frame,
            volumetrics: (*input).volumetrics,
            result: 0,
            pad: 0,
        };
        light_actor::setup_tail_with_ops(ops.tail_ops(), &mut tail)?;
        Ok(tail.result)
    }
}

struct NativeActorSetupOps<'a> {
    svc: &'a Hp2LightServices,
    leaves: &'a ActorLeaves,
}

impl<'a> NativeActorSetupOps<'a> {
    fn new(svc: &'a Hp2LightServices, leaves: &'a ActorLeaves) -> Self {
        Self { svc, leaves }
    }
}

impl light_actor::CandidateOps for NativeActorSetupOps<'_> {
    type Error = std::convert::Infallible;
    fn sender_alive(&mut self, entry: &light_actor::SenderEntry) -> bool {
        unsafe { callback(|| (self.svc.sender_alive)(entry.index, entry.actor)) != 0 }
    }
    fn world_light_radius(&mut self, light: *mut c_void) -> f32 {
        unsafe { callback(|| (self.svc.world_light_radius)(light)) }
    }
    fn line_check(&mut self, actor: *mut c_void, light: *mut c_void) -> Result<bool, Self::Error> {
        Ok(unsafe { callback(|| (self.svc.line_check)(actor, light)) != 0 })
    }
    fn add_light(&mut self, actor: *mut c_void, light: *mut c_void) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.svc.add_light)(actor, light)); }
        Ok(())
    }
}

impl light_actor::SetupTailOps for NativeActorSetupOps<'_> {
    type Error = std::convert::Infallible;
    fn compute_light(&mut self, index: u32, frame: *mut c_void) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.svc.light_compute)(index, frame)); }
        Ok(())
    }
    fn stat_vtric_inc(&mut self) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.svc.stat_vtric_inc)()); }
        Ok(())
    }
    fn stat_mesh_light_add(&mut self, count: u32) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.svc.stat_mesh_light_add)(count)); }
        Ok(())
    }
}

impl ActorSetupOps for NativeActorSetupOps<'_> {
    type Error = std::convert::Infallible;
    type Candidate = Self;
    type Tail = Self;
    fn mark_begin(&mut self) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.svc.surf_gmem_mark)(0, 0)); }
        Ok(())
    }
    fn read_context(&mut self, input: *mut ActorInput, field: u32) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.leaves.read_context)(input, field)); }
        Ok(())
    }
    fn set_zone(&mut self) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.leaves.set_zone)()); }
        Ok(())
    }
    fn read_begin(&mut self, input: *mut ActorInput) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.leaves.read_begin)(input)); }
        Ok(())
    }
    fn read_gate(&mut self, input: *mut ActorInput, field: u32) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.leaves.read_gate)(input, field)); }
        Ok(())
    }
    fn read_cache(&mut self, input: *mut ActorInput) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.leaves.read_cache)(input)); }
        Ok(())
    }
    fn cache_get(&mut self, id: u64) -> Result<*mut c_void, Self::Error> {
        Ok(unsafe { callback(|| (self.svc.surf_cache_get)(id)) })
    }
    fn cache_create(&mut self, id: u64, size: u64) -> Result<std::ptr::NonNull<c_void>, Self::Error> {
        let data = unsafe { callback(|| (self.svc.surf_cache_create)(id, size, 0, 0)) };
        Ok(std::ptr::NonNull::new(data).expect("GCache.Create returned null"))
    }
    fn read_candidates(&mut self, input: *mut ActorInput) -> Result<(), Self::Error> {
        unsafe { callback(|| (self.leaves.read_candidates)(input)); }
        Ok(())
    }
    fn candidate_ops(&mut self) -> &mut Self::Candidate { self }
    fn tail_ops(&mut self) -> &mut Self::Tail { self }
}

pub unsafe fn setup_actor(
    svc: &Hp2LightServices,
    leaves: &ActorLeaves,
    input: *mut ActorInput,
) -> u32 {
    let mut ops = NativeActorSetupOps::new(svc, leaves);
    match unsafe { setup_actor_with_ops(&mut ops, input) } {
        Ok(result) => result,
        Err(never) => match never {},
    }
}
/// # Safety
/// Called on the renderer thread after Init, within the SDK's fixed slot bound.
pub unsafe fn reserve_slot() -> *mut *mut c_void {
    unsafe {
        let s = state();
        let slot = (*s).TopItemToUnlock;
        (*s).TopItemToUnlock = slot.add(1);
        slot
    }
}

/// # Safety
/// A preceding cache lookup must have reserved a slot in the current setup.
pub unsafe fn previous_slot() -> *mut *mut c_void {
    unsafe { (*state()).TopItemToUnlock.sub(1) }
}

/// # Safety
/// The manager must be initialized and accessed only on the renderer thread.
pub unsafe fn unlock_pending() -> bool {
    unsafe { (*state()).TopItemToUnlock > ptr::addr_of_mut!((*state()).ItemsToUnlock).cast() }
}

/// # Safety
/// The current setup must have a pending item. Removal precedes native Unlock,
/// including when that SDK operation throws; no nesting restoration is applied.
pub unsafe fn pop_unlock() -> *mut c_void {
    unsafe {
        let s = state();
        (*s).TopItemToUnlock = (*s).TopItemToUnlock.sub(1);
        *(*s).TopItemToUnlock
    }
}

/// # Safety
/// The single renderer thread must have completed actor or surface setup.
pub unsafe fn light_count() -> usize {
    unsafe {
        (*state())
            .LastLight
            .offset_from(ptr::addr_of!((*state()).FirstLight).cast()) as usize
    }
}

/// `LightSvcHasRoom()`: `LastLight < FinalLight`.
///
/// # Safety
/// The manager must be initialized.
pub unsafe fn light_has_room() -> bool {
    unsafe { (*state()).LastLight < (*state()).FinalLight }
}

/// `LightSvcActorAt(i)`: `FirstLight[i].Actor`.
///
/// # Safety
/// `i` must be a valid index below the current light count.
pub unsafe fn light_actor_at(i: u32) -> *mut c_void {
    unsafe { (*state()).FirstLight[i as usize].Actor }
}

/// `LightSvcAppend(actor, opt)`: fills `LastLight` and advances it.
///
/// # Safety
/// `LastLight` must have room (checked by `light_has_room`).
pub unsafe fn light_append(actor: *mut c_void, opt: u32) {
    unsafe {
        let s = state();
        (*(*s).LastLight).Actor = actor;
        (*(*s).LastLight).Opt = opt as i32;
        (*s).LastLight = (*s).LastLight.add(1);
    }
}

/// `LightSvcSetVolumetric(i)`: `FirstLight[i].IsVolumetric = 1`.
///
/// # Safety
/// `i` must be a valid index below the current light count.
pub unsafe fn light_set_volumetric(i: u32) {
    unsafe { (*state()).FirstLight[i as usize].IsVolumetric = 1 }
}

/// `LightSvcIsVolumetric(i)`.
///
/// # Safety
/// `i` must be a valid index below the current light count.
pub unsafe fn light_is_volumetric(i: u32) -> bool {
    unsafe { (*state()).FirstLight[i as usize].IsVolumetric != 0 }
}

/// `LightSvcFloatColor(i)`: `&FirstLight[i].FloatColor`.
///
/// # Safety
/// `i` must be a valid index below the current light count.
pub unsafe fn light_float_color(i: u32) -> *mut c_void {
    unsafe { ptr::addr_of_mut!((*state()).FirstLight[i as usize].FloatColor).cast() }
}

/// `LightSvcVtricReset()`: `LastVtric = FirstVtric`.
///
/// # Safety
/// The manager must be initialized.
pub unsafe fn vtric_reset() {
    unsafe { (*state()).LastVtric = ptr::addr_of_mut!((*state()).FirstVtric).cast() }
}

/// `LightSvcVtricPush(i)`: `*LastVtric++ = &FirstLight[i]`.
///
/// # Safety
/// `i` must be a valid `FirstLight` index; `LastVtric` must have room (<=256 entries).
pub unsafe fn vtric_push(i: u32) {
    unsafe {
        let s = state();
        let entry = ptr::addr_of_mut!((*s).FirstLight[i as usize]);
        *(*s).LastVtric = entry;
        (*s).LastVtric = (*s).LastVtric.add(1);
    }
}

/// `LightSvcVtricCount()`: `LastVtric - FirstVtric`.
///
/// # Safety
/// The manager must be initialized.
pub unsafe fn vtric_count() -> u32 {
    unsafe {
        (*state())
            .LastVtric
            .offset_from(ptr::addr_of!((*state()).FirstVtric).cast()) as u32
    }
}

unsafe extern "C" fn vtric_compare(a: *const c_void, b: *const c_void) -> std::os::raw::c_int {
    unsafe {
        let pa = *a.cast::<*mut LightInfo>();
        let pb = *b.cast::<*mut LightInfo>();
        crate::light_info::dist_compare((*pa).Location, (*pb).Location) as std::os::raw::c_int
    }
}

unsafe extern "C" {
    fn qsort(
        base: *mut c_void,
        nel: usize,
        width: usize,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> std::os::raw::c_int>,
    );
}

/// `LightSvcVtricSort(n)`: `appQsort` over the first `n` vtric entries. `appQsort`
/// is itself a thin libc `qsort` wrapper (`Core/Src/UnAnsi.cpp`) kept in C++ only
/// because its general comparator may be a C++ callback that can raise; this
/// comparator is pure Rust, so there is nothing to cross an extern "C" frame and
/// libc `qsort` is called directly (same precedent as `hp2-engine::levact`).
///
/// # Safety
/// `n` must not exceed the populated prefix of `FirstVtric`.
pub unsafe fn vtric_sort(n: u32) {
    unsafe {
        qsort(
            ptr::addr_of_mut!((*state()).FirstVtric).cast(),
            n as usize,
            std::mem::size_of::<*mut LightInfo>(),
            Some(vtric_compare),
        );
    }
}

/// `LightSurfGetCounter(field)`: 0 = `DynamicLights`, 1 = `MovingLights`, else
/// `StaticLightingChanged`.
///
/// # Safety
/// The manager must be initialized.
pub unsafe fn counter_get(field: u32) -> u32 {
    unsafe {
        let s = &*state();
        (match field {
            0 => s.DynamicLights,
            1 => s.MovingLights,
            _ => s.StaticLightingChanged,
        }) as u32
    }
}

/// `LightSurfSetCounter(field, v)`.
///
/// # Safety
/// The manager must be initialized.
pub unsafe fn counter_set(field: u32, v: u32) {
    unsafe {
        let s = &mut *state();
        match field {
            0 => s.DynamicLights = v as i32,
            1 => s.MovingLights = v as i32,
            _ => s.StaticLightingChanged = v as i32,
        }
    }
}

/// `LightEffectFlags(effect)`: bit 0 = `IsSpatialDynamic`, bit 1 = `IsMergeDynamic`.
///
/// # Safety
/// `effect` must be a valid `Effects` index (`< LE_MAX`).
pub unsafe fn effect_flags(effect: u8) -> u32 {
    unsafe {
        let e = &(*state()).Effects[effect as usize];
        u32::from(e.IsSpatialDynamic != 0) | (u32::from(e.IsMergeDynamic != 0) << 1)
    }
}

/// `LightCopyEffect(index, out)`: copies `Effects[index]` to `out`, an `Effect`
/// always living at `&FLightInfo::Effect` of a `FirstLight` slot.
///
/// # Safety
/// `index` must be a valid `Effects` index; `out` must be writable for one `Effect`.
pub unsafe fn copy_effect(index: u32, out: *mut c_void) {
    unsafe { *out.cast::<Effect>() = (*state()).Effects[index as usize] }
}

/// `LightSurfSetShadowBits(i, bits)`.
///
/// # Safety
/// `i` must be a valid `FirstLight` index.
pub unsafe fn set_shadow_bits(i: u32, bits: *mut c_void) {
    unsafe { (*state()).FirstLight[i as usize].ShadowBits = bits.cast() }
}

/// `LightSurfSetIllum(i, ptr)`.
///
/// # Safety
/// `i` must be a valid `FirstLight` index.
pub unsafe fn set_illum(i: u32, illum: *mut c_void) {
    unsafe { (*state()).FirstLight[i as usize].IlluminationMap = illum.cast() }
}

/// `LightSurfMarkVolumetric(actor)`: sets `IsVolumetric` on the existing
/// `FirstLight` slot for `actor`, or appends an `ALO_NotLight` slot.
///
/// # Safety
/// `LastLight` must have room when `actor` is not already present.
pub unsafe fn mark_volumetric(actor: *mut c_void) {
    unsafe {
        let s = state();
        let mut info = ptr::addr_of_mut!((*s).FirstLight).cast::<LightInfo>();
        while info < (*s).LastLight {
            if (*info).Actor == actor {
                (*info).IsVolumetric = 1;
                return;
            }
            info = info.add(1);
        }
        (*(*s).LastLight).Actor = actor;
        (*(*s).LastLight).Opt = light_actor::ALO_NOT_LIGHT as i32;
        (*(*s).LastLight).IsVolumetric = 1;
        (*s).LastLight = (*s).LastLight.add(1);
    }
}

/// `LightSurfVtricCommit(indices, n, _)`: rebuilds `FirstVtric` from the given
/// `FirstLight` indices.
///
/// # Safety
/// `indices` must hold `n` valid `FirstLight` indices.
pub unsafe fn vtric_commit(indices: *const u32, n: u32) {
    unsafe {
        let s = state();
        (*s).LastVtric = ptr::addr_of_mut!((*s).FirstVtric).cast();
        for i in 0..n {
            let idx = *indices.add(i as usize) as usize;
            *(*s).LastVtric = ptr::addr_of_mut!((*s).FirstLight[idx]);
            (*s).LastVtric = (*s).LastVtric.add(1);
        }
    }
}

/// C++ `LightLinkRead(link, actor, next)`: reads an `FVolActorLink` leaf-light node.
///
/// # Safety
/// `link` must be a valid, live `FVolActorLink`.
pub unsafe fn read_vol_actor_link(link: *const c_void) -> (*mut c_void, *mut c_void) {
    unsafe {
        let l = &*link.cast::<hp2_abi::render_scene::FVolActorLink>();
        (l.actor, l.next.cast())
    }
}

/// C++ `LightActorLinkRead(link, actor, next)`: reads an `FActorLink`, used by
/// both `SurfLights` and `Volumetrics`.
///
/// # Safety
/// `link` must be a valid, live `FActorLink`.
pub unsafe fn read_actor_link(link: *const c_void) -> (*mut c_void, *mut c_void) {
    unsafe {
        let l = &*link.cast::<hp2_abi::render_scene::FActorLink>();
        (l.actor, l.next.cast())
    }
}
/// C++ `LightFogReadLight` (UnLight.cpp, deleted in this change): private
/// `FLightInfo` field access, inlined now that `FirstLight` is Rust's own array.
/// `Actor->LightSource`/`Actor->bDarkLight` are only read when `Opt != ALO_NOT_LIGHT`,
/// matching the original guard (an unprocessed slot's `Actor` is not valid to deref).
///
/// # Safety
/// `info` must be a valid, live `LightInfo`; if `info.Opt != ALO_NOT_LIGHT`,
/// `info.Actor` must be a valid `AActor`.
pub unsafe fn fog_light_at(info: *const LightInfo) -> light::FogLight {
    unsafe {
        let info = &*info;
        let (light_source, dark_light) = if info.Opt != light::ALO_NOT_LIGHT as i32 {
            let a = &*(info.Actor as *const AActor);
            (a.light_source as u32, a.b_dark_light() as u32)
        } else {
            (0, 0)
        };
        light::FogLight {
            location: FVector {
                x: info.Location[0],
                y: info.Location[1],
                z: info.Location[2],
            },
            light_dir: FVector {
                x: info.LightDir[0],
                y: info.LightDir[1],
                z: info.LightDir[2],
            },
            float_color: info.FloatColor,
            radius: info.Radius,
            rradius_mult: info.RRadiusMult,
            opt: info.Opt as u32,
            light_source,
            dark_light,
        }
    }
}

/// C++ `LightFogReadVtric` (UnLight.cpp, deleted in this change): private
/// `FLightInfo` field access through `FirstVtric`'s pointer indirection, inlined
/// the same way as `fog_light_at`. `Actor` is always valid here (only actual
/// lights are ever appended to `FirstVtric`), so the read is unconditional,
/// matching the original.
///
/// # Safety
/// `info` must be a valid, live `LightInfo` with a valid `Actor`.
pub unsafe fn fog_vtric_at(info: *const LightInfo) -> light::FogVtric {
    unsafe {
        let info = &*info;
        let a = &*(info.Actor as *const AActor);
        light::FogVtric {
            location: FVector {
                x: info.Location[0],
                y: info.Location[1],
                z: info.Location[2],
            },
            vol_radius_squared: info.VolRadiusSquared,
            rvol_radius: info.RVolRadius,
            rvol_radius_squared: info.RVolRadiusSquared,
            vol_brightness: info.VolBrightness,
            location_size_squared: info.LocationSizeSquared,
            vol_inside: (info.VolInside != 0) as u32,
            ambient: (a.light_source as u32 == light::LD_AMBIENT) as u32,
            color: info.VolumetricColor,
        }
    }
}

/// # Safety
/// The concrete manager must be initialized, its current setup must remain
/// live, and calls must obey the SDK renderer's single-threaded lifetime.
pub unsafe fn light_and_fog(
    services: &Hp2RenderServices,
    vertex: &mut light::FogVertex,
    flags: u32,
) -> bool {
    unsafe { light_and_fog_with_gmath(&light::GMath::new(services), vertex, flags) }
}

/// Runs the selected manager's shared fog policy with caller-owned table math.
///
/// # Safety
/// The concrete manager must be initialized and its current setup remain live.
pub unsafe fn light_and_fog_with_gmath(
    gmath: &light::GMath<'_>,
    vertex: &mut light::FogVertex,
    flags: u32,
) -> bool {
    unsafe {
        let s = state();
        let params = light::FogParams {
            ambient: (*s).AmbientVector,
            diffuse: (*s).Diffuse,
            specular: (*s).Specular,
            spec_min_cos: (*s).SpecMinCos,
        };
        let n = if flags & light::PF_UNLIT == 0 {
            light_count()
        } else {
            0
        };
        let nv = if flags & (light::PF_UNLIT | light::PF_RENDER_FOG) == light::PF_RENDER_FOG {
            (*s).LastVtric
                .offset_from(ptr::addr_of!((*s).FirstVtric).cast()) as usize
        } else {
            0
        };
        let first_light: *const LightInfo = ptr::addr_of!((*s).FirstLight).cast();
        let first_vtric: *const *mut LightInfo = ptr::addr_of!((*s).FirstVtric).cast();
        let lights = (0..n).map(|i| fog_light_at(first_light.add(i)));
        let volumes = (0..nv).map(|i| fog_vtric_at(*first_vtric.add(i)));
        light::light_and_fog(
            gmath,
            &params,
            vertex,
            flags,
            lights,
            volumes,
            &mut (*s).FogRejectionMethod,
        )
    }
}

/// Applies one source spatial-effect kernel to the selected manager's light info.
///
/// # Safety
/// `map`, `info`, `src`, and `dest` must cover the source extents for this effect.
pub unsafe fn spatial_apply_with_gmath(
    gmath: &light::GMath<'_>,
    kind: light::Spatial,
    map: *const c_void,
    info: *const c_void,
    src: &[u8],
    dest: &mut [u8],
    view: FVector,
) {
    unsafe {
        let tex = map.cast::<u8>();
        let uclamp = *tex.add(68).cast::<i32>();
        let info = &*info.cast::<LightInfo>();
        let actor = &*info.Actor.cast::<AActor>();
        let s = state();
        let level_info = &*(*s).LevelInfo.cast::<ALevelInfo>();
        let time_seconds = if matches!(
            kind,
            light::Spatial::SearchLight
                | light::Spatial::Rotor
                | light::Spatial::SlowWave
                | light::Spatial::FastWave
                | light::Spatial::Shock
                | light::Spatial::Disco
                | light::Spatial::Interference
        ) {
            level_info.time_seconds
        } else {
            0.0
        };
        let p = light::SpatialParams {
            uclamp,
            shadow_mask_u: (*s).ShadowMaskU,
            shadow_skip: (*s).ShadowSkip,
            min_u: info.MinU,
            max_u: info.MaxU,
            min_v: info.MinV,
            max_v: info.MaxV,
            radius: info.Radius,
            rradius: info.RRadius,
            radius_inner: info.RadiusInner,
            diffuse: info.Diffuse,
            actor_location: actor.location,
            view,
            light_period: actor.light_period,
            light_phase: actor.light_phase,
            light_cone: actor.light_cone,
            time_seconds,
            vertex_base: FVector { x: (*s).VertexBase[0], y: (*s).VertexBase[1], z: (*s).VertexBase[2] },
            vertex_du: FVector { x: (*s).VertexDU[0], y: (*s).VertexDU[1], z: (*s).VertexDU[2] },
            vertex_dv: FVector { x: (*s).VertexDV[0], y: (*s).VertexDV[1], z: (*s).VertexDV[2] },
        };
        let dest_len = (uclamp * *tex.add(72).cast::<i32>()).max(0) as usize;
        let src_len = ((*s).ShadowMaskSpace * 8).max(0) as usize;
        light::spatial(
            gmath,
            kind,
            &p,
            &*(info.LightDistTab as *const [f32; light::LIGHT_DIST_LEN]),
            &(*s).LightDist,
            &src[..src_len],
            &mut dest[..dest_len],
        );
    }
}
const _: () = assert!(std::mem::size_of::<LightInfo>() == 232);
const _: () = {
    use std::mem::{align_of, offset_of, size_of};
    assert!(size_of::<State>() == 122592 && align_of::<State>() == 8);
    assert!(offset_of!(State, Frame) == 96 && offset_of!(State, Mark) == 112);
    assert!(offset_of!(State, LightMap) == 168 && offset_of!(State, LightMip) == 568);
    assert!(offset_of!(State, FirstLight) == 720 && offset_of!(State, FirstVtric) == 60112);
    assert!(offset_of!(State, AmbientVector) == 62176 && offset_of!(State, LightDist) == 62212);
    assert!(offset_of!(State, Effects) == 113424 && offset_of!(State, ItemsToUnlock) == 113744);
    assert!(offset_of!(State, SurfLocalMark) == 115808 && offset_of!(State, Consider) == 115848);
    assert!(size_of::<ActorInput>() == 208 && offset_of!(ActorInput, begin) == 60);
    assert!(offset_of!(ActorInput, candidates) == 152);
};
