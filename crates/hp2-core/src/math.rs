//! `UnMath.cpp` and active `UnMath.h` mathematical policies. Native public
//! types retain layout, construction, archive dispatch, and typed ABI shells;
//! scalar decisions, arithmetic, transformations and reference-alias ordering
//! are implemented here.
//!
//! Bit-exactness: the oracle is clang `-O3 -ffp-contract=on`. Clang fuses only
//! inside one C++ expression. A left-associated sum of products `a0*b0 + a1*b1 + ...`
//! becomes `fma(a0, b0, round(a1*b1))` chained with `fma(ak, bk, acc)`. A difference
//! `p*q - r*s` becomes `fma(p, q, -round(r*s))`. Calls and separate statements
//! (including `FVector` operator temporaries) do not fuse. Each fused site below is
//! pinned to the oracle disassembly and is covered by the differential fixture.
//!
//! libm: `appSin/appCos/appAcos/appAtan2/appSqrt/appPow` take `DOUBLE`. They are
//! called here through `f64` methods with the same argument casts as the C++.

use std::cell::UnsafeCell;
use std::f32::consts::PI;
use std::sync::Once;

use hp2_abi::{FCoords, FPlane, FVector};

/// `MAXWORD`.
pub const MAXWORD: f32 = 65535.0;
/// `SMALL_NUMBER`.
pub const SMALL_NUMBER: f32 = 1.0e-8;
/// `DELTA`.
pub const DELTA: f32 = 0.00001;
/// `FGlobalMath::NUM_ANGLES`.
pub const NUM_ANGLES: usize = 16384;
/// `FGlobalMath` `APPROX_MAN_BITS`.
pub const APPROX_MAN_BITS: u32 = 10;
/// `FGlobalMath` `APPROX_EXP_BITS`.
pub const APPROX_EXP_BITS: u32 = 9;

/// Native `FScale` storage: the axis is a byte, followed by zeroed static padding.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeScale {
    pub scale: FVector,
    pub sheer_rate: f32,
    pub sheer_axis: u8,
    padding: [u8; 3],
}
const _: () = {
    assert!(size_of::<NativeScale>() == 20);
    assert!(align_of::<NativeScale>() == 4);
    assert!(std::mem::offset_of!(NativeScale, sheer_axis) == 16);
};

// These symbols are declared with native value types in UnMath.h. Rust and C++
// address the same immutable storage; no constructor or getter creates a copy.
/// cbindgen:ignore
#[unsafe(export_name = "hp2_rs_core_math_world_min")]
pub static WORLD_MIN: FVector = v3(-32700.0, -32700.0, -32700.0);
/// cbindgen:ignore
#[unsafe(export_name = "hp2_rs_core_math_world_max")]
pub static WORLD_MAX: FVector = v3(32700.0, 32700.0, 32700.0);
/// cbindgen:ignore
#[unsafe(export_name = "hp2_rs_core_math_unit_coords")]
pub static UNIT_COORDS: FCoords = FCoords {
    origin: v3(0.0, 0.0, 0.0),
    x_axis: v3(1.0, 0.0, 0.0),
    y_axis: v3(0.0, 1.0, 0.0),
    z_axis: v3(0.0, 0.0, 1.0),
};
/// cbindgen:ignore
#[unsafe(export_name = "hp2_rs_core_math_unit_scale")]
pub static UNIT_SCALE: NativeScale = NativeScale {
    scale: v3(1.0, 1.0, 1.0),
    sheer_rate: 0.0,
    sheer_axis: 5, // SHEER_ZX
    padding: [0; 3],
};
/// cbindgen:ignore
#[unsafe(export_name = "hp2_rs_core_math_view_coords")]
pub static VIEW_COORDS: FCoords = FCoords {
    origin: v3(0.0, 0.0, 0.0),
    x_axis: v3(0.0, 1.0, 0.0),
    y_axis: v3(0.0, 0.0, -1.0),
    z_axis: v3(1.0, 0.0, 0.0),
};

/// `FRotator` layout mirror (`INT Pitch, Yaw, Roll`). hp2-abi does not carry it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FRotator {
    pub pitch: i32,
    pub yaw: i32,
    pub roll: i32,
}
const _: () = assert!(size_of::<FRotator>() == 12);

/// `FQuat` layout mirror (`FLOAT X, Y, Z, W`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}
const _: () = assert!(size_of::<FQuat>() == 16);

/// `FBox` layout mirror (`FVector Min, Max; BYTE IsValid`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FBox {
    pub min: FVector,
    pub max: FVector,
    pub is_valid: u8,
}
const _: () = assert!(size_of::<FBox>() == 28);

/// `FPlace` layout mirror (`FQuat Quat; FVector Pos`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FPlace {
    pub quat: FQuat,
    pub pos: FVector,
}
const _: () = assert!(size_of::<FPlace>() == 28);

// ---------------------------------------------------------------------------
// Scalar and vector helpers (fusion-faithful).

#[inline]
const fn v3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

#[inline]
fn zero3() -> FVector {
    v3(0.0, 0.0, 0.0)
}

#[inline]
fn xyz(p: FPlane) -> FVector {
    v3(p.x, p.y, p.z)
}

/// `Abs` from UnMath/UnTemplate: `A>=0 ? A : -A`.
#[inline]
fn abs(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `::Min` (UnTemplate.h) as compiled: `fcsel(a, b, ls)`/`bif` select the accumulator `a` when `a <= b`
/// (`fcmp`/`fcmge` in the oracle FBox loop), so equal operands like -0/+0 keep `a`.
#[inline]
fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `::Max` (UnTemplate.h) as compiled: selects `a` when `a >= b`, so equal operands keep `a`.
#[inline]
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `FVector::operator|`: `X*V.X + Y*V.Y + Z*V.Z`, fused.
#[inline]
pub(crate) fn dot3(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `FQuat::operator|`: `X*Q.X + Y*Q.Y + Z*Q.Z + W*Q.W`, fused.
#[inline]
fn dot4(a: FQuat, b: FQuat) -> f32 {
    a.w.mul_add(b.w, a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y)))
}

/// `cross(a, b)` as the oracle emits it inside `FVector::FindBestAxisVectors` (`UnMath.cpp` 110-111,
/// `UnMath.h` 681-683). The oracle computes each component as `fnmul t, p, q` followed by
/// `fmadd r, u, v, t`, with the negated product first. Release codegen for the portable form can
/// rewrite that into a different NaN-producing sequence, so a NaN in `this` (for example `inf * 0`
/// upstream) would flip sign. The aarch64 `asm!` block emits the oracle's instruction sequence
/// verbatim, so LLVM cannot refold it.
#[cfg(target_arch = "aarch64")]
#[inline]
fn cross_fnmul_fmadd(a: FVector, b: FVector) -> FVector {
    let (x, y, z): (f32, f32, f32);
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!(
            "fnmul {t0:s}, {by:s}, {az:s}",
            "fmadd {x:s}, {ay:s}, {bz:s}, {t0:s}",
            "fnmul {t1:s}, {bz:s}, {ax:s}",
            "fmadd {y:s}, {az:s}, {bx:s}, {t1:s}",
            "fnmul {t2:s}, {bx:s}, {ay:s}",
            "fmadd {z:s}, {ax:s}, {by:s}, {t2:s}",
            ax = in(vreg) a.x,
            ay = in(vreg) a.y,
            az = in(vreg) a.z,
            bx = in(vreg) b.x,
            by = in(vreg) b.y,
            bz = in(vreg) b.z,
            t0 = out(vreg) _,
            t1 = out(vreg) _,
            t2 = out(vreg) _,
            x = out(vreg) x,
            y = out(vreg) y,
            z = out(vreg) z,
            options(pure, nomem, nostack),
        );
    }
    v3(x, y, z)
}

/// Portable form of [`cross_fnmul_fmadd`] for non-aarch64 targets.
#[cfg(not(target_arch = "aarch64"))]
#[inline]
fn cross_fnmul_fmadd(a: FVector, b: FVector) -> FVector {
    v3(
        a.y.mul_add(b.z, -(a.z * b.y)),
        a.z.mul_add(b.x, -(a.x * b.z)),
        a.x.mul_add(b.y, -(a.y * b.x)),
    )
}

#[inline]
fn add(a: FVector, b: FVector) -> FVector {
    v3(a.x + b.x, a.y + b.y, a.z + b.z)
}

#[inline]
pub(crate) fn sub(a: FVector, b: FVector) -> FVector {
    v3(a.x - b.x, a.y - b.y, a.z - b.z)
}

#[inline]
pub(crate) fn scale(a: FVector, s: f32) -> FVector {
    v3(a.x * s, a.y * s, a.z * s)
}

/// `FVector::operator/(FLOAT)`: multiplies by `1.f/Scale`.
#[inline]
fn div_by(a: FVector, s: f32) -> FVector {
    let r = 1.0f32 / s;
    scale(a, r)
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
fn neg(a: FVector) -> FVector {
    v3(-a.x, -a.y, -a.z)
}

/// `FVector::SizeSquared` and `FVector::SafeNormal`/`Normalize` square sum: `X*X+Y*Y+Z*Z`.
#[inline]
fn sum_sq(v: FVector) -> f32 {
    dot3(v, v)
}

/// `operator<<(FVector, FCoords)`: per component `V.X*C.XAxis.c + V.Y*C.YAxis.c + V.Z*C.ZAxis.c + C.Origin.c`.
#[inline]
fn shift(v: FVector, c: &FCoords) -> FVector {
    let x =
        v.z.mul_add(c.z_axis.x, v.x.mul_add(c.x_axis.x, v.y * c.y_axis.x)) + c.origin.x;
    let y =
        v.z.mul_add(c.z_axis.y, v.x.mul_add(c.x_axis.y, v.y * c.y_axis.y)) + c.origin.y;
    let z =
        v.z.mul_add(c.z_axis.z, v.x.mul_add(c.x_axis.z, v.y * c.y_axis.z)) + c.origin.z;
    v3(x, y, z)
}

/// `FCoords::Inverse` origin (UnMath.cpp:141, `-Origin << Inv`, `C.Origin` = 0). The object emits
/// these exact negated/fused sequences (UnMath.h:2905-2907); LLVM refolds the same products into a
/// different FMA shape, which flips NaN signs. Pinned per component to reproduce the object's order.
#[cfg(target_arch = "aarch64")]
fn inverse_origin(o: FVector, inv: &FCoords) -> FVector {
    v3(
        inverse_origin_xy(o, inv.y_axis.x, inv.x_axis.x, inv.z_axis.x),
        inverse_origin_xy(o, inv.y_axis.y, inv.x_axis.y, inv.z_axis.y),
        inverse_origin_z(o, inv),
    )
}

/// Origin x or y: `fmul(Y, -O.y)`, `fmls(X, O.x)`, `fmls(Z, O.z)`, `+0` (object: `fmul.2s`, `fmls.2s`, `fmls.2s`, `fadd.2s`).
#[cfg(target_arch = "aarch64")]
fn inverse_origin_xy(o: FVector, yl: f32, xl: f32, zl: f32) -> f32 {
    let r: f32;
    // SAFETY: pure scalar FP arithmetic on register inputs; no memory, flags, or stack.
    unsafe {
        std::arch::asm!(
            "fmul {r:s}, {yl:s}, {noy:s}",
            "fmls {r:s}, {xl:s}, {ox:v}.s[0]",
            "fmls {r:s}, {zl:s}, {oz:v}.s[0]",
            "fadd {r:s}, {r:s}, {zero:s}",
            r = out(vreg) r,
            yl = in(vreg) yl,
            xl = in(vreg) xl,
            zl = in(vreg) zl,
            noy = in(vreg) -o.y,
            ox = in(vreg) o.x,
            oz = in(vreg) o.z,
            zero = in(vreg) 0.0f32,
            options(pure, nomem, nostack),
        );
    }
    r
}

/// Origin z: `fnmul(O.y, Y.z)`, `fmls(O.x, X.z)`, `fmsub(O.z, Z.z)`, `+0`.
#[cfg(target_arch = "aarch64")]
fn inverse_origin_z(o: FVector, inv: &FCoords) -> f32 {
    let mut r = 0.0f32;
    // SAFETY: pure scalar FP arithmetic on register inputs; no memory, flags, or stack.
    unsafe {
        std::arch::asm!(
            "fnmul {r:s}, {oy:s}, {yz:s}",
            "fmls {r:s}, {ox:s}, {xz:v}.s[0]",
            "fmsub {r:s}, {oz:s}, {zz:s}, {r:s}",
            "fadd {r:s}, {r:s}, {zero:s}",
            r = inout(vreg) r,
            oy = in(vreg) o.y,
            yz = in(vreg) inv.y_axis.z,
            ox = in(vreg) o.x,
            xz = in(vreg) inv.x_axis.z,
            oz = in(vreg) o.z,
            zz = in(vreg) inv.z_axis.z,
            zero = in(vreg) 0.0f32,
            options(pure, nomem, nostack),
        );
    }
    r
}

#[cfg(not(target_arch = "aarch64"))]
fn inverse_origin(o: FVector, inv: &FCoords) -> FVector {
    shift(neg(o), inv)
}

/// `SinTab(i)`.
#[inline]
fn sin_tab(trig: &[f32; NUM_ANGLES], i: i32) -> f32 {
    trig[((i >> 2) & (NUM_ANGLES as i32 - 1)) as usize]
}

/// `CosTab(i)`.
#[inline]
fn cos_tab(trig: &[f32; NUM_ANGLES], i: i32) -> f32 {
    trig[((i.wrapping_add(16384) >> 2) & (NUM_ANGLES as i32 - 1)) as usize]
}

// ---------------------------------------------------------------------------
// FGlobalMath's canonical, process-wide table backing.

struct MathTables {
    trig: [f32; NUM_ANGLES],
    sqrt_man: [f32; 2 << APPROX_MAN_BITS],
    div_sqrt_man: [f32; 1 << APPROX_MAN_BITS],
    div_man: [f32; 1 << APPROX_MAN_BITS],
    div_sqrt_exp: [f32; 1 << APPROX_EXP_BITS],
    div_exp: [f32; 1 << APPROX_EXP_BITS],
}

struct MathTableStorage(UnsafeCell<MathTables>);

// SAFETY: INIT_TABLES is the sole writer. All readers pass its publication
// barrier, and no mutable reference exists after initialization.
unsafe impl Sync for MathTableStorage {}

static INIT_TABLES: Once = Once::new();
static TABLES: MathTableStorage = MathTableStorage(UnsafeCell::new(MathTables {
    trig: [0.0; NUM_ANGLES],
    sqrt_man: [0.0; 2 << APPROX_MAN_BITS],
    div_sqrt_man: [0.0; 1 << APPROX_MAN_BITS],
    div_man: [0.0; 1 << APPROX_MAN_BITS],
    div_sqrt_exp: [0.0; 1 << APPROX_EXP_BITS],
    div_exp: [0.0; 1 << APPROX_EXP_BITS],
}));

/// Initializes the canonical tables in static storage, without an allocation or
/// a stack-sized table temporary. Native construction and lazy Rust access share
/// this path. `fill_tables` only performs scalar arithmetic/libm operations: it
/// never invokes callbacks or table getters, so initialization cannot reenter
/// this `Once` or publish a partially filled table.
pub fn initialize_tables() {
    INIT_TABLES.call_once(|| {
        // SAFETY: Once grants exclusive initialization access before publication.
        let tables = unsafe { &mut *TABLES.0.get() };
        fill_tables(
            &mut tables.trig,
            &mut tables.sqrt_man,
            &mut tables.div_sqrt_man,
            &mut tables.div_man,
            &mut tables.div_sqrt_exp,
            &mut tables.div_exp,
        );
    });
}

#[inline]
fn tables() -> &'static MathTables {
    initialize_tables();
    // SAFETY: initialization has completed and the storage is now immutable.
    unsafe { &*TABLES.0.get() }
}

/// Canonical `TrigFLOAT`, lazily initialized for native-free callers.
#[inline]
pub fn trig_table() -> &'static [f32; NUM_ANGLES] {
    &tables().trig
}

/// Canonical square-root mantissa table.
#[inline]
pub fn sqrt_man_table() -> &'static [f32; 2 << APPROX_MAN_BITS] {
    &tables().sqrt_man
}

/// Canonical reciprocal-square-root mantissa table.
#[inline]
pub fn div_sqrt_man_table() -> &'static [f32; 1 << APPROX_MAN_BITS] {
    &tables().div_sqrt_man
}

/// Canonical reciprocal mantissa table.
#[inline]
pub fn div_man_table() -> &'static [f32; 1 << APPROX_MAN_BITS] {
    &tables().div_man
}

/// Canonical reciprocal-square-root exponent table.
#[inline]
pub fn div_sqrt_exp_table() -> &'static [f32; 1 << APPROX_EXP_BITS] {
    &tables().div_sqrt_exp
}

/// Canonical reciprocal exponent table.
#[inline]
pub fn div_exp_table() -> &'static [f32; 1 << APPROX_EXP_BITS] {
    &tables().div_exp
}

/// `SetupTable(ManTbl, ExpTbl, Power)`. The union is emulated with `u32` bit
/// patterns, including the masks that carry state between iterations.
fn setup_table(man_tbl: &mut [f32], exp_tbl: &mut [f32], power: f32) {
    let mut d: u32 = 1.0f32.to_bits();
    for i in 0..(1u32 << APPROX_EXP_BITS) {
        d = (d & 0x007f_ffff).wrapping_add(i << (32 - APPROX_EXP_BITS));
        let v = (abs(f32::from_bits(d)) as f64).powf(power as f64) as f32;
        exp_tbl[i as usize] = if v.is_nan() { 0.0 } else { v };
    }
    d = 1.0f32.to_bits();
    for i in 0..(1u32 << APPROX_MAN_BITS) {
        d = (d & 0xff80_0000).wrapping_add(i << (32 - APPROX_EXP_BITS - APPROX_MAN_BITS));
        let v = (abs(f32::from_bits(d)) as f64).powf(power as f64) as f32;
        man_tbl[i as usize] = if v.is_nan() { 0.0 } else { v };
    }
}

/// Fills caller-owned tables using the original `FGlobalMath` algorithms.
/// The canonical static owner uses this same implementation once, in place;
/// differential consumers may supply independent storage.
pub fn fill_tables(
    trig: &mut [f32; NUM_ANGLES],
    sqrt_man: &mut [f32; 2 << APPROX_MAN_BITS],
    div_sqrt_man: &mut [f32; 1 << APPROX_MAN_BITS],
    div_man: &mut [f32; 1 << APPROX_MAN_BITS],
    div_sqrt_exp: &mut [f32; 1 << APPROX_EXP_BITS],
    div_exp: &mut [f32; 1 << APPROX_EXP_BITS],
) {
    for (i, t) in trig.iter_mut().enumerate() {
        let angle = (i as f32) * 2.0f32 * PI / (NUM_ANGLES as f32);
        *t = (angle as f64).sin() as f32;
    }
    for d in 0..(1usize << APPROX_MAN_BITS) {
        let mut bits = 1.0f32.to_bits();
        bits = (bits & 0xff80_0000).wrapping_add((d as u32) << (23 - APPROX_MAN_BITS));
        let root = (f32::from_bits(bits) as f64).sqrt() as f32;
        // Exponent bias re-adjust, then the stored value uses the adjusted float.
        let adjusted = f32::from_bits(root.to_bits().wrapping_sub(64 << 23));
        sqrt_man[d] = (adjusted as f64 * 2.0f64.sqrt()) as f32; // odd exponents
        sqrt_man[d + (1 << APPROX_MAN_BITS)] = adjusted * 2.0f32;
    }
    setup_table(div_sqrt_man, div_sqrt_exp, -0.5);
    setup_table(div_man, div_exp, -1.0);
}

// ---------------------------------------------------------------------------
// FVector.

/// `FVector::Rotation`.
pub fn vector_rotation(v: FVector) -> FRotator {
    let two_pi = (2.0f32 * PI) as f64;
    let max = MAXWORD as f64;
    let yaw = ((v.y as f64).atan2(v.x as f64) * max / two_pi) as i32;
    let sq = v.x.mul_add(v.x, v.y * v.y);
    let pitch = ((v.z as f64).atan2((sq as f64).sqrt()) * max / two_pi) as i32;
    FRotator {
        pitch,
        yaw,
        roll: 0,
    }
}

/// `FVector::FindBestAxisVectors`. Returns `(Axis1, Axis2)`; the input `Axis1` is
/// overwritten unconditionally, so it is not an input.
pub fn vector_find_best_axis_vectors(this: FVector) -> (FVector, FVector) {
    let nx = abs(this.x);
    let ny = abs(this.y);
    let nz = abs(this.z);
    let a0 = if nz > nx && nz > ny {
        v3(1.0, 0.0, 0.0)
    } else {
        v3(0.0, 0.0, 1.0)
    };
    let d = dot3(a0, this);
    let a1 = vector_safe_normal(sub(a0, scale(this, d)));
    let a2 = cross_fnmul_fmadd(a1, this);
    (a1, a2)
}

/// `FVector::SafeNormal`.
pub fn vector_safe_normal(v: FVector) -> FVector {
    let sq = sum_sq(v);
    if sq < SMALL_NUMBER {
        return zero3();
    }
    let size = (sq as f64).sqrt() as f32;
    let s = 1.0f32 / size;
    v3(mul_rn(v.x, s), mul_rn(v.y, s), mul_rn(s, v.z))
}

/// Internal value-only form of `FVector::Normalize`.
fn vector_normalize(v: FVector) -> FVector {
    vector_normalize_inline(v).1
}

/// `FVector::MirrorByVector` (header inline): `*this - N*(2.f*(*this|N))`.
pub fn vector_mirror_by_vector(this: FVector, n: FVector) -> FVector {
    let dot = dot3(this, n);
    let s = add_rn(dot, dot);
    sub(this, v3(mul_rn(s, n.x), mul_rn(s, n.y), mul_rn(s, n.z)))
}

/// `FDistSquared(V1, V2)`: `Square(V2.X-V1.X) + Square(V2.Y-V1.Y) + Square(V2.Z-V1.Z)`.
fn dist_sq(v1: FVector, v2: FVector) -> f32 {
    let dx = v2.x - v1.x;
    let dy = v2.y - v1.y;
    let dz = v2.z - v1.z;
    dx * dx + dy * dy + dz * dz
}

// ---------------------------------------------------------------------------
// FCoords.

/// `FCoords::Inverse`. Lane order from the oracle disassembly of `FCoords::Inverse` (`-O3 -ffp-contract=on`):
/// each cofactor is one fused `fmadd` whose first multiplicand is the cofactor's own `Rn` (see the lane
/// comments); cofactor rows scale by `rdet` cofactor-first for X and Y.x, rdet-first for Y.y, Y.z and Z.
pub fn coords_inverse(c: &FCoords) -> FCoords {
    let x = c.x_axis;
    let y = c.y_axis;
    let z = c.z_axis;
    // Cofactors (oracle lane order; `fnmul` only where the oracle negates a product).
    let cxx = fmadd_rn(z.z, y.y, mul_rn(y.z, -z.y));
    let cxy = fmadd_rn(z.y, x.z, mul_rn(z.z, -x.y));
    let cxz = fmadd_rn(y.z, x.y, mul_rn(x.z, -y.y));
    let cyx = fmadd_rn(z.x, y.z, mul_rn(z.z, -y.x));
    let cyy = fmadd_rn(x.x, z.z, mul_rn(x.z, -z.x));
    let cyz = fmadd_rn(x.z, y.x, fnmul_rn(y.z, x.x));
    let czx = fmadd_rn(z.y, y.x, mul_rn(y.y, -z.x));
    let czy = fmadd_rn(z.x, x.y, mul_rn(z.y, -x.x));
    let czz = fmadd_rn(x.x, y.y, mul_rn(x.y, -y.x));
    // Determinant: the `Y.x` term in the triple is fused with `Yz` as the first multiplicand.
    let ytriple = fmadd_rn(y.z, z.x, mul_rn(-z.z, y.x));
    let triple = fmadd_rn(x.z, czx, fmadd_rn(x.x, cxx, mul_rn(ytriple, x.y)));
    let rdet = div_rn(1.0, triple);
    let mut inv = FCoords {
        origin: zero3(),
        x_axis: v3(mul_rn(cxx, rdet), mul_rn(cxy, rdet), mul_rn(cxz, rdet)),
        y_axis: v3(mul_rn(cyx, rdet), mul_rn(rdet, cyy), mul_rn(rdet, cyz)),
        z_axis: v3(mul_rn(rdet, czx), mul_rn(rdet, czy), mul_rn(rdet, czz)),
    };
    inv.origin = inverse_origin(c.origin, &inv);
    inv
}

/// `a * b` with `a` as the first AArch64 operand, which sets the NaN sign when both are NaN.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) fn mul_rn(a: f32, b: f32) -> f32 {
    let r: f32;
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!("fmul {r:s}, {a:s}, {b:s}", r = lateout(vreg) r, a = in(vreg) a, b = in(vreg) b, options(pure, nomem, nostack));
    }
    r
}

/// `a + b` with `a` as the first AArch64 operand (see [`mul_rn`]).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) fn add_rn(a: f32, b: f32) -> f32 {
    let r: f32;
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!("fadd {r:s}, {a:s}, {b:s}", r = lateout(vreg) r, a = in(vreg) a, b = in(vreg) b, options(pure, nomem, nostack));
    }
    r
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub(crate) fn mul_rn(a: f32, b: f32) -> f32 {
    a * b
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub(crate) fn add_rn(a: f32, b: f32) -> f32 {
    a + b
}

/// `a * b + c` fused, with `a` as the first AArch64 multiplicand (`fmadd`/`fmla` operand order).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) fn fmadd_rn(a: f32, b: f32, c: f32) -> f32 {
    let r: f32;
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!("fmadd {r:s}, {a:s}, {b:s}, {c:s}", r = lateout(vreg) r, a = in(vreg) a, b = in(vreg) b, c = in(vreg) c, options(pure, nomem, nostack));
    }
    r
}

/// `-(a * b)` as one AArch64 `fnmul`, with `a` first.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn fnmul_rn(a: f32, b: f32) -> f32 {
    let r: f32;
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!("fnmul {r:s}, {a:s}, {b:s}", r = lateout(vreg) r, a = in(vreg) a, b = in(vreg) b, options(pure, nomem, nostack));
    }
    r
}

/// `c - a*b` as one native fused instruction, without negating a NaN operand.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn fmsub_rn(a: f32, b: f32, c: f32) -> f32 {
    let r: f32;
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!("fmsub {r:s}, {a:s}, {b:s}, {c:s}", r = lateout(vreg) r,
            a = in(vreg) a, b = in(vreg) b, c = in(vreg) c,
            options(pure, nomem, nostack));
    }
    r
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
fn fmsub_rn(a: f32, b: f32, c: f32) -> f32 {
    (-a).mul_add(b, c)
}

/// `a / b` as one AArch64 `fdiv`.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn div_rn(a: f32, b: f32) -> f32 {
    let r: f32;
    // SAFETY: pure register arithmetic; no memory or stack access.
    unsafe {
        std::arch::asm!("fdiv {r:s}, {a:s}, {b:s}", r = lateout(vreg) r, a = in(vreg) a, b = in(vreg) b, options(pure, nomem, nostack));
    }
    r
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub(crate) fn fmadd_rn(a: f32, b: f32, c: f32) -> f32 {
    a.mul_add(b, c)
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
fn fnmul_rn(a: f32, b: f32) -> f32 {
    -(a * b)
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
fn div_rn(a: f32, b: f32) -> f32 {
    a / b
}

/// `FCoords::operator/=(const FCoords&)`: compiled order from `UnMath.cpp` 148-151 (oracle disassembly).
/// X/Y lanes multiply the `T` axis first; the Z lane multiplies the scalar first; each sum accumulates
/// left with the next term second; the origin adds `T.Origin` second.
pub fn coords_div_assign(c: FCoords, t: &FCoords) -> FCoords {
    let term =
        |s: f32, ax: FVector| -> FVector { v3(mul_rn(ax.x, s), mul_rn(ax.y, s), mul_rn(s, ax.z)) };
    let comb = |a: FVector| -> FVector {
        let p0 = term(a.x, t.x_axis);
        let p1 = term(a.y, t.y_axis);
        let p2 = term(a.z, t.z_axis);
        v3(
            add_rn(add_rn(p0.x, p1.x), p2.x),
            add_rn(add_rn(p0.y, p1.y), p2.y),
            add_rn(add_rn(p0.z, p1.z), p2.z),
        )
    };
    let o = comb(c.origin);
    FCoords {
        origin: v3(
            add_rn(o.x, t.origin.x),
            add_rn(o.y, t.origin.y),
            add_rn(o.z, t.origin.z),
        ),
        x_axis: comb(c.x_axis),
        y_axis: comb(c.y_axis),
        z_axis: comb(c.z_axis),
    }
}

/// `FCoords::operator/=(const FRotator&)` (header inline, `UnMath.h` 2344): inverse Roll, then Pitch,
/// then Yaw, as three inlined `operator*=(FCoords)` steps against constant axes. The compiled body
/// (`harness.o` `__ZN7FCoordsdVERK8FRotator`) keeps `-0.0` and `+0.0` as registers and fuses each lane
/// with a fixed `Rn`. LLVM refolds that per build, so the whole body is pinned in one `asm!` block.
/// `buf[0..12]` = origin, X, Y, Z; `buf[12..18]` = `CosTab(Roll)`, `SinTab(Roll)`, `CosTab(Pitch)`,
/// `SinTab(Pitch)`, `CosTab(Yaw)`, `SinTab(Yaw)`.
#[cfg(not(target_arch = "aarch64"))]
compile_error!(
    "hp2-core: coords_div_rotator is a verbatim aarch64 asm transcription; no other target is supported"
);
#[cfg(target_arch = "aarch64")]
pub fn coords_div_rotator(c: &FCoords, rot: FRotator, trig: &[f32; NUM_ANGLES]) -> FCoords {
    let mut buf = [
        c.origin.x,
        c.origin.y,
        c.origin.z,
        c.x_axis.x,
        c.x_axis.y,
        c.x_axis.z,
        c.y_axis.x,
        c.y_axis.y,
        c.y_axis.z,
        c.z_axis.x,
        c.z_axis.y,
        c.z_axis.z,
        cos_tab(trig, rot.roll),
        sin_tab(trig, rot.roll),
        cos_tab(trig, rot.pitch),
        sin_tab(trig, rot.pitch),
        cos_tab(trig, rot.yaw),
        sin_tab(trig, rot.yaw),
    ];
    // SAFETY: `buf` holds 18 contiguous f32 and every access is at byte offset 0..72 from `p`. The
    // asm writes only inside `buf`, makes no calls, and touches no stack. It clobbers v0-v25.
    unsafe {
        std::arch::asm!(
            "ldr s21, [{p}, #48]",
            "ldr s22, [{p}, #52]",
            "ldp s4, s5, [{p}]",
            "movi v0.2s, #0x80, lsl #24",
            "fmul s1, s5, s0",
            "fadd s2, s4, s1",
            "movi v1.2d, #0",
            "ldp s6, s7, [{p}, #8]",
            "fmadd s2, s6, s1, s2",
            "fmul s3, s21, s5",
            "fmadd s3, s4, s0, s3",
            "fmadd s3, s6, s22, s3",
            "fnmul s5, s22, s5",
            "fmadd s4, s4, s1, s5",
            "fmadd s4, s6, s21, s4",
            "stp s2, s3, [{p}]",
            "ldp s16, s17, [{p}, #16]",
            "fmul s5, s16, s0",
            "fadd s5, s7, s5",
            "fmadd s5, s17, s1, s5",
            "fmul s6, s21, s16",
            "fmadd s6, s7, s0, s6",
            "fmadd s6, s17, s22, s6",
            "fnmul s16, s22, s16",
            "fmadd s7, s7, s1, s16",
            "fmadd s7, s17, s21, s7",
            "stp s4, s5, [{p}, #8]",
            "stp s6, s7, [{p}, #16]",
            "ldp s18, s19, [{p}, #24]",
            "fmul s16, s19, s0",
            "fadd s16, s18, s16",
            "ldp s20, s23, [{p}, #32]",
            "fmadd s16, s20, s1, s16",
            "fmul s17, s21, s19",
            "fmadd s17, s18, s0, s17",
            "fmadd s17, s20, s22, s17",
            "fnmul s19, s22, s19",
            "fmadd s18, s18, s1, s19",
            "fmadd s18, s20, s21, s18",
            "stp s16, s17, [{p}, #24]",
            "ldp s24, s25, [{p}, #40]",
            "fmul s19, s24, s0",
            "fadd s19, s23, s19",
            "fmadd s19, s25, s1, s19",
            "fmul s20, s21, s24",
            "fmadd s20, s23, s0, s20",
            "fmadd s20, s25, s22, s20",
            "fnmul s22, s22, s24",
            "fmadd s22, s23, s1, s22",
            "fmadd s21, s25, s21, s22",
            "stp s18, s19, [{p}, #32]",
            "stp s20, s21, [{p}, #40]",
            "ldr s22, [{p}, #56]",
            "ldr s23, [{p}, #60]",
            "fmul s24, s3, s1",
            "fmadd s25, s2, s22, s24",
            "fmsub s25, s4, s23, s25",
            "fmadd s3, s2, s1, s3",
            "fmadd s3, s4, s0, s3",
            "fmadd s2, s2, s23, s24",
            "fmadd s2, s4, s22, s2",
            "stp s25, s3, [{p}]",
            "fmul s4, s6, s1",
            "fmadd s24, s5, s22, s4",
            "fmsub s24, s7, s23, s24",
            "fmadd s6, s5, s1, s6",
            "fmadd s6, s7, s0, s6",
            "fmadd s4, s5, s23, s4",
            "fmadd s4, s7, s22, s4",
            "stp s2, s24, [{p}, #8]",
            "stp s6, s4, [{p}, #16]",
            "fmul s5, s17, s1",
            "fmadd s7, s16, s22, s5",
            "fmsub s7, s18, s23, s7",
            "fmadd s17, s16, s1, s17",
            "fmadd s17, s18, s0, s17",
            "fmadd s5, s16, s23, s5",
            "fmadd s5, s18, s22, s5",
            "stp s7, s17, [{p}, #24]",
            "fmul s16, s20, s1",
            "fmadd s18, s19, s22, s16",
            "fmsub s18, s21, s23, s18",
            "fmadd s20, s19, s1, s20",
            "fmadd s20, s21, s0, s20",
            "fmadd s16, s19, s23, s16",
            "fmadd s16, s21, s22, s16",
            "stp s5, s18, [{p}, #32]",
            "stp s20, s16, [{p}, #40]",
            "ldr s19, [{p}, #64]",
            "ldr s21, [{p}, #68]",
            "fnmul s22, s21, s3",
            "fmadd s22, s25, s19, s22",
            "fmadd s22, s2, s0, s22",
            "fmul s23, s3, s19",
            "fmadd s23, s25, s21, s23",
            "fmadd s23, s2, s1, s23",
            "fmul s3, s3, s1",
            "fmadd s3, s25, s0, s3",
            "fadd s2, s2, s3",
            "stp s22, s23, [{p}]",
            "fnmul s3, s21, s6",
            "fmadd s3, s24, s19, s3",
            "fmadd s3, s4, s0, s3",
            "fmul s22, s6, s19",
            "fmadd s22, s24, s21, s22",
            "fmadd s22, s4, s1, s22",
            "fmul s6, s6, s1",
            "fmadd s6, s24, s0, s6",
            "fadd s4, s4, s6",
            "stp s2, s3, [{p}, #8]",
            "stp s22, s4, [{p}, #16]",
            "fnmul s2, s21, s17",
            "fmadd s2, s7, s19, s2",
            "fmadd s2, s5, s0, s2",
            "fmul s3, s17, s19",
            "fmadd s3, s7, s21, s3",
            "fmadd s3, s5, s1, s3",
            "fmul s4, s17, s1",
            "fmadd s4, s7, s0, s4",
            "fadd s4, s5, s4",
            "stp s2, s3, [{p}, #24]",
            "fnmul s2, s21, s20",
            "fmadd s2, s18, s19, s2",
            "fmadd s2, s16, s0, s2",
            "fmul s3, s20, s19",
            "fmadd s3, s18, s21, s3",
            "fmadd s3, s16, s1, s3",
            "fmul s1, s20, s1",
            "fmadd s0, s18, s0, s1",
            "stp s4, s2, [{p}, #32]",
            "fadd s0, s16, s0",
            "stp s3, s0, [{p}, #40]",
            p = in(reg) buf.as_mut_ptr(),
            out("v0") _, out("v1") _, out("v2") _, out("v3") _, out("v4") _, out("v5") _,
            out("v6") _, out("v7") _, out("v8") _, out("v9") _, out("v10") _, out("v11") _,
            out("v12") _, out("v13") _, out("v14") _, out("v15") _, out("v16") _, out("v17") _,
            out("v18") _, out("v19") _, out("v20") _, out("v21") _, out("v22") _, out("v23") _,
            out("v24") _, out("v25") _,
            options(nostack),
        );
    }
    FCoords {
        origin: v3(buf[0], buf[1], buf[2]),
        x_axis: v3(buf[3], buf[4], buf[5]),
        y_axis: v3(buf[6], buf[7], buf[8]),
        z_axis: v3(buf[9], buf[10], buf[11]),
    }
}

/// `FRotator::Vector` (header inline): `(GMath.UnitCoords / *this).XAxis`.
pub fn rotator_vector(rot: FRotator, trig: &[f32; NUM_ANGLES]) -> FVector {
    coords_div_rotator(&unit_coords(), rot, trig).x_axis
}

/// Returns the canonical identity coordinate system by value.
pub fn unit_coords() -> FCoords {
    UNIT_COORDS
}

/// `FCoords::OrthoRotation`.
pub fn coords_ortho_rotation(c: &FCoords, trig: &[f32; NUM_ANGLES]) -> FRotator {
    let x = c.x_axis;
    let sq_sum = x.x * x.x + x.y * x.y; // Square() calls: not fused
    let pitch = ((x.z as f64).atan2((sq_sum as f64).sqrt()) * 32768.0 / PI as f64) as i32;
    let yaw = ((x.y as f64).atan2(x.x as f64) * 32768.0 / PI as f64) as i32;
    let r = FRotator {
        pitch,
        yaw,
        roll: 0,
    };
    let s = coords_div_rotator(&unit_coords(), r, trig);
    let roll = ((dot3(c.z_axis, s.y_axis) as f64).atan2(dot3(c.y_axis, s.y_axis) as f64) * 32768.0
        / PI as f64) as i32;
    FRotator { roll, ..r }
}

/// `FCoords::FCoords(const FPlace&)`.
pub fn coords_from_place(p: &FPlace) -> FCoords {
    let q = p.quat;
    let x2 = q.x + q.x;
    let y2 = q.y + q.y;
    let z2 = q.z + q.z;
    let xx = q.x * x2;
    let xy = q.x * y2;
    let xz = q.x * z2;
    let yy = q.y * y2;
    let yz = q.y * z2;
    let zz = q.z * z2;
    let wx = q.w * x2;
    let wy = q.w * y2;
    let wz = q.w * z2;
    FCoords {
        origin: p.pos,
        x_axis: v3(1.0 - (yy + zz), xy - wz, xz + wy),
        y_axis: v3(xy + wz, 1.0 - (xx + zz), yz - wx),
        z_axis: v3(xz - wy, yz + wx, 1.0 - (xx + yy)),
    }
}

// ---------------------------------------------------------------------------
// FBox / FSphere.

impl FBox {
    /// `FBox::operator+=(const FVector&)`.
    fn add_point(&mut self, p: FVector) {
        if self.is_valid != 0 {
            self.min.x = min_f(self.min.x, p.x);
            self.min.y = min_f(self.min.y, p.y);
            self.min.z = min_f(self.min.z, p.z);
            self.max.x = max_f(self.max.x, p.x);
            self.max.y = max_f(self.max.y, p.y);
            self.max.z = max_f(self.max.z, p.z);
        } else {
            self.min = p;
            self.max = p;
            self.is_valid = 1;
        }
    }
}

/// `FBox::FBox(const FVector* Points, INT Count)`. Pass the first `Count` points
/// as a slice (empty for `Count <= 0`).
pub fn box_from_points(pts: &[FVector]) -> FBox {
    let mut b = FBox {
        min: zero3(),
        max: zero3(),
        is_valid: 0,
    };
    for p in pts {
        b.add_point(*p);
    }
    b
}

/// `FBox::FBox(const FCoords&)`. Compiled as `fadd s0, s0, s1` with the axis component first
/// (`__ZN4FBoxC2ERK7FCoords`, oracle 0xf18): the axis is the AArch64 first operand, which sets the
/// NaN sign when both operands are NaN.
pub fn box_from_coords(c: &FCoords) -> FBox {
    let mut min = c.origin;
    let mut max = c.origin;
    for a in [c.x_axis, c.y_axis, c.z_axis] {
        if a.x >= 0.0 {
            max.x = add_rn(a.x, max.x);
        } else {
            min.x = add_rn(a.x, min.x);
        }
        if a.y >= 0.0 {
            max.y = add_rn(a.y, max.y);
        } else {
            min.y = add_rn(a.y, min.y);
        }
        if a.z >= 0.0 {
            max.z = add_rn(a.z, max.z);
        } else {
            min.z = add_rn(a.z, min.z);
        }
    }
    FBox {
        min,
        max,
        is_valid: 1,
    }
}

/// `operator<<(const FBox&, const FCoords&)`.
pub fn box_shift(b: &FBox, c: &FCoords) -> FCoords {
    FCoords {
        origin: shift(b.min, c),
        x_axis: scale(c.x_axis, b.max.x - b.min.x),
        y_axis: scale(c.y_axis, b.max.y - b.min.y),
        z_axis: scale(c.z_axis, b.max.z - b.min.z),
    }
}

/// `FBox::FBox(const FBox&, const FCoords&)`: `*this = FBox(Box << Coords)`.
pub fn box_from_box_coords(b: &FBox, c: &FCoords) -> FBox {
    box_from_coords(&box_shift(b, c))
}

/// `FBox::GetCoords`.
pub fn box_get_coords(b: &FBox) -> FCoords {
    FCoords {
        origin: b.min,
        x_axis: v3(b.max.x - b.min.x, 0.0, 0.0),
        y_axis: v3(0.0, b.max.y - b.min.y, 0.0),
        z_axis: v3(0.0, 0.0, b.max.z - b.min.z),
    }
}

/// `FSphere::FSphere(const FVector* Pts, INT Count)`. An empty slice yields the
/// zero plane, the same as the `if (Count)` guard in C++.
pub fn sphere_from_points(pts: &[FVector]) -> FPlane {
    if pts.is_empty() {
        return FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
    }
    let bx = box_from_points(pts);
    let c = div_by(add(bx.min, bx.max), 2.0);
    let mut w = 0.0f32;
    for p in pts {
        let d = dist_sq(*p, c);
        if d > w {
            w = d;
        }
    }
    w = ((w as f64).sqrt() * 1.001f32 as f64) as f32;
    FPlane {
        x: c.x,
        y: c.y,
        z: c.z,
        w,
    }
}

// ---------------------------------------------------------------------------
// FQuat / SlerpQuat.

/// `FQuat::Normalize`. Returns `false` in the reset branch (value ignored by callers).
fn quat_normalize(q: &mut FQuat) -> bool {
    let square_sum =
        q.w.mul_add(q.w, q.z.mul_add(q.z, q.x.mul_add(q.x, q.y * q.y)));
    if abs(square_sum - 1.0) > DELTA {
        if square_sum >= DELTA {
            let s = 1.5f32 - square_sum * 0.5f32;
            q.x *= s;
            q.y *= s;
            q.z *= s;
            q.w *= s;
        } else {
            q.x = 0.0;
            q.y = 0.0;
            q.z = 0.1;
            q.w = 0.0;
            return false;
        }
    }
    true
}

/// `SlerpQuat(const FQuat&, const FQuat&, FLOAT)`.
pub fn slerp_quat(quat1: FQuat, quat2: FQuat, slerp: f32) -> FQuat {
    let cosom = dot4(quat1, quat2);
    let abs_cosom = abs(cosom);
    if abs_cosom < 1.0 {
        let omega = (abs_cosom as f64).acos() as f32;
        let sininv = (1.0f32 as f64 / (omega as f64).sin()) as f32;
        let scale0 = ((((1.0f32 - slerp) * omega) as f64).sin() * sininv as f64) as f32;
        let mut scale1 = (((slerp * omega) as f64).sin() * sininv as f64) as f32;
        if cosom < 0.0 {
            scale1 = -scale1;
        }
        let mut result = FQuat {
            x: scale0.mul_add(quat1.x, scale1 * quat2.x),
            y: scale0.mul_add(quat1.y, scale1 * quat2.y),
            z: scale0.mul_add(quat1.z, scale1 * quat2.z),
            w: scale0.mul_add(quat1.w, scale1 * quat2.w),
        };
        quat_normalize(&mut result);
        result
    } else {
        quat1
    }
}

// ---------------------------------------------------------------------------
// Plane intersections (UnMath.h header inlines).

/// `FIntersectPlanes3`. Returns `(valid, I)`.
///
/// Operand order follows the oracle disassembly of `FIntersectPlanes3` (`-O3 -ffp-contract=on`):
/// each cross is fused lane-wise (`fmla` accumulator second), `Det = (C12 | P3)` is fused in the
/// order `fmadd(C12.z, P3.z, fmadd(C12.x, P3.x, P3.y * C12.y))`, each cross term multiplies the
/// cross first, and the result is `num * (1/Det)` with the reciprocal first in the Z lane.
pub fn intersect_planes3(p1: FPlane, p2: FPlane, p3: FPlane) -> (bool, FVector) {
    let (a, b, c) = (xyz(p1), xyz(p2), xyz(p3));
    let c12 = v3(
        fmadd_rn(b.z, a.y, mul_rn(a.z, -b.y)),
        fmadd_rn(b.x, a.z, mul_rn(a.x, -b.z)),
        fmadd_rn(a.x, b.y, mul_rn(-b.x, a.y)),
    );
    let c23 = v3(
        fmadd_rn(c.z, b.y, mul_rn(b.z, -c.y)),
        fmadd_rn(c.x, b.z, mul_rn(b.x, -c.z)),
        fmadd_rn(b.x, c.y, fnmul_rn(c.x, b.y)),
    );
    let c31 = v3(
        fmadd_rn(a.z, c.y, mul_rn(c.z, -a.y)),
        fmadd_rn(a.x, c.z, mul_rn(c.x, -a.z)),
        fmadd_rn(c.x, a.y, fnmul_rn(a.x, c.y)),
    );
    let det = fmadd_rn(c12.z, c.z, fmadd_rn(c12.x, c.x, mul_rn(c.y, c12.y)));
    let eps = 0.001f32 * 0.001f32;
    if det * det < eps {
        return (false, zero3());
    }
    let term =
        |cross: FVector, w: f32| v3(mul_rn(cross.x, w), mul_rn(cross.y, w), mul_rn(cross.z, w));
    let (t1, t2, t3) = (term(c23, p1.w), term(c31, p2.w), term(c12, p3.w));
    let num = v3(
        add_rn(add_rn(t1.x, t2.x), t3.x),
        add_rn(add_rn(t1.y, t2.y), t3.y),
        add_rn(add_rn(t1.z, t2.z), t3.z),
    );
    let recip = div_rn(1.0, det);
    (
        true,
        v3(
            mul_rn(num.x, recip),
            mul_rn(num.y, recip),
            mul_rn(recip, num.z),
        ),
    )
}

/// Cross product `a × b` with the second operand as the first AArch64 multiplicand in each fused lane.
/// Matches the oracle `FIntersectPlanes2` operand order; the multiplicand choice sets the NaN sign.
fn cross_b_first(a: FVector, b: FVector) -> FVector {
    v3(
        fmadd_rn(b.z, a.y, mul_rn(a.z, -b.y)),
        fmadd_rn(b.x, a.z, mul_rn(a.x, -b.z)),
        fmadd_rn(b.y, a.x, mul_rn(a.y, -b.x)),
    )
}

/// `FIntersectPlanes2`. Returns `(valid, I, D)`.
///
/// Compiled order from the oracle disassembly: cross lanes via [`cross_b_first`]; each cross term
/// multiplies the cross first; the sum accumulates left; the result is `num * (1/dd)` with the
/// reciprocal first in the Z lane.
pub fn intersect_planes2(p1: FPlane, p2: FPlane) -> (bool, FVector, FVector) {
    let (a, b) = (xyz(p1), xyz(p2));
    let d0 = cross_b_first(a, b);
    let dd = sum_sq(d0);
    let eps = 0.001f32 * 0.001f32;
    if dd < eps {
        return (false, zero3(), zero3());
    }
    let term =
        |cross: FVector, w: f32| v3(mul_rn(cross.x, w), mul_rn(cross.y, w), mul_rn(cross.z, w));
    let (t1, t2) = (
        term(cross_b_first(b, d0), p1.w),
        term(cross_b_first(d0, a), p2.w),
    );
    let num = v3(add_rn(t1.x, t2.x), add_rn(t1.y, t2.y), add_rn(t1.z, t2.z));
    let recip = div_rn(1.0, dd);
    let i = v3(
        mul_rn(num.x, recip),
        mul_rn(num.y, recip),
        mul_rn(recip, num.z),
    );
    (true, i, vector_normalize(d0))
}

// Active UnMath.h inline policies.
pub fn vector_cross(a: FVector, b: FVector) -> FVector {
    cross_fnmul_fmadd(a, b)
}

pub fn vector_add(a: FVector, b: FVector) -> FVector {
    add(a, b)
}

pub fn vector_sub(a: FVector, b: FVector) -> FVector {
    sub(a, b)
}

pub fn vector_mul(a: FVector, b: FVector) -> FVector {
    v3(mul_rn(a.x, b.x), mul_rn(a.y, b.y), mul_rn(a.z, b.z))
}

pub fn vector_div_components(a: FVector, b: FVector) -> FVector {
    v3(div_rn(a.x, b.x), div_rn(a.y, b.y), div_rn(a.z, b.z))
}

pub fn vector_min(a: FVector, b: FVector) -> FVector {
    v3(min_f(a.x, b.x), min_f(a.y, b.y), min_f(a.z, b.z))
}

pub fn vector_max(a: FVector, b: FVector) -> FVector {
    v3(max_f(a.x, b.x), max_f(a.y, b.y), max_f(a.z, b.z))
}

pub fn vector_scale(a: FVector, s: f32) -> FVector {
    v3(mul_rn(a.x, s), mul_rn(a.y, s), mul_rn(a.z, s))
}

pub fn vector_div(a: FVector, s: f32) -> FVector {
    let r = 1.0 / s;
    v3(mul_rn(r, a.x), mul_rn(r, a.y), mul_rn(r, a.z))
}

pub fn vector_bound(a: FVector, s: f32) -> FVector {
    v3(
        clamp_inline(a.x, -s, s),
        clamp_inline(a.y, -s, s),
        clamp_inline(a.z, -s, s),
    )
}

/// Native Clamp lowers its upper comparison to FMINNM. Unlike Rust's `min`,
/// this quiets and propagates a signaling NaN while ignoring a quiet NaN.
pub fn clamp_inline(a: f32, lo: f32, hi: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    let upper = {
        let result: f32;
        // SAFETY: register-only IEEE arithmetic; preserves native NaN handling.
        unsafe {
            std::arch::asm!("fminnm {r:s}, {a:s}, {b:s}", r = lateout(vreg) result,
                a = in(vreg) a, b = in(vreg) hi, options(pure, nomem, nostack));
        }
        result
    };
    #[cfg(not(target_arch = "aarch64"))]
    let upper = a.min(hi);
    if a < lo { lo } else { upper }
}

pub fn vector_neg(a: FVector) -> FVector {
    v3(-a.x, -a.y, -a.z)
}

pub fn vector_projection(a: FVector) -> FVector {
    let r = 1.0 / a.z;
    v3(a.x * r, a.y * r, 1.0)
}

pub fn vector_unsafe_normal(a: FVector) -> FVector {
    let s = (1.0 / (sum_sq(a) as f64).sqrt()) as f32;
    scale(a, s)
}

pub fn vector_dot(a: FVector, b: FVector) -> f32 {
    dot3(a, b)
}

pub fn vector_distance_sq(a: FVector, b: FVector) -> f32 {
    dist_sq(a, b)
}

pub fn vector_distance(a: FVector, b: FVector) -> f32 {
    (dist_sq(a, b) as f64).sqrt() as f32
}

pub fn vector_equal(a: FVector, b: FVector) -> i32 {
    (a.x == b.x && a.y == b.y && a.z == b.z) as i32
}

pub fn vector_not_equal(a: FVector, b: FVector) -> i32 {
    (a.x != b.x || a.y != b.y || a.z != b.z) as i32
}

pub fn vector_size(a: FVector) -> f32 {
    (sum_sq(a) as f64).sqrt() as f32
}

pub fn vector_size_sq(a: FVector) -> f32 {
    sum_sq(a)
}

pub fn vector_size_2d(a: FVector) -> f32 {
    (a.x.mul_add(a.x, a.y * a.y) as f64).sqrt() as f32
}

pub fn vector_size_sq_2d(a: FVector) -> f32 {
    a.x.mul_add(a.x, a.y * a.y)
}

pub fn vector_div_size(a: FVector) -> f32 {
    1.0 / vector_size(a)
}

pub fn vector_max_val(a: FVector) -> f32 {
    max_f(max_f(abs(a.x), abs(a.y)), abs(a.z))
}

pub fn vector_nearly_zero(a: FVector) -> i32 {
    (abs(a.x) < 0.0001 && abs(a.y) < 0.0001 && abs(a.z) < 0.0001) as i32
}

pub fn vector_zero(a: FVector) -> i32 {
    (a.x == 0.0 && a.y == 0.0 && a.z == 0.0) as i32
}

pub fn vector_booleanize(a: FVector) -> i32 {
    if a.x > 0.0 {
        1
    } else if a.x < 0.0 {
        0
    } else if a.y > 0.0 {
        1
    } else if a.y < 0.0 {
        0
    } else {
        (a.z >= 0.0) as i32
    }
}

pub fn vector_grid(a: FVector, b: FVector) -> FVector {
    v3(snap(a.x, b.x), snap(a.y, b.y), snap(a.z, b.z))
}

pub fn vector_interp(a: FVector, b: FVector, s: f32) -> FVector {
    let r = 1.0 - s;
    v3(
        fmadd_rn(a.x, r, b.x * s),
        fmadd_rn(a.y, r, b.y * s),
        fmadd_rn(a.z, r, b.z * s),
    )
}

pub fn vector_add_bounded(a: FVector, b: FVector, s: f32) -> FVector {
    vector_bound(add(a, b), s)
}

pub fn fix_int(a: i32) -> i32 {
    a.wrapping_shl(16)
}

pub fn fix_float(a: f32) -> i32 {
    (a * 65536.0) as i32
}

pub fn unfix(a: i32) -> i32 {
    a >> 16
}

pub fn snap(a: f32, grid: f32) -> f32 {
    if grid == 0.0 {
        a
    } else {
        ((fmadd_rn(0.5, grid, a) / grid) as i32 as f32) * grid
    }
}

pub fn sheer_snap(a: f32) -> f32 {
    if a < -0.65 {
        a + 0.15
    } else if a > 0.65 {
        a - 0.15
    } else if a < -0.55 {
        -0.5
    } else if a > 0.55 {
        0.5
    } else if a < -0.05 {
        a + 0.05
    } else if a > 0.05 {
        a - 0.05
    } else {
        0.0
    }
}

pub fn next_power(a: u32) -> u32 {
    if a == 0 || a > 65536 {
        0
    } else {
        a.next_power_of_two()
    }
}

pub fn reduce_angle(a: i32) -> i32 {
    a & 65532
}

pub fn angle_confined(a: i32, d: i32, lo: i32, hi: i32) -> u16 {
    if d < 0 {
        if d <= -65536 || d <= -((a.wrapping_sub(lo) as u16) as i32) {
            return lo as u16;
        }
    } else if d > 0 && (d >= 65536 || d >= ((hi.wrapping_sub(a) as u16) as i32)) {
        return hi as u16;
    }
    a.wrapping_add(d) as u16
}

pub fn positive_smaller(a: f32, b: f32) -> i32 {
    (a.to_bits() < b.to_bits()) as i32
}

pub fn positive_min(a: f32, b: f32) -> f32 {
    if a.to_bits() < b.to_bits() { a } else { b }
}

pub fn positive_equal(a: f32, b: f32) -> i32 {
    (a.to_bits() == b.to_bits()) as i32
}

pub fn negative_float(a: f32) -> i32 {
    (a.to_bits() >= 0x80000000) as i32
}

pub fn positive_max(a: f32, b: f32) -> f32 {
    if a.to_bits() < b.to_bits() { b } else { a }
}

pub fn positive_clamp(a: f32, b: f32, c: f32) -> f32 {
    if a.to_bits() < b.to_bits() {
        b
    } else if a.to_bits() > c.to_bits() {
        c
    } else {
        a
    }
}

/// The original macro rereads its operands, evaluates the range lazily, and
/// performs no store when already in range. Actions are value/range reads and
/// a value store; each native access can fail before the next side effect.
pub fn clip_zero(mut access: impl FnMut(i32, f32) -> Result<f32, ()>) -> Result<(), ()> {
    if access(0, 0.0)?.to_bits() >= 0x80000000 {
        access(2, 0.0)?;
    } else if access(0, 0.0)?.to_bits() > access(1, 0.0)?.to_bits() {
        let range = access(1, 0.0)?;
        access(2, range)?;
    }
    Ok(())
}

pub fn plane_add(a: FPlane, b: FPlane) -> FPlane {
    FPlane {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
        w: a.w + b.w,
    }
}

pub fn plane_sub(a: FPlane, b: FPlane) -> FPlane {
    FPlane {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
        w: a.w - b.w,
    }
}

pub fn plane_mul(a: FPlane, b: FPlane) -> FPlane {
    FPlane {
        x: a.x * b.x,
        y: a.y * b.y,
        z: a.z * b.z,
        w: a.w * b.w,
    }
}

pub fn plane_div_components(a: FPlane, b: FPlane) -> FPlane {
    FPlane {
        x: a.x / b.x,
        y: a.y / b.y,
        z: a.z / b.z,
        w: a.w / b.w,
    }
}

pub fn plane_scale(a: FPlane, s: f32) -> FPlane {
    FPlane {
        x: a.x * s,
        y: a.y * s,
        z: a.z * s,
        w: a.w * s,
    }
}

pub fn plane_div(a: FPlane, s: f32) -> FPlane {
    let r = 1.0 / s;
    FPlane {
        x: a.x * r,
        y: a.y * r,
        z: a.z * r,
        w: a.w * s,
    }
}

pub fn plane_div_assign(a: FPlane, s: f32) -> FPlane {
    let r = 1.0 / s;
    FPlane {
        x: mul_rn(r, a.x),
        y: mul_rn(r, a.y),
        z: mul_rn(r, a.z),
        w: mul_rn(r, a.w),
    }
}

pub fn plane_flip(a: FPlane) -> FPlane {
    FPlane {
        x: -a.x,
        y: -a.y,
        z: -a.z,
        w: -a.w,
    }
}

pub fn plane_dot(a: FPlane, b: FVector) -> f32 {
    dot3(xyz(a), b) - a.w
}

pub fn plane_equal(a: FPlane, b: FPlane) -> i32 {
    (a.x == b.x && a.y == b.y && a.z == b.z && a.w == b.w) as i32
}

pub fn plane_not_equal(a: FPlane, b: FPlane) -> i32 {
    (a.x != b.x || a.y != b.y || a.z != b.z || a.w != b.w) as i32
}

pub fn plane_from_points(a: FVector, b: FVector, c: FVector) -> FPlane {
    let n = vector_safe_normal(cross_fnmul_fmadd(sub(b, a), sub(c, a)));
    FPlane {
        x: n.x,
        y: n.y,
        z: n.z,
        w: dot3(a, n),
    }
}

pub fn scale_orientation(a: FVector) -> f32 {
    let p = (a.x * a.y) * a.z;
    if p > 0.0 {
        1.0
    } else if p < 0.0 {
        -1.0
    } else {
        0.0
    }
}

pub fn scale_equal(a: FVector, b: FVector, ar: f32, br: f32, ax: u8, bx: u8) -> i32 {
    (vector_equal(a, b) != 0 && ar == br && ax == bx) as i32
}

pub fn vector_transform(a: FVector, c: FCoords) -> FVector {
    v3(dot3(a, c.x_axis), dot3(a, c.y_axis), dot3(a, c.z_axis))
}

pub fn vector_transform_point(a: FVector, c: FCoords) -> FVector {
    vector_transform(sub(a, c.origin), c)
}

pub fn vector_shift(a: FVector, c: FCoords) -> FVector {
    shift(a, &c)
}

pub fn vector_mirror_plane(a: FVector, b: FPlane) -> FVector {
    sub(a, scale(xyz(b), 2.0 * plane_dot(b, a)))
}

pub fn plane_transform(a: FPlane, c: FCoords) -> FPlane {
    let n = vector_transform(xyz(a), c);
    FPlane {
        x: n.x,
        y: n.y,
        z: n.z,
        w: a.w - dot3(vector_transform(c.origin, c), n),
    }
}

pub fn coords_transpose(a: FCoords) -> FCoords {
    FCoords {
        origin: vector_neg(vector_transform(a.origin, a)),
        x_axis: v3(a.x_axis.x, a.y_axis.x, a.z_axis.x),
        y_axis: v3(a.x_axis.y, a.y_axis.y, a.z_axis.y),
        z_axis: v3(a.x_axis.z, a.y_axis.z, a.z_axis.z),
    }
}

pub fn coords_mirror_vector(a: FCoords, b: FVector) -> FCoords {
    FCoords {
        origin: vector_mirror_by_vector(a.origin, b),
        x_axis: vector_mirror_by_vector(a.x_axis, b),
        y_axis: vector_mirror_by_vector(a.y_axis, b),
        z_axis: vector_mirror_by_vector(a.z_axis, b),
    }
}

pub fn coords_mirror_plane(a: FCoords, b: FPlane) -> FCoords {
    FCoords {
        origin: vector_mirror_plane(a.origin, b),
        x_axis: vector_mirror_by_vector(a.x_axis, xyz(b)),
        y_axis: vector_mirror_by_vector(a.y_axis, xyz(b)),
        z_axis: vector_mirror_by_vector(a.z_axis, xyz(b)),
    }
}

pub fn coords_mul(a: FCoords, b: FCoords) -> FCoords {
    FCoords {
        origin: vector_transform_point(a.origin, b),
        x_axis: vector_transform(a.x_axis, b),
        y_axis: vector_transform(a.y_axis, b),
        z_axis: vector_transform(a.z_axis, b),
    }
}

pub fn coords_mul_self(mut a: FCoords) -> FCoords {
    a.origin = vector_transform_point(a.origin, a);
    a.x_axis = vector_transform(a.x_axis, a);
    a.y_axis = vector_transform(a.y_axis, a);
    a.z_axis = vector_transform(a.z_axis, a);
    a
}

pub fn coords_mul_point(mut a: FCoords, b: FVector) -> FCoords {
    a.origin = sub(a.origin, b);
    a
}

pub fn coords_div_point(mut a: FCoords, b: FVector) -> FCoords {
    a.origin = add(a.origin, b);
    a
}

pub fn coords_mul_rotator(mut a: FCoords, r: FRotator, trig: &[f32; NUM_ANGLES]) -> FCoords {
    let (sy, cy) = (sin_tab(trig, r.yaw), cos_tab(trig, r.yaw));
    a = coords_mul(
        a,
        FCoords {
            origin: zero3(),
            x_axis: v3(cy, sy, 0.0),
            y_axis: v3(-sy, cy, 0.0),
            z_axis: v3(0.0, 0.0, 1.0),
        },
    );
    let (sp, cp) = (sin_tab(trig, r.pitch), cos_tab(trig, r.pitch));
    a = coords_mul(
        a,
        FCoords {
            origin: zero3(),
            x_axis: v3(cp, 0.0, sp),
            y_axis: v3(0.0, 1.0, 0.0),
            z_axis: v3(-sp, 0.0, cp),
        },
    );
    let (sr, cr) = (sin_tab(trig, r.roll), cos_tab(trig, r.roll));
    coords_mul(
        a,
        FCoords {
            origin: zero3(),
            x_axis: v3(1.0, 0.0, 0.0),
            y_axis: v3(0.0, cr, -sr),
            z_axis: v3(0.0, sr, cr),
        },
    )
}

pub fn sheer_coords(rate: f32, axis: u8) -> FCoords {
    let mut c = unit_coords();
    match axis {
        1 => c.x_axis.y = rate,
        2 => c.x_axis.z = rate,
        3 => c.y_axis.x = rate,
        4 => c.y_axis.z = rate,
        5 => c.z_axis.x = rate,
        6 => c.z_axis.y = rate,
        _ => {}
    }
    c
}

// Keep every native FMA, including zero coefficients in the shear matrix:
// folding them changes the result of infinity times zero with a NaN addend.
fn coords_apply_sheer(a: FCoords, c: FCoords, origin_y_first: bool) -> FCoords {
    let transform = |v: FVector, y_first: bool| {
        let dot = |row: FVector| {
            let y = if y_first {
                mul_rn(v.y, row.y)
            } else {
                mul_rn(row.y, v.y)
            };
            fmadd_rn(v.z, row.z, fmadd_rn(v.x, row.x, y))
        };
        v3(dot(c.x_axis), dot(c.y_axis), dot(c.z_axis))
    };
    FCoords {
        origin: transform(sub(a.origin, c.origin), origin_y_first),
        x_axis: transform(a.x_axis, false),
        y_axis: transform(a.y_axis, false),
        z_axis: transform(a.z_axis, false),
    }
}

pub fn coords_mul_scale(mut a: FCoords, s: FVector, rate: f32, axis: u8) -> FCoords {
    a = coords_apply_sheer(a, sheer_coords(sheer_snap(rate), axis), false);
    a.x_axis = vector_mul(a.x_axis, s);
    a.y_axis = vector_mul(a.y_axis, s);
    a.z_axis = vector_mul(a.z_axis, s);
    a.origin = vector_div_components(a.origin, s);
    a
}

pub fn coords_div_scale(mut a: FCoords, s: FVector, rate: f32, axis: u8) -> FCoords {
    a.x_axis = vector_div_components(a.x_axis, s);
    a.y_axis = vector_div_components(a.y_axis, s);
    a.z_axis = vector_div_components(a.z_axis, s);
    a.origin = vector_mul(a.origin, s);
    coords_apply_sheer(a, sheer_coords(-sheer_snap(rate), axis), true)
}

pub fn coords_shift_scale(a: FCoords, s: FVector) -> FCoords {
    FCoords {
        origin: vector_mul(a.origin, s),
        x_axis: vector_mul(a.x_axis, s),
        y_axis: vector_mul(a.y_axis, s),
        z_axis: vector_mul(a.z_axis, s),
    }
}

pub fn coords_shift_right(a: FCoords, b: FCoords) -> FCoords {
    coords_div_assign(a, &coords_inverse(&b))
}

pub fn rotator_add(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_add(b.pitch),
        yaw: a.yaw.wrapping_add(b.yaw),
        roll: a.roll.wrapping_add(b.roll),
    }
}

pub fn rotator_sub(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_sub(b.pitch),
        yaw: a.yaw.wrapping_sub(b.yaw),
        roll: a.roll.wrapping_sub(b.roll),
    }
}

pub fn rotator_grid(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: snap(a.pitch as f32, b.pitch as f32) as i32,
        yaw: snap(a.yaw as f32, b.yaw as f32) as i32,
        roll: snap(a.roll as f32, b.roll as f32) as i32,
    }
}

pub fn rotator_scale(a: FRotator, s: f32) -> FRotator {
    FRotator {
        pitch: (a.pitch as f32 * s) as i32,
        yaw: (a.yaw as f32 * s) as i32,
        roll: (a.roll as f32 * s) as i32,
    }
}

pub fn rotator_reduce(a: FRotator) -> FRotator {
    FRotator {
        pitch: reduce_angle(a.pitch),
        yaw: reduce_angle(a.yaw),
        roll: reduce_angle(a.roll),
    }
}

pub fn rotator_zero(a: FRotator) -> i32 {
    ((a.pitch & 65535) == 0 && (a.yaw & 65535) == 0 && (a.roll & 65535) == 0) as i32
}

pub fn rotator_equal(a: FRotator, b: FRotator) -> i32 {
    (a == b) as i32
}

pub fn rotator_not_equal(a: FRotator, b: FRotator) -> i32 {
    (a != b) as i32
}

pub fn rotator_add_delta(a: FRotator, pitch: i32, yaw: i32, roll: i32) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_add(pitch),
        yaw: a.yaw.wrapping_add(yaw),
        roll: a.roll.wrapping_add(roll),
    }
}

pub fn rotator_add_bounded(a: FRotator, pitch: i32, yaw: i32, roll: i32) -> FRotator {
    FRotator {
        pitch: angle_confined(a.pitch, pitch, 49152, 16384) as i32,
        yaw: a.yaw.wrapping_add(yaw),
        roll: angle_confined(a.roll, roll, 49152, 16384) as i32,
    }
}

pub fn quat_add(a: FQuat, b: FQuat) -> FQuat {
    FQuat {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
        w: a.w + b.w,
    }
}

pub fn quat_sub(a: FQuat, b: FQuat) -> FQuat {
    FQuat {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
        w: a.w - b.w,
    }
}

pub fn quat_shift(a: FQuat, b: FQuat) -> FQuat {
    FQuat {
        x: fmsub_rn(
            a.y,
            b.z,
            fmsub_rn(a.w, b.x, fmadd_rn(a.z, b.y, fnmul_rn(b.w, a.x))),
        ),
        y: fmsub_rn(
            a.z,
            b.x,
            fmsub_rn(a.w, b.y, fmadd_rn(a.x, b.z, fnmul_rn(b.w, a.y))),
        ),
        z: fmsub_rn(
            a.x,
            b.y,
            fmsub_rn(a.w, b.z, fmadd_rn(a.y, b.x, fnmul_rn(b.w, a.z))),
        ),
        w: fmsub_rn(
            a.w,
            b.w,
            fmadd_rn(a.z, b.z, fmadd_rn(a.x, b.x, mul_rn(a.y, b.y))),
        ),
    }
}

pub fn quat_scale(a: FQuat, s: f32) -> FQuat {
    FQuat {
        x: s * a.x,
        y: s * a.y,
        z: s * a.z,
        w: s * a.w,
    }
}

pub fn quat_neg(a: FQuat) -> FQuat {
    FQuat { w: -a.w, ..a }
}

pub fn quat_not_equal(a: FQuat, b: FQuat) -> i32 {
    (a != b) as i32
}

pub fn quat_dot(a: FQuat, b: FQuat) -> f32 {
    dot4(a, b)
}

pub fn quat_vector(a: FQuat) -> FVector {
    if a.w < 0.0 {
        v3(-a.x, -a.y, -a.z)
    } else {
        v3(a.x, a.y, a.z)
    }
}

pub fn quat_from_xyz(x: f32, y: f32, z: f32) -> FQuat {
    let t = (-z).mul_add(z, (-y).mul_add(y, (-x).mul_add(x, 1.0)));
    FQuat {
        x,
        y,
        z,
        w: (max_f(0.0, t) as f64).sqrt() as f32,
    }
}

pub fn quat_to_angle_axis(a: FQuat) -> FQuat {
    let s = (a.w as f64).sin() as f32;
    if s >= DELTA {
        FQuat {
            x: a.z / s,
            y: a.y / s,
            z: a.z / s,
            w: (2.0 * (a.w as f64).acos()) as f32,
        }
    } else {
        FQuat {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        }
    }
}

pub fn quat_from_angle_axis(a: FQuat) -> FQuat {
    let s = sum_sq(v3(a.x, a.y, a.z));
    if s >= DELTA {
        let inv = 1.0 / ((s as f64).sqrt() as f32);
        FQuat {
            x: a.x * inv,
            y: a.y * inv,
            z: a.z * inv,
            w: ((a.w * 0.5) as f64).cos() as f32,
        }
    } else {
        FQuat {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        }
    }
}

pub fn quat_error(a: FQuat, b: FQuat) -> f32 {
    (min_f(dot4(a, b), 1.0) as f64).acos() as f32
}

pub fn quat_align(a: FQuat, b: FQuat) -> FQuat {
    if dot4(a, b) < 0.0 {
        FQuat {
            x: -a.x,
            y: -a.y,
            z: -a.z,
            w: -a.w,
        }
    } else {
        a
    }
}

pub fn quat_normalize_inline(a: FQuat, checks: bool) -> (i32, FQuat) {
    let sq = dot4(a, a);
    if abs(sq - 1.0) > DELTA {
        if sq >= DELTA {
            let s = (-sq).mul_add(0.5, 1.5);
            if checks && !(abs(s * s / sq - 1.0) > DELTA) {
                return (2, a);
            }
            return (
                1,
                FQuat {
                    x: a.x * s,
                    y: a.y * s,
                    z: a.z * s,
                    w: a.w * s,
                },
            );
        }
        return (
            0,
            FQuat {
                x: 0.0,
                y: 0.0,
                z: 0.1,
                w: 0.0,
            },
        );
    }
    (1, a)
}

pub fn points_same(a: FVector, b: FVector) -> i32 {
    let d = sub(a, b);
    (d.x > -0.002 && d.x < 0.002 && d.y > -0.002 && d.y < 0.002 && d.z > -0.002 && d.z < 0.002)
        as i32
}

pub fn parallel(a: FVector, b: FVector) -> i32 {
    (abs(dot3(a, b) - 1.0) <= 0.02) as i32
}

pub fn points_near(a: FVector, b: FVector, d: f32) -> i32 {
    let t = sub(a, b);
    (!(abs(t.x) >= d || abs(t.y) >= d || abs(t.z) >= d)) as i32
}

pub fn point_plane_distance(a: FVector, b: FVector, c: FVector) -> f32 {
    dot3(sub(a, b), c)
}

pub fn coplanar(a: FVector, b: FVector, c: FVector, d: FVector) -> i32 {
    if parallel(b, d) == 0 {
        0
    } else {
        (!(point_plane_distance(c, a, b) > 0.1)) as i32
    }
}

pub fn triple(a: FVector, b: FVector, c: FVector) -> f32 {
    dot3(a, cross_fnmul_fmadd(b, c))
}

pub fn box_push_out(a: FVector, b: FVector) -> f32 {
    (abs(a.x * b.x) + abs(a.y * b.y)) + abs(a.z * b.z)
}

pub fn line_plane_points(a: FVector, b: FVector, c: FVector, d: FVector) -> FVector {
    add(a, scale(sub(b, a), dot3(sub(c, a), d) / dot3(sub(b, a), d)))
}

pub fn line_plane(a: FVector, b: FVector, c: FPlane) -> FVector {
    add(
        a,
        scale(sub(b, a), (c.w - dot3(a, xyz(c))) / dot3(sub(b, a), xyz(c))),
    )
}

pub fn box_add_point_inline(mut a: FBox, b: FVector) -> FBox {
    if a.is_valid != 0 {
        a.min = vector_min(a.min, b);
        a.max = vector_max(a.max, b);
    } else {
        a.min = b;
        a.max = b;
        a.is_valid = 1;
    }
    a
}

pub fn box_add_box_inline(mut a: FBox, b: FBox) -> FBox {
    if b.is_valid != 0 {
        if a.is_valid != 0 {
            a.min = vector_min(a.min, b.min);
            a.max = vector_max(a.max, b.max);
        } else {
            a = b;
        }
    }
    a
}

pub fn box_scale_inline(a: FBox, s: f32) -> FBox {
    FBox {
        min: vector_scale(a.min, s),
        max: vector_scale(a.max, s),
        is_valid: 1,
    }
}

pub fn box_expand_inline(a: FBox, s: f32) -> FBox {
    FBox {
        min: sub(a.min, v3(s, s, s)),
        max: v3(add_rn(a.max.x, s), add_rn(a.max.y, s), add_rn(a.max.z, s)),
        is_valid: 1,
    }
}

pub fn box_transform_inline(a: FBox, c: FCoords) -> FBox {
    let mut result = FBox {
        min: zero3(),
        max: zero3(),
        is_valid: 0,
    };
    for x in [a.min.x, a.max.x] {
        for y in [a.min.y, a.max.y] {
            for z in [a.min.z, a.max.z] {
                result = box_add_point_inline(result, vector_transform_point(v3(x, y, z), c));
            }
        }
    }
    result
}

pub fn sqrt_approx_inline(a: f32, table: &[f32; 2048]) -> f32 {
    let bits = a.to_bits();
    let index = ((bits >> 13) & 2047) as usize;
    f32::from_bits(
        table[index]
            .to_bits()
            .wrapping_add((bits & 0x7f000000) >> 1),
    )
}

pub fn power_inline(a: f32, man: &[f32; 1024], exp: &[f32; 512]) -> f32 {
    let bits = a.to_bits();
    man[((bits >> 13) & 1023) as usize] * exp[(bits >> 23) as usize]
}

pub fn vector_div_size_approx(a: FVector, man: &[f32; 1024], exp: &[f32; 512]) -> f32 {
    power_inline(sum_sq(a) + 1e-12, man, exp)
}

pub fn sin_tab_inline(i: i32, trig: &[f32; NUM_ANGLES]) -> f32 {
    sin_tab(trig, i)
}

pub fn cos_tab_inline(i: i32, trig: &[f32; NUM_ANGLES]) -> f32 {
    cos_tab(trig, i)
}

pub fn sin_float_inline(a: f32, trig: &[f32; NUM_ANGLES]) -> f32 {
    sin_tab(trig, ((a * 65536.0) / (2.0 * PI)) as i32)
}

pub fn cos_float_inline(a: f32, trig: &[f32; NUM_ANGLES]) -> f32 {
    cos_tab(trig, ((a * 65536.0) / (2.0 * PI)) as i32)
}

pub fn random_vector(mut random: impl FnMut() -> f32) -> FVector {
    loop {
        let a = v3(
            random().mul_add(2.0, -1.0),
            random().mul_add(2.0, -1.0),
            random().mul_add(2.0, -1.0),
        );
        if !(sum_sq(a) > 1.0) {
            return vector_unsafe_normal(a);
        }
    }
}

pub fn random_spread(degrees: f32, mut random: impl FnMut() -> f32) -> FVector {
    let max_pitch = clamp_inline(degrees * (PI / 180.0 / 2.0), 0.0, 180.0);
    let k = (1.0 - (max_pitch as f64).cos()) as f32;
    let pitch = ((-random()).mul_add(k, 1.0) as f64).acos() as f32;
    let roll = random() * (2.0 * PI);
    let radius = (pitch as f64).sin() as f32;
    v3(
        (pitch as f64).cos() as f32,
        ((radius as f64) * (roll as f64).sin()) as f32,
        ((radius as f64) * (roll as f64).cos()) as f32,
    )
}

pub fn place_shift(a: FPlace, b: FPlace) -> FPlace {
    FPlace {
        quat: quat_shift(a.quat, b.quat),
        pos: shift(a.pos, &coords_from_place(&b)),
    }
}

pub fn vector_shift_place(a: FVector, b: FPlace) -> FVector {
    shift(a, &coords_from_place(&b))
}

/// `FVector::Normalize`. `Scale` is computed in double, then cast, unlike
/// `SafeNormal`. Compute the squared length once for both status and result.
pub fn vector_normalize_inline(a: FVector) -> (i32, FVector) {
    let sq = sum_sq(a);
    if sq >= SMALL_NUMBER {
        let s = (1.0f64 / (sq as f64).sqrt()) as f32;
        (1, scale(a, s))
    } else {
        (0, a)
    }
}

/// Plane intersection with the original reference-alias store order.
/// # Safety
/// All pointers must address initialized, writable/readable corresponding native layouts.
pub unsafe fn intersect_planes2_inline(
    i: *mut FVector,
    d: *mut FVector,
    p1: *const FPlane,
    p2: *const FPlane,
) -> i32 {
    unsafe {
        d.write(cross_b_first(xyz(p1.read()), xyz(p2.read())));
        let dd = sum_sq(d.read());
        if dd < 0.001f32 * 0.001f32 {
            i.write(zero3());
            d.write(zero3());
            return 0;
        }
        let a = p1.read();
        let b = p2.read();
        let direction = d.read();
        let t1 = scale(cross_b_first(xyz(b), direction), a.w);
        let t2 = scale(cross_b_first(direction, xyz(a)), b.w);
        i.write(div_by(add(t1, t2), dd));
        d.write(vector_normalize(d.read()));
        1
    }
}

#[cfg(test)]
mod inline_contract_tests {
    use super::*;

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn quaternion_shift_preserves_fused_subtraction_nan_signs() {
        let a = FQuat {
            x: f32::from_bits(0x322f_de01),
            y: f32::from_bits(0xf41c_c436),
            z: f32::from_bits(0xffdc_7d1b),
            w: f32::from_bits(0xc0e8_1129),
        };
        let b = FQuat {
            x: f32::NEG_INFINITY,
            y: f32::from_bits(0xfa4f_c813),
            z: f32::from_bits(0x6af8_9f15),
            w: f32::from_bits(0x9b67_59af),
        };
        let result = quat_shift(a, b);
        assert_eq!(
            [
                result.x.to_bits(),
                result.y.to_bits(),
                result.z.to_bits(),
                result.w.to_bits()
            ],
            [0xffdc_7d1b, 0x7fdc_7d1b, 0x7fdc_7d1b, 0x7fc0_0000]
        );
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn reciprocal_scaling_keeps_the_native_nan_operand_priority() {
        let a = v3(
            f32::from_bits(0xffc2_3456),
            1.0,
            f32::from_bits(0x7fc1_2345),
        );
        let s = f32::from_bits(0x7f9b_053a);
        let result = vector_div(a, s);
        assert_eq!(result.x.to_bits(), 0x7fdb_053a);
        assert_eq!(result.z.to_bits(), 0x7fdb_053a);
        let result = vector_scale(a, f32::from_bits(0xffc2_3456));
        assert_eq!(result.z.to_bits(), 0x7fc1_2345);
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn cube_bounds_quiet_signaling_nan_but_replace_quiet_nan() {
        let a = v3(
            f32::from_bits(0xffac_832e),
            f32::from_bits(0x7fc1_2345),
            f32::NEG_INFINITY,
        );
        let result = vector_bound(a, 256.0);
        assert_eq!(result.x.to_bits(), 0xffec_832e);
        assert_eq!(result.y, 256.0);
        assert_eq!(result.z, -256.0);
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn shear_keeps_invalid_products_with_nan_addends() {
        let a = FCoords {
            origin: v3(f32::NEG_INFINITY, -1.0, f32::from_bits(0x7fc1_2345)),
            ..unit_coords()
        };
        let result = coords_mul_scale(a, v3(0.5, 2.0, -1.0), 0.4, 3);
        assert_eq!(result.origin.x.to_bits(), 0x7fc1_2345);
        assert_eq!(result.origin.z.to_bits(), 0x7fc0_0000);
    }

    #[test]
    fn plane_division_preserves_legacy_w_asymmetry() {
        let p = FPlane {
            x: 2.0,
            y: 4.0,
            z: 8.0,
            w: 16.0,
        };
        assert_eq!(plane_div(p, 2.0).w, 32.0);
        assert_eq!(plane_div_assign(p, 2.0).w, 8.0);
    }

    #[test]
    fn tiny_normalization_preserves_vector_but_resets_quaternion() {
        let v = v3(-0.0, 0.0, 0.0);
        let (changed, result) = vector_normalize_inline(v);
        assert_eq!(changed, 0);
        assert_eq!(result.x.to_bits(), (-0.0f32).to_bits());
        let (changed, result) = quat_normalize_inline(FQuat::default(), false);
        assert_eq!(changed, 0);
        assert_eq!(
            result,
            FQuat {
                x: 0.0,
                y: 0.0,
                z: 0.1,
                w: 0.0
            }
        );
    }

    #[test]
    fn two_plane_shared_output_normalizes_the_intersection() {
        let a = FPlane {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 2.0,
        };
        let b = FPlane {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.0,
        };
        let mut output = zero3();
        let ptr = &raw mut output;
        // SAFETY: writable output and readable plane layouts; alias is intentional.
        assert_eq!(unsafe { intersect_planes2_inline(ptr, ptr, &a, &b) }, 1);
        assert_eq!(output, v3(1.0, 0.0, 0.0));
    }

    #[test]
    fn coordinate_self_multiply_observes_earlier_axis_stores() {
        let c = FCoords {
            origin: v3(1.0, 2.0, 3.0),
            x_axis: v3(2.0, 1.0, 0.0),
            y_axis: v3(1.0, 3.0, 0.0),
            z_axis: v3(0.0, 0.0, 1.0),
        };
        let result = coords_mul_self(c);
        assert_eq!(result.x_axis, v3(5.0, 5.0, 0.0));
        assert_eq!(result.y_axis, v3(20.0, 10.0, 0.0));
        assert_eq!(result.origin, zero3());
    }

    #[test]
    fn clipping_skips_unused_range_and_stops_after_access_failure() {
        let mut value = -1.0;
        assert_eq!(
            clip_zero(|action, input| {
                match action {
                    0 => Ok(value),
                    1 => panic!("negative values must not evaluate the range"),
                    _ => {
                        value = input;
                        Ok(0.0)
                    }
                }
            }),
            Ok(())
        );
        assert_eq!(value.to_bits(), 0.0f32.to_bits());

        let mut reads = 0;
        assert_eq!(
            clip_zero(|action, _| {
                assert_eq!(action, 0);
                reads += 1;
                if reads == 1 { Ok(1.0) } else { Err(()) }
            }),
            Err(())
        );
        assert_eq!(reads, 2);
    }
}
