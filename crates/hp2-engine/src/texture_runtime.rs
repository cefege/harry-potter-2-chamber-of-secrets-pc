//! Texture object policy. SDK callbacks only access live fields, own typed
//! lifetimes, dispatch virtuals, and capture native exceptions.
use crate::tex;
use std::ffi::c_void;
type Result<T, E = i32> = std::result::Result<T, E>;

/// Applies the pre-version-66 palette fixup after the color array is archived.
///
/// # Safety
/// `data` must contain `count` writable, four-byte native FColor records.
pub(crate) unsafe fn force_palette_opaque(data: *mut u8, count: i32) {
    for index in 0..count {
        unsafe { *data.add(index as usize * 4 + 3) = 255; }
    }
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum TextureRuntimeOp {
    BitmapDefaults,
    TextureDefaults,
    Update,
    Lock,
    DefaultLod,
    Tick,
    Prime,
    ConstantTick,
    Serialize,
    PostLoad,
    Init,
    CreateMips,
    Clear,
    PaletteSerialize,
    PaletteReplace,
    InfoLoad,
    InfoUnload,
    Snap,
    Bmp,
    Factory,
    RippedFixup,
    FactoryDefaults,
    Get,
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum TextureRuntimeField {
    Format,
    Palette,
    UBits,
    VBits,
    USize,
    VSize,
    UClamp,
    VClamp,
    MipZero,
    MaxColor,
    LastTime,
    SelfTexture,
    Diffuse,
    Specular,
    Alpha,
    Scale,
    Friction,
    MipMult,
    Realtime,
    Changed,
    Parametric,
    HasComp,
    CompFormat,
    HighColor,
    HighTexture,
    PrimeCurrent,
    PrimeCount,
    MinRate,
    MaxRate,
    Accumulator,
    AnimCur,
    AnimNext,
    ClientLod,
    HasClient,
    RecommendedLod,
    SupportsTc,
    Deferred,
    HasDevice,
    Lazy,
    Loading,
    Saving,
    Version,
    InfoTexture,
    InfoParametric,
    InfoCount,
    PolyFlags,
    FactoryCreateNew,
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum TextureRuntimeLeaf {
    Read,
    Write,
    Seconds,
    VirtualTick,
    VirtualPrime,
    VirtualConstantTick,
    VirtualUpdate,
    VirtualInit,
    SuperSerialize,
    SuperPostLoad,
    PaletteSuperSerialize,
    PaletteArchive,
    MipCount,
    MipEmpty,
    MipAppend,
    MipAppendSized,
    MipRemoveTail,
    MipCountBytes,
    ArchiveIndex,
    ArchiveMip,
    DataEmpty,
    DataZeroed,
    DataClear,
    DataLoad,
    DataUnload,
    DataView,
    MipDimensions,
    PaletteNew,
    PaletteAppend,
    PaletteView,
    PaletteCount,
    PaletteIterator,
    PaletteAdvance,
    PaletteOuter,
    PaletteLogDelete,
    InfoBegin,
    InfoPublish,
    InfoFlags,
    InfoMip,
    InfoMipLoad,
    InfoMipUnload,
    InfoMipPublish,
    AssignMips,
    CheckInit,
    CheckPalette,
    Compress,
    Decompress,
    RippedScope,
    FactoryClass,
    FactoryFormat,
    SnapView,
    FileLoad,
    FileSentinel,
    ImageView,
    ImageCursor,
    FactoryConstruct,
    FactoryPostLoad,
    FactoryPaletteNew,
    FactoryPalettePublish,
    FactoryWarn,
    FactoryAnimationName,
    FactoryAnimationCopy,
    FactoryAnimationFind,
    FactoryAnimationLink,
    FactoryAnimationLog,
    FactoryCompressParam,
    FactoryResult,
    PaletteEmpty,
}
#[repr(C)]
pub struct TextureRuntimeServices {
    pub call:
        unsafe extern "C" fn(*mut c_void, TextureRuntimeLeaf, usize, usize, *mut usize) -> i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextureRuntimeArgs {
    pub native: *mut c_void,
    pub a: usize,
    pub b: usize,
    pub time: i64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TextureRuntimeData {
    pub data: *mut u8,
    pub len: i32,
    pub u: i32,
    pub v: i32,
}
#[repr(C)]
pub struct TextureRuntimeLock {
    pub lod: i32,
    pub u: i32,
    pub v: i32,
    pub count: i32,
    pub format: i32,
    pub scale: f32,
}
use TextureRuntimeField as F;
use TextureRuntimeLeaf as L;
use TextureRuntimeOp as O;
pub(crate) trait TextureArchiveIo {
    type Error;

    fn is_loading(&self) -> Result<bool, Self::Error>;
    fn is_saving(&self) -> Result<bool, Self::Error>;
    fn is_parametric(&self) -> Result<bool, Self::Error>;
    fn has_compressed_mips(&self) -> Result<bool, Self::Error>;
    fn serialize_super(&mut self) -> Result<(), Self::Error>;
    fn mip_count_bytes(&mut self, compressed: bool) -> Result<(), Self::Error>;
    fn mip_count(&self, compressed: bool) -> Result<i32, Self::Error>;
    fn empty_mips(&mut self, compressed: bool, reserve: i32) -> Result<(), Self::Error>;
    fn append_mip(&mut self, compressed: bool) -> Result<(), Self::Error>;
    fn archive_mip(&mut self, compressed: bool, index: i32) -> Result<(), Self::Error>;
    fn archive_mip_index(&mut self, count: Option<i32>) -> Result<i32, Self::Error>;
    fn client_lod(&self) -> Result<i32, Self::Error>;
    fn lazy_load(&self) -> Result<bool, Self::Error>;
    fn set_lazy_load(&mut self, lazy: bool) -> Result<(), Self::Error>;
    fn parametric_data_empty(&mut self, index: i32) -> Result<(), Self::Error>;
    fn mip_dimensions(&self, index: i32) -> Result<(i32, i32), Self::Error>;
    fn parametric_data_empty_sized(&mut self, index: i32, len: i32)
        -> Result<(), Self::Error>;
    fn parametric_data_zeroed(&mut self, index: i32, len: i32) -> Result<(), Self::Error>;
    fn post_load_super(&mut self) -> Result<(), Self::Error>;
    fn palette_exists(&self) -> Result<bool, Self::Error>;
    fn create_palette(&mut self) -> Result<(), Self::Error>;
    fn append_gray_palette_entry(&mut self, gray: u8, alpha: u8) -> Result<(), Self::Error>;
    fn u_clamp(&self) -> Result<i32, Self::Error>;
    fn set_u_clamp(&mut self, value: i32) -> Result<(), Self::Error>;
    fn u_size(&self) -> Result<i32, Self::Error>;
    fn v_clamp(&self) -> Result<i32, Self::Error>;
    fn set_v_clamp(&mut self, value: i32) -> Result<(), Self::Error>;
    fn v_size(&self) -> Result<i32, Self::Error>;
    fn reset_accumulator(&mut self) -> Result<(), Self::Error>;
    fn seconds(&mut self) -> Result<i64, Self::Error>;
    fn set_last_time(&mut self, value: i64) -> Result<(), Self::Error>;
}

pub(crate) fn serialize_mips<I: TextureArchiveIo>(
    io: &mut I,
    compressed: bool,
) -> Result<(), I::Error> {
    io.mip_count_bytes(compressed)?;
    let loading = io.is_loading()?;
    if loading {
        let count = io.archive_mip_index(None)?;
        io.empty_mips(compressed, count)?;
        let lod = io.client_lod()?.min(count - 1);
        for index in 0..count {
            let saved_lazy = io.lazy_load()?;
            if index < lod {
                io.set_lazy_load(true)?;
            }
            io.append_mip(compressed)?;
            io.archive_mip(compressed, index)?;
            io.set_lazy_load(saved_lazy)?;
        }
    } else {
        let count = io.mip_count(compressed)?;
        io.archive_mip_index(Some(count))?;
        let mut index = 0;
        while index < io.mip_count(compressed)? {
            io.archive_mip(compressed, index)?;
            index += 1;
        }
    }
    Ok(())
}

pub(crate) fn serialize_texture<I: TextureArchiveIo>(io: &mut I) -> Result<(), I::Error> {
    io.serialize_super()?;
    let serialize_data = (io.is_saving()? || io.is_loading()?) && io.is_parametric()?;
    if serialize_data {
        let mut index = 0;
        while index < io.mip_count(false)? {
            io.parametric_data_empty(index)?;
            index += 1;
        }
    }
    serialize_mips(io, false)?;
    if io.has_compressed_mips()? {
        serialize_mips(io, true)?;
    }
    if serialize_data {
        let mut index = 0;
        while index < io.mip_count(false)? {
            let (u, v) = io.mip_dimensions(index)?;
            let len = u.wrapping_mul(v);
            io.parametric_data_empty_sized(index, len)?;
            io.parametric_data_zeroed(index, len)?;
            index += 1;
        }
    }
    Ok(())
}

pub(crate) fn post_load<I: TextureArchiveIo>(io: &mut I) -> Result<(), I::Error> {
    io.post_load_super()?;
    if !io.palette_exists()? {
        io.create_palette()?;
        for gray in 0..=u8::MAX {
            io.append_gray_palette_entry(gray, 0)?;
        }
    }
    io.set_u_clamp(io.u_clamp()?.max(0).min(io.u_size()?))?;
    io.set_v_clamp(io.v_clamp()?.max(0).min(io.v_size()?))?;
    io.reset_accumulator()?;
    let now = io.seconds()?;
    io.set_last_time(now)
}
struct Flow<'a> {
    s: &'a TextureRuntimeServices,
    a: TextureRuntimeArgs,
}
impl Flow<'_> {
    fn call(&self, l: L, a: usize, b: usize) -> Result<usize> {
        let mut v = 0;
        let e = unsafe { (self.s.call)(self.a.native, l, a, b, &mut v) };
        if e == 0 { Ok(v) } else { Err(e) }
    }
    fn leaf(&self, l: L) -> Result<usize> {
        self.call(l, 0, 0)
    }
    fn get(&self, f: F) -> Result<usize> {
        self.call(L::Read, f as usize, 0)
    }
    fn set(&self, f: F, v: usize) -> Result<()> {
        self.call(L::Write, f as usize, v)?;
        Ok(())
    }
    fn int(&self, f: F) -> Result<i32> {
        Ok(self.get(f)? as i32)
    }
    fn float(&self, f: F) -> Result<f32> {
        Ok(f32::from_bits(self.get(f)? as u32))
    }
    fn put_float(&self, f: F, v: f32) -> Result<()> {
        self.set(f, v.to_bits() as usize)
    }
    fn count(&self, comp: usize) -> Result<i32> {
        Ok(self.call(L::MipCount, comp, 0)? as i32)
    }
    fn data(&self, comp: usize, i: i32) -> Result<TextureRuntimeData> {
        let mut d = TextureRuntimeData::default();
        self.call(
            L::DataView,
            (comp << 32) | (i as u32 as usize),
            &mut d as *mut _ as usize,
        )?;
        Ok(d)
    }
    fn dimensions(&self, comp: usize, i: i32) -> Result<TextureRuntimeData> {
        let mut d = TextureRuntimeData::default();
        self.call(
            L::MipDimensions,
            (comp << 32) | (i as u32 as usize),
            &mut d as *mut _ as usize,
        )?;
        Ok(d)
    }
    fn run(&mut self, op: O) -> Result<usize> {
        match op {
            O::BitmapDefaults => {
                for (f, v) in [
                    (F::Format, 0),
                    (F::Palette, 0),
                    (F::UBits, 0),
                    (F::VBits, 0),
                    (F::USize, 0),
                    (F::VSize, 0),
                    (F::MipZero, 0x00408040),
                    (F::MaxColor, 0xffffffff),
                ] {
                    self.set(f, v)?;
                }
                self.set(F::LastTime, self.leaf(L::Seconds)?)?;
            }
            O::TextureDefaults => {
                for f in [F::Diffuse, F::Specular, F::Scale, F::Friction, F::MipMult] {
                    self.put_float(f, 1.)?;
                }
                self.put_float(F::Alpha, 0.)?;
            }
            O::Update => {
                let last = self.get(F::LastTime)? as i64;
                if self.a.time != last {
                    if self.get(F::Realtime)? != 0 {
                        self.set(F::Changed, 1)?;
                    }
                    let delta = (self.a.time.wrapping_sub(self.get(F::LastTime)? as i64) as f32)
                        / 4294967296.;
                    self.call(L::VirtualTick, delta.to_bits() as usize, 0)?;
                    self.set(F::LastTime, self.a.time as usize)?;
                }
            }
            O::Tick => {
                self.leaf(L::VirtualPrime)?;
                let mut acc = self.float(F::Accumulator)?;
                let ticks = tex::tick_plan(
                    self.float(F::MaxRate)?,
                    self.float(F::MinRate)?,
                    &mut acc,
                    f32::from_bits(self.a.a as u32),
                );
                self.put_float(F::Accumulator, acc)?;
                for _ in 0..ticks {
                    self.leaf(L::VirtualConstantTick)?;
                }
            }
            O::Prime => {
                let n = tex::prime_steps(
                    self.get(F::PrimeCurrent)? as u8,
                    self.get(F::PrimeCount)? as u8,
                );
                for _ in 0..n {
                    self.set(
                        F::PrimeCurrent,
                        self.get(F::PrimeCurrent)?.wrapping_add(1) & 255,
                    )?;
                    self.leaf(L::VirtualConstantTick)?;
                }
            }
            O::ConstantTick => {
                if self.get(F::AnimCur)? == 0 {
                    self.set(F::AnimCur, self.get(F::SelfTexture)?)?;
                }
                let next = self.call(L::Read, F::AnimNext as usize, self.get(F::AnimCur)?)?;
                self.set(F::AnimCur, next)?;
                if next == 0 {
                    self.set(F::AnimCur, self.get(F::SelfTexture)?)?;
                }
            }
            O::DefaultLod => {
                return Ok(if self.get(F::HasClient)? != 0 {
                    self.int(F::ClientLod)?.min(self.count(0)? - 1) as usize
                } else {
                    0
                });
            }
            O::Serialize => serialize_texture(self)?,
            O::PostLoad => post_load(self)?,
            O::Init => {
                self.leaf(L::CheckInit)?;
                self.set(F::USize, self.a.a)?;
                self.set(F::UClamp, self.a.a)?;
                self.set(F::VSize, self.a.b)?;
                self.set(F::VClamp, self.a.b)?;
                self.set(
                    F::UBits,
                    hp2_core::bits::ceil_log_two(self.get(F::USize)? as u32) as usize,
                )?;
                self.set(
                    F::VBits,
                    hp2_core::bits::ceil_log_two(self.get(F::VSize)? as u32) as usize,
                )?;
                self.call(L::MipEmpty, 0, 0)?;
                let bits = (self.get(F::UBits)? << 8) | self.get(F::VBits)?;
                if self.get(F::Format)? == 5 {
                    self.call(
                        L::MipAppendSized,
                        bits,
                        self.int(F::USize)?
                            .wrapping_mul(self.int(F::VSize)?)
                            .wrapping_mul(4) as usize,
                    )?;
                } else {
                    self.call(L::MipAppend, 0, bits)?;
                }
                self.call(L::DataClear, 0, 0)?;
            }
            O::CreateMips => self.create_mips()?,
            O::Clear => {
                if self.a.a & 2 != 0 {
                    let mut i = 0;
                    while i < self.count(0)? {
                        self.call(L::DataClear, i as usize, 0)?;
                        i += 1;
                    }
                }
            }
            O::PaletteSerialize => {
                self.leaf(L::PaletteSuperSerialize)?;
                self.leaf(L::PaletteArchive)?;
                if self.int(F::Version)? < 66 {
                    let mut d = TextureRuntimeData::default();
                    self.call(L::PaletteView, 0, &mut d as *mut _ as usize)?;
                    unsafe { force_palette_opaque(d.data, d.len); }
                }
            }
            O::PaletteReplace => {
                let owner = self.leaf(L::PaletteOuter)?;
                let mut p = self.leaf(L::PaletteIterator)?;
                while p != 0 {
                    if p != self.get(F::Palette)? && self.call(L::PaletteOuter, p, 0)? == owner {
                        let mut a = TextureRuntimeData::default();
                        let mut b = a;
                        self.call(L::PaletteView, 0, &mut a as *mut _ as usize)?;
                        self.call(L::PaletteView, p, &mut b as *mut _ as usize)?;
                        let mut equal = true;
                        for i in 0..256 {
                            let (left, right) = unsafe {
                                (
                                    std::ptr::read_unaligned(a.data.add(i * 4) as *const u32),
                                    std::ptr::read_unaligned(b.data.add(i * 4) as *const u32),
                                )
                            };
                            if left != right {
                                equal = false;
                                break;
                            }
                        }
                        if equal {
                            return self.call(L::PaletteLogDelete, p, 0);
                        }
                    }
                    p = self.leaf(L::PaletteAdvance)?;
                }
                return self.leaf(L::FactoryResult);
            }
            O::InfoLoad | O::InfoUnload => {
                if self.get(F::InfoTexture)? != 0 && self.get(F::InfoParametric)? == 0 {
                    let mut i = 0;
                    while i < self.int(F::InfoCount)? {
                        let mip = self.call(L::InfoMip, i as usize, 0)?;
                        if matches!(op, O::InfoLoad) {
                            self.call(L::InfoMipLoad, mip, 0)?;
                            self.call(L::InfoMipPublish, mip, 0)?;
                        } else {
                            self.call(L::InfoMipUnload, mip, 0)?;
                        }
                        i += 1;
                    }
                }
            }
            O::Lock => {
                let comp = (self.get(F::HasComp)? != 0
                    && self.get(F::HasDevice)? != 0
                    && self.get(F::SupportsTc)? != 0
                    && self.get(F::CompFormat)? == 3) as usize;
                if self.get(F::Parametric)? == 0 && self.count(comp)? == 0 {
                    self.call(L::RippedScope, comp, 0)?;
                }
                let mut lod = self.a.a as i32;
                if lod == -1 {
                    lod = self.int(F::ClientLod)?;
                    if self.get(F::HasDevice)? != 0 {
                        lod = lod.max(self.int(F::RecommendedLod)?);
                    }
                }
                lod = lod.min((self.count(comp)? - 1).min(3));
                self.call(L::InfoBegin, lod as usize, 0)?;
                let base = self.dimensions(0, 0)?;
                let mip = self.dimensions(comp, lod)?;
                let info = TextureRuntimeLock {
                    lod,
                    u: mip.u,
                    v: mip.v,
                    count: self.count(comp)? - lod,
                    format: self.int(if comp != 0 { F::CompFormat } else { F::Format })?,
                    scale: self.float(F::Scale)? * (base.u as f32 / mip.u as f32),
                };
                self.call(L::InfoPublish, &info as *const _ as usize, 0)?;
                if self.get(F::Parametric)? == 0
                    && (self.get(F::HasDevice)? == 0 || self.get(F::Deferred)? == 0)
                {
                    let mut i = lod;
                    while i < self.count(comp)? {
                        self.call(L::DataLoad, comp, i as usize)?;
                        i += 1;
                    }
                }
                let mut i = lod;
                while i < self.count(comp)? {
                    self.call(
                        L::InfoMipPublish,
                        (comp << 32) | (i as u32 as usize),
                        (i - lod) as usize + 1,
                    )?;
                    i += 1;
                }
                if self.a.time != 0 {
                    self.call(L::VirtualUpdate, self.a.time as usize, 0)?;
                }
                self.leaf(L::InfoFlags)?;
                self.set(F::Changed, 0)?;
            }
            O::RippedFixup => {
                self.call(L::AssignMips, self.a.a, 0)?;
                self.call(L::VirtualInit, self.get(F::USize)?, self.get(F::VSize)?)?;
            }
            O::Get => {
                self.call(L::VirtualUpdate, self.a.time as usize, 0)?;
                let current = self.get(F::AnimCur)?;
                return if current != 0 {
                    Ok(current)
                } else {
                    self.get(F::SelfTexture)
                };
            }
            O::FactoryDefaults => {
                self.leaf(L::FactoryClass)?;
                self.leaf(L::FactoryFormat)?;
                self.set(F::FactoryCreateNew, 0)?;
            }
            O::Snap => return self.snap(),
            O::Bmp => return self.bmp(),
            O::Factory => return self.factory(),
        }
        Ok(0)
    }
    fn create_mips(&self) -> Result<()> {
        self.leaf(L::CheckPalette)?;
        let mut palette = TextureRuntimeData::default();
        self.call(L::PaletteView, 0, &mut palette as *mut _ as usize)?;
        let base = self.data(0, 0)?;
        let p8 = self.get(F::Format)? == 0;
        let zero = unsafe {
            let colors = std::slice::from_raw_parts(palette.data as *const hp2_abi::FColor, 256);
            let bytes = if base.len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(base.data, base.len as usize)
            };
            tex::color_to_int4(tex::mip_zero(colors, bytes, p8))
        };
        self.set(F::MipZero, zero as usize)?;
        if self.count(0)? > 1 {
            self.leaf(L::MipRemoveTail)?;
        }
        if self.get(F::Format)? == 5 {
            self.call(L::Compress, self.a.a, 0)?;
            self.set(F::HasComp, 1)?;
            self.leaf(L::Decompress)?;
        } else if self.a.a != 0 {
            while self.int(F::UBits)? - self.count(0)? >= 0
                || self.int(F::VBits)? - self.count(0)? >= 0
            {
                let n = self.count(0)?;
                let u = (self.int(F::UBits)? - n).max(0);
                let v = (self.int(F::VBits)? - n).max(0);
                self.call(L::MipAppend, 0, ((u as usize) << 8) | v as usize)?;
            }
            if self.a.b != 0 {
                let count = self.count(0)? as usize;
                let masked = self.get(F::PolyFlags)? & 2 != 0;
                let mut descs = [TextureRuntimeData::default(); 32];
                for (i, d) in descs[..count].iter_mut().enumerate() {
                    *d = self.data(0, i as i32)?;
                }
                // The original caches the palette before constructing/removing mip arrays.
                // No native callbacks occur while these mutable slices exist.
                unsafe {
                    let colors =
                        std::slice::from_raw_parts(palette.data as *const hp2_abi::FColor, 256);
                    let mut levels: [tex::MipLevel<'_>; 32] =
                        std::array::from_fn(|_| tex::MipLevel {
                            data: &mut [],
                            size_u: 0,
                            size_v: 0,
                        });
                    for i in 0..count {
                        levels[i] = tex::MipLevel {
                            data: if descs[i].len == 0 {
                                &mut []
                            } else {
                                std::slice::from_raw_parts_mut(descs[i].data, descs[i].len as usize)
                            },
                            size_u: descs[i].u,
                            size_v: descs[i].v,
                        };
                    }
                    tex::downsample_p8(colors, masked, &mut levels[..count]);
                }
            }
        }
        Ok(())
    }
    fn snap(&self) -> Result<usize> {
        self.set(F::Format, 5)?;
        self.call(L::VirtualInit, self.a.a, self.a.b)?;
        let mut src = TextureRuntimeData::default();
        self.call(L::SnapView, 0, &mut src as *mut _ as usize)?;
        let dst = self.data(0, 0)?;
        let u = (self.a.a as u32).min(src.u as u32);
        let v = (self.a.b as u32).min(src.v as u32);
        let x = (src.u as u32 - u) / 2;
        for y in 0..v {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src.data.add(((y * src.u as u32 + x) * 4) as usize),
                    dst.data.add((y * self.a.a as u32 * 4) as usize),
                    (u * 4) as usize,
                );
            }
        }
        self.set(F::Changed, 1)?;
        Ok(1)
    }
    fn image(&self) -> Result<TextureRuntimeData> {
        let mut d = TextureRuntimeData::default();
        self.call(L::ImageView, 0, &mut d as *mut _ as usize)?;
        Ok(d)
    }
    fn bmp(&self) -> Result<usize> {
        if self.leaf(L::FileLoad)? == 0 {
            return Ok(0);
        }
        self.leaf(L::FileSentinel)?;
        let image = self.image()?;
        let h = ImageHeader::read(image);
        if !h.bmp {
            return Ok(0);
        }
        let u = 1u32.wrapping_shl(32 - h.u.leading_zeros());
        let v = 1u32.wrapping_shl(32 - h.v.leading_zeros());
        self.set(F::Format, 5)?;
        self.call(L::VirtualInit, u as usize, v as usize)?;
        if h.compression != 0 {
            self.call(L::FactoryWarn, 1, 0)?;
            return Ok(0);
        }
        if h.planes != 1 || h.bits != 24 {
            self.call(
                L::FactoryWarn,
                2,
                ((h.planes as usize) << 16) | h.bits as usize,
            )?;
            return Ok(0);
        }
        copy_bmp(image.data, h, self.data(0, 0)?.data, u);
        self.set(F::Changed, 1)?;
        Ok(1)
    }
    fn factory(&self) -> Result<usize> {
        let image = self.image()?;
        let h = ImageHeader::read(image);
        let pcx = !h.bmp && image.len >= 128 && unsafe { *image.data } == 10;
        if !h.bmp && !pcx {
            self.call(L::FactoryWarn, 3, 0)?;
            return Ok(0);
        }
        let (u, v, planes, bits) = if h.bmp {
            (h.u, h.v, h.planes, h.bits)
        } else {
            unsafe {
                (
                    word(image.data, 8) as u32 + 1 - word(image.data, 4) as u32,
                    word(image.data, 10) as u32 + 1 - word(image.data, 6) as u32,
                    *image.data.add(65) as u16,
                    *image.data.add(3) as u16,
                )
            }
        };
        if u & (u.wrapping_sub(1)) != 0 || v & (v.wrapping_sub(1)) != 0 {
            self.call(L::FactoryWarn, 0, 0)?;
            return Ok(0);
        }
        if h.bmp && h.compression != 0 {
            self.call(L::FactoryWarn, 1, 0)?;
            return Ok(0);
        }
        let indexed = planes == 1 && bits == 8;
        let rgba = if h.bmp {
            planes == 1 && bits == 24
        } else {
            planes == 3 && bits == 8
        };
        if !indexed && !rgba {
            self.call(
                L::FactoryWarn,
                2,
                ((h.planes as usize) << 16) | h.bits as usize,
            )?;
            return Ok(0);
        }
        // File-owned views must cover every byte the source decoder reads.
        // Keep the source's RGB addressing (including its row-padding behavior).
        if h.bmp {
            let row_bytes = if indexed {
                let stride = (u as usize).checked_add(3).ok_or(1)? & !3;
                (v.saturating_sub(1) as usize).checked_mul(stride).and_then(|n| n.checked_add(u as usize)).ok_or(1)?
            } else {
                (u as usize).checked_mul(v as usize).and_then(|n| n.checked_mul(3)).ok_or(1)?
            };
            if (h.offset as usize).checked_add(row_bytes).ok_or(1)? > image.len as usize {
                return Err(1);
            }
            if indexed {
                let entries = if h.used != 0 { (h.used as i32).min(256).max(0) } else { 256 };
                if BMP_FILE_HEADER + BMP_INFO_HEADER + entries as usize * 4 > image.len as usize {
                    return Err(1);
                }
            }
        }
        self.leaf(L::FactoryConstruct)?;
        if rgba {
            self.set(F::Format, 5)?;
        }
        self.call(L::VirtualInit, u as usize, v as usize)?;
        self.leaf(L::FactoryPostLoad)?;
        if h.bmp {
            if indexed {
                self.leaf(L::FactoryPaletteNew)?;
                self.leaf(L::PaletteEmpty)?;
                let used = ImageHeader::read(image).used as i32;
                let n = if used != 0 { used.min(256) } else { 256 };
                for i in 0..n {
                    let p = unsafe {
                        image
                            .data
                            .add(BMP_FILE_HEADER + BMP_INFO_HEADER + i as usize * 4)
                    };
                    let c = unsafe {
                        (*p.add(2) as usize)
                            | ((*p.add(1) as usize) << 8)
                            | ((*p as usize) << 16)
                            | 0xff000000
                    };
                    self.call(L::PaletteAppend, c, 0)?;
                }
                while self.leaf(L::PaletteCount)? < 256 {
                    self.call(L::PaletteAppend, 0xff000000, 0)?;
                }
                self.leaf(L::FactoryPalettePublish)?;
            }
            let mut live = ImageHeader::read(image);
            live.bits = if indexed { 8 } else { 24 };
            let destination = self.data(0, 0)?;
            let required = (u as usize).checked_mul(v as usize).and_then(|n| n.checked_mul(if indexed { 1 } else { 4 })).ok_or(1)?;
            if destination.len < 0 || required > destination.len as usize { return Err(1); }
            copy_bmp(image.data, live, destination.data, live.u);
        } else {
            let dst = self.data(0, 0)?;
            let required = (u as usize).checked_mul(v as usize).and_then(|n| n.checked_mul(if indexed { 1 } else { 4 })).ok_or(1)?;
            if dst.len < 0 || required > dst.len as usize { return Err(1); }
            let bytes = unsafe { std::slice::from_raw_parts(image.data, image.len as usize) };
            let mut cursor = 128usize;
            let slot = self.leaf(L::ImageCursor)? as *mut *const u8;
            unsafe {
                *slot = image.data.add(cursor);
            }
            if indexed {
                let mut out = 0usize;
                while out < dst.len as usize {
                    let color = *bytes.get(cursor).ok_or(1)?;
                    cursor += 1;
                    unsafe {
                        *slot = image.data.add(cursor);
                    }
                    if color & 0xc0 == 0xc0 {
                        let count = (color & 0x3f) as usize;
                        let color = *bytes.get(cursor).ok_or(1)?;
                        cursor += 1;
                        unsafe {
                            *slot = image.data.add(cursor);
                            std::ptr::write_bytes(
                                dst.data.add(out),
                                color,
                                count.min(dst.len as usize - out),
                            );
                        }
                        out += count;
                    } else {
                        unsafe {
                            *dst.data.add(out) = color;
                        }
                        out += 1;
                    }
                }
                self.leaf(L::FactoryPaletteNew)?;
                self.leaf(L::PaletteEmpty)?;
                if image.len < 768 { return Err(1); }
                for i in 0..256 {
                    let p = unsafe { image.data.add(image.len as usize - 768 + i * 3) };
                    let c = unsafe {
                        (*p as usize)
                            | ((*p.add(1) as usize) << 8)
                            | ((*p.add(2) as usize) << 16)
                            | 0xff000000
                    };
                    self.call(L::PaletteAppend, c, 0)?;
                }
                self.leaf(L::FactoryPalettePublish)?;
            } else {
                let count = unsafe { word(image.data, 66) as u32 }.min(u);
                // The supported SDK's three-plane loop consumes the marker itself as colour,
                // and advances j by one, not by run length. Preserve that defined behavior.
                for y in 0..v {
                    for plane in (0..3).rev() {
                        for x in 0..count {
                            let color = *bytes.get(cursor).ok_or(1)?;
                            let n = if color & 0xc0 == 0xc0 {
                                ((color & 0x3f) as u32).min(count - x)
                            } else {
                                1
                            };
                            cursor += 1;
                            unsafe {
                                *slot = image.data.add(cursor);
                            }
                            for k in x..x + n {
                                unsafe {
                                    *dst.data.add(((y * u + k) * 4 + plane) as usize) = color;
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut name = TextureRuntimeData::default();
        self.call(L::FactoryAnimationName, 0, &mut name as *mut _ as usize)?;
        if name.len >= 4 {
            self.call(L::FactoryAnimationCopy, 0, &mut name as *mut _ as usize)?;
            let p = name.data as *mut u32;
            let n = name.len as usize;
            let end = n - 4;
            let a = unsafe { *p.add(end + 1) };
            let d0 = unsafe { *p.add(end + 2) };
            let d1 = unsafe { *p.add(end + 3) };
            if unsafe { *p.add(end) } == 95
                && (a == 65 || a == 97)
                && (48..=57).contains(&d0)
                && (48..=57).contains(&d1)
            {
                let frame = ((d0 - 48) * 10 + d1 - 48) as usize;
                self.call(L::FactoryAnimationLog, frame, 0)?;
                if frame > 0 {
                    let other = self.call(L::FactoryAnimationFind, frame - 1, end + 2)?;
                    if other != 0 {
                        self.call(L::FactoryAnimationLink, other, 0)?;
                    }
                }
                if frame < 99 {
                    let other = self.call(L::FactoryAnimationFind, frame + 1, end + 2)?;
                    if other != 0 {
                        self.call(L::FactoryAnimationLink, other, 1)?;
                    }
                }
            }
        }
        if self.leaf(L::FactoryCompressParam)? != 0 {
            self.call(L::Compress, 1, 0)?;
        }
        let mut masked = false;
        if self.get(F::Palette)? != 0 {
            let mut p = TextureRuntimeData::default();
            self.call(L::PaletteView, 0, &mut p as *mut _ as usize)?;
            if p.len != 0 && unsafe { std::ptr::read_unaligned(p.data as *const u32) } == 0xffff00ff
            {
                let d = self.data(0, 0)?;
                masked =
                    unsafe { std::slice::from_raw_parts(d.data, d.u.wrapping_mul(d.v) as usize) }
                        .contains(&0);
            }
        }
        let flags = self.get(F::PolyFlags)?;
        self.set(F::PolyFlags, if masked { flags | 2 } else { flags & !2 })?;
        self.leaf(L::FactoryResult)
    }
}
impl TextureArchiveIo for Flow<'_> {
    type Error = i32;

    fn is_loading(&self) -> Result<bool> {
        Ok(self.get(F::Loading)? != 0)
    }
    fn is_saving(&self) -> Result<bool> {
        Ok(self.get(F::Saving)? != 0)
    }
    fn is_parametric(&self) -> Result<bool> {
        Ok(self.get(F::Parametric)? != 0)
    }
    fn has_compressed_mips(&self) -> Result<bool> {
        Ok(self.get(F::HasComp)? != 0)
    }
    fn serialize_super(&mut self) -> Result<()> {
        self.leaf(L::SuperSerialize)?;
        Ok(())
    }
    fn mip_count_bytes(&mut self, compressed: bool) -> Result<()> {
        self.call(L::MipCountBytes, compressed as usize, 0)?;
        Ok(())
    }
    fn mip_count(&self, compressed: bool) -> Result<i32> {
        self.count(compressed as usize)
    }
    fn empty_mips(&mut self, compressed: bool, reserve: i32) -> Result<()> {
        self.call(L::MipEmpty, compressed as usize, reserve as usize)?;
        Ok(())
    }
    fn append_mip(&mut self, compressed: bool) -> Result<()> {
        self.call(L::MipAppend, compressed as usize, usize::MAX)?;
        Ok(())
    }
    fn archive_mip(&mut self, compressed: bool, index: i32) -> Result<()> {
        self.call(L::ArchiveMip, compressed as usize, index as usize)?;
        Ok(())
    }
    fn archive_mip_index(&mut self, count: Option<i32>) -> Result<i32> {
        let (saving, count) = count.map_or((false, 0), |count| (true, count));
        Ok(self.call(L::ArchiveIndex, saving as usize, count as usize)? as i32)
    }
    fn client_lod(&self) -> Result<i32> {
        self.int(F::ClientLod)
    }
    fn lazy_load(&self) -> Result<bool> {
        Ok(self.get(F::Lazy)? != 0)
    }
    fn set_lazy_load(&mut self, lazy: bool) -> Result<()> {
        self.set(F::Lazy, lazy as usize)
    }
    fn parametric_data_empty(&mut self, index: i32) -> Result<()> {
        self.call(L::DataEmpty, index as usize, 0)?;
        Ok(())
    }
    fn mip_dimensions(&self, index: i32) -> Result<(i32, i32)> {
        let dimensions = self.dimensions(0, index)?;
        Ok((dimensions.u, dimensions.v))
    }
    fn parametric_data_empty_sized(&mut self, index: i32, len: i32) -> Result<()> {
        self.call(L::DataEmpty, index as usize, len as usize)?;
        Ok(())
    }
    fn parametric_data_zeroed(&mut self, index: i32, len: i32) -> Result<()> {
        self.call(L::DataZeroed, index as usize, len as usize)?;
        Ok(())
    }
    fn post_load_super(&mut self) -> Result<()> {
        self.leaf(L::SuperPostLoad)?;
        Ok(())
    }
    fn palette_exists(&self) -> Result<bool> {
        Ok(self.get(F::Palette)? != 0)
    }
    fn create_palette(&mut self) -> Result<()> {
        self.leaf(L::PaletteNew)?;
        Ok(())
    }
    fn append_gray_palette_entry(&mut self, gray: u8, alpha: u8) -> Result<()> {
        let color = usize::from(gray) * 0x010101 | (usize::from(alpha) << 24);
        self.call(L::PaletteAppend, color, 0)?;
        Ok(())
    }
    fn u_clamp(&self) -> Result<i32> {
        self.int(F::UClamp)
    }
    fn set_u_clamp(&mut self, value: i32) -> Result<()> {
        self.set(F::UClamp, value as usize)
    }
    fn u_size(&self) -> Result<i32> {
        self.int(F::USize)
    }
    fn v_clamp(&self) -> Result<i32> {
        self.int(F::VClamp)
    }
    fn set_v_clamp(&mut self, value: i32) -> Result<()> {
        self.set(F::VClamp, value as usize)
    }
    fn v_size(&self) -> Result<i32> {
        self.int(F::VSize)
    }
    fn reset_accumulator(&mut self) -> Result<()> {
        self.put_float(F::Accumulator, 0.)
    }
    fn seconds(&mut self) -> Result<i64> {
        Ok(self.leaf(L::Seconds)? as i64)
    }
    fn set_last_time(&mut self, value: i64) -> Result<()> {
        self.set(F::LastTime, value as usize)
    }
}

pub fn run(
    s: &TextureRuntimeServices,
    a: TextureRuntimeArgs,
    op: TextureRuntimeOp,
) -> Result<usize> {
    let mut flow = Flow { s, a };
    flow.run(op)
}

#[derive(Clone, Copy, Default)]
struct ImageHeader {
    bmp: bool,
    u: u32,
    v: u32,
    planes: u16,
    bits: u16,
    compression: u32,
    used: u32,
    offset: u32,
}
/// `BITMAPFILEHEADER` on disk: `bfType`, `bfSize`, two reserved words,
/// `bfOffBits` at byte 10. The C++ struct mirror lost `#pragma pack(1)` outside
/// MSVC and reported 16, shifting every field read below.
const BMP_FILE_HEADER: usize = 14;
/// `BITMAPINFOHEADER` on disk.
const BMP_INFO_HEADER: usize = 40;
unsafe fn word(p: *const u8, n: usize) -> u16 {
    unsafe { u16::from_le(std::ptr::read_unaligned(p.add(n) as *const u16)) }
}
unsafe fn dword(p: *const u8, n: usize) -> u32 {
    unsafe { u32::from_le(std::ptr::read_unaligned(p.add(n) as *const u32)) }
}
impl ImageHeader {
    fn read(d: TextureRuntimeData) -> Self {
        if d.len < (BMP_FILE_HEADER + BMP_INFO_HEADER) as i32 {
            return Self::default();
        }
        unsafe {
            let p = d.data.add(BMP_FILE_HEADER);
            Self {
                bmp: *d.data == b'B' && *d.data.add(1) == b'M',
                u: dword(p, 4),
                v: dword(p, 8),
                planes: word(p, 12),
                bits: word(p, 14),
                compression: dword(p, 16),
                used: dword(p, 32),
                offset: dword(d.data, 10),
            }
        }
    }
}
fn copy_bmp(src: *const u8, h: ImageHeader, dst: *mut u8, stride: u32) {
    for y in 0..h.v {
        unsafe {
            if h.bits == 8 {
                std::ptr::copy_nonoverlapping(
                    src.add((h.offset + y * ((h.u + 3) & !3)) as usize),
                    dst.add(((h.v - 1 - y) * stride) as usize),
                    h.u as usize,
                );
            } else {
                for x in 0..h.u {
                    let p = src.add((h.offset + (y * h.u + x) * 3) as usize);
                    let q = dst.add((((h.v - 1 - y) * stride + x) * 4) as usize);
                    std::ptr::copy_nonoverlapping(p, q, 3);
                    *q.add(3) = 255;
                }
            }
        }
    }
}
