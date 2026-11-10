//! Canonical Core ownership for render devices and their Rust GPU resources.

use hp2_abi::{FColor, FPlane, engine::generated::{self, FSceneNode, FSurfaceFacet, FSurfaceInfo, FTextureInfo, FMipmapBase, FTransTexture, FSavedPoly, FCanvasTextRequest, FCanvasTextLayout, UFont, TArrayRaw, UTexture, UObject, UClient, UViewport, UWgpuRenderDevice, URenderDevice, USDLViewport}};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupDefault, StartupProperty, StartupPropertyKind},
    object_owner::ClassOps,
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    ptr::{self, NonNull},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_ABSTRACT: u32 = 1;
const CLASS_CONFIG: u32 = 4;
const CPF_CONFIG: u32 = 0x0000_4000;
const BASE: ClassOps = NativeRecordKind::Subsystem.operations();
const BASE_OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.destroy = destroy_render_device;
    operations
};
const WGPU_OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.construct = construct_wgpu_device;
    operations.destroy = destroy_wgpu_device;
    operations
};
#[cfg(feature = "renderer-wgpu")]
static TEXT_DRAW_LOGGED: AtomicBool = AtomicBool::new(false);
const NATIVE_TEXT_PAGE_SIZE: i32 = 1024;
const NATIVE_TEXT_LAYOUT_MAGIC: u32 = 0x4E54_5854;

fn fstring_units(value: TArrayRaw) -> RuntimeResult<Vec<u32>> {
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical font FString"));
    }
    let length = usize::try_from(value.num)
        .ok()
        .filter(|&length| length.checked_mul(std::mem::size_of::<u32>()).is_some_and(|bytes| isize::try_from(bytes).is_ok()))
        .ok_or_else(|| RuntimeError::new("Canonical font FString is too large"))?;
    if length == 0 { return Ok(Vec::new()); }
    let units = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), length) };
    let count = units.len() - if units.last() == Some(&0) { 1 } else { 0 };
    Ok(units[..count].to_vec())
}

unsafe extern "C" fn font_page_height(font: *const c_void, page: i32) -> i32 {
    if font.is_null() || page < 0 { return 0; }
    unsafe {
        let pages = ptr::addr_of!((*font.cast::<UFont>()).pages).read_unaligned();
        if page >= pages.num || pages.data.is_null() { return 0; }
        let page = pages.data.cast::<hp2_abi::engine::generated::FFontPage>().add(page as usize);
        let characters = ptr::addr_of!((*page).characters).read_unaligned();
        if characters.num <= 0 || characters.data.is_null() { return 0; }
        let records = characters.data.cast::<hp2_abi::engine::generated::FFontCharacter>();
        let mut height = 0;
        for index in 0..characters.num as usize {
            height = height.max(ptr::addr_of!((*records.add(index)).v_size).read_unaligned());
        }
        height
    }
}
#[cfg(all(feature = "renderer-wgpu", feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
fn freetype_font_pages(
    host: &mut hp2_render::native_text_layout_freetype_host::Hp2TextLayoutFreeTypeHost,
    font: *const c_void,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{FFontCharacter, FFontPage};
    if font.is_null() { return Err(RuntimeError::new("FreeType text request has no UFont")); }
    let pages = unsafe { ptr::addr_of!((*font.cast::<UFont>()).pages).read_unaligned() };
    if pages.num < 0 || (pages.num > 0 && pages.data.is_null()) {
        return Err(RuntimeError::new("Canonical UFont has invalid page array"));
    }
    let count = usize::try_from(pages.num).map_err(|_| RuntimeError::new("Canonical UFont page count is invalid"))?;
    for index in 0..count {
        let page = unsafe { pages.data.cast::<FFontPage>().add(index).read_unaligned() };
        let characters = page.characters;
        if characters.num < 0 || (characters.num > 0 && characters.data.is_null()) {
            return Err(RuntimeError::new("Canonical UFont page has invalid character array"));
        }
        let character_count = usize::try_from(characters.num)
            .map_err(|_| RuntimeError::new("Canonical UFont character count is invalid"))?;
        let stride = std::mem::size_of::<FFontCharacter>();
        let length = character_count.checked_mul(stride)
            .filter(|&length| isize::try_from(length).is_ok())
            .ok_or_else(|| RuntimeError::new("Canonical UFont character span is too large"))?;
        let bytes = if length == 0 { Vec::new() } else {
            unsafe { std::slice::from_raw_parts(characters.data.cast::<u8>(), length) }.to_vec()
        };
        host.add_page(hp2_render::native_text_layout_freetype_host::Hp2TextLayoutFreeTypePage::new(
            bytes, character_count, stride, std::mem::offset_of!(FFontCharacter, v_size),
        ));
    }
    Ok(())
}

const BACKENDS: &[&str] = &["Auto", "Metal", "Vulkan", "Dx12", "Gl"];
const VSYNCS: &[&str] = &["Off", "On", "Adaptive"];

const fn config_property(
    name: &'static str,
    kind: StartupPropertyKind,
    category: &'static str,
    offset: usize,
) -> StartupProperty {
    StartupProperty {
        name,
        kind,
        category,
        property_flags: CPF_CONFIG,
        object_flags: 4,
        inner_name: None,
        offset,
        array_dim: 1,
    }
}

const fn bit_property(name: &'static str, category: &'static str, bit: usize) -> StartupProperty {
    config_property(
        name,
        StartupPropertyKind::Bool { bit_mask: 1u32 << (bit % 8) },
        category,
        bit / 8,
    )
}

const RENDER_DEVICE_PROPERTIES: &[StartupProperty] = &[
    bit_property("VolumetricLighting", "Client", URenderDevice::VOLUMETRIC_LIGHTING_BIT),
    bit_property("ShinySurfaces", "Client", URenderDevice::SHINY_SURFACES_BIT),
    bit_property("Coronas", "Client", URenderDevice::CORONAS_BIT),
    bit_property("HighDetailActors", "Client", URenderDevice::HIGH_DETAIL_ACTORS_BIT),
    config_property("Description", StartupPropertyKind::String, "Client", std::mem::offset_of!(URenderDevice, description)),
    config_property("DescFlags", StartupPropertyKind::Int, "Client", std::mem::offset_of!(URenderDevice, desc_flags)),
    bit_property("DetailTextures", "Client", URenderDevice::DETAIL_TEXTURES_BIT),
];

const WGPU_PROPERTIES: &[StartupProperty] = &[
    config_property("Backend", StartupPropertyKind::ByteEnum { name: "Backends", values: BACKENDS }, "Options", std::mem::offset_of!(UWgpuRenderDevice, backend)),
    config_property("OneXBlending", StartupPropertyKind::Bool { bit_mask: 1 }, "Options", std::mem::offset_of!(UWgpuRenderDevice, one_x_blending)),
    config_property("ActorXBlending", StartupPropertyKind::Bool { bit_mask: 1 }, "Options", std::mem::offset_of!(UWgpuRenderDevice, actor_x_blending)),
    config_property("GammaMultiplier", StartupPropertyKind::Float, "Options", std::mem::offset_of!(UWgpuRenderDevice, gamma_multiplier)),
    config_property("GammaMultiplierUED", StartupPropertyKind::Float, "Options", std::mem::offset_of!(UWgpuRenderDevice, gamma_multiplier_ued)),
    config_property("UseVSync", StartupPropertyKind::ByteEnum { name: "VSyncs", values: VSYNCS }, "Options", std::mem::offset_of!(UWgpuRenderDevice, use_v_sync)),
    config_property("RenderScale", StartupPropertyKind::Float, "Options", std::mem::offset_of!(UWgpuRenderDevice, render_scale)),
    config_property("UseAA", StartupPropertyKind::Bool { bit_mask: 1 }, "Options", std::mem::offset_of!(UWgpuRenderDevice, use_aa)),
    config_property("NumAASamples", StartupPropertyKind::Int, "Options", std::mem::offset_of!(UWgpuRenderDevice, num_aa_samples)),
    config_property("MaxAnisotropy", StartupPropertyKind::Float, "Options", std::mem::offset_of!(UWgpuRenderDevice, max_anisotropy)),
    config_property("UseSRGBTextures", StartupPropertyKind::Bool { bit_mask: 1 }, "Options", std::mem::offset_of!(UWgpuRenderDevice, use_srgb_textures)),
];

const RENDER_DEVICE_DEFAULTS: &[StartupDefault] = &[];
const WGPU_DEFAULTS: &[StartupDefault] = &[
    StartupDefault::SetByte { field: "Backend", value: 0 },
    StartupDefault::SetBool { field: "OneXBlending", value: true },
    StartupDefault::SetBool { field: "ActorXBlending", value: true },
    StartupDefault::SetFloat { field: "GammaMultiplier", value: 1.75 },
    StartupDefault::SetFloat { field: "GammaMultiplierUED", value: 1.75 },
    StartupDefault::SetByte { field: "UseVSync", value: if cfg!(target_os = "macos") { 0 } else { 2 } },
    StartupDefault::SetFloat { field: "RenderScale", value: 1.0 },
    StartupDefault::SetBool { field: "UseAA", value: true },
    StartupDefault::SetInt { field: "NumAASamples", value: 4 },
    StartupDefault::SetFloat { field: "MaxAnisotropy", value: 16.0 },
    StartupDefault::SetBool { field: "UseSRGBTextures", value: false },
];

/// The canonical render-device class declarations, in superclass-first order.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "RenderDevice",
        package: "Engine",
        super_name: Some("Subsystem"),
        within: "Object",
        flags: CLASS_ABSTRACT | CLASS_CONFIG,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Subsystem,
        layout: Some(native_layout::<URenderDevice>(URenderDevice::NATIVE_ALIGNMENT)),
        operations: Some(BASE_OPERATIONS),
        properties: RENDER_DEVICE_PROPERTIES,
        defaults: RENDER_DEVICE_DEFAULTS,
    },
    NativeClassSpec {
        short_name: "WgpuRenderDevice",
        package: "WgpuDrv",
        super_name: Some("Engine.RenderDevice"),
        within: "Object",
        flags: CLASS_CONFIG,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Subsystem,
        layout: Some(native_layout::<UWgpuRenderDevice>(UWgpuRenderDevice::NATIVE_ALIGNMENT)),
        operations: Some(WGPU_OPERATIONS),
        properties: WGPU_PROPERTIES,
        defaults: WGPU_DEFAULTS,
    },
];

const fn native_layout<T>(alignment: usize) -> Layout {
    match Layout::from_size_align(std::mem::size_of::<T>(), alignment) {
        Ok(layout) => layout,
        Err(_) => panic!("render-device class alignment is invalid"),
    }
}

/// Rust-owned GPU and provider state, keyed by canonical Core object identity.
#[derive(Default)]
pub(crate) struct RenderOwnerState {
    resources: RefCell<HashMap<usize, Rc<RenderRuntime>>>,
    // Canonical GRenderDevice: the last successfully initialized viewport device.
    global_device: Cell<*mut URenderDevice>,
}

struct RenderRuntime {
    busy: Cell<bool>,
    resources: RefCell<Option<RenderResources>>,
}

#[cfg(feature = "renderer-wgpu")]
struct RenderResources {
    generation: u64,
    loop_state: hp2_wgpu::RenderLoop,
    persisted_world_scale_loaded: bool,
    startup_requested_scale: Option<u8>,
    persisted_graphics: Option<StartupGraphics>,
    startup_warnings: StartupWarnings,
    persisted_graphics_loaded: bool,
    created: Option<hp2_wgpu::CreatedDevice>,
    text: TextResources,
    #[cfg(all(feature = "sdl", target_os = "macos"))]
    metal_view: Option<OwnedMetalView>,
}

#[cfg(not(feature = "renderer-wgpu"))]
struct RenderResources;

#[cfg(feature = "renderer-wgpu")]
#[derive(Default)]
struct TextResources {
    #[cfg(all(feature = "text-coretext", target_os = "macos"))]
    coretext: Option<CoreTextState>,
    #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
    freetype: Option<FreeTypeState>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
struct CoreTextState {
    scratch: hp2_render::native_text_layout_coretext_host::Hp2TextLayoutCoreTextScratch,
    status: i32,
    ready: bool,
    layouts: HashMap<usize, OwnedCanvasTextLayout>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
struct OwnedCanvasTextLayout {
    record: Box<FCanvasTextLayout>,
    layout: Box<hp2_render::native_text_layout_coretext::Layout>,
    draw_cache: Option<Box<NativeTextDrawCache>>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
struct NativeTextDrawCache {
    glyphs: Vec<hp2_wgpu::native_text::NativeTextGlyph>,
    underlines: Vec<hp2_wgpu::native_text::NativeTextUnderline>,
    raster: Vec<TextRaster>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
#[derive(Clone, Copy)]
struct TextRaster {
    font: *const c_void,
    glyph: u16,
    bounds: [f64; 4],
    drawable: bool,
}
#[cfg(all(feature = "renderer-wgpu", feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
struct FreeTypeState {
    ready: bool,
    layouts: HashMap<usize, OwnedFreeTypeCanvasTextLayout>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
struct OwnedFreeTypeCanvasTextLayout {
    record: Box<FCanvasTextLayout>,
    layout: Box<hp2_render::native_text_layout_freetype::Layout>,
    host: Box<hp2_render::native_text_layout_freetype_host::Hp2TextLayoutFreeTypeHost>,
    draw_cache: Option<Box<FreeTypeDrawCache>>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
struct FreeTypeDrawCache {
    glyphs: Vec<hp2_wgpu::native_text::NativeTextGlyph>,
    underlines: Vec<hp2_wgpu::native_text::NativeTextUnderline>,
    raster: Vec<hp2_render::native_text_layout_freetype::Hp2TextLayoutFreeTypePlacement>,
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
impl OwnedFreeTypeCanvasTextLayout {
    fn valid(&self) -> bool {
        self.record.magic == NATIVE_TEXT_LAYOUT_MAGIC && !self.record.raw.is_null()
    }
}

#[cfg(feature = "renderer-wgpu")]
#[derive(Clone, Copy, Default)]
struct TextLayoutFields {
    start_x: i32,
    start_y: i32,
    origin_x: f32,
    origin_y: f32,
    clip_x: f32,
    clip_y: f32,
    clip: bool,
    poly_flags: u32,
    color: [f32; 4],
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
impl CoreTextState {
    fn new() -> Self {
        let mut scratch = hp2_render::native_text_layout_coretext_host::Hp2TextLayoutCoreTextScratch::new();
        let services = hp2_render::native_text_layout_coretext_host::services();
        let status = unsafe {
            hp2_render::native_text_layout_coretext_host::probe_availability(
                &services,
                std::ptr::from_mut(&mut scratch).cast(),
            )
        };
        Self { scratch, status, ready: status == 0, layouts: HashMap::new() }
    }
}

#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
impl OwnedCanvasTextLayout {
    fn valid(&self) -> bool {
        self.record.magic == 0x4E54_5854 && !self.record.raw.is_null()
    }
}
#[cfg(all(feature = "renderer-wgpu", feature = "text-coretext", target_os = "macos"))]
fn coretext_draw_cache(
    layout: &hp2_render::native_text_layout_coretext::Layout,
) -> RuntimeResult<NativeTextDrawCache> {
    use hp2_render::native_text_layout_coretext as text;
    let view = layout.view();
    if (view.glyph_count != 0 && view.glyphs.is_null())
        || (view.underline_count != 0 && view.underlines.is_null())
    {
        return Err(RuntimeError::new("Native text layout has invalid provider spans"));
    }
    let placements = unsafe { std::slice::from_raw_parts(view.glyphs, view.glyph_count) };
    let mut glyphs = Vec::with_capacity(placements.len());
    let mut raster = Vec::with_capacity(placements.len());
    for placement in placements {
        let drawable = placement.drawable != 0;
        if !drawable { continue; }
        let Some(rect) = text::rectangle(placement.bounds, true, NATIVE_TEXT_PAGE_SIZE) else {
            return Err(RuntimeError::new("Native text glyph raster bounds are invalid"));
        };
        if placement.identity_len != 0 && placement.identity.is_null() {
            return Err(RuntimeError::new("Native text glyph has no font identity"));
        }
        let identity = if placement.identity_len == 0 {
            String::new()
        } else {
            String::from_utf8_lossy(unsafe {
                std::slice::from_raw_parts(placement.identity, placement.identity_len)
            }).into_owned()
        };
        let offset = text::offset(placement.position, placement.bounds);
        glyphs.push(hp2_wgpu::native_text::NativeTextGlyph {
            key: hp2_wgpu::text_atlas::GlyphKey {
                font_identity: identity,
                glyph: u32::from(placement.key_glyph),
                raster_point_size_64: placement.point_size64,
                antialias: placement.antialias != 0,
                subpixel: placement.subpixel != 0,
            },
            draw_left: offset[0],
            draw_top: offset[1],
            raster_width: rect[2] as u32,
            raster_height: rect[3] as u32,
        });
        raster.push(TextRaster {
            font: placement.font,
            glyph: placement.glyph,
            bounds: placement.bounds,
            drawable,
        });
    }
    let underlines = unsafe {
        std::slice::from_raw_parts(view.underlines, view.underline_count)
    }.iter().map(|underline| hp2_wgpu::native_text::NativeTextUnderline {
        start: underline.start,
        end: underline.end,
        baseline: underline.baseline,
        position: underline.position,
        thickness: underline.thickness,
    }).collect();
    Ok(NativeTextDrawCache { glyphs, underlines, raster })
}
#[cfg(all(feature = "renderer-wgpu", feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
fn freetype_draw_cache(
    layout: &hp2_render::native_text_layout_freetype::Layout,
) -> RuntimeResult<FreeTypeDrawCache> {
    let mut glyphs = Vec::with_capacity(layout.glyphs.len());
    let mut raster = Vec::with_capacity(layout.glyphs.len());
    for (index, placement) in layout.glyphs.iter().enumerate() {
        if placement.drawable == 0 { continue; }
        let Some(rect) = hp2_render::native_text_layout::raster_rectangle(
            placement.bounds, true, NATIVE_TEXT_PAGE_SIZE,
        ) else {
            return Err(RuntimeError::new("FreeType text glyph raster bounds are invalid"));
        };
        let Some((identity, length)) = layout.identity(index) else {
            return Err(RuntimeError::new("FreeType text glyph has no font identity"));
        };
        if length != 0 && identity.is_null() {
            return Err(RuntimeError::new("FreeType text glyph has an invalid font identity"));
        }
        let identity = if length == 0 { String::new() } else {
            String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(identity, length) }).into_owned()
        };
        let offset = hp2_render::native_text_layout::draw_offset(placement.position, placement.bounds);
        glyphs.push(hp2_wgpu::native_text::NativeTextGlyph {
            key: hp2_wgpu::text_atlas::GlyphKey {
                font_identity: identity,
                glyph: placement.glyph,
                raster_point_size_64: placement.size64,
                antialias: true,
                subpixel: false,
            },
            draw_left: offset[0],
            draw_top: offset[1],
            raster_width: rect[2] as u32,
            raster_height: rect[3] as u32,
        });
        raster.push(*placement);
    }
    let underlines = layout.underlines.iter().map(|underline| {
        hp2_wgpu::native_text::NativeTextUnderline {
            start: underline.start,
            end: underline.end,
            baseline: underline.baseline,
            position: underline.position,
            thickness: underline.thickness,
        }
    }).collect();
    Ok(FreeTypeDrawCache { glyphs, underlines, raster })
}


impl RenderRuntime {
    fn new() -> Self {
        Self {
            busy: Cell::new(false),
            resources: RefCell::new(Some(RenderResources::new())),
        }
    }

    fn lease(self: &Rc<Self>) -> RuntimeResult<RenderLease> {
        if self.busy.replace(true) {
            return Err(RuntimeError::new("Canonical render device is already in use"));
        }
        let resources = self.resources.borrow_mut().take();
        let Some(resources) = resources else {
            self.busy.set(false);
            return Err(RuntimeError::new("Canonical render device resources are unavailable"));
        };
        Ok(RenderLease { runtime: self.clone(), resources: Some(resources) })
    }
}

#[cfg(feature = "renderer-wgpu")]
fn next_graphics_generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_update(std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed, |n| n.checked_add(1)).expect("render device generations exhausted")
}

#[cfg(feature = "renderer-wgpu")]
impl RenderResources {
    fn new() -> Self {
        Self {
            generation: next_graphics_generation(),
            loop_state: hp2_wgpu::RenderLoop::new(),
            persisted_world_scale_loaded: false,
            startup_requested_scale: None,
            persisted_graphics: None,
            startup_warnings: StartupWarnings::default(),
            persisted_graphics_loaded: false,
            created: None,
            text: TextResources::default(),
            #[cfg(all(feature = "sdl", target_os = "macos"))]
            metal_view: None,
        }
    }

    fn release_device(&mut self) {
        self.generation = next_graphics_generation();
        self.loop_state = hp2_wgpu::RenderLoop::new();
        self.persisted_world_scale_loaded = false;
        self.startup_requested_scale = None;
        self.persisted_graphics = None;
        self.startup_warnings = StartupWarnings::default();
        self.persisted_graphics_loaded = false;
        self.created = None;
    }
}

#[cfg(not(feature = "renderer-wgpu"))]
impl RenderResources {
    fn new() -> Self { Self }
}

struct RenderLease {
    runtime: Rc<RenderRuntime>,
    resources: Option<RenderResources>,
}

impl RenderLease {
    fn resources(&mut self) -> &mut RenderResources {
        self.resources.as_mut().expect("render resource lease owns its resources")
    }
}

impl Drop for RenderLease {
    fn drop(&mut self) {
        let resources = self.resources.take().expect("render resource lease is restored once");
        *self.runtime.resources.borrow_mut() = Some(resources);
        self.runtime.busy.set(false);
    }
}

impl RenderOwnerState {
    fn register(&self, object: usize) -> RuntimeResult<()> {
        let mut resources = self.resources.borrow_mut();
        if resources.contains_key(&object) {
            return Err(RuntimeError::new("Canonical render device is already registered"));
        }
        resources.insert(object, Rc::new(RenderRuntime::new()));
        Ok(())
    }

    fn runtime(&self, object: usize) -> RuntimeResult<Rc<RenderRuntime>> {
        self.resources.borrow().get(&object).cloned()
            .ok_or_else(|| RuntimeError::new("Canonical render device is not registered"))
    }

    fn unregister(&self, object: usize) -> RuntimeResult<()> {
        let mut resources = self.resources.borrow_mut();
        let Some(runtime) = resources.get(&object) else {
            return Err(RuntimeError::new("Canonical render device is not registered"));
        };
        if runtime.busy.get() {
            return Err(RuntimeError::new("Cannot destroy a leased canonical render device"));
        }
        resources.remove(&object);
        if self.global_device.get() as usize == object {
            self.global_device.set(ptr::null_mut());
        }
        Ok(())
    }
}

/// Returns the render resources owned by the current canonical Engine graph.
pub(crate) fn state(access: CoreAccess<'_>) -> RuntimeResult<Rc<RenderOwnerState>> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine render owner state is unavailable"))?;
    Ok(engine.render_resources.clone())
}

fn runtime(access: CoreAccess<'_>, device: *mut UWgpuRenderDevice) -> RuntimeResult<Rc<RenderRuntime>> {
    if device.is_null() {
        return Err(RuntimeError::new("Null canonical WGPU render device"));
    }
    state(access)?.runtime(device as usize)
}

unsafe fn construct_wgpu_device(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical WGPU render device")); }
    unsafe { (BASE.construct)(access, object)?; }
    let device = object.cast::<UWgpuRenderDevice>();
    unsafe {
        ptr::addr_of_mut!((*device).rust_device).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*device).metal_view).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*device).native_text_backend).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*device).set_scene_node_call_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).draw2_d_line_call_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).draw_tile_call_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).draw2_d_point_call_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).draw_triangles_call_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).draw_complex_surface_call_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).current_flash_scale).write_unaligned(hp2_abi::FPlane { x: 0.5, y: 0.5, z: 0.5, w: 0.0 });
        ptr::addr_of_mut!((*device).current_flash_fog).write_unaligned(hp2_abi::FPlane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 });
        ptr::addr_of_mut!((*device).end_flash_draw_count).write_unaligned(0);
        ptr::addr_of_mut!((*device).gamma).write_unaligned(1.0);
    }
    let owner = state(access)?;
    if let Err(error) = owner.register(device as usize) {
        unsafe { (BASE.destroy)(access, object)?; }
        return Err(error);
    }
    #[cfg(feature = "renderer-wgpu")]
    if let Err(error) = policy(access, device, ptr::null_mut(), 1, 1)
        .and_then(|adapter| with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::StaticConstructor).map(|_| ()))
    {
        owner.unregister(device as usize)?;
        unsafe { (BASE.destroy)(access, object)?; }
        return Err(error);
    }
    Ok(())
}

unsafe fn destroy_render_device(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice")); }
    let device = object.cast::<URenderDevice>();
    if !state(access)?.resources.borrow().contains_key(&(device as usize)) {
        return unsafe { (BASE.destroy)(access, object) };
    }
    destroy(access, device)?;
    state(access)?.unregister(device as usize)?;
    unsafe { (BASE.destroy)(access, object) }
}

unsafe fn destroy_wgpu_device(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical WGPU render device")); }
    let device = object.cast::<UWgpuRenderDevice>();
    if state(access)?.resources.borrow().contains_key(&(device as usize)) {
        destroy(access, device.cast())?;
        state(access)?.unregister(device as usize)?;
    }
    unsafe { (BASE.destroy)(access, object) }
}

/// Creates and initializes the canonical RenderDevice for an SDL viewport.
///
/// # Safety
/// `viewport` must be a live canonical SDLViewport owned by the current Engine graph.
pub(crate) unsafe fn create_for_viewport(
    access: CoreAccess<'_>,
    viewport: *mut USDLViewport,
    width: i32,
    height: i32,
    _color_bytes: i32,
    _fullscreen: bool,
) -> RuntimeResult<bool> {
    if viewport.is_null() { return Err(RuntimeError::new("Null canonical SDL viewport")); }
    let class = access.native_class("WgpuDrv.WgpuRenderDevice")?;
    let object = unsafe { access.construct_object(class, viewport.cast(), 0, 0, ptr::null_mut())? }
        .cast::<UWgpuRenderDevice>();
    unsafe { ptr::addr_of_mut!((*object).base.viewport).write_unaligned(viewport.cast()); }
    match init(access, object, viewport, width, height) {
        Ok(true) => {
            unsafe { ptr::addr_of_mut!((*viewport).base.ren_dev).write_unaligned(object.cast()); }
            state(access)?.global_device.set(object.cast());
            Ok(true)
        }
        Ok(false) => {
            unsafe { access.conditional_destroy(object.cast())?; access.delete_object(object.cast())?; }
            Ok(false)
        }
        Err(error) => {
            let _ = destroy(access, object.cast());
            unsafe { access.conditional_destroy(object.cast())?; access.delete_object(object.cast())?; }
            Err(error)
        }
    }
}

/// Configures the real renderer surface after viewport sizing has been applied.
///
/// # Safety
/// `device` and `viewport` must be live canonical records in the current Engine graph.
pub(crate) unsafe fn set_res(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    viewport: *mut UViewport,
    width: i32,
    height: i32,
    _color_bytes: i32,
    _fullscreen: bool,
) -> RuntimeResult<bool> {
    if device.is_null() || viewport.is_null() { return Err(RuntimeError::new("Invalid canonical render resize")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let concrete = device.cast::<UWgpuRenderDevice>();
        unsafe { ptr::addr_of_mut!((*device).viewport).write_unaligned(viewport.cast()); }
        let adapter = policy(access, concrete, viewport.cast(), width, height)?;
        let result = with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetRes)?;
        Ok(result != 0)
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, viewport, width, height);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}
/// Returns the live WGPU device's maximum 2D texture dimension.
///
/// # Safety
/// The device and viewport must be live canonical records from this owner.
pub(crate) unsafe fn max_texture_dimension_2d(
    access: CoreAccess<'_>, device: *mut URenderDevice, viewport: *mut UViewport,
) -> RuntimeResult<u32> {
    if device.is_null() || viewport.is_null() { return Err(RuntimeError::new("Invalid canonical render device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let adapter = policy(access, device.cast::<UWgpuRenderDevice>(), viewport.cast(), 1, 1)?;
        adapter.with_resources(|resources| {
            let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("Canonical WGPU device is unavailable"))?;
            Ok(created.device.limits().max_texture_dimension_2d)
        })
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = access; Err(RuntimeError::new("Canonical WGPU renderer provider is disabled")) }
}

/// A staged target pair. Dropping it leaves the active renderer targets intact.
pub(crate) struct PreparedFullscreenTargets {
    #[cfg(feature = "renderer-wgpu")]
    targets: hp2_wgpu::PreparedLogicalTargets,
}

/// Preallocates the fullscreen scene targets before committing a resolution.
///
/// # Safety
/// The device and viewport must be live canonical records from this owner.
pub(crate) unsafe fn prepare_fullscreen_targets(
    access: CoreAccess<'_>, device: *mut URenderDevice, viewport: *mut UViewport, width: u32, height: u32,
) -> RuntimeResult<PreparedFullscreenTargets> {
    if device.is_null() || viewport.is_null() { return Err(RuntimeError::new("Invalid canonical render device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let adapter = policy(access, device.cast::<UWgpuRenderDevice>(), viewport.cast(), width as i32, height as i32)?;
        adapter.with_resources(|resources| {
            let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("Canonical WGPU device is unavailable"))?;
            let targets = resources.loop_state.prepare_fullscreen_targets(created, width, height).map_err(|e| RuntimeError::new(&e))?;
            Ok(PreparedFullscreenTargets { targets })
        })
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, width, height); Err(RuntimeError::new("Canonical WGPU renderer provider is disabled")) }
}

/// Publish staged targets after the renderer and config have accepted the change.
///
/// # Safety
/// The device and viewport must remain live and belong to the same renderer that prepared the targets.
pub(crate) unsafe fn commit_fullscreen_targets(
    access: CoreAccess<'_>, device: *mut URenderDevice, viewport: *mut UViewport, staged: PreparedFullscreenTargets,
) -> RuntimeResult<()> {
    if device.is_null() || viewport.is_null() { return Err(RuntimeError::new("Invalid canonical render device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let adapter = policy(access, device.cast::<UWgpuRenderDevice>(), viewport.cast(), 1, 1)?;
        adapter.with_resources(|resources| { resources.loop_state.commit_logical_targets(staged.targets); Ok(()) })
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, staged); Err(RuntimeError::new("Canonical WGPU renderer provider is disabled")) }
}

/// Draw-call counters owned by one canonical WGPU render-device record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RenderDrawStats {
    pub set_scene_node: i32,
    pub draw_2d_line: i32,
    pub draw_tile: i32,
    pub draw_2d_point: i32,
    pub draw_triangles: i32,
    pub draw_complex_surface: i32,
}

/// Returns the empty string written by `UWgpuRenderDevice::GetStats`.
///
/// The caller converts this text to the target's native `TCHAR` representation.
pub(crate) fn get_stats(device: *mut URenderDevice) -> RuntimeResult<&'static str> {
    if device.is_null() {
        return Err(RuntimeError::new("Null canonical RenderDevice GetStats target"));
    }
    Ok("")
}

/// Reads the live WGPU draw-call counters from its compiler-probed class record.
///
/// # Safety
/// `device` must be a live canonical `WgpuDrv.WgpuRenderDevice` record.
pub(crate) unsafe fn draw_stats(
    device: *mut UWgpuRenderDevice,
) -> RuntimeResult<RenderDrawStats> {
    if device.is_null() {
        return Err(RuntimeError::new("Null canonical WGPU DrawStats target"));
    }
    unsafe {
        Ok(RenderDrawStats {
            set_scene_node: ptr::addr_of!((*device).set_scene_node_call_count).read_unaligned(),
            draw_2d_line: ptr::addr_of!((*device).draw2_d_line_call_count).read_unaligned(),
            draw_tile: ptr::addr_of!((*device).draw_tile_call_count).read_unaligned(),
            draw_2d_point: ptr::addr_of!((*device).draw2_d_point_call_count).read_unaligned(),
            draw_triangles: ptr::addr_of!((*device).draw_triangles_call_count).read_unaligned(),
            draw_complex_surface: ptr::addr_of!((*device).draw_complex_surface_call_count)
                .read_unaligned(),
        })
    }
}

/// Reports whether a fixed WGPU texture cache key currently has a real cached resource.
///
/// # Safety
/// `device` must be a live canonical `WgpuDrv.WgpuRenderDevice` record.
pub(crate) unsafe fn cache_status(
    access: CoreAccess<'_>,
    device: *mut UWgpuRenderDevice,
    cache_id: u64,
) -> RuntimeResult<bool> {
    if device.is_null() {
        return Err(RuntimeError::new("Null canonical WGPU cache status target"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let owner = runtime(access, device)?;
        let mut lease = owner.lease()?;
        Ok(lease.resources().loop_state.is_texture_cached(cache_id))
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, cache_id);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// Reads the last completed WGPU scene frame into Unreal's canonical RGBA pixels.
///
/// # Safety
/// `device` must be a live canonical RenderDevice and belong to `access`.
pub unsafe fn read_pixels(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    destination: &mut [FColor],
) -> RuntimeResult<()> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice ReadPixels target")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<UViewport>() };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical RenderDevice has no viewport")); }
        let (width, height) = unsafe {
            (ptr::addr_of!((*viewport).size_x).read_unaligned(), ptr::addr_of!((*viewport).size_y).read_unaligned())
        };
        if width <= 0 || height <= 0 {
            return Err(RuntimeError::new("Invalid canonical ReadPixels dimensions"));
        }
        let count = usize::try_from(width)
            .ok()
            .and_then(|width| usize::try_from(height).ok().and_then(|height| width.checked_mul(height)))
            .ok_or_else(|| RuntimeError::new("Invalid canonical ReadPixels dimensions"))?;
        if destination.len() != count {
            return Err(RuntimeError::new("Canonical ReadPixels destination size does not match viewport"));
        }
        let owner = runtime(access, device.cast::<UWgpuRenderDevice>())?;
        let mut lease = owner.lease()?;
        let resources = lease.resources();
        let created = resources.created.as_ref()
            .ok_or_else(|| RuntimeError::new("Canonical WGPU device is unavailable"))?;
        let (pixel_width, pixel_height, pixels) = resources.loop_state.read_pixels_rgba8(created)
            .ok_or_else(|| RuntimeError::new("Canonical WGPU frame readback is unavailable"))?;
        let source_count = usize::try_from(pixel_width).ok()
            .and_then(|width| usize::try_from(pixel_height).ok().and_then(|height| width.checked_mul(height)))
            .ok_or_else(|| RuntimeError::new("Invalid canonical WGPU readback dimensions"))?;
        if pixel_width == 0 || pixel_height == 0 || pixels.len() != source_count.checked_mul(4)
            .ok_or_else(|| RuntimeError::new("Canonical WGPU readback size overflow"))?
        {
            return Err(RuntimeError::new("Canonical WGPU readback dimensions are invalid"));
        }
        let output_width = width as usize;
        let output_height = height as usize;
        for (index, pixel) in destination.iter_mut().enumerate() {
            let output_x = index % output_width;
            let output_y = index / output_width;
            let source_x = (output_x as u64 * u64::from(pixel_width) / output_width as u64) as usize;
            let source_y = (output_y as u64 * u64::from(pixel_height) / output_height as u64) as usize;
            let offset = (source_y * pixel_width as usize + source_x) * 4;
            *pixel = FColor {
                r: pixels[offset],
                g: pixels[offset + 1],
                b: pixels[offset + 2],
                a: pixels[offset + 3],
            };
        }
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, destination);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// Locks the canonical WGPU frame after storing the source flash and clear colors.
///
/// # Safety
/// `device` and all three planes must be live for this call.
pub unsafe fn lock(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    flash_scale: *const hp2_abi::FPlane,
    flash_fog: *const hp2_abi::FPlane,
    screen_clear: *const hp2_abi::FPlane,
) -> RuntimeResult<()> {
    if device.is_null() || flash_scale.is_null() || flash_fog.is_null() || screen_clear.is_null() {
        return Err(RuntimeError::new("Invalid canonical RenderDevice Lock arguments"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let concrete = device.cast::<UWgpuRenderDevice>();
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let scale = unsafe { flash_scale.read_unaligned() };
        let fog = unsafe { flash_fog.read_unaligned() };
        let clear = unsafe { screen_clear.read_unaligned() };
        unsafe {
            ptr::addr_of_mut!((*concrete).current_flash_scale).write_unaligned(scale);
            ptr::addr_of_mut!((*concrete).current_flash_fog).write_unaligned(fog);
        }
        let (width, height) = if viewport.is_null() { (1, 1) } else {
            unsafe { (ptr::addr_of!((*viewport).base.size_x).read_unaligned(), ptr::addr_of!((*viewport).base.size_y).read_unaligned()) }
        };
        let mut adapter = policy(access, concrete, viewport, width, height)?;
        adapter.screen_clear = [clear.x, clear.y, clear.z, clear.w];
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Lock)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, flash_scale, flash_fog, screen_clear); Err(RuntimeError::new("Canonical WGPU renderer provider is disabled")) }
}

/// Unlocks and conditionally presents the current canonical WGPU frame.
///
/// # Safety
/// `device` must be a live canonical RenderDevice.
pub unsafe fn unlock(access: CoreAccess<'_>, device: *mut URenderDevice, blit: bool) -> RuntimeResult<()> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice Unlock target")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let concrete = device.cast::<UWgpuRenderDevice>();
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, concrete, viewport, 1, 1)?;
        adapter.blit = blit;
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Unlock)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, blit); Err(RuntimeError::new("Canonical WGPU renderer provider is disabled")) }
}

pub(crate) unsafe fn destroy(access: CoreAccess<'_>, device: *mut URenderDevice) -> RuntimeResult<()> {
    #[cfg(feature = "renderer-wgpu")]
    {
        if device.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice")); }
        let concrete = device.cast::<UWgpuRenderDevice>();
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let (width, height) = if viewport.is_null() { (1, 1) } else {
            unsafe { (ptr::addr_of!((*viewport).base.size_x).read_unaligned(), ptr::addr_of!((*viewport).base.size_y).read_unaligned()) }
        };
        let adapter = policy(access, concrete, viewport, width, height)?;
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Exit)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, device); Ok(()) }
}

unsafe fn init(access: CoreAccess<'_>, device: *mut UWgpuRenderDevice, viewport: *mut USDLViewport, width: i32, height: i32) -> RuntimeResult<bool> {
    #[cfg(feature = "renderer-wgpu")]
    {
        let adapter = policy(access, device, viewport, width, height)?;
        Ok(with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Init)? != 0)
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, device, viewport, width, height); Err(RuntimeError::new("Canonical WGPU renderer provider is disabled")) }
}

/// Executes inherited `URenderDevice::Exec` commands through Core's typed parser.
pub(crate) unsafe fn exec(access: CoreAccess<'_>, device: *mut URenderDevice, command: *const u32, output: &mut crate::exec_output::ExecOutput<'_>) -> RuntimeResult<bool> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice Exec target")); }
    let result = unsafe {
        crate::camera_exec::render_dev_exec_with(
            device,
            command,
            |cursor, token| Ok(hp2_core::command::parse_command(cursor.cast(), token.cast()) != 0),
            |value| output(&[if value == 0 { 48 } else { 49 }]),
            |render_device| {
                (*render_device).set_detail_textures(!(*render_device).detail_textures());
                access.save_config(render_device.cast(), CPF_CONFIG, None)
            },
        )
    }?;
    Ok(result != 0)
}

#[cfg(feature = "renderer-wgpu")]
fn with_policy(mut adapter: CorePolicy<'_>, action: hp2_wgpu::driver_policy::Hp2WgpuDriverAction) -> RuntimeResult<i64> {
    adapter.action = action;
    hp2_wgpu::driver_policy::run_with(adapter, action)
}

#[cfg(feature = "renderer-wgpu")]
#[derive(Clone, Default)]
struct RawMip {
    width: u32,
    height: u32,
    data: *const u8,
    len: usize,
}

#[cfg(feature = "renderer-wgpu")]
#[derive(Default)]
struct CallState {
    frame: *mut FSceneNode,
    info: *mut FTextureInfo,
    points: *mut *mut FTransTexture,
    indices: *mut u16,
    num_points: i32,
    num_indices: i32,
    poly_flags: u32,
    upload_poly_flags: u32,
    line_flags: u32,
    p1: [f32; 3],
    p2: [f32; 3],
    tile: [f32; 9],
    color: [f32; 4],
    fog: [f32; 4],
    surface: *mut FSurfaceInfo,
    facet: *mut FSurfaceFacet,
    c_alpha: u8,
    current_poly: *mut FSavedPoly,
    current_mip: *mut FMipmapBase,
    raw_mips: Vec<RawMip>,
    vertices: Vec<hp2_wgpu::draw::GouraudVertexInput>,
    complex_vertices: Vec<hp2_wgpu::draw::ComplexVertexInput>,
    output_count: usize,
    indexed: bool,
    cache_id: u64,
    sampler_flags: u32,
    has_texture: bool,
    texture_alpha: f32,
    diffuse: f32,
    specular: f32,
    light_cache_id: u64,
    fog_cache_id: u64,
    diffuse_cache_id: u64,
    has_light: bool,
    has_fog: bool,
    light_material: hp2_wgpu::ComplexTextureLayer,
    fog_material: hp2_wgpu::ComplexTextureLayer,
    x_axis: [f32; 4],
    y_axis: [f32; 4],
    text_request: *const FCanvasTextRequest,
    text_layout: *mut FCanvasTextLayout,
    text_layout_output: *mut *mut FCanvasTextLayout,
    text_measure: *mut i32,
    text_measure_height: *mut i32,
    text_drawn: bool,
    text_glyph_count: usize,
    text_underline_count: usize,
    text_fields: Option<TextLayoutFields>,
    #[cfg(all(feature = "text-coretext", target_os = "macos"))]
    text_core_request: Option<hp2_render::native_text_layout_coretext::Hp2TextLayoutCoreTextRequest>,
    #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
    text_freetype_request: Option<hp2_render::native_text_layout_freetype::Hp2TextLayoutFreeTypeRequest>,
    text_name: Vec<u32>,
    text_family: Vec<u32>,
    scene: Option<hp2_wgpu::SceneFrame>,
}

#[cfg(feature = "renderer-wgpu")]
#[derive(Clone)]
struct CorePolicy<'a> {
    access: CoreAccess<'a>,
    device: *mut UWgpuRenderDevice,
    viewport: *mut USDLViewport,
    runtime: Rc<RenderRuntime>,
    width: i32,
    height: i32,
    pixel_width: Cell<i32>,
    pixel_height: Cell<i32>,
    logical_width: Cell<i32>,
    logical_height: Cell<i32>,
    screen_clear: [f32; 4],
    blit: bool,
    action: hp2_wgpu::driver_policy::Hp2WgpuDriverAction,
    call: Rc<RefCell<CallState>>,
}

#[cfg(feature = "renderer-wgpu")]
fn policy<'a>(access: CoreAccess<'a>, device: *mut UWgpuRenderDevice, viewport: *mut USDLViewport, width: i32, height: i32) -> RuntimeResult<CorePolicy<'a>> {
    let runtime = runtime(access, device)?;
    let width = width.max(1);
    let height = height.max(1);
    Ok(CorePolicy {
        access, device, viewport, runtime, width, height,
        pixel_width: Cell::new(width), pixel_height: Cell::new(height),
        logical_width: Cell::new(width), logical_height: Cell::new(height),
        screen_clear: [0.0, 0.0, 0.0, 1.0], blit: true,
        action: hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Init,
        call: Rc::new(RefCell::new(CallState::default())),
    })
}

/// Records a native scene node using the complete shared WGPU scene policy.
///
/// # Safety
/// `frame` must be a live `FSceneNode` belonging to the active viewport.
pub(crate) unsafe fn set_scene_node(access: CoreAccess<'_>, device: *mut URenderDevice, frame: *mut FSceneNode) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() { return Err(RuntimeError::new("Invalid canonical SetSceneNode arguments")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        adapter.call.borrow_mut().frame = frame;
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetSceneNode)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}
/// Returns the shared WGPU policy's maximum vertex count.
///
/// # Safety
/// `device` must be a live canonical RenderDevice.
pub(crate) unsafe fn max_vertices(access: CoreAccess<'_>, device: *mut URenderDevice) -> RuntimeResult<i32> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice MaxVertices target")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let adapter = policy(access, device.cast(), viewport, 1, 1)?;
        i32::try_from(with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::MaxVertices)?)
            .map_err(|_| RuntimeError::new("Canonical WGPU maximum vertex count is invalid"))
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = access;
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// # Safety
/// All native pointers/spans must remain readable for this call.
pub(crate) unsafe fn draw_triangles(access: CoreAccess<'_>, device: *mut URenderDevice, frame: *mut FSceneNode,
info: *mut FTextureInfo, points: *mut *mut FTransTexture, num_points: i32,
indices: *mut u16, num_indices: i32, poly_flags: u32, _span: *mut c_void,) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() || info.is_null() { return Err(RuntimeError::new("Invalid canonical DrawTriangles arguments")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.frame = frame; call.info = info; call.points = points; call.num_points = num_points;
            call.indices = indices; call.num_indices = num_indices; call.poly_flags = poly_flags; call.upload_poly_flags = poly_flags;
            call.indexed = !indices.is_null();
        }
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame, info, points, num_points, indices, num_indices, poly_flags);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// # Safety
/// `frame` and `info` must be valid native objects for this call.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_tile(access: CoreAccess<'_>, device: *mut URenderDevice, frame: *mut FSceneNode,
info: *mut FTextureInfo, x: f32, y: f32, xl: f32, yl: f32,
u: f32, v: f32, ul: f32, vl: f32, _span: *mut c_void, z: f32,
color: FPlane, _fog: FPlane, poly_flags: u32,) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() || info.is_null() { return Err(RuntimeError::new("Invalid canonical DrawTile arguments")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.frame = frame; call.info = info; call.poly_flags = poly_flags; call.upload_poly_flags=poly_flags;
            call.tile = [x, y, xl, yl, u, v, ul, vl, z];
            call.color = [color.x, color.y, color.z, color.w];
        }
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTile)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame, info, x, y, xl, yl, u, v, ul, vl, z, color, poly_flags);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// Uploads a locked canonical texture through the shared WGPU upload policy.
///
/// # Safety
/// `device` and `info` must be live records, with mip and palette pointers
/// borrowed from a successful texture lock for the duration of this call.
pub(crate) unsafe fn precache_texture(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    info: *mut FTextureInfo,
    poly_flags: u32,
) -> RuntimeResult<()> {
    if device.is_null() || info.is_null() {
        return Err(RuntimeError::new("Invalid canonical PrecacheTexture arguments"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe {
            ptr::addr_of!((*device).viewport)
                .read_unaligned()
                .cast::<USDLViewport>()
        };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        adapter.call.borrow_mut().info = info;
        adapter.call.borrow_mut().upload_poly_flags = poly_flags;
        with_policy(
            adapter,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::UploadTexture,
        )?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, info, poly_flags);
        Err(RuntimeError::new(
            "Canonical WGPU renderer provider is disabled",
        ))
    }
}

/// # Safety
/// `frame` must be a live native scene node.
pub(crate) unsafe fn draw_2d_line(access: CoreAccess<'_>, device: *mut URenderDevice, frame: *mut FSceneNode,
color: FPlane, line_flags: u32, p1: hp2_abi::FVector, p2: hp2_abi::FVector,) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() { return Err(RuntimeError::new("Invalid canonical Draw2DLine arguments")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.frame=frame; call.line_flags=line_flags; call.color=[color.x,color.y,color.z,color.w];
            call.p1=[p1.x,p1.y,p1.z]; call.p2=[p2.x,p2.y,p2.z];
        }
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DLine)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame, color, line_flags, p1, p2);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// # Safety
/// `frame` must be a live native scene node.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_2d_point(access: CoreAccess<'_>, device: *mut URenderDevice, frame: *mut FSceneNode,
color: FPlane, line_flags: u32, x1: f32, y1: f32, x2: f32, y2: f32, z: f32,) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() { return Err(RuntimeError::new("Invalid canonical Draw2DPoint arguments")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.frame=frame; call.line_flags=line_flags; call.color=[color.x,color.y,color.z,color.w];
            call.tile=[x1,y1,x2,y2,z,0.0,0.0,0.0,0.0];
        }
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DPoint)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame, color, line_flags, x1, y1, x2, y2, z);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// Transforms, clips, and submits the source render-device 3D line plan.
///
/// # Safety
/// `device`, `frame`, its viewport, and viewport actor must be live records.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_3d_line(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    frame: *mut FSceneNode,
    color: FPlane,
    line_flags: u32,
    p1: hp2_abi::FVector,
    p2: hp2_abi::FVector,
) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Invalid canonical Draw3DLine arguments"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let plan = unsafe { crate::camera::plan_3d_line(frame, line_flags, p1, p2) };
        match plan.kind {
            1 => unsafe {
                draw_2d_line(access, device, frame, color, plan.flags, plan.p1, plan.p2)
            },
            2 => unsafe {
                draw_2d_point(
                    access, device, frame, color, plan.flags,
                    plan.p1.x, plan.p1.y, plan.p2.x, plan.p2.y, plan.p1.z,
                )
            },
            _ => Ok(()),
        }
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame, color, line_flags, p1, p2);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

pub(crate) unsafe fn end_flash(access: CoreAccess<'_>, device: *mut URenderDevice) -> RuntimeResult<()> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical EndFlash device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport=unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        with_policy(policy(access,device.cast(),viewport,1,1)?,hp2_wgpu::driver_policy::Hp2WgpuDriverAction::EndFlash)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

pub(crate) unsafe fn clear_z(access: CoreAccess<'_>, device: *mut URenderDevice) -> RuntimeResult<()> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical ClearZ device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport=unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        with_policy(policy(access,device.cast(),viewport,1,1)?,hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ClearZ)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

#[cfg(feature = "renderer-wgpu")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StartupGraphics { msaa: i32, anisotropy: u16, vsync: u8 }

#[cfg(feature = "renderer-wgpu")]
#[derive(Clone, Debug, Default)]
struct StartupWarnings { vsync: Option<String>, msaa: Option<String>, anisotropy: Option<String>, scale: Option<String> }

#[cfg(feature = "renderer-wgpu")]
fn fallback_apply<T: Copy + PartialEq>(requested: T, fallback: T, mut apply: impl FnMut(T) -> Result<(), String>) -> Result<(T, Option<String>), String> {
    match apply(requested) {
        Ok(()) => Ok((requested, None)),
        Err(error) if requested != fallback => {
            apply(fallback).map_err(|rollback| format!("requested setting failed: {error}; fallback failed: {rollback}"))?;
            Ok((fallback, Some(error)))
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "renderer-wgpu")]
fn graphics_bool(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(feature = "renderer-wgpu")]
fn parse_startup_graphics(default: StartupGraphics, aa: Option<&str>, samples: Option<&str>, anisotropy: Option<&str>, vsync: Option<&str>) -> RuntimeResult<StartupGraphics> {
    let enabled = match aa { Some(v) => graphics_bool(v).ok_or_else(|| RuntimeError::new("Invalid persisted UseAA"))?, None => default.msaa != 0 };
    let msaa = if enabled {
        let value = match samples { Some(v) => v.trim().parse::<i32>().map_err(|_| RuntimeError::new("Invalid persisted NumAASamples"))?, None => if default.msaa == 0 { 4 } else { default.msaa } };
        if ![2, 4].contains(&value) { return Err(RuntimeError::new("Invalid persisted NumAASamples")); }
        value
    } else { 0 };
    let anisotropy = if let Some(value) = anisotropy {
        let value = value.trim().parse::<f64>().map_err(|_| RuntimeError::new("Invalid persisted MaxAnisotropy"))?;
        // Legacy Game.ini files can contain 2, even though the current live
        // control only offers off/4/8/16. Keep the request for fallback reporting.
        if !value.is_finite() || value.fract() != 0.0 || ![0.0, 2.0, 4.0, 8.0, 16.0].contains(&value) { return Err(RuntimeError::new("Invalid persisted MaxAnisotropy")); }
        value as u16
    } else { default.anisotropy };
    if ![0, 2, 4, 8, 16].contains(&anisotropy) { return Err(RuntimeError::new("Invalid persisted MaxAnisotropy")); }
    let vsync = match vsync {
        Some(v) if v.trim().eq_ignore_ascii_case("adaptive") || v.trim() == "2" => 2,
        Some(v) => u8::from(graphics_bool(v).ok_or_else(|| RuntimeError::new("Invalid persisted UseVSync"))?),
        None => default.vsync,
    };
    if vsync > 2 { return Err(RuntimeError::new("Invalid persisted UseVSync")); }
    Ok(StartupGraphics { msaa, anisotropy, vsync })
}

#[cfg(feature = "renderer-wgpu")]
unsafe fn apply_startup_scene(renderer: &mut hp2_wgpu::RenderLoop, created: &hp2_wgpu::CreatedDevice, device: *mut UWgpuRenderDevice, graphics: StartupGraphics) -> RuntimeResult<(Option<String>, Option<String>)> {
    let old_anisotropy = renderer.anisotropy_live();
    let (anisotropy, anisotropy_warning) = fallback_apply(graphics.anisotropy, 0, |value| renderer.set_anisotropy_live(created, value))
        .map_err(|error| RuntimeError::new(&format!("Cannot apply persisted anisotropy or fallback: {error}")))?;
    let (msaa, msaa_warning) = match fallback_apply(graphics.msaa, 0, |value| renderer.set_msaa(created, value)) {
        Ok(value) => value,
        Err(error) => {
            if let Err(rollback) = renderer.restore_anisotropy_live(created, old_anisotropy) {
                return Err(RuntimeError::new(&format!("Cannot apply persisted MSAA or fallback: {error}; anisotropy rollback failed: {rollback}")));
            }
            return Err(RuntimeError::new(&format!("Cannot apply persisted MSAA or fallback: {error}")));
        }
    };
    unsafe {
        let aa = ptr::addr_of!((*device).use_aa).read_unaligned();
        ptr::addr_of_mut!((*device).use_aa).write_unaligned((aa & !1) | i32::from(msaa != 0));
        ptr::addr_of_mut!((*device).num_aa_samples).write_unaligned(msaa);
        ptr::addr_of_mut!((*device).max_anisotropy).write_unaligned(f32::from(anisotropy));
    }
    Ok((anisotropy_warning, msaa_warning))
}

/// Interpret stale configuration without modifying it or preventing startup.
#[cfg(feature = "renderer-wgpu")]
fn tolerant_startup_graphics(default: StartupGraphics, aa: Option<&str>, samples: Option<&str>, anisotropy: Option<&str>, vsync: Option<&str>) -> (StartupGraphics, StartupWarnings) {
    let mut warnings = StartupWarnings::default();
    let enabled = match aa {
        Some(text) => graphics_bool(text).unwrap_or_else(|| { warnings.msaa = Some(format!("invalid UseAA {text:?}; using off")); false }),
        None => default.msaa != 0,
    };
    let msaa = if enabled {
        let value = samples.and_then(|text| text.trim().parse::<i32>().ok()).unwrap_or(if samples.is_some() { -1 } else if default.msaa == 0 { 4 } else { default.msaa });
        if [2, 4].contains(&value) { value } else { warnings.msaa = Some(format!("invalid NumAASamples {:?}; using off", samples)); 0 }
    } else {
        if let Some(text) = samples {
            if !matches!(text.trim().parse::<i32>(), Ok(0 | 2 | 4)) {
                warnings.msaa.get_or_insert_with(|| format!("invalid NumAASamples {text:?}; AA remains off"));
            }
        }
        0
    };
    let anisotropy = match anisotropy {
        Some(text) => match text.trim().parse::<f32>() {
            Ok(value) if value.is_finite() => {
                let clamp = hp2_wgpu::texture::anisotropy_clamp(value);
                let effective = if clamp == 1 { 0 } else { clamp };
                if value != f32::from(effective) { warnings.anisotropy = Some(format!("legacy MaxAnisotropy {text:?} maps to {effective}")); }
                effective
            }
            _ => { warnings.anisotropy = Some(format!("invalid MaxAnisotropy {text:?}; using off")); 0 }
        },
        None if default.anisotropy <= 16 => default.anisotropy,
        None => { warnings.anisotropy = Some("invalid default MaxAnisotropy; using off".into()); 0 }
    };
    let vsync = match vsync {
        Some(text) if text.trim().eq_ignore_ascii_case("adaptive") || text.trim() == "2" => 2,
        Some(text) => match graphics_bool(text) { Some(value) => u8::from(value), None => { warnings.vsync = Some(format!("invalid UseVSync {text:?}; using FIFO")); 1 } },
        None if default.vsync <= 2 => default.vsync,
        None => { warnings.vsync = Some("invalid default UseVSync; using FIFO".into()); 1 }
    };
    (StartupGraphics { msaa, anisotropy, vsync }, warnings)
}

#[cfg(feature = "renderer-wgpu")]
fn read_startup_graphics(access: CoreAccess<'_>, default: StartupGraphics) -> RuntimeResult<(StartupGraphics, StartupWarnings)> {
    let section: Vec<u32> = "WgpuDrv.WgpuRenderDevice".chars().map(u32::from).collect();
    let read = |key: &str| -> RuntimeResult<Option<String>> {
        let key: Vec<u32> = key.chars().map(u32::from).collect();
        Ok(access.config_get_units_from(None, &section, &key)?.map(|units| units.into_iter().map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect()))
    };
    let aa = read("UseAA")?;
    let samples = read("NumAASamples")?;
    let anisotropy = read("MaxAnisotropy")?;
    let vsync = read("UseVSync")?;
    Ok(tolerant_startup_graphics(default, aa.as_deref(), samples.as_deref(), anisotropy.as_deref(), vsync.as_deref()))
}

/// Convert the reflected 0..1 setting without accepting malformed values.
#[cfg(feature = "renderer-wgpu")]
fn persisted_world_scale_percent(value: f32) -> RuntimeResult<u8> {
    let percent = value * 100.0;
    if !percent.is_finite() || !(1.0..=200.0).contains(&percent)
        || (percent - percent.round()).abs() > 0.001 {
        return Err(RuntimeError::new("Invalid persisted world render scale"));
    }
    Ok(percent.round() as u8)
}

#[cfg(all(test, feature = "renderer-wgpu"))]
mod persisted_world_scale_tests {
    #[test]
    fn validated_percent_accepts_saved_slider_and_launcher_values() {
        for (value, expected) in [(0.5, 50), (0.67, 67), (0.75, 75), (1.0, 100)] {
            assert_eq!(super::persisted_world_scale_percent(value).unwrap(), expected);
        }
        for value in [f32::NAN, f32::INFINITY, 0.0, 1.01, 0.505] {
            assert!(super::persisted_world_scale_percent(value).is_err());
        }
    }
}

#[cfg(all(test, feature = "renderer-wgpu"))]
mod live_graphics_wrapper_tests {
    use super::*;
    use hp2_core::core_access::CoreOptions;
    fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

    #[test]
    fn startup_fallback_keeps_reasons_and_rejects_failed_fallbacks() {
        let mut attempted = Vec::new();
        let result = fallback_apply(4, 0, |value| { attempted.push(value); if value == 4 { Err("unsupported".into()) } else { Ok(()) } }).unwrap();
        assert_eq!(result, (0, Some("unsupported".into())));
        assert_eq!(attempted, [4, 0]);
        assert!(fallback_apply(4, 0, |_| Err("device lost".into())).unwrap_err().contains("fallback failed"));
        assert!(fallback_apply(0, 0, |_| Err("device lost".into())).is_err());
        // VSync Off/Adaptive -> FIFO; anisotropy/MSAA -> off; scale -> 100%.
        for (requested, fallback) in [(0, 1), (2, 1), (16, 0), (4, 0), (50, 100)] {
            let (effective, warning) = fallback_apply(requested, fallback, |v| if v == requested { Err("unsupported".into()) } else { Ok(()) }).unwrap();
            assert_eq!(effective, fallback);
            assert!(warning.is_some());
        }
    }

    #[test]
    fn startup_graphics_rejects_invalid_values_and_matches_launcher_booleans() {
        let d = StartupGraphics { msaa: 4, anisotropy: 16, vsync: 0 };
        assert_eq!(parse_startup_graphics(d, Some("yes"), Some("2"), Some("8.000"), Some("On")).unwrap(), StartupGraphics { msaa: 2, anisotropy: 8, vsync: 1 });
        assert_eq!(parse_startup_graphics(d, Some("Off"), Some("stale"), Some("0"), Some("Adaptive")).unwrap(), StartupGraphics { msaa: 0, anisotropy: 0, vsync: 2 });
        assert_eq!(parse_startup_graphics(d, None, None, Some("2.000"), None).unwrap().anisotropy, 2);
        assert_eq!(parse_startup_graphics(StartupGraphics { anisotropy: 2, ..d }, None, None, None, None).unwrap().anisotropy, 2);
        for args in [(Some("maybe"), None, None, None), (Some("true"), Some("3"), None, None), (None, None, Some("4.5"), None), (None, None, Some("NaN"), None), (None, None, Some("-2"), None), (None, None, Some("3"), None), (None, None, None, Some("invalid"))] {
            assert!(parse_startup_graphics(d, args.0, args.1, args.2, args.3).is_err());
        }
    }

    #[test]
    fn stale_graphics_settings_use_warned_fallbacks() {
        let defaults = StartupGraphics { msaa: 4, anisotropy: 8, vsync: 0 };
        let (settings, warnings) = tolerant_startup_graphics(defaults, Some("true"), Some("nonsense"), Some("4.5"), Some("stale"));
        assert_eq!(settings, StartupGraphics { msaa: 0, anisotropy: 4, vsync: 1 });
        assert!(warnings.msaa.is_some() && warnings.anisotropy.is_some() && warnings.vsync.is_some());
        for value in ["NaN", "inf", "junk"] {
            let (settings, warnings) = tolerant_startup_graphics(defaults, None, None, Some(value), None);
            assert_eq!(settings.anisotropy, 0);
            assert!(warnings.anisotropy.is_some());
        }
        for value in ["-3", "0.5", "1", "2", "4.5", "7", "16", "64"] {
            let (settings, _) = tolerant_startup_graphics(defaults, None, None, Some(value), None);
            let clamp = hp2_wgpu::texture::anisotropy_clamp(value.parse().unwrap());
            assert_eq!(settings.anisotropy, if clamp == 1 { 0 } else { clamp });
        }
        let (settings, warnings) = tolerant_startup_graphics(defaults, Some("stale"), None, None, None);
        assert_eq!(settings.msaa, 0);
        assert!(warnings.msaa.is_some());
    }

    #[test]
    #[ignore = "requires GPU and HP2_ARTIFACT_DIR"]
    fn persisted_graphics_reopen_and_gpu_reset_reapply_before_first_draw() {
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join("persisted-graphics-restart");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Game.ini"), "[WgpuDrv.WgpuRenderDevice]\nUseAA=True\nNumAASamples=4\nMaxAnisotropy=8\nUseVSync=On\n").unwrap();
        let open = || crate::EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("Game.ini"), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
        let defaults = StartupGraphics { msaa: 0, anisotropy: 0, vsync: 0 };
        let first = open();
        let saved = read_startup_graphics(first.access(), defaults).unwrap().0;
        assert_eq!(saved, StartupGraphics { msaa: 4, anisotropy: 8, vsync: 1 });
        first.shutdown().unwrap();
        let second = open();
        assert_eq!(read_startup_graphics(second.access(), defaults).unwrap().0, saved);
        let mut device: UWgpuRenderDevice = unsafe { std::mem::zeroed() };
        let mut resources = RenderResources::new();
        let target = |gpu: &hp2_wgpu::CreatedDevice| gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("persisted graphics first draw"), size: wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
        });
        for _ in 0..2 {
            resources.created = Some(hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, None::<hp2_wgpu::HeadlessSurfaceCreator>).unwrap());
            let gpu = resources.created.as_ref().unwrap();
            let target = target(gpu);
            hp2_wgpu::begin_offscreen_frame(gpu, &mut resources.loop_state, &target, wgpu::Color::BLACK);
            unsafe { apply_startup_scene(&mut resources.loop_state, gpu, &mut device, saved).unwrap(); }
            assert_eq!((resources.loop_state.msaa(), resources.loop_state.anisotropy_live()), (4, 8));
            assert_eq!((device.use_aa & 1, device.num_aa_samples, device.max_anisotropy), (1, 4, 8.0));
            assert!(resources.loop_state.begin_scaled_world(gpu).unwrap());
            resources.loop_state.end_scaled_world(gpu).unwrap();
            hp2_wgpu::unlock_frame(gpu, &mut resources.loop_state, false);
            resources.persisted_graphics = Some(saved);
            resources.persisted_graphics_loaded = true;
            resources.release_device();
            assert!(resources.persisted_graphics.is_none());
            assert!(!resources.persisted_graphics_loaded);
        }
        resources.created = Some(hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, None::<hp2_wgpu::HeadlessSurfaceCreator>).unwrap());
        // A rejected MSAA setting uses the safe off fallback while retaining
        // the independently supported anisotropy setting.
        let gpu = resources.created.as_ref().unwrap();
        let target = target(gpu);
        hp2_wgpu::begin_offscreen_frame(gpu, &mut resources.loop_state, &target, wgpu::Color::BLACK);
        device.num_aa_samples = 4;
        device.max_anisotropy = 12.0;
        let (anisotropy_warning, msaa_warning) = unsafe { apply_startup_scene(&mut resources.loop_state, gpu, &mut device, StartupGraphics { msaa: 3, anisotropy: 4, vsync: 1 }) }.unwrap();
        assert!(anisotropy_warning.is_none() && msaa_warning.is_some());
        assert_eq!((resources.loop_state.msaa(), resources.loop_state.anisotropy_live()), (0, 4));
        assert_eq!((device.num_aa_samples, device.max_anisotropy), (0, 4.0));
        let legacy = parse_startup_graphics(defaults, Some("False"), None, Some("2"), None).unwrap();
        let (warning, _) = unsafe { apply_startup_scene(&mut resources.loop_state, gpu, &mut device, legacy) }.unwrap();
        assert!(warning.is_some());
        assert_eq!(legacy.anisotropy, 2);
        assert_eq!(resources.loop_state.anisotropy_live(), 0);
        assert_eq!(device.max_anisotropy, 0.0);
        hp2_wgpu::unlock_frame(gpu, &mut resources.loop_state, false);
        second.shutdown().unwrap();
        let legacy_root = root.join("legacy-anisotropy");
        std::fs::create_dir_all(&legacy_root).unwrap();
        let legacy_ini = "[WgpuDrv.WgpuRenderDevice]\nMaxAnisotropy=2\n";
        std::fs::write(legacy_root.join("Game.ini"), legacy_ini).unwrap();
        let legacy_owner = crate::EngineOwner::new(CoreOptions { data_root: legacy_root.clone(), system_ini: legacy_root.join("Game.ini"), user_ini: legacy_root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
        assert_eq!(read_startup_graphics(legacy_owner.access(), defaults).unwrap().0.anisotropy, 2);
        legacy_owner.shutdown().unwrap();
        assert_eq!(std::fs::read_to_string(legacy_root.join("Game.ini")).unwrap(), legacy_ini);
        let stale_ini = "[WgpuDrv.WgpuRenderDevice]\nUseAA=True\nNumAASamples=garbage\nMaxAnisotropy=4.5\nUseVSync=broken\n";
        std::fs::write(legacy_root.join("Game.ini"), stale_ini).unwrap();
        let stale_owner = crate::EngineOwner::new(CoreOptions { data_root: legacy_root.clone(), system_ini: legacy_root.join("Game.ini"), user_ini: legacy_root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
        let (settings, warnings) = read_startup_graphics(stale_owner.access(), defaults).unwrap();
        assert_eq!(settings, StartupGraphics { msaa: 0, anisotropy: 4, vsync: 1 });
        assert!(warnings.msaa.is_some() && warnings.anisotropy.is_some() && warnings.vsync.is_some());
        stale_owner.shutdown().unwrap();
        assert_eq!(std::fs::read_to_string(legacy_root.join("Game.ini")).unwrap(), stale_ini);
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn startup_unsupported_gpu_format_falls_back_without_changing_request() {
        let gpu = hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, None::<hp2_wgpu::HeadlessSurfaceCreator>).unwrap();
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("unsupported MSAA format"), size: wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, view_formats: &[],
        });
        let mut renderer = hp2_wgpu::RenderLoop::new();
        hp2_wgpu::begin_offscreen_frame(&gpu, &mut renderer, &target, wgpu::Color::BLACK);
        let request = StartupGraphics { msaa: 4, anisotropy: 0, vsync: 1 };
        let mut device: UWgpuRenderDevice = unsafe { std::mem::zeroed() };
        let (_, warning) = unsafe { apply_startup_scene(&mut renderer, &gpu, &mut device, request) }.unwrap();
        assert!(warning.is_some());
        assert_eq!(request.msaa, 4);
        assert_eq!(renderer.msaa(), 0);
        assert_eq!(device.num_aa_samples, 0);
        let requested_scale = 50;
        let (effective_scale, scale_warning) = fallback_apply(requested_scale, 100, |value| renderer.set_world_scale(&gpu, value)).unwrap();
        assert_eq!(requested_scale, 50);
        assert_eq!(effective_scale, 100);
        assert_eq!(renderer.world_scale(), 100);
        assert!(scale_warning.is_some());
        hp2_wgpu::unlock_frame(&gpu, &mut renderer, false);
    }

    #[test]
    #[ignore = "requires GPU and HP2_ARTIFACT_DIR"]
    fn snapshot_apply_and_restore_keep_reflection_and_gpu_distinct() {
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join("live-graphics-wrapper");
        std::fs::create_dir_all(&root).unwrap();
        let owner = crate::EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("Game.ini"), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
        let access = owner.access();
        let mut device: UWgpuRenderDevice = unsafe { std::mem::zeroed() };
        device.use_aa = 2;
        device.num_aa_samples = 4;
        device.max_anisotropy = 12.0;
        let device_ptr = &mut device as *mut UWgpuRenderDevice;
        let owner_state = state(access).unwrap();
        owner_state.register(device_ptr as usize).unwrap();
        owner_state.global_device.set(device_ptr.cast());
        let runtime = owner_state.runtime(device_ptr as usize).unwrap();
        let gpu = hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, None::<hp2_wgpu::HeadlessSurfaceCreator>).unwrap();
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wrapper test"), size: wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        {
            let mut lease = runtime.lease().unwrap();
            let resources = lease.resources();
            resources.loop_state.set_max_anisotropy(7.0);
            hp2_wgpu::begin_offscreen_frame(&gpu, &mut resources.loop_state, &target, wgpu::Color::BLACK);
            resources.created = Some(gpu);
        }
        assert!(unsafe { global_live_graphics_snapshot(access) }.is_err());
        assert!(unsafe { set_global_msaa(access, 4) }.is_err());
        { let mut lease = runtime.lease().unwrap(); lease.resources().persisted_graphics_loaded = true; }
        let snapshot = unsafe { global_live_graphics_snapshot(access).unwrap() };
        assert_eq!((snapshot.msaa, snapshot.anisotropy), (0, 7));
        unsafe { set_global_msaa(access, 4).unwrap(); }
        assert_eq!((device.use_aa, device.num_aa_samples), (3, 4));
        unsafe { restore_global_msaa(access, snapshot).unwrap(); }
        assert_eq!((device.use_aa, device.num_aa_samples), (2, 4));
        assert_eq!(unsafe { global_live_graphics_snapshot(access).unwrap() }.msaa, 0);
        unsafe { set_global_anisotropy(access, 4).unwrap(); }
        assert_eq!(device.max_anisotropy, 4.0);
        unsafe { restore_global_anisotropy(access, snapshot).unwrap(); }
        assert_eq!(device.max_anisotropy, 12.0);
        assert_eq!(unsafe { global_live_graphics_snapshot(access).unwrap() }.anisotropy, 7);
        {
            let mut lease = runtime.lease().unwrap();
            let resources = lease.resources();
            resources.persisted_graphics = Some(StartupGraphics { msaa: 4, anisotropy: 16, vsync: 0 });
            resources.startup_requested_scale = Some(50);
            resources.startup_warnings.msaa = Some("unsupported".into());
            resources.startup_warnings.scale = Some("allocation failed".into());
        }
        let state = unsafe { global_live_graphics_snapshot(access).unwrap() };
        assert_eq!((state.requested_msaa, state.msaa, state.requested_scale, state.effective_scale), (4, 0, 50, 100));
        assert!(state.startup_msaa_fallback && state.startup_scale_fallback);
        unsafe { commit_global_graphics_preference(access, 0).unwrap(); }
        let committed = unsafe { global_live_graphics_snapshot(access).unwrap() };
        assert_eq!(committed.requested_msaa, 0);
        assert!(!committed.startup_msaa_fallback && committed.startup_scale_fallback);
        { let mut lease = runtime.lease().unwrap(); lease.resources().release_device(); }
        assert!(unsafe { restore_global_msaa(access, snapshot) }.is_err());
        owner_state.unregister(device_ptr as usize).unwrap();
        // A new renderer can reuse the same canonical address. Its new
        // generation must reject rollback into that replacement.
        owner_state.register(device_ptr as usize).unwrap();
        owner_state.global_device.set(device_ptr.cast());
        assert!(unsafe { restore_global_msaa(access, snapshot) }.is_err());
        owner_state.unregister(device_ptr as usize).unwrap();
        owner.shutdown().unwrap();
    }
}

/// Change the live world scale only after its GPU attachments validate.
#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn set_global_world_scale(access: CoreAccess<'_>, percent: u8) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("No active GPU device"))?;
    resources.loop_state.set_world_scale(created, percent).map_err(|e| RuntimeError::new(&e))?;
    // The reflected configuration property uses a 0..1 float, not percent.
    // Publish it only after GPU target preflight has succeeded.
    unsafe { ptr::addr_of_mut!((*device.cast::<UWgpuRenderDevice>()).render_scale).write_unaligned(f32::from(percent) / 100.0); }
    Ok(())
}

/// Snapshot both GPU state and reflected values before a settings transaction.
/// They can differ, for example when a persisted value has not yet been applied.
#[cfg(feature = "renderer-wgpu")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct LiveGraphicsSnapshot {
    device_id: usize,
    generation: u64,
    pub msaa: i32,
    /// A world scope has run since this renderer was created.
    pub msaa_observed: bool,
    /// The most recent world scope could not safely use MSAA.
    pub msaa_unavailable: bool,
    pub anisotropy: u16,
    pub vsync_mode: Option<u8>,
    pub requested_msaa: i32,
    pub requested_anisotropy: u16,
    pub requested_vsync: u8,
    pub requested_scale: u8,
    pub effective_scale: u8,
    pub startup_msaa_fallback: bool,
    pub startup_anisotropy_fallback: bool,
    pub startup_vsync_fallback: bool,
    pub startup_scale_fallback: bool,
    pub reflected_use_aa: i32,
    pub reflected_samples: i32,
    pub reflected_anisotropy: f32,
    pub reflected_vsync: u8,
}

#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn global_live_graphics_snapshot(access: CoreAccess<'_>) -> RuntimeResult<LiveGraphicsSnapshot> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    if !resources.persisted_graphics_loaded { return Err(RuntimeError::new("Persisted graphics settings are not applied")); }
    let renderer = &resources.loop_state;
    Ok(LiveGraphicsSnapshot {
        device_id: device as usize,
        generation: resources.generation,
        msaa: renderer.msaa(), msaa_observed: renderer.msaa_observed(),
        msaa_unavailable: renderer.msaa_unavailable(),
        anisotropy: renderer.anisotropy_live(), vsync_mode: renderer.vsync_mode_live(),
        requested_msaa: resources.persisted_graphics.map_or(renderer.msaa(), |v| v.msaa),
        requested_anisotropy: resources.persisted_graphics.map_or(renderer.anisotropy_live(), |v| v.anisotropy),
        requested_vsync: resources.persisted_graphics.map_or(1, |v| v.vsync),
        requested_scale: resources.startup_requested_scale.unwrap_or(renderer.world_scale()),
        effective_scale: renderer.world_scale(),
        startup_msaa_fallback: resources.startup_warnings.msaa.is_some(),
        startup_anisotropy_fallback: resources.startup_warnings.anisotropy.is_some(),
        startup_vsync_fallback: resources.startup_warnings.vsync.is_some(),
        startup_scale_fallback: resources.startup_warnings.scale.is_some(),
        reflected_use_aa: unsafe { ptr::addr_of!((*device).use_aa).read_unaligned() },
        reflected_samples: unsafe { ptr::addr_of!((*device).num_aa_samples).read_unaligned() },
        reflected_anisotropy: unsafe { ptr::addr_of!((*device).max_anisotropy).read_unaligned() },
        reflected_vsync: unsafe { ptr::addr_of!((*device).use_v_sync).read_unaligned() },
    })
}

/// Applies one live value and publishes its reflected property only on success.
/// The settings owner must persist afterward and roll back on persistence failure.
#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn set_global_msaa(access: CoreAccess<'_>, setting: i32) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    if !resources.persisted_graphics_loaded { return Err(RuntimeError::new("Persisted graphics settings are not applied")); }
    let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("No active GPU device"))?;
    resources.loop_state.set_msaa(created, setting).map_err(|e| RuntimeError::new(&e))?;
    unsafe {
        let old = ptr::addr_of!((*device).use_aa).read_unaligned();
        ptr::addr_of_mut!((*device).use_aa).write_unaligned((old & !1) | i32::from(setting != 0));
        ptr::addr_of_mut!((*device).num_aa_samples).write_unaligned(setting);
    }
    Ok(())
}

#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn set_global_anisotropy(access: CoreAccess<'_>, setting: u16) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    if !resources.persisted_graphics_loaded { return Err(RuntimeError::new("Persisted graphics settings are not applied")); }
    let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("No active GPU device"))?;
    resources.loop_state.set_anisotropy_live(created, setting).map_err(|e| RuntimeError::new(&e))?;
    unsafe { ptr::addr_of_mut!((*device).max_anisotropy).write_unaligned(f32::from(setting)); }
    Ok(())
}

/// The optional image-quality features: `(mipmaps, alpha_to_coverage, smooth_lightmaps)`.
/// Stored as `[WgpuDrv.WgpuRenderDevice] Mipmaps / AlphaToCoverage / SmoothLightmaps`; all default on.
pub(crate) fn quality_settings(access: CoreAccess<'_>) -> (bool, bool, bool) {
    let read = |key: &str| {
        let units = |t: &str| t.chars().map(u32::from).collect::<Vec<_>>();
        access.config_get_units(&units(QUALITY_SECTION), &units(key)).ok().flatten()
            .map(|v| v.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect::<String>())
            .map_or(true, |v| !matches!(v.trim().to_ascii_lowercase().as_str(), "false" | "off" | "0"))
    };
    (read("Mipmaps"), read("AlphaToCoverage"), read("SmoothLightmaps"))
}
pub(crate) const QUALITY_SECTION: &str = "WgpuDrv.WgpuRenderDevice";

/// Applies the quality features to the running renderer (textures and pipelines rebuild on demand).
#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn set_global_quality(access: CoreAccess<'_>, mipmaps: bool, alpha_to_coverage: bool, smooth_lightmaps: bool) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    resources.loop_state.set_quality(mipmaps, alpha_to_coverage, smooth_lightmaps);
    Ok(())
}

#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn set_global_vsync_mode(access: CoreAccess<'_>, setting: u8) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    if !resources.persisted_graphics_loaded { return Err(RuntimeError::new("Persisted graphics settings are not applied")); }
    let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("No active GPU device"))?;
    resources.loop_state.set_vsync_mode_live(created, setting).map_err(|e| RuntimeError::new(&e))?;
    unsafe { ptr::addr_of_mut!((*device).use_v_sync).write_unaligned(setting); }
    Ok(())
}

/// Roll back one field after persistence fails. A device replacement invalidates
/// the snapshot; do not write reflected fields on the replacement device.
#[cfg(feature = "renderer-wgpu")]
unsafe fn live_graphics_snapshot_device(access: CoreAccess<'_>, snapshot: LiveGraphicsSnapshot) -> RuntimeResult<*mut UWgpuRenderDevice> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() || device as usize != snapshot.device_id { return Err(RuntimeError::new("Render device changed during graphics transaction")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    if lease.resources().generation != snapshot.generation { return Err(RuntimeError::new("Render device changed during graphics transaction")); }
    Ok(device)
}

#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn restore_global_msaa(access: CoreAccess<'_>, snapshot: LiveGraphicsSnapshot) -> RuntimeResult<()> {
    let device = unsafe { live_graphics_snapshot_device(access, snapshot)? };
    unsafe { set_global_msaa(access, snapshot.msaa)?; }
    unsafe {
        ptr::addr_of_mut!((*device).use_aa).write_unaligned(snapshot.reflected_use_aa);
        ptr::addr_of_mut!((*device).num_aa_samples).write_unaligned(snapshot.reflected_samples);
    }
    Ok(())
}

#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn restore_global_anisotropy(access: CoreAccess<'_>, snapshot: LiveGraphicsSnapshot) -> RuntimeResult<()> {
    let device = unsafe { live_graphics_snapshot_device(access, snapshot)? };
    let owner = state(access)?;
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("No active GPU device"))?;
    resources.loop_state.restore_anisotropy_live(created, snapshot.anisotropy).map_err(|e| RuntimeError::new(&e))?;
    unsafe { ptr::addr_of_mut!((*device).max_anisotropy).write_unaligned(snapshot.reflected_anisotropy); }
    Ok(())
}

#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn restore_global_vsync(access: CoreAccess<'_>, snapshot: LiveGraphicsSnapshot) -> RuntimeResult<()> {
    let device = unsafe { live_graphics_snapshot_device(access, snapshot)? };
    let mode = snapshot.vsync_mode.ok_or_else(|| RuntimeError::new("Previous surface present mode is unknown"))?;
    unsafe { set_global_vsync_mode(access, mode)?; }
    unsafe { ptr::addr_of_mut!((*device).use_v_sync).write_unaligned(snapshot.reflected_vsync); }
    Ok(())
}

/// Called only after the matching live change has been persisted successfully.
/// Kind: 0=MSAA, 1=anisotropy, 2=VSync, 3=world scale.
#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn commit_global_graphics_preference(access: CoreAccess<'_>, kind: u8) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get().cast::<UWgpuRenderDevice>();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    match kind {
        0 => { if let Some(prefs) = resources.persisted_graphics.as_mut() { prefs.msaa = resources.loop_state.msaa(); } resources.startup_warnings.msaa = None; }
        1 => { if let Some(prefs) = resources.persisted_graphics.as_mut() { prefs.anisotropy = resources.loop_state.anisotropy_live(); } resources.startup_warnings.anisotropy = None; }
        2 => { if let Some(prefs) = resources.persisted_graphics.as_mut() { prefs.vsync = unsafe { ptr::addr_of!((*device).use_v_sync).read_unaligned() }; } resources.startup_warnings.vsync = None; }
        3 => { resources.startup_requested_scale = Some(resources.loop_state.world_scale()); resources.startup_warnings.scale = None; }
        _ => return Err(RuntimeError::new("Invalid graphics preference kind")),
    }
    Ok(())
}

/// True when the last world scope could not apply the requested scale because
/// prior full-resolution color draws require an unsupported composition.
/// The UI should present this as a temporary unavailable state, not as a
/// successful change to the effective scale.
#[cfg(feature = "renderer-wgpu")]
pub(crate) unsafe fn world_scale_unavailable(access: CoreAccess<'_>) -> RuntimeResult<bool> {
    let owner = state(access)?;
    let device = owner.global_device.get();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    Ok(resources.loop_state.world_scale_unavailable() || resources.startup_warnings.scale.is_some())
}

#[cfg(feature = "renderer-wgpu")]
unsafe fn scaled_world_scope_action(access: CoreAccess<'_>, begin: bool) -> RuntimeResult<()> {
    let owner = state(access)?;
    let device = owner.global_device.get();
    if device.is_null() { return Err(RuntimeError::new("No active render device")); }
    let runtime = owner.runtime(device as usize)?;
    let mut lease = runtime.lease()?;
    let resources = lease.resources();
    let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("No active GPU device"))?;
    if begin { resources.loop_state.begin_scaled_world(created).map(|_| ()) }
    else { resources.loop_state.end_scaled_world(created) }.map_err(|e| RuntimeError::new(&e))
}

pub(crate) unsafe fn begin_scaled_world(access: CoreAccess<'_>) -> RuntimeResult<()> {
    #[cfg(feature = "renderer-wgpu")]
    { unsafe { scaled_world_scope_action(access, true) } }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = access; Ok(()) }
}

pub(crate) unsafe fn end_scaled_world(access: CoreAccess<'_>) -> RuntimeResult<()> {
    #[cfg(feature = "renderer-wgpu")]
    { unsafe { scaled_world_scope_action(access, false) } }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = access; Ok(()) }
}

/// Clears depth on the source global device, not necessarily the Canvas viewport's device.
///
/// # Safety
/// The global device is null or a live canonical record published by viewport initialization.
pub(crate) unsafe fn global_clear_z(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let device = state(access)?.global_device.get();
    unsafe { clear_z(access, device) }
}

/// # Safety
/// `frame`, `surface`, and `facet` must remain valid for the duration of the call.
pub(crate) unsafe fn draw_complex_surface(access: CoreAccess<'_>, device: *mut URenderDevice, frame: *mut FSceneNode,
surface: *mut FSurfaceInfo, facet: *mut FSurfaceFacet, poly_flags: u32, c_alpha: u8,) -> RuntimeResult<()> {
    if device.is_null() || frame.is_null() || surface.is_null() || facet.is_null() {
        return Err(RuntimeError::new("Invalid canonical DrawComplexSurface arguments"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport=unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter=policy(access,device.cast(),viewport,1,1)?;
        { let mut call=adapter.call.borrow_mut(); call.frame=frame; call.surface=surface; call.facet=facet; call.poly_flags=poly_flags; call.c_alpha=c_alpha; }
        with_policy(adapter,hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = (access, device, frame, surface, facet, poly_flags, c_alpha);
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}
/// The WGPU driver intentionally ignores hit payloads, matching
/// `UWgpuRenderDevice::PushHit` in WgpuDrv.cpp. Viewport hit-size ownership
/// remains in Core's caller; this is only the typed SDK-provider boundary.
///
/// # Safety
/// `device` is a live canonical WGPU render device. The provider does not
/// inspect `hit`, preserving the source-empty SDK behavior without fabricating
/// an `HHitProxy` vtable.
pub(crate) unsafe fn push_hit(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    _hit: *const c_void,
    _size: i32,
) -> RuntimeResult<()> {
    if device.is_null() {
        return Err(RuntimeError::new("Null canonical PushHit render device"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let _ = runtime(access, device.cast())?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = access;
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}

/// `UWgpuRenderDevice::PopHit` is source-empty; viewport hit-size ownership
/// remains in Core's caller.
///
/// # Safety
/// `device` is a live canonical WGPU render device.
pub(crate) unsafe fn pop_hit(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    _size: i32,
    _force: bool,
) -> RuntimeResult<()> {
    if device.is_null() {
        return Err(RuntimeError::new("Null canonical PopHit render device"));
    }
    #[cfg(feature = "renderer-wgpu")]
    {
        let _ = runtime(access, device.cast())?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = access;
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}
#[cfg(feature = "renderer-wgpu")]
impl hp2_wgpu::driver_policy::DriverPolicyAdapter for CorePolicy<'_> {
    type Error = RuntimeError;

    fn invoke(
        &self,
        leaf: hp2_wgpu::driver_policy::Hp2WgpuDriverLeaf,
        a: i64,
        b: i64,
        x: f64,
    ) -> RuntimeResult<hp2_wgpu::driver_policy::Hp2WgpuDriverValue> {
        use hp2_wgpu::driver_policy::Hp2WgpuDriverLeaf as Leaf;
        let value = |integer, real| hp2_wgpu::driver_policy::Hp2WgpuDriverValue { integer, real };
        match leaf {
            Leaf::Properties => Ok(value(0, 0.0)),
            Leaf::DefaultBlending => {
                unsafe {
                    ptr::addr_of_mut!((*self.device).one_x_blending).write_unaligned(a as i32);
                    ptr::addr_of_mut!((*self.device).actor_x_blending).write_unaligned(a as i32);
                }
                Ok(value(0, 0.0))
            }
            Leaf::DefaultGammaMultiplier => {
                unsafe {
                    ptr::addr_of_mut!((*self.device).gamma_multiplier).write_unaligned(x as f32);
                    ptr::addr_of_mut!((*self.device).gamma_multiplier_ued).write_unaligned(x as f32);
                }
                Ok(value(0, x))
            }
            Leaf::DefaultVSync => {
                unsafe { ptr::addr_of_mut!((*self.device).use_v_sync).write_unaligned(a as u8); }
                Ok(value(0, 0.0))
            }
            Leaf::DefaultScale => {
                unsafe { ptr::addr_of_mut!((*self.device).render_scale).write_unaligned(x as f32); }
                Ok(value(0, x))
            }
            Leaf::DefaultAA => {
                unsafe { ptr::addr_of_mut!((*self.device).use_aa).write_unaligned(a as i32); }
                Ok(value(0, 0.0))
            }
            Leaf::DefaultSamples => {
                unsafe { ptr::addr_of_mut!((*self.device).num_aa_samples).write_unaligned(a as i32); }
                Ok(value(0, 0.0))
            }
            Leaf::DefaultAnisotropy => {
                unsafe { ptr::addr_of_mut!((*self.device).max_anisotropy).write_unaligned(x as f32); }
                Ok(value(0, x))
            }
            Leaf::DefaultSrgb => {
                unsafe { ptr::addr_of_mut!((*self.device).use_srgb_textures).write_unaligned(a as i32); }
                Ok(value(0, 0.0))
            }
            Leaf::Device => Ok(value(i64::from(!unsafe { ptr::addr_of!((*self.device).rust_device).read_unaligned() }.is_null()), 0.0)),
            Leaf::DestroyDevice => self.with_resources(|resources| {
                resources.release_device();
                unsafe { ptr::addr_of_mut!((*self.device).rust_device).write_unaligned(ptr::null_mut()); }
                Ok(0)
            }).map(|v| value(v, 0.0)),
            Leaf::DestroyText => self.destroy_text().map(|()| value(0, 0.0)),
            Leaf::Mac => Ok(value(i64::from(cfg!(target_os = "macos")), 0.0)),
            Leaf::Metal => self.with_resources(|resources| {
                #[cfg(all(feature = "sdl", target_os = "macos"))]
                { Ok(i64::from(resources.metal_view.is_some())) }
                #[cfg(not(all(feature = "sdl", target_os = "macos")))]
                { let _ = resources; Ok(0) }
            }).map(|v| value(v, 0.0)),
            Leaf::DestroyMetal => self.with_resources(|resources| {
                #[cfg(all(feature = "sdl", target_os = "macos"))]
                {
                    resources.metal_view = None;
                    unsafe { ptr::addr_of_mut!((*self.device).metal_view).write_unaligned(ptr::null_mut()); }
                }
                #[cfg(not(all(feature = "sdl", target_os = "macos")))]
                { let _ = resources; }
                Ok(0)
            }).map(|v| value(v, 0.0)),
            Leaf::Window => self.window().map(|window| value(i64::from(window.is_some()), 0.0)),
            Leaf::CreateMetal => self.create_metal().map(|created| value(i64::from(created), 0.0)),
            Leaf::MetalFailed => self.log("Init", "WgpuDrv: SDL Metal view creation failed, continuing without it").map(|()| value(0, 0.0)),
            Leaf::WindowMissing => self.log("Init", "WgpuDrv: Viewport has no SDL window, continuing without it").map(|()| value(0, 0.0)),
            Leaf::CreateDevice => self.create_device().map(|created| value(i64::from(created), 0.0)),
            Leaf::Backend => Ok(value(i64::from(self.read_u8(unsafe { ptr::addr_of!((*self.device).backend) })), 0.0)),
            Leaf::BackendError | Leaf::BackendRetryError => Err(RuntimeError::new("Unable to create the selected WGPU backend")),
            Leaf::BackendInvalidError => Err(RuntimeError::new("Canonical WGPU backend selection is invalid")),
            Leaf::PublishDevice => self.with_resources(|resources| {
                let marker = if resources.created.is_some() {
                    Rc::as_ptr(&self.runtime).cast_mut().cast::<c_void>()
                } else {
                    ptr::null_mut()
                };
                unsafe { ptr::addr_of_mut!((*self.device).rust_device).write_unaligned(marker); }
                Ok(0)
            }).map(|v| value(v, 0.0)),
            Leaf::PublishMetal => self.with_resources(|resources| {
                #[cfg(all(feature = "sdl", target_os = "macos"))]
                {
                    let view = resources.metal_view.as_ref().map_or(ptr::null_mut(), |view| view.0.as_ptr());
                    unsafe { ptr::addr_of_mut!((*self.device).metal_view).write_unaligned(view); }
                }
                #[cfg(not(all(feature = "sdl", target_os = "macos")))]
                { let _ = resources; }
                Ok(0)
            }).map(|v| value(v, 0.0)),
            // Applied transactionally with persisted graphics before the first draw.
            Leaf::Anisotropy => Ok(value(0, 0.0)),
            Leaf::Srgb => self.with_resources(|resources| {
                let srgb = self.read_i32(unsafe { ptr::addr_of!((*self.device).use_srgb_textures) }) != 0;
                resources.loop_state.set_srgb_framebuffer(srgb);
                let (mipmaps, alpha_to_coverage, smooth_lightmaps) = quality_settings(self.access);
                resources.loop_state.set_quality(mipmaps, alpha_to_coverage, smooth_lightmaps);
                Ok(0)
            }).map(|v| value(v, 0.0)),
            Leaf::AdapterLog => self.log_adapter().map(|()| value(0, 0.0)),
            Leaf::Surface => self.with_resources(|resources| Ok(i64::from(resources.created.as_ref().is_some_and(|created| created.surface.is_some())))).map(|v| value(v, 0.0)),
            Leaf::SurfaceLog => self.log("Log", "WgpuDrv: surface is active").map(|()| value(0, 0.0)),
            Leaf::CreateText => self.create_text().map(|created| value(i64::from(created), 0.0)),
            Leaf::InitState => {
                unsafe {
                    ptr::addr_of_mut!((*self.device).base.viewport).write_unaligned(self.viewport.cast());
                    ptr::addr_of_mut!((*self.device).set_scene_node_call_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).draw2_d_line_call_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).draw_tile_call_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).draw2_d_point_call_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).draw_triangles_call_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).draw_complex_surface_call_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).end_flash_draw_count).write_unaligned(0);
                    ptr::addr_of_mut!((*self.device).current_flash_scale).write_unaligned(hp2_abi::FPlane { x: 0.5, y: 0.5, z: 0.5, w: 0.0 });
                    ptr::addr_of_mut!((*self.device).current_flash_fog).write_unaligned(hp2_abi::FPlane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 });
                }
                Ok(value(0, x))
            }
            Leaf::InitSetRes => {
                let mut nested = self.clone();
                nested.width = self.viewport_width();
                nested.height = self.viewport_height();
                with_policy(nested, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetRes).map(|v| value(v, 0.0))
            }
            Leaf::Flush => {
                let nested = self.clone();
                with_policy(nested, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Flush).map(|v| value(v, 0.0))
            }
            Leaf::ConfigureSurface => self.configure_surface().map(|configured| value(i64::from(configured), 0.0)),
            Leaf::Viewport => Ok(value(i64::from(!self.viewport.is_null()), 0.0)),
            Leaf::Client => Ok(value(i64::from(self.client().is_some()), 0.0)),
            Leaf::Brightness => Ok(value(0, f64::from(self.brightness()))),
            Leaf::Editor => Ok(value(i64::from(self.access.process_mode().is_editor), 0.0)),
            Leaf::GammaMultiplier => Ok(value(0, f64::from(self.read_f32(unsafe { ptr::addr_of!((*self.device).gamma_multiplier) })))),
            Leaf::EditorGammaMultiplier => Ok(value(0, f64::from(self.read_f32(unsafe { ptr::addr_of!((*self.device).gamma_multiplier_ued) })))),
            Leaf::GammaWrite => {
                unsafe { ptr::addr_of_mut!((*self.device).gamma).write_unaligned(x as f32); }
                Ok(value(0, x))
            }
            Leaf::ClearCache => self.with_resources(|resources| {
                if let Some(created) = resources.created.as_ref() {
                    resources.loop_state.clear_texture_cache(created);
                }
                Ok(0)
            }).map(|v| value(v, 0.0)),
            Leaf::CountsLog => self.log_counts().map(|()| value(0, 0.0)),
            Leaf::DrawCountsLog => self.log_draw_counts().map(|()| value(0, 0.0)),
            Leaf::FlashState => Ok(value(0, 0.0)),
            Leaf::Drawable | Leaf::WindowSize | Leaf::PixelWidth | Leaf::PixelHeight |
            Leaf::LogicalHeight | Leaf::LogicalFallback => self.viewport_dimension(leaf).map(|dimension| value(dimension, 0.0)),
            Leaf::BackingScale => self.with_resources(|resources| {
                resources.loop_state.set_backing_scale(x);
                Ok(0)
            }).map(|v| value(v, 0.0)),
            Leaf::LockDevice => self.lock_frame(a, b).map(|outcome| value(outcome as i64, 0.0)),
            Leaf::LockLog => self.log("Dev", &format!("WgpuDrv: Lock outcome code {a}: no window frame presented this tick")).map(|()| value(0, 0.0)),
            Leaf::UnlockDevice => self.unlock_frame().map(|presented| value(i64::from(presented), 0.0)),
            Leaf::FixCache => self.fix_cache().map(|id| value(id as i64, 0.0)),
            Leaf::Cached => self.cached().map(|cached| value(i64::from(cached), 0.0)),
            Leaf::Dirty => self.dirty().map(|dirty| value(i64::from(dirty), 0.0)),
            Leaf::ResetDirty => self.reset_dirty().map(|()| value(0, 0.0)),
            Leaf::Texture => self.texture_present().map(|present| value(i64::from(present), 0.0)),
            Leaf::UClamp => self.texture_field_i32(|i| i.u_clamp).map(|v| value(i64::from(v), 0.0)),
            Leaf::VClamp => self.texture_field_i32(|i| i.v_clamp).map(|v| value(i64::from(v), 0.0)),
            Leaf::USize => self.texture_field_i32(|i| i.u_size).map(|v| value(i64::from(v), 0.0)),
            Leaf::VSize => self.texture_field_i32(|i| i.v_size).map(|v| value(i64::from(v), 0.0)),
            Leaf::PolyFlags => Ok(value(i64::from(self.call.borrow().upload_poly_flags), 0.0)),
            Leaf::Sampler => { self.call.borrow_mut().sampler_flags = a as u32; Ok(value(0, 0.0)) }
            Leaf::Format => self.texture_field_i32(|i| i.format).map(|v| value(i64::from(v), 0.0)),
            Leaf::Palette => self.texture_field_ptr(|i| i.palette).map(|p| value(i64::from(!p.is_null()), 0.0)),
            Leaf::NoPalette => self.log("Warning", "WgpuDrv: TEXF_P8 texture has a null Palette, skipping upload").map(|()| value(0, 0.0)),
            Leaf::NumMips => self.texture_field_i32(|i| i.num_mips).map(|v| value(i64::from(v), 0.0)),
            Leaf::Mip => self.select_mip(a as usize).map(|present| value(i64::from(present), 0.0)),
            Leaf::MipData => self.mip_data().map(|present| value(i64::from(present), 0.0)),
            Leaf::MipWidth => self.mip_dimension(true).map(|v| value(i64::from(v), 0.0)),
            Leaf::MipHeight => self.mip_dimension(false).map(|v| value(i64::from(v), 0.0)),
            Leaf::AllocMips => self.alloc_mips(a as usize).map(|()| value(0, 0.0)),
            Leaf::InvalidMip => self.log("Warning", &format!("WgpuDrv: DXT1 texture has invalid mip {}", a)).map(|()| value(0, 0.0)),
            Leaf::WriteMip => self.write_mip(a as usize, b as usize).map(|()| value(0, 0.0)),
            Leaf::UploadCompressed => self.upload_compressed().map(|uploaded| value(i64::from(uploaded), 0.0)),
            Leaf::UploadUncompressed => self.upload_uncompressed(a as usize, b != 0).map(|()| value(0, 0.0)),
            Leaf::SceneFields => self.scene_fields().map(|()| value(0, 0.0)),
            Leaf::Wire => self.wire().map(|wire| value(i64::from(wire), 0.0)),
            Leaf::SceneWire => self.scene_update(|scene| scene.is_wireframe_zfar = a != 0).map(|()| value(0, 0.0)),
            Leaf::SceneOrtho => self.scene_read(|scene| i64::from(scene.is_ortho)).map(|v| value(v, 0.0)),
            Leaf::Gamma => Ok(value(0, f64::from(self.read_f32(unsafe { ptr::addr_of!((*self.device).gamma) })))),
            Leaf::SceneGamma => self.scene_update(|scene| scene.gamma = x as f32).map(|()| value(0, 0.0)),
            Leaf::ActorBlending => Ok(value(i64::from(self.read_i32(unsafe { ptr::addr_of!((*self.device).actor_x_blending) }) != 0), 0.0)),
            Leaf::OneBlending => Ok(value(i64::from(self.read_i32(unsafe { ptr::addr_of!((*self.device).one_x_blending) }) != 0), 0.0)),
            Leaf::SceneColor => self.scene_update(|scene| scene.light_color_intensity = x as f32).map(|()| value(0, 0.0)),
            Leaf::SceneMap => self.scene_update(|scene| scene.light_map_intensity = x as f32).map(|()| value(0, 0.0)),
            Leaf::Count => self.draw_count().map(|count| value(i64::from(count), 0.0)),
            Leaf::CountWrite => self.set_draw_count(a as i32).map(|()| value(0, 0.0)),
            Leaf::FirstLog => self.log_first_draw().map(|()| value(0, 0.0)),
            Leaf::Submit => self.submit().map(|submitted| value(i64::from(submitted), 0.0)),
            Leaf::Points => Ok(value(i64::from(!self.call.borrow().points.is_null()), 0.0)),
            Leaf::NumPoints => {
                let call = self.call.borrow();
                let count = match self.action {
                    hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles
                    | hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody => call.num_points,
                    hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface
                    | hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ComplexBody => {
                        if call.current_poly.is_null() {
                            return Err(RuntimeError::new("Canonical complex surface has no current polygon"));
                        }
                        unsafe {
                            ptr::addr_of!((*call.current_poly).num_pts).read_unaligned()
                        }
                    }
                    _ => return Err(RuntimeError::new("Canonical NumPoints leaf reached outside a geometry draw")),
                };
                Ok(value(i64::from(count), 0.0))
            }
            Leaf::Indexed => Ok(value(i64::from(self.call.borrow().indexed), 0.0)),
            Leaf::NumIndices => Ok(value(i64::from(self.call.borrow().num_indices), 0.0)),
            Leaf::ReadIndex => self.read_index(a as usize).map(|index| value(i64::from(index), 0.0)),
            Leaf::PointExists => self.point_exists(a as usize).map(|exists| value(i64::from(exists), 0.0)),
            Leaf::InvalidVertex => self.log("Warning", "WgpuDrv: DrawTriangles contains an invalid vertex index").map(|()| value(0, 0.0)),
            Leaf::AllocateVertices => self.allocate_vertices(a as usize, b != 0).map(|()| value(0, 0.0)),
            Leaf::FillVertex => self.fill_vertex(a as usize, b as usize).map(|()| value(0, 0.0)),
            Leaf::OutputCount => Ok(value(self.call.borrow().output_count as i64, 0.0)),
            Leaf::Upload => self.upload_for_draw().map(|id| value(id as i64, 0.0)),
            Leaf::HasTexture => { self.call.borrow_mut().has_texture = a != 0; Ok(value(0, 0.0)) }
            Leaf::TextureMaterial => self.read_texture_material().map(|()| value(0, 0.0)),
            Leaf::TextureAlpha => self.read_texture_alpha().map(|()| value(0, 0.0)),
            Leaf::NeutralScale => {
                let flash = unsafe { ptr::addr_of!((*self.device).current_flash_scale).read_unaligned() };
                Ok(value(i64::from(flash == FPlane { x:0.5,y:0.5,z:0.5,w:0.0 }),0.0))
            }
            Leaf::ZeroFog => {
                let fog = unsafe { ptr::addr_of!((*self.device).current_flash_fog).read_unaligned() };
                Ok(value(i64::from(fog == FPlane { x:0.0,y:0.0,z:0.0,w:0.0 }),0.0))
            }
            Leaf::FlashScale => {
                let flash = unsafe { ptr::addr_of!((*self.device).current_flash_scale).read_unaligned() };
                Ok(value(0, f64::from(flash.x)))
            }
            Leaf::FlashAlpha => { self.call.borrow_mut().texture_alpha = x as f32; Ok(value(0, x)) }
            Leaf::SurfaceTexture => self.surface_field_ptr(|s| s.texture).map(|p| value(i64::from(!p.is_null()), 0.0)),
            Leaf::UploadDiffuse => self.upload_surface_texture(true, false).map(|id| value(id as i64, 0.0)),
            Leaf::LightMap => self.surface_field_ptr(|s| s.light_map).map(|p| value(i64::from(!p.is_null()), 0.0)),
            Leaf::HasLightMap => { self.call.borrow_mut().has_light = a != 0; Ok(value(0, 0.0)) }
            Leaf::UploadLight => self.upload_surface_texture(false, false).map(|id| value(id as i64, 0.0)),
            Leaf::FogMap => self.surface_field_ptr(|s| s.fog_map).map(|p| value(i64::from(!p.is_null()), 0.0)),
            Leaf::FogMip => self.fog_mip().map(|present| value(i64::from(present), 0.0)),
            Leaf::FogData => self.mip_data().map(|present| value(i64::from(present), 0.0)),
            Leaf::HasFogMap => { self.call.borrow_mut().has_fog = a != 0; Ok(value(0, 0.0)) }
            Leaf::UploadFog => self.upload_surface_texture(false, true).map(|id| value(id as i64, 0.0)),
            Leaf::Axes => self.surface_axes().map(|()| value(0, 0.0)),
            Leaf::FirstPoly => self.first_poly().map(|present| value(i64::from(present), 0.0)),
            Leaf::NextPoly => self.next_poly().map(|present| value(i64::from(present), 0.0)),
            Leaf::LightMaterial => self.light_material().map(|()| value(0, 0.0)),
            Leaf::FogMaterial => self.fog_material().map(|()| value(0, 0.0)),
            Leaf::SavedLight => Ok(value(i64::from(self.call.borrow().has_light), 0.0)),
            Leaf::SavedFog => Ok(value(i64::from(self.call.borrow().has_fog), 0.0)),
            Leaf::SavedTexture => Ok(value(i64::from(self.call.borrow().has_texture), 0.0)),
            _ => self.text_leaf(leaf, a, b, x).map(|(integer, real)| value(integer, real)),
        }
    }
}
/// Flushes queued render work/cache state for the canonical RenderDevice.
///
/// # Safety
/// `device` must be a live canonical RenderDevice record.
pub(crate) unsafe fn flush(access: CoreAccess<'_>, device: *mut URenderDevice, _allow_precache: bool) -> RuntimeResult<()> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical RenderDevice")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let concrete = device.cast::<UWgpuRenderDevice>();
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let adapter = policy(access, concrete, viewport, 1, 1)?;
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Flush)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    {
        let _ = access;
        Err(RuntimeError::new("Canonical WGPU renderer provider is disabled"))
    }
}


/// Creates a canonical text layout through the shared WGPU driver policy.
///
/// # Safety
/// `device`, `request`, and `out_layout` must be readable/writable for this call.
pub(crate) unsafe fn create_canvas_text_layout(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    request: *const FCanvasTextRequest,
    out_layout: *mut *mut FCanvasTextLayout,
) -> RuntimeResult<bool> {
    if device.is_null() || request.is_null() || out_layout.is_null() {
        return Err(RuntimeError::new("Invalid canonical CreateCanvasTextLayout arguments"));
    }
    unsafe { out_layout.write_unaligned(ptr::null_mut()); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.text_request = request;
            call.text_layout_output = out_layout;
        }
        Ok(with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::CreateCanvasTextLayout)? != 0)
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, request); Ok(false) }
}

/// Releases a canonical provider-owned text layout.
///
/// # Safety
/// `device` must be live; `layout` may be null or a live layout created by this renderer.
pub(crate) unsafe fn destroy_canvas_text_layout(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    layout: *mut FCanvasTextLayout,
) -> RuntimeResult<()> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical DestroyCanvasTextLayout device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        adapter.call.borrow_mut().text_layout = layout;
        with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DestroyCanvasTextLayout)?;
        Ok(())
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, layout); Ok(()) }
}

/// Measures a canonical provider-owned text layout.
///
/// # Safety
/// `device`, `width`, and `height` must be valid; `layout` may be null or a live text layout.
pub(crate) unsafe fn measure_canvas_text(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    layout: *mut FCanvasTextLayout,
    width: *mut i32,
    height: *mut i32,
) -> RuntimeResult<bool> {
    if device.is_null() || width.is_null() || height.is_null() {
        return Err(RuntimeError::new("Invalid canonical MeasureCanvasText arguments"));
    }
    unsafe { width.write_unaligned(0); height.write_unaligned(0); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.text_layout = layout;
            call.text_measure = width;
            call.text_measure_height = height;
        }
        Ok(with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::MeasureCanvasText)? != 0)
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, layout); Ok(false) }
}

/// Draws a canonical provider-owned text layout into the active WGPU frame.
///
/// # Safety
/// `device`, `frame`, and `layout` must belong to the current live render graph.
pub(crate) unsafe fn draw_canvas_text(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    frame: *mut FSceneNode,
    layout: *mut FCanvasTextLayout,
) -> RuntimeResult<bool> {
    if device.is_null() { return Err(RuntimeError::new("Null canonical DrawCanvasText device")); }
    #[cfg(feature = "renderer-wgpu")]
    {
        let viewport = unsafe { ptr::addr_of!((*device).viewport).read_unaligned().cast::<USDLViewport>() };
        let mut adapter = policy(access, device.cast(), viewport, 1, 1)?;
        {
            let mut call = adapter.call.borrow_mut();
            call.frame = frame;
            call.text_layout = layout;
        }
        Ok(with_policy(adapter, hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawCanvasText)? != 0)
    }
    #[cfg(not(feature = "renderer-wgpu"))]
    { let _ = (access, frame, layout); Ok(false) }
}
/// Converts the shared Canvas UTF-16 request to the host TCHAR scalar units
/// consumed by the native text providers.
fn canvas_text_tchar_units(text: &[u16]) -> Vec<u32> {
    let mut units = Vec::with_capacity(text.len() + 1);
    units.extend(char::decode_utf16(text.iter().copied())
        .map(|decoded| u32::from(decoded.unwrap_or(char::REPLACEMENT_CHARACTER))));
    units
}

#[cfg(test)]
mod canvas_text_tchar_tests {
    #[test]
    fn utf16_surrogate_pair_is_one_native_scalar_unit() {
        assert_eq!(
            super::canvas_text_tchar_units(&[b'A' as u16, 0xd83e, 0xddd9, b'7' as u16]),
            [u32::from(b'A'), 0x1f9d9, u32::from(b'7')],
        );
    }
}

/// Attempts the source `TryNativeCanvasText` flow through the real provider,
/// with only typed Core client/viewport access and no C++ engine callback.
///
/// # Safety
/// `device`, `frame`, `font`, and any text units must stay live through this
/// synchronous attempt; `view` and `request` must describe the active Canvas.
pub(crate) unsafe fn try_native_canvas_text(
    access: CoreAccess<'_>,
    device: *mut URenderDevice,
    frame: *mut FSceneNode,
    view: &crate::canvas_text::Hp2CanvasView,
    request: &crate::canvas_text::Hp2NativeTextRequest,
    font: *mut UFont,
    text: &[u16],
    color: crate::canvas_text::Hp2Plane,
) -> RuntimeResult<Option<(i32, i32)>> {
    if device.is_null() || font.is_null() || text.is_empty() {
        return Ok(None);
    }
    let viewport = unsafe { ptr::read_unaligned(ptr::addr_of!((*device).viewport)).cast::<UViewport>() };
    if viewport.is_null() {
        return Ok(None);
    }
    let client = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).outer)).cast::<UClient>() };
    if client.is_null() {
        return Ok(None);
    }
    let font_name = unsafe { ptr::read_unaligned(ptr::addr_of!((*font).font_name)) };
    if font_name.num < 0 || (font_name.num > 0 && font_name.data.is_null()) {
        return Err(RuntimeError::new("Canonical UFont has an invalid FontName"));
    }
    let name_len = usize::try_from(font_name.num)
        .map_err(|_| RuntimeError::new("Canonical UFont FontName is invalid"))?;
    let name_units = if name_len == 0 {
        &[][..]
    } else {
        let byte_len = name_len.checked_mul(4)
            .filter(|&bytes| isize::try_from(bytes).is_ok())
            .ok_or_else(|| RuntimeError::new("Canonical UFont FontName is too large"))?;
        let _ = byte_len;
        unsafe { std::slice::from_raw_parts(font_name.data.cast::<u32>(), name_len) }
    };
    let name_nonempty = name_units.len() > if name_units.last() == Some(&0) { 1 } else { 0 };
    let native_text = unsafe { (*client).native_text() };
    if !native_text && !name_nonempty {
        return Ok(None);
    }
    let client_scale = unsafe {
        ptr::read_unaligned(ptr::addr_of!((*client.cast::<hp2_abi::engine::generated::USDLClient>()).ui_scale))
    };
    // `GetCanvasTextScale(Canvas)` over the same view and client UI scale.
    let text_scale = crate::canvas_text::text_scale_from(view, client_scale);
    let utf16_length = text.iter().position(|unit| *unit == 0).unwrap_or(text.len());
    let mut terminated = canvas_text_tchar_units(&text[..utf16_length]);
    let text_length = i32::try_from(terminated.len())
        .map_err(|_| RuntimeError::new("Canonical Canvas text exceeds native length range"))?;
    if text_length == 0 {
        return Ok(None);
    }
    terminated.push(0);
    let visible = if request.visible > 0 {
        let visible_units = usize::try_from(request.visible).unwrap_or(usize::MAX).min(utf16_length);
        i32::try_from(char::decode_utf16(text[..visible_units].iter().copied()).count())
            .map_err(|_| RuntimeError::new("Canonical Canvas visible text exceeds native length range"))?
    } else {
        text_length
    };
    let native_request = FCanvasTextRequest {
        font: font.cast(),
        text: terminated.as_mut_ptr().cast(),
        text_length,
        text_scale,
        space_x: view.space_x * text_scale,
        space_y: view.space_y * text_scale,
        origin_x: view.org_x,
        origin_y: view.org_y,
        clip_x: view.clip_x,
        clip_y: view.clip_y,
        start_x: request.draw_x,
        start_y: request.draw_y,
        poly_flags: request.flags,
        color: FPlane {
            x: color.x,
            y: color.y,
            z: color.z,
            w: color.w,
        },
        b_clip: if request.clip != 0 { 1 } else { 0 },
        b_center: if request.center != 0 { 1 } else { 0 },
        b_handle_ampersand: if request.amp != 0 { 1 } else { 0 },
        visible_source_characters: visible,
        mode: if request.wrap != 0 { 1 } else { 0 },
    };
    let mut layout = ptr::null_mut();
    let create = unsafe {
        create_canvas_text_layout(access, device, &native_request, &mut layout)
    };
    let create = match create {
        Ok(value) => value,
        Err(error) => {
            if !layout.is_null() {
                unsafe { destroy_canvas_text_layout(access, device, layout)?; }
            }
            return Err(error);
        }
    };
    let attempt = (|| -> RuntimeResult<Option<(i32, i32)>> {
        if !create || layout.is_null() {
            return Ok(None);
        }
        let mut width = 0;
        let mut height = 0;
        let measured = unsafe {
            measure_canvas_text(access, device, layout, &mut width, &mut height)?
        };
        if !measured {
            Ok(None)
        } else if request.flags & crate::canvas_text::PF_INVISIBLE != 0 {
            Ok(Some((width, height)))
        } else {
            let drawn = unsafe { draw_canvas_text(access, device, frame, layout)? };
            Ok(drawn.then_some((width, height)))
        }
    })();
    let destroy = if layout.is_null() {
        Ok(())
    } else {
        unsafe { destroy_canvas_text_layout(access, device, layout) }
    };
    match (attempt, destroy) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(result), Ok(())) => Ok(result),
    }
}
/// Exits the provider while retaining the reflected object for possible reinitialization.
///
/// # Safety
/// `device` must be a live canonical RenderDevice record.
pub(crate) unsafe fn exit(access: CoreAccess<'_>, device: *mut URenderDevice) -> RuntimeResult<()> {
    unsafe { destroy(access, device) }
}

#[cfg(feature = "renderer-wgpu")]
impl CorePolicy<'_> {
    fn scene_fields(&self) -> RuntimeResult<()> {
        let frame=self.call.borrow().frame;
        if frame.is_null() { return Err(RuntimeError::new("Canonical SetSceneNode has no FSceneNode")); }
        let (vp,fx,fy,effective,coords,uncoords)=unsafe {
            ((*frame).viewport.cast::<UViewport>(),(*frame).fx,(*frame).fy,(*frame).effective_fov_angle,(*frame).coords,(*frame).uncoords)
        };
        if vp.is_null() { return Err(RuntimeError::new("Canonical FSceneNode has no viewport")); }
        let actor=unsafe { (*vp).actor.cast::<hp2_abi::engine::generated::APlayerPawn>() };
        if actor.is_null() { return Err(RuntimeError::new("Canonical scene viewport has no player pawn")); }
        let rend_map=unsafe { (*actor).rend_map };
        let scene=hp2_wgpu::SceneFrame {
            coords_origin:[coords.origin.x,coords.origin.y,coords.origin.z],
            coords_x_axis:[coords.x_axis.x,coords.x_axis.y,coords.x_axis.z],
            coords_y_axis:[coords.y_axis.x,coords.y_axis.y,coords.y_axis.z],
            coords_z_axis:[coords.z_axis.x,coords.z_axis.y,coords.z_axis.z],
            uncoords_origin:[uncoords.origin.x,uncoords.origin.y,uncoords.origin.z],
            uncoords_x_axis:[uncoords.x_axis.x,uncoords.x_axis.y,uncoords.x_axis.z],
            uncoords_y_axis:[uncoords.y_axis.x,uncoords.y_axis.y,uncoords.y_axis.z],
            uncoords_z_axis:[uncoords.z_axis.x,uncoords.z_axis.y,uncoords.z_axis.z],
            fx,fy,effective_fov_angle:effective,viewport_fov_angle:unsafe{(*actor).base.fov_angle},
            is_ortho:matches!(rend_map,13|14|15),
            is_wireframe_zfar:self.access.process_mode().is_editor&&rend_map==1,
            gamma:self.read_f32(unsafe { ptr::addr_of!((*self.device).gamma) }),
            light_color_intensity:0.0,light_map_intensity:0.0,
        };
        self.call.borrow_mut().scene=Some(scene);
        Ok(())
    }
    fn wire(&self) -> RuntimeResult<bool> {
        let frame=self.call.borrow().frame;
        if frame.is_null(){return Ok(false);}
        let vp=unsafe{(*frame).viewport.cast::<UViewport>()};
        if vp.is_null(){return Ok(false);}
        let actor=unsafe{(*vp).actor.cast::<hp2_abi::engine::generated::APlayerPawn>()};
        Ok(!actor.is_null()&&unsafe{(*actor).rend_map==1})
    }
    fn scene_update(&self, update:impl FnOnce(&mut hp2_wgpu::SceneFrame))->RuntimeResult<()>{
        let mut c=self.call.borrow_mut();
        update(c.scene.as_mut().ok_or_else(||RuntimeError::new("Canonical scene update preceded SceneFields"))?);
        Ok(())
    }
    fn scene_read(&self, read:impl FnOnce(&hp2_wgpu::SceneFrame)->i64)->RuntimeResult<i64>{
        let c=self.call.borrow();
        Ok(read(c.scene.as_ref().ok_or_else(||RuntimeError::new("Canonical scene read preceded SceneFields"))?))
    }
    fn draw_count(&self)->RuntimeResult<i32>{
        let d=self.device;
        Ok(unsafe{match self.action{
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetSceneNode=>(*d).set_scene_node_call_count,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles|hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody=>(*d).draw_triangles_call_count,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTile=>(*d).draw_tile_call_count,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DLine=>(*d).draw2_d_line_call_count,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DPoint=>(*d).draw2_d_point_call_count,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface|hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ComplexBody=>(*d).draw_complex_surface_call_count,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::EndFlash=>(*d).end_flash_draw_count,
            _=>0,
        }})
    }
    fn set_draw_count(&self,count:i32)->RuntimeResult<()>{
        let d=self.device;
        unsafe{match self.action{
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetSceneNode=>ptr::addr_of_mut!((*d).set_scene_node_call_count).write_unaligned(count),
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles|hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody=>ptr::addr_of_mut!((*d).draw_triangles_call_count).write_unaligned(count),
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTile=>ptr::addr_of_mut!((*d).draw_tile_call_count).write_unaligned(count),
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DLine=>ptr::addr_of_mut!((*d).draw2_d_line_call_count).write_unaligned(count),
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DPoint=>ptr::addr_of_mut!((*d).draw2_d_point_call_count).write_unaligned(count),
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface|hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ComplexBody=>ptr::addr_of_mut!((*d).draw_complex_surface_call_count).write_unaligned(count),
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::EndFlash=>ptr::addr_of_mut!((*d).end_flash_draw_count).write_unaligned(count),
            _=>{},
        }}
        Ok(())
    }
    fn log_draw_counts(&self)->RuntimeResult<()>{
        let d=self.device;
        let c=unsafe{((*d).set_scene_node_call_count,(*d).draw2_d_line_call_count,(*d).draw_tile_call_count,(*d).draw2_d_point_call_count,(*d).draw_triangles_call_count,(*d).draw_complex_surface_call_count)};
        self.log("Init",&format!("WgpuDrv: SetSceneNode calls={} Draw2DLine calls={} DrawTile calls={} Draw2DPoint calls={} DrawTriangles calls={} DrawComplexSurface calls={}",c.0,c.1,c.2,c.3,c.4,c.5))
    }
    fn log_first_draw(&self)->RuntimeResult<()>{
        match self.action{
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetSceneNode=>{let s=self.call.borrow().scene.ok_or_else(||RuntimeError::new("Missing scene state"))?;self.log("Init",&format!("WgpuDrv: SetSceneNode first call: is_ortho={} fx={} fy={} effective_fov={}",s.is_ortho,s.fx,s.fy,s.effective_fov_angle))}
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles|hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody=>{let c=self.call.borrow();let f=if c.info.is_null(){-1}else{unsafe{(*c.info).format}};self.log("Init",&format!("WgpuDrv: DrawTriangles first call: OutVertexCount={} CacheID={} Format={}",c.output_count,c.cache_id,f))}
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTile=>{let c=self.call.borrow();self.log("Init",&format!("WgpuDrv: DrawTile first call: X={} Y={} XL={} YL={} CacheID={}",c.tile[0],c.tile[1],c.tile[2],c.tile[3],c.cache_id))}
            _=>self.log("Init","WgpuDrv: first draw call"),
        }
    }
    fn read_index(&self,index:usize)->RuntimeResult<u16>{
        let c=self.call.borrow();
        if c.indices.is_null()||index>=usize::try_from(c.num_indices.max(0)).unwrap_or(0){return Err(RuntimeError::new("Canonical triangle index span is invalid"));}
        Ok(unsafe{c.indices.add(index).read_unaligned()})
    }
    fn point_exists(&self,index:usize)->RuntimeResult<bool>{
        let c=self.call.borrow();
        if c.points.is_null()||index>=usize::try_from(c.num_points.max(0)).unwrap_or(0){return Ok(false);}
        Ok(!unsafe{c.points.add(index).read_unaligned()}.is_null())
    }
    fn allocate_vertices(&self,count:usize,indexed:bool)->RuntimeResult<()>{
        let action=self.action;
        {
            let mut c=self.call.borrow_mut();
            c.output_count=count;c.indexed=indexed;
            match action{
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles=>c.vertices.resize_with(count,||unsafe{std::mem::zeroed()}),
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface=>c.complex_vertices.resize_with(count,||unsafe{std::mem::zeroed()}),
                _=>return Err(RuntimeError::new("Vertex allocation occurred outside a draw policy")),
            }
        }
        let nested=self.clone();
        match action{
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTriangles=>{with_policy(nested,hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody)?;}
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface=>{with_policy(nested,hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ComplexBody)?;}
            _=>unreachable!(),
        }
        Ok(())
    }
    fn fill_vertex(&self,out:usize,index:usize)->RuntimeResult<()>{
        let mut c=self.call.borrow_mut();
        match self.action{
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody=>{
                if out>=c.vertices.len()||c.points.is_null(){return Err(RuntimeError::new("Canonical triangle output vertex is out of bounds"));}
                let p=unsafe{c.points.add(index).read_unaligned()};
                if p.is_null(){return Err(RuntimeError::new("Canonical triangle input vertex is null"));}
                let p=unsafe{p.read_unaligned()};
                c.vertices[out]=hp2_wgpu::draw::GouraudVertexInput{coords:[p.base.base.base.point.x,p.base.base.base.point.y,p.base.base.base.point.z],tex_coords:[p.u,p.v],light_color:[p.base.light.x,p.base.light.y,p.base.light.z,p.base.light.w],fog_color:[p.base.fog.x,p.base.fog.y,p.base.fog.z,p.base.fog.w]};
            }
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ComplexBody=>{
                if out>=c.complex_vertices.len()||c.current_poly.is_null(){return Err(RuntimeError::new("Canonical complex vertex index is invalid"));}
                let points=unsafe{ptr::addr_of!((*c.current_poly).pts).cast::<*mut c_void>()};
                let transform=unsafe{points.add(index).read_unaligned().cast::<hp2_abi::engine::generated::FTransform>()};
                if transform.is_null(){return Err(RuntimeError::new("Canonical complex transform is null"));}
                let point=unsafe{transform.read_unaligned()}.base.point;
                c.complex_vertices[out]=hp2_wgpu::draw::ComplexVertexInput{coords:[point.x,point.y,point.z]};
            }
            _=>return Err(RuntimeError::new("Vertex fill occurred outside a draw body")),
        }
        Ok(())
    }
    fn surface_info_ptr(&self, light:bool, fog:bool)->RuntimeResult<*mut FTextureInfo>{
        let surface=self.call.borrow().surface;
        if surface.is_null(){return Err(RuntimeError::new("Canonical complex surface is null"));}
        let info=unsafe{if light{(*surface).light_map}else if fog{(*surface).fog_map}else{(*surface).texture}}.cast::<FTextureInfo>();
        if info.is_null(){return Err(RuntimeError::new("Canonical complex surface texture is null"));}
        Ok(info)
    }
    fn surface_field_ptr(&self,f:impl FnOnce(&FSurfaceInfo)->*mut c_void)->RuntimeResult<*mut c_void>{
        let surface=self.call.borrow().surface;
        if surface.is_null(){return Err(RuntimeError::new("Canonical complex surface is null"));}
        Ok(unsafe{f(&*surface)})
    }
    fn upload_surface_texture(&self,diffuse:bool,fog:bool)->RuntimeResult<u64>{
        let info=self.surface_info_ptr(!diffuse&&!fog,fog)?;
        {
            let mut c=self.call.borrow_mut();
            c.info=info;
            c.upload_poly_flags=if diffuse{c.poly_flags}else{0};
        }
        let id=self.upload_for_draw()?;
        let mut c=self.call.borrow_mut();
        if diffuse{c.diffuse_cache_id=id;}else if fog{c.fog_cache_id=id;}else{c.light_cache_id=id;}
        Ok(id)
    }
    fn fog_mip(&self)->RuntimeResult<bool>{
        let info=self.surface_info_ptr(false,true)?;
        let mip=unsafe{(*info).mips[0].cast::<FMipmapBase>()};
        self.call.borrow_mut().current_mip=mip;
        Ok(!mip.is_null())
    }
    fn surface_axes(&self)->RuntimeResult<()>{
        let facet=self.call.borrow().facet;
        if facet.is_null(){return Err(RuntimeError::new("Canonical surface facet is null"));}
        let coords=unsafe{(*facet).map_coords};
        let dot=hp2_core::math::vector_dot;
        let mut c=self.call.borrow_mut();
        c.x_axis=[coords.x_axis.x,coords.x_axis.y,coords.x_axis.z,dot(coords.x_axis,coords.origin)];
        c.y_axis=[coords.y_axis.x,coords.y_axis.y,coords.y_axis.z,dot(coords.y_axis,coords.origin)];
        Ok(())
    }
    fn first_poly(&self)->RuntimeResult<bool>{
        let facet=self.call.borrow().facet;
        if facet.is_null(){return Err(RuntimeError::new("Canonical surface facet is null"));}
        let poly=unsafe{(*facet).polys.cast::<FSavedPoly>()};
        self.call.borrow_mut().current_poly=poly;
        Ok(!poly.is_null())
    }
    fn next_poly(&self)->RuntimeResult<bool>{
        let poly=self.call.borrow().current_poly;
        if poly.is_null(){return Ok(false);}
        let next=unsafe{(*poly).next.cast::<FSavedPoly>()};
        self.call.borrow_mut().current_poly=next;
        Ok(!next.is_null())
    }
    fn light_material(&self)->RuntimeResult<()>{
        let info=self.surface_info_ptr(true,false)?;
        let i=unsafe{&*info};
        let layer=hp2_wgpu::ComplexTextureLayer{cache_id:self.call.borrow().light_cache_id,u_scale:i.u_scale,v_scale:i.v_scale,pan_x:i.pan.x,pan_y:i.pan.y};
        self.call.borrow_mut().light_material=layer;
        Ok(())
    }
    fn fog_material(&self)->RuntimeResult<()>{
        let info=self.surface_info_ptr(false,true)?;
        let i=unsafe{&*info};
        let layer=hp2_wgpu::ComplexTextureLayer{cache_id:self.call.borrow().fog_cache_id,u_scale:i.u_scale,v_scale:i.v_scale,pan_x:i.pan.x,pan_y:i.pan.y};
        self.call.borrow_mut().fog_material=layer;
        Ok(())
    }
    fn read_texture_alpha(&self)->RuntimeResult<()>{
        let info=self.texture_info()?;
        let texture=unsafe{(*info).texture.cast::<UTexture>()};
        if texture.is_null(){return Err(RuntimeError::new("Canonical texture alpha requested without UTexture"));}
        self.call.borrow_mut().texture_alpha=unsafe{(*texture).alpha};
        Ok(())
    }
    fn read_texture_material(&self)->RuntimeResult<()>{
        let info=if self.call.borrow().surface.is_null(){self.texture_info()?}else{self.surface_info_ptr(false,false)?};
        let texture=unsafe{(*info).texture.cast::<UTexture>()};
        if texture.is_null(){return Err(RuntimeError::new("Canonical texture material requested without UTexture"));}
        let t=unsafe{&*texture};
        let mut c=self.call.borrow_mut();c.diffuse=t.diffuse;c.specular=t.specular;c.texture_alpha=t.alpha;
        Ok(())
    }
    fn submit(&self)->RuntimeResult<bool>{
        let action=self.action;
        let d=self.device;
        let c=self.call.borrow();
        self.with_resources(|resources|{
            let Some(created)=resources.created.as_ref() else{return Err(RuntimeError::new("Canonical WGPU device is unavailable"));};
            match action{
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::SetSceneNode=>{
                    let scene=c.scene.ok_or_else(||RuntimeError::new("SetSceneNode has no scene state"))?;
                    resources.loop_state.set_scene_node(&created.device,&created.queue,&scene);
                }
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::TrianglesBody=>{
                    let info=c.info;
                    let (u_scale,v_scale,texture)=unsafe{((*info).u_scale,(*info).v_scale,(*info).texture.cast::<UTexture>())};
                    let (alpha,diffuse)=if texture.is_null(){(None,None)}else{(Some(c.texture_alpha),Some(c.diffuse))};
                    resources.loop_state.queue_triangles(hp2_wgpu::TrianglesDrawRequest{cache_id:c.cache_id,poly_flags:c.poly_flags,u_scale,v_scale,texture_alpha:alpha,diffuse},&c.vertices);
                }
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawTile=>{
                    let info=c.info;let (u_scale,v_scale,texture)=unsafe{((*info).u_scale,(*info).v_scale,(*info).texture.cast::<UTexture>())};
                    let f=c.frame;if f.is_null(){return Err(RuntimeError::new("DrawTile has no FSceneNode"));}
                    resources.loop_state.queue_tile(hp2_wgpu::TileDrawRequest{cache_id:c.cache_id,x:c.tile[0],y:c.tile[1],xl:c.tile[2],yl:c.tile[3],u:c.tile[4],v:c.tile[5],ul:c.tile[6],vl:c.tile[7],z:c.tile[8],color:c.color,poly_flags:c.poly_flags,u_scale,v_scale,texture_alpha:(!texture.is_null()).then_some(c.texture_alpha),fx2:unsafe{(*f).fx2},fy2:unsafe{(*f).fy2}});
                }
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DLine=>{
                    let f=c.frame;if f.is_null(){return Err(RuntimeError::new("Draw2DLine has no FSceneNode"));}
                    resources.loop_state.queue_line(c.p1,c.p2,unsafe{(*f).fx2},unsafe{(*f).fy2},[c.color[0],c.color[1],c.color[2],1.0],c.line_flags);
                }
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::Draw2DPoint=>{
                    let f=c.frame;if f.is_null(){return Err(RuntimeError::new("Draw2DPoint has no FSceneNode"));}
                    resources.loop_state.queue_point(c.tile[0],c.tile[1],c.tile[2],c.tile[3],c.tile[4],unsafe{(*f).fx2},unsafe{(*f).fy2},[c.color[0],c.color[1],c.color[2]],c.line_flags);
                }
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::EndFlash=>{
                    let fog=unsafe{ptr::addr_of!((*d).current_flash_fog).read_unaligned()};
                    return Ok(resources.loop_state.queue_end_flash([fog.x,fog.y,fog.z,c.texture_alpha]));
                }
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ClearZ=>resources.loop_state.clear_z(created),
                hp2_wgpu::driver_policy::Hp2WgpuDriverAction::ComplexBody=>{
                    let info=self.surface_info_ptr(false,false)?;
                    let (diffuse_scale,diffuse_pan)=unsafe{((*info).u_scale,(*info).v_scale)};
                    let pan=unsafe{(*info).pan};
                    let req=hp2_wgpu::ComplexSurfaceDrawRequest{diffuse:hp2_wgpu::ComplexTextureLayer{cache_id:c.diffuse_cache_id,u_scale:diffuse_scale,v_scale:diffuse_pan,pan_x:pan.x,pan_y:pan.y},
                        light_map:c.has_light.then_some(c.light_material),fog_map:c.has_fog.then_some(c.fog_material),poly_flags:c.poly_flags,
                        texture_fields:c.has_texture.then_some((c.diffuse,c.specular,c.texture_alpha)),c_alpha:c.c_alpha,x_axis:c.x_axis,y_axis:c.y_axis};
                    resources.loop_state.queue_complex_surface(req,&c.complex_vertices);
                }
                _=>return Err(RuntimeError::new("Canonical render Submit called outside an implemented draw action")),
            }
            Ok(false)
        })
    }
    fn texture_info(&self) -> RuntimeResult<*mut FTextureInfo> {
        let info = self.call.borrow().info;
        if info.is_null() { Err(RuntimeError::new("Canonical texture operation has no FTextureInfo")) } else { Ok(info) }
    }
    fn texture_field_i32(&self, f: impl FnOnce(&FTextureInfo) -> i32) -> RuntimeResult<i32> {
        let info = self.texture_info()?;
        Ok(unsafe { f(&*info) })
    }
    fn texture_field_ptr(&self, f: impl FnOnce(&FTextureInfo) -> *mut c_void) -> RuntimeResult<*mut c_void> {
        let info = self.texture_info()?;
        Ok(unsafe { f(&*info) })
    }
    fn fix_cache(&self) -> RuntimeResult<u64> {
        let info = self.texture_info()?;
        let (cache, raw_format, flags) = unsafe {
            ((*info).cache_id, (*info).format, self.call.borrow().upload_poly_flags)
        };
        let raw_format = u8::try_from(raw_format)
            .map_err(|_| RuntimeError::new("Canonical texture format is invalid"))?;
        let format = hp2_wgpu::texture::TextureFormat::from_raw(raw_format)
            .ok_or_else(|| RuntimeError::new("Canonical texture format is invalid"))?;
        let cache = hp2_wgpu::texture::fix_cache_id(cache, format, flags);
        self.call.borrow_mut().cache_id = cache;
        Ok(cache)
    }
    fn cached(&self) -> RuntimeResult<bool> {
        let id = self.call.borrow().cache_id;
        self.with_resources(|resources| Ok(resources.loop_state.is_texture_cached(id)))
    }
    fn dirty(&self) -> RuntimeResult<bool> {
        let info = self.texture_info()?;
        Ok(unsafe { (*info).b_realtime_changed() })
    }
    fn reset_dirty(&self) -> RuntimeResult<()> {
        let info = self.texture_info()?;
        unsafe { (*info).set_b_realtime_changed(false); }
        Ok(())
    }
    fn texture_present(&self) -> RuntimeResult<bool> {
        // `DrawComplexSurface`'s `Texture` leaf reads `Surface.Texture`; every
        // other action reads the `FTextureInfo` passed to that action, including
        // the nested uploads that reuse this adapter while a surface is set.
        let c = self.call.borrow();
        let reads_surface = matches!(
            self.action,
            hp2_wgpu::driver_policy::Hp2WgpuDriverAction::DrawComplexSurface
        ) && !c.surface.is_null();
        let info = if reads_surface {
            unsafe { (*c.surface).texture.cast::<FTextureInfo>() }
        } else {
            c.info
        };
        if info.is_null() { return Ok(false); }
        Ok(unsafe { !(*info).texture.is_null() })
    }
    fn select_mip(&self, index: usize) -> RuntimeResult<bool> {
        let info = self.texture_info()?;
        let (count, mip) = unsafe {
            let count = (*info).num_mips;
            if index >= 12 || index >= usize::try_from(count.max(0)).unwrap_or(0) {
                (count, ptr::null_mut())
            } else {
                (count, (*info).mips[index].cast::<FMipmapBase>())
            }
        };
        let _ = count;
        self.call.borrow_mut().current_mip = mip;
        Ok(!mip.is_null())
    }
    fn mip_data(&self) -> RuntimeResult<bool> {
        let mip = self.call.borrow().current_mip;
        if mip.is_null() { return Ok(false); }
        Ok(unsafe { !(*mip).data_ptr.is_null() })
    }
    fn mip_dimension(&self, horizontal: bool) -> RuntimeResult<i32> {
        let mip = self.call.borrow().current_mip;
        if mip.is_null() { return Err(RuntimeError::new("Canonical texture mip is null")); }
        Ok(unsafe { if horizontal { (*mip).u_size } else { (*mip).v_size } })
    }
    fn alloc_mips(&self, count: usize) -> RuntimeResult<()> {
        if count > 12 { return Err(RuntimeError::new("Canonical compressed texture mip count exceeds FTextureInfo")); }
        self.call.borrow_mut().raw_mips = vec![RawMip::default(); count];
        Ok(())
    }
    fn write_mip(&self, index: usize, byte_count: usize) -> RuntimeResult<()> {
        let mip = self.call.borrow().current_mip;
        if mip.is_null() { return Err(RuntimeError::new("Canonical compressed texture mip is null")); }
        let (width, height, data) = unsafe { ((*mip).u_size,(*mip).v_size,(*mip).data_ptr.cast::<u8>()) };
        if width <= 0 || height <= 0 || data.is_null() { return Err(RuntimeError::new("Canonical compressed texture mip data is invalid")); }
        let blocks_wide = (width as usize).checked_add(3)
            .ok_or_else(|| RuntimeError::new("Canonical compressed mip width overflow"))? / 4;
        let blocks_high = (height as usize).checked_add(3)
            .ok_or_else(|| RuntimeError::new("Canonical compressed mip height overflow"))? / 4;
        let expected = blocks_wide.checked_mul(blocks_high).and_then(|blocks| blocks.checked_mul(8))
            .ok_or_else(|| RuntimeError::new("Canonical compressed mip byte count overflow"))?;
        if byte_count != expected {
            return Err(RuntimeError::new("Canonical compressed mip byte count is invalid"));
        }
        let mut call = self.call.borrow_mut();
        let out = call.raw_mips.get_mut(index).ok_or_else(|| RuntimeError::new("Canonical compressed mip index is invalid"))?;
        *out = RawMip { width: width as u32, height: height as u32, data, len: expected };
        Ok(())
    }
    fn upload_compressed(&self) -> RuntimeResult<bool> {
        let info = self.texture_info()?;
        let format = u8::try_from(unsafe { (*info).format })
            .ok()
            .and_then(hp2_wgpu::texture::TextureFormat::from_raw)
            .ok_or_else(|| RuntimeError::new("Canonical compressed texture format is invalid"))?;
        if format != hp2_wgpu::texture::TextureFormat::Dxt1 {
            return Err(RuntimeError::new("Uncompressed texture reached the compressed upload path"));
        }
        let (cache_id, flags, srgb) = {
            let call = self.call.borrow();
            (call.cache_id, call.sampler_flags, self.read_i32(unsafe { ptr::addr_of!((*self.device).use_srgb_textures) }) != 0)
        };
        let mips = {
            let call = self.call.borrow();
            call.raw_mips.iter().map(|mip| {
                if mip.data.is_null() { return Err(RuntimeError::new("Canonical compressed mip has null data")); }
                let data = unsafe { std::slice::from_raw_parts(mip.data, mip.len) };
                Ok(hp2_wgpu::texture::CompressedMipLevel { width:mip.width, height:mip.height, data })
            }).collect::<RuntimeResult<Vec<_>>>()?
        };
        let width = unsafe { (*info).u_size };
        let height = unsafe { (*info).v_size };
        if width <= 0 || height <= 0 { return Err(RuntimeError::new("Canonical compressed texture dimensions are invalid")); }
        let uploaded = self.with_resources(|resources| {
            let created = resources.created.as_ref().ok_or_else(|| RuntimeError::new("Canonical WGPU device is unavailable"))?;
            Ok(resources.loop_state.upload_compressed_texture(
                created, cache_id, width as u32, height as u32, &mips,
                hp2_wgpu::texture::TextureOptions { srgb, sampler_key: hp2_wgpu::texture::SamplerKey::from_bits(flags) },
            ))
        })?;
        if !uploaded { return Err(RuntimeError::new("Canonical compressed texture upload failed")); }
        Ok(true)
    }
    fn upload_uncompressed(&self, byte_count: usize, masked: bool) -> RuntimeResult<()> {
        let info = self.texture_info()?;
        let mip = self.call.borrow().current_mip;
        if mip.is_null() { return Err(RuntimeError::new("Canonical texture mip is null")); }
        let (width, height, data) = unsafe { ((*mip).u_size,(*mip).v_size,(*mip).data_ptr.cast::<u8>()) };
        if width <= 0 || height <= 0 || data.is_null() { return Err(RuntimeError::new("Canonical texture mip data is invalid")); }
        let (cache_id, flags) = { let call=self.call.borrow(); (call.cache_id,call.sampler_flags) };
        let raw_format = u8::try_from(unsafe { (*info).format })
            .map_err(|_| RuntimeError::new("Canonical texture format is invalid"))?;
        let format = hp2_wgpu::texture::TextureFormat::from_raw(raw_format)
            .ok_or_else(|| RuntimeError::new("Canonical texture format is invalid"))?;
        if format == hp2_wgpu::texture::TextureFormat::Dxt1 {
            return Err(RuntimeError::new("Compressed texture reached the uncompressed upload path"));
        }
        let pixel_count = (width as usize).checked_mul(height as usize)
            .ok_or_else(|| RuntimeError::new("Canonical texture dimensions overflow"))?;
        let bytes_per_pixel = match format {
            hp2_wgpu::texture::TextureFormat::P8 => 1,
            hp2_wgpu::texture::TextureFormat::Rgba7 | hp2_wgpu::texture::TextureFormat::Rgba8 => 4,
            hp2_wgpu::texture::TextureFormat::Rgb8 => 3,
            hp2_wgpu::texture::TextureFormat::Rgb16 => 2,
            hp2_wgpu::texture::TextureFormat::Dxt1 => unreachable!(),
        };
        let expected_bytes = pixel_count.checked_mul(bytes_per_pixel)
            .filter(|&length| isize::try_from(length).is_ok())
            .ok_or_else(|| RuntimeError::new("Canonical texture mip byte count overflow"))?;
        let rgba_byte_count = pixel_count.checked_mul(4)
            .filter(|&length| isize::try_from(length).is_ok())
            .ok_or_else(|| RuntimeError::new("Canonical RGBA texture size overflow"))?;
        if byte_count != expected_bytes {
            return Err(RuntimeError::new("Canonical texture mip byte count is invalid"));
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, expected_bytes) };
        let rgba = match format {
            hp2_wgpu::texture::TextureFormat::P8 => {
                let palette = unsafe { (*info).palette.cast::<hp2_wgpu::texture::FColor>() };
                if palette.is_null() { return Err(RuntimeError::new("Canonical P8 texture has no palette")); }
                let palette = unsafe { &*palette.cast::<[hp2_wgpu::texture::FColor; 256]>() };
                let mode = if masked { hp2_wgpu::texture::P8MaskMode::Masked } else { hp2_wgpu::texture::P8MaskMode::Unmasked };
                hp2_wgpu::texture::p8_to_rgba8_bytes(bytes,palette,mode)
            }
            hp2_wgpu::texture::TextureFormat::Rgba7 | hp2_wgpu::texture::TextureFormat::Rgba8 => bytes.to_vec(),
            hp2_wgpu::texture::TextureFormat::Rgb8 => hp2_wgpu::texture::rgb8_to_rgba8(bytes),
            hp2_wgpu::texture::TextureFormat::Rgb16 => hp2_wgpu::texture::rgb16_to_rgba8(bytes),
            hp2_wgpu::texture::TextureFormat::Dxt1 => unreachable!(),
        };
        if rgba.len() != rgba_byte_count {
            return Err(RuntimeError::new("Canonical RGBA texture conversion has an invalid size"));
        }
        let srgb = self.read_i32(unsafe { ptr::addr_of!((*self.device).use_srgb_textures) }) != 0
            && !matches!(format,hp2_wgpu::texture::TextureFormat::Rgba7|hp2_wgpu::texture::TextureFormat::Rgb16);
        self.with_resources(|resources| {
            let created=resources.created.as_ref().ok_or_else(|| RuntimeError::new("Canonical WGPU device is unavailable"))?;
            resources.loop_state.upload_texture(created,cache_id,width as u32,height as u32,&rgba,
                hp2_wgpu::texture::TextureOptions { srgb, sampler_key:hp2_wgpu::texture::SamplerKey::from_bits(flags) });
            Ok(())
        })
    }
    fn upload_for_draw(&self) -> RuntimeResult<u64> {
        let mut nested=self.clone();
        let id=with_policy(nested.clone(),hp2_wgpu::driver_policy::Hp2WgpuDriverAction::UploadTexture)?;
        nested.call.borrow_mut().cache_id=id as u64;
        Ok(id as u64)
    }
    fn with_resources<T>(&self, f: impl FnOnce(&mut RenderResources) -> RuntimeResult<T>) -> RuntimeResult<T> {
        let mut lease = self.runtime.lease()?;
        f(lease.resources())
    }

    fn read_u8(&self, field: *const u8) -> u8 { unsafe { field.read_unaligned() } }
    fn read_i32(&self, field: *const i32) -> i32 { unsafe { field.read_unaligned() } }
    fn read_f32(&self, field: *const f32) -> f32 { unsafe { field.read_unaligned() } }

    fn viewport_width(&self) -> i32 {
        if self.viewport.is_null() { self.width.max(1) }
        else { unsafe { ptr::addr_of!((*self.viewport).base.size_x).read_unaligned() } }
    }
    fn viewport_height(&self) -> i32 {
        if self.viewport.is_null() { self.height.max(1) }
        else { unsafe { ptr::addr_of!((*self.viewport).base.size_y).read_unaligned() } }
    }
    fn viewport_dimension(&self, leaf: hp2_wgpu::driver_policy::Hp2WgpuDriverLeaf) -> RuntimeResult<i64> {
        use hp2_wgpu::driver_policy::Hp2WgpuDriverLeaf as Leaf;
        match leaf {
            Leaf::Drawable => {
                #[cfg(feature = "sdl")]
                if let Some(window) = self.window()? {
                    let (mut width, mut height) = (0, 0);
                    #[cfg(target_os = "macos")]
                    {
                        let has_view = self.with_resources(|resources| Ok(resources.metal_view.is_some()))?;
                        if has_view { unsafe { SDL_Metal_GetDrawableSize(window.as_ptr(), &mut width, &mut height); } }
                    }
                    #[cfg(not(target_os = "macos"))]
                    unsafe { SDL_GL_GetDrawableSize(window.as_ptr(), &mut width, &mut height); }
                    self.pixel_width.set(width);
                    self.pixel_height.set(height);
                }
                Ok(0)
            }
            Leaf::WindowSize => {
                #[cfg(feature = "sdl")]
                if let Some(window) = self.window()? {
                    let (mut width, mut height) = (0, 0);
                    unsafe { SDL_GetWindowSize(window.as_ptr(), &mut width, &mut height); }
                    self.logical_width.set(width);
                    self.logical_height.set(height);
                }
                Ok(0)
            }
            Leaf::PixelWidth => Ok(i64::from(self.pixel_width.get().max(1))),
            Leaf::PixelHeight => Ok(i64::from(self.pixel_height.get().max(1))),
            Leaf::LogicalHeight => Ok(i64::from(self.logical_height.get().max(1))),
            Leaf::LogicalFallback => {
                self.pixel_width.set(self.viewport_width().max(1));
                self.pixel_height.set(self.viewport_height().max(1));
                Ok(0)
            }
            _ => Ok(0),
        }
    }
    fn client(&self) -> Option<*mut UClient> {
        if self.viewport.is_null() { return None; }
        let client = unsafe { ptr::addr_of!((*self.viewport).base.outer).read_unaligned().cast::<UClient>() };
        (!client.is_null()).then_some(client)
    }
    fn brightness(&self) -> f32 {
        self.client().map_or(1.0, |client| unsafe { ptr::addr_of!((*client).brightness).read_unaligned() })
    }
    fn log(&self, category: &str, message: &str) -> RuntimeResult<()> {
        let category = self.access.intern_str(category, hp2_core::name_owner::FindName::Add)?;
        let text = message.chars().map(u32::from).collect::<Vec<_>>();
        self.access.emit_log(category, &text)
    }
    fn log_adapter(&self) -> RuntimeResult<()> {
        let name = self.with_resources(|resources| Ok(resources.created.as_ref().map_or_else(|| "unknown adapter".to_owned(), |created| created.adapter_name.clone())))?;
        self.log("Log", &format!("WgpuDrv: selected adapter {name}"))
    }
    fn log_counts(&self) -> RuntimeResult<()> {
        let counts = self.with_resources(|resources| Ok(resources.loop_state.counts()))?;
        self.log(
            "Init",
            &format!(
                "WgpuDrv: frames presented={} occluded={} outdated={} other_failure={}",
                counts.presented, counts.occluded, counts.outdated, counts.other_failure,
            ),
        )
    }
    fn window(&self) -> RuntimeResult<Option<NonNull<c_void>>> {
        #[cfg(feature = "sdl")]
        {
            if self.viewport.is_null() { return Ok(None); }
            let window = unsafe { ptr::addr_of!((*self.viewport).sdl_window).read_unaligned() };
            if window.is_null() { return Ok(None); }
            return crate::platform_owner::sdl_window(self.access, self.viewport).map(Some);
        }
        #[cfg(not(feature = "sdl"))]
        { Ok(None) }
    }
    fn create_metal(&self) -> RuntimeResult<bool> {
        #[cfg(all(feature = "sdl", target_os = "macos"))]
        {
            let Some(window) = self.window()? else { return Ok(false); };
            let view = unsafe { SDL_Metal_CreateView(window.as_ptr()) };
            let Some(view) = NonNull::new(view) else { return Ok(false); };
            self.with_resources(|resources| {
                resources.metal_view = Some(OwnedMetalView(view));
                Ok(true)
            })
        }
        #[cfg(not(all(feature = "sdl", target_os = "macos")))]
        { Ok(false) }
    }
    fn create_device(&self) -> RuntimeResult<bool> {
        #[cfg(all(feature = "sdl", target_os = "macos"))]
        {
            let Some(backend) = hp2_wgpu::BackendChoice::from_ffi_discriminant(u32::from(self.read_u8(unsafe { ptr::addr_of!((*self.device).backend) }))) else {
                return Ok(false);
            };
            let view = self.with_resources(|resources| Ok(resources.metal_view.as_ref().map(|view| view.0.as_ptr())))?;
            let Some(view) = view else { return Ok(false); };
            let create_surface = move |instance: &_| unsafe {
                hp2_wgpu::macos::create_surface_from_ns_view(instance, view)
            };
            let created = hp2_wgpu::create_device_sync(backend, Some(create_surface));
            let Some(created) = created else { return Ok(false); };
            self.with_resources(|resources| {
                resources.created = Some(created);
                Ok(true)
            })
        }
        #[cfg(all(feature = "sdl", not(target_os = "macos")))]
        {
            let Some(backend)=hp2_wgpu::BackendChoice::from_ffi_discriminant(u32::from(self.read_u8(unsafe { ptr::addr_of!((*self.device).backend) }))) else {
                return Ok(false);
            };
            let Some(window)=self.window()? else{return Ok(false);};
            let Some(handles)=(unsafe{hp2_wgpu::sdl_surface::window_handles(window.as_ptr())}) else{return Ok(false);};
            let create_surface = move |instance: &_| unsafe {
                hp2_wgpu::sdl_surface::create_surface_from_handles(instance, handles)
            };
            let created = hp2_wgpu::create_device_sync(backend, Some(create_surface));
            let Some(created)=created else{return Ok(false);};
            self.with_resources(|resources|{resources.created=Some(created);Ok(true)})
        }
        #[cfg(not(feature = "sdl"))]
        { Ok(false) }
    }
    fn create_text(&self) -> RuntimeResult<bool> {
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let state = CoreTextState::new();
            let ready = state.ready;
            let reason = match state.status { 0 => "text.ready", 2 => "text.rasterizer_unavailable", _ => "text.fonts_unavailable" };
            self.with_resources(|resources| {
                resources.text.coretext = Some(state);
                Ok(ready)
            })?;
            self.log("Init", &format!("WgpuDrv: native text backend {} ({reason})", if ready { "ready" } else { "unavailable" }))?;
            Ok(ready)
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let mut host = hp2_render::native_text_layout_freetype_host::Hp2TextLayoutFreeTypeHost::new();
            let services = host.services();
            let status = hp2_render::native_text_layout_freetype::probe(&services).unwrap_or(u32::MAX);
            let ready = status == 0;
            let reason = match status {
                0 => "text.ready",
                1 => "text.fonts_unavailable",
                2 => "text.rasterizer_unavailable",
                _ => "text.internal_error",
            };
            let state = FreeTypeState { ready, layouts: HashMap::new() };
            self.with_resources(|resources| {
                resources.text.freetype = Some(state);
                Ok(())
            })?;
            self.log("Init", &format!(
                "WgpuDrv: native text backend {} ({reason})", if ready { "ready" } else { "unavailable" },
            ))?;
            Ok(ready)
        }
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        {
            self.log("Init", "WgpuDrv: native text backend unavailable (capability.compile_time_disabled)")?;
            Ok(false)
        }
    }
    fn destroy_text(&self) -> RuntimeResult<()> {
        self.with_resources(|resources| {
            #[cfg(all(feature = "text-coretext", target_os = "macos"))]
            { resources.text.coretext = None; }
            #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
            { resources.text.freetype = None; }
            Ok(())
        })
    }
    fn text_backend(&self) -> RuntimeResult<bool> {
        self.with_resources(|resources| {
            #[cfg(all(feature = "text-coretext", target_os = "macos"))]
            { return Ok(resources.text.coretext.as_ref().is_some_and(|state| state.ready)); }
            #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
            { return Ok(resources.text.freetype.as_ref().is_some_and(|state| state.ready)); }
            #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
            { let _ = resources; Ok(false) }
        })
    }
    fn text_leaf(
        &self,
        leaf: hp2_wgpu::driver_policy::Hp2WgpuDriverLeaf,
        a: i64,
        b: i64,
        x: f64,
    ) -> RuntimeResult<(i64, f64)> {
        use hp2_wgpu::driver_policy::{Hp2WgpuDriverAction as Action, Hp2WgpuDriverLeaf as Leaf};
        let value = match leaf {
            Leaf::ResetLayout => {
                let mut call = self.call.borrow_mut();
                call.text_layout = ptr::null_mut();
                if !call.text_layout_output.is_null() {
                    unsafe { call.text_layout_output.write(ptr::null_mut()); }
                }
                0
            }
            Leaf::TextBackend => i64::from(self.text_backend()?),
            Leaf::SafeOriginX => self.safe_text_origin(true)?,
            Leaf::SafeOriginY => self.safe_text_origin(false)?,
            Leaf::TextCreateScope => hp2_wgpu::driver_policy::run_with(
                self.clone(), Action::CreateCanvasTextLayoutBody,
            )?,
            Leaf::ProjectRequest => self.project_text_request()?,
            Leaf::CreateLayout => self.create_text_layout()?,
            Leaf::LayoutExists => i64::from(!self.call.borrow().text_layout.is_null()),
            Leaf::LayoutOrigin => self.set_text_layout_origin()?,
            Leaf::DestroyLayout => self.destroy_text_layout()?,
            Leaf::ResetMeasure => {
                let call = self.call.borrow();
                if !call.text_measure.is_null() { unsafe { call.text_measure.write(0); } }
                if !call.text_measure_height.is_null() { unsafe { call.text_measure_height.write(0); } }
                0
            }
            Leaf::MeasureLayout => self.measure_text_layout()?,
            Leaf::DrawBridge => i64::from(self.text_draw_bridge()?),
            Leaf::PrepareDraw => self.prepare_text_draw()?,
            Leaf::TextFields => self.read_text_fields()?,
            Leaf::DrawText => self.draw_text()?,
            Leaf::TextLogged => i64::from(TEXT_DRAW_LOGGED.load(Ordering::Relaxed)),
            Leaf::TextMarkLogged => {
                TEXT_DRAW_LOGGED.store(a != 0, Ordering::Relaxed);
                0
            }
            Leaf::TextLog => {
                let (glyphs, underlines, drawn) = {
                    let call = self.call.borrow();
                    (call.text_glyph_count, call.text_underline_count, call.text_drawn)
                };
                self.log("Dev", &format!(
                    "WgpuDrv: DrawCanvasText first call: glyphs={glyphs} underlines={underlines} result={}",
                    i32::from(drawn),
                ))?;
                0
            }
            Leaf::Frame => i64::from(!self.call.borrow().frame.is_null()),
            Leaf::IsLayout => i64::from(self.text_layout_exists()?),
            Leaf::TextDrawScope => hp2_wgpu::driver_policy::run_with(
                self.clone(), Action::DrawCanvasTextBody,
            )?,
            _ => return Err(RuntimeError::new("Unexpected canonical render text policy leaf")),
        };
        let _ = (b, x);
        Ok((value, 0.0))
    }
    fn project_text_request(&self) -> RuntimeResult<i64> {
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let request = self.call.borrow().text_request;
            if request.is_null() { return Ok(0); }
            let projected = unsafe { &*request };
            if projected.font.is_null() || projected.text.is_null() || projected.text_length < 0 {
                return Ok(0);
            }
            let clip_safe = |value: f32| {
                value.is_finite() && f64::from(value) >= f64::from(i32::MIN)
                    && f64::from(value) <= f64::from(i32::MAX)
            };
            if !clip_safe(projected.clip_x) || !clip_safe(projected.clip_y) {
                return Ok(0);
            }
            let font = unsafe { &*projected.font.cast::<UFont>() };
            if font.pages.num < 0 { return Ok(0); }
            let family = fstring_units(font.font_name)?;
            let name = self.access.name_text(i32::from_ne_bytes(font.base.name))?;
            let mut call = self.call.borrow_mut();
            call.text_name = name;
            call.text_family = family;
            call.text_core_request = Some(hp2_render::native_text_layout_coretext::Hp2TextLayoutCoreTextRequest {
                text: projected.text,
                length: projected.text_length,
                unit_bytes: 4,
                name: call.text_name.as_ptr().cast(),
                name_length: call.text_name.len(),
                family: call.text_family.as_ptr().cast(),
                family_length: call.text_family.len(),
                font: projected.font,
                font_height: font.font_height,
                page_count: font.pages.num,
                scale: projected.text_scale,
                space_x: projected.space_x,
                space_y: projected.space_y,
                clip_x: projected.clip_x,
                clip_y: projected.clip_y,
                color: [projected.color.x, projected.color.y, projected.color.z, projected.color.w],
                start_x: projected.start_x,
                start_y: projected.start_y,
                visible: projected.visible_source_characters,
                ampersand: u8::from(projected.b_handle_ampersand != 0),
                center: u8::from(projected.b_center != 0),
                wrapped: u8::from(projected.mode == 1),
                poly_flags: projected.poly_flags,
                clip: u8::from(projected.b_clip != 0),
            });
            Ok(1)
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let request = self.call.borrow().text_request;
            if request.is_null() { return Ok(0); }
            let projected = unsafe { &*request };
            if projected.font.is_null() || projected.text.is_null() || projected.text_length < 0 {
                return Ok(0);
            }
            let clip_safe = |value: f32| {
                value.is_finite() && f64::from(value) >= f64::from(i32::MIN)
                    && f64::from(value) <= f64::from(i32::MAX)
            };
            if !clip_safe(projected.clip_x) || !clip_safe(projected.clip_y) {
                return Ok(0);
            }
            let font = unsafe { &*projected.font.cast::<UFont>() };
            let page_count = match usize::try_from(font.pages.num) {
                Ok(count) => count,
                Err(_) => return Ok(0),
            };
            let family = fstring_units(font.font_name)?;
            let name = self.access.name_text(i32::from_ne_bytes(font.base.name))?;
            let mut call = self.call.borrow_mut();
            call.text_name = name;
            call.text_family = family;
            call.text_freetype_request = Some(hp2_render::native_text_layout_freetype::Hp2TextLayoutFreeTypeRequest {
                text: projected.text,
                length: projected.text_length,
                unit_bytes: 4,
                name: call.text_name.as_ptr().cast(),
                name_length: call.text_name.len(),
                family: call.text_family.as_ptr().cast(),
                family_length: call.text_family.len(),
                font_height: font.font_height,
                page_count,
                has_font: 1,
                scale: projected.text_scale,
                space_x: projected.space_x,
                space_y: projected.space_y,
                clip_x: projected.clip_x,
                clip_y: projected.clip_y,
                color: [projected.color.x, projected.color.y, projected.color.z, projected.color.w],
                start_x: projected.start_x,
                start_y: projected.start_y,
                visible: projected.visible_source_characters,
                amp: u32::from(projected.b_handle_ampersand != 0),
                center: u32::from(projected.b_center != 0),
                wrapped: u32::from(projected.mode == 1),
                poly_flags: projected.poly_flags,
                clip: u8::from(projected.b_clip != 0),
            });
            Ok(1)
        }
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { Ok(0) }
    }
    fn safe_text_origin(&self, x_axis: bool) -> RuntimeResult<i64> {
        let request = self.call.borrow().text_request;
        if request.is_null() { return Ok(0); }
        let origin = unsafe {
            if x_axis { (*request).origin_x } else { (*request).origin_y }
        };
        let safe = origin.is_finite() && f64::from(origin) >= f64::from(i32::MIN)
            && f64::from(origin) <= f64::from(i32::MAX);
        Ok(i64::from(safe))
    }
    fn create_text_layout(&self) -> RuntimeResult<i64> {
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let (request, output) = {
                let call = self.call.borrow();
                (call.text_core_request, call.text_layout_output)
            };
            let Some(request) = request else { return Ok(0); };
            self.with_resources(|resources| {
                let Some(state) = resources.text.coretext.as_mut().filter(|state| state.ready) else {
                    return Ok(0);
                };
                let services = hp2_render::native_text_layout_coretext_host::services();
                let layout = unsafe {
                    hp2_render::native_text_layout_coretext::create(
                        &services,
                        std::ptr::from_mut(&mut state.scratch).cast(),
                        &request,
                        font_page_height,
                    )
                };
                let Some(layout) = layout else { return Ok(0); };
                let mut layout = Box::new(layout);
                let mut record = Box::new(unsafe {
                    std::mem::MaybeUninit::<FCanvasTextLayout>::zeroed().assume_init()
                });
                record.magic = NATIVE_TEXT_LAYOUT_MAGIC;
                record.raw = (&mut *layout as *mut hp2_render::native_text_layout_coretext::Layout).cast();
                let address = (&mut *record as *mut FCanvasTextLayout) as usize;
                let owned = OwnedCanvasTextLayout { record, layout, draw_cache: None };
                state.layouts.insert(address, owned);
                let pointer = address as *mut FCanvasTextLayout;
                if !output.is_null() { unsafe { output.write(pointer); } }
                self.call.borrow_mut().text_layout = pointer;
                Ok(1)
            })
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let (request, output) = {
                let call = self.call.borrow();
                (call.text_freetype_request, call.text_layout_output)
            };
            let Some(request) = request else { return Ok(0); };
            self.with_resources(|resources| {
                let Some(state) = resources.text.freetype.as_mut().filter(|state| state.ready) else {
                    return Ok(0);
                };
                let mut host = Box::new(
                    hp2_render::native_text_layout_freetype_host::Hp2TextLayoutFreeTypeHost::new(),
                );
                freetype_font_pages(&mut host, request.font)?;
                let services = host.services();
                let layout = unsafe {
                    hp2_render::native_text_layout_freetype::create(&services, &request)
                };
                let Some(layout) = layout else { return Ok(0); };
                let mut layout = Box::new(layout);
                let mut record = Box::new(unsafe {
                    std::mem::MaybeUninit::<FCanvasTextLayout>::zeroed().assume_init()
                });
                record.magic = NATIVE_TEXT_LAYOUT_MAGIC;
                record.raw = (&mut *layout as *mut hp2_render::native_text_layout_freetype::Layout).cast();
                let address = (&mut *record as *mut FCanvasTextLayout) as usize;
                let pointer = address as *mut FCanvasTextLayout;
                state.layouts.insert(address, OwnedFreeTypeCanvasTextLayout {
                    record, layout, host, draw_cache: None,
                });
                if !output.is_null() { unsafe { output.write(pointer); } }
                self.call.borrow_mut().text_layout = pointer;
                Ok(1)
            })
        }
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { Ok(0) }
    }
    fn set_text_layout_origin(&self) -> RuntimeResult<i64> {
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let (pointer, request) = {
                let call = self.call.borrow();
                (call.text_layout, call.text_request)
            };
            if pointer.is_null() || request.is_null() { return Ok(0); }
            let origin = unsafe { ((*request).origin_x, (*request).origin_y) };
            self.with_resources(|resources| {
                let Some(state) = resources.text.coretext.as_mut() else { return Ok(0); };
                let Some(layout) = state.layouts.get_mut(&(pointer as usize)) else { return Ok(0); };
                layout.layout.set_origin(origin.0, origin.1);
                Ok(1)
            })
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let (pointer, request) = {
                let call = self.call.borrow();
                (call.text_layout, call.text_request)
            };
            if pointer.is_null() || request.is_null() { return Ok(0); }
            let origin = unsafe { ((*request).origin_x, (*request).origin_y) };
            self.with_resources(|resources| {
                let Some(state) = resources.text.freetype.as_mut() else { return Ok(0); };
                let Some(layout) = state.layouts.get_mut(&(pointer as usize)) else { return Ok(0); };
                layout.layout.set_origin(origin.0, origin.1);
                Ok(1)
            })
        }
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { Ok(0) }
    }
    fn destroy_text_layout(&self) -> RuntimeResult<i64> {
        let pointer = self.call.borrow().text_layout;
        if pointer.is_null() { return Ok(0); }
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        self.with_resources(|resources| {
            if let Some(state) = resources.text.coretext.as_mut() {
                state.layouts.remove(&(pointer as usize));
            }
            Ok(0)
        })?;
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        self.with_resources(|resources| {
            if let Some(state) = resources.text.freetype.as_mut() {
                state.layouts.remove(&(pointer as usize));
            }
            Ok(0)
        })?;
        self.call.borrow_mut().text_layout = ptr::null_mut();
        Ok(0)
    }
    fn measure_text_layout(&self) -> RuntimeResult<i64> {
        let pointer = self.call.borrow().text_layout;
        if pointer.is_null() { return Ok(0); }
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        return self.with_resources(|resources| {
            let Some(state) = resources.text.coretext.as_ref() else { return Ok(0); };
            let Some(layout) = state.layouts.get(&(pointer as usize)) else { return Ok(0); };
            let view = layout.layout.view();
            let call = self.call.borrow();
            if !call.text_measure.is_null() { unsafe { call.text_measure.write(view.width); } }
            if !call.text_measure_height.is_null() { unsafe { call.text_measure_height.write(view.height); } }
            Ok(1)
        });
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        return self.with_resources(|resources| {
            let Some(state) = resources.text.freetype.as_ref() else { return Ok(0); };
            let Some(layout) = state.layouts.get(&(pointer as usize)) else { return Ok(0); };
            if !layout.valid() { return Ok(0); }
            let view = layout.layout.view();
            let call = self.call.borrow();
            if !call.text_measure.is_null() { unsafe { call.text_measure.write(view.width); } }
            if !call.text_measure_height.is_null() { unsafe { call.text_measure_height.write(view.height); } }
            Ok(1)
        });
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { Ok(0) }
    }
    fn text_layout_exists(&self) -> RuntimeResult<bool> {
        let pointer = self.call.borrow().text_layout;
        if pointer.is_null() { return Ok(false); }
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        return self.with_resources(|resources| Ok(resources.text.coretext.as_ref()
            .and_then(|state| state.layouts.get(&(pointer as usize)))
            .is_some_and(OwnedCanvasTextLayout::valid)));
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        return self.with_resources(|resources| Ok(resources.text.freetype.as_ref()
            .and_then(|state| state.layouts.get(&(pointer as usize)))
            .is_some_and(OwnedFreeTypeCanvasTextLayout::valid)));
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { Ok(false) }
    }
    fn text_draw_bridge(&self) -> RuntimeResult<bool> {
        let pointer = self.call.borrow().text_layout;
        if pointer.is_null() { return Ok(false); }
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        return self.with_resources(|resources| Ok(resources.text.coretext.as_ref()
            .and_then(|state| state.layouts.get(&(pointer as usize)))
            .is_some_and(|layout| !layout.layout.draw_bridge().is_null())));
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        return self.with_resources(|resources| Ok(resources.text.freetype.as_ref()
            .and_then(|state| state.layouts.get(&(pointer as usize)))
            .is_some_and(|layout| !layout.layout.draw_bridge().is_null())));
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { Ok(false) }
    }
    fn prepare_text_draw(&self) -> RuntimeResult<i64> {
        let pointer = self.call.borrow().text_layout;
        if pointer.is_null() { return Ok(0); }
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let counts = self.with_resources(|resources| {
                let Some(state) = resources.text.coretext.as_mut() else { return Ok(None); };
                let Some(owned) = state.layouts.get_mut(&(pointer as usize)) else { return Ok(None); };
                if !owned.valid() { return Ok(None); }
                let cache = Box::new(coretext_draw_cache(&owned.layout)?);
                let bridge = (&*cache as *const NativeTextDrawCache as *mut c_void);
                owned.layout.set_draw_bridge(bridge);
                let view = owned.layout.view();
                owned.draw_cache = Some(cache);
                Ok(Some((view.glyph_count, view.underline_count)))
            })?;
            if let Some((glyphs, underlines)) = counts {
                let mut call = self.call.borrow_mut();
                call.text_glyph_count = glyphs;
                call.text_underline_count = underlines;
            }
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let counts = self.with_resources(|resources| {
                let Some(state) = resources.text.freetype.as_mut() else { return Ok(None); };
                let Some(owned) = state.layouts.get_mut(&(pointer as usize)) else { return Ok(None); };
                if !owned.valid() { return Ok(None); }
                let cache = Box::new(freetype_draw_cache(&owned.layout)?);
                let bridge = (&*cache as *const FreeTypeDrawCache as *mut c_void);
                owned.layout.set_draw_bridge(bridge);
                let counts = (owned.layout.glyphs.len(), owned.layout.underlines.len());
                owned.draw_cache = Some(cache);
                Ok(Some(counts))
            })?;
            if let Some((glyphs, underlines)) = counts {
                let mut call = self.call.borrow_mut();
                call.text_glyph_count = glyphs;
                call.text_underline_count = underlines;
            }
        }
        Ok(0)
    }
    fn read_text_fields(&self) -> RuntimeResult<i64> {
        let pointer = self.call.borrow().text_layout;
        if pointer.is_null() { return Ok(0); }
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let fields = self.with_resources(|resources| {
                let Some(state) = resources.text.coretext.as_ref() else { return Ok(None); };
                let Some(owned) = state.layouts.get(&(pointer as usize)) else { return Ok(None); };
                if !owned.valid() { return Ok(None); }
                let fields = owned.layout.fields();
                let view = owned.layout.view();
                Ok(Some((TextLayoutFields {
                    start_x: fields.start_x,
                    start_y: fields.start_y,
                    origin_x: fields.origin_x,
                    origin_y: fields.origin_y,
                    clip_x: fields.clip_x,
                    clip_y: fields.clip_y,
                    clip: fields.clip != 0,
                    poly_flags: fields.poly_flags,
                    color: fields.color,
                }, view.glyph_count, view.underline_count)))
            })?;
            if let Some((fields, glyphs, underlines)) = fields {
                let mut call = self.call.borrow_mut();
                call.text_fields = Some(fields);
                call.text_glyph_count = glyphs;
                call.text_underline_count = underlines;
            }
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let fields = self.with_resources(|resources| {
                let Some(state) = resources.text.freetype.as_ref() else { return Ok(None); };
                let Some(owned) = state.layouts.get(&(pointer as usize)) else { return Ok(None); };
                if !owned.valid() { return Ok(None); }
                let fields = owned.layout.fields();
                let view = owned.layout.view();
                Ok(Some((TextLayoutFields {
                    start_x: fields.start_x,
                    start_y: fields.start_y,
                    origin_x: fields.origin_x,
                    origin_y: fields.origin_y,
                    clip_x: fields.clip_x,
                    clip_y: fields.clip_y,
                    clip: fields.clip != 0,
                    poly_flags: fields.poly_flags,
                    color: fields.color,
                }, view.glyph_count, view.underline_count)))
            })?;
            if let Some((fields, glyphs, underlines)) = fields {
                let mut call = self.call.borrow_mut();
                call.text_fields = Some(fields);
                call.text_glyph_count = glyphs;
                call.text_underline_count = underlines;
            }
        }
        Ok(0)
    }
    fn draw_text(&self) -> RuntimeResult<i64> {
        let (pointer, frame, fields) = {
            let call = self.call.borrow();
            (call.text_layout, call.frame, call.text_fields)
        };
        if pointer.is_null() || frame.is_null() || fields.is_none() { return Ok(0); }
        let fields = fields.unwrap();
        let (fx2, fy2) = unsafe { ((*frame).fx2, (*frame).fy2) };
        #[cfg(all(feature = "text-coretext", target_os = "macos"))]
        {
            let drawn = self.with_resources(|resources| {
                let Some(created) = resources.created.as_ref() else {
                    return Err(RuntimeError::new("Canonical WGPU device is unavailable for native text"));
                };
                let Some(state) = resources.text.coretext.as_mut() else { return Ok(false); };
                let scratch = std::ptr::from_mut(&mut state.scratch);
                let Some(owned) = state.layouts.get_mut(&(pointer as usize)) else { return Ok(false); };
                let Some(cache) = owned.draw_cache.as_ref() else { return Ok(false); };
                let layout = hp2_wgpu::native_text::NativeTextLayout {
                    start_x: fields.start_x,
                    start_y: fields.start_y,
                    origin_x: fields.origin_x,
                    origin_y: fields.origin_y,
                    clip_x: fields.clip_x,
                    clip_y: fields.clip_y,
                    clip: fields.clip,
                    poly_flags: fields.poly_flags,
                    color: fields.color,
                };
                let services = hp2_render::native_text_layout_coretext_host::services();
                let raster = &cache.raster;
                let mut rasterize = |index: usize| {
                    let glyph = *raster.get(index)?;
                    unsafe {
                        hp2_render::native_text_layout_coretext::raster(
                            &services, scratch.cast(), glyph.font, glyph.glyph,
                            glyph.bounds, glyph.drawable, NATIVE_TEXT_PAGE_SIZE,
                        ).map(|(_, _, coverage)| coverage)
                    }
                };
                Ok(resources.loop_state.draw_native_text(
                    created, &layout, &cache.glyphs, &cache.underlines, fx2, fy2, &mut rasterize,
                ))
            })?;
            self.call.borrow_mut().text_drawn = drawn;
            return Ok(i64::from(drawn));
        }
        #[cfg(all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))]
        {
            let drawn = self.with_resources(|resources| {
                let Some(created) = resources.created.as_ref() else {
                    return Err(RuntimeError::new("Canonical WGPU device is unavailable for native text"));
                };
                let Some(state) = resources.text.freetype.as_mut() else { return Ok(false); };
                let Some(owned) = state.layouts.get_mut(&(pointer as usize)) else { return Ok(false); };
                let OwnedFreeTypeCanvasTextLayout { host, draw_cache, .. } = owned;
                let Some(cache) = draw_cache.as_ref() else { return Ok(false); };
                let layout = hp2_wgpu::native_text::NativeTextLayout {
                    start_x: fields.start_x,
                    start_y: fields.start_y,
                    origin_x: fields.origin_x,
                    origin_y: fields.origin_y,
                    clip_x: fields.clip_x,
                    clip_y: fields.clip_y,
                    clip: fields.clip,
                    poly_flags: fields.poly_flags,
                    color: fields.color,
                };
                let placements = &cache.raster;
                let mut rasterize = |index: usize| {
                    let placement = *placements.get(index)?;
                    let services = host.services();
                    hp2_render::native_text_layout_freetype::raster(
                        &services, &placement, NATIVE_TEXT_PAGE_SIZE,
                    )?;
                    let coverage = host.take_alpha_buffer();
                    (!coverage.is_empty()).then_some(coverage)
                };
                Ok(resources.loop_state.draw_native_text(
                    created, &layout, &cache.glyphs, &cache.underlines, fx2, fy2, &mut rasterize,
                ))
            })?;
            self.call.borrow_mut().text_drawn = drawn;
            return Ok(i64::from(drawn));
        }
        #[cfg(not(any(all(feature = "text-coretext", target_os = "macos"), all(feature = "text-freetype", not(all(feature = "text-coretext", target_os = "macos"))))))]
        { let _ = (fx2, fy2); Ok(0) }
    }
    fn configure_surface(&self) -> RuntimeResult<bool> {
        use hp2_wgpu::driver_policy::Hp2WgpuDriverLeaf as Leaf;
        self.viewport_dimension(Leaf::Drawable)?;
        let mut width = self.pixel_width.get();
        let mut height = self.pixel_height.get();
        if width <= 0 || height <= 0 {
            width = self.viewport_width().max(1);
            height = self.viewport_height().max(1);
            self.pixel_width.set(width);
            self.pixel_height.set(height);
        }
        let (width, height) = (width as u32, height as u32);
        self.with_resources(|resources| {
            let Some(created) = resources.created.as_ref() else { return Err(RuntimeError::new("Canonical WGPU device is unavailable")); };
            let vsync = if resources.persisted_graphics_loaded {
                self.read_u8(unsafe { ptr::addr_of!((*self.device).use_v_sync) })
            } else {
                let raw_aa = self.read_i32(unsafe { ptr::addr_of!((*self.device).use_aa) });
                let raw_samples = self.read_i32(unsafe { ptr::addr_of!((*self.device).num_aa_samples) });
                let raw_anisotropy = self.read_f32(unsafe { ptr::addr_of!((*self.device).max_anisotropy) });
                let default_clamp = hp2_wgpu::texture::anisotropy_clamp(raw_anisotropy);
                let defaults = StartupGraphics { msaa: if raw_aa & 1 != 0 { raw_samples } else { 0 }, anisotropy: if default_clamp == 1 { 0 } else { default_clamp }, vsync: self.read_u8(unsafe { ptr::addr_of!((*self.device).use_v_sync) }) };
                let (requested, warnings) = read_startup_graphics(self.access, defaults)?;
                resources.startup_warnings = warnings;
                for warning in [&resources.startup_warnings.msaa, &resources.startup_warnings.anisotropy, &resources.startup_warnings.vsync].into_iter().flatten() { eprintln!("WgpuDrv: WARNING: {warning}"); }
                resources.persisted_graphics = Some(requested);
                requested.vsync
            };
            let (effective, warning) = fallback_apply(vsync, 1, |mode| hp2_wgpu::configure_surface_for_vsync_strict(created, &mut resources.loop_state, width, height, mode))
                .map_err(|e| RuntimeError::new(&format!("Cannot apply VSync or FIFO fallback: {e}")))?;
            if let Some(error) = &warning { eprintln!("WgpuDrv: WARNING: requested VSync mode {vsync} unavailable; using FIFO: {error}"); }
            if !resources.persisted_graphics_loaded && warning.is_some() { resources.startup_warnings.vsync = warning; }
            unsafe { ptr::addr_of_mut!((*self.device).use_v_sync).write_unaligned(effective); }
            Ok(true)
        })
    }
    fn lock_frame(&self, width: i64, height: i64) -> RuntimeResult<hp2_wgpu::FrameOutcome> {
        self.with_resources(|resources| {
            let Some(created) = resources.created.as_ref() else { return Err(RuntimeError::new("Canonical WGPU device is unavailable")); };
            let fullscreen = !self.viewport.is_null() && unsafe {
                hp2_platform::viewport::is_fullscreen((*self.viewport).blit_flags)
            };
            resources.loop_state.set_logical_scene_size(if fullscreen {
                Some((self.viewport_width().max(1) as u32, self.viewport_height().max(1) as u32))
            } else { None });
            let requested = if resources.persisted_world_scale_loaded {
                persisted_world_scale_percent(self.read_f32(unsafe { ptr::addr_of!((*self.device).render_scale) }))?
            } else {
                let section: Vec<u32> = "WgpuDrv.WgpuRenderDevice".chars().map(u32::from).collect();
                let key: Vec<u32> = "RenderScale".chars().map(u32::from).collect();
                let raw = self.access.config_get_units_from(None, &section, &key)?;
                let value = match raw {
                    Some(units) => units.into_iter().map(char::from_u32).collect::<Option<String>>().and_then(|text| text.trim().parse::<f32>().ok()),
                    None => Some(self.read_f32(unsafe { ptr::addr_of!((*self.device).render_scale) })),
                };
                match value.and_then(|value| persisted_world_scale_percent(value).ok()) {
                    Some(percent) => percent,
                    None => {
                        let warning = "invalid persisted world scale; using 100%".to_owned();
                        eprintln!("WgpuDrv: WARNING: {warning}");
                        resources.startup_warnings.scale = Some(warning);
                        100
                    }
                }
            };
            if !resources.persisted_world_scale_loaded { resources.startup_requested_scale = Some(requested); }
            let outcome = hp2_wgpu::lock_frame_rgba(
                created, &mut resources.loop_state, width.max(1) as u32, height.max(1) as u32,
                self.screen_clear, self.read_u8(unsafe { ptr::addr_of!((*self.device).use_v_sync) }),
            );
            // The first scene is created by Lock, not Init. Preflight before
            // any draws so the very first world uses the persisted setting.
            // A reset replaces RenderLoop, so this also reapplies it after a
            // device recreation without touching the stored property.
            if resources.loop_state.has_world_scale_frame() {
                let apply = (|| -> RuntimeResult<()> {
                    if resources.loop_state.world_scale() != requested {
                        let (effective, warning) = fallback_apply(requested, 100, |value| resources.loop_state.set_world_scale(created, value))
                            .map_err(|error| RuntimeError::new(&format!("Cannot apply world scale or 100% fallback: {error}")))?;
                        if let Some(error) = &warning { eprintln!("WgpuDrv: WARNING: requested world scale {requested}% unavailable; using 100%: {error}"); }
                        if warning.is_some() { resources.startup_warnings.scale = warning; }
                        unsafe { ptr::addr_of_mut!((*self.device).render_scale).write_unaligned(f32::from(effective) / 100.0); }
                    }
                    if !resources.persisted_graphics_loaded {
                        let graphics = resources.persisted_graphics.ok_or_else(|| RuntimeError::new("Persisted graphics settings were not prepared"))?;
                        let (anisotropy_warning, msaa_warning) = unsafe { apply_startup_scene(&mut resources.loop_state, created, self.device, graphics)? };
                        if let Some(error) = &anisotropy_warning { eprintln!("WgpuDrv: WARNING: requested anisotropy {}x unavailable; using off: {error}", graphics.anisotropy); }
                        if let Some(error) = &msaa_warning { eprintln!("WgpuDrv: WARNING: requested MSAA {}x unavailable; using off: {error}", graphics.msaa); }
                        if anisotropy_warning.is_some() { resources.startup_warnings.anisotropy = anisotropy_warning; }
                        if msaa_warning.is_some() { resources.startup_warnings.msaa = msaa_warning; }
                        resources.persisted_graphics_loaded = true;
                    }
                    if !resources.persisted_world_scale_loaded {
                        unsafe { ptr::addr_of_mut!((*self.device).render_scale).write_unaligned(f32::from(resources.loop_state.world_scale()) / 100.0); }
                        resources.persisted_world_scale_loaded = true;
                    }
                    Ok(())
                })();
                if let Err(error) = apply {
                    // Discard the acquired frame; it must not show a partial startup.
                    hp2_wgpu::unlock_frame(created, &mut resources.loop_state, false);
                    return Err(error);
                }
            }
            Ok(outcome)
        })
    }
    fn unlock_frame(&self) -> RuntimeResult<bool> {
        self.with_resources(|resources| {
            let Some(created) = resources.created.as_ref() else { return Err(RuntimeError::new("Canonical WGPU device is unavailable")); };
            Ok(hp2_wgpu::unlock_frame(created, &mut resources.loop_state, self.blit))
        })
    }
}

#[cfg(feature = "renderer-wgpu")]
#[cfg(all(feature = "sdl", target_os = "macos"))]
struct OwnedMetalView(NonNull<c_void>);

#[cfg(feature = "renderer-wgpu")]
#[cfg(all(feature = "sdl", target_os = "macos"))]
impl Drop for OwnedMetalView {
    fn drop(&mut self) { unsafe { SDL_Metal_DestroyView(self.0.as_ptr()); } }
}

#[cfg(all(feature = "renderer-wgpu", feature = "sdl", target_os = "macos"))]
unsafe extern "C" {
    fn SDL_GetWindowSize(window: *mut c_void, width: *mut i32, height: *mut i32);
    fn SDL_Metal_CreateView(window: *mut c_void) -> *mut c_void;
    fn SDL_Metal_DestroyView(view: *mut c_void);
    fn SDL_Metal_GetDrawableSize(window: *mut c_void, width: *mut i32, height: *mut i32);
}

#[cfg(all(feature="renderer-wgpu",feature="sdl",not(target_os="macos")))]
unsafe extern "C" {
    fn SDL_GetWindowSize(window:*mut c_void,width:*mut i32,height:*mut i32);
    fn SDL_GL_GetDrawableSize(window:*mut c_void,width:*mut i32,height:*mut i32);
}



#[cfg(all(test, feature = "renderer-wgpu"))]
mod render_host_contracts {
    use super::*;

    #[test]
    fn surface_texture_bias_preserves_native_fused_dot_rounding() {
        let owner = crate::render_owner::render_test_support::owner("surface-axis-rounding");
        let access = owner.access();
        let device = crate::render_owner::render_test_support::construct(
            access, "WgpuDrv.WgpuRenderDevice",
        );
        {
            let host = policy(access, device.cast(), ptr::null_mut(), 1, 1).unwrap();
            let vector = |bits: [u32; 3]| hp2_abi::FVector {
                x: f32::from_bits(bits[0]), y: f32::from_bits(bits[1]), z: f32::from_bits(bits[2]),
            };
            let mut facet: FSurfaceFacet = unsafe { std::mem::zeroed() };
            // Actual matching native/Core facet: separate arithmetic rounds the
            // Y bias one ULP below FVector::operator|, shifting sampled UVs.
            facet.map_coords.origin = vector([0xc30b0655, 0xc392d733, 0x441015f7]);
            facet.map_coords.x_axis = vector([0xbf5db3d0, 0xbf000000, 0x374a98c8]);
            facet.map_coords.y_axis = vector([0x3efffff8, 0xbf5db3da, 0xb33bbd28]);
            host.call.borrow_mut().facet = ptr::addr_of_mut!(facet);
            host.surface_axes().unwrap();
            let call = host.call.borrow();
            assert_eq!(call.x_axis[3].to_bits(), 0x43859f8c);
            assert_eq!(call.y_axis[3].to_bits(), 0x4338d2b7);
        }
        owner.shutdown().unwrap();
    }
}

#[cfg(all(test, feature = "renderer-wgpu"))]
mod p8_upload_tests;
