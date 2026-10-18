//! Span buffer exports (`crates/hp2-render/src/span.rs`). The C++ `FSpanBuffer`
//! methods in `Render/Src/UnSpan.cpp` forward here. Buffers, screens, rasters
//! and memory stacks cross as opaque `void*`: the mirror layout lives in
//! `hp2-abi::render_span` and is asserted on both sides.

use std::ffi::c_void;

use hp2_abi::render_span::{FMemMark, FRasterSpan, FSpanBuffer};
use hp2_ffi_support::guard;
use hp2_render::services::Hp2RenderServices;
use hp2_render::span::{self, SpanAlloc};

/// `SpanAlloc` over the engine services table. `mem` stays the engine's
/// `FMemStack*`; only the callbacks reach C++.
struct Services(Hp2RenderServices);

impl SpanAlloc for Services {
    unsafe fn push(&self, mem: *mut c_void, size: usize, align: usize) -> *mut u8 {
        let size = i32::try_from(size).expect("span push size fits i32");
        let align = i32::try_from(align).expect("span push align fits i32");
        unsafe { (self.0.mem_push)(mem, size, align) }
    }
    unsafe fn mark_save(&self, mem: *mut c_void, mark: *mut FMemMark) {
        unsafe { (self.0.mem_mark_save)(mem, mark as *mut c_void) }
    }
    unsafe fn mark_pop(&self, mark: *mut FMemMark) {
        unsafe { (self.0.mem_mark_pop)(mark as *mut c_void) }
    }
    fn warn(&self, text: &str) {
        unsafe { (self.0.log_warning)(text.as_ptr(), text.len()) }
    }
}

unsafe fn services_of(services: *const Hp2RenderServices) -> Option<Services> {
    if services.is_null() {
        None
    } else {
        Some(Services(unsafe { *services }))
    }
}

unsafe fn span_buffer<'a>(p: *mut c_void) -> Option<&'a mut FSpanBuffer> {
    if p.is_null() {
        None
    } else {
        Some(unsafe { &mut *(p as *mut FSpanBuffer) })
    }
}

/// `FSpanBuffer::AllocIndex`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_alloc_index(
    services: *const Hp2RenderServices,
    this: *mut c_void,
    alloc_start_y: i32,
    alloc_end_y: i32,
    mem: *mut c_void,
) {
    guard("hp2_rs_render_span_alloc_index", (), move || unsafe {
        let (Some(a), Some(b)) = (services_of(services), span_buffer(this)) else {
            return;
        };
        span::alloc_index(&a, b, alloc_start_y, alloc_end_y, mem);
    });
}

/// `FSpanBuffer::AllocIndexForScreen`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_alloc_index_for_screen(
    services: *const Hp2RenderServices,
    this: *mut c_void,
    sxr: i32,
    syr: i32,
    mem: *mut c_void,
) {
    guard(
        "hp2_rs_render_span_alloc_index_for_screen",
        (),
        move || unsafe {
            let (Some(a), Some(b)) = (services_of(services), span_buffer(this)) else {
                return;
            };
            span::alloc_index_for_screen(&a, b, sxr, syr, mem);
        },
    );
}

/// `FSpanBuffer::Release`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_release(
    services: *const Hp2RenderServices,
    this: *mut c_void,
) {
    guard("hp2_rs_render_span_release", (), move || unsafe {
        let (Some(a), Some(b)) = (services_of(services), span_buffer(this)) else {
            return;
        };
        span::release(&a, b);
    });
}

/// `FSpanBuffer::GetValidRange`. Writes through the caller's `SWORD*` out-params.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_get_valid_range(
    this: *mut c_void,
    valid_start_y: *mut i16,
    valid_end_y: *mut i16,
) {
    guard("hp2_rs_render_span_get_valid_range", (), move || unsafe {
        let Some(b) = span_buffer(this) else {
            return;
        };
        if valid_start_y.is_null() || valid_end_y.is_null() {
            return;
        }
        let (s, e) = span::get_valid_range(b);
        *valid_start_y = s;
        *valid_end_y = e;
    });
}

/// `FSpanBuffer::BoxIsVisible`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_box_is_visible(
    this: *mut c_void,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
) -> i32 {
    guard("hp2_rs_render_span_box_is_visible", 0, move || unsafe {
        let Some(b) = span_buffer(this) else {
            return 0;
        };
        span::box_is_visible(b, x1, y1, x2, y2)
    })
}

/// `FSpanBuffer::CopyFromRasterUpdate`. `raster` is `FRasterSpan*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_copy_from_raster_update(
    services: *const Hp2RenderServices,
    this: *mut c_void,
    screen: *mut c_void,
    raster_start_y: i32,
    raster_end_y: i32,
    raster: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_render_span_copy_from_raster_update",
        0,
        move || unsafe {
            let (Some(a), Some(this), Some(screen)) = (
                services_of(services),
                span_buffer(this),
                span_buffer(screen),
            ) else {
                return 0;
            };
            span::copy_from_raster_update(
                &a,
                this,
                screen,
                raster_start_y,
                raster_end_y,
                raster as *const FRasterSpan,
            )
        },
    )
}

/// `FSpanBuffer::CopyFromRaster`. `screen` is only read. `raster` is `FRasterSpan*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_copy_from_raster(
    services: *const Hp2RenderServices,
    this: *mut c_void,
    screen: *mut c_void,
    raster_start_y: i32,
    raster_end_y: i32,
    raster: *const c_void,
) -> i32 {
    guard("hp2_rs_render_span_copy_from_raster", 0, move || unsafe {
        let (Some(a), Some(this), Some(screen)) = (
            services_of(services),
            span_buffer(this),
            span_buffer(screen),
        ) else {
            return 0;
        };
        span::copy_from_raster(
            &a,
            this,
            screen,
            raster_start_y,
            raster_end_y,
            raster as *const FRasterSpan,
        )
    })
}

/// `FSpanBuffer::MergeWith`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_merge_with(
    services: *const Hp2RenderServices,
    this: *mut c_void,
    other: *mut c_void,
) {
    guard("hp2_rs_render_span_merge_with", (), move || unsafe {
        let (Some(a), Some(this), Some(other)) =
            (services_of(services), span_buffer(this), span_buffer(other))
        else {
            return;
        };
        span::merge_with(&a, this, other);
    });
}

/// `FSpanBuffer::CopyIndexFrom`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_span_copy_index_from(
    services: *const Hp2RenderServices,
    this: *mut c_void,
    source: *mut c_void,
    mem: *mut c_void,
) {
    guard("hp2_rs_render_span_copy_index_from", (), move || unsafe {
        let (Some(a), Some(this), Some(source)) = (
            services_of(services),
            span_buffer(this),
            span_buffer(source),
        ) else {
            return;
        };
        span::copy_index_from(&a, this, source, mem);
    });
}
