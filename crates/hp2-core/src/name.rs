//! Port of the executable bodies of `HarryPotter2/Unreal/Core/Src/UnName.cpp`:
//! `FName::Hardcode`, the `FName(const TCHAR*, EFindName)` constructor,
//! `StaticInit`, `StaticExit`, `DisplayHash`, `DeleteEntry`,
//! `operator<<(FArchive&, FNameEntry&)` and `AllocateNameEntry`. The C++ shells
//! forward through `hp2_rs_core_name_*` (`crates/hp2-ffi/src/core_name.rs`).
//!
//! [`NameStorage`] owns the canonical table, free-index stack, hash buckets and
//! initialization state. The legacy adapter retains its mutable compatibility
//! entries and service allocator. Direct [`crate::name_owner::NameOwner`] instances
//! copy immutable intrinsic specifications into one owned slab and own dynamic
//! entries with Rust allocations. Legacy service leaves capture exceptions and
//! return status 1; that adapter returns [`Thrown`] after preserving prior mutations.
//! Policy takes a raw storage pointer so no borrow spans reentrant native leaves.
//!
//! Build facts this port relies on (see `ansi.rs`): Core TUs are built with
//! `-DUNICODE=1` on macOS arm64, so `TCHAR` is a 4-byte `wchar_t`. `FromAnsi`
//! on that branch zero-extends the byte. `appStrihash` runs two CRC steps per
//! character, and `GCRCTable` is filled by `appInit` before any name call.

use std::cell::UnsafeCell;
use std::ffi::c_void;

use hp2_abi::engine::generated::{FName, FNameEntry};

use crate::ansi::Tchar;
use crate::crc::CRC_TABLE;

/// `NAME_SIZE` (UnName.h).
pub const NAME_SIZE: usize = 128;
/// Number of name hash buckets in the engine contract.
pub const NAME_HASH_BINS: usize = 4096;
/// `RF_Native` (UnObjBas.h).
pub const RF_NATIVE: u32 = 0x0400_0000;
/// `EFindName` (UnName.h).
pub const FNAME_FIND: i32 = 0;
pub const FNAME_ADD: i32 = 1;
pub const FNAME_INTRINSIC: i32 = 2;
pub const FNAME_DEFINE: i32 = 3;

const HARDCODED_COUNT: usize = 261;

/// Build the native aggregate's UTF-32, NUL-terminated name at compile time.
/// Indices start at zero, as in the old aggregate; `static_init` assigns their
/// sequential declaration positions on every initialization.
const fn intrinsic_name(name: &[u8], highlighted: bool) -> FNameEntry {
    assert!(name.len() < NAME_SIZE);
    let mut entry = FNameEntry {
        index: 0,
        flags: RF_NATIVE | if highlighted { 0x0000_0400 } else { 0 },
        hash_next: std::ptr::null_mut(),
        name: [0; NAME_SIZE],
    };
    let mut i = 0;
    while i < name.len() {
        assert!(name[i].is_ascii());
        entry.name[i] = name[i] as u32;
        i += 1;
    }
    entry
}

struct IntrinsicNames(UnsafeCell<[FNameEntry; HARDCODED_COUNT]>);

// SAFETY: all entry access is engine-thread serialized, just like NameStorage.
// Only raw pointers escape; no shared reference to an entry spans mutation or
// a reentrant service call. The static is never moved, reset, copied or freed.
unsafe impl Sync for IntrinsicNames {}

// Exact UnNames.h declaration order, NOT its historical numeric macro arguments.
// `true` is REG_NAME_HIGH (RF_HighlightedName); every entry also has RF_Native.
// The legacy mutable backing and direct owner slabs are both initialized from this catalog.
const INTRINSIC_SPECS: [FNameEntry; HARDCODED_COUNT] = [
    intrinsic_name(b"None", true),
    intrinsic_name(b"ByteProperty", false),
    intrinsic_name(b"IntProperty", false),
    intrinsic_name(b"BoolProperty", false),
    intrinsic_name(b"FloatProperty", false),
    intrinsic_name(b"ObjectProperty", false),
    intrinsic_name(b"NameProperty", false),
    intrinsic_name(b"StringProperty", false),
    intrinsic_name(b"ClassProperty", false),
    intrinsic_name(b"ArrayProperty", false),
    intrinsic_name(b"StructProperty", false),
    intrinsic_name(b"VectorProperty", false),
    intrinsic_name(b"RotatorProperty", false),
    intrinsic_name(b"StrProperty", false),
    intrinsic_name(b"MapProperty", false),
    intrinsic_name(b"FixedArrayProperty", false),
    intrinsic_name(b"Core", false),
    intrinsic_name(b"Engine", false),
    intrinsic_name(b"Editor", false),
    intrinsic_name(b"UnrealI", false),
    intrinsic_name(b"UnrealShare", false),
    intrinsic_name(b"Byte", true),
    intrinsic_name(b"Int", true),
    intrinsic_name(b"Bool", true),
    intrinsic_name(b"Float", true),
    intrinsic_name(b"Name", true),
    intrinsic_name(b"String", true),
    intrinsic_name(b"Struct", true),
    intrinsic_name(b"Vector", true),
    intrinsic_name(b"Rotator", true),
    intrinsic_name(b"Color", true),
    intrinsic_name(b"Plane", true),
    intrinsic_name(b"Begin", false),
    intrinsic_name(b"State", true),
    intrinsic_name(b"Function", true),
    intrinsic_name(b"Self", true),
    intrinsic_name(b"True", true),
    intrinsic_name(b"False", true),
    intrinsic_name(b"Transient", true),
    intrinsic_name(b"Enum", true),
    intrinsic_name(b"Replication", true),
    intrinsic_name(b"Reliable", true),
    intrinsic_name(b"Unreliable", true),
    intrinsic_name(b"Always", true),
    intrinsic_name(b"Field", false),
    intrinsic_name(b"Object", false),
    intrinsic_name(b"TextBuffer", false),
    intrinsic_name(b"Linker", false),
    intrinsic_name(b"LinkerLoad", false),
    intrinsic_name(b"LinkerSave", false),
    intrinsic_name(b"Subsystem", false),
    intrinsic_name(b"Factory", false),
    intrinsic_name(b"TextBufferFactory", false),
    intrinsic_name(b"Exporter", false),
    intrinsic_name(b"StackNode", false),
    intrinsic_name(b"Property", false),
    intrinsic_name(b"Camera", false),
    intrinsic_name(b"Vect", true),
    intrinsic_name(b"Rot", true),
    intrinsic_name(b"ArrayCount", true),
    intrinsic_name(b"EnumCount", true),
    intrinsic_name(b"Else", true),
    intrinsic_name(b"If", true),
    intrinsic_name(b"Goto", true),
    intrinsic_name(b"Stop", true),
    intrinsic_name(b"Until", true),
    intrinsic_name(b"While", true),
    intrinsic_name(b"Do", true),
    intrinsic_name(b"Break", true),
    intrinsic_name(b"For", true),
    intrinsic_name(b"ForEach", true),
    intrinsic_name(b"Assert", true),
    intrinsic_name(b"Switch", true),
    intrinsic_name(b"Case", true),
    intrinsic_name(b"Default", true),
    intrinsic_name(b"Continue", true),
    intrinsic_name(b"Private", true),
    intrinsic_name(b"Const", true),
    intrinsic_name(b"Out", true),
    intrinsic_name(b"Export", true),
    intrinsic_name(b"Skip", true),
    intrinsic_name(b"Coerce", true),
    intrinsic_name(b"Optional", true),
    intrinsic_name(b"Input", true),
    intrinsic_name(b"Config", true),
    intrinsic_name(b"Travel", true),
    intrinsic_name(b"EditConst", true),
    intrinsic_name(b"Localized", true),
    intrinsic_name(b"GlobalConfig", true),
    intrinsic_name(b"SafeReplace", true),
    intrinsic_name(b"New", true),
    intrinsic_name(b"Expands", true),
    intrinsic_name(b"Intrinsic", true),
    intrinsic_name(b"Within", true),
    intrinsic_name(b"Abstract", true),
    intrinsic_name(b"Package", true),
    intrinsic_name(b"Guid", true),
    intrinsic_name(b"Parent", true),
    intrinsic_name(b"Class", true),
    intrinsic_name(b"Extends", true),
    intrinsic_name(b"NoExport", true),
    intrinsic_name(b"NoUserCreate", true),
    intrinsic_name(b"PerObjectConfig", true),
    intrinsic_name(b"NativeReplication", true),
    intrinsic_name(b"Auto", true),
    intrinsic_name(b"Ignores", true),
    intrinsic_name(b"Global", true),
    intrinsic_name(b"Super", true),
    intrinsic_name(b"Outer", true),
    intrinsic_name(b"Operator", true),
    intrinsic_name(b"PreOperator", true),
    intrinsic_name(b"PostOperator", true),
    intrinsic_name(b"Final", true),
    intrinsic_name(b"Iterator", true),
    intrinsic_name(b"Latent", true),
    intrinsic_name(b"Return", true),
    intrinsic_name(b"Singular", true),
    intrinsic_name(b"Simulated", true),
    intrinsic_name(b"Exec", true),
    intrinsic_name(b"Event", true),
    intrinsic_name(b"Static", true),
    intrinsic_name(b"Native", true),
    intrinsic_name(b"Invariant", true),
    intrinsic_name(b"Var", true),
    intrinsic_name(b"Local", true),
    intrinsic_name(b"Import", true),
    intrinsic_name(b"From", true),
    intrinsic_name(b"Spawn", true),
    intrinsic_name(b"Array", true),
    intrinsic_name(b"Map", true),
    intrinsic_name(b"Tag", false),
    intrinsic_name(b"Role", false),
    intrinsic_name(b"RemoteRole", false),
    intrinsic_name(b"System", false),
    intrinsic_name(b"User", false),
    intrinsic_name(b"Log", false),
    intrinsic_name(b"Critical", false),
    intrinsic_name(b"Init", false),
    intrinsic_name(b"Exit", false),
    intrinsic_name(b"Cmd", false),
    intrinsic_name(b"Play", false),
    intrinsic_name(b"Console", false),
    intrinsic_name(b"Warning", false),
    intrinsic_name(b"ExecWarning", false),
    intrinsic_name(b"ScriptWarning", false),
    intrinsic_name(b"ScriptLog", false),
    intrinsic_name(b"Dev", false),
    intrinsic_name(b"DevNet", false),
    intrinsic_name(b"DevPath", false),
    intrinsic_name(b"DevNetTraffic", false),
    intrinsic_name(b"DevAudio", false),
    intrinsic_name(b"DevLoad", false),
    intrinsic_name(b"DevSave", false),
    intrinsic_name(b"DevGarbage", false),
    intrinsic_name(b"DevKill", false),
    intrinsic_name(b"DevReplace", false),
    intrinsic_name(b"DevMusic", false),
    intrinsic_name(b"DevSound", false),
    intrinsic_name(b"DevCompile", false),
    intrinsic_name(b"DevBind", false),
    intrinsic_name(b"Localization", false),
    intrinsic_name(b"Compatibility", false),
    intrinsic_name(b"NetComeGo", false),
    intrinsic_name(b"Title", false),
    intrinsic_name(b"Error", false),
    intrinsic_name(b"Heading", false),
    intrinsic_name(b"SubHeading", false),
    intrinsic_name(b"FriendlyError", false),
    intrinsic_name(b"Progress", false),
    intrinsic_name(b"UserPrompt", false),
    intrinsic_name(b"White", false),
    intrinsic_name(b"Black", false),
    intrinsic_name(b"Red", false),
    intrinsic_name(b"Green", false),
    intrinsic_name(b"Blue", false),
    intrinsic_name(b"Cyan", false),
    intrinsic_name(b"Magenta", false),
    intrinsic_name(b"Yellow", false),
    intrinsic_name(b"DefaultColor", false),
    intrinsic_name(b"KeyType", false),
    intrinsic_name(b"KeyEvent", false),
    intrinsic_name(b"Write", false),
    intrinsic_name(b"Message", false),
    intrinsic_name(b"InitialState", false),
    intrinsic_name(b"Texture", false),
    intrinsic_name(b"Sound", false),
    intrinsic_name(b"FireTexture", false),
    intrinsic_name(b"IceTexture", false),
    intrinsic_name(b"WaterTexture", false),
    intrinsic_name(b"WaveTexture", false),
    intrinsic_name(b"WetTexture", false),
    intrinsic_name(b"Main", false),
    intrinsic_name(b"NotifyLevelChange", false),
    intrinsic_name(b"VideoChange", false),
    intrinsic_name(b"SendText", false),
    intrinsic_name(b"SendBinary", false),
    intrinsic_name(b"ConnectFailure", false),
    intrinsic_name(b"Spawned", false),
    intrinsic_name(b"Destroyed", false),
    intrinsic_name(b"GainedChild", false),
    intrinsic_name(b"LostChild", false),
    intrinsic_name(b"Probe4", false),
    intrinsic_name(b"Probe5", false),
    intrinsic_name(b"Trigger", false),
    intrinsic_name(b"UnTrigger", false),
    intrinsic_name(b"Timer", false),
    intrinsic_name(b"HitWall", false),
    intrinsic_name(b"Falling", false),
    intrinsic_name(b"Landed", false),
    intrinsic_name(b"ZoneChange", false),
    intrinsic_name(b"Touch", false),
    intrinsic_name(b"UnTouch", false),
    intrinsic_name(b"Bump", false),
    intrinsic_name(b"BeginState", false),
    intrinsic_name(b"EndState", false),
    intrinsic_name(b"BaseChange", false),
    intrinsic_name(b"Attach", false),
    intrinsic_name(b"Detach", false),
    intrinsic_name(b"ActorEntered", false),
    intrinsic_name(b"ActorLeaving", false),
    intrinsic_name(b"KillCredit", false),
    intrinsic_name(b"AnimEnd", false),
    intrinsic_name(b"EndedRotation", false),
    intrinsic_name(b"InterpolateEnd", false),
    intrinsic_name(b"EncroachingOn", false),
    intrinsic_name(b"EncroachedBy", false),
    intrinsic_name(b"FootZoneChange", false),
    intrinsic_name(b"HeadZoneChange", false),
    intrinsic_name(b"PainTimer", false),
    intrinsic_name(b"SpeechTimer", false),
    intrinsic_name(b"MayFall", false),
    intrinsic_name(b"Probe34", false),
    intrinsic_name(b"Die", false),
    intrinsic_name(b"Tick", false),
    intrinsic_name(b"PlayerTick", false),
    intrinsic_name(b"Expired", false),
    intrinsic_name(b"Probe39", false),
    intrinsic_name(b"SeePlayer", false),
    intrinsic_name(b"EnemyNotVisible", false),
    intrinsic_name(b"HearNoise", false),
    intrinsic_name(b"UpdateEyeHeight", false),
    intrinsic_name(b"SeeMonster", false),
    intrinsic_name(b"SeeFriend", false),
    intrinsic_name(b"SpecialHandling", false),
    intrinsic_name(b"BotDesireability", false),
    intrinsic_name(b"Probe48", false),
    intrinsic_name(b"Probe49", false),
    intrinsic_name(b"Probe50", false),
    intrinsic_name(b"Probe51", false),
    intrinsic_name(b"Probe52", false),
    intrinsic_name(b"Probe53", false),
    intrinsic_name(b"Probe54", false),
    intrinsic_name(b"Probe55", false),
    intrinsic_name(b"Probe56", false),
    intrinsic_name(b"Probe57", false),
    intrinsic_name(b"Probe58", false),
    intrinsic_name(b"Probe59", false),
    intrinsic_name(b"Probe60", false),
    intrinsic_name(b"Probe61", false),
    intrinsic_name(b"Probe62", false),
    intrinsic_name(b"All", false),
];
static INTRINSIC_NAMES: IntrinsicNames =
    IntrinsicNames(UnsafeCell::new(INTRINSIC_SPECS));

/// Number of intrinsic entries, in the sequential `EName` declaration order.
pub const fn hardcoded_count() -> i32 {
    HARDCODED_COUNT as i32
}
/// Copies an immutable intrinsic name specification for a Rust-owned table.
pub(crate) fn intrinsic_spec(index: usize) -> Option<FNameEntry> {
    INTRINSIC_SPECS.get(index).copied()
}


/// Stable intrinsic entry storage, available even before `static_init` and after
/// `static_exit`. Initialization rebuilds indices/hash links, not names or flags.
///
/// # Safety
/// `index` must be in `0..hardcoded_count()`. Entry access must be engine-thread
/// serialized with all name operations. Do not hold Rust references to entries
/// across mutations or reentrant callbacks; use the returned raw pointer.
pub unsafe fn hardcoded_entry(index: i32) -> *mut FNameEntry {
    unsafe {
        INTRINSIC_NAMES
            .0
            .get()
            .cast::<FNameEntry>()
            .add(index as usize)
    }
}

/// A services call reported a C++ throw (status 1). The C++ side has captured
/// the exception and rethrows it after the export returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrown;

/// Result of a name operation. `Err(Thrown)` means a C++ throw is pending.
pub type NameResult<T = ()> = Result<T, Thrown>;

fn status(code: i32) -> NameResult {
    if code == 0 { Ok(()) } else { Err(Thrown) }
}

/// Per-instance owner of the name subsystem. Native FFI supplies its singleton;
/// contracts can supply independent instances without sharing mutable global state.
///
/// Table growth moves pointer slots, never entries. Explicit oracle resets retain
/// inactive slot bits, matching legacy uninitialized `Names.Add()` on reused backing.
/// Only `names_len` slots are live; `StaticExit` releases both owned vectors.
pub struct NameStorage {
    pub(crate) names: Vec<*mut FNameEntry>,
    pub(crate) names_len: usize,
    pub(crate) available: Vec<i32>,
    pub(crate) bins: [*mut FNameEntry; NAME_HASH_BINS],
    pub(crate) initialized: bool,
}

impl Default for NameStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl NameStorage {
    pub const fn new() -> Self {
        Self {
            names: Vec::new(),
            names_len: 0,
            available: Vec::new(),
            bins: [std::ptr::null_mut(); NAME_HASH_BINS],
            initialized: false,
        }
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn len(&self) -> i32 {
        self.names_len as i32
    }

    pub fn is_empty(&self) -> bool {
        self.names_len == 0
    }

    pub fn entry(&self, index: i32) -> *mut FNameEntry {
        assert!(index >= 0 && (index as usize) < self.names_len);
        self.names[index as usize]
    }

    pub fn is_valid(&self, index: i32) -> bool {
        index == 0 || (index > 0 && index < self.len() && !self.entry(index).is_null())
    }

    pub fn entries(&self) -> &[*mut FNameEntry] {
        &self.names[..self.names_len]
    }

    pub fn available(&self) -> &[i32] {
        &self.available
    }

    pub fn bins(&self) -> &[*mut FNameEntry; NAME_HASH_BINS] {
        &self.bins
    }

    /// Reset the table without freeing entries, as the oracle's explicit reset does.
    ///
    /// # Safety
    /// The caller must retain or release the old entries separately and must not
    /// use old handles until the table has been initialized again.
    pub unsafe fn reset(&mut self) {
        self.names_len = 0;
        self.available.clear();
        self.bins.fill(std::ptr::null_mut());
        self.initialized = false;
    }

    pub(crate) fn add_slot(&mut self) -> i32 {
        let index = self.names_len;
        assert!(index < i32::MAX as usize);
        if index == self.names.len() {
            self.names.push(std::ptr::null_mut());
        }
        self.names_len += 1;
        index as i32
    }

    pub(crate) fn set_entry(&mut self, index: i32, entry: *mut FNameEntry) {
        assert!(index >= 0 && (index as usize) < self.names_len);
        self.names[index as usize] = entry;
    }

    /// Raw stable bucket storage for policy and corruption-replay contracts.
    ///
    /// # Safety
    /// `storage` must remain live and unmoved while the returned pointer is used.
    /// Access must be serialized; callers must not retain a storage borrow across
    /// callbacks which can access this same instance.
    pub unsafe fn bins_ptr(storage: *mut Self) -> *mut *mut FNameEntry {
        unsafe { (&raw mut (*storage).bins).cast() }
    }
}

/// Searches one name-hash chain using a caller-supplied case-insensitive matcher.
///
/// # Safety
/// `head` and every linked entry must remain live throughout the search. The matcher
/// must not free a linked entry before the search advances to its next link.
pub(crate) unsafe fn find_entry<E>(
    mut entry: *mut FNameEntry,
    mut matches: impl FnMut(*mut FNameEntry) -> Result<bool, E>,
) -> Result<Option<*mut FNameEntry>, E> {
    while !entry.is_null() {
        if matches(entry)? {
            return Ok(Some(entry));
        }
        entry = unsafe { (*entry).hash_next.cast() };
    }
    Ok(None)
}


/// Allocates and publishes a name entry using the canonical bucket and slot table.
///
/// The bucket head is captured before allocation, matching legacy allocator reentry.
///
/// # Safety
/// `storage` must be live and engine-thread serialized. The allocation closure must
/// return one live entry or an error, and must not free the captured bucket chain.
pub(crate) unsafe fn commit_entry<E>(
    storage: *mut NameStorage,
    index: i32,
    hash: usize,
    allocate: impl FnOnce(*mut FNameEntry) -> Result<*mut FNameEntry, E>,
) -> Result<*mut FNameEntry, E> {
    let bins = unsafe { NameStorage::bins_ptr(storage) };
    let previous = unsafe { *bins.add(hash) };
    let entry = allocate(previous)?;
    unsafe {
        *bins.add(hash) = entry;
        (*storage).set_entry(index, entry);
    }
    Ok(entry)
}
/// C++ leaf services the name port needs. Filled in `Core/Src/Hp2NameServices.cpp`.
/// Field order is the C++ table order. Each function returns 0 on completion and
/// 1 when a C++ exception was captured. Output parameters are written only on
/// status 0.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2NameServices {
    /// `appFailAssert(Expr, File, Line)`, `File` being the original `UnName.cpp` `__FILE__`.
    pub check_fail: unsafe extern "C" fn(expr: *const c_void, line: i32) -> i32,
    /// `appErrorf("FName constructed before FName::StaticInit")`.
    pub err_ctor_before_init: unsafe extern "C" fn() -> i32,
    /// `appErrorf("Hardcoded name %i was duplicated", Index)`.
    pub err_hardcode_duplicate: unsafe extern "C" fn(index: i32) -> i32,
    /// `appErrorf("Name '%s' was duplicated", Name)`.
    pub err_name_duplicated: unsafe extern "C" fn(name: *const c_void) -> i32,
    /// `appErrorf("Unhashed name '%s'", Name)`.
    pub err_unhashed: unsafe extern "C" fn(name: *const c_void) -> i32,
    /// `appErrorf("Allocation size exceeds the 32-bit allocator contract")`.
    pub err_alloc_size: unsafe extern "C" fn() -> i32,
    /// `debugf(NAME_Init, "Name subsystem initialized")`.
    pub debugf_init: unsafe extern "C" fn() -> i32,
    /// `debugf(NAME_Exit, "Name subsystem shut down")`.
    pub debugf_exit: unsafe extern "C" fn() -> i32,
    /// `Ar.Logf("Hash: %i names, %i/%i hash bins", ...)`.
    pub display_hash:
        unsafe extern "C" fn(ar: *mut c_void, names: i32, used_bins: i32, bins: i32) -> i32,
    /// Count of the Rust-owned intrinsic catalog.
    pub hardcoded_count: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// Raw pointer into the same Rust-owned intrinsic catalog.
    pub hardcoded_entry: unsafe extern "C" fn(index: i32, out: *mut *mut c_void) -> i32,
    /// `appStrlen(S)`.
    pub str_len: unsafe extern "C" fn(s: *const c_void, out: *mut i32) -> i32,
    /// `appStricmp(A, B)`.
    pub str_icmp: unsafe extern "C" fn(a: *const c_void, b: *const c_void, out: *mut i32) -> i32,
    /// `appStrcmp(A, B)`.
    pub str_cmp: unsafe extern "C" fn(a: *const c_void, b: *const c_void, out: *mut i32) -> i32,
    /// `appStrcpy(Dst, Src)`.
    pub str_cpy: unsafe extern "C" fn(dst: *mut c_void, src: *const c_void) -> i32,
    /// `GCRCTable` (256 entries).
    pub crc_table: unsafe extern "C" fn(out: *mut *const c_void) -> i32,
    /// `appMalloc(Size, TEXT("NameEntry"))`.
    pub alloc: unsafe extern "C" fn(size: u32, out: *mut *mut c_void) -> i32,
    /// `delete` of an `FNameEntry` allocated by `alloc`.
    pub free_entry: unsafe extern "C" fn(entry: *mut c_void) -> i32,
    /// `Ar.Ver()`.
    pub ar_ver: unsafe extern "C" fn(ar: *mut c_void, out: *mut i32) -> i32,
    /// `Ar.IsLoading()`.
    pub ar_is_loading: unsafe extern "C" fn(ar: *mut c_void, out: *mut i32) -> i32,
    /// `BYTE B; Ar << B;`.
    pub ar_read_byte: unsafe extern "C" fn(ar: *mut c_void, out: *mut u8) -> i32,
    /// `FString Str; Ar << Str; appStrcpy(Dst, *Str);`.
    pub ar_load_string: unsafe extern "C" fn(ar: *mut c_void, dst: *mut c_void) -> i32,
    /// `FString Str(Src); Ar << Str;`.
    pub ar_save_string: unsafe extern "C" fn(ar: *mut c_void, src: *const c_void) -> i32,
    /// `Ar << E.Flags` (DWORD).
    pub ar_serialize_flags: unsafe extern "C" fn(ar: *mut c_void, flags: *mut u32) -> i32,
}

const _: () = assert!(size_of::<Hp2NameServices>() == 24 * size_of::<usize>());

/// A call-scoped copy of the actual canonical provider and its live owner.
/// No reference to the mutable table is held across allocator/string/log leaves.
#[derive(Clone, Copy)]
pub struct PublishedNames {
    pub storage: *mut NameStorage,
    pub services: Hp2NameServices,
}

impl PublishedNames {
    /// # Safety
    /// The owner and index must be live and engine-thread serialized.
    pub unsafe fn text(self, index: i32) -> *const Tchar {
        unsafe { name_ptr((*self.storage).entry(index)) }
    }

    /// # Safety
    /// The owner/provider must remain live; text is a terminated TCHAR string.
    pub unsafe fn find(self, text: *const Tchar) -> Result<i32, i32> {
        let mut value = FName { index: 0 };
        unsafe {
            ctor(
                &self.services,
                self.storage,
                &raw mut value,
                text,
                FNAME_FIND,
            )
        }
        .map_err(|_| 1)?;
        Ok(value.index)
    }
}

impl Hp2NameServices {
    /// The check leaf. Always throws on the C++ side. `expr` is NUL-terminated.
    pub fn check_fail(&self, expr: &'static [u8], line: i32) -> NameResult {
        debug_assert_eq!(expr.last(), Some(&0));
        // SAFETY: the table is filled by the C++ side and outlives the call. `expr` is NUL-terminated.
        status(unsafe { (self.check_fail)(expr.as_ptr().cast(), line) })
    }

    pub fn err_ctor_before_init(&self) -> NameResult {
        // SAFETY: C++ services table; always throws.
        status(unsafe { (self.err_ctor_before_init)() })
    }

    pub fn err_hardcode_duplicate(&self, index: i32) -> NameResult {
        // SAFETY: C++ services table; always throws.
        status(unsafe { (self.err_hardcode_duplicate)(index) })
    }

    pub fn err_name_duplicated(&self, name: *const Tchar) -> NameResult {
        // SAFETY: C++ services table; always throws.
        status(unsafe { (self.err_name_duplicated)(name.cast()) })
    }

    pub fn err_unhashed(&self, name: *const Tchar) -> NameResult {
        // SAFETY: C++ services table; always throws.
        status(unsafe { (self.err_unhashed)(name.cast()) })
    }

    pub fn err_alloc_size(&self) -> NameResult {
        // SAFETY: C++ services table; always throws.
        status(unsafe { (self.err_alloc_size)() })
    }

    pub fn debugf_init(&self) -> NameResult {
        // SAFETY: C++ services table.
        status(unsafe { (self.debugf_init)() })
    }

    pub fn debugf_exit(&self) -> NameResult {
        // SAFETY: C++ services table.
        status(unsafe { (self.debugf_exit)() })
    }

    /// # Safety
    /// `ar` must be a live `FOutputDevice`.
    pub unsafe fn display_hash(
        &self,
        ar: *mut c_void,
        names: i32,
        used_bins: i32,
        bins: i32,
    ) -> NameResult {
        // SAFETY: `ar` is the caller's `FArchive&`, valid for the call.
        status(unsafe { (self.display_hash)(ar, names, used_bins, bins) })
    }

    pub fn hardcoded_count(&self) -> NameResult<i32> {
        let mut out = 0;
        // SAFETY: `out` is a valid out-parameter.
        status(unsafe { (self.hardcoded_count)(&raw mut out) })?;
        Ok(out)
    }

    pub fn hardcoded_entry(&self, index: i32) -> NameResult<*mut FNameEntry> {
        let mut out = std::ptr::null_mut();
        // SAFETY: `out` is a valid out-parameter.
        status(unsafe { (self.hardcoded_entry)(index, &raw mut out) })?;
        Ok(out.cast())
    }

    pub fn str_len(&self, s: *const Tchar) -> NameResult<i32> {
        let mut out = 0;
        // SAFETY: `s` is a NUL-terminated string (caller contract); `out` is valid.
        status(unsafe { (self.str_len)(s.cast(), &raw mut out) })?;
        Ok(out)
    }

    pub fn str_icmp(&self, a: *const Tchar, b: *const Tchar) -> NameResult<i32> {
        let mut out = 0;
        // SAFETY: both are NUL-terminated strings (caller contract); `out` is valid.
        status(unsafe { (self.str_icmp)(a.cast(), b.cast(), &raw mut out) })?;
        Ok(out)
    }

    pub fn str_cmp(&self, a: *const Tchar, b: *const Tchar) -> NameResult<i32> {
        let mut out = 0;
        // SAFETY: both are NUL-terminated strings (caller contract); `out` is valid.
        status(unsafe { (self.str_cmp)(a.cast(), b.cast(), &raw mut out) })?;
        Ok(out)
    }

    pub fn str_cpy(&self, dst: *mut Tchar, src: *const Tchar) -> NameResult {
        // SAFETY: `dst` has room for `src` (caller contract).
        status(unsafe { (self.str_cpy)(dst.cast(), src.cast()) })
    }

    pub fn crc_table(&self) -> NameResult<*const u32> {
        let mut out = std::ptr::null();
        // SAFETY: `out` is a valid out-parameter.
        status(unsafe { (self.crc_table)(&raw mut out) })?;
        Ok(out.cast())
    }

    pub fn alloc(&self, size: u32) -> NameResult<*mut FNameEntry> {
        let mut out = std::ptr::null_mut();
        // SAFETY: `out` is a valid out-parameter.
        status(unsafe { (self.alloc)(size, &raw mut out) })?;
        Ok(out.cast())
    }

    /// # Safety
    /// `entry` must be a live `FNameEntry` from `alloc` that has not been freed.
    pub unsafe fn free_entry(&self, entry: *mut FNameEntry) -> NameResult {
        // SAFETY: `entry` came from `alloc` (caller contract).
        status(unsafe { (self.free_entry)(entry.cast()) })
    }

    /// # Safety
    /// `ar` must be a live `FArchive`.
    pub unsafe fn ar_ver(&self, ar: *mut c_void) -> NameResult<i32> {
        let mut out = 0;
        // SAFETY: `ar` is the caller's `FArchive&`; `out` is valid.
        status(unsafe { (self.ar_ver)(ar, &raw mut out) })?;
        Ok(out)
    }

    /// # Safety
    /// `ar` must be a live `FArchive`.
    pub unsafe fn ar_is_loading(&self, ar: *mut c_void) -> NameResult<i32> {
        let mut out = 0;
        // SAFETY: `ar` is the caller's `FArchive&`; `out` is valid.
        status(unsafe { (self.ar_is_loading)(ar, &raw mut out) })?;
        Ok(out)
    }

    /// # Safety
    /// `ar` must be a live `FArchive`.
    pub unsafe fn ar_read_byte(&self, ar: *mut c_void) -> NameResult<u8> {
        let mut out = 0;
        // SAFETY: `ar` is the caller's `FArchive&`; `out` is valid.
        status(unsafe { (self.ar_read_byte)(ar, &raw mut out) })?;
        Ok(out)
    }

    /// # Safety
    /// `ar` must be a live `FArchive`; `dst` must have room for the loaded string.
    pub unsafe fn ar_load_string(&self, ar: *mut c_void, dst: *mut Tchar) -> NameResult {
        // SAFETY: `ar` is the caller's `FArchive&`; `dst` has room for the string (caller contract).
        status(unsafe { (self.ar_load_string)(ar, dst.cast()) })
    }

    /// # Safety
    /// `ar` must be a live `FArchive`; `src` must be NUL-terminated.
    pub unsafe fn ar_save_string(&self, ar: *mut c_void, src: *const Tchar) -> NameResult {
        // SAFETY: `ar` is the caller's `FArchive&`; `src` is NUL-terminated.
        status(unsafe { (self.ar_save_string)(ar, src.cast()) })
    }

    /// # Safety
    /// `ar` must be a live `FArchive`; `flags` must point to a live `u32`.
    pub unsafe fn ar_serialize_flags(&self, ar: *mut c_void, flags: *mut u32) -> NameResult {
        // SAFETY: `ar` is the caller's `FArchive&`; `flags` points to a live DWORD.
        status(unsafe { (self.ar_serialize_flags)(ar, flags) })
    }
}

/// `FName::Hardcode(FNameEntry* AutoName)`.
///
/// # Safety
/// `svc` must be the live services table. `auto_name` must point to a live `FNameEntry`.
/// `storage` must be live, unmoved and accessed serially, including by reentrant leaves.
pub unsafe fn hardcode(
    svc: &Hp2NameServices,
    storage: *mut NameStorage,
    auto_name: *mut FNameEntry,
) -> NameResult {
    unsafe {
        let crc = svc.crc_table()?;
        let i_hash = (strihash(crc, name_ptr(auto_name)) as usize) & (NAME_HASH_BINS - 1);
        let bins = NameStorage::bins_ptr(storage);
        (*auto_name).hash_next = (*bins.add(i_hash)).cast();
        *bins.add(i_hash) = auto_name;

        let target = (*auto_name).index;
        while (*storage).len() <= target {
            let index = (*storage).add_slot();
            (*storage).set_entry(index, std::ptr::null_mut());
        }
        if !(*storage).entry(target).is_null() {
            svc.err_hardcode_duplicate(target)?;
        }
        (*storage).set_entry(target, auto_name);
        Ok(())
    }
}

/// `FName::FName(const TCHAR* Name, EFindName FindType)`, applied to the object at `this`.
///
/// # Safety
/// `svc` must be the live services table. `this` must point to a live `FName`.
/// `name` must be NUL-terminated (a NULL `name` is checked, then read as the C++ does).
/// `storage` must be live, unmoved and accessed serially, including by reentrant leaves.
pub unsafe fn ctor(
    svc: &Hp2NameServices,
    storage: *mut NameStorage,
    this: *mut FName,
    name: *const Tchar,
    find_type: i32,
) -> NameResult {
    unsafe {
        if name.is_null() {
            svc.check_fail(b"Name\0", 55)?;
        }
        if !(*storage).initialized {
            svc.err_ctor_before_init()?;
        }
        if *name == 0 {
            (*this).index = 0;
            return Ok(());
        }

        let crc = svc.crc_table()?;
        let i_hash = (strihash(crc, name) as usize) & (NAME_HASH_BINS - 1);
        let bins = NameStorage::bins_ptr(storage);

        let found = find_entry(*bins.add(i_hash), |entry| {
            svc.str_icmp(name, name_ptr(entry)).map(|comparison| comparison == 0)
        })?;
        if let Some(hash) = found {
            (*this).index = (*hash).index;
            if find_type == FNAME_DEFINE && svc.str_cmp(name, name_ptr(hash))? < 0 {
                // Conflicting defines. Uppercase wins.
                svc.str_cpy(name_ptr(hash), name)?;
            }
            return Ok(());
        }

        if find_type == FNAME_FIND {
            (*this).index = 0;
            return Ok(());
        }

        // Reuse the most recently freed index, else grow `Names`.
        let new_index = match (*storage).available.pop() {
            Some(index) => index,
            None => (*storage).add_slot(),
        };
        // The C++ stores the index in the object before allocating the entry.
        (*this).index = new_index;

        let entry = commit_entry(storage, new_index, i_hash, |head| {
            allocate_entry(svc, name, new_index as u32, 0, head)
        })?;
        if find_type == FNAME_INTRINSIC {
            (*entry).flags |= RF_NATIVE;
        }
        Ok(())
    }
}

/// `FName::StaticInit()`.
///
/// # Safety
/// `svc` must be the live services table.
/// `storage` must be live, unmoved and accessed serially, including by reentrant leaves.
pub unsafe fn static_init(svc: &Hp2NameServices, storage: *mut NameStorage) -> NameResult {
    unsafe {
        if (*storage).initialized {
            svc.check_fail(b"Initialized==0\0", 135)?;
        }
        // `check((ARRAY_COUNT(NameHash)&(ARRAY_COUNT(NameHash)-1)) == 0)` is always true for
        // the 4096-bin table; the compile-time assert below pins that.
        const _: () = assert!(NAME_HASH_BINS & (NAME_HASH_BINS - 1) == 0);

        let bins = NameStorage::bins_ptr(storage);
        for i in 0..NAME_HASH_BINS {
            *bins.add(i) = std::ptr::null_mut();
        }

        // `checkSlow` on the name length is compiled out (DO_GUARD_SLOW=0), so its expression is never evaluated.
        let count = svc.hardcoded_count()?;
        for i in 0..count {
            let entry = svc.hardcoded_entry(i)?;
            (*entry).index = i;
            hardcode(svc, storage, entry)?;
        }

        for i in 0..NAME_HASH_BINS {
            let mut hash = *bins.add(i);
            while !hash.is_null() {
                let mut other = (*hash).hash_next.cast::<FNameEntry>();
                while !other.is_null() {
                    if svc.str_icmp(name_ptr(hash), name_ptr(other))? == 0 {
                        svc.err_name_duplicated(name_ptr(hash))?;
                    }
                    other = (*other).hash_next.cast();
                }
                hash = (*hash).hash_next.cast();
            }
        }

        (*storage).initialized = true;
        svc.debugf_init()
    }
}

/// `FName::StaticExit()`.
///
/// # Safety
/// `svc` must be the live services table.
/// `storage` must be live, unmoved and accessed serially, including by reentrant leaves.
pub unsafe fn static_exit(svc: &Hp2NameServices, storage: *mut NameStorage) -> NameResult {
    unsafe {
        if !(*storage).initialized {
            svc.check_fail(b"Initialized\0", 168)?;
        }
        // Dynamic entries follow the hardcoded table. Free them one by one.
        let mut i = svc.hardcoded_count()?;
        while i < (*storage).len() {
            let entry = (*storage).entry(i);
            if !entry.is_null() {
                svc.free_entry(entry)?;
            }
            i += 1;
        }
        (*storage).names = Vec::new();
        (*storage).names_len = 0;
        (*storage).available = Vec::new();
        (*storage).initialized = false;
        svc.debugf_exit()
    }
}

/// `FName::DisplayHash(FOutputDevice& Ar)`.
///
/// # Safety
/// `svc` must be the live services table. `ar` must be a live `FOutputDevice`.
/// `storage` must be live, unmoved and accessed serially, including by reentrant leaves.
pub unsafe fn display_hash(
    svc: &Hp2NameServices,
    storage: *mut NameStorage,
    ar: *mut c_void,
) -> NameResult {
    unsafe {
        let bins = NameStorage::bins_ptr(storage);
        let mut used_bins = 0;
        let mut name_count = 0;
        for i in 0..NAME_HASH_BINS {
            if !(*bins.add(i)).is_null() {
                used_bins += 1;
            }
            let mut hash = *bins.add(i);
            while !hash.is_null() {
                name_count += 1;
                hash = (*hash).hash_next.cast();
            }
        }
        svc.display_hash(ar, name_count, used_bins, NAME_HASH_BINS as i32)
    }
}

/// `FName::DeleteEntry(NAME_INDEX i)`.
///
/// # Safety
/// `svc` must be the live services table.
/// `storage` must be live, unmoved and accessed serially, including by reentrant leaves.
pub unsafe fn delete_entry(svc: &Hp2NameServices, storage: *mut NameStorage, i: i32) -> NameResult {
    unsafe {
        let entry = (*storage).entry(i);
        if entry.is_null() {
            svc.check_fail(b"NameEntry\0", 212)?;
        }
        if (*entry).flags & RF_NATIVE != 0 {
            svc.check_fail(b"!(NameEntry->Flags & RF_Native)\0", 213)?;
        }

        let crc = svc.crc_table()?;
        let i_hash = (strihash(crc, name_ptr(entry)) as usize) & (NAME_HASH_BINS - 1);
        let bins = NameStorage::bins_ptr(storage);
        let mut link: *mut *mut c_void = bins.add(i_hash).cast();
        while !(*link).is_null() && *link != entry.cast() {
            link = &raw mut (*(*link).cast::<FNameEntry>()).hash_next;
        }
        if (*link).is_null() {
            svc.err_unhashed(name_ptr(entry))?;
        }
        *link = (*(*link).cast::<FNameEntry>()).hash_next;

        svc.free_entry(entry)?;
        (*storage).set_entry(i, std::ptr::null_mut());
        (*storage).available.push(i);
        Ok(())
    }
}

/// `operator<<(FArchive& Ar, FNameEntry& E)`.
///
/// # Safety
/// `svc` must be the live services table. `ar` must be a live `FArchive`. `entry` must be live.
pub unsafe fn entry_serialize(
    svc: &Hp2NameServices,
    ar: *mut c_void,
    entry: *mut FNameEntry,
) -> NameResult {
    unsafe {
        if svc.ar_ver(ar)? < 64 {
            if svc.ar_is_loading(ar)? == 0 {
                svc.check_fail(b"Ar.IsLoading()\0", 238)?;
            }
            // Pre-64 archives store the name byte by byte. `FromAnsi` zero-extends each byte.
            // Writes past NAME_SIZE are the C++ overflow; the original has no bound here either.
            let name = name_ptr(entry);
            let mut count = 0usize;
            loop {
                let ach = svc.ar_read_byte(ar)?;
                *name.add(count) = Tchar::from(ach);
                if ach == 0 {
                    break;
                }
                count += 1;
            }
            *name.add(count) = 0;
        } else if svc.ar_is_loading(ar)? != 0 {
            svc.ar_load_string(ar, name_ptr(entry))?;
        } else {
            svc.ar_save_string(ar, name_ptr(entry))?;
        }
        svc.ar_serialize_flags(ar, &raw mut (*entry).flags)
    }
}

/// `AllocateNameEntry(const TCHAR* Name, DWORD Index, DWORD Flags, FNameEntry* HashNext)`.
///
/// # Safety
/// `svc` must be the live services table. `name` must be NUL-terminated.
pub unsafe fn allocate_entry(
    svc: &Hp2NameServices,
    name: *const Tchar,
    index: u32,
    flags: u32,
    hash_next: *mut FNameEntry,
) -> NameResult<*mut FNameEntry> {
    unsafe {
        let name_length = svc.str_len(name)? as usize;
        if !(name_length < NAME_SIZE) {
            svc.check_fail(b"NameLength<NAME_SIZE\0", 269)?;
        }
        let entry_size =
            size_of::<FNameEntry>() - (NAME_SIZE - name_length - 1) * size_of::<Tchar>();
        let size = checked_alloc_size(svc, entry_size)?;
        let entry = svc.alloc(size)?;
        (*entry).index = index as i32;
        (*entry).flags = flags;
        (*entry).hash_next = hash_next.cast();
        svc.str_cpy(name_ptr(entry), name)?;
        Ok(entry)
    }
}

/// `appCheckedAllocSize(SIZE_T)` (UnFile.h:610).
fn checked_alloc_size(svc: &Hp2NameServices, size: usize) -> NameResult<u32> {
    if size > u32::MAX as usize {
        svc.err_alloc_size()?;
    }
    Ok(size as u32)
}

/// `appStrihash(Data)` (UnFile.h:363), UNICODE build: two CRC steps per character.
///
/// # Safety
/// `crc` must point to the 256-entry `GCRCTable`. `data` must be NUL-terminated.
pub unsafe fn strihash(crc: *const u32, data: *const Tchar) -> u32 {
    let mut pointer = data;
    // SAFETY: The caller guarantees a NUL-terminated input and a 256-entry CRC table.
    strihash_chars(crc, std::iter::from_fn(|| unsafe {
        let character = *pointer;
        pointer = pointer.add(1);
        Some(character)
    }))
}

/// Hashes UTF-32 text with the same case folding and CRC kernel as `appStrihash`.
pub fn strihash_text(data: &[u32]) -> u32 {
    strihash_chars(CRC_TABLE.as_ptr(), data.iter().copied().map(|unit| unit as Tchar))
}

fn strihash_chars(crc: *const u32, characters: impl Iterator<Item = Tchar>) -> u32 {
    let mut hash = 0;
    for character in characters {
        if character == 0 {
            break;
        }
        let ch = to_upper(character);
        let low = ch as u8;
        hash = ((hash >> 8) & 0x00FF_FFFF)
            ^ unsafe { *crc.add(((hash ^ u32::from(low)) & 0xFF) as usize) };
        let high = (ch >> 8) as u8;
        hash = ((hash >> 8) & 0x00FF_FFFF)
            ^ unsafe { *crc.add(((hash ^ u32::from(high)) & 0xFF) as usize) };
    }
    hash
}

/// `appToUpper` (UnFile.h:288).
pub fn to_upper(c: Tchar) -> Tchar {
    if c < Tchar::from(b'a') || c > Tchar::from(b'z') {
        c
    } else {
        c + Tchar::from(b'A') - Tchar::from(b'a')
    }
}

/// Address of the `TCHAR Name[NAME_SIZE]` storage of an entry.
///
/// # Safety
/// `entry` must point to a live `FNameEntry`.
unsafe fn name_ptr(entry: *mut FNameEntry) -> *mut Tchar {
    unsafe { (&raw mut (*entry).name).cast::<Tchar>() }
}
