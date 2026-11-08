//! Canonical Rust ownership and typed Core adapter for SDL-backed client/viewports.
use hp2_abi::engine::generated::{AActor, ALevelInfo, TArrayRaw, UCanvas, UClient, UEngine, UGameEngine, UInput, UObject, ULevel, UViewport, USDLClient, USDLViewport, UStruct};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind};
use hp2_core::object_archive::ObjectArchive;
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, cell::RefCell, collections::HashSet, ffi::c_void, mem, ptr};
const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const INPUT_BASE: ClassOps = NativeRecordKind::Subsystem.operations();
const INPUT_OPERATIONS: ClassOps = {
    let mut operations = INPUT_BASE;
    operations.construct = construct_input;
    operations.serialize = serialize_input;
    operations.visit_references = serialize_input;
    operations
};
const CLIENT_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_client;
    operations.destroy = destroy_client;
    operations.finalize_members = finalize_client_members;
    operations.serialize = serialize_client;
    operations.visit_references = serialize_client;
    operations
};
const SDL_CLIENT_OPERATIONS: ClassOps = {
    let mut operations = CLIENT_OPERATIONS;
    operations.destroy = destroy_sdl_client;
    operations.finalize_members = finalize_sdl_client_members;
    operations
};
const VIEWPORT_OPERATIONS: ClassOps = {
    let mut operations = crate::player_owner::OPERATIONS;
    operations.destroy = destroy_viewport_base;
    operations.serialize = serialize_viewport;
    operations.visit_references = serialize_viewport;
    operations
};
const SDL_VIEWPORT_OPERATIONS: ClassOps = {
    let mut operations = VIEWPORT_OPERATIONS;
    operations.destroy = destroy_sdl_viewport;
    operations.construct = construct_sdl_viewport;
    operations
};
const CLASS_ABSTRACT: u32 = 1;
const CLASS_CONFIG: u32 = 4;
const CLASS_TRANSIENT: u32 = 8;
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;

const CPF_CONFIG: u32 = 0x0000_4000;
const fn sdl_config(name: &'static str, kind: StartupPropertyKind, category: &'static str, offset: usize) -> StartupProperty {
    StartupProperty { name, kind, category, property_flags: CPF_CONFIG, object_flags: 4, inner_name: None, offset, array_dim: 1 }
}
const SDL_CLIENT_PROPERTIES: &[StartupProperty] = &[
    sdl_config("UseJoystick", StartupPropertyKind::Bool { bit_mask: 1 }, "Joystick", std::mem::offset_of!(USDLClient, use_joystick)),
    sdl_config("StartupFullscreen", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, startup_fullscreen)),
    sdl_config("BorderlessWindow", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, borderless_window)),
    sdl_config("UseDesktopResolution", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, use_desktop_resolution)),
    sdl_config("IgnoreHat", StartupPropertyKind::Bool { bit_mask: 1 }, "Joystick", std::mem::offset_of!(USDLClient, ignore_hat)),
    sdl_config("IgnoreUngrabbedMouse", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, ignore_ungrabbed_mouse)),
    sdl_config("AllowUnicodeKeys", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, allow_unicode_keys)),
    sdl_config("AllowCommandQKeys", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, allow_command_q_keys)),
    sdl_config("MacKeepAllScreensOn", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, mac_keep_all_screens_on)),
    sdl_config("MacNativeTextToSpeech", StartupPropertyKind::Bool { bit_mask: 1 }, "Display", std::mem::offset_of!(USDLClient, mac_native_text_to_speech)),
    sdl_config("JoystickNumber", StartupPropertyKind::Int, "Joystick", std::mem::offset_of!(USDLClient, joystick_number)),
    sdl_config("JoystickHatNumber", StartupPropertyKind::Int, "Joystick", std::mem::offset_of!(USDLClient, joystick_hat_number)),
    sdl_config("ScaleJBX", StartupPropertyKind::Float, "Joystick", std::mem::offset_of!(USDLClient, scale_jbx)),
    sdl_config("ScaleJBY", StartupPropertyKind::Float, "Joystick", std::mem::offset_of!(USDLClient, scale_jby)),
    sdl_config("JoystickDeadZone", StartupPropertyKind::Float, "Joystick", std::mem::offset_of!(USDLClient, joystick_dead_zone)),
    sdl_config("TextToSpeechFile", StartupPropertyKind::String, "Display", std::mem::offset_of!(USDLClient, text_to_speech_file)),
    sdl_config("UIScale", StartupPropertyKind::Float, "Display", std::mem::offset_of!(USDLClient, ui_scale)),
];
const fn client_config(
    name: &'static str,
    kind: StartupPropertyKind,
    category: &'static str,
    offset: usize,
    array_dim: usize,
) -> StartupProperty {
    StartupProperty {
        name,
        kind,
        category,
        property_flags: CPF_CONFIG,
        object_flags: 4,
        inner_name: None,
        offset,
        array_dim,
    }
}

/// `BITFIELD` member declared through `CPP_PROPERTY`: the generated bit index
/// selects the containing byte and its mask inside that byte.
const fn client_flag(name: &'static str, bit: usize) -> StartupProperty {
    client_config(
        name,
        StartupPropertyKind::Bool { bit_mask: 1u32 << (bit % 8) },
        "Display",
        bit / 8,
        1,
    )
}
const DETAILS_VALUES: &[&str] = &["High", "Medium", "Low", "Ultra Low"];
const CLIENT_PROPERTIES: &[StartupProperty] = &[
    client_config("WindowedViewportX", StartupPropertyKind::Int, "Client", std::mem::offset_of!(UClient, windowed_viewport_x), 1),
    client_config("WindowedViewportY", StartupPropertyKind::Int, "Client", std::mem::offset_of!(UClient, windowed_viewport_y), 1),
    client_config("WindowedColorBits", StartupPropertyKind::Int, "Client", std::mem::offset_of!(UClient, windowed_color_bits), 1),
    client_config("FullscreenViewportX", StartupPropertyKind::Int, "Client", std::mem::offset_of!(UClient, fullscreen_viewport_x), 1),
    client_config("FullscreenViewportY", StartupPropertyKind::Int, "Client", std::mem::offset_of!(UClient, fullscreen_viewport_y), 1),
    client_config("FullscreenColorBits", StartupPropertyKind::Int, "Client", std::mem::offset_of!(UClient, fullscreen_color_bits), 1),
    client_config("MipFactor", StartupPropertyKind::Float, "Client", std::mem::offset_of!(UClient, mip_factor), 1),
    client_config("Brightness", StartupPropertyKind::Float, "Display", std::mem::offset_of!(UClient, brightness), 1),
    client_flag("CaptureMouse", UClient::CAPTURE_MOUSE_BIT),
    client_flag("CurvedSurfaces", UClient::CURVED_SURFACES_BIT),
    client_config("TextureDetail", StartupPropertyKind::ByteEnum { name: "Details", values: DETAILS_VALUES }, "Display", std::mem::offset_of!(UClient, texture_lod_set) + std::mem::size_of::<i32>(), 1),
    client_config("SkinDetail", StartupPropertyKind::ByteEnum { name: "Details", values: DETAILS_VALUES }, "Display", std::mem::offset_of!(UClient, texture_lod_set) + 2 * std::mem::size_of::<i32>(), 1),
    client_flag("ScreenFlashes", UClient::SCREEN_FLASHES_BIT),
    client_flag("MaintainVerticalFOV", UClient::MAINTAIN_VERTICAL_FOV_BIT),
    client_flag("NativeText", UClient::NATIVE_TEXT_BIT),
    client_flag("NoLighting", UClient::NO_LIGHTING_BIT),
    client_config("MinDesiredFrameRate", StartupPropertyKind::Float, "Display", std::mem::offset_of!(UClient, min_desired_frame_rate), 1),
    client_flag("Decals", UClient::DECALS_BIT),
    client_flag("NoDynamicLights", UClient::NO_DYNAMIC_LIGHTS_BIT),
    client_flag("NoFractalAnim", UClient::NO_FRACTAL_ANIM_BIT),
    client_flag("ShowFPS", UClient::SHOW_FPS_BIT),
    client_config("ParticleDensity", StartupPropertyKind::Int, "Display", std::mem::offset_of!(UClient, particle_density), 1),
];
unsafe fn construct_client(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.construct)(access, object)?; }
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Client construction has no canonical Engine state"))?;
    state.bitmap_client.set(std::ptr::NonNull::new(object.cast()));
    Ok(())
}

unsafe fn destroy_client(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Client destruction has no canonical Engine state"))?;
    state.bitmap_client.set(None);
    unsafe { (OBJECT.destroy)(access, object) }
}

unsafe fn construct_input(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UInput")); }
    unsafe { (INPUT_BASE.construct)(access, object)?; }
    let actor = access.native_class("Engine.Actor")?;
    let enum_class = access.native_class("Core.Enum")?;
    let name = access.intern_str("EInputKey", hp2_core::name_owner::FindName::Find)?;
    let keys: *mut c_void = unsafe { access.find_object(enum_class, actor.cast(), name, false) }.cast();
    if keys.is_null() { return Err(RuntimeError::new("UInput EInputKey metadata is unavailable")); }
    unsafe { ptr::addr_of_mut!((*object.cast::<UInput>()).input_keys).write(keys); }
    Ok(())
}
unsafe fn construct_viewport_parts(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<()> {
    let canvas_base = access.native_class("Engine.Canvas")?;
    let canvas_name: Vec<u32> = "ini:Engine.Engine.Canvas\0".chars().map(u32::from).collect();
    let canvas_class = unsafe { access.static_load_class(canvas_base, ptr::null_mut(), &canvas_name, 0x0002)? };
    let canvas = unsafe { access.construct_object(canvas_class, viewport.cast(), 0, 0, ptr::null_mut())? };
    unsafe {
        ptr::addr_of_mut!((*viewport).canvas).write(canvas.cast());
        ptr::addr_of_mut!((*canvas.cast::<UCanvas>()).viewport).write(viewport.cast());
    }
    let input_base = access.native_class("Engine.Input")?;
    let input_name: Vec<u32> = "ini:Engine.Engine.Input\0".chars().map(u32::from).collect();
    let input_class = unsafe { access.static_load_class(input_base, ptr::null_mut(), &input_name, 0x0002)? };
    let input = unsafe { access.construct_object(input_class, viewport.cast(), 0, 0, ptr::null_mut())? };
    unsafe {
        ptr::addr_of_mut!((*viewport).input).write(input.cast());
        ptr::addr_of_mut!((*input.cast::<UInput>()).viewport).write(viewport.cast());
    }
    Ok(())
}
unsafe fn construct_sdl_viewport(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical SDL viewport")); }
    unsafe {
        (VIEWPORT_OPERATIONS.construct)(access, object)?;
        construct_viewport_parts(access, object.cast())?;
    }
    let viewport = object.cast::<USDLViewport>();
    unsafe {
        (*viewport).last_joy_hat = hp2_platform::input_event::ik::NONE;
        (*viewport).mouse_scale_x = 1.0;
        (*viewport).mouse_scale_y = 1.0;
    }
    crate::engine_owner::platform_state(access)?.register_viewport(viewport)
}
unsafe fn destroy_sdl_viewport(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical SDL viewport")); }
    // Drop the address-keyed UI state even if a later teardown operation fails.
    #[cfg(feature = "in-game-overlay")]
    crate::in_game_overlay::destroy_viewport(object as usize);
    unsafe {
        let viewport = object.cast::<USDLViewport>();
        let client = (*viewport).base.outer.cast::<USDLClient>();
        if !client.is_null() {
            let engine = (*client).base.engine.cast::<UEngine>();
            if !engine.is_null() && !(*engine).audio.is_null() {
                crate::audio_owner::detach_viewport(access, (*engine).audio.cast(), viewport.cast())?;
            }
        }
        let render_device = (*viewport).base.ren_dev;
        if !render_device.is_null() {
            access.delete_object(render_device.cast())?;
            (*viewport).base.ren_dev = ptr::null_mut();
        }
        crate::engine_owner::platform_state(access)?.destroy_viewport(viewport)?;
        destroy_viewport_base(access, object)
    }
}
unsafe fn serialize_input(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (INPUT_BASE.serialize)(access, object, archive)?; }
    unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*object.cast::<UInput>()).input_keys).cast()) }
}

unsafe fn serialize_viewport(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (crate::player_owner::OPERATIONS.serialize)(access, object, archive)?; }
    let viewport = object.cast::<UViewport>();
    unsafe {
        for slot in [
            ptr::addr_of_mut!((*viewport).console),
            ptr::addr_of_mut!((*viewport).misc_res),
            ptr::addr_of_mut!((*viewport).canvas),
            ptr::addr_of_mut!((*viewport).ren_dev),
            ptr::addr_of_mut!((*viewport).input),
        ] {
            archive.serialize_object_slot(slot.cast())?;
        }
    }
    Ok(())
}
unsafe fn serialize_client_viewport(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_object_slot(value.cast::<*mut UObject>()) }
}
unsafe fn serialize_client(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.serialize)(access, object, archive)?; }
    unsafe {
        crate::animation_owner::serialize_array(
            archive,
            ptr::addr_of_mut!((*object.cast::<UClient>()).viewports),
            mem::size_of::<*mut UObject>(),
            Some(serialize_client_viewport),
            None,
            false,
        )
    }
}
unsafe fn destroy_sdl_client(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical SDL client")); }
    let client = object.cast::<USDLClient>();
    let viewports = unsafe { ptr::addr_of_mut!((*client).base.viewports) };
    let count = unsafe { (*viewports).num };
    if count < 0 || (count > 0 && unsafe { (*viewports).data.is_null() }) {
        return Err(RuntimeError::new("Invalid canonical SDL client viewport array"));
    }
    for index in 0..count as usize {
        let viewport = unsafe { (*viewports).data.cast::<*mut USDLViewport>().add(index).read_unaligned() };
        if !viewport.is_null() {
            let render_device = unsafe { (*viewport).base.ren_dev };
            if !render_device.is_null() {
                unsafe {
                    access.delete_object(render_device.cast())?;
                    (*viewport).base.ren_dev = ptr::null_mut();
                }
            }
        }
    }
    while unsafe { (*viewports).num } > 0 {
        let viewport = unsafe { (*viewports).data.cast::<*mut USDLViewport>().read_unaligned() };
        let slots = unsafe { hp2_core::array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!((*viewports).data),
            num: ptr::addr_of_mut!((*viewports).num),
            max: ptr::addr_of_mut!((*viewports).max),
        } };
        unsafe {
            hp2_core::array_storage::remove(
                slots, 0, 1, mem::size_of::<*mut USDLViewport>() as i32, ptr::null(),
                hp2_core::array_storage::system_allocator(),
            ).map_err(|_| RuntimeError::new("Unable to remove canonical SDL client viewport"))?;
        }
        if !viewport.is_null() { unsafe { access.conditional_destroy(viewport.cast())?; } }
    }
    #[cfg(feature = "sdl")]
    unsafe {
        let joystick = (*client).joystick;
        if !joystick.is_null() { hp2_platform::joystick::close_joystick(joystick.cast()); }
        (*client).joystick = ptr::null_mut();
        (*client).joy_buttons = 0;
        (*client).joy_hats = 0;
        (*client).joy_axes = 0;
        (*client).joy_hat_folds_to_buttons = 0;
    }
    #[cfg(feature = "sdl")]
    unsafe { hp2_platform::launcher_boot_video::quit(); }
    unsafe { (CLIENT_OPERATIONS.destroy)(access, object) }
}

unsafe fn finalize_client_members(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UClient")); }
    let client = object.cast::<UClient>();
    let slots = unsafe { hp2_core::array_storage::Hp2ArraySlots {
        data: ptr::addr_of_mut!((*client).viewports.data),
        num: ptr::addr_of_mut!((*client).viewports.num),
        max: ptr::addr_of_mut!((*client).viewports.max),
    } };
    unsafe { hp2_core::array_storage::release(slots, hp2_core::array_storage::system_allocator()) }
        .map_err(|_| RuntimeError::new("Unable to release canonical Client viewport array"))
}

unsafe fn finalize_sdl_client_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical SDL client")); }
    let client = object.cast::<USDLClient>();
    let base = unsafe { finalize_client_members(access, object) };
    let text = unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*client).text_to_speech_file)) };
    base.and(text)
}

unsafe fn destroy_viewport_base(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::player_owner::OPERATIONS.destroy)(access, object) }
}

const CLIENT_DEFAULTS: &[hp2_core::core_class_specs::StartupDefault] = &[
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "MaintainVerticalFOV", value: true },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "NativeText", value: true },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "ShowFPS", value: false },
];
const SDL_CLIENT_DEFAULTS: &[hp2_core::core_class_specs::StartupDefault] = &[
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "WindowedViewportX", value: 1024 },
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "WindowedViewportY", value: 768 },
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "WindowedColorBits", value: 32 },
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "FullscreenViewportX", value: 1024 },
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "FullscreenViewportY", value: 768 },
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "FullscreenColorBits", value: 32 },
    hp2_core::core_class_specs::StartupDefault::SetFloat { field: "Brightness", value: 0.5 },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "CaptureMouse", value: true },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "ScreenFlashes", value: true },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "MaintainVerticalFOV", value: true },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "Decals", value: true },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "ShowFPS", value: false },
    hp2_core::core_class_specs::StartupDefault::SetFloat { field: "MinDesiredFrameRate", value: 30.0 },
    hp2_core::core_class_specs::StartupDefault::SetInt { field: "ParticleDensity", value: 1 },
    hp2_core::core_class_specs::StartupDefault::SetBool { field: "StartupFullscreen", value: false },
    hp2_core::core_class_specs::StartupDefault::SetFloat { field: "UIScale", value: 1.0 },
    hp2_core::core_class_specs::StartupDefault::SetFloat { field: "JoystickDeadZone", value: 0.2 },
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Input", package: "Engine", super_name: Some("Subsystem"), within: "Object",
        flags: CLASS_CONFIG | CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "User",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<UInput>()),
        operations: Some(INPUT_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "Client", package: "Engine", super_name: Some("Object"), within: "Object",
        flags: CLASS_ABSTRACT | CLASS_CONFIG, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<UClient>()),
        operations: Some(CLIENT_OPERATIONS), properties: CLIENT_PROPERTIES, defaults: CLIENT_DEFAULTS,
    },
    NativeClassSpec {
        short_name: "Viewport", package: "Engine", super_name: Some("Player"), within: "Client",
        flags: CLASS_ABSTRACT | CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<UViewport>()),
        operations: Some(VIEWPORT_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "SDLClient", package: "SDLDrv", super_name: Some("Engine.Client"), within: "Object",
        flags: CLASS_CONFIG | CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<USDLClient>()),
        operations: Some(SDL_CLIENT_OPERATIONS), properties: SDL_CLIENT_PROPERTIES, defaults: SDL_CLIENT_DEFAULTS,
    },
    NativeClassSpec {
        short_name: "SDLViewport", package: "SDLDrv", super_name: Some("Engine.Viewport"), within: "SDLClient",
        flags: CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<USDLViewport>()),
        operations: Some(SDL_VIEWPORT_OPERATIONS), properties: &[], defaults: &[],
    },
];
/// Creates the source SDL client viewport and publishes it in `Viewports`.
///
/// # Safety
/// `client` must be a live canonical USDLClient and `name` must be a valid FName index.
pub unsafe fn new_viewport(
    access: CoreAccess<'_>,
    client: *mut USDLClient,
    name: i32,
) -> RuntimeResult<*mut USDLViewport> {
    if client.is_null() { return Err(RuntimeError::new("Null canonical SDL client")); }
    let class = access.native_class("SDLDrv.SDLViewport")?;
    let viewport = unsafe { access.construct_object(class, client.cast(), name, 0, ptr::null_mut())? }
        .cast::<USDLViewport>();
    let array = unsafe { ptr::addr_of_mut!((*client).base.viewports) };
    let slots = unsafe { hp2_core::array_storage::Hp2ArraySlots {
        data: ptr::addr_of_mut!((*array).data),
        num: ptr::addr_of_mut!((*array).num),
        max: ptr::addr_of_mut!((*array).max),
    } };
    let index = unsafe {
        hp2_core::array_storage::add(
            slots, 1, mem::size_of::<*mut USDLViewport>() as i32, ptr::null(),
            hp2_core::array_storage::system_allocator(),
        )
    }.map_err(|_| RuntimeError::new("Unable to grow canonical SDL client viewport array"));
    match index {
        Ok(index) => {
            unsafe { (*array).data.cast::<*mut USDLViewport>().add(index as usize).write_unaligned(viewport); }
            Ok(viewport)
        }
        Err(error) => {
            unsafe { access.conditional_destroy(viewport.cast())?; }
            Err(error)
        }
    }
}

/// Applies `USDLClient::Init`'s source-owned engine and joystick state.
///
/// # Safety
/// `client` must be a live canonical USDLClient; `engine` must be its canonical engine.
pub unsafe fn init_client(
    access: CoreAccess<'_>,
    client: *mut USDLClient,
    engine: *mut UEngine,
) -> RuntimeResult<()> {
    if client.is_null() || engine.is_null() { return Err(RuntimeError::new("Invalid canonical SDL client initialization")); }
    unsafe {
        (*client).base.engine = engine.cast();
        crate::cam_mgr::post_edit_change(
            ptr::addr_of_mut!((*client).base.brightness),
            ptr::addr_of_mut!((*client).base.mip_factor),
            ptr::addr_of_mut!((*client).base.particle_density),
        );
        // `USDLClient` is packed, so the generated bitfield accessor cannot borrow
        // `base`; apply it to a copy and store the whole header back.
        let mut base = ptr::read_unaligned(ptr::addr_of!((*client).base));
        base.set_capture_mouse(true);
        ptr::addr_of_mut!((*client).base).write_unaligned(base);
    }
    const DEFAULTRES: [hp2_core::ansi::Tchar; 11] = [100, 101, 102, 97, 117, 108, 116, 114, 101, 115, 0];
    let command_line = hp2_core::native_text::tchar_units(access.command_line());
    if unsafe { hp2_core::search::parse_param(command_line.as_ptr(), DEFAULTRES.as_ptr()) } {
        unsafe {
            (*client).base.windowed_viewport_x = 640;
            (*client).base.windowed_viewport_y = 480;
        }
    }
    #[cfg(feature = "sdl")]
    if unsafe { (*client).use_joystick != 0 } {
        let opened = unsafe {
            hp2_platform::joystick::open_joystick(
                (*client).joystick_number,
                (*client).ignore_hat != 0,
            )
        };
        if let Some((joystick, buttons, hats, axes, folds)) = opened {
            unsafe {
                (*client).joystick = joystick.cast();
                (*client).joy_buttons = buttons;
                (*client).joy_hats = hats;
                (*client).joy_axes = axes;
                (*client).joy_hat_folds_to_buttons = u32::from(folds);
            }
        } else {
            unsafe { (*client).use_joystick = 0; }
        }
    }
    Ok(())
}
/// Executes shared `UClient::Exec`; USDLClient inherits this exact source path.
///
/// # Safety
/// `client` must be a live canonical UClient and `command` must be NUL terminated.
pub(crate) unsafe fn exec(
    access: CoreAccess<'_>,
    client: *mut UClient,
    command: *const u32,
    _output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if client.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid canonical UClient Exec arguments"));
    }
    const BRIGHTNESS: [hp2_core::ansi::Tchar; 11] = hp2_core::ansi::wide(b"BRIGHTNESS\0");
    let brightness = unsafe { ptr::addr_of_mut!((*client).brightness) };
    let result = unsafe {
        crate::cam_mgr::exec_with(
            client.cast(),
            brightness,
            command.cast(),
            |command| {
                let mut stream = command;
                Ok(unsafe {
                    hp2_core::command::parse_command(
                        (&mut stream as *mut *const hp2_core::ansi::Tchar).cast(),
                        BRIGHTNESS.as_ptr(),
                    ) != 0
                })
            },
            |client, allow| {
                let engine = unsafe { (*client.cast::<UClient>()).engine.cast::<UEngine>() };
                if engine.is_null() { return Err(RuntimeError::new("UClient has no Engine for Flush")); }
                unsafe { crate::engine_owner::flush(access, engine, allow != 0) }
            },
            |client| unsafe { access.save_config(client.cast(), 0x4000, None) },
            |client| {
                let array = unsafe { (*client.cast::<UClient>()).viewports };
                validate_client_viewports(array)?;
                Ok(array.num)
            },
            |client| {
                let array = unsafe { (*client.cast::<UClient>()).viewports };
                validate_client_viewports(array)?;
                if array.num == 0 { return Ok(ptr::null_mut()); }
                let viewport = unsafe { array.data.cast::<*mut UViewport>().read_unaligned() };
                Ok(if viewport.is_null() { ptr::null_mut() } else { unsafe { (*viewport).actor } })
            },
            |client, level| unsafe { send_brightness_message(access, client.cast(), level) },
        )
    }?;
    Ok(result != 0)
}

fn validate_client_viewports(viewports: TArrayRaw) -> RuntimeResult<()> {
    if viewports.num < 0
        || viewports.num > viewports.max
        || (viewports.num > 0 && viewports.data.is_null())
    {
        return Err(RuntimeError::new("UClient viewport array has invalid storage"));
    }
    Ok(())
}

unsafe fn send_brightness_message(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    level: i32,
) -> RuntimeResult<()> {
    if actor.is_null() { return Ok(()); }
    let actor_object = actor.cast::<UObject>();
    let event_name = access.intern_str("ClientMessage", hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(actor_object, event_name, false)? };
    if function.is_null() { return Err(RuntimeError::new("APlayerPawn.ClientMessage metadata is unavailable")); }
    let message_text = format!("Brightness level {level}/10");
    let mut units = message_text.chars().map(u32::from).collect::<Vec<_>>();
    units.push(0);
    let mut message = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    unsafe { access.assign_fstring(&mut message, &units)?; }
    #[repr(C)]
    struct Params { message: TArrayRaw, kind: hp2_abi::engine::generated::FName, life: f32 }
    let mut params = Params {
        message,
        kind: hp2_abi::engine::generated::FName { index: 0 },
        life: 0.0,
    };
    let result = unsafe {
        crate::actor_owner::process_event(access, actor.cast(), function, (&mut params as *mut Params).cast(), ptr::null_mut())
    };
    let destroy = unsafe { access.destroy_owned_fstring(&mut params.message) };
    result.and(destroy)
}


/// Flushes every render device in a canonical UClient in source array order.
///
/// # Safety
/// `client` must be a live canonical UClient and every array entry must remain
/// live through the synchronous flush.
pub(crate) unsafe fn flush(
    access: CoreAccess<'_>,
    client: *mut UClient,
    allow_precache: bool,
) -> RuntimeResult<()> {
    if client.is_null() { return Err(RuntimeError::new("Null canonical UClient Flush")); }
    unsafe {
        crate::cam_mgr::flush_with(
            client.cast(),
            i32::from(allow_precache),
            |client| {
                let viewports = unsafe { (*client.cast::<UClient>()).viewports };
                validate_client_viewports(viewports)?;
                Ok(viewports.num)
            },
            |client, index| {
                let viewports = unsafe { (*client.cast::<UClient>()).viewports };
                validate_client_viewports(viewports)?;
                if index < 0 || index >= viewports.num {
                    return Err(RuntimeError::new("UClient viewport index is outside its live array"));
                }
                let viewport = unsafe { viewports.data.cast::<*mut UViewport>().add(index as usize).read_unaligned() };
                Ok(if viewport.is_null() { ptr::null_mut() } else { unsafe { (*viewport).ren_dev } })
            },
            |device, allow| unsafe { crate::render_device_owner::flush(access, device.cast(), allow != 0) },
        )
    }
}
#[derive(Default)]
pub(crate) struct PlatformOwnerState {
    viewports: RefCell<HashSet<usize>>,
}

impl PlatformOwnerState {
    pub(crate) fn register_viewport(&self, viewport: *mut USDLViewport) -> RuntimeResult<()> {
        if viewport.is_null() { return Err(RuntimeError::new("Null canonical SDL viewport")); }
        self.viewports.borrow_mut().insert(viewport as usize);
        Ok(())
    }
    fn owns(&self, viewport: *mut USDLViewport) -> bool {
        !viewport.is_null() && self.viewports.borrow().contains(&(viewport as usize))
    }
    pub(crate) unsafe fn destroy_viewport(&self, viewport: *mut USDLViewport) -> RuntimeResult<()> {
        if viewport.is_null() { return Err(RuntimeError::new("Null canonical SDL viewport")); }
        self.unregister(viewport);
        #[cfg(feature = "sdl")]
        {
            let grabbed = unsafe { (*viewport).mouse_is_grabbed() };
            let window = unsafe { ptr::addr_of!((*viewport).sdl_window).read_unaligned() };
            unsafe { hp2_platform::mouse_state::update_mouse_grab_state(window.cast(), false, grabbed); }
            unsafe { (*viewport).set_mouse_is_grabbed(false); }
            if !window.is_null() {
                unsafe { hp2_platform::window_lifecycle::destroy_window(window.cast()); }
                unsafe { ptr::addr_of_mut!((*viewport).sdl_window).write_unaligned(ptr::null_mut()); }
            }
            let cursors = unsafe { ptr::addr_of_mut!((*viewport).system_cursors).cast::<*mut c_void>() };
            for index in 0..7 {
                let cursor = unsafe { cursors.add(index).read_unaligned() };
                if !cursor.is_null() {
                    unsafe { hp2_platform::cursor::free_cursor(cursor.cast()); }
                    unsafe { cursors.add(index).write_unaligned(ptr::null_mut()); }
                }
            }
        }
        Ok(())
    }
    fn unregister(&self, viewport: *mut USDLViewport) {
        self.viewports.borrow_mut().remove(&(viewport as usize));
    }
}
#[cfg(feature = "sdl")]
pub(crate) fn sdl_window(access: CoreAccess<'_>, viewport: *mut USDLViewport) -> RuntimeResult<std::ptr::NonNull<c_void>> {
    let state = crate::engine_owner::platform_state(access)?;
    if !state.owns(viewport) { return Err(RuntimeError::new("SDL viewport is not owned by the canonical platform runtime")); }
    let window = unsafe { ptr::addr_of!((*viewport).sdl_window).read_unaligned() };
    std::ptr::NonNull::new(window).ok_or_else(|| RuntimeError::new("SDL viewport has no window"))
}

/// Source-order `UViewport::Lock` state update and typed renderer dispatch.
///
/// # Safety
/// `viewport`, all planes, and any hit-test outputs must remain live through the call.
#[allow(clippy::too_many_arguments)]
pub unsafe fn lock_viewport(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    flash_scale: *const hp2_abi::FPlane,
    flash_fog: *const hp2_abi::FPlane,
    screen_clear: *const hp2_abi::FPlane,
    render_lock_flags: u32,
    hit_data: *mut c_void,
    hit_size: *mut i32,
) -> RuntimeResult<bool> {
    if viewport.is_null() || flash_scale.is_null() || flash_fog.is_null() || screen_clear.is_null() {
        return Err(RuntimeError::new("Invalid canonical UViewport Lock arguments"));
    }
    let device = unsafe { (*viewport).ren_dev };
    if device.is_null() { return Err(RuntimeError::new("UViewport::Lock requires a RenderDevice")); }
    let locked = unsafe {
        crate::camera_exec::lock_with(
            viewport,
            flash_scale,
            flash_fog,
            screen_clear,
            render_lock_flags,
            hit_data,
            hit_size,
            |viewport| {
                let now = hp2_core::time::from_double(
                    hp2_audio::verification::visual_seconds()
                        .unwrap_or_else(hp2_core::unix_clock::seconds_slow),
                ).to_ne_bytes();
                unsafe { ptr::addr_of_mut!((*viewport).current_time).write_unaligned(now); }
                Ok::<(), RuntimeError>(())
            },
            // WgpuDrv.cpp::UWgpuRenderDevice::Lock accepts but does not read
            // RenderLockFlags, HitData, or HitSize; viewport state above is the
            // complete source-side effect for this backend.
            |device, scale, fog, clear, _flags, _hit_data, _hit_size| unsafe {
                crate::render_device_owner::lock(access, device.cast(), scale, fog, clear)
            },
        )?
    };
    Ok(locked != 0)
}

/// Source-order `UViewport::Unlock` dispatch and blit-time update.
///
/// # Safety
/// `viewport` must remain live through the call.
pub unsafe fn unlock_viewport(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    blit: bool,
) -> RuntimeResult<()> {
    if viewport.is_null() { return Err(RuntimeError::new("Null canonical UViewport Unlock target")); }
    let device = unsafe { (*viewport).ren_dev };
    if device.is_null() { return Err(RuntimeError::new("UViewport::Unlock requires a RenderDevice")); }
    let hit_sizes = unsafe { (*viewport).hit_sizes };
    if hit_sizes.num != 0 { return Err(RuntimeError::new("UViewport::Unlock requires an empty hit-size array")); }
    unsafe {
        crate::camera_exec::unlock_with(viewport, i32::from(blit), |device, blit| {
            crate::render_device_owner::unlock(access, device.cast(), blit != 0)
        })
    }
}

/// Source-order `UViewport::PushHit`: append the size, then dispatch to the device.
///
/// # Safety
/// `viewport` and the `size` bytes beginning at `hit` must be live.
pub(crate) unsafe fn push_hit(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    hit: *mut hp2_abi::engine::generated::HHitProxy,
    size: i32,
) -> RuntimeResult<()> {
    unsafe {
        (*hit).size = size;
        let array = ptr::addr_of_mut!((*viewport).hit_sizes);
        let slots = hp2_core::array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        };
        let index = hp2_core::array_storage::add(
            slots, 1, mem::size_of::<i32>() as i32, ptr::null(),
            hp2_core::array_storage::system_allocator(),
        ).map_err(|_| RuntimeError::new("Unable to grow canonical viewport hit-size array"))?;
        (*array).data.cast::<i32>().add(index as usize).write_unaligned(size);
        crate::render_device_owner::push_hit(access, (*viewport).ren_dev.cast(), hit.cast(), size)
    }
}

/// Source-order `UViewport::PopHit`: remove the latest size, then dispatch.
///
/// # Safety
/// `viewport` must be live with an initialized hit-size array.
pub(crate) unsafe fn pop_hit(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    force: bool,
) -> RuntimeResult<()> {
    unsafe {
        let array = ptr::addr_of_mut!((*viewport).hit_sizes);
        if (*array).num <= 0 {
            return Err(RuntimeError::new("Canonical viewport hit-size stack is empty"));
        }
        let index = (*array).num - 1;
        let size = (*array).data.cast::<i32>().add(index as usize).read_unaligned();
        let slots = hp2_core::array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        };
        hp2_core::array_storage::remove(
            slots, index, 1, mem::size_of::<i32>() as i32, ptr::null(),
            hp2_core::array_storage::system_allocator(),
        ).map_err(|_| RuntimeError::new("Unable to pop canonical viewport hit-size array"))?;
        crate::render_device_owner::pop_hit(access, (*viewport).ren_dev.cast(), size, force)
    }
}

/// Updates one canonical SDL viewport via the platform crate's shared pump.
///
/// # Safety
/// `viewport` must be a live canonical USDLViewport and remain live through the pump.
pub(crate) unsafe fn update_input(access: CoreAccess<'_>, viewport: *mut USDLViewport, reset: bool) -> RuntimeResult<()> {
    let state = crate::engine_owner::platform_state(access)?;
    if !state.owns(viewport) { return Err(RuntimeError::new("SDL viewport is not owned by the canonical platform runtime")); }
    #[cfg(feature = "sdl")]
    {
        let mut backend = CoreInputBackend { access, viewport };
        // SAFETY: This call is synchronous; the registered viewport and SDL
        // window remain live while its event pump runs.
        unsafe { hp2_platform::input_pump::update_input_with(&mut backend, reset) }
            .map_err(|error| error)
    }
    #[cfg(not(feature = "sdl"))]
    {
        let _ = reset;
        Err(RuntimeError::new("SDL input update requested without the SDL platform feature"))
    }
}

/// Applies source mouse capture to a canonical SDL viewport.
///
/// # Safety
/// `viewport` is a live viewport owned by this Engine's platform owner.
pub(crate) unsafe fn set_mouse_capture(
    access: CoreAccess<'_>, viewport: *mut UViewport, capture: i32, _clip: i32, focus_only: i32,
) -> RuntimeResult<()> {
    let viewport = viewport.cast::<USDLViewport>();
    if !crate::engine_owner::platform_state(access)?.owns(viewport) {
        return Err(RuntimeError::new("Mouse capture viewport is not owned by this platform"));
    }
    #[cfg(feature = "sdl")]
    {
        hp2_platform::viewport_policy::set_mouse_capture_with(
            capture, focus_only,
            |lost| { unsafe { (*viewport).set_lost_grab(lost); } Ok(()) },
            || {
                let window = unsafe { (*viewport).sdl_window };
                Ok(if window.is_null() { None } else {
                    Some(unsafe { hp2_platform::window_lifecycle::get_window_flags(window.cast()) })
                })
            },
            |capture| {
                let (grabbed, available) = unsafe {
                    hp2_platform::mouse_state::update_mouse_grab_state(
                        (*viewport).sdl_window.cast(), capture != 0, (*viewport).mouse_is_grabbed(),
                    )
                };
                unsafe {
                    (*viewport.cast::<UViewport>()).set_b_windows_mouse_available(available);
                    (*viewport).set_mouse_is_grabbed(grabbed);
                }
                Ok(())
            },
        )
    }
    #[cfg(not(feature = "sdl"))]
    {
        let _ = (capture, focus_only);
        Err(RuntimeError::new("Mouse capture requested without the SDL platform feature"))
    }
}
/// Runs source-ordered `UClient::Tick` input updates and repaint selection.
///
/// # Safety
/// `client` and every non-null viewport entry must remain live through the call.
pub(crate) unsafe fn client_tick(
    access: CoreAccess<'_>,
    client: *mut USDLClient,
) -> RuntimeResult<()> {
    if client.is_null() {
        return Err(RuntimeError::new("Null canonical SDL client Tick target"));
    }
    let mut index = 0;
    loop {
        let viewports = unsafe { ptr::addr_of!((*client).base.viewports).read_unaligned() };
        validate_client_viewports(viewports)?;
        if index >= viewports.num { break; }
        let viewport = unsafe {
            viewports.data.cast::<*mut USDLViewport>().add(index as usize).read_unaligned()
        };
        if !viewport.is_null() {
            unsafe { update_input(access, viewport, false)?; }
        }
        index += 1;
    }

    let mut best: *mut UViewport = ptr::null_mut();
    index = 0;
    loop {
        let viewports = unsafe { ptr::addr_of!((*client).base.viewports).read_unaligned() };
        validate_client_viewports(viewports)?;
        if index >= viewports.num { break; }
        let viewport = unsafe {
            viewports.data.cast::<*mut UViewport>().add(index as usize).read_unaligned()
        };
        if !viewport.is_null() {
            const SHOW_REAL_TIME: i32 = hp2_abi::engine::generated::SHOW_REAL_TIME as i32;
            const SHOW_PLAYER_CTRL: i32 = hp2_abi::engine::generated::SHOW_PLAYER_CTRL as i32;
            let actor = unsafe { (*viewport).actor.cast::<hp2_abi::engine::generated::APlayerPawn>() };
            let realtime = !actor.is_null()
                && unsafe { (*actor).show_flags & (SHOW_REAL_TIME | SHOW_PLAYER_CTRL) != 0 };
            let candidate = hp2_platform::input_pump::RepaintCandidate {
                realtime,
                size_x: unsafe { (*viewport).size_x },
                size_y: unsafe { (*viewport).size_y },
                best_minus_candidate: if best.is_null() {
                    0.0
                } else {
                    let best_time = i64::from_ne_bytes(unsafe { (*best).last_update_time });
                    let candidate_time = i64::from_ne_bytes(unsafe { (*viewport).last_update_time });
                    hp2_core::time::subtract(best_time, candidate_time)
                },
            };
            if hp2_platform::input_pump::repaint_replaces_best(!best.is_null(), candidate) {
                best = viewport;
            }
        }
        index += 1;
    }
    if !best.is_null() && unsafe { (*best).size_x != 0 && (*best).size_y != 0 } {
        let engine = unsafe { (*client).base.engine.cast::<UGameEngine>() };
        if engine.is_null() {
            return Err(RuntimeError::new("Canonical UClient has no Engine for viewport repaint"));
        }
        unsafe {
            crate::game_draw_owner::draw(
                access,
                engine,
                best,
                true,
                ptr::null_mut(),
                ptr::null_mut(),
            )?;
        }
    }
    Ok(())
}


/// Creates and initializes the SDL window portion of `USDLViewport::OpenWindow`.
///
/// # Safety
/// `viewport` must be a live registered viewport and SDL video must already be initialized.
pub unsafe fn open_viewport_window(
    access: CoreAccess<'_>,
    viewport: *mut USDLViewport,
    temporary: bool,
    new_x: i32,
    new_y: i32,
    open_x: i32,
    open_y: i32,
) -> RuntimeResult<()> {
    if viewport.is_null() { return Err(RuntimeError::new("Null canonical SDL viewport")); }
    #[cfg(feature = "sdl")]
    {
        let owner = crate::engine_owner::platform_state(access)?;
        if !owner.owns(viewport) { return Err(RuntimeError::new("SDL viewport is not registered")); }
        let client = unsafe { ptr::addr_of!((*viewport).base.outer).read_unaligned().cast::<USDLClient>() };
        if client.is_null() { return Err(RuntimeError::new("SDL viewport has no client")); }
        let cursors = unsafe { ptr::addr_of_mut!((*viewport).system_cursors).cast::<*mut c_void>() };
        for slot in 0..hp2_platform::viewport::NUM_SYSTEM_CURSORS {
            if unsafe { cursors.add(slot as usize).read_unaligned() }.is_null() {
                let cursor = hp2_platform::cursor::create_system_cursor(slot);
                unsafe { cursors.add(slot as usize).write_unaligned(cursor.cast()); }
            }
        }
        let desktop = hp2_platform::viewport::desktop_color_depth(
            hp2_platform::display_mode::desktop_color_bits(),
        );
        unsafe {
            (*viewport).base.color_bytes = desktop.color_bytes;
            if desktop.rgb565 { (*viewport).base.caps |= 1; }
        }
        let width = unsafe { ptr::addr_of!((*client).base.windowed_viewport_x).read_unaligned() };
        let height = unsafe { ptr::addr_of!((*client).base.windowed_viewport_y).read_unaligned() };
        let (width, height) = hp2_platform::viewport::open_window_size(new_x, new_y, open_x, open_y, width, height);
        let existing = unsafe { ptr::addr_of!((*viewport).sdl_window).read_unaligned() };
        if existing.is_null() {
            use hp2_platform::window_lifecycle as windows;
            for (attribute, value) in [
                (windows::SDL_GL_DOUBLEBUFFER, 1),
                (windows::SDL_GL_DEPTH_SIZE, 24),
                (windows::SDL_GL_RED_SIZE, 8),
                (windows::SDL_GL_GREEN_SIZE, 8),
                (windows::SDL_GL_BLUE_SIZE, 8),
            ] { windows::set_gl_attribute(attribute, value); }
            let borderless = unsafe { ptr::addr_of!((*client).borderless_window).read_unaligned() != 0 };
            let window = windows::create_window(width, height, hp2_platform::viewport::window_flags(temporary, borderless));
            if window.is_null() {
                return Err(RuntimeError::new("SDL_CreateWindow failed"));
            }
            unsafe { ptr::addr_of_mut!((*viewport).sdl_window).write_unaligned(window.cast()); }
        }
        unsafe {
            (*viewport).base.size_x = width;
            (*viewport).base.size_y = height;
            (*viewport).base.color_bytes = hp2_platform::viewport::window_color_bytes(
                ptr::addr_of!((*client).base.windowed_color_bits).read_unaligned(),
            );
            (*viewport).blit_flags &= !hp2_platform::viewport::BLIT_FULLSCREEN;
        }
        let startup = unsafe { ptr::addr_of!((*client).startup_fullscreen).read_unaligned() != 0 };
        let desktop = unsafe { ptr::addr_of!((*client).use_desktop_resolution).read_unaligned() != 0 };
        if hp2_platform::viewport::startup_fullscreen(startup, desktop) {
            unsafe {
                (*viewport).blit_flags |= hp2_platform::viewport::BLIT_FULLSCREEN;
                hp2_platform::window_lifecycle::start_fullscreen(
                    ptr::addr_of!((*viewport).sdl_window).read_unaligned().cast(),
                );
            }
        }
        unsafe { (*viewport).set_lost_fullscreen(false); }
        let window = unsafe { ptr::addr_of!((*viewport).sdl_window).read_unaligned().cast() };
        let (window_x, window_y) = unsafe { hp2_platform::window_lifecycle::get_window_size(window) };
        let size_x = unsafe { ptr::addr_of!((*viewport).base.size_x).read_unaligned() };
        let size_y = unsafe { ptr::addr_of!((*viewport).base.size_y).read_unaligned() };
        let (mouse_scale_x, mouse_scale_y) =
            hp2_platform::viewport::mouse_scale(window_x, window_y, size_x, size_y);
        unsafe {
            (*viewport).mouse_scale_x = mouse_scale_x;
            (*viewport).mouse_scale_y = mouse_scale_y;
        }
        let initialized = unsafe {
            crate::render_device_owner::create_for_viewport(
                access,
                viewport,
                size_x,
                size_y,
                (*viewport).base.color_bytes,
                hp2_platform::viewport::is_fullscreen((*viewport).blit_flags),
            )?
        };
        if !initialized { return Err(RuntimeError::new("SDL viewport renderer initialization failed")); }
        Ok(())
    }
    #[cfg(not(feature = "sdl"))]
    {
        let _ = (temporary, new_x, new_y, open_x, open_y);
        Err(RuntimeError::new("SDL viewport requested without the SDL platform feature"))
    }
}

#[cfg(feature = "sdl")]
struct CoreInputBackend<'a> { access: CoreAccess<'a>, viewport: *mut USDLViewport }

#[cfg(feature = "sdl")]
impl CoreInputBackend<'_> {
    fn dispatch_event(&mut self, object: *mut UObject, event: &str, params: *mut c_void) -> RuntimeResult<()> {
        if object.is_null() { return Err(RuntimeError::new("Cannot dispatch SDL input to a null object")); }
        let name = self.access.intern_str(event, hp2_core::name_owner::FindName::Find)?;
        let function = unsafe { self.access.find_object_function_by_name(object, name, false)? };
        if function.is_null() { return Err(RuntimeError::new("SDL input event metadata is unavailable")); }
        unsafe { self.access.execute_event(object, function, params, std::ptr::null_mut()) }
    }
    fn record_replay_input(&mut self, key: i32, state: i32, delta: f32) -> RuntimeResult<()> {
        let client = unsafe { (*self.viewport).base.outer.cast::<USDLClient>().as_ref() }
            .ok_or_else(|| RuntimeError::new("SDL viewport has no client"))?;
        let engine = client.base.engine.cast::<UGameEngine>();
        if engine.is_null() { return Err(RuntimeError::new("SDL client has no game engine")); }
        let replay = unsafe { ptr::addr_of_mut!((*engine).replay) };
        if unsafe { crate::replay_owner::recording(replay) } {
            let _ = unsafe { crate::replay_owner::serialize_frame_input_write(self.access, replay, key, state, delta)? };
        }
        Ok(())
    }
    fn game_engine_input_event(&mut self, key: i32, action: i32, delta: f32) -> RuntimeResult<bool> {
        let client = unsafe { (*self.viewport).base.outer.cast::<USDLClient>().as_ref() }
            .ok_or_else(|| RuntimeError::new("SDL viewport has no client"))?;
        let engine = client.base.engine.cast::<UGameEngine>();
        if engine.is_null() { return Err(RuntimeError::new("SDL client has no game engine")); }
        if action == 1 && key == 46 {
            let level = unsafe { (*engine).g_level.cast::<ULevel>() };
            if level.is_null() { return Err(RuntimeError::new("GameEngine has no level for special-pause input")); }
            unsafe { (*level).b_in_special_pause_mode ^= 1; }
        }
        let replay = unsafe { ptr::addr_of_mut!((*engine).replay) };
        let replay_active = unsafe { !(*replay).file_ar.is_null() };
        let replaying = replay_active && unsafe { (*replay).b_recording == 0 };
        if replaying && unsafe { (*replay).b_in_replay == 0 } {
            if action == 1 && key == 27 {
                let _ = unsafe { crate::replay_owner::stop(self.access, replay)? };
            } else if action == 1 && key == 19 {
                unsafe { crate::replay_owner::pause(replay, !crate::replay_owner::paused(replay), 0); }
            } else if action == 1 && key == 145 && unsafe { crate::replay_owner::paused(replay) } {
                unsafe { crate::replay_owner::pause(replay, true, 1); }
            }
            return Ok(true);
        }
        if action == 1 && key == 220 && unsafe { crate::replay_owner::recording(replay) } {
            let _ = unsafe { crate::replay_owner::stop(self.access, replay)? };
            return Ok(true);
        }
        self.record_replay_input(key, action, delta)?;
        Ok(false)
    }
    fn engine_key(&mut self, key: i32) -> RuntimeResult<()> {
        #[cfg(feature = "in-game-overlay")]
        if crate::in_game_overlay::is_open(self.viewport as usize) { return Ok(()); }
        let console = unsafe { (*self.viewport).base.console.cast::<UObject>() };
        if console.is_null() { return Ok(()); }
        #[repr(C)] struct Params { key: u8, _pad: [u8; 3], result: u32 }
        let mut params = Params { key: key as u8, _pad: [0; 3], result: 0 };
        self.dispatch_event(console, "KeyType", (&mut params as *mut Params).cast())
    }
    fn engine_mouse_position(&mut self, _buttons: u32, x: f32, y: f32) -> RuntimeResult<()> {
        unsafe {
            (*self.viewport).base.windows_mouse_x = x;
            (*self.viewport).base.windows_mouse_y = y;
        }
        Ok(())
    }
    fn resize(&mut self, width: i32, height: i32) -> RuntimeResult<()> {
        let viewport = unsafe { &mut *self.viewport };
        let state = hp2_platform::viewport::resize_viewport(
            hp2_platform::viewport::ViewportSize { blit_flags: viewport.blit_flags, size_x: viewport.base.size_x, size_y: viewport.base.size_y, color_bytes: viewport.base.color_bytes },
            hp2_platform::viewport::BLIT_NO_WINDOW_CHANGE, width, height, hp2_platform::viewport::INDEX_NONE,
        );
        viewport.blit_flags = state.blit_flags;
        viewport.base.size_x = state.size_x;
        viewport.base.size_y = state.size_y;
        viewport.base.color_bytes = state.color_bytes;
        if !viewport.sdl_window.is_null() {
            let window = viewport.sdl_window.cast();
            unsafe {
                hp2_platform::window_lifecycle::set_window_size(window, state.size_x, state.size_y);
                hp2_platform::window_lifecycle::set_window_fullscreen(
                    window,
                    hp2_platform::viewport::is_fullscreen(state.blit_flags),
                );
            }
            let (window_x, window_y) = unsafe { hp2_platform::window_lifecycle::get_window_size(window) };
            (viewport.mouse_scale_x, viewport.mouse_scale_y) =
                hp2_platform::viewport::mouse_scale(window_x, window_y, state.size_x, state.size_y);
        }
        let render_device = unsafe { ptr::addr_of!((*viewport).base.ren_dev).read_unaligned() };
        if !render_device.is_null() {
            let resized = unsafe {
                crate::render_device_owner::set_res(
                    self.access,
                    render_device.cast(),
                    ptr::addr_of_mut!((*viewport).base),
                    state.size_x,
                    state.size_y,
                    state.color_bytes,
                    hp2_platform::viewport::is_fullscreen(state.blit_flags),
                )?
            };
            if !resized { return Err(RuntimeError::new("SDL viewport renderer resize failed")); }
        }
        Ok(())
    }
}
#[cfg(feature = "sdl")]
impl hp2_platform::input_pump::UpdateInputBackend for CoreInputBackend<'_> {
    type Error = RuntimeError;
    fn bind_client(&mut self) -> RuntimeResult<()> {
        if unsafe { (*self.viewport).base.outer }.is_null() { return Err(RuntimeError::new("SDL viewport has no client")); }
        Ok(())
    }
    fn mouse_is_grabbed(&mut self) -> RuntimeResult<bool> { Ok(unsafe { (*self.viewport).mouse_is_grabbed() }) }
    fn context(&mut self) -> RuntimeResult<hp2_platform::input_pump::Context> {
        let vp = unsafe { &*self.viewport };
        let base = unsafe { &*self.viewport.cast::<UViewport>() };
        let client = unsafe { base.outer.cast::<USDLClient>().as_ref() }.ok_or_else(|| RuntimeError::new("SDL viewport has no client"))?;
        let client_base = unsafe { &*base.outer.cast::<UClient>() };
        Ok(hp2_platform::input_pump::Context {
            mouse_is_grabbed: vp.mouse_is_grabbed(), lost_grab: vp.lost_grab(), capture_mouse: client_base.capture_mouse(),
            has_window: !vp.sdl_window.is_null(), has_engine: !client_base.engine.is_null(), has_actor: !base.actor.is_null(),
            joy_hat_folds_to_buttons: client.joy_hat_folds_to_buttons != 0, mouse_scale_x: vp.mouse_scale_x, mouse_scale_y: vp.mouse_scale_y,
            size_x: base.size_x, size_y: base.size_y, joystick_dead_zone: client.joystick_dead_zone, scale_jbx: client.scale_jbx,
            scale_jby: client.scale_jby, last_joy_hat: vp.last_joy_hat,
        })
    }
    fn has_input(&mut self) -> RuntimeResult<bool> { Ok(!unsafe { (*self.viewport).base.input }.is_null()) }
    fn game_time(&mut self) -> RuntimeResult<f32> {
        let actor = unsafe { (*self.viewport.cast::<UViewport>()).actor.cast::<AActor>() };
        if actor.is_null() { return Ok(0.0); }
        let level = unsafe { (*actor).level.cast::<ALevelInfo>() };
        if level.is_null() { return Ok(0.0); }
        Ok(unsafe { (*level).time_seconds })
    }
    fn window(&mut self) -> RuntimeResult<*mut c_void> { Ok(unsafe { (*self.viewport).sdl_window }) }
    fn key_down(&mut self, key: i32) -> RuntimeResult<bool> {
        if key < 0 || key >= 255 { return Ok(false); }
        let input = unsafe { (*self.viewport).base.input.cast::<UInput>() };
        if input.is_null() { return Err(RuntimeError::new("SDL viewport has no input")); }
        Ok(unsafe { (*input).key_down_table[key as usize] != 0 })
    }
    fn cause_input(&mut self, key: i32, action: i32, delta: f32) -> RuntimeResult<()> {
        let input = unsafe { (*self.viewport).base.input.cast::<UInput>() };
        if input.is_null() { return Err(RuntimeError::new("SDL viewport has no input")); }
        let access = self.access;
        let mut execute_input = || -> RuntimeResult<()> {
            let category = access.intern_str("Log", hp2_core::name_owner::FindName::Find)?;
            let mut output = |message: &[u32]| access.emit_log(category, message);
            unsafe { crate::input_owner::process(access, input, key, action, delta, &mut output)?; }
            Ok(())
        };
        #[cfg(feature = "in-game-overlay")]
        {
            // Replay modes retain their control keys and cannot open local UI.
            let client = unsafe { (*self.viewport).base.outer.cast::<UClient>() };
            if client.is_null() { return Err(RuntimeError::new("SDL viewport has no client")); }
            let engine = unsafe { (*client).engine.cast::<UGameEngine>() };
            let replay_active = !engine.is_null() && unsafe { !(*ptr::addr_of!((*engine).replay)).file_ar.is_null() };
            if !replay_active {
                let viewport = self.viewport as usize;
                let handled = crate::in_game_overlay::event(viewport, key, action);
                if crate::in_game_overlay::is_open(viewport) {
                    unsafe { crate::overlay_settings_provider::sync(self.access, viewport, &mut *client); }
                }
                if handled { return Ok(()); }
            }
        }
        if self.game_engine_input_event(key, action, delta)? {
            return Ok(());
        }
        let console = unsafe { (*self.viewport).base.console.cast::<UObject>() };
        if !console.is_null() {
            #[repr(C)] struct Params { key: u8, state: u8, delta: f32, result: u32 }
            let mut params = Params { key: key as u8, state: action as u8, delta, result: 0 };
            self.dispatch_event(console, "KeyEvent", (&mut params as *mut Params).cast())?;
            if params.result != 0 {
                if action == crate::input::IST_RELEASE
                    && unsafe { crate::input::pre_process(input, key, action) != 0 }
                {
                    execute_input()?;
                }
                return Ok(());
            }
        }
        if unsafe { crate::input::pre_process(input, key, action) != 0 } {
            execute_input()?;
        }
        Ok(())
    }
    fn execute_action(&mut self, action: hp2_platform::input_pump::Action) -> RuntimeResult<()> {
        match action {
            hp2_platform::input_pump::Action::Cause { key, action, delta } => self.cause_input(key, action, delta),
            hp2_platform::input_pump::Action::SetLastJoyHat(key) => { unsafe { (*self.viewport).last_joy_hat = key; } Ok(()) }
            hp2_platform::input_pump::Action::EngineKey { key } => self.engine_key(key),
            hp2_platform::input_pump::Action::EngineMousePosition { buttons, x, y } => self.engine_mouse_position(buttons, x, y),
            hp2_platform::input_pump::Action::UpdateMouseGrab { capture } => {
                let viewport = self.viewport;
                let (grabbed, available) = unsafe {
                    hp2_platform::mouse_state::update_mouse_grab_state(
                        (*viewport).sdl_window.cast(),
                        capture,
                        (*viewport).mouse_is_grabbed(),
                    )
                };
                unsafe {
                    (*viewport.cast::<UViewport>()).set_b_windows_mouse_available(available);
                    (*viewport).set_mouse_is_grabbed(grabbed);
                }
                Ok(())
            }
            hp2_platform::input_pump::Action::SetLostGrab(value) => { unsafe { (*self.viewport).set_lost_grab(value); } Ok(()) }
            hp2_platform::input_pump::Action::ResizeViewport { width, height } => self.resize(width, height),
            hp2_platform::input_pump::Action::ReleaseAll =>
                unreachable!("shared input executor consumes ReleaseAll before side effects"),
        }
    }
    fn set_last_joy_hat(&mut self, key: i32) -> RuntimeResult<()> { unsafe { (*self.viewport).last_joy_hat = key; } Ok(()) }
    fn before_release_all(&mut self) -> RuntimeResult<()> {
        #[cfg(feature = "in-game-overlay")]
        crate::in_game_overlay::focus_lost(self.viewport as usize);
        Ok(())
    }
}

/// SDLViewport::CreateNativeFont, or UViewport's intentional null default.
///
/// # Safety
/// Viewport and NUL-terminated font name belong to the live canonical graph.
pub(crate) unsafe fn create_native_font(
    access: CoreAccess<'_>, viewport: *mut UViewport, name: *const u32, height: i32,
) -> RuntimeResult<*mut c_void> {
    if viewport.is_null() { return Err(RuntimeError::new("CreateNativeFont has no viewport")); }
    let sdl = access.native_class("SDLDrv.SDLViewport")?;
    if unsafe { !hp2_core::object_model::is_a(viewport.cast(), sdl.cast()) } { return Ok(ptr::null_mut()); }
    hp2_platform::viewport_policy::create_native_font_with(
        || unsafe {
            access.construct_object(access.native_class("Engine.Font")?,
                (*viewport.cast::<UObject>()).outer.cast(), 0, 0, ptr::null_mut()).map(|font| font.cast())
        },
        |font| unsafe {
            let font = font.cast::<hp2_abi::engine::generated::UFont>();
            let mut length = 0;
            while *name.add(length) != 0 { length += 1; }
            access.write_vm_fstring_result(ptr::addr_of_mut!((*font).font_name), std::slice::from_raw_parts(name.cast(), length))?;
            (*font).font_height = height;
            (*font).native_font = ptr::null_mut();
            Ok(())
        },
    )
}
