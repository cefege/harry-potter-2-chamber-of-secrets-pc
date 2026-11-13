//! Canonical UCanvas metadata and direct Core operations.
use hp2_abi::{engine::generated::{AActor, APlayerPawn, FCanvasTextLayout, FCanvasTextRequest, FColor, FSceneNode, FTextureInfo, UFont, URender, URenderDevice, UCanvas, UObject, UTexture, UViewport}, FPlane, FVector};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind, StartupScalarKind, StartupStructField},
};
use crate::canvas_text::{self, CanvasTextHost, Font, Hp2CanvasView, Hp2FontChar, Hp2NativeTextRequest, Hp2Plane, Hp2CanvasSize, Texture};
use hp2_abi::engine::generated::{FFontCharacter, FFontPage, TMapCharChar, TMapCharCharPair, TArrayRaw};
use std::{alloc::Layout, ffi::c_void, ptr};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const fn field(name: &'static str, offset: usize) -> StartupStructField {
    StartupStructField { name, kind: StartupScalarKind::Byte, offset }
}
const COLOR_FIELDS: &[StartupStructField] = &[
    field("R", std::mem::offset_of!(FColor, r)),
    field("G", std::mem::offset_of!(FColor, g)),
    field("B", std::mem::offset_of!(FColor, b)),
    field("A", std::mem::offset_of!(FColor, a)),
];
const fn prop(name: &'static str, kind: StartupPropertyKind, offset: usize) -> StartupProperty {
    StartupProperty { name, kind, category: "Canvas", property_flags: 0, object_flags: 4, inner_name: None, offset, array_dim: 1 }
}
static PROPERTIES: &[StartupProperty] = &[
    prop("Font", StartupPropertyKind::Object { class: "Engine.Font" }, std::mem::offset_of!(UCanvas, font)),
    prop("SpaceX", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, space_x)),
    prop("SpaceY", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, space_y)),
    prop("OrgX", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, org_x)),
    prop("OrgY", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, org_y)),
    prop("ClipX", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, clip_x)),
    prop("ClipY", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, clip_y)),
    prop("CurX", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, cur_x)),
    prop("CurY", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, cur_y)),
    prop("Z", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, z)),
    prop("Style", StartupPropertyKind::Byte, std::mem::offset_of!(UCanvas, style)),
    prop("CurYL", StartupPropertyKind::Float, std::mem::offset_of!(UCanvas, cur_yl)),
    prop("DrawColor", StartupPropertyKind::Struct { name: "Color", fields: COLOR_FIELDS }, std::mem::offset_of!(UCanvas, color)),
    prop("bCenter", StartupPropertyKind::Bool { bit_mask: 1 }, std::mem::offset_of!(UCanvas, color) + 4),
    prop("bNoSmooth", StartupPropertyKind::Bool { bit_mask: 2 }, std::mem::offset_of!(UCanvas, color) + 4),
    prop("SizeX", StartupPropertyKind::Int, std::mem::offset_of!(UCanvas, x)),
    prop("SizeY", StartupPropertyKind::Int, std::mem::offset_of!(UCanvas, y)),
    prop("SmallFont", StartupPropertyKind::Object { class: "Engine.Font" }, std::mem::offset_of!(UCanvas, small_font)),
    prop("MedFont", StartupPropertyKind::Object { class: "Engine.Font" }, std::mem::offset_of!(UCanvas, med_font)),
    prop("BigFont", StartupPropertyKind::Object { class: "Engine.Font" }, std::mem::offset_of!(UCanvas, big_font)),
    prop("LargeFont", StartupPropertyKind::Object { class: "Engine.Font" }, std::mem::offset_of!(UCanvas, large_font)),
    prop("Viewport", StartupPropertyKind::Object { class: "Engine.Viewport" }, std::mem::offset_of!(UCanvas, viewport)),
    prop("FramePtr", StartupPropertyKind::Int, std::mem::offset_of!(UCanvas, frame)),
    prop("RenderPtr", StartupPropertyKind::Int, std::mem::offset_of!(UCanvas, render)),
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Canvas", package: "Engine", super_name: Some("Object"), within: "Object",
    flags: 8, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(match Layout::from_size_align(std::mem::size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) {
        Ok(layout) => layout,
        Err(_) => panic!("Canvas native alignment is invalid"),
    }),
    operations: None,
    properties: PROPERTIES, defaults: &[],
}];

/// Draws a live actor with Canvas's exact temporary visibility and render-map effects.
///
/// # Safety
/// Inputs and the Canvas's scene, viewport, renderer and view actor must be live
/// canonical records. Failures intentionally retain the native partial effects.
pub(crate) unsafe fn draw_actor(
    access: CoreAccess<'_>, canvas: *mut UCanvas, actor: *mut AActor,
    wireframe: bool, clear_z: bool,
) -> RuntimeResult<()> {
    unsafe {
        if canvas.is_null() || actor.is_null() {
            return Err(RuntimeError::new("Null canonical Canvas DrawActor input"));
        }
        let render = ptr::addr_of!((*canvas).render).read_unaligned().cast::<URender>();
        let scene = ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>();
        let viewport = ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>();
        let view_actor = if viewport.is_null() { ptr::null_mut() } else {
            ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>()
        };
        if render.is_null() || scene.is_null() || viewport.is_null() || view_actor.is_null() {
            return Err(RuntimeError::new("Invalid canonical Canvas DrawActor input"));
        }
        let old_render_map = ptr::addr_of!((*view_actor).rend_map).read_unaligned();
        if wireframe {
            ptr::addr_of_mut!((*view_actor).rend_map).write_unaligned(hp2_abi::engine::generated::ENGINE_REN_WIRE);
        }
        (*actor).set_b_hidden(false);
        if clear_z {
            let device = ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast();
            crate::render_device_owner::clear_z(access, device)?;
        }
        crate::render_owner::draw_actor(access, render, scene, actor)?;
        (*actor).set_b_hidden(true);
        ptr::addr_of_mut!((*view_actor).rend_map).write_unaligned(old_render_map);
        Ok(())
    }
}

/// Runs Script Reset and copies the active scene extents into canonical Canvas fields.
///
/// # Safety
/// `canvas` and `frame` must be live canonical objects for the duration of event dispatch.
pub(crate) unsafe fn update(access: CoreAccess<'_>, canvas: *mut UCanvas, frame: *mut FSceneNode) -> RuntimeResult<()> {
    if canvas.is_null() || frame.is_null() { return Err(RuntimeError::new("Null canonical Canvas Update input")); }
    unsafe { access.execute_named_event(canvas.cast::<UObject>(), "Reset")?; }
    unsafe {
        (*canvas).frame = frame.cast();
        (*canvas).clip_x = (*frame).x as f32;
        (*canvas).x = (*canvas).clip_x as i32;
        (*canvas).clip_y = (*frame).y as f32;
        (*canvas).y = (*canvas).clip_y as i32;
    }
    Ok(())
}

/// Assigns a canonical render base during the active draw interval.
/// # Safety
/// `canvas` must be a live canonical Canvas.
pub(crate) unsafe fn set_render(canvas: *mut UCanvas, render: *mut std::ffi::c_void) -> RuntimeResult<()> {
    if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    unsafe { ptr::addr_of_mut!((*canvas).render).write_unaligned(render); }
    Ok(())
}

/// Draws a clipped texture tile through the typed render-device and texture owners.
///
/// # Safety
/// Canvas, texture, and frame must be live canonical objects in the active draw graph.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_tile(
    access: CoreAccess<'_>, canvas: *mut UCanvas, texture: *mut UTexture,
    x: f32, y: f32, xl: f32, yl: f32, u: f32, v: f32, ul: f32, vl: f32,
    span: *mut c_void, z: f32, color: FPlane, fog: FPlane, poly_flags: u32,
) -> RuntimeResult<()> {
    if canvas.is_null() || texture.is_null() { return Err(RuntimeError::new("Invalid canonical Canvas DrawTile input")); }
    let (frame, viewport) = unsafe {
        (ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>(),
         ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>())
    };
    if frame.is_null() || viewport.is_null() { return Err(RuntimeError::new("Canonical Canvas has no active frame or viewport")); }
    let mut rect = crate::canvas::Hp2CanvasTileRect { x, y, xl, yl, u, v, ul, vl };
    if !crate::canvas::clip_tile(unsafe { (*frame).fx }, unsafe { (*frame).fy }, &mut rect) { return Ok(()); }
    let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
    if device.is_null() { return Err(RuntimeError::new("Canonical Canvas viewport has no render device")); }
    let time = i64::from_ne_bytes(unsafe { ptr::addr_of!((*viewport).current_time).read_unaligned() });
    let mut info: FTextureInfo = unsafe { std::mem::zeroed() };
    unsafe { crate::texture_owner::lock(access, texture, &mut info, time, -1, device)?; }
    let (base_u, base_v) = unsafe {
        (ptr::addr_of!((*texture).base.u_size).read_unaligned(),
         ptr::addr_of!((*texture).base.v_size).read_unaligned())
    };
    let u_factor = crate::canvas::uv_factor(info.u_scale, info.u_size, base_u);
    let v_factor = crate::canvas::uv_factor(info.v_scale, info.v_size, base_v);
    let draw = unsafe {
        crate::render_device_owner::draw_tile(
            access, device, frame, &mut info, rect.x, rect.y, rect.xl, rect.yl,
            rect.u * u_factor, rect.v * v_factor, rect.ul * u_factor, rect.vl * v_factor,
            span, z, color, fog, poly_flags | (ptr::addr_of!((*texture).poly_flags).read_unaligned() & 2),
        )
    };
    let unlock = unsafe { crate::texture_owner::unlock(access, texture, &mut info) };
    draw.and(unlock)
}

/// Submits one canonical Canvas 2D line through the owned render device.
///
/// # Safety
/// Canvas and endpoints must belong to the active canonical draw graph.
pub(crate) unsafe fn draw_2d_line(
    access: CoreAccess<'_>, canvas: *mut UCanvas, color: FPlane,
    line_flags: u32, start: FVector, end: FVector,
) -> RuntimeResult<()> {
    if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let (frame, viewport) = unsafe {
        (ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>(),
         ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>())
    };
    if frame.is_null() || viewport.is_null() {
        return Err(RuntimeError::new("Canonical Canvas line has no active render graph"));
    }
    let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
    if device.is_null() { return Err(RuntimeError::new("Canonical Canvas viewport has no render device")); }
    unsafe { crate::render_device_owner::draw_2d_line(access, device, frame, color, line_flags, start, end) }
}

/// Submits one canonical Canvas 2D point through the owned render device.
///
/// # Safety
/// Canvas must belong to the active canonical draw graph.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_2d_point(
    access: CoreAccess<'_>, canvas: *mut UCanvas, color: FPlane, line_flags: u32,
    x1: f32, y1: f32, x2: f32, y2: f32, z: f32,
) -> RuntimeResult<()> {
    if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let (frame, viewport) = unsafe {
        (ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>(),
         ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>())
    };
    if frame.is_null() || viewport.is_null() { return Err(RuntimeError::new("Canonical Canvas has no active frame or viewport")); }
    let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };

    if device.is_null() { return Err(RuntimeError::new("Canonical Canvas viewport has no render device")); }
    unsafe { crate::render_device_owner::draw_2d_point(access, device, frame, color, line_flags, x1, y1, x2, y2, z) }
}
/// Creates, measures, optionally draws, and always destroys a typed canonical
/// text layout. `request` is built by the shared Canvas text policy.
///
/// # Safety
/// Canvas and request must be valid; draw requires an active canonical frame.
pub(crate) unsafe fn draw_text_layout(
    access: CoreAccess<'_>, canvas: *mut UCanvas, request: *const FCanvasTextRequest,
    draw: bool,
) -> RuntimeResult<Option<(i32, i32)>> {
    if canvas.is_null() || request.is_null() {
        return Err(RuntimeError::new("Invalid canonical Canvas text request"));
    }
    let (viewport, frame) = unsafe {
        (ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>(),
         ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>())
    };
    if viewport.is_null() { return Err(RuntimeError::new("Canonical Canvas has no viewport")); }
    let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
    if device.is_null() { return Err(RuntimeError::new("Canonical Canvas viewport has no render device")); }
    let mut layout: *mut FCanvasTextLayout = ptr::null_mut();
    if !unsafe { crate::render_device_owner::create_canvas_text_layout(access, device, request, &mut layout)? } {
        return Ok(None);
    }
    if layout.is_null() {
        return Err(RuntimeError::new("Canonical text provider created no layout"));
    }
    let mut width = 0;
    let mut height = 0;
    let result = (|| {
        if !unsafe { crate::render_device_owner::measure_canvas_text(access, device, layout, &mut width, &mut height)? } {
            return Ok(None);
        }
        if draw {
            if frame.is_null() { return Err(RuntimeError::new("Canonical Canvas has no active frame")); }
            if !unsafe { crate::render_device_owner::draw_canvas_text(access, device, frame, layout)? } {
                return Ok(None);
            }
        }
        Ok(Some((width, height)))
    })();
    let destroyed = unsafe { crate::render_device_owner::destroy_canvas_text_layout(access, device, layout) };
    match (result, destroyed) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Attempts the renderer's native-text provider path for a shared-policy
/// request. `None` preserves the shared policy's bitmap/font-page fallback.
///
/// # Safety
/// Canvas, view, request, and text must be valid for the duration of the call.
pub(crate) unsafe fn try_native_text(
    access: CoreAccess<'_>, canvas: *mut UCanvas,
    view: &crate::canvas_text::Hp2CanvasView,
    request: &crate::canvas_text::Hp2NativeTextRequest,
    font: *mut UFont, text: &[u16], color: crate::canvas_text::Hp2Plane,
) -> RuntimeResult<Option<(i32, i32)>> {
    if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let (viewport, frame) = unsafe {
        (ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>(),
         ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>())
    };
    if viewport.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical Canvas text has no active viewport/frame"));
    }
    let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
    if device.is_null() { return Err(RuntimeError::new("Canonical Canvas viewport has no render device")); }
    unsafe {
        crate::render_device_owner::try_native_canvas_text(access, device, frame, view, request, font, text, color)
    }
}

struct CanonicalTextHost<'a> {
    access: CoreAccess<'a>,
    canvas: *mut UCanvas,
    slots: [*mut UTexture; canvas_text::TEXT_SLOTS],
    infos: [FTextureInfo; canvas_text::TEXT_SLOTS],
    error: Option<RuntimeError>,
}

impl CanonicalTextHost<'_> {
    fn fail(&mut self, error: RuntimeError) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }

    fn take_error(&mut self) -> RuntimeResult<()> {
        self.error.take().map_or(Ok(()), Err)
    }

    fn font(font: Font) -> *mut UFont {
        font.0.cast()
    }

    unsafe fn array_item<T: Copy>(array: TArrayRaw, index: i32) -> Option<T> {
        if index < 0 || index >= array.num || array.data.is_null() {
            return None;
        }
        Some(unsafe { array.data.cast::<T>().add(index as usize).read_unaligned() })
    }
}

impl CanvasTextHost for CanonicalTextHost<'_> {
    fn ui_scale(&mut self) -> f32 {
        let viewport = unsafe { ptr::addr_of!((*self.canvas).viewport).read_unaligned().cast::<UViewport>() };
        if viewport.is_null() { return 1.0; }
        let client = unsafe { ptr::addr_of!((*viewport).outer).read_unaligned().cast::<hp2_abi::engine::generated::USDLClient>() };
        if client.is_null() { 1.0 } else { unsafe { ptr::addr_of!((*client).ui_scale).read_unaligned() } }
    }

    fn language_is_int(&mut self) -> bool {
        match self.access.language_text() {
            Ok(language) => language.iter().take_while(|&&unit| unit != 0)
                .map(|unit| char::from_u32(*unit).unwrap_or('\0').to_ascii_lowercase())
                .eq("int".chars()),
            Err(error) => { self.fail(error); true }
        }
    }

    // Source UViewport/USDLViewport::DrawString is intentionally empty; the
    // RenderDevice provider was attempted before this stock fallback.
    fn viewport_draw_string(&mut self, _cv: &Hp2CanvasView, _flags: u32, _font: Font, _x: &mut i32, _y: &mut i32, _text: &[u16], _color: Hp2Plane) {}

    fn try_native_text(&mut self, cv: &Hp2CanvasView, req: &Hp2NativeTextRequest, font: Font, text: &[u16], color: Hp2Plane) -> Option<(i32, i32)> {
        match unsafe { try_native_text(self.access, self.canvas, cv, req, Self::font(font), text, color) } {
            Ok(value) => value,
            Err(error) => { self.fail(error); None }
        }
    }

    fn font_chars_per_page(&mut self, font: Font) -> i32 {
        if font.is_null() { return 0; }
        unsafe { ptr::addr_of!((*Self::font(font)).characters_per_page).read_unaligned() }
    }

    fn font_remap_char(&mut self, font: Font, ch: u16) -> u16 {
        if font.is_null() || unsafe { ptr::addr_of!((*Self::font(font)).is_remapped).read_unaligned() == 0 } {
            return ch;
        }
        let map = unsafe { ptr::addr_of!((*Self::font(font)).char_remap).cast::<TMapCharChar>() };
        let count = unsafe { ptr::addr_of!((*map).pairs).read_unaligned().num };
        let pairs = unsafe { ptr::addr_of!((*map).pairs).read_unaligned().data.cast::<TMapCharCharPair>() };
        if count <= 0 || pairs.is_null() { return u16::from(b' '); }
        let hash_count = unsafe { ptr::addr_of!((*map).hash_count).read_unaligned() };
        let hash = unsafe { ptr::addr_of!((*map).hash).read_unaligned().cast::<i32>() };
        if hash_count <= 0 || hash.is_null() { return u16::from(b' '); }
        let bucket = (i32::from(ch) & (hash_count - 1)) as usize;
        let index = unsafe { hash.add(bucket).read_unaligned() };
        match hp2_core::map_storage::find_index_with(
            index,
            |i| Ok::<bool, ()>(i >= 0 && i < count && unsafe { (*pairs.add(i as usize)).key == u32::from(ch) }),
            |i| Ok::<i32, ()>(unsafe { (*pairs.add(i as usize)).hash_next }),
        ) {
            Ok(i) if i >= 0 && i < count => unsafe { (*pairs.add(i as usize)).value as u16 },
            _ => u16::from(b' '),
        }
    }

    fn font_page_count(&mut self, font: Font) -> i32 {
        if font.is_null() { 0 } else { unsafe { ptr::addr_of!((*Self::font(font)).pages).read_unaligned().num } }
    }

    fn font_page_texture(&mut self, font: Font, page: i32) -> Texture {
        if font.is_null() { return Texture(ptr::null_mut()); }
        let pages = unsafe { ptr::addr_of!((*Self::font(font)).pages).read_unaligned() };
        unsafe { Self::array_item::<FFontPage>(pages, page) }.map_or(Texture(ptr::null_mut()), |p| Texture(p.texture))
    }

    fn font_page_char_count(&mut self, font: Font, page: i32) -> i32 {
        if font.is_null() { return 0; }
        let pages = unsafe { ptr::addr_of!((*Self::font(font)).pages).read_unaligned() };
        unsafe { Self::array_item::<FFontPage>(pages, page) }.map_or(0, |p| p.characters.num)
    }

    fn font_page_char(&mut self, font: Font, page: i32, index: i32) -> Hp2FontChar {
        if font.is_null() { return Hp2FontChar { start_u: 0, start_v: 0, u_size: 0, v_size: 0 }; }
        let pages = unsafe { ptr::addr_of!((*Self::font(font)).pages).read_unaligned() };
        let ch = unsafe { Self::array_item::<FFontPage>(pages, page) }
            .and_then(|p| unsafe { Self::array_item::<FFontCharacter>(p.characters, index) });
        ch.map_or(Hp2FontChar { start_u: 0, start_v: 0, u_size: 0, v_size: 0 }, |c| Hp2FontChar {
            start_u: c.start_u, start_v: c.start_v, u_size: c.u_size, v_size: c.v_size,
        })
    }

    fn font_has_name(&mut self, font: Font) -> bool {
        if font.is_null() { return false; }
        let name = unsafe { ptr::addr_of!((*Self::font(font)).font_name).read_unaligned() };
        if name.num < 0 || (name.num > 0 && name.data.is_null()) { return false; }
        let len = name.num as usize;
        let units = if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(name.data.cast::<u32>(), len) } };
        units.len() > if units.last() == Some(&0) { 1 } else { 0 }
    }

    fn slot_reset(&mut self) { self.slots.fill(ptr::null_mut()); }
    fn slot_texture(&mut self, slot: usize) -> Texture { Texture(self.slots[slot].cast()) }

    fn slot_lock(&mut self, cv: &Hp2CanvasView, slot: usize, tex: Texture) {
        let viewport = unsafe { ptr::addr_of!((*self.canvas).viewport).read_unaligned().cast::<UViewport>() };
        if viewport.is_null() { self.fail(RuntimeError::new("Canonical Canvas has no viewport")); return; }
        let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
        let time = i64::from_ne_bytes(unsafe { ptr::addr_of!((*viewport).current_time).read_unaligned() });
        let texture = tex.0.cast::<UTexture>();
        let access = self.access;
        let result = match unsafe { crate::texture_owner::get(access, texture, time) } {
            Ok(texture) => unsafe {
                crate::texture_owner::lock(access, texture, &mut self.infos[slot], time, 0, device)
                    .map(|()| texture)
            },
            Err(error) => Err(error),
        };
        match result { Ok(texture) => self.slots[slot] = texture, Err(error) => self.fail(error) }
        let _ = cv;
    }

    fn slot_unlock(&mut self, slot: usize) {
        if self.slots[slot].is_null() { return; }
        if let Err(error) = unsafe { crate::texture_owner::unlock(self.access, self.slots[slot], &mut self.infos[slot]) } { self.fail(error); }
        self.slots[slot] = ptr::null_mut();
    }

    fn draw_tile_slot(&mut self, cv: &Hp2CanvasView, slot: usize, rect: &crate::canvas::Hp2CanvasTileRect, color: Hp2Plane, flags: u32) {
        let viewport = unsafe { ptr::addr_of!((*self.canvas).viewport).read_unaligned().cast::<UViewport>() };
        let frame = unsafe { ptr::addr_of!((*self.canvas).frame).read_unaligned().cast::<FSceneNode>() };
        if viewport.is_null() || frame.is_null() { self.fail(RuntimeError::new("Canonical Canvas has no active frame")); return; }
        let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
        let r = unsafe { crate::render_device_owner::draw_tile(
            self.access, device, frame, &mut self.infos[slot], rect.x, rect.y, rect.xl, rect.yl,
            rect.u, rect.v, rect.ul, rect.vl, ptr::null_mut(), cv.z,
            FPlane { x: color.x, y: color.y, z: color.z, w: color.w }, FPlane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 }, flags,
        ) };
        if let Err(error) = r { self.fail(error); }
    }

    fn draw_tile_texture(&mut self, cv: &Hp2CanvasView, draw: &canvas_text::Hp2CanvasTileDraw) {
        let c = draw.color;
        let f = draw.fog;
        if let Err(error) = unsafe { draw_tile(self.access, self.canvas, draw.tex.cast(), draw.rect.x, draw.rect.y, draw.rect.xl, draw.rect.yl, draw.rect.u, draw.rect.v, draw.rect.ul, draw.rect.vl, draw.span, draw.z, FPlane { x: c.x, y: c.y, z: c.z, w: c.w }, FPlane { x: f.x, y: f.y, z: f.z, w: f.w }, draw.flags) } { self.fail(error); }
        let _ = cv;
    }

    fn texture_size(&mut self, tex: Texture) -> Hp2CanvasSize {
        if tex.is_null() { return Hp2CanvasSize::default(); }
        let texture = tex.0.cast::<UTexture>();
        Hp2CanvasSize { w: unsafe { ptr::addr_of!((*texture).base.u_size).read_unaligned() }, h: unsafe { ptr::addr_of!((*texture).base.v_size).read_unaligned() } }
    }

    fn log(&mut self, code: u32) {
        let message = match code {
            canvas_text::LOG_DRAW_TEXT_NO_FONT => "DrawText: No font",
            canvas_text::LOG_DRAW_TEXT_CLIPPED_NO_FONT => "DrawTextClipped: No font",
            canvas_text::LOG_TEXT_SIZE_NO_FONT => "TextSize: No font",
            canvas_text::LOG_DRAW_TILE_MISSING_TEXTURE => "DrawTile: Missing Texture",
            canvas_text::LOG_DRAW_TILE_CLIPPED_MISSING_TEXTURE => "DrawTileClipped: Missing Texture",
            canvas_text::LOG_DRAW_ACTOR_MISSING_ACTOR => "DrawActor: Missing Actor",
            canvas_text::LOG_DRAW_CLIPPED_ACTOR_MISSING_ACTOR => "DrawClippedActor: Missing Actor",
            _ => return,
        };
        let units = message.chars().map(u32::from).collect::<Vec<_>>();
        let result = self.access.intern_str("Log", hp2_core::name_owner::FindName::Add)
            .and_then(|category| self.access.emit_log(category, &units));
        if let Err(error) = result { self.fail(error); }
    }
}

fn canvas_view(canvas: *mut UCanvas) -> RuntimeResult<Hp2CanvasView> {
    if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let (viewport, frame) = unsafe { (ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>(), ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>()) };
    let color = unsafe { ptr::addr_of!((*canvas).color).read_unaligned() };
    // `UCanvas::Color` mirrors as its four `FColor` bytes.
    let color = FColor { r: color[0], g: color[1], b: color[2], a: color[3] };
    let plane = Hp2Plane { x: color.r as f32 / 255.0, y: color.g as f32 / 255.0, z: color.b as f32 / 255.0, w: color.a as f32 / 255.0 };
    let (frame_x, frame_y, frame_fx, frame_fy) = if frame.is_null() { (0, 0, 0.0, 0.0) } else { unsafe { ((*frame).x, (*frame).y, (*frame).fx, (*frame).fy) } };
    Ok(unsafe { Hp2CanvasView {
        font: Font(ptr::addr_of!((*canvas).font).read_unaligned()), large_font: Font(ptr::addr_of!((*canvas).large_font).read_unaligned()),
        big_font: Font(ptr::addr_of!((*canvas).big_font).read_unaligned()), med_font: Font(ptr::addr_of!((*canvas).med_font).read_unaligned()),
        cur_x: (*canvas).cur_x, cur_y: (*canvas).cur_y, cur_yl: (*canvas).cur_yl, clip_x: (*canvas).clip_x,
        clip_y: (*canvas).clip_y, org_x: (*canvas).org_x, org_y: (*canvas).org_y, space_x: (*canvas).space_x,
        space_y: (*canvas).space_y, z: (*canvas).z, x: (*canvas).x, frame_x, frame_y, frame_fx, frame_fy,
        has_frame: u8::from(!frame.is_null()), style: (*canvas).style as i32, center: u8::from((*canvas).b_center()),
        no_smooth: u8::from((*canvas).b_no_smooth()), color: plane,
    } })
}

unsafe fn write_canvas_view(canvas: *mut UCanvas, view: &Hp2CanvasView) {
    unsafe {
        ptr::addr_of_mut!((*canvas).cur_x).write_unaligned(view.cur_x);
        ptr::addr_of_mut!((*canvas).cur_y).write_unaligned(view.cur_y);
        ptr::addr_of_mut!((*canvas).cur_yl).write_unaligned(view.cur_yl);
        ptr::addr_of_mut!((*canvas).clip_x).write_unaligned(view.clip_x);
        ptr::addr_of_mut!((*canvas).clip_y).write_unaligned(view.clip_y);
        ptr::addr_of_mut!((*canvas).org_x).write_unaligned(view.org_x);
        ptr::addr_of_mut!((*canvas).org_y).write_unaligned(view.org_y);
        
    }
}

/// Runs the shared source WrappedPrint policy against canonical Engine resources.
pub(crate) unsafe fn wrapped_print(access: CoreAccess<'_>, canvas: *mut UCanvas, font: *mut UFont, style: i32, center: bool, text: &[u16], count: i32) -> RuntimeResult<(i32, i32)> {
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    let result = canvas_text::wrapped_print(&mut host, &mut view, Font(font.cast()), style, center, text, count);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(result)
}

/// Measures through `WrappedStrLenf`, preserving its WrappedPrint cursor updates.
pub(crate) unsafe fn wrapped_strlenf(access: CoreAccess<'_>, canvas: *mut UCanvas, font: *mut UFont, text: &[u16]) -> RuntimeResult<(i32, i32)> {
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    let result = canvas_text::wrapped_print(&mut host, &mut view, Font(font.cast()), canvas_text::STY_NONE, false, text, 0);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(result)
}

/// Draws text with source `WrappedPrintf` style, centering, and cursor semantics.
pub(crate) unsafe fn wrapped_printf(access: CoreAccess<'_>, canvas: *mut UCanvas, font: *mut UFont, center: bool, text: &[u16]) -> RuntimeResult<()> {
    let _ = unsafe { wrapped_print(access, canvas, font, 1, center, text, 0)? };
    Ok(())
}

pub(crate) unsafe fn canvas_replay_text(access: CoreAccess<'_>, canvas: *mut UCanvas, recording: bool, frame_num: i32, time_seconds: f32) -> RuntimeResult<()> {
    let label = if recording { "Recording" } else { "Replaying" };
    let mut text = formatted_canvas_text(format_args!("{label} F={frame_num} T={time_seconds:.3}"));
    let font = unsafe { ptr::addr_of!((*canvas).small_font).read_unaligned() };
    unsafe { wrapped_printf(access, canvas, font.cast::<UFont>(), false, text.as_nul_terminated()) }
}

pub(crate) unsafe fn canvas_strlen_fps(access: CoreAccess<'_>, canvas: *mut UCanvas, fps: i32) -> RuntimeResult<(i32, i32)> {
    let mut text = formatted_canvas_text(format_args!("{fps}"));
    let font = unsafe { ptr::addr_of!((*canvas).small_font).read_unaligned() };
    unsafe { wrapped_strlenf(access, canvas, font.cast::<UFont>(), text.as_nul_terminated()) }
}

/// Draws source FPS text with the active Canvas color and cursor semantics.
pub(crate) unsafe fn canvas_printf_fps(access: CoreAccess<'_>, canvas: *mut UCanvas, fps: i32) -> RuntimeResult<()> {
    let mut text = formatted_canvas_text(format_args!("{fps}"));
    let font = unsafe { ptr::addr_of!((*canvas).small_font).read_unaligned() };
    unsafe { wrapped_printf(access, canvas, font.cast::<UFont>(), false, text.as_nul_terminated()) }
}

/// Runs canonical `UCanvas::DrawString` through the shared bitmap/native policy.
pub(crate) unsafe fn draw_string(access: CoreAccess<'_>, canvas: *mut UCanvas, args: &canvas_text::DrawStringArgs, text: &[u16]) -> RuntimeResult<i32> {
    let view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    let advance = canvas_text::draw_string(&mut host, &view, args, text);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    Ok(advance)
}

/// Implements `execDrawText` state, wrapping, and cursor behavior on a canonical Canvas.
pub(crate) unsafe fn draw_text(access: CoreAccess<'_>, canvas: *mut UCanvas, text: &[u16], cr: bool, num_chars: i32) -> RuntimeResult<()> {
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    canvas_text::native_draw_text(&mut host, &mut view, text, cr, num_chars);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(())
}

/// Implements `execDrawTextClipped` including its Large/Big font remap writeback.
pub(crate) unsafe fn draw_text_clipped(access: CoreAccess<'_>, canvas: *mut UCanvas, text: &[u16], check_hotkey: bool) -> RuntimeResult<()> {
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    canvas_text::native_draw_text_clipped(&mut host, &mut view, text, check_hotkey);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    unsafe { ptr::addr_of_mut!((*canvas).font).write_unaligned(view.font.0); }
    Ok(())
}

/// Measures wrapped source text while preserving all cursor and clip state.
pub(crate) unsafe fn wrapped_str_lenf(access: CoreAccess<'_>, canvas: *mut UCanvas, font: *mut UFont, text: &[u16]) -> RuntimeResult<(i32, i32)> {
    unsafe { wrapped_strlenf(access, canvas, font, text) }
}

struct CanvasTextBuffer {
    units: [u16; 4096],
    len: usize,
}

impl Default for CanvasTextBuffer {
    fn default() -> Self { Self { units: [0; 4096], len: 0 } }
}

impl std::fmt::Write for CanvasTextBuffer {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        for unit in value.encode_utf16() {
            if self.len == self.units.len() - 1 { break; }
            self.units[self.len] = unit;
            self.len += 1;
        }
        Ok(())
    }
}

impl CanvasTextBuffer {
    fn as_nul_terminated(&mut self) -> &[u16] {
        self.units[self.len] = 0;
        &self.units[..=self.len]
    }
}

fn formatted_canvas_text(args: std::fmt::Arguments<'_>) -> CanvasTextBuffer {
    let mut text = CanvasTextBuffer::default();
    let _ = std::fmt::Write::write_fmt(&mut text, args);
    text
}

/// Implements the `execStrLen` cursor-save behavior on a canonical Canvas.
pub(crate) unsafe fn native_str_len(access: CoreAccess<'_>, canvas: *mut UCanvas, text: &[u16]) -> RuntimeResult<(i32, i32)> {
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    let size = canvas_text::native_str_len(&mut host, &mut view, text);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(size)
}

/// Implements `execTextSize` including source font and cursor/clip preservation.
pub(crate) unsafe fn native_text_size(access: CoreAccess<'_>, canvas: *mut UCanvas, text: &[u16]) -> RuntimeResult<Option<(i32, i32)>> {
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    let size = canvas_text::native_text_size(&mut host, &mut view, text);
    for slot in 0..canvas_text::TEXT_SLOTS { host.slot_unlock(slot); }
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(size)
}

/// Implements `execDrawTile` state/cursor behavior and submits through typed owners.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn native_draw_tile(access: CoreAccess<'_>, canvas: *mut UCanvas, texture: *mut UTexture, xl: f32, yl: f32, u: f32, v: f32, ul: f32, vl: f32) -> RuntimeResult<()> {
    if texture.is_null() {
        let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
        host.log(canvas_text::LOG_DRAW_TILE_MISSING_TEXTURE);
        return host.take_error();
    }
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    canvas_text::native_draw_tile(&mut host, &mut view, texture.cast(), crate::canvas::Hp2CanvasTileRect { x: 0.0, y: 0.0, xl, yl, u, v, ul, vl });
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(())
}

/// Implements `execDrawTileClipped` state/cursor behavior and submits through typed owners.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn native_draw_tile_clipped(access: CoreAccess<'_>, canvas: *mut UCanvas, texture: *mut UTexture, xl: f32, yl: f32, u: f32, v: f32, ul: f32, vl: f32) -> RuntimeResult<()> {
    if texture.is_null() {
        let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
        host.log(canvas_text::LOG_DRAW_TILE_CLIPPED_MISSING_TEXTURE);
        return host.take_error();
    }
    let mut view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    canvas_text::native_draw_tile_clipped(&mut host, &mut view, texture.cast(), crate::canvas::Hp2CanvasTileRect { x: 0.0, y: 0.0, xl, yl, u, v, ul, vl });
    host.take_error()?;
    unsafe { write_canvas_view(canvas, &view); }
    Ok(())
}

/// Implements `UCanvas::DrawIcon` using source icon UVs and the typed tile path.
pub(crate) unsafe fn draw_icon(access: CoreAccess<'_>, canvas: *mut UCanvas, args: &canvas_text::Hp2CanvasPatternArgs) -> RuntimeResult<()> {
    if args.tex.is_null() { return Err(RuntimeError::new("Canvas DrawIcon requires a texture")); }
    let view = canvas_view(canvas)?;
    let mut host = CanonicalTextHost { access, canvas, slots: [ptr::null_mut(); canvas_text::TEXT_SLOTS], infos: unsafe { std::mem::zeroed() }, error: None };
    let size = host.texture_size(Texture(args.tex));
    let draw = canvas_text::draw_icon(args, size);
    host.draw_tile_texture(&view, &draw);
    host.take_error()
}

/// Returns the source Canvas `FFrame::Logf` literal associated with a log code.
pub(crate) fn source_log_message(code: u32) -> Option<&'static str> {
    match code {
        canvas_text::LOG_DRAW_TEXT_NO_FONT => Some("DrawText: No font"),
        canvas_text::LOG_DRAW_TEXT_CLIPPED_NO_FONT => Some("DrawTextClipped: No font"),
        canvas_text::LOG_TEXT_SIZE_NO_FONT => Some("TextSize: No font"),
        canvas_text::LOG_DRAW_TILE_MISSING_TEXTURE => Some("DrawTile: Missing Texture"),
        canvas_text::LOG_DRAW_TILE_CLIPPED_MISSING_TEXTURE => Some("DrawTileClipped: Missing Texture"),
        canvas_text::LOG_DRAW_ACTOR_MISSING_ACTOR => Some("DrawActor: Missing Actor"),
        canvas_text::LOG_DRAW_CLIPPED_ACTOR_MISSING_ACTOR => Some("DrawClippedActor: Missing Actor"),
        _ => None,
    }
}
