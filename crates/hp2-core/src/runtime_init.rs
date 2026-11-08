//! Runtime lifecycle ordering and persistent actor cache policy. Native leaves
//! own engine storage and typed operations; every failing leaf stops the flow.
use core::ffi::c_void;

type Result<T, E = i32> = core::result::Result<T, E>;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2RuntimeOperation {
    Init,
    PreExit,
    Exit,
    SaveCache,
    LoadCache,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2RuntimeLeaf {
    PublishFileManager,
    Crc,
    CommandLine,
    ErrorHistory,
    PublishDevices,
    PublishAllocator,
    AllocatorInit,
    NamesInit,
    PlatformPreInit,
    FileManagerInit,
    DefaultDirectory,
    LogVersion,
    LogCompiled,
    LogCommandLine,
    LogBaseDirectory,
    LogCharacterSet,
    Strict,
    ParseIniName,
    DefaultIniName,
    IniSize,
    DefaultTime,
    IniTime,
    LoadDefault,
    MissingDefault,
    SaveDefault,
    EmptyDefault,
    ConfigCreate,
    ConfigInit,
    DefaultConfigCreate,
    DefaultConfigInit,
    ReadOldLanguage,
    ReadDefaultLanguage,
    WriteDefaultLanguage,
    ReadLanguage,
    SetLanguage,
    ObjectsInit,
    MemoryInit,
    ParseCdPath,
    ReadCdPath,
    PaletteSize,
    ClearCdPath,
    HasCdPath,
    CdPathLast,
    AppendCdSeparator,
    LogCdPath,
    Channels,
    ChannelValue,
    StreamInit,
    PlatformInit,
    HasUserDirectory,
    UserSavePath,
    PathCount,
    MissingPaths,
    PublishSaveGlob,
    LocalSavePath,
    MakeSaveDirectory,
    ParseSaveSlot,
    SetSaveSlot,
    DefaultSaveSlot,
    HasSaveSlot,
    LogSaveSlot,
    MakeSlotDirectory,
    MakeCacheDirectory,
    IsMapPath,
    PathLength,
    PathCharacter,
    SetMapBase,
    MapBaseSystemOffset,
    TrimMapBase,
    AppendMapSuffix,
    LoadCacheCall,
    ConfigSavePath,
    ConfigSaveGlob,
    ConfigFlush,
    CacheFind,
    CacheCount,
    CacheFilename,
    CacheSize,
    CacheDelete,
    CacheCopy,
    CacheEmpty,
    LogPreExit,
    PlatformPreExit,
    MemoryExit,
    ObjectsExit,
    LogExit,
    StreamDestroy,
    PlatformExit,
    HasConfig,
    ConfigExit,
    ConfigDelete,
    ClearConfig,
    NamesExit,
    CriticalError,
    DumpAllocations,
    FlushRandomTrace,
}

#[repr(C)]
pub struct Hp2RuntimeServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, Hp2RuntimeLeaf, i32, i32, *mut i64) -> i32,
    pub command: *const c_void,
    pub require_config: i32,
    pub path_separator: u32,
}

/// FileSize's legacy boolean conversion: missing (-1) is true, empty (0) false.
pub fn cache_file_selected(size: i64) -> bool {
    size != 0
}

impl Hp2RuntimeServices {
    fn call(&self, leaf: Hp2RuntimeLeaf, a: i32, b: i32) -> Result<i64> {
        let mut value = 0;
        let status = unsafe { (self.call)(self.context, leaf, a, b, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn step(&self, leaf: Hp2RuntimeLeaf) -> Result<i64> {
        self.call(leaf, 0, 0)
    }
}

/// Owner-independent file-cache policy used by startup and save/reload paths.
pub trait RuntimePathServices {
    fn call(&self, leaf: Hp2RuntimeLeaf, a: i32, b: i32) -> core::result::Result<i64, i32>;
}
impl RuntimePathServices for Hp2RuntimeServices {
    fn call(&self, leaf: Hp2RuntimeLeaf, a: i32, b: i32) -> core::result::Result<i64, i32> {
        Hp2RuntimeServices::call(self, leaf, a, b)
    }
}

fn path_step(s: &impl RuntimePathServices, leaf: Hp2RuntimeLeaf) -> Result<i64> {
    s.call(leaf, 0, 0)
}

fn cache(s: &impl RuntimePathServices, load: bool) -> Result<()> {
    use Hp2RuntimeLeaf::*;
    if path_step(s, HasSaveSlot)? == 0 {
        return Ok(());
    }
    // Location 0 = slot, 1 = cache. Load deletes *.*, save deletes *_pa.*.
    let delete_location = i32::from(load);
    s.call(CacheFind, delete_location, i32::from(load))?;
    let mut index = 0;
    while (index as i64) < path_step(s, CacheCount)? {
        s.call(CacheFilename, delete_location, index)?;
        if cache_file_selected(s.call(CacheSize, 0, 0)?) {
            path_step(s, CacheDelete)?;
        }
        index += 1;
    }
    path_step(s, CacheEmpty)?;
    let source = i32::from(!load);
    s.call(CacheFind, source, 0)?;
    index = 0;
    while (index as i64) < path_step(s, CacheCount)? {
        s.call(CacheFilename, source, index)?;
        if cache_file_selected(s.call(CacheSize, 0, 0)?) {
            s.call(CacheCopy, delete_location, index)?;
        }
        index += 1;
    }
    Ok(())
}

/// Executes the exact shared save-slot, map-path, cache-load and config policy.
pub fn initialize_paths(s: &impl RuntimePathServices) -> core::result::Result<(), i32> {
    use Hp2RuntimeLeaf::*;
    if path_step(s, HasUserDirectory)? != 0 {
        path_step(s, UserSavePath)?;
        if path_step(s, PathCount)? <= 5 {
            path_step(s, MissingPaths)?;
        }
        path_step(s, PublishSaveGlob)?;
    } else {
        path_step(s, LocalSavePath)?;
    }
    path_step(s, MakeSaveDirectory)?;
    if path_step(s, ParseSaveSlot)? != 0 {
        path_step(s, SetSaveSlot)?;
    } else {
        path_step(s, DefaultSaveSlot)?;
    }
    if path_step(s, HasSaveSlot)? != 0 {
        for leaf in [LogSaveSlot, MakeSlotDirectory, MakeCacheDirectory] {
            path_step(s, leaf)?;
        }
    }
    let mut index = 0;
    while (index as i64) < path_step(s, PathCount)? {
        if s.call(IsMapPath, index, 0)? != 0 {
            let mut separator = s.call(PathLength, index, 0)? as i32 - 1;
            while separator >= 0 {
                let ch = s.call(PathCharacter, index, separator)?;
                if ch == 47 || ch == 92 || ch == 58 { break; }
                separator -= 1;
            }
            path_step(s, SetMapBase)?;
            let system = path_step(s, MapBaseSystemOffset)? as i32;
            s.call(TrimMapBase, system, 0)?;
            s.call(AppendMapSuffix, index, separator)?;
            break;
        }
        index += 1;
    }
    path_step(s, LoadCacheCall)?;
    for leaf in [ConfigSavePath, ConfigSaveGlob, ConfigFlush] {
        path_step(s, leaf)?;
    }
    Ok(())
}
pub fn load_persistent_cache(s: &impl RuntimePathServices) -> core::result::Result<(), i32> {
    cache(s, true)
}
pub fn save_persistent_cache(s: &impl RuntimePathServices) -> core::result::Result<(), i32> {
    cache(s, false)
}

fn initialize(s: &Hp2RuntimeServices) -> Result<()> {
    use Hp2RuntimeLeaf::*;
    s.step(PublishFileManager)?;
    s.step(Crc)?;
    s.call(CommandLine, 0, 0)?;
    for leaf in [
        ErrorHistory,
        PublishDevices,
        PublishAllocator,
        AllocatorInit,
        NamesInit,
        PlatformPreInit,
        FileManagerInit,
        DefaultDirectory,
        LogVersion,
        LogCompiled,
        LogCommandLine,
        LogBaseDirectory,
        LogCharacterSet,
        Strict,
    ] {
        s.step(leaf)?;
    }
    let mut default_changed = false;
    for user in 0..2 {
        if s.call(ParseIniName, user, 0)? == 0 {
            s.call(DefaultIniName, user, 0)?;
        }
        if s.require_config != 0
            && (s.call(IniSize, user, 0)? < 0
                || s.call(DefaultTime, user, 0)? > s.call(IniTime, user, 0)?
                || (user == 1 && default_changed))
        {
            if s.call(LoadDefault, user, 0)? == 0 {
                s.call(MissingDefault, user, 0)?;
            }
            s.call(SaveDefault, user, 0)?;
            if user == 0 {
                default_changed = true;
            }
            s.step(EmptyDefault)?;
        }
    }
    for leaf in [
        ConfigCreate,
        ConfigInit,
        DefaultConfigCreate,
        DefaultConfigInit,
        ReadOldLanguage,
        ReadDefaultLanguage,
        WriteDefaultLanguage,
    ] {
        s.step(leaf)?;
    }
    if s.step(ReadLanguage)? != 0 {
        s.step(SetLanguage)?;
    }
    s.step(ObjectsInit)?;
    s.step(MemoryInit)?;
    if s.step(ParseCdPath)? == 0 {
        s.step(ReadCdPath)?;
        if s.step(PaletteSize)? >= 0 {
            s.step(ClearCdPath)?;
        }
    }
    if s.step(HasCdPath)? != 0 && s.step(CdPathLast)? != s.path_separator as i64 {
        s.step(AppendCdSeparator)?;
    }
    if s.step(HasCdPath)? != 0 {
        s.step(LogCdPath)?;
    }
    let channels = if s.step(Channels)? != 0 {
        s.step(ChannelValue)?
    } else {
        32
    };
    s.call(StreamInit, channels as i32, 0)?;
    s.step(PlatformInit)?;
    initialize_paths(s)?;
    Ok(())
}


/// # Safety
/// Native context and command storage must remain live through all callbacks.
pub unsafe fn execute(s: &Hp2RuntimeServices, operation: Hp2RuntimeOperation) -> Result<()> {
    use Hp2RuntimeLeaf::*;
    match operation {
        Hp2RuntimeOperation::Init => initialize(s),
        Hp2RuntimeOperation::SaveCache => cache(s, false),
        Hp2RuntimeOperation::LoadCache => cache(s, true),
        Hp2RuntimeOperation::PreExit => {
            for leaf in [LogPreExit, PlatformPreExit, MemoryExit, ObjectsExit] {
                s.step(leaf)?;
            }
            Ok(())
        }
        Hp2RuntimeOperation::Exit => {
            for leaf in [LogExit, StreamDestroy, PlatformExit] {
                s.step(leaf)?;
            }
            if s.step(HasConfig)? != 0 {
                for leaf in [ConfigExit, ConfigDelete, ClearConfig] {
                    s.step(leaf)?;
                }
            }
            s.step(NamesExit)?;
            if s.step(CriticalError)? == 0 {
                s.step(DumpAllocations)?;
            }
            s.step(FlushRandomTrace)?;
            Ok(())
        }
    }
}
