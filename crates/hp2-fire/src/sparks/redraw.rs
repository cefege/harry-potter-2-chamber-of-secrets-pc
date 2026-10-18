//! Port of `UFireTexture::RedrawSparks` and its helpers from `UnFractal.cpp`:
//! `DrawFlashRamp`, `MoveSpark`, `MoveSparkXY`, `MoveSparkAngle`, `MoveSparkTwo`.
//! The `FIRE_X86_ASM && 0` variants of `MoveSparkXY` and `MoveSpark` are dead
//! and are not ported. Only the C paths remain.

use std::ffi::c_void;

use super::clamp_sparks_limit;
use super::{
    FireSparkState, ISPARK_CUSTOM, ISPARK_DRIFT_SLOW, ISPARK_DRIFTER, ISPARK_DROP, ISPARK_FALLER,
    ISPARK_GRAVITON, ISPARK_MOVE, ISPARK_SPAWNED_EEL, ISPARK_SPAWNED_SPERM, ISPARK_SPAWNED_TWIRL,
    ISPARK_SPRINKLER_TWIRL, ISPARK_VSHOOTER, LOWEST_GLOW, SPARK_BLAZE, SPARK_BLAZE_LEFT,
    SPARK_BLAZE_RIGHT, SPARK_BURN, SPARK_CONE, SPARK_CUSTOM_CLOUD, SPARK_CYLINDER,
    SPARK_CYLINDER3D, SPARK_EELS, SPARK_EMIT, SPARK_FLOCKS, SPARK_FOUNTAIN, SPARK_GAMETES,
    SPARK_JUGGLERS, SPARK_LINE_LIGHTNING, SPARK_LISSAJOUS, SPARK_LISSAJX, SPARK_LISSAJY,
    SPARK_LOCAL_CLOUD, SPARK_ORGANIC, SPARK_OZ_HAS_SPOKEN, SPARK_PULSE, SPARK_RAMP_LIGHTNING,
    SPARK_RANDOM_CLOUD, SPARK_SIGNAL, SPARK_SPARKLE, SPARK_SPHERE_LIGHTNING, SPARK_SPRINKLER,
    SPARK_STARS, SPARK_WANDER_ORGANIC, SPARK_WHEEL, spawn_index, texel,
};
use crate::abi::{FireLineSeg, FireSpark, FireTraceHooks};
use crate::globals::{self, FireGlobals};

/// `UFireTexture::DrawFlashRamp`: draws a Bresenham-style ramp along `ll`.
/// `ll` is passed by value and the copy is what gets modified, as in C.
#[allow(clippy::needless_range_loop)] // index loops mirror the C flash loops
fn draw_flash_ramp(
    g: &mut FireGlobals,
    st: &FireSparkState,
    mips0: &mut [u8],
    mut ll: FireLineSeg,
    color1: u8,
    color2: u8,
) {
    let mut c1 = color1;
    let mut c2 = color2;

    // Make writing cache-friendlier by drawing approximately to the right or downwards.
    if (ll.ylen & 1 != 0 && (ll.ylen as i32) * 2 >= ll.xlen as i32)
        || (ll.xlen & 1 != 0 && (ll.ylen as i32) * 2 < ll.xlen as i32)
    {
        // Odd lengths step backwards from the start point.
        let dx = if ll.xlen & 1 != 0 {
            ll.xlen.wrapping_neg()
        } else {
            ll.xlen
        };
        let dy = if ll.ylen & 1 != 0 {
            ll.ylen.wrapping_neg()
        } else {
            ll.ylen
        };
        ll.xpos = ll.xpos.wrapping_add(dx);
        ll.ypos = ll.ypos.wrapping_add(dy);
        ll.xlen ^= 1;
        ll.ylen ^= 1;
        std::mem::swap(&mut c1, &mut c2);
    }

    let major_len: i32 = 1 | (if ll.xlen >= ll.ylen { ll.xlen } else { ll.ylen }) as i32;

    // Fill the array for the specific length.
    let mut flash_array = [0u8; 256];
    let mut flash_pos: i32 = 0;
    for flash in 0..major_len as usize {
        let r = g.speed_rand();
        flash_array[flash] = r;
        flash_pos = flash_pos.wrapping_add(r as i32);
    }

    let (xstep, real_xlen): (i32, i32) = if ll.xlen & 1 != 0 {
        (-1, -(ll.xlen as i32))
    } else {
        (1, ll.xlen as i32)
    };
    let (ystep, real_ylen): (i32, i32) = if ll.ylen & 1 != 0 {
        (-1, -(ll.ylen as i32))
    } else {
        (1, ll.ylen as i32)
    };

    // Setup color ramp.
    let mut ramp_color: i32 = (c1 as i32).wrapping_shl(23);
    let color_slope: i32 = ((c2 as i32).wrapping_sub(c1 as i32)).wrapping_shl(23) / major_len;

    let umask = st.u_mask;
    let vmask = st.v_mask;
    let ubits = st.u_bits;

    if ll.xlen >= ll.ylen {
        // X major axis.
        let mut ypoz: i32 = (ll.ypos as i32).wrapping_shl(6);
        let flash_bias: i32 = (real_ylen.wrapping_shl(6).wrapping_sub(flash_pos)) / major_len;
        for flash in 0..ll.xlen as usize {
            ypoz = ypoz.wrapping_add(flash_array[flash] as i32 + flash_bias);
            // C: (((Ypoz>>6)&VMask)<<UBits) + (Xpos&UMask)
            let dest = texel((ll.xpos as i32) & umask, (ypoz >> 6) & vmask, ubits);
            ll.xpos = (ll.xpos as i32 + xstep) as u8;
            ramp_color = ramp_color.wrapping_add(color_slope);
            mips0[dest] = (ramp_color >> 23) as u8;
        }
    } else {
        // Y major axis.
        let mut xpoz: i32 = (ll.xpos as i32).wrapping_shl(6);
        let flash_bias: i32 = (real_xlen.wrapping_shl(6).wrapping_sub(flash_pos)) / major_len;
        for flash in 0..ll.ylen as usize {
            xpoz = xpoz.wrapping_add(flash_array[flash] as i32 + flash_bias);
            // C: ((Ypos&VMask)<<UBits) + ((Xpoz>>6)&UMask)
            let dest = texel((xpoz >> 6) & umask, (ll.ypos as i32) & vmask, ubits);
            ll.ypos = (ll.ypos as i32 + ystep) as u8;
            ramp_color = ramp_color.wrapping_add(color_slope);
            mips0[dest] = (ramp_color >> 23) as u8;
        }
    }
}

/// One axis of `MoveSparkXY`/`MoveSpark`/`MoveSparkAngle`/`MoveSparkTwo`.
/// Both branches call `SpeedRand()` exactly once, inside the condition.
fn step_axis(g: &mut FireGlobals, pos: &mut u8, speed: i32, mask: i32) {
    if speed < 0 {
        if ((g.speed_rand() & 127) as i32) < -speed {
            *pos = (mask & (*pos as i32 - 1)) as u8;
        }
    } else if ((g.speed_rand() & 127) as i32) < speed {
        *pos = (mask & (*pos as i32 + 1)) as u8;
    }
}

/// `UFireTexture::MoveSparkXY(Spark, SBYTE Xspeed, SBYTE Yspeed)`, C path.
fn move_spark_xy(
    g: &mut FireGlobals,
    st: &FireSparkState,
    spark: &mut FireSpark,
    xspeed: i8,
    yspeed: i8,
) {
    step_axis(g, &mut spark.x, xspeed as i32, st.u_mask);
    step_axis(g, &mut spark.y, yspeed as i32, st.v_mask);
}

/// `UFireTexture::MoveSpark(Spark)`, C path. Speeds come from ByteA and ByteB.
fn move_spark(g: &mut FireGlobals, st: &FireSparkState, spark: &mut FireSpark) {
    // Read both speeds before moving, as C does.
    let xspeed = spark.byte_a as i8;
    let yspeed = spark.byte_b as i8;
    move_spark_xy(g, st, spark, xspeed, yspeed);
}

/// `UFireTexture::MoveSparkAngle(Spark, Angle)`: moves along the phase-table direction of `angle`.
fn move_spark_angle(g: &mut FireGlobals, st: &FireSparkState, spark: &mut FireSpark, angle: u8) {
    // SBYTE Xdir = -127 + (SBYTE)PhaseTable[Angle]; stored as SBYTE.
    let xdir = (-127i32 + (g.phase_table[angle as usize] as i8 as i32)) as i8;
    // SBYTE Ydir = -127 + (SBYTE)PhaseTable[(Angle+64)&255]; stored as SBYTE.
    let ydir = (-127i32 + (g.phase_table[angle.wrapping_add(64) as usize] as i8 as i32)) as i8;
    move_spark_xy(g, st, spark, xdir, ydir);
}

/// `UFireTexture::MoveSparkTwo(Spark)`: X follows ByteA's sign, Y always drifts up by two.
fn move_spark_two(g: &mut FireGlobals, st: &FireSparkState, spark: &mut FireSpark) {
    let xspeed = spark.byte_a as i8 as i32;
    step_axis(g, &mut spark.x, xspeed, st.u_mask);
    spark.y = (st.v_mask & (spark.y as i32 - 2)) as u8;
}

/// Allocates the next spark slot (`NS = ActiveSparkNum++`) and returns it.
fn spawn<'a>(st: &mut FireSparkState, sparks: &'a mut [FireSpark]) -> &'a mut FireSpark {
    let ns = spawn_index(st);
    &mut sparks[ns]
}

/// `UFireTexture::RedrawSparks`: advances every spark one frame, writing into `mips0`.
pub fn redraw_sparks(
    trace: FireTraceHooks,
    st: &mut FireSparkState,
    sparks: &mut [FireSpark],
    mips0: &mut [u8],
) {
    let mut g = globals::lock();
    g.trace = trace;
    clamp_sparks_limit(st, sparks);

    st.aux_phase = st.aux_phase.wrapping_add(st.fx_frequency);
    st.global_phase = st.global_phase.wrapping_add(1);

    // `for( S=0; S<ActiveSparkNum; S++ )`: the bound is re-read every iteration.
    let mut s: usize = 0;
    while (s as i32) < st.active_spark_num {
        // C writes through ThisSpark. Spawns only touch slots >= s+1, so
        // working on a copy and writing it back is equivalent.
        let mut t = sparks[s];
        let mut removed = false;

        match t.ty {
            SPARK_BURN => {
                let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                let previous = mips0[dest];
                let value = g.speed_rand();
                mips0[dest] = value;
                (g.trace.burn_write)(
                    st.texture as *mut c_void,
                    s as i32,
                    dest as u32,
                    previous,
                    value,
                );
            }

            SPARK_SPARKLE => {
                let new_x = t.x as i32 + ((g.speed_rand() as i32 * t.byte_a as i32) >> 8);
                let new_y = t.y as i32 + ((g.speed_rand() as i32 * t.byte_b as i32) >> 8);
                let dest = texel(new_x & st.u_mask, new_y & st.v_mask, st.u_bits);
                mips0[dest] = t.heat;
            }

            SPARK_PULSE => {
                let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                mips0[dest] = t.heat;
                t.heat = t.heat.wrapping_add(t.byte_d);
            }

            SPARK_SIGNAL => {
                let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                if t.heat > t.byte_c {
                    mips0[dest] = t.heat;
                }
                // The `< ByteD` test uses the stored BYTE, so wrap-around triggers a reseed.
                t.heat = t.heat.wrapping_add(t.byte_d);
                if t.heat < t.byte_d {
                    t.heat = g.speed_rand();
                }
            }

            SPARK_CYLINDER => {
                let htemp = (g.phase_table[t.byte_a.wrapping_add(64) as usize] as i32
                    + t.heat as i32)
                    .min(255) as u8;
                let xtemp = (t.x as i32
                    + ((g.phase_table[t.byte_a as usize] as i32 * t.byte_b as i32) >> 8))
                    as u8;
                let dest = texel(xtemp as i32 & st.u_mask, t.y as i32, st.u_bits);
                mips0[dest] = htemp;
                t.byte_a = t.byte_a.wrapping_add(t.byte_d);
            }

            SPARK_CYLINDER3D => {
                // Only the front half of the cylinder is drawn.
                if t.byte_a.wrapping_add(64) < 128 {
                    let htemp = (g.phase_table[t.byte_a.wrapping_add(64) as usize] as i32
                        + t.heat as i32)
                        .min(255) as u8;
                    let xtemp = (t.x as i32
                        + ((g.phase_table[t.byte_a as usize] as i32 * t.byte_b as i32) >> 8))
                        as u8;
                    let dest = texel(xtemp as i32 & st.u_mask, t.y as i32, st.u_bits);
                    mips0[dest] = htemp;
                }
                t.byte_a = t.byte_a.wrapping_add(t.byte_d);
            }

            SPARK_JUGGLERS => {
                let htemp = (g.phase_table[t.byte_a.wrapping_add(64) as usize] as i32
                    + t.heat as i32)
                    .min(255) as u8;
                let ytemp = (t.y as i32
                    + ((g.phase_table[t.byte_a as usize] as i32 * t.byte_b as i32) >> 8))
                    as u8;
                // X is not masked by UMask here, unlike the other cylinder-style sparks.
                let dest = texel(t.x as i32, ytemp as i32 & st.v_mask, st.u_bits);
                mips0[dest] = htemp;
                t.byte_a = t.byte_a.wrapping_add(t.byte_d);
            }

            SPARK_LISSAJOUS => {
                let htemp = g.light_phase_table[t.byte_a.wrapping_add(64) as usize];
                let xtemp = (t.x as i32
                    + ((g.phase_table[t.byte_a as usize] as i32 * t.heat as i32) >> 8))
                    as u8;
                let ytemp = (t.y as i32
                    + ((g.phase_table[t.byte_b as usize] as i32 * t.heat as i32) >> 8))
                    as u8;
                let dest = texel(
                    xtemp as i32 & st.u_mask,
                    ytemp as i32 & st.v_mask,
                    st.u_bits,
                );
                mips0[dest] = htemp;
                t.byte_a = t.byte_a.wrapping_add(t.byte_c);
                t.byte_b = t.byte_b.wrapping_add(t.byte_d);
            }

            SPARK_LISSAJX => {
                let xtemp = (t.x as i32
                    + ((g.phase_table[t.byte_a as usize] as i32 * t.heat as i32) >> 8))
                    as u8;
                let htemp = g.light_phase_table[t.byte_a.wrapping_add(64) as usize];
                let dest = texel(xtemp as i32 & st.u_mask, t.y as i32, st.u_bits);
                mips0[dest] = htemp;
                t.byte_a = t.byte_a.wrapping_add(t.byte_c);
            }

            SPARK_LISSAJY => {
                let ytemp = (t.y as i32
                    + ((g.phase_table[t.byte_b as usize] as i32 * t.heat as i32) >> 8))
                    as u8;
                let htemp = g.light_phase_table[t.byte_b.wrapping_add(64) as usize];
                let dest = texel(t.x as i32, ytemp as i32 & st.v_mask, st.u_bits);
                mips0[dest] = htemp;
                t.byte_b = t.byte_b.wrapping_add(t.byte_d);
            }

            SPARK_BLAZE => {
                if st.active_spark_num < st.sparks_limit && 128 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_DRIFTER;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = g.speed_rand();
                    n.byte_b = g.speed_rand();
                    n.byte_c = t.byte_c;
                    n.byte_d = t.byte_d;
                }
            }

            SPARK_OZ_HAS_SPOKEN => {
                if st.active_spark_num < st.sparks_limit && 128 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_DRIFT_SLOW;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = (g.speed_rand() & 127).wrapping_sub(63);
                    n.byte_b = (-127i32) as u8;
                    n.byte_d = 2;
                }
            }

            SPARK_CONE => {
                if st.active_spark_num < st.sparks_limit && 64 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_FALLER;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = (g.speed_rand() & 127).wrapping_sub(63);
                    n.byte_b = 0;
                    n.byte_c = 50;
                }
            }

            SPARK_BLAZE_RIGHT => {
                if st.active_spark_num < st.sparks_limit && 64 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_FALLER;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = (g.speed_rand() & 63).wrapping_add(63);
                    n.byte_b = (-29i32) as u8;
                    n.byte_c = t.byte_c;
                }
            }

            SPARK_BLAZE_LEFT => {
                if st.active_spark_num < st.sparks_limit && 64 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_FALLER;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = (g.speed_rand() & 63).wrapping_sub(128);
                    n.byte_b = (-29i32) as u8;
                    n.byte_c = t.byte_c;
                }
            }

            SPARK_EMIT => {
                if st.active_spark_num < st.sparks_limit && 64 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_DRIFT_SLOW;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = t.byte_a;
                    n.byte_b = t.byte_b;
                    n.byte_d = t.byte_d;
                }
            }

            SPARK_FOUNTAIN => {
                if st.active_spark_num < st.sparks_limit && 64 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_GRAVITON;
                    n.heat = t.heat;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_a = t.byte_a;
                    n.byte_b = t.byte_b;
                    n.byte_d = t.byte_d;
                }
            }

            SPARK_ORGANIC => {
                if st.active_spark_num < st.sparks_limit && 128 > g.speed_rand() {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_VSHOOTER;
                    n.x = (st.u_mask
                        & (t.x as i32 + ((g.speed_rand() as i32 * t.byte_c as i32) >> 8)))
                        as u8;
                    n.y = (st.v_mask
                        & (t.y as i32 + ((g.speed_rand() as i32 * t.byte_c as i32) >> 8)))
                        as u8;
                    n.byte_a = g.speed_rand().wrapping_sub(127);
                    n.byte_b = (256 - 127) as u8;
                    n.byte_c = 255;
                }
            }

            SPARK_WANDER_ORGANIC => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_VSHOOTER;
                    n.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.byte_a = g.speed_rand().wrapping_sub(127);
                    n.byte_b = (256 - 127) as u8;
                    n.byte_c = 255;
                }
                // `if( SpeedRand() & 15 == 15 )` is `SpeedRand() & 1` in C, since `==` binds tighter than `&`.
                if g.speed_rand() & 1 != 0 {
                    t.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
                if g.speed_rand() & 1 != 0 {
                    t.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
            }

            SPARK_RANDOM_CLOUD => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_DROP;
                    n.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.byte_a = (g.speed_rand() & 31).wrapping_sub(15);
                    n.byte_b = (256 - 127) as u8;
                    n.byte_c = 0;
                }
                // Same `SpeedRand() & 15 == 15` precedence quirk as WanderOrganic.
                if g.speed_rand() & 1 != 0 {
                    t.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
                if g.speed_rand() & 1 != 0 {
                    t.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
            }

            SPARK_EELS => {
                // SpeedRand() is evaluated before the limit check, so it always runs.
                if g.speed_rand() < 20 && st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.heat = t.heat;
                    n.ty = ISPARK_SPAWNED_EEL;
                    n.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.byte_a = g.speed_rand();
                    n.byte_b = g.speed_rand();
                    n.byte_c = t.byte_c;
                }
                // Same `SpeedRand() & 15 == 15` precedence quirk as WanderOrganic.
                if g.speed_rand() & 1 != 0 {
                    t.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
                if g.speed_rand() & 1 != 0 {
                    t.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
            }

            SPARK_GAMETES => {
                if g.speed_rand() < 20 && st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_SPAWNED_SPERM;
                    n.heat = t.heat;
                    n.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.byte_a = g.speed_rand();
                    n.byte_c = t.byte_c;
                    n.byte_d = g.speed_rand();
                }
                // Same `SpeedRand() & 15 == 15` precedence quirk as WanderOrganic.
                if g.speed_rand() & 1 != 0 {
                    t.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
                if g.speed_rand() & 1 != 0 {
                    t.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 15) as i32 - 7)) as u8;
                }
            }

            SPARK_CUSTOM_CLOUD => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_MOVE;
                    n.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.byte_a = t.byte_a;
                    n.byte_b = t.byte_b;
                    n.byte_c = t.byte_d;
                }
                // The C expression `X + (SpeedRand()&7) - (SpeedRand()&7)` has unspecified
                // operand order. This port evaluates left to right.
                let dx1 = (g.speed_rand() & 7) as i32;
                let dx2 = (g.speed_rand() & 7) as i32;
                t.x = (st.u_mask & (t.x as i32 + dx1 - dx2)) as u8;
                let dy1 = (g.speed_rand() & 7) as i32;
                let dy2 = (g.speed_rand() & 7) as i32;
                t.y = (st.v_mask & (t.y as i32 + dy1 - dy2)) as u8;
            }

            SPARK_LOCAL_CLOUD => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_MOVE;
                    n.x = (st.u_mask
                        & (t.x as i32 + ((g.speed_rand() as i32 * t.byte_c as i32) >> 8)))
                        as u8;
                    n.y = (st.v_mask
                        & (t.y as i32 + ((g.speed_rand() as i32 * t.byte_c as i32) >> 8)))
                        as u8;
                    n.byte_a = t.byte_a;
                    n.byte_b = t.byte_b;
                    n.byte_c = t.byte_d;
                }
            }

            SPARK_FLOCKS => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_SPAWNED_TWIRL;
                    n.x = (st.u_mask & (t.x as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.y = (st.v_mask & (t.y as i32 + (g.speed_rand() & 31) as i32)) as u8;
                    n.byte_b = t.byte_a;
                    n.byte_a = 0;
                    n.byte_c = t.byte_b;
                    n.byte_d = t.byte_d;
                    n.heat = t.heat;
                    // Turn the spawner's own direction; this is read before the update above.
                    t.byte_a = t.byte_a.wrapping_add(t.byte_c);
                }
                // Unspecified operand order in C, left to right here.
                let dx1 = (g.speed_rand() & 7) as i32;
                let dx2 = (g.speed_rand() & 7) as i32;
                t.x = (st.u_mask & (t.x as i32 + dx1 - dx2)) as u8;
                let dy1 = (g.speed_rand() & 7) as i32;
                let dy2 = (g.speed_rand() & 7) as i32;
                t.y = (st.v_mask & (t.y as i32 + dy1 - dy2)) as u8;
            }

            SPARK_WHEEL => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_SPAWNED_TWIRL;
                    n.x = t.x;
                    n.y = t.y;
                    n.byte_b = t.byte_a;
                    n.byte_a = 0;
                    n.byte_c = t.byte_b;
                    n.byte_d = t.byte_d;
                    n.heat = t.heat;
                }
                t.byte_a = t.byte_a.wrapping_add(t.byte_c);
            }

            SPARK_SPRINKLER => {
                if st.active_spark_num < st.sparks_limit {
                    let n = spawn(st, sparks);
                    n.ty = ISPARK_SPRINKLER_TWIRL;
                    n.x = t.x;
                    n.y = t.y;
                    n.heat = t.heat;
                    n.byte_a = t.byte_a;
                    n.byte_b = t.byte_b;
                    n.byte_c = t.byte_c;
                    n.byte_d = 2;
                }
                t.byte_a = t.byte_a.wrapping_add(t.byte_d);
            }

            SPARK_STARS => {
                let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                mips0[dest] = t.byte_b;
            }

            SPARK_LINE_LIGHTNING => {
                // Too-small bolts have Heat 0.
                if t.heat != 0 {
                    if t.byte_c > 0 {
                        t.byte_c = t.byte_c.wrapping_sub(1);
                        let ll = FireLineSeg {
                            xlen: t.byte_a,
                            ylen: t.byte_b,
                            xpos: t.x,
                            ypos: t.y,
                        };
                        let heat_a = t.heat;
                        draw_flash_ramp(&mut g, st, mips0, ll, heat_a, heat_a);
                    } else if g.speed_rand() >= t.byte_d {
                        // Initiate new flash. `1 + SpeedRand() & 5` is `(1 + SpeedRand()) & 5` in C.
                        let r = g.speed_rand();
                        t.byte_c = ((1 + r as i32) & 5) as u8;
                    }
                }
            }

            SPARK_RAMP_LIGHTNING => {
                if t.heat != 0 {
                    if t.byte_c > 0 {
                        t.byte_c = t.byte_c.wrapping_sub(1);
                        let ll = FireLineSeg {
                            xlen: t.byte_a,
                            ylen: t.byte_b,
                            xpos: t.x,
                            ypos: t.y,
                        };
                        let heat_a = t.heat;
                        let heat_b = heat_a >> 3;
                        draw_flash_ramp(&mut g, st, mips0, ll, heat_a, heat_b);
                    } else if g.speed_rand() >= t.byte_d {
                        // Same `1 + SpeedRand() & 5` precedence as LineLightning.
                        let r = g.speed_rand();
                        t.byte_c = ((1 + r as i32) & 5) as u8;
                    }
                }
            }

            SPARK_SPHERE_LIGHTNING => {
                if g.speed_rand() >= t.byte_d {
                    let spark_angle = g.speed_rand() as i32;
                    let radius = t.byte_c as i32;
                    let col1 = t.heat;
                    let col2 = t.heat >> 2;
                    let sin_x = g.phase_table[spark_angle as usize] as i32;
                    let sin_y = g.phase_table[((spark_angle + 64) & 255) as usize] as i32;
                    let sdisp_x = (radius * sin_x) >> 8;
                    let sdisp_y = (radius * sin_y) >> 8;
                    let mut xlen = sdisp_x - radius / 2;
                    let mut ylen = sdisp_y - radius / 2;
                    // Cram the sign bit into the lsbit for negatives, else clear the lsbit.
                    if xlen <= 0 {
                        xlen = xlen.wrapping_neg() | 1;
                    } else {
                        xlen &= -2i32; // 0xFFFFFFFE
                    }
                    if ylen <= 0 {
                        ylen = ylen.wrapping_neg() | 1;
                    } else {
                        ylen &= -2i32; // 0xFFFFFFFE
                    }
                    let ll = FireLineSeg {
                        xpos: t.x,
                        ypos: t.y,
                        xlen: xlen as u8,
                        ylen: ylen as u8,
                    };
                    draw_flash_ramp(&mut g, st, mips0, ll, col1, col2);
                }
            }

            ISPARK_DRIFTER => {
                t.heat = t.heat.wrapping_sub(5);
                if t.heat < 251 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    move_spark(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_DRIFT_SLOW => {
                t.heat = t.heat.wrapping_sub(t.byte_d);
                if t.heat > t.byte_d {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    move_spark(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_CUSTOM => {
                t.heat = t.heat.wrapping_sub(t.byte_c);
                if t.heat < 250 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    move_spark(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_FALLER => {
                t.byte_c = t.byte_c.wrapping_sub(1);
                if t.byte_c > 0 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    move_spark(&mut g, st, &mut t);
                    if (t.byte_b as i8) < 122 {
                        t.byte_b = t.byte_b.wrapping_add(3);
                    }
                } else {
                    removed = true;
                }
            }

            ISPARK_GRAVITON => {
                t.heat = t.heat.wrapping_sub(t.byte_d);
                if t.heat as i32 > LOWEST_GLOW {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    move_spark(&mut g, st, &mut t);
                    if st.global_phase & 1 != 0 && (t.byte_b as i8) < 124 {
                        t.byte_b = t.byte_b.wrapping_add(3);
                    }
                } else {
                    removed = true;
                }
            }

            ISPARK_VSHOOTER => {
                t.byte_c = t.byte_c.wrapping_sub(3);
                if t.byte_c > 190 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.byte_c;
                    move_spark_two(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_DROP => {
                t.byte_c = t.byte_c.wrapping_add(4);
                if t.byte_c < 250 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.byte_c;
                    move_spark_two(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_MOVE => {
                t.byte_c = t.byte_c.wrapping_add(4);
                if t.byte_c < 250 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.byte_c;
                    move_spark(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_SPAWNED_EEL => {
                t.byte_c = t.byte_c.wrapping_sub(1);
                if t.byte_c < 255 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    move_spark(&mut g, st, &mut t);
                } else {
                    removed = true;
                }
            }

            ISPARK_SPAWNED_SPERM => {
                t.byte_c = t.byte_c.wrapping_sub(1);
                if t.byte_c < 255 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    // Jiggle direction = ByteD using a zigzag ByteA.
                    t.byte_a = t.byte_a.wrapping_add(7);
                    let mut saw_tooth = 127 & t.byte_a;
                    if saw_tooth > 63 {
                        saw_tooth = 127 - saw_tooth;
                    }
                    // C: 255 & (SawTooth + ByteD), stored as BYTE; u8 wrapping add is the same.
                    let angle = saw_tooth.wrapping_add(t.byte_d);
                    move_spark_angle(&mut g, st, &mut t, angle);
                } else {
                    removed = true;
                }
            }

            ISPARK_SPAWNED_TWIRL => {
                t.byte_c = t.byte_c.wrapping_sub(1);
                if t.byte_c < 255 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    // Both speeds come from the table before ByteA is updated.
                    let temp_speed_x = g.signed_phase_table[t.byte_b.wrapping_add(64) as usize];
                    let temp_speed_y = g.signed_phase_table[t.byte_b as usize];
                    // `*(WORD*)&ByteA += (WORD)ByteD << 4`. This assumes little-endian layout,
                    // with ByteA as the low byte and ByteB as the high byte.
                    let angle = u16::from_le_bytes([t.byte_a, t.byte_b])
                        .wrapping_add((t.byte_d as u16) << 4);
                    let [lo, hi] = angle.to_le_bytes();
                    t.byte_a = lo;
                    t.byte_b = hi;
                    move_spark_xy(&mut g, st, &mut t, temp_speed_x as i8, temp_speed_y as i8);
                } else {
                    removed = true;
                }
            }

            ISPARK_SPRINKLER_TWIRL => {
                t.byte_c = t.byte_c.wrapping_sub(1);
                if t.byte_c < 255 {
                    let dest = texel(t.x as i32, t.y as i32, st.u_bits);
                    mips0[dest] = t.heat;
                    // C: BYTE TempSpeedX = -128 + (SBYTE)PhaseTable[(ByteA+64)&255];
                    let phase = g.phase_table[t.byte_a.wrapping_add(64) as usize] as i8 as i32;
                    let temp_speed_x = ((-128i32 + phase) as u8) as i8;
                    let temp_speed_y = t.byte_b as i8;
                    t.byte_a = t.byte_a.wrapping_add(t.byte_d);
                    move_spark_xy(&mut g, st, &mut t, temp_speed_x, temp_speed_y);
                } else {
                    removed = true;
                }
            }

            _ => {}
        }

        // C mutates ThisSpark in place, so those changes stay in the slot. When the
        // removed spark is the last active one, C's self-copy keeps them too.
        sparks[s] = t;
        if removed {
            // `*ThisSpark = Sparks(--ActiveSparkNum)`. The moved spark is not re-tested.
            st.active_spark_num -= 1;
            sparks[s] = sparks[st.active_spark_num as usize];
        }
        s += 1;
    }
}
