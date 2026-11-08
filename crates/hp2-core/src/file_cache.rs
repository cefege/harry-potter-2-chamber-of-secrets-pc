//! Unix file-cache cleanup policy. Native leaves own the FString/TArray scopes;
//! Rust owns traversal, threshold decisions and the ordering of observable calls.
use std::ffi::c_void;

/// Checked native operations for one `appCleanFileCache` invocation.
///
/// The argument is zero unless documented. Only `FoundCount`, `PurgeCacheDays`
/// and `AgeDays` return a value. All other operation results are ignored.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2FileCacheLeaf {
    /// Enter the original native DeleteTemps guard, construct Temp and Found
    /// directly, and call clean_temporary_pass before leaving that stack frame.
    RunTemporaryPass,
    /// Enter DeleteExpired, construct Found unconditionally, and call
    /// clean_expired_pass before destroying Found and leaving the native guard.
    RunExpiredPass,
    /// Return the active Found.Num(), without refreshing the FindFiles snapshot.
    FoundCount,
    /// Assign Temp with FString::Printf(TEXT("%s") PATH_SEPARATOR TEXT("%s"),
    /// *GSys->CachePath, *Found(argument)). Read the current CachePath here.
    TemporaryPath,
    /// debugf(TEXT("Deleting temporary file: %s"), *Temp).
    LogTemporary,
    /// GFileManager->Delete(*Temp), ignoring its return value. Read the current
    /// GFileManager here, after logging; retain the same Temp across both calls.
    DeleteCurrent,
    /// Return the current GSys->PurgeCacheDays as a signed native INT.
    PurgeCacheDays,
    /// Construct the per-entry Temp with FString::Printf(TEXT("%s")
    /// PATH_SEPARATOR TEXT("%s"), *GSys->CachePath, *Found(argument)).
    ExpiredPath,
    /// Return GetFileAgeDays(*Temp) as a signed native INT.
    AgeDays,
    /// debugf(TEXT("Purging outdated file from cache: %s (%i days old)"),
    /// *Temp, argument). The argument is the previously obtained DiffDays.
    LogExpired,
    /// Destroy the per-entry Temp, whether or not the entry was deleted.
    FinishExpiredPath,
}

/// Immutable callback table, valid through all synchronous calls and reentry.
///
/// `context` owns native typed storage, not Rust. Each callback returns zero on
/// success or its exact nonzero failure status, containing native exceptions
/// before returning. It writes a numeric output only after successful completion.
/// On failure the native shell must unwind any active native scopes itself;
/// Rust issues no subsequent callbacks, including cleanup callbacks. Nested
/// invocations must use independent contexts and must not replace this one's
/// Found snapshot or Temp. No native-object borrow or lock spans a callback.
#[repr(C)]
pub struct Hp2FileCacheServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, Hp2FileCacheLeaf, i32, *mut i32) -> i32,
}

impl Hp2FileCacheServices {
    fn call(&self, leaf: Hp2FileCacheLeaf, argument: i32) -> Result<i32, i32> {
        let mut value = 0;
        let status = unsafe { (self.call)(self.context, leaf, argument, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
}

/// Run both passes in order. Native phase callbacks retain real lexical guards
/// and directly constructed typed locals while synchronously calling the Rust
/// pass below; C++ exception fences remain live until their callbacks return.
///
/// # Safety
/// Services satisfy `Hp2FileCacheServices`, including independent reentry state.
pub unsafe fn clean_file_cache(services: &Hp2FileCacheServices) -> Result<(), i32> {
    services.call(Hp2FileCacheLeaf::RunTemporaryPass, 0)?;
    services.call(Hp2FileCacheLeaf::RunExpiredPass, 0)?;
    Ok(())
}

/// Traverse the native temporary-file snapshot inside its original guard.
///
/// # Safety
/// The native caller owns the live Found and Temp for this entire pass.
pub unsafe fn clean_temporary_pass(services: &Hp2FileCacheServices) -> Result<(), i32> {
    use Hp2FileCacheLeaf as L;
    let mut index = 0;
    while index < services.call(L::FoundCount, 0)? {
        services.call(L::TemporaryPath, index)?;
        services.call(L::LogTemporary, 0)?;
        services.call(L::DeleteCurrent, 0)?;
        index += 1;
    }
    Ok(())
}

/// Traverse expired files after unconditional native discovery.
///
/// # Safety
/// Found remains live throughout the pass. ExpiredPath directly constructs one
/// native FString, and FinishExpiredPath destroys it. On failure the native
/// caller unwinds that same object before its Found and DeleteExpired guard.
pub unsafe fn clean_expired_pass(services: &Hp2FileCacheServices) -> Result<(), i32> {
    use Hp2FileCacheLeaf as L;
    if services.call(L::PurgeCacheDays, 0)? != 0 {
        let mut index = 0;
        while index < services.call(L::FoundCount, 0)? {
            services.call(L::ExpiredPath, index)?;
            let age = services.call(L::AgeDays, 0)?;
            // The original rereads the live signed threshold after the query.
            if age > services.call(L::PurgeCacheDays, 0)? {
                services.call(L::LogExpired, age)?;
                services.call(L::DeleteCurrent, 0)?;
            }
            services.call(L::FinishExpiredPath, 0)?;
            index += 1;
        }
    }
    Ok(())
}
