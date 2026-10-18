//! Sprite-filter exports (`hp2-render::sprite_filter`). `FDynamicChunk::Filter`
//! in `UnSprite.cpp` runs in two phases: `alloc_bytes` decides whether the split
//! path allocates its two rasters (the C++ then allocates them with `GDynMem`),
//! and `chunk` runs the split and filter-down decisions. `FDynamicLight::Filter`
//! takes its flag bits from `light_flags`. Mirror structs cross as `void*`; the
//! layouts are asserted in `hp2-abi::render_sprite_filter` and `hp2-abi::render_span`.

use std::ffi::c_void;

use hp2_abi::FPlane;
use hp2_abi::FVector;
use hp2_abi::render_particle::FTransform;
use hp2_abi::render_span::FRasterSpan;
use hp2_abi::render_sprite_filter::FRasterPolyHeader;
use hp2_ffi_support::guard;
use hp2_render::sprite_filter::{self, Action, Buf, Raster, Source};

const HEADER_BYTES: usize = std::mem::size_of::<FRasterPolyHeader>();

/// Span count of a raster: `EndY - StartY`, never negative for a valid raster.
fn span_count(hdr: &FRasterPolyHeader) -> usize {
    (hdr.end_y - hdr.start_y).max(0) as usize
}

/// # Safety
/// `raster` must point to a `FRasterPoly` with at least `count` spans after its header.
unsafe fn lines_at<'a>(raster: *const c_void, count: usize) -> &'a [FRasterSpan] {
    unsafe {
        let p = (raster as *const u8).add(HEADER_BYTES) as *const FRasterSpan;
        std::slice::from_raw_parts(p, count)
    }
}

/// # Safety
/// `raster` must point to a writable `FRasterPoly` with at least `count` spans after its header.
unsafe fn buf_at<'a>(raster: *mut c_void, count: usize) -> Buf<'a> {
    unsafe {
        let header = &mut *(raster as *mut FRasterPolyHeader);
        let p = (raster as *mut u8).add(HEADER_BYTES) as *mut FRasterSpan;
        Buf {
            header,
            lines: std::slice::from_raw_parts_mut(p, count),
        }
    }
}

fn raster_pick(p: Raster, front: *mut c_void, back: *mut c_void, src: *mut c_void) -> *mut c_void {
    match p {
        Raster::Null => std::ptr::null_mut(),
        Raster::Front => front,
        Raster::Back => back,
        Raster::Source => src,
    }
}

fn action_code(a: Action) -> u8 {
    match a {
        Action::None => 0,
        Action::Chunk => 1,
        Action::Final => 2,
    }
}

/// Phase 1 of `FDynamicChunk::Filter`. Returns 1 when the split path allocates two
/// rasters, each `*out_bytes` bytes (`sizeof(FRasterPoly) + (EndY - StartY) *
/// sizeof(FRasterSpan)`); 0 when no allocation happens. `out_bytes` is always written.
///
/// # Safety
/// `verts` points to four `FTransform`, `plane` to an `FPlane`, `src_raster` to a
/// `FRasterPoly` header; `out_bytes` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_filter_alloc_bytes(
    verts: *const c_void,
    plane: *const c_void,
    src_raster: *const c_void,
    out_bytes: *mut usize,
) -> u8 {
    guard(
        "hp2_rs_render_sprite_filter_alloc_bytes",
        0,
        move || unsafe {
            if verts.is_null() || plane.is_null() || src_raster.is_null() || out_bytes.is_null() {
                return 0;
            }
            let verts = &*(verts as *const [FTransform; 4]);
            let plane = &*(plane as *const FPlane);
            let hdr = &*(src_raster as *const FRasterPolyHeader);
            *out_bytes = sprite_filter::raster_alloc_bytes(hdr.start_y, hdr.end_y);
            let c = sprite_filter::crossings(verts, plane);
            sprite_filter::split_allocates(&c) as u8
        },
    )
}

/// Phase 2 of `FDynamicChunk::Filter`: split and filter-down decisions. `front_raster`
/// and `back_raster` are the two rasters phase 1 allocated (required, and non-null,
/// exactly when phase 1 returned 1). Writes the raster pointers to pass on (null,
/// `front_raster`, `back_raster` or `src_raster`) and the action per side (0 none,
/// 1 `FDynamicChunk`, 2 `FDynamicFinalChunk`). Returns 0 without writing outputs when
/// a required raster is null.
///
/// # Safety
/// As `alloc_bytes`; `front_raster`/`back_raster` are writable rasters of the size
/// phase 1 reported; the `out_*` pointers are writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hp2_rs_render_sprite_filter_chunk(
    verts: *const c_void,
    plane: *const c_void,
    src_raster: *mut c_void,
    front_raster: *mut c_void,
    back_raster: *mut c_void,
    i_front: i32,
    i_back: i32,
    outside: u8,
    csg: u8,
    out_front: *mut *mut c_void,
    out_back: *mut *mut c_void,
    out_front_action: *mut u8,
    out_back_action: *mut u8,
) -> u8 {
    guard("hp2_rs_render_sprite_filter_chunk", 0, move || unsafe {
        if verts.is_null()
            || plane.is_null()
            || src_raster.is_null()
            || out_front.is_null()
            || out_back.is_null()
            || out_front_action.is_null()
            || out_back_action.is_null()
        {
            return 0;
        }
        let verts = &*(verts as *const [FTransform; 4]);
        let plane = &*(plane as *const FPlane);
        let src_hdr = &*(src_raster as *const FRasterPolyHeader);
        let count = span_count(src_hdr);
        let src = Source {
            start_y: src_hdr.start_y,
            end_y: src_hdr.end_y,
            lines: lines_at(src_raster, count),
        };
        let c = sprite_filter::crossings(verts, plane);
        let bufs = if sprite_filter::split_allocates(&c) {
            if front_raster.is_null() || back_raster.is_null() {
                return 0;
            }
            Some([buf_at(front_raster, count), buf_at(back_raster, count)])
        } else {
            None
        };
        let f = sprite_filter::filter_chunk(
            verts,
            plane,
            &src,
            bufs,
            i_front,
            i_back,
            outside != 0,
            csg != 0,
        );
        *out_front = raster_pick(f.front, front_raster, back_raster, src_raster);
        *out_back = raster_pick(f.back, front_raster, back_raster, src_raster);
        *out_front_action = action_code(f.front_action);
        *out_back_action = action_code(f.back_action);
        1
    })
}

/// `FDynamicLight::Filter` decision bits: bit 0 `Dist > -Radius`, bit 1
/// `IsVol && Dist > -VolRadius`, bit 2 `Dist < Radius`, bit 3 `IsVol && Dist < VolRadius`.
/// `Dist` is `Node.Plane.PlaneDot(Actor->Location)`.
///
/// # Safety
/// `plane` points to an `FPlane`, `location` to an `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_light_flags(
    plane: *const c_void,
    location: *const c_void,
    radius: f32,
    vol_radius: f32,
    is_vol: u8,
) -> u8 {
    guard("hp2_rs_render_sprite_light_flags", 0, move || unsafe {
        if plane.is_null() || location.is_null() {
            return 0;
        }
        sprite_filter::light_flags(
            &*(plane as *const FPlane),
            &*(location as *const FVector),
            radius,
            vol_radius,
            is_vol != 0,
        )
    })
}
