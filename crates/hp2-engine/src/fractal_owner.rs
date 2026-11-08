//! Canonical ownership of Fire package fractal texture records and native behavior.
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

use hp2_abi::engine::generated::{
    FColor, FTextureInfo, TArrayRaw, UClass, UFireTexture, UFractalTexture, UIceTexture, UObject,
    UPalette, URenderDevice, UTexture, UWaterTexture, UWaveTexture, UWetTexture,
};
use hp2_core::{
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    cache_owner::CacheOwner,
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    object_archive::ObjectArchive,
    object_model,
    object_owner::ClassOps,
};
use hp2_fire::{
    abi::{FireArena, FireInitServices, FireSpark, FireTraceHooks},
    lifecycle::{self, FractalEntry, FractalLeaf, FractalServices},
};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const ARRAYS: Hp2ArrayServices = array_storage::system_allocator();

const FRACTAL_OPS: ClassOps = {
    let mut ops = crate::texture_owner::TEXTURE;
    ops.construct = construct_fractal;
    ops.post_load = post_load_fractal;
    ops
};
const FIRE_OPS: ClassOps = {
    let mut ops = FRACTAL_OPS;
    ops.construct = construct_fire;
    ops.finalize_members = finalize_fire;
    ops.serialize = serialize_fire;
    ops.post_load = post_load_fire;
    ops
};
const WATER_OPS: ClassOps = {
    let mut ops = FRACTAL_OPS;
    ops.construct = construct_water;
    ops.destroy = destroy_water;
    ops.post_load = post_load_water;
    ops
};
const WAVE_OPS: ClassOps = {
    let mut ops = WATER_OPS;
    ops.construct = construct_wave;
    ops.post_load = post_load_wave;
    ops
};
const WET_OPS: ClassOps = {
    let mut ops = WATER_OPS;
    ops.construct = construct_wet;
    ops.destroy = destroy_wet;
    ops.post_load = post_load_wet;
    ops
};
const ICE_OPS: ClassOps = {
    let mut ops = FRACTAL_OPS;
    ops.construct = construct_ice;
    ops.destroy = destroy_ice;
    ops.post_load = post_load_ice;
    ops
};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    spec::<UFractalTexture>("FractalTexture", "Engine.Texture", FRACTAL_OPS, 1),
    spec::<UFireTexture>("FireTexture", "Fire.FractalTexture", FIRE_OPS, 0),
    spec::<UWaterTexture>("WaterTexture", "Fire.FractalTexture", WATER_OPS, 1),
    spec::<UWaveTexture>("WaveTexture", "Fire.WaterTexture", WAVE_OPS, 0),
    spec::<UWetTexture>("WetTexture", "Fire.WaterTexture", WET_OPS, 0),
    spec::<UIceTexture>("IceTexture", "Fire.FractalTexture", ICE_OPS, 0),
];

const fn spec<T>(
    name: &'static str,
    superclass: &'static str,
    operations: ClassOps,
    flags: u32,
) -> NativeClassSpec {
    NativeClassSpec {
        short_name: name,
        package: "Fire",
        super_name: Some(superclass),
        within: "Object",
        flags,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<T>()),
        operations: Some(operations),
        properties: &[],
        defaults: &[],
    }
}

fn error(message: &'static str) -> RuntimeError {
    RuntimeError::new(message)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FractalClass {
    Fractal,
    Fire,
    Water,
    Wave,
    Wet,
    Ice,
}

fn class_of(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<Option<FractalClass>> {
    if object.is_null() {
        return Ok(None);
    }
    for (path, kind) in [
        ("Fire.FireTexture", FractalClass::Fire),
        ("Fire.WaveTexture", FractalClass::Wave),
        ("Fire.WetTexture", FractalClass::Wet),
        ("Fire.WaterTexture", FractalClass::Water),
        ("Fire.IceTexture", FractalClass::Ice),
        ("Fire.FractalTexture", FractalClass::Fractal),
    ] {
        let class = access.native_class(path)?.cast::<UClass>();
        if unsafe { object_model::is_a(object.cast(), class.cast()) } {
            return Ok(Some(kind));
        }
    }
    Ok(None)
}

unsafe fn construct_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
    entry: FractalEntry,
    a: i64,
    b: i64,
) -> RuntimeResult<()> {
    unsafe { (crate::texture_owner::TEXTURE.construct)(access, object)? };
    unsafe { run_entry(access, object, entry, a, b, 0) }
}
unsafe fn construct_fractal(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_base(access, object, FractalEntry::FractalCtor, 0, 0) }
}
unsafe fn construct_fire(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_fractal(access, object)? };
    unsafe { run_entry(access, object, FractalEntry::FireCtor, 0, 0, 0) }
}
unsafe fn construct_water(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_fractal(access, object)? };
    unsafe { run_entry(access, object, FractalEntry::WaterCtor, 0, 0, 0) }
}
unsafe fn construct_wave(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_water(access, object)? };
    unsafe { run_entry(access, object, FractalEntry::WaveCtor, 0, 0, 0) }
}
unsafe fn construct_wet(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_water(access, object)? };
    unsafe { run_entry(access, object, FractalEntry::WetCtor, 0, 0, 0) }
}
unsafe fn construct_ice(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_fractal(access, object)? };
    unsafe { run_entry(access, object, FractalEntry::IceCtor, 0, 0, 0) }
}

unsafe fn finalize_fire(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let fire = object.cast::<UFireTexture>();
    let sparks = unsafe { array_storage::release(array_slots(ptr::addr_of_mut!((*fire).sparks)), ARRAYS) }
        .map_err(|_| error("Fire spark storage release failed"));
    let texture = unsafe { (crate::texture_owner::TEXTURE.finalize_members)(access, object) };
    match (sparks, texture) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}
unsafe fn destroy_water(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::WaterDestroy, 0, 0, 0) }
}
unsafe fn destroy_wet(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::WetDestroy, 0, 0, 0) }
}
unsafe fn destroy_ice(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::IceDestroy, 0, 0, 0) }
}

unsafe fn post_load_fractal(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::FractalPostLoad, 0, 0, 0) }
}
unsafe fn post_load_fire(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::FirePostLoad, 0, 0, 0) }
}
unsafe fn post_load_water(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::WaterPostLoad, 0, 0, 0) }
}
unsafe fn post_load_wave(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::WavePostLoad, 0, 0, 0) }
}
unsafe fn post_load_wet(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::WetPostLoad, 0, 0, 0) }
}
unsafe fn post_load_ice(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { run_entry(access, object, FractalEntry::IcePostLoad, 0, 0, 0) }
}

unsafe fn serialize_fire(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    run_entry_archive(
        access,
        object,
        FractalEntry::FireSerialize,
        0,
        0,
        0,
        archive,
    )
}

/// Runs the shared hp2-fire lifecycle state machine with canonical Core leaves.
///
/// # Safety
/// `object` is a live canonical fractal record; entry arguments retain their native meaning.
pub(super) unsafe fn run_entry(
    access: CoreAccess<'_>,
    object: *mut UObject,
    entry: FractalEntry,
    a: i64,
    b: i64,
    c: i64,
) -> RuntimeResult<()> {
    run_entry_inner(
        access,
        object,
        entry,
        a,
        b,
        c,
        ptr::null(),
        ptr::null_mut(),
        None,
    )
}

unsafe fn run_entry_archive(
    access: CoreAccess<'_>,
    object: *mut UObject,
    entry: FractalEntry,
    a: i64,
    b: i64,
    c: i64,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    let archive = archive as *const ObjectArchive<'_> as *const ObjectArchive<'static>;
    run_entry_inner(
        access,
        object,
        entry,
        a,
        b,
        c,
        archive,
        ptr::null_mut(),
        None,
    )
}

unsafe fn run_entry_inner(
    access: CoreAccess<'_>,
    object: *mut UObject,
    entry: FractalEntry,
    a: i64,
    b: i64,
    c: i64,
    archive: *const ObjectArchive<'static>,
    spark: *mut FireSpark,
    lock: Option<LockArgs>,
) -> RuntimeResult<()> {
    if object.is_null()
        && !matches!(
            entry,
            FractalEntry::BlueLaguna | FractalEntry::SparkSerialize
        )
    {
        return Err(error("Null canonical Fire fractal object"));
    }
    let mut context = FractalContext {
        access: Some(access),
        object,
        archive,
        spark,
        lock: lock.unwrap_or_default(),
        source_info: unsafe { std::mem::zeroed() },
        ice: crate::fractal_water_owner::IceIo::default(),
        error: None,
    };
    let services = FractalServices { call: fractal_leaf };
    let status = unsafe {
        lifecycle::run(
            services,
            (&mut context as *mut FractalContext<'_>).cast(),
            object as usize,
            entry,
            a,
            b,
            c,
        )
    };
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    if status != 0 {
        return Err(error("Canonical Fire fractal lifecycle operation failed"));
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct LockArgs {
    info: *mut FTextureInfo,
    time: i64,
    device: *mut URenderDevice,
}

struct FractalContext<'a> {
    access: Option<CoreAccess<'a>>,
    object: *mut UObject,
    archive: *const ObjectArchive<'static>,
    spark: *mut FireSpark,
    lock: LockArgs,
    source_info: FTextureInfo,
    ice: crate::fractal_water_owner::IceIo,
    error: Option<RuntimeError>,
}

/// Optional Fire observer context, kept at its stack address until synchronous
/// callbacks return. No allocation is needed for a texture operation.
pub(super) struct TraceContext<'a> {
    state: FireTraceState<'a>,
}
struct FireTraceState<'a> {
    access: CoreAccess<'a>,
    error: Option<RuntimeError>,
    arenas: Option<FireTraceArenas>,
}
#[derive(Clone, Copy)]
struct FireTraceArenas {
    object: *mut UObject,
    state: *mut hp2_fire::sparks::FireSparkState,
    sparks: *mut FireArena<FireSpark>,
    mip: *mut FireArena<u8>,
}
impl<'a> TraceContext<'a> {
    pub(super) fn new(access: CoreAccess<'a>) -> Self {
        Self {
            state: FireTraceState {
                access,
                error: None,
                arenas: None,
            },
        }
    }
    pub(super) fn hooks(&mut self) -> FireTraceHooks {
        FireTraceHooks {
            context: ptr::from_mut(&mut self.state).cast(),
            init_tables: Some(trace_init_tables),
            speed_rand: Some(trace_speed_rand),
            burn_write: Some(trace_burn_write),
        }
    }
    pub(super) fn take_error(&mut self) -> Option<RuntimeError> {
        self.state.error.take()
    }
    fn bind_fire(
        &mut self,
        object: *mut UObject,
        state: *mut hp2_fire::sparks::FireSparkState,
        sparks: *mut FireArena<FireSpark>,
        mip: *mut FireArena<u8>,
    ) {
        self.state.arenas = Some(FireTraceArenas {
            object,
            state,
            sparks,
            mip,
        });
    }
}

extern "C" fn trace_init_tables(
    raw: *mut c_void,
    before: u64,
    after: u64,
    fingerprint: u64,
) -> i32 {
    trace_event(
        raw,
        crate::core_owner::FireTraceEvent::InitTables {
            before,
            after,
            fingerprint,
        },
    )
}
extern "C" fn trace_speed_rand(raw: *mut c_void, index: u32, value: u8) -> i32 {
    trace_event(
        raw,
        crate::core_owner::FireTraceEvent::SpeedRand { index, value },
    )
}
extern "C" fn trace_burn_write(
    raw: *mut c_void,
    texture: *mut c_void,
    spark: i32,
    offset: u32,
    before: u8,
    value: u8,
) -> i32 {
    trace_event(
        raw,
        crate::core_owner::FireTraceEvent::BurnWrite {
            texture,
            spark,
            offset,
            before,
            value,
        },
    )
}
fn trace_event(raw: *mut c_void, event: crate::core_owner::FireTraceEvent) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let state = raw.cast::<FireTraceState<'static>>();
    let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*state).access)) };
    let prepared = unsafe { prepare_fire_state(state) };
    let observed = match prepared {
        Err(error) => Err(error),
        Ok(()) => match access.owner_context::<crate::core_owner::EngineState>() {
            Some(owner) => owner
                .fire_trace_observer
                .get()
                .map_or(Ok(()), |observer| observer(access, event)),
            None => Err(error("Fire trace callback has no Engine owner context")),
        },
    };
    let refreshed = unsafe { refresh_fire_arenas(state) };
    let error = observed.err().or_else(|| refreshed.err());
    if let Some(error) = error {
        unsafe { ptr::addr_of_mut!((*state).error).write(Some(error)) };
        1
    } else {
        0
    }
}
unsafe fn prepare_fire_state(state: *mut FireTraceState<'_>) -> RuntimeResult<()> {
    if let Some(arenas) = (unsafe { ptr::read_unaligned(ptr::addr_of!((*state).arenas)) }) {
        unsafe {
            hp2_fire::sparks::store_spark_state_to_texture(arenas.object.cast(), &*arenas.state);
        }
    }
    Ok(())
}
unsafe fn refresh_fire_arenas(state: *mut FireTraceState<'_>) -> RuntimeResult<()> {
    let Some(arenas) = (unsafe { ptr::read_unaligned(ptr::addr_of!((*state).arenas)) }) else {
        return Ok(());
    };
    let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*state).access)) };
    if !arenas.mip.is_null() {
        let (data, len) = unsafe { mip_zero(access, arenas.object.cast())? };
        unsafe {
            ptr::addr_of_mut!((*arenas.mip).data).write(data);
            ptr::addr_of_mut!((*arenas.mip).len).write(len);
        }
    }
    let (data, len) = unsafe { spark_array(arenas.object)? };
    unsafe {
        ptr::addr_of_mut!((*arenas.sparks).data).write(data);
        ptr::addr_of_mut!((*arenas.sparks).len).write(len);
    }
    let refreshed_state =
        unsafe { hp2_fire::sparks::spark_state_from_texture(arenas.object.cast()) };
    unsafe { arenas.state.write_unaligned(refreshed_state) };
    Ok(())
}

unsafe extern "C" fn fractal_leaf(
    raw: *mut c_void,
    leaf: FractalLeaf,
    a: i64,
    b: i64,
    c: i64,
    result: *mut i64,
) -> i32 {
    if raw.is_null() || result.is_null() {
        return 1;
    }
    let context = raw.cast::<FractalContext<'static>>();
    match unsafe { dispatch_leaf(context, leaf, a, b, c) } {
        Ok(value) => {
            unsafe { result.write_unaligned(value) };
            0
        }
        Err(error) => {
            unsafe { ptr::addr_of_mut!((*context).error).write(Some(error)) };
            1
        }
    }
}

unsafe fn dispatch_leaf(
    context: *mut FractalContext<'_>,
    leaf: FractalLeaf,
    a: i64,
    b: i64,
    c: i64,
) -> RuntimeResult<i64> {
    use FractalLeaf as L;
    let object = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).object)) };
    let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
    if let Some(access) = access {
        if let Some(value) = unsafe {
            crate::fractal_water_owner::try_leaf(
                access,
                object,
                &mut (*context).ice,
                leaf,
                a,
                b,
                c,
            )?
        } {
            return Ok(value);
        }
    } else if matches!(leaf, L::ArchiveSparkByte) {
        return unsafe { archive_spark_byte(context, a as i32) };
    } else {
        return Err(error("Fire spark archive leaf has no Core access"));
    }
    let access = access.ok_or_else(|| error("Fire fractal leaf has no Core access"))?;
    match leaf {
        L::ClientPresent => {
            let state = access
                .owner_context::<crate::core_owner::EngineState>()
                .ok_or_else(|| error("Fire fractal client lookup has no Engine owner"))?;
            Ok(state
                .bitmap_client
                .get()
                .map_or(0, |client| client.as_ptr() as i64))
        }
        L::NoFractalAnim | L::SetNoFractalAnim => {
            let state = access
                .owner_context::<crate::core_owner::EngineState>()
                .ok_or_else(|| error("Fire fractal client flag has no Engine owner"))?;
            let client = state
                .bitmap_client
                .get()
                .ok_or_else(|| error("Fire fractal client is unavailable"))?;
            let client = client.as_ptr();
            if matches!(leaf, L::NoFractalAnim) {
                Ok(unsafe { (*client).no_fractal_anim() as i64 })
            } else {
                unsafe { (*client).set_no_fractal_anim(a != 0) };
                Ok(0)
            }
        }
        L::TextureInit => unsafe {
            crate::texture_owner::runtime_operation(
                access,
                object.cast(),
                crate::texture_runtime::TextureRuntimeOp::Init,
                a as usize,
                b as usize,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .map(|v| v as i64),
        L::TexturePostLoad => {
            unsafe { crate::texture_owner::post_load_base(access, object) }.map(|_| 0)
        }
        L::TexturePrime => unsafe {
            crate::texture_owner::runtime_operation(
                access,
                object.cast(),
                crate::texture_runtime::TextureRuntimeOp::Prime,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .map(|v| v as i64),
        L::TextureClear => unsafe { texture_clear(access, object, a as u32) }.map(|_| 0),
        L::TextureDestroy => unsafe { texture_destroy(access, object) }.map(|_| 0),
        L::TextureSerialize => {
            let archive = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).archive)) };
            let archive = unsafe { archive.as_ref() }
                .ok_or_else(|| error("Fire texture Serialize has no archive"))?;
            unsafe { crate::texture_owner::serialize_base(access, object, archive)? };
            Ok(0)
        }
        L::TextureTick => unsafe {
            crate::texture_owner::runtime_operation(
                access,
                object.cast(),
                crate::texture_runtime::TextureRuntimeOp::Tick,
                a as usize,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .map(|v| v as i64),
        L::VirtualConstantTick => {
            let class = class_of(access, object)?;
            let entry = match class {
                Some(FractalClass::Fire) => Some(FractalEntry::FireTick),
                Some(FractalClass::Wave) => Some(FractalEntry::WaveTick),
                Some(FractalClass::Wet) => Some(FractalEntry::WetTick),
                Some(FractalClass::Ice) => Some(FractalEntry::IceConstantTick),
                _ => None,
            };
            if let Some(entry) = entry {
                unsafe {
                    run_entry(access, object, entry, 0, 0, 0)?;
                }
                Ok(0)
            } else {
                unsafe {
                    crate::texture_owner::runtime_operation(
                        access,
                        object.cast(),
                        crate::texture_runtime::TextureRuntimeOp::ConstantTick,
                        0,
                        0,
                        0,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    )
                }
                .map(|v| v as i64)
            }
        }
        L::InitTables => init_tables(access).map(|_| 0),
        L::AssertCondition => {
            if a != 0 {
                Ok(0)
            } else {
                Err(error("Fire fractal source assertion failed"))
            }
        }
        L::NewPalette => new_palette(access, object, a),
        L::AppendColor => append_color(a as usize as *mut UPalette, b as u32).map(|_| 0),
        L::CopyColor => copy_color(object, a as usize as *mut UPalette, c as i32).map(|_| 0),
        L::ReplacePalette => replace_palette(access, object, a as usize as *mut UPalette)
            .map(|palette| palette as i64),
        L::SetMipZero => set_mip_zero(object).map(|_| 0),
        L::FlushCache => flush_cache(access).map(|_| 0),
        L::SetPaletteRgb => {
            set_palette_rgb(a as usize as *mut UPalette, b as i32, c as u32).map(|_| 0)
        }
        L::FillRenderTable => fill_render_table(object).map(|_| 0),
        L::SparkTypeAt => spark_type_at(object, a as i32).map(|ty| ty as i64),
        L::CopySpark => copy_spark(object, a as i32, b as i32).map(|_| 0),
        L::AddSparks => add_sparks(object, a as i32).map(|_| 0),
        L::RemoveSparks => remove_sparks(object, a as i32, b as i32).map(|_| 0),
        L::ArchiveSaving => {
            let archive = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).archive)) };
            Ok(unsafe { archive.as_ref() }
                .ok_or_else(|| error("Fire Serialize has no archive"))?
                .is_saving() as i64)
        }
        L::ArchiveSparks => archive_sparks(context),
        L::RedrawSparks => fire_kernel(
            context,
            |trace, state, sparks, mip| unsafe {
                hp2_fire::sparks::redraw_sparks(trace, state, sparks, mip)
            },
            true,
        ),
        L::WrapFire => diffuse_fire(access, object, true),
        L::SlowFire => diffuse_fire(access, object, false),
        L::PostDrawSparks => fire_kernel(
            context,
            |trace, state, sparks, mip| unsafe {
                hp2_fire::sparks::post_draw_sparks(trace, state, sparks, mip)
            },
            true,
        ),
        L::FirePaint => fire_kernel(
            context,
            |trace, state, sparks, _| unsafe {
                hp2_fire::sparks::fire_paint(trace, state, sparks, a as i32, b as i32, c as u32)
            },
            false,
        ),
        L::TempDrawSpark => temp_draw_spark(access, object, a as i32, b as i32, c as i32),
        L::WritePixel => write_pixel(access, object, a as i32, b as u8),
        L::LockSource => lock_source(context, a as u32, b as i32).map(|_| 0),
        L::UnlockSource => unlock_source(context, a as u32).map(|_| 0),
        L::MipPointer => {
            unsafe { mip_zero(access, (a as usize as *mut UObject).cast()) }.map(|(p, _)| p as i64)
        }
        L::MipLength => {
            unsafe { mip_zero(access, (a as usize as *mut UObject).cast()) }.map(|(_, n)| n as i64)
        }
        L::FractalInitBase => {
            unsafe { run_entry(access, object, FractalEntry::FractalInit, a, b, 0) }.map(|_| 0)
        }
        L::FractalPostLoadBase => {
            unsafe { run_entry(access, object, FractalEntry::FractalPostLoad, 0, 0, 0) }.map(|_| 0)
        }
        L::TextureLock => texture_lock(context),
        L::ArchiveSparkByte => archive_spark_byte(context, a as i32),
        // Every water/wave/wet/ice-specific leaf is owned by the delegated water module.
        L::WaveTablePointer
        | L::AllocateBytes
        | L::DeleteBytes
        | L::WaterPaint
        | L::AddDrop
        | L::DeleteDrops
        | L::WaterRedrawDrops
        | L::CalculateWater
        | L::SetWaveLight
        | L::SetRefractionTable
        | L::WetResample
        | L::IceResample
        | L::ApplyWet
        | L::MoveIcePosition
        | L::BlitIceTex
        | L::BlitTexIce
        | L::WaterInitBase
        | L::WaterPostLoadBase
        | L::WaterClearBase
        | L::WaterDestroyBase
        | L::SetIceRead
        | L::SetIceDestination
        | L::IceBlitTexKernel
        | L::IceBlitIceKernel
        | L::ApplyWetTexture
        | L::RenderIce => Err(error("Water fractal owner declined its native leaf")),
    }
}

fn init_tables(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let mut context = InitContext {
        access,
        trace: TraceContext::new(access),
    };
    let trace = context.trace.hooks();
    let init = FireInitServices {
        context: (&mut context as *mut InitContext<'_>).cast(),
        trace,
        ordinal: Some(init_ordinal),
        scoped: Some(init_scoped),
        sine: Some(init_sine),
        random: Some(init_random),
        publish: None,
    };
    let result = hp2_fire::fire::initialize(init);
    if let Some(error) = context.trace.take_error() {
        return Err(error);
    }
    result.map_err(|_| error("Fire table initialization failed"))
}
struct InitContext<'a> {
    access: CoreAccess<'a>,
    trace: TraceContext<'a>,
}
extern "C" fn init_ordinal(raw: *mut c_void, out: *mut u64) -> i32 {
    if raw.is_null() || out.is_null() {
        return 1;
    }
    let ordinal =
        hp2_core::ansi_trace::with_process_state(|state| hp2_core::ansi_trace::get_ordinal(state));
    unsafe { out.write_unaligned(ordinal) };
    0
}
extern "C" fn init_sine(_: *mut c_void, angle: f32, out: *mut f64) -> i32 {
    if out.is_null() {
        return 1;
    }
    unsafe { out.write_unaligned(hp2_core::ansi_math::sin(angle as f64)) };
    0
}
extern "C" fn init_random(raw: *mut c_void, out: *mut i32) -> i32 {
    if raw.is_null() || out.is_null() {
        return 1;
    }
    let access =
        unsafe { ptr::read_unaligned(ptr::addr_of!((*raw.cast::<InitContext<'static>>()).access)) };
    unsafe { out.write_unaligned(access.rand()) };
    0
}
// Rust consumers read the canonical hp2-fire tables directly; there is no
// separate C++ table copy to publish in the Rust-owned graph.
extern "C" fn init_scoped(raw: *mut c_void, before: u64) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let handle = hp2_core::ansi_trace::with_process_state(|state| {
        hp2_core::ansi_trace::phase_scope_begin(state, Some(b"fire_init_tables"))
    });
    let trace = unsafe { ptr::addr_of_mut!((*raw.cast::<InitContext<'static>>()).trace) };
    let hooks = unsafe { (&mut *trace).hooks() };
    let services = FireInitServices {
        context: raw,
        trace: hooks,
        ordinal: Some(init_ordinal),
        scoped: None,
        sine: Some(init_sine),
        random: Some(init_random),
        publish: None,
    };
    let result = hp2_fire::fire::initialize_scoped(services, before);
    hp2_core::ansi_trace::with_process_state(|state| unsafe {
        hp2_core::ansi_trace::phase_scope_end(state, handle);
    });
    i32::from(result.is_err())
}

unsafe fn array_slots(raw: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*raw).data),
            num: ptr::addr_of_mut!((*raw).num),
            max: ptr::addr_of_mut!((*raw).max),
        }
    }
}
fn validate_array(array: TArrayRaw, message: &'static str) -> RuntimeResult<()> {
    if array.num < 0 || array.max < array.num || (array.num > 0 && array.data.is_null()) {
        Err(error(message))
    } else {
        Ok(())
    }
}
unsafe fn spark_array(object: *mut UObject) -> RuntimeResult<(*mut FireSpark, usize)> {
    let raw = unsafe { ptr::addr_of!((*object.cast::<UFireTexture>()).sparks).read_unaligned() };
    validate_array(raw, "Invalid Fire spark array")?;
    Ok((raw.data.cast(), raw.num as usize))
}
unsafe fn fire_kernel(
    context: *mut FractalContext<'_>,
    kernel: impl FnOnce(
        FireTraceHooks,
        *mut hp2_fire::sparks::FireSparkState,
        *const FireArena<FireSpark>,
        *const FireArena<u8>,
    ) -> Result<(), hp2_fire::abi::FireCallbackError>,
    needs_mip: bool,
) -> RuntimeResult<i64> {
    let access = unsafe { context_access(context)? };
    let object = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).object)) };
    let (mip, mip_len) = if needs_mip {
        unsafe { mip_zero(access, object.cast())? }
    } else {
        (ptr::null_mut(), 0)
    };
    let (sparks, spark_count) = unsafe { spark_array(object)? };
    let mut spark_arena = FireArena {
        data: sparks,
        len: spark_count,
    };
    let mut mip_arena = FireArena {
        data: mip,
        len: mip_len,
    };
    let mut state = unsafe { hp2_fire::sparks::spark_state_from_texture(object.cast()) };
    let mut trace = TraceContext::new(access);
    trace.bind_fire(
        object,
        &mut state,
        &mut spark_arena,
        if needs_mip {
            &mut mip_arena
        } else {
            ptr::null_mut()
        },
    );
    let result = kernel(trace.hooks(), &mut state, &spark_arena, &mip_arena);
    if let Some(error) = trace.take_error() {
        return Err(error);
    }
    if result.is_err() {
        return Err(error("Fire spark kernel failed"));
    }
    unsafe { hp2_fire::sparks::store_spark_state_to_texture(object.cast(), &state) };
    Ok(0)
}

unsafe fn diffuse_fire(
    access: CoreAccess<'_>,
    object: *mut UObject,
    wrap: bool,
) -> RuntimeResult<i64> {
    let (mip, mip_len) = unsafe { mip_zero(access, object.cast())? };
    let (u, v) = unsafe {
        let texture = object.cast::<UTexture>();
        ((*texture).base.u_size, (*texture).base.v_size)
    };
    let width = usize::try_from(u).map_err(|_| error("Negative Fire texture width"))?;
    let height = usize::try_from(v).map_err(|_| error("Negative Fire texture height"))?;
    let required = width
        .checked_mul(height)
        .ok_or_else(|| error("Fire texture dimensions overflow"))?;
    if width < 2 || height < 2 || required > mip_len || (!wrap && width > 256) {
        return Err(error(
            "Fire mip does not contain supported declared dimensions",
        ));
    }
    let table = unsafe { object.cast::<u8>().add(336) };
    let bitmap = unsafe { std::slice::from_raw_parts_mut(mip, mip_len) };
    let render = unsafe { std::slice::from_raw_parts(table, 1024) };
    if wrap {
        hp2_fire::fire::calc_wrap_fire(bitmap, render, width as u32, height as u32);
    } else {
        hp2_fire::fire::calc_slow_fire(bitmap, render, width as u32, height as u32);
    }
    Ok(0)
}

unsafe fn fill_render_table(object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(error("Null Fire texture render table"));
    }
    let heat = unsafe { (*object.cast::<UFireTexture>()).render_heat };
    let render = unsafe { std::slice::from_raw_parts_mut(object.cast::<u8>().add(336), 1024) };
    hp2_fire::fire::fill_render_table(heat, render);
    Ok(())
}

unsafe fn temp_draw_spark(
    access: CoreAccess<'_>,
    object: *mut UObject,
    x: i32,
    y: i32,
    intensity: i32,
) -> RuntimeResult<i64> {
    let (mip, len) = unsafe { mip_zero(access, object.cast())? };
    let fractal = object.cast::<UFractalTexture>();
    let u_mask = unsafe { (*fractal).u_mask };
    let v_mask = unsafe { (*fractal).v_mask };
    let bits = unsafe { (*object.cast::<UTexture>()).base.u_bits };
    let index =
        (u_mask & x).wrapping_add((v_mask & y).wrapping_shl(u32::from(bits))) as u32 as usize;
    if index >= len {
        return Err(error("Fire temporary spark pixel is outside mip storage"));
    }
    let bitmap = unsafe { std::slice::from_raw_parts_mut(mip, len) };
    hp2_fire::water::temp_draw_spark(
        FireTraceHooks::NOOP,
        bitmap,
        u_mask,
        v_mask,
        bits,
        x,
        y,
        intensity,
    );
    Ok(0)
}

unsafe fn write_pixel(
    access: CoreAccess<'_>,
    object: *mut UObject,
    index: i32,
    value: u8,
) -> RuntimeResult<i64> {
    let (mip, len) = unsafe { mip_zero(access, object.cast())? };
    let index = usize::try_from(index).map_err(|_| error("Negative Fire mip pixel index"))?;
    if index >= len {
        return Err(error("Fire mip pixel index is outside storage"));
    }
    unsafe { mip.add(index).write(value) };
    Ok(0)
}

fn palette_slots(palette: *mut UPalette) -> RuntimeResult<Hp2ArraySlots> {
    if palette.is_null() {
        return Err(error("Null canonical palette"));
    }
    Ok(unsafe { array_slots(ptr::addr_of_mut!((*palette).colors)) })
}
unsafe fn palette_color(palette: *mut UPalette, index: i32) -> RuntimeResult<*mut FColor> {
    let raw = unsafe { ptr::read_unaligned(ptr::addr_of!((*palette).colors)) };
    validate_array(raw, "Invalid canonical palette color array")?;
    if index < 0 || index >= raw.num {
        return Err(error("Palette color index is outside its array"));
    }
    Ok(unsafe {
        raw.data
            .cast::<u8>()
            .add(index as usize * size_of::<FColor>())
            .cast()
    })
}
fn new_palette(
    access: CoreAccess<'_>,
    object: *mut UObject,
    name_owner: i64,
) -> RuntimeResult<i64> {
    if !matches!(name_owner, 0 | 1) {
        return Err(error("Invalid Fire palette constructor mode"));
    }
    let (outer, name) = unsafe {
        let name = if name_owner == 0 {
            0
        } else {
            i32::from_ne_bytes((*object).name)
        };
        ((*object).outer, name)
    };
    let palette = unsafe {
        access.construct_object(
            access.native_class("Engine.Palette")?,
            outer.cast(),
            name,
            0,
            ptr::null_mut(),
        )?
    };
    Ok(palette as i64)
}
fn append_color(palette: *mut UPalette, rgb: u32) -> RuntimeResult<()> {
    let slots = palette_slots(palette)?;
    let index =
        unsafe { array_storage::add(slots, 1, size_of::<FColor>() as i32, ptr::null(), ARRAYS) }
            .map_err(|_| error("Fire palette color allocation failed"))?;
    let data = unsafe { slots.data.read_unaligned() };
    if data.is_null() {
        return Err(error("Fire palette allocation returned null"));
    }
    unsafe {
        data.cast::<u8>()
            .add(index as usize * size_of::<FColor>())
            .cast::<FColor>()
            .write_unaligned(FColor {
                r: rgb as u8,
                g: (rgb >> 8) as u8,
                b: (rgb >> 16) as u8,
                a: 255,
            });
    }
    Ok(())
}
unsafe fn copy_color(
    object: *mut UObject,
    destination: *mut UPalette,
    index: i32,
) -> RuntimeResult<()> {
    let source =
        unsafe { ptr::read_unaligned(ptr::addr_of!((*object.cast::<UTexture>()).base.palette)) }
            .cast::<UPalette>();
    let color = unsafe { palette_color(source, index)? }.read_unaligned();
    let slots = palette_slots(destination)?;
    let target =
        unsafe { array_storage::add(slots, 1, size_of::<FColor>() as i32, ptr::null(), ARRAYS) }
            .map_err(|_| error("Fire palette color copy allocation failed"))?;
    let data = unsafe { slots.data.read_unaligned() };
    if data.is_null() {
        return Err(error("Fire palette allocation returned null"));
    }
    unsafe {
        data.cast::<u8>()
            .add(target as usize * size_of::<FColor>())
            .cast::<FColor>()
            .write_unaligned(color);
    }
    Ok(())
}
unsafe fn set_palette_rgb(palette: *mut UPalette, index: i32, rgb: u32) -> RuntimeResult<()> {
    let color = unsafe { palette_color(palette, index)? };
    unsafe {
        (*color).r = rgb as u8;
        (*color).g = (rgb >> 8) as u8;
        (*color).b = (rgb >> 16) as u8;
    }
    Ok(())
}
unsafe fn set_mip_zero(object: *mut UObject) -> RuntimeResult<()> {
    let texture = object.cast::<UTexture>();
    let palette =
        unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).base.palette)) }.cast::<UPalette>();
    let color = unsafe { palette_color(palette, 128)? };
    let value = unsafe { color.cast::<u8>().read_unaligned() };
    unsafe {
        ptr::addr_of_mut!((*texture).base.mip_zero).write_unaligned([
            value,
            color.cast::<u8>().add(1).read_unaligned(),
            color.cast::<u8>().add(2).read_unaligned(),
            color.cast::<u8>().add(3).read_unaligned(),
        ]);
    }
    Ok(())
}

struct PaletteReplaceContext<'a> {
    access: CoreAccess<'a>,
    candidate: *mut UPalette,
    cursor: i32,
    result: *mut UPalette,
    error: Option<RuntimeError>,
}
fn replace_palette(
    access: CoreAccess<'_>,
    _texture: *mut UObject,
    candidate: *mut UPalette,
) -> RuntimeResult<*mut UPalette> {
    if candidate.is_null() {
        return Err(error("Null Fire replacement palette"));
    }
    let mut context = PaletteReplaceContext {
        access,
        candidate,
        cursor: 0,
        result: candidate,
        error: None,
    };
    let services = crate::texture_runtime::TextureRuntimeServices {
        call: palette_replace_leaf,
    };
    let args = crate::texture_runtime::TextureRuntimeArgs {
        native: (&mut context as *mut PaletteReplaceContext<'_>).cast(),
        a: 0,
        b: 0,
        time: 0,
    };
    let result = crate::texture_runtime::run(
        &services,
        args,
        crate::texture_runtime::TextureRuntimeOp::PaletteReplace,
    );
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    result
        .map(|value| value as *mut UPalette)
        .map_err(|_| error("Fire palette replacement policy failed"))
}
unsafe extern "C" fn palette_replace_leaf(
    raw: *mut c_void,
    leaf: crate::texture_runtime::TextureRuntimeLeaf,
    a: usize,
    b: usize,
    out: *mut usize,
) -> i32 {
    if raw.is_null() || out.is_null() {
        return 1;
    }
    let context = raw.cast::<PaletteReplaceContext<'static>>();
    let result = unsafe { palette_replace_dispatch(context, leaf, a, b) };
    match result {
        Ok(value) => {
            unsafe { out.write_unaligned(value) };
            0
        }
        Err(error) => {
            unsafe { ptr::addr_of_mut!((*context).error).write(Some(error)) };
            1
        }
    }
}
unsafe fn palette_replace_dispatch(
    context: *mut PaletteReplaceContext<'_>,
    leaf: crate::texture_runtime::TextureRuntimeLeaf,
    a: usize,
    b: usize,
) -> RuntimeResult<usize> {
    use crate::texture_runtime::{TextureRuntimeField as F, TextureRuntimeLeaf as L};
    let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
    let candidate = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).candidate)) };
    match leaf {
        L::Read if a == F::Palette as usize => Ok(candidate as usize),
        L::PaletteOuter => {
            let palette = if a == 0 {
                candidate
            } else {
                a as *mut UPalette
            };
            if palette.is_null() {
                return Err(error("Null Fire palette outer lookup"));
            }
            Ok(unsafe {
                ptr::read_unaligned(ptr::addr_of!((*palette.cast::<UObject>()).outer)) as usize
            })
        }
        L::PaletteIterator | L::PaletteAdvance => {
            if matches!(leaf, L::PaletteIterator) {
                unsafe { ptr::addr_of_mut!((*context).cursor).write(0) };
            }
            let palette_class = access.native_class("Engine.Palette")?;
            loop {
                let cursor = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).cursor)) };
                if cursor >= access.object_count() {
                    return Ok(0);
                }
                unsafe { ptr::addr_of_mut!((*context).cursor).write(cursor + 1) };
                let object = access.object_at(cursor);
                if !object.is_null()
                    && unsafe { object_model::is_a(object.cast(), palette_class.cast()) }
                {
                    return Ok(object as usize);
                }
            }
        }
        L::PaletteView => {
            let palette = if a == 0 {
                candidate
            } else {
                a as *mut UPalette
            };
            if palette.is_null() {
                return Err(error("Fire palette view is null"));
            }
            let colors = unsafe { ptr::read_unaligned(ptr::addr_of!((*palette).colors)) };
            validate_array(colors, "Invalid Fire palette colors")?;
            if colors.num < 256 {
                return Err(error("Fire palette has fewer than 256 colors"));
            }
            unsafe {
                (b as *mut crate::texture_runtime::TextureRuntimeData).write_unaligned(
                    crate::texture_runtime::TextureRuntimeData {
                        data: colors.data.cast(),
                        len: colors.num,
                        u: 0,
                        v: 0,
                    },
                );
            }
            Ok(0)
        }
        L::PaletteLogDelete => {
            log_palette_replacement(access, candidate, a as *mut UPalette)?;
            unsafe {
                access.delete_object(candidate.cast())?;
            }
            unsafe { ptr::addr_of_mut!((*context).result).write(a as *mut UPalette) };
            Ok(a)
        }
        L::FactoryResult => {
            Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*context).result)) } as usize)
        }
        _ => Err(error("Unexpected leaf in Fire palette replacement")),
    }
}
fn log_palette_replacement(
    access: CoreAccess<'_>,
    old: *mut UPalette,
    new: *mut UPalette,
) -> RuntimeResult<()> {
    use hp2_core::name_owner::FindName;
    use hp2_core::native_text::{self, TextPart};
    let old_name = palette_name(access, old.cast())?;
    let new_name = palette_name(access, new.cast())?;
    let category = access.intern_str("Log", FindName::Add)?;
    if access.name_suppressed(category) {
        return Ok(());
    }
    let mut message = unsafe {
        native_text::fmt(&[
            TextPart::L("Replaced palette "),
            TextPart::P(old_name.as_ptr()),
            TextPart::L(" with "),
            TextPart::P(new_name.as_ptr()),
        ])
    };
    if message.last() == Some(&0) {
        message.pop();
    }
    access.emit_log(category, &message)
}
fn palette_name(access: CoreAccess<'_>, palette: *mut UObject) -> RuntimeResult<Vec<u32>> {
    if palette.is_null() {
        return Err(error("Null palette name source"));
    }
    let index = unsafe { i32::from_ne_bytes((*palette).name) };
    let mut name = access.name_text(index)?;
    name.push(0);
    Ok(name)
}

struct CacheLease<'a> {
    slot: &'a std::cell::RefCell<Option<CacheOwner>>,
    cache: Option<CacheOwner>,
}
impl<'a> CacheLease<'a> {
    fn acquire(slot: &'a std::cell::RefCell<Option<CacheOwner>>) -> RuntimeResult<Self> {
        let cache = slot
            .try_borrow_mut()
            .map_err(|_| error("Engine cache flush reentered"))?
            .take()
            .ok_or_else(|| error("Engine cache is already in use"))?;
        Ok(Self {
            slot,
            cache: Some(cache),
        })
    }
}
impl Drop for CacheLease<'_> {
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.cache.take();
    }
}
fn flush_cache(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let state = access
        .owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| error("Fire cache flush has no Engine owner"))?;
    let mut lease = CacheLease::acquire(&state.cache)?;
    lease
        .cache
        .as_mut()
        .expect("Cache lease owns its cache")
        .flush(access, 0, u32::MAX, 0)
}

unsafe fn fill_spark_type(object: *mut UObject, index: i32) -> RuntimeResult<*mut FireSpark> {
    let (data, count) = unsafe { spark_array(object)? };
    if index < 0 || index as usize >= count {
        return Err(error("Fire spark index is outside its array"));
    }
    Ok(unsafe { data.add(index as usize) })
}
fn spark_type_at(object: *mut UObject, index: i32) -> RuntimeResult<u8> {
    Ok(unsafe { (*fill_spark_type(object, index)?).ty })
}
fn copy_spark(object: *mut UObject, destination: i32, source: i32) -> RuntimeResult<()> {
    let src = unsafe { fill_spark_type(object, source)?.read_unaligned() };
    unsafe {
        fill_spark_type(object, destination)?.write_unaligned(src);
    }
    Ok(())
}
fn add_sparks(object: *mut UObject, count: i32) -> RuntimeResult<()> {
    if count < 0 {
        return Err(error("Negative Fire spark count"));
    }
    let array = unsafe { ptr::addr_of_mut!((*object.cast::<UFireTexture>()).sparks) };
    unsafe {
        array_storage::add(
            array_slots(array),
            count,
            size_of::<FireSpark>() as i32,
            ptr::null(),
            ARRAYS,
        )
    }
    .map_err(|_| error("Fire spark allocation failed"))?;
    Ok(())
}
fn remove_sparks(object: *mut UObject, index: i32, count: i32) -> RuntimeResult<()> {
    let array = unsafe { ptr::addr_of_mut!((*object.cast::<UFireTexture>()).sparks) };
    let raw = unsafe { ptr::read_unaligned(array) };
    validate_array(raw, "Invalid Fire spark array")?;
    if index < 0 || count < 0 || index.checked_add(count).is_none_or(|end| end > raw.num) {
        return Err(error("Fire spark removal is outside its array"));
    }
    unsafe {
        array_storage::remove(
            array_slots(array),
            index,
            count,
            size_of::<FireSpark>() as i32,
            ptr::null(),
            ARRAYS,
        )
    }
    .map_err(|_| error("Fire spark removal failed"))
}

unsafe fn archive_sparks(context: *mut FractalContext<'_>) -> RuntimeResult<i64> {
    let archive = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).archive)) };
    let archive = unsafe { archive.as_ref() }
        .ok_or_else(|| error("Fire Sparks serialization has no archive"))?;
    let object = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).object)) };
    let array = unsafe { ptr::addr_of_mut!((*object.cast::<UFireTexture>()).sparks) };
    unsafe {
        crate::animation_owner::serialize_array(
            archive,
            array,
            size_of::<FireSpark>(),
            Some(serialize_spark),
            None,
            false,
        )?;
    }
    Ok(0)
}

unsafe fn archive_spark_byte(context: *mut FractalContext<'_>, index: i32) -> RuntimeResult<i64> {
    if !(0..8).contains(&index) {
        return Err(error("Invalid Fire spark archive byte index"));
    }
    let spark = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).spark)) };
    if spark.is_null() {
        return Err(error("Fire spark archive has no current element"));
    }
    let archive = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).archive)) };
    let archive =
        unsafe { archive.as_ref() }.ok_or_else(|| error("Fire spark archive has no archive"))?;
    unsafe {
        archive.serialize_u8(&mut *spark.cast::<u8>().add(index as usize))?;
    }
    Ok(0)
}

unsafe fn lock_source(
    context: *mut FractalContext<'_>,
    selector: u32,
    mode: i32,
) -> RuntimeResult<()> {
    let source = source_texture(context, selector)?;
    if source.is_null() {
        return Err(error("Fractal source texture is null during Lock"));
    }
    let access = unsafe { context_access(context)? };
    let owner = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).object)) }.cast::<UTexture>();
    let time = match mode {
        0 => 0,
        1 => i64::from_ne_bytes(unsafe {
            ptr::read_unaligned(ptr::addr_of!((*owner).base.__last_update_time))
        }),
        2 => unsafe { ptr::read_unaligned(ptr::addr_of!((*context).lock.time)) },
        _ => return Err(error("Invalid fractal source lock time mode")),
    };
    let info = unsafe { ptr::addr_of_mut!((*context).source_info) };
    unsafe { crate::texture_owner::lock(access, source, info, time, 0, ptr::null_mut()) }
}
unsafe fn unlock_source(context: *mut FractalContext<'_>, selector: u32) -> RuntimeResult<()> {
    let source = source_texture(context, selector)?;
    if source.is_null() {
        return Err(error("Fractal source texture is null during Unlock"));
    }
    let access = unsafe { context_access(context)? };
    let info = unsafe { ptr::addr_of_mut!((*context).source_info) };
    unsafe { crate::texture_owner::unlock(access, source, info) }
}
unsafe fn source_texture(
    context: *mut FractalContext<'_>,
    selector: u32,
) -> RuntimeResult<*mut UTexture> {
    let object = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).object)) };
    let pointer = match selector {
        0 => unsafe {
            ptr::read_unaligned(ptr::addr_of!(
                (*object.cast::<UWetTexture>()).source_texture
            ))
        },
        1 => unsafe {
            ptr::read_unaligned(ptr::addr_of!(
                (*object.cast::<UIceTexture>()).source_texture
            ))
        },
        2 => unsafe {
            ptr::read_unaligned(ptr::addr_of!((*object.cast::<UIceTexture>()).glass_texture))
        },
        _ => return Err(error("Invalid fractal source selector")),
    };
    Ok(pointer.cast())
}
unsafe fn texture_lock(context: *mut FractalContext<'_>) -> RuntimeResult<i64> {
    let access = unsafe { context_access(context)? };
    let object = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).object)) };
    let lock = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).lock)) };
    if lock.info.is_null() {
        return Err(error("Fractal lock has no caller FTextureInfo"));
    }
    unsafe {
        crate::texture_owner::lock_base(
            access,
            object.cast(),
            lock.info,
            lock.time,
            0,
            lock.device,
        )?;
    }
    Ok(0)
}

unsafe fn context_access<'a>(context: *mut FractalContext<'a>) -> RuntimeResult<CoreAccess<'a>> {
    unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) }
        .ok_or_else(|| error("Fire fractal operation has no canonical Core access"))
}

/// Returns one owner-keyed allocation retained by hp2-fire's lifecycle state.
pub(super) fn water_pointer(
    access: CoreAccess<'_>,
    object: *mut UObject,
    slot: usize,
) -> RuntimeResult<*mut u8> {
    if object.is_null() || slot > 1 {
        return Err(error("Invalid canonical water allocation lookup"));
    }
    let mut context = FractalContext {
        access: Some(access),
        object,
        archive: ptr::null(),
        spark: ptr::null_mut(),
        lock: LockArgs::default(),
        source_info: unsafe { std::mem::zeroed() },
        ice: crate::fractal_water_owner::IceIo::default(),
        error: None,
    };
    let pointer = unsafe {
        lifecycle::water_pointer(
            FractalServices { call: fractal_leaf },
            ptr::from_mut(&mut context).cast(),
            object as usize,
            slot,
        )
    }
    .map_err(|_| error("Canonical water allocation is unavailable"))?;
    Ok(pointer as *mut u8)
}

/// Allocates source `new BYTE[Size]` storage from the Core system heap.
pub(super) fn allocate_bytes(size: i64) -> RuntimeResult<*mut u8> {
    let size =
        usize::try_from(size).map_err(|_| error("Invalid canonical Fire allocation size"))?;
    Ok(unsafe { hp2_core::unix_alloc::system_malloc(size) }.cast())
}

/// Releases bytes allocated by [`allocate_bytes`] through the same system heap.
pub(super) unsafe fn delete_bytes(pointer: *mut u8) -> RuntimeResult<()> {
    unsafe { libc::free(pointer.cast()) };
    Ok(())
}

/// Canonical mip-zero bytes, lazily loading the Core archive on demand.
///
/// # Safety
/// `texture` is a live canonical UTexture.
pub(super) unsafe fn mip_zero(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
) -> RuntimeResult<(*mut u8, usize)> {
    unsafe { crate::texture_owner::mip_zero(access, texture) }
}

/// Executes native UTexture::Clear without redispatching the derived override.
pub(super) unsafe fn texture_clear(
    access: CoreAccess<'_>,
    object: *mut UObject,
    flags: u32,
) -> RuntimeResult<()> {
    unsafe {
        crate::texture_owner::runtime_operation(
            access,
            object.cast(),
            crate::texture_runtime::TextureRuntimeOp::Clear,
            flags as usize,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    }
    .map(|_| ())
}

/// Executes native UTexture::Destroy, bypassing the derived texture operation.
pub(super) unsafe fn texture_destroy(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe { crate::texture_owner::destroy_base(access, object) }
}

/// Virtual `UTexture::Lock` dispatch for source textures with native overrides.
pub(crate) unsafe fn lock_virtual(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    info: *mut FTextureInfo,
    time: i64,
    lod: i32,
    device: *mut URenderDevice,
) -> RuntimeResult<bool> {
    let object = texture.cast::<UObject>();
    let entry = match class_of(access, object)? {
        Some(FractalClass::Wet) => FractalEntry::WetLock,
        Some(FractalClass::Ice) => FractalEntry::IceLock,
        _ => return Ok(false),
    };
    if info.is_null() {
        return Err(error("Fractal texture Lock has no FTextureInfo"));
    }
    unsafe { info.write_unaligned(std::mem::zeroed()) };
    run_entry_inner(
        access,
        object,
        entry,
        lod as i64,
        0,
        0,
        ptr::null(),
        ptr::null_mut(),
        Some(LockArgs { info, time, device }),
    )?;
    Ok(true)
}

/// Dynamic Init/Prime/Tick/constant-tick target used by Texture's nested virtual leaves.
pub(crate) unsafe fn virtual_call(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    operation: crate::texture_runtime::TextureRuntimeOp,
    a: usize,
    b: usize,
    time: i64,
    info: *mut FTextureInfo,
    device: *mut URenderDevice,
) -> RuntimeResult<Option<usize>> {
    use crate::texture_runtime::TextureRuntimeOp as O;
    let object = texture.cast::<UObject>();
    let class = class_of(access, object)?;
    let entry = match (operation, class) {
        (O::Init, Some(FractalClass::Fractal)) => Some(FractalEntry::FractalInit),
        (O::Init, Some(FractalClass::Fire)) => Some(FractalEntry::FireInit),
        (O::Init, Some(FractalClass::Water)) => Some(FractalEntry::WaterInit),
        (O::Init, Some(FractalClass::Wave)) => Some(FractalEntry::WaveInit),
        (O::Init, Some(FractalClass::Wet)) => Some(FractalEntry::WetInit),
        (O::Init, Some(FractalClass::Ice)) => Some(FractalEntry::IceInit),
        (O::Prime, Some(_)) => Some(FractalEntry::Prime),
        (O::ConstantTick, Some(FractalClass::Fire)) => Some(FractalEntry::FireTick),
        (O::ConstantTick, Some(FractalClass::Wave)) => Some(FractalEntry::WaveTick),
        (O::ConstantTick, Some(FractalClass::Wet)) => Some(FractalEntry::WetTick),
        (O::ConstantTick, Some(FractalClass::Ice)) => Some(FractalEntry::IceConstantTick),
        (O::Tick, Some(FractalClass::Ice)) => Some(FractalEntry::IceTick),
        _ => None,
    };
    if let Some(entry) = entry {
        let lock = if matches!(operation, O::Lock) {
            Some(LockArgs { info, time, device })
        } else {
            None
        };
        run_entry_inner(
            access,
            object,
            entry,
            a as i64,
            b as i64,
            0,
            ptr::null(),
            ptr::null_mut(),
            lock,
        )?;
        Ok(Some(0))
    } else {
        Ok(None)
    }
}
/// One real engine operation in SDLLaunch's Fire texture boot probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireTextureProbeStep {
    Lock,
    Update,
    Tick,
    ConstantTimeTick,
    Unlock,
}

/// Runs one source Fire texture probe operation through its canonical owner.
///
/// `info` must point to the live stack `FTextureInfo` used by the surrounding
/// Lock/Unlock pair. `time` is the source `Update(1.0)` timestamp.
///
/// # Safety
/// `texture` must be a live canonical Fire texture and `info` must remain live
/// through the call.
pub unsafe fn fire_texture_probe_step(
    access: CoreAccess<'_>,
    texture: *mut UFireTexture,
    info: *mut FTextureInfo,
    step: FireTextureProbeStep,
) -> RuntimeResult<bool> {
    if texture.is_null() {
        return Err(error("Fire texture probe received a null texture"));
    }
    let object = texture.cast::<UObject>();
    match step {
        FireTextureProbeStep::Lock => {
            if info.is_null() {
                return Err(error("Fire texture probe Lock has no FTextureInfo"));
            }
            unsafe {
                crate::texture_owner::lock(
                    access,
                    texture.cast(),
                    info,
                    0,
                    0,
                    ptr::null_mut(),
                )?;
                Ok((*info).b_realtime_changed())
            }
        }
        FireTextureProbeStep::Update => unsafe {
            crate::texture_owner::runtime_operation(
                access,
                texture.cast(),
                crate::texture_runtime::TextureRuntimeOp::Update,
                0,
                0,
                hp2_core::time::from_double(1.0),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .map(|_| false),
        FireTextureProbeStep::Tick => unsafe {
            crate::texture_owner::runtime_operation(
                access,
                texture.cast(),
                crate::texture_runtime::TextureRuntimeOp::Tick,
                1.0f32.to_bits() as usize,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .map(|_| false),
        FireTextureProbeStep::ConstantTimeTick => unsafe {
            run_entry(access, object, FractalEntry::FireTick, 0, 0, 0)
        }
        .map(|_| false),
        FireTextureProbeStep::Unlock => {
            if info.is_null() {
                return Err(error("Fire texture probe Unlock has no FTextureInfo"));
            }
            unsafe { crate::texture_owner::unlock(access, texture.cast(), info)?; }
            Ok(false)
        }
    }
}

/// Serializes one FSpark through the same eight-byte shared lifecycle leaf sequence.
unsafe fn serialize_spark(archive: &ObjectArchive<'_>, spark: *mut c_void) -> RuntimeResult<()> {
    let archive = archive as *const ObjectArchive<'_> as *const ObjectArchive<'static>;
    let mut context = FractalContext {
        access: None,
        object: ptr::null_mut(),
        archive,
        spark: spark.cast(),
        lock: LockArgs::default(),
        source_info: unsafe { std::mem::zeroed() },
        ice: crate::fractal_water_owner::IceIo::default(),
        error: None,
    };
    let status = unsafe {
        lifecycle::run(
            FractalServices { call: fractal_leaf },
            ptr::from_mut(&mut context).cast(),
            0,
            FractalEntry::SparkSerialize,
            0,
            0,
            0,
        )
    };
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    if status != 0 {
        return Err(error("Fire spark archive lifecycle failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp2_core::{
        core_access::{CoreOptions, CoreProcessMode},
        name_owner::FindName,
    };
    use std::{fs, path::PathBuf};

    fn reject_water_random(
        access: CoreAccess<'_>,
        event: crate::core_owner::FireTraceEvent,
    ) -> RuntimeResult<()> {
        if matches!(event, crate::core_owner::FireTraceEvent::SpeedRand { .. }) {
            let class = access.native_class("Fire.WaveTexture")?;
            let name = access.intern_str("ObservedWave", FindName::Find)?;
            let object =
                unsafe { access.find_object(class, access.transient_package(), name, true) };
            assert!(!object.is_null());
            let water = object.cast::<UWaterTexture>();
            unsafe {
                ptr::addr_of_mut!((*water).wave_amp).write_unaligned(37);
                ptr::addr_of_mut!((*water).fx_frequency).write_unaligned(80);
            }
            return Err(error("Water random observer rejected this event"));
        }
        Ok(())
    }

    unsafe fn new_texture(
        access: CoreAccess<'_>,
        class: &str,
        name: &str,
        init: Option<FractalEntry>,
        edge: i64,
    ) -> *mut UObject {
        let class = access.native_class(class).unwrap();
        let name = access.intern_str(name, FindName::Add).unwrap();
        let object = unsafe {
            access.construct_object(
                class,
                access.transient_package(),
                name,
                hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_post_load,
                ptr::null_mut(),
            )
        }
        .unwrap();
        if let Some(init) = init {
            unsafe { run_entry(access, object, init, edge, edge, 0) }.unwrap();
        } else {
            unsafe {
                crate::texture_owner::runtime_operation(
                    access,
                    object.cast(),
                    crate::texture_runtime::TextureRuntimeOp::Init,
                    edge as usize,
                    edge as usize,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            }
            .unwrap();
        }
        object
    }

    #[test]
    fn canonical_water_reentry_wet_resample_and_ice_pixels() {
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("fractal-native-{}", std::process::id()));
        fs::create_dir_all(root.join("System")).unwrap();
        let prototype =
            PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype metadata"));
        for name in ["Core.u", "Engine.u", "Fire.u", "Default.ini", "DefUser.ini"] {
            fs::copy(
                prototype.join("System").join(name),
                root.join("System").join(name),
            )
            .unwrap();
        }
        let owner = crate::EngineOwner::new(CoreOptions {
            data_root: root.clone(),
            system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"),
            command_line: Vec::new(),
            output: |_, _, _| Ok(()),
            log_file: None,
        })
        .unwrap();
        let access = owner.access();
        access.set_process_mode(CoreProcessMode {
            is_client: true,
            is_server: true,
            is_scriptable: true,
            ..access.process_mode()
        });
        for name in ["Engine.u", "Core.u", "Fire.u"] {
            unsafe { access.load_package(&root.join("System").join(name), 1) }.unwrap();
        }

        let wave = unsafe {
            new_texture(
                access,
                "Fire.WaveTexture",
                "ObservedWave",
                Some(FractalEntry::WaveInit),
                16,
            )
        };
        unsafe { access.conditional_post_load(wave) }.unwrap();
        let fields = water_pointer(access, wave, 0).unwrap();
        assert!(
            !fields.is_null(),
            "Wave PostLoad must run its Water base allocation"
        );
        assert!(
            unsafe { std::slice::from_raw_parts(fields, 128) }
                .iter()
                .all(|&value| value == 128)
        );
        unsafe {
            ptr::write_bytes(fields, 17, 128);
            ptr::addr_of_mut!((*wave.cast::<UWaterTexture>()).num_drops).write_unaligned(5);
            run_entry(access, wave, FractalEntry::WaveClear, 3, 0, 0).unwrap();
        }
        assert!(
            unsafe { std::slice::from_raw_parts(fields, 128) }
                .iter()
                .all(|&value| value == 128)
        );
        let drops =
            unsafe { ptr::addr_of!((*wave.cast::<UWaterTexture>()).num_drops).read_unaligned() };
        assert_eq!(drops, 0);

        // The drippy-tap branch increments NumDrops before SpeedRand. A real
        // observer edit/error must survive the native kernel's partial update.
        unsafe {
            ptr::addr_of_mut!((*wave.cast::<UWaterTexture>()).drop_type).write_unaligned(19);
        }
        owner
            .publish_fire_trace_observer(reject_water_random)
            .unwrap();
        let rejected = unsafe {
            run_entry(
                access,
                wave,
                FractalEntry::WaterClick,
                1,
                8.0f32.to_bits() as i64,
                8.0f32.to_bits() as i64,
            )
        };
        assert!(rejected.is_err());
        owner.clear_fire_trace_observer().unwrap();
        let state = unsafe { hp2_fire::water::water_state_from_texture(wave.cast()) };
        let wave_amp =
            unsafe { ptr::addr_of!((*wave.cast::<UWaterTexture>()).wave_amp).read_unaligned() };
        assert_eq!((state.num_drops, wave_amp, state.fx_frequency), (1, 37, 80));

        let source = unsafe { new_texture(access, "Engine.Texture", "PixelSource", None, 16) };
        let glass = unsafe { new_texture(access, "Engine.Texture", "NeutralGlass", None, 16) };
        unsafe {
            access.conditional_post_load(source).unwrap();
            access.conditional_post_load(glass).unwrap();
        }
        let expected: Vec<u8> = (0..256).map(|index| (index % 251) as u8).collect();
        let (source_bytes, source_len) = unsafe { mip_zero(access, source.cast()) }.unwrap();
        let (glass_bytes, glass_len) = unsafe { mip_zero(access, glass.cast()) }.unwrap();
        assert_eq!((source_len, glass_len), (256, 256));
        unsafe {
            ptr::copy_nonoverlapping(expected.as_ptr(), source_bytes, expected.len());
            ptr::write_bytes(glass_bytes, 128, glass_len);
        }

        let wet = unsafe {
            new_texture(
                access,
                "Fire.WetTexture",
                "ResampledWet",
                Some(FractalEntry::WetInit),
                32,
            )
        };
        unsafe {
            ptr::addr_of_mut!((*wet.cast::<UWetTexture>()).source_texture)
                .write_unaligned(source.cast());
            access.conditional_post_load(wet).unwrap();
        }
        let local = water_pointer(access, wet, 1).unwrap();
        assert!(!local.is_null());
        let resampled = unsafe { std::slice::from_raw_parts(local, 1024) };
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(resampled[y * 32 + x], expected[(y / 2) * 16 + x / 2]);
            }
        }

        let ice = unsafe {
            new_texture(
                access,
                "Fire.IceTexture",
                "IdentityIce",
                Some(FractalEntry::IceInit),
                16,
            )
        };
        unsafe {
            ptr::addr_of_mut!((*ice.cast::<UIceTexture>()).source_texture)
                .write_unaligned(source.cast());
            ptr::addr_of_mut!((*ice.cast::<UIceTexture>()).glass_texture)
                .write_unaligned(glass.cast());
            access.conditional_post_load(ice).unwrap();
            run_entry(access, ice, FractalEntry::IceBlitTex, 0, 0, 0).unwrap();
        }
        let (pixels, len) = unsafe { mip_zero(access, ice.cast()) }.unwrap();
        assert_eq!(unsafe { std::slice::from_raw_parts(pixels, len) }, expected);
        owner.shutdown().unwrap();
        fs::write(root.join("report.txt"),
            "data_profile=data-prototype\nwater-base-allocation-and-clear=ok\nwater-reentry-error-state=ok\nwet-source-resample=ok\nice-cpu-pixels=ok\nshutdown=ok\n").unwrap();
    }
    #[test]
    fn canonical_parametric_wave_mip_survives_lazy_reload_generation_and_lock() {
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("fractal-parametric-mip-{}", std::process::id()));
        fs::create_dir_all(root.join("System")).unwrap();
        let prototype =
            PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype metadata"));
        for name in ["Core.u", "Engine.u", "Fire.u", "Default.ini", "DefUser.ini"] {
            fs::copy(
                prototype.join("System").join(name),
                root.join("System").join(name),
            )
            .unwrap();
        }
        let owner = crate::EngineOwner::new(CoreOptions {
            data_root: root.clone(),
            system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"),
            command_line: Vec::new(),
            output: |_, _, _| Ok(()),
            log_file: None,
        })
        .unwrap();
        let access = owner.access();
        access.set_process_mode(CoreProcessMode {
            is_client: true,
            ..CoreProcessMode::default()
        });
        for name in ["Engine.u", "Core.u", "Fire.u"] {
            unsafe { access.load_package(&root.join("System").join(name), 1) }.unwrap();
        }

        let package = unsafe {
            access.construct_object(
                access.native_class("Package").unwrap(),
                ptr::null_mut(),
                access.intern_str("ParametricWavePackage", FindName::Add).unwrap(),
                4,
                ptr::null_mut(),
            )
        }
        .unwrap();
        let wave = unsafe {
            access.construct_object(
                access.native_class("Fire.WaveTexture").unwrap(),
                package,
                access.intern_str("GeneratedWave", FindName::Add).unwrap(),
                4 | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_post_load,
                ptr::null_mut(),
            )
        }
        .unwrap();
        unsafe { run_entry(access, wave, FractalEntry::WaveInit, 16, 16, 0) }.unwrap();
        unsafe { access.conditional_post_load(wave) }.unwrap();
        assert!(unsafe { (*wave.cast::<UTexture>()).b_parametric() });
        unsafe {
            access.add_root(package);
            access.add_root(wave);
        }
        let saved_path = root.join("System/ParametricWavePackage.u");
        assert!(
            unsafe { access.save_package(package, wave, 4, &saved_path, None) }.unwrap(),
            "save the initialized parametric WaveTexture export"
        );
        access.remove_root(wave);
        access.remove_root(package);
        owner.shutdown().unwrap();

        let restored_owner = crate::EngineOwner::new(CoreOptions {
            data_root: root.clone(),
            system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"),
            command_line: Vec::new(),
            output: |_, _, _| Ok(()),
            log_file: None,
        })
        .unwrap();
        let restored = restored_owner.access();
        restored.set_process_mode(CoreProcessMode {
            is_client: true,
            is_server: true,
            is_scriptable: true,
            lazy_load: true,
            ..CoreProcessMode::default()
        });
        for name in ["Engine.u", "Core.u", "Fire.u"] {
            unsafe { restored.load_package(&root.join("System").join(name), 1) }.unwrap();
        }
        unsafe { restored.load_package(&saved_path, 1) }.unwrap();
        let wave_path = ["ParametricWavePackage", "GeneratedWave"].join(".");
        let wave_path: Vec<u32> = wave_path
            .chars()
            .map(u32::from)
            .chain([0])
            .collect();
        let wave = unsafe {
            restored.static_load_object(
                restored.native_class("Fire.WaveTexture").unwrap(),
                ptr::null_mut(),
                &wave_path,
                1,
            )
        }
        .unwrap();
        unsafe { restored.conditional_post_load(wave) }.unwrap();
        assert!(unsafe { (*wave.cast::<UTexture>()).b_parametric() });
        let mips = unsafe { ptr::read_unaligned(ptr::addr_of!((*wave.cast::<UTexture>()).mips)) };
        assert_eq!(mips.num, 1);
        let mip = mips.data.cast::<hp2_abi::engine::generated::FMipmap>();
        let saved_pos = unsafe {
            ptr::read_unaligned(ptr::addr_of!((*mip).data_array.saved_pos))
        };
        assert!(saved_pos > 0, "saved parametric mip body remains deferred");

        let (bitmap, bitmap_len) = unsafe { mip_zero(restored, wave.cast()) }.unwrap();
        assert_eq!(bitmap_len, 256, "parametric mip retains its USize*VSize storage");
        assert!(
            unsafe { std::slice::from_raw_parts(bitmap, bitmap_len) }
                .iter()
                .all(|&pixel| pixel == 0),
            "reload recreates the native zeroed procedural mip"
        );
        let fields = water_pointer(restored, wave, 0).unwrap();
        unsafe { ptr::write_bytes(fields, 128, 128) };
        unsafe {
            ptr::addr_of_mut!((*wave.cast::<UWaterTexture>()).wave_parity).write_unaligned(1);
            std::slice::from_raw_parts_mut(wave.cast::<u8>().add(2356), 1028).fill(77);
            assert_eq!(
                crate::fractal_water_owner::try_leaf(
                    restored,
                    wave,
                    &mut crate::fractal_water_owner::IceIo::default(),
                    FractalLeaf::CalculateWater,
                    0,
                    0,
                    0,
                )
                .unwrap(),
                Some(0)
            );
        }
        assert_eq!(
            unsafe { std::slice::from_raw_parts(bitmap, bitmap_len)[0] },
            77,
            "CalculateWater writes generated pixels into the parametric mip"
        );

        let mut info: FTextureInfo = unsafe { std::mem::zeroed() };
        unsafe {
            crate::texture_owner::runtime_operation(
                restored,
                wave.cast(),
                crate::texture_runtime::TextureRuntimeOp::Lock,
                0,
                0,
                0,
                ptr::addr_of_mut!(info),
                ptr::null_mut(),
            )
        }
        .unwrap();
        let published_mip = info.mips[0].cast::<hp2_abi::engine::generated::FMipmap>();
        assert_eq!(published_mip, mip, "Lock publishes the canonical mip record");
        let published_bytes = unsafe { (*published_mip).base.data_ptr.cast::<u8>() };
        assert_eq!(published_bytes, bitmap, "Lock publishes the generated parametric bytes");
        assert_eq!(
            unsafe { std::slice::from_raw_parts(published_bytes, bitmap_len)[0] },
            77,
            "InfoMipPublish preserves the generated pixel"
        );
        restored_owner.shutdown().unwrap();
    }
}
