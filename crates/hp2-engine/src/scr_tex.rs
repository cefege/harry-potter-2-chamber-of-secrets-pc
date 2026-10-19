//! Port of `Engine/Src/UnScrTex.cpp` (UScriptedTexture): `DrawString` and
//! `GetCharSize`, the `DrawTile` pixel kernel, the PostLoad/Tick copies, and the
//! bodies of the `execDrawText`, `execDrawColoredText`, `execDrawTile`,
//! `execReplaceTexture`, and `execTextSize` natives.
//!
//! Engine objects are reached only through [`ScrTexHost`]. The production host,
//! [`ServiceHost`], calls the C++ `Hp2ScrTexServices` table; C++ keeps the UObject
//! lifecycle, the exec-stack reads, and the texture `Lock`/`Unlock` calls whose
//! `FTime` argument is not expressible here. Palette-map storage is owned by Rust
//! (see [`PaletteCache`]); C++ holds the opaque pointer.
//!
//! Divergences from the C++ (undefined behaviour there; defined here):
//! - PostLoad shifts by a negative scaler (`UBits - SourceTexture->UBits` < 0):
//!   C++ UB. Rust uses `wrapping_shr`/`wrapping_shl`, i.e. the shift count mod 32
//!   that arm64 hardware applies.
//! - Signed overflow in the fixed-point sums (`FixU`, `FixV`, `XFixStart`,
//!   `YFixStart`): Rust `wrapping_add`/`wrapping_sub`.
//! - `appRound` of a non-finite or out-of-range float: `as i32` saturates; C is UB.
//! - Source-mip reads (DrawTile and PostLoad) are unchecked raw reads at the C++
//!   address `base + index`, as in the original. DrawTile's masks keep every index
//!   inside `USize * VSize`. PostLoad's index `(U >> UScaler) + ((V >> VScaler) <<
//!   SrcUBits)` is a signed 32-bit `int` in C++ (`sxtw`), so it is sign-extended here
//!   too. It stays inside the source mip only for power-of-two source sizes; for other
//!   sizes the C++ reads adjacent heap bytes, and Rust performs the same read (UB in
//!   Rust terms, same value as C++).
//! - `DrawTile(bUseColor=1, bMasked=0)` with a source pixel: C++ takes the unmasked
//!   branch and dereferences the NULL PalMap, so it faults. Rust calls the
//!   `palette_map_lookup` service with a null map, which faults the same way. No
//!   caller reaches it (`DrawString` always passes `bMasked=1`; the natives use
//!   `bUseColor=0`).
//! - `CharactersPerPage == 0` divides by zero (SIGFPE in C++). Rust panics and
//!   `hp2_ffi_support::guard` catches it.
//! - `ReplaceTexture` copies with `ptr::copy` (memmove). C++ `appMemcpy` is UB if
//!   source and destination overlap, which happens only when a texture replaces
//!   itself.

use std::collections::HashMap;
use std::ffi::c_void;

use hp2_abi::FColor;

use crate::canvas_text::Hp2FontChar;
use crate::math::app_round;
use crate::tex::{self, NUM_PAL_COLORS};

/// Log codes; the C++ services map each to its byte-identical `TEXT(...)` literal.
pub const LOG_DRAW_TEXT_NO_FONT: u32 = 0;
pub const LOG_REPLACE_MISSING_TEXTURE: u32 = 1;
pub const LOG_REPLACE_DIMENSIONS: u32 = 2;
pub const LOG_DRAW_TILE_MISSING_TEXTURE: u32 = 3;
pub const LOG_TEXT_SIZE_NO_FONT: u32 = 4;

/// `(FLOAT)(1 << 19)`: the fixed-point scale of the source walk.
const FIX_SCALE: f32 = 524_288.0;

/// Opaque engine object pointer (`UFont*`, `UTexture*`, `FTextureInfo*`,
/// `UScriptedTexture*`, `FFrame*`, or a `TCHAR` string). A newtype so the
/// [`ScrTexHost`] methods that forward it do not trip `not_unsafe_ptr_arg_deref`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjPtr(pub *mut c_void);

impl ObjPtr {
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}

/// `TMap<UTexture*, TArray<BYTE>>` replacement: 256-entry palette maps keyed by
/// the source texture address. Boxes keep each entry at a stable address.
pub type PaletteCache = HashMap<usize, Box<[u8; NUM_PAL_COLORS]>>;

/// One DrawTile destination rectangle plus source origin/extent (`FLOAT` in C++).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRect {
    pub x: f32,
    pub y: f32,
    pub xl: f32,
    pub yl: f32,
    pub u: f32,
    pub v: f32,
    pub ul: f32,
    pub vl: f32,
}

/// Engine operations the port needs. The `self_*` methods act on the owning
/// `UScriptedTexture`; `info` is the caller's `FTextureInfo` scratch.
pub trait ScrTexHost {
    // UFont
    fn font_chars_per_page(&self, font: ObjPtr) -> i32;
    /// `(TCHARU)Font->RemapChar((TCHAR)ch)`.
    fn font_remap_char(&self, font: ObjPtr, ch: i32) -> i32;
    fn font_page_count(&self, font: ObjPtr) -> i32;
    /// `Font->Pages(page).Texture`, may be null.
    fn font_page_texture(&self, font: ObjPtr, page: i32) -> ObjPtr;
    fn font_page_char_count(&self, font: ObjPtr, page: i32) -> i32;
    fn font_page_char(&self, font: ObjPtr, page: i32, index: i32) -> Hp2FontChar;
    /// Raw `(int)Text[index]`; 0 terminates the string.
    fn text_char(&self, text: ObjPtr, index: i32) -> i32;

    // UTexture (the argument texture)
    fn tex_usize(&self, tex: ObjPtr) -> i32;
    fn tex_vsize(&self, tex: ObjPtr) -> i32;
    fn tex_ubits(&self, tex: ObjPtr) -> i32;
    fn tex_vbits(&self, tex: ObjPtr) -> i32;
    /// `&tex->Mips(0).DataArray(0)`.
    fn tex_mip0(&self, tex: ObjPtr) -> ObjPtr;
    /// Copies `tex->Palette->Colors(0..256)`.
    fn tex_palette_colors(&self, tex: ObjPtr, out: &mut [FColor; NUM_PAL_COLORS]);
    /// `tex->Get(self->LocalTime)`.
    fn tex_get_local(&self, tex: ObjPtr) -> ObjPtr;
    /// `tex->Lock(*info, self->LocalTime, 0, NULL)`.
    fn tex_lock_local(&mut self, tex: ObjPtr);
    /// `tex->Lock(*info, appSeconds(), 0, NULL)`.
    fn tex_lock_now(&mut self, tex: ObjPtr);
    fn tex_unlock(&mut self, tex: ObjPtr);
    /// `info->Mips[0]->DataPtr`.
    fn info_mip0(&self) -> ObjPtr;

    // Owning UScriptedTexture
    fn self_usize(&self) -> i32;
    fn self_vsize(&self) -> i32;
    fn self_ubits(&self) -> i32;
    fn self_vbits(&self) -> i32;
    fn self_mip0(&self) -> ObjPtr;
    fn self_local_bitmap(&self) -> ObjPtr;
    /// `if(LocalSourceBitmap) delete LocalSourceBitmap; LocalSourceBitmap = new BYTE[bytes];`
    fn self_alloc_local_bitmap(&mut self, bytes: i32) -> ObjPtr;
    fn self_source_texture(&self) -> ObjPtr;
    fn self_old_source(&self) -> ObjPtr;
    fn self_set_old_source(&mut self, tex: ObjPtr);
    /// `Palette = tex->Palette`.
    fn self_copy_palette_from(&mut self, tex: ObjPtr);
    /// Copies `Palette->Colors(0..256)`.
    fn self_palette_colors(&self, out: &mut [FColor; NUM_PAL_COLORS]);
    /// `Palette->BestMatch(color, 0)`; `color` is the `FColor` packed as `int4`.
    fn self_palette_best_match(&self, color: u32) -> u8;
    /// `(*PalMap)(index)` where `map` is the `TArray<BYTE>*` of DrawTile's
    /// `bUseColor=0` path. Only the `bUseColor=1, bMasked=0` path calls it, with a
    /// null map, so the C++ dereference faults.
    fn palette_map_lookup(&self, map: ObjPtr, index: u8) -> u8;
    /// `if(NotifyActor) NotifyActor->eventRenderTexture(this)`.
    fn self_notify_render(&mut self);

    /// Rust-owned `PaletteMap` storage.
    fn palette_cache(&mut self) -> &mut PaletteCache;

    /// `Stack.Logf(TEXT(...))` for the given log code.
    fn log(&mut self, code: u32);
}

/// C++ `Hp2ScrTexServices` (defined in `Src/Hp2ScrTexServices.cpp`). Field order and
/// types must match the C++ struct. Every call takes engine pointers only.
#[repr(C)]
pub struct Hp2ScrTexServices {
    pub font_chars_per_page: unsafe extern "C" fn(font: *mut c_void) -> i32,
    pub font_remap_char: unsafe extern "C" fn(font: *mut c_void, ch: i32) -> i32,
    pub font_page_count: unsafe extern "C" fn(font: *mut c_void) -> i32,
    pub font_page_texture: unsafe extern "C" fn(font: *mut c_void, page: i32) -> *mut c_void,
    pub font_page_char_count: unsafe extern "C" fn(font: *mut c_void, page: i32) -> i32,
    pub font_page_char:
        unsafe extern "C" fn(font: *mut c_void, page: i32, index: i32) -> Hp2FontChar,
    pub text_char: unsafe extern "C" fn(text: *const c_void, index: i32) -> i32,
    pub tex_usize: unsafe extern "C" fn(tex: *mut c_void) -> i32,
    pub tex_vsize: unsafe extern "C" fn(tex: *mut c_void) -> i32,
    pub tex_ubits: unsafe extern "C" fn(tex: *mut c_void) -> i32,
    pub tex_vbits: unsafe extern "C" fn(tex: *mut c_void) -> i32,
    pub tex_mip0: unsafe extern "C" fn(tex: *mut c_void) -> *mut c_void,
    pub tex_palette_colors: unsafe extern "C" fn(tex: *mut c_void, out: *mut c_void),
    pub tex_get_local: unsafe extern "C" fn(obj: *mut c_void, tex: *mut c_void) -> *mut c_void,
    pub tex_lock_local: unsafe extern "C" fn(obj: *mut c_void, tex: *mut c_void, info: *mut c_void),
    pub tex_lock_now: unsafe extern "C" fn(tex: *mut c_void, info: *mut c_void),
    pub tex_unlock: unsafe extern "C" fn(tex: *mut c_void, info: *mut c_void),
    pub info_mip0: unsafe extern "C" fn(info: *mut c_void) -> *mut c_void,
    pub self_usize: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub self_vsize: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub self_ubits: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub self_vbits: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub self_mip0: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    pub self_local_bitmap: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    pub self_alloc_local_bitmap: unsafe extern "C" fn(obj: *mut c_void, bytes: i32) -> *mut c_void,
    pub self_source_texture: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    pub self_old_source: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    pub self_set_old_source: unsafe extern "C" fn(obj: *mut c_void, tex: *mut c_void),
    pub self_copy_palette_from: unsafe extern "C" fn(obj: *mut c_void, tex: *mut c_void),
    pub self_palette_colors: unsafe extern "C" fn(obj: *mut c_void, out: *mut c_void),
    pub self_palette_best_match: unsafe extern "C" fn(obj: *mut c_void, color: u32) -> u8,
    pub self_notify_render: unsafe extern "C" fn(obj: *mut c_void),
    pub palette_map_lookup: unsafe extern "C" fn(map: *mut c_void, index: u8) -> u8,
    pub log: unsafe extern "C" fn(stack: *mut c_void, code: u32),
}

/// Context the C++ shell passes for each call: the services table, the owning
/// `UScriptedTexture`, the exec frame (null outside natives), the `FTextureInfo`
/// scratch, and the Rust-owned palette map.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Hp2ScrTexCtx {
    pub services: *const Hp2ScrTexServices,
    pub obj: *mut c_void,
    pub stack: *mut c_void,
    pub info: *mut c_void,
    pub palette_map: *mut c_void,
}

/// Production host: every engine operation goes through the C++ services table.
pub struct ServiceHost {
    ctx: Hp2ScrTexCtx,
    cache: *mut PaletteCache,
}

impl ServiceHost {
    /// # Safety
    /// `ctx.services` points to a live `Hp2ScrTexServices`; `ctx.obj` is a live
    /// `UScriptedTexture`; `ctx.info` is a live `FTextureInfo`; `ctx.palette_map`
    /// is a `PaletteCache` from [`palette_cache_new`] that outlives the call.
    /// `ctx.stack` is a live `FFrame` for the log calls.
    pub unsafe fn new(ctx: Hp2ScrTexCtx) -> Self {
        let cache = ctx.palette_map as *mut PaletteCache;
        Self { ctx, cache }
    }

    fn svc(&self) -> &Hp2ScrTexServices {
        // SAFETY: see `ServiceHost::new`.
        unsafe { &*self.ctx.services }
    }

    fn obj(&self) -> *mut c_void {
        self.ctx.obj
    }

    fn info(&self) -> *mut c_void {
        self.ctx.info
    }
}

/// Allocates an empty palette map for the C++ `PaletteMap` member.
pub fn palette_cache_new() -> *mut PaletteCache {
    Box::into_raw(Box::new(PaletteCache::new()))
}

/// Frees a palette map from [`palette_cache_new`].
///
/// # Safety
/// `cache` came from [`palette_cache_new`] and is freed once.
pub unsafe fn palette_cache_free(cache: *mut PaletteCache) {
    if !cache.is_null() {
        // SAFETY: caller contract.
        drop(unsafe { Box::from_raw(cache) });
    }
}

impl ScrTexHost for ServiceHost {
    fn font_chars_per_page(&self, font: ObjPtr) -> i32 {
        // SAFETY: services table contract; `font` is a live UFont.
        unsafe { (self.svc().font_chars_per_page)(font.0) }
    }
    fn font_remap_char(&self, font: ObjPtr, ch: i32) -> i32 {
        unsafe { (self.svc().font_remap_char)(font.0, ch) }
    }
    fn font_page_count(&self, font: ObjPtr) -> i32 {
        unsafe { (self.svc().font_page_count)(font.0) }
    }
    fn font_page_texture(&self, font: ObjPtr, page: i32) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().font_page_texture)(font.0, page) })
    }
    fn font_page_char_count(&self, font: ObjPtr, page: i32) -> i32 {
        unsafe { (self.svc().font_page_char_count)(font.0, page) }
    }
    fn font_page_char(&self, font: ObjPtr, page: i32, index: i32) -> Hp2FontChar {
        unsafe { (self.svc().font_page_char)(font.0, page, index) }
    }
    fn text_char(&self, text: ObjPtr, index: i32) -> i32 {
        unsafe { (self.svc().text_char)(text.0, index) }
    }
    fn tex_usize(&self, tex: ObjPtr) -> i32 {
        unsafe { (self.svc().tex_usize)(tex.0) }
    }
    fn tex_vsize(&self, tex: ObjPtr) -> i32 {
        unsafe { (self.svc().tex_vsize)(tex.0) }
    }
    fn tex_ubits(&self, tex: ObjPtr) -> i32 {
        unsafe { (self.svc().tex_ubits)(tex.0) }
    }
    fn tex_vbits(&self, tex: ObjPtr) -> i32 {
        unsafe { (self.svc().tex_vbits)(tex.0) }
    }
    fn tex_mip0(&self, tex: ObjPtr) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().tex_mip0)(tex.0) })
    }
    fn tex_palette_colors(&self, tex: ObjPtr, out: &mut [FColor; NUM_PAL_COLORS]) {
        unsafe { (self.svc().tex_palette_colors)(tex.0, out.as_mut_ptr().cast()) }
    }
    fn tex_get_local(&self, tex: ObjPtr) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().tex_get_local)(self.obj(), tex.0) })
    }
    fn tex_lock_local(&mut self, tex: ObjPtr) {
        unsafe { (self.svc().tex_lock_local)(self.obj(), tex.0, self.info()) }
    }
    fn tex_lock_now(&mut self, tex: ObjPtr) {
        unsafe { (self.svc().tex_lock_now)(tex.0, self.info()) }
    }
    fn tex_unlock(&mut self, tex: ObjPtr) {
        unsafe { (self.svc().tex_unlock)(tex.0, self.info()) }
    }
    fn info_mip0(&self) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().info_mip0)(self.info()) })
    }
    fn self_usize(&self) -> i32 {
        unsafe { (self.svc().self_usize)(self.obj()) }
    }
    fn self_vsize(&self) -> i32 {
        unsafe { (self.svc().self_vsize)(self.obj()) }
    }
    fn self_ubits(&self) -> i32 {
        unsafe { (self.svc().self_ubits)(self.obj()) }
    }
    fn self_vbits(&self) -> i32 {
        unsafe { (self.svc().self_vbits)(self.obj()) }
    }
    fn self_mip0(&self) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().self_mip0)(self.obj()) })
    }
    fn self_local_bitmap(&self) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().self_local_bitmap)(self.obj()) })
    }
    fn self_alloc_local_bitmap(&mut self, bytes: i32) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().self_alloc_local_bitmap)(self.obj(), bytes) })
    }
    fn self_source_texture(&self) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().self_source_texture)(self.obj()) })
    }
    fn self_old_source(&self) -> ObjPtr {
        ObjPtr(unsafe { (self.svc().self_old_source)(self.obj()) })
    }
    fn self_set_old_source(&mut self, tex: ObjPtr) {
        unsafe { (self.svc().self_set_old_source)(self.obj(), tex.0) }
    }
    fn self_copy_palette_from(&mut self, tex: ObjPtr) {
        unsafe { (self.svc().self_copy_palette_from)(self.obj(), tex.0) }
    }
    fn self_palette_colors(&self, out: &mut [FColor; NUM_PAL_COLORS]) {
        unsafe { (self.svc().self_palette_colors)(self.obj(), out.as_mut_ptr().cast()) }
    }
    fn self_palette_best_match(&self, color: u32) -> u8 {
        unsafe { (self.svc().self_palette_best_match)(self.obj(), color) }
    }
    fn palette_map_lookup(&self, map: ObjPtr, index: u8) -> u8 {
        // SAFETY: services table contract; C++ dereferences `map` as the palette
        // `TArray<BYTE>*`, and the only caller passes null (see the trait doc).
        unsafe { (self.svc().palette_map_lookup)(map.0, index) }
    }
    fn self_notify_render(&mut self) {
        unsafe { (self.svc().self_notify_render)(self.obj()) }
    }
    fn palette_cache(&mut self) -> &mut PaletteCache {
        // SAFETY: see `ServiceHost::new`; the cache outlives the host.
        unsafe { &mut *self.cache }
    }
    fn log(&mut self, code: u32) {
        unsafe { (self.svc().log)(self.ctx.stack, code) }
    }
}

/// Font glyph size lookup used by `GetCharSize`. Returns `(USize, VSize)`, or
/// `(0, 0)` when the glyph is outside the font's pages.
pub fn char_size<H: ScrTexHost>(h: &H, font: ObjPtr, in_ch: i32) -> (i32, i32) {
    let ch = h.font_remap_char(font, in_ch);
    let cpp = h.font_chars_per_page(font);
    let page = ch / cpp;
    let index = ch - page * cpp;
    if page < h.font_page_count(font) && index < h.font_page_char_count(font, page) {
        let c = h.font_page_char(font, page, index);
        (c.u_size, c.v_size)
    } else {
        (0, 0)
    }
}

/// `DrawString`: draws `text` at `(draw_x, draw_y)` one glyph at a time, locking
/// each font page texture as it is entered. Returns the advanced `LineX`.
/// `entry` selects the colored (`Some(palette index)`) or textured path.
pub fn draw_string<H: ScrTexHost>(
    h: &mut H,
    font: ObjPtr,
    draw_x: i32,
    draw_y: i32,
    text: ObjPtr,
    entry: Option<u8>,
) -> i32 {
    let cpp = h.font_chars_per_page(font);
    let mut line_x: i32 = 0;
    let mut page: i32 = -1;
    let mut i: i32 = 0;
    loop {
        let raw = h.text_char(text, i);
        if raw == 0 {
            break;
        }
        let ch = h.font_remap_char(font, raw);
        let new_page = ch / cpp;
        if new_page < h.font_page_count(font) && !h.font_page_texture(font, new_page).is_null() {
            let index = ch - new_page * cpp;
            if index < h.font_page_char_count(font, new_page) {
                if new_page != page {
                    if page != -1 {
                        let old = h.font_page_texture(font, page);
                        h.tex_unlock(old);
                    }
                    page = new_page;
                    let t = h.font_page_texture(font, new_page);
                    h.tex_lock_now(t);
                }
                let glyph = h.font_page_char(font, new_page, index);
                let tex = h.font_page_texture(font, new_page);
                let rect = TileRect {
                    x: draw_x.wrapping_add(line_x) as f32,
                    y: draw_y as f32,
                    xl: glyph.u_size as f32,
                    yl: glyph.v_size as f32,
                    u: glyph.start_u as f32,
                    v: glyph.start_v as f32,
                    ul: glyph.u_size as f32,
                    vl: glyph.v_size as f32,
                };
                match entry {
                    Some(color) => draw_tile(h, rect, tex, true, true, color),
                    None => draw_tile(h, rect, tex, true, false, 0),
                }
                line_x = line_x.wrapping_add(glyph.u_size);
            }
        }
        i += 1;
    }
    if page != -1 {
        let last = h.font_page_texture(font, page);
        h.tex_unlock(last);
    }
    line_x
}

/// Palette map entry for `tex`, built on first use:
/// `Palette->BestMatch(Tex->Palette->Colors(i), 0)` for every `i < 256`.
fn palette_for<H: ScrTexHost>(h: &mut H, tex: ObjPtr) -> [u8; NUM_PAL_COLORS] {
    let key = tex.0 as usize;
    if let Some(entry) = h.palette_cache().get(&key) {
        return **entry;
    }
    let mut own = [tex::color_from_int4(0); NUM_PAL_COLORS];
    let mut src = [tex::color_from_int4(0); NUM_PAL_COLORS];
    h.self_palette_colors(&mut own);
    h.tex_palette_colors(tex, &mut src);
    let mut built = Box::new([0u8; NUM_PAL_COLORS]);
    for (out, color) in built.iter_mut().zip(src.iter()) {
        *out = tex::best_match(&own, *color, 0);
    }
    let copy = *built;
    h.palette_cache().insert(key, built);
    copy
}

/// The `DrawTile` pixel kernel: clips `rect`, then blits the locked source mip
/// (`Info`, via `info_mip0`) of `tex` into the owning texture.
/// `use_color && masked` writes `color_index` where the source is nonzero.
/// `!use_color` maps every source byte (or every nonzero one when `masked`)
/// through the palette map.
pub fn draw_tile<H: ScrTexHost>(
    h: &mut H,
    rect: TileRect,
    tex: ObjPtr,
    masked: bool,
    use_color: bool,
    color_index: u8,
) {
    let TileRect {
        x,
        y,
        xl,
        yl,
        mut u,
        mut v,
        ul,
        vl,
    } = rect;
    let usize_ = h.self_usize();
    let vsize = h.self_vsize();
    let src = h.info_mip0();
    let dst = h.self_mip0();
    if x + xl < 0.0 || y + yl < 0.0 || x > usize_ as f32 || y > vsize as f32 {
        return;
    }

    let pal = if use_color {
        None
    } else {
        Some(palette_for(h, tex))
    };

    let dx1 = app_round(x).max(0).min(usize_ - 1);
    // `U += (DX1 - X) * (UL / XL)` is one `fmadd` in the original (UnScrTex.cpp:243).
    u = ((dx1 as f32) - x).mul_add(ul / xl, u);
    let dx2 = app_round(xl + x).min(usize_ - 1);
    let dy1 = app_round(y).max(0).min(vsize - 1);
    // `V += (DY1 - Y) * (VL / YL)` is one `fmadd` in the original (UnScrTex.cpp:246).
    v = ((dy1 as f32) - y).mul_add(vl / yl, v);
    let dy2 = app_round(yl + y).min(vsize - 1);

    let tex_usize = h.tex_usize(tex);
    let tex_vsize = h.tex_vsize(tex);
    let x_step = app_round(ul / xl * FIX_SCALE);
    let fix_u = app_round(u * FIX_SCALE).wrapping_sub(x_step);
    let y_step = app_round(vl / yl * FIX_SCALE);
    let fix_v = app_round(v * FIX_SCALE).wrapping_sub(y_step);

    let walk = Walk {
        dx1,
        dx2,
        dy1,
        dy2,
        fix_u,
        fix_v,
        x_step,
        y_step,
        umask: (tex_usize.wrapping_sub(1)) as u32,
        vmask: (tex_vsize.wrapping_sub(1)) as u32,
        src_usize: tex_usize as u32,
        dst_usize: usize_,
        dst_base: dy1.wrapping_mul(usize_),
    };
    let src_ptr = src.0 as *const u8;
    let dst_ptr = dst.0 as *mut u8;
    // Destination indices produced by `walk` are `dy in [dy1, dy2)` and
    // `dx in [dx1, dx2)` with `dy2 <= vsize - 1` and `dx2 <= usize - 1`, so each
    // lies inside the `usize * vsize` destination mip.
    let put = |i: usize, value: u8| {
        debug_assert!(i < (usize_.max(0) as usize) * (vsize.max(0) as usize));
        // SAFETY: `i` is in range per the comment above; `dst` is the owning
        // texture's mip, valid for `usize * vsize` bytes.
        unsafe { *dst_ptr.add(i) = value };
    };

    if use_color && masked {
        // Masked monochrome blit using `color_index`.
        walk_pixels(&walk, src_ptr, |i, s| {
            if s != 0 {
                put(i, color_index);
            }
        });
    } else if use_color {
        // `use_color && !masked`: C++ takes the unmasked branch and dereferences the
        // NULL PalMap for each pixel. Rust makes the same call with a null map.
        walk_pixels(&walk, src_ptr, |i, s| {
            put(i, h.palette_map_lookup(ObjPtr(std::ptr::null_mut()), s));
        });
    } else if let Some(pal) = pal {
        if masked {
            // Masked textured blit: zero source bytes are transparent.
            walk_pixels(&walk, src_ptr, |i, s| {
                if s != 0 {
                    put(i, pal[s as usize]);
                }
            });
        } else {
            // Unmasked textured blit.
            walk_pixels(&walk, src_ptr, |i, s| put(i, pal[s as usize]));
        }
    }
}

/// Fixed-point walk state for one DrawTile call (`XFixStep`, `FixU`, ... in C++).
struct Walk {
    dx1: i32,
    dx2: i32,
    dy1: i32,
    dy2: i32,
    fix_u: i32,
    fix_v: i32,
    x_step: i32,
    y_step: i32,
    umask: u32,
    vmask: u32,
    src_usize: u32,
    dst_usize: i32,
    dst_base: i32,
}

/// Visits every destination pixel in the C++ loop order, handing `put` the
/// destination index and the source byte (read unchecked; see [`read_src`]).
fn walk_pixels(w: &Walk, src: *const u8, mut put: impl FnMut(usize, u8)) {
    let mut dst_base = w.dst_base;
    let mut y_fix = w.fix_v;
    for _dy in w.dy1..w.dy2 {
        let mut x_fix = w.fix_u;
        y_fix = y_fix.wrapping_add(w.y_step);
        let src_y = (w.vmask & ((y_fix >> 19) as u32)).wrapping_mul(w.src_usize);
        for dx in w.dx1..w.dx2 {
            x_fix = x_fix.wrapping_add(w.x_step);
            let idx = src_y.wrapping_add(w.umask & ((x_fix >> 19) as u32)) as usize;
            let s = read_src(src, idx as isize);
            put((dst_base + dx) as usize, s);
        }
        dst_base += w.dst_usize;
    }
}

/// Reads `src[idx]` through a raw pointer with no bounds check, as the C++
/// `SourceBitmap[idx]` does. `idx` is the C++ signed 32-bit index, sign-extended
/// (`sxtw` in the oracle), so a negative index reads below the mip, as C++ does.
/// DrawTile indices are masked into `[0, USize * VSize)` and stay in bounds. PostLoad
/// indices stay in bounds only for power-of-two source sizes; for other sizes this
/// reads the adjacent heap bytes the C++ reads, which is UB in Rust terms.
#[inline]
fn read_src(src: *const u8, idx: isize) -> u8 {
    // SAFETY: this is the C++ address. DrawTile reads are in bounds; PostLoad reads
    // are in bounds only for power-of-two sources (see the doc comment).
    unsafe { src.wrapping_offset(idx).read() }
}

/// `execDrawTile` body: locks `Tex->Get(LocalTime)`, draws from the second
/// `Get` result (as the C++ does), then unlocks the first.
pub fn exec_draw_tile<H: ScrTexHost>(h: &mut H, rect: TileRect, tex: ObjPtr, masked: bool) {
    if tex.is_null() {
        h.log(LOG_DRAW_TILE_MISSING_TEXTURE);
        return;
    }
    let locked = h.tex_get_local(tex);
    h.tex_lock_local(locked);
    let draw_src = h.tex_get_local(tex);
    draw_tile(h, rect, draw_src, masked, false, 0);
    h.tex_unlock(locked);
}

/// `execDrawText` body (`DrawString` with the textured path).
pub fn exec_draw_text<H: ScrTexHost>(h: &mut H, x: f32, y: f32, text: ObjPtr, font: ObjPtr) {
    if font.is_null() {
        h.log(LOG_DRAW_TEXT_NO_FONT);
        return;
    }
    draw_string(h, font, app_round(x), app_round(y), text, None);
}

/// `execDrawColoredText` body. `color` is the `FColor` packed as `int4`.
pub fn exec_draw_colored_text<H: ScrTexHost>(
    h: &mut H,
    x: f32,
    y: f32,
    text: ObjPtr,
    font: ObjPtr,
    color: u32,
) {
    if font.is_null() {
        h.log(LOG_DRAW_TEXT_NO_FONT);
        return;
    }
    let entry = h.self_palette_best_match(color);
    draw_string(h, font, app_round(x), app_round(y), text, Some(entry));
}

/// `execReplaceTexture` body: copies `tex`'s mip 0 into the owning texture when
/// the dimensions match.
pub fn exec_replace_texture<H: ScrTexHost>(h: &mut H, tex: ObjPtr) {
    if tex.is_null() {
        h.log(LOG_REPLACE_MISSING_TEXTURE);
        return;
    }
    if h.self_usize() != h.tex_usize(tex) || h.self_vsize() != h.tex_vsize(tex) {
        h.log(LOG_REPLACE_DIMENSIONS);
        return;
    }
    let locked = h.tex_get_local(tex);
    h.tex_lock_local(locked);
    let src = h.info_mip0();
    let dst = h.self_mip0();
    let n = (h.self_usize().max(0) as usize) * (h.self_vsize().max(0) as usize);
    // SAFETY: `src` is the locked mip of `tex`, whose dimensions equal ours, so it
    // holds `n` bytes; `dst` is our mip of `n` bytes. `copy` (memmove) also covers
    // the self-replace case where they alias.
    unsafe { std::ptr::copy(src.0 as *const u8, dst.0 as *mut u8, n) };
    h.tex_unlock(locked);
}

/// `execTextSize` body. Returns `None` when the font is missing (the C++ logs
/// and leaves `XL`/`YL` untouched).
pub fn exec_text_size<H: ScrTexHost>(h: &mut H, text: ObjPtr, font: ObjPtr) -> Option<(f32, f32)> {
    if font.is_null() {
        h.log(LOG_TEXT_SIZE_NO_FONT);
        return None;
    }
    let mut xli: i32 = 0;
    let mut yli: i32 = 0;
    let mut i: i32 = 0;
    loop {
        let raw = h.text_char(text, i);
        if raw == 0 {
            break;
        }
        let (w, height) = char_size(h, font, raw);
        xli = xli.wrapping_add(w);
        if yli < height {
            yli = height;
        }
        i += 1;
    }
    Some((xli as f32, yli as f32))
}

/// `UScriptedTexture::PostLoad` after `UTexture::PostLoad()`: re-copies the source
/// texture into `LocalSourceBitmap` with the scaler shifts.
pub fn post_load<H: ScrTexHost>(h: &mut H) {
    let src = h.self_source_texture();
    if src.is_null() {
        return;
    }
    let locked = src;
    h.tex_lock_local(locked);
    h.tex_unlock(locked);

    let usize_ = h.self_usize();
    let vsize = h.self_vsize();
    let local = h.self_alloc_local_bitmap(usize_.wrapping_mul(vsize));

    // CalcScaler
    let uscaler = h.self_ubits().wrapping_sub(h.tex_ubits(src));
    let vscaler = h.self_vbits().wrapping_sub(h.tex_vbits(src));

    // CopyPalette
    if src != h.self_old_source() {
        h.self_copy_palette_from(src);
    }
    h.self_set_old_source(src);

    // CopyData
    let src_ubits = h.tex_ubits(src);
    let src_map = h.tex_mip0(src).0 as *const u8;
    let local_ptr = local.0 as *mut u8;
    for v in 0..vsize {
        for u in 0..usize_ {
            let a = u.wrapping_shr(uscaler as u32);
            let b = v
                .wrapping_shr(vscaler as u32)
                .wrapping_shl(src_ubits as u32);
            // `a + b` is a 32-bit wrapping `int` (C++ `sxtw`); `as isize` sign-extends it.
            let idx = a.wrapping_add(b) as isize;
            let s = read_src(src_map, idx);
            let dst_idx = (u.wrapping_add(v.wrapping_mul(usize_))) as usize;
            // SAFETY: `dst_idx < usize * vsize`, the size of `local`.
            unsafe { *local_ptr.add(dst_idx) = s };
        }
    }
}

/// `UScriptedTexture::Tick` after `Super::Tick` and the `check(DestBitmap)`:
/// copies `LocalSourceBitmap` into the texture, then notifies the actor.
pub fn tick<H: ScrTexHost>(h: &mut H) {
    let local = h.self_local_bitmap();
    if !local.is_null() {
        let dst = h.self_mip0();
        let n = (h.self_usize().max(0) as usize) * (h.self_vsize().max(0) as usize);
        // SAFETY: `local` and `dst` are distinct allocations of `n` bytes each.
        unsafe { std::ptr::copy_nonoverlapping(local.0 as *const u8, dst.0 as *mut u8, n) };
    }
    h.self_notify_render();
}
