//! Player login and travelling-actor acceptance policy. Native scopes own all
//! engine strings, arrays, stack buffers and reflection iterators.
use std::ffi::c_void;
#[repr(C)]
pub struct Hp2LevelSpawnPlayActorServices {
    pub call: unsafe extern "C" fn(*mut c_void, u32, usize, usize, *mut usize) -> i32,
    pub scope: unsafe extern "C" fn(*mut c_void, u32, usize, *mut usize) -> i32,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Op {
    ClearError,
    PackageMap,
    ClassOption,
    LoadPlayer,
    DefaultUser,
    DefaultIni,
    ClassError,
    SaveClass,
    OptionCount,
    Question,
    Option,
    Login,
    LoginFailed,
    HasLoad,
    LoadValue,
    SaveLoading,
    ClearTravel,
    SetPlayer,
    SetRole,
    SetRemoteRole,
    SetShow,
    SetRender,
    AllAdmin,
    NetDriver,
    SetAdmin,
    PreAccept,
    PlayerName,
    FindTravel,
    TravelText,
    Standalone,
    SaveStr,
    Str,
    ParseClass,
    ParseName,
    Incoming,
    LoadTravel,
    CannotAccept,
    PlayerClass,
    Spawn,
    SpawnFailed,
    Spawned,
    Accept,
    ParseLine,
    CompareEndLine,
    SaveLine,
    AcceptedCount,
    ParmCount,
    Parm,
    Char,
    Advance,
    Alnum,
    Digit,
    VarChar,
    Atoi,
    SaveValue,
    IteratorValid,
    IteratorNext,
    PropertyFlags,
    ComparePropertyName,
    ArrayDim,
    Data,
    ObjectProperty,
    ActorReference,
    NameMatches,
    StoreReference,
    Import,
    AcceptedActor,
    AcceptInventory,
    PostAccept,
    PreviousProperty,
    PreviousFlags,
    ClearPrevious,
    PostLogin,
    Actor,
    Loading,
    FirstValid,
    FirstValue,
    Element,
    Connection,
}
const CPF_TRAVEL: usize = 0x0001_0000;
/// Supplies checked native or Core-owned operations to the shared spawn policy.
pub trait Backend {
    type Error;
    fn invalid_entry(&self) -> Self::Error;
    fn call(&self, op: Op, a: usize, b: usize) -> std::result::Result<usize, Self::Error>;
    fn scope(&self, entry: u32, arg: usize) -> std::result::Result<usize, Self::Error>;
}

struct NativeBackend<'a> {
    services: &'a Hp2LevelSpawnPlayActorServices,
    context: *mut c_void,
}

impl Backend for NativeBackend<'_> {
    type Error = i32;
    fn invalid_entry(&self) -> Self::Error { 1 }
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

struct Policy<'a, B: Backend> {
    backend: &'a B,
}
impl<B: Backend> Policy<'_, B> {
    fn call(&self, op: Op, a: usize, b: usize) -> std::result::Result<usize, B::Error> {
        self.backend.call(op, a, b)
    }
    fn get(&self, op: Op) -> std::result::Result<usize, B::Error> {
        self.call(op, 0, 0)
    }
    fn one(&self, op: Op, a: usize) -> std::result::Result<usize, B::Error> {
        self.call(op, a, 0)
    }
    fn scope(&self, entry: u32, arg: usize) -> std::result::Result<usize, B::Error> {
        self.backend.scope(entry, arg)
    }
    fn root(&self) -> std::result::Result<usize, B::Error> {
        self.get(Op::ClearError)?;
        let connection = self.get(Op::Connection)?;
        if connection != 0 {
            self.one(Op::PackageMap, connection)?;
        }
        let option = self.get(Op::ClassOption)?;
        let mut class = if option != 0 {
            self.one(Op::LoadPlayer, option)?
        } else {
            0
        };
        if class == 0 {
            class = self.one(Op::LoadPlayer, self.get(Op::DefaultUser)?)?;
        }
        if class == 0 {
            class = self.one(Op::LoadPlayer, self.get(Op::DefaultIni)?)?;
        }
        if class == 0 {
            self.get(Op::ClassError)?;
        }
        self.one(Op::SaveClass, class)?;
        self.scope(1, 0)
    }
    fn login(&self) -> std::result::Result<usize, B::Error> {
        let mut i = 0;
        while i < self.get(Op::OptionCount)? {
            self.get(Op::Question)?;
            self.one(Op::Option, i)?;
            i += 1;
        }
        let actor = self.get(Op::Login)?;
        if actor == 0 {
            self.get(Op::LoginFailed)?;
            return Ok(0);
        }
        let loading = self.get(Op::HasLoad)? != 0 || self.get(Op::LoadValue)? != 0;
        self.one(Op::SaveLoading, loading as usize)?;
        if loading {
            self.get(Op::ClearTravel)?;
        }
        self.get(Op::SetPlayer)?;
        self.get(Op::SetRole)?;
        self.get(Op::SetRemoteRole)?;
        self.get(Op::SetShow)?;
        self.get(Op::SetRender)?;
        if self.get(Op::AllAdmin)? != 0 || self.get(Op::NetDriver)? == 0 {
            self.get(Op::SetAdmin)?;
        }
        self.one(Op::PreAccept, actor)?;
        let mut text = 0;
        if !loading {
            let name = self.get(Op::PlayerName)?;
            if name != 0 {
                let found = self.one(Op::FindTravel, name)?;
                if found != 0 {
                    text = self.one(Op::TravelText, found)?;
                }
            }
            if text == 0 && self.get(Op::Standalone)? != 0 {
                text = self.scope(4, 0)?;
            }
        }
        self.one(Op::SaveStr, text)?;
        self.scope(2, 0)
    }
    fn accepted(&self) -> std::result::Result<usize, B::Error> {
        let actor = self.get(Op::Actor)?;
        while self.get(Op::Str)? != 0
            && self.get(Op::ParseClass)? != 0
            && self.get(Op::ParseName)? != 0
        {
            self.get(Op::Incoming)?;
            let mut accept = 0;
            let class = self.get(Op::LoadTravel)?;
            if class == 0 {
                self.get(Op::CannotAccept)?;
            } else if self.one(Op::PlayerClass, class)? != 0 {
                accept = self.one(Op::Accept, actor)?;
            } else {
                let spawned = self.one(Op::Spawn, class)?;
                if spawned == 0 {
                    self.get(Op::SpawnFailed)?;
                } else {
                    self.get(Op::Spawned)?;
                    accept = self.one(Op::Accept, spawned)?;
                }
            }
            self.get(Op::ParseLine)?;
            self.get(Op::ParseLine)?;
            while self.get(Op::ParseLine)? != 0 && self.get(Op::CompareEndLine)? != 0 {
                if accept != 0 {
                    self.one(Op::SaveLine, accept)?;
                }
            }
        }
        let mut i = 0;
        while i < self.get(Op::AcceptedCount)? {
            let mut j = 0;
            while j < self.one(Op::ParmCount, i)? {
                self.property(i, j)?;
                j += 1;
            }
            i += 1;
        }
        let mut reverse = self.get(Op::AcceptedCount)?;
        while reverse != 0 {
            reverse -= 1;
            self.one(Op::PreAccept, self.one(Op::AcceptedActor, reverse)?)?;
        }
        self.get(Op::AcceptInventory)?;
        reverse = self.get(Op::AcceptedCount)?;
        while reverse != 0 {
            reverse -= 1;
            self.one(Op::PostAccept, self.one(Op::AcceptedActor, reverse)?)?;
        }
        // FindField runs even when this is not a load URL.
        let previous = self.get(Op::PreviousProperty)?;
        if self.get(Op::Loading)? != 0
            && previous != 0
            && self.one(Op::PreviousFlags, previous)? & CPF_TRAVEL != 0
        {
            self.one(Op::ClearPrevious, previous)?;
        }
        self.one(Op::PostAccept, actor)?;
        self.get(Op::PostLogin)?;
        Ok(actor)
    }
    fn property(&self, i: usize, j: usize) -> std::result::Result<usize, B::Error> {
        let mut ptr = self.call(Op::Parm, i, j)?;
        while self.one(Op::Char, ptr)? == b' ' as usize {
            ptr = self.call(Op::Advance, ptr, 1)?;
        }
        let mut end = 0;
        loop {
            let ch = self.one(Op::Char, ptr)?;
            if self.one(Op::Alnum, ch)? == 0 && ch != b'_' as usize {
                break;
            }
            self.call(Op::VarChar, end, ch)?;
            end += 1;
            ptr = self.call(Op::Advance, ptr, 1)?;
        }
        self.call(Op::VarChar, end, 0)?;
        let mut element = 0;
        if self.one(Op::Char, ptr)? == b'[' as usize {
            ptr = self.call(Op::Advance, ptr, 1)?;
            element = self.one(Op::Atoi, ptr)?;
            while self.one(Op::Digit, self.one(Op::Char, ptr)?)? != 0 {
                ptr = self.call(Op::Advance, ptr, 1)?;
            }
            let ch = self.one(Op::Char, ptr)?;
            ptr = self.call(Op::Advance, ptr, 1)?;
            if ch != b']' as usize {
                return Ok(0);
            }
        }
        let ch = self.one(Op::Char, ptr)?;
        ptr = self.call(Op::Advance, ptr, 1)?;
        if ch != b'=' as usize {
            return Ok(0);
        }
        self.call(Op::SaveValue, ptr, element)?;
        self.scope(3, i)
    }
    fn properties(&self, actor_index: usize) -> std::result::Result<usize, B::Error> {
        while self.get(Op::IteratorValid)? != 0 {
            if self.get(Op::PropertyFlags)? & CPF_TRAVEL != 0
                && self.get(Op::ComparePropertyName)? == 0
            {
                // Element is captured by the native scope with the original INT width.
                let element = self.get(Op::Element)? as isize;
                if element < self.get(Op::ArrayDim)? as isize {
                    self.one(Op::Data, actor_index)?;
                    let reference = self.get(Op::ObjectProperty)?;
                    if reference != 0 && self.one(Op::ActorReference, reference)? != 0 {
                        let mut k = 0;
                        while k < self.get(Op::AcceptedCount)? {
                            if self.one(Op::NameMatches, k)? != 0 {
                                self.one(Op::StoreReference, k)?;
                                break;
                            }
                            k += 1;
                        }
                    } else {
                        self.get(Op::Import)?;
                    }
                }
            }
            self.get(Op::IteratorNext)?;
        }
        Ok(0)
    }
}
/// Runs one shared spawn-policy continuation with the supplied backend.
pub fn run_with<B: Backend>(
    backend: &B,
    entry: u32,
    arg: usize,
) -> std::result::Result<usize, B::Error> {
    let policy = Policy { backend };
    match entry {
        0 => policy.root(),
        1 => policy.login(),
        2 => policy.accepted(),
        3 => policy.properties(arg),
        4 => {
            if policy.get(Op::FirstValid)? != 0 {
                policy.get(Op::FirstValue)
            } else {
                Ok(0)
            }
        }
        _ => Err(backend.invalid_entry()),
    }
}

/// Runs the shared policy through the native continuation callbacks.
///
/// # Safety
/// `services` and `context` must remain valid for the synchronous call and every
/// scoped leaf must point to storage alive until its continuation returns.
pub unsafe fn run(
    services: *const Hp2LevelSpawnPlayActorServices,
    context: *mut c_void,
    entry: u32,
    arg: usize,
) -> std::result::Result<usize, i32> {
    let backend = NativeBackend { services: unsafe { &*services }, context };
    run_with(&backend, entry, arg)
}
