//! Fractal UObject lifecycle policy. Native callbacks are individual typed storage
//! or engine leaves; no native object borrow, mip borrow, or owner-map lock lives
//! across a callback. Every scalar read is live (including after virtual calls).
use std::ffi::c_void;
use std::sync::Mutex;

use hp2_abi::engine::generated::{
    UFireTexture, UFractalTexture, UIceTexture, UObject, UTexture, UWaterTexture, UWaveTexture,
    UWetTexture,
};

/// Dispatch key for [`Host::get`]/[`Host::set`]/[`Host::float`]/[`Host::set_float`]. Every
/// variant but `Client`/`NoFractalAnim` reads or writes a named member of an `hp2-abi` engine
/// mirror directly; `Client`/`NoFractalAnim` stay routed through [`FractalLeaf`] because
/// `UBitmap::__Client` is a C++ Engine-module static global, not reachable from any Fire
/// instance mirror offset. Internal to this crate: never crosses the FFI boundary itself (the
/// C++ side no longer needs a field selector; see [`FractalSourceField`] for the one leaf pair
/// that still does).
#[repr(u32)]
#[derive(Clone, Copy)]
enum FractalField {
    Parametric,
    Realtime,
    DrawPhase,
    AuxPhase,
    GlobalPhase,
    USize,
    VSize,
    UBits,
    VBits,
    UMask,
    VMask,
    PrimeCurrent,
    PrimeCount,
    Client,
    NoFractalAnim,
    Palette,
    Outer,
    PolyFlags,
    RenderHeat,
    OldRenderHeat,
    ActiveSparkNum,
    SparksLimit,
    SparksNum,
    SparkType,
    StarStatus,
    Rising,
    FireFrequency,
    FirePhase,
    FireHeat,
    FireSize,
    FireArea,
    FireHorizSpeed,
    FireVertSpeed,
    NumDrops,
    DropType,
    WaveAmp,
    OldWaveAmp,
    WaveParity,
    SourceFieldsLegacy,
    WaterFrequency,
    WaterPhase,
    WaterAmplitude,
    WaterSpeed,
    WaterRadius,
    WaterSize,
    WaterDepth,
    WaterTime,
    BumpMapAngle,
    BumpMapLight,
    PhongRange,
    PhongSize,
    WetSource,
    WetOldSource,
    WetLocalLegacy,
    IceSource,
    IceGlass,
    IceOldSource,
    IceOldGlass,
    IceLocal,
    MasterCount,
    OldUDisp,
    OldVDisp,
    ForceRefresh,
    UDisplace,
    VDisplace,
    UPosition,
    VPosition,
    HorizPanSpeed,
    VertPanSpeed,
    PanningStyle,
    Frequency,
    Amplitude,
    MoveIce,
    TimeMethod,
}

/// cbindgen:prefix-with-name
/// The only field selector still crossing the FFI boundary: which `UTexture*` member
/// `Hp2FractalNative::Source()` should dereference for `FractalLeaf::LockSource`/
/// `FractalLeaf::UnlockSource`. `Lock`/`Unlock` are virtual `UTexture` methods (still C++),
/// so the pointer itself must be resolved and dispatched on the C++ side.
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum FractalSourceField {
    WetSource,
    IceSource,
    IceGlass,
}

fn source_field(field: FractalField) -> FractalSourceField {
    match field {
        FractalField::WetSource => FractalSourceField::WetSource,
        FractalField::IceSource => FractalSourceField::IceSource,
        FractalField::IceGlass => FractalSourceField::IceGlass,
        _ => unreachable!("source_update is only called with Wet/Ice source fields"),
    }
}

/// cbindgen:prefix-with-name
/// Float operands/results use their exact IEEE bits in the low 32 bits.
/// Pointer operands/results use the process pointer width, not legacy DWORDs.
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum FractalLeaf {
    ClientPresent,
    NoFractalAnim,
    SetNoFractalAnim,
    TextureInit,
    TexturePostLoad,
    TexturePrime,
    TextureClear,
    TextureDestroy,
    TextureSerialize,
    TextureTick,
    VirtualConstantTick,
    InitTables,
    AssertCondition,
    NewPalette,
    AppendColor,
    CopyColor,
    ReplacePalette,
    SetMipZero,
    FlushCache,
    SetPaletteRgb,
    FillRenderTable,
    SparkTypeAt,
    CopySpark,
    AddSparks,
    RemoveSparks,
    ArchiveSaving,
    ArchiveSparks,
    RedrawSparks,
    WrapFire,
    SlowFire,
    PostDrawSparks,
    FirePaint,
    TempDrawSpark,
    WritePixel,
    WaveTablePointer,
    AllocateBytes,
    DeleteBytes,
    WaterPaint,
    AddDrop,
    DeleteDrops,
    WaterRedrawDrops,
    CalculateWater,
    SetWaveLight,
    SetRefractionTable,
    LockSource,
    UnlockSource,
    WetResample,
    IceResample,
    ApplyWet,
    MipPointer,
    MipLength,
    MoveIcePosition,
    BlitIceTex,
    BlitTexIce,
    FractalInitBase,
    FractalPostLoadBase,
    WaterInitBase,
    WaterPostLoadBase,
    WaterClearBase,
    WaterDestroyBase,
    SetIceRead,
    SetIceDestination,
    IceBlitTexKernel,
    IceBlitIceKernel,
    TextureLock,
    ApplyWetTexture,
    RenderIce,
    ArchiveSparkByte,
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum FractalEntry {
    FractalCtor,
    FractalInit,
    FractalPostLoad,
    Prime,
    FireCtor,
    FireInit,
    FireClear,
    FirePostLoad,
    FireTouch,
    FireTick,
    FireMouse,
    FireClick,
    FireSerialize,
    WaterCtor,
    WaterInit,
    WaterPostLoad,
    WaterDestroy,
    WaterClear,
    WaterMouse,
    WaterClick,
    WaveCtor,
    WaveInit,
    WavePostLoad,
    WaveClear,
    WaveTick,
    WetCtor,
    WetInit,
    WetPostLoad,
    WetClear,
    WetTick,
    WetDestroy,
    WetApply,
    IceCtor,
    IceInit,
    IcePostLoad,
    IceClear,
    IceConstantTick,
    IceTick,
    IceRender,
    IceMouse,
    IceClick,
    IceDestroy,
    BlueLaguna,
    IceBlitTex,
    IceBlitIce,
    WetLock,
    IceLock,
    SparkSerialize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FractalServices {
    pub call: unsafe extern "C" fn(*mut c_void, FractalLeaf, i64, i64, i64, *mut i64) -> i32,
}

type Result<T = ()> = std::result::Result<T, ()>;
struct Host {
    services: FractalServices,
    context: *mut c_void,
    owner: usize,
}
impl Host {
    fn call(&self, op: FractalLeaf, a: i64, b: i64, c: i64) -> Result<i64> {
        let mut out = 0;
        if unsafe { (self.services.call)(self.context, op, a, b, c, &mut out) } == 0 {
            Ok(out)
        } else {
            Err(())
        }
    }
    fn leaf(&self, op: FractalLeaf) -> Result {
        self.call(op, 0, 0, 0).map(|_| ())
    }
    fn get(&self, field: FractalField) -> Result<i64> {
        use FractalField::*;
        unsafe {
            Ok(match field {
                Client => return self.call(FractalLeaf::ClientPresent, 0, 0, 0),
                NoFractalAnim => return self.call(FractalLeaf::NoFractalAnim, 0, 0, 0),
                Parametric => (*(self.owner as *const UTexture)).b_parametric() as i64,
                Realtime => (*(self.owner as *const UTexture)).b_realtime() as i64,
                USize => (*(self.owner as *const UTexture)).base.u_size as i64,
                VSize => (*(self.owner as *const UTexture)).base.v_size as i64,
                UBits => (*(self.owner as *const UTexture)).base.u_bits as i64,
                VBits => (*(self.owner as *const UTexture)).base.v_bits as i64,
                PrimeCurrent => (*(self.owner as *const UTexture)).prime_current as i64,
                PrimeCount => (*(self.owner as *const UTexture)).prime_count as i64,
                Palette => (*(self.owner as *const UTexture)).base.palette as i64,
                PolyFlags => (*(self.owner as *const UTexture)).poly_flags as i64,
                Outer => (*(self.owner as *const UObject)).outer as i64,
                DrawPhase => (*(self.owner as *const UFractalTexture)).draw_phase as i64,
                AuxPhase => (*(self.owner as *const UFractalTexture)).aux_phase as i64,
                GlobalPhase => (*(self.owner as *const UFractalTexture)).global_phase as i64,
                UMask => (*(self.owner as *const UFractalTexture)).u_mask as i64,
                VMask => (*(self.owner as *const UFractalTexture)).v_mask as i64,
                RenderHeat => (*(self.owner as *const UFireTexture)).render_heat as i64,
                OldRenderHeat => (*(self.owner as *const UFireTexture)).old_render_heat as i64,
                ActiveSparkNum => (*(self.owner as *const UFireTexture)).active_spark_num as i64,
                SparksLimit => (*(self.owner as *const UFireTexture)).sparks_limit as i64,
                SparksNum => (*(self.owner as *const UFireTexture)).sparks.num as i64,
                SparkType => (*(self.owner as *const UFireTexture)).spark_type as i64,
                StarStatus => (*(self.owner as *const UFireTexture)).star_status as i64,
                Rising => (*(self.owner as *const UFireTexture)).b_rising() as i64,
                FireFrequency => (*(self.owner as *const UFireTexture)).fx_frequency as i64,
                FirePhase => (*(self.owner as *const UFireTexture)).fx_phase as i64,
                FireHeat => (*(self.owner as *const UFireTexture)).fx_heat as i64,
                FireSize => (*(self.owner as *const UFireTexture)).fx_size as i64,
                FireArea => (*(self.owner as *const UFireTexture)).fx_area as i64,
                FireHorizSpeed => (*(self.owner as *const UFireTexture)).fx_horiz_speed as i64,
                FireVertSpeed => (*(self.owner as *const UFireTexture)).fx_vert_speed as i64,
                NumDrops => (*(self.owner as *const UWaterTexture)).num_drops as i64,
                DropType => (*(self.owner as *const UWaterTexture)).drop_type as i64,
                WaveAmp => (*(self.owner as *const UWaterTexture)).wave_amp as i64,
                OldWaveAmp => (*(self.owner as *const UWaterTexture)).old_wave_amp as i64,
                WaveParity => (*(self.owner as *const UWaterTexture)).wave_parity as i64,
                SourceFieldsLegacy => (*(self.owner as *const UWaterTexture)).source_fields as i64,
                WaterFrequency => (*(self.owner as *const UWaterTexture)).fx_frequency as i64,
                WaterPhase => (*(self.owner as *const UWaterTexture)).fx_phase as i64,
                WaterAmplitude => (*(self.owner as *const UWaterTexture)).fx_amplitude as i64,
                WaterSpeed => (*(self.owner as *const UWaterTexture)).fx_speed as i64,
                WaterRadius => (*(self.owner as *const UWaterTexture)).fx_radius as i64,
                WaterSize => (*(self.owner as *const UWaterTexture)).fx_size as i64,
                WaterDepth => (*(self.owner as *const UWaterTexture)).fx_depth as i64,
                WaterTime => (*(self.owner as *const UWaterTexture)).fx_time as i64,
                BumpMapAngle => (*(self.owner as *const UWaveTexture)).bump_map_angle as i64,
                BumpMapLight => (*(self.owner as *const UWaveTexture)).bump_map_light as i64,
                PhongRange => (*(self.owner as *const UWaveTexture)).phong_range as i64,
                PhongSize => (*(self.owner as *const UWaveTexture)).phong_size as i64,
                WetSource => (*(self.owner as *const UWetTexture)).source_texture as i64,
                WetOldSource => (*(self.owner as *const UWetTexture)).old_source_tex as i64,
                WetLocalLegacy => (*(self.owner as *const UWetTexture)).local_source_bitmap as i64,
                IceSource => (*(self.owner as *const UIceTexture)).source_texture as i64,
                IceGlass => (*(self.owner as *const UIceTexture)).glass_texture as i64,
                IceOldSource => (*(self.owner as *const UIceTexture)).old_source_tex as i64,
                IceOldGlass => (*(self.owner as *const UIceTexture)).old_glass_tex as i64,
                IceLocal => (*(self.owner as *const UIceTexture)).local_source_bitmap as i64,
                OldUDisp => (*(self.owner as *const UIceTexture)).old_u_disp as i64,
                OldVDisp => (*(self.owner as *const UIceTexture)).old_v_disp as i64,
                ForceRefresh => (*(self.owner as *const UIceTexture)).force_refresh as i64,
                HorizPanSpeed => (*(self.owner as *const UIceTexture)).horiz_pan_speed as i64,
                VertPanSpeed => (*(self.owner as *const UIceTexture)).vert_pan_speed as i64,
                PanningStyle => (*(self.owner as *const UIceTexture)).panning_style as i64,
                Frequency => (*(self.owner as *const UIceTexture)).frequency as i64,
                Amplitude => (*(self.owner as *const UIceTexture)).amplitude as i64,
                MoveIce => (*(self.owner as *const UIceTexture)).move_ice() as i64,
                TimeMethod => (*(self.owner as *const UIceTexture)).time_method as i64,
                MasterCount => (*(self.owner as *const UIceTexture)).master_count.to_bits() as i64,
                UDisplace => (*(self.owner as *const UIceTexture)).u_displace.to_bits() as i64,
                VDisplace => (*(self.owner as *const UIceTexture)).v_displace.to_bits() as i64,
                UPosition => (*(self.owner as *const UIceTexture)).u_position.to_bits() as i64,
                VPosition => (*(self.owner as *const UIceTexture)).v_position.to_bits() as i64,
            })
        }
    }
    fn set(&self, field: FractalField, value: i64) -> Result {
        use FractalField::*;
        unsafe {
            match field {
                Client => unreachable!("FractalField::Client is read-only"),
                Outer => unreachable!("FractalField::Outer is read-only"),
                SparksNum => unreachable!("FractalField::SparksNum is read-only; use AddSparks/RemoveSparks"),
                NoFractalAnim => {
                    self.call(FractalLeaf::SetNoFractalAnim, value, 0, 0)?;
                }
                Parametric => (*(self.owner as *mut UTexture)).set_b_parametric(value != 0),
                Realtime => (*(self.owner as *mut UTexture)).set_b_realtime(value != 0),
                USize => (*(self.owner as *mut UTexture)).base.u_size = value as i32,
                VSize => (*(self.owner as *mut UTexture)).base.v_size = value as i32,
                UBits => (*(self.owner as *mut UTexture)).base.u_bits = value as u8,
                VBits => (*(self.owner as *mut UTexture)).base.v_bits = value as u8,
                PrimeCurrent => (*(self.owner as *mut UTexture)).prime_current = value as u8,
                PrimeCount => (*(self.owner as *mut UTexture)).prime_count = value as u8,
                Palette => (*(self.owner as *mut UTexture)).base.palette = value as *mut c_void,
                PolyFlags => (*(self.owner as *mut UTexture)).poly_flags = value as u32,
                DrawPhase => (*(self.owner as *mut UFractalTexture)).draw_phase = value as u8,
                AuxPhase => (*(self.owner as *mut UFractalTexture)).aux_phase = value as u8,
                GlobalPhase => (*(self.owner as *mut UFractalTexture)).global_phase = value as i32,
                UMask => (*(self.owner as *mut UFractalTexture)).u_mask = value as i32,
                VMask => (*(self.owner as *mut UFractalTexture)).v_mask = value as i32,
                RenderHeat => (*(self.owner as *mut UFireTexture)).render_heat = value as u8,
                OldRenderHeat => (*(self.owner as *mut UFireTexture)).old_render_heat = value as i32,
                ActiveSparkNum => (*(self.owner as *mut UFireTexture)).active_spark_num = value as i32,
                SparksLimit => (*(self.owner as *mut UFireTexture)).sparks_limit = value as i32,
                SparkType => (*(self.owner as *mut UFireTexture)).spark_type = value as u8,
                StarStatus => (*(self.owner as *mut UFireTexture)).star_status = value as u8,
                Rising => (*(self.owner as *mut UFireTexture)).set_b_rising(value != 0),
                FireFrequency => (*(self.owner as *mut UFireTexture)).fx_frequency = value as u8,
                FirePhase => (*(self.owner as *mut UFireTexture)).fx_phase = value as u8,
                FireHeat => (*(self.owner as *mut UFireTexture)).fx_heat = value as u8,
                FireSize => (*(self.owner as *mut UFireTexture)).fx_size = value as u8,
                FireArea => (*(self.owner as *mut UFireTexture)).fx_area = value as u8,
                FireHorizSpeed => (*(self.owner as *mut UFireTexture)).fx_horiz_speed = value as u8,
                FireVertSpeed => (*(self.owner as *mut UFireTexture)).fx_vert_speed = value as u8,
                NumDrops => (*(self.owner as *mut UWaterTexture)).num_drops = value as i32,
                DropType => (*(self.owner as *mut UWaterTexture)).drop_type = value as u8,
                WaveAmp => (*(self.owner as *mut UWaterTexture)).wave_amp = value as u8,
                OldWaveAmp => (*(self.owner as *mut UWaterTexture)).old_wave_amp = value as i32,
                WaveParity => (*(self.owner as *mut UWaterTexture)).wave_parity = value as u8,
                SourceFieldsLegacy => (*(self.owner as *mut UWaterTexture)).source_fields = value as i32,
                WaterFrequency => (*(self.owner as *mut UWaterTexture)).fx_frequency = value as u8,
                WaterPhase => (*(self.owner as *mut UWaterTexture)).fx_phase = value as u8,
                WaterAmplitude => (*(self.owner as *mut UWaterTexture)).fx_amplitude = value as u8,
                WaterSpeed => (*(self.owner as *mut UWaterTexture)).fx_speed = value as u8,
                WaterRadius => (*(self.owner as *mut UWaterTexture)).fx_radius = value as u8,
                WaterSize => (*(self.owner as *mut UWaterTexture)).fx_size = value as u8,
                WaterDepth => (*(self.owner as *mut UWaterTexture)).fx_depth = value as u8,
                WaterTime => (*(self.owner as *mut UWaterTexture)).fx_time = value as u8,
                BumpMapAngle => (*(self.owner as *mut UWaveTexture)).bump_map_angle = value as u8,
                BumpMapLight => (*(self.owner as *mut UWaveTexture)).bump_map_light = value as u8,
                PhongRange => (*(self.owner as *mut UWaveTexture)).phong_range = value as u8,
                PhongSize => (*(self.owner as *mut UWaveTexture)).phong_size = value as u8,
                WetSource => (*(self.owner as *mut UWetTexture)).source_texture = value as *mut c_void,
                WetOldSource => (*(self.owner as *mut UWetTexture)).old_source_tex = value as *mut c_void,
                WetLocalLegacy => (*(self.owner as *mut UWetTexture)).local_source_bitmap = value as i32,
                IceSource => (*(self.owner as *mut UIceTexture)).source_texture = value as *mut c_void,
                IceGlass => (*(self.owner as *mut UIceTexture)).glass_texture = value as *mut c_void,
                IceOldSource => (*(self.owner as *mut UIceTexture)).old_source_tex = value as *mut c_void,
                IceOldGlass => (*(self.owner as *mut UIceTexture)).old_glass_tex = value as *mut c_void,
                IceLocal => (*(self.owner as *mut UIceTexture)).local_source_bitmap = value as *mut c_void,
                OldUDisp => (*(self.owner as *mut UIceTexture)).old_u_disp = value as i32,
                OldVDisp => (*(self.owner as *mut UIceTexture)).old_v_disp = value as i32,
                ForceRefresh => (*(self.owner as *mut UIceTexture)).force_refresh = value as i32,
                HorizPanSpeed => (*(self.owner as *mut UIceTexture)).horiz_pan_speed = value as u8,
                VertPanSpeed => (*(self.owner as *mut UIceTexture)).vert_pan_speed = value as u8,
                PanningStyle => (*(self.owner as *mut UIceTexture)).panning_style = value as u8,
                Frequency => (*(self.owner as *mut UIceTexture)).frequency = value as u8,
                Amplitude => (*(self.owner as *mut UIceTexture)).amplitude = value as u8,
                MoveIce => (*(self.owner as *mut UIceTexture)).set_move_ice(value != 0),
                TimeMethod => (*(self.owner as *mut UIceTexture)).time_method = value as u8,
                MasterCount => (*(self.owner as *mut UIceTexture)).master_count = f32::from_bits(value as u32),
                UDisplace => (*(self.owner as *mut UIceTexture)).u_displace = f32::from_bits(value as u32),
                VDisplace => (*(self.owner as *mut UIceTexture)).v_displace = f32::from_bits(value as u32),
                UPosition => (*(self.owner as *mut UIceTexture)).u_position = f32::from_bits(value as u32),
                VPosition => (*(self.owner as *mut UIceTexture)).v_position = f32::from_bits(value as u32),
            }
        }
        Ok(())
    }
    fn texture_bits(&self, pointer: i64, which: i64) -> Result<i64> {
        if pointer == 0 {
            return Err(());
        }
        unsafe {
            let t = pointer as *const UTexture;
            Ok(if which == 0 { (*t).base.u_bits as i64 } else { (*t).base.v_bits as i64 })
        }
    }
    fn texture_size(&self, pointer: i64, which: i64) -> Result<i64> {
        if pointer == 0 {
            return Err(());
        }
        unsafe {
            let t = pointer as *const UTexture;
            Ok(if which == 0 { (*t).base.u_size as i64 } else { (*t).base.v_size as i64 })
        }
    }
    fn texture_palette(&self, pointer: i64) -> Result<i64> {
        if pointer == 0 {
            return Err(());
        }
        unsafe { Ok((*(pointer as *const UTexture)).base.palette as i64) }
    }
    fn object_outer(&self, pointer: i64) -> Result<i64> {
        if pointer == 0 {
            return Err(());
        }
        unsafe { Ok((*(pointer as *const UObject)).outer as i64) }
    }
    fn float(&self, field: FractalField) -> Result<f32> {
        Ok(f32::from_bits(self.get(field)? as u32))
    }
    fn set_float(&self, field: FractalField, value: f32) -> Result {
        self.set(field, value.to_bits() as i64)
    }
    fn defaults(&self, fields: &[(FractalField, i64)]) -> Result {
        for &(field, value) in fields {
            self.set(field, value)?;
        }
        Ok(())
    }
    fn animated(&self) -> Result<bool> {
        Ok(self.get(FractalField::Client)? != 0 && self.get(FractalField::NoFractalAnim)? == 0)
    }
    fn safe_size(&self) -> Result<bool> {
        Ok(self.get(FractalField::USize)? >= 8 && self.get(FractalField::VSize)? >= 8)
    }
    fn field_size(&self) -> Result<i64> {
        Ok(self.get(FractalField::USize)? * self.get(FractalField::VSize)? / 2)
    }
    fn palette(&self, kind: i64, laguna: bool) -> Result {
        use FractalLeaf::*;
        let palette = self.call(NewPalette, 0, 0, 0)?;
        self.set(FractalField::Palette, palette)?;
        for i in 0..256 {
            let color = match kind {
                0 => i | (i << 8),
                1 => 0,
                _ => i | (i << 8) | (i << 16),
            };
            // The native three-channel FColor constructor retains its original alpha semantics.
            self.call(AppendColor, self.get(FractalField::Palette)?, color, 0)?;
        }
        if laguna {
            self.blue_laguna(self.get(FractalField::Palette)?)?;
        }
        self.leaf(SetMipZero)
    }
    fn blue_laguna(&self, palette: i64) -> Result {
        let mut colors = [crate::palette::FireRgb::default(); 256];
        crate::palette::blue_laguna_palette(&mut colors);
        for (index, color) in colors.iter().enumerate() {
            self.call(
                FractalLeaf::SetPaletteRgb,
                palette,
                index as i64,
                color.r as i64 | ((color.g as i64) << 8) | ((color.b as i64) << 16),
            )?;
        }
        Ok(())
    }
    fn copy_palette(&self) -> Result {
        use FractalField::*;
        use FractalLeaf::*;
        let palette = self.get(Palette)?;
        if palette != 0 && self.get(Outer)? != self.object_outer(palette)? {
            let new_palette = self.call(NewPalette, 1, 0, 0)?;
            for i in 0..256 {
                self.call(CopyColor, new_palette, 0, i)?;
            }
            let replacement = self.call(ReplacePalette, new_palette, 0, 0)?;
            self.set(Palette, replacement)?;
            self.leaf(SetMipZero)?;
            self.leaf(FlushCache)?;
        }
        Ok(())
    }
    fn source_update(&self, field: FractalField, time_mode: i64) -> Result {
        let source = self.get(field)?;
        if source != 0 && source as usize != self.owner {
            let selector = source_field(field) as i64;
            self.call(FractalLeaf::LockSource, selector, time_mode, 0)?;
            // Original C++ reevaluates the source member for Unlock, not a cached pointer.
            self.call(FractalLeaf::UnlockSource, selector, 0, 0)?;
        }
        Ok(())
    }
    fn clear_water(&self, flags: i64) -> Result {
        if flags & 2 != 0 {
            let fields = self.water_pointer(0)?;
            // The fill contains no callbacks; native dimensions cannot change
            // within this uninterrupted write pass.
            let size = self.field_size()?;
            for i in 0..size {
                unsafe {
                    (fields as *mut u8).add(i as usize).write(128);
                }
            }
        }
        if flags & 1 != 0 {
            self.set(FractalField::NumDrops, 0)?;
        }
        Ok(())
    }
    fn water_pointer(&self, slot: usize) -> Result<usize> {
        Ok(water_host_get(self.owner, slot))
    }
    fn allocate_water(&self, slot: usize, size: i64) -> Result<usize> {
        use FractalLeaf::*;
        self.call(AssertCondition, i64::from(size > 0), 0, 0)?;
        self.call(AssertCondition, i64::from(self.owner != 0), 0, 0)?;
        let current = water_host_get(self.owner, slot);
        self.call(AssertCondition, i64::from(current == 0), 0, 0)?;
        let pointer = self.call(AllocateBytes, size, 0, 0)?;
        // Re-find by owner after the allocator leaf, never retain a stale
        // borrow of the host-state storage across a possible native
        // allocator callback.
        water_host_set(self.owner, slot, pointer as usize);
        Ok(pointer as usize)
    }
    fn free_water(&self, slot: usize) -> Result {
        use FractalLeaf::*;
        let pointer = water_host_get(self.owner, slot);
        if pointer == 0 {
            return Ok(());
        }
        self.call(DeleteBytes, pointer as i64, 0, 0)?;
        water_host_set(self.owner, slot, 0);
        water_host_remove_if_empty(self.owner);
        Ok(())
    }
    fn cull(&self, stop_at_limit: bool) -> Result {
        use FractalField::*;
        let mut index = self.get(ActiveSparkNum)? - 1;
        while index >= 0 {
            if self.call(FractalLeaf::SparkTypeAt, index, 0, 0)? >= 31 {
                let active = self.get(ActiveSparkNum)? - 1;
                self.set(ActiveSparkNum, active)?;
                self.call(FractalLeaf::CopySpark, index, active, 0)?;
                if stop_at_limit && self.get(ActiveSparkNum)? <= self.get(SparksLimit)? {
                    break;
                }
            }
            index -= 1;
        }
        Ok(())
    }
    fn fire_postload(&self) -> Result {
        use FractalField::*;
        use FractalLeaf::*;
        self.leaf(FractalPostLoadBase)?;
        self.set(PolyFlags, self.get(PolyFlags)? & !2)?;
        self.copy_palette()?;
        if self.get(OldRenderHeat)? != self.get(RenderHeat)? {
            self.leaf(FillRenderTable)?;
            self.set(OldRenderHeat, self.get(RenderHeat)?)?;
        }
        if self.get(SparksNum)? != self.get(SparksLimit)? {
            self.set(SparksLimit, self.get(SparksLimit)?.clamp(4, 8192))?;
            if self.get(SparksNum)? > self.get(SparksLimit)? {
                if self.get(ActiveSparkNum)? > self.get(SparksLimit)? {
                    self.cull(true)?;
                }
                self.set(
                    ActiveSparkNum,
                    self.get(ActiveSparkNum)?.min(self.get(SparksLimit)?),
                )?;
                self.call(
                    RemoveSparks,
                    self.get(SparksLimit)?,
                    self.get(SparksNum)? - self.get(SparksLimit)?,
                    0,
                )?;
            } else {
                self.call(
                    AddSparks,
                    self.get(SparksLimit)? - self.get(SparksNum)?,
                    0,
                    0,
                )?;
            }
        }
        Ok(())
    }
    fn fractal_postload(&self) -> Result {
        self.leaf(FractalLeaf::TexturePostLoad)?;
        self.set(FractalField::UMask, self.get(FractalField::USize)? - 1)?;
        self.set(FractalField::VMask, self.get(FractalField::VSize)? - 1)
    }
    fn fractal_init(&self, u: i64, v: i64) -> Result {
        self.call(
            FractalLeaf::AssertCondition,
            i64::from((u as i32 & (u as i32).wrapping_sub(1)) == 0),
            0,
            0,
        )?;
        self.call(
            FractalLeaf::AssertCondition,
            i64::from((v as i32 & (v as i32).wrapping_sub(1)) == 0),
            0,
            0,
        )?;
        self.call(FractalLeaf::TextureInit, u, v, 0).map(|_| ())
    }
    fn water_init(&self, u: i64, v: i64) -> Result {
        use FractalField::*;
        self.call(FractalLeaf::FractalInitBase, u, v, 0)?;
        self.defaults(&[
            (NumDrops, 0),
            (DropType, 1),
            (WaveAmp, 128),
            (WaterFrequency, 8),
            (WaterPhase, 8),
            (WaterAmplitude, 255),
            (WaterSpeed, 255),
            (WaterRadius, 128),
            (WaterSize, 30),
            (WaterDepth, 255),
            (WaterTime, 30),
        ])
    }
    fn water_postload(&self) -> Result {
        self.leaf(FractalLeaf::FractalPostLoadBase)?;
        if self.water_pointer(0)? == 0 {
            let size = self.field_size()?;
            let fields = self.allocate_water(0, size)?;
            for i in 0..size {
                unsafe {
                    (fields as *mut u8).add(i as usize).write(128);
                }
            }
        }
        Ok(())
    }
    fn wet_postload(&self) -> Result {
        use FractalField::*;
        use FractalLeaf::*;
        self.leaf(WaterPostLoadBase)?;
        self.set(WetLocalLegacy, 0)?;
        if self.get(WetSource)? == 0 {
            return Ok(());
        }
        self.source_update(WetSource, 0)?;
        let source_u = self.texture_bits(self.get(WetSource)?, 0)?;
        let source_v = self.texture_bits(self.get(WetSource)?, 1)?;
        if source_u != self.get(UBits)? || source_v != self.get(VBits)? {
            let u = self.get(UBits)? - source_u;
            let v = self.get(VBits)? - source_v;
            if u >= 0 && v >= 0 {
                self.free_water(1)?;
                let local = self.allocate_water(1, self.get(USize)? * self.get(VSize)?)?;
                self.call(WetResample, local as i64, u, v)?;
            } else {
                self.set(WetSource, 0)?;
            }
        } else {
            self.free_water(1)?;
        }
        if self.get(WetSource)? != 0 && self.get(WetSource)? != self.get(WetOldSource)? {
            self.set(
                Palette,
                self.texture_palette(self.get(WetSource)?)?,
            )?;
        }
        self.set(WetOldSource, self.get(WetSource)?)?;
        if self.get(WaveAmp)? != self.get(OldWaveAmp)? {
            self.leaf(SetRefractionTable)?;
        }
        self.set(OldWaveAmp, self.get(WaveAmp)?)
    }
    fn ice_postload(&self) -> Result {
        use FractalField::*;
        use FractalLeaf::*;
        self.leaf(FractalPostLoadBase)?;
        if self.get(IceSource)? != 0 {
            self.source_update(IceSource, 0)?;
            if !self.safe_size()? {
                self.set(IceSource, 0)?;
                self.set(IceGlass, 0)?;
            } else {
                let wrong_source = self.texture_size(self.get(IceSource)?, 0)?
                    != self.get(USize)?
                    || self.texture_size(self.get(IceSource)?, 1)? != self.get(VSize)?;
                let mut wrong_glass = true;
                if self.get(IceGlass)? != 0 {
                    wrong_glass = self.texture_size(self.get(IceGlass)?, 0)?
                        != self.get(USize)?
                        || self.texture_size(self.get(IceGlass)?, 1)? != self.get(VSize)?;
                    if wrong_glass {
                        self.set(IceGlass, 0)?;
                    }
                }
                if !wrong_source && !wrong_glass {
                    self.set(IceLocal, 0)?;
                } else {
                    let u =
                        self.get(UBits)? - self.texture_bits(self.get(IceSource)?, 0)?;
                    let v =
                        self.get(VBits)? - self.texture_bits(self.get(IceSource)?, 1)?;
                    if u >= 0 && v >= 0 {
                        self.set(IceLocal, self.leaf_pointer()?)?;
                        self.call(IceResample, u, v, 0)?;
                    } else {
                        self.set(IceSource, 0)?;
                    }
                }
            }
        }
        if self.get(IceSource)? != 0 && self.get(IceSource)? != self.get(IceOldSource)? {
            self.set(
                Palette,
                self.texture_palette(self.get(IceSource)?)?,
            )?;
            self.set(ForceRefresh, 1)?;
        }
        self.set(IceOldSource, self.get(IceSource)?)?;
        if self.get(IceGlass)? != self.get(IceOldGlass)? {
            self.set(ForceRefresh, 1)?;
        }
        self.set(IceOldGlass, self.get(IceGlass)?)
    }
    fn leaf_pointer(&self) -> Result<i64> {
        self.call(FractalLeaf::MipPointer, self.owner as i64, 0, 0)
    }
    fn ice_render(&self, delta: i64) -> Result {
        use FractalField::*;
        use FractalLeaf::*;
        if self.get(IceGlass)? == 0 || self.get(IceSource)? == 0 {
            return Ok(());
        }
        self.call(MoveIcePosition, delta, 0, 0)?;
        if self.float(UPosition)? as i32 as i64 == self.get(OldUDisp)?
            && self.float(VPosition)? as i32 as i64 == self.get(OldVDisp)?
            && self.get(ForceRefresh)? == 0
        {
            return Ok(());
        }
        self.set(OldUDisp, self.float(UPosition)? as i32 as i64)?;
        self.set(OldVDisp, self.float(VPosition)? as i32 as i64)?;
        if self.get(MoveIce)? == 0 {
            self.leaf(BlitIceTex)?;
        } else {
            self.leaf(BlitTexIce)?;
        }
        self.set(ForceRefresh, 0)
    }
}

/// Water/wet per-instance host state: `SourceFields` (slot 0) and
/// `LocalSourceBitmap` (slot 1), the two allocations behind `FindWaterSourceFields`
/// and the wet local-source resample. Replaces the native
/// `TMap<UWaterTexture*,FWaterHostState>`; Rust already owned every
/// create/find/insert/remove/delete decision, so it owns the storage too,
/// keyed by the owning texture's address. An entry is removed once both slots
/// are freed (`free_water`), which `WaterDestroy`/`WetDestroy` always do.
static WATER_HOST_STATES: Mutex<Vec<(usize, [usize; 2])>> = Mutex::new(Vec::new());

fn water_host_get(owner: usize, slot: usize) -> usize {
    WATER_HOST_STATES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(key, _)| *key == owner)
        .map_or(0, |(_, slots)| slots[slot])
}

fn water_host_set(owner: usize, slot: usize, value: usize) {
    let mut states = WATER_HOST_STATES.lock().unwrap_or_else(|e| e.into_inner());
    match states.iter_mut().find(|(key, _)| *key == owner) {
        Some((_, slots)) => slots[slot] = value,
        None => {
            let mut slots = [0usize; 2];
            slots[slot] = value;
            states.push((owner, slots));
        }
    }
}

fn water_host_remove_if_empty(owner: usize) {
    let mut states = WATER_HOST_STATES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pos) = states.iter().position(|(key, _)| *key == owner)
        && states[pos].1 == [0, 0]
    {
        states.remove(pos);
    }
}

static ICE_MOUSE: Mutex<(f32, f32)> = Mutex::new((0.0, 0.0));
/// Finds a water allocation for the owner and slot.
///
/// # Safety
/// The caller must uphold the lifecycle owner and context invariants, and `slot` must be 0 or 1.
#[expect(
    clippy::result_unit_err,
    reason = "Preserve the existing lifecycle error contract"
)]
pub unsafe fn water_pointer(
    services: FractalServices,
    context: *mut c_void,
    owner: usize,
    slot: usize,
) -> std::result::Result<usize, ()> {
    Host {
        services,
        context,
        owner,
    }
    .water_pointer(slot)
}

/// Calls the lifecycle entry through synchronous checked callbacks.
///
/// # Safety
/// The native context, owner, callback table, and any objects or buffers selected by the
/// entry and its arguments must remain valid through all synchronous callbacks and reentry.
/// Native typed leaves retain all allocator, UObject, archive and mip lifetimes.
pub unsafe fn run(
    services: FractalServices,
    context: *mut c_void,
    owner: usize,
    entry: FractalEntry,
    a: i64,
    b: i64,
    c: i64,
) -> i32 {
    let host = Host {
        services,
        context,
        owner,
    };
    match execute(&host, entry, a, b, c) {
        Ok(()) => 0,
        Err(()) => 1,
    }
}
fn execute(h: &Host, entry: FractalEntry, a: i64, b: i64, c: i64) -> Result {
    use FractalEntry::*;
    use FractalField::*;
    use FractalLeaf as L;
    match entry {
        FractalCtor => {
            h.set(Parametric, 1)?;
            h.set(Realtime, 1)?;
            h.leaf(L::InitTables)?;
            h.defaults(&[(DrawPhase, 0), (AuxPhase, 0), (GlobalPhase, 0)])?;
        }
        FractalInit => h.fractal_init(a, b)?,
        FractalPostLoad => h.fractal_postload()?,
        Prime => {
            if h.get(PrimeCurrent)? != 0 {
                return Ok(());
            }
            if h.get(Client)? != 0 && h.get(NoFractalAnim)? != 0 {
                h.set(NoFractalAnim, 0)?;
                h.set(PrimeCount, h.get(PrimeCount)?.max(48))?;
                while h.get(PrimeCurrent)? < h.get(PrimeCount)? {
                    h.set(PrimeCurrent, (h.get(PrimeCurrent)? + 1) as u8 as i64)?;
                    h.leaf(L::VirtualConstantTick)?;
                }
                h.set(NoFractalAnim, 1)?;
                h.set(Realtime, 0)?;
            } else {
                h.leaf(L::TexturePrime)?;
            }
        }
        FireCtor => h.defaults(&[(OldRenderHeat, -1), (AuxPhase, 0), (StarStatus, 1)])?,
        FireInit => {
            h.call(L::FractalInitBase, a, b, 0)?;
            h.palette(0, true)?;
            h.defaults(&[
                (RenderHeat, 220),
                (ActiveSparkNum, 0),
                (SparkType, 4),
                (SparksLimit, 1024),
            ])?;
            h.call(L::AddSparks, h.get(SparksLimit)?, 0, 0)?;
            h.defaults(&[
                (FireFrequency, 16),
                (FirePhase, 16),
                (FireHeat, 255),
                (FireSize, 96),
                (FireArea, 24),
                (FireHorizSpeed, 130),
                (FireVertSpeed, 142),
            ])?;
        }
        FireClear => {
            h.call(L::TextureClear, a, 0, 0)?;
            if a & 1 != 0 {
                h.set(ActiveSparkNum, 0)?;
                h.set(StarStatus, 0)?;
            }
        }
        FirePostLoad => h.fire_postload()?,
        FireTouch => {
            h.call(
                L::WritePixel,
                a.wrapping_add(b.wrapping_shl(h.get(UBits)? as u32)),
                f32::from_bits(c as u32) as u8 as i64,
                0,
            )?;
        }
        FireTick => {
            if h.animated()? && h.safe_size()? {
                h.leaf(L::RedrawSparks)?;
                if h.get(Rising)? != 0 {
                    h.leaf(L::WrapFire)?;
                } else {
                    h.leaf(L::SlowFire)?;
                }
                h.leaf(L::PostDrawSparks)?;
            }
        }
        FireMouse | FireClick => {
            let x = f32::from_bits(b as u32) as i32 as i64;
            let y = f32::from_bits(c as u32) as i32 as i64;
            h.call(L::FirePaint, x, y, a)?;
            if matches!(entry, FireMouse) {
                for dx in [0, 1, -1] {
                    h.call(L::TempDrawSpark, x + dx, y, 255)?;
                }
            } else {
                h.call(L::WritePixel, x + y * h.get(USize)?, 255, 0)?;
            }
        }
        FireSerialize => {
            h.leaf(L::TextureSerialize)?;
            if h.call(L::ArchiveSaving, 0, 0, 0)? != 0 {
                h.cull(false)?;
                if h.get(ActiveSparkNum)? < h.get(SparksNum)? {
                    h.call(
                        L::RemoveSparks,
                        h.get(ActiveSparkNum)?,
                        h.get(SparksNum)? - h.get(ActiveSparkNum)?,
                        0,
                    )?;
                }
            }
            h.leaf(L::ArchiveSparks)?;
            if h.get(SparksNum)? < h.get(SparksLimit)? {
                h.call(L::AddSparks, h.get(SparksLimit)? - h.get(SparksNum)?, 0, 0)?;
            }
        }
        WaterCtor => {
            let table = h.call(L::WaveTablePointer, 0, 0, 0)? as *mut u8;
            for t in 0i32..1536 {
                unsafe {
                    table
                        .add(t as usize)
                        .write((t / 2 - 256 + i32::from(t - 512 < 256)).clamp(0, 255) as u8);
                }
            }
            h.defaults(&[(OldWaveAmp, -1), (WaveParity, 0), (SourceFieldsLegacy, 0)])?;
        }
        WaterInit => h.water_init(a, b)?,
        WaterPostLoad => h.water_postload()?,
        WaterDestroy => {
            h.free_water(0)?;
            h.set(SourceFieldsLegacy, 0)?;
            h.leaf(L::TextureDestroy)?;
        }
        WaterClear => {
            h.call(L::TextureClear, a, 0, 0)?;
            h.clear_water(a)?;
        }
        WaveClear | WetClear => {
            h.call(L::WaterClearBase, a, 0, 0)?;
            h.clear_water(a)?;
        }
        WaterMouse => {
            h.call(
                L::WaterPaint,
                f32::from_bits(b as u32) as i32 as i64,
                f32::from_bits(c as u32) as i32 as i64,
                a,
            )?;
        }
        WaterClick => {
            let x = f32::from_bits(b as u32) as i32 as i64;
            let y = f32::from_bits(c as u32) as i32 as i64;
            if a & 1 != 0 {
                h.call(L::AddDrop, x, y, 0)?;
            }
            if a & 2 != 0 {
                h.call(L::DeleteDrops, x, y, 12)?;
            }
        }
        WaveCtor | IceClick => {}
        WaveInit => {
            h.call(L::WaterInitBase, a, b, 0)?;
            h.defaults(&[
                (BumpMapAngle, 170),
                (BumpMapLight, 50),
                (PhongRange, 180),
                (PhongSize, 32),
            ])?;
            h.palette(1, true)?;
        }
        WavePostLoad => {
            h.leaf(L::WaterPostLoadBase)?;
            h.copy_palette()?;
            h.leaf(L::SetWaveLight)?;
        }
        WaveTick => {
            if h.animated()? && h.safe_size()? {
                h.leaf(L::WaterRedrawDrops)?;
                h.leaf(L::CalculateWater)?;
            }
        }
        WetCtor => h.defaults(&[(WetOldSource, 0), (WetLocalLegacy, 0)])?,
        WetInit => {
            h.call(L::WaterInitBase, a, b, 0)?;
            h.palette(2, true)?;
        }
        WetPostLoad => h.wet_postload()?,
        WetTick => {
            if h.animated()? && h.get(WetSource)? != 0 && h.safe_size()? {
                h.source_update(WetSource, 1)?;
                h.leaf(L::WaterRedrawDrops)?;
                h.leaf(L::CalculateWater)?;
                h.leaf(L::ApplyWetTexture)?;
            }
        }
        WetDestroy => {
            h.free_water(1)?;
            h.set(WetLocalLegacy, 0)?;
            h.leaf(L::WaterDestroyBase)?;
        }
        WetApply => {
            if h.get(WetSource)? != 0 {
                let destination = h.leaf_pointer()?;
                let local = h.water_pointer(1)?;
                let (source, length) = if local != 0 {
                    (local as i64, h.get(USize)? * h.get(VSize)?)
                } else {
                    (
                        h.call(L::MipPointer, h.get(WetSource)?, 0, 0)?,
                        h.call(L::MipLength, h.get(WetSource)?, 0, 0)?,
                    )
                };
                h.call(
                    L::ApplyWet,
                    source,
                    length,
                    i64::from(source == destination),
                )?;
            }
        }
        IceCtor => h.defaults(&[
            (MasterCount, 0),
            (OldUDisp, -1),
            (OldVDisp, -1),
            (IceOldSource, 0),
            (IceOldGlass, 0),
            (IceLocal, 0),
            (ForceRefresh, 1),
        ])?,
        IceInit => {
            h.call(L::FractalInitBase, a, b, 0)?;
            h.defaults(&[
                (UDisplace, 0),
                (VDisplace, 0),
                (HorizPanSpeed, 128),
                (VertPanSpeed, 128),
                (PanningStyle, 0),
                (Frequency, 11),
                (Amplitude, 44),
                (MoveIce, 1),
            ])?;
            h.palette(2, false)?;
        }
        IcePostLoad => h.ice_postload()?,
        IceClear => {
            h.call(L::TextureClear, a, 0, 0)?;
        }
        IceConstantTick | IceTick => {
            if h.animated()? {
                if matches!(entry, IceTick) && h.get(TimeMethod)? == 0 {
                    h.call(L::TextureTick, a, 0, 0)?;
                } else {
                    let current = matches!(entry, IceConstantTick);
                    h.source_update(IceSource, i64::from(current))?;
                    h.source_update(IceGlass, i64::from(current))?;
                    h.call(
                        L::RenderIce,
                        if current {
                            (1.0f32 / 120.0f32).to_bits() as i64
                        } else {
                            a
                        },
                        0,
                        0,
                    )?;
                }
            }
        }
        IceRender => h.ice_render(a)?,
        IceBlitTex | IceBlitIce => {
            h.leaf(L::SetIceDestination)?;
            for (slot, field) in [(0, IceSource), (1, IceGlass)] {
                let texture = h.get(field)?;
                let (pointer, length) = if texture == 0 {
                    (0, 0)
                } else {
                    (
                        h.call(L::MipPointer, texture, 0, 0)?,
                        h.call(L::MipLength, h.get(field)?, 0, 0)?,
                    )
                };
                h.call(L::SetIceRead, slot, pointer, length)?;
            }
            h.leaf(if matches!(entry, IceBlitTex) {
                L::IceBlitTexKernel
            } else {
                L::IceBlitIceKernel
            })?;
        }
        IceMouse => {
            let x = f32::from_bits(b as u32);
            let y = f32::from_bits(c as u32);
            let previous = *ICE_MOUSE.lock().unwrap_or_else(|e| e.into_inner());
            if a & 1 != 0 {
                h.set_float(UDisplace, h.float(UDisplace)? + (previous.0 - x))?;
                h.set_float(VDisplace, h.float(VDisplace)? + (previous.1 - y))?;
                h.set(UPosition, h.get(UDisplace)?)?;
                h.set(VPosition, h.get(VDisplace)?)?;
            }
            *ICE_MOUSE.lock().unwrap_or_else(|e| e.into_inner()) = (x, y);
        }
        IceDestroy => h.leaf(L::TextureDestroy)?,
        WetLock | IceLock => {
            if matches!(entry, IceLock) {
                h.source_update(IceGlass, 2)?;
            }
            h.source_update(
                if matches!(entry, WetLock) {
                    WetSource
                } else {
                    IceSource
                },
                2,
            )?;
            h.leaf(L::TextureLock)?;
        }
        SparkSerialize => {
            for index in 0..8 {
                h.call(L::ArchiveSparkByte, index, 0, 0)?;
            }
        }
        BlueLaguna => h.blue_laguna(a)?,
    }
    Ok(())
}

#[cfg(test)]
mod field_tests {
    use super::*;
    use hp2_abi::engine::generated::{TArrayRaw, UFireTexture};

    unsafe extern "C" fn unreachable_call(
        _: *mut c_void,
        _: FractalLeaf,
        _: i64,
        _: i64,
        _: i64,
        _: *mut i64,
    ) -> i32 {
        panic!("direct field mirror access must not reach the native leaf callback");
    }

    fn host_over(buffer: &mut [u8]) -> Host {
        Host {
            services: FractalServices {
                call: unreachable_call,
            },
            context: std::ptr::null_mut(),
            owner: buffer.as_mut_ptr() as usize,
        }
    }

    // UnFractal.cpp (deleted): FractalField_DrawPhase/AuxPhase/GlobalPhase/UMask/VMask,
    // read at lines 206-208,213-214; write at 286-288,293-294.
    #[test]
    fn fractal_texture_scalar_fields_round_trip() {
        let mut buf = vec![0u8; 512];
        let host = host_over(&mut buf);
        host.set(FractalField::DrawPhase, 7).unwrap();
        host.set(FractalField::AuxPhase, 9).unwrap();
        host.set(FractalField::GlobalPhase, -123).unwrap();
        host.set(FractalField::UMask, 0x1F).unwrap();
        host.set(FractalField::VMask, 0x3F).unwrap();
        assert_eq!(host.get(FractalField::DrawPhase).unwrap(), 7);
        assert_eq!(host.get(FractalField::AuxPhase).unwrap(), 9);
        assert_eq!(host.get(FractalField::GlobalPhase).unwrap(), -123);
        assert_eq!(host.get(FractalField::UMask).unwrap(), 0x1F);
        assert_eq!(host.get(FractalField::VMask).unwrap(), 0x3F);
    }

    // UnFractal.cpp (deleted): FractalField_USize/VSize/UBits/VBits/PrimeCurrent/
    // PrimeCount/Palette/PolyFlags/Parametric/Realtime, read 209-220, write 289-300.
    #[test]
    fn texture_base_fields_round_trip() {
        let mut buf = vec![0u8; 512];
        let host = host_over(&mut buf);
        host.set(FractalField::USize, 256).unwrap();
        host.set(FractalField::VSize, 128).unwrap();
        host.set(FractalField::UBits, 8).unwrap();
        host.set(FractalField::VBits, 7).unwrap();
        host.set(FractalField::PrimeCurrent, 3).unwrap();
        host.set(FractalField::PrimeCount, 5).unwrap();
        host.set(FractalField::Palette, 0xdead_beef).unwrap();
        host.set(FractalField::PolyFlags, 0x40).unwrap();
        host.set(FractalField::Parametric, 1).unwrap();
        host.set(FractalField::Realtime, 0).unwrap();
        assert_eq!(host.get(FractalField::USize).unwrap(), 256);
        assert_eq!(host.get(FractalField::VSize).unwrap(), 128);
        assert_eq!(host.get(FractalField::UBits).unwrap(), 8);
        assert_eq!(host.get(FractalField::VBits).unwrap(), 7);
        assert_eq!(host.get(FractalField::PrimeCurrent).unwrap(), 3);
        assert_eq!(host.get(FractalField::PrimeCount).unwrap(), 5);
        assert_eq!(host.get(FractalField::Palette).unwrap(), 0xdead_beef);
        assert_eq!(host.get(FractalField::PolyFlags).unwrap(), 0x40);
        assert_eq!(host.get(FractalField::Parametric).unwrap(), 1);
        assert_eq!(host.get(FractalField::Realtime).unwrap(), 0);
    }

    // UnFractal.cpp (deleted): FractalField_Outer read at line 221 (`X->GetOuter()`).
    // UObject::GetOuter() is non-virtual (`return Outer;`), so the mirror field read
    // is exact; the field had no Write arm (hit `default: check(0)` on write).
    #[test]
    fn outer_is_read_only_and_reads_the_object_header() {
        let mut buf = vec![0u8; 512];
        // UObject.outer lives at byte offset 48 in the mirror.
        buf[48..56].copy_from_slice(&0x1234_5678u64.to_ne_bytes());
        let host = host_over(&mut buf);
        assert_eq!(host.get(FractalField::Outer).unwrap(), 0x1234_5678);
    }

    #[test]
    #[should_panic]
    fn outer_write_matches_original_check_0_crash() {
        let mut buf = vec![0u8; 512];
        let host = host_over(&mut buf);
        let _ = host.set(FractalField::Outer, 1);
    }

    // UnFractal.cpp (deleted): FractalField_RenderHeat..FireVertSpeed,
    // read 224-238, write 302-315.
    #[test]
    fn fire_texture_fields_round_trip() {
        let mut buf = vec![0u8; 2048];
        let host = host_over(&mut buf);
        host.set(FractalField::RenderHeat, 200).unwrap();
        host.set(FractalField::OldRenderHeat, 100).unwrap();
        host.set(FractalField::ActiveSparkNum, 42).unwrap();
        host.set(FractalField::SparksLimit, 64).unwrap();
        host.set(FractalField::SparkType, 3).unwrap();
        host.set(FractalField::StarStatus, 1).unwrap();
        host.set(FractalField::Rising, 1).unwrap();
        host.set(FractalField::FireFrequency, 11).unwrap();
        host.set(FractalField::FirePhase, 22).unwrap();
        host.set(FractalField::FireHeat, 33).unwrap();
        host.set(FractalField::FireSize, 44).unwrap();
        host.set(FractalField::FireArea, 55).unwrap();
        host.set(FractalField::FireHorizSpeed, 66).unwrap();
        host.set(FractalField::FireVertSpeed, 77).unwrap();
        assert_eq!(host.get(FractalField::RenderHeat).unwrap(), 200);
        assert_eq!(host.get(FractalField::OldRenderHeat).unwrap(), 100);
        assert_eq!(host.get(FractalField::ActiveSparkNum).unwrap(), 42);
        assert_eq!(host.get(FractalField::SparksLimit).unwrap(), 64);
        assert_eq!(host.get(FractalField::SparkType).unwrap(), 3);
        assert_eq!(host.get(FractalField::StarStatus).unwrap(), 1);
        assert_eq!(host.get(FractalField::Rising).unwrap(), 1);
        assert_eq!(host.get(FractalField::FireFrequency).unwrap(), 11);
        assert_eq!(host.get(FractalField::FirePhase).unwrap(), 22);
        assert_eq!(host.get(FractalField::FireHeat).unwrap(), 33);
        assert_eq!(host.get(FractalField::FireSize).unwrap(), 44);
        assert_eq!(host.get(FractalField::FireArea).unwrap(), 55);
        assert_eq!(host.get(FractalField::FireHorizSpeed).unwrap(), 66);
        assert_eq!(host.get(FractalField::FireVertSpeed).unwrap(), 77);
    }

    // UnFractal.cpp (deleted): FractalField_SparksNum read at line 230
    // (`F->Sparks.Num()`); no Write arm (hit `default: check(0)` on write).
    #[test]
    fn sparks_num_reads_the_tarray_length_and_is_read_only() {
        let mut buf = vec![0u8; 2048];
        let fire = buf.as_mut_ptr() as *mut UFireTexture;
        unsafe {
            (*fire).sparks = TArrayRaw {
                data: std::ptr::null_mut(),
                num: 9,
                max: 9,
            };
        }
        let host = host_over(&mut buf);
        assert_eq!(host.get(FractalField::SparksNum).unwrap(), 9);
    }

    #[test]
    #[should_panic]
    fn sparks_num_write_matches_original_check_0_crash() {
        let mut buf = vec![0u8; 2048];
        let host = host_over(&mut buf);
        let _ = host.set(FractalField::SparksNum, 1);
    }

    // UnFractal.cpp (deleted): FractalField_NumDrops..WaterTime, read 239-252,
    // write 316-329.
    #[test]
    fn water_texture_fields_round_trip() {
        let mut buf = vec![0u8; 8192];
        let host = host_over(&mut buf);
        host.set(FractalField::NumDrops, 5).unwrap();
        host.set(FractalField::DropType, 2).unwrap();
        host.set(FractalField::WaveAmp, 9).unwrap();
        host.set(FractalField::OldWaveAmp, 3).unwrap();
        host.set(FractalField::WaveParity, 1).unwrap();
        host.set(FractalField::SourceFieldsLegacy, 77).unwrap();
        host.set(FractalField::WaterFrequency, 4).unwrap();
        host.set(FractalField::WaterPhase, 6).unwrap();
        host.set(FractalField::WaterAmplitude, 8).unwrap();
        host.set(FractalField::WaterSpeed, 10).unwrap();
        host.set(FractalField::WaterRadius, 12).unwrap();
        host.set(FractalField::WaterSize, 14).unwrap();
        host.set(FractalField::WaterDepth, 16).unwrap();
        host.set(FractalField::WaterTime, 18).unwrap();
        assert_eq!(host.get(FractalField::NumDrops).unwrap(), 5);
        assert_eq!(host.get(FractalField::DropType).unwrap(), 2);
        assert_eq!(host.get(FractalField::WaveAmp).unwrap(), 9);
        assert_eq!(host.get(FractalField::OldWaveAmp).unwrap(), 3);
        assert_eq!(host.get(FractalField::WaveParity).unwrap(), 1);
        assert_eq!(host.get(FractalField::SourceFieldsLegacy).unwrap(), 77);
        assert_eq!(host.get(FractalField::WaterFrequency).unwrap(), 4);
        assert_eq!(host.get(FractalField::WaterPhase).unwrap(), 6);
        assert_eq!(host.get(FractalField::WaterAmplitude).unwrap(), 8);
        assert_eq!(host.get(FractalField::WaterSpeed).unwrap(), 10);
        assert_eq!(host.get(FractalField::WaterRadius).unwrap(), 12);
        assert_eq!(host.get(FractalField::WaterSize).unwrap(), 14);
        assert_eq!(host.get(FractalField::WaterDepth).unwrap(), 16);
        assert_eq!(host.get(FractalField::WaterTime).unwrap(), 18);
    }

    // UnFractal.cpp (deleted): FractalField_BumpMapAngle/BumpMapLight/PhongRange/
    // PhongSize, read 253-256, write 330-333.
    #[test]
    fn wave_texture_fields_round_trip() {
        let mut buf = vec![0u8; 8192];
        let host = host_over(&mut buf);
        host.set(FractalField::BumpMapAngle, 11).unwrap();
        host.set(FractalField::BumpMapLight, 22).unwrap();
        host.set(FractalField::PhongRange, 33).unwrap();
        host.set(FractalField::PhongSize, 44).unwrap();
        assert_eq!(host.get(FractalField::BumpMapAngle).unwrap(), 11);
        assert_eq!(host.get(FractalField::BumpMapLight).unwrap(), 22);
        assert_eq!(host.get(FractalField::PhongRange).unwrap(), 33);
        assert_eq!(host.get(FractalField::PhongSize).unwrap(), 44);
    }

    // UnFractal.cpp (deleted): FractalField_WetSource/WetOldSource/WetLocalLegacy,
    // read 257-259, write 334-336.
    #[test]
    fn wet_texture_fields_round_trip() {
        let mut buf = vec![0u8; 8192];
        let host = host_over(&mut buf);
        host.set(FractalField::WetSource, 0x1000).unwrap();
        host.set(FractalField::WetOldSource, 0x2000).unwrap();
        host.set(FractalField::WetLocalLegacy, 0x3000).unwrap();
        assert_eq!(host.get(FractalField::WetSource).unwrap(), 0x1000);
        assert_eq!(host.get(FractalField::WetOldSource).unwrap(), 0x2000);
        assert_eq!(host.get(FractalField::WetLocalLegacy).unwrap(), 0x3000);
    }

    // UnFractal.cpp (deleted): FractalField_IceSource..VPosition, read 260-279,
    // write 337-356 (includes the MasterCount/UDisplace/VDisplace/UPosition/
    // VPosition `Bits(...)`/`Float(B)` float round trips and the MoveIce bit).
    #[test]
    fn ice_texture_fields_round_trip() {
        let mut buf = vec![0u8; 2048];
        let host = host_over(&mut buf);
        host.set(FractalField::IceSource, 0x10).unwrap();
        host.set(FractalField::IceGlass, 0x20).unwrap();
        host.set(FractalField::IceOldSource, 0x30).unwrap();
        host.set(FractalField::IceOldGlass, 0x40).unwrap();
        host.set(FractalField::IceLocal, 0x50).unwrap();
        host.set(FractalField::OldUDisp, 7).unwrap();
        host.set(FractalField::OldVDisp, 9).unwrap();
        host.set(FractalField::ForceRefresh, 1).unwrap();
        host.set(FractalField::HorizPanSpeed, 3).unwrap();
        host.set(FractalField::VertPanSpeed, 4).unwrap();
        host.set(FractalField::PanningStyle, 2).unwrap();
        host.set(FractalField::Frequency, 5).unwrap();
        host.set(FractalField::Amplitude, 6).unwrap();
        host.set(FractalField::MoveIce, 1).unwrap();
        host.set(FractalField::TimeMethod, 1).unwrap();
        host.set_float(FractalField::MasterCount, 1.5).unwrap();
        host.set_float(FractalField::UDisplace, -2.25).unwrap();
        host.set_float(FractalField::VDisplace, 3.75).unwrap();
        host.set_float(FractalField::UPosition, 0.125).unwrap();
        host.set_float(FractalField::VPosition, 99.0).unwrap();
        assert_eq!(host.get(FractalField::IceSource).unwrap(), 0x10);
        assert_eq!(host.get(FractalField::IceGlass).unwrap(), 0x20);
        assert_eq!(host.get(FractalField::IceOldSource).unwrap(), 0x30);
        assert_eq!(host.get(FractalField::IceOldGlass).unwrap(), 0x40);
        assert_eq!(host.get(FractalField::IceLocal).unwrap(), 0x50);
        assert_eq!(host.get(FractalField::OldUDisp).unwrap(), 7);
        assert_eq!(host.get(FractalField::OldVDisp).unwrap(), 9);
        assert_eq!(host.get(FractalField::ForceRefresh).unwrap(), 1);
        assert_eq!(host.get(FractalField::HorizPanSpeed).unwrap(), 3);
        assert_eq!(host.get(FractalField::VertPanSpeed).unwrap(), 4);
        assert_eq!(host.get(FractalField::PanningStyle).unwrap(), 2);
        assert_eq!(host.get(FractalField::Frequency).unwrap(), 5);
        assert_eq!(host.get(FractalField::Amplitude).unwrap(), 6);
        assert_eq!(host.get(FractalField::MoveIce).unwrap(), 1);
        assert_eq!(host.get(FractalField::TimeMethod).unwrap(), 1);
        assert_eq!(host.float(FractalField::MasterCount).unwrap(), 1.5);
        assert_eq!(host.float(FractalField::UDisplace).unwrap(), -2.25);
        assert_eq!(host.float(FractalField::VDisplace).unwrap(), 3.75);
        assert_eq!(host.float(FractalField::UPosition).unwrap(), 0.125);
        assert_eq!(host.float(FractalField::VPosition).unwrap(), 99.0);
    }

    // UnFractal.cpp (deleted): FractalLeaf_TextureBits/TextureSize/TexturePalette,
    // lines 405-407 (`((UTexture*)A)->UBits/VBits/USize/VSize/Palette`); these took
    // an arbitrary `UTexture*` operand, not `self.owner`.
    #[test]
    fn texture_bits_size_palette_read_an_arbitrary_utexture_pointer() {
        let mut buf = vec![0u8; 512];
        let ptr = buf.as_mut_ptr() as i64;
        let host = host_over(&mut buf);
        host.set(FractalField::UBits, 6).unwrap();
        host.set(FractalField::VBits, 5).unwrap();
        host.set(FractalField::USize, 64).unwrap();
        host.set(FractalField::VSize, 32).unwrap();
        host.set(FractalField::Palette, 0xabc).unwrap();
        assert_eq!(host.texture_bits(ptr, 0).unwrap(), 6);
        assert_eq!(host.texture_bits(ptr, 1).unwrap(), 5);
        assert_eq!(host.texture_size(ptr, 0).unwrap(), 64);
        assert_eq!(host.texture_size(ptr, 1).unwrap(), 32);
        assert_eq!(host.texture_palette(ptr).unwrap(), 0xabc);
    }

    #[test]
    fn texture_and_outer_helpers_reject_null_like_original_source_lock_guards() {
        let mut buf = vec![0u8; 512];
        let host = host_over(&mut buf);
        assert!(host.texture_bits(0, 0).is_err());
        assert!(host.texture_size(0, 0).is_err());
        assert!(host.texture_palette(0).is_err());
        assert!(host.object_outer(0).is_err());
    }

    // UnFractal.cpp (deleted): FractalLeaf_PaletteOuter, line 371
    // (`((UPalette*)A)->GetOuter()`); UObject::GetOuter() is non-virtual.
    #[test]
    fn object_outer_reads_the_object_header_of_an_arbitrary_pointer() {
        let mut buf = vec![0u8; 512];
        buf[48..56].copy_from_slice(&0x55aa_55aau64.to_ne_bytes());
        let ptr = buf.as_mut_ptr() as i64;
        let host = host_over(&mut buf);
        assert_eq!(host.object_outer(ptr).unwrap(), 0x55aa_55aa);
    }

    // UnFractal.cpp (deleted): Hp2FractalNative::Source(), lines 469-478
    // (narrowed to FractalSourceField; the LockSource/UnlockSource selector).
    #[test]
    fn source_field_matches_the_three_lock_unlock_selectors() {
        assert!(matches!(
            source_field(FractalField::WetSource),
            FractalSourceField::WetSource
        ));
        assert!(matches!(
            source_field(FractalField::IceSource),
            FractalSourceField::IceSource
        ));
        assert!(matches!(
            source_field(FractalField::IceGlass),
            FractalSourceField::IceGlass
        ));
    }
}
