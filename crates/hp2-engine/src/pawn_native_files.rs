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

use hp2_abi::FVector;
use hp2_abi::engine::generated::FRotator;

use crate::coords::{self, Trig};
use crate::prim;

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
/// Every `*const u16` return is a NUL-terminated UNICHAR buffer owned by C++.
/// Rust copies it before returning. A null return means "absent" where noted.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeFilesServices {
    /// `CacheIni.GetString("Cache", guid, out, 256, IniName)`. Null when missing.
    pub cache_get_string: unsafe extern "C" fn(guid: *const u16) -> *const u16,
    /// `CacheIni.SetString("Cache", guid, "", IniName)`.
    pub cache_clear: unsafe extern "C" fn(guid: *const u16),
    /// `GSys->CachePath`.
    pub cache_path: unsafe extern "C" fn() -> *const u16,
    /// `GSys->CacheExt`.
    pub cache_ext: unsafe extern "C" fn() -> *const u16,
    /// `PATH_SEPARATOR`.
    pub path_separator: unsafe extern "C" fn() -> *const u16,
    /// `GSys->Paths.Num()`.
    pub paths_count: unsafe extern "C" fn() -> i32,
    /// `GSys->Paths(i)`.
    pub path_at: unsafe extern "C" fn(i: i32) -> *const u16,
    /// `GFileManager->Move(dest, src)`; nonzero on success.
    pub file_move: unsafe extern "C" fn(dest: *const u16, src: *const u16) -> i32,
    /// `debugf(TEXT("MoveCacheEntry: No extension: %s"), new_filename)`.
    pub log_no_extension: unsafe extern "C" fn(new_filename: *const u16),
    /// `debugf(TEXT("MoveCacheEntry: %s -> %s"), src, dest)`.
    pub log_move: unsafe extern "C" fn(src: *const u16, dest: *const u16),
    /// `FURL::LoadURLConfig("DefaultPlayer","User")` then `GetOption(option, "")`.
    /// `option` includes the trailing `=`. The result is copied by C++.
    pub url_default_option: unsafe extern "C" fn(option: *const u16) -> *const u16,
    /// `CastChecked<UGameEngine>(GetLevel()->Engine)->LastURL.AddOption(text)`.
    pub url_add_option: unsafe extern "C" fn(this_: *mut c_void, text: *const u16),
    /// `CastChecked<UGameEngine>(...)->LastURL.SaveURLConfig("DefaultPlayer", option, "User")`.
    pub url_save_default: unsafe extern "C" fn(this_: *mut c_void, option: *const u16),
    /// `engine.gameengine`'s `serverpackages` ExportText. Null when the class
    /// or property is missing. Empty when `Defaults` has no entries.
    pub server_packages: unsafe extern "C" fn() -> *const u16,
    /// `GFileManager->FindFiles(pattern, 1, 0)`; returns the count.
    pub find_files: unsafe extern "C" fn(pattern: *const u16) -> i32,
    /// `GFileManager->FindFiles` result `i`.
    pub found_file_at: unsafe extern "C" fn(i: i32) -> *const u16,
    /// `FURL::DefaultMapExt`.
    pub map_ext: unsafe extern "C" fn() -> *const u16,
    /// `GCdPath`.
    pub cd_path: unsafe extern "C" fn() -> *const u16,
    /// `findPathTowardBestInventory(newPath, 1, weight, predict)`. Writes the
    /// new path through `new_path` and returns the best weight.
    pub find_path_toward_best: unsafe extern "C" fn(
        this_: *mut c_void,
        weight: f32,
        predict: i32,
        new_path: *mut *mut c_void,
    ) -> f32,
    /// `SpecialPause = 0.f; bShootSpecial = 0;`.
    pub reset_special_state: unsafe extern "C" fn(this_: *mut c_void),
    /// `path->IsProbing(NAME_SpecialHandling)`; nonzero when true.
    pub is_probing_special: unsafe extern "C" fn(path: *mut c_void) -> i32,
    /// `HandleSpecial(path)`.
    pub handle_special: unsafe extern "C" fn(this_: *mut c_void, path: *mut c_void),
    /// `SpecialGoal`.
    pub special_goal: unsafe extern "C" fn(this_: *mut c_void) -> *mut c_void,
    /// `SpecialGoal = NULL`.
    pub clear_special_goal: unsafe extern "C" fn(this_: *mut c_void),
    /// `Cast<UViewport>(Player)`, then `Viewport->Input->GetClass()`. Null when
    /// either is missing.
    pub keyboard_config_class: unsafe extern "C" fn(this_: *mut c_void) -> *mut c_void,
    /// `ResetConfig(klass)`.
    pub reset_config: unsafe extern "C" fn(klass: *mut c_void),
    /// `Player != NULL`.
    pub has_player: unsafe extern "C" fn(this_: *mut c_void) -> i32,
    /// `Player->Exec(command, StrOut)`; returns `StrOut`.
    pub player_exec: unsafe extern "C" fn(this_: *mut c_void, command: *const u16) -> *const u16,
    /// `GetLevel()->Engine->Exec(command, StrOut)`; returns `StrOut`.
    pub engine_exec: unsafe extern "C" fn(this_: *mut c_void, command: *const u16) -> *const u16,
    /// `Client->Viewports(0)->SizeX/SizeY`.
    pub viewport_size: unsafe extern "C" fn(this_: *mut c_void, size_x: *mut i32, size_y: *mut i32),
    /// `FovAngle`.
    pub pawn_fov: unsafe extern "C" fn(this_: *mut c_void) -> f32,
    /// `Client->MaintainVerticalFOV`.
    pub maintain_vertical_fov: unsafe extern "C" fn(this_: *mut c_void) -> i32,
    /// `UClient::GetEffectiveFovAngle(fov, size_x, size_y, maintain)`.
    pub effective_fov:
        unsafe extern "C" fn(fov: f32, size_x: i32, size_y: i32, maintain: i32) -> f32,
    /// `Location`.
    pub pawn_location: unsafe extern "C" fn(this_: *mut c_void, out: *mut c_void),
    /// `Rotation`.
    pub pawn_rotation: unsafe extern "C" fn(this_: *mut c_void, out: *mut c_void),
    /// `eventPlayerCalcView(ViewActor=this, loc, rot)`.
    pub calc_view: unsafe extern "C" fn(this_: *mut c_void, loc: *mut c_void, rot: *mut c_void),
    /// `GMath.SinTab[i]`.
    pub sin_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `GMath.CosTab[i]`.
    pub cos_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `AActor::execGetURLMap`: `Engine.LastURL.Map` as a NUL-terminated UNICHAR
    /// buffer. Valid until the next string-returning call.
    pub last_url_map: unsafe extern "C" fn(this_: *mut c_void) -> *const u16,
    /// `APlayerPawn::execGetEntryLevel`: the entry level info. The engine checks
    /// run inside the call.
    pub entry_level: unsafe extern "C" fn(this_: *mut c_void) -> *mut c_void,
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
) -> bool {
    let pkg = caps(pkg);
    // SAFETY: `server_packages` returns null or a NUL-terminated buffer.
    let raw = unsafe { (svc.server_packages)() };
    let found = if raw.is_null() {
        false
    } else {
        // SAFETY: `raw` is non-null and NUL-terminated (checked above).
        let list = unsafe { units(raw) };
        instr(&pkg, &list) != 0
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
    !proscribed
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
) {
    let mut text = option.to_vec();
    text.extend(lit("="));
    text.extend_from_slice(value);
    // SAFETY: the buffers are NUL-terminated and live across the call.
    unsafe { (svc.url_add_option)(this, nul(&text).as_ptr()) };
    if save {
        // SAFETY: as above.
        unsafe { (svc.url_save_default)(this, nul(option).as_ptr()) };
    }
}

/// `GetDefaultURL`: `GetOption(Option + "=", "")` on the saved default URL.
///
/// # Safety
/// The services must return valid NUL-terminated buffers.
pub unsafe fn get_default_url(svc: &Hp2PawnNativeFilesServices, option: &[u16]) -> Vec<u16> {
    let mut key = option.to_vec();
    key.extend(lit("="));
    // SAFETY: `key` is NUL-terminated for the call.
    unsafe { units((svc.url_default_option)(nul(&key).as_ptr())) }
}

/// `AActor::execGetURLMap`: the last URL's map name. The pointer comes from the
/// services and stays valid until the next string-returning call.
///
/// # Safety
/// `svc` must be a valid services table and `this` a live `AActor`.
pub unsafe fn url_map(svc: &Hp2PawnNativeFilesServices, this: *mut c_void) -> *const u16 {
    // SAFETY: the services return a buffer the caller copies before the next call.
    unsafe { (svc.last_url_map)(this) }
}

/// `APlayerPawn::execGetEntryLevel`: the entry level info, with the engine checks
/// run by the services.
///
/// # Safety
/// `svc` must be a valid services table and `this` a live `APlayerPawn`.
pub unsafe fn entry_level(svc: &Hp2PawnNativeFilesServices, this: *mut c_void) -> *mut c_void {
    // SAFETY: the services are called with the pawn that the native was invoked on.
    unsafe { (svc.entry_level)(this) }
}

/// `GetMapName`: searches `GSys->Paths` for maps, optionally under the CD path,
/// and returns the entry `dir` steps from `map_name`.
///
/// `find_files(pattern)` returns the matches for `pattern`.
#[allow(clippy::too_many_arguments)] // mirrors the GetMapName inputs one-to-one
pub fn pick_map_name(
    paths: &[Vec<u16>],
    cd_path: &[u16],
    map_ext: &[u16],
    path_sep: &[u16],
    prefix: &[u16],
    map_name: &[u16],
    dir: i32,
    mut find_files: impl FnMut(&[u16]) -> Vec<Vec<u16>>,
) -> Vec<u16> {
    let mut wildcard = lit("*.");
    wildcard.extend_from_slice(map_ext);
    let do_cd_count = 1 + usize::from(!cd_path.is_empty());
    let mut names: Vec<Vec<u16>> = Vec::new();
    for do_cd in 0..do_cd_count {
        for path in paths {
            if instr(path, &wildcard) == -1 {
                continue;
            }
            let mut tmp: Vec<u16> = Vec::new();
            if do_cd == 1 {
                tmp.extend_from_slice(cd_path);
                tmp.extend(lit("System"));
                tmp.extend_from_slice(path_sep);
            }
            tmp.extend_from_slice(path);
            // `*appStrstr(Tmp, Wildcard) = 0`: the first occurrence is always present.
            let cut = instr(&tmp, &wildcard) as usize;
            tmp.truncate(cut);
            tmp.extend_from_slice(prefix);
            tmp.extend_from_slice(&wildcard);
            for found in find_files(&tmp) {
                if !names.iter().any(|n| stricmp_eq(n, &found)) {
                    names.push(found);
                }
            }
        }
    }
    let separator = lit("\\");
    for name in names.iter_mut() {
        while instr(name, &separator) >= 0 {
            *name = after_last(name, b'\\' as u16);
        }
    }
    for (i, name) in names.iter().enumerate() {
        if name.as_slice() == map_name {
            let mut offset = i as i32 + dir;
            if offset < 0 {
                offset = names.len() as i32 - 1;
            } else if offset as usize >= names.len() {
                offset = 0;
            }
            return names[offset as usize].clone();
        }
    }
    names.first().cloned().unwrap_or_default()
}

/// `MoveCacheEntry(GUID, NewFilename)`: moves the cached file to the first
/// `GSys->Paths` entry with the same extension, then clears the cache entry.
/// Returns the oracle's `Result` (true when the move succeeded).
///
/// # Safety
/// The services must return valid NUL-terminated buffers.
pub unsafe fn move_cache_entry(
    svc: &Hp2PawnNativeFilesServices,
    guid: &[u16],
    new_filename: &[u16],
) -> bool {
    let guid_p = nul(guid);
    // SAFETY: `guid_p` is NUL-terminated for the call.
    let orig_p = unsafe { (svc.cache_get_string)(guid_p.as_ptr()) };
    if orig_p.is_null() {
        return false;
    }
    // SAFETY: a non-null result is NUL-terminated.
    let orig = unsafe { units(orig_p) };
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
        // SAFETY: `p` is NUL-terminated for the call.
        unsafe { (svc.log_no_extension)(p.as_ptr()) };
        return false;
    };
    let mut file_ext = lit(".");
    file_ext.extend_from_slice(&name[dot + 1..]);
    // SAFETY: the path services return NUL-terminated buffers.
    let (path_sep, cache_path, cache_ext, count) = unsafe {
        (
            units((svc.path_separator)()),
            units((svc.cache_path)()),
            units((svc.cache_ext)()),
            (svc.paths_count)(),
        )
    };
    for i in 0..count {
        // SAFETY: `i` is within `0..paths_count`.
        let path = unsafe { units((svc.path_at)(i)) };
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
        let mut src = cache_path.clone();
        src.extend_from_slice(&path_sep);
        src.extend_from_slice(guid);
        src.extend_from_slice(&cache_ext);
        let mut dest = dest_path.to_vec();
        dest.extend_from_slice(&name);
        let src_p = nul(&src);
        let dest_p = nul(&dest);
        // SAFETY: both buffers are NUL-terminated for the calls.
        let moved = unsafe {
            (svc.log_move)(src_p.as_ptr(), dest_p.as_ptr());
            (svc.file_move)(dest_p.as_ptr(), src_p.as_ptr()) != 0
        };
        if moved {
            // SAFETY: `guid_p` is NUL-terminated for the call.
            unsafe { (svc.cache_clear)(guid_p.as_ptr()) };
        }
        return moved;
    }
    false
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
) -> Vec<u16> {
    let cmd = nul(command);
    // SAFETY: `cmd` is NUL-terminated for the call.
    unsafe {
        if (svc.has_player)(this) != 0 {
            units((svc.player_exec)(this, cmd.as_ptr()))
        } else {
            units((svc.engine_exec)(this, cmd.as_ptr()))
        }
    }
}

/// `ResetKeyboard`: resets the viewport input class's config when one exists.
///
/// # Safety
/// `this` must be valid for the services.
pub unsafe fn reset_keyboard(svc: &Hp2PawnNativeFilesServices, this: *mut c_void) {
    // SAFETY: the service returns null or a live class pointer.
    unsafe {
        let klass = (svc.keyboard_config_class)(this);
        if !klass.is_null() {
            (svc.reset_config)(klass);
        }
    }
}

/// `FindBestInventoryPath` body after the clock. Returns `bestPath`, and writes
/// `*Weight = BestWeight` through `weight` when the search improved it.
///
/// # Safety
/// `this` and `weight` must be valid for the services and writes.
pub unsafe fn find_best_inventory_path(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
    weight: f32,
    predict: bool,
    weight_out: *mut f32,
) -> *mut c_void {
    let mut new_path: *mut c_void = std::ptr::null_mut();
    // SAFETY: the services write `new_path` and read `this`.
    let best_weight =
        unsafe { (svc.find_path_toward_best)(this, weight, i32::from(predict), &mut new_path) };
    let mut best_path: *mut c_void = std::ptr::null_mut();
    if best_weight > weight {
        best_path = new_path;
        // SAFETY: `weight_out` is the caller's `Weight` reference.
        unsafe {
            *weight_out = best_weight;
            (svc.reset_special_state)(this);
            if !best_path.is_null() && (svc.is_probing_special)(best_path) != 0 {
                (svc.handle_special)(this, best_path);
            }
            if best_path == (svc.special_goal)(this) {
                (svc.clear_special_goal)(this);
            }
        }
    }
    best_path
}

/// `ScreenToWorld(S)`: projects a screen point through the pawn's view.
///
/// # Safety
/// `this` and `out` must be valid for the services and writes.
pub unsafe fn screen_to_world(
    svc: &Hp2PawnNativeFilesServices,
    this: *mut c_void,
    s: FVector,
    out: *mut FVector,
) {
    let mut size_x = 0i32;
    let mut size_y = 0i32;
    // SAFETY: the services read the viewport and pawn fields.
    let effective = unsafe {
        (svc.viewport_size)(this, &mut size_x, &mut size_y);
        let fov = (svc.pawn_fov)(this);
        let maintain = (svc.maintain_vertical_fov)(this);
        (svc.effective_fov)(fov, size_x, size_y, maintain)
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
    let mut loc = prim::v3(0.0, 0.0, 0.0);
    let mut rot = FRotator {
        pitch: 0,
        yaw: 0,
        roll: 0,
    };
    // SAFETY: `calc_view` writes the view location and rotation.
    unsafe {
        (svc.pawn_location)(this, (&mut loc as *mut FVector).cast());
        (svc.pawn_rotation)(this, (&mut rot as *mut FRotator).cast());
        (svc.calc_view)(
            this,
            (&mut loc as *mut FVector).cast(),
            (&mut rot as *mut FRotator).cast(),
        );
    }
    let sin = |i: i32| unsafe { (svc.sin_tab)(i) };
    let cos = |i: i32| unsafe { (svc.cos_tab)(i) };
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

/// `GetMapName`: gathers `GSys->Paths`, the CD path and the map extension from
/// the services, then runs [`pick_map_name`] with `find_files` as the scanner.
///
/// # Safety
/// The services must return valid NUL-terminated buffers.
pub unsafe fn get_map_name(
    svc: &Hp2PawnNativeFilesServices,
    prefix: &[u16],
    map: &[u16],
    dir: i32,
) -> Vec<u16> {
    // SAFETY: the services return NUL-terminated buffers or counts.
    let (paths, cd_path, map_ext, path_sep) = unsafe {
        let count = (svc.paths_count)();
        let paths: Vec<Vec<u16>> = (0..count).map(|i| units((svc.path_at)(i))).collect();
        (
            paths,
            units((svc.cd_path)()),
            units((svc.map_ext)()),
            units((svc.path_separator)()),
        )
    };
    pick_map_name(
        &paths,
        &cd_path,
        &map_ext,
        &path_sep,
        prefix,
        map,
        dir,
        |pattern| {
            let pat = nul(pattern);
            // SAFETY: `pat` is NUL-terminated for the call.
            let n = unsafe { (svc.find_files)(pat.as_ptr()) };
            // SAFETY: `i` is within the count `find_files` returned.
            (0..n)
                .map(|i| unsafe { units((svc.found_file_at)(i)) })
                .collect()
        },
    )
}
