//! Guarded C exports for `Engine/Src/UnScrTex.cpp` (see `hp2_engine::scr_tex`).
//! Each export takes the `Hp2ScrTexCtx` the C++ shell builds for the call, except
//! the palette map constructor and destructor. Object arguments are opaque
//! engine pointers; colours cross as `FColor::Int4`.

use std::ffi::c_void;

use hp2_engine::scr_tex::{self, Hp2ScrTexCtx, ObjPtr, PaletteCache, ServiceHost, TileRect};
use hp2_ffi_support::guard;

pub const HP2_SCR_TEX_LOG_DRAW_TEXT_NO_FONT: u32 = 0;
const _: () = assert!(HP2_SCR_TEX_LOG_DRAW_TEXT_NO_FONT == scr_tex::LOG_DRAW_TEXT_NO_FONT);
pub const HP2_SCR_TEX_LOG_REPLACE_MISSING_TEXTURE: u32 = 1;
const _: () =
    assert!(HP2_SCR_TEX_LOG_REPLACE_MISSING_TEXTURE == scr_tex::LOG_REPLACE_MISSING_TEXTURE);
pub const HP2_SCR_TEX_LOG_REPLACE_DIMENSIONS: u32 = 2;
const _: () = assert!(HP2_SCR_TEX_LOG_REPLACE_DIMENSIONS == scr_tex::LOG_REPLACE_DIMENSIONS);
pub const HP2_SCR_TEX_LOG_DRAW_TILE_MISSING_TEXTURE: u32 = 3;
const _: () =
    assert!(HP2_SCR_TEX_LOG_DRAW_TILE_MISSING_TEXTURE == scr_tex::LOG_DRAW_TILE_MISSING_TEXTURE);
pub const HP2_SCR_TEX_LOG_TEXT_SIZE_NO_FONT: u32 = 4;
const _: () = assert!(HP2_SCR_TEX_LOG_TEXT_SIZE_NO_FONT == scr_tex::LOG_TEXT_SIZE_NO_FONT);

/// # Safety
/// `ctx` points to a live `Hp2ScrTexCtx` for the duration of the call.
unsafe fn host(ctx: *const Hp2ScrTexCtx) -> ServiceHost {
    unsafe { ServiceHost::new(*ctx) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_palette_map_new() -> *mut c_void {
    guard(
        "hp2_rs_engine_scr_tex_palette_map_new",
        std::ptr::null_mut(),
        move || scr_tex::palette_cache_new() as *mut c_void,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_palette_map_free(cache: *mut c_void) {
    guard(
        "hp2_rs_engine_scr_tex_palette_map_free",
        (),
        move || unsafe {
            scr_tex::palette_cache_free(cache as *mut PaletteCache);
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_post_load(ctx: *const Hp2ScrTexCtx) {
    guard("hp2_rs_engine_scr_tex_post_load", (), move || unsafe {
        let mut h = host(ctx);
        scr_tex::post_load(&mut h);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_tick(ctx: *const Hp2ScrTexCtx) {
    guard("hp2_rs_engine_scr_tex_tick", (), move || unsafe {
        let mut h = host(ctx);
        scr_tex::tick(&mut h);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_draw_text(
    ctx: *const Hp2ScrTexCtx,
    x: f32,
    y: f32,
    text: *mut c_void,
    font: *mut c_void,
) {
    guard("hp2_rs_engine_scr_tex_draw_text", (), move || unsafe {
        let mut h = host(ctx);
        scr_tex::exec_draw_text(&mut h, x, y, ObjPtr(text), ObjPtr(font));
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_draw_colored_text(
    ctx: *const Hp2ScrTexCtx,
    x: f32,
    y: f32,
    text: *mut c_void,
    font: *mut c_void,
    color: u32,
) {
    guard(
        "hp2_rs_engine_scr_tex_draw_colored_text",
        (),
        move || unsafe {
            let mut h = host(ctx);
            scr_tex::exec_draw_colored_text(&mut h, x, y, ObjPtr(text), ObjPtr(font), color);
        },
    )
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_draw_tile(
    ctx: *const Hp2ScrTexCtx,
    x: f32,
    y: f32,
    xl: f32,
    yl: f32,
    u: f32,
    v: f32,
    ul: f32,
    vl: f32,
    tex: *mut c_void,
    masked: u32,
) {
    guard("hp2_rs_engine_scr_tex_draw_tile", (), move || unsafe {
        let mut h = host(ctx);
        let rect = TileRect {
            x,
            y,
            xl,
            yl,
            u,
            v,
            ul,
            vl,
        };
        scr_tex::exec_draw_tile(&mut h, rect, ObjPtr(tex), masked != 0);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_replace_texture(
    ctx: *const Hp2ScrTexCtx,
    tex: *mut c_void,
) {
    guard(
        "hp2_rs_engine_scr_tex_replace_texture",
        (),
        move || unsafe {
            let mut h = host(ctx);
            scr_tex::exec_replace_texture(&mut h, ObjPtr(tex));
        },
    )
}

/// Writes `*xl`/`*yl` only when the font is present, matching the C++ early return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_scr_tex_text_size(
    ctx: *const Hp2ScrTexCtx,
    text: *mut c_void,
    font: *mut c_void,
    xl: *mut f32,
    yl: *mut f32,
) {
    guard("hp2_rs_engine_scr_tex_text_size", (), move || unsafe {
        let mut h = host(ctx);
        if let Some((w, height)) = scr_tex::exec_text_size(&mut h, ObjPtr(text), ObjPtr(font)) {
            *xl = w;
            *yl = height;
        }
    })
}
