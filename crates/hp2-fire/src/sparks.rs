//! Port of the spark editor and spark redraw in `UnFractal.cpp`: `AddSpark`,
//! `CloseSpark`, `DeleteSparks`, `DrawSparkLine`, `FirePaint`, `PostDrawSparks`,
//! and `RedrawSparks` (in `sparks/redraw.rs`).
//!
//! `Sparks` is a `TArray<FSpark>` whose length is `Sparks.Num() == SparksLimit`.
//! The texture scalars travel in [`FireSparkState`]. The editor statics
//! (`LightPinX/Y`, `LastMouseX/Y`, `LastLeftButton`, `LastRightButton`) live in
//! [`EDITOR`]. Lock order: globals first, then `EDITOR`.

use std::sync::{Mutex, MutexGuard};

use crate::abi::{FireSpark, FireTraceHooks};
use crate::globals::{self, FireGlobals};

mod redraw;

pub use redraw::redraw_sparks;

// `ESpark` (UnFractal.h).
pub const SPARK_BURN: u8 = 0;
pub const SPARK_SPARKLE: u8 = 1;
pub const SPARK_PULSE: u8 = 2;
pub const SPARK_SIGNAL: u8 = 3;
pub const SPARK_BLAZE: u8 = 4;
pub const SPARK_OZ_HAS_SPOKEN: u8 = 5;
pub const SPARK_CONE: u8 = 6;
pub const SPARK_BLAZE_RIGHT: u8 = 7;
pub const SPARK_BLAZE_LEFT: u8 = 8;
pub const SPARK_CYLINDER: u8 = 9;
pub const SPARK_CYLINDER3D: u8 = 10;
pub const SPARK_LISSAJOUS: u8 = 11;
pub const SPARK_JUGGLERS: u8 = 12;
pub const SPARK_EMIT: u8 = 13;
pub const SPARK_FOUNTAIN: u8 = 14;
pub const SPARK_FLOCKS: u8 = 15;
pub const SPARK_EELS: u8 = 16;
pub const SPARK_ORGANIC: u8 = 17;
pub const SPARK_WANDER_ORGANIC: u8 = 18;
pub const SPARK_RANDOM_CLOUD: u8 = 19;
pub const SPARK_CUSTOM_CLOUD: u8 = 20;
pub const SPARK_LOCAL_CLOUD: u8 = 21;
pub const SPARK_STARS: u8 = 22;
pub const SPARK_LINE_LIGHTNING: u8 = 23;
pub const SPARK_RAMP_LIGHTNING: u8 = 24;
pub const SPARK_SPHERE_LIGHTNING: u8 = 25;
pub const SPARK_WHEEL: u8 = 26;
pub const SPARK_GAMETES: u8 = 27;
pub const SPARK_SPRINKLER: u8 = 28;
pub const SPARK_LISSAJX: u8 = 29;
pub const SPARK_LISSAJY: u8 = 30;

// Internal spawned spark types (UnFractal.cpp, next to the ESpark list).
pub const ISPARK_DRIFTER: u8 = 32;
pub const ISPARK_DRIFT_SLOW: u8 = 33;
pub const ISPARK_FALLER: u8 = 34;
pub const ISPARK_VSHOOTER: u8 = 35;
pub const ISPARK_DROP: u8 = 36;
pub const ISPARK_MOVE: u8 = 37;
pub const ISPARK_SPAWNED_EEL: u8 = 38;
pub const ISPARK_SPAWNED_TWIRL: u8 = 39;
pub const ISPARK_SPRINKLER_TWIRL: u8 = 40;
pub const ISPARK_CUSTOM: u8 = 41;
pub const ISPARK_GRAVITON: u8 = 42;
pub const ISPARK_SPAWNED_SPERM: u8 = 43;

// `EDrawMode` (UnFractal.h). `DRAW_Lathe` (1) is never compared in the spark code.
const DRAW_NORMAL: u8 = 0;
const DRAW_LATHE_2: u8 = 2;
const DRAW_LATHE_3: u8 = 3;
const DRAW_LATHE_4: u8 = 4;

// UnFractal.cpp constants.
const DEL_RANGE: i32 = 12;
const STAR_CUTOFF: u8 = 38;
const LOWEST_GLOW: i32 = 50;

// `MOUSE_Left` / `MOUSE_Right` (Engine headers).
const MOUSE_LEFT: u32 = 0x0001;
const MOUSE_RIGHT: u32 = 0x0002;

/// Texture scalars read and written by the spark functions. Mirrors the
/// `UFireTexture` and `UFractalTexture` members those functions touch.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireSparkState {
    /// `UFireTexture*` identity, passed back to the burn-write trace hook.
    pub texture: usize,
    /// `USize`, `VSize`.
    pub u_size: i32,
    pub v_size: i32,
    /// `UBits`, a shift count.
    pub u_bits: u8,
    /// `UMask`, `VMask`.
    pub u_mask: i32,
    pub v_mask: i32,
    /// `GlobalPhase` (INT), `DrawPhase` and `AuxPhase` (BYTE).
    pub global_phase: i32,
    pub draw_phase: u8,
    pub aux_phase: u8,
    /// `SparkType`, `DrawMode` (BYTE).
    pub spark_type: u8,
    pub draw_mode: u8,
    /// `SparksLimit`, `ActiveSparkNum` (INT).
    pub sparks_limit: i32,
    pub active_spark_num: i32,
    /// `StarStatus`, `PenDownX`, `PenDownY` (BYTE).
    pub star_status: u8,
    pub pen_down_x: u8,
    pub pen_down_y: u8,
    /// `FX_*` brush settings (BYTE).
    pub fx_heat: u8,
    pub fx_size: u8,
    pub fx_aux_size: u8,
    pub fx_area: u8,
    pub fx_frequency: u8,
    pub fx_phase: u8,
    pub fx_horiz_speed: u8,
    pub fx_vert_speed: u8,
}

/// Editor-time statics from `AddSpark`, `CloseSpark` (none), and `FirePaint`.
struct EditorState {
    light_pin_x: i32,
    light_pin_y: i32,
    last_mouse_x: i32,
    last_mouse_y: i32,
    last_left_button: i32,
    last_right_button: i32,
}

static EDITOR: Mutex<EditorState> = Mutex::new(EditorState {
    light_pin_x: 0,
    light_pin_y: 0,
    last_mouse_x: 0,
    last_mouse_y: 0,
    last_left_button: 0,
    last_right_button: 0,
});

/// Locks the editor statics. A poisoned lock is recovered, as in `globals::lock`.
fn editor() -> MutexGuard<'static, EditorState> {
    EDITOR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `(DWORD)(lo + (hi << UBits))`: the texel index of a spark or a masked position.
fn texel(lo: i32, hi: i32, ubits: u8) -> usize {
    lo.wrapping_add(hi.wrapping_shl(ubits as u32)) as u32 as usize
}

/// `(GlobalPhase * FX_Frequency + FX_Phase) & 255`, the start phase of a twister spark.
fn start_phase(st: &FireSparkState) -> u8 {
    (st.global_phase
        .wrapping_mul(st.fx_frequency as i32)
        .wrapping_add(st.fx_phase as i32)
        & 255) as u8
}

/// `UFireTexture::AddSpark`, body only. The caller holds the locks.
fn add_spark_inner(
    g: &mut FireGlobals,
    ed: &mut EditorState,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    mouse_x: i32,
    mouse_y: i32,
) {
    // Return if out of bounds or out of sparks.
    if mouse_x < 0
        || mouse_y < 0
        || mouse_x >= st.u_size
        || mouse_y >= st.v_size
        || st.active_spark_num >= st.sparks_limit
    {
        return;
    }

    // Make new spark. ->Sparks[0] is the first one.
    let s = st.active_spark_num;
    st.active_spark_num += 1;
    let si = s as usize;

    // General new spark initialization.
    sparks[si].ty = st.spark_type;
    sparks[si].x = mouse_x as u8;
    sparks[si].y = mouse_y as u8;
    sparks[si].heat = st.fx_heat;

    // Spark-type specific assignments.
    match st.spark_type {
        SPARK_SPARKLE => {
            sparks[si].byte_a = st.fx_size;
            sparks[si].byte_b = st.fx_aux_size;
        }
        SPARK_PULSE => {
            sparks[si].heat = st.draw_phase.wrapping_add(st.aux_phase);
            st.draw_phase = st.draw_phase.wrapping_add(st.fx_phase);
            sparks[si].byte_d = st.fx_frequency; // Improve useful range.
        }
        SPARK_SIGNAL => {
            sparks[si].heat = g.speed_rand();
            sparks[si].byte_d = st.fx_frequency; // Improve useful range.
            sparks[si].byte_c = 255u8.wrapping_sub(st.fx_heat); // 255 - FX_Heat
        }
        SPARK_STARS => {
            // Stars, FieldA for this type holds (negative) start Intensity.
            sparks[si].byte_a = st.fx_heat;
            st.star_status = 1; // At least one star..
        }
        SPARK_ORGANIC => {
            sparks[si].byte_c = st.fx_area;
        }
        SPARK_EELS | SPARK_BLAZE_LEFT | SPARK_BLAZE_RIGHT => {
            sparks[si].byte_c = st.fx_size;
        }
        SPARK_OZ_HAS_SPOKEN => {}
        SPARK_EMIT | SPARK_FOUNTAIN => {
            // Spawn at angle, lifetime, linear movers (Fountain adds gravity later).
            sparks[si].byte_a = st.fx_horiz_speed.wrapping_sub(128);
            sparks[si].byte_b = 127 ^ st.fx_vert_speed;
            sparks[si].byte_d = (255 / (st.fx_size as i32 + 1)) as u8;
        }
        SPARK_CYLINDER | SPARK_CYLINDER3D => {
            // Twister (X=sin()) movement.
            let mut temp_size = st.fx_size as i32;
            let mut temp_speed = st.fx_horiz_speed as i32 - 128;
            let this_idx = si;
            let axis2 = st.pen_down_x as i32 * 2 + st.fx_size as i32;

            // Scale to imaginary axis while mousebutton remains down.
            if st.draw_mode > DRAW_NORMAL && st.pen_down_x != 0 {
                temp_size = axis2 - mouse_x * 2;

                // Axis = at PenDownX + 0.5 * FX_Size.
                if temp_size < 0 {
                    // Crossed the axis.
                    temp_size = mouse_x * 2 - axis2;
                    sparks[si].x = (mouse_x - temp_size) as u8; // Minus total diameter.
                    temp_speed = (256 - temp_speed) & 255;
                }
            }

            sparks[si].byte_a = start_phase(st);
            sparks[si].byte_b = temp_size as u8; // Size.
            sparks[si].byte_d = temp_speed as u8;

            // Draw second spark at +0.5 phase.
            if st.draw_mode == DRAW_LATHE_2 && st.active_spark_num < st.sparks_limit {
                let n = spawn_index(st);
                sparks[n].ty = st.spark_type;
                sparks[n].x = sparks[this_idx].x;
                sparks[n].y = mouse_y as u8;
                sparks[n].heat = st.fx_heat;
                sparks[n].byte_a = sparks[this_idx].byte_a.wrapping_add(128);
                sparks[n].byte_b = temp_size as u8; // size
                sparks[n].byte_d = temp_speed as u8;
            }

            // Draw 2 more sparks at +.33/.67 phases.
            if st.draw_mode == DRAW_LATHE_3 && st.active_spark_num + 2 <= st.sparks_limit {
                for t in 1..3i32 {
                    let n = spawn_index(st);
                    sparks[n].ty = st.spark_type;
                    sparks[n].x = sparks[this_idx].x;
                    sparks[n].y = mouse_y as u8;
                    sparks[n].heat = st.fx_heat;
                    // `256/3` is 85 in C integer arithmetic.
                    sparks[n].byte_a = sparks[this_idx].byte_a.wrapping_add((t * (256 / 3)) as u8);
                    sparks[n].byte_b = temp_size as u8; // Size.
                    sparks[n].byte_d = temp_speed as u8;
                }
            }

            // Draw 3 more sparks at +.25/.5/.75 phases.
            if st.draw_mode == DRAW_LATHE_4 && st.active_spark_num + 3 <= st.sparks_limit {
                for t in 1..4i32 {
                    let n = spawn_index(st);
                    sparks[n].ty = st.spark_type;
                    sparks[n].x = sparks[this_idx].x;
                    sparks[n].y = mouse_y as u8;
                    sparks[n].heat = st.fx_heat;
                    sparks[n].byte_a = sparks[this_idx].byte_a.wrapping_add((t * 64) as u8);
                    sparks[n].byte_b = temp_size as u8; // Size.
                    sparks[n].byte_d = temp_speed as u8;
                }
            }
        }
        SPARK_JUGGLERS => {
            // Twister (X=sin()) movement.
            sparks[si].byte_a = start_phase(st);
            sparks[si].byte_b = st.fx_size; // size
            sparks[si].byte_d = st.fx_horiz_speed.wrapping_sub(128);
        }
        SPARK_LISSAJOUS => {
            sparks[si].heat = st.fx_size; // size
            sparks[si].byte_a = st.fx_phase; // phase u
            sparks[si].byte_b = st.fx_frequency; // phase v
            sparks[si].byte_c = st.fx_horiz_speed.wrapping_sub(128);
            sparks[si].byte_d = st.fx_vert_speed.wrapping_sub(128);
            // Special-cases redirected to more efficient setters.
            if sparks[si].byte_c == 0 {
                sparks[si].ty = SPARK_LISSAJY;
            }
            if sparks[si].byte_d == 0 {
                sparks[si].ty = SPARK_LISSAJX;
            }
        }
        SPARK_SPHERE_LIGHTNING => {
            // Spherical lightning.
            sparks[si].heat = st.fx_heat; // heat
            sparks[si].byte_c = st.fx_size; // radius
            // minimal 8 (=radius 4)
            if sparks[si].byte_c < 8 {
                sparks[si].byte_c = 8;
            }
            sparks[si].byte_d = 96; // Hardcoded chance.
        }
        SPARK_FLOCKS => {
            sparks[si].byte_a = 128;
            sparks[si].byte_b = st.fx_size; // Lifetime
            sparks[si].byte_c = 128;
            sparks[si].byte_d = 255u8.wrapping_sub(st.fx_area); // spawn direction delta
        }
        SPARK_WHEEL => {
            // Swirly emitter.
            sparks[si].byte_a = st.fx_phase; // Initial spawn dir.
            sparks[si].byte_b = st.fx_size; // Lifetime
            sparks[si].byte_c = st.fx_frequency;
            sparks[si].byte_d = 255u8.wrapping_sub(st.fx_area); // spawn direction delta
        }
        SPARK_SPRINKLER => {
            sparks[si].byte_a = st.fx_phase; // Initial spawn dir.
            sparks[si].byte_b = 128; // (BYTE)-128
            sparks[si].byte_c = 128; // lifetime
            sparks[si].byte_d = st.fx_frequency;
        }
        SPARK_GAMETES => {
            sparks[si].byte_c = st.fx_size; // Lifetime of spawned spermatozoids.
        }
        SPARK_LOCAL_CLOUD => {
            sparks[si].byte_c = st.fx_area;
            sparks[si].byte_a = st.fx_horiz_speed.wrapping_sub(128);
            sparks[si].byte_b = (255 ^ st.fx_vert_speed).wrapping_sub(128);
            sparks[si].byte_d = 255u8.wrapping_sub(st.fx_size);
        }
        SPARK_CUSTOM_CLOUD => {
            sparks[si].byte_a = st.fx_horiz_speed.wrapping_sub(128);
            sparks[si].byte_b = (255 ^ st.fx_vert_speed).wrapping_sub(128);
            sparks[si].byte_d = 255u8.wrapping_sub(st.fx_size);
        }
        SPARK_LINE_LIGHTNING | SPARK_RAMP_LIGHTNING => {
            // We have a new spark if no other was an 'open' one.
            // Not always the last one since short-lived sparks may shuffle the spark
            // order at any moment.
            let mut open = s;
            loop {
                open -= 1;
                if open < 0 {
                    break;
                }
                let o = &sparks[open as usize];
                if o.ty == st.spark_type && o.byte_d == 0 {
                    break;
                }
            }

            if open >= 0 {
                // Open point found, draw it while being stretched.
                st.active_spark_num -= 1; // so don't allocate 'current' spark
                let o = open as usize;

                // Refresh to 'starting' point.
                sparks[o].x = ed.light_pin_x as u8;
                sparks[o].y = ed.light_pin_y as u8;

                // Dragging the lightning: Must have a heat value.
                // Close it by setting ByteD = heat ?
                // To nonzero, eventually.
                sparks[o].heat = 3 | st.fx_heat; // Brightness.

                let mut len_x = mouse_x - sparks[o].x as i32;
                let mut len_y = mouse_y - sparks[o].y as i32;

                if len_x < 0 {
                    len_x = len_x.wrapping_neg() | 1;
                } else {
                    len_x &= -2; // 0xFFFFFFFE
                }
                if len_y < 0 {
                    len_y = len_y.wrapping_neg() | 1;
                } else {
                    len_y &= -2; // 0xFFFFFFFE
                }

                if len_x == 0 && len_y == 0 {
                    sparks[o].heat = 0; // too short!
                }

                sparks[o].byte_a = len_x as u8;
                sparks[o].byte_b = len_y as u8;
                // ByteD Stays 0 while drawing to indicate open bolt.
            } else {
                // No open point found: new lightning thing.
                let n = (st.active_spark_num - 1) as usize;
                sparks[n].byte_d = 0; // Signify open bolt.
                sparks[n].heat = 0;
                ed.light_pin_x = mouse_x;
                ed.light_pin_y = mouse_y;
            }
        }
        _ => {}
    }
}

/// `UFireTexture::CloseSpark`, body only. The mouse position is unused in C.
fn close_spark_inner(st: &FireSparkState, sparks: &mut [FireSpark]) {
    // Sparks[S=0] is only one if ActiveSparkNum = 1.
    let mut s = st.active_spark_num;

    // Warning: extremely kludgy.
    // Find ANY old spark of type 23/24 which has an 'open' end.
    loop {
        s -= 1;
        if s < 0 {
            break;
        }
        let sp = &sparks[s as usize];
        if (sp.ty == SPARK_LINE_LIGHTNING || sp.ty == SPARK_RAMP_LIGHTNING) && sp.byte_d == 0 {
            break;
        }
    }

    if s >= 0 {
        // Open point found, closin' it.
        // ByteD, minimally 1...
        let sp = &mut sparks[s as usize];
        sp.byte_d = st.fx_frequency;
        if sp.byte_d == 0 {
            sp.byte_d = 1; // Ensure != 0.
        }
    }
}

/// `UFireTexture::DeleteSparks`, body only. Each deleted spark is replaced by the last one,
/// and the moved spark is not re-tested.
fn delete_sparks_inner(
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    spark_x: i32,
    spark_y: i32,
    area_width: i32,
) {
    if st.active_spark_num > 0 {
        let mut s: i32 = 0;
        while s < st.active_spark_num {
            let sp = sparks[s as usize];
            // Diamond-shaped eraser.
            if area_width
                >= (spark_x - sp.x as i32).wrapping_abs() + (spark_y - sp.y as i32).wrapping_abs()
            {
                // Delete spark by replacing it with last one (+ delete last one).
                st.active_spark_num -= 1;
                let last = st.active_spark_num as usize;
                sparks[s as usize] = sparks[last];
            }
            s += 1;
        }
    }
}

/// `UFireTexture::DrawSparkLine`, body only. The C++ `Density` argument is unused.
#[allow(clippy::too_many_arguments)]
fn draw_spark_line_inner(
    g: &mut FireGlobals,
    ed: &mut EditorState,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    start_x: i32,
    start_y: i32,
    dest_x: i32,
    dest_y: i32,
) {
    let mut div_x = dest_x - start_x;
    let mut div_y = dest_y - start_y;

    if div_x == 0 && div_y == 0 {
        return;
    }

    let xinc: i32 = if div_x < 0 {
        -1
    } else if div_x > 0 {
        1
    } else {
        0
    };
    let yinc: i32 = if div_y < 0 {
        -1
    } else if div_y > 0 {
        1
    } else {
        0
    };

    div_x = div_x.wrapping_abs();
    div_y = div_y.wrapping_abs();

    let mut xpoint = start_x;
    let mut ypoint = start_y;

    if div_x >= div_y {
        // Draw line based on X loop.
        let div_y2 = div_y + div_y;
        let mut diff = div_y2 - div_x;
        let div_xy2 = div_y2 - div_x - div_x;

        for _ in 1..=div_x {
            add_spark_inner(g, ed, st, sparks, xpoint, ypoint);

            if diff < 0 {
                diff += div_y2;
            } else {
                diff += div_xy2;
                ypoint += yinc;
            }
            xpoint += xinc;
        }
    } else {
        // Draw line based on Y loop.
        let div_x2 = div_x + div_x;
        let mut diff = div_x2 - div_y;
        let div_xy2 = div_x2 - div_y - div_y;

        for _ in 1..=div_y {
            add_spark_inner(g, ed, st, sparks, xpoint, ypoint);

            if diff < 0 {
                diff += div_x2;
            } else {
                diff += div_xy2;
                xpoint += xinc;
            }
            ypoint += yinc;
        }
    }
}

/// `UFireTexture::FirePaint`, body only. The caller holds the locks.
fn fire_paint_inner(
    g: &mut FireGlobals,
    ed: &mut EditorState,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    mouse_x: i32,
    mouse_y: i32,
    buttons: u32,
) {
    // Raw masked values, as in C: `UBOOL RightButton = (Buttons & MOUSE_Right)`.
    let right_button = (buttons & MOUSE_RIGHT) as i32;
    let left_button = (buttons & MOUSE_LEFT) as i32;

    // Perform painting.
    let last_mouse_x = ed.last_mouse_x;
    let last_mouse_y = ed.last_mouse_y;
    let pos_changed = last_mouse_x != mouse_x || last_mouse_y != mouse_y;
    let right_changed = ed.last_right_button != right_button;
    let left_changed = ed.last_left_button != left_button;

    if left_changed && left_button != 0 {
        st.pen_down_x = mouse_x as u8;
        st.pen_down_y = mouse_y as u8;
    }
    if left_changed && left_button == 0 {
        st.pen_down_x = 0;
        st.pen_down_y = 0;
    }

    // Draws the kind of spark/linetype currently selected.
    if left_button != 0 {
        if !left_changed && pos_changed && st.spark_type < 4 {
            draw_spark_line_inner(
                g,
                ed,
                st,
                sparks,
                last_mouse_x,
                last_mouse_y,
                mouse_x,
                mouse_y,
            );
        }

        if left_changed || (pos_changed && st.spark_type >= 4) {
            add_spark_inner(g, ed, st, sparks, mouse_x, mouse_y);
        }
    } else {
        // LeftButton == 0. Lightning bolt close on release + movement.
        if left_changed || pos_changed {
            close_spark_inner(st, sparks);
        }
    }

    // Lightning bolt close on non-moving re-clicking.
    if !left_changed && !pos_changed && left_button != 0 {
        close_spark_inner(st, sparks);
    }

    // Delete any sparks within certain range of mouse cursor.
    if right_button != 0 && (pos_changed || right_changed) {
        delete_sparks_inner(st, sparks, mouse_x, mouse_y, DEL_RANGE);
    }

    // Remember.
    ed.last_mouse_x = mouse_x;
    ed.last_mouse_y = mouse_y;
    ed.last_left_button = left_button;
    ed.last_right_button = right_button;
}

/// `spawn_index`: `INT NS = ActiveSparkNum++;`.
fn spawn_index(st: &mut FireSparkState) -> usize {
    let n = st.active_spark_num as usize;
    st.active_spark_num += 1;
    n
}

/// Caps `SparksLimit` at `Sparks.Num()`. C writes past the array when the limit is larger (UB);
/// here the spawn guards stop at the slice length. Only this local copy changes.
fn clamp_sparks_limit(st: &mut FireSparkState, sparks: &[FireSpark]) {
    st.sparks_limit = st.sparks_limit.min(sparks.len() as i32);
}

/// `UFireTexture::AddSpark`: spawns one spark of `spark_type` at the mouse position.
pub fn add_spark(
    trace: FireTraceHooks,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    mouse_x: i32,
    mouse_y: i32,
) {
    let mut g = globals::lock();
    g.trace = trace;
    clamp_sparks_limit(st, sparks);
    let mut ed = editor();
    add_spark_inner(&mut g, &mut ed, st, sparks, mouse_x, mouse_y);
}

/// `UFireTexture::CloseSpark`: closes the most recent open lightning bolt.
pub fn close_spark(trace: FireTraceHooks, st: &mut FireSparkState, sparks: &mut [FireSpark]) {
    let mut g = globals::lock();
    g.trace = trace;
    close_spark_inner(st, sparks);
}

/// `UFireTexture::DeleteSparks`: removes sparks inside a diamond around the cursor.
pub fn delete_sparks(
    trace: FireTraceHooks,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    spark_x: i32,
    spark_y: i32,
    area_width: i32,
) {
    let mut g = globals::lock();
    g.trace = trace;
    delete_sparks_inner(st, sparks, spark_x, spark_y, area_width);
}

/// `UFireTexture::DrawSparkLine`: Bresenham line of sparks between two points.
pub fn draw_spark_line(
    trace: FireTraceHooks,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    start_x: i32,
    start_y: i32,
    dest_x: i32,
    dest_y: i32,
) {
    let mut g = globals::lock();
    g.trace = trace;
    clamp_sparks_limit(st, sparks);
    let mut ed = editor();
    draw_spark_line_inner(
        &mut g, &mut ed, st, sparks, start_x, start_y, dest_x, dest_y,
    );
}

/// `UFireTexture::FirePaint`: the fire-specific editor mouse handler.
pub fn fire_paint(
    trace: FireTraceHooks,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    mouse_x: i32,
    mouse_y: i32,
    buttons: u32,
) {
    let mut g = globals::lock();
    g.trace = trace;
    clamp_sparks_limit(st, sparks);
    let mut ed = editor();
    fire_paint_inner(&mut g, &mut ed, st, sparks, mouse_x, mouse_y, buttons);
}

/// `UFireTexture::PostDrawSparks`: restores each fixed star's pixel after the update.
pub fn post_draw_sparks(
    trace: FireTraceHooks,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    mips0: &mut [u8],
) {
    let mut g = globals::lock();
    g.trace = trace;

    // No stars so don't search the array.
    if st.star_status == 0 {
        return;
    }

    let mut found_star: u8 = 0;
    for s in 0..st.active_spark_num {
        let sp = &mut sparks[s as usize];
        if sp.ty == SPARK_STARS {
            found_star = 1;
            // Fixed stars disappear depending to fire Intensity.
            // called here after the update: so restore the original star.
            // ByteA has the star itself, ByteB the saved bckgrnd.
            let dest = texel(sp.x as i32, sp.y as i32, st.u_bits);

            // Save real sky value.
            let dim_star = mips0[dest];
            sp.byte_b = dim_star;

            // Put in the star if under cutoff.
            if dim_star < STAR_CUTOFF {
                mips0[dest] = sp.byte_a;
            }
        }
    }

    if found_star == 0 {
        st.star_status = 0;
    }
}
