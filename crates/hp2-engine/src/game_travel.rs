//! Checked, handle-only policy for the live UnGame.cpp travel paths.
use std::ffi::c_void;

#[repr(C)]
pub struct Hp2GameTravelServices {
    pub call: unsafe extern "C" fn(*mut c_void, u32, usize, usize, *mut usize) -> i32,
    pub scope: unsafe extern "C" fn(*mut c_void, u32, usize, *mut usize) -> i32,
}

/// Each operation is one native SDK primitive or typed access; decisions stay here.
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Op {
    ClearError,
    MapText,
    FindLink,
    TextLen,
    LinkText,
    LogLink,
    ReadLink,
    AssignLink,
    InvalidLinkError,
    LogBrowse,
    UrlValid,
    InvalidUrlError,
    HasOption,
    LoadOption,
    LogAbort,
    Level,
    Entry,
    Brush,
    DeleteBrush,
    ClearBrush,
    ResetLoaders,
    Notify,
    SetLevel,
    ClearAction,
    CheckClient,
    Client,
    ViewportCount,
    Viewport,
    Console,
    ConsoleNotify,
    Audio,
    AudioViewport,
    AudioSetViewport,
    Pending,
    ConnectionProgress,
    HubDepth,
    PopFilename,
    PopToken,
    SavePortal,
    PopUrl,
    RestorePortal,
    Restart,
    Atoi,
    AddLoad,
    LoadSaved,
    CopySource,
    CopyDest,
    CopyFile,
    GameFilename,
    FileSize,
    DeleteFile,
    RepairSlot,
    RepairPending,
    SetRepairPending,
    LastLevelUrl,
    IsLocal,
    LoadLocal,
    IsInternal,
    IsClient,
    CancelPending,
    NewPending,
    SetPending,
    PendingDriver,
    NetworkProgress,
    DeletePending,
    ServerError,
    Launch,
    LogSpawn,
    Spawn,
    SpawnError,
    FullName,
    CompareText,
    LogFix,
    Navigation,
    ObjectName,
    LogFixed,
    TwoWay,
    PlayerStart,
    DisableStart,
    NextNavigation,
    LogLevel,
    CheckEnabled,
    MatchClient,
}

/// Source travel leaves and nested lifetime scopes, without a Native service table.
pub trait TravelOps {
    type Error;

    fn call(&self, op: Op, a: usize, b: usize) -> std::result::Result<usize, Self::Error>;
    fn scope(&self, entry: u32, arg: usize) -> std::result::Result<usize, Self::Error>;
}

struct NativeOps<'a> {
    services: &'a Hp2GameTravelServices,
    context: *mut c_void,
}

impl TravelOps for NativeOps<'_> {
    type Error = i32;

    fn call(&self, op: Op, a: usize, b: usize) -> std::result::Result<usize, i32> {
        let mut out = 0;
        let status = unsafe { (self.services.call)(self.context, op as u32, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }

    fn scope(&self, entry: u32, arg: usize) -> std::result::Result<usize, i32> {
        let mut out = 0;
        let status = unsafe { (self.services.scope)(self.context, entry, arg, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
}

struct Policy<'a, O: TravelOps> {
    ops: &'a O,
}

type TravelResult<O> = std::result::Result<usize, <O as TravelOps>::Error>;

impl<O: TravelOps> Policy<'_, O> {
    fn call(&self, op: Op, a: usize, b: usize) -> TravelResult<O> {
        self.ops.call(op, a, b)
    }
    fn get(&self, op: Op) -> TravelResult<O> {
        self.call(op, 0, 0)
    }
    fn scope(&self, entry: u32, arg: usize) -> TravelResult<O> {
        self.ops.scope(entry, arg)
    }
    fn option(&self, option: usize) -> std::result::Result<bool, O::Error> {
        Ok(self.call(Op::HasOption, option, 0)? != 0)
    }
    fn browse(&self) -> TravelResult<O> {
        self.get(Op::ClearError)?;
        // Preserve first-substring rather than ends_with: repeated .unreal is not a link.
        // wrapping subtraction also retains the original null-result arithmetic without
        // performing Rust pointer subtraction on unrelated/null pointers.
        let found = self.get(Op::FindLink)?;
        let map = self.get(Op::MapText)?;
        let map_len = self.call(Op::TextLen, self.get(Op::MapText)?, 0)?;
        let link_len = self.call(Op::TextLen, self.get(Op::LinkText)?, 0)?;
        if found.wrapping_sub(map) / std::mem::size_of::<u32>() == map_len.wrapping_sub(link_len) {
            self.get(Op::LogLink)?;
            if self.scope(11, 0)? == 0 {
                return Ok(0);
            }
        }
        self.get(Op::LogBrowse)?;
        if self.get(Op::UrlValid)? == 0 {
            return self.scope(2, 0);
        }
        if self.option(0)? || self.option(1)? {
            return self.scope(3, 0);
        }
        if self.option(2)? {
            if self.scope(4, 0)? == 0 {
                return Ok(0);
            }
        } else if self.option(3)? {
            self.scope(5, 0)?;
        } else {
            let option = self.get(Op::LoadOption)?;
            if option != 0 {
                return self.scope(6, option);
            }
        }
        if self.get(Op::IsLocal)? != 0 {
            return self.scope(7, 0);
        }
        if self.get(Op::IsInternal)? != 0 && self.get(Op::IsClient)? != 0 {
            return self.scope(8, 0);
        }
        if self.get(Op::IsInternal)? != 0 {
            self.scope(9, 0)
        } else {
            self.scope(10, 0)
        }
    }
    fn failed(&self) -> TravelResult<O> {
        self.get(Op::LogAbort)?;
        if self.get(Op::Level)? != 0 && self.get(Op::Level)? != self.get(Op::Entry)? {
            if self.call(Op::Brush, self.get(Op::Level)?, 0)? != 0 {
                self.call(Op::DeleteBrush, self.get(Op::Level)?, 0)?;
                self.call(Op::ClearBrush, self.get(Op::Level)?, 0)?;
            }
            self.call(Op::ResetLoaders, self.get(Op::Level)?, 0)?;
        }
        self.get(Op::Notify)?;
        self.call(Op::SetLevel, self.get(Op::Entry)?, 0)?;
        self.call(Op::ClearAction, self.get(Op::Level)?, 0)?;
        if self.get(Op::CheckEnabled)? != 0
            && (self.get(Op::Client)? == 0
                || self.call(Op::ViewportCount, self.get(Op::Client)?, 0)? == 0)
        {
            self.get(Op::CheckClient)?;
        }
        // This scope captures the original helper arguments once; its loop count is live.
        self.scope(14, 0)?;
        if self.get(Op::Audio)? != 0 {
            let audio = self.get(Op::Audio)?;
            let viewport = self.call(Op::AudioViewport, self.get(Op::Audio)?, 0)?;
            self.call(Op::AudioSetViewport, audio, viewport)?;
        }
        if self.option(0)? && self.get(Op::Pending)? == 0 {
            self.get(Op::ConnectionProgress)?;
        }
        Ok(1)
    }
    fn pop(&self) -> TravelResult<O> {
        if self.get(Op::Level)? == 0
            || self.call(Op::HubDepth, self.get(Op::Level)?, 0)? as i32 <= 0
        {
            return Ok(0);
        }
        // Hub depth is signed, and the guard is strictly >0.
        let depth = self.call(Op::HubDepth, self.get(Op::Level)?, 0)? as i32;
        self.call(Op::PopFilename, depth.wrapping_sub(1) as usize, 0)?;
        if self.get(Op::PopToken)? == 0 {
            return Ok(0);
        }
        self.get(Op::SavePortal)?;
        self.get(Op::PopUrl)?;
        self.get(Op::RestorePortal)?;
        Ok(1)
    }
    fn load(&self, slot: usize) -> TravelResult<O> {
        self.get(Op::AddLoad)?;
        if self.get(Op::LoadSaved)? == 0 {
            return Ok(0);
        }
        let mut i: i32 = 0;
        while i < self.call(Op::HubDepth, self.get(Op::Level)?, 0)? as i32 {
            self.call(Op::CopySource, i as usize, 0)?;
            self.call(Op::CopyDest, i as usize, 0)?;
            self.get(Op::CopyFile)?;
            i = i.wrapping_add(1);
        }
        loop {
            let index = i;
            i = i.wrapping_add(1);
            self.call(Op::GameFilename, index as usize, 0)?;
            if self.get(Op::FileSize)? as i32 <= 0 {
                break;
            }
            self.get(Op::DeleteFile)?;
        }
        if self.get(Op::RepairSlot)? as i32 == slot as i32 && self.get(Op::RepairPending)? == 0 {
            self.get(Op::SetRepairPending)?;
        }
        self.get(Op::LastLevelUrl)?;
        Ok(1)
    }
    fn network(&self) -> TravelResult<O> {
        if self.get(Op::Pending)? != 0 {
            self.get(Op::CancelPending)?;
        }
        self.call(Op::SetPending, self.get(Op::NewPending)?, 0)?;
        if self.call(Op::PendingDriver, self.get(Op::Pending)?, 0)? == 0 {
            self.get(Op::NetworkProgress)?;
            self.call(Op::DeletePending, self.get(Op::Pending)?, 0)?;
            self.call(Op::SetPending, 0, 0)?;
        }
        Ok(0)
    }
    fn matching(&self) -> TravelResult<O> {
        let client = self.get(Op::MatchClient)?;
        let mut i: i32 = 0;
        while i < self.call(Op::ViewportCount, client, 0)? as i32 {
            self.scope(13, i as usize)?;
            i = i.wrapping_add(1);
        }
        Ok(0)
    }
    fn notify(&self) -> TravelResult<O> {
        if self.get(Op::Client)? != 0
            && self.call(Op::ViewportCount, self.get(Op::Client)?, 0)? != 0
        {
            let first = self.call(Op::Viewport, self.get(Op::Client)?, 0)?;
            if self.call(Op::Console, first, 0)? != 0 {
                let first = self.call(Op::Viewport, self.get(Op::Client)?, 0)?;
                self.call(Op::ConsoleNotify, self.call(Op::Console, first, 0)?, 0)?;
            }
        }
        Ok(0)
    }
    fn fixup(&self) -> TravelResult<O> {
        if self.call(
            Op::CompareText,
            self.call(Op::FullName, self.get(Op::Level)?, 0)?,
            0,
        )? == 0
        {
            self.get(Op::LogFix)?;
            let mut nav = self.call(Op::Navigation, self.get(Op::Level)?, 0)?;
            while nav != 0 {
                if self.call(Op::CompareText, self.call(Op::ObjectName, nav, 0)?, 1)? == 0
                    || self.call(Op::CompareText, self.call(Op::ObjectName, nav, 0)?, 2)? == 0
                {
                    self.call(Op::LogFixed, nav, 0)?;
                    self.call(Op::TwoWay, nav, 0)?;
                } else if self.call(Op::CompareText, self.call(Op::ObjectName, nav, 0)?, 3)? == 0 {
                    let start = self.call(Op::PlayerStart, nav, 0)?;
                    if start != 0 {
                        self.call(Op::DisableStart, start, 0)?;
                        self.call(Op::LogFixed, nav, 0)?;
                    }
                }
                nav = self.call(Op::NextNavigation, nav, 0)?;
            }
        }
        self.get(Op::LogLevel)?;
        Ok(0)
    }
    fn run(&self, entry: u32, arg: usize) -> TravelResult<O> {
        match entry {
            1 => {
                self.get(Op::InvalidLinkError)?;
                Ok(0)
            }
            2 => {
                self.get(Op::InvalidUrlError)?;
                Ok(0)
            }
            3 => self.failed(),
            4 => self.pop(),
            5 => {
                self.get(Op::Restart)?;
                Ok(0)
            }
            6 => self.scope(12, self.call(Op::Atoi, arg, 0)?),
            7 => Ok((self.get(Op::LoadLocal)? != 0) as usize),
            8 => self.network(),
            9 => {
                self.get(Op::ServerError)?;
                Ok(0)
            }
            10 => {
                self.get(Op::Launch)?;
                Ok(0)
            }
            11 => {
                if self.get(Op::ReadLink)? != 0 {
                    self.get(Op::AssignLink)?;
                    Ok(1)
                } else {
                    self.scope(1, 0)
                }
            }
            12 => self.load(arg),
            13 => {
                let viewport = self.call(Op::Viewport, self.get(Op::MatchClient)?, arg)?;
                self.call(Op::LogSpawn, viewport, 0)?;
                if self.call(Op::Spawn, viewport, 0)? == 0 {
                    self.get(Op::SpawnError)?;
                }
                Ok(0)
            }
            100 => self.browse(),
            101 => self.matching(),
            102 => self.notify(),
            103 => self.fixup(),
            _ => unreachable!("native travel entry contract"),
        }
    }
}

/// Runs the source travel policy using the caller's object and scope providers.
pub fn run_with_ops<O: TravelOps>(ops: &O, entry: u32, arg: usize) -> TravelResult<O> {
    Policy { ops }.run(entry, arg)
}

/// The service table and native stack context must remain live for this call.
/// All object handles are re-read at the original evaluation points.
pub unsafe fn run(
    services: *const Hp2GameTravelServices,
    context: *mut c_void,
    entry: u32,
    arg: usize,
    out: *mut usize,
) -> i32 {
    let ops = NativeOps {
        services: unsafe { &*services },
        context,
    };
    match run_with_ops(&ops, entry, arg) {
        Ok(value) => {
            unsafe {
                *out = value;
            }
            0
        }
        Err(status) => status,
    }
}
