//! Canonical Engine.ScriptedTexture object layout and Rust-owned native members.

use hp2_abi::engine::generated::{
    self, FFontCharacter, FFontPage, FFrame, FTextureInfo, FMipmap, TMapCharChar,
    TMapCharCharPair, UFont, UObject, UPalette, UScriptedTexture, UTexture,
};
use hp2_abi::FColor;
use hp2_core::{
    canonical_vm_native::log_frame,
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
};
use std::{alloc::Layout, cell::RefCell, ffi::c_void, mem::size_of, ptr};
struct CanonicalHost<'a> {
    access: CoreAccess<'a>,
    object: *mut UScriptedTexture,
    stack: *mut FFrame,
    info: RefCell<FTextureInfo>,
    palettes: *mut crate::scr_tex::PaletteCache,
    failure: RefCell<Option<RuntimeError>>,
    text_len: usize,
}
impl CanonicalHost<'_> {
    fn time(&self) -> i64 {
        let seconds = unsafe { ptr::addr_of!((*self.object).local_time).read_unaligned() };
        hp2_core::time::from_double(f64::from(seconds))
    }
    fn fail(&self, error: RuntimeError) { self.failure.borrow_mut().get_or_insert(error); }
    fn lock(&self, texture: *mut UTexture, now: bool) -> *mut UTexture {
        if texture.is_null() || self.failure.borrow().is_some() { return ptr::null_mut(); }
        let time = if now {
            hp2_core::time::from_double(
                hp2_audio::verification::visual_seconds()
                    .unwrap_or_else(hp2_core::unix_clock::seconds_slow),
            )
        } else { self.time() };
        let mut info = self.info.borrow_mut();
        if let Err(error) = unsafe { crate::texture_owner::lock(self.access, texture, &mut *info, time, 0, ptr::null_mut()) } {
            self.fail(error);
            ptr::null_mut()
        } else { texture }
    }
    fn unlock(&self, texture: *mut UTexture) {
        if !texture.is_null() {
            if let Err(error) = unsafe { crate::texture_owner::unlock(self.access, texture, &mut *self.info.borrow_mut()) } { self.fail(error); }
        }
    }
    fn mip_data(&self, texture: *mut UTexture) -> *mut u8 {
        if texture.is_null() { return ptr::null_mut(); }
        let mips = unsafe { ptr::addr_of!((*texture).mips).read_unaligned() };
        if mips.num <= 0 || mips.data.is_null() { self.fail(RuntimeError::new("Canonical ScriptedTexture requires mip 0")); return ptr::null_mut(); }
        let mip = unsafe { &*mips.data.cast::<FMipmap>() };
        let data = unsafe { ptr::addr_of!((*mip).data_array.data).read_unaligned() };
        if data.is_null() { self.fail(RuntimeError::new("Canonical ScriptedTexture mip 0 has no pixel storage")); }
        data.cast()
    }
    fn palette(texture: *mut UTexture) -> RuntimeResult<*mut UPalette> {
        if texture.is_null() { return Err(RuntimeError::new("Null canonical texture palette source")); }
        let palette = unsafe { ptr::addr_of!((*texture).base.palette).read_unaligned().cast::<UPalette>() };
        if palette.is_null() { Err(RuntimeError::new("Canonical texture has no palette")) } else { Ok(palette) }
    }
    fn colors(palette: *mut UPalette, out: &mut [FColor; 256]) -> RuntimeResult<()> {
        if palette.is_null() { return Err(RuntimeError::new("Null canonical palette")); }
        let colors = unsafe { ptr::addr_of!((*palette).colors).read_unaligned() };
        if colors.num < 256 || colors.data.is_null() { return Err(RuntimeError::new("Canonical palette has fewer than 256 colors")); }
        unsafe { ptr::copy_nonoverlapping(colors.data.cast::<FColor>(), out.as_mut_ptr(), 256); }
        Ok(())
    }
    fn log_code(&self, code: u32) {
        let text: &[u32] = match code {
            crate::scr_tex::LOG_DRAW_TEXT_NO_FONT => &[68,114,97,119,84,101,120,116,58,32,78,111,32,102,111,110,116],
            crate::scr_tex::LOG_REPLACE_MISSING_TEXTURE => &[82,101,112,108,97,99,101,84,101,120,116,117,114,101,58,32,77,105,115,115,105,110,103,32,84,101,120,116,117,114,101],
            crate::scr_tex::LOG_REPLACE_DIMENSIONS => &[82,101,112,108,97,99,101,84,101,120,116,117,114,101,58,32,68,105,109,101,110,115,105,111,110,115,32,97,114,101,32,100,105,102,102,101,114,101,110,116],
            crate::scr_tex::LOG_DRAW_TILE_MISSING_TEXTURE => &[68,114,97,119,84,105,108,101,58,32,77,105,115,115,105,110,103,32,84,101,120,116,117,114,101],
            crate::scr_tex::LOG_TEXT_SIZE_NO_FONT => &[84,101,120,116,83,105,122,101,58,32,78,111,32,102,111,110,116],
            _ => { self.fail(RuntimeError::new("Unknown ScriptedTexture log operation")); return; }
        };
        if let Err(error) = unsafe { log_frame(self.access, self.stack, text) } { self.fail(error); }
    }
    fn result(&self) -> RuntimeResult<()> { self.failure.borrow_mut().take().map_or(Ok(()), Err) }
    fn page(&self, font: crate::scr_tex::ObjPtr, page: i32) -> *mut FFontPage {
        if font.is_null() { return ptr::null_mut(); }
        let pages = unsafe { ptr::addr_of!((*font.0.cast::<UFont>()).pages).read_unaligned() };
        if page < 0 || page >= pages.num || pages.data.is_null() { ptr::null_mut() } else { unsafe { pages.data.cast::<FFontPage>().add(page as usize) } }
    }
}

impl crate::scr_tex::ScrTexHost for CanonicalHost<'_> {
    fn font_chars_per_page(&self, font: crate::scr_tex::ObjPtr) -> i32 {
        if font.is_null() { 0 } else { unsafe { ptr::addr_of!((*font.0.cast::<UFont>()).characters_per_page).read_unaligned() } }
    }
    fn font_remap_char(&self, font: crate::scr_tex::ObjPtr, ch: i32) -> i32 {
        if font.is_null() { return ch; }
        let font = font.0.cast::<UFont>();
        if unsafe { ptr::addr_of!((*font).is_remapped).read_unaligned() == 0 } { return ch; }
        let map = unsafe { ptr::addr_of!((*font).char_remap).cast::<TMapCharChar>() };
        let pairs_raw = unsafe { ptr::addr_of!((*map).pairs).read_unaligned() };
        let count = unsafe { ptr::addr_of!((*map).hash_count).read_unaligned() };
        let hash = unsafe { ptr::addr_of!((*map).hash).read_unaligned().cast::<i32>() };
        if pairs_raw.num <= 0 || pairs_raw.data.is_null() || count <= 0 || hash.is_null() { return 32; }
        let mut index = unsafe { hash.add((ch & (count - 1)) as usize).read_unaligned() };
        let pairs = pairs_raw.data.cast::<TMapCharCharPair>();
        while index >= 0 && index < pairs_raw.num {
            let pair = unsafe { pairs.add(index as usize) };
            if unsafe { ptr::addr_of!((*pair).key).read_unaligned() == ch as u32 } {
                return unsafe { ptr::addr_of!((*pair).value).read_unaligned() as i32 };
            }
            index = unsafe { ptr::addr_of!((*pair).hash_next).read_unaligned() };
        }
        32
    }
    fn font_page_count(&self, font: crate::scr_tex::ObjPtr) -> i32 { if font.is_null() { 0 } else { unsafe { ptr::addr_of!((*font.0.cast::<UFont>()).pages).read_unaligned().num } } }
    fn font_page_texture(&self, font: crate::scr_tex::ObjPtr, page: i32) -> crate::scr_tex::ObjPtr {
        let page = self.page(font, page);
        crate::scr_tex::ObjPtr(if page.is_null() { ptr::null_mut() } else { unsafe { ptr::addr_of!((*page).texture).read_unaligned() } })
    }
    fn font_page_char_count(&self, font: crate::scr_tex::ObjPtr, page: i32) -> i32 {
        let page = self.page(font, page);
        if page.is_null() { 0 } else { unsafe { ptr::addr_of!((*page).characters).read_unaligned().num } }
    }
    fn font_page_char(&self, font: crate::scr_tex::ObjPtr, page: i32, index: i32) -> crate::canvas_text::Hp2FontChar {
        let page = self.page(font, page);
        if page.is_null() { return glyph_zero(); }
        let chars = unsafe { ptr::addr_of!((*page).characters).read_unaligned() };
        if index < 0 || index >= chars.num || chars.data.is_null() { return glyph_zero(); }
        let glyph = unsafe { &*chars.data.cast::<FFontCharacter>().add(index as usize) };
        crate::canvas_text::Hp2FontChar { start_u: glyph.start_u, start_v: glyph.start_v, u_size: glyph.u_size, v_size: glyph.v_size }
    }
    fn text_char(&self, text: crate::scr_tex::ObjPtr, index: i32) -> i32 {
        if text.is_null() || index < 0 || index as usize >= self.text_len { 0 } else { unsafe { text.0.cast::<i32>().add(index as usize).read_unaligned() } }
    }
    fn tex_usize(&self, texture: crate::scr_tex::ObjPtr) -> i32 { unsafe { ptr::addr_of!((*texture.0.cast::<UTexture>()).base.u_size).read_unaligned() } }
    fn tex_vsize(&self, texture: crate::scr_tex::ObjPtr) -> i32 { unsafe { ptr::addr_of!((*texture.0.cast::<UTexture>()).base.v_size).read_unaligned() } }
    fn tex_get_local(&self, texture: crate::scr_tex::ObjPtr) -> crate::scr_tex::ObjPtr {
        if texture.is_null() { return texture; }
        match unsafe { crate::texture_owner::get(self.access, texture.0.cast(), self.time()) } {
            Ok(value) => crate::scr_tex::ObjPtr(value.cast()),
            Err(error) => { self.fail(error); crate::scr_tex::ObjPtr(ptr::null_mut()) }
        }
    }
    fn tex_lock_local(&mut self, texture: crate::scr_tex::ObjPtr) { self.lock(texture.0.cast(), false); }
    fn tex_lock_now(&mut self, texture: crate::scr_tex::ObjPtr) { self.lock(texture.0.cast(), true); }
    fn tex_unlock(&mut self, texture: crate::scr_tex::ObjPtr) { self.unlock(texture.0.cast()); }
    fn info_mip0(&self) -> crate::scr_tex::ObjPtr {
        let info = self.info.borrow(); let mip = info.mips[0];
        if mip.is_null() { crate::scr_tex::ObjPtr(ptr::null_mut()) } else {
            crate::scr_tex::ObjPtr(unsafe { ptr::addr_of!((*mip.cast::<FMipmap>()).base.data_ptr).read_unaligned().cast() })
        }
    }
    fn self_usize(&self) -> i32 { unsafe { ptr::addr_of!((*self.object).base.base.u_size).read_unaligned() } }
    fn self_vsize(&self) -> i32 { unsafe { ptr::addr_of!((*self.object).base.base.v_size).read_unaligned() } }
    fn self_mip0(&self) -> crate::scr_tex::ObjPtr { crate::scr_tex::ObjPtr(unsafe { self.mip_data(ptr::addr_of_mut!((*self.object).base)).cast() }) }
    fn tex_palette_colors(&self, texture: crate::scr_tex::ObjPtr, out: &mut [FColor; 256]) {
        if let Err(error) = Self::palette(texture.0.cast()).and_then(|palette| Self::colors(palette, out)) { self.fail(error); }
    }
    fn self_palette_colors(&self, out: &mut [FColor; 256]) {
        let palette = unsafe { ptr::addr_of!((*self.object).base.base.palette).read_unaligned().cast::<UPalette>() };
        if let Err(error) = Self::colors(palette, out) { self.fail(error); }
    }
    fn self_palette_best_match(&self, color: u32) -> u8 {
        let mut own = [FColor { r: 0, g: 0, b: 0, a: 0 }; 256];
        if let Err(error) = Self::colors(unsafe { ptr::addr_of!((*self.object).base.base.palette).read_unaligned().cast() }, &mut own) { self.fail(error); return 0; }
        crate::tex::best_match(&own, FColor { r: color.to_ne_bytes()[0], g: color.to_ne_bytes()[1], b: color.to_ne_bytes()[2], a: color.to_ne_bytes()[3] }, 0)
    }
    fn palette_map_lookup(&self, map: crate::scr_tex::ObjPtr, index: u8) -> u8 { unsafe { *map.0.cast::<u8>().add(index as usize) } }
    fn palette_cache(&mut self) -> &mut crate::scr_tex::PaletteCache { unsafe { &mut *self.palettes } }
    fn log(&mut self, code: u32) { self.log_code(code); }
    fn tex_ubits(&self, texture: crate::scr_tex::ObjPtr) -> i32 { unsafe { ptr::addr_of!((*texture.0.cast::<UTexture>()).base.u_bits).read_unaligned() as i32 } }
    fn tex_vbits(&self, texture: crate::scr_tex::ObjPtr) -> i32 { unsafe { ptr::addr_of!((*texture.0.cast::<UTexture>()).base.v_bits).read_unaligned() as i32 } }
    fn tex_mip0(&self, texture: crate::scr_tex::ObjPtr) -> crate::scr_tex::ObjPtr { crate::scr_tex::ObjPtr(self.mip_data(texture.0.cast()).cast()) }
    fn self_ubits(&self) -> i32 { unsafe { ptr::addr_of!((*self.object).base.base.u_bits).read_unaligned() as i32 } }
    fn self_vbits(&self) -> i32 { unsafe { ptr::addr_of!((*self.object).base.base.v_bits).read_unaligned() as i32 } }
    fn self_local_bitmap(&self) -> crate::scr_tex::ObjPtr {
        crate::scr_tex::ObjPtr(unsafe { ptr::addr_of!((*self.object).local_source_bitmap).read_unaligned() })
    }
    fn self_alloc_local_bitmap(&mut self, bytes: i32) -> crate::scr_tex::ObjPtr {
        match unsafe { replace_local_bitmap(self.object, bytes as usize) } {
            Ok(bitmap) => crate::scr_tex::ObjPtr(bitmap.cast()),
            Err(error) => { self.fail(error); crate::scr_tex::ObjPtr(ptr::null_mut()) }
        }
    }
    fn self_source_texture(&self) -> crate::scr_tex::ObjPtr {
        crate::scr_tex::ObjPtr(unsafe { ptr::addr_of!((*self.object).source_texture).read_unaligned() })
    }
    fn self_old_source(&self) -> crate::scr_tex::ObjPtr {
        crate::scr_tex::ObjPtr(unsafe { ptr::addr_of!((*self.object).old_source_tex).read_unaligned() })
    }
    fn self_set_old_source(&mut self, texture: crate::scr_tex::ObjPtr) {
        unsafe { ptr::addr_of_mut!((*self.object).old_source_tex).write_unaligned(texture.0); }
    }
    fn self_copy_palette_from(&mut self, texture: crate::scr_tex::ObjPtr) {
        if texture.is_null() { self.fail(RuntimeError::new("Null ScriptedTexture palette source")); return; }
        let palette = unsafe { ptr::addr_of!((*texture.0.cast::<UTexture>()).base.palette).read_unaligned() };
        unsafe { ptr::addr_of_mut!((*self.object).base.base.palette).write_unaligned(palette); }
    }
    fn self_notify_render(&mut self) {
        let actor = unsafe { ptr::addr_of!((*self.object).notify_actor).read_unaligned() };
        if actor.is_null() { return; }
        let mut texture: *mut UObject = self.object.cast();
        if let Err(error) = unsafe {
            crate::actor_tick_owner::execute_event_params(
                self.access, actor, "RenderTexture", (&mut texture as *mut *mut UObject).cast(),
                std::mem::size_of::<*mut UObject>(),
            )
        } { self.fail(error); }
    }
}

pub(crate) unsafe fn post_load(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::texture_owner::TEXTURE.post_load)(access, object)?; }
    let mut host = host(access, object, ptr::null_mut(), 0)?;
    crate::scr_tex::post_load(&mut host);
    host.result()
}

pub(crate) unsafe fn begin_tick(object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical ScriptedTexture")); }
    let scripted = object.cast::<UScriptedTexture>();
    unsafe {
        ptr::addr_of_mut!((*scripted).local_time).write_unaligned(
            hp2_audio::verification::visual_seconds()
                .map(|seconds| hp2_core::time::get_float(hp2_core::time::from_double(seconds)))
                .unwrap_or_else(|| hp2_core::unix_clock::seconds_slow() as f32),
        );
    }
    Ok(())
}

pub(crate) unsafe fn tick(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let mut host = host(access, object, ptr::null_mut(), 0)?;
    crate::scr_tex::tick(&mut host);
    host.result()
}
fn glyph_zero() -> crate::canvas_text::Hp2FontChar { crate::canvas_text::Hp2FontChar { start_u: 0, start_v: 0, u_size: 0, v_size: 0 } }
fn host<'a>(access: CoreAccess<'a>, object: *mut UObject, frame: *mut FFrame, text_len: usize) -> RuntimeResult<CanonicalHost<'a>> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical ScriptedTexture")); }
    let object = object.cast::<UScriptedTexture>();
    let palettes = unsafe { ptr::addr_of!((*object).palette_map).read_unaligned().cast::<crate::scr_tex::PaletteCache>() };
    if palettes.is_null() { return Err(RuntimeError::new("Canonical ScriptedTexture has no PaletteCache")); }
    Ok(CanonicalHost { access, object, stack: frame, info: RefCell::new(unsafe { std::mem::zeroed() }), palettes, failure: RefCell::new(None), text_len })
}
pub(crate) unsafe fn draw_text(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame,
    x: f32, y: f32, text: *mut c_void, text_len: usize, font: *mut UFont,
) -> RuntimeResult<()> {
    let mut host = host(access, object, frame, text_len)?;
    crate::scr_tex::exec_draw_text(&mut host, x, y, crate::scr_tex::ObjPtr(text), crate::scr_tex::ObjPtr(font.cast()));
    host.result()
}

pub(crate) unsafe fn draw_colored_text(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame,
    x: f32, y: f32, text: *mut c_void, text_len: usize, font: *mut UFont, color: u32,
) -> RuntimeResult<()> {
    let mut host = host(access, object, frame, text_len)?;
    crate::scr_tex::exec_draw_colored_text(&mut host, x, y, crate::scr_tex::ObjPtr(text), crate::scr_tex::ObjPtr(font.cast()), color);
    host.result()
}

pub(crate) unsafe fn draw_tile(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame,
    rect: crate::scr_tex::TileRect, texture: *mut UTexture, masked: bool,
) -> RuntimeResult<()> {
    let mut host = host(access, object, frame, 0)?;
    crate::scr_tex::exec_draw_tile(&mut host, rect, crate::scr_tex::ObjPtr(texture.cast()), masked);
    host.result()
}

pub(crate) unsafe fn replace_texture(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, texture: *mut UTexture,
) -> RuntimeResult<()> {
    let mut host = host(access, object, frame, 0)?;
    crate::scr_tex::exec_replace_texture(&mut host, crate::scr_tex::ObjPtr(texture.cast()));
    host.result()
}

pub(crate) unsafe fn text_size(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame,
    text: *mut c_void, text_len: usize, font: *mut UFont,
) -> RuntimeResult<Option<(f32, f32)>> {
    let mut host = host(access, object, frame, text_len)?;
    let size = crate::scr_tex::exec_text_size(&mut host, crate::scr_tex::ObjPtr(text), crate::scr_tex::ObjPtr(font.cast()));
    host.result().map(|()| size)
}

const SCRIPTED_TEXTURE: ClassOps = {
    let mut operations = crate::texture_owner::TEXTURE;
    operations.construct = construct;
    operations.destroy = destroy;
    operations.post_load = post_load;
    operations.post_edit_change = post_load;
    operations.visit_references = visit_references;
    operations
};

const SCRIPTED_TEXTURE_PROPERTIES: &[StartupProperty] = &[
    StartupProperty {
        name: "NotifyActor", kind: StartupPropertyKind::Object { class: "Engine.Actor" },
        category: "", property_flags: 0, object_flags: 4, inner_name: None,
        offset: std::mem::offset_of!(UScriptedTexture, notify_actor), array_dim: 1,
    },
    StartupProperty {
        name: "SourceTexture", kind: StartupPropertyKind::Object { class: "Engine.Texture" },
        category: "", property_flags: 1, object_flags: 4, inner_name: None,
        offset: std::mem::offset_of!(UScriptedTexture, source_texture), array_dim: 1,
    },
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "ScriptedTexture",
    package: "Engine",
    super_name: Some("Texture"),
    within: "Object",
    flags: generated::TEXTURE_CLASS_FLAGS,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(match Layout::from_size_align(size_of::<UScriptedTexture>(), UScriptedTexture::NATIVE_ALIGNMENT) {
        Ok(layout) => layout,
        Err(_) => panic!("compiler-probed ScriptedTexture layout is invalid"),
    }),
    operations: Some(SCRIPTED_TEXTURE),
    properties: SCRIPTED_TEXTURE_PROPERTIES,
    defaults: &[],
}];

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::texture_owner::TEXTURE.construct)(access, object)?; }
    let scripted = object.cast::<UScriptedTexture>();
    unsafe {
        ptr::addr_of_mut!((*scripted).notify_actor).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*scripted).source_texture).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*scripted).old_source_tex).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*scripted).local_source_bitmap).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*scripted).palette_map).write_unaligned(crate::scr_tex::palette_cache_new().cast::<c_void>());
        ptr::addr_of_mut!((*scripted).local_time).write_unaligned(0.0);
        (*scripted).base.set_b_parametric(true);
        (*scripted).base.set_b_realtime(true);
    }
    Ok(())
}

pub(crate) unsafe fn replace_local_bitmap(
    scripted: *mut UScriptedTexture,
    bytes: usize,
) -> RuntimeResult<*mut u8> {
    let field = unsafe { ptr::addr_of_mut!((*scripted).local_source_bitmap) };
    let old = unsafe { field.read_unaligned().cast::<u8>() };
    unsafe { field.write_unaligned(ptr::null_mut()); }
    if !old.is_null() { unsafe { libc::free(old.cast()); } }
    let bitmap = unsafe { alloc_local_bitmap(bytes)? };
    unsafe { field.write_unaligned(bitmap.cast::<c_void>()); }
    Ok(bitmap)
}

unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let scripted = object.cast::<UScriptedTexture>();
    let bitmap = unsafe { ptr::addr_of!((*scripted).local_source_bitmap).read_unaligned().cast::<u8>() };
    unsafe { ptr::addr_of_mut!((*scripted).local_source_bitmap).write_unaligned(ptr::null_mut()); }
    if !bitmap.is_null() { unsafe { libc::free(bitmap.cast()); } }
    let cache = unsafe { ptr::addr_of!((*scripted).palette_map).read_unaligned().cast::<crate::scr_tex::PaletteCache>() };
    unsafe { ptr::addr_of_mut!((*scripted).palette_map).write_unaligned(ptr::null_mut()); }
    unsafe { crate::scr_tex::palette_cache_free(cache); }
    unsafe { (crate::texture_owner::TEXTURE.destroy)(access, object) }
}

unsafe fn visit_references(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { (crate::texture_owner::TEXTURE.visit_references)(access, object, archive)?; }
    let scripted = object.cast::<UScriptedTexture>();
    for slot in [
        unsafe { ptr::addr_of_mut!((*scripted).notify_actor).cast::<*mut UObject>() },
        unsafe { ptr::addr_of_mut!((*scripted).source_texture).cast::<*mut UObject>() },
        unsafe { ptr::addr_of_mut!((*scripted).old_source_tex).cast::<*mut UObject>() },
    ] {
        unsafe { archive.serialize_object_slot(slot)?; }
    }
    Ok(())
}

/// Allocates the canonical byte buffer for `LocalSourceBitmap`.
pub unsafe fn alloc_local_bitmap(bytes: usize) -> RuntimeResult<*mut u8> {
    Ok(unsafe { hp2_core::unix_alloc::system_malloc(bytes) }.cast())
}
