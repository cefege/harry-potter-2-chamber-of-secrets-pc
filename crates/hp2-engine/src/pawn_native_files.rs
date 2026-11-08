//! Port of the file, URL, skin, map and console natives of `Engine/Src/UnPawn.cpp`:
//! `AActor::execGetNextInt`, `execGetNextIntDesc`, `execGetCacheEntry`,
//! `execMoveCacheEntry`, `APawn::execCheckValidSkinPackage`,
//! `APlayerPawn::execUpdateURL`, `execGetDefaultURL`, `AActor::execGetNextSkin`,
//! `APlayerPawn::execResetKeyboard`, `APawn::execFindBestInventoryPath`,
//! `AActor::execGetMapName`, `APlayerPawn::execConsoleCommand`,
//! `APlayerPawn::execScreenToWorld`.
//!
//! The C++ shells keep `guard`/`unguard`, `P_GET_*`, `P_FINISH`, `*(T*)Result`
//! and clock/unclock. Everything else (selection, string parsing and building,
//! case-folded comparisons, path scans and math) lives here.
//!
//! Engine calls go through [`Hp2PawnNativeFilesServices`], filled in by
//! `Hp2PawnNativeFilesServices.cpp`. Those services own the engine state the C++
//! reads and writes: the `Cache` section of `FConfigCacheIni`, `GSys->Paths`,
//! `GFileManager`, `FURL`, the registry snapshots, the pawn fields and the
//! viewport.
//!
//! Text crosses the boundary as NUL-terminated UNICHAR (`u16`) buffers, the
//! same convention `canvas_text.rs` uses. A string result is a pointer into a
//! thread-local buffer owned here, valid until the next string-returning call on
//! the same thread. The C++ shell copies it into an `FString` immediately.
//!
//! FP contraction: the only float math here is in `screen_to_world`. Clang
//! (`-O3`, `-ffp-contract=on`) fused the `2.f*S.Y - SizeY` and `2.f*S.X - SizeX`
//! terms into `fnmsub` (`Rn*Rm - Ra`), so those use `mul_add` here. The
//! `TransformVectorBy` dot products use [`crate::prim::transform_vector`]: its
//! `fma(z, bz, fma(x, bx, y*by))` shape matches the fused `fmadd` chain in the
//! oracle objdump. The `Z * (...)` multiplies, the `S.Z / ProjZ` divide, and the
//! `+ ViewLocation` add are unfused. The `tan` argument is double-precision as in
//! `appTan(DOUBLE)`.
//!
//! Case folding: the oracle's `appStricmp`/`appStrnicmp` are `wcscasecmp` and
//! `wcsncasecmp` in the C locale. `appToUpper` is ASCII-only here, and
//! [`fold`] matches it.

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, APlayerPawn, FRotator};
use hp2_core::math;

use crate::coords::{self, Trig};
use crate::pawn::{PawnResult, checked};
use crate::prim;

// Native values are never observed on the failure path.
macro_rules! native_value {
    ($service:expr $(, $arg:expr)* $(,)?) => {{
        let mut value = std::mem::MaybeUninit::uninit();
        checked(($service)($($arg,)* value.as_mut_ptr()))?;
        value.assume_init()
    }};
}

/// `PI` as `UnMath.h` defines it (`3.1415926535897932f`, bit pattern `0x40490fdb`).
const PI_F32: f32 = std::f32::consts::PI;

/// `SkinPacks[0..7]` of `AActor::execGetNextSkin`'s neighbour (`BOSSSKINS` ...).
/// The oracle's `SkinPacks[7]` loop reads one element past the array (UB); see
/// [`check_valid_skin_package`].
const SKIN_PACKS: [&str; 7] = [
    "BOSSSKINS",
    "SOLDIERSKINS",
    "SGIRLSKINS",
    "COMMANDOSKINS",
    "TNALIMESHSKINS",
    "TSKMSKINS",
    "TCOWMESHSKINS",
];

/// Engine services for the file, URL, skin, map and console natives.
///
/// Every callback returns status separately from its value. Zero is success;
/// failure propagates to the public native guard before any subsequent callback.
/// Text is copied immediately; native per-invocation scopes own its storage.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeFilesServices {
    /// Reads the GUID from the native exec's lexical config context.
    pub cache_get_string: unsafe extern "C" fn(context: *mut c_void, out: *mut *const u16) -> i32,
    /// Clears the GUID in that same native config object; does not flush/destroy it.
    pub cache_clear: unsafe extern "C" fn(context: *mut c_void) -> i32,
    /// `GSys->CachePath`.
    pub cache_path: unsafe extern "C" fn(out: *mut *const u16) -> i32,
    /// `GSys->CacheExt`.
    pub cache_ext: unsafe extern "C" fn(out: *mut *const u16) -> i32,
    /// `PATH_SEPARATOR`.
    pub path_separator: unsafe extern "C" fn(out: *mut *const u16) -> i32,
    /// `GSys->Paths.Num()`.
    pub paths_count: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GSys->Paths(i)`.
    pub path_at: unsafe extern "C" fn(i: i32, out: *mut *const u16) -> i32,
    /// `GFileManager->Move(dest, src)`; nonzero on success.
    pub file_move: unsafe extern "C" fn(dest: *const u16, src: *const u16, out: *mut i32) -> i32,
    /// `debugf(TEXT("MoveCacheEntry: No extension: %s"), new_filename)`.
    pub log_no_extension: unsafe extern "C" fn(new_filename: *const u16) -> i32,
    /// `debugf(TEXT("MoveCacheEntry: %s -> %s"), src, dest)`.
    pub log_move: unsafe extern "C" fn(src: *const u16, dest: *const u16) -> i32,
    /// `FURL::LoadURLConfig("DefaultPlayer","User")` then `GetOption(option, "")`.
    /// `option` includes the trailing `=`. The result is copied by C++.
    pub url_default_option: unsafe extern "C" fn(option: *const u16, out: *mut *const u16) -> i32,
    /// `CastChecked<UGameEngine>(GetLevel()->Engine)->LastURL.AddOption(text)`.
    pub url_add_option: unsafe extern "C" fn(this_: *mut c_void, text: *const u16) -> i32,
    /// `CastChecked<UGameEngine>(...)->LastURL.SaveURLConfig("DefaultPlayer", option, "User")`.
    pub url_save_default: unsafe extern "C" fn(this_: *mut c_void, option: *const u16) -> i32,
    /// `engine.gameengine`'s `serverpackages` ExportText. Null when the class
    /// or property is missing. Empty when `Defaults` has no entries.
    pub server_packages: unsafe extern "C" fn(out: *mut *const u16) -> i32,
    /// `GFileManager->FindFiles(pattern, 1, 0)`; returns the count.
    pub find_files: unsafe extern "C" fn(pattern: *const u16, out: *mut i32) -> i32,
    /// `GFileManager->FindFiles` result `i`.
    pub found_file_at: unsafe extern "C" fn(i: i32, out: *mut *const u16) -> i32,
    /// `FURL::DefaultMapExt`.
    pub map_ext: unsafe extern "C" fn(out: *mut *const u16) -> i32,
    /// `GCdPath`.
    pub cd_path: unsafe extern "C" fn(out: *mut *const u16) -> i32,
    /// `findPathTowardBestInventory(newPath, 1, weight, predict)`. Writes the
    /// new path through `new_path` and returns the best weight.
    pub find_path_toward_best: unsafe extern "C" fn(
        this_: *mut c_void,
        weight: f32,
        predict: i32,
        new_path: *mut *mut c_void,
        out: *mut f32,
    ) -> i32,
    /// `path->IsProbing(NAME_SpecialHandling)`; nonzero when true.
    pub is_probing_special: unsafe extern "C" fn(path: *mut c_void, out: *mut i32) -> i32,
    /// `HandleSpecial(*path)`, retaining its native by-reference path update.
    pub handle_special: unsafe extern "C" fn(this_: *mut c_void, path: *mut *mut c_void) -> i32,
    /// `Cast<UViewport>(Player)`, then `Viewport->Input->GetClass()`. Null when
    /// either is missing.
    pub keyboard_config_class:
        unsafe extern "C" fn(this_: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ResetConfig(klass)`.
    pub reset_config: unsafe extern "C" fn(klass: *mut c_void) -> i32,
    /// `Player->Exec(command, StrOut)`; returns `StrOut`.
    pub player_exec:
        unsafe extern "C" fn(this_: *mut c_void, command: *const u16, out: *mut *const u16) -> i32,
    /// `GetLevel()->Engine->Exec(command, StrOut)`; returns `StrOut`.
    pub engine_exec:
        unsafe extern "C" fn(this_: *mut c_void, command: *const u16, out: *mut *const u16) -> i32,
    /// `Client->Viewports(0)->SizeX/SizeY`.
    pub viewport_size:
        unsafe extern "C" fn(this_: *mut c_void, size_x: *mut i32, size_y: *mut i32) -> i32,
    /// `Client->MaintainVerticalFOV`.
    pub maintain_vertical_fov: unsafe extern "C" fn(this_: *mut c_void, out: *mut i32) -> i32,
    /// `UClient::GetEffectiveFovAngle(fov, size_x, size_y, maintain)`.
    pub effective_fov: unsafe extern "C" fn(
        fov: f32,
        size_x: i32,
        size_y: i32,
        maintain: i32,
        out: *mut f32,
    ) -> i32,
    /// `eventPlayerCalcView(ViewActor=this, loc, rot)`.
    pub calc_view:
        unsafe extern "C" fn(this_: *mut c_void, loc: *mut c_void, rot: *mut c_void) -> i32,
    /// `AActor::execGetURLMap`: `Engine.LastURL.Map` as a NUL-terminated UNICHAR
    /// buffer. Valid until the next string-returning call.
    pub last_url_map: unsafe extern "C" fn(this_: *mut c_void, out: *mut *const u16) -> i32,
    /// `APlayerPawn::execGetEntryLevel`: the entry level info. The engine checks
    /// run inside the call.
    pub entry_level: unsafe extern "C" fn(this_: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UObject::GetRegistryObjects(Results, UClass::StaticClass(), FindObjectChecked<UClass>(ANY_PACKAGE, meta_class_name), 0)`.
    /// `meta_class_name` is a NUL-terminated UNICHAR buffer. Snapshots into the
    /// native scope; valid for `registry_object_at` until the next
    /// `registry_classes`/`registry_textures` call. Returns the count.
    pub registry_classes: unsafe extern "C" fn(meta_class_name: *const u16, out: *mut i32) -> i32,
    /// `UObject::GetRegistryObjects(Results, UTexture::StaticClass(), NULL, 0)`.
    /// Returns the count.
    pub registry_textures: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `Results(i).Object`/`.Description` from the last `registry_classes`/
    /// `registry_textures` snapshot. Both outputs stay valid simultaneously
    /// until the next `registry_object_at`/`registry_classes`/`registry_textures` call.
    pub registry_object_at: unsafe extern "C" fn(
        i: i32,
        out_object: *mut *const u16,
        out_description: *mut *const u16,
    ) -> i32,
    /// `FConfigCacheIni::GetSectionPrivate("Cache", 0, 1, IniName)` iterated into
    /// key/value snapshots; 0 when the section is missing. Returns the count.
    pub cache_entries: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// The key/value pair at `i` from the last `cache_entries` snapshot.
    pub cache_entry_at:
        unsafe extern "C" fn(i: i32, out_key: *mut *const u16, out_value: *mut *const u16) -> i32,
    /// Heap-allocates a new `Hp2PawnNativeFilesScope`, publishing it as the
    /// active per-invocation scope; `scope_exit` restores the parent and frees it.
    pub scope_enter: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// Restores the parent scope and frees `scope` (from `scope_enter`).
    pub scope_exit: unsafe extern "C" fn(scope: *mut c_void) -> i32,
    /// Widens the live native FString at `fstr` (its address) into a
    /// NUL-terminated UNICHAR buffer. Valid until the next string-returning
    /// call on this thread.
    pub str_units: unsafe extern "C" fn(fstr: *const c_void, out: *mut *const u16) -> i32,
    /// `*(FString*)fstr = Units` (narrows a NUL-terminated UNICHAR buffer).
    pub str_assign: unsafe extern "C" fn(fstr: *mut c_void, units: *const u16) -> i32,
    /// `execMoveCacheEntry`'s `FConfigCacheIni CacheIni; TCHAR IniName[256];`
    /// locals, heap-allocated so `cache_get_string`/`cache_clear` can share them
    /// across both calls. `guid` is a NUL-terminated UNICHAR buffer.
    pub cache_context_open: unsafe extern "C" fn(guid: *const u16, out: *mut *mut c_void) -> i32,
    /// Destroys a session opened by `cache_context_open`.
    pub cache_context_close: unsafe extern "C" fn(context: *mut c_void) -> i32,
    /// `clock(GetLevel()->FindPathCycles)`.
    pub find_path_clock_begin: unsafe extern "C" fn(this_: *mut c_void) -> i32,
    /// `unclock(GetLevel()->FindPathCycles)`.
    pub find_path_clock_end: unsafe extern "C" fn(this_: *mut c_void) -> i32,
}

static mut SERVICES: Option<Hp2PawnNativeFilesServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2PawnNativeFilesServices) {
    unsafe {
        (&raw mut SERVICES).write(Some(value));
    }
}

/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2PawnNativeFilesServices, i32> {
    unsafe { (&raw const SERVICES).read().ok_or(1) }
}

thread_local! {
    static OUT: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
}

/// Copies a NUL-terminated UNICHAR buffer. A null pointer reads as empty.
///
/// # Safety
/// `p` must be null or point at a readable NUL-terminated `u16` sequence.
pub unsafe fn units(p: *const u16) -> Vec<u16> {
    if p.is_null() {
        return Vec::new();
    }
    let mut len = 0usize;
    // SAFETY: the caller guarantees a NUL terminator within readable memory.
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` readable units were just observed before the terminator.
    unsafe { std::slice::from_raw_parts(p, len) }.to_vec()
}

/// Publishes `units` as a NUL-terminated buffer owned by this thread.
fn publish(units: &[u16]) -> *const u16 {
    OUT.with_borrow_mut(|buf| {
        buf.clear();
        buf.extend_from_slice(units);
        buf.push(0);
        buf.as_ptr()
    })
}

/// Appends a NUL terminator so the slice can be passed as a C string.
fn nul(units: &[u16]) -> Vec<u16> {
    let mut v = units.to_vec();
    v.push(0);
    v
}

/// `TEXT("...")` literal to UNICHAR units.
fn lit(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// ASCII fold (`appToUpper`/`appToLower` in the C locale).
fn fold(c: u16) -> u16 {
    if (b'a' as u16..=b'z' as u16).contains(&c) {
        c - 32
    } else {
        c
    }
}

fn lower(c: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&c) {
        c + 32
    } else {
        c
    }
}

/// `FString::Caps()`: uppercases every unit.
fn caps(s: &[u16]) -> Vec<u16> {
    s.iter().map(|&c| fold(c)).collect()
}

/// `appStricmp(a, b) == 0`. Inputs carry no NUL.
fn stricmp_eq(a: &[u16], b: &[u16]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| lower(x) == lower(y))
}

/// `appStrnicmp(a, b, n) == 0`: the first `n` units, stopping at the shorter NUL.
fn strnicmp_eq(a: &[u16], b: &[u16], n: usize) -> bool {
    for k in 0..n {
        let x = a.get(k).copied().unwrap_or(0);
        let y = b.get(k).copied().unwrap_or(0);
        if lower(x) != lower(y) {
            return false;
        }
        if x == 0 {
            return true;
        }
    }
    true
}

/// `FString::InStr(sub)` (`appStrstr`): first index of `needle`, `-1` when absent.
/// An empty needle matches at 0, as `wcsstr` does.
fn instr(hay: &[u16], needle: &[u16]) -> i32 {
    if needle.is_empty() {
        return 0;
    }
    hay.windows(needle.len())
        .position(|w| w == needle)
        .map_or(-1, |p| p as i32)
}

/// Text after the last `sep` (`FString` strip loop with `InStr`/`Mid`).
fn after_last(s: &[u16], sep: u16) -> Vec<u16> {
    match s.iter().rposition(|&c| c == sep) {
        Some(p) => s[p + 1..].to_vec(),
        None => s.to_vec(),
    }
}

/// `Right(n) == suffix` under `FString::operator==` (`appStricmp`), where `Right` clamps to the string length.
fn ends_with(s: &[u16], suffix: &[u16]) -> bool {
    s.len() >= suffix.len() && stricmp_eq(&s[s.len() - suffix.len()..], suffix)
}

/// `GetNextInt` / `GetNextIntDesc` index: `List(CurrentInt)` when in range.
///
/// Negative `CurrentInt` reads `List` out of bounds in the oracle (UB). It maps
/// to `-1`, the empty result.
pub fn registry_index(count: i32, current: i32) -> i32 {
    if current >= 0 && current < count {
        current
    } else {
        -1
    }
}

/// `GetCacheEntry` index: the `num`-th entry whose value is non-empty, in the
/// order the `Cache` section iterates. `empty[i]` is `*It.Value()==0`.
pub fn cache_entry_index(num: i32, empty: &[bool]) -> i32 {
    let mut seen = 0i32;
    for (idx, &is_empty) in empty.iter().enumerate() {
        if is_empty {
            continue;
        }
        if seen == num {
            return idx as i32;
        }
        seen += 1;
    }
    -1
}

/// `GetNextSkin` selection over `GetRegistryObjects(..., UTexture, NULL, 0)`.
/// Returns the chosen index, or `None` for the empty result.
///
/// `dir == -1` takes the previous match, `dir == 0` takes the current match,
/// and any other value takes the next match (wrapping to the first).
pub fn next_skin_index(
    objects: &[Vec<u16>],
    prefix: &[u16],
    current: &[u16],
    dir: i32,
) -> Option<usize> {
    let n = prefix.len();
    let mut use_skin: i32 = -1;
    let mut first_skin: i32 = -1;
    let mut prev_skin: i32 = -1;
    let mut use_next = false;
    for (i, obj) in objects.iter().enumerate() {
        if !strnicmp_eq(obj, prefix, n) {
            continue;
        }
        if use_next {
            use_skin = i as i32;
            use_next = false;
            break;
        }
        if first_skin == -1 {
            first_skin = i as i32;
        }
        if stricmp_eq(obj, current) {
            if dir == -1 {
                use_skin = prev_skin;
                break;
            } else if dir == 0 {
                use_skin = i as i32;
                break;
            } else {
                use_next = true;
            }
        }
        prev_skin = i as i32;
    }
    if use_next {
        use_skin = first_skin;
    }
    if use_skin == -1 {
        use_skin = prev_skin;
    }
    if use_skin >= 0 && (use_skin as usize) < objects.len() {
        Some(use_skin as usize)
    } else {
        None
    }
}

/// `CheckValidSkinPackage(Pkg, MeshName)`. Returns the oracle's `!bProscribed`.
///
/// Three quirks are kept on purpose:
/// - `if (Pkg.InStr(List))` is truthy for any index other than 0, including
///   `-1` (not found). So `bFound` is set unless the whole list starts `Pkg`.
/// - `List.Caps();` in the oracle discards its result, so `List` is searched in its exported case.
/// - The oracle's `for (i<8)` loop reads `SkinPacks[7]`, one past the array (UB).
///   This port stops at the seven real entries.
///
/// # Safety
/// `svc.server_packages` must return null or a NUL-terminated buffer.
pub unsafe fn check_valid_skin_package(
    svc: &Hp2PawnNativeFilesServices,
    pkg: &[u16],
    mesh: &[u16],
) -> PawnResult<bool> {
    check_valid_skin_package_with(pkg, mesh, || unsafe {
        let raw = native_value!(svc.server_packages,);
        Ok(if raw.is_null() { None } else { Some(units(raw)) })
    })
}

/// Preserves the source's folding/search rules using the real CDO property export.
pub(crate) fn check_valid_skin_package_with<E>(
    pkg: &[u16], mesh: &[u16],
    server_packages: impl FnOnce() -> Result<Option<Vec<u16>>, E>,
) -> Result<bool, E> {
    let pkg = caps(pkg);
    let found = match server_packages()? {
        Some(list) => instr(&pkg, &list) != 0,
        None => false,
    };
    let proscribed = if !found
        || stricmp_eq(&pkg, &lit("UNREALSHARE"))
        || stricmp_eq(&pkg, &lit("UNREALI"))
        || stricmp_eq(&pkg, &lit("BOTPACK"))
    {
        true
    } else {
        let mut legal = mesh.to_vec();
        legal.extend(lit("SKINS"));
        let legal = caps(&legal);
        instr(&pkg, &legal) == -1
            && SKIN_PACKS
                .into_iter()
                .any(|pack| instr(&pkg, &lit(pack)) != -1)
    };
    Ok(!proscribed)
}

/// Operations needed by the shared `UpdateURL` policy.
pub(crate) trait UpdateUrlOps {
    type Error;

    fn add_option(&mut self, text: &[u16]) -> Result<(), Self::Error>;
    fn save_default(&mut self, option: &[u16]) -> Result<(), Self::Error>;
}

/// Adds `option=value` to the live last URL, then optionally persists that option.
pub(crate) fn update_url_with<O: UpdateUrlOps>(
    ops: &mut O,
    option: &[u16],
    value: &[u16],
    save: bool,
) -> Result<(), O::Error> {
    let mut text = option.to_vec();
    text.extend(lit("="));
    text.extend_from_slice(value);
    ops.add_option(&text)?;
    if save {
        ops.save_default(option)?;
    }
    Ok(())
}

struct NativeUpdateUrlOps<'a> {
    services: &'a Hp2PawnNativeFilesServices,
    actor: *mut c_void,
}

impl UpdateUrlOps for NativeUpdateUrlOps<'_> {
    type Error = i32;

    fn add_option(&mut self, text: &[u16]) -> PawnResult<()> {
        unsafe { checked((self.services.url_add_option)(self.actor, nul(text).as_ptr())) }
    }

    fn save_default(&mut self, option: &[u16]) -> PawnResult<()> {
        unsafe { checked((self.services.url_save_default)(self.actor, nul(option).as_ptr())) }
    }
}

/// `UpdateURL`: adds `option=value` to the engine's last URL and optionally
/// saves `option` to the default-player config.
///
/// # Safety
/// `this` and the pointers in `svc` must be valid for the engine call.
pub unsafe fn update_url(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
    option: &[u16],
    value: &[u16],
    save: bool,
) -> PawnResult<()> {
    let mut ops = NativeUpdateUrlOps { services: svc, actor: this };
    update_url_with(&mut ops, option, value, save)
}

/// `GetDefaultURL`: `GetOption(Option + "=", "")` on the saved default URL.
///
/// # Safety
/// The services must return valid NUL-terminated buffers.
pub unsafe fn get_default_url(
    svc: &Hp2PawnNativeFilesServices,
    option: &[u16],
) -> PawnResult<Vec<u16>> {
    let mut key = option.to_vec();
    key.extend(lit("="));
    // SAFETY: `key` is NUL-terminated for the call.
    Ok(unsafe { units(native_value!(svc.url_default_option, nul(&key).as_ptr())) })
}

/// `AActor::execGetURLMap`: the last URL's map name. The pointer comes from the
/// services and stays valid until the next string-returning call.
///
/// # Safety
/// `svc` must be a valid services table and `this` a live `AActor`.
pub unsafe fn url_map(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
) -> PawnResult<*const u16> {
    // SAFETY: the services return a buffer the caller copies before the next call.
    Ok(unsafe { native_value!(svc.last_url_map, this) })
}

/// `APlayerPawn::execGetEntryLevel`: the entry level info, with the engine checks
/// run by the services.
///
/// # Safety
/// `svc` must be a valid services table and `this` a live `APlayerPawn`.
pub unsafe fn entry_level(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
) -> PawnResult<*mut c_void> {
    // SAFETY: the services are called with the pawn that the native was invoked on.
    Ok(unsafe { native_value!(svc.entry_level, this) })
}

/// Operations used by the cache-file move policy. Path arguments passed to
/// logging and moving include their terminal NUL, as in the native expression.
pub trait MoveCacheEntryOps {
    type Error;
    fn cached_filename(&mut self) -> Result<Option<Vec<u16>>, Self::Error>;
    fn cache_path(&mut self) -> Result<Vec<u16>, Self::Error>;
    fn cache_ext(&mut self) -> Result<Vec<u16>, Self::Error>;
    fn path_separator(&mut self) -> Result<Vec<u16>, Self::Error>;
    fn paths_count(&mut self) -> Result<i32, Self::Error>;
    fn path_at(&mut self, index: i32) -> Result<Vec<u16>, Self::Error>;
    fn log_no_extension(&mut self, name: &[u16]) -> Result<(), Self::Error>;
    fn log_move(&mut self, source: &[u16], destination: &[u16]) -> Result<(), Self::Error>;
    fn move_file(&mut self, destination: &[u16], source: &[u16]) -> Result<bool, Self::Error>;
    fn clear_cache_entry(&mut self) -> Result<(), Self::Error>;
}

struct NativeCacheMove<'a> {
    services: &'a Hp2PawnNativeFilesServices,
    context: *mut c_void,
}

impl MoveCacheEntryOps for NativeCacheMove<'_> {
    type Error = i32;
    fn cached_filename(&mut self) -> PawnResult<Option<Vec<u16>>> {
        let value = unsafe { native_value!(self.services.cache_get_string, self.context) };
        Ok(if value.is_null() { None } else { Some(unsafe { units(value) }) })
    }
    fn cache_path(&mut self) -> PawnResult<Vec<u16>> {
        Ok(unsafe { units(native_value!(self.services.cache_path)) })
    }
    fn cache_ext(&mut self) -> PawnResult<Vec<u16>> {
        Ok(unsafe { units(native_value!(self.services.cache_ext)) })
    }
    fn path_separator(&mut self) -> PawnResult<Vec<u16>> {
        Ok(unsafe { units(native_value!(self.services.path_separator)) })
    }
    fn paths_count(&mut self) -> PawnResult<i32> {
        Ok(unsafe { native_value!(self.services.paths_count) })
    }
    fn path_at(&mut self, index: i32) -> PawnResult<Vec<u16>> {
        Ok(unsafe { units(native_value!(self.services.path_at, index)) })
    }
    fn log_no_extension(&mut self, name: &[u16]) -> PawnResult<()> {
        unsafe { checked((self.services.log_no_extension)(name.as_ptr())) }
    }
    fn log_move(&mut self, source: &[u16], destination: &[u16]) -> PawnResult<()> {
        unsafe { checked((self.services.log_move)(source.as_ptr(), destination.as_ptr())) }
    }
    fn move_file(&mut self, destination: &[u16], source: &[u16]) -> PawnResult<bool> {
        Ok(unsafe { native_value!(self.services.file_move, destination.as_ptr(), source.as_ptr()) } != 0)
    }
    fn clear_cache_entry(&mut self) -> PawnResult<()> {
        unsafe { checked((self.services.cache_clear)(self.context)) }
    }
}

/// `MoveCacheEntry(GUID, NewFilename)`: moves the cached file to the first
/// `GSys->Paths` entry with the same extension, then clears the cache entry.
/// Returns the oracle's `Result` (true when the move succeeded).
///
/// # Safety
/// The services return valid NUL-terminated buffers. `context` points to the
/// native exec's live config/GUID/IniName session until this call returns.
pub unsafe fn move_cache_entry(
    svc: &Hp2PawnNativeFilesServices,
    context: *mut c_void,
    guid: &[u16],
    new_filename: &[u16],
) -> PawnResult<bool> {
    move_cache_entry_with(&mut NativeCacheMove { services: svc, context }, guid, new_filename)
}

pub fn move_cache_entry_with<O: MoveCacheEntryOps>(
    ops: &mut O,
    guid: &[u16],
    new_filename: &[u16],
) -> Result<bool, O::Error> {
    let Some(orig) = ops.cached_filename()? else { return Ok(false); };
    let name = if new_filename.is_empty() {
        orig
    } else {
        new_filename.to_vec()
    };
    let name = after_last(
        &after_last(&after_last(&name, b'/' as u16), b'\\' as u16),
        b':' as u16,
    );
    let Some(dot) = name.iter().rposition(|&c| c == b'.' as u16) else {
        let p = nul(&name);
        ops.log_no_extension(&p)?;
        return Ok(false);
    };
    let mut file_ext = lit(".");
    file_ext.extend_from_slice(&name[dot + 1..]);
    let mut i = 0;
    while i < ops.paths_count()? {
        // Each condition observes the native path list at that iteration.
        let path = ops.path_at(i)?;
        i += 1;
        if !ends_with(&path, &file_ext) {
            continue;
        }
        // SeparatorIndex: last `/`, `\` or `:`; DestPath = Left(SeparatorIndex+1).
        let dest_path: &[u16] = match path
            .iter()
            .rposition(|&c| c == b'/' as u16 || c == b'\\' as u16 || c == b':' as u16)
        {
            Some(p) => &path[..=p],
            None => &[],
        };
        let src = cache_source_with(ops, guid)?;
        let mut dest = dest_path.to_vec();
        dest.extend_from_slice(&name);
        dest.push(0);
        ops.log_move(&src, &dest)?;
        drop(src);
        // debugf is a native reentry seam: the original reevaluates CachePath
        // and CacheExt for Move rather than reusing the logging expression.
        let src = cache_source_with(ops, guid)?;
        let moved = ops.move_file(&dest, &src)?;
        if moved {
            ops.clear_cache_entry()?;
        }
        return Ok(moved);
    }
    Ok(false)
}

/// Constructs the original native source expression at its point of use.
fn cache_source_with<O: MoveCacheEntryOps>(ops: &mut O, guid: &[u16]) -> Result<Vec<u16>, O::Error> {
    let mut source = ops.cache_path()?;
    source.extend(ops.path_separator()?);
    source.extend_from_slice(guid);
    source.extend(ops.cache_ext()?);
    source.push(0);
    Ok(source)
}

/// `ConsoleCommand`: runs the command on the player when one exists, otherwise
/// on the engine. Returns the captured output.
///
/// # Safety
/// `this` must be valid for the services.
pub unsafe fn console_command(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
    command: &[u16],
) -> PawnResult<Vec<u16>> {
    let cmd = nul(command);
    unsafe { console_command_with(&mut NativeConsoleCommand { svc, this, command: cmd.as_ptr() }, this.cast()) }
}

/// Typed console targets retain their own text representation and output storage.
pub(crate) trait ConsoleCommandOps {
    type Error;
    type Output;
    unsafe fn player_exec(&mut self) -> Result<Self::Output, Self::Error>;
    unsafe fn engine_exec(&mut self) -> Result<Self::Output, Self::Error>;
}

/// Shares the live Player gate without requiring unrelated file services.
///
/// # Safety
/// Pawn and both target providers remain live through synchronous command dispatch.
pub(crate) unsafe fn console_command_with<O: ConsoleCommandOps>(
    ops: &mut O, pawn: *mut APlayerPawn,
) -> Result<O::Output, O::Error> {
    unsafe {
        if !(*pawn).player.is_null() { ops.player_exec() } else { ops.engine_exec() }
    }
}

struct NativeConsoleCommand<'a> {
    svc: &'a Hp2PawnNativeFilesServices,
    this: *mut c_void,
    command: *const u16,
}
impl ConsoleCommandOps for NativeConsoleCommand<'_> {
    type Error = i32;
    type Output = Vec<u16>;
    unsafe fn player_exec(&mut self) -> PawnResult<Vec<u16>> {
        unsafe { Ok(units(native_value!(self.svc.player_exec, self.this, self.command))) }
    }
    unsafe fn engine_exec(&mut self) -> PawnResult<Vec<u16>> {
        unsafe { Ok(units(native_value!(self.svc.engine_exec, self.this, self.command))) }
    }
}

/// `ResetKeyboard`: resets the viewport input class's config when one exists.
///
/// # Safety
/// `this` must be valid for the services.
pub unsafe fn reset_keyboard(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
) -> PawnResult<()> {
    reset_keyboard_with(
        || unsafe { Ok(native_value!(svc.keyboard_config_class, this)) },
        |klass| unsafe { checked((svc.reset_config)(klass)) },
    )
}

/// Shares ResetKeyboard's class-selection/error ordering between native and
/// canonical objects; the selected input class owns the config reset.
pub fn reset_keyboard_with<E>(
    mut config_class: impl FnMut() -> Result<*mut c_void, E>,
    mut reset_config: impl FnMut(*mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    let class = config_class()?;
    if !class.is_null() {
        reset_config(class)?;
    }
    Ok(())
}

/// `FindBestInventoryPath` body after the clock. Returns `bestPath`, and writes
/// `*Weight = BestWeight` through `weight` when the search improved it.
///
/// # Safety
/// `this` is valid for the services; `weight_out` is an initialized, readable
/// and writable native Weight reference, which reentrant script may update.
pub unsafe fn find_best_inventory_path(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
    weight: f32,
    predict: bool,
    weight_out: *mut f32,
) -> PawnResult<*mut c_void> {
    unsafe { find_best_inventory_path_with(
        this, weight, predict, weight_out,
        |weight, predict, path| Ok(native_value!(svc.find_path_toward_best, this, weight, i32::from(predict), path)),
        |path| Ok(native_value!(svc.is_probing_special, path) != 0),
        |path| checked((svc.handle_special)(this, path)),
    ) }
}

/// Shared inventory-selection/Weight policy over graph-owned search and event callbacks.
///
/// # Safety
/// Same live Pawn and caller-owned Weight-reference contract as `find_best_inventory_path`.
pub unsafe fn find_best_inventory_path_with<E>(
    this: *mut c_void, weight: f32, predict: bool, weight_out: *mut f32,
    search: impl FnOnce(f32, bool, *mut *mut c_void) -> Result<f32, E>,
    probing: impl FnOnce(*mut c_void) -> Result<bool, E>,
    special: impl FnOnce(*mut *mut c_void) -> Result<(), E>,
) -> Result<*mut c_void, E> {
    let mut new_path: *mut c_void = std::ptr::null_mut();
    let best_weight = search(weight, predict, &raw mut new_path)?;
    let mut best_path: *mut c_void = std::ptr::null_mut();
    // Script reached during the search can mutate the caller's Weight property.
    if best_weight > unsafe { *weight_out } {
        best_path = new_path;
        // SAFETY: `weight_out` is the caller's `Weight` reference.
        unsafe {
            *weight_out = best_weight;
            let pawn = this.cast::<APawn>();
            (*pawn).special_pause = 0.0;
            (*pawn).set_b_shoot_special(false);
            if !best_path.is_null() && probing(best_path)? {
                special(&raw mut best_path)?;
            }
            if best_path == (*this.cast::<APawn>()).special_goal {
                (*this.cast::<APawn>()).special_goal = std::ptr::null_mut();
            }
        }
    }
    Ok(best_path)
}

/// Native SDK adapter for the shared projection policy.
///
/// # Safety
/// Pawn, output and immutable services remain live through dispatch.
pub unsafe fn screen_to_world(
    svc: &Hp2PawnNativeFilesServices, this: *mut c_void, s: FVector, out: *mut FVector,
) -> PawnResult<()> {
    unsafe { screen_to_world_with(&mut NativeScreenToWorld(svc), this, s, out) }
}

/// `ScreenToWorld(S)`: projects a screen point through the pawn's view.
///
/// # Safety
/// `this` and `out` must be valid for the services and writes.
pub(crate) unsafe fn screen_to_world_with<O: ScreenToWorldOps>(
    ops: &mut O,
    this: *mut c_void,
    s: FVector,
    out: *mut FVector,
) -> Result<(), O::Error> {
    let (size_x, size_y) = unsafe { ops.viewport_size(this)? };
    // SAFETY: the services read the viewport and pawn fields.
    let effective = unsafe {
        let fov = (*this.cast::<APawn>()).fov_angle;
        let maintain = ops.maintain_vertical_fov(this)?;
        ops.effective_fov(fov, size_x, size_y, maintain)?
    };
    // `ProjZ = Viewport->SizeX / appTan(EffectiveFovAngle * PI/360.f)`:
    // `appTan` is `tan` on a double, and the quotient is double before the
    // narrowing to FLOAT.
    let half = effective * PI_F32 / 360.0_f32;
    let proj_z = (size_x as f64 / (half as f64).tan()) as f32;
    let z = s.z / proj_z;
    let c = FVector {
        x: 2.0_f32.mul_add(s.x, -(size_x as f32)) * z,
        y: 2.0_f32.mul_add(s.y, -(size_y as f32)) * z,
        z: s.z,
    };
    let mut loc: FVector;
    let mut rot: FRotator;
    // SAFETY: `calc_view` writes the view location and rotation.
    unsafe {
        loc = (*this.cast::<AActor>()).location;
        rot = (*this.cast::<AActor>()).rotation;
        ops.calc_view(this, &mut loc, &mut rot)?;
    }
    // Read in the same yaw/pitch/roll order as mul_rot. Each native failure
    // returns before a subsequent trigonometric callback or the output write.
    let mut samples = [(0.0_f32, 0.0_f32); 3];
    for (index, angle) in [rot.yaw, rot.pitch, rot.roll].into_iter().enumerate() {
        let cosine = math::cos_tab_inline(angle, math::trig_table());
        let sine = math::sin_tab_inline(angle, math::trig_table());
        samples[index] = (cosine, sine);
    }
    let sine_index = std::cell::Cell::new(0);
    let cosine_index = std::cell::Cell::new(0);
    let sin = |_| {
        let i = sine_index.replace(sine_index.get() + 1);
        samples[i].1
    };
    let cos = |_| {
        let i = cosine_index.replace(cosine_index.get() + 1);
        samples[i].0
    };
    let trig = Trig {
        sin: &sin,
        cos: &cos,
    };
    // `GMath.UnitCoords * ViewRotation`.
    let mut frame = coords::unit_coords();
    coords::mul_rot(&mut frame, rot, &trig);
    // `C.TransformVectorBy(frame) + ViewLocation`.
    let result = prim::add(prim::transform_vector(c, &frame), loc);
    // SAFETY: `out` is the caller's result slot.
    unsafe { *out = result };
    Ok(())
}

/// Publishes a string result for the C++ shell to copy.
pub fn publish_units(units: &[u16]) -> *const u16 {
    publish(units)
}

/// Converts a `(ptr, len)` UNICHAR array into owned strings for `next_skin_index`.
///
/// # Safety
/// `objects` must point at `count` readable NUL-terminated string pointers.
pub unsafe fn object_names(objects: *const *const u16, count: i32) -> Vec<Vec<u16>> {
    if objects.is_null() || count <= 0 {
        return Vec::new();
    }
    // SAFETY: the caller guarantees `count` readable entries.
    (0..count as usize)
        .map(|i| unsafe { units(*objects.add(i)) })
        .collect()
}

/// Backend operations shared by native `GFileManager` and canonical Core paths.
pub(crate) trait GetMapNameOps {
    type Error;

    fn map_extension(&mut self) -> Result<Vec<u16>, Self::Error>;
    fn cd_path(&mut self) -> Result<Vec<u16>, Self::Error>;
    fn path_separator(&mut self) -> Result<Vec<u16>, Self::Error>;
    fn paths_count(&mut self) -> Result<i32, Self::Error>;
    fn path_at(&mut self, index: i32) -> Result<Vec<u16>, Self::Error>;
    fn find_files(&mut self, pattern: &[u16]) -> Result<Vec<Vec<u16>>, Self::Error>;
}

/// Shared native `GetMapName` path scan; ordering, case-insensitive de-duplication,
/// directory-name stripping, and wraparound selection are identical for both owners.
pub(crate) fn get_map_name_with<O: GetMapNameOps>(
    ops: &mut O,
    prefix: &[u16],
    map: &[u16],
    dir: i32,
) -> Result<Vec<u16>, O::Error> {
    let mut wildcard = lit("*.");
    wildcard.extend(ops.map_extension()?);
    let mut names: Vec<Vec<u16>> = Vec::new();
    let mut do_cd = 0;
    loop {
        if do_cd != 0 && ops.cd_path()?.is_empty() {
            break;
        }
        let mut i = 0;
        while i < ops.paths_count()? {
            let path = ops.path_at(i)?;
            i += 1;
            if instr(&path, &wildcard) == -1 {
                continue;
            }
            let mut pattern = Vec::new();
            if do_cd != 0 {
                pattern.extend(ops.cd_path()?);
                pattern.extend(lit("System"));
                pattern.extend(ops.path_separator()?);
            }
            pattern.extend(path);
            pattern.truncate(instr(&pattern, &wildcard) as usize);
            pattern.extend_from_slice(prefix);
            pattern.extend_from_slice(&wildcard);
            pattern.push(0);
            for found in ops.find_files(&pattern)? {
                if !names.iter().any(|name| stricmp_eq(name, &found)) {
                    names.push(found);
                }
            }
        }
        do_cd += 1;
        if do_cd == 2 {
            break;
        }
    }
    for name in &mut names {
        if name.contains(&(b'\\' as u16)) {
            *name = after_last(name, b'\\' as u16);
        }
    }
    for (i, name) in names.iter().enumerate() {
        if name.as_slice() == map {
            let mut index = i as i32 + dir;
            if index < 0 {
                index = names.len() as i32 - 1;
            } else if index as usize >= names.len() {
                index = 0;
            }
            return Ok(names.swap_remove(index as usize));
        }
    }
    Ok(if names.is_empty() {
        Vec::new()
    } else {
        names.swap_remove(0)
    })
}

struct NativeGetMapNameOps<'a>(&'a Hp2PawnNativeFilesServices);

impl GetMapNameOps for NativeGetMapNameOps<'_> {
    type Error = i32;

    fn map_extension(&mut self) -> Result<Vec<u16>, Self::Error> {
        Ok(unsafe { units(native_value!(self.0.map_ext)) })
    }

    fn cd_path(&mut self) -> Result<Vec<u16>, Self::Error> {
        Ok(unsafe { units(native_value!(self.0.cd_path)) })
    }

    fn path_separator(&mut self) -> Result<Vec<u16>, Self::Error> {
        Ok(unsafe { units(native_value!(self.0.path_separator)) })
    }

    fn paths_count(&mut self) -> Result<i32, Self::Error> {
        Ok(unsafe { native_value!(self.0.paths_count) })
    }

    fn path_at(&mut self, index: i32) -> Result<Vec<u16>, Self::Error> {
        Ok(unsafe { units(native_value!(self.0.path_at, index)) })
    }

    fn find_files(&mut self, pattern: &[u16]) -> Result<Vec<Vec<u16>>, Self::Error> {
        let count = unsafe { native_value!(self.0.find_files, pattern.as_ptr()) };
        (0..count)
            .map(|index| Ok(unsafe { units(native_value!(self.0.found_file_at, index)) }))
            .collect()
    }
}

/// `GetMapName`: visits live native paths and owns each returned name before
/// another callback. Reentrant scans have independent native result arrays.
///
/// # Safety
/// The services must return valid NUL-terminated buffers.
pub unsafe fn get_map_name(
    svc: &Hp2PawnNativeFilesServices,
    prefix: &[u16],
    map: &[u16],
    dir: i32,
) -> PawnResult<Vec<u16>> {
    get_map_name_with(&mut NativeGetMapNameOps(svc), prefix, map, dir)
}

/// Registry snapshots consumed by shared Engine class and texture selectors.
pub(crate) trait RegistryLookupOps {
    type Error;
    fn classes(&mut self, class_name: &[u16]) -> Result<i32, Self::Error>;
    fn textures(&mut self) -> Result<i32, Self::Error>;
    fn object_at(&mut self, index: i32) -> Result<(Vec<u16>, Vec<u16>), Self::Error>;
}

pub(crate) fn get_next_int_with<O: RegistryLookupOps>(
    ops: &mut O,
    class_name: &[u16],
    current_int: i32,
) -> Result<Vec<u16>, O::Error> {
    let count = ops.classes(class_name)?;
    let index = registry_index(count, current_int);
    if index < 0 { return Ok(lit("")); }
    Ok(ops.object_at(index)?.0)
}

pub(crate) fn get_next_int_desc_with<O: RegistryLookupOps>(
    ops: &mut O,
    class_name: &[u16],
    current_int: i32,
) -> Result<(Vec<u16>, Vec<u16>), O::Error> {
    let count = ops.classes(class_name)?;
    let index = registry_index(count, current_int);
    if index < 0 { return Ok((lit(""), lit(""))); }
    ops.object_at(index)
}

pub(crate) fn get_next_skin_with<O: RegistryLookupOps>(
    ops: &mut O,
    prefix: &[u16],
    current: &[u16],
    dir: i32,
) -> Result<(Vec<u16>, Vec<u16>), O::Error> {
    let count = ops.textures()?.max(0);
    let mut objects = Vec::with_capacity(count as usize);
    let mut descriptions = Vec::with_capacity(count as usize);
    for index in 0..count {
        let (object, description) = ops.object_at(index)?;
        objects.push(object);
        descriptions.push(description);
    }
    Ok(match next_skin_index(&objects, prefix, current, dir) {
        Some(index) => (objects[index].clone(), descriptions[index].clone()),
        None => (lit(""), lit("")),
    })
}

struct NativeRegistryLookup<'a>(&'a Hp2PawnNativeFilesServices);

impl RegistryLookupOps for NativeRegistryLookup<'_> {
    type Error = i32;

    fn classes(&mut self, class_name: &[u16]) -> Result<i32, Self::Error> {
        let class_name = nul(class_name);
        // SAFETY: the NUL-terminated class name remains live for this snapshot call.
        Ok(unsafe { native_value!(self.0.registry_classes, class_name.as_ptr()) })
    }

    fn textures(&mut self) -> Result<i32, Self::Error> {
        // SAFETY: the callback snapshots the texture registry and returns its count.
        Ok(unsafe { native_value!(self.0.registry_textures) })
    }

    fn object_at(&mut self, index: i32) -> Result<(Vec<u16>, Vec<u16>), Self::Error> {
        let mut object = std::mem::MaybeUninit::uninit();
        let mut description = std::mem::MaybeUninit::uninit();
        // SAFETY: caller only requests indices within the last registry snapshot.
        unsafe {
            checked((self.0.registry_object_at)(index, object.as_mut_ptr(), description.as_mut_ptr()))?;
            Ok((units(object.assume_init()), units(description.assume_init())))
        }
    }
}

/// `AActor::execGetNextInt`; returns an empty string when the index is out of range.
///
/// # Safety
/// The services must return valid NUL-terminated buffers and honor each snapshot count.
pub unsafe fn get_next_int(
    svc: &Hp2PawnNativeFilesServices,
    class_name: &[u16],
    current_int: i32,
) -> PawnResult<Vec<u16>> {
    get_next_int_with(&mut NativeRegistryLookup(svc), class_name, current_int)
}

/// `AActor::execGetNextIntDesc`; empty outputs when the index is out of range.
///
/// # Safety
/// The services must return valid NUL-terminated buffers and honor each snapshot count.
pub unsafe fn get_next_int_desc(
    svc: &Hp2PawnNativeFilesServices,
    class_name: &[u16],
    current_int: i32,
) -> PawnResult<(Vec<u16>, Vec<u16>)> {
    get_next_int_desc_with(&mut NativeRegistryLookup(svc), class_name, current_int)
}

/// Operations used by the shared cache-entry selection kernel.
pub(crate) trait CacheEntryOps {
    type Error;
    fn cache_entries(&mut self) -> Result<Vec<(Vec<u16>, Vec<u16>)>, Self::Error>;
}

/// Selects the `num`-th non-empty Cache section entry in its original order.
pub(crate) fn get_cache_entry_with<O: CacheEntryOps>(
    ops: &mut O,
    num: i32,
) -> Result<Option<(Vec<u16>, Vec<u16>)>, O::Error> {
    let entries = ops.cache_entries()?;
    let empty: Vec<bool> = entries.iter().map(|(_, value)| value.is_empty()).collect();
    let index = cache_entry_index(num, &empty);
    Ok((index >= 0).then(|| entries[index as usize].clone()))
}

struct NativeCacheEntryOps<'a>(&'a Hp2PawnNativeFilesServices);

impl CacheEntryOps for NativeCacheEntryOps<'_> {
    type Error = i32;

    fn cache_entries(&mut self) -> Result<Vec<(Vec<u16>, Vec<u16>)>, Self::Error> {
        // SAFETY: the service snapshots the Cache section and returns its count.
        let count = unsafe { native_value!(self.0.cache_entries) };
        let mut entries = Vec::with_capacity(count.max(0) as usize);
        for index in 0..count {
            let mut key = std::mem::MaybeUninit::uninit();
            let mut value = std::mem::MaybeUninit::uninit();
            // SAFETY: index is within the snapshot returned above.
            unsafe {
                checked((self.0.cache_entry_at)(index, key.as_mut_ptr(), value.as_mut_ptr()))?;
                entries.push((units(key.assume_init()), units(value.assume_init())));
            }
        }
        Ok(entries)
    }
}

/// `AActor::execGetCacheEntry`; `None` leaves the caller's output strings unchanged.
///
/// # Safety
/// The services must return valid NUL-terminated buffers and `cache_entry_at`
/// must honor the count returned by `cache_entries`.
pub unsafe fn get_cache_entry(
    svc: &Hp2PawnNativeFilesServices,
    num: i32,
) -> PawnResult<Option<(Vec<u16>, Vec<u16>)>> {
    get_cache_entry_with(&mut NativeCacheEntryOps(svc), num)
}

/// `AActor::execGetNextSkin` uses the same registry selection for Native and Core.
///
/// # Safety
/// The services must return valid NUL-terminated buffers and honor each snapshot count.
pub unsafe fn get_next_skin(
    svc: &Hp2PawnNativeFilesServices,
    prefix: &[u16],
    current: &[u16],
    dir: i32,
) -> PawnResult<(Vec<u16>, Vec<u16>)> {
    get_next_skin_with(&mut NativeRegistryLookup(svc), prefix, current, dir)
}

pub(crate) trait ScreenToWorldOps {
    type Error;
    unsafe fn viewport_size(&mut self, pawn: *mut c_void) -> Result<(i32, i32), Self::Error>;
    unsafe fn maintain_vertical_fov(&mut self, pawn: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn effective_fov(&mut self, fov: f32, x: i32, y: i32, maintain: i32) -> Result<f32, Self::Error>;
    unsafe fn calc_view(&mut self, pawn: *mut c_void, location: &mut FVector, rotation: &mut FRotator) -> Result<(), Self::Error>;
}
struct NativeScreenToWorld<'a>(&'a Hp2PawnNativeFilesServices);
impl ScreenToWorldOps for NativeScreenToWorld<'_> {
    type Error = i32;
    unsafe fn viewport_size(&mut self, pawn: *mut c_void) -> PawnResult<(i32, i32)> {
        let mut x = 0; let mut y = 0;
        unsafe { checked((self.0.viewport_size)(pawn, &mut x, &mut y))?; }
        Ok((x, y))
    }
    unsafe fn maintain_vertical_fov(&mut self, pawn: *mut c_void) -> PawnResult<i32> {
        unsafe { Ok(native_value!(self.0.maintain_vertical_fov, pawn)) }
    }
    unsafe fn effective_fov(&mut self, fov: f32, x: i32, y: i32, maintain: i32) -> PawnResult<f32> {
        unsafe { Ok(native_value!(self.0.effective_fov, fov, x, y, maintain)) }
    }
    unsafe fn calc_view(&mut self, pawn: *mut c_void, location: &mut FVector, rotation: &mut FRotator) -> PawnResult<()> {
        unsafe { checked((self.0.calc_view)(pawn, ptr::from_mut(location).cast(), ptr::from_mut(rotation).cast())) }
    }
}
