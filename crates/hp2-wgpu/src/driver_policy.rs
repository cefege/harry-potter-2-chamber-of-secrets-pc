//! Remaining UWgpuRenderDevice policy. Native callbacks are narrow SDK field,
//! typed-storage, logging and provider leaves. All branches, live traversal,
//! triangle assembly, texture selection and lifecycle ordering execute here.
//! No callback table or native field is borrowed exclusively across reentry.
use core::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2WgpuDriverValue {
    pub integer: i64,
    pub real: f64,
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2WgpuDriverAction {
    UploadTexture,
    StaticConstructor,
    Init,
    SetRes,
    Exit,
    Flush,
    Lock,
    Unlock,
    SetSceneNode,
    MaxVertices,
    DrawTriangles,
    DrawTile,
    Draw2DLine,
    Draw2DPoint,
    EndFlash,
    ClearZ,
    DrawComplexSurface,
    CreateCanvasTextLayoutBody,
    CreateCanvasTextLayout,
    DestroyCanvasTextLayout,
    MeasureCanvasText,
    DrawCanvasTextBody,
    DrawCanvasText,
    CompressedBody,
    TrianglesBody,
    ComplexBody,
}

/// Each action accepts only the leaves documented by its native SDK adapter.
/// Integer/real outputs are live reads; A/B select native array entries and X
/// supplies a computed scalar. Native storage never escapes the invocation.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2WgpuDriverLeaf {
    FixCache,
    Cached,
    Dirty,
    ResetDirty,
    Texture,
    UClamp,
    VClamp,
    USize,
    VSize,
    PolyFlags,
    Sampler,
    Format,
    Palette,
    NoPalette,
    NumMips,
    Mip,
    MipData,
    MipWidth,
    MipHeight,
    AllocMips,
    InvalidMip,
    WriteMip,
    UploadCompressed,
    UploadUncompressed,
    Properties,
    DefaultBlending,
    GammaWrite,
    DefaultGammaMultiplier,
    Mac,
    DefaultVSync,
    DefaultScale,
    DefaultAA,
    DefaultSamples,
    DefaultAnisotropy,
    DefaultSrgb,
    InitState,
    Device,
    DestroyDevice,
    DestroyText,
    Metal,
    DestroyMetal,
    Window,
    CreateMetal,
    MetalFailed,
    WindowMissing,
    CreateDevice,
    Backend,
    BackendError,
    BackendRetryError,
    BackendInvalidError,
    PublishDevice,
    Anisotropy,
    Srgb,
    AdapterLog,
    Surface,
    SurfaceLog,
    CreateText,
    InitSetRes,
    Flush,
    CountsLog,
    DrawCountsLog,
    Viewport,
    Client,
    Brightness,
    Editor,
    GammaMultiplier,
    EditorGammaMultiplier,
    ClearCache,
    FlashState,
    Drawable,
    WindowSize,
    PixelWidth,
    PixelHeight,
    LogicalHeight,
    LogicalFallback,
    BackingScale,
    LockDevice,
    LockLog,
    UnlockDevice,
    SceneFields,
    Wire,
    SceneWire,
    SceneOrtho,
    Gamma,
    SceneGamma,
    ActorBlending,
    OneBlending,
    SceneColor,
    SceneMap,
    Count,
    CountWrite,
    FirstLog,
    Submit,
    Points,
    NumPoints,
    Indexed,
    NumIndices,
    ReadIndex,
    PointExists,
    InvalidVertex,
    AllocateVertices,
    FillVertex,
    Upload,
    HasTexture,
    TextureMaterial,
    TextureAlpha,
    NeutralScale,
    ZeroFog,
    FlashScale,
    FlashAlpha,
    SurfaceTexture,
    UploadDiffuse,
    LightMap,
    HasLightMap,
    UploadLight,
    FogMap,
    FogMip,
    FogData,
    HasFogMap,
    UploadFog,
    Axes,
    FirstPoly,
    NextPoly,
    LightMaterial,
    FogMaterial,
    ProjectRequest,
    CreateLayout,
    LayoutExists,
    LayoutOrigin,
    ResetLayout,
    TextBackend,
    SafeOriginX,
    SafeOriginY,
    TextCreateScope,
    DestroyLayout,
    ResetMeasure,
    MeasureLayout,
    DrawBridge,
    PrepareDraw,
    TextFields,
    DrawText,
    TextLogged,
    TextMarkLogged,
    TextLog,
    Frame,
    IsLayout,
    TextDrawScope,
    OutputCount,
    SavedLight,
    SavedFog,
    SavedTexture,
    PublishMetal,
    ConfigureSurface,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2WgpuDriverServices {
    pub context: *mut c_void,
    pub invoke: unsafe extern "C" fn(
        *mut c_void,
        Hp2WgpuDriverLeaf,
        i64,
        i64,
        f64,
        *mut Hp2WgpuDriverValue,
    ) -> i32,
}

/// Supplies the checked leaves used by the shared renderer policy.
///
/// Implementations are invocation-local. `invoke` performs one actual storage,
/// SDK, or renderer operation; policy ordering remains in [`run_with`].
pub trait DriverPolicyAdapter {
    /// Error returned by one checked leaf.
    type Error;

    /// Runs one leaf with the exact scalar inputs selected by shared policy.
    fn invoke(
        &self,
        leaf: Hp2WgpuDriverLeaf,
        a: i64,
        b: i64,
        x: f64,
    ) -> Result<Hp2WgpuDriverValue, Self::Error>;
}

struct NativeHost(Hp2WgpuDriverServices);

impl DriverPolicyAdapter for NativeHost {
    type Error = i32;

    fn invoke(
        &self,
        leaf: Hp2WgpuDriverLeaf,
        a: i64,
        b: i64,
        x: f64,
    ) -> Result<Hp2WgpuDriverValue, Self::Error> {
        let mut value = Hp2WgpuDriverValue {
            integer: 0,
            real: 0.0,
        };
        // SAFETY: `run`'s contract guarantees an invocation-local checked frame;
        // the C++ callback contains exceptions and preserves the first error.
        let status = unsafe { (self.0.invoke)(self.0.context, leaf, a, b, x, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
}

struct Host<A>(A);

impl<A: DriverPolicyAdapter> Host<A> {
    fn call(
        &self,
        leaf: Hp2WgpuDriverLeaf,
        a: i64,
        b: i64,
        x: f64,
    ) -> Result<Hp2WgpuDriverValue, A::Error> {
        self.0.invoke(leaf, a, b, x)
    }
    fn i(&self, leaf: Hp2WgpuDriverLeaf) -> Result<i64, A::Error> {
        self.a(leaf, 0, 0)
    }
    fn a(&self, leaf: Hp2WgpuDriverLeaf, a: i64, b: i64) -> Result<i64, A::Error> {
        Ok(self.call(leaf, a, b, 0.0)?.integer)
    }
    fn r(&self, leaf: Hp2WgpuDriverLeaf) -> Result<f64, A::Error> {
        Ok(self.call(leaf, 0, 0, 0.0)?.real)
    }
    fn f(&self, leaf: Hp2WgpuDriverLeaf, x: f64) -> Result<(), A::Error> {
        self.call(leaf, 0, 0, x)?;
        Ok(())
    }
}

/// Executes one action against the complete shared renderer policy.
///
/// The adapter supplies checked leaves only; all branching, ordering, traversal,
/// geometry assembly, caching, and lifecycle decisions remain in this function.
pub fn run_with<A: DriverPolicyAdapter>(
    adapter: A,
    action: Hp2WgpuDriverAction,
) -> Result<i64, A::Error> {
    use Hp2WgpuDriverLeaf::*;
    let h = Host(adapter);
    match action {
        Hp2WgpuDriverAction::UploadTexture => {
            let id = h.i(FixCache)?;
            if h.i(Cached)? != 0 && h.i(Dirty)? == 0 {
                return Ok(id);
            }
            h.i(ResetDirty)?;
            let map = h.i(Texture)? == 0;
            let mut sampler = 0;
            if map || {
                let n = h.i(UClamp)?;
                n > 0 && n < h.i(USize)?
            } {
                sampler |= 1;
            }
            if map || {
                let n = h.i(VClamp)?;
                n > 0 && n < h.i(VSize)?
            } {
                sampler |= 2;
            }
            if h.i(PolyFlags)? & 0x800 != 0 {
                sampler |= 4;
            }
            h.a(Sampler, sampler, 0)?;
            if h.i(Format)? == 0 && h.i(Palette)? == 0 {
                h.i(NoPalette)?;
                return Ok(id);
            }
            if h.i(Format)? == 3 {
                if h.i(NumMips)? <= 0 || h.a(Mip, 0, 0)? == 0 || h.i(MipData)? == 0 {
                    return Ok(id);
                }
                h.a(AllocMips, h.i(NumMips)?, 0)?;
                return Ok(id);
            }
            if h.i(NumMips)? > 0 && h.a(Mip, 0, 0)? != 0 && h.i(MipData)? != 0 {
                let bpp = match h.i(Format)? {
                    0 => 1,
                    2 => 2,
                    4 => 3,
                    _ => 4,
                };
                let bytes = (h.i(MipWidth)? as usize)
                    .wrapping_mul(h.i(MipHeight)? as usize)
                    .wrapping_mul(bpp);
                h.a(
                    UploadUncompressed,
                    bytes as i64,
                    i64::from(h.i(PolyFlags)? & 2 != 0),
                )?;
            }
            Ok(id)
        }
        Hp2WgpuDriverAction::StaticConstructor => {
            h.i(Properties)?;
            h.a(DefaultBlending, 1, 0)?;
            h.f(GammaWrite, 1.0)?;
            h.f(DefaultGammaMultiplier, 1.75)?;
            h.a(DefaultVSync, if h.i(Mac)? != 0 { 0 } else { 2 }, 0)?;
            h.f(DefaultScale, 1.0)?;
            h.a(DefaultAA, 1, 0)?;
            h.a(DefaultSamples, 4, 0)?;
            h.f(DefaultAnisotropy, 16.0)?;
            h.a(DefaultSrgb, 0, 0)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::Init => {
            h.f(InitState, 0.5)?;
            if h.i(Device)? != 0 {
                h.i(DestroyDevice)?;
            }
            h.i(DestroyText)?;
            if h.i(Mac)? != 0 {
                if h.i(Metal)? != 0 {
                    h.i(DestroyMetal)?;
                }
                if h.i(Window)? != 0 {
                    if h.i(CreateMetal)? != 0 {
                        h.i(PublishMetal)?;
                    } else {
                        h.i(MetalFailed)?;
                    }
                } else {
                    h.i(WindowMissing)?;
                }
            }
            if h.i(CreateDevice)? == 0 {
                if h.i(Mac)? != 0 && h.i(Metal)? != 0 {
                    h.i(DestroyMetal)?;
                }
                let backend = h.i(Backend)?;
                if backend == 4 {
                    h.a(BackendError, backend, 0)?;
                } else if backend < 5 {
                    h.a(BackendRetryError, backend, 0)?;
                } else {
                    h.i(BackendInvalidError)?;
                }
            }
            h.i(PublishDevice)?;
            h.i(Anisotropy)?;
            h.i(Srgb)?;
            h.i(AdapterLog)?;
            if h.i(Surface)? != 0 {
                h.i(SurfaceLog)?;
            }
            h.i(CreateText)?;
            h.i(InitSetRes)?;
            Ok(1)
        }
        Hp2WgpuDriverAction::SetRes => {
            h.i(Flush)?;
            h.i(ConfigureSurface)?;
            Ok(1)
        }
        Hp2WgpuDriverAction::Exit => {
            if h.i(Device)? != 0 {
                h.i(CountsLog)?;
                h.i(DrawCountsLog)?;
                h.i(DestroyDevice)?;
            }
            h.i(DestroyText)?;
            if h.i(Mac)? != 0 && h.i(Metal)? != 0 {
                h.i(DestroyMetal)?;
            }
            Ok(0)
        }
        Hp2WgpuDriverAction::Flush => {
            if h.i(Viewport)? != 0 && h.i(Client)? != 0 {
                let brightness = h.r(Brightness)? as f32;
                let multiplier = if h.i(Editor)? != 0 {
                    h.r(EditorGammaMultiplier)?
                } else {
                    h.r(GammaMultiplier)?
                } as f32;
                h.f(GammaWrite, ((brightness + 0.1f32) * multiplier) as f64)?;
            }
            h.i(ClearCache)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::Lock => {
            h.i(FlashState)?;
            if h.i(Mac)? != 0 {
                if h.i(Window)? != 0 {
                    h.i(Drawable)?;
                    h.i(WindowSize)?;
                }
                if h.i(PixelWidth)? <= 0 || h.i(PixelHeight)? <= 0 {
                    h.i(LogicalFallback)?;
                }
                let height = h.i(LogicalHeight)?;
                h.f(
                    BackingScale,
                    if height > 0 {
                        h.i(PixelHeight)? as f64 / height as f64
                    } else {
                        1.0
                    },
                )?;
            } else {
                h.i(LogicalFallback)?;
                h.f(BackingScale, 1.0)?;
            }
            let outcome = h.a(
                LockDevice,
                h.i(PixelWidth)?.max(1),
                h.i(PixelHeight)?.max(1),
            )?;
            if outcome != 0 {
                h.a(LockLog, outcome, 0)?;
            }
            Ok(0)
        }
        Hp2WgpuDriverAction::Unlock => {
            h.i(UnlockDevice)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::SetSceneNode => {
            if h.i(Device)? == 0 {
                return Ok(0);
            }
            h.i(SceneFields)?;
            h.a(
                SceneWire,
                i64::from(h.i(Editor)? != 0 && h.i(Wire)? != 0),
                0,
            )?;
            h.f(
                SceneGamma,
                if h.i(SceneOrtho)? != 0 {
                    1.0
                } else {
                    h.r(Gamma)?
                },
            )?;
            h.f(SceneColor, if h.i(ActorBlending)? != 0 { 1.0 } else { 1.5 })?;
            h.f(SceneMap, if h.i(OneBlending)? != 0 { 2.0 } else { 4.0 })?;
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            h.i(Submit)?;
            Ok(0)
        }
        // Source `UWgpuRenderDevice::MaxVertices()` returned 256; the value
        // only sets the engine's per-flush vertex scratch size, so raising it
        // keeps identical geometry and draw order while cutting the number of
        // `DrawTriangles` flushes (and their per-call validation/upload cost)
        // by the same factor.
        Hp2WgpuDriverAction::MaxVertices => Ok(2048),
        Hp2WgpuDriverAction::DrawTriangles => {
            if h.i(Device)? == 0 || h.i(Points)? == 0 || h.i(NumPoints)? < 3 {
                return Ok(0);
            }
            let indexed = h.i(Indexed)? != 0;
            let count = if indexed {
                let n = h.i(NumIndices)? as i32;
                n.wrapping_sub(n % 3)
            } else {
                (h.i(NumPoints)? as i32).wrapping_sub(2).wrapping_mul(3)
            };
            if count < 3 {
                return Ok(0);
            }
            let mut i = 0;
            while i < if indexed {
                count as i64
            } else {
                h.i(NumPoints)?
            } {
                let point = if indexed { h.a(ReadIndex, i, 0)? } else { i };
                if point < 0 || point >= h.i(NumPoints)? || h.a(PointExists, point, 0)? == 0 {
                    h.i(InvalidVertex)?;
                    return Ok(0);
                }
                i += 1;
            }
            h.a(AllocateVertices, count as i64, i64::from(indexed))?;
            Ok(0)
        }
        Hp2WgpuDriverAction::TrianglesBody => {
            let count = h.i(OutputCount)?;
            let indexed = h.i(Indexed)? != 0;
            if indexed {
                for i in 0..count as i64 {
                    h.a(FillVertex, i, h.a(ReadIndex, i, 0)?)?;
                }
            } else {
                let mut out = 0;
                let mut i = 0;
                while i < h.i(NumPoints)? - 2 {
                    h.a(FillVertex, out, 0)?;
                    h.a(FillVertex, out + 1, i + 1)?;
                    h.a(FillVertex, out + 2, i + 2)?;
                    out += 3;
                    i += 1;
                }
            }
            h.i(Upload)?;
            let has = h.i(Texture)?;
            h.a(HasTexture, has, 0)?;
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            if has != 0 {
                h.i(TextureMaterial)?;
            }
            h.i(Submit)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::DrawTile => {
            if h.i(Device)? == 0 {
                return Ok(0);
            }
            h.i(Upload)?;
            let has = h.i(Texture)?;
            h.a(HasTexture, has, 0)?;
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            if has != 0 {
                h.i(TextureAlpha)?;
            }
            h.i(Submit)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::Draw2DLine => {
            if h.i(Device)? == 0 {
                return Ok(0);
            }
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            h.i(Submit)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::Draw2DPoint => {
            if h.i(Device)? == 0 {
                return Ok(0);
            }
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            h.i(Submit)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::EndFlash => {
            if h.i(Device)? == 0 {
                return Ok(0);
            }
            if h.i(NeutralScale)? != 0 && h.i(ZeroFog)? != 0 {
                return Ok(0);
            }
            let x = h.r(FlashScale)? as f32 * 2.0;
            // Native Min uses the ordered comparison, including its NaN behavior.
            let alpha = 1.0f32 - if x < 1.0 { x } else { 1.0 };
            h.f(FlashAlpha, alpha as f64)?;
            if h.i(Submit)? == 0 {
                return Ok(0);
            }
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::ClearZ => {
            if h.i(Device)? != 0 {
                h.i(Submit)?;
            }
            Ok(0)
        }
        Hp2WgpuDriverAction::DrawComplexSurface => {
            if h.i(Device)? == 0 || h.i(SurfaceTexture)? == 0 {
                return Ok(0);
            }
            h.i(UploadDiffuse)?;
            let light = h.i(LightMap)?;
            h.a(HasLightMap, light, 0)?;
            if light != 0 {
                h.i(UploadLight)?;
            }
            let fog = h.i(FogMap)? != 0 && h.i(FogMip)? != 0 && h.i(FogData)? != 0;
            h.a(HasFogMap, i64::from(fog), 0)?;
            if fog {
                h.i(UploadFog)?;
            }
            let texture = h.i(Texture)?;
            h.a(HasTexture, texture, 0)?;
            h.i(Axes)?;
            let mut total = 0i32;
            let mut exists = h.i(FirstPoly)? != 0;
            while exists {
                if h.i(NumPoints)? >= 3 {
                    total = total
                        .wrapping_add((h.i(NumPoints)? as i32).wrapping_sub(2).wrapping_mul(3));
                }
                exists = h.i(NextPoly)? != 0;
            }
            if total <= 0 {
                return Ok(0);
            }
            h.a(AllocateVertices, total as i64, 0)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::ComplexBody => {
            let light = h.i(SavedLight)?;
            let fog = h.i(SavedFog)? != 0;
            let texture = h.i(SavedTexture)?;
            let mut out = 0;
            let mut exists = h.i(FirstPoly)? != 0;
            while exists {
                let n = h.i(NumPoints)?;
                if n >= 3 {
                    for i in 0..n - 2 {
                        h.a(FillVertex, out, 0)?;
                        h.a(FillVertex, out + 1, i + 1)?;
                        h.a(FillVertex, out + 2, i + 2)?;
                        out += 3;
                    }
                }
                exists = h.i(NextPoly)? != 0;
            }
            if h.i(Count)? == 0 {
                h.i(FirstLog)?;
            }
            h.a(CountWrite, (h.i(Count)? as i32).wrapping_add(1) as i64, 0)?;
            if light != 0 {
                h.i(LightMaterial)?;
            }
            if fog {
                h.i(FogMaterial)?;
            }
            if texture != 0 {
                h.i(TextureMaterial)?;
            }
            h.i(Submit)?;
            Ok(0)
        }
        Hp2WgpuDriverAction::CreateCanvasTextLayoutBody => {
            if h.i(ProjectRequest)? == 0 || h.i(CreateLayout)? == 0 || h.i(LayoutExists)? == 0 {
                return Ok(0);
            }
            h.i(LayoutOrigin)?;
            Ok(1)
        }
        Hp2WgpuDriverAction::CreateCanvasTextLayout => {
            h.i(ResetLayout)?;
            if h.i(TextBackend)? == 0 || h.i(SafeOriginX)? == 0 || h.i(SafeOriginY)? == 0 {
                return Ok(0);
            }
            h.i(TextCreateScope)
        }
        Hp2WgpuDriverAction::DestroyCanvasTextLayout => {
            if h.i(TextBackend)? != 0 {
                h.i(DestroyLayout)?;
            }
            Ok(0)
        }
        Hp2WgpuDriverAction::MeasureCanvasText => {
            h.i(ResetMeasure)?;
            if h.i(TextBackend)? != 0 {
                h.i(MeasureLayout)
            } else {
                Ok(0)
            }
        }
        Hp2WgpuDriverAction::DrawCanvasTextBody => {
            if h.i(DrawBridge)? == 0 {
                h.i(PrepareDraw)?;
            }
            if h.i(DrawBridge)? == 0 {
                return Ok(0);
            }
            h.i(TextFields)?;
            let drawn = h.i(DrawText)?;
            if h.i(TextLogged)? == 0 {
                h.a(TextMarkLogged, 1, 0)?;
                h.i(TextLog)?;
            }
            Ok(drawn)
        }
        Hp2WgpuDriverAction::DrawCanvasText => {
            if h.i(Device)? == 0
                || h.i(TextBackend)? == 0
                || h.i(Frame)? == 0
                || h.i(IsLayout)? == 0
            {
                return Ok(0);
            }
            h.i(TextDrawScope)
        }
        Hp2WgpuDriverAction::CompressedBody => {
            let mut i = 0;
            while i < h.i(NumMips)? {
                if h.a(Mip, i, 0)? == 0 || h.i(MipData)? == 0 {
                    h.a(InvalidMip, i, 0)?;
                    return Ok(0);
                }
                let w = h.i(MipWidth)? as i32;
                let v = h.i(MipHeight)? as i32;
                let bytes = ((w.wrapping_add(3) / 4).max(1) as usize)
                    .wrapping_mul((v.wrapping_add(3) / 4).max(1) as usize)
                    .wrapping_mul(8);
                h.a(WriteMip, i, bytes as i64)?;
                i += 1;
            }
            h.i(UploadCompressed)?;
            Ok(0)
        }
    }
}

/// Execute one complete native method (or its native typed-scope continuation).
///
/// # Safety
/// Services must refer to an invocation-local native frame for `action`. The
/// callback must contain exceptions/signals, preserve first-error status, and
/// keep all native objects and array spans valid through their original scopes.
/// Reentry must create a separate frame. No native exception may unwind here.
pub unsafe fn run(
    services: Hp2WgpuDriverServices,
    action: Hp2WgpuDriverAction,
) -> Result<i64, i32> {
    run_with(NativeHost(services), action)
}

