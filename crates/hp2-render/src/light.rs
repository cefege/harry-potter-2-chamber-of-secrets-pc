//! Lighting kernels ported from `HarryPotter2/Unreal/Render/Src/UnLight.cpp`.
//!
//! Bit-exact port: same operation order, integer math, and the arm64 clang
//! `-ffp-contract=on` fusions written as `mul_add` where the C++ contracts.
//! GMath table accessors and RNG come from `Hp2RenderServices`.
//!
//! Fusion rules (clang, AST-level): `a*b + c` and `c + a*b` fuse; when both
//! operands of `a ± b` are multiplies, the left one fuses. Operator calls
//! (FVector `+`, `-`, `*`, `|`, `Square`, `SizeSquared`, `SizeApprox`,
//! `CosFloat`, `ToByte`, `Max`, `Abs`) are calls, so nothing fuses across them
//! in the caller; fusion inside a callee's own body still applies.

use hp2_abi::FVector;

use crate::services::Hp2RenderServices;

#[allow(clippy::approx_constant, clippy::excessive_precision)] // C literal, value kept exact
pub const PI: f32 = 3.1415926535897932;
const TO_RADIANS: f32 = PI / 32768.0;

pub const LIGHT_DIST_LEN: usize = 4096;
pub const FILTER_TAB_LEN: usize = 128;
pub const BYTE_FOG_LEN: usize = 128 * 128;

const LE_TORCH_WAVER: u8 = 1;
const LE_FIRE_WAVER: u8 = 2;
const LE_WATERY_SHIMMER: u8 = 3;

/// `FILTER_TAB` as laid out by `FLightManager::FilterTab[128][4]`.
pub type FilterTab = [[u32; 4]; FILTER_TAB_LEN];

// ---------------------------------------------------------------------------
// Helpers (UnMath.h / UnGnuG.h / UnLight.cpp macros)
// ---------------------------------------------------------------------------

/// `appRound` / `appFloor`: portable branch is `static_cast<INT>(f)`.
#[inline]
fn app_round(f: f32) -> i32 {
    f as i32
}

#[inline]
fn app_floor(f: f32) -> i32 {
    f as i32
}

/// `MinPositiveFloat(F1, F2)`: F1 when `bits(F1) < bits(F2)`, else F2.
#[inline]
fn min_positive(f1: f32, f2: f32) -> f32 {
    if f1.to_bits() < f2.to_bits() { f1 } else { f2 }
}

/// `Max(A, B) = A >= B ? A : B`.
#[inline]
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `Abs(A) = A >= 0 ? A : -A`.
#[inline]
fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `IsNegativeFloat(F)`: sign bit set.
#[inline]
fn is_negative_float(f: f32) -> bool {
    f.to_bits() >= 0x8000_0000
}

/// `ToByte(F)`: float add of `2<<22`, then the low byte of the bit pattern.
#[inline]
fn to_byte(f: f32) -> u8 {
    (f + 8388608.0).to_bits() as u8
}

#[inline]
fn sq(x: f32) -> f32 {
    x * x
}

/// `FVector::operator|`, and `SizeSquared` for `a == b`.
/// `X*V.X + Y*V.Y + Z*V.Z` contracted left-first.
#[inline]
fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

#[inline]
fn size_squared(v: FVector) -> f32 {
    dot(v, v)
}

#[inline]
fn v_add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

#[inline]
fn v_sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

#[inline]
fn v_scale(a: FVector, s: f32) -> FVector {
    FVector {
        x: a.x * s,
        y: a.y * s,
        z: a.z * s,
    }
}

/// Unqualified `SqrtApprox(F)` in UnLight.cpp: `(FLOAT)appSqrt(F)`, where
/// `appSqrt` is double-precision.
#[inline]
fn sqrt_plain(f: f32) -> f32 {
    (f as f64).sqrt() as f32
}

// ---------------------------------------------------------------------------
// GMath access through the services table
// ---------------------------------------------------------------------------

/// Table-backed GMath accessors (the `GMath.*` members in UnMath.h).
#[derive(Clone, Copy)]
pub struct GMath<'a> {
    services: &'a Hp2RenderServices,
}

impl<'a> GMath<'a> {
    pub fn new(services: &'a Hp2RenderServices) -> Self {
        Self { services }
    }

    /// `GMath.SinTab(i)`.
    pub fn sin_tab(&self, angle: i32) -> f32 {
        unsafe { (self.services.sin_tab)(angle) }
    }

    /// `GMath.CosTab(i)`.
    pub fn cos_tab(&self, angle: i32) -> f32 {
        unsafe { (self.services.cos_tab)(angle) }
    }

    /// `GMath.SqrtApprox(F)` (table).
    pub fn sqrt_approx(&self, f: f32) -> f32 {
        unsafe { (self.services.gmath_sqrt_approx)(f) }
    }

    /// `GMath.DivSqrtApprox(F)` (table).
    pub fn div_sqrt_approx(&self, f: f32) -> f32 {
        unsafe { (self.services.gmath_div_sqrt_approx)(f) }
    }

    /// `CosFloat(F) = CosTab(appRound((F*65536.f)/(2.f*PI)))`.
    pub fn cos_float(&self, f: f32) -> f32 {
        self.cos_tab(app_round((f * 65536.0) / (2.0 * PI)))
    }

    /// `FVector::SizeApprox() = GMath.SqrtApprox(SizeSquared())`.
    pub fn size_approx(&self, v: FVector) -> f32 {
        self.sqrt_approx(size_squared(v))
    }
}

// ---------------------------------------------------------------------------
// Table initialisation (FLightManager::Init)
// ---------------------------------------------------------------------------

/// `LightDist[i]` / `LightDistInc[i]` for `i < 4096`.
pub fn init_light_dist(
    light_dist: &mut [f32; LIGHT_DIST_LEN],
    light_dist_inc: &mut [f32; LIGHT_DIST_LEN],
) {
    for i in 0..LIGHT_DIST_LEN {
        // S = appSqrt((FLOAT)(i+1) * (1.0f/4096)), double sqrt then FLOAT.
        let x = (i as i32 + 1) as f32 * (1.0f32 / 4096.0);
        let s = (x as f64).sqrt() as f32;
        // Temp = 2*S*S*S - 3*S*S + 1, LHS-fused: fma((2*S)*S, S, -((3*S)*S)).
        let temp = (2.0 * s * s).mul_add(s, -(3.0 * s * s)) + 1.0;
        light_dist[i] = temp;
        light_dist_inc[i] = temp / s;
    }
}

/// `ByteFog[i*128+j] = (127-i)*j/127`.
pub fn init_byte_fog(byte_fog: &mut [u8; BYTE_FOG_LEN]) {
    for i in 0..128i32 {
        for j in 0..128i32 {
            byte_fog[(i * 128 + j) as usize] = (((127 - i) * j) / 127) as u8;
        }
    }
}

/// `FilterTab[i][j]` for `i < 128`, `j < 4`.
#[allow(clippy::needless_range_loop)] // index loops mirror the C `for (bit...)`
pub fn init_filter_tab() -> FilterTab {
    const FILTER_WEIGHT: [[i32; 8]; 8] = [
        [0, 24, 40, 24, 0, 0, 0, 0],
        [0, 40, 64, 40, 0, 0, 0, 0],
        [0, 24, 40, 24, 0, 0, 0, 0],
        [0; 8],
        [0; 8],
        [0; 8],
        [0; 8],
        [0; 8],
    ];
    let filter_sum: i32 = FILTER_WEIGHT.iter().flatten().sum();

    let mut tab: FilterTab = [[0; 4]; FILTER_TAB_LEN];
    for (i, row) in tab.iter_mut().enumerate() {
        for (j, entry) in row.iter_mut().enumerate() {
            *entry = 0;
            for pack in 0..4 {
                let mut acc: i32 = 0;
                for bit in 0..8 {
                    if (i as i32) & (1 << (pack + bit)) != 0 {
                        acc += FILTER_WEIGHT[j][bit];
                    }
                }
                let result = ((acc * 255) / filter_sum) as u32;
                *entry = entry.wrapping_add(result << (pack * 8));
            }
        }
    }
    tab
}

// ---------------------------------------------------------------------------
// Shadow map (FLightManager::ShadowMapGen)
// ---------------------------------------------------------------------------

/// Convolves the packed shadow bitmask into `dest` (`shadow_mask_u * v_clamp * 2`
/// DWORDs). `src` is `None` when the light has no shadow bits: `dest` is filled
/// with 0x7F bytes (`appMemset(Dest1, 127, ...)`).
pub fn shadow_map_gen(
    filter_tab: &FilterTab,
    shadow_mask_u: usize,
    v_clamp: usize,
    src: Option<&[u8]>,
    dest: &mut [u32],
) {
    assert_eq!(dest.len(), shadow_mask_u * v_clamp * 2, "shadow dest size");

    let Some(src) = src else {
        dest.fill(0x7F7F_7F7F);
        return;
    };

    let u_count = shadow_mask_u;
    let size4 = u_count * 2;
    dest.fill(0);

    // Dests[0] and Dests[1] start at Dest1, Dests[2] at Dest1 + Size4 (DWORD indexes).
    let mut dests = [0usize, 0usize, size4];
    let mut src_pos = 0usize;

    for v in 0..v_clamp {
        let mut sp = src_pos;

        // Offset of shadow map relative to convolution filter left edge.
        let mut d: u32 = (src[sp] as u32) << (8 + 2);
        sp += 1;
        if d & 0x400 != 0 {
            d |= 0x300;
        }

        for u in 0..u_count {
            d >>= 8;
            let add = if u + 1 < u_count {
                let b = src[sp] as u32;
                sp += 1;
                b << (8 + 2)
            } else if d & 0x200 != 0 {
                0xC00
            } else {
                0
            };
            d = d.wrapping_add(add);

            let tab1 = filter_tab[(d & 0x7f) as usize];
            for ch in 0..3 {
                let idx = dests[ch];
                add_word(dest, idx, tab1[ch]);
                dests[ch] += 1;
            }

            let tab2 = filter_tab[((d >> 4) & 0x7f) as usize];
            for ch in 0..3 {
                let idx = dests[ch];
                add_word(dest, idx, tab2[ch]);
                dests[ch] += 1;
            }
        }

        src_pos += u_count;
        if v == 0 {
            dests[0] -= size4;
        }
        if v + 2 == v_clamp {
            dests[2] -= size4;
        }
    }
}

/// `*Dest += Value` for one DWORD of the map. Only a single-row map (`v_clamp == 1`)
/// makes channel 2 reach past the end: the C++ writes that DWORD past the map (UB). It is
/// dropped here because it lies outside the map and nothing reads it back.
fn add_word(dest: &mut [u32], idx: usize, value: u32) {
    if let Some(slot) = dest.get_mut(idx) {
        *slot = slot.wrapping_add(value);
    }
}

// ---------------------------------------------------------------------------
// Texture merge (FLightManager::Merge, portable branch)
// ---------------------------------------------------------------------------

/// Per-merge parameters, from `FTextureInfo` (`UClamp`, `USize`) and `FLightInfo`.
#[derive(Clone, Copy, Debug)]
pub struct MergeParams {
    pub effect: u8,
    pub key: i32,
    pub uclamp: i32,
    pub usize_: i32,
    pub min_u: i32,
    pub max_u: i32,
    pub min_v: i32,
    pub max_v: i32,
}

/// Blends the illumination `src` (stride `uclamp`) through `palette` into
/// `dest`, adding `stream` (stride `usize_`) with per-channel saturation.
/// `stream` is `None` when it is `dest` itself (`Merge(.., Stream, Stream)`);
/// each word is read before it is written, so the in-place form is exact.
pub fn merge(
    p: &MergeParams,
    src: &[u8],
    palette: &[u32; 256],
    stream: Option<&[u32]>,
    dest: &mut [u32],
    random_base: &[f32; 256],
    random: &[f32; 256],
) {
    if p.max_u - p.min_u <= 0 {
        return;
    }

    let fx_detect = matches!(p.effect, LE_TORCH_WAVER | LE_FIRE_WAVER | LE_WATERY_SHIMMER);
    let uclamp = p.uclamp as usize;
    let usize_ = p.usize_ as usize;
    let min_u = p.min_u as usize;
    let max_u = p.max_u as usize;

    let mut src_row = p.min_v as usize * uclamp;
    let mut dest_row = p.min_v as usize * usize_;
    let mut stream_row = p.min_v as usize * usize_;
    let mut key = p.key;

    let mut temp = [0u8; 1024];

    for _ in p.min_v..p.max_v {
        if fx_detect {
            for i in min_u..max_u {
                let s = src[src_row + i] as f32;
                let k = (key as usize) & 255;
                let factor = match p.effect {
                    LE_TORCH_WAVER => 0.05f32.mul_add(random_base[k], 0.95),
                    LE_FIRE_WAVER => 0.20f32.mul_add(random_base[k], 0.80),
                    _ => 0.40f32.mul_add(random[k], 0.60),
                };
                temp[i] = app_floor(s * factor) as u8;
                key = key.wrapping_add(1);
            }
        }

        for j in min_u..max_u {
            let c = if fx_detect { temp[j] } else { src[src_row + j] };
            let base = match stream {
                Some(s) => s[stream_row + j],
                None => dest[dest_row + j],
            };
            let mut d = base.wrapping_add(palette[c as usize]);
            if d & 0x8080_8080 != 0 {
                let mut sat = d & 0x8080_8080;
                sat -= sat >> 7;
                d = (d & 0x7f7f_7f7f) | sat;
            }
            dest[dest_row + j] = d;
        }

        src_row += uclamp;
        stream_row += usize_;
        dest_row += usize_;
    }
}

// ---------------------------------------------------------------------------
// Volumetric fog (FLightManager::Volumetric)
// ---------------------------------------------------------------------------

/// Light and volume fields read by `Volumetric`.
#[derive(Clone, Copy, Debug)]
pub struct VolumetricParams {
    pub location: FVector,
    pub vol_radius_squared: f32,
    pub rvol_radius: f32,
    pub rvol_radius_squared: f32,
    pub vol_brightness: f32,
    pub location_size_squared: f32,
    pub vol_inside: bool,
    /// `Info->Actor->LightSource == LD_Ambient`.
    pub ambient: bool,
}

/// Volumetric fog contribution at `vertex`. `fog_rejection` is the
/// function-static `FogRejectionMethod`, carried by the caller.
pub fn volumetric(
    g: &GMath<'_>,
    info: &VolumetricParams,
    vertex: FVector,
    fog_rejection: &mut i32,
) -> f32 {
    if info.ambient {
        return min_positive(
            g.size_approx(vertex) * info.rvol_radius * info.vol_brightness,
            1.0,
        );
    }

    let s = dot(info.location, vertex);
    if !info.vol_inside && is_negative_float(s) {
        return 0.0;
    }

    let vertex_size_squared = size_squared(vertex);
    let s2 = s * s;

    if *fog_rejection != 0
        && (vertex_size_squared * info.vol_radius_squared)
            < vertex_size_squared.mul_add(info.location_size_squared, -s2)
    {
        return 0.0;
    }

    let d = info.location_size_squared - s2 / vertex_size_squared;
    if info.vol_radius_squared < d {
        *fog_rejection = 1;
        return 0.0;
    }
    *fog_rejection = 0;

    let vertex_size = sqrt_plain(vertex_size_squared);
    let c0 = s / vertex_size;
    let h = sqrt_plain(info.vol_radius_squared - d);
    let mut full_sphere = false;
    let mut c1 = c0 - vertex_size;
    let mut c2;

    if c0 > h {
        c2 = h;
        if c1 < -h {
            c1 = -h;
            full_sphere = true;
        }
    } else {
        c2 = c0;
        if c1 < -h {
            c1 = -h;
        }
    }

    if c1 >= c2 {
        return 0.0;
    }

    let d2 = d * info.rvol_radius_squared;
    c2 *= info.rvol_radius;

    let f = if full_sphere {
        // I = 3 - 3*D2; F = (2*VB) * (c2 * (I - c2*c2))
        let i = (-3.0f32).mul_add(d2, 3.0);
        (2.0 * info.vol_brightness) * (c2 * (-c2).mul_add(c2, i))
    } else {
        c1 *= info.rvol_radius;
        let i = (-3.0f32).mul_add(d2, 3.0);
        info.vol_brightness * c2.mul_add((-c2).mul_add(c2, i), -(c1 * (-c1).mul_add(c1, i)))
    };

    if f < 0.0 {
        return 0.0;
    }
    min_positive(f, 1.0)
}

// ---------------------------------------------------------------------------
// Spatial shadow/attenuation kernels (SpatialGeneral / SpatialRadial)
// ---------------------------------------------------------------------------

/// Spatial effect selected by the `Effects[]` table (`spatial_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spatial {
    /// `spatial_None`: radial path; `Effect_Simple` when `RadiusInner == 0`, else `Effect_None`.
    None,
    SearchLight,
    Rotor,
    SlowWave,
    FastWave,
    Shock,
    Disco,
    Interference,
    Spotlight,
    Cylinder,
    NonIncidence,
    Shell,
}

/// `FLightInfo` / `FTextureInfo` / `FLightManager` values read by the spatial kernels.
#[derive(Clone, Copy, Debug)]
pub struct SpatialParams {
    /// `Map.UClamp`.
    pub uclamp: i32,
    /// `FLightManager::ShadowMaskU`.
    pub shadow_mask_u: i32,
    /// `FLightManager::ShadowSkip`.
    pub shadow_skip: i32,
    pub min_u: i32,
    pub max_u: i32,
    pub min_v: i32,
    pub max_v: i32,
    pub radius: f32,
    pub rradius: f32,
    pub radius_inner: f32,
    pub diffuse: f32,
    /// `Info->Actor->Location`.
    pub actor_location: FVector,
    /// `Info->Actor->GetViewRotation().Vector()` (used by Spotlight only).
    pub view: FVector,
    /// `Info->Actor->LightPeriod`.
    pub light_period: u8,
    /// `Info->Actor->LightPhase`.
    pub light_phase: u8,
    /// `Info->Actor->LightCone`.
    pub light_cone: u8,
    /// `LevelInfo->TimeSeconds`.
    pub time_seconds: f32,
    /// `FLightManager::VertexBase`, `VertexDU`, `VertexDV`.
    pub vertex_base: FVector,
    pub vertex_du: FVector,
    pub vertex_dv: FVector,
}

/// Runs one `spatial_*` entry. `light_dist_tab` is `Info->LightDistTab`,
/// `light_dist` is the global `FLightManager::LightDist`.
pub fn spatial(
    g: &GMath<'_>,
    kind: Spatial,
    p: &SpatialParams,
    light_dist_tab: &[f32; LIGHT_DIST_LEN],
    light_dist: &[f32; LIGHT_DIST_LEN],
    src: &[u8],
    dest: &mut [u8],
) {
    match kind {
        Spatial::None => {
            let simple = p.radius_inner == 0.0;
            spatial_radial(
                Spatial::None,
                simple,
                p,
                light_dist_tab,
                light_dist,
                src,
                dest,
            );
        }
        other => spatial_general(g, other, p, light_dist_tab, light_dist, src, dest),
    }
}

/// Effect state: `Effect_*` classes in UnLight.cpp. `simple` selects `Effect_Simple`
/// for `Spatial::None`.
struct Effect<'a> {
    kind: Spatial,
    simple: bool,
    light_dist_tab: &'a [f32; LIGHT_DIST_LEN],
    light_dist: &'a [f32; LIGHT_DIST_LEN],
    diffuse: f32,
    cycle_angle: f32,
    cycle_ang_tab: i32,
    /// `Effect_SearchLight::Angle`, set by `visible`.
    angle: f32,
    rrad_sq: f32,
    view: FVector,
    rsine: f32,
    sine_rsine: f32,
    sine_sq: f32,
    size_sq: f32,
    vdot_v: f32,
}

impl<'a> Effect<'a> {
    fn init(
        kind: Spatial,
        simple: bool,
        p: &SpatialParams,
        light_dist_tab: &'a [f32; LIGHT_DIST_LEN],
        light_dist: &'a [f32; LIGHT_DIST_LEN],
    ) -> Self {
        let zero = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut e = Self {
            kind,
            simple,
            light_dist_tab,
            light_dist,
            diffuse: 0.0,
            cycle_angle: 0.0,
            cycle_ang_tab: 0,
            angle: 0.0,
            rrad_sq: 0.0,
            view: zero,
            rsine: 0.0,
            sine_rsine: 0.0,
            sine_sq: 0.0,
            size_sq: 0.0,
            vdot_v: 0.0,
        };

        match kind {
            Spatial::Cylinder => {
                e.rrad_sq = sq(p.rradius);
            }
            Spatial::NonIncidence => {
                e.none_init(p);
                e.light_dist_tab = light_dist;
                e.diffuse = p.radius / (p.radius - p.radius_inner);
            }
            Spatial::None | Spatial::Shell => {
                e.none_init(p);
            }
            Spatial::Spotlight => {
                e.none_init(p);
                e.view = p.view;
                let sine = 1.0 - p.light_cone as f32 / 256.0;
                e.rsine = 1.0 / (1.0 - sine);
                e.sine_rsine = sine * e.rsine;
                e.sine_sq = sine * sine;
            }
            Spatial::SearchLight
            | Spatial::Rotor
            | Spatial::SlowWave
            | Spatial::FastWave
            | Spatial::Shock
            | Spatial::Disco
            | Spatial::Interference => {
                e.none_init(p);
                e.cycle_init(p);
                if kind == Spatial::SearchLight {
                    e.cycle_angle += 0.5 * PI;
                }
            }
        }
        e
    }

    /// `Effect_None::Init`.
    fn none_init(&mut self, p: &SpatialParams) {
        self.diffuse = (p.diffuse * p.radius) / (p.radius - p.radius_inner);
    }

    /// `Effect_Cycle::Init` (after `Effect_None::Init`).
    fn cycle_init(&mut self, p: &SpatialParams) {
        let mut cat = if p.light_period != 0 {
            (p.time_seconds * 1048576.0 / (p.light_period as f32)) as i32
        } else {
            0
        };
        cat = cat.wrapping_add((p.light_phase as i32) << 8);
        self.cycle_ang_tab = cat;
        self.cycle_angle = cat as f32 * TO_RADIANS;
    }

    /// `Visible(V)`. Only SearchLight and Spotlight reject vertices; the
    /// rest return true.
    fn visible(&mut self, v: FVector) -> bool {
        match self.kind {
            Spatial::SearchLight => {
                let atan = (v.x as f64).atan2(v.y as f64);
                let a = (4.0f64 * (self.cycle_angle as f64 + atan)) % ((8.0f32 * PI) as f64);
                self.angle = a as f32;
                self.angle >= PI && self.angle <= PI * 3.0
            }
            Spatial::Spotlight => {
                self.size_sq = size_squared(v);
                self.vdot_v = dot(v, self.view);
                self.vdot_v > 0.0 && sq(self.vdot_v) > self.sine_sq * self.size_sq
            }
            _ => true,
        }
    }

    /// `Spatial(DistSq, V)`.
    fn spatial(&self, dist_sq: usize, v: FVector) -> f32 {
        match self.kind {
            Spatial::Cylinder => {
                let p = sq(v.x) + sq(v.y);
                max_f(0.0, (-p).mul_add(self.rrad_sq, 1.0))
            }
            Spatial::Shell => {
                let amp = self.light_dist[dist_sq];
                max_f(0.0, (-10.0f32).mul_add(abs_f(amp - 0.1), 1.0))
            }
            _ => {
                let raw = self.diffuse * self.light_dist_tab[dist_sq];
                if self.simple {
                    raw
                } else {
                    min_positive(raw, 1.0)
                }
            }
        }
    }

    /// `Scale(V)`.
    fn scale(&self, g: &GMath<'_>, v: FVector) -> f32 {
        match self.kind {
            Spatial::SearchLight => {
                let mut scale = 0.5f32.mul_add(g.cos_float(self.angle), 0.5);
                let d = 0.00006f32 * (sq(v.x) + sq(v.y));
                if d < 1.0 {
                    scale *= d;
                }
                scale
            }
            Spatial::Rotor => {
                let angle =
                    6.0f64.mul_add((v.x as f64).atan2(v.y as f64), self.cycle_angle as f64) as f32;
                let mut scale = 0.5f32.mul_add(g.cos_float(angle), 0.5);
                let d = 0.0001f32 * (sq(v.x) + sq(v.y));
                if d < 1.0 {
                    scale = scale.mul_add(d, 1.0 - d);
                }
                scale
            }
            Spatial::SlowWave => {
                let idx = app_round(g.size_approx(v) * 1024.0).wrapping_sub(self.cycle_ang_tab);
                0.3f32.mul_add(g.sin_tab(idx), 0.7)
            }
            Spatial::FastWave => {
                let idx = app_round(g.size_approx(v) * 512.0).wrapping_sub(self.cycle_ang_tab);
                0.3f32.mul_add(g.sin_tab(idx), 0.7)
            }
            Spatial::Shock => {
                let dist = (8.0f32 * g.size_approx(v)) as i32;
                let mut b = 0.1f32.mul_add(
                    g.sin_tab(dist.wrapping_shl(5).wrapping_sub(self.cycle_ang_tab)),
                    0.9,
                );
                b *= 0.1f32.mul_add(
                    g.cos_tab(dist.wrapping_shl(4).wrapping_add(self.cycle_ang_tab)),
                    0.9,
                );
                b *= 0.1f32.mul_add(g.sin_tab((dist >> 3).wrapping_sub(self.cycle_ang_tab)), 0.9);
                b
            }
            Spatial::Disco => {
                let yaw = (11.0f64 * (v.x as f64).atan2(v.y as f64)) as f32;
                let pitch = pitch(g, v);
                let scale1 = 0.5f32.mul_add(g.cos_float(yaw + self.cycle_angle), 0.5);
                let scale2 = 0.5f32.mul_add(g.cos_float(pitch + self.cycle_angle), 0.5);
                let mut scale = (-scale1).mul_add(scale2, scale1 + scale2);
                let d = 0.00005f32 * (sq(v.x) + sq(v.y));
                if d < 1.0 {
                    scale *= d;
                }
                1.0 - scale
            }
            Spatial::Interference => {
                let p = pitch(g, v);
                0.5f32.mul_add(g.cos_float(p + self.cycle_angle), 0.5)
            }
            Spatial::Spotlight => {
                let t = (self.vdot_v * self.rsine)
                    .mul_add(g.div_sqrt_approx(self.size_sq), -self.sine_rsine);
                sq(t)
            }
            Spatial::None | Spatial::Cylinder | Spatial::NonIncidence | Spatial::Shell => 1.0,
        }
    }
}

/// `11.0f * appAtan2(GMath.SqrtApprox(Square(V.X)+Square(V.Y)), V.Z)`.
fn pitch(g: &GMath<'_>, v: FVector) -> f32 {
    let r = g.sqrt_approx(sq(v.x) + sq(v.y));
    (11.0f64 * (r as f64).atan2(v.z as f64)) as f32
}

/// `SpatialGeneral<E>`: per-pixel visibility, distance, spatial and scale terms.
fn spatial_general(
    g: &GMath<'_>,
    kind: Spatial,
    p: &SpatialParams,
    light_dist_tab: &[f32; LIGHT_DIST_LEN],
    light_dist: &[f32; LIGHT_DIST_LEN],
    src: &[u8],
    dest: &mut [u8],
) {
    let mut effect = Effect::init(kind, false, p, light_dist_tab, light_dist);

    // VertexV = VertexBase + VertexDV*MinV + VertexDU*MinU - Actor->Location.
    let mut vertex_v = v_sub(
        v_add(
            v_add(p.vertex_base, v_scale(p.vertex_dv, p.min_v as f32)),
            v_scale(p.vertex_du, p.min_u as f32),
        ),
        p.actor_location,
    );

    let mut src_pos = (p.shadow_mask_u as isize * 8) * p.min_v as isize + p.min_u as isize;
    let mut dest_pos = p.uclamp as isize * p.min_v as isize + p.min_u as isize;
    let uskip = (p.uclamp - (p.max_u - p.min_u)) as isize;
    let radius_mult = 4093.0f32 / sq(p.radius);

    for _v in p.min_v..p.max_v {
        let mut vertex = vertex_v;
        for _u in p.min_u..p.max_u {
            let d = dest_pos as usize;
            if effect.visible(vertex) {
                let dist_sq = app_round(size_squared(vertex) * radius_mult);
                if dist_sq < 4096 && src[src_pos as usize] != 0 {
                    let s = src[src_pos as usize];
                    let value = (s as f32) * effect.spatial(dist_sq as usize, vertex);
                    dest[d] = to_byte(value * effect.scale(g, vertex));
                } else {
                    dest[d] = 0;
                }
            } else {
                dest[d] = 0;
            }
            vertex = v_add(vertex, p.vertex_du);
            src_pos += 1;
            dest_pos += 1;
        }
        src_pos += uskip + p.shadow_skip as isize;
        dest_pos += uskip;
        vertex_v = v_add(vertex_v, p.vertex_dv);
    }
}

/// `SpatialRadial<E>`: forward-differenced squared distance for radial lights.
fn spatial_radial(
    kind: Spatial,
    simple: bool,
    p: &SpatialParams,
    light_dist_tab: &[f32; LIGHT_DIST_LEN],
    light_dist: &[f32; LIGHT_DIST_LEN],
    src: &[u8],
    dest: &mut [u8],
) {
    let effect = Effect::init(kind, simple, p, light_dist_tab, light_dist);

    // Vertex = VertexBase - Location + VertexDU*MinU + VertexDV*MinV.
    let vertex = v_add(
        v_add(
            v_sub(p.vertex_base, p.actor_location),
            v_scale(p.vertex_du, p.min_u as f32),
        ),
        v_scale(p.vertex_dv, p.min_v as f32),
    );
    let rr = p.rradius * 4095.0;
    let scale = sq(rr);
    let dist = app_round(dot(vertex, vertex) * scale);
    let dist_u = app_round(dot(vertex, p.vertex_du) * scale);
    let dist_v = app_round(dot(vertex, p.vertex_dv) * scale);
    let dist_uu = app_round(dot(p.vertex_du, p.vertex_du) * scale);
    let dist_vv = app_round(dot(p.vertex_dv, p.vertex_dv) * scale);
    let dist_uv = app_round(dot(p.vertex_du, p.vertex_dv) * scale);

    let mut interp00: i32 = dist;
    let mut interp10: i32 = 2i32.wrapping_mul(dist_v).wrapping_add(dist_vv);
    let interp20: i32 = 2i32.wrapping_mul(dist_vv);
    let mut interp01: i32 = 2i32.wrapping_mul(dist_u).wrapping_add(dist_uu);
    let interp11: i32 = 2i32.wrapping_mul(dist_uv);
    let interp02: i32 = 2i32.wrapping_mul(dist_uu);

    let mut src_pos = (p.shadow_mask_u as isize * 8) * p.min_v as isize + p.min_u as isize;
    let mut dest_pos = p.uclamp as isize * p.min_v as isize + p.min_u as isize;
    let uskip = (p.uclamp - (p.max_u - p.min_u)) as isize;

    for _v in p.min_v..p.max_v {
        let mut inner0: u32 = interp00 as u32;
        let mut inner1: u32 = interp01 as u32;
        for _u in p.min_u..p.max_u {
            let d = dest_pos as usize;
            if inner0 < 4096 * 4096 && src[src_pos as usize] != 0 {
                let s = src[src_pos as usize];
                dest[d] = to_byte((s as f32) * effect.spatial((inner0 >> 12) as usize, vertex));
            } else {
                dest[d] = 0;
            }
            src_pos += 1;
            dest_pos += 1;
            inner0 = inner0.wrapping_add(inner1);
            inner1 = inner1.wrapping_add(interp02 as u32);
        }
        interp00 = interp00.wrapping_add(interp10);
        interp10 = interp10.wrapping_add(interp20);
        interp01 = interp01.wrapping_add(interp11);
        src_pos += uskip + p.shadow_skip as isize;
        dest_pos += uskip;
    }
}
