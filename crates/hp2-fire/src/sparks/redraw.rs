//! Port of `UFireTexture::RedrawSparks` and its helpers from `UnFractal.cpp`:
//! `DrawFlashRamp`, `MoveSpark`, `MoveSparkXY`, `MoveSparkAngle`, `MoveSparkTwo`.
//! The `FIRE_X86_ASM && 0` variants of `MoveSparkXY` and `MoveSpark` are dead
//! and are not ported. Only the C paths remain.

use std::ffi::c_void;

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
use super::{clamp_sparks_limit, pixel_at, spark_at};
use crate::abi::{FireArena, FireCallbackError, FireLineSeg, FireSpark, FireTraceHooks};
use crate::globals::FireSession;

/// `UFireTexture::DrawFlashRamp`: draws a Bresenham-style ramp along `ll`.
/// `ll` is passed by value and the copy is what gets modified, as in C.
#[allow(clippy::needless_range_loop)] // index loops mirror the C flash loops
unsafe fn draw_flash_ramp(
    g: &mut FireSession,
    st: *mut FireSparkState,
    mips0: *const FireArena<u8>,
    mut ll: FireLineSeg,
    color1: u8,
    color2: u8,
) -> Result<(), FireCallbackError> {
    unsafe {
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
            let r = g.speed_rand()?;
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

        let umask = (*st).u_mask;
        let vmask = (*st).v_mask;
        let ubits = (*st).u_bits;

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
                (*pixel_at(mips0, dest)) = (ramp_color >> 23) as u8;
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
                (*pixel_at(mips0, dest)) = (ramp_color >> 23) as u8;
            }
        }
        Ok(())
    }
}

/// One axis of movement. Resolve the field and mask only after tracing succeeds.
unsafe fn step_axis(
    g: &mut FireSession,
    st: *mut FireSparkState,
    sparks: *const FireArena<FireSpark>,
    index: usize,
    x_axis: bool,
    speed: i32,
) -> Result<(), FireCallbackError> {
    let random = (g.speed_rand()? & 127) as i32;
    let delta = if speed < 0 {
        if random >= -speed {
            return Ok(());
        }
        -1
    } else {
        if random >= speed {
            return Ok(());
        }
        1
    };
    unsafe {
        let spark = spark_at(sparks, index);
        if x_axis {
            (*spark).x = ((*st).u_mask & ((*spark).x as i32 + delta)) as u8;
        } else {
            (*spark).y = ((*st).v_mask & ((*spark).y as i32 + delta)) as u8;
        }
    }
    Ok(())
}

/// `UFireTexture::MoveSparkXY`, with no retained native field reference.
unsafe fn move_spark_xy(
    g: &mut FireSession,
    st: *mut FireSparkState,
    sparks: *const FireArena<FireSpark>,
    index: usize,
    xspeed: i8,
    yspeed: i8,
) -> Result<(), FireCallbackError> {
    unsafe {
        step_axis(g, st, sparks, index, true, xspeed as i32)?;
        step_axis(g, st, sparks, index, false, yspeed as i32)
    }
}

/// `UFireTexture::MoveSpark`: both speed arguments are read before moving.
unsafe fn move_spark(
    g: &mut FireSession,
    st: *mut FireSparkState,
    sparks: *const FireArena<FireSpark>,
    index: usize,
) -> Result<(), FireCallbackError> {
    unsafe {
        let xspeed = (*spark_at(sparks, index)).byte_a as i8;
        let yspeed = (*spark_at(sparks, index)).byte_b as i8;
        move_spark_xy(g, st, sparks, index, xspeed, yspeed)
    }
}

/// `UFireTexture::MoveSparkAngle`: phase-table directions are speed arguments.
unsafe fn move_spark_angle(
    g: &mut FireSession,
    st: *mut FireSparkState,
    sparks: *const FireArena<FireSpark>,
    index: usize,
    angle: u8,
) -> Result<(), FireCallbackError> {
    let xdir = (-127i32 + (g.phase(angle as usize) as i8 as i32)) as i8;
    let ydir = (-127i32 + (g.phase(angle.wrapping_add(64) as usize) as i8 as i32)) as i8;
    unsafe { move_spark_xy(g, st, sparks, index, xdir, ydir) }
}

/// `UFireTexture::MoveSparkTwo`: X follows ByteA, Y drifts up by two.
unsafe fn move_spark_two(
    g: &mut FireSession,
    st: *mut FireSparkState,
    sparks: *const FireArena<FireSpark>,
    index: usize,
) -> Result<(), FireCallbackError> {
    unsafe {
        let xspeed = (*spark_at(sparks, index)).byte_a as i8 as i32;
        step_axis(g, st, sparks, index, true, xspeed)?;
        let spark = spark_at(sparks, index);
        (*spark).y = ((*st).v_mask & ((*spark).y as i32 - 2)) as u8;
    }
    Ok(())
}

/// `UFireTexture::RedrawSparks`: advances every spark one frame, writing into `mips0`.
///
/// # Safety
/// State and descriptor addresses must survive reentry; successful callbacks must
/// refresh scalars and descriptors before returning. See the parent module.
pub unsafe fn redraw_sparks(
    trace: FireTraceHooks,
    st: *mut FireSparkState,
    sparks: *const FireArena<FireSpark>,
    mips0: *const FireArena<u8>,
) -> Result<(), FireCallbackError> {
    unsafe {
        let mut g = FireSession::new(trace);
        clamp_sparks_limit(st, sparks);

        (*st).aux_phase = (*st).aux_phase.wrapping_add((*st).fx_frequency);
        (*st).global_phase = (*st).global_phase.wrapping_add(1);

        // `for( S=0; S<ActiveSparkNum; S++ )`: the bound is re-read every iteration.
        let mut s: usize = 0;
        while (s as i32) < (*st).active_spark_num {
            // Keep only the slot index across callbacks, never a spark snapshot or base.
            let t = s;
            let mut removed = false;

            match (*spark_at(sparks, t)).ty {
                SPARK_BURN => {
                    let dest = texel(
                        (*spark_at(sparks, t)).x as i32,
                        (*spark_at(sparks, t)).y as i32,
                        (*st).u_bits,
                    );
                    let previous = (*pixel_at(mips0, dest));
                    let value = g.speed_rand()?;
                    (*pixel_at(mips0, dest)) = value;
                    trace.burn_write(
                        (*st).texture as *mut c_void,
                        s as i32,
                        dest as u32,
                        previous,
                        value,
                    )?;
                }

                SPARK_SPARKLE => {
                    let new_x = (*spark_at(sparks, t)).x as i32
                        + ((g.speed_rand()? as i32 * (*spark_at(sparks, t)).byte_a as i32) >> 8);
                    let new_y = (*spark_at(sparks, t)).y as i32
                        + ((g.speed_rand()? as i32 * (*spark_at(sparks, t)).byte_b as i32) >> 8);
                    let dest = texel(new_x & (*st).u_mask, new_y & (*st).v_mask, (*st).u_bits);
                    (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                }

                SPARK_PULSE => {
                    let dest = texel(
                        (*spark_at(sparks, t)).x as i32,
                        (*spark_at(sparks, t)).y as i32,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                    (*spark_at(sparks, t)).heat = (*spark_at(sparks, t))
                        .heat
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_SIGNAL => {
                    let dest = texel(
                        (*spark_at(sparks, t)).x as i32,
                        (*spark_at(sparks, t)).y as i32,
                        (*st).u_bits,
                    );
                    if (*spark_at(sparks, t)).heat > (*spark_at(sparks, t)).byte_c {
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                    }
                    // The `< ByteD` test uses the stored BYTE, so wrap-around triggers a reseed.
                    (*spark_at(sparks, t)).heat = (*spark_at(sparks, t))
                        .heat
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                    if (*spark_at(sparks, t)).heat < (*spark_at(sparks, t)).byte_d {
                        (*spark_at(sparks, t)).heat = g.speed_rand()?;
                    }
                }

                SPARK_CYLINDER => {
                    let htemp = (g.phase((*spark_at(sparks, t)).byte_a.wrapping_add(64) as usize)
                        as i32
                        + (*spark_at(sparks, t)).heat as i32)
                        .min(255) as u8;
                    let xtemp = ((*spark_at(sparks, t)).x as i32
                        + ((g.phase((*spark_at(sparks, t)).byte_a as usize) as i32
                            * (*spark_at(sparks, t)).byte_b as i32)
                            >> 8)) as u8;
                    let dest = texel(
                        xtemp as i32 & (*st).u_mask,
                        (*spark_at(sparks, t)).y as i32,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = htemp;
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_CYLINDER3D => {
                    // Only the front half of the cylinder is drawn.
                    if (*spark_at(sparks, t)).byte_a.wrapping_add(64) < 128 {
                        let htemp = (g
                            .phase((*spark_at(sparks, t)).byte_a.wrapping_add(64) as usize)
                            as i32
                            + (*spark_at(sparks, t)).heat as i32)
                            .min(255) as u8;
                        let xtemp = ((*spark_at(sparks, t)).x as i32
                            + ((g.phase((*spark_at(sparks, t)).byte_a as usize) as i32
                                * (*spark_at(sparks, t)).byte_b as i32)
                                >> 8)) as u8;
                        let dest = texel(
                            xtemp as i32 & (*st).u_mask,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = htemp;
                    }
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_JUGGLERS => {
                    let htemp = (g.phase((*spark_at(sparks, t)).byte_a.wrapping_add(64) as usize)
                        as i32
                        + (*spark_at(sparks, t)).heat as i32)
                        .min(255) as u8;
                    let ytemp = ((*spark_at(sparks, t)).y as i32
                        + ((g.phase((*spark_at(sparks, t)).byte_a as usize) as i32
                            * (*spark_at(sparks, t)).byte_b as i32)
                            >> 8)) as u8;
                    // X is not masked by UMask here, unlike the other cylinder-style sparks.
                    let dest = texel(
                        (*spark_at(sparks, t)).x as i32,
                        ytemp as i32 & (*st).v_mask,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = htemp;
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_LISSAJOUS => {
                    let htemp =
                        g.light_phase((*spark_at(sparks, t)).byte_a.wrapping_add(64) as usize);
                    let xtemp = ((*spark_at(sparks, t)).x as i32
                        + ((g.phase((*spark_at(sparks, t)).byte_a as usize) as i32
                            * (*spark_at(sparks, t)).heat as i32)
                            >> 8)) as u8;
                    let ytemp = ((*spark_at(sparks, t)).y as i32
                        + ((g.phase((*spark_at(sparks, t)).byte_b as usize) as i32
                            * (*spark_at(sparks, t)).heat as i32)
                            >> 8)) as u8;
                    let dest = texel(
                        xtemp as i32 & (*st).u_mask,
                        ytemp as i32 & (*st).v_mask,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = htemp;
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_c);
                    (*spark_at(sparks, t)).byte_b = (*spark_at(sparks, t))
                        .byte_b
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_LISSAJX => {
                    let xtemp = ((*spark_at(sparks, t)).x as i32
                        + ((g.phase((*spark_at(sparks, t)).byte_a as usize) as i32
                            * (*spark_at(sparks, t)).heat as i32)
                            >> 8)) as u8;
                    let htemp =
                        g.light_phase((*spark_at(sparks, t)).byte_a.wrapping_add(64) as usize);
                    let dest = texel(
                        xtemp as i32 & (*st).u_mask,
                        (*spark_at(sparks, t)).y as i32,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = htemp;
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_c);
                }

                SPARK_LISSAJY => {
                    let ytemp = ((*spark_at(sparks, t)).y as i32
                        + ((g.phase((*spark_at(sparks, t)).byte_b as usize) as i32
                            * (*spark_at(sparks, t)).heat as i32)
                            >> 8)) as u8;
                    let htemp =
                        g.light_phase((*spark_at(sparks, t)).byte_b.wrapping_add(64) as usize);
                    let dest = texel(
                        (*spark_at(sparks, t)).x as i32,
                        ytemp as i32 & (*st).v_mask,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = htemp;
                    (*spark_at(sparks, t)).byte_b = (*spark_at(sparks, t))
                        .byte_b
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_BLAZE => {
                    if (*st).active_spark_num < (*st).sparks_limit && 128 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_DRIFTER;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = g.speed_rand()?;
                        (*spark_at(sparks, n)).byte_b = g.speed_rand()?;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_c;
                        (*spark_at(sparks, n)).byte_d = (*spark_at(sparks, t)).byte_d;
                    }
                }

                SPARK_OZ_HAS_SPOKEN => {
                    if (*st).active_spark_num < (*st).sparks_limit && 128 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_DRIFT_SLOW;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = (g.speed_rand()? & 127).wrapping_sub(63);
                        (*spark_at(sparks, n)).byte_b = (-127i32) as u8;
                        (*spark_at(sparks, n)).byte_d = 2;
                    }
                }

                SPARK_CONE => {
                    if (*st).active_spark_num < (*st).sparks_limit && 64 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_FALLER;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = (g.speed_rand()? & 127).wrapping_sub(63);
                        (*spark_at(sparks, n)).byte_b = 0;
                        (*spark_at(sparks, n)).byte_c = 50;
                    }
                }

                SPARK_BLAZE_RIGHT => {
                    if (*st).active_spark_num < (*st).sparks_limit && 64 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_FALLER;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = (g.speed_rand()? & 63).wrapping_add(63);
                        (*spark_at(sparks, n)).byte_b = (-29i32) as u8;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_c;
                    }
                }

                SPARK_BLAZE_LEFT => {
                    if (*st).active_spark_num < (*st).sparks_limit && 64 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_FALLER;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = (g.speed_rand()? & 63).wrapping_sub(128);
                        (*spark_at(sparks, n)).byte_b = (-29i32) as u8;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_c;
                    }
                }

                SPARK_EMIT => {
                    if (*st).active_spark_num < (*st).sparks_limit && 64 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_DRIFT_SLOW;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_d = (*spark_at(sparks, t)).byte_d;
                    }
                }

                SPARK_FOUNTAIN => {
                    if (*st).active_spark_num < (*st).sparks_limit && 64 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_GRAVITON;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_a = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_d = (*spark_at(sparks, t)).byte_d;
                    }
                }

                SPARK_ORGANIC => {
                    if (*st).active_spark_num < (*st).sparks_limit && 128 > g.speed_rand()? {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_VSHOOTER;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32
                                + ((g.speed_rand()? as i32
                                    * (*spark_at(sparks, t)).byte_c as i32)
                                    >> 8)))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32
                                + ((g.speed_rand()? as i32
                                    * (*spark_at(sparks, t)).byte_c as i32)
                                    >> 8)))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = g.speed_rand()?.wrapping_sub(127);
                        (*spark_at(sparks, n)).byte_b = (256 - 127) as u8;
                        (*spark_at(sparks, n)).byte_c = 255;
                    }
                }

                SPARK_WANDER_ORGANIC => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_VSHOOTER;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = g.speed_rand()?.wrapping_sub(127);
                        (*spark_at(sparks, n)).byte_b = (256 - 127) as u8;
                        (*spark_at(sparks, n)).byte_c = 255;
                    }
                    // `if( SpeedRand() & 15 == 15 )` is `SpeedRand() & 1` in C, since `==` binds tighter than `&`.
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                }

                SPARK_RANDOM_CLOUD => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_DROP;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = (g.speed_rand()? & 31).wrapping_sub(15);
                        (*spark_at(sparks, n)).byte_b = (256 - 127) as u8;
                        (*spark_at(sparks, n)).byte_c = 0;
                    }
                    // Same `SpeedRand() & 15 == 15` precedence quirk as WanderOrganic.
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                }

                SPARK_EELS => {
                    // SpeedRand() is evaluated before the limit check, so it always runs.
                    if g.speed_rand()? < 20 && (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).ty = ISPARK_SPAWNED_EEL;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = g.speed_rand()?;
                        (*spark_at(sparks, n)).byte_b = g.speed_rand()?;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_c;
                    }
                    // Same `SpeedRand() & 15 == 15` precedence quirk as WanderOrganic.
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                }

                SPARK_GAMETES => {
                    if g.speed_rand()? < 20 && (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_SPAWNED_SPERM;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = g.speed_rand()?;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_c;
                        (*spark_at(sparks, n)).byte_d = g.speed_rand()?;
                    }
                    // Same `SpeedRand() & 15 == 15` precedence quirk as WanderOrganic.
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                    if g.speed_rand()? & 1 != 0 {
                        (*spark_at(sparks, t)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 15) as i32 - 7))
                            as u8;
                    }
                }

                SPARK_CUSTOM_CLOUD => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_MOVE;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_d;
                    }
                    // The C expression `X + (SpeedRand()&7) - (SpeedRand()&7)` has unspecified
                    // operand order. This port evaluates left to right.
                    let dx1 = (g.speed_rand()? & 7) as i32;
                    let dx2 = (g.speed_rand()? & 7) as i32;
                    (*spark_at(sparks, t)).x =
                        ((*st).u_mask & ((*spark_at(sparks, t)).x as i32 + dx1 - dx2)) as u8;
                    let dy1 = (g.speed_rand()? & 7) as i32;
                    let dy2 = (g.speed_rand()? & 7) as i32;
                    (*spark_at(sparks, t)).y =
                        ((*st).v_mask & ((*spark_at(sparks, t)).y as i32 + dy1 - dy2)) as u8;
                }

                SPARK_LOCAL_CLOUD => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_MOVE;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32
                                + ((g.speed_rand()? as i32
                                    * (*spark_at(sparks, t)).byte_c as i32)
                                    >> 8)))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32
                                + ((g.speed_rand()? as i32
                                    * (*spark_at(sparks, t)).byte_c as i32)
                                    >> 8)))
                            as u8;
                        (*spark_at(sparks, n)).byte_a = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_d;
                    }
                }

                SPARK_FLOCKS => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_SPAWNED_TWIRL;
                        (*spark_at(sparks, n)).x = ((*st).u_mask
                            & ((*spark_at(sparks, t)).x as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).y = ((*st).v_mask
                            & ((*spark_at(sparks, t)).y as i32 + (g.speed_rand()? & 31) as i32))
                            as u8;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_a = 0;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_d = (*spark_at(sparks, t)).byte_d;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        // Turn the spawner's own direction; this is read before the update above.
                        (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                            .byte_a
                            .wrapping_add((*spark_at(sparks, t)).byte_c);
                    }
                    // Unspecified operand order in C, left to right here.
                    let dx1 = (g.speed_rand()? & 7) as i32;
                    let dx2 = (g.speed_rand()? & 7) as i32;
                    (*spark_at(sparks, t)).x =
                        ((*st).u_mask & ((*spark_at(sparks, t)).x as i32 + dx1 - dx2)) as u8;
                    let dy1 = (g.speed_rand()? & 7) as i32;
                    let dy2 = (g.speed_rand()? & 7) as i32;
                    (*spark_at(sparks, t)).y =
                        ((*st).v_mask & ((*spark_at(sparks, t)).y as i32 + dy1 - dy2)) as u8;
                }

                SPARK_WHEEL => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_SPAWNED_TWIRL;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_a = 0;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_d = (*spark_at(sparks, t)).byte_d;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                    }
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_c);
                }

                SPARK_SPRINKLER => {
                    if (*st).active_spark_num < (*st).sparks_limit {
                        let n = spawn_index(st);
                        (*spark_at(sparks, n)).ty = ISPARK_SPRINKLER_TWIRL;
                        (*spark_at(sparks, n)).x = (*spark_at(sparks, t)).x;
                        (*spark_at(sparks, n)).y = (*spark_at(sparks, t)).y;
                        (*spark_at(sparks, n)).heat = (*spark_at(sparks, t)).heat;
                        (*spark_at(sparks, n)).byte_a = (*spark_at(sparks, t)).byte_a;
                        (*spark_at(sparks, n)).byte_b = (*spark_at(sparks, t)).byte_b;
                        (*spark_at(sparks, n)).byte_c = (*spark_at(sparks, t)).byte_c;
                        (*spark_at(sparks, n)).byte_d = 2;
                    }
                    (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                        .byte_a
                        .wrapping_add((*spark_at(sparks, t)).byte_d);
                }

                SPARK_STARS => {
                    let dest = texel(
                        (*spark_at(sparks, t)).x as i32,
                        (*spark_at(sparks, t)).y as i32,
                        (*st).u_bits,
                    );
                    (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).byte_b;
                }

                SPARK_LINE_LIGHTNING => {
                    // Too-small bolts have Heat 0.
                    if (*spark_at(sparks, t)).heat != 0 {
                        if (*spark_at(sparks, t)).byte_c > 0 {
                            (*spark_at(sparks, t)).byte_c =
                                (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                            let ll = FireLineSeg {
                                xlen: (*spark_at(sparks, t)).byte_a,
                                ylen: (*spark_at(sparks, t)).byte_b,
                                xpos: (*spark_at(sparks, t)).x,
                                ypos: (*spark_at(sparks, t)).y,
                            };
                            let heat_a = (*spark_at(sparks, t)).heat;
                            draw_flash_ramp(&mut g, st, mips0, ll, heat_a, heat_a)?;
                        } else if g.speed_rand()? >= (*spark_at(sparks, t)).byte_d {
                            // Initiate new flash. `1 + SpeedRand() & 5` is `(1 + SpeedRand()) & 5` in C.
                            let r = g.speed_rand()?;
                            (*spark_at(sparks, t)).byte_c = ((1 + r as i32) & 5) as u8;
                        }
                    }
                }

                SPARK_RAMP_LIGHTNING => {
                    if (*spark_at(sparks, t)).heat != 0 {
                        if (*spark_at(sparks, t)).byte_c > 0 {
                            (*spark_at(sparks, t)).byte_c =
                                (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                            let ll = FireLineSeg {
                                xlen: (*spark_at(sparks, t)).byte_a,
                                ylen: (*spark_at(sparks, t)).byte_b,
                                xpos: (*spark_at(sparks, t)).x,
                                ypos: (*spark_at(sparks, t)).y,
                            };
                            let heat_a = (*spark_at(sparks, t)).heat;
                            let heat_b = heat_a >> 3;
                            draw_flash_ramp(&mut g, st, mips0, ll, heat_a, heat_b)?;
                        } else if g.speed_rand()? >= (*spark_at(sparks, t)).byte_d {
                            // Same `1 + SpeedRand() & 5` precedence as LineLightning.
                            let r = g.speed_rand()?;
                            (*spark_at(sparks, t)).byte_c = ((1 + r as i32) & 5) as u8;
                        }
                    }
                }

                SPARK_SPHERE_LIGHTNING => {
                    if g.speed_rand()? >= (*spark_at(sparks, t)).byte_d {
                        let spark_angle = g.speed_rand()? as i32;
                        let radius = (*spark_at(sparks, t)).byte_c as i32;
                        let col1 = (*spark_at(sparks, t)).heat;
                        let col2 = (*spark_at(sparks, t)).heat >> 2;
                        let sin_x = g.phase(spark_angle as usize) as i32;
                        let sin_y = g.phase(((spark_angle + 64) & 255) as usize) as i32;
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
                            xpos: (*spark_at(sparks, t)).x,
                            ypos: (*spark_at(sparks, t)).y,
                            xlen: xlen as u8,
                            ylen: ylen as u8,
                        };
                        draw_flash_ramp(&mut g, st, mips0, ll, col1, col2)?;
                    }
                }

                ISPARK_DRIFTER => {
                    (*spark_at(sparks, t)).heat = (*spark_at(sparks, t)).heat.wrapping_sub(5);
                    if (*spark_at(sparks, t)).heat < 251 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        move_spark(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_DRIFT_SLOW => {
                    (*spark_at(sparks, t)).heat = (*spark_at(sparks, t))
                        .heat
                        .wrapping_sub((*spark_at(sparks, t)).byte_d);
                    if (*spark_at(sparks, t)).heat > (*spark_at(sparks, t)).byte_d {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        move_spark(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_CUSTOM => {
                    (*spark_at(sparks, t)).heat = (*spark_at(sparks, t))
                        .heat
                        .wrapping_sub((*spark_at(sparks, t)).byte_c);
                    if (*spark_at(sparks, t)).heat < 250 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        move_spark(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_FALLER => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                    if (*spark_at(sparks, t)).byte_c > 0 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        move_spark(&mut g, st, sparks, t)?;
                        if ((*spark_at(sparks, t)).byte_b as i8) < 122 {
                            (*spark_at(sparks, t)).byte_b =
                                (*spark_at(sparks, t)).byte_b.wrapping_add(3);
                        }
                    } else {
                        removed = true;
                    }
                }

                ISPARK_GRAVITON => {
                    (*spark_at(sparks, t)).heat = (*spark_at(sparks, t))
                        .heat
                        .wrapping_sub((*spark_at(sparks, t)).byte_d);
                    if (*spark_at(sparks, t)).heat as i32 > LOWEST_GLOW {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        move_spark(&mut g, st, sparks, t)?;
                        if (*st).global_phase & 1 != 0
                            && ((*spark_at(sparks, t)).byte_b as i8) < 124
                        {
                            (*spark_at(sparks, t)).byte_b =
                                (*spark_at(sparks, t)).byte_b.wrapping_add(3);
                        }
                    } else {
                        removed = true;
                    }
                }

                ISPARK_VSHOOTER => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_sub(3);
                    if (*spark_at(sparks, t)).byte_c > 190 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).byte_c;
                        move_spark_two(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_DROP => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_add(4);
                    if (*spark_at(sparks, t)).byte_c < 250 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).byte_c;
                        move_spark_two(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_MOVE => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_add(4);
                    if (*spark_at(sparks, t)).byte_c < 250 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).byte_c;
                        move_spark(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_SPAWNED_EEL => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                    if (*spark_at(sparks, t)).byte_c < 255 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        move_spark(&mut g, st, sparks, t)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_SPAWNED_SPERM => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                    if (*spark_at(sparks, t)).byte_c < 255 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        // Jiggle direction = ByteD using a zigzag ByteA.
                        (*spark_at(sparks, t)).byte_a =
                            (*spark_at(sparks, t)).byte_a.wrapping_add(7);
                        let mut saw_tooth = 127 & (*spark_at(sparks, t)).byte_a;
                        if saw_tooth > 63 {
                            saw_tooth = 127 - saw_tooth;
                        }
                        // C: 255 & (SawTooth + ByteD), stored as BYTE; u8 wrapping add is the same.
                        let angle = saw_tooth.wrapping_add((*spark_at(sparks, t)).byte_d);
                        move_spark_angle(&mut g, st, sparks, t, angle)?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_SPAWNED_TWIRL => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                    if (*spark_at(sparks, t)).byte_c < 255 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        // Both speeds come from the table before ByteA is updated.
                        let temp_speed_x =
                            g.signed_phase((*spark_at(sparks, t)).byte_b.wrapping_add(64) as usize);
                        let temp_speed_y = g.signed_phase((*spark_at(sparks, t)).byte_b as usize);
                        // `*(WORD*)&ByteA += (WORD)ByteD << 4`. This assumes little-endian layout,
                        // with ByteA as the low byte and ByteB as the high byte.
                        let angle = u16::from_le_bytes([
                            (*spark_at(sparks, t)).byte_a,
                            (*spark_at(sparks, t)).byte_b,
                        ])
                        .wrapping_add(((*spark_at(sparks, t)).byte_d as u16) << 4);
                        let [lo, hi] = angle.to_le_bytes();
                        (*spark_at(sparks, t)).byte_a = lo;
                        (*spark_at(sparks, t)).byte_b = hi;
                        move_spark_xy(
                            &mut g,
                            st,
                            sparks,
                            t,
                            temp_speed_x as i8,
                            temp_speed_y as i8,
                        )?;
                    } else {
                        removed = true;
                    }
                }

                ISPARK_SPRINKLER_TWIRL => {
                    (*spark_at(sparks, t)).byte_c = (*spark_at(sparks, t)).byte_c.wrapping_sub(1);
                    if (*spark_at(sparks, t)).byte_c < 255 {
                        let dest = texel(
                            (*spark_at(sparks, t)).x as i32,
                            (*spark_at(sparks, t)).y as i32,
                            (*st).u_bits,
                        );
                        (*pixel_at(mips0, dest)) = (*spark_at(sparks, t)).heat;
                        // C: BYTE TempSpeedX = -128 + (SBYTE)PhaseTable[(ByteA+64)&255];
                        let phase = g.phase((*spark_at(sparks, t)).byte_a.wrapping_add(64) as usize)
                            as i8 as i32;
                        let temp_speed_x = ((-128i32 + phase) as u8) as i8;
                        let temp_speed_y = (*spark_at(sparks, t)).byte_b as i8;
                        (*spark_at(sparks, t)).byte_a = (*spark_at(sparks, t))
                            .byte_a
                            .wrapping_add((*spark_at(sparks, t)).byte_d);
                        move_spark_xy(&mut g, st, sparks, t, temp_speed_x, temp_speed_y)?;
                    } else {
                        removed = true;
                    }
                }

                _ => {}
            }

            if removed {
                // `*ThisSpark = Sparks(--ActiveSparkNum)`. The moved spark is not re-tested.
                (*st).active_spark_num -= 1;
                (*spark_at(sparks, s)) = (*spark_at(sparks, (*st).active_spark_num as usize));
            }
            s += 1;
        }
        Ok(())
    }
}
