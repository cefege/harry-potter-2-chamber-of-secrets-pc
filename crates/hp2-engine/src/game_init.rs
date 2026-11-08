//! Shared source policy for `UGameEngine::Init`.
//!
//! All command flow is expressed through typed operations in native/Core adapters.
//! The native backend owns its stack temporaries; the Core backend owns equivalent
//! temporary strings and URLs through Rust owner APIs.
use core::ffi::c_void;

/// Keep discriminants in sync with `Hp2GameInitOp` in the native adapter.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InitOp {
    CheckSize,
    BaseInit,
    ClearLevel,
    ClearViewport,
    ClearDrawing,
    CleanFileCache,
    IsClient,
    LoadClientClass,
    ConstructClient,
    InitClient,
    LoadRenderClass,
    ConstructRender,
    InitRender,
    ConstructError,
    HasClient,
    ConstructEntryUrl,
    LoadEntry,
    DestroyEntryUrl,
    FailedEntry,
    ExchangeEntry,
    ConstructDefaultUrl,
    LoadDefaultConfig,
    InitCommandLine,
    ParseToken,
    IsServerToken,
    IsOptionToken,
    CopyDefaultMap,
    ConstructReplayFile,
    ParseReplay,
    StartReplay,
    CopyReplayUrl,
    ParseRecord,
    StartRecord,
    HasProtocol,
    IsOptionUrl,
    ConvertFileToken,
    CopyFileToken,
    ConstructUrl,
    IsValidUrl,
    InvalidUrl,
    Browse,
    IsErrorEmpty,
    IsDefaultMap,
    BrowseFallback,
    FailedBrowse,
    InitInputSystem,
    NewViewport,
    LoadConsoleClass,
    ConstructConsole,
    InitConsole,
    ConstructPlayError,
    SpawnPlayActor,
    FailedPlayActor,
    InitViewportInput,
    OpenWindow,
    DetailChange,
    InitAudio,
    HasAudio,
    SetAudioViewport,
    DestroyPlayError,
    LogInitialized,
    ConsumeLoadSlot,
    MakeSaveDirectory,
    MakeSaveSlotDirectory,
    FormatLoadUrl,
    InvalidLoadSlot,
    LogLoadSlot,
    ServerTravel,
}

/// Checked native leaf ABI retained for the C++ Init adapter.
pub type InitLeaf = unsafe extern "C" fn(*mut c_void, u32, *mut i32) -> i32;

/// Backend-specific checked leaves for the single Init control-flow policy.
pub(crate) trait InitOps {
    type Error;
    fn value(&mut self, op: InitOp) -> Result<i32, Self::Error>;

    fn run(&mut self, op: InitOp) -> Result<(), Self::Error> {
        self.value(op).map(|_| ())
    }
    fn yes(&mut self, op: InitOp) -> Result<bool, Self::Error> {
        self.value(op).map(|value| value != 0)
    }
}

struct NativeCalls {
    context: *mut c_void,
    leaf: InitLeaf,
}
impl InitOps for NativeCalls {
    type Error = i32;
    fn value(&mut self, op: InitOp) -> Result<i32, Self::Error> {
        let mut value = 0;
        let status = unsafe { (self.leaf)(self.context, op as u32, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
}

pub(crate) fn init_with<O: InitOps>(c: &mut O) -> Result<(), O::Error> {
    use InitOp::*;
    {
        c.run(CheckSize)?;
        c.run(BaseInit)?;
        c.run(ClearLevel)?;
        c.run(ClearViewport)?;
        c.run(ClearDrawing)?;
        c.run(CleanFileCache)?;
        if c.yes(IsClient)? {
            c.run(LoadClientClass)?;
            c.run(ConstructClient)?;
            c.run(InitClient)?;
            c.run(LoadRenderClass)?;
            c.run(ConstructRender)?;
            c.run(InitRender)?;
        }
        c.run(ConstructError)?;
        if c.yes(HasClient)? {
            c.run(ConstructEntryUrl)?;
            let loaded = c.yes(LoadEntry)?;
            c.run(DestroyEntryUrl)?;
            if !loaded {
                c.run(FailedEntry)?;
            }
            c.run(ExchangeEntry)?;
        }
        c.run(ConstructDefaultUrl)?;
        c.run(LoadDefaultConfig)?;
        c.run(InitCommandLine)?;
        if !c.yes(ParseToken)?
            || (c.yes(IsServerToken)? && !c.yes(ParseToken)?)
            || c.yes(IsOptionToken)?
        {
            c.run(CopyDefaultMap)?;
        }
        c.run(ConstructReplayFile)?;
        if c.yes(ParseReplay)? {
            if c.yes(StartReplay)? {
                c.run(CopyReplayUrl)?;
            }
        } else if c.yes(ParseRecord)? {
            c.run(StartRecord)?;
        }
        if !c.yes(HasProtocol)? && !c.yes(IsOptionUrl)? && c.yes(ConvertFileToken)? {
            c.run(CopyFileToken)?;
        }
        c.run(ConstructUrl)?;
        if !c.yes(IsValidUrl)? {
            c.run(InvalidUrl)?;
        }
        let mut success = c.yes(Browse)?;
        if !success && c.yes(IsErrorEmpty)? && !c.yes(IsDefaultMap)? {
            success = c.yes(BrowseFallback)?;
        }
        if !success {
            c.run(FailedBrowse)?;
        }
        if c.yes(HasClient)? {
            c.run(InitInputSystem)?;
            c.run(NewViewport)?;
            c.run(LoadConsoleClass)?;
            c.run(ConstructConsole)?;
            c.run(InitConsole)?;
            c.run(ConstructPlayError)?;
            if !c.yes(SpawnPlayActor)? {
                c.run(FailedPlayActor)?;
            }
            c.run(InitViewportInput)?;
            c.run(OpenWindow)?;
            c.run(DetailChange)?;
            c.run(InitAudio)?;
            if c.yes(HasAudio)? {
                c.run(SetAudioViewport)?;
            }
            c.run(DestroyPlayError)?;
        }
        c.run(LogInitialized)?;
        if c.yes(ConsumeLoadSlot)? {
            c.run(MakeSaveDirectory)?;
            c.run(MakeSaveSlotDirectory)?;
            if !c.yes(FormatLoadUrl)? {
                c.run(InvalidLoadSlot)?;
            }
            c.run(LogLoadSlot)?;
            c.run(ServerTravel)?;
        }
    }
    Ok(())
}

/// # Safety
/// `context` must be the live native Init stack context; `leaf` must implement
/// the matching operation ABI and capture every native exception. No callback
/// may retain the context or the out pointer after returning.
pub unsafe fn init(context: *mut c_void, leaf: InitLeaf) -> Result<(), i32> {
    let mut calls = NativeCalls { context, leaf };
    init_with(&mut calls)
}
