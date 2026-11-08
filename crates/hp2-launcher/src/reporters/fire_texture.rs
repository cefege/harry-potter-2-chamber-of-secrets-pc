//! Port of `FireTextureTraceReporter`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 5138-5381). A deliberately narrow oracle for one retail `UFireTexture`
//! export: it observes Fire's own RNG and Mip0 seams, gated by
//! `HP2_FIRE_TEXTURE_TRACE`.
//!
//! Unlike the other five reporters, `Probe()`'s `Lock`/`Update`/`Tick`/
//! `ConstantTimeTick`/`Unlock` calls are real `UFireTexture` virtual calls
//! that only C++ can make. The engine still owns that call sequence (see
//! `HP2FireTextureProbe` in `SDLLaunch.cpp`); this module owns every byte of
//! bookkeeping and serialization around it, bracketed by
//! `probe_begin`/`step_begin`/`step_end`/`probe_end`. `InitTables`,
//! `SpeedRand` and `BurnWrite` are still reached as reentrant hook callbacks
//! from inside `Update()`, exactly as in the original.
//!
//! Its engine-read leaf table ([`Hp2FireTextureServices`]) is separate from
//! [`super::trace_services`]'s and covers only a handful of
//! `UFireTexture`/`ULinkerLoad` fields; `export_path_ansi`/
//! `texture_class_path_ansi` are the two exceptions, composing
//! `super::trace_services`'s published `object_path`/`object_class` generics
//! (a `GetPathName()` walk and one `GetClass()` step) instead of duplicating
//! their own `UObject`/`UClass` reflection leaves.

use sha2::Digest;
use std::ffi::{c_char, c_void};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;

use super::trace_services::{self as svc, ansi, push_json_bool, push_json_string, write_report};

/// `FireTextureTraceReporter`'s own engine-read leaves. Every leaf is
/// null-safe for a null `UFireTexture*`, returning the same default the
/// ported ternary used (`-1`/`0`/`NULL`), so callers here never have to
/// branch on `target`/`texture` before calling one.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2FireTextureServices {
    /// `Texture ? (Texture->GetLinker() ? Text(*Texture->GetLinker()->Filename) : "") : ""`.
    pub linker_filename_ansi: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    /// `Texture ? Texture->GetLinkerIndex() : -1`.
    pub linker_index: unsafe extern "C" fn(*mut c_void) -> i32,
    /// `Texture ? Texture->USize : 0`.
    pub texture_u_size: unsafe extern "C" fn(*mut c_void) -> i32,
    /// `Texture ? Texture->VSize : 0`.
    pub texture_v_size: unsafe extern "C" fn(*mut c_void) -> i32,
    /// `Texture ? Texture->ActiveSparkNum : 0`.
    pub active_spark_count: unsafe extern "C" fn(*mut c_void) -> i32,
    /// `Texture->bRealtime` (only ever called with a live `Texture`).
    pub realtime: unsafe extern "C" fn(*mut c_void) -> i32,
    /// `Texture->bRealtimeChanged`.
    pub realtime_changed: unsafe extern "C" fn(*mut c_void) -> i32,
}

static mut SERVICES: Option<Hp2FireTextureServices> = None;

/// # Safety
/// `value` is the real, immutable leaf table; called once before any trace
/// reporter call.
pub unsafe fn publish_services(value: Hp2FireTextureServices) {
    unsafe { (&raw mut SERVICES).write(Some(value)) }
}

unsafe fn published_services() -> Result<Hp2FireTextureServices, i32> {
    unsafe { (&raw const SERVICES).read().ok_or(1) }
}

/// `Step` identifiers; `HP2FireTextureProbe` always runs them in this order.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Lock = 0,
    Update = 1,
    Tick = 2,
    ConstantTimeTick = 3,
}

impl StepKind {
    fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Lock),
            1 => Some(Self::Update),
            2 => Some(Self::Tick),
            3 => Some(Self::ConstantTimeTick),
            _ => None,
        }
    }
    fn name(self) -> &'static [u8] {
        match self {
            Self::Lock => b"lock",
            Self::Update => b"update",
            Self::Tick => b"tick",
            Self::ConstantTimeTick => b"constant_time_tick",
        }
    }
}

#[derive(Clone, Copy, Default)]
struct TextureState {
    realtime: bool,
    dirty: bool,
}

struct Step {
    name: &'static [u8],
    before: TextureState,
    after: TextureState,
    lock_reported_dirty: bool,
}

struct BurnWriteSample {
    spark_index: i32,
    offset: u32,
    before: u8,
    value: u8,
}

struct State {
    report_path: Vec<u8>,
    target: *mut c_void,
    active: *mut c_void,
    init_seen: bool,
    init_before: u64,
    init_after: u64,
    init_fingerprint: u64,
    first_speed_seen: bool,
    first_speed_index: u32,
    first_speed_value: u8,
    operation: Option<StepKind>,
    burn_writes: Vec<BurnWriteSample>,
    steps: Vec<Step>,
    step_before: TextureState,
}

static mut STATE: Option<State> = None;

fn env_bytes(name: &str) -> Option<Vec<u8>> {
    std::env::var_os(name).map(|v| v.as_bytes().to_vec())
}

/// `IsEnabled`.
pub fn is_enabled() -> bool {
    env_bytes("HP2_FIRE_TEXTURE_TRACE").is_some_and(|v| !v.is_empty())
}

/// # Safety
/// Single game thread; no concurrent reporter call.
pub unsafe fn create() {
    if !is_enabled() {
        return;
    }
    let state = State {
        report_path: env_bytes("HP2_FIRE_TEXTURE_TRACE").unwrap_or_default(),
        target: std::ptr::null_mut(),
        active: std::ptr::null_mut(),
        init_seen: false,
        init_before: 0,
        init_after: 0,
        init_fingerprint: 0,
        first_speed_seen: false,
        first_speed_index: 0,
        first_speed_value: 0,
        operation: None,
        burn_writes: Vec::new(),
        steps: Vec::new(),
        step_before: TextureState::default(),
    };
    unsafe { (&raw mut STATE).write(Some(state)) };
}

/// # Safety
/// Single game thread.
pub unsafe fn reset() {
    unsafe { (&raw mut STATE).write(None) };
}

/// `SetTarget`.
///
/// # Safety
/// Single game thread; `texture` null or a live `UFireTexture*`.
pub unsafe fn set_target(texture: *mut c_void) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        state.target = texture;
    }
}

/// `InitTables`.
///
/// # Safety
/// Single game thread (reentered from inside `Update()`/static init, same
/// thread as every other call).
pub unsafe fn init_tables(before: u64, after: u64, fingerprint: u64) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() }
        && !state.init_seen
    {
        state.init_seen = true;
        state.init_before = before;
        state.init_after = after;
        state.init_fingerprint = fingerprint;
    }
}

/// `SpeedRand`.
///
/// # Safety
/// Single game thread.
pub unsafe fn speed_rand(state_index: u32, value: u8) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() }
        && state.active == state.target
        && !state.first_speed_seen
    {
        state.first_speed_seen = true;
        state.first_speed_index = state_index;
        state.first_speed_value = value;
    }
}

/// `BurnWrite`.
///
/// # Safety
/// Single game thread; `texture` null or a live `UFireTexture*`.
pub unsafe fn burn_write(texture: *mut c_void, spark_index: i32, offset: u32, before: u8, value: u8) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() }
        && state.active == state.target
        && texture == state.target
        && state.operation == Some(StepKind::Update)
    {
        state.burn_writes.push(BurnWriteSample {
            spark_index,
            offset,
            before,
            value,
        });
    }
}

fn read_texture_state(services: &Hp2FireTextureServices, texture: *mut c_void) -> TextureState {
    TextureState {
        realtime: unsafe { (services.realtime)(texture) } != 0,
        dirty: unsafe { (services.realtime_changed)(texture) } != 0,
    }
}

/// `Texture ? Text(Texture->GetPathName()) : ""`, replacing the old
/// dedicated `path_ansi` leaf with the already-published generic
/// `object_path_ansi` one.
fn export_path_ansi(texture: *mut c_void) -> Vec<u8> {
    match unsafe { svc::published_services() } {
        Ok(services) => unsafe { svc::object_path(&services, texture) },
        Err(_) => Vec::new(),
    }
}

/// `Texture && Texture->GetClass() ? Text(Texture->GetClass()->GetPathName()) : ""`,
/// replacing the old dedicated `class_path_ansi` leaf by composing the
/// already-published generic `object_class`/`object_path_ansi` leaves.
fn texture_class_path_ansi(texture: *mut c_void) -> Vec<u8> {
    if texture.is_null() {
        return Vec::new();
    }
    match unsafe { svc::published_services() } {
        Ok(services) => {
            let class = unsafe { (services.object_class)(texture) };
            unsafe { svc::object_path(&services, class) }
        }
        Err(_) => Vec::new(),
    }
}

/// `Probe`'s `!Texture || Texture != Target` guard plus `SetActive(Texture)`.
/// Returns 0 (skip the whole probe) or 1 (continue; the caller runs
/// `Lock`/[`step_begin`]/[`step_end`]/.../`Unlock` itself).
///
/// # Safety
/// `texture` null or a live `UFireTexture*`.
pub unsafe fn probe_begin(texture: *mut c_void) -> i32 {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return 0;
    };
    if texture.is_null() || texture != state.target {
        return 0;
    }
    state.active = texture;
    1
}

/// `Before = State(Texture); Operation = <step>;`.
///
/// # Safety
/// Only valid between a successful [`probe_begin`] and the matching
/// [`probe_end`].
pub unsafe fn step_begin(step_kind: i32) {
    let Some(kind) = StepKind::from_i32(step_kind) else {
        return;
    };
    let Ok(services) = (unsafe { published_services() }) else {
        return;
    };
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        state.operation = Some(kind);
        state.step_before = read_texture_state(&services, state.target);
    }
}

/// `Steps.push_back(Step(<step>, Before, State(Texture), LockReportedDirty))`.
///
/// # Safety
/// Only valid after the matching [`step_begin`].
pub unsafe fn step_end(step_kind: i32, lock_reported_dirty: i32) {
    let Some(kind) = StepKind::from_i32(step_kind) else {
        return;
    };
    let Ok(services) = (unsafe { published_services() }) else {
        return;
    };
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        let after = read_texture_state(&services, state.target);
        state.steps.push(Step {
            name: kind.name(),
            before: state.step_before,
            after,
            lock_reported_dirty: lock_reported_dirty != 0,
        });
    }
}

/// `Operation.clear(); SetActive(NULL);` (the caller still runs
/// `Texture->Unlock(Info)` itself, immediately after this, exactly as the
/// original ran it after both of these).
///
/// # Safety
/// Single game thread.
pub unsafe fn probe_end() {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        state.operation = None;
        state.active = std::ptr::null_mut();
    }
}

/// `Sha256`: a file read failure (open or mid-read) returns empty, matching
/// the original's `return ""`. Bit-exact with CommonCrypto's `CC_SHA256_*`
/// used by the original `Sha256()`: both are FIPS 180-4 SHA-256.
fn sha256_hex(path: &[u8]) -> Vec<u8> {
    let Ok(mut file) = std::fs::File::open(std::ffi::OsStr::from_bytes(path)) else {
        return Vec::new();
    };
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 16384];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => hasher.update(&buffer[..count]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Vec::new(),
        }
    }
    let digest = hasher.finalize();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize]);
        out.push(HEX[(byte & 15) as usize]);
    }
    out
}

fn push_hex64(value: u64, out: &mut Vec<u8>) {
    out.extend_from_slice(format!("{value:016x}").as_bytes());
}

fn serialize(state: &State, services: &Hp2FireTextureServices) -> Vec<u8> {
    let target = state.target;
    let archive_path = unsafe { ansi((services.linker_filename_ansi)(target)) };
    let archive_sha = if archive_path.is_empty() {
        Vec::new()
    } else {
        sha256_hex(&archive_path)
    };
    let linker_index = unsafe { (services.linker_index)(target) };
    let export_path = export_path_ansi(target);
    let class_path = texture_class_path_ansi(target);
    let u_size = unsafe { (services.texture_u_size)(target) };
    let v_size = unsafe { (services.texture_v_size)(target) };
    let active_spark_count = unsafe { (services.active_spark_count)(target) };

    let mut out = Vec::new();
    out.extend_from_slice(b"{\"format\":\"hp2-fire-texture-trace\",\"version\":1,\"enabled\":true");
    out.extend_from_slice(b",\"target\":{\"archive\":{\"path\":");
    push_json_string(&archive_path, &mut out);
    out.extend_from_slice(b",\"sha256\":");
    push_json_string(&archive_sha, &mut out);
    out.extend_from_slice(b"},\"export\":{\"index\":");
    out.extend_from_slice(linker_index.to_string().as_bytes());
    out.extend_from_slice(b",\"path\":");
    push_json_string(&export_path, &mut out);
    out.extend_from_slice(b"},\"class\":");
    push_json_string(&class_path, &mut out);
    out.extend_from_slice(b",\"dimensions\":{\"u\":");
    out.extend_from_slice(u_size.to_string().as_bytes());
    out.extend_from_slice(b",\"v\":");
    out.extend_from_slice(v_size.to_string().as_bytes());
    out.extend_from_slice(b"},\"active_spark_count\":");
    out.extend_from_slice(active_spark_count.to_string().as_bytes());
    out.push(b'}');

    out.extend_from_slice(b",\"init_tables\":{\"rng\":{\"before\":");
    out.extend_from_slice(state.init_before.to_string().as_bytes());
    out.extend_from_slice(b",\"after\":");
    out.extend_from_slice(state.init_after.to_string().as_bytes());
    out.extend_from_slice(b"},\"fingerprint\":\"");
    push_hex64(state.init_fingerprint, &mut out);
    out.extend_from_slice(b"\"}");

    out.extend_from_slice(b",\"first_speed_rand\":");
    if state.first_speed_seen {
        out.extend_from_slice(b"{\"index\":");
        out.extend_from_slice(state.first_speed_index.to_string().as_bytes());
        out.extend_from_slice(b",\"value\":");
        out.extend_from_slice(state.first_speed_value.to_string().as_bytes());
        out.push(b'}');
    } else {
        out.extend_from_slice(b"null");
    }

    out.extend_from_slice(b",\"burn_mip0_writes\":[");
    for (index, sample) in state.burn_writes.iter().enumerate() {
        if index != 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"spark_index\":");
        out.extend_from_slice(sample.spark_index.to_string().as_bytes());
        out.extend_from_slice(b",\"offset\":");
        out.extend_from_slice(sample.offset.to_string().as_bytes());
        out.extend_from_slice(b",\"before\":");
        out.extend_from_slice(sample.before.to_string().as_bytes());
        out.extend_from_slice(b",\"value\":");
        out.extend_from_slice(sample.value.to_string().as_bytes());
        out.push(b'}');
    }
    out.extend_from_slice(b"],\"lifecycle\":[");
    for (index, step) in state.steps.iter().enumerate() {
        if index != 0 {
            out.push(b',');
        }
        // `Name` is concatenated raw in the original (always one of the four
        // fixed, safe literals), not run through `Escape`.
        out.extend_from_slice(b"{\"step\":\"");
        out.extend_from_slice(step.name);
        out.extend_from_slice(b"\",\"realtime\":{\"before\":");
        push_json_bool(step.before.realtime, &mut out);
        out.extend_from_slice(b",\"after\":");
        push_json_bool(step.after.realtime, &mut out);
        out.extend_from_slice(b"},\"dirty\":{\"before\":");
        push_json_bool(step.before.dirty, &mut out);
        out.extend_from_slice(b",\"after\":");
        push_json_bool(step.after.dirty, &mut out);
        out.extend_from_slice(b",\"lock_reported\":");
        push_json_bool(step.lock_reported_dirty, &mut out);
        out.extend_from_slice(b"}}");
    }
    out.extend_from_slice(b"]}\n");
    out
}

/// `Flush`.
///
/// # Safety
/// Single game thread.
pub unsafe fn flush() {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    let Ok(services) = (unsafe { published_services() }) else {
        return;
    };
    let document = serialize(state, &services);
    match write_report(&state.report_path, &document) {
        Ok(()) => {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(b"<HP2_FIRE_TEXTURE_TRACE> report=");
            let _ = out.write_all(&state.report_path);
            let _ = out.write_all(format!(" burn_writes={}\n", state.burn_writes.len()).as_bytes());
        }
        Err(kind) => {
            let reason: &[u8] = match kind {
                svc::ReportWriteError::Unwritable => b"unwritable",
                svc::ReportWriteError::WriteFailed => b"write_failed",
            };
            let mut err = std::io::stderr().lock();
            let _ = err.write_all(b"<HP2_FIRE_TEXTURE_TRACE> report_error=");
            let _ = err.write_all(reason);
            let _ = err.write_all(b" path=");
            let _ = err.write_all(&state.report_path);
            let _ = err.write_all(b"\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ground truth: `FireTextureTraceReporter::Hex64` (SDLLaunch.cpp,
    // deleted, formerly ~5127-5131): `snprintf(Buffer, sizeof(Buffer),
    // "%016llx", Value)` — zero-padded, lowercase, 16 hex digits.
    #[test]
    fn push_hex64_matches_cpp_hex64() {
        let mut out = Vec::new();
        push_hex64(0, &mut out);
        assert_eq!(out, b"0000000000000000");
        out.clear();
        push_hex64(0xDEAD_BEEF, &mut out);
        assert_eq!(out, b"00000000deadbeef");
        out.clear();
        push_hex64(u64::MAX, &mut out);
        assert_eq!(out, b"ffffffffffffffff");
    }

    // Ground truth: `FireTextureTraceReporter::Sha256` (SDLLaunch.cpp,
    // deleted, formerly ~5101-5125): a missing/unreadable file returns ""
    // (`!Input -> return ""`); SHA-256 of an empty file is the well-known
    // empty-input digest.
    #[test]
    fn sha256_hex_of_a_missing_file_is_empty() {
        let path = std::env::temp_dir().join(format!(
            "hp2_fire_texture_missing_{}_{}",
            std::process::id(),
            line!()
        ));
        let bytes = path.as_os_str().as_bytes().to_vec();
        assert!(sha256_hex(&bytes).is_empty());
    }

    #[test]
    fn sha256_hex_of_an_empty_file_is_the_well_known_empty_digest() {
        let path = std::env::temp_dir().join(format!(
            "hp2_fire_texture_empty_{}_{}",
            std::process::id(),
            line!()
        ));
        std::fs::write(&path, b"").unwrap();
        let bytes = path.as_os_str().as_bytes().to_vec();
        assert_eq!(
            sha256_hex(&bytes),
            b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_vec()
        );
        std::fs::remove_file(&path).unwrap();
    }

    unsafe extern "C" fn empty_ansi(_obj: *mut c_void) -> *const c_char {
        c"".as_ptr()
    }
    unsafe extern "C" fn fixed_index(_obj: *mut c_void) -> i32 {
        7
    }
    unsafe extern "C" fn fixed_u_size(_obj: *mut c_void) -> i32 {
        512
    }
    unsafe extern "C" fn fixed_v_size(_obj: *mut c_void) -> i32 {
        256
    }
    unsafe extern "C" fn fixed_spark_count(_obj: *mut c_void) -> i32 {
        3
    }
    unsafe extern "C" fn unused_bool(_obj: *mut c_void) -> i32 {
        0
    }

    fn fake_services() -> Hp2FireTextureServices {
        Hp2FireTextureServices {
            linker_filename_ansi: empty_ansi,
            linker_index: fixed_index,
            texture_u_size: fixed_u_size,
            texture_v_size: fixed_v_size,
            active_spark_count: fixed_spark_count,
            realtime: unused_bool,
            realtime_changed: unused_bool,
        }
    }

    // Ground truth: `FireTextureTraceReporter::Serialize` on a reporter that
    // has seen no probes yet (SDLLaunch.cpp, deleted, formerly ~5133-5160):
    // `ArchivePath.empty()` skips `Sha256` entirely (sha256 stays `""`,
    // quoted not `null` — this reporter never uses `JsonStringOrNull`);
    // `first_speed_rand` is `null` until `FirstSpeedSeen`.
    #[test]
    fn serialize_of_a_freshly_created_state_matches_cpp_serialize() {
        let state = State {
            report_path: Vec::new(),
            target: std::ptr::null_mut(),
            active: std::ptr::null_mut(),
            init_seen: false,
            init_before: 11,
            init_after: 22,
            init_fingerprint: 0xABCD,
            first_speed_seen: false,
            first_speed_index: 0,
            first_speed_value: 0,
            operation: None,
            burn_writes: Vec::new(),
            steps: Vec::new(),
            step_before: TextureState::default(),
        };
        let out = serialize(&state, &fake_services());
        assert_eq!(
            out,
            b"{\"format\":\"hp2-fire-texture-trace\",\"version\":1,\"enabled\":true,\"target\":{\"archive\":{\"path\":\"\",\"sha256\":\"\"},\"export\":{\"index\":7,\"path\":\"\"},\"class\":\"\",\"dimensions\":{\"u\":512,\"v\":256},\"active_spark_count\":3},\"init_tables\":{\"rng\":{\"before\":11,\"after\":22},\"fingerprint\":\"000000000000abcd\"},\"first_speed_rand\":null,\"burn_mip0_writes\":[],\"lifecycle\":[]}\n"
                .to_vec()
        );
    }

    // Ground truth: the `BurnWrites`/`Steps`/`FirstSpeedRand` loops
    // (SDLLaunch.cpp, deleted, formerly ~5143-5165); step names are
    // concatenated raw (one of the four fixed literals), never `Escape`d.
    #[test]
    fn serialize_with_burn_writes_speed_rand_and_lifecycle_steps() {
        let state = State {
            report_path: Vec::new(),
            target: std::ptr::null_mut(),
            active: std::ptr::null_mut(),
            init_seen: true,
            init_before: 0,
            init_after: 0,
            init_fingerprint: 0,
            first_speed_seen: true,
            first_speed_index: 5,
            first_speed_value: 9,
            operation: None,
            burn_writes: vec![BurnWriteSample {
                spark_index: 1,
                offset: 100,
                before: 10,
                value: 20,
            }],
            steps: vec![Step {
                name: StepKind::Lock.name(),
                before: TextureState {
                    realtime: false,
                    dirty: false,
                },
                after: TextureState {
                    realtime: true,
                    dirty: true,
                },
                lock_reported_dirty: true,
            }],
            step_before: TextureState::default(),
        };
        let out = serialize(&state, &fake_services());
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains(
            "\"first_speed_rand\":{\"index\":5,\"value\":9}"
        ));
        assert!(text.contains(
            "\"burn_mip0_writes\":[{\"spark_index\":1,\"offset\":100,\"before\":10,\"value\":20}]"
        ));
        assert!(text.contains(
            "\"lifecycle\":[{\"step\":\"lock\",\"realtime\":{\"before\":false,\"after\":true},\"dirty\":{\"before\":false,\"after\":true,\"lock_reported\":true}}]"
        ));
    }
}
