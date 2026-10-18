//! FFI exports for the canvas text and tile logic in `hp2_engine::canvas_text`.
//!
//! C++ (`UnCanvas.cpp`) copies the `UCanvas` state into a `Hp2CanvasView`,
//! passes an `Hp2CanvasTextCtx` (the `Hp2CanvasServices` table plus engine
//! pointers), and writes the mutated view fields back afterwards. Text is
//! NUL-terminated UTF-16 (`UNICHAR`); the C++ shims convert host `TCHAR` text.

use std::ffi::c_void;

use hp2_engine::canvas::Hp2CanvasTileRect;
use hp2_engine::canvas_text as ct;
use hp2_engine::canvas_text::{
    Font, Hp2CanvasPatternArgs, Hp2CanvasSize, Hp2CanvasTextCtx, Hp2CanvasView, Hp2Plane,
    ServiceHost,
};
use hp2_ffi_support::guard;

// Log codes consumed by the C++ `Hp2CanvasServices` switch. Literal values are
// emitted into hp2_rs.h by cbindgen; the asserts fail the build on drift.
pub const HP2_CANVAS_LOG_DRAW_TEXT_NO_FONT: u32 = 0;
pub const HP2_CANVAS_LOG_DRAW_TEXT_CLIPPED_NO_FONT: u32 = 1;
pub const HP2_CANVAS_LOG_TEXT_SIZE_NO_FONT: u32 = 2;
const _: () = assert!(HP2_CANVAS_LOG_DRAW_TEXT_NO_FONT == ct::LOG_DRAW_TEXT_NO_FONT);
const _: () =
    assert!(HP2_CANVAS_LOG_DRAW_TEXT_CLIPPED_NO_FONT == ct::LOG_DRAW_TEXT_CLIPPED_NO_FONT);
const _: () = assert!(HP2_CANVAS_LOG_TEXT_SIZE_NO_FONT == ct::LOG_TEXT_SIZE_NO_FONT);

/// Borrows a NUL-terminated UTF-16 string, including its terminator.
///
/// # Safety
/// `text` must be null or point to a NUL-terminated UTF-16 string.
unsafe fn nul_terminated<'a>(text: *const u16) -> Option<&'a [u16]> {
    if text.is_null() {
        return None;
    }
    let mut len = 0usize;
    // SAFETY: the caller guarantees a NUL terminator, so every read is in bounds.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: the slice covers the units read above plus the terminator.
    Some(unsafe { std::slice::from_raw_parts(text, len + 1) })
}

/// Builds the service host for one call; `None` when `ctx` is null.
///
/// # Safety
/// When non-null, `ctx` must satisfy [`ServiceHost::new`].
unsafe fn host(ctx: *const Hp2CanvasTextCtx) -> Option<ServiceHost> {
    // SAFETY: a non-null `ctx` is a live context for the duration of the call.
    let ctx = unsafe { ctx.as_ref()? };
    // SAFETY: forwarded to `ServiceHost::new`.
    Some(unsafe { ServiceHost::new(*ctx) })
}

/// `GetCanvasTextScale` for a view, given the client UI scale.
///
/// # Safety
/// `view` must be null or point to a live `Hp2CanvasView`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_text_scale(
    view: *const Hp2CanvasView,
    ui_scale: f32,
) -> f32 {
    guard("hp2_rs_canvas_text_scale", 1.0, move || unsafe {
        match view.as_ref() {
            Some(view) => ct::text_scale_from(view, ui_scale),
            None => 1.0,
        }
    })
}

/// `HasSafeCanvasTextRequestFloats` for a view, given the text scale.
///
/// # Safety
/// `view` must be null or point to a live `Hp2CanvasView`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_has_safe_request_floats(
    view: *const Hp2CanvasView,
    text_scale: f32,
) -> u8 {
    guard("hp2_rs_canvas_has_safe_request_floats", 0, move || unsafe {
        view.as_ref().map_or(0, |view| {
            u8::from(ct::has_safe_request_floats(view, text_scale))
        })
    })
}

/// `UCanvas::SetClip` on the view.
///
/// # Safety
/// `view` must be null or point to a live `Hp2CanvasView`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_set_clip(
    view: *mut Hp2CanvasView,
    x: i32,
    y: i32,
    xl: i32,
    yl: i32,
) {
    guard("hp2_rs_canvas_set_clip", (), move || unsafe {
        if let Some(view) = view.as_mut() {
            ct::set_clip(view, x, y, xl, yl);
        }
    })
}

/// `DrawString` (bitmap path; the native and viewport branches are taken
/// through the services). Returns the horizontal advance.
///
/// # Safety
/// `ctx` and `view` must be live for the call; `text` must be NUL-terminated.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hp2_rs_canvas_draw_string(
    ctx: *const Hp2CanvasTextCtx,
    view: *const Hp2CanvasView,
    flags: u32,
    font: Font,
    x: i32,
    y: i32,
    text: *const u16,
    color: Hp2Plane,
    clip: u8,
    amp: u8,
) -> i32 {
    guard("hp2_rs_canvas_draw_string", 0, move || unsafe {
        let (Some(mut host), Some(view), Some(text)) =
            (host(ctx), view.as_ref(), nul_terminated(text))
        else {
            return 0;
        };
        let args = ct::DrawStringArgs {
            flags,
            font,
            x,
            y,
            color,
            clip: clip != 0,
            amp: amp != 0,
        };
        ct::draw_string(&mut host, view, &args, text)
    })
}

/// `WrappedPrint`. Mutates the view's cursor and clip fields.
///
/// # Safety
/// `ctx` and `view` must be live for the call; `text` must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_wrapped_print(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    font: Font,
    style: i32,
    center: u8,
    text: *const u16,
    num_chars: i32,
) -> Hp2CanvasSize {
    guard(
        "hp2_rs_canvas_wrapped_print",
        Hp2CanvasSize::default(),
        move || unsafe {
            let (Some(mut host), Some(view), Some(text)) =
                (host(ctx), view.as_mut(), nul_terminated(text))
            else {
                return Hp2CanvasSize::default();
            };
            let (w, h) =
                ct::wrapped_print(&mut host, view, font, style, center != 0, text, num_chars);
            Hp2CanvasSize { w, h }
        },
    )
}

/// `execStrLen` body: `WrappedPrint(STY_None, ...)` with the cursor saved.
///
/// # Safety
/// `ctx` and `view` must be live for the call; `text` must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_native_str_len(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    text: *const u16,
) -> Hp2CanvasSize {
    guard(
        "hp2_rs_canvas_native_str_len",
        Hp2CanvasSize::default(),
        move || unsafe {
            let (Some(mut host), Some(view), Some(text)) =
                (host(ctx), view.as_mut(), nul_terminated(text))
            else {
                return Hp2CanvasSize::default();
            };
            let (w, h) = ct::native_str_len(&mut host, view, text);
            Hp2CanvasSize { w, h }
        },
    )
}

/// `execDrawText` body.
///
/// # Safety
/// `ctx` and `view` must be live for the call; `text` must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_native_draw_text(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    text: *const u16,
    cr: u8,
    num_chars: i32,
) {
    guard("hp2_rs_canvas_native_draw_text", (), move || unsafe {
        let (Some(mut host), Some(view), Some(text)) =
            (host(ctx), view.as_mut(), nul_terminated(text))
        else {
            return;
        };
        ct::native_draw_text(&mut host, view, text, cr != 0, num_chars);
    })
}

/// `execDrawTextClipped` body. May swap `view.font` (LargeFont/BigFont to MedFont).
///
/// # Safety
/// `ctx` and `view` must be live for the call; `text` must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_native_draw_text_clipped(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    text: *const u16,
    check_hotkey: u8,
) {
    guard(
        "hp2_rs_canvas_native_draw_text_clipped",
        (),
        move || unsafe {
            let (Some(mut host), Some(view), Some(text)) =
                (host(ctx), view.as_mut(), nul_terminated(text))
            else {
                return;
            };
            ct::native_draw_text_clipped(&mut host, view, text, check_hotkey != 0);
        },
    )
}

/// `execTextSize` body. Returns 0 when the font is null (the log line is emitted),
/// otherwise 1 with the measured size in `out`.
///
/// # Safety
/// `ctx`, `view` and `out` must be live for the call; `text` must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_native_text_size(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    text: *const u16,
    out: *mut Hp2CanvasSize,
) -> u8 {
    guard("hp2_rs_canvas_native_text_size", 0, move || unsafe {
        let (Some(mut host), Some(view), Some(text), Some(out)) =
            (host(ctx), view.as_mut(), nul_terminated(text), out.as_mut())
        else {
            return 0;
        };
        match ct::native_text_size(&mut host, view, text) {
            Some((w, h)) => {
                *out = Hp2CanvasSize { w, h };
                1
            }
            None => 0,
        }
    })
}

/// `execDrawTile` body (the C++ shim checks the texture and logs when it is null).
///
/// # Safety
/// `ctx`, `view` and `rect` must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_native_draw_tile(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    tex: *mut c_void,
    rect: *const Hp2CanvasTileRect,
) {
    guard("hp2_rs_canvas_native_draw_tile", (), move || unsafe {
        let (Some(mut host), Some(view), Some(rect)) = (host(ctx), view.as_mut(), rect.as_ref())
        else {
            return;
        };
        ct::native_draw_tile(&mut host, view, tex, *rect);
    })
}

/// `execDrawTileClipped` body (the C++ shim checks the texture and logs when it is null).
///
/// # Safety
/// `ctx`, `view` and `rect` must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_native_draw_tile_clipped(
    ctx: *const Hp2CanvasTextCtx,
    view: *mut Hp2CanvasView,
    tex: *mut c_void,
    rect: *const Hp2CanvasTileRect,
) {
    guard(
        "hp2_rs_canvas_native_draw_tile_clipped",
        (),
        move || unsafe {
            let (Some(mut host), Some(view), Some(rect)) =
                (host(ctx), view.as_mut(), rect.as_ref())
            else {
                return;
            };
            ct::native_draw_tile_clipped(&mut host, view, tex, *rect);
        },
    )
}

/// `UCanvas::DrawPattern` (the tile itself is drawn by the `UCanvas::DrawTile` service).
///
/// # Safety
/// `ctx`, `view` and `args` must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_draw_pattern(
    ctx: *const Hp2CanvasTextCtx,
    view: *const Hp2CanvasView,
    args: *const Hp2CanvasPatternArgs,
) {
    guard("hp2_rs_canvas_draw_pattern", (), move || unsafe {
        let (Some(mut host), Some(view), Some(args)) = (host(ctx), view.as_ref(), args.as_ref())
        else {
            return;
        };
        ct::draw_pattern(&mut host, view, args);
    })
}

/// `UCanvas::DrawIcon` (the tile itself is drawn by the `UCanvas::DrawTile` service).
///
/// # Safety
/// `ctx`, `view` and `args` must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_draw_icon(
    ctx: *const Hp2CanvasTextCtx,
    view: *const Hp2CanvasView,
    args: *const Hp2CanvasPatternArgs,
) {
    guard("hp2_rs_canvas_draw_icon", (), move || unsafe {
        let (Some(mut host), Some(view), Some(args)) = (host(ctx), view.as_ref(), args.as_ref())
        else {
            return;
        };
        ct::draw_icon(&mut host, view, args);
    })
}
