//! Canvas text and tile port of `Engine/Src/UnCanvas.cpp`: `GetCanvasTextScale`,
//! the safe-float gates, `ScaleFontMetric`, `GetCharSize`, `DrawChar`,
//! `DrawString` (bitmap path), `WrappedPrint`, `SetClip`, `DrawPattern`,
//! `DrawIcon`, and the bodies of the `exec*` text/tile natives.
//!
//! UCanvas state crosses the FFI as a copy-in/copy-out [`Hp2CanvasView`]. Every
//! engine touch-point (Viewport, RenDev, texture locks, font pages, the
//! native-text backend, the log) is a call through [`CanvasTextHost`]. The
//! production host, [`ServiceHost`], calls the C++ `Hp2CanvasServices` table;
//! tests drive the same logic with a fake host.
//!
//! Float math follows the C++ operation order exactly. `appRound` is
//! `static_cast<INT>` (truncation); Rust `as` saturates where C is undefined,
//! which is the one documented numeric divergence.

use std::ffi::c_void;

use crate::canvas::Hp2CanvasTileRect;

/// `ERenderStyle` (`Engine/Inc/UnCamera.h`).
pub const STY_NONE: i32 = 0;
pub const STY_TRANSLUCENT: i32 = 3;
pub const STY_MODULATED: i32 = 4;

/// `PF_*` poly flags (`Engine/Inc/UnRender.h`), as used by the canvas text paths.
pub const PF_INVISIBLE: u32 = 0x0000_0001;
pub const PF_MASKED: u32 = 0x0000_0002;
pub const PF_TRANSLUCENT: u32 = 0x0000_0004;
pub const PF_MODULATED: u32 = 0x0000_0040;
pub const PF_TWO_SIDED: u32 = 0x0000_0100;
pub const PF_NO_SMOOTH: u32 = 0x0000_0800;
pub const PF_RENDER_HINT: u32 = 0x0100_0000;

/// Log codes; the C++ side maps each to its byte-identical `TEXT("...")` literal.
pub const LOG_DRAW_TEXT_NO_FONT: u32 = 0;
pub const LOG_DRAW_TEXT_CLIPPED_NO_FONT: u32 = 1;
pub const LOG_TEXT_SIZE_NO_FONT: u32 = 2;

/// Texture slots used by `DrawString` (`Infos[5]`).
pub const TEXT_SLOTS: usize = 5;

/// Opaque `UFont*` / `UTexture*` handle.
/// Opaque `UFont*`. A newtype (not a raw-pointer alias) so trait methods that
/// forward it do not trip `clippy::not_unsafe_ptr_arg_deref`; ABI is identical.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Font(pub *mut c_void);

impl Font {
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}

/// Opaque `UTexture*` / `FTextureInfo` handle; newtype for the same lint reason as [`Font`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texture(pub *mut c_void);

impl Texture {
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}

/// `FPlane` (`Engine/Inc/UnTex.h`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hp2Plane {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Hp2Plane {
    pub const ZERO: Hp2Plane = Hp2Plane {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };
}

/// `FFontCharacter` fields read by the text paths.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hp2FontChar {
    pub start_u: i32,
    pub start_v: i32,
    pub u_size: i32,
    pub v_size: i32,
}

/// Two INTs (texture or text measurement).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hp2CanvasSize {
    pub w: i32,
    pub h: i32,
}

/// UCanvas state the text/tile paths read or write. C++ copies the fields in
/// before a call and writes `cur_x`, `cur_y`, `cur_yl`, `clip_x`, `clip_y`,
/// and `font` back after it (see `Hp2CanvasView` in `Hp2CanvasServices.h`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2CanvasView {
    pub font: Font,
    pub large_font: Font,
    pub big_font: Font,
    pub med_font: Font,
    pub cur_x: f32,
    pub cur_y: f32,
    pub cur_yl: f32,
    pub clip_x: f32,
    pub clip_y: f32,
    pub org_x: f32,
    pub org_y: f32,
    pub space_x: f32,
    pub space_y: f32,
    pub z: f32,
    /// `UCanvas::X` (viewport width, INT).
    pub x: i32,
    /// `Frame->X` / `Frame->Y` (INT) and `Frame->FX` / `Frame->FY` (FLOAT); valid when `has_frame != 0`.
    pub frame_x: i32,
    pub frame_y: i32,
    pub frame_fx: f32,
    pub frame_fy: f32,
    pub has_frame: u8,
    pub style: i32,
    pub center: u8,
    pub no_smooth: u8,
    /// `Color.Plane()`.
    pub color: Hp2Plane,
}

/// `TryNativeCanvasText` request (`Hp2NativeTextRequest` in C++).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2NativeTextRequest {
    pub flags: u32,
    pub draw_x: i32,
    pub draw_y: i32,
    pub clip: u8,
    pub amp: u8,
    pub center: u8,
    pub wrap: u8,
    pub visible: i32,
}

/// Argument bundle for one `UCanvas::DrawTile` call through the host.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2CanvasTileDraw {
    pub tex: *mut c_void,
    pub rect: Hp2CanvasTileRect,
    pub span: *mut c_void,
    pub z: f32,
    pub color: Hp2Plane,
    pub fog: Hp2Plane,
    pub flags: u32,
}

/// `UCanvas::DrawPattern` / `DrawIcon` arguments (`DrawIcon` ignores scale/org/fog-free fields it does not use).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2CanvasPatternArgs {
    pub tex: *mut c_void,
    pub x: f32,
    pub y: f32,
    pub xl: f32,
    pub yl: f32,
    pub scale: f32,
    pub org_x: f32,
    pub org_y: f32,
    pub span: *mut c_void,
    pub z: f32,
    pub color: Hp2Plane,
    pub fog: Hp2Plane,
    pub flags: u32,
}

/// Engine touch-points for the canvas text logic. Every method is a direct
/// engine operation; see `Hp2CanvasServices.cpp` for the production bodies.
pub trait CanvasTextHost {
    /// `GetOuterUClient()->GetUIScale()`, or 1.0 when there is no client.
    fn ui_scale(&mut self) -> f32;
    /// `appStricmp(GetLanguage(), "INT") == 0`.
    fn language_is_int(&mut self) -> bool;
    /// `Canvas->Viewport->DrawString(flags, Font, X, Y, Text, Color)`.
    // Mirrors the C signature one-to-one; bundling would only move the arguments.
    #[allow(clippy::too_many_arguments)]
    fn viewport_draw_string(
        &mut self,
        cv: &Hp2CanvasView,
        flags: u32,
        font: Font,
        x: &mut i32,
        y: &mut i32,
        text: &[u16],
        color: Hp2Plane,
    );
    /// `TryNativeCanvasText`. `Some((w, h))` when the native backend drew the text.
    fn try_native_text(
        &mut self,
        cv: &Hp2CanvasView,
        req: &Hp2NativeTextRequest,
        font: Font,
        text: &[u16],
        color: Hp2Plane,
    ) -> Option<(i32, i32)>;
    fn font_chars_per_page(&mut self, font: Font) -> i32;
    fn font_remap_char(&mut self, font: Font, ch: u16) -> u16;
    fn font_page_count(&mut self, font: Font) -> i32;
    fn font_page_texture(&mut self, font: Font, page: i32) -> Texture;
    fn font_page_char_count(&mut self, font: Font, page: i32) -> i32;
    fn font_page_char(&mut self, font: Font, page: i32, index: i32) -> Hp2FontChar;
    /// `FontName` is non-empty.
    fn font_has_name(&mut self, font: Font) -> bool;
    /// `FTextureInfo.Texture = NULL` for every slot.
    fn slot_reset(&mut self);
    fn slot_texture(&mut self, slot: usize) -> Texture;
    /// `Texture->Lock(Slot, CurrentTime, 0, RenDev)`.
    fn slot_lock(&mut self, cv: &Hp2CanvasView, slot: usize, tex: Texture);
    /// `Texture->Unlock(Slot)`.
    fn slot_unlock(&mut self, slot: usize);
    /// `Viewport->RenDev->DrawTile(Frame, Slot, ...)` with fog zero, span NULL, `Canvas->Z`.
    fn draw_tile_slot(
        &mut self,
        cv: &Hp2CanvasView,
        slot: usize,
        rect: &Hp2CanvasTileRect,
        color: Hp2Plane,
        flags: u32,
    );
    /// `UCanvas::DrawTile(Texture, ...)` (C++ member: clip block, then RenDev).
    fn draw_tile_texture(&mut self, cv: &Hp2CanvasView, draw: &Hp2CanvasTileDraw);
    /// `Texture->USize` / `Texture->VSize`.
    fn texture_size(&mut self, tex: Texture) -> Hp2CanvasSize;
    /// Byte-identical engine log line selected by `code` (`LOG_*`).
    fn log(&mut self, code: u32);
}

// ---------------------------------------------------------------------------
// Engine helper semantics (`UnTemplate.h`, `UnGnuG.h`).
// ---------------------------------------------------------------------------

/// `Max(A, B) = (A >= B) ? A : B`.
fn c_max<T: PartialOrd>(a: T, b: T) -> T {
    if a >= b { a } else { b }
}

/// `Min(A, B) = (A <= B) ? A : B`.
fn c_min<T: PartialOrd>(a: T, b: T) -> T {
    if a <= b { a } else { b }
}

/// `Clamp(X, Min, Max) = X < Min ? Min : X < Max ? X : Max`.
fn c_clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// `appRound` truncates (`static_cast<INT>`).
pub fn app_round(f: f32) -> i32 {
    f as i32
}

/// `IsSafeCanvasTextFloat`: finite and inside the INT range, computed in double.
pub fn is_safe_float(v: f32) -> bool {
    let wide = f64::from(v);
    !v.is_nan() && wide >= -f64::from(i32::MAX) - 1.0 && wide <= f64::from(i32::MAX)
}

/// `GetCanvasTextScale` given the UI scale from the host.
pub fn text_scale_from(cv: &Hp2CanvasView, ui_scale: f32) -> f32 {
    let width = if cv.has_frame != 0 {
        cv.frame_fx
    } else {
        cv.x as f32
    };
    let mut ui = ui_scale;
    if ui.is_nan() {
        ui = 1.0;
    }
    ui = c_clamp(ui, 0.75, 2.0);
    c_max(1.0, width / 640.0) * ui
}

/// `GetCanvasTextScale(Canvas)`.
pub fn text_scale<H: CanvasTextHost + ?Sized>(h: &mut H, cv: &Hp2CanvasView) -> f32 {
    let ui = h.ui_scale();
    text_scale_from(cv, ui)
}

/// `HasSafeCanvasTextRequestFloats` given the text scale.
pub fn has_safe_request_floats(cv: &Hp2CanvasView, text_scale: f32) -> bool {
    is_safe_float(text_scale)
        && is_safe_float(cv.space_x * text_scale)
        && is_safe_float(cv.space_y * text_scale)
        && is_safe_float(cv.org_x)
        && is_safe_float(cv.org_y)
        && is_safe_float(cv.clip_x)
        && is_safe_float(cv.clip_y)
        && is_safe_float(cv.cur_x)
        && is_safe_float(cv.cur_y)
}

/// `ScaleFontMetric(Value, Scale)`.
pub fn scale_font_metric(value: i32, scale: f32) -> i32 {
    if value > 0 {
        c_max(1, app_round(value as f32 * scale))
    } else {
        0
    }
}

/// `GetCharSize(Font, InCh, TextScale, Width, Height)`.
pub fn get_char_size<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    font: Font,
    in_ch: u16,
    text_scale: f32,
) -> (i32, i32) {
    if font.is_null() {
        return (0, 0);
    }
    let cpp = h.font_chars_per_page(font);
    if cpp <= 0 {
        return (0, 0);
    }
    let ch = i32::from(h.font_remap_char(font, in_ch));
    let page = ch / cpp;
    let index = ch - page * cpp;
    if page < h.font_page_count(font) && index < h.font_page_char_count(font, page) {
        let c = h.font_page_char(font, page, index);
        (
            scale_font_metric(c.u_size, text_scale),
            scale_font_metric(c.v_size, text_scale),
        )
    } else {
        (0, 0)
    }
}

/// `PolyFlags` used by `WrappedPrint`, `DrawTextClipped`, and their natives.
pub fn text_poly_flags(style: i32) -> u32 {
    let style_bits = if style == STY_NONE {
        PF_INVISIBLE
    } else if style == STY_TRANSLUCENT {
        PF_TRANSLUCENT
    } else if style == STY_MODULATED {
        PF_MODULATED
    } else {
        0
    };
    (PF_NO_SMOOTH | PF_MASKED | PF_RENDER_HINT) | style_bits
}

/// `SetClip(X, Y, XL, YL)`.
pub fn set_clip(cv: &mut Hp2CanvasView, x: i32, y: i32, xl: i32, yl: i32) {
    cv.cur_x = 0.0;
    cv.cur_y = 0.0;
    cv.org_x = x as f32;
    cv.org_y = y as f32;
    cv.clip_x = xl as f32;
    cv.clip_y = yl as f32;
}

/// Character index into the string, with `Text[i]` reads past the NUL returning 0.
fn at(text: &[u16], i: usize) -> u16 {
    text.get(i).copied().unwrap_or(0)
}

fn count_units(text: &[u16]) -> usize {
    text.iter().take_while(|&&c| c != 0).count()
}

fn is_lower_ascii(ch: i32) -> bool {
    (i32::from(b'a')..=i32::from(b'z')).contains(&ch)
}

// ---------------------------------------------------------------------------
// DrawChar / DrawString / WrappedPrint.
// ---------------------------------------------------------------------------

/// Integer tile rect for `DrawChar`.
#[derive(Clone, Copy, Debug)]
pub struct CharRect {
    pub x: i32,
    pub y: i32,
    pub xl: i32,
    pub yl: i32,
    pub u: i32,
    pub v: i32,
    pub ul: i32,
    pub vl: i32,
}

/// `DrawChar`. C dereferences `Canvas->Frame` unconditionally; here a missing
/// frame draws nothing (documented divergence).
pub fn draw_char<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &Hp2CanvasView,
    slot: usize,
    flags: u32,
    rect: CharRect,
    color: Hp2Plane,
) {
    if cv.has_frame == 0 {
        return;
    }
    let fx = cv.frame_x;
    let fy = cv.frame_y;
    let CharRect {
        mut x,
        mut y,
        mut xl,
        mut yl,
        mut u,
        mut v,
        mut ul,
        mut vl,
    } = rect;
    if flags & PF_INVISIBLE == 0 && x + xl > 0 && y + yl > 0 && x < fx && y < fy && xl > 0 && yl > 0
    {
        if x < 0 {
            let c = x.wrapping_mul(ul) / xl;
            u = u.wrapping_sub(c);
            ul = ul.wrapping_add(c);
            xl = xl.wrapping_add(x);
            x = 0;
        }
        if y < 0 {
            let c = y.wrapping_mul(vl) / yl;
            v = v.wrapping_sub(c);
            vl = vl.wrapping_add(c);
            yl = yl.wrapping_add(y);
            y = 0;
        }
        if xl > fx.wrapping_sub(x) {
            ul = ul.wrapping_add(fx.wrapping_sub(x).wrapping_sub(xl).wrapping_mul(ul) / xl);
            xl = fx.wrapping_sub(x);
        }
        if yl > fy.wrapping_sub(y) {
            vl = vl.wrapping_add(fy.wrapping_sub(y).wrapping_sub(yl).wrapping_mul(vl) / yl);
            yl = fy.wrapping_sub(y);
        }
        let rect = Hp2CanvasTileRect {
            x: x as f32,
            y: y as f32,
            xl: xl as f32,
            yl: yl as f32,
            u: u as f32,
            v: v as f32,
            ul: ul as f32,
            vl: vl as f32,
        };
        h.draw_tile_slot(cv, slot, &rect, color, flags);
    }
}

/// Arguments for one `DrawString` call.
#[derive(Clone, Copy, Debug)]
pub struct DrawStringArgs {
    pub flags: u32,
    pub font: Font,
    pub x: i32,
    pub y: i32,
    pub color: Hp2Plane,
    pub clip: bool,
    pub amp: bool,
}

/// `DrawString` (bitmap path). `text` is NUL-terminated. Returns the advance.
pub fn draw_string<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &Hp2CanvasView,
    a: &DrawStringArgs,
    text: &[u16],
) -> i32 {
    let ts = text_scale(h, cv);
    if at(text, 0) == 0 || !has_safe_request_floats(cv, ts) {
        return 0;
    }
    if a.font.is_null() {
        // C dereferences Font below (UB); callers always pass a font.
        return 0;
    }
    let req = Hp2NativeTextRequest {
        flags: a.flags,
        draw_x: a.x,
        draw_y: a.y,
        clip: u8::from(a.clip),
        amp: u8::from(a.amp),
        center: 0,
        wrap: 0,
        visible: count_units(text) as i32,
    };
    if let Some((native_w, _)) = h.try_native_text(cv, &req, a.font, text, a.color) {
        return native_w;
    }
    if h.font_has_name(a.font) {
        let mut x = a.x;
        let mut y = a.y;
        h.viewport_draw_string(cv, a.flags, a.font, &mut x, &mut y, text, a.color);
        return x.wrapping_sub(a.x);
    }
    let cpp = h.font_chars_per_page(a.font);
    if cpp <= 0 {
        return 0;
    }
    let scaled_space_x = app_round(cv.space_x * ts);
    h.slot_reset();

    let mut line_x: i32 = 0;
    let mut draw_underline = false;
    let mut underline_width: i32 = 0;
    let remap = |h: &mut H, ch: u16| i32::from(h.font_remap_char(a.font, ch));

    let mut i = 0usize;
    'outer: while at(text, i) != 0 {
        let mut underline_next = false;
        let mut ch = remap(h, text[i]);
        if a.amp {
            if draw_underline {
                ch = remap(h, u16::from(b'_'));
            }
            if text[i] == u16::from(b'&') {
                if at(text, i + 1) == 0 {
                    break 'outer;
                }
                if at(text, i + 1) != u16::from(b'&') {
                    underline_next = true;
                    ch = remap(h, at(text, i + 1));
                }
            }
        }

        // `try_char:` label; `goto try_char` re-enters with the lowered `Ch`.
        'try_char: loop {
            let new_page = ch / cpp;
            if new_page >= h.font_page_count(a.font) {
                break 'try_char;
            }
            let tex = h.font_page_texture(a.font, new_page);
            if tex.is_null() {
                break 'try_char;
            }
            let index = ch - new_page * cpp;
            if index >= h.font_page_char_count(a.font, new_page) {
                break 'try_char;
            }
            let slot = c_min(new_page, 4) as usize;
            if h.slot_texture(slot) != tex {
                if !h.slot_texture(slot).is_null() {
                    h.slot_unlock(slot);
                }
                h.slot_lock(cv, slot, tex);
            }
            let c = h.font_page_char(a.font, new_page, index);
            if c.u_size == 0 && is_lower_ascii(ch) {
                ch += i32::from(b'A') - i32::from(b'a');
                continue 'try_char;
            }

            let mut char_width = if draw_underline {
                c_min(underline_width, c.u_size)
            } else {
                c.u_size
            };
            let mut draw_width = scale_font_metric(char_width, ts);
            let mut draw_height = scale_font_metric(c.v_size, ts);
            let mut x = line_x.wrapping_add(a.x);
            let mut y = a.y;
            let mut cu = c.start_u;
            let mut cvv = c.start_v;
            let mut cu_size = char_width;
            let mut cv_size = c.v_size;

            if !a.clip
                || (x.wrapping_add(draw_width) > 0
                    && (x as f32) <= cv.clip_x
                    && y.wrapping_add(draw_height) > 0
                    && (y as f32) <= cv.clip_y)
            {
                if a.clip && draw_width > 0 && draw_height > 0 {
                    if x < 0 {
                        let clip = c_min(-x, draw_width);
                        let source_clip = c_min(
                            cu_size,
                            app_round(clip as f32 * cu_size as f32 / draw_width as f32),
                        );
                        x += clip;
                        draw_width -= clip;
                        cu += source_clip;
                        cu_size -= source_clip;
                    }
                    if y < 0 {
                        let clip = c_min(-y, draw_height);
                        let source_clip = c_min(
                            cv_size,
                            app_round(clip as f32 * cv_size as f32 / draw_height as f32),
                        );
                        y += clip;
                        draw_height -= clip;
                        cvv += source_clip;
                        cv_size -= source_clip;
                    }
                    if (x.wrapping_add(draw_width) as f32) > cv.clip_x && draw_width > 0 {
                        let keep = c_max(0, (cv.clip_x - x as f32) as i32);
                        cu_size = c_min(
                            cu_size,
                            app_round(keep as f32 * cu_size as f32 / draw_width as f32),
                        );
                        draw_width = keep;
                    }
                    if (y.wrapping_add(draw_height) as f32) > cv.clip_y && draw_height > 0 {
                        let keep = c_max(0, (cv.clip_y - y as f32) as i32);
                        cv_size = c_min(
                            cv_size,
                            app_round(keep as f32 * cv_size as f32 / draw_height as f32),
                        );
                        draw_height = keep;
                    }
                }
                if draw_width > 0 && draw_height > 0 && cu_size > 0 && cv_size > 0 {
                    draw_char(
                        h,
                        cv,
                        slot,
                        a.flags,
                        CharRect {
                            x: (cv.org_x + x as f32) as i32,
                            y: (cv.org_y + y as f32) as i32,
                            xl: draw_width,
                            yl: draw_height,
                            u: cu,
                            v: cvv,
                            ul: cu_size,
                            vl: cv_size,
                        },
                        a.color,
                    );
                }
            }

            if draw_underline {
                char_width = underline_width;
            }
            if !underline_next {
                line_x = line_x
                    .wrapping_add(scale_font_metric(char_width, ts).wrapping_add(scaled_space_x));
            } else {
                underline_width = c.u_size;
            }
            draw_underline = underline_next;
            break 'try_char;
        }
        i += 1;
    }

    for slot in 0..TEXT_SLOTS {
        if !h.slot_texture(slot).is_null() {
            h.slot_unlock(slot);
        }
    }
    line_x
}

/// `WrappedPrint`. `text` is NUL-terminated. Returns `(XL, YL)`; advances `cv.cur_x` / `cv.cur_y`.
///
/// Divergences from C (documented in the report): a null `font` returns
/// `(0, 0)` (C dereferences it); a negative `num_chars` returns `(0, 0)` (C
/// moves `Text` backwards, which is undefined behaviour).
pub fn wrapped_print<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    mut font: Font,
    style: i32,
    center: bool,
    text: &[u16],
    mut num_chars: i32,
) -> (i32, i32) {
    let do_teletype = num_chars != 0;
    let ts = text_scale(h, cv);
    if !has_safe_request_floats(cv, ts) || cv.clip_x < 0.0 || cv.clip_y < 0.0 || at(text, 0) == 0 {
        return (0, 0);
    }
    if font.is_null() || (do_teletype && num_chars < 0) {
        return (0, 0);
    }
    if (font == cv.large_font || font == cv.big_font) && !h.language_is_int() {
        font = cv.med_font;
    }

    let color = cv.color;
    let poly = text_poly_flags(style);
    let native_req = Hp2NativeTextRequest {
        flags: poly,
        draw_x: cv.cur_x as i32,
        draw_y: cv.cur_y as i32,
        clip: 0,
        amp: 0,
        center: u8::from(center),
        wrap: 1,
        visible: num_chars,
    };
    if let Some((native_w, native_h)) = h.try_native_text(cv, &native_req, font, text, color) {
        cv.cur_y += native_h as f32;
        return (native_w, native_h);
    }

    if h.font_has_name(font) {
        let mut x = cv.cur_x as i32;
        let mut y = cv.cur_y as i32;
        let mut flags = poly;
        if center {
            flags |= PF_TWO_SIDED;
        }
        h.viewport_draw_string(cv, flags, font, &mut x, &mut y, text, color);
        return ((x as f32 - cv.cur_x) as i32, (y as f32 - cv.cur_y) as i32);
    }

    let scaled_space_x = app_round(cv.space_x * ts);
    let scaled_space_y = app_round(cv.space_y * ts);
    let mut xl = 0;
    let mut yl = 0;
    let orig_x = cv.cur_x;
    let mut pos = 0usize;
    loop {
        let t = &text[pos..];
        let mut clean_word_end: i32 = 0;
        let mut test_xl: i32 = if center { 0 } else { cv.cur_x as i32 };
        let mut clean_xl: i32 = 0;
        let mut test_yl: i32 = 0;
        let mut clean_yl: i32 = 0;
        let mut got_word = false;
        let mut test: usize = 0;
        while at(t, test) != 0 && at(t, test) != u16::from(b'\n') {
            let ch = at(t, test);
            let (mut ch_w, mut ch_h) = get_char_size(h, font, ch, ts);
            if ch_w == 0 && is_lower_ascii(i32::from(ch)) {
                let upper = (i32::from(ch) + i32::from(b'A') - i32::from(b'a')) as u16;
                (ch_w, ch_h) = get_char_size(h, font, upper, ts);
            }
            test_xl = test_xl.wrapping_add(ch_w.wrapping_add(scaled_space_x));
            test_yl = c_max(test_yl, ch_h.wrapping_add(scaled_space_y));
            if (test_xl as f32) > cv.clip_x {
                break;
            }
            test += 1;
            let next = at(t, test);
            let word_break = next == u16::from(b' ') || next == u16::from(b'\n') || next == 0;
            if word_break || !got_word {
                clean_word_end = test as i32;
                clean_xl = test_xl;
                clean_yl = test_yl;
                got_word = got_word || word_break;
            }
        }
        if clean_word_end == 0 {
            break;
        }
        if do_teletype && clean_word_end > num_chars {
            clean_word_end = num_chars;
        }

        if style != STY_NONE
            && cv.has_frame != 0
            && cv.org_y + cv.cur_y < cv.frame_y as f32
            && cv.org_y + cv.cur_y + clean_yl as f32 > 0.0
        {
            let mut word: Vec<u16> = t[..clean_word_end as usize].to_vec();
            word.push(0);
            let line_x0 = if center {
                (cv.cur_x - (clean_xl / 2) as f32) as i32
            } else {
                cv.cur_x as i32
            };
            let advance = draw_string(
                h,
                cv,
                &DrawStringArgs {
                    flags: poly,
                    font,
                    x: line_x0,
                    y: cv.cur_y as i32,
                    color,
                    clip: false,
                    amp: false,
                },
                &word,
            );
            cv.cur_x = line_x0.wrapping_add(advance) as f32;
        }

        cv.cur_x = orig_x;
        cv.cur_y += clean_yl as f32;
        yl += clean_yl;
        xl = c_max(xl, clean_xl);
        pos += clean_word_end as usize;
        if do_teletype {
            num_chars -= clean_word_end;
            if num_chars == 0 {
                break;
            }
        }
        while at(text, pos) == u16::from(b' ') {
            pos += 1;
        }
        if at(text, pos) == 0 {
            break;
        }
    }
    (xl, yl)
}

// ---------------------------------------------------------------------------
// exec* natives (bodies). Log and state handling are the host's job.
// ---------------------------------------------------------------------------

/// `execStrLen`: `WrappedStrLenf(Font, XL, YL, "%s", *InText)` with CurX/CurY saved.
/// The C++ shim truncates to the 4095-unit `GET_VARARGS` buffer before calling.
pub fn native_str_len<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    text: &[u16],
) -> (i32, i32) {
    let old_x = cv.cur_x;
    let old_y = cv.cur_y;
    cv.cur_x = 0.0;
    cv.cur_y = 0.0;
    let font = cv.font;
    let r = wrapped_print(h, cv, font, STY_NONE, false, text, 0);
    cv.cur_y = old_y;
    cv.cur_x = old_x;
    r
}

/// `execDrawText`. `cr` is `CR` (newline after the text); the unused `bCursor`
/// argument of `WrappedPrint` is not passed.
pub fn native_draw_text<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    text: &[u16],
    cr: bool,
    num_chars: i32,
) {
    if cv.font.is_null() {
        h.log(LOG_DRAW_TEXT_NO_FONT);
        return;
    }
    let mut xl = 0;
    let mut yl = 0;
    if cv.style != STY_NONE {
        let font = cv.font;
        let style = cv.style;
        let center = cv.center != 0;
        (xl, yl) = wrapped_print(h, cv, font, style, center, text, num_chars);
    }
    cv.cur_x += xl as f32;
    cv.cur_yl = c_max(cv.cur_yl, yl as f32);
    if cr {
        cv.cur_x = 0.0;
        cv.cur_y += cv.cur_yl;
        cv.cur_yl = 0.0;
    }
}

/// `execDrawTextClipped`. Mutates `cv.font` when the LargeFont/BigFont swap applies.
pub fn native_draw_text_clipped<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    text: &[u16],
    check_hotkey: bool,
) {
    if cv.font.is_null() {
        h.log(LOG_DRAW_TEXT_CLIPPED_NO_FONT);
        return;
    }
    if (cv.font == cv.large_font || cv.font == cv.big_font) && !h.language_is_int() {
        cv.font = cv.med_font;
    }
    let poly = text_poly_flags(cv.style);
    let ts = text_scale(h, cv);
    if !has_safe_request_floats(cv, ts) {
        return;
    }
    let args = DrawStringArgs {
        flags: poly,
        font: cv.font,
        x: cv.cur_x as i32,
        y: cv.cur_y as i32,
        color: cv.color,
        clip: true,
        amp: check_hotkey,
    };
    draw_string(h, cv, &args, text);
}

/// `execTextSize`: `WrappedPrint(STY_None, ...)` with CurX/CurY/ClipX saved, ClipX = 32767.
/// Returns `None` when the font is null (the log line is emitted).
pub fn native_text_size<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    text: &[u16],
) -> Option<(i32, i32)> {
    if cv.font.is_null() {
        h.log(LOG_TEXT_SIZE_NO_FONT);
        return None;
    }
    let old_x = cv.cur_x;
    let old_y = cv.cur_y;
    let old_clip_x = cv.clip_x;
    cv.cur_x = 0.0;
    cv.cur_y = 0.0;
    cv.clip_x = 32767.0;
    let font = cv.font;
    let r = wrapped_print(h, cv, font, STY_NONE, false, text, 0);
    cv.cur_y = old_y;
    cv.cur_x = old_x;
    cv.clip_x = old_clip_x;
    Some(r)
}

/// `execDrawTile` body (texture is non-null; the shim checks it and logs).
pub fn native_draw_tile<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    tex: *mut c_void,
    rect: Hp2CanvasTileRect,
) {
    if cv.style != STY_NONE {
        let draw = Hp2CanvasTileDraw {
            tex,
            rect: Hp2CanvasTileRect {
                x: cv.org_x + cv.cur_x,
                y: cv.org_y + cv.cur_y,
                ..rect
            },
            span: std::ptr::null_mut(),
            z: cv.z,
            color: cv.color,
            fog: Hp2Plane::ZERO,
            flags: PF_TWO_SIDED
                | tile_style_flags(cv.style)
                | if cv.no_smooth != 0 { PF_NO_SMOOTH } else { 0 },
        };
        h.draw_tile_texture(cv, &draw);
    }
    cv.cur_x += rect.xl + cv.space_x;
    cv.cur_yl = c_max(cv.cur_yl, rect.yl);
}

/// `execDrawTileClipped` body (texture is non-null; the shim checks it and logs).
/// The masked flag is intentionally absent (commented out in the original).
pub fn native_draw_tile_clipped<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &mut Hp2CanvasView,
    tex: *mut c_void,
    rect: Hp2CanvasTileRect,
) {
    let Hp2CanvasTileRect {
        mut xl,
        mut yl,
        mut u,
        mut v,
        mut ul,
        mut vl,
        ..
    } = rect;
    if xl > 0.0 && yl > 0.0 {
        if cv.cur_x < 0.0 {
            let c = cv.cur_x * ul / xl;
            u -= c;
            ul += c;
            xl += cv.cur_x;
            cv.cur_x = 0.0;
        }
        if cv.cur_y < 0.0 {
            let c = cv.cur_y * vl / yl;
            v -= c;
            vl += c;
            yl += cv.cur_y;
            cv.cur_y = 0.0;
        }
        if xl > cv.clip_x - cv.cur_x {
            ul += (cv.clip_x - cv.cur_x - xl) * ul / xl;
            xl = cv.clip_x - cv.cur_x;
        }
        if yl > cv.clip_y - cv.cur_y {
            vl += (cv.clip_y - cv.cur_y - yl) * vl / yl;
            yl = cv.clip_y - cv.cur_y;
        }
        if cv.style != STY_NONE {
            let draw = Hp2CanvasTileDraw {
                tex,
                rect: Hp2CanvasTileRect {
                    x: cv.org_x + cv.cur_x,
                    y: cv.org_y + cv.cur_y,
                    xl,
                    yl,
                    u,
                    v,
                    ul,
                    vl,
                },
                span: std::ptr::null_mut(),
                z: cv.z,
                color: cv.color,
                fog: Hp2Plane::ZERO,
                flags: PF_TWO_SIDED
                    | tile_style_flags(cv.style)
                    | if cv.no_smooth != 0 { PF_NO_SMOOTH } else { 0 },
            };
            h.draw_tile_texture(cv, &draw);
        }
        cv.cur_x += xl + cv.space_x;
        cv.cur_yl = c_max(cv.cur_yl, yl);
    }
}

/// `Translucent ? PF_Translucent : Modulated ? PF_Modulated : 0` (tile paths; no masked).
fn tile_style_flags(style: i32) -> u32 {
    if style == STY_TRANSLUCENT {
        PF_TRANSLUCENT
    } else if style == STY_MODULATED {
        PF_MODULATED
    } else {
        0
    }
}

/// `UCanvas::DrawPattern`: `DrawTile(Texture, X, Y, XL, YL, (X-OrgX)*Scale + USize, (Y-OrgY)*Scale + VSize, XL*Scale, YL*Scale, ...)`.
pub fn draw_pattern<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &Hp2CanvasView,
    a: &Hp2CanvasPatternArgs,
) {
    let size = h.texture_size(Texture(a.tex));
    let draw = Hp2CanvasTileDraw {
        tex: a.tex,
        rect: Hp2CanvasTileRect {
            x: a.x,
            y: a.y,
            xl: a.xl,
            yl: a.yl,
            // Original arm64 object contracts these to `fmadd` (see UnCanvas.cpp.o); `mul_add` matches.
            u: (a.x - a.org_x).mul_add(a.scale, size.w as f32),
            v: (a.y - a.org_y).mul_add(a.scale, size.h as f32),
            ul: a.xl * a.scale,
            vl: a.yl * a.scale,
        },
        span: a.span,
        z: a.z,
        color: a.color,
        fog: a.fog,
        flags: a.flags,
    };
    h.draw_tile_texture(cv, &draw);
}

/// `UCanvas::DrawIcon`: `DrawTile(Texture, ScreenX, ScreenY, XSize, YSize, 0, 0, USize, VSize, ...)`.
/// Reuses the pattern argument layout: `x/y/xl/yl` plus `z`, `color`, `fog`, `flags`, `span`.
pub fn draw_icon<H: CanvasTextHost + ?Sized>(
    h: &mut H,
    cv: &Hp2CanvasView,
    a: &Hp2CanvasPatternArgs,
) {
    let size = h.texture_size(Texture(a.tex));
    let draw = Hp2CanvasTileDraw {
        tex: a.tex,
        rect: Hp2CanvasTileRect {
            x: a.x,
            y: a.y,
            xl: a.xl,
            yl: a.yl,
            u: 0.0,
            v: 0.0,
            ul: size.w as f32,
            vl: size.h as f32,
        },
        span: a.span,
        z: a.z,
        color: a.color,
        fog: a.fog,
        flags: a.flags,
    };
    h.draw_tile_texture(cv, &draw);
}

// ---------------------------------------------------------------------------
// Production host: the C++ `Hp2CanvasServices` table.
// ---------------------------------------------------------------------------

/// Per-slice services table filled by `Engine/Src/Hp2CanvasServices.cpp`.
/// Every entry is a plain C function pointer; `canvas`, `slots`, and `stack`
/// are opaque engine pointers owned by the C++ caller.
#[repr(C)]
pub struct Hp2CanvasServices {
    pub ui_scale: unsafe extern "C" fn(canvas: *mut c_void) -> f32,
    pub language_is_int: unsafe extern "C" fn() -> u8,
    pub viewport_draw_string: unsafe extern "C" fn(
        canvas: *mut c_void,
        flags: u32,
        font: Font,
        x: *mut i32,
        y: *mut i32,
        text: *const u16,
        color: Hp2Plane,
    ),
    pub try_native_text: unsafe extern "C" fn(
        canvas: *mut c_void,
        view: *const Hp2CanvasView,
        req: *const Hp2NativeTextRequest,
        font: Font,
        text: *const u16,
        color: Hp2Plane,
        out: *mut Hp2CanvasSize,
    ) -> u8,
    pub font_chars_per_page: unsafe extern "C" fn(font: Font) -> i32,
    pub font_remap_char: unsafe extern "C" fn(font: Font, ch: u16) -> u16,
    pub font_page_count: unsafe extern "C" fn(font: Font) -> i32,
    pub font_page_texture: unsafe extern "C" fn(font: Font, page: i32) -> Texture,
    pub font_page_char_count: unsafe extern "C" fn(font: Font, page: i32) -> i32,
    pub font_page_char: unsafe extern "C" fn(font: Font, page: i32, index: i32) -> Hp2FontChar,
    pub font_has_name: unsafe extern "C" fn(font: Font) -> u8,
    pub slot_reset: unsafe extern "C" fn(slots: *mut c_void),
    pub slot_texture: unsafe extern "C" fn(slots: *mut c_void, slot: i32) -> Texture,
    pub slot_lock:
        unsafe extern "C" fn(canvas: *mut c_void, slots: *mut c_void, slot: i32, tex: Texture),
    pub slot_unlock: unsafe extern "C" fn(slots: *mut c_void, slot: i32),
    pub draw_tile_slot: unsafe extern "C" fn(
        canvas: *mut c_void,
        slots: *mut c_void,
        slot: i32,
        rect: *const Hp2CanvasTileRect,
        color: Hp2Plane,
        flags: u32,
    ),
    pub draw_tile_texture:
        unsafe extern "C" fn(canvas: *mut c_void, draw: *const Hp2CanvasTileDraw),
    pub texture_size: unsafe extern "C" fn(tex: Texture) -> Hp2CanvasSize,
    pub log: unsafe extern "C" fn(stack: *mut c_void, code: u32),
}

/// Engine pointers for one FFI call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2CanvasTextCtx {
    pub services: *const Hp2CanvasServices,
    pub canvas: *mut c_void,
    pub slots: *mut c_void,
    pub stack: *mut c_void,
}

/// [`CanvasTextHost`] over the C++ services table.
///
/// # Safety
/// `ctx.services` must point to a live table, and `ctx.canvas`, `ctx.slots`
/// (when used) and `ctx.stack` must be the engine pointers for the call.
pub struct ServiceHost {
    ctx: Hp2CanvasTextCtx,
}

impl ServiceHost {
    /// # Safety
    /// See [`ServiceHost`].
    pub unsafe fn new(ctx: Hp2CanvasTextCtx) -> Self {
        Self { ctx }
    }

    fn svc(&self) -> &Hp2CanvasServices {
        // SAFETY: the caller guarantees `ctx.services` is live (see `ServiceHost::new`).
        unsafe { &*self.ctx.services }
    }
}

impl CanvasTextHost for ServiceHost {
    fn ui_scale(&mut self) -> f32 {
        let f = self.svc().ui_scale;
        // SAFETY: service call on the live canvas.
        unsafe { f(self.ctx.canvas) }
    }

    fn language_is_int(&mut self) -> bool {
        let f = self.svc().language_is_int;
        // SAFETY: service call.
        unsafe { f() != 0 }
    }

    fn viewport_draw_string(
        &mut self,
        _cv: &Hp2CanvasView,
        flags: u32,
        font: Font,
        x: &mut i32,
        y: &mut i32,
        text: &[u16],
        color: Hp2Plane,
    ) {
        let f = self.svc().viewport_draw_string;
        // SAFETY: `text` is NUL-terminated by the logic contract; `x`/`y` are live.
        unsafe { f(self.ctx.canvas, flags, font, x, y, text.as_ptr(), color) }
    }

    fn try_native_text(
        &mut self,
        cv: &Hp2CanvasView,
        req: &Hp2NativeTextRequest,
        font: Font,
        text: &[u16],
        color: Hp2Plane,
    ) -> Option<(i32, i32)> {
        let f = self.svc().try_native_text;
        let mut out = Hp2CanvasSize::default();
        // SAFETY: all pointers are live for the call; the C++ side syncs `cv` into the canvas first.
        let ok = unsafe {
            f(
                self.ctx.canvas,
                cv,
                req,
                font,
                text.as_ptr(),
                color,
                &mut out,
            )
        };
        (ok != 0).then_some((out.w, out.h))
    }

    fn font_chars_per_page(&mut self, font: Font) -> i32 {
        let f = self.svc().font_chars_per_page;
        // SAFETY: font is non-null (checked by callers).
        unsafe { f(font) }
    }

    fn font_remap_char(&mut self, font: Font, ch: u16) -> u16 {
        let f = self.svc().font_remap_char;
        // SAFETY: font is non-null.
        unsafe { f(font, ch) }
    }

    fn font_page_count(&mut self, font: Font) -> i32 {
        let f = self.svc().font_page_count;
        // SAFETY: font is non-null.
        unsafe { f(font) }
    }

    fn font_page_texture(&mut self, font: Font, page: i32) -> Texture {
        let f = self.svc().font_page_texture;
        // SAFETY: font is non-null; page is in range.
        unsafe { f(font, page) }
    }

    fn font_page_char_count(&mut self, font: Font, page: i32) -> i32 {
        let f = self.svc().font_page_char_count;
        // SAFETY: font is non-null; page is in range.
        unsafe { f(font, page) }
    }

    fn font_page_char(&mut self, font: Font, page: i32, index: i32) -> Hp2FontChar {
        let f = self.svc().font_page_char;
        // SAFETY: font is non-null; page and index are in range.
        unsafe { f(font, page, index) }
    }

    fn font_has_name(&mut self, font: Font) -> bool {
        let f = self.svc().font_has_name;
        // SAFETY: font is non-null.
        unsafe { f(font) != 0 }
    }

    fn slot_reset(&mut self) {
        let f = self.svc().slot_reset;
        // SAFETY: slots is the live per-call slot array.
        unsafe { f(self.ctx.slots) }
    }

    fn slot_texture(&mut self, slot: usize) -> Texture {
        let f = self.svc().slot_texture;
        // SAFETY: slot < TEXT_SLOTS.
        unsafe { f(self.ctx.slots, slot as i32) }
    }

    fn slot_lock(&mut self, _cv: &Hp2CanvasView, slot: usize, tex: Texture) {
        let f = self.svc().slot_lock;
        // SAFETY: canvas/slots are live; tex is non-null.
        unsafe { f(self.ctx.canvas, self.ctx.slots, slot as i32, tex) }
    }

    fn slot_unlock(&mut self, slot: usize) {
        let f = self.svc().slot_unlock;
        // SAFETY: slot < TEXT_SLOTS.
        unsafe { f(self.ctx.slots, slot as i32) }
    }

    fn draw_tile_slot(
        &mut self,
        _cv: &Hp2CanvasView,
        slot: usize,
        rect: &Hp2CanvasTileRect,
        color: Hp2Plane,
        flags: u32,
    ) {
        let f = self.svc().draw_tile_slot;
        // SAFETY: rect is a live stack value.
        unsafe {
            f(
                self.ctx.canvas,
                self.ctx.slots,
                slot as i32,
                rect,
                color,
                flags,
            )
        }
    }

    fn draw_tile_texture(&mut self, _cv: &Hp2CanvasView, draw: &Hp2CanvasTileDraw) {
        let f = self.svc().draw_tile_texture;
        // SAFETY: draw is a live stack value.
        unsafe { f(self.ctx.canvas, draw) }
    }

    fn texture_size(&mut self, tex: Texture) -> Hp2CanvasSize {
        let f = self.svc().texture_size;
        // SAFETY: tex is non-null.
        unsafe { f(tex) }
    }

    fn log(&mut self, code: u32) {
        let f = self.svc().log;
        // SAFETY: stack is the live FFrame of the calling native (may be null for shims).
        unsafe { f(self.ctx.stack, code) }
    }
}
