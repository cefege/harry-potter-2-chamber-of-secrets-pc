//! Pre-launch source precedence and the complete prepare/UI/commit round.
//! Native services retain typed model/UTF-32/SDL scopes. All decisions and
//! ordering live here; callback failures stop immediately without compensation.
use std::ffi::c_void;

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherSourceRootLeaf {
    RunOverrideScope,
    RunConfiguredScope,
    RunValidationScope,
    CandidateEmpty,
    DiscoverRetail,
    DiscoverPrototype,
    MissingRetail,
    MissingPrototype,
    Validate,
    ValidationErrorEmpty,
    PublishValidationError,
    PublishOverrideFallback,
    PublishConfiguredFallback,
    PublishCanonical,
}

/// Native validation temporaries are constructed only after discovery succeeds.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2LauncherSourceRootPhase {
    Root,
    Configured,
    Validation,
}

/// Each scope directly constructs the original native strings, calls the
/// scoped policy synchronously, and rethrows only after Rust returns. Native
/// exceptions are contained; zero is success and nonzero is the exact failure.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherSourceRootServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, Hp2LauncherSourceRootLeaf, i32, *mut i32) -> i32,
}
impl Hp2LauncherSourceRootServices {
    fn call(&self, leaf: Hp2LauncherSourceRootLeaf, argument: i32) -> Result<i32, i32> {
        let mut value = 0;
        let status = unsafe { (self.call)(self.context, leaf, argument, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
}

/// Source values other than Retail (zero) retain the native Prototype branch.
/// # Safety
/// The copied services refer to independent invocation-local native storage.
pub unsafe fn resolve_source_root(
    services: &Hp2LauncherSourceRootServices,
    source: i32,
    has_override: bool,
    phase: Hp2LauncherSourceRootPhase,
) -> Result<bool, i32> {
    use Hp2LauncherSourceRootLeaf as L;
    let source = i32::from(source != 0);
    if phase == Hp2LauncherSourceRootPhase::Root {
        return Ok(services.call(
            if has_override {
                L::RunOverrideScope
            } else {
                L::RunConfiguredScope
            },
            source,
        )? != 0);
    }
    if phase == Hp2LauncherSourceRootPhase::Configured {
        if services.call(L::CandidateEmpty, 0)? != 0 {
            let found = services.call(
                if source == 0 {
                    L::DiscoverRetail
                } else {
                    L::DiscoverPrototype
                },
                0,
            )?;
            if found == 0 {
                services.call(
                    if source == 0 {
                        L::MissingRetail
                    } else {
                        L::MissingPrototype
                    },
                    0,
                )?;
                return Ok(false);
            }
        }
        return Ok(services.call(L::RunValidationScope, 0)? != 0);
    }
    if services.call(L::Validate, 0)? == 0 {
        let leaf = if services.call(L::ValidationErrorEmpty, 0)? == 0 {
            L::PublishValidationError
        } else if has_override {
            L::PublishOverrideFallback
        } else {
            L::PublishConfiguredFallback
        };
        services.call(leaf, 0)?;
        return Ok(false);
    }
    services.call(L::PublishCanonical, 0)?;
    Ok(true)
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherRoundPhase {
    Prepare,
    PrepareResolution,
    RequestState,
    SourceOption,
    Finish,
    FinishResolution,
    SettingsCommit,
    Prefix,
    PrefixConversion,
    ReleaseVideo,
    RunRound,
    RunUi,
}

/// Query leaves return their value through the signed output. All scope leaves
/// return the nested policy's ordinary boolean, not its checked failure status.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherRoundLeaf {
    PrepareHome,
    LoadDataSources,
    ConventionalDestination,
    SelectConfiguredSource,
    PrepareSelectedProfile,
    RunPrepareResolution,
    ResolveSelectedRoot,
    HasOverride,
    HasDataRoot,
    RunRequestState,
    LoadState,
    ResetState,
    PublishRequest,
    RunOption,
    SelectedSource,
    OptionSource,
    ResolveOption,
    PublishOptionRoot,
    PublishOptionError,
    AppendOption,
    CollectModes,
    OwnsVideo,
    QuitVideo,
    ClearOwnsVideo,
    ResetOutcome,
    Action,
    CommitDataSources,
    CommitSelection,
    FinalSource,
    PrepareFinalProfile,
    RunFinishResolution,
    ResolveFinalRoot,
    ValidateSettings,
    RunSettingsCommit,
    CommitSettings,
    InstallPaths,
    RunPrefix,
    BuildCommand,
    PrefixEmpty,
    RunPrefixConversion,
    DecodePrefix,
    PublishOutcome,
    PrepareRound,
    RunUiScope,
    RunLauncherUi,
    FinishRound,
}

/// The text argument is an immutable NUL-terminated Rust literal valid through
/// the callback. Assignment leaves copy it; reports pass it directly as the
/// context to the existing native ShowLauncherError, without an extra string.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherRoundText {
    ImportDestination,
    RequestError,
    StepError,
    ResolveFailure,
    FinalFailure,
    PrefixError,
    UiError,
}

/// Native scope leaves preserve directly constructed LauncherPaths/State,
/// source options, prefix strings and TCHAR[1024] for the full nested call.
/// Each invocation has its own context; no borrow/lock spans callbacks. All
/// callbacks return zero on success or the exact captured native error status.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherRoundServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, Hp2LauncherRoundLeaf, i32, *mut i32) -> i32,
    pub text: unsafe extern "C" fn(*mut c_void, Hp2LauncherRoundText, *const u8) -> i32,
}
impl Hp2LauncherRoundServices {
    fn call(&self, leaf: Hp2LauncherRoundLeaf, argument: i32) -> Result<i32, i32> {
        let mut value = 0;
        let status = unsafe { (self.call)(self.context, leaf, argument, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn text(&self, kind: Hp2LauncherRoundText, text: &'static [u8]) -> Result<(), i32> {
        let status = unsafe { (self.text)(self.context, kind, text.as_ptr()) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}

/// # Safety
/// Services satisfy the native scope, exception and reentry contract above.
pub unsafe fn run_round_phase(
    services: &Hp2LauncherRoundServices,
    phase: Hp2LauncherRoundPhase,
) -> Result<bool, i32> {
    use Hp2LauncherRoundLeaf as L;
    use Hp2LauncherRoundPhase as P;
    use Hp2LauncherRoundText as T;
    let s = services;
    match phase {
        P::Prepare => {
            if s.call(L::PrepareHome, 0)? == 0 {
                s.text(T::StepError, b"Could not prepare the launcher folder.\0")?;
                return Ok(false);
            }
            s.call(L::LoadDataSources, 0)?; // Native deliberately ignores ordinary false.
            if s.call(L::ConventionalDestination, 0)? == 0 {
                s.text(T::ImportDestination, b"./Retail\0")?;
            }
            s.call(L::SelectConfiguredSource, 0)?;
            if s.call(L::PrepareSelectedProfile, 0)? == 0 {
                s.text(
                    T::StepError,
                    b"Could not prepare the profile for the selected data source.\0",
                )?;
                return Ok(false);
            }
            Ok(s.call(L::RunPrepareResolution, 0)? != 0)
        }
        P::PrepareResolution => {
            let found = s.call(L::ResolveSelectedRoot, 0)? != 0;
            if !found && s.call(L::HasOverride, 0)? != 0 {
                s.text(
                    T::ResolveFailure,
                    b"The data folder given on the command line cannot be used.\0",
                )?;
                return Ok(false);
            }
            Ok(s.call(L::RunRequestState, i32::from(found))? != 0)
        }
        P::RequestState => {
            let found = s.call(L::HasDataRoot, 0)? != 0;
            if found && s.call(L::LoadState, 0)? == 0 {
                s.call(L::ResetState, 0)?;
            }
            s.call(L::PublishRequest, i32::from(!found))?;
            if !found {
                s.text(T::RequestError, b"No game data is installed yet. Choose a source in Game Data to get started.\0")?;
            }
            s.call(L::RunOption, 0)?;
            s.call(L::RunOption, 1)?;
            Ok(s.call(L::CollectModes, 0)? != 0)
        }
        P::SourceOption => {
            let active_override = if s.call(L::HasOverride, 0)? != 0 {
                s.call(L::OptionSource, 0)? == s.call(L::SelectedSource, 0)?
            } else {
                false
            };
            let found = s.call(L::ResolveOption, i32::from(active_override))? != 0;
            s.call(
                if found {
                    L::PublishOptionRoot
                } else {
                    L::PublishOptionError
                },
                0,
            )?;
            s.call(L::AppendOption, 0)?;
            Ok(true)
        }
        P::Finish => {
            s.call(L::ResetOutcome, 0)?;
            match s.call(L::Action, 0)? {
                3 => {
                    s.text(T::StepError, b"The launcher reported an error.\0")?;
                    return Ok(false);
                }
                2 => return Ok(false),
                _ => {}
            }
            if s.call(L::CommitDataSources, 0)? == 0 || s.call(L::CommitSelection, 0)? == 0 {
                s.text(T::StepError, b"Could not save the launcher selection.\0")?;
                return Ok(false);
            }
            let final_source = s.call(L::FinalSource, 0)?;
            if final_source != s.call(L::SelectedSource, 0)?
                && s.call(L::PrepareFinalProfile, 0)? == 0
            {
                s.text(
                    T::StepError,
                    b"Could not prepare the newly selected data source.\0",
                )?;
                return Ok(false);
            }
            Ok(s.call(L::RunFinishResolution, 0)? != 0)
        }
        P::FinishResolution => {
            if s.call(L::ResolveFinalRoot, 0)? == 0 {
                s.text(
                    T::FinalFailure,
                    b"The selected data source has no usable game data.\0",
                )?;
                return Ok(false);
            }
            if s.call(L::ValidateSettings, 0)? == 0 {
                s.text(T::StepError, b"The selected settings are invalid.\0")?;
                return Ok(false);
            }
            Ok(s.call(L::RunSettingsCommit, 0)? != 0)
        }
        P::SettingsCommit => {
            if s.call(L::CommitSettings, 0)? == 0 {
                s.text(T::StepError, b"Could not save the selected settings.\0")?;
                return Ok(false);
            }
            if s.call(L::InstallPaths, 0)? == 0 {
                s.text(T::StepError, b"Could not install the selected paths.\0")?;
                return Ok(false);
            }
            Ok(s.call(L::RunPrefix, 0)? != 0)
        }
        P::Prefix => {
            if s.call(L::BuildCommand, 0)? == 0 {
                s.text(T::StepError, b"The launcher selection cannot be started.\0")?;
                return Ok(false);
            }
            if s.call(L::PrefixEmpty, 0)? != 0 {
                return Ok(false);
            }
            Ok(s.call(L::RunPrefixConversion, 0)? != 0)
        }
        P::PrefixConversion => {
            if s.call(L::DecodePrefix, 0)? == 0 {
                s.text(T::PrefixError, b"The launch command is invalid.\0")?;
                return Ok(false);
            }
            s.call(L::PublishOutcome, 0)?;
            Ok(true)
        }
        P::ReleaseVideo => {
            if s.call(L::OwnsVideo, 0)? != 0 {
                s.call(L::QuitVideo, 0)?;
                s.call(L::ClearOwnsVideo, 0)?;
            }
            Ok(true)
        }
        P::RunRound => {
            if s.call(L::PrepareRound, 0)? == 0 {
                return Ok(false);
            }
            Ok(s.call(L::RunUiScope, 0)? != 0)
        }
        P::RunUi => {
            let opened = s.call(L::RunLauncherUi, 0)? != 0;
            unsafe { run_round_phase(s, P::ReleaseVideo) }?;
            if !opened {
                s.text(T::UiError, b"Could not open the launcher.\0")?;
                return Ok(false);
            }
            Ok(s.call(L::FinishRound, 0)? != 0)
        }
    }
}

/// Replaces `ShowLauncherError`'s text formatting (`SDLLaunch.cpp`:
/// `Context + "\n\n" + (Detail.empty() ? "Unknown error." : Detail)`).
/// Operates on raw bytes, not `&str`: `Detail` crosses from C++, which
/// never validated it as UTF-8 either.
pub fn format_launcher_error(context: &[u8], detail: &[u8]) -> Vec<u8> {
    let detail: &[u8] = if detail.is_empty() {
        b"Unknown error."
    } else {
        detail
    };
    let mut text = Vec::with_capacity(context.len() + 2 + detail.len());
    text.extend_from_slice(context);
    text.extend_from_slice(b"\n\n");
    text.extend_from_slice(detail);
    text
}
