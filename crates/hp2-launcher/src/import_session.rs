//! Desktop launcher import-session and coupled result-selection policy.
//!
//! Native `Impl` remains the owner of the request/result, strings, option/save
//! vectors, `std::thread`, and `HP2Import::Progress` atomics. No Rust model clone,
//! thread, progress object, or persistent session allocation is introduced. The
//! Emscripten-only bridge is deliberately outside this desktop policy: that
//! target has no Rust importer/runtime in the existing build.

use std::ffi::c_void;

/// One native session operation. Picker actions are invoked only after the UI
/// reports a click; capability checks and drawing remain in the native UI.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherImportSessionAction {
    Initialize,
    Start,
    Worker,
    FinishIfReady,
    Cancel,
    Close,
    RefreshRetail,
    PickRetailFolder,
    PickRetailArchive,
    PickPrototypeFolder,
    SelectRetail,
    SelectPrototype,
    CompleteContinue,
    CompleteNewGame,
    CompleteQuit,
    CompleteError,
    /// DrawPlay's fallback immediately before its nonempty save combo. The
    /// caller retains the UI's nonempty-saves branch; do not clamp high indices.
    PrepareSavePicker,
}

/// Numeric values match the existing native LaunchAction, without changing the
/// representation of the Rust launcher's already-owned model enum.
///
/// cbindgen:prefix-with-name
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2LauncherImportSessionLaunchAction {
    Continue = 0,
    NewGame = 1,
    Quit = 2,
    Error = 3,
}

/// Checked, typed native leaves. `index` and `value` are zero unless documented.
/// Boolean query outputs are 0/1; other outputs are described below. Mutation
/// leaves must perform only the stated assignment/call, without policy branches.
///
/// A native invocation frame owns ordinary stack-local strings `source`,
/// `destination`, `root`, `canonicalRoot`, and `error` as needed. In Start,
/// `source` may instead refer to the caller's const string. In Worker, source
/// and destination refer to that thread's lambda captures. Scratch error is not
/// the session's settings `validationError` or worker `importError`.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherImportSessionLeaf {
    /// Return live Impl::importing (main thread only).
    Importing,
    /// Return invocation source.empty().
    SourceEmpty,
    /// destination = request.retailDataRoot (after start gating).
    CaptureDestination,
    /// Return destination.empty().
    DestinationEmpty,
    /// Call GetHP2ConventionalRetailDataRoot(destination) directly
    /// (`HP2Paths.h`), returning its bool. Do not add root validation or
    /// fallback selection.
    ConventionalDestination,
    /// request.retailDataRoot = destination.
    PublishDestination,
    /// importProgress.filesDone.store(0), default seq_cst.
    ResetFilesDone,
    /// importProgress.filesTotal.store(0), default seq_cst.
    ResetFilesTotal,
    /// importProgress.cancelRequested.store(false), default seq_cst.
    ResetCancellation,
    /// importDone.store(false), default seq_cst.
    ResetDone,
    /// importing = (value != 0).
    SetImporting,
    /// Exactly importer = std::thread([this, source, destination] { ... }).
    /// The lambda creates its OWN checked frame/table and invokes Worker; it
    /// does not capture the spawning invocation frame. Do not join, detach,
    /// reset messages/results, or catch an exception across threads here.
    StartWorker,
    /// Worker-thread only: importSucceeded = (value != 0).
    SetSucceeded,
    /// Worker-thread only: importDone.store(true, memory_order_release).
    PublishDone,
    /// Return importDone.load(memory_order_acquire).
    DoneAcquire,
    /// Return importer.joinable().
    Joinable,
    /// importer.join(), with no native-object borrow or lock held by Rust.
    Join,
    /// Return importSucceeded, only after DoneAcquire true and any join.
    Succeeded,
    /// Return importError.empty(), only after worker completion synchronization.
    ImportErrorEmpty,
    /// dataMessage = importError.
    MessageImportError,
    /// importProgress.cancelRequested.store(true, memory_order_relaxed).
    RequestCancellation,
    /// root = request.retailDataRoot; canonicalRoot and error start empty in
    /// this invocation, matching RefreshRetailSource's local strings.
    CaptureRetailRoot,
    /// Return root.empty().
    RootEmpty,
    /// ValidateHP2DataRoot(root, canonicalRoot, error); return its bool.
    ValidateRetailRoot,
    /// Return options.size() as a nonnegative i64; vector must not be resized
    /// during this range traversal (the original range-for has that contract).
    OptionCount,
    /// Return static_cast<int>(options[index].source).
    OptionSource,
    /// options[index].root = value ? canonicalRoot : root. For the prototype
    /// picker the raw-root slot is source, not a second copy of the picked path.
    SetOptionRoot,
    /// options[index].available = (value != 0).
    SetOptionAvailable,
    /// options[index].error.clear().
    ClearOptionError,
    /// options[index].error = invocation error.
    SetOptionError,
    /// request.dataSources.retailRoot = value ? canonicalRoot : root.
    SetRetailRoot,
    /// request.dataSources.selected = static_cast<DataSource>(value).
    SetSelectedSource,
    /// result.dataSources = request.dataSources (read live at this call).
    CopyResultDataSources,
    /// Return invocation error.empty().
    RootErrorEmpty,
    /// dataMessage = invocation error.
    MessageRootError,
    /// hp2_rs_launcher_pick_folder(source); return its bool. The genuine
    /// modal OS dialog and all its lexical native lifetimes stay in this leaf.
    PickFolder,
    /// ValidateHP2DataRoot(source, canonicalRoot, error); return its bool.
    /// Do not skip this validation when a successful picker returns empty text.
    ValidatePrototypeRoot,
    /// request.dataSources.prototypeRoot = value ? canonicalRoot : source.
    SetPrototypeRoot,
    /// ValidateLauncherSettings(result.settings, validationError); return its
    /// bool. Use the existing native store adapter to the Rust-owned validator.
    ValidateSettings,
    /// validationError.clear().
    ClearValidationError,
    /// result.selection = LaunchSelection{}.
    ResetSelection,
    /// result.selection.action = static_cast<LaunchAction>(value).
    SetSelectionAction,
    /// Return selectedSave as a signed i32 widened to i64.
    SelectedSave,
    /// Return static_cast<int>(request.saves.size()), widened to i64. Preserve
    /// the native signed comparison in HasSave, not a normalized usize check.
    SaveCountAsInt,
    /// Return request.saves.size() as a nonnegative i64, reread each iteration
    /// of initial save selection.
    SaveCount,
    /// Return request.saves[index].saveIndex as a signed i32 widened to i64.
    SaveIndex,
    /// selectedSave = static_cast<int>(index).
    SetSelectedSave,
    /// result.selection.hasSave = true (before copying the SaveRecord).
    SetSelectionHasSave,
    /// result.selection.save = request.saves[static_cast<size_t>(selectedSave)].
    /// Read the selected index live here, as the native assignment does.
    CopySelectedSave,
    /// finished = true. Do not cancel or join here; teardown owns that order.
    SetFinished,
    /// result.settings = request.settings, for constructor initialization only.
    CopyInitialSettings,
}

/// One non-NUL-terminated picker extension. Read only during pick_archive.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherImportSessionExtension {
    pub data: *const u8,
    pub len: usize,
}

/// Copied callback table for ONE synchronous invocation on ONE native thread.
///
/// Every leaf catches native exceptions into that invocation's first-exception
/// slot and returns a nonzero status, or zero on success. A query writes its
/// output only on success. An ordinary false importer/validator/picker outcome
/// is a successful callback with output zero, not a callback error. Rust stops
/// immediately on error: it neither compensates partial mutations nor performs
/// additional cleanup callbacks. The native shell rethrows only AFTER its Rust
/// export returns, on the same thread, unwinding real native stack locals.
///
/// Worker entry MUST have an independent exception slot/frame from Start and
/// the main thread. Do not publish completion after a throwing importer. The
/// original uncaught worker exception terminates the native thread/process;
/// do not convert it to an import failure, ferry exception_ptr to the UI thread,
/// or invent recovery. A thread-construction exception leaves importing=true,
/// as in the original; later Close cancels and queries joinability normally.
///
/// The native owner serializes UI actions. Nested callbacks may reenter with
/// independent invocation scratch/exception slots; no Rust exclusive reference
/// to Impl, arena guard, or mutex survives a host call, worker spawn, or join.
/// Destruction must invoke Close BEFORE destroying any worker-accessible field
/// or SDL/ImGui resource, and keep Impl alive until Join returns. Closing is not
/// finishing: Close never clears importing, publishes results, or refreshes data.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherImportSessionServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2LauncherImportSessionLeaf,
        usize,
        i32,
        *mut i64,
    ) -> i32,
    /// dataMessage.assign(bytes, len); copy before return. No NUL is supplied.
    pub message: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32,
    /// Forward the supplied extension spans to hp2_rs_launcher_pick_file(source,
    /// extensions), and return its bool through picked. Keep the OS-dialog
    /// scope in this native call.
    pub pick_archive: unsafe extern "C" fn(
        *mut c_void,
        *const Hp2LauncherImportSessionExtension,
        usize,
        *mut bool,
    ) -> i32,
    /// Worker-thread only: this invocation's `source`/`destination` byte
    /// spans (raw bytes, not necessarily UTF-8 -- the real worker lambda's
    /// `std::string` captures) and `&importProgress`, each valid only for
    /// this call's duration. Replaces the former RunRetailImport leaf's
    /// implicit access to those same lambda captures: Rust now runs the
    /// import itself (`run_retail_import`, below) instead of calling back
    /// into the deleted HP2DataImport::RunRetailImport.
    pub import_locals: unsafe extern "C" fn(
        *mut c_void,
        *mut *const u8,
        *mut usize,
        *mut *const u8,
        *mut usize,
        *mut *mut c_void,
    ) -> i32,
    /// Worker-thread only: importOutput.assign(output); importError.assign(error).
    /// Replaces HP2DataImport::RunRetailImport's own `Output`/`Error`
    /// out-parameters: the caller passes the exact final text for both
    /// (including an intentionally empty one), matching that function's own
    /// quirk that Output stays empty on failure even though its text is
    /// still the one written to the import log.
    pub publish_import_result: unsafe extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        *const u8,
        usize,
    ) -> i32,
}

impl Hp2LauncherImportSessionServices {
    fn call(
        &self,
        leaf: Hp2LauncherImportSessionLeaf,
        index: usize,
        value: i32,
    ) -> Result<i64, i32> {
        let mut output = 0;
        let status = unsafe { (self.call)(self.context, leaf, index, value, &mut output) };
        if status == 0 { Ok(output) } else { Err(status) }
    }

    fn query(&self, leaf: Hp2LauncherImportSessionLeaf) -> Result<bool, i32> {
        self.call(leaf, 0, 0).map(|value| value != 0)
    }

    fn perform(&self, leaf: Hp2LauncherImportSessionLeaf) -> Result<(), i32> {
        self.call(leaf, 0, 0).map(|_| ())
    }

    fn message(&self, text: &[u8]) -> Result<(), i32> {
        let status = unsafe { (self.message)(self.context, text.as_ptr(), text.len()) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}

use Hp2LauncherImportSessionLeaf as L;

fn start(services: &Hp2LauncherImportSessionServices) -> Result<(), i32> {
    if services.query(L::Importing)? || services.query(L::SourceEmpty)? {
        return Ok(());
    }
    services.perform(L::CaptureDestination)?;
    if services.query(L::DestinationEmpty)? && !services.query(L::ConventionalDestination)? {
        return services.message(b"Could not determine the retail data destination.");
    }
    services.perform(L::PublishDestination)?;
    services.perform(L::ResetFilesDone)?;
    services.perform(L::ResetFilesTotal)?;
    services.perform(L::ResetCancellation)?;
    services.perform(L::ResetDone)?;
    services.call(L::SetImporting, 0, 1)?;
    services.perform(L::StartWorker)
}

fn join_if_joinable(services: &Hp2LauncherImportSessionServices) -> Result<(), i32> {
    if services.query(L::Joinable)? {
        services.perform(L::Join)?;
    }
    Ok(())
}

fn update_options(
    services: &Hp2LauncherImportSessionServices,
    source: i64,
    valid: bool,
) -> Result<(), i32> {
    let count = services.call(L::OptionCount, 0, 0)? as usize;
    for index in 0..count {
        if services.call(L::OptionSource, index, 0)? == source {
            services.call(L::SetOptionRoot, index, i32::from(valid))?;
            services.call(L::SetOptionAvailable, index, i32::from(valid))?;
            services.call(
                if valid {
                    L::ClearOptionError
                } else {
                    L::SetOptionError
                },
                index,
                0,
            )?;
        }
    }
    Ok(())
}

fn refresh_retail(services: &Hp2LauncherImportSessionServices) -> Result<(), i32> {
    services.perform(L::CaptureRetailRoot)?;
    let valid = !services.query(L::RootEmpty)? && services.query(L::ValidateRetailRoot)?;
    update_options(services, 0, valid)?;
    services.call(L::SetRetailRoot, 0, i32::from(valid))?;
    services.call(L::SetSelectedSource, 0, 0)?;
    services.perform(L::CopyResultDataSources)?;
    if valid {
        services.message(b"Retail game data imported successfully.")
    } else if services.query(L::RootErrorEmpty)? {
        services.message(b"Imported data could not be validated.")
    } else {
        services.perform(L::MessageRootError)
    }
}

/// Native HasSave semantics, including short-circuiting and live signed reads.
///
/// # Safety
/// The services must obey their synchronous, checked native callback contract.
pub unsafe fn has_save(services: &Hp2LauncherImportSessionServices) -> Result<bool, i32> {
    Ok(services.call(L::SelectedSave, 0, 0)? >= 0
        && services.call(L::SelectedSave, 0, 0)? < services.call(L::SaveCountAsInt, 0, 0)?)
}

fn complete(
    services: &Hp2LauncherImportSessionServices,
    action: Hp2LauncherImportSessionLaunchAction,
) -> Result<(), i32> {
    use Hp2LauncherImportSessionLaunchAction as A;
    if action != A::Quit {
        if !services.query(L::ValidateSettings)? {
            return Ok(());
        }
    } else {
        services.perform(L::ClearValidationError)?;
    }
    services.perform(L::ResetSelection)?;
    services.call(L::SetSelectionAction, 0, action as i32)?;
    if action == A::Continue && unsafe { has_save(services)? } {
        services.perform(L::SetSelectionHasSave)?;
        services.perform(L::CopySelectedSave)?;
    }
    services.perform(L::CopyResultDataSources)?;
    services.perform(L::SetFinished)
}

/// Worker-thread only: replaces `HP2DataImport::RunRetailImport`
/// (`HP2DataImport.cpp:13-96`, deleted). `source`/`destination` and
/// `&importProgress` come from `services.import_locals`; the result is
/// published back through `services.publish_import_result`, matching the
/// deleted function's exact mutation order and quirks:
///
/// - The import log (`destination + ".import.log"`) is created and opened
///   BEFORE the import runs at all; if the parent directory can't be
///   created or the log file can't be opened, the import is never
///   attempted and this returns `false` with
///   "Could not write the import log next to the game data folder."
/// - `source` is a directory (`retailRoot`) or an archive (`archive`),
///   decided by the throwing `std::filesystem::is_directory(u8path(...))`
///   overload (`HP2DataImport.cpp:60`): a not-found path (`ENOENT`/
///   `ENOTDIR`) is simply `false`, but any other `stat` failure throws
///   `std::filesystem::filesystem_error`. That exception was never
///   caught: it unwound out of `StartImport`'s background `std::thread`
///   lambda (`HP2Launcher.cpp`'s original `importer = std::thread([this,
///   ...]){ ... RunRetailImport(...) ... }`, no surrounding try/catch)
///   straight into `std::terminate`, aborting the whole launcher process.
///   Reproduced here by returning a nonzero status from this function: it
///   passes through `hp2_rs_launcher_import_session`'s `guard` untouched,
///   and `HP2Launcher.cpp`'s `ImportSessionFrame::Complete` throws on any
///   nonzero `Worker`-action status on that same background thread,
///   uncaught, reproducing the same uncaught-exception process abort (see
///   that file's "Rethrow after Rust returns retains the original
///   uncaught-worker termination" comment). Residual gap: the crash text
///   differs -- the original's uncaught exception is a
///   `std::filesystem::filesystem_error`, while `Complete` throws a
///   `std::runtime_error("Rust launcher import session failed.")`; both
///   terminate the process uncaught on the same thread, but the
///   runtime's printed "terminate called after throwing..." text names a
///   different exception type.
/// - The importer's result text becomes `importOutput` on success or
///   `importError` on failure; `importOutput` is NEVER the failure text.
/// - The log file receives exactly `importOutput`'s bytes, success or
///   empty-on-failure, THEN the write is checked: a write failure
///   overrides a successful import's return value to `false` and
///   overwrites `importError`, while leaving `importOutput` (the success
///   text) untouched.
///
/// The directory-branch UTF-8 check ("retail root path is not valid
/// UTF-8") shares the archive branch's `std::str::from_utf8` call below
/// and is covered by `native_launcher_contract`/`launcher_smoke`; it has
/// no standalone unit test here because constructing its precondition (a
/// real, existing directory whose name is invalid UTF-8) is impossible on
/// this host's filesystem -- APFS rejects such names with EILSEQ even via
/// a raw `mkdir`, confirmed with `python3 -c "os.mkdir(b'...\xff\xfe')"`.
fn run_retail_import(services: &Hp2LauncherImportSessionServices) -> Result<bool, i32> {
    use std::ffi::OsStr;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    fn raw_slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
        if len == 0 {
            &[]
        } else {
            // SAFETY: the caller (`import_locals`) guarantees `(ptr, len)`
            // is valid for reads of `len` bytes for this call's duration.
            unsafe { std::slice::from_raw_parts(ptr, len) }
        }
    }

    let mut source_ptr: *const u8 = std::ptr::null();
    let mut source_len: usize = 0;
    let mut destination_ptr: *const u8 = std::ptr::null();
    let mut destination_len: usize = 0;
    let mut progress: *mut c_void = std::ptr::null_mut();
    let status = unsafe {
        (services.import_locals)(
            services.context,
            &mut source_ptr,
            &mut source_len,
            &mut destination_ptr,
            &mut destination_len,
            &mut progress,
        )
    };
    if status != 0 {
        return Err(status);
    }
    let source = raw_slice(source_ptr, source_len);
    let destination = raw_slice(destination_ptr, destination_len);

    let publish = |output: &str, error: &str| -> Result<(), i32> {
        let status = unsafe {
            (services.publish_import_result)(
                services.context,
                output.as_ptr(),
                output.len(),
                error.as_ptr(),
                error.len(),
            )
        };
        if status == 0 { Ok(()) } else { Err(status) }
    };

    const CANNOT_WRITE_LOG: &str = "Could not write the import log next to the game data folder.";

    // Matches the original `std::filesystem::is_directory(u8path(SourcePath))`
    // call (`HP2DataImport.cpp:60`), which runs *before* the log's parent
    // directories are created and the log file is opened -- preserve that
    // exact order so the decision can never be influenced by `create_dir_all`
    // creating `source` as a side effect. `stat`'s `ENOENT`/`ENOTDIR` map to
    // the throwing overload's `file_not_found` outcome (no throw, `false`);
    // any other failure (`EACCES`, `ELOOP`, ...) is the overload's actual
    // throw, uncaught on the original's worker thread (see the doc comment
    // above) -- reproduced by returning a nonzero status (any nonzero value
    // works: `HP2Launcher.cpp`'s `Complete` only checks `status != 0`).
    let is_dir = match std::fs::metadata(Path::new(OsStr::from_bytes(source))) {
        Ok(metadata) => metadata.is_dir(),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.raw_os_error() == Some(libc::ENOTDIR) =>
        {
            false
        }
        Err(_) => return Err(1),
    };

    let mut log_path_bytes = destination.to_vec();
    log_path_bytes.extend_from_slice(b".import.log");
    let log_path = Path::new(OsStr::from_bytes(&log_path_bytes)).to_path_buf();

    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() {
            publish("", CANNOT_WRITE_LOG)?;
            return Ok(false);
        }
    }

    let mut log_file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&log_path)
    {
        Ok(file) => file,
        Err(_) => {
            publish("", CANNOT_WRITE_LOG)?;
            return Ok(false);
        }
    };

    let import_result: Result<String, String> = (|| {
        let retail_root = if is_dir {
            std::str::from_utf8(source)
                .map_err(|_| "retail root path is not valid UTF-8".to_string())?
        } else {
            ""
        };
        let archive = if is_dir {
            ""
        } else {
            std::str::from_utf8(source).map_err(|_| "archive path is not valid UTF-8".to_string())?
        };
        let output_path =
            std::str::from_utf8(destination).map_err(|_| "output path is not valid UTF-8".to_string())?;
        let options = hp2_import::output::Options {
            retail_root: retail_root.to_string(),
            archive: archive.to_string(),
            prototype_root: String::new(),
            output: output_path.to_string(),
            profile: hp2_ini::Profile::RetailOnly,
            link_mode: hp2_import::plan::LinkMode::Copy,
            check_only: false,
        };
        // SAFETY: `progress`, non-null, points at a live
        // `HP2Import::Progress`-layout-compatible value for the duration of
        // this call, per `import_locals`'s contract.
        let progress_ref = unsafe { progress.cast::<hp2_import::output::Progress>().as_ref() };
        // Rule 4: no panics across FFI. A panic inside `run_import` (e.g. an
        // unexpected internal invariant failure) must not unwind through
        // this function's `extern "C"` caller; treat it as an ordinary
        // import failure with an empty error, the same outcome the deleted
        // `hp2_rs_import_run`'s `hp2_ffi_support::guard` produced on panic
        // (sink never called, so the C++ side's `Error` stayed empty).
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            hp2_import::output::run_import(&options, progress_ref)
        }))
        .unwrap_or_else(|_| Err(String::new()))
    })();

    let (mut succeeded, output_text, mut error_text) = match import_result {
        Ok(text) => (true, text, String::new()),
        Err(text) => (false, String::new(), text),
    };

    if log_file.write_all(output_text.as_bytes()).is_err() {
        error_text = CANNOT_WRITE_LOG.to_string();
        succeeded = false;
    }

    publish(&output_text, &error_text)?;
    Ok(succeeded)
}

fn pick_archive(services: &Hp2LauncherImportSessionServices) -> Result<(), i32> {
    // Exactly the original nine archive extensions, in original dialog order.
    let extensions = [
        "zip", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "7z",
    ]
    .map(|extension| Hp2LauncherImportSessionExtension {
        data: extension.as_ptr(),
        len: extension.len(),
    });
    let mut picked = false;
    let status = unsafe {
        (services.pick_archive)(
            services.context,
            extensions.as_ptr(),
            extensions.len(),
            &mut picked,
        )
    };
    if status != 0 {
        return Err(status);
    }
    if picked { start(services) } else { Ok(()) }
}

/// Execute complete desktop session control, with exact native mutation order.
/// In particular, Start does NOT clear importOutput/importError/dataMessage or
/// importSucceeded; the real RunRetailImport clears its output/error itself.
/// Finish consumes success/error only after acquire and join, then clears
/// importing before reporting failure or refreshing all matching retail options.
/// Neither successful completion nor Quit itself joins/cancels the worker.
///
/// # Safety
/// The services must satisfy the documented lifetime, thread, and exception
/// contracts, including a separate native Worker invocation frame.
pub unsafe fn execute(
    services: &Hp2LauncherImportSessionServices,
    action: Hp2LauncherImportSessionAction,
) -> Result<(), i32> {
    use Hp2LauncherImportSessionAction as A;
    use Hp2LauncherImportSessionLaunchAction as Launch;
    match action {
        A::Initialize => {
            services.perform(L::CopyInitialSettings)?;
            services.perform(L::CopyResultDataSources)?;
            services.call(L::SetSelectionAction, 0, Launch::Quit as i32)?;
            let mut index = 0;
            while index < services.call(L::SaveCount, 0, 0)? as usize {
                if services.call(L::SaveIndex, index, 0)? >= 0 {
                    services.call(L::SetSelectedSave, index, 0)?;
                    break;
                }
                index += 1;
            }
            Ok(())
        }
        A::Start => start(services),
        A::Worker => {
            let succeeded = run_retail_import(services)?;
            services.call(L::SetSucceeded, 0, i32::from(succeeded))?;
            services.perform(L::PublishDone)
        }
        A::FinishIfReady => {
            if !services.query(L::Importing)? || !services.query(L::DoneAcquire)? {
                return Ok(());
            }
            join_if_joinable(services)?;
            services.call(L::SetImporting, 0, 0)?;
            if !services.query(L::Succeeded)? {
                if services.query(L::ImportErrorEmpty)? {
                    return services.message(b"Import failed.");
                }
                return services.perform(L::MessageImportError);
            }
            refresh_retail(services)
        }
        A::Cancel => services.perform(L::RequestCancellation),
        A::Close => {
            if services.query(L::Importing)? {
                services.perform(L::RequestCancellation)?;
            }
            join_if_joinable(services)
        }
        A::RefreshRetail => refresh_retail(services),
        A::PickRetailFolder => {
            if services.query(L::PickFolder)? {
                start(services)
            } else {
                Ok(())
            }
        }
        A::PickRetailArchive => pick_archive(services),
        A::PickPrototypeFolder => {
            if services.query(L::PickFolder)? {
                let valid = services.query(L::ValidatePrototypeRoot)?;
                services.call(L::SetPrototypeRoot, 0, i32::from(valid))?;
                update_options(services, 1, valid)?;
            }
            // Desktop prototype picking changes neither selected nor result.
            Ok(())
        }
        A::SelectRetail => services.call(L::SetSelectedSource, 0, 0).map(|_| ()),
        A::SelectPrototype => services.call(L::SetSelectedSource, 0, 1).map(|_| ()),
        A::CompleteContinue => complete(services, Launch::Continue),
        A::CompleteNewGame => complete(services, Launch::NewGame),
        A::CompleteQuit => complete(services, Launch::Quit),
        A::CompleteError => complete(services, Launch::Error),
        A::PrepareSavePicker => {
            if services.call(L::SelectedSave, 0, 0)? < 0 {
                services.call(L::SetSelectedSave, 0, 0)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod run_retail_import_tests {
    use super::*;
    use crate::test_support::tempdir;
    use std::cell::RefCell;

    struct TestContext {
        source: Vec<u8>,
        destination: Vec<u8>,
        published: RefCell<Option<(String, String)>>,
    }

    unsafe extern "C" fn call_unreachable(
        _ctx: *mut c_void,
        _leaf: Hp2LauncherImportSessionLeaf,
        _index: usize,
        _value: i32,
        _output: *mut i64,
    ) -> i32 {
        unreachable!("run_retail_import never calls the leaf dispatcher directly")
    }

    unsafe extern "C" fn message_unreachable(_ctx: *mut c_void, _text: *const u8, _len: usize) -> i32 {
        unreachable!("run_retail_import never writes dataMessage")
    }

    unsafe extern "C" fn pick_archive_unreachable(
        _ctx: *mut c_void,
        _extensions: *const Hp2LauncherImportSessionExtension,
        _count: usize,
        _picked: *mut bool,
    ) -> i32 {
        unreachable!("run_retail_import never picks an archive")
    }

    unsafe extern "C" fn import_locals(
        ctx: *mut c_void,
        source_ptr: *mut *const u8,
        source_len: *mut usize,
        destination_ptr: *mut *const u8,
        destination_len: *mut usize,
        progress: *mut *mut c_void,
    ) -> i32 {
        let ctx = unsafe { &*ctx.cast::<TestContext>() };
        unsafe {
            *source_ptr = ctx.source.as_ptr();
            *source_len = ctx.source.len();
            *destination_ptr = ctx.destination.as_ptr();
            *destination_len = ctx.destination.len();
            *progress = std::ptr::null_mut();
        }
        0
    }

    unsafe extern "C" fn publish_import_result(
        ctx: *mut c_void,
        output: *const u8,
        output_len: usize,
        error: *const u8,
        error_len: usize,
    ) -> i32 {
        let ctx = unsafe { &*ctx.cast::<TestContext>() };
        let output = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(output, output_len) })
            .into_owned();
        let error = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(error, error_len) })
            .into_owned();
        *ctx.published.borrow_mut() = Some((output, error));
        0
    }

    fn services(ctx: &TestContext) -> Hp2LauncherImportSessionServices {
        Hp2LauncherImportSessionServices {
            context: (ctx as *const TestContext).cast_mut().cast::<c_void>(),
            call: call_unreachable,
            message: message_unreachable,
            pick_archive: pick_archive_unreachable,
            import_locals,
            publish_import_result,
        }
    }

    /// Archive-branch UTF-8 validation (`HP2DataImport.cpp` via the deleted
    /// `hp2_rs_import_run`'s "archive path is not valid UTF-8", now
    /// produced directly by `run_retail_import`): invalid UTF-8 fails
    /// before the importer ever runs, `importOutput` stays empty, the log
    /// file still receives those (zero) bytes, and the final return is
    /// `false`.
    #[test]
    fn invalid_utf8_archive_path_reports_exact_error() {
        let dir = tempdir();
        let destination = dir.path().join("dest");
        let ctx = TestContext {
            source: vec![b'/', 0xFF, 0xFE],
            destination: destination.as_os_str().as_encoded_bytes().to_vec(),
            published: RefCell::new(None),
        };
        let services = services(&ctx);
        let succeeded = run_retail_import(&services).unwrap();
        assert!(!succeeded);
        let (output, error) = ctx.published.borrow().clone().unwrap();
        assert_eq!(output, "");
        assert_eq!(error, "archive path is not valid UTF-8");
        let log_path = dir.path().join("dest.import.log");
        assert_eq!(std::fs::read(&log_path).unwrap(), b"");
    }

    /// Log open/create happens BEFORE the import is attempted at all
    /// (`HP2DataImport.cpp:33-48`): when the log's parent can't be created
    /// (here, an existing plain file occupies that path), the importer is
    /// never reached -- `call_unreachable`/`pick_archive_unreachable`
    /// panicking if hit is the proof -- and the published result is exactly
    /// ("", "Could not write the import log next to the game data
    /// folder.").
    #[test]
    fn log_parent_cannot_be_created_fails_before_import_attempted() {
        let dir = tempdir();
        let blocking_file = dir.path().join("blocked");
        std::fs::write(&blocking_file, b"not a directory").unwrap();
        let destination = blocking_file.join("nested").join("dest");
        let ctx = TestContext {
            source: b"/nonexistent/archive.zip".to_vec(),
            destination: destination.as_os_str().as_encoded_bytes().to_vec(),
            published: RefCell::new(None),
        };
        let services = services(&ctx);
        let succeeded = run_retail_import(&services).unwrap();
        assert!(!succeeded);
        let (output, error) = ctx.published.borrow().clone().unwrap();
        assert_eq!(output, "");
        assert_eq!(
            error,
            "Could not write the import log next to the game data folder."
        );
    }

    /// `std::filesystem::is_directory`'s throwing overload
    /// (`HP2DataImport.cpp:60`) throws on any `stat` failure other than
    /// not-found; the original never caught that exception on the
    /// background worker thread, so it aborted the whole process
    /// (`std::terminate`). A self-referential symlink always fails `stat`
    /// with `ELOOP`, regardless of privilege -- Rust's equivalent is
    /// `run_retail_import` returning a nonzero status before ever reaching
    /// the log file or the importer, matching that the original crashed
    /// before writing anything either.
    #[test]
    fn non_not_found_stat_error_fails_before_log_or_publish() {
        let dir = tempdir();
        let loop_path = dir.path().join("loop");
        std::os::unix::fs::symlink(&loop_path, &loop_path).unwrap();
        let destination = dir.path().join("dest");
        let ctx = TestContext {
            source: loop_path.as_os_str().as_encoded_bytes().to_vec(),
            destination: destination.as_os_str().as_encoded_bytes().to_vec(),
            published: RefCell::new(None),
        };
        let services = services(&ctx);
        assert_eq!(run_retail_import(&services), Err(1));
        assert!(ctx.published.borrow().is_none());
        assert!(!dir.path().join("dest.import.log").exists());
    }

    /// The `is_directory` throwing overload's `file_not_found` outcome
    /// also covers `ENOTDIR` (`HP2DataImport.cpp:60`; a path component
    /// that isn't a directory never gets to throw, `stat` just reports
    /// not-found-shaped), not only `ENOENT` -- pins the other half of the
    /// three-way `is_dir` match against the `Err(1)` branch above: this
    /// one must NOT abort. A non-directory path component falls through
    /// to `is_dir == false`, the log is still opened and written, and the
    /// import proceeds into the archive branch and fails there instead
    /// (not with the Rust-side "path is not valid UTF-8" guard, which
    /// never triggers for a plain, valid-UTF-8 path).
    #[test]
    fn enotdir_stat_error_falls_through_to_the_archive_path() {
        let dir = tempdir();
        let not_a_directory = dir.path().join("file");
        std::fs::write(&not_a_directory, b"not a directory").unwrap();
        let source = not_a_directory.join("child");
        let destination = dir.path().join("dest");
        let ctx = TestContext {
            source: source.as_os_str().as_encoded_bytes().to_vec(),
            destination: destination.as_os_str().as_encoded_bytes().to_vec(),
            published: RefCell::new(None),
        };
        let services = services(&ctx);
        let succeeded = run_retail_import(&services).unwrap();
        assert!(!succeeded);
        let (output, error) = ctx.published.borrow().clone().unwrap();
        assert_eq!(output, "");
        assert!(
            error.starts_with("retail archive is not a regular file: "),
            "{error}"
        );
        let log_path = dir.path().join("dest.import.log");
        assert_eq!(std::fs::read(&log_path).unwrap(), b"");
    }
}
