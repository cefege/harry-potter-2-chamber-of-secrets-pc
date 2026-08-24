//! Interactive windowed mode: a winit [`ApplicationHandler`] hosting the
//! live game viewport.
//!
//! Ownership split (mirrors the launcher shell's shape, but egui-free):
//! - this module owns the window, the swapchain, input, frame pacing, and
//!   audio boot;
//! - [`Engine`](hp_engine::sim::Engine) owns simulation (tick, input
//!   dispatch, console);
//! - [`RendererSession`](hp_engine::render_bridge::RendererSession) owns
//!   GPU resources; the swapchain reuses its pipelines unchanged.
//!
//! Frame cadence: one tick + one present per redraw, paced by vertical sync
//! (`Fifo`) and additionally capped by the engine's own `GetMaxTickRate`
//! policy (`FrameRateLimit`) when set. No busy-spin: the thread parks in
//! the event loop between redraws.
//!
//! Input: keyboard only. WASD/arrows fly the camera at 256 uu/s along the
//! view axes (Q/E add vertical); presses and releases are ALSO routed
//! through `Engine::dispatch_input` so `[Engine.Input]` bindings stay
//! observable. The mouse is ignored. ESC quits.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hp_engine::error::{EngineError, Result};
use hp_engine::input::{action, key};
use hp_engine::render_bridge::{RendererSession, rotation_matrix};
use hp_engine::scene::SceneCamera;
use hp_engine::sim::Engine;
use hp_render::WindowTarget;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Window, WindowId};

/// Free-fly camera speed in Unreal units per second.
pub const FREE_FLY_SPEED: f32 = 256.0;

/// Default viewport: the authored 4:3 resolution the projection math keeps.
const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;

/// IK keys that translate the camera, mapped to their axis contribution:
/// `(forward, strafe_right, up)` multipliers.
fn movement_axes(held: &HashSet<u8>) -> [f32; 3] {
    let mut axes = [0.0f32; 3];
    let mut apply = |k: u8, delta: [f32; 3]| {
        if held.contains(&k) {
            for axis in 0..3 {
                axes[axis] += delta[axis];
            }
        }
    };
    let (w, a, s, d, q, e) = (
        key::letter(b'w' - b'a'),
        key::letter(0),
        key::letter(b's' - b'a'),
        key::letter(b'd' - b'a'),
        key::letter(b'q' - b'a'),
        key::letter(b'e' - b'a'),
    );
    apply(w, [1.0, 0.0, 0.0]);
    apply(s, [-1.0, 0.0, 0.0]);
    apply(d, [0.0, 1.0, 0.0]);
    apply(a, [0.0, -1.0, 0.0]);
    apply(e, [0.0, 0.0, 1.0]);
    apply(q, [0.0, 0.0, -1.0]);
    apply(key::UP, [1.0, 0.0, 0.0]);
    apply(key::DOWN, [-1.0, 0.0, 0.0]);
    apply(key::RIGHT, [0.0, 1.0, 0.0]);
    apply(key::LEFT, [0.0, -1.0, 0.0]);
    axes
}

/// Map a winit logical key to its `EInputKey` value (the subset the
/// free-fly camera uses). Mouse keys are deliberately unmapped — the mouse
/// is ignored in interactive mode.
fn ik_key<S: AsRef<str>>(logical: &WinitKey<S>) -> Option<u8> {
    match logical {
        WinitKey::Character(c) => match c.as_ref().to_ascii_lowercase().as_str() {
            "w" => Some(key::letter(b'w' - b'a')),
            "a" => Some(key::letter(0)),
            "s" => Some(key::letter(b's' - b'a')),
            "d" => Some(key::letter(b'd' - b'a')),
            "q" => Some(key::letter(b'q' - b'a')),
            "e" => Some(key::letter(b'e' - b'a')),
            _ => None,
        },
        WinitKey::Named(named) => match named {
            NamedKey::Escape => Some(key::ESCAPE),
            NamedKey::ArrowLeft => Some(key::LEFT),
            NamedKey::ArrowUp => Some(key::UP),
            NamedKey::ArrowRight => Some(key::RIGHT),
            NamedKey::ArrowDown => Some(key::DOWN),
            NamedKey::Space => Some(key::SPACE),
            _ => None,
        },
        _ => None,
    }
}

/// Everything audio owns once sounds are enabled: the real output stream
/// (kept alive for the session), plus the lifecycle-traced output graph
/// whose streams get real-device sinks.
struct AudioOut {
    _device: rodio::MixerDeviceSink,
    _graph: hp_audio::graph::OutputGraph,
}

/// Open the default output device and wire an
/// [`OutputGraph`](hp_audio::graph::OutputGraph) with a real-device sink
/// factory. Best-effort: failure is loud but never fatal — the game runs
/// silent rather than aborting.
fn init_audio(data_root: &Path) -> Option<AudioOut> {
    let mut device = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("hp-engine: [audio.output_unavailable] continuing without sound: {error}");
            return None;
        }
    };
    let sample_rate = device.config().sample_rate();
    let channels = device.config().channel_count();
    // Dropping the sink at session end is intentional; silence its warning.
    device.log_on_drop(false);

    let graph = hp_audio::graph::OutputGraph::with_sink_factory(Arc::new(|_info| {
        Box::new(hp_audio::graph::RodioSink::new())
    }));

    println!(
        "hp2rs: audio enabled ({channels}ch @ {sample_rate} Hz; datadir {})",
        data_root.display()
    );
    play_tone(device.mixer(), 660.0, 0.09, 0.20);
    play_tone(device.mixer(), 880.0, 0.12, 0.16);
    Some(AudioOut {
        _device: device,
        _graph: graph,
    })
}

/// Short sine blip on the live output stream (startup confirmation cue).
/// Trivial by construction — synthesized samples, no asset dependency — and
/// never blocks: append hands the buffer to the mix thread.
fn play_tone(mixer: &rodio::mixer::Mixer, freq: f32, seconds: f32, gain: f32) {
    const RATE: u32 = 44100;
    let count = (RATE as f32 * seconds) as usize;
    if count == 0 {
        return;
    }
    let samples: Vec<f32> = (0..count)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let envelope = 1.0 - i as f32 / count as f32;
            (t * freq * std::f32::consts::TAU).sin() * envelope * gain
        })
        .collect();
    if let (Some(rate), Some(channels)) =
        (rodio::SampleRate::new(RATE), rodio::ChannelCount::new(2))
    {
        let stereo: Vec<f32> = samples.iter().flat_map(|&s| [s, s]).collect();
        mixer.add(rodio::buffer::SamplesBuffer::new(channels, rate, stereo));
    }
}

/// Interactive session state. Constructed before the event loop opens;
/// everything touching the display server happens inside handler callbacks.
pub struct GameApp {
    engine: Engine,
    session: Option<RendererSession>,
    target: Option<WindowTarget>,
    window: Option<Arc<Window>>,
    camera: SceneCamera,
    audio: Option<AudioOut>,
    held_keys: HashSet<u8>,
    presented_frames: u64,
    last_frame: Option<Instant>,
    started: Instant,
    run_seconds: Option<f64>,
    /// First error escapes the event loop and is re-reported by `main`.
    fatal: Option<EngineError>,
}

impl GameApp {
    /// Assemble the session. `session` is `None` under `--null-render`.
    /// Audio opens here (outside the event loop) so device failure fails
    /// fast and loud before any window exists.
    pub fn new(
        mut engine: Engine,
        session: Option<RendererSession>,
        camera: SceneCamera,
        data_root: &Path,
        sound_enabled: bool,
        run_seconds: Option<f64>,
    ) -> Self {
        engine.sounds_enabled = sound_enabled;
        let audio = if sound_enabled {
            init_audio(data_root)
        } else {
            println!("hp2rs: audio disabled (-nosound)");
            None
        };
        GameApp {
            engine,
            session,
            target: None,
            window: None,
            camera,
            audio,
            held_keys: HashSet::new(),
            presented_frames: 0,
            last_frame: None,
            started: Instant::now(),
            run_seconds,
            fatal: None,
        }
    }

    /// Run the interactive session to completion (window close, ESC, or
    /// scripted timeout), then report observable counters.
    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<()> {
        let result = event_loop.run_app(&mut self);
        if let Some(fatal) = self.fatal {
            return Err(fatal);
        }
        result.map_err(|error| {
            EngineError::new(
                "engine.event_loop_failed",
                format!("event loop failed: {error}"),
            )
        })?;

        let ticks = self.engine.counters().ticks;
        let audio_note = match self.audio {
            Some(_) => "audio=on",
            None => "audio=off",
        };
        println!(
            "hp2rs: session ended {audio_note} presented_frames={} ticks={} input_events={} sound_cues={}",
            self.presented_frames,
            ticks,
            self.engine.counters().input_events,
            self.engine.sound_cues.len()
        );
        if self.run_seconds.is_some() {
            // Scripted-run contract: an N presented-frame counter on stdout.
            println!("<HP2_RES> presented_frames={}", self.presented_frames);
        }
        Ok(())
    }

    /// One frame = one tick + one present, at vsync pace.
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        // Scripted auto-quit (verification hook): stop cleanly at the
        // requested wall-clock mark.
        if let Some(seconds) = self.run_seconds
            && self.started.elapsed().as_secs_f64() >= seconds
        {
            event_loop.exit();
            return;
        }

        let now = Instant::now();
        // Frame pacing: vsync paces naturally; honor the engine's tick cap
        // by sleeping toward the next allowed frame when it demands less
        // than the display refresh.
        let cap = self.engine.get_max_tick_rate();
        if cap > 0.0
            && let Some(last) = self.last_frame
        {
            let interval = Duration::from_secs_f64(1.0 / f64::from(cap));
            let since_last = now.duration_since(last);
            if since_last < interval {
                std::thread::sleep(interval - since_last);
            }
        }
        let dt = self
            .last_frame
            .map_or(1.0 / 60.0, |last| now.duration_since(last).as_secs_f32())
            .min(0.25);
        self.last_frame = Some(now);

        // Free-fly translation along the authored view basis; speed scales
        // with real elapsed time so movement is frame-rate independent.
        let axes = movement_axes(&self.held_keys);
        if axes != [0.0; 3] {
            let r = rotation_matrix(self.camera.rotation);
            let forward = mat_vec(&r, &[1.0, 0.0, 0.0]);
            let right = mat_vec(&r, &[0.0, 1.0, 0.0]);
            let step = FREE_FLY_SPEED * dt;
            for axis in 0..3 {
                self.camera.location[axis] +=
                    step * (axes[0] * forward[axis] + axes[1] * right[axis]);
                self.camera.location[axis] += step * axes[2];
            }
            if let Some(session) = self.session.as_mut() {
                session.update_camera(self.camera.location, self.camera.rotation);
            }
        }

        if let Err(error) = self.engine.tick(&[]) {
            self.fatal = Some(error);
            event_loop.exit();
            return;
        }

        // Present per tick-end.
        if let (Some(session), Some(target)) = (self.session.as_ref(), self.target.as_mut()) {
            let ctx = session.gpu_context();
            match target.begin(ctx) {
                Ok(Some(frame)) => {
                    session.render_frame_to_views(frame.color_view(), frame.depth_view());
                    frame.present(&ctx.queue);
                    self.presented_frames += 1;
                }
                Ok(None) => {} // swapchain not ready (resize/occlusion); skip
                Err(error) => {
                    eprintln!(
                        "hp-engine: [{}] swapchain acquire failed, skipping frame",
                        error.reason_code()
                    );
                }
            }
        }

        if let Some(window) = self.window.as_ref()
            && !self.engine.quit_requested()
        {
            window.request_redraw();
            return;
        }
        event_loop.exit();
    }
}

impl ApplicationHandler for GameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Harry Potter II (hp2rs)")
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fatal = Some(EngineError::new(
                    "engine.window_unavailable",
                    format!("unable to create the game window: {error}"),
                ));
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        match self.session.as_ref().map(RendererSession::gpu_context) {
            Some(ctx) => {
                match WindowTarget::new(ctx, &window, size.width.max(1), size.height.max(1)) {
                    Ok(target) => self.target = Some(target),
                    Err(error) => {
                        eprintln!(
                            "hp-engine: [{}] continuing without swapchain: {error}",
                            error.reason_code()
                        );
                    }
                }
            }
            None => eprintln!("hp2rs: null render — the window will stay black"),
        }

        self.window = Some(window);
        self.window.as_ref().expect("just set").request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(session), Some(target)) = (self.session.as_ref(), self.target.as_mut())
                {
                    target.resize(session.gpu_context(), size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(ik) = ik_key(&event.logical_key) else {
                    return;
                };
                match event.state {
                    ElementState::Pressed => {
                        if ik == key::ESCAPE {
                            event_loop.exit();
                            return;
                        }
                        if self.held_keys.insert(ik) {
                            self.engine.dispatch_input(ik, action::PRESS, 0.0);
                        }
                    }
                    ElementState::Released => {
                        if self.held_keys.remove(&ik) {
                            self.engine.dispatch_input(ik, action::RELEASE, 0.0);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }
}

fn mat_vec(m: &[[f32; 3]; 3], v: &[f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}
