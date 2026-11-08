//! Hand-written Fontconfig FFI bindings: only the `Fc*` entry points and
//! object-name strings the NativeText FreeType provider leaf's `Resolve()`
//! used before this port. Fontconfig is not vendored/pinned (no
//! `ThirdParty/sources.json` entry; `HP2Dependencies.cmake` finds it via
//! `pkg_check_modules`), but every symbol below is part of Fontconfig's
//! long-stable public API (unchanged since the 2.x series), and all access
//! goes through Fontconfig's own accessor functions rather than raw struct
//! field reads, so (unlike `freetype_sys`/`harfbuzz_sys`) no struct-layout
//! `static_assert` cross-check is needed: `FcPattern`/`FcCharSet`/`FcConfig`
//! stay fully opaque to Rust.
#![cfg(feature = "freetype")]

use core::ffi::{c_char, c_double, c_int, c_uchar, c_uint, c_void};

pub type FcBool = c_int;
pub type FcChar8 = c_uchar;
pub type FcChar32 = c_uint;
pub type FcPattern = c_void;
pub type FcCharSet = c_void;
pub type FcConfig = c_void;
/// `FcResult` (`fontconfig.h`): only `FcResultMatch` (0) is compared against.
pub type FcResult = c_int;
/// `FcMatchKind` (`fontconfig.h`): only `FcMatchPattern` (0) is used.
pub type FcMatchKind = c_int;

pub const FC_RESULT_MATCH: FcResult = 0;
pub const FC_MATCH_PATTERN: FcMatchKind = 0;
pub const FC_WEIGHT_REGULAR: c_int = 80;
pub const FC_WEIGHT_BOLD: c_int = 200;

// Fontconfig "object" names: plain NUL-terminated string constants per
// `fontconfig.h`'s `#define FC_FAMILY "family"` style macros, not opaque
// linker symbols.
pub const FC_FAMILY: &[u8] = b"family\0";
pub const FC_WEIGHT: &[u8] = b"weight\0";
pub const FC_SIZE: &[u8] = b"size\0";
pub const FC_CHARSET: &[u8] = b"charset\0";
pub const FC_FILE: &[u8] = b"file\0";
pub const FC_INDEX: &[u8] = b"index\0";

// No unconditional `#[link(name = "...")]`: like `freetype_sys`, these
// symbols are resolved when CMake links the final `hp2_game` executable
// against the system Fontconfig library found by `HP2Dependencies.cmake`'s
// `pkg_check_modules(HP2_FONTCONFIG ...)`, not by a Cargo-level link
// directive.
unsafe extern "C" {
    pub fn FcInit() -> FcBool;
    pub fn FcPatternCreate() -> *mut FcPattern;
    pub fn FcPatternDestroy(p: *mut FcPattern);
    pub fn FcCharSetCreate() -> *mut FcCharSet;
    pub fn FcCharSetDestroy(fcs: *mut FcCharSet);
    pub fn FcCharSetAddChar(fcs: *mut FcCharSet, ucs4: FcChar32) -> FcBool;
    pub fn FcCharSetHasChar(fcs: *const FcCharSet, ucs4: FcChar32) -> FcBool;
    pub fn FcPatternAddCharSet(
        p: *mut FcPattern,
        object: *const c_char,
        c: *const FcCharSet,
    ) -> FcBool;
    pub fn FcPatternAddString(
        p: *mut FcPattern,
        object: *const c_char,
        s: *const FcChar8,
    ) -> FcBool;
    pub fn FcPatternAddInteger(p: *mut FcPattern, object: *const c_char, i: c_int) -> FcBool;
    pub fn FcPatternAddDouble(p: *mut FcPattern, object: *const c_char, d: c_double) -> FcBool;
    pub fn FcConfigSubstitute(
        config: *mut FcConfig,
        p: *mut FcPattern,
        kind: FcMatchKind,
    ) -> FcBool;
    pub fn FcDefaultSubstitute(p: *mut FcPattern);
    pub fn FcFontMatch(
        config: *mut FcConfig,
        p: *mut FcPattern,
        result: *mut FcResult,
    ) -> *mut FcPattern;
    pub fn FcPatternGetCharSet(
        p: *const FcPattern,
        object: *const c_char,
        n: c_int,
        c: *mut *mut FcCharSet,
    ) -> FcResult;
    pub fn FcPatternGetString(
        p: *const FcPattern,
        object: *const c_char,
        n: c_int,
        s: *mut *mut FcChar8,
    ) -> FcResult;
    pub fn FcPatternGetInteger(
        p: *const FcPattern,
        object: *const c_char,
        n: c_int,
        i: *mut c_int,
    ) -> FcResult;
}
