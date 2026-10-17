//! Shared render ABI: `#[repr(C)]` mirrors of the plain-data math types
//! `URenderDevice`'s interface (`Engine/Inc/UnRenDev.h`) and the scene/
//! texture types it passes (`Engine/Inc/UnRender.h`, `Engine/Inc/UnTex.h`)
//! are built from. Phase 3.1 of `local://rust-inplace-port-plan.md`.
//!
//! **Scope of this crate today: the four portable math types only**
//! (`FVector`, `FPlane`, `FColor`, `FCoords`) plus the `EPolyFlags` bit
//! constants (`Engine/Inc/UnObj.h:221-281`, not `UnRenDev.h`/`UnTex.h` as
//! the plan's own text guessed -- confirmed by grep, corrected here).
//! These four are genuinely portable on their own: pure value types, used
//! everywhere in the engine (not renderer-specific), with no pointer or
//! `UObject` reference in their fields. Every size/offset below was
//! confirmed empirically against the real pinned build (`clang++
//! -std=c++17` with this project's own `compile_commands.json` flags for
//! `ALAudioSubsystem.cpp`, substituting a throwaway probe source that
//! `#include`s `Engine.h` and prints `sizeof`/`offsetof` for each type),
//! not assumed from reading the header alone -- `FVector`'s `#if
//! __PSX2_EE__` branch (16 bytes, `X,Y,Z,W`) is dead on every platform
//! this project targets; the active `#else` branch (12 bytes, `X,Y,Z`
//! only) is what every probe number below reflects.
//!
//! Deliberately NOT in this crate yet: `FSceneNode`, `FSurfaceInfo`,
//! `FSurfaceFacet`, `FTextureInfo`, `FTransTexture`, `FTransSample`.
//! Every one of them holds a raw pointer to a live engine object
//! (`ULevel*`, `FTextureInfo*`, `FSpanBuffer*`, `UViewport*`, ...) or
//! embeds a type that does. Mirroring their layout today, before Phase
//! 3.2's actual `WgpuDrv`/`hp2-wgpu` device exists to consume them, would
//! mean guessing at a Rust-side representation for those pointers (opaque
//! handle? Some future live-object FFI pattern?) with no real consumer to
//! prove the design against -- the device design in 3.2 determines that
//! choice, not the other way around. Porting them is still Phase 3.1
//! scope; this is the first commit of it, not all of it.
//!
//! No existing C++ oracle pinned any of these four types' layout before
//! this commit (`Tests/AbiTests.cpp` pins `FColor`'s *reflected UProperty*
//! layout on `UCanvas::DrawColor`, not `FColor`'s own `sizeof`/`offsetof`
//! -- confirmed by grep, no match for `FVector`/`FPlane`/`FCoords` at
//! all). New `Tests/AbiRenderTests.cpp` (`abi_render_widths` ctest) pins
//! the same facts this crate's own `const _: () = assert!(...)` blocks
//! already check, so a future accidental change to either side is caught
//! on both.

/// Matches `Core/Inc/UnMath.h`'s active (non-`__PSX2_EE__`) `FVector`:
/// `FLOAT X,Y,Z;`. 12 bytes, 4-byte aligned, no padding -- confirmed via
/// a real compiled probe (see module doc), not assumed.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

const _: () = assert!(size_of::<FVector>() == 12);
const _: () = assert!(align_of::<FVector>() == 4);
const _: () = assert!(std::mem::offset_of!(FVector, x) == 0);
const _: () = assert!(std::mem::offset_of!(FVector, y) == 4);
const _: () = assert!(std::mem::offset_of!(FVector, z) == 8);

/// Matches `Core/Inc/UnMath.h`'s `FPlane : public FVector { FLOAT W; }`.
/// 16 bytes: `X,Y,Z` inherited at their `FVector` offsets, `W` at 12 --
/// confirmed via the same probe, not assumed from the inheritance alone
/// (single, non-virtual base with no vtable, so no hidden layout surprise
/// here, but checked anyway).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FPlane {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

const _: () = assert!(size_of::<FPlane>() == 16);
const _: () = assert!(align_of::<FPlane>() == 4);
const _: () = assert!(std::mem::offset_of!(FPlane, x) == 0);
const _: () = assert!(std::mem::offset_of!(FPlane, y) == 4);
const _: () = assert!(std::mem::offset_of!(FPlane, z) == 8);
const _: () = assert!(std::mem::offset_of!(FPlane, w) == 12);

/// Matches `Engine/Inc/UnTex.h`'s `FColor`: a `union { DWORD Int4; struct
/// { BYTE R,G,B,A; }; }` under `#if __INTEL_BYTE_ORDER__`. That symbol is
/// not a compiler builtin or a real endianness probe -- grepped every
/// definition site: `Core/Inc/UnGnuG.h`'s macOS arm64 branch
/// (`:43-62`, `#define __INTEL_BYTE_ORDER__ 1` at `:49`) and its Linux
/// arm64 branch (`:63-82`, same define at `:69`) both set it
/// unconditionally, and each branch also has its own `#error` guard
/// (`:51-53` macOS, `:71-73` Linux) that refuses to compile at all
/// unless `__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__` -- so the
/// `R,G,B,A` branch is active on *both* of this project's supported
/// presets by construction, not just "confirmed on this one machine
/// that happened to run the probe": a Linux arm64 build, never run
/// this session, would fail to compile at all before it could ever
/// reach the big-endian `#else` branch (`A,B,G,R`), so that branch is
/// unreachable dead code project-wide, on every platform this project
/// builds for, not probed-and-assumed. `Core/Inc/UnVcWin32.h` (the
/// MSVC path, `:12`) sets the same define unconditionally too.
/// Cross-checked against the real build regardless: the macOS probe
/// (see module doc) reports `R` at offset 0, `A` at offset 3, matching.
/// `align(4)`: the real C++'s `union { DWORD Int4; struct {...}; }` forces
/// 4-byte alignment on the whole type even though four bare `u8` fields
/// alone would only need 1 -- confirmed by the probe (`align=4`, not the
/// `align_of::<FColor>() == 1` a plain `#[repr(C)]` struct would have
/// produced; caught by this crate's own layout assert failing before
/// this attribute was added).
#[repr(C, align(4))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

const _: () = assert!(size_of::<FColor>() == 4);
const _: () = assert!(align_of::<FColor>() == 4);
const _: () = assert!(std::mem::offset_of!(FColor, r) == 0);
const _: () = assert!(std::mem::offset_of!(FColor, g) == 1);
const _: () = assert!(std::mem::offset_of!(FColor, b) == 2);
const _: () = assert!(std::mem::offset_of!(FColor, a) == 3);

/// Matches `Core/Inc/UnMath.h`'s `FCoords`: four consecutive `FVector`s
/// (`Origin, XAxis, YAxis, ZAxis`), no padding between them. 48 bytes --
/// confirmed via the probe.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FCoords {
    pub origin: FVector,
    pub x_axis: FVector,
    pub y_axis: FVector,
    pub z_axis: FVector,
}

const _: () = assert!(size_of::<FCoords>() == 48);
const _: () = assert!(align_of::<FCoords>() == 4);
const _: () = assert!(std::mem::offset_of!(FCoords, origin) == 0);
const _: () = assert!(std::mem::offset_of!(FCoords, x_axis) == 12);
const _: () = assert!(std::mem::offset_of!(FCoords, y_axis) == 24);
const _: () = assert!(std::mem::offset_of!(FCoords, z_axis) == 36);

/// `EPolyFlags` (`Engine/Inc/UnObj.h:221-281`) bits this crate's first
/// consumers need. Transcribed verbatim, including the deliberate bit-
/// value aliases the real C++ reuses (`MASKED`/`ENVIRONMENT`/...): not
/// every flag is listed yet, only the ones read by code ported so far
/// (currently none -- these constants are added ahead of their first use,
/// same as the rest of this crate, since the full 32-entry enum is purely
/// mechanical to transcribe once and avoids a second partial pass later).
pub mod poly_flags {
    pub const INVISIBLE: u32 = 0x0000_0001;
    pub const MASKED: u32 = 0x0000_0002;
    pub const TRANSLUCENT: u32 = 0x0000_0004;
    pub const NOT_SOLID: u32 = 0x0000_0008;
    pub const ENVIRONMENT: u32 = 0x0000_0010;
    pub const FORCE_VIEW_ZONE: u32 = 0x0000_0010;
    pub const SEMISOLID: u32 = 0x0000_0020;
    pub const MODULATED: u32 = 0x0000_0040;
    pub const FAKE_BACKDROP: u32 = 0x0000_0080;
    pub const TWO_SIDED: u32 = 0x0000_0100;
    pub const AUTO_U_PAN: u32 = 0x0000_0200;
    pub const AUTO_V_PAN: u32 = 0x0000_0400;
    pub const NO_SMOOTH: u32 = 0x0000_0800;
    pub const BIG_WAVY: u32 = 0x0000_1000;
    pub const SPECIAL_POLY: u32 = 0x0000_1000;
    pub const SMALL_WAVY: u32 = 0x0000_2000;
    pub const FLAT: u32 = 0x0000_4000;
    pub const LOW_SHADOW_DETAIL: u32 = 0x0000_8000;
    pub const NO_MERGE: u32 = 0x0001_0000;
    pub const CLOUD_WAVY: u32 = 0x0002_0000;
    pub const DIRTY_SHADOWS: u32 = 0x0004_0000;
    pub const BRIGHT_CORNERS: u32 = 0x0008_0000;
    pub const SPECIAL_LIT: u32 = 0x0010_0000;
    pub const GOURAUD: u32 = 0x0020_0000;
    pub const NO_BOUND_REJECTION: u32 = 0x0020_0000;
    pub const UNLIT: u32 = 0x0040_0000;
    pub const HIGH_SHADOW_DETAIL: u32 = 0x0080_0000;
    pub const MEMORIZED: u32 = 0x0100_0000;
    pub const RENDER_HINT: u32 = 0x0100_0000;
    pub const SELECTED: u32 = 0x0200_0000;
    pub const PORTAL: u32 = 0x0400_0000;
    pub const MIRRORED: u32 = 0x0800_0000;
    pub const HIGHLIGHTED: u32 = 0x1000_0000;
    pub const FLAT_SHADED: u32 = 0x4000_0000;
    pub const ED_PROCESSED: u32 = 0x4000_0000;
    pub const RENDER_FOG: u32 = 0x4000_0000;
    pub const ED_CUT: u32 = 0x8000_0000;
    pub const OCCLUDE: u32 = 0x8000_0000;

    pub const NO_OCCLUDE: u32 = MASKED | TRANSLUCENT | INVISIBLE | MODULATED;
    pub const NO_EDIT: u32 = MEMORIZED | SELECTED | ED_PROCESSED | NO_MERGE | ED_CUT;
    pub const NO_IMPORT: u32 = NO_EDIT | NO_MERGE | MEMORIZED | SELECTED | ED_PROCESSED | ED_CUT;
    pub const ADD_LAST: u32 = SEMISOLID | NOT_SOLID;
    pub const NO_ADD_TO_BSP: u32 = ED_CUT | ED_PROCESSED | SELECTED | MEMORIZED;
    pub const NO_SHADOWS: u32 = UNLIT | INVISIBLE | ENVIRONMENT | FAKE_BACKDROP;
    pub const TRANSIENT: u32 = HIGHLIGHTED;
    pub const LUMOS_AFFECTED: u32 = HIGH_SHADOW_DETAIL | MODULATED;
    pub const LUMOS_DISAPPEAR: u32 = LUMOS_AFFECTED;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every layout fact here is already enforced at compile time by this
    // module's own `const _: () = assert!(...)` blocks; these tests exist
    // to exercise the types as values (construction, field access,
    // equality), not to re-check layout a second time at runtime.

    #[test]
    fn fvector_fields_round_trip() {
        let v = FVector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        assert_eq!((v.x, v.y, v.z), (1.0, 2.0, 3.0));
    }

    #[test]
    fn fplane_fields_round_trip() {
        let p = FPlane {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        assert_eq!((p.x, p.y, p.z, p.w), (1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn fcolor_equality_matches_the_real_cpp_int4_comparison() {
        // Real C++: `operator==` compares the whole `Int4` DWORD, so two
        // FColors are equal iff all four bytes match -- the same as
        // Rust's derived `PartialEq` on r/g/b/a, confirmed explicitly.
        assert_eq!(
            FColor {
                r: 1,
                g: 2,
                b: 3,
                a: 4
            },
            FColor {
                r: 1,
                g: 2,
                b: 3,
                a: 4
            }
        );
        assert_ne!(
            FColor {
                r: 1,
                g: 2,
                b: 3,
                a: 4
            },
            FColor {
                r: 1,
                g: 2,
                b: 3,
                a: 5
            }
        );
    }

    #[test]
    fn fcoords_axes_are_independent_fvectors() {
        let c = FCoords {
            origin: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            x_axis: FVector {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            y_axis: FVector {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            z_axis: FVector {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        };
        assert_eq!(c.x_axis.x, 1.0);
        assert_eq!(c.y_axis.y, 1.0);
        assert_eq!(c.z_axis.z, 1.0);
    }

    #[test]
    fn poly_flag_aliases_share_the_real_cpp_bit_values() {
        // The real C++ enum deliberately reuses bit values across
        // differently-named flags (see UnObj.h's own "(reuse ... flag)"
        // comments); this is not a transcription bug, so pin it.
        assert_eq!(poly_flags::ENVIRONMENT, poly_flags::FORCE_VIEW_ZONE);
        assert_eq!(poly_flags::BIG_WAVY, poly_flags::SPECIAL_POLY);
        assert_eq!(poly_flags::GOURAUD, poly_flags::NO_BOUND_REJECTION);
        assert_eq!(poly_flags::FLAT_SHADED, poly_flags::ED_PROCESSED);
        assert_eq!(poly_flags::ED_PROCESSED, poly_flags::RENDER_FOG);
        assert_eq!(poly_flags::ED_CUT, poly_flags::OCCLUDE);
        assert_eq!(poly_flags::MEMORIZED, poly_flags::RENDER_HINT);
    }

    #[test]
    fn poly_flag_combinations_match_the_real_cpp_formulas() {
        assert_eq!(
            poly_flags::NO_OCCLUDE,
            poly_flags::MASKED
                | poly_flags::TRANSLUCENT
                | poly_flags::INVISIBLE
                | poly_flags::MODULATED
        );
        assert_eq!(
            poly_flags::ADD_LAST,
            poly_flags::SEMISOLID | poly_flags::NOT_SOLID
        );
        assert_eq!(poly_flags::TRANSIENT, poly_flags::HIGHLIGHTED);
        assert_eq!(poly_flags::LUMOS_DISAPPEAR, poly_flags::LUMOS_AFFECTED);
    }
}
