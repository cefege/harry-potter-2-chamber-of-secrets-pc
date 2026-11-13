//! Port of `Tests/CanvasCompatibilityTests.cpp`.
//!
//! AGENTS non-negotiable #5: the bitmap-font compatibility mode is intentional.
//! `[Display] NativeText=False` must retain the page-backed `UCanvas` path and
//! its cursor advance while native text is the default. This file pins:
//!
//! * the three immutable font-package canaries (`Engine.u`, `HGame.u`,
//!   `UWindow.u`) that the native text path must never rewrite;
//! * the version-68/69 and page-backed `UFont` serialize round trips;
//! * `NativeText=False` ⇒ the page-backed Canvas path and cursor rules.
//!
//! The C++ test boots the engine against real game data through `-datadir=`.
//! The Rust harness receives the same root through `HP2_CORE_DATA_ROOT`, which
//! `hp2_add_behavior_test` sets to `${HP2_UNREAL_ROOT}`, and copies the real
//! `System/Default.ini` into an isolated writable root.
//!
//! The C++ file also exercises `FCanvasTextRequest`/`FCanvasTextLayoutRequest`
//! directly (`TestCanvasLayoutContract`, `TestWrappedSeamEquivalence`). The
//! request layouts are mirrored in `hp2-abi`, but the functions those cases
//! call — `ProjectCanvasTextLayoutRequest`, `DispatchCanvasTextLayout`,
//! `CreateNativeTextPlatformBackend`, `NativeTextInspectLayoutForTests`,
//! `FNativeTextLayoutTestInfo` and
//! `RunCanvasNativeTextCompatibilityForTests` — have no Rust equivalent: the
//! projection and dispatch live in the crate-private
//! `hp2_engine::render_device_owner::try_native_canvas_text`, and the provider
//! entry point `hp2_render::native_text_layout_coretext::create` needs the
//! `coretext` feature for its services table, which the bare Cargo test does
//! not enable. Those two cases are therefore not fabricated here: the
//! canvas-text layer is exercised through its public seam instead.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;

use hp2_abi::engine::generated::{
    FFontCharacter, FFontPage, TArrayRaw, TMapCharChar, TMapCharCharPair, UFont, UObject, USDLClient,
};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_core::package_archive::ExportArchive;
use hp2_core::sha256;
use hp2_engine::canvas_text::{
    CanvasTextHost, DrawStringArgs, Font, Hp2CanvasSize, Hp2CanvasTileDraw, Hp2CanvasView,
    Hp2FontChar, Hp2NativeTextRequest, Hp2Plane, STY_NONE, STY_TRANSLUCENT, Texture, draw_string,
    native_draw_text, native_draw_text_clipped, wrapped_print,
};
use hp2_engine::EngineOwner;


// ---------------------------------------------------------------------------
// Data root and isolated System directory.
// ---------------------------------------------------------------------------

/// The prototype data root. `HP2_CORE_DATA_ROOT` is the mechanism every
/// data-driven `hp2-engine` integration test uses (`hp2_add_behavior_test`
/// passes `HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}`); the repository checkout
/// default keeps a bare `cargo test` runnable from the repo root.
fn data_root() -> PathBuf {
    std::env::var_os("HP2_CORE_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("HarryPotter2/Unreal")
        })
}

fn require_data_root() -> PathBuf {
    let root = data_root();
    assert!(
        root.join("System/Default.ini").is_file(),
        "HP2_CORE_DATA_ROOT must point to the prototype Unreal data root"
    );
    root
}

/// `HP2_ARTIFACT_DIR` when the harness pre-creates it, otherwise Cargo's
/// per-test-target temporary directory.
fn sandbox(label: &str) -> PathBuf {
    let base = std::env::var_os("HP2_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let root = base.join(format!("canvas-compatibility-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    root
}

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

/// Boots the canonical engine from an isolated root that carries the real
/// `System/Default.ini` (and `DefUser.ini`) from the prototype data root.
fn engine(label: &str) -> (EngineOwner, PathBuf) {
    engine_with(label, None)
}

fn engine_with(label: &str, configuration: Option<&str>) -> (EngineOwner, PathBuf) {
    let data = require_data_root();
    let root = sandbox(label);
    for name in ["Default.ini", "DefUser.ini"] {
        std::fs::copy(data.join("System").join(name), root.join("System").join(name))
            .expect("copy real prototype configuration");
    }
    if let Some(configuration) = configuration {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(root.join("System/Default.ini"))
            .expect("append isolated configuration");
        file.write_all(configuration.as_bytes())
            .expect("write isolated configuration");
    }
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("initialize canonical Core and Engine owners");
    (owner, root)
}

// ---------------------------------------------------------------------------
// `SHA256` from CanvasCompatibilityTests.cpp (`CC_SHA256_*`).
// ---------------------------------------------------------------------------

fn sha256_hex(path: &Path) -> String {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut context = std::mem::MaybeUninit::<sha256::Sha256Context>::uninit();
    unsafe { sha256::init(context.as_mut_ptr()) };
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).unwrap_or(0);
        if count == 0 {
            break;
        }
        unsafe { sha256::update(context.as_mut_ptr(), buffer.as_ptr(), count as u32) };
    }
    let mut digest = [0u8; 32];
    unsafe { sha256::final_digest(digest.as_mut_ptr(), context.as_mut_ptr()) };
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `TestImmutableFontPackages`: the three prototype code packages are hashed
/// from the real data root, exactly as the C++ test hashed `RepositoryRoot`.
#[test]
fn immutable_font_packages_are_not_rewritten() {
    let root = require_data_root();
    let packages = [
        (
            "HarryPotter2/Unreal/System/Engine.u",
            "Engine.u",
            "c75c3e849bd07a3a315d3e87f24bb5d8960b5ce32f8fc1438d2c1c3906f93182",
        ),
        (
            "HarryPotter2/Unreal/System/HGame.u",
            "HGame.u",
            "6c996cd90d7841cba6ea1bc5edece67471307ff47fc6d856774106b01afc22a2",
        ),
        (
            "HarryPotter2/Unreal/System/UWindow.u",
            "UWindow.u",
            "711656cdfe4ed45ca29cec4e87748b2ef668c675b25a1480619a7f784cccc94a",
        ),
    ];
    // The C++ test resolves these against `std::filesystem::current_path()`;
    // the Rust harness resolves the same three files under the data root.
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, leaf, expected) in packages {
        let mut path = repository_root.join(relative);
        if !path.is_file() {
            path = root.join("System").join(leaf);
        }
        let actual = sha256_hex(&path);
        assert!(
            !actual.is_empty(),
            "immutable package fixture is unavailable"
        );
        assert_eq!(
            actual, expected,
            "native text changed an immutable font package"
        );
    }
}

// ---------------------------------------------------------------------------
// `TestFontSerializeRoundTrips`.
// ---------------------------------------------------------------------------

struct Mapping<'a> {
    access: CoreAccess<'a>,
}

impl ArchiveMapping for Mapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 {
            return Ok(ptr::null_mut());
        }
        let object = self.access.object_at(index - 1);
        if object.is_null() {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Unknown canonical archive object",
            ));
        }
        Ok(object)
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        Ok(if object.is_null() { 0 } else { unsafe { (*object).index + 1 } })
    }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> {
        self.access.name_text(index)?;
        Ok(index)
    }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> {
        self.access.name_text(index)?;
        Ok(index)
    }
    fn visit_object(&self, _slot: *mut *mut UObject) -> RuntimeResult<()> {
        Ok(())
    }
    fn visit_name(&self, index: i32) -> RuntimeResult<()> {
        self.access.name_text(index)?;
        Ok(())
    }
}

fn text(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}

unsafe fn construct(access: CoreAccess<'_>, name: &str) -> *mut UObject {
    unsafe {
        access.construct_object(
            access.native_class(name).unwrap(),
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )
    }
    .unwrap()
}

unsafe fn set_name(access: CoreAccess<'_>, font: *mut UFont, value: &str) {
    unsafe { access.assign_fstring(ptr::addr_of_mut!((*font).font_name), &text(value)) }.unwrap();
}

unsafe fn name(font: *mut UFont) -> Vec<u32> {
    let value = unsafe { ptr::addr_of!((*font).font_name).read_unaligned() };
    if value.num == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize - 1).to_vec() }
    }
}

/// `CreateNativeFontFixture`: a transient `UFont` with only `FontName` and
/// `FontHeight` set.
unsafe fn native_font(access: CoreAccess<'_>, name: &str, height: i32) -> *mut UFont {
    let font = unsafe { construct(access, "Engine.Font") }.cast::<UFont>();
    assert!(!font.is_null(), "native font serialization fixtures could not be allocated");
    unsafe { set_name(access, font, name) };
    unsafe { (*font).font_height = height };
    font
}

/// `CreatePageBackedFontFixture`: `FontName=""`, `CharactersPerPage=256`, one
/// page of 256 glyphs, and `'A'` at `USize=9`/`VSize=18`.
unsafe fn page_backed_font(access: CoreAccess<'_>) -> *mut UFont {
    let font = unsafe { construct(access, "Engine.Font") }.cast::<UFont>();
    assert!(!font.is_null(), "transient page-backed font fixture allocation failed");
    unsafe { set_name(access, font, "") };
    unsafe { (*font).characters_per_page = 256 };
    let pages = unsafe { libc::calloc(1, std::mem::size_of::<FFontPage>()) }.cast::<FFontPage>();
    let characters = unsafe { libc::calloc(256, std::mem::size_of::<FFontCharacter>()) }
        .cast::<FFontCharacter>();
    assert!(!pages.is_null() && !characters.is_null());
    unsafe {
        (*pages).characters = TArrayRaw { data: characters.cast(), num: 256, max: 256 };
        (*font).pages = TArrayRaw { data: pages.cast(), num: 1, max: 1 };
        (*characters.add(65)).u_size = 9;
        (*characters.add(65)).v_size = 18;
    }
    font
}

/// `Legacy->CharRemap.Set('A','B')` on a transient font.
unsafe fn set_remap(font: *mut UFont) {
    unsafe { (*font).is_remapped = 1 };
    let map = unsafe { ptr::addr_of_mut!((*font).char_remap).cast::<TMapCharChar>() };
    let pairs = unsafe { libc::calloc(1, std::mem::size_of::<TMapCharCharPair>()) }
        .cast::<TMapCharCharPair>();
    let hash = unsafe { libc::calloc(2, std::mem::size_of::<i32>()) }.cast::<i32>();
    assert!(!pairs.is_null() && !hash.is_null());
    unsafe {
        (*pairs).key = 65;
        (*pairs).value = 66;
        (*pairs).hash_next = -1;
        ptr::addr_of_mut!((*map).pairs).write_unaligned(TArrayRaw { data: pairs.cast(), num: 1, max: 1 });
        ptr::addr_of_mut!((*map).hash).write_unaligned(hash.cast());
        ptr::addr_of_mut!((*map).hash_count).write_unaligned(2);
        hash.add(1).write(0);
    }
}

fn font_remap_pairs(font: *mut UFont) -> Vec<(u32, u32)> {
    let map = unsafe { ptr::addr_of!((*font).char_remap).cast::<TMapCharChar>() };
    let header = unsafe { ptr::addr_of!((*map).pairs).read_unaligned() };
    if header.num <= 0 || header.data.is_null() {
        return Vec::new();
    }
    let pairs = header.data.cast::<TMapCharCharPair>();
    (0..header.num as usize)
        .map(|index| unsafe {
            (
                ptr::addr_of!((*pairs.add(index)).key).read_unaligned(),
                ptr::addr_of!((*pairs.add(index)).value).read_unaligned(),
            )
        })
        .collect()
}

fn round_trip(access: CoreAccess<'_>, font: *mut UFont, version: u16) -> Vec<u8> {
    let mapping = Mapping { access };
    let archive = ObjectArchive::saving(ArchiveFlags::new(version, 0, false), &mapping);
    unsafe { access.serialize_object(font.cast(), &archive) }.unwrap();
    archive.into_written_bytes().unwrap()
}

fn load_into(access: CoreAccess<'_>, font: *mut UFont, version: u16, bytes: &[u8], message: &str) {
    let mapping = Mapping { access };
    let archive = ObjectArchive::loading(
        ExportArchive::from_slice(bytes),
        ArchiveFlags::new(version, 0, false),
        &mapping,
    );
    unsafe { access.serialize_object(font.cast(), &archive) }.unwrap();
    assert_eq!(archive.tell().unwrap(), bytes.len(), "{message}");
}

#[test]
fn font_serialization_round_trips() {
    let (owner, _root) = engine("font-serialize");
    let access = owner.access();
    let mut mode = access.process_mode();
    mode.lazy_load = false;
    access.set_process_mode(mode);

    unsafe {
        // Version 68 clears the unsupported character remapping.
        let legacy = native_font(access, "Legacy", 17);
        let legacy_copy = native_font(access, "", 0);
        assert!(
            !legacy.is_null() && !legacy_copy.is_null(),
            "native font serialization fixtures could not be allocated"
        );
        (*legacy).characters_per_page = 0;
        set_remap(legacy);
        let bytes = round_trip(access, legacy, 68);
        load_into(access, legacy_copy, 68, &bytes, "version-68 UFont archive did not round-trip cleanly");
        assert_eq!(
            name(legacy_copy),
            "Legacy".chars().map(u32::from).collect::<Vec<_>>(),
            "version-68 UFont archive lost native-font fields"
        );
        assert_eq!((*legacy_copy).font_height, 17, "version-68 UFont archive lost native-font fields");
        assert!(
            (*legacy_copy).is_remapped == 0 && font_remap_pairs(legacy_copy).is_empty(),
            "version-68 UFont archive did not clear unsupported character remapping"
        );
        assert!(
            !access.process_mode().lazy_load,
            "UFont serialization did not restore GLazyLoad"
        );

        // Version 69 keeps the remapping.
        let modern = native_font(access, "Modern", 21);
        let modern_copy = native_font(access, "", 0);
        assert!(
            !modern.is_null() && !modern_copy.is_null(),
            "modern font serialization fixtures could not be allocated"
        );
        (*modern).characters_per_page = 0;
        set_remap(modern);
        let bytes = round_trip(access, modern, 69);
        load_into(access, modern_copy, 69, &bytes, "version-69 UFont archive did not round-trip cleanly");
        assert_eq!(
            name(modern_copy),
            "Modern".chars().map(u32::from).collect::<Vec<_>>(),
            "version-69 UFont archive lost native-font fields"
        );
        assert_eq!((*modern_copy).font_height, 21, "version-69 UFont archive lost native-font fields");
        assert!(
            (*modern_copy).is_remapped != 0 && font_remap_pairs(modern_copy) == vec![(65, 66)],
            "version-69 UFont archive lost character remapping"
        );
        assert!(
            !access.process_mode().lazy_load,
            "UFont serialization did not restore GLazyLoad"
        );

        // Page-backed fonts carry their glyph table through the archive.
        let page_font = page_backed_font(access);
        let page_copy = native_font(access, "", 0);
        assert!(
            !page_font.is_null() && !page_copy.is_null(),
            "page-backed font serialization fixtures could not be allocated"
        );
        let bytes = round_trip(access, page_font, 69);
        load_into(access, page_copy, 69, &bytes, "page-backed UFont archive did not round-trip cleanly");
        let pages = (*page_copy).pages;
        assert!((*page_copy).characters_per_page == 256, "page-backed UFont archive changed its glyph table");
        assert_eq!(pages.num, 1, "page-backed UFont archive changed its glyph table");
        assert!(!pages.data.is_null());
        let characters = (*pages.data.cast::<FFontPage>()).characters;
        assert_eq!(characters.num, 256, "page-backed UFont archive changed its glyph table");
        let a = characters.data.cast::<FFontCharacter>().add(65);
        assert_eq!(
            ((*a).u_size, (*a).v_size),
            (9, 18),
            "page-backed UFont archive changed its glyph table"
        );
        assert!(
            !access.process_mode().lazy_load,
            "UFont serialization did not restore GLazyLoad"
        );
    }
}

// ---------------------------------------------------------------------------
// `NativeText=False` compatibility mode.
// ---------------------------------------------------------------------------

const PAGE_FONT: Font = Font(1 as *mut c_void);
const NAMED_FONT: Font = Font(2 as *mut c_void);
const PAGE_TEXTURE: Texture = Texture(3 as *mut c_void);

/// Host standing in for the engine's native-text seam.
///
/// `try_native_text` mirrors the production dispatcher's gate in
/// `crates/hp2-engine/src/render_device_owner.rs` (`try_native_canvas_text`):
///
/// ```text
/// if !native_text && !name_nonempty { return Ok(None); }
/// ```
///
/// `shaped` is the shaped-metrics result the platform backend would return;
/// `None` models an unavailable backend (the C++ `Backend == NULL` case).
struct Fixture {
    native_text: bool,
    shaped: Option<(i32, i32)>,
    slots: [Texture; 5],
    native_calls: usize,
    viewport_calls: usize,
    page_locks: usize,
}

impl Fixture {
    fn new(native_text: bool, shaped: Option<(i32, i32)>) -> Self {
        Self {
            native_text,
            shaped,
            slots: [Texture(ptr::null_mut()); 5],
            native_calls: 0,
            viewport_calls: 0,
            page_locks: 0,
        }
    }

    fn has_name(font: Font) -> bool {
        font == NAMED_FONT
    }
}

impl CanvasTextHost for Fixture {
    fn ui_scale(&mut self) -> f32 {
        1.0
    }

    fn language_is_int(&mut self) -> bool {
        true
    }

    fn viewport_draw_string(
        &mut self,
        _cv: &Hp2CanvasView,
        _flags: u32,
        _font: Font,
        _x: &mut i32,
        _y: &mut i32,
        _text: &[u16],
        _color: Hp2Plane,
    ) {
        self.viewport_calls += 1;
    }

    fn try_native_text(
        &mut self,
        _cv: &Hp2CanvasView,
        _req: &Hp2NativeTextRequest,
        font: Font,
        _text: &[u16],
        _color: Hp2Plane,
    ) -> Option<(i32, i32)> {
        if !self.native_text && !Self::has_name(font) {
            return None;
        }
        let shaped = self.shaped?;
        self.native_calls += 1;
        Some(shaped)
    }

    fn font_chars_per_page(&mut self, font: Font) -> i32 {
        if font == PAGE_FONT { 256 } else { 0 }
    }

    fn font_remap_char(&mut self, _font: Font, ch: u16) -> u16 {
        ch
    }

    fn font_page_count(&mut self, font: Font) -> i32 {
        if font == PAGE_FONT { 1 } else { 0 }
    }

    fn font_page_texture(&mut self, font: Font, page: i32) -> Texture {
        if font == PAGE_FONT && page == 0 { PAGE_TEXTURE } else { Texture(ptr::null_mut()) }
    }

    fn font_page_char_count(&mut self, font: Font, page: i32) -> i32 {
        if font == PAGE_FONT && page == 0 { 256 } else { 0 }
    }

    fn font_page_char(&mut self, font: Font, page: i32, index: i32) -> Hp2FontChar {
        if font == PAGE_FONT && page == 0 && index == 65 {
            Hp2FontChar { start_u: 17, start_v: 23, u_size: 9, v_size: 18 }
        } else {
            Hp2FontChar { start_u: 0, start_v: 0, u_size: 0, v_size: 0 }
        }
    }

    fn font_has_name(&mut self, font: Font) -> bool {
        Self::has_name(font)
    }

    fn slot_reset(&mut self) {
        self.slots = [Texture(ptr::null_mut()); 5];
    }

    fn slot_texture(&mut self, slot: usize) -> Texture {
        self.slots[slot]
    }

    fn slot_lock(&mut self, _cv: &Hp2CanvasView, slot: usize, tex: Texture) {
        self.slots[slot] = tex;
        self.page_locks += 1;
    }

    fn slot_unlock(&mut self, slot: usize) {
        self.slots[slot] = Texture(ptr::null_mut());
    }

    fn draw_tile_slot(
        &mut self,
        _cv: &Hp2CanvasView,
        _slot: usize,
        _rect: &hp2_engine::canvas::Hp2CanvasTileRect,
        _color: Hp2Plane,
        _flags: u32,
    ) {
    }

    fn draw_tile_texture(&mut self, _cv: &Hp2CanvasView, _draw: &Hp2CanvasTileDraw) {}

    fn texture_size(&mut self, _tex: Texture) -> Hp2CanvasSize {
        Hp2CanvasSize { w: 256, h: 256 }
    }

    fn log(&mut self, _code: u32) {}
}

fn canvas_view(font: Font, cur_x: f32, cur_y: f32, cur_yl: f32) -> Hp2CanvasView {
    Hp2CanvasView {
        font,
        large_font: font,
        big_font: font,
        med_font: font,
        cur_x,
        cur_y,
        cur_yl,
        clip_x: 512.0,
        clip_y: 128.0,
        org_x: 0.0,
        org_y: 0.0,
        space_x: 0.0,
        space_y: 0.0,
        z: 1.0,
        x: 640,
        frame_x: 640,
        frame_y: 480,
        frame_fx: 640.0,
        frame_fy: 480.0,
        has_frame: 1,
        style: STY_NONE,
        center: 0,
        no_smooth: 0,
        color: Hp2Plane { x: 1.0, y: 1.0, z: 1.0, w: 1.0 },
    }
}

const TEXT_A: [u16; 2] = [b'A' as u16, 0];
const SHAPED_WIDTH: i32 = 10;
const SHAPED_HEIGHT: i32 = 20;

/// `RunCanvasNativeTextCompatibilityForTests`: dispatch through the native
/// seam and apply the Canvas cursor rules the C++ seam pins.
fn run_canvas_native_text_compatibility(
    host: &mut Fixture,
    state_native_text: bool,
    font: Font,
    cr: bool,
    clipped: bool,
    cursor: &mut (f32, f32, f32),
    dimensions: &mut (i32, i32),
) -> bool {
    *dimensions = (0, 0);
    let request = Hp2NativeTextRequest {
        flags: 0,
        draw_x: 0,
        draw_y: 0,
        clip: 0,
        amp: 0,
        center: 0,
        wrap: 0,
        visible: 1,
    };
    let view = canvas_view(font, cursor.0, cursor.1, cursor.2);
    // `TryNativeCanvasText`'s test branch: `TestNativeText || FontName.Len()>0`.
    let saved = host.native_text;
    host.native_text = state_native_text;
    let shaped = host.try_native_text(&view, &request, font, &TEXT_A, view.color);
    host.native_text = saved;
    let Some((width, height)) = shaped else {
        return false;
    };
    *dimensions = (width, height);
    if !clipped {
        cursor.0 += width as f32;
        cursor.2 = cursor.2.max(height as f32);
        if cr {
            cursor.0 = 0.0;
            cursor.1 += cursor.2;
            cursor.2 = 0.0;
        }
    }
    true
}

#[test]
fn canvas_policy_and_cursor_contract() {
    let mut host = Fixture::new(false, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));

    // NativeText=False keeps the page-backed Canvas path and leaves the
    // caller's cursor untouched.
    let mut cursor = (9.0, 11.0, 3.0);
    let mut dimensions = (0, 0);
    let run = run_canvas_native_text_compatibility(
        &mut host, false, PAGE_FONT, false, false, &mut cursor, &mut dimensions,
    );
    assert!(
        !run && cursor == (9.0, 11.0, 3.0),
        "NativeText=False did not retain the page-backed Canvas path and cursor"
    );

    // An inherently native font still dispatches safely when NativeText=False.
    let mut cursor = (9.0, 11.0, 3.0);
    let mut dimensions = (0, 0);
    let run = run_canvas_native_text_compatibility(
        &mut host, false, NAMED_FONT, false, false, &mut cursor, &mut dimensions,
    );
    assert!(
        run
            && dimensions.0 > 0
            && dimensions.1 > 0
            && cursor.0 == 9.0 + dimensions.0 as f32
            && cursor.1 == 11.0
            && cursor.2 >= dimensions.1 as f32,
        "NativeText=False did not dispatch an inherently native font safely"
    );

    // An unavailable backend changes neither Canvas dimensions nor cursor.
    let mut unavailable = Fixture::new(false, None);
    let mut cursor = (9.0, 11.0, 3.0);
    let mut dimensions = (0, 0);
    let run = run_canvas_native_text_compatibility(
        &mut unavailable, false, NAMED_FONT, false, false, &mut cursor, &mut dimensions,
    );
    assert!(
        !run && dimensions == (0, 0) && cursor == (9.0, 11.0, 3.0),
        "unavailable native backend changed Canvas dimensions or cursor"
    );

    // NativeText=True shapes page-backed text and advances by the metrics.
    let mut native = Fixture::new(true, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));
    let mut cursor = (9.0, 11.0, 3.0);
    let mut dimensions = (0, 0);
    let run = run_canvas_native_text_compatibility(
        &mut native, true, PAGE_FONT, false, false, &mut cursor, &mut dimensions,
    );
    assert!(
        run
            && dimensions.0 > 0
            && dimensions.1 > 0
            && cursor.0 == 9.0 + dimensions.0 as f32
            && cursor.1 == 11.0
            && cursor.2 >= dimensions.1 as f32,
        "native DrawText did not use shaped metrics for cursor advance"
    );

    // CR advances by the measured line height and clears the line cursor.
    let mut cursor = (9.0, 11.0, 0.0);
    let mut dimensions = (0, 0);
    let run = run_canvas_native_text_compatibility(
        &mut native, true, PAGE_FONT, true, false, &mut cursor, &mut dimensions,
    );
    assert!(
        run
            && cursor.0 == 0.0
            && cursor.1 == 11.0 + dimensions.1 as f32
            && cursor.2 == 0.0,
        "native DrawText CR did not use the measured line height"
    );

    // DrawTextClipped never moves the cursor.
    let mut cursor = (9.0, 11.0, 3.0);
    let mut dimensions = (0, 0);
    let run = run_canvas_native_text_compatibility(
        &mut native, true, PAGE_FONT, false, true, &mut cursor, &mut dimensions,
    );
    assert!(
        run && cursor == (9.0, 11.0, 3.0),
        "DrawTextClipped changed Canvas cursor state"
    );
}

/// The production `DrawString` bit that the compatibility pin depends on:
/// with the flag off, a page-backed font takes the glyph-page path and returns
/// its page advance (`'A'` is 9 pixels wide), not shaped metrics.
#[test]
fn native_text_false_draws_page_backed_glyphs() {
    let mut host = Fixture::new(false, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));
    let view = canvas_view(PAGE_FONT, 9.0, 11.0, 3.0);
    let args = DrawStringArgs {
        flags: 0,
        font: PAGE_FONT,
        x: 9,
        y: 11,
        color: view.color,
        clip: false,
        amp: false,
    };
    let advance = draw_string(&mut host, &view, &args, &TEXT_A);
    assert_eq!(
        advance, 9,
        "NativeText=False did not retain the page-backed Canvas path and cursor"
    );
    assert_eq!(host.page_locks, 1, "NativeText=False did not retain the page-backed Canvas path and cursor");
    assert_eq!(host.native_calls, 0, "NativeText=False did not retain the page-backed Canvas path and cursor");
    // `UCanvas::DrawString` leaves the caller's cursor alone.
    assert_eq!((view.cur_x, view.cur_y, view.cur_yl), (9.0, 11.0, 3.0));
}

/// Production `DrawText` (`execDrawText`) with the native seam accepting: the
/// shaped width advances `CurX` and the measured height raises `CurYL`.
#[test]
fn native_draw_text_uses_shaped_metrics() {
    let mut host = Fixture::new(true, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));
    let mut view = canvas_view(PAGE_FONT, 9.0, 11.0, 3.0);
    view.style = STY_TRANSLUCENT;
    native_draw_text(&mut host, &mut view, &TEXT_A, false, 0);
    assert_eq!(
        view.cur_x,
        9.0 + SHAPED_WIDTH as f32,
        "native DrawText did not use shaped metrics for cursor advance"
    );
    assert!(
        view.cur_yl >= SHAPED_HEIGHT as f32,
        "native DrawText did not use shaped metrics for cursor advance"
    );
    assert_eq!(host.native_calls, 1);

    // `WrappedPrint` already advanced `CurY` by the native height, and the CR
    // rule then advances by the measured line height (stock `execDrawText`).
    let mut view = canvas_view(PAGE_FONT, 9.0, 11.0, 0.0);
    view.style = STY_TRANSLUCENT;
    native_draw_text(&mut host, &mut view, &TEXT_A, true, 0);
    assert_eq!(view.cur_x, 0.0, "native DrawText CR did not use the measured line height");
    assert_eq!(view.cur_yl, 0.0, "native DrawText CR did not use the measured line height");
    assert_eq!(
        view.cur_y,
        11.0 + 2.0 * SHAPED_HEIGHT as f32,
        "native DrawText CR did not use the measured line height"
    );
}

/// Production `DrawTextClipped` leaves the Canvas cursor state untouched.
#[test]
fn draw_text_clipped_does_not_move_the_cursor() {
    let mut host = Fixture::new(true, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));
    let mut view = canvas_view(PAGE_FONT, 9.0, 11.0, 3.0);
    native_draw_text_clipped(&mut host, &mut view, &TEXT_A, false);
    assert_eq!(
        (view.cur_x, view.cur_y, view.cur_yl),
        (9.0, 11.0, 3.0),
        "DrawTextClipped changed Canvas cursor state"
    );
}

/// The engine-side `NativeText` bit is the selector: booting with
/// `NativeText=False` in the isolated `System/Default.ini` leaves the
/// constructed `SDLDrv.SDLClient` in the page-backed compatibility mode, while
/// the stock configuration keeps native text as the default.
///
/// `Display` is the property's declared `UProperty` category; stock
/// `UObject::LoadConfig` keys sections by class path, and
/// `Build/prepare_retail_data.py` writes the same `[SDLDrv.SDLClient]` section.
#[test]
fn native_text_flag_selects_the_compatibility_mode() {
    {
        let (default_owner, _default_root) = engine("native-text-flag-default");
        unsafe {
            let client = construct(default_owner.access(), "SDLDrv.SDLClient").cast::<USDLClient>();
            let base = ptr::addr_of!((*client).base).read_unaligned();
            assert!(base.native_text(), "native text must be the default Canvas path");
        }
    }

    let (owner, _root) = engine_with(
        "native-text-flag",
        Some("\n[SDLDrv.SDLClient]\nNativeText=False\n"),
    );
    unsafe {
        let client = construct(owner.access(), "SDLDrv.SDLClient").cast::<USDLClient>();
        let base = ptr::addr_of!((*client).base).read_unaligned();
        assert!(
            !base.native_text(),
            "NativeText=False did not retain the page-backed Canvas path and cursor"
        );
    }
}

/// `wrapped_print` is the production bridge between the two modes: the page
/// path measures with `GetCharSize`, the native path returns shaped metrics.
#[test]
fn wrapped_print_selects_page_metrics_or_shaped_metrics() {
    let mut page = Fixture::new(false, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));
    let mut view = canvas_view(PAGE_FONT, 9.0, 11.0, 0.0);
    let (width, height) = wrapped_print(&mut page, &mut view, PAGE_FONT, STY_TRANSLUCENT, false, &TEXT_A, 0);
    assert_eq!((width, height), (9 + 9, 18), "NativeText=False did not retain the page-backed Canvas path and cursor");
    assert_eq!(page.native_calls, 0);
    assert_eq!(view.cur_y, 11.0 + 18.0);

    let mut native = Fixture::new(true, Some((SHAPED_WIDTH, SHAPED_HEIGHT)));
    let mut view = canvas_view(PAGE_FONT, 9.0, 11.0, 0.0);
    let (width, height) = wrapped_print(&mut native, &mut view, PAGE_FONT, STY_TRANSLUCENT, false, &TEXT_A, 0);
    assert_eq!((width, height), (SHAPED_WIDTH, SHAPED_HEIGHT), "native DrawText did not use shaped metrics for cursor advance");
    assert_eq!(native.native_calls, 1);
}

/// The unreachable-backend path of `wrapped_print` returns zero metrics and
/// does not advance the cursor.
#[test]
fn unavailable_backend_changes_neither_dimensions_nor_cursor() {
    let mut host = Fixture::new(false, None);
    let mut view = canvas_view(NAMED_FONT, 9.0, 11.0, 3.0);
    let (width, height) = wrapped_print(&mut host, &mut view, NAMED_FONT, STY_NONE, false, &TEXT_A, 0);
    assert!(
        width == 0 && height == 0 && view.cur_x == 9.0 && view.cur_y == 11.0 && view.cur_yl == 3.0,
        "unavailable native backend changed Canvas dimensions or cursor"
    );
    assert_eq!(host.viewport_calls, 1);
}