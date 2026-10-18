//! Engine services the UnRender port (`UnRender.cpp`, class `URender`) reaches
//! through `GUrenServices`. Passed per call, like `Hp2RenderServices`; nothing
//! is installed globally. The C++ side defines `UrenServicesTable` and
//! `GUrenServices` in `HarryPotter2/Unreal/Render/Src/UnRender.cpp`.
//!
//! Conventions:
//! - Engine objects are `*mut c_void`. Cast to the mirror in
//!   `hp2_abi::render_scene` (or `hp2_abi::render_span`) at the use site. The
//!   C++ header that owns each layout pins it with `static_assert`.
//! - `Hp2UrenGlobals` holds **addresses** of C++ statics (`&GFrame`,
//!   `&URender::DynamicsCache`, ...), not their values. C++ may reassign or
//!   `appRealloc` them, so Rust reads through the address each time. Each field
//!   names the address and the type behind it.
//! - Types here use only primitives, `c_void`, and `Hp2RenderServices`, so
//!   cbindgen can emit them into `hp2_rs.h`.
//! - Not exposed: `GCache.Status(TCHAR*)` (stats text, `Core/Inc/UnCache.h:88`).
//!   Its `TCHAR` buffer width is a C++ build detail, so that path stays in C++.

use std::ffi::c_void;

use crate::services::Hp2RenderServices;

/// Addresses of C++ statics that UnRender.cpp and the URender state share.
/// Filled once by the C++ side (`UrenGlobals`, `UnRender.cpp`) and valid for the
/// process lifetime. Every field is the address of the named symbol.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2UrenGlobals {
    /// `&GFrame`: `FSceneNode**`. Current scene frame (`Render/Src/UnRender.cpp`).
    pub frame: *mut c_void,
    /// `&GNodes`: `FBspNode**`. Model node array.
    pub nodes: *mut c_void,
    /// `&GSurfs`: `FBspSurf**`. Model surface array.
    pub surfs: *mut c_void,
    /// `&GVerts`: `FVert**`. Model vertex array.
    pub verts: *mut c_void,
    /// `&GPoints`: `TArray<FVector>**`. Model point array.
    pub points: *mut c_void,
    /// `&GStat`: `FStats*` (`CORE_API extern FStats GStat`, `Core/Inc/UnStat.h:157`).
    /// Rust increments counters here; its layout must be mirrored before use.
    pub stat: *mut c_void,
    /// `&GSecondsPerCycle`: `FLOAT*` (`CORE_API extern FLOAT`, `Core/Inc/UnGnuG.h:615`).
    pub seconds_per_cycle: *mut c_void,
    /// `&GDynMem`: `FMemStack*` (`RENDER_API extern FMemStack GDynMem, GSceneMem`,
    /// `Render/Inc/Render.h:195`; defined in `Render.cpp:17`).
    pub dyn_mem: *mut c_void,
    /// `&GSceneMem`: `FMemStack*` (`Render/Inc/Render.h:195`).
    pub scene_mem: *mut c_void,
    /// `&GMem`: `FMemStack*` (`CORE_API extern FMemStack GMem`, `Core/Inc/Core.h:539`).
    pub gmem: *mut c_void,
    /// `&URender::VectorMem`: `FMemStack*` (`Render/Inc/Render.h:452`, C++-owned).
    pub vector_mem: *mut c_void,
    /// `&URender::DynamicsCache`: `TArray<URender::FDynamicsCache>*`
    /// (`Render/Inc/Render.h:445`, C++-owned; indexed by `iNode`).
    pub dynamics_cache: *mut c_void,
    /// `&URender::NumDynLightSurfs`: `INT*` (`Render/Inc/Render.h:375`, C++-owned).
    pub num_dyn_light_surfs: *mut c_void,
    /// `&URender::NumDynLightLeaves`: `INT*` (`Render/Inc/Render.h:376`, C++-owned).
    pub num_dyn_light_leaves: *mut c_void,
    /// `&URender::MaxSurfLights`: `INT*` (`Render/Inc/Render.h:377`, C++-owned).
    pub max_surf_lights: *mut c_void,
    /// `&URender::MaxLeafLights`: `INT*` (`Render/Inc/Render.h:378`, C++-owned).
    pub max_leaf_lights: *mut c_void,
    /// `URender::DynLightSurfs`: `INT[MAX_DYN_LIGHT_SURFS]` (2048) array base
    /// (`Render/Inc/Render.h:373,379`, C++-owned). Same address as `&DynLightSurfs[0]`.
    pub dyn_light_surfs: *mut c_void,
    /// `URender::DynLightLeaves`: `INT[MAX_DYN_LIGHT_LEAVES]` (1024) array base
    /// (`Render/Inc/Render.h:374,380`, C++-owned).
    pub dyn_light_leaves: *mut c_void,
    /// `&URender::SurfLights`: `FActorLink***` (`Render/Inc/Render.h:381`, C++-owned;
    /// `appRealloc`'d, so read through the address).
    pub surf_lights: *mut c_void,
    /// `&URender::LeafLights`: `FVolActorLink***` (`Render/Inc/Render.h:382`, C++-owned;
    /// `appRealloc`'d, so read through the address).
    pub leaf_lights: *mut c_void,
}

/// Engine-side table of function pointers, passed to each Rust export.
/// Every fn forwards to one C++ symbol named in its doc comment.
/// Field order matches `UrenServicesTable` in `UnRender.cpp`; the C++ side pins
/// the count with `static_assert(sizeof(Hp2UrenServices) == 17*sizeof(void*))`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UrenServices {
    /// Address table for C++ statics. See `Hp2UrenGlobals`.
    pub globals: *const Hp2UrenGlobals,

    /// `GRenderServices` (`Render/Inc/Render.h:231`, filled in `Render/Src/Render.cpp:108`).
    /// Provides `frand` (`appFrand`), `mem_push`, `mem_mark_save`, `mem_mark_pop`,
    /// and `log_warning`. Those are not duplicated in this table.
    pub render: *const Hp2RenderServices,

    /// Forwards to `appSeconds().GetFloat()` (`Core/Inc/UnFile.h:275`, `FTime`).
    /// Returned as `f32`. Full `FTime` precision is not exposed; a slice that needs it adds a field.
    pub app_seconds: unsafe extern "C" fn() -> f32,
    /// Forwards to `appProcessSeconds().GetFloat()` (`Core/Inc/UnFile.h:281`).
    pub app_process_seconds: unsafe extern "C" fn() -> f32,

    /// Forwards to `debugf(NAME_Log, TEXT("%s"), ...)` with UTF-8 bytes converted to TCHAR
    /// (`Render/Src/Render.cpp` `RenderLogWarning` pattern). `text` is UTF-8, `len` bytes.
    /// The C++ thunk fixes the severity.
    pub log_debugf: unsafe extern "C" fn(text: *const u8, len: usize),

    /// Forwards to `GRandoms->Tick(TimeSeconds)` (`GRandoms` at `Render/Inc/Render.h:227`).
    pub randoms_tick: unsafe extern "C" fn(time_seconds: f32),
    /// Forwards to `GLightManager->Init()` (`GLightManager` at `Render/Inc/Render.h:194`).
    pub light_manager_init: unsafe extern "C" fn(),
    /// Forwards to `GLightManager->Exit()` (`GLightManager` at `Render/Inc/Render.h:194`).
    pub light_manager_exit: unsafe extern "C" fn(),

    /// Forwards to `GCache.Get(Id, Item, Alignment)` (`Core/Inc/UnCache.h:109`;
    /// `GCache` is `FMemCache` at `Engine/Inc/Engine.h:28`).
    /// `item` is `FCacheItem**` (the C++ `FCacheItem*&` out parameter).
    pub cache_get: unsafe extern "C" fn(id: u64, item: *mut *mut c_void, alignment: i32) -> *mut u8,
    /// Forwards to `GCache.Flush(Id, Mask, IgnoreLocked)` (`Core/Inc/UnCache.h:83`).
    /// The no-argument `GCache.Flush()` is `(0, !0, 0)`.
    pub cache_flush: unsafe extern "C" fn(id: u64, mask: u32, ignore_locked: i32),
    /// Forwards to `GCache.Create(Id, Item, CreateSize, Alignment, SafetyPad)`
    /// (`Core/Inc/UnCache.h:84`). `item` is `FCacheItem**`. Callers that omit
    /// Alignment and SafetyPad in C++ pass `DEFAULT_ALIGNMENT` and `0`.
    pub cache_create: unsafe extern "C" fn(
        id: u64,
        item: *mut *mut c_void,
        create_size: i32,
        alignment: i32,
        safety_pad: i32,
    ) -> *mut u8,
    /// Forwards to `FCacheItem::Unlock()` (`Core/Inc/UnCache.h:25`). `item` is `FCacheItem*`.
    pub cache_item_unlock: unsafe extern "C" fn(item: *mut c_void),
    /// Forwards to `MakeCacheID(Base, Frame)` (`Core/Inc/UnCId.h:51`).
    /// `base` is `ECacheIDBase`. `frame` is `const UObject*`.
    pub cache_id_frame: unsafe extern "C" fn(base: u32, frame: *const c_void) -> u64,
    /// Forwards to `MakeCacheID(Base, Obj, Frame)` (`Core/Inc/UnCId.h:56`).
    pub cache_id_obj:
        unsafe extern "C" fn(base: u32, obj: *const c_void, frame: *const c_void) -> u64,
    /// Forwards to `MakeCacheID(Base, Word, Byte, Frame)` (`Core/Inc/UnCId.h:61`).
    pub cache_id_word:
        unsafe extern "C" fn(base: u32, word: u32, byte: u32, frame: *const c_void) -> u64,

    /// Virtual dispatch: `item->Filter(Viewport, Frame, iNode, Outside)` on an
    /// `FDynamicItem*` (`Render/Inc/Render.h:115`). Pointers are C++ objects.
    pub dyn_filter: unsafe extern "C" fn(
        item: *mut c_void,
        viewport: *mut c_void,
        frame: *mut c_void,
        i_node: i32,
        outside: i32,
    ),
    /// Virtual dispatch: `item->PreRender(Viewport, Frame, SpanBuffer, iNode, Volumetrics)`
    /// (`Render/Inc/Render.h:116`).
    pub dyn_pre_render: unsafe extern "C" fn(
        item: *mut c_void,
        viewport: *mut c_void,
        frame: *mut c_void,
        span_buffer: *mut c_void,
        i_node: i32,
        volumetrics: *mut c_void,
    ),
}
