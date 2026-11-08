//! Canonical implementations of the Water/Wave/Wet/Ice native leaves.
use std::ffi::c_void;
use std::ptr;

use hp2_abi::engine::generated::{UIceTexture, UObject, UTexture, UWetTexture};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_fire::abi::{FireArena, FireDrop, FireTraceHooks};
use hp2_fire::ice::{FireIceState, IceRead};
use hp2_fire::lifecycle::{FractalEntry, FractalLeaf};
use hp2_fire::water::FireWaterState;

use crate::fractal_owner::{self, TraceContext};

const DROPS_OFFSET: usize = 304;
const RENDER_TABLE_OFFSET: usize = 2356;
const WAVE_TABLE_OFFSET: usize = 3384;

#[derive(Default)]
pub(super) struct IceIo {
    destination: *mut u8,
    destination_len: usize,
    reads: [(*const u8, usize); 2],
}

fn error(message: &'static str) -> RuntimeError {
    RuntimeError::new(message)
}
fn fire_error(_: hp2_fire::abi::FireCallbackError, where_: &'static str) -> RuntimeError {
    RuntimeError::new(where_)
}
fn trace_result<T>(trace: &mut TraceContext<'_>, result: RuntimeResult<T>) -> RuntimeResult<T> {
    match trace.take_error() {
        Some(error) => Err(error),
        None => result,
    }
}

fn allocate(size: i64) -> RuntimeResult<usize> {
    Ok(unsafe { fractal_owner::allocate_bytes(size)? } as usize)
}

fn free(address: usize) -> RuntimeResult<()> {
    unsafe { fractal_owner::delete_bytes(address as *mut u8) }
}

unsafe fn bytes_mut<'a>(
    pointer: *mut u8,
    length: usize,
    what: &'static str,
) -> RuntimeResult<&'a mut [u8]> {
    if length != 0 && pointer.is_null() {
        return Err(error(what));
    }
    let pointer = if pointer.is_null() {
        ptr::NonNull::<u8>::dangling().as_ptr()
    } else {
        pointer
    };
    Ok(unsafe { std::slice::from_raw_parts_mut(pointer, length) })
}
unsafe fn bytes<'a>(
    pointer: *const u8,
    length: usize,
    what: &'static str,
) -> RuntimeResult<&'a [u8]> {
    if length != 0 && pointer.is_null() {
        return Err(error(what));
    }
    let pointer = if pointer.is_null() {
        ptr::NonNull::<u8>::dangling().as_ptr()
    } else {
        pointer as *mut u8
    };
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}

unsafe fn array_at<T>(object: *mut UObject, offset: usize) -> *mut T {
    unsafe { object.cast::<u8>().add(offset).cast() }
}

fn dimensions(object: *mut UObject) -> RuntimeResult<(usize, usize)> {
    let texture = object.cast::<UTexture>();
    let (width, height) = unsafe { ((*texture).base.u_size, (*texture).base.v_size) };
    if width < 0 || height < 0 {
        return Err(error("Negative Fire texture dimensions"));
    }
    let bytes = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| error("Fire texture dimensions overflow"))?;
    Ok((bytes, width as usize))
}

fn water_field(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<(*mut u8, usize)> {
    let pointer = unsafe { fractal_owner::water_pointer(access, object, 0)? };
    if pointer.is_null() {
        return Err(error("Water source fields are not allocated"));
    }
    Ok((pointer, dimensions(object)?.0 / 2))
}

struct WaterTrace<'a> {
    access: CoreAccess<'a>,
    object: *mut UObject,
    state: *mut FireWaterState,
    drops: *mut FireArena<FireDrop>,
    water: *mut FireArena<u8>,
    base: FireTraceHooks,
}

fn refresh_water(raw: *mut c_void, push_state: bool) -> bool {
    let ctx = unsafe { &mut *raw.cast::<WaterTrace<'_>>() };
    unsafe {
        if push_state {
            hp2_fire::water::store_water_state_to_texture(ctx.object.cast(), &*ctx.state);
        } else {
            *ctx.state = hp2_fire::water::water_state_from_texture(ctx.object.cast());
        }
        (*ctx.drops).data = array_at::<FireDrop>(ctx.object, DROPS_OFFSET);
        (*ctx.drops).len = 256;
        if !ctx.water.is_null() {
            let Ok((field, length)) = water_field(ctx.access, ctx.object) else {
                return false;
            };
            (*ctx.water).data = field;
            (*ctx.water).len = length;
        }
    }
    true
}
extern "C" fn water_init_tables(
    raw: *mut c_void,
    before: u64,
    after: u64,
    fingerprint: u64,
) -> i32 {
    if !refresh_water(raw, true) {
        return 1;
    }
    let (callback, context) = unsafe {
        let ctx = &*raw.cast::<WaterTrace<'_>>();
        (ctx.base.init_tables, ctx.base.context)
    };
    let status = callback.map_or(1, |f| f(context, before, after, fingerprint));
    if !refresh_water(raw, false) {
        return 1;
    }
    status
}
extern "C" fn water_speed_rand(raw: *mut c_void, index: u32, value: u8) -> i32 {
    if !refresh_water(raw, true) {
        return 1;
    }
    let (callback, context) = unsafe {
        let ctx = &*raw.cast::<WaterTrace<'_>>();
        (ctx.base.speed_rand, ctx.base.context)
    };
    let status = callback.map_or(1, |f| f(context, index, value));
    if !refresh_water(raw, false) {
        return 1;
    }
    status
}
extern "C" fn water_burn_write(
    raw: *mut c_void,
    texture: *mut c_void,
    spark: i32,
    offset: u32,
    before: u8,
    value: u8,
) -> i32 {
    if !refresh_water(raw, true) {
        return 1;
    }
    let (callback, context) = unsafe {
        let ctx = &*raw.cast::<WaterTrace<'_>>();
        (ctx.base.burn_write, ctx.base.context)
    };
    let status = callback.map_or(1, |f| f(context, texture, spark, offset, before, value));
    if !refresh_water(raw, false) {
        return 1;
    }
    status
}

unsafe fn run_water_kernel<T>(
    access: CoreAccess<'_>,
    object: *mut UObject,
    with_water: bool,
    with_bitmap: bool,
    body: impl FnOnce(
        FireTraceHooks,
        *mut FireWaterState,
        *mut FireArena<FireDrop>,
        *mut FireArena<u8>,
        *mut u8,
        usize,
    ) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    if object.is_null() {
        return Err(error("Null Water texture"));
    }
    let mut state = unsafe { hp2_fire::water::water_state_from_texture(object.cast()) };
    let mut drops = FireArena {
        data: unsafe { array_at(object, DROPS_OFFSET) },
        len: 256,
    };
    let mut water = FireArena {
        data: ptr::null_mut(),
        len: 0,
    };
    if with_water {
        let (field, length) = water_field(access, object)?;
        water = FireArena {
            data: field,
            len: length,
        };
    }
    let mut trace = TraceContext::new(access);
    let base = trace.hooks();
    let mut context = WaterTrace {
        access,
        object,
        state: &mut state,
        drops: &mut drops,
        water: if with_water {
            &mut water
        } else {
            ptr::null_mut()
        },
        base,
    };
    let hooks = FireTraceHooks {
        context: (&mut context as *mut WaterTrace<'_>).cast(),
        init_tables: Some(water_init_tables),
        speed_rand: Some(water_speed_rand),
        burn_write: Some(water_burn_write),
    };
    let (bitmap, bitmap_len) = if with_bitmap {
        unsafe { fractal_owner::mip_zero(access, object.cast())? }
    } else {
        (ptr::null_mut(), 0)
    };
    let result = body(
        hooks, &mut state, &mut drops, &mut water, bitmap, bitmap_len,
    );
    unsafe {
        hp2_fire::water::store_water_state_to_texture(object.cast(), &state);
    }
    let observer_error = trace.take_error();
    drop(trace);
    match observer_error {
        Some(error) => Err(error),
        None => result,
    }
}

unsafe fn ice_state(object: *mut UObject) -> FireIceState {
    unsafe { hp2_fire::ice::ice_state_from_texture(object.cast()) }
}

pub(super) unsafe fn try_leaf(
    access: CoreAccess<'_>,
    object: *mut UObject,
    ice: &mut IceIo,
    leaf: FractalLeaf,
    a: i64,
    b: i64,
    c: i64,
) -> RuntimeResult<Option<i64>> {
    use FractalLeaf::*;
    let answer = match leaf {
        WaveTablePointer => Some(unsafe { array_at::<u8>(object, WAVE_TABLE_OFFSET) as i64 }),
        AllocateBytes => Some(allocate(a)? as i64),
        DeleteBytes => {
            free(a as usize)?;
            Some(0)
        }
        WaterPaint => {
            unsafe {
                run_water_kernel(
                    access,
                    object,
                    false,
                    false,
                    |hooks, state, drops, _, _, _| {
                        hp2_fire::water::water_paint(
                            hooks, state, drops, a as i32, b as i32, c as u32,
                        )
                        .map_err(|e| fire_error(e, "WaterPaint kernel failed"))?;
                        Ok(0)
                    },
                )?
            };
            Some(0)
        }
        AddDrop => {
            unsafe {
                run_water_kernel(
                    access,
                    object,
                    false,
                    false,
                    |hooks, state, drops, _, _, _| {
                        hp2_fire::water::add_drop(hooks, state, drops, a as i32, b as i32)
                            .map_err(|e| fire_error(e, "AddDrop kernel failed"))?;
                        Ok(0)
                    },
                )?
            };
            Some(0)
        }
        DeleteDrops => {
            unsafe {
                run_water_kernel(
                    access,
                    object,
                    false,
                    false,
                    |hooks, state, drops, _, _, _| {
                        if (*drops).data.is_null() {
                            return Err(error("Water drops array is null"));
                        }
                        hp2_fire::water::delete_drops(
                            hooks,
                            &mut *state,
                            std::slice::from_raw_parts_mut((*drops).data, (*drops).len),
                            a as i32,
                            b as i32,
                            c as i32,
                        );
                        Ok(0)
                    },
                )?
            };
            Some(0)
        }
        WaterRedrawDrops => {
            unsafe {
                run_water_kernel(
                    access,
                    object,
                    true,
                    false,
                    |hooks, state, drops, water, _, _| {
                        hp2_fire::water::water_redraw_drops(hooks, state, drops, water)
                            .map_err(|e| fire_error(e, "WaterRedrawDrops kernel failed"))?;
                        Ok(0)
                    },
                )?
            };
            Some(0)
        }
        CalculateWater => {
            unsafe {
                run_water_kernel(
                    access,
                    object,
                    true,
                    true,
                    |hooks, state, _, water, bitmap, bitmap_len| {
                        let render = unsafe { array_at::<u8>(object, RENDER_TABLE_OFFSET) };
                        let wave = unsafe { array_at::<u8>(object, WAVE_TABLE_OFFSET) };
                        hp2_fire::water::calculate_water(
                            hooks,
                            &mut *state,
                            bytes_mut(bitmap, bitmap_len, "Water mip is null")?,
                            bytes_mut((*water).data, (*water).len, "Water field is null")?,
                            bytes(render, 1028, "Water render table is null")?,
                            bytes(wave, 1536, "Wave table is null")?,
                        );
                        Ok(0)
                    },
                )?
            };
            Some(0)
        }
        SetWaveLight => {
            let params = unsafe { hp2_fire::wave::wave_light_from_texture(object.cast()) };
            let table = unsafe { array_at::<u8>(object, RENDER_TABLE_OFFSET) };
            let mut trace = TraceContext::new(access);
            hp2_fire::wave::set_wave_light(trace.hooks(), &params, unsafe {
                bytes_mut(table, 1028, "Wave render table is null")?
            });
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        SetRefractionTable => {
            let amp = unsafe { hp2_fire::wet::wave_amp_from_texture(object.cast()) };
            let table = unsafe { array_at::<u8>(object, RENDER_TABLE_OFFSET) };
            let mut trace = TraceContext::new(access);
            hp2_fire::wet::set_refraction_table(trace.hooks(), amp, unsafe {
                bytes_mut(table, 1028, "Wet render table is null")?
            });
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        WetResample => {
            let (dst_len, _) = dimensions(object)?;
            let source = unsafe {
                (*object.cast::<UWetTexture>())
                    .source_texture
                    .cast::<UTexture>()
            };
            if source.is_null() {
                return Err(error("Wet source texture is null"));
            }
            let (src, src_len) = unsafe { fractal_owner::mip_zero(access, source)? };
            let params = unsafe {
                hp2_fire::wet::wet_resample_params_from_texture(object.cast(), b as i32, c as i32)
            };
            let mut trace = TraceContext::new(access);
            unsafe {
                hp2_fire::wet::resample_wet_source(
                    trace.hooks(),
                    FireArena {
                        data: a as usize as *mut u8,
                        len: dst_len,
                    },
                    FireArena {
                        data: src,
                        len: src_len,
                    },
                    &params,
                );
            }
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        IceResample => {
            let local = unsafe {
                (*object.cast::<UIceTexture>())
                    .local_source_bitmap
                    .cast::<u8>()
            };
            let len = unsafe { fractal_owner::mip_zero(access, object.cast())?.1 };
            let source = unsafe {
                (*object.cast::<UIceTexture>())
                    .source_texture
                    .cast::<UTexture>()
            };
            if source.is_null() {
                return Err(error("Ice source texture is null"));
            }
            let (src, src_len) = unsafe { fractal_owner::mip_zero(access, source)? };
            let params = unsafe {
                hp2_fire::ice::ice_resample_params_from_texture(object.cast(), a as i32, b as i32)
            };
            let mut trace = TraceContext::new(access);
            hp2_fire::ice::resample_source(
                trace.hooks(),
                &params,
                unsafe { bytes_mut(local, len, "Ice local source is null")? },
                IceRead::Separate(unsafe { bytes(src, src_len, "Ice source mip is null")? }),
            );
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        ApplyWet => {
            let source = a as usize as *const u8;
            let source_len = b as usize;
            let (bitmap, bitmap_len) = unsafe { fractal_owner::mip_zero(access, object.cast())? };
            let layout = unsafe { hp2_fire::wet::wet_layout_from_texture(object.cast()) };
            let mut trace = TraceContext::new(access);
            hp2_fire::wet::apply_wet_texture(
                trace.hooks(),
                FireArena {
                    data: bitmap,
                    len: bitmap_len,
                },
                FireArena {
                    data: source.cast_mut(),
                    len: source_len,
                },
                &layout,
            );
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        MoveIcePosition => {
            let mut state = unsafe { ice_state(object) };
            let mut trace = TraceContext::new(access);
            hp2_fire::ice::move_ice_position(trace.hooks(), &mut state, f32::from_bits(a as u32));
            unsafe {
                hp2_fire::ice::store_ice_state_to_texture(object.cast(), &state);
            }
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        BlitIceTex | BlitTexIce => {
            let state = unsafe { ice_state(object) };
            let (destination, length) = unsafe { fractal_owner::mip_zero(access, object.cast())? };
            let dest = unsafe { bytes_mut(destination, length, "Ice destination mip is null")? };
            let empty = [];
            if state.local_source_bitmap != 0 {
                let mut trace = TraceContext::new(access);
                if matches!(leaf, BlitIceTex) {
                    hp2_fire::ice::blit_ice_tex(
                        trace.hooks(),
                        &state,
                        dest,
                        IceRead::Separate(&empty),
                        IceRead::Separate(&empty),
                    );
                } else {
                    hp2_fire::ice::blit_tex_ice(
                        trace.hooks(),
                        &state,
                        dest,
                        IceRead::Separate(&empty),
                        IceRead::Separate(&empty),
                    );
                }
                trace_result(&mut trace, Ok(()))?;
            } else {
                let source = unsafe {
                    (*object.cast::<UIceTexture>())
                        .source_texture
                        .cast::<UTexture>()
                };
                let glass = unsafe {
                    (*object.cast::<UIceTexture>())
                        .glass_texture
                        .cast::<UTexture>()
                };
                if source.is_null() || glass.is_null() {
                    return Err(error("Ice blit source or glass texture is null"));
                }
                let (source_bytes, source_len) =
                    unsafe { fractal_owner::mip_zero(access, source)? };
                let (glass_bytes, glass_len) = unsafe { fractal_owner::mip_zero(access, glass)? };
                let tex_read = if source.cast::<UObject>() == object {
                    IceRead::Dest
                } else {
                    IceRead::Separate(unsafe {
                        bytes(source_bytes, source_len, "Ice source mip is null")?
                    })
                };
                let glass_read = if glass.cast::<UObject>() == object {
                    IceRead::Dest
                } else {
                    IceRead::Separate(unsafe {
                        bytes(glass_bytes, glass_len, "Ice glass mip is null")?
                    })
                };
                let mut trace = TraceContext::new(access);
                if matches!(leaf, BlitIceTex) {
                    hp2_fire::ice::blit_ice_tex(trace.hooks(), &state, dest, tex_read, glass_read);
                } else {
                    hp2_fire::ice::blit_tex_ice(trace.hooks(), &state, dest, tex_read, glass_read);
                }
                trace_result(&mut trace, Ok(()))?;
            }
            Some(0)
        }
        WaterInitBase => {
            unsafe {
                fractal_owner::run_entry(access, object, FractalEntry::WaterInit, a, b, 0)?;
            }
            Some(0)
        }
        WaterPostLoadBase => {
            unsafe {
                fractal_owner::run_entry(access, object, FractalEntry::WaterPostLoad, 0, 0, 0)?;
            }
            Some(0)
        }
        WaterClearBase => {
            unsafe {
                fractal_owner::run_entry(access, object, FractalEntry::WaterClear, a, 0, 0)?;
            }
            Some(0)
        }
        WaterDestroyBase => {
            unsafe {
                fractal_owner::run_entry(access, object, FractalEntry::WaterDestroy, 0, 0, 0)?;
            }
            Some(0)
        }
        ApplyWetTexture => {
            unsafe {
                fractal_owner::run_entry(access, object, FractalEntry::WetApply, 0, 0, 0)?;
            }
            Some(0)
        }
        RenderIce => {
            unsafe {
                fractal_owner::run_entry(access, object, FractalEntry::IceRender, a, b, c)?;
            }
            Some(0)
        }
        SetIceDestination => {
            let (pointer, length) = unsafe { fractal_owner::mip_zero(access, object.cast())? };
            ice.destination = pointer;
            ice.destination_len = length;
            Some(0)
        }
        SetIceRead => {
            let index = usize::try_from(a).map_err(|_| error("Invalid ice source slot"))?;
            if index >= ice.reads.len() {
                return Err(error("Invalid ice source slot"));
            }
            let length = usize::try_from(c).map_err(|_| error("Invalid ice source length"))?;
            if length != 0 && b == 0 {
                return Err(error("Null nonempty ice source span"));
            }
            ice.reads[index] = (b as usize as *const u8, length);
            Some(0)
        }
        IceBlitTexKernel | IceBlitIceKernel => {
            let state = unsafe { ice_state(object) };
            let [r0, r1] = ice.reads;
            if state.local_source_bitmap == 0
                && (ice.destination.is_null() || r0.0.is_null() || r1.0.is_null())
            {
                return Err(error("Ice blit destination or source span was not set"));
            }
            let empty = [];
            let first = if state.local_source_bitmap != 0 {
                IceRead::Separate(&empty)
            } else if r0.0 == ice.destination {
                IceRead::Dest
            } else {
                IceRead::Separate(unsafe { bytes(r0.0, r0.1, "Invalid ice read span")? })
            };
            let second = if state.local_source_bitmap != 0 {
                IceRead::Separate(&empty)
            } else if r1.0 == ice.destination {
                IceRead::Dest
            } else {
                IceRead::Separate(unsafe { bytes(r1.0, r1.1, "Invalid ice read span")? })
            };
            let destination = unsafe {
                bytes_mut(
                    ice.destination,
                    ice.destination_len,
                    "Invalid ice destination span",
                )?
            };
            let mut trace = TraceContext::new(access);
            if matches!(leaf, IceBlitTexKernel) {
                hp2_fire::ice::blit_tex_ice(trace.hooks(), &state, destination, first, second);
            } else {
                hp2_fire::ice::blit_ice_tex(trace.hooks(), &state, destination, first, second);
            }
            trace_result(&mut trace, Ok(()))?;
            Some(0)
        }
        _ => None,
    };
    Ok(answer)
}
