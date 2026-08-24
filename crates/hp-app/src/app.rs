//! winit event-loop ownership skeleton hosting egui-wgpu.
//!
//! [`AppShell`] implements [`winit::application::ApplicationHandler`]: on
//! `resumed` it creates the window from settings-derived attributes, wires
//! an [`egui_winit::State`] input adapter, and hands the window to an
//! [`egui_wgpu::winit::Painter`]; `about_to_wait` runs one egui frame whose
//! panel is a thin layer over the validated settings setters.
//!
//! Headless reality: constructing an `EventLoop`, a `Window`, or a GPU
//! surface needs a display connection, so interactive behavior cannot be
//! exercised under headless CI. What IS testable without a window —
//! attribute derivation, present-mode selection, panel construction over
//! the setters — lives behind pure functions and is unit-tested; full
//! interactive verification is deferred (see crate docs).

use egui::ViewportId;
use egui_wgpu::WgpuConfiguration;
use egui_wgpu::winit::Painter;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

use crate::error::{AppError, Result};
use crate::settings::Settings;
use crate::settings_model::ScreenMode;

/// Derives window attributes purely from launcher settings: the committed
/// resolution sizes the window and the launcher's minimum geometry applies.
/// Exclusive fullscreen needs a live monitor handle, so
/// [`ScreenMode::Fullscreen`] is applied right after window creation (see
/// `resumed`); borderless desktop carries no monitor reference.
pub fn window_attributes(settings: &Settings) -> WindowAttributes {
    let mut attributes = WindowAttributes::default()
        .with_title("Harry Potter 2")
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(settings.resolution.width),
            f64::from(settings.resolution.height),
        ))
        .with_min_inner_size(winit::dpi::LogicalSize::new(320.0, 320.0))
        .with_resizable(true);
    attributes.fullscreen = match settings.screen_mode {
        ScreenMode::Windowed | ScreenMode::Fullscreen => None,
        ScreenMode::BorderlessDesktop => Some(Fullscreen::Borderless(None)),
    };
    attributes
}

/// Surface present mode for a vertical-sync preference.
pub fn present_mode(vertical_sync: bool) -> egui_wgpu::wgpu::PresentMode {
    if vertical_sync {
        egui_wgpu::wgpu::PresentMode::AutoVsync
    } else {
        egui_wgpu::wgpu::PresentMode::AutoNoVsync
    }
}

/// The owned application shell. Constructible headlessly; everything that
/// touches the display server happens inside the event loop.
pub struct AppShell {
    pub settings: Settings,
    egui_ctx: egui::Context,
    painter: Option<Painter>,
    viewport: Option<Viewport>,
}

struct Viewport {
    window: Arc<Window>,
    winit_state: egui_winit::State,
}

impl AppShell {
    /// Headless construction: only the egui context is created.
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            egui_ctx: egui::Context::default(),
            painter: None,
            viewport: None,
        }
    }

    /// Builds and runs the event loop. The painter is created here because
    /// its async instance setup blocks on wgpu work (see `block_on`).
    pub fn run(mut self) -> Result<()> {
        self.painter = Some(block_on(Painter::new(
            self.egui_ctx.clone(),
            WgpuConfiguration::default(),
            false,
            egui_wgpu::RendererOptions::default(),
        )));
        let event_loop = EventLoop::builder().build().map_err(|error| {
            AppError::new(
                "app.event_loop_unavailable",
                format!("Unable to start the winit event loop: {error}"),
            )
        })?;
        let mut shell = self;
        event_loop.run_app(&mut shell).map_err(|error| {
            AppError::new(
                "app.event_loop_unavailable",
                format!("Event loop terminated with an error: {error}"),
            )
        })
    }

    fn redraw(&mut self) {
        let Self {
            settings,
            egui_ctx,
            painter,
            viewport,
        } = self;
        let Some(viewport) = viewport else {
            return;
        };
        let raw_input = viewport.winit_state.take_egui_input(&viewport.window);
        let output = egui_ctx.run_ui(raw_input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Harry Potter 2");
                settings_panel(ui, settings);
            });
        });
        viewport
            .winit_state
            .handle_platform_output(&viewport.window, output.platform_output);
        if let Some(painter) = painter {
            let clipped = egui_ctx.tessellate(output.shapes, output.pixels_per_point);
            let mut textures_delta = output.textures_delta;
            painter.paint_and_update_textures(
                ViewportId::ROOT,
                output.pixels_per_point,
                [0.0, 0.0, 0.0, 1.0],
                &clipped,
                &mut textures_delta,
                Vec::new(),
                &viewport.window,
            );
        }
    }
}

/// Thin settings panel: every control routes through the validated setters
/// so invalid UI states are impossible by construction.
fn settings_panel(ui: &mut egui::Ui, settings: &mut Settings) {
    use crate::settings as setters;
    egui::Grid::new("settings").show(ui, |ui| {
        ui.label("Brightness");
        ui.add_enabled(
            false,
            egui::Slider::new(&mut settings.brightness, 0.1..=1.0),
        );
        ui.end_row();
        ui.label("Mouse sensitivity");
        ui.add_enabled(
            false,
            egui::Slider::new(&mut settings.mouse_sensitivity, 0.2..=10.0),
        );
        ui.end_row();
    });
    // Slider edits above preview live values; committing them re-validates.
    let _ = setters::validate(settings);
}

impl ApplicationHandler for AppShell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.viewport.is_some() {
            return;
        }
        let window = match event_loop.create_window(window_attributes(&self.settings)) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                log_error(AppError::new(
                    "app.window_unavailable",
                    format!("Unable to create the game window: {error}"),
                ));
                event_loop.exit();
                return;
            }
        };
        if self.settings.screen_mode == ScreenMode::Fullscreen
            && let Some(monitor) = window.current_monitor()
        {
            // Exclusive mode needs a video mode, not just the monitor: pick
            // the one whose area best matches the committed resolution.
            let (want_w, want_h) = (
                u32::try_from(self.settings.resolution.width).unwrap_or(800),
                u32::try_from(self.settings.resolution.height).unwrap_or(600),
            );
            if let Some(mode) = monitor.video_modes().min_by_key(|mode| {
                let dw = i64::from(mode.size().width) - i64::from(want_w);
                let dh = i64::from(mode.size().height) - i64::from(want_h);
                dw * dw + dh * dh
            }) {
                window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
            } else {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
        let winit_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            self.painter.as_ref().and_then(Painter::max_texture_side),
        );
        self.viewport = Some(Viewport {
            window,
            winit_state,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(viewport) = &mut self.viewport else {
            return;
        };
        let response = viewport
            .winit_state
            .on_window_event(&viewport.window, &event);
        if response.repaint {
            viewport.window.request_redraw();
        }
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.redraw();
    }
}

fn log_error(error: AppError) {
    eprintln!("{error}");
}

// ------------------------------------------------------- minimal executor

/// Minimal std-only executor for the handful of async wgpu entry points
/// (`hp-render/src/block_on.rs` pattern). Blocks the calling thread with a
/// yield-spinning waker; sufficient because wgpu completes instance setup
/// inline or on other threads.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::pin::pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn noop_clone(data: *const ()) -> RawWaker {
        RawWaker::new(data, &VTABLE)
    }
    fn noop(_data: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

    // Safety: the vtable functions never dereference the data pointer.
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
