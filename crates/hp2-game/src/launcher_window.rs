use egui::{Event, Key, Modifiers, PointerButton, RawInput};
use egui_wgpu::{Renderer, RendererOptions};
use hp2_core::core_access::{RuntimeError, RuntimeResult};
use hp2_platform::input_pump::{
    ffi::{SdlEventRaw, SdlTextEditingEvent},
    sdl_event, sdl_window_event,
};
use std::{ffi::CString, time::Instant};

type Window = hp2_platform::window_lifecycle::SdlWindow;
#[repr(C)]
struct MetalView {
    _private: [u8; 0],
}
#[repr(C)]
struct SdlRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
unsafe extern "C" {
    fn SDL_PollEvent(event: *mut SdlEventRaw) -> i32;
    fn SDL_StartTextInput();
    fn SDL_StopTextInput();
    fn SDL_Metal_CreateView(window: *mut Window) -> *mut MetalView;
    fn SDL_Metal_DestroyView(view: *mut MetalView);
    fn SDL_SetCursor(cursor: *mut std::ffi::c_void);
    fn SDL_CreateSystemCursor(id: i32) -> *mut std::ffi::c_void;
    fn SDL_FreeCursor(cursor: *mut std::ffi::c_void);
    fn SDL_SetClipboardText(text: *const std::ffi::c_char) -> i32;
    fn SDL_GetClipboardText() -> *mut std::ffi::c_char;
    fn SDL_free(mem: *mut std::ffi::c_void);
    fn SDL_SetTextInputRect(rect: *const SdlRect);
    fn SDL_OpenURL(url: *const std::ffi::c_char) -> i32;
    fn SDL_ShowCursor(toggle: i32) -> i32;
    fn SDL_RaiseWindow(window: *mut Window);
    fn SDL_CreateRGBSurfaceFrom(
        pixels: *mut std::ffi::c_void,
        width: i32,
        height: i32,
        depth: i32,
        pitch: i32,
        r: u32,
        g: u32,
        b: u32,
        a: u32,
    ) -> *mut std::ffi::c_void;
    fn SDL_FreeSurface(surface: *mut std::ffi::c_void);
    fn SDL_RWFromFile(
        path: *const std::ffi::c_char,
        mode: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
    fn SDL_SaveBMP_RW(
        surface: *mut std::ffi::c_void,
        destination: *mut std::ffi::c_void,
        free_destination: i32,
    ) -> i32;
}

fn smoke_frame_limit() -> Option<u32> {
    // Match the existing launcher smoke's strtol/end-pointer contract.
    unsafe {
        let text = libc::getenv(c"HP2_LAUNCHER_SMOKE_FRAMES".as_ptr());
        if text.is_null() {
            return None;
        }
        let mut end = std::ptr::null_mut();
        let frames = libc::strtol(text, &mut end, 10);
        (end != text && *end == 0 && frames > 0 && frames < 100000).then_some(frames as u32)
    }
}

fn save_launcher_bitmap(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixels: &mut [u8],
) -> RuntimeResult<()> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| error("launcher capture path contains NUL"))?;
    let pitch = width
        .checked_mul(4)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| error("launcher bitmap row size exceeds SDL limits"))?;
    let surface = unsafe {
        SDL_CreateRGBSurfaceFrom(
            pixels.as_mut_ptr().cast(),
            width as i32,
            height as i32,
            32,
            pitch,
            u32::from_ne_bytes([255, 0, 0, 0]),
            u32::from_ne_bytes([0, 255, 0, 0]),
            u32::from_ne_bytes([0, 0, 255, 0]),
            u32::from_ne_bytes([0, 0, 0, 255]),
        )
    };
    if surface.is_null() {
        return Err(error("SDL failed to create launcher capture bitmap"));
    }
    let destination = unsafe { SDL_RWFromFile(path.as_ptr(), c"wb".as_ptr()) };
    let saved = !destination.is_null() && unsafe { SDL_SaveBMP_RW(surface, destination, 1) == 0 };
    unsafe {
        SDL_FreeSurface(surface);
    }
    if saved {
        Ok(())
    } else {
        Err(error("SDL failed to save launcher capture bitmap"))
    }
}

struct OwnedWindow(*mut Window);
impl Drop for OwnedWindow {
    fn drop(&mut self) {
        unsafe { hp2_platform::window_lifecycle::destroy_window(self.0) }
    }
}
struct Metal(*mut MetalView);
impl Drop for Metal {
    fn drop(&mut self) {
        unsafe { SDL_Metal_DestroyView(self.0) }
    }
}
struct TextInput;
impl Drop for TextInput {
    fn drop(&mut self) {
        unsafe { SDL_StopTextInput() }
    }
}
struct CursorVisibility(i32);
impl Drop for CursorVisibility {
    fn drop(&mut self) {
        unsafe {
            SDL_ShowCursor(self.0);
        }
    }
}
struct Cursor(*mut std::ffi::c_void);
impl Drop for Cursor {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                SDL_FreeCursor(self.0)
            }
        }
    }
}
fn error(s: &str) -> RuntimeError {
    RuntimeError::new(s)
}
fn modifiers(bits: u16) -> Modifiers {
    const SHIFT: u16 = 3;
    const CTRL: u16 = 0xc0;
    const ALT: u16 = 0x300;
    const GUI: u16 = 0xc00;
    let ctrl = bits & CTRL != 0;
    let command = if cfg!(target_os = "macos") {
        bits & GUI != 0
    } else {
        ctrl
    };
    Modifiers {
        alt: bits & ALT != 0,
        ctrl,
        shift: bits & SHIFT != 0,
        mac_cmd: cfg!(target_os = "macos") && command,
        command,
    }
}
fn key(s: i32) -> Option<Key> {
    Some(match s {
        27 => Key::Escape,
        13 => Key::Enter,
        9 => Key::Tab,
        8 => Key::Backspace,
        127 => Key::Delete,
        32 => Key::Space,
        1_073_741_903 => Key::ArrowRight,
        1_073_741_904 => Key::ArrowLeft,
        1_073_741_905 => Key::ArrowDown,
        1_073_741_906 => Key::ArrowUp,
        1_073_741_898 => Key::Home,
        1_073_741_899 => Key::End,
        1_073_741_897 => Key::PageUp,
        1_073_741_896 => Key::PageDown,
        1_073_741_902 => Key::Insert,
        58 => Key::Colon,
        44 => Key::Comma,
        92 => Key::Backslash,
        47 => Key::Slash,
        124 => Key::Pipe,
        63 => Key::Questionmark,
        33 => Key::Exclamationmark,
        91 => Key::OpenBracket,
        93 => Key::CloseBracket,
        123 => Key::OpenCurlyBracket,
        125 => Key::CloseCurlyBracket,
        96 => Key::Backtick,
        45 => Key::Minus,
        46 => Key::Period,
        43 => Key::Plus,
        61 => Key::Equals,
        59 => Key::Semicolon,
        39 => Key::Quote,
        97 | 65 => Key::A,
        98 | 66 => Key::B,
        99 | 67 => Key::C,
        100 | 68 => Key::D,
        101 | 69 => Key::E,
        102 | 70 => Key::F,
        103 | 71 => Key::G,
        104 | 72 => Key::H,
        105 | 73 => Key::I,
        106 | 74 => Key::J,
        107 | 75 => Key::K,
        108 | 76 => Key::L,
        109 | 77 => Key::M,
        110 | 78 => Key::N,
        111 | 79 => Key::O,
        112 | 80 => Key::P,
        113 | 81 => Key::Q,
        114 | 82 => Key::R,
        115 | 83 => Key::S,
        116 | 84 => Key::T,
        117 | 85 => Key::U,
        118 | 86 => Key::V,
        119 | 87 => Key::W,
        120 | 88 => Key::X,
        121 | 89 => Key::Y,
        122 | 90 => Key::Z,
        48..=57 => [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ][(s - 48) as usize],
        1_073_741_882..=1_073_741_893 => [
            Key::F1,
            Key::F2,
            Key::F3,
            Key::F4,
            Key::F5,
            Key::F6,
            Key::F7,
            Key::F8,
            Key::F9,
            Key::F10,
            Key::F11,
            Key::F12,
        ][(s - 1_073_741_882) as usize],
        _ => return None,
    })
}
fn text(bytes: &[u8]) -> &str {
    let n = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..n]).unwrap_or("")
}
fn cursor_id(c: egui::CursorIcon) -> Option<i32> {
    Some(match c {
        egui::CursorIcon::None => return None,
        egui::CursorIcon::Default
        | egui::CursorIcon::ContextMenu
        | egui::CursorIcon::Help
        | egui::CursorIcon::ZoomIn
        | egui::CursorIcon::ZoomOut => 0,
        egui::CursorIcon::PointingHand
        | egui::CursorIcon::Alias
        | egui::CursorIcon::Copy
        | egui::CursorIcon::Grab
        | egui::CursorIcon::Grabbing => 11,
        egui::CursorIcon::Progress => 4,
        egui::CursorIcon::Wait => 2,
        egui::CursorIcon::Cell | egui::CursorIcon::Crosshair => 3,
        egui::CursorIcon::Text | egui::CursorIcon::VerticalText => 1,
        egui::CursorIcon::Move | egui::CursorIcon::AllScroll => 9,
        egui::CursorIcon::NoDrop | egui::CursorIcon::NotAllowed => 10,
        egui::CursorIcon::ResizeHorizontal
        | egui::CursorIcon::ResizeEast
        | egui::CursorIcon::ResizeWest
        | egui::CursorIcon::ResizeColumn => 7,
        egui::CursorIcon::ResizeNwSe
        | egui::CursorIcon::ResizeSouthEast
        | egui::CursorIcon::ResizeNorthWest => 5,
        egui::CursorIcon::ResizeNeSw
        | egui::CursorIcon::ResizeSouthWest
        | egui::CursorIcon::ResizeNorthEast => 6,
        egui::CursorIcon::ResizeVertical
        | egui::CursorIcon::ResizeSouth
        | egui::CursorIcon::ResizeNorth
        | egui::CursorIcon::ResizeRow => 8,
    })
}
fn mouse_button(b: u8) -> Option<PointerButton> {
    Some(match b {
        1 => PointerButton::Primary,
        2 => PointerButton::Middle,
        3 => PointerButton::Secondary,
        4 => PointerButton::Extra1,
        5 => PointerButton::Extra2,
        _ => return None,
    })
}

/// Creates a visible SDL window and presents egui through its native Metal/WGPU surface.
/// SDL video is owned by boot services and remains initialized after this function returns.
pub(crate) fn run<T>(
    title: &str,
    width: i32,
    height: i32,
    mut draw: impl FnMut(&mut egui::Ui) -> RuntimeResult<Option<T>>,
) -> RuntimeResult<Option<T>> {
    let title =
        CString::new(title).map_err(|_| error("launcher title contains an embedded NUL"))?;
    let ptr = hp2_platform::window_lifecycle::create_launcher_window(&title, width, height);
    if ptr.is_null() {
        return Err(error("SDL_CreateWindow failed for launcher"));
    }
    let window = OwnedWindow(ptr);
    unsafe {
        SDL_RaiseWindow(window.0);
    }
    let view = unsafe { SDL_Metal_CreateView(window.0) };
    if view.is_null() {
        return Err(error("SDL_Metal_CreateView failed for launcher"));
    }
    let metal = Metal(view);
    let create = |i: &wgpu::Instance| unsafe {
        hp2_wgpu::macos::create_surface_from_ns_view(i, metal.0.cast())
    };
    let mut gpu = hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, Some(create))
        .ok_or_else(|| error("no WGPU adapter supports launcher surface"))?;
    let caps = gpu
        .surface
        .as_ref()
        .ok_or_else(|| error("WGPU launcher surface was not created"))?
        .get_capabilities(&gpu.adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| caps.formats.first().copied())
        .ok_or_else(|| error("launcher surface has no formats"))?;
    let alpha_mode = *caps
        .alpha_modes
        .first()
        .ok_or_else(|| error("launcher surface has no alpha modes"))?;
    let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        *caps
            .present_modes
            .first()
            .ok_or_else(|| error("launcher surface has no present modes"))?
    };
    let smoke_limit = smoke_frame_limit();
    let capture_path = smoke_limit
        .and_then(|_| std::env::var_os("HP2_ARTIFACT_DIR"))
        .filter(|value| !value.is_empty())
        .map(|value| {
            let mut path = std::path::PathBuf::from(value);
            path.push("launcher.bmp");
            path
        });
    let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
    if capture_path.is_some() {
        if !caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            return Err(error(
                "launcher surface does not support smoke capture readback",
            ));
        }
        usage |= wgpu::TextureUsages::COPY_SRC;
    }
    let mut presented_frames = 0u32;
    let ((lw, lh), (dw, dh)) =
        unsafe { hp2_platform::window_lifecycle::launcher_window_sizes(window.0) };
    if lw <= 0 || lh <= 0 || dw <= 0 || dh <= 0 {
        return Err(error("launcher has zero drawable size"));
    }
    let mut config = wgpu::SurfaceConfiguration {
        usage,
        format,
        width: dw as u32,
        height: dh as u32,
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    gpu.surface
        .as_ref()
        .unwrap()
        .configure(&gpu.device, &config);
    let mut renderer = Renderer::new(&gpu.device, format, RendererOptions::default());
    let context = egui::Context::default();
    let mut events = Vec::with_capacity(64);
    let mut focused = true;
    let mut minimized = false;
    let mut closed = false;
    let start = Instant::now();
    let mut previous = start;
    unsafe { SDL_StartTextInput() };
    let _text = TextInput;
    let old_cursor_visibility = unsafe { SDL_ShowCursor(-1) };
    if old_cursor_visibility < 0 {
        return Err(error("SDL failed to query launcher cursor visibility"));
    }
    let _cursor_visibility = CursorVisibility(old_cursor_visibility);
    let cursors: Vec<Cursor> = (0..=11)
        .map(|id| Cursor(unsafe { SDL_CreateSystemCursor(id) }))
        .collect();
    if cursors.iter().any(|cursor| cursor.0.is_null()) {
        return Err(error("SDL failed to create launcher system cursors"));
    }
    let mut current_modifiers = Modifiers::default();
    while !closed {
        events.clear();
        let mut raw = SdlEventRaw { padding: [0; 56] };
        while unsafe { SDL_PollEvent(&mut raw) } != 0 {
            let kind = unsafe { raw.kind };
            match kind {
                sdl_event::TEXTEDITING => {
                    let e = unsafe {
                        (&raw as *const SdlEventRaw)
                            .cast::<SdlTextEditingEvent>()
                            .read_unaligned()
                    };
                    let composition = text(&e.text).to_owned();
                    let count = composition.chars().count();
                    let a = (e.start.max(0) as usize).min(count);
                    let b = (e.start.saturating_add(e.length).max(0) as usize).min(count);
                    events.push(Event::Ime(egui::ImeEvent::Preedit {
                        text: composition,
                        active_range_chars: Some(a..b),
                    }));
                }
                sdl_event::QUIT => closed = true,
                sdl_event::WINDOWEVENT => {
                    let e = unsafe { raw.window };
                    match e.event {
                        sdl_window_event::CLOSE => closed = true,
                        sdl_window_event::FOCUS_GAINED => {
                            focused = true;
                            events.push(Event::WindowFocused(true));
                        }
                        sdl_window_event::FOCUS_LOST => {
                            focused = false;
                            current_modifiers = Modifiers::default();
                            events.push(Event::ModifiersChanged(current_modifiers));
                            events.push(Event::WindowFocused(false));
                        }
                        sdl_window_event::MINIMIZED => minimized = true,
                        sdl_window_event::RESTORED | sdl_window_event::SHOWN => {
                            minimized = false;
                            focused =
                                hp2_platform::window_lifecycle::launcher_window_flags(window.0)
                                    & 0x0000_0200
                                    != 0;
                        }
                        sdl_window_event::RESIZED => {}
                        _ => {}
                    }
                }
                sdl_event::KEYDOWN | sdl_event::KEYUP => {
                    let e = unsafe { raw.key };
                    let mods = modifiers(e.keysym.modifiers);
                    if mods != current_modifiers {
                        current_modifiers = mods;
                        events.push(Event::ModifiersChanged(mods));
                    }
                    if kind == sdl_event::KEYDOWN
                        && mods.command
                        && (e.keysym.sym == 118 || e.keysym.sym == 86)
                    {
                        let p = unsafe { SDL_GetClipboardText() };
                        if !p.is_null() {
                            let pasted = unsafe { std::ffi::CStr::from_ptr(p) }
                                .to_string_lossy()
                                .into_owned();
                            unsafe { SDL_free(p.cast()) };
                            events.push(Event::Paste(pasted));
                        }
                    } else if kind == sdl_event::KEYDOWN
                        && mods.command
                        && (e.keysym.sym == 99 || e.keysym.sym == 67)
                    {
                        events.push(Event::Copy);
                    } else if kind == sdl_event::KEYDOWN
                        && mods.command
                        && (e.keysym.sym == 120 || e.keysym.sym == 88)
                    {
                        events.push(Event::Cut);
                    }
                    if let Some(k) = key(e.keysym.sym) {
                        events.push(Event::Key {
                            key: k,
                            physical_key: None,
                            pressed: kind == sdl_event::KEYDOWN,
                            repeat: e.repeat != 0,
                            modifiers: mods,
                        });
                    }
                }
                sdl_event::TEXTINPUT => {
                    let e = unsafe { raw.text };
                    let s = text(&e.text);
                    if !s.is_empty() {
                        events.push(Event::Text(s.to_owned()));
                    }
                }
                sdl_event::MOUSEMOTION => {
                    let e = unsafe { raw.motion };
                    events.push(Event::PointerMoved(egui::pos2(e.x as f32, e.y as f32)));
                }
                sdl_event::MOUSEBUTTONDOWN | sdl_event::MOUSEBUTTONUP => {
                    let e = unsafe { raw.button };
                    if let Some(b) = mouse_button(e.button) {
                        events.push(Event::PointerButton {
                            pos: egui::pos2(e.x as f32, e.y as f32),
                            button: b,
                            pressed: kind == sdl_event::MOUSEBUTTONDOWN,
                            modifiers: current_modifiers,
                        });
                    }
                }
                sdl_event::MOUSEWHEEL => {
                    let e = unsafe { raw.wheel };
                    let sign = if e.direction == 1 { -1.0 } else { 1.0 };
                    events.push(Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(e.precise_x, e.precise_y) * sign,
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        modifiers: current_modifiers,
                    });
                }
                _ => {}
            }
        }
        if closed {
            break;
        }
        if minimized {
            std::thread::sleep(std::time::Duration::from_millis(20));
            continue;
        }
        let ((lw, lh), (dw, dh)) =
            unsafe { hp2_platform::window_lifecycle::launcher_window_sizes(window.0) };
        if lw <= 0 || lh <= 0 || dw <= 0 || dh <= 0 {
            minimized = true;
            continue;
        }
        if config.width != dw as u32 || config.height != dh as u32 {
            config.width = dw as u32;
            config.height = dh as u32;
            gpu.surface
                .as_ref()
                .unwrap()
                .configure(&gpu.device, &config);
        }
        let now = Instant::now();
        let dt = (now - previous).as_secs_f32().clamp(0.001, 0.25);
        previous = now;
        let ppp = dw as f32 / lw as f32;
        let mut raw_input = RawInput::default();
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(lw as f32, lh as f32),
        ));
        raw_input.time = Some(now.duration_since(start).as_secs_f64());
        raw_input.predicted_dt = dt;
        raw_input.events = std::mem::take(&mut events);
        raw_input.focused = focused;
        let viewport_id = raw_input.viewport_id;
        raw_input
            .viewports
            .get_mut(&viewport_id)
            .unwrap()
            .native_pixels_per_point = Some(ppp);
        let mut result = None;
        let mut out = context.run_ui(raw_input, |ui| {
            if result
                .as_ref()
                .is_none_or(|value| matches!(value, Ok(None)))
            {
                result = Some(draw(ui));
            }
        });
        match result {
            Some(Ok(None)) => {}
            Some(Ok(Some(value))) => {
                // The window is closing; this frame will not be painted.
                out.textures_delta.clear();
                return Ok(Some(value));
            }
            Some(Err(failure)) => {
                out.textures_delta.clear();
                return Err(failure);
            }
            None => {
                out.textures_delta.clear();
                return Err(error("egui launcher callback did not execute"));
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point: ppp,
        };
        for (id, deltas) in out.textures_delta.set.drain() {
            for delta in deltas {
                renderer.update_texture(&gpu.device, &gpu.queue, id, &delta);
            }
        }
        let jobs = context.tessellate(out.shapes, ppp);
        let paint = (|| -> RuntimeResult<bool> {
            hp2_wgpu::macos::flush_pending_layer_changes();
            let current = gpu
                .surface
                .as_ref()
                .ok_or_else(|| error("launcher surface is unavailable"))?
                .get_current_texture();
            let (frame, suboptimal) = match current {
                wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                wgpu::CurrentSurfaceTexture::Outdated => {
                    gpu.surface
                        .as_ref()
                        .unwrap()
                        .configure(&gpu.device, &config);
                    return Ok(false);
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    let surface = unsafe {
                        hp2_wgpu::macos::create_surface_from_ns_view(&gpu.instance, metal.0.cast())
                    }
                    .map_err(|_| error("failed to recreate lost launcher surface"))?;
                    gpu.surface = Some(surface);
                    gpu.surface
                        .as_ref()
                        .unwrap()
                        .configure(&gpu.device, &config);
                    return Ok(false);
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(false);
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(error(
                        "WGPU validation error while acquiring launcher surface",
                    ));
                }
            };
            let target = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui launcher frame"),
                });
            let extra =
                renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui launcher pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
            }
            gpu.queue
                .submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
            let smoke_complete = smoke_limit.is_some_and(|limit| presented_frames + 1 >= limit);
            let capture = if smoke_complete && capture_path.is_some() {
                Some(
                    hp2_wgpu::capture::read_texture_rgba8(&gpu, &frame.texture).ok_or_else(
                        || error("could not read the actual launcher surface texture"),
                    )?,
                )
            } else {
                None
            };
            gpu.queue.present(frame);
            if smoke_limit.is_some() {
                presented_frames += 1;
            }
            if let (Some(path), Some(mut pixels)) = (capture_path.as_ref(), capture) {
                save_launcher_bitmap(path, config.width, config.height, &mut pixels)?;
            }
            if suboptimal {
                gpu.surface
                    .as_ref()
                    .unwrap()
                    .configure(&gpu.device, &config);
            }
            Ok(smoke_complete)
        })();
        // Retire commands even if acquisition skipped painting or failed.
        for id in out.textures_delta.free.drain() {
            renderer.free_texture(&id);
        }
        if paint? {
            eprintln!("HP2_LAUNCHER_SMOKE frames={presented_frames}");
            return Ok(None);
        }
        for command in out.platform_output.commands {
            match command {
                egui::OutputCommand::CopyText(value) => {
                    let value =
                        CString::new(value).map_err(|_| error("clipboard text contains NUL"))?;
                    if unsafe { SDL_SetClipboardText(value.as_ptr()) } != 0 {
                        return Err(error("SDL failed to set launcher clipboard text"));
                    }
                }
                egui::OutputCommand::TextSelectionSettled(_) => {}
                egui::OutputCommand::OpenUrl(url) => {
                    let url = CString::new(url.url)
                        .map_err(|_| error("launcher URL contains an embedded NUL"))?;
                    if unsafe { SDL_OpenURL(url.as_ptr()) } != 0 {
                        return Err(error("SDL failed to open launcher URL"));
                    }
                }
                egui::OutputCommand::CopyImage(_) => {
                    return Err(error("SDL launcher clipboard does not support image data"));
                }
            }
        }
        if let Some(id) = cursor_id(out.platform_output.cursor_icon) {
            if unsafe { SDL_ShowCursor(1) } < 0 {
                return Err(error("SDL failed to show launcher cursor"));
            }
            if let Some(cursor) = cursors.get(id as usize) {
                unsafe {
                    SDL_SetCursor(cursor.0);
                }
            }
        } else if unsafe { SDL_ShowCursor(0) } < 0 {
            return Err(error("SDL failed to hide launcher cursor"));
        }
        if let Some(rect) = out.platform_output.ime.as_ref().map(|ime| ime.rect) {
            let rect = SdlRect {
                x: rect.min.x as i32,
                y: rect.min.y as i32,
                w: rect.width() as i32,
                h: rect.height() as i32,
            };
            unsafe {
                SDL_SetTextInputRect(&rect);
            }
        }
    }
    Ok(None)
}
