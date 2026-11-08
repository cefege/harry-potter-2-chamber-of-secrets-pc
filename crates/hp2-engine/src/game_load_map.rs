//! Shared checked source policy for `UGameEngine::LoadMap`.
//! Native and Core owners supply the same typed operations and lifetime scopes.
use std::ffi::c_void;

/// Preserve native raw-tick subtraction before binary32 fade arithmetic.
pub fn fade_fog(current: i64, previous: i64, duration: f32, fog: f32) -> f32 {
    fog - hp2_core::time::subtract(current, previous) / duration
}

#[repr(C)]
pub struct Hp2GameLoadMapServices {
    pub call: unsafe extern "C" fn(*mut c_void, u32, usize, usize, *mut usize) -> i32,
    pub scope: unsafe extern "C" fn(*mut c_void, u32, usize, *mut usize) -> i32,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Op {
    ClearError,
    Level,
    DumpAllocs,
    LogMap,
    TraceMap,
    ResetGuard,
    Hub,
    SaveHub,
    SavedHub,
    Client,
    ViewportCount,
    Viewport,
    Pauser,
    ViewportActor,
    HideMenu,
    ClearAction,
    FadePositive,
    DrewWorld,
    FogPositive,
    Draw,
    FadeStep,
    ClearFog,
    Audio,
    AudioViewport,
    AudioSetViewport,
    Pending,
    PackageMap,
    BeginLoad,
    PackageCount,
    PackageLinker,
    VerifyLinker,
    MapParent,
    VerifyMap,
    EndLoad,
    DemoKind,
    DemoDriver,
    DemoSuffix,
    DemoAppend,
    DemoSize,
    DemoLog,
    DemoName,
    DemoFileSize,
    DemoError,
    Progress,
    CatchError,
    RestoreFog,
    RestoreGlow,
    RestoreFade,
    Notify,
    ActorLevel,
    ClearPlayer,
    ClearViewportActor,
    DestroyActor,
    ResetLoaders,
    Brush,
    DeleteBrush,
    ClearBrush,
    NetDriver,
    DeleteNet,
    ClearNet,
    Demo,
    DeleteDemo,
    ClearDemo,
    Option,
    CleanupDestroyed,
    SaveFilename,
    SavePackage,
    ClearLevel,
    LoadInfo,
    EnterTextEmpty,
    EnterText,
    ViewportLock,
    EndFlash,
    Console,
    Canvas,
    DrawInfo,
    Unlock,
    LoadLevel,
    HasSlotDump,
    SlotDump,
    LonePlayer,
    ClearPending,
    ComputePackages,
    GenerationMismatch,
    LogHave,
    VerifyClasses,
    CheckLevel,
    GetInfo,
    ComputerName,
    InfoHub,
    SetHub,
    CheckPending,
    AttachNet,
    NotifyNet,
    AttachDemo,
    NotifyDemo,
    ClientMode,
    CheckNoNet,
    IsEntry,
    Persistent,
    LoadOption,
    AssignUrl,
    Version,
    MinVersion,
    SetEngine,
    Travel,
    AssignTravel,
    Flush,
    IteratorValid,
    IteratorActor,
    IteratorNext,
    InLevel,
    Eliminate,
    KeepActor,
    ActorCount,
    Actor,
    Gc,
    AudioCleanup,
    Collision,
    Connectivity,
    ZoneDistances,
    UpdateTime,
    OptionCount,
    AppendQuestion,
    AppendOption,
    ParseGame,
    IsServer,
    Game,
    GameName,
    DefaultGame,
    LoadDefaultGame,
    DefaultGameName,
    DefaultServerGameName,
    GameNamePointer,
    LogGame,
    SpawnGame,
    CheckGame,
    EnginePending,
    CheckNotPending,
    DeletePending,
    ClearEnginePending,
    Listen,
    ListenError,
    HighDetail,
    RenderDevice,
    DeviceDetail,
    FirstDynamic,
    BegunPlay,
    Fixup,
    LogBringUp,
    ClearTime,
    ClearInfoTime,
    TouchCount,
    ClearTouch,
    Static,
    NoDelete,
    ExchangeRoles,
    InitExecution,
    SetBegun,
    Startup,
    InitGame,
    ScriptInitialized,
    PreBefore,
    PreBegin,
    PreAfter,
    BeginPlay,
    SetZone,
    PostBegin,
    InitialState,
    HasAttach,
    AttachMatches,
    SetBase,
    Base,
    CollideWorld,
    Decoration,
    Inventory,
    Pawn,
    PhysicsNone,
    PhysicsRotating,
    FindBase,
    Info,
    ClearBase,
    SyncTime,
    Lifecycle,
    AddActor,
    Relevant,
    ReorderedCount,
    FirstRelevant,
    EmptyActors,
    AllocateActors,
    CopyActor,
    ProfileEnabled,
    FunctionValid,
    FunctionNext,
    ClearProfile,
    ResetTicks,
    Entry,
    MatchViewports,
    NewBrush,
    ResetInput,
    ViewportDevice,
    FlushDevice,
    ScreenActors,
    DetailChange,
    LastUrl,
    IsClient,
    SaveConfig,
    LogTiming,
}
type Result = std::result::Result<usize, i32>;

pub(crate) trait LoadMapOps {
    type Error;
    fn call(&self, op: Op, a: usize, b: usize) -> std::result::Result<usize, Self::Error>;
    fn scope(&self, entry: u32, arg: usize) -> std::result::Result<usize, Self::Error>;
    fn invalid_entry(&self) -> std::result::Result<usize, Self::Error>;
}

struct Policy<O> {
    ops: O,
}
impl<O: LoadMapOps> Policy<O> {
    fn call(&self, op: Op, a: usize, b: usize) -> std::result::Result<usize, O::Error> {
        self.ops.call(op, a, b)
    }
    fn get(&self, op: Op) -> std::result::Result<usize, O::Error> {
        self.call(op, 0, 0)
    }
    fn one(&self, op: Op, a: usize) -> std::result::Result<usize, O::Error> {
        self.call(op, a, 0)
    }
    fn scope(&self, entry: u32, arg: usize) -> std::result::Result<usize, O::Error> {
        self.ops.scope(entry, arg)
    }
    fn invalid_entry(&self) -> std::result::Result<usize, O::Error> {
        self.ops.invalid_entry()
    }
    fn audio(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Audio)? != 0 {
            // Both original member reads precede the GetViewport callback.
            let audio = self.get(Op::Audio)?;
            let viewport = self.one(Op::AudioViewport, self.get(Op::Audio)?)?;
            self.call(Op::AudioSetViewport, audio, viewport)?;
        }
        Ok(0)
    }
    fn root(&self) -> std::result::Result<usize, O::Error> {
        self.get(Op::ClearError)?;
        if self.get(Op::Level)? != 0 {
            self.get(Op::DumpAllocs)?;
        }
        self.get(Op::LogMap)?;
        self.scope(32, 0)
    }
    fn timed_root(&self) -> std::result::Result<usize, O::Error> {
        self.get(Op::TraceMap)?;
        self.get(Op::ResetGuard)?;
        let hub = if self.get(Op::Level)? != 0 {
            self.one(Op::Hub, self.get(Op::Level)?)?
        } else {
            0
        };
        self.one(Op::SaveHub, hub)?;
        self.scope(1, 0)?;
        if self.get(Op::Pending)? != 0 {
            self.get(Op::PackageMap)?;
        }
        if self.scope(2, 0)? == 0 {
            return Ok(0);
        }
        self.scope(4, 0)?;
        self.scope(5, 0)?;
        self.scope(6, 0)?;
        if self.get(Op::LoadInfo)? != 0 {
            if self.get(Op::EnterTextEmpty)? != 0 {
                self.get(Op::EnterText)?;
            }
            if self.get(Op::Client)? != 0 && self.get(Op::ViewportCount)? != 0 {
                let viewport = self.one(Op::Viewport, 0)?;
                if self.one(Op::ViewportLock, viewport)? != 0 {
                    self.one(Op::EndFlash, viewport)?;
                    if self.one(Op::Console, viewport)? != 0 && self.one(Op::Canvas, viewport)? != 0
                    {
                        self.one(Op::DrawInfo, viewport)?;
                    }
                    self.one(Op::Unlock, viewport)?;
                }
            }
        }
        self.scope(8, 0)?;
        self.scope(9, 0)
    }
    fn loading_screen(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Client)? != 0
            && self.get(Op::ViewportCount)? != 0
            && self.get(Op::Level)? != 0
        {
            self.get(Op::Pauser)?;
            let pp = self.one(Op::ViewportActor, self.one(Op::Viewport, 0)?)?;
            if pp != 0 {
                self.one(Op::HideMenu, pp)?;
            }
            let viewport = self.one(Op::Viewport, 0)?;
            self.get(Op::ClearAction)?;
            if self.one(Op::FadePositive, viewport)? != 0 && self.one(Op::DrewWorld, viewport)? != 0
            {
                self.scope(31, viewport)?;
            }
            self.one(Op::ClearFog, viewport)?;
            self.one(Op::Draw, viewport)?;
            self.audio()?;
        }
        Ok(0)
    }
    fn verify(&self) -> std::result::Result<usize, O::Error> {
        self.get(Op::BeginLoad)?;
        if self.get(Op::Pending)? != 0 {
            let mut i = 0;
            while i < self.get(Op::PackageCount)? {
                self.one(Op::PackageLinker, i)?;
                i += 1;
            }
            i = 0;
            while i < self.get(Op::PackageCount)? {
                self.one(Op::VerifyLinker, i)?;
                i += 1;
            }
            if self.get(Op::PackageCount)? != 0 {
                self.get(Op::MapParent)?;
            }
        }
        self.get(Op::VerifyMap)?;
        self.get(Op::EndLoad)?;
        let kind = self.get(Op::DemoKind)?;
        for bit in [1, 2] {
            if kind & bit != 0
                && (self.get(Op::Pending)? == 0 || self.get(Op::DemoDriver)? == 0)
                && self.scope(3, bit)? == 0
            {
                return Ok(0);
            }
        }
        Ok(1)
    }
    fn demo(&self, kind: usize) -> std::result::Result<usize, O::Error> {
        if self.get(Op::DemoSuffix)? == 0 {
            self.get(Op::DemoAppend)?;
        }
        self.get(Op::DemoSize)?;
        self.get(Op::DemoLog)?;
        let mut allowed = false;
        if kind == 1 {
            for i in 0..3 {
                if self.one(Op::DemoName, i)? != 0 {
                    allowed = true;
                    break;
                }
            }
        } else {
            for (i, size) in [
                2135105, 1618994, 1193759, 2152238, 3498978, 2155658, 34822, 34884,
            ]
            .into_iter()
            .enumerate()
            {
                if self.one(Op::DemoName, i + 3)? != 0 && self.get(Op::DemoFileSize)? == size {
                    allowed = true;
                    break;
                }
            }
        }
        if allowed {
            Ok(1)
        } else {
            self.get(Op::DemoError)?;
            self.get(Op::Progress)?;
            Ok(0)
        }
    }
    fn recover(&self) -> std::result::Result<usize, O::Error> {
        self.get(Op::EndLoad)?;
        self.get(Op::CatchError)?;
        self.get(Op::Progress)?;
        if self.get(Op::Client)? != 0 && (self.get(Op::ViewportCount)? as i32) > 0 {
            let viewport = self.one(Op::Viewport, 0)?;
            self.get(Op::ClearAction)?;
            self.one(Op::RestoreFog, viewport)?;
            self.one(Op::RestoreGlow, viewport)?;
            self.one(Op::RestoreFade, viewport)?;
        }
        Ok(0)
    }
    fn dissociate(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Client)? != 0 {
            let mut i = 0;
            while i < self.get(Op::ViewportCount)? {
                let actor = self.one(Op::ViewportActor, self.one(Op::Viewport, i)?)?;
                let level = self.one(Op::ActorLevel, actor)?;
                self.one(Op::ClearPlayer, actor)?;
                self.one(Op::ClearViewportActor, i)?;
                self.call(Op::DestroyActor, level, actor)?;
                i += 1;
            }
        }
        Ok(0)
    }
    fn exit_level(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Level)? != 0 {
            self.get(Op::ResetLoaders)?;
            for (get, delete, clear) in [
                (Op::Brush, Op::DeleteBrush, Op::ClearBrush),
                (Op::NetDriver, Op::DeleteNet, Op::ClearNet),
                (Op::Demo, Op::DeleteDemo, Op::ClearDemo),
            ] {
                if self.get(get)? != 0 {
                    self.get(delete)?;
                    self.get(clear)?;
                }
            }
            if self.one(Op::Option, 1)? != 0 {
                self.get(Op::CleanupDestroyed)?;
                self.scope(7, 0)?;
            }
            self.get(Op::ClearLevel)?;
        }
        Ok(0)
    }
    fn post_load(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::HasSlotDump)? != 0 {
            self.get(Op::SlotDump)?;
        }
        if self.get(Op::Pending)? != 0 {
            if self.get(Op::LonePlayer)? != 0 {
                self.get(Op::ClearPending)?;
            }
            self.get(Op::ComputePackages)?;
            let mut i = 0;
            while i < self.get(Op::PackageCount)? {
                // Preserve the original lone-player/null-pending mismatch behavior.
                if self.one(Op::GenerationMismatch, i)? != 0 {
                    self.one(Op::LogHave, i)?;
                }
                i += 1;
            }
        }
        self.scope(10, 0)?;
        self.get(Op::CheckLevel)?;
        self.get(Op::GetInfo)?;
        self.get(Op::ComputerName)?;
        self.scope(11, 0)?;
        self.scope(12, 0)?;
        if self.get(Op::IsEntry)? == 0 {
            self.scope(13, 0)?;
        }
        self.scope(14, 0)?;
        self.scope(15, 0)?;
        if self.get(Op::Audio)? != 0 {
            self.one(Op::AudioCleanup, self.get(Op::Audio)?)?;
        }
        self.get(Op::Collision)?;
        self.scope(17, 0)?;
        self.get(Op::UpdateTime)?;
        self.scope(18, 0)
    }
    fn hub(&self) -> std::result::Result<usize, O::Error> {
        let saved = self.get(Op::SavedHub)? as i32;
        let hub = if self.one(Op::Option, 0)? != 0 {
            self.get(Op::InfoHub)? as i32
        } else if self.one(Op::Option, 1)? != 0 {
            saved.wrapping_add(1)
        } else if self.one(Op::Option, 2)? != 0 {
            saved.wrapping_sub(1).max(0)
        } else if self.one(Op::Option, 3)? != 0 {
            saved
        } else {
            0
        };
        self.one(Op::SetHub, hub as usize)
    }
    fn activate(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Pending)? != 0 {
            self.get(Op::CheckPending)?;
            self.get(Op::AttachNet)?;
            if self.get(Op::NetDriver)? != 0 {
                self.get(Op::NotifyNet)?;
            }
            self.get(Op::AttachDemo)?;
            if self.get(Op::Demo)? != 0 {
                self.get(Op::NotifyDemo)?;
            }
            self.get(Op::ClientMode)?;
        } else {
            self.get(Op::CheckNoNet)?;
        }
        Ok(0)
    }
    fn init_level(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::LoadOption)? == 0 {
            self.get(Op::AssignUrl)?;
        }
        self.get(Op::Version)?;
        self.get(Op::MinVersion)?;
        self.get(Op::SetEngine)?;
        if self.get(Op::Travel)? != 0 {
            self.get(Op::AssignTravel)?;
        }
        Ok(0)
    }
    fn cleanup(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::IsEntry)? == 0 {
            self.get(Op::Flush)?;
            self.scope(16, 0)?;
            let mut i = 0;
            while i < self.get(Op::ActorCount)? {
                let actor = self.one(Op::Actor, i)?;
                if actor != 0 {
                    self.one(Op::KeepActor, actor)?;
                }
                i += 1;
            }
            self.get(Op::Gc)?;
        }
        Ok(0)
    }
    fn eliminate(&self) -> std::result::Result<usize, O::Error> {
        while self.get(Op::IteratorValid)? != 0 {
            if self.one(Op::InLevel, self.get(Op::IteratorActor)?)? != 0 {
                self.one(Op::Eliminate, self.get(Op::IteratorActor)?)?;
            }
            self.get(Op::IteratorNext)?;
        }
        Ok(0)
    }
    fn zones(&self) -> std::result::Result<usize, O::Error> {
        let mut old = [0u64; 64];
        let mut conn = [0u64; 64];
        // This native table is fixed-size. No virtual, script, allocation or other
        // callback occurs in the original zone kernel, so retain its raw address
        // without an exclusive native borrow or per-cell FFI dispatch.
        let distances = self.get(Op::ZoneDistances)? as *mut u8;
        for (i, value) in old.iter_mut().enumerate() {
            self.call(Op::Connectivity, i, value as *mut u64 as usize)?;
            for j in 0..64 {
                unsafe {
                    distances
                        .add(i * 64 + j)
                        .write(if i == j { 0 } else { 255 });
                }
            }
        }
        for distance in 1..64u8 {
            for (j, value) in old.iter().enumerate() {
                for k in 0..64 {
                    let cell = unsafe { distances.add(j * 64 + k) };
                    if unsafe { cell.read() } > distance && value & (1u64 << k) != 0 {
                        unsafe {
                            cell.write(distance);
                        }
                    }
                }
            }
            conn.fill(0);
            for j in 0..64 {
                for k in 0..64 {
                    if old[j] & (1u64 << k) != 0 {
                        conn[j] |= old[k];
                    }
                }
            }
            old = conn;
        }
        Ok(0)
    }
    fn game_tail(&self) -> std::result::Result<usize, O::Error> {
        self.scope(19, 0)?;
        self.scope(20, 0)?;
        self.one(Op::HighDetail, 1)?;
        if self.get(Op::Client)? != 0
            && self.get(Op::ViewportCount)? != 0
            && self.get(Op::RenderDevice)? != 0
            && self.get(Op::DeviceDetail)? == 0
        {
            self.one(Op::HighDetail, 0)?;
        }
        self.scope(23, 0)?;
        self.scope(24, 0)?;
        self.final_tail()
    }
    fn init_game(&self) -> std::result::Result<usize, O::Error> {
        let mut i = 0;
        while i < self.get(Op::OptionCount)? {
            self.get(Op::AppendQuestion)?;
            self.one(Op::AppendOption, i)?;
            self.one(Op::ParseGame, i)?;
            i += 1;
        }
        if self.get(Op::IsServer)? != 0 && self.get(Op::Game)? == 0 {
            let mut class;
            if self.get(Op::GameName)? == 0 {
                class = self.get(Op::DefaultGame)?;
                if class == 0 {
                    let name = if self.get(Op::Client)? != 0 {
                        Op::DefaultGameName
                    } else {
                        Op::DefaultServerGameName
                    };
                    class = self.one(Op::LoadDefaultGame, self.get(name)?)?;
                }
            } else {
                class = self.one(Op::LoadDefaultGame, self.get(Op::GameNamePointer)?)?;
            }
            self.one(Op::LogGame, class)?;
            self.one(Op::SpawnGame, class)?;
            self.get(Op::CheckGame)?;
        }
        Ok(0)
    }
    fn listen(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Client)? == 0 || self.one(Op::Option, 4)? != 0 {
            if self.get(Op::EnginePending)? != 0 {
                self.scope(21, 0)?;
            }
            self.scope(22, 0)?;
        }
        Ok(0)
    }
    fn begin_play(&self) -> std::result::Result<usize, O::Error> {
        self.one(Op::FirstDynamic, 0)?;
        if self.get(Op::BegunPlay)? == 0 {
            self.get(Op::Fixup)?;
            self.get(Op::LogBringUp)?;
            self.get(Op::ClearTime)?;
            self.get(Op::ClearInfoTime)?;
            let mut i = 0;
            while i < self.get(Op::ActorCount)? {
                if self.one(Op::Actor, i)? != 0 {
                    let mut j = 0;
                    while j < self.one(Op::TouchCount, i)? {
                        self.call(Op::ClearTouch, i, j)?;
                        j += 1;
                    }
                }
                i += 1;
            }
            if self.get(Op::IsServer)? == 0 {
                i = 0;
                while i < self.get(Op::ActorCount)? {
                    let actor = self.one(Op::Actor, i)?;
                    if actor != 0 {
                        if self.one(Op::Static, actor)? != 0 || self.one(Op::NoDelete, actor)? != 0
                        {
                            self.one(Op::ExchangeRoles, actor)?;
                        } else {
                            self.call(Op::DestroyActor, self.get(Op::Level)?, actor)?;
                        }
                    }
                    i += 1;
                }
            }
            i = 0;
            while i < self.get(Op::ActorCount)? {
                if self.one(Op::Actor, i)? != 0 {
                    self.one(Op::InitExecution, self.one(Op::Actor, i)?)?;
                }
                i += 1;
            }
            self.get(Op::SetBegun)?;
            self.one(Op::Startup, 1)?;
            if self.get(Op::Game)? != 0 {
                self.get(Op::InitGame)?;
            }
            for event in [
                Op::PreBegin,
                Op::BeginPlay,
                Op::SetZone,
                Op::PostBegin,
                Op::InitialState,
            ] {
                i = 0;
                while i < self.get(Op::ActorCount)? {
                    if self.one(Op::Actor, i)? != 0
                        && self.one(Op::ScriptInitialized, self.one(Op::Actor, i)?)? == 0
                    {
                        if matches!(event, Op::PreBegin) {
                            self.one(Op::PreBefore, i)?;
                        }
                        self.one(event, i)?;
                        if matches!(event, Op::PreBegin) {
                            self.one(Op::PreAfter, i)?;
                        }
                    }
                    i += 1;
                }
            }
            i = 0;
            while i < self.get(Op::ActorCount)? {
                if self.one(Op::Actor, i)? != 0 {
                    if self.one(Op::HasAttach, i)? != 0 {
                        let mut j = 0;
                        while j < self.get(Op::ActorCount)? {
                            if self.one(Op::Actor, j)? != 0
                                && self.call(Op::AttachMatches, i, j)? != 0
                            {
                                self.call(Op::SetBase, i, j)?;
                                break;
                            }
                            j += 1;
                        }
                    } else if self.one(Op::Base, i)? == 0
                        && self.one(Op::CollideWorld, i)? != 0
                        && (self.one(Op::Decoration, i)? != 0
                            || self.one(Op::Inventory, i)? != 0
                            || self.one(Op::Pawn, i)? != 0)
                        && (self.one(Op::PhysicsNone, i)? != 0
                            || self.one(Op::PhysicsRotating, i)? != 0)
                    {
                        self.one(Op::FindBase, i)?;
                        if self.one(Op::Base, i)? == self.get(Op::Info)? {
                            self.one(Op::ClearBase, i)?;
                        }
                    }
                }
                i += 1;
            }
            self.one(Op::Startup, 0)?;
        } else {
            self.get(Op::SyncTime)?;
        }
        self.get(Op::Lifecycle)
    }
    fn rearrange(&self) -> std::result::Result<usize, O::Error> {
        self.one(Op::AddActor, 0)?;
        self.one(Op::AddActor, 1)?;
        for phase in 0..3 {
            let mut i = 2;
            while i < self.get(Op::ActorCount)? {
                if self.one(Op::Actor, i)? != 0 {
                    let is_static = self.one(Op::Static, self.one(Op::Actor, i)?)? != 0;
                    let include = if phase == 2 {
                        !is_static
                    } else {
                        is_static
                            && (self.one(Op::Relevant, self.one(Op::Actor, i)?)? != 0)
                                == (phase == 1)
                    };
                    if include {
                        self.one(Op::AddActor, i)?;
                    }
                }
                i += 1;
            }
            if phase == 0 {
                self.one(Op::FirstRelevant, self.get(Op::ReorderedCount)?)?;
            }
            if phase == 1 {
                self.one(Op::FirstDynamic, self.get(Op::ReorderedCount)?)?;
            }
        }
        self.get(Op::EmptyActors)?;
        self.get(Op::AllocateActors)?;
        let mut i = 0;
        while i < self.get(Op::ReorderedCount)? {
            self.one(Op::CopyActor, i)?;
            i += 1;
        }
        Ok(0)
    }
    fn final_tail(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::ProfileEnabled)? != 0 {
            self.scope(25, 0)?;
        }
        self.scope(26, 0)?;
        if self.get(Op::IsEntry)? == 0 {
            self.scope(27, 0)?;
        }
        self.get(Op::DetailChange)?;
        self.scope(28, 0)?;
        if self.get(Op::IsClient)? != 0 {
            for i in 0..7 {
                self.one(Op::SaveConfig, i)?;
            }
        }
        self.get(Op::LogTiming)?;
        self.get(Op::Level)
    }
    fn client_init(&self) -> std::result::Result<usize, O::Error> {
        if self.get(Op::Client)? != 0 {
            let target = if self.get(Op::IsServer)? != 0 {
                self.get(Op::Level)?
            } else {
                self.get(Op::Entry)?
            };
            self.one(Op::MatchViewports, target)?;
            if self.get(Op::IsEntry)? == 0 {
                self.get(Op::NewBrush)?;
            }
            self.audio()?;
            let mut i = 0;
            while i < self.get(Op::ViewportCount)? {
                let viewport = self.one(Op::Viewport, i)?;
                self.one(Op::ResetInput, viewport)?;
                if self.one(Op::ViewportDevice, viewport)? != 0 {
                    self.one(Op::FlushDevice, viewport)?;
                }
                i += 1;
            }
        }
        Ok(0)
    }
    fn run(&self, entry: u32, arg: usize) -> std::result::Result<usize, O::Error> {
        match entry {
            0 => self.root(),
            1 => self.loading_screen(),
            2 => self.verify(),
            3 => self.demo(arg),
            4 => {
                if self.get(Op::Level)? != 0 {
                    self.get(Op::Notify)?;
                }
                Ok(0)
            }
            5 => self.dissociate(),
            6 => self.exit_level(),
            7 => {
                self.get(Op::SaveFilename)?;
                self.get(Op::SavePackage)
            }
            8 => self.get(Op::LoadLevel),
            9 => self.post_load(),
            10 => self.get(Op::VerifyClasses),
            11 => self.hub(),
            12 => self.activate(),
            13 => self.get(Op::Persistent),
            14 => self.init_level(),
            15 => self.cleanup(),
            16 => self.eliminate(),
            17 => self.zones(),
            18 => self.game_tail(),
            19 => self.init_game(),
            20 => self.listen(),
            21 => {
                self.get(Op::CheckNotPending)?;
                self.get(Op::DeletePending)?;
                self.get(Op::ClearEnginePending)
            }
            22 => {
                if self.get(Op::Listen)? == 0 {
                    self.get(Op::ListenError)?;
                }
                Ok(0)
            }
            23 => self.begin_play(),
            24 => self.rearrange(),
            25 => {
                self.scope(33, 0)?;
                self.get(Op::ResetTicks)
            }
            26 => self.client_init(),
            27 => self.get(Op::ScreenActors),
            28 => self.get(Op::LastUrl),
            29 => self.recover(),
            31 => {
                while self.one(Op::FogPositive, arg)? != 0 {
                    self.one(Op::Draw, arg)?;
                    self.one(Op::FadeStep, arg)?;
                }
                Ok(0)
            }
            33 => {
                while self.get(Op::FunctionValid)? != 0 {
                    self.get(Op::ClearProfile)?;
                    self.get(Op::FunctionNext)?;
                }
                Ok(0)
            }
            32 => self.timed_root(),
            _ => self.invalid_entry(),
        }
    }
}
struct NativeOps {
    services: *const Hp2GameLoadMapServices,
    context: *mut c_void,
}
impl LoadMapOps for NativeOps {
    type Error = i32;
    fn call(&self, op: Op, a: usize, b: usize) -> Result {
        let mut out = 0;
        let status = unsafe { ((*self.services).call)(self.context, op as u32, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
    fn scope(&self, entry: u32, arg: usize) -> Result {
        let mut out = 0;
        let status = unsafe { ((*self.services).scope)(self.context, entry, arg, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
    fn invalid_entry(&self) -> Result {
        Err(1)
    }
}

pub(crate) fn run_with<O: LoadMapOps>(
    ops: O,
    entry: u32,
    arg: usize,
) -> std::result::Result<usize, O::Error> {
    Policy { ops }.run(entry, arg)
}

/// # Safety
/// Services remain immutable; context and native scope pointers remain live throughout
/// synchronous calls. Every callback catches native exceptions before returning status.
pub unsafe fn run(
    services: *const Hp2GameLoadMapServices,
    context: *mut c_void,
    entry: u32,
    arg: usize,
) -> Result {
    run_with(NativeOps { services, context }, entry, arg)
}

#[cfg(test)]
mod tests {
    use super::fade_fog;

    #[test]
    fn fade_keeps_tick_precision_and_unclamped_overshoot() {
        let previous = 1_i64 << 56;
        let one_tick = 1.0 / 4_294_967_296.0;
        assert_eq!(fade_fog(previous + 1, previous, one_tick, 1.0).to_bits(), 0);
        assert_eq!(fade_fog(1_i64 << 32, 0, 0.25, 0.75), -3.25);
    }
}
