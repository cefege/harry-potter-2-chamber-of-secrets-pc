//! Complete desktop launcher presentation policy. Native objects remain native-owned.
//! Every invocation copies only a callback table. No native reference, lock, UI
//! context lease or mutable model snapshot survives a reentrant SDK callback.
//! Ordinary SDL/ImGui false results are data; checked errors stop immediately.
use crate::import_session::Hp2LauncherImportSessionAction as Import;
use std::ffi::c_void;

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherPresentationPhase {
    Initialize,
    Close,
    Open,
    Frame,
    Result,
    Graphics,
    Resolution,
    Audio,
    Gameplay,
    Play,
    Saves,
    GameData,
    Thumbnail,
    Fonts,
    FontCandidates,
    FontLoading,
    Date,
    DateFormatted,
    Smoke,
    SmokeCapture,
    SmokeSurface,
    RunLauncher,
}

/// Single native SDK operation, field access, or synchronous typed local scope.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherPresentationLeaf {
    Opened,
    Finished,
    Tab,
    SetTab,
    CapsScreenModes,
    CapsResolutions,
    CapsPicker,
    CapsReveal,
    ModeCount,
    ModeWidth,
    ModeHeight,
    ResolutionWidth,
    ResolutionHeight,
    ModeLabelEmpty,
    LabelsReserve,
    AppendModeLabel,
    AppendModeDimensions,
    AppendLabelRef,
    ResolutionCombo,
    SetResolution,
    FullWidth,
    SaveCount,
    SelectedSave,
    SaveCountInt,
    AppendSaveLabel,
    SaveCombo,
    TextSavedGames,
    TextNoSaves,
    TextSelectedDate,
    ValidationEmpty,
    TextValidation,
    Thumbnail,
    ThumbnailDestroy,
    ThumbnailClear,
    ThumbnailCaptureSelected,
    ThumbnailSame,
    ThumbnailPublish,
    ThumbnailPathEmpty,
    ThumbnailLoad,
    ThumbnailCreate,
    ThumbnailFree,
    ThumbnailImage,
    SelectedSource,
    OptionCount,
    OptionSource,
    OptionRootEmpty,
    TextOptionRoot,
    TextOptionUnconfigured,
    OptionAvailable,
    TextAvailable,
    TextUnavailable,
    OptionErrorEmpty,
    TextOptionError,
    LogEmpty,
    RevealUser,
    RevealLog,
    Importing,
    FilesDone,
    FilesTotal,
    Progress,
    DataMessageEmpty,
    TextDataMessage,
    ButtonPadding,
    BeginDisabled,
    EndDisabled,
    Button,
    PopStyle,
    Combo,
    ImportAction,
    VideoInitialized,
    InitVideo,
    OwnVideo,
    OwnsVideo,
    QuitVideo,
    CreateWindow,
    CreateAccelerated,
    CreateSoftware,
    ErrorSdl,
    ErrorText,
    ClearError,
    CheckVersion,
    CreateContext,
    SetContext,
    Context,
    NoIni,
    StyleDark,
    StylePadding,
    SetStylePadding,
    SetTouchPadding,
    InitSdlBackend,
    SetSdlBackend,
    SdlBackend,
    InitRendererBackend,
    SetRendererBackend,
    RendererBackend,
    DestroyContext,
    ShutdownSdl,
    ShutdownRenderer,
    Renderer,
    Window,
    DestroyRenderer,
    DestroyWindow,
    SetOpened,
    RequestErrorEmpty,
    CopyRequestError,
    PollEvent,
    ProcessEvent,
    EventQuit,
    RendererNewFrame,
    SdlNewFrame,
    WindowSize,
    OutputSize,
    Width,
    Height,
    OutputWidth,
    OutputHeight,
    LogicalSize,
    LogLogicalSize,
    FramebufferScale,
    NewFrame,
    WindowPos,
    WindowSizeUi,
    BeginWindow,
    Title,
    Separator,
    BodyY,
    SidebarPosition,
    SidebarItem,
    ContentPosition,
    BeginContent,
    BeginTabs,
    BeginTab,
    EndTab,
    EndTabs,
    EndContent,
    EndWindow,
    Render,
    RenderColor,
    RenderClear,
    RenderData,
    FrameCount,
    Present,
    LogReady,
    SmokeFrames,
    IncrementFrame,
    LogSmoke,
    ResultAddress,
    GetScreenMode,
    SetScreenMode,
    GetRenderScale,
    SetRenderScale,
    GetUiScale,
    SetUiScale,
    GetFps,
    SetFps,
    GetAa,
    SetAa,
    GetAniso,
    SetAniso,
    GetBrightness,
    SetBrightness,
    GetTexture,
    SetTexture,
    GetObject,
    SetObject,
    GetEffects,
    SetEffects,
    GetMusic,
    SetMusic,
    GetSensitivity,
    SetSensitivity,
    GetControls,
    SetControls,
    GetDifficulty,
    SetDifficulty,
    CheckVsync,
    CheckShowFps,
    CheckVerticalFov,
    CheckNativeText,
    CheckSound,
    CheckInvert,
    CheckJoystick,
    CheckCenter,
    CheckCasting,
    CheckQuaff,
    CheckFlashes,
    SoundEnabled,
    SliderBrightness,
    SliderEffects,
    SliderMusic,
    SliderSensitivity,
    FontBase,
    FontExecutableBase,
    FontExecutableCurrent,
    FontFreeBase,
    FontCandidateExists,
    FontPublish,
    FontLatinPath,
    FontLatinExists,
    FontLatinLoad,
    FontCjkPath,
    FontCjkExists,
    FontCjkLoad,
    FontBuild,
    FontMissing,
    DateSeconds,
    DateUnavailable,
    DateFormat,
    DateCount,
    DatePublish,
    SmokeEnvironment,
    SmokeEnvironmentNonempty,
    SmokeSize,
    SmokeWidth,
    SmokeHeight,
    SmokePixels,
    SmokeRead,
    SmokeSurface,
    SmokeSave,
    SmokeFree,
    SmokeEnvironmentParse,
    SmokeParse,
    SmokeParsedAny,
    SmokeParsedEnd,
    SmokePublish,
    ScopeResolution,
    ScopeSaves,
    ScopeThumbnail,
    ScopeFonts,
    ScopeSmoke,
    SaveBytes,
    RunOpen,
    RunFrame,
    RunDelay,
    RunResult,
    HasSave,
    FontHasBase,
    ScopeFontCandidates,
    ScopeFontLoading,
    ScopeSmokeCapture,
    ScopeSmokeSurface,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2LauncherPresentationValue {
    pub integer: i64,
    pub real: f64,
}

/// Both strings are immutable NUL-terminated literals valid during the call.
/// Combo choices are double-NUL terminated. The native frame owns all typed
/// vectors, strings, fonts and SDK objects for exactly the synchronous scope.
/// Each callback contains C++ exceptions in its same-thread first-error slot;
/// the native caller rethrows only after Rust returns. No compensation on error.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherPresentationServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2LauncherPresentationLeaf,
        usize,
        i64,
        f64,
        *const u8,
        *const u8,
        *mut Hp2LauncherPresentationValue,
    ) -> i32,
}
use Hp2LauncherPresentationLeaf as L;
use Hp2LauncherPresentationPhase as P;
type R<T> = Result<T, i32>;
impl Hp2LauncherPresentationServices {
    fn invoke(
        &self,
        leaf: L,
        index: usize,
        integer: i64,
        real: f64,
        text: &'static [u8],
        choices: &'static [u8],
    ) -> R<Hp2LauncherPresentationValue> {
        let mut output = Hp2LauncherPresentationValue::default();
        let status = unsafe {
            (self.call)(
                self.context,
                leaf,
                index,
                integer,
                real,
                text.as_ptr(),
                choices.as_ptr(),
                &mut output,
            )
        };
        if status == 0 { Ok(output) } else { Err(status) }
    }
    fn at(&self, leaf: L, index: usize, value: i64) -> R<i64> {
        Ok(self
            .invoke(leaf, index, value, 0.0, b"\0", b"\0\0")?
            .integer)
    }
    fn get(&self, leaf: L) -> R<i64> {
        self.at(leaf, 0, 0)
    }
    fn yes(&self, leaf: L) -> R<bool> {
        Ok(self.get(leaf)? != 0)
    }
    fn put(&self, leaf: L, value: i64) -> R<()> {
        self.at(leaf, 0, value).map(|_| ())
    }
    fn do_(&self, leaf: L) -> R<()> {
        self.put(leaf, 0)
    }
    fn real(&self, leaf: L) -> R<f64> {
        Ok(self.invoke(leaf, 0, 0, 0.0, b"\0", b"\0\0")?.real)
    }
    fn set_real(&self, leaf: L, value: f64) -> R<()> {
        self.invoke(leaf, 0, 0, value, b"\0", b"\0\0").map(|_| ())
    }
    fn text(&self, leaf: L, text: &'static [u8]) -> R<()> {
        self.invoke(leaf, 0, 0, 0.0, text, b"\0\0").map(|_| ())
    }
    fn import(&self, action: Import) -> R<()> {
        self.put(L::ImportAction, action as i64)
    }
    fn combo(&self, label: &'static [u8], choices: &'static [u8], current: i64) -> R<i64> {
        self.do_(L::FullWidth)?;
        Ok(self
            .invoke(L::Combo, 0, current, 0.0, label, choices)?
            .integer)
    }
    fn button(&self, label: &'static [u8], enabled: bool) -> R<bool> {
        self.do_(L::ButtonPadding)?;
        if !enabled {
            self.put(L::BeginDisabled, 1)?;
        }
        let pressed = self.invoke(L::Button, 0, 0, 0.0, label, b"\0\0")?.integer != 0;
        if !enabled {
            self.do_(L::EndDisabled)?;
        }
        self.do_(L::PopStyle)?;
        Ok(pressed && enabled)
    }
    fn has_save(&self) -> R<bool> {
        // Reuse the exact existing import-session query via native checked leaf.
        self.yes(L::HasSave)
    }
}

fn integer_combo(
    s: &Hp2LauncherPresentationServices,
    get: L,
    set: L,
    label: &'static [u8],
    choices: &'static [u8],
    values: &[i32],
) -> R<()> {
    let mut current = 0;
    for (i, value) in values.iter().enumerate() {
        if s.get(get)? == i64::from(*value) {
            current = i as i64;
        }
    }
    let selected = s.combo(label, choices, current)?;
    if selected >= 0 {
        s.put(set, i64::from(values[selected as usize]))?;
    }
    Ok(())
}
fn scale_combo(
    s: &Hp2LauncherPresentationServices,
    get: L,
    set: L,
    label: &'static [u8],
    choices: &'static [u8],
    values: &[f64],
    default: i64,
) -> R<()> {
    let mut current = default;
    for (i, value) in values.iter().enumerate() {
        if (s.real(get)? - *value).abs() < 0.005 {
            current = i as i64;
        }
    }
    let selected = s.combo(label, choices, current)?;
    if selected >= 0 {
        s.set_real(set, values[selected as usize])?;
    }
    Ok(())
}
fn enum_combo(
    s: &Hp2LauncherPresentationServices,
    get: L,
    set: L,
    label: &'static [u8],
    choices: &'static [u8],
) -> R<()> {
    let selected = s.combo(label, choices, s.get(get)?)?;
    if selected >= 0 {
        s.put(set, selected)?;
    }
    Ok(())
}
fn slider(s: &Hp2LauncherPresentationServices, get: L, ui: L, set: L) -> R<()> {
    // Preserve the native double -> float temporary and changed-only assignment.
    let value = s.invoke(ui, 0, 0, s.real(get)? as f32 as f64, b"\0", b"\0\0")?;
    if value.integer != 0 {
        s.set_real(set, value.real)?;
    }
    Ok(())
}
fn graphics(s: &Hp2LauncherPresentationServices) -> R<()> {
    if s.yes(L::CapsScreenModes)? {
        enum_combo(
            s,
            L::GetScreenMode,
            L::SetScreenMode,
            b"Screen mode\0",
            b"Windowed\0Fullscreen\0Borderless desktop\0\0",
        )?;
    }
    if s.yes(L::CapsResolutions)? && s.get(L::ModeCount)? != 0 {
        s.do_(L::ScopeResolution)?;
    }
    s.do_(L::CheckVsync)?;
    scale_combo(
        s,
        L::GetRenderScale,
        L::SetRenderScale,
        b"Render scale\0",
        b"50%\067%\075%\085%\0100%\0\0",
        &crate::model::RENDER_SCALE_VALUES,
        4,
    )?;
    scale_combo(
        s,
        L::GetUiScale,
        L::SetUiScale,
        b"UI scale\0",
        b"75%\0100%\0125%\0150%\0175%\0200%\0\0",
        &crate::model::UI_SCALE_VALUES,
        1,
    )?;
    integer_combo(
        s,
        L::GetFps,
        L::SetFps,
        b"FPS cap\0",
        b"Unlimited\030\060\0120\0144\0\0",
        &crate::model::FRAME_RATE_LIMIT_VALUES,
    )?;
    s.do_(L::CheckShowFps)?;
    s.do_(L::CheckVerticalFov)?;
    s.do_(L::CheckNativeText)?;
    integer_combo(
        s,
        L::GetAa,
        L::SetAa,
        b"Anti-aliasing\0",
        b"Off\02x\04x\0\0",
        &crate::model::ANTI_ALIASING_SAMPLE_VALUES,
    )?;
    integer_combo(
        s,
        L::GetAniso,
        L::SetAniso,
        b"Anisotropy\0",
        b"Off\04x\08x\016x\0\0",
        &crate::model::ANISOTROPY_VALUES,
    )?;
    slider(s, L::GetBrightness, L::SliderBrightness, L::SetBrightness)?;
    enum_combo(
        s,
        L::GetTexture,
        L::SetTexture,
        b"Texture detail\0",
        b"Low\0Medium\0High\0\0",
    )?;
    enum_combo(
        s,
        L::GetObject,
        L::SetObject,
        b"Object detail\0",
        b"Very low\0Low\0Medium\0High\0Very high\0\0",
    )
}
fn resolution(s: &Hp2LauncherPresentationServices) -> R<()> {
    let mut current = 0;
    let mut i = 0;
    while i < s.get(L::ModeCount)? as usize {
        if s.at(L::ModeWidth, i, 0)? == s.get(L::ResolutionWidth)?
            && s.at(L::ModeHeight, i, 0)? == s.get(L::ResolutionHeight)?
        {
            current = i as i64;
        }
        i += 1;
    }
    let count = s.get(L::ModeCount)? as usize;
    s.at(L::LabelsReserve, count, 0)?;
    for i in 0..count {
        let leaf = if s.at(L::ModeLabelEmpty, i, 0)? != 0 {
            L::AppendModeDimensions
        } else {
            L::AppendModeLabel
        };
        s.at(leaf, i, 0)?;
    }
    for i in 0..count {
        s.at(L::AppendLabelRef, i, 0)?;
    }
    s.do_(L::FullWidth)?;
    let selected = s.at(L::ResolutionCombo, 0, current)?;
    if selected >= 0 {
        s.at(L::SetResolution, selected as usize, 0)?;
    }
    Ok(())
}
fn audio(s: &Hp2LauncherPresentationServices) -> R<()> {
    s.do_(L::CheckSound)?;
    s.put(L::BeginDisabled, i64::from(!s.yes(L::SoundEnabled)?))?;
    slider(s, L::GetEffects, L::SliderEffects, L::SetEffects)?;
    slider(s, L::GetMusic, L::SliderMusic, L::SetMusic)?;
    s.do_(L::EndDisabled)
}
fn gameplay(s: &Hp2LauncherPresentationServices) -> R<()> {
    slider(
        s,
        L::GetSensitivity,
        L::SliderSensitivity,
        L::SetSensitivity,
    )?;
    s.do_(L::CheckInvert)?;
    enum_combo(
        s,
        L::GetControls,
        L::SetControls,
        b"Controls mode\0",
        b"Classic\0Modern\0\0",
    )?;
    s.do_(L::CheckJoystick)?;
    s.do_(L::CheckCenter)?;
    s.do_(L::CheckCasting)?;
    s.do_(L::CheckQuaff)?;
    s.do_(L::CheckFlashes)?;
    enum_combo(
        s,
        L::GetDifficulty,
        L::SetDifficulty,
        b"Difficulty\0",
        b"Easy\0Medium\0Hard\0\0",
    )
}
fn play(s: &Hp2LauncherPresentationServices) -> R<()> {
    s.do_(L::TextSavedGames)?;
    if s.get(L::SaveCount)? == 0 {
        s.do_(L::TextNoSaves)?;
    } else {
        s.do_(L::ScopeSaves)?;
    }
    if !s.yes(L::ValidationEmpty)? {
        s.do_(L::TextValidation)?;
    }
    if s.button(b"Continue\0", s.has_save()?)? {
        s.import(Import::CompleteContinue)?;
    }
    if s.button(b"New Game\0", true)? {
        s.import(Import::CompleteNewGame)?;
    }
    if s.button(b"Quit\0", true)? {
        s.import(Import::CompleteQuit)?;
    }
    Ok(())
}
fn saves(s: &Hp2LauncherPresentationServices) -> R<()> {
    let count = s.get(L::SaveCount)? as usize;
    s.at(L::LabelsReserve, count, 0)?;
    for i in 0..count {
        s.at(L::AppendSaveLabel, i, s.at(L::SaveBytes, i, 0)? / 1024)?;
    }
    for i in 0..count {
        s.at(L::AppendLabelRef, i, 0)?;
    }
    s.import(Import::PrepareSavePicker)?;
    s.do_(L::FullWidth)?;
    s.do_(L::SaveCombo)?;
    s.do_(L::ScopeThumbnail)?;
    if s.yes(L::Thumbnail)? {
        s.do_(L::ThumbnailImage)?;
    }
    if s.has_save()? {
        s.do_(L::TextSelectedDate)?;
    }
    Ok(())
}
fn thumbnail(s: &Hp2LauncherPresentationServices) -> R<()> {
    if s.get(L::SelectedSave)? >= 0 && s.get(L::SelectedSave)? < s.get(L::SaveCountInt)? {
        s.do_(L::ThumbnailCaptureSelected)?;
    }
    if s.yes(L::ThumbnailSame)? {
        return Ok(());
    }
    if s.yes(L::Thumbnail)? {
        s.do_(L::ThumbnailDestroy)?;
        s.do_(L::ThumbnailClear)?;
    }
    s.do_(L::ThumbnailPublish)?;
    if !s.yes(L::ThumbnailPathEmpty)? && s.yes(L::ThumbnailLoad)? {
        s.do_(L::ThumbnailCreate)?;
        s.do_(L::ThumbnailFree)?;
    }
    Ok(())
}
fn game_data(s: &Hp2LauncherPresentationServices) -> R<()> {
    let selected = s.combo(
        b"Data source\0",
        b"Retail game data\0Prototype data\0\0",
        s.get(L::SelectedSource)?,
    )?;
    if selected >= 0 {
        s.import(if selected == 0 {
            Import::SelectRetail
        } else {
            Import::SelectPrototype
        })?;
    }
    // FindOption returns the first match; read the selected source AFTER combo.
    let selected = s.get(L::SelectedSource)?;
    let count = s.get(L::OptionCount)? as usize;
    for i in 0..count {
        if s.at(L::OptionSource, i, 0)? != selected {
            continue;
        }
        let root = if s.at(L::OptionRootEmpty, i, 0)? != 0 {
            L::TextOptionUnconfigured
        } else {
            L::TextOptionRoot
        };
        s.at(root, i, 0)?;
        let available = if s.at(L::OptionAvailable, i, 0)? != 0 {
            L::TextAvailable
        } else {
            L::TextUnavailable
        };
        s.do_(available)?;
        if s.at(L::OptionErrorEmpty, i, 0)? == 0 {
            s.at(L::TextOptionError, i, 0)?;
        }
        break;
    }
    if s.yes(L::CapsPicker)? {
        if s.button("Choose prototype folder…\0".as_bytes(), true)? {
            s.import(Import::PickPrototypeFolder)?;
        }
        if s.button("Import retail game…\0".as_bytes(), true)? {
            s.import(Import::PickRetailFolder)?;
        }
        if s.button("Import retail archive…\0".as_bytes(), true)? {
            s.import(Import::PickRetailArchive)?;
        }
    }
    if s.yes(L::CapsReveal)? {
        if s.button(b"Open user folder\0", true)? {
            s.do_(L::RevealUser)?;
        }
        if s.button(b"Show log\0", !s.yes(L::LogEmpty)?)? {
            s.do_(L::RevealLog)?;
        }
    }
    if s.yes(L::Importing)? {
        let done = s.get(L::FilesDone)?;
        let total = s.get(L::FilesTotal)?;
        let fraction = if total > 0 {
            (done as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let text = if total > 0 {
            b"Importing files\0".as_slice()
        } else {
            "Preparing import…\0".as_bytes()
        };
        s.invoke(L::Progress, 0, 0, fraction as f64, text, b"\0\0")?;
        if s.button(b"Cancel import\0", true)? {
            s.import(Import::Cancel)?;
        }
    }
    if !s.yes(L::DataMessageEmpty)? {
        s.do_(L::TextDataMessage)?;
    }
    Ok(())
}

fn fonts(s: &Hp2LauncherPresentationServices) -> R<()> {
    if s.yes(L::FontBase)? {
        s.do_(L::FontExecutableBase)?;
    } else {
        s.do_(L::FontExecutableCurrent)?;
    }
    if s.yes(L::FontHasBase)? {
        s.do_(L::FontFreeBase)?;
    }
    s.do_(L::ScopeFontCandidates)
}
fn font_candidates(s: &Hp2LauncherPresentationServices) -> R<()> {
    let mut selected = 0;
    for i in 0..3 {
        if s.at(L::FontCandidateExists, i, 0)? != 0 {
            selected = i;
            break;
        }
    }
    s.at(L::FontPublish, selected, 0).map(|_| ())
}
fn font_loading(s: &Hp2LauncherPresentationServices) -> R<()> {
    s.do_(L::FontLatinPath)?;
    if s.yes(L::FontLatinExists)? {
        let base = s.yes(L::FontLatinLoad)?;
        s.do_(L::FontCjkPath)?;
        if base && s.yes(L::FontCjkExists)? {
            s.do_(L::FontCjkLoad)?;
        }
        s.do_(L::FontBuild)?;
    } else {
        s.do_(L::FontMissing)?;
    }
    Ok(())
}
fn open(s: &Hp2LauncherPresentationServices) -> R<bool> {
    if !s.yes(L::VideoInitialized)? {
        if s.get(L::InitVideo)? != 0 {
            s.do_(L::ErrorSdl)?;
            return Ok(false);
        }
        s.do_(L::OwnVideo)?;
    }
    if !s.yes(L::CreateWindow)? {
        s.do_(L::ErrorSdl)?;
        return Ok(false);
    }
    if !s.yes(L::CreateAccelerated)? && !s.yes(L::CreateSoftware)? {
        s.do_(L::ErrorSdl)?;
        return Ok(false);
    }
    s.do_(L::CheckVersion)?;
    s.do_(L::CreateContext)?;
    s.put(L::SetContext, 1)?;
    s.do_(L::NoIni)?;
    s.do_(L::StyleDark)?;
    let padding = s.real(L::StylePadding)? as f32;
    // std::max(a,b) retains a for NaN; f32::max would not.
    s.set_real(
        L::SetStylePadding,
        if padding < 16.0 { 16.0 } else { padding as f64 },
    )?;
    s.do_(L::SetTouchPadding)?;
    if !s.yes(L::InitSdlBackend)? {
        s.text(
            L::ErrorText,
            b"Could not initialize Dear ImGui SDL input.\0",
        )?;
        s.do_(L::DestroyContext)?;
        s.put(L::SetContext, 0)?;
        return Ok(false);
    }
    s.put(L::SetSdlBackend, 1)?;
    if !s.yes(L::InitRendererBackend)? {
        s.text(
            L::ErrorText,
            b"Could not initialize Dear ImGui SDL renderer.\0",
        )?;
        s.do_(L::ShutdownSdl)?;
        s.put(L::SetSdlBackend, 0)?;
        s.do_(L::DestroyContext)?;
        s.put(L::SetContext, 0)?;
        return Ok(false);
    }
    s.put(L::SetRendererBackend, 1)?;
    s.do_(L::ScopeFonts)?;
    s.yes(L::ScopeFontLoading)
}
fn close(s: &Hp2LauncherPresentationServices) -> R<()> {
    s.import(Import::Close)?;
    if s.yes(L::Thumbnail)? {
        s.do_(L::ThumbnailDestroy)?;
    }
    if s.yes(L::RendererBackend)? {
        s.do_(L::ShutdownRenderer)?;
    }
    if s.yes(L::SdlBackend)? {
        s.do_(L::ShutdownSdl)?;
    }
    if s.yes(L::Context)? {
        s.do_(L::DestroyContext)?;
    }
    if s.yes(L::Renderer)? {
        s.do_(L::DestroyRenderer)?;
    }
    if s.yes(L::Window)? {
        s.do_(L::DestroyWindow)?;
    }
    if s.yes(L::OwnsVideo)? {
        s.do_(L::QuitVideo)?;
    }
    Ok(())
}
fn frame(s: &Hp2LauncherPresentationServices) -> R<bool> {
    if !s.yes(L::Opened)? || s.yes(L::Finished)? {
        return Ok(false);
    }
    while s.yes(L::PollEvent)? {
        s.do_(L::ProcessEvent)?;
        if s.yes(L::EventQuit)? {
            s.import(Import::CompleteQuit)?;
        }
    }
    // Quit still finishes this frame and drains every queued event.
    s.import(Import::FinishIfReady)?;
    s.do_(L::RendererNewFrame)?;
    s.do_(L::SdlNewFrame)?;
    s.do_(L::WindowSize)?;
    s.do_(L::OutputSize)?;
    if s.get(L::Width)? > 0
        && s.get(L::Height)? > 0
        && s.get(L::OutputWidth)? > 0
        && s.get(L::OutputHeight)? > 0
    {
        if s.get(L::LogicalSize)? != 0 {
            s.do_(L::LogLogicalSize)?;
        }
        s.do_(L::FramebufferScale)?;
    }
    s.do_(L::NewFrame)?;
    s.do_(L::WindowPos)?;
    s.do_(L::WindowSizeUi)?;
    s.do_(L::BeginWindow)?;
    s.do_(L::Title)?;
    s.do_(L::Separator)?;
    const TABS: [&[u8]; 5] = [
        b"Play\0",
        b"Graphics\0",
        b"Audio\0",
        b"Gameplay\0",
        b"Game Data\0",
    ];
    if s.get(L::Width)? >= 800 {
        let body = s.real(L::BodyY)? as f32;
        for (i, label) in TABS.iter().enumerate() {
            s.set_real(L::SidebarPosition, (body + i as f32 * 44.0) as f64)?;
            let selected = s.get(L::Tab)? == i as i64;
            if s.invoke(L::SidebarItem, 0, i64::from(selected), 0.0, label, b"\0\0")?
                .integer
                != 0
            {
                s.put(L::SetTab, i as i64)?;
            }
        }
        s.set_real(L::ContentPosition, body as f64)?;
        s.do_(L::BeginContent)?;
    } else {
        // Original intentionally ignores BeginTabBar's bool.
        s.do_(L::BeginTabs)?;
        for (i, label) in TABS.iter().enumerate() {
            let selected = s.get(L::Tab)? == i as i64;
            if s.invoke(L::BeginTab, 0, i64::from(selected), 0.0, label, b"\0\0")?
                .integer
                != 0
            {
                s.put(L::SetTab, i as i64)?;
                s.do_(L::EndTab)?;
            }
        }
        s.do_(L::EndTabs)?;
        s.do_(L::BeginContent)?;
    }
    match s.get(L::Tab)? {
        0 => play(s)?,
        1 => graphics(s)?,
        2 => audio(s)?,
        3 => gameplay(s)?,
        4 => game_data(s)?,
        _ => {}
    }
    s.do_(L::EndContent)?;
    s.do_(L::EndWindow)?;
    s.do_(L::Render)?;
    s.do_(L::RenderColor)?;
    s.do_(L::RenderClear)?;
    s.do_(L::RenderData)?;
    let first = s.get(L::FrameCount)? == 0;
    s.do_(L::ScopeSmoke)?;
    s.do_(L::Present)?;
    if first {
        s.do_(L::LogReady)?;
    }
    Ok(!s.yes(L::Finished)?)
}
fn smoke(s: &Hp2LauncherPresentationServices) -> R<()> {
    if s.get(L::SmokeFrames)? < 0 || s.get(L::IncrementFrame)? < s.get(L::SmokeFrames)? {
        return Ok(());
    }
    s.do_(L::ScopeSmokeCapture)?;
    s.do_(L::LogSmoke)?;
    s.import(Import::CompleteQuit)
}
fn smoke_capture(s: &Hp2LauncherPresentationServices) -> R<()> {
    if s.yes(L::SmokeEnvironment)? && s.yes(L::SmokeEnvironmentNonempty)? {
        s.do_(L::SmokeSize)?;
        let width = s.get(L::SmokeWidth)?;
        let height = s.get(L::SmokeHeight)?;
        if width > 0 && height > 0 {
            // Same size_t multiplication as the original native vector size.
            s.at(
                L::SmokePixels,
                (width as usize)
                    .wrapping_mul(height as usize)
                    .wrapping_mul(4),
                0,
            )?;
            if s.get(L::SmokeRead)? == 0 && s.yes(L::SmokeSurface)? {
                s.do_(L::ScopeSmokeSurface)?;
            }
        }
    }
    Ok(())
}

/// # Safety
/// The copied native service table satisfies its checked synchronous scope
/// contract. Each invocation (including nested SDK callbacks) has its own frame.
pub unsafe fn execute(s: &Hp2LauncherPresentationServices, phase: P) -> R<i64> {
    match phase {
        P::Initialize => {
            if s.yes(L::SmokeEnvironmentParse)? {
                let parsed = s.get(L::SmokeParse)?;
                if s.yes(L::SmokeParsedAny)?
                    && s.yes(L::SmokeParsedEnd)?
                    && parsed > 0
                    && parsed < 100000
                {
                    s.put(L::SmokePublish, parsed)?;
                }
            }
        }
        P::Close => close(s)?,
        P::Open => return open(s).map(i64::from),
        P::Frame => return frame(s).map(i64::from),
        P::Result => return s.get(L::ResultAddress),
        P::Graphics => graphics(s)?,
        P::Resolution => resolution(s)?,
        P::Audio => audio(s)?,
        P::Gameplay => gameplay(s)?,
        P::Play => play(s)?,
        P::Saves => saves(s)?,
        P::GameData => game_data(s)?,
        P::Thumbnail => thumbnail(s)?,
        P::Fonts => fonts(s)?,
        P::FontCandidates => font_candidates(s)?,
        P::FontLoading => {
            font_loading(s)?;
            s.do_(L::SetOpened)?;
            if !s.yes(L::RequestErrorEmpty)? {
                s.do_(L::CopyRequestError)?;
            }
            s.do_(L::ClearError)?;
            return Ok(1);
        }
        P::Date => {
            if s.get(L::DateSeconds)? <= 0 {
                s.do_(L::DateUnavailable)?;
            } else {
                s.do_(L::DateFormat)?;
            }
        }
        P::DateFormatted => {
            let leaf = if s.get(L::DateCount)? == 0 {
                L::DateUnavailable
            } else {
                L::DatePublish
            };
            s.do_(leaf)?;
        }
        P::Smoke => smoke(s)?,
        P::SmokeCapture => smoke_capture(s)?,
        P::SmokeSurface => {
            s.do_(L::SmokeSave)?;
            s.do_(L::SmokeFree)?;
        }
        P::RunLauncher => {
            if !s.yes(L::RunOpen)? {
                return Ok(0);
            }
            while s.yes(L::RunFrame)? {
                s.do_(L::RunDelay)?;
            }
            s.do_(L::RunResult)?;
            s.do_(L::ClearError)?;
            return Ok(1);
        }
    }
    Ok(0)
}
