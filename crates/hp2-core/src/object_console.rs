//! UObject console policy. Native leaves own SDK access, formatting and archives.
use std::{collections::HashMap, ffi::c_void};

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectConsoleLeaf {
    ProfileEnabled,
    DumpAllocs,
    ObjectLen,
    ObjectAt,
    IsKind,
    ResetFunction,
    ResetTicks,
    Token,
    FindClass,
    FindProperty,
    ExportDefault,
    SkipSpaces,
    SetProperty,
    NoGc,
    SetNoGc,
    CollectGarbage,
    Flags,
    SetFlags,
    ClearFlags,
    ParseObject,
    ObjectClass,
    Outer,
    IsIn,
    PendingKill,
    CountMem,
    SortObjects,
    SortClasses,
    ClassCompare,
    NameNumber,
    ObjectStaticClass,
    SuperClass,
    ExcludePackage,
    DisplayHash,
    Serialize,
    HashCount,
    HashHead,
    HashNext,
    LoaderLen,
    LoaderAt,
    NameLen,
    NameSize,
    LinkerRow,
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectConsoleCommand {
    Mem,
    ResetProfile,
    DumpNatives,
    Get,
    Set,
    Obj,
    Garbage,
    Mark,
    MarkCheck,
    Refs,
    Hash,
    Classes,
    Dependencies,
    List,
    VfHash,
    Linkers,
    GTime,
    Verbose,
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectConsoleLog {
    ClassWarning,
    PropertyWarning,
    Marking,
    Unmarked,
    DebugObject,
    Blank,
    Referencers,
    Referencer,
    Reachability,
    Route,
    Unrooted,
    Dependencies,
    ReferenceHeading,
    Reference,
    ClassTree,
    Objects,
    Space,
    ObjectColumns,
    ObjectRow,
    ClassColumns,
    ClassRow,
    Total,
    Deleted,
    Hashes,
    StateName,
    HashBucket,
    Linkers,
    Time,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2ObjectConsoleRow {
    pub object: usize,
    pub count: i32,
    pub num: usize,
    pub max: usize,
}
pub type Hp2ObjectConsoleVisitor = unsafe extern "C" fn(*mut c_void, usize, *mut c_void) -> i32;
#[repr(C)]
pub struct Hp2ObjectConsoleServices {
    pub call:
        unsafe extern "C" fn(*mut c_void, Hp2ObjectConsoleLeaf, usize, usize, *mut usize) -> i32,
    pub archive:
        unsafe extern "C" fn(*mut c_void, usize, i32, *mut c_void, Hp2ObjectConsoleVisitor) -> i32,
    pub command: unsafe extern "C" fn(*mut c_void, Hp2ObjectConsoleCommand, *mut i32) -> i32,
    pub log:
        unsafe extern "C" fn(*mut c_void, Hp2ObjectConsoleLog, *const Hp2ObjectConsoleRow) -> i32,
}
#[derive(Clone, Copy)]
struct Flow<'a> {
    context: *mut c_void,
    services: &'a Hp2ObjectConsoleServices,
    visitor: Hp2ObjectConsoleVisitor,
}
use Hp2ObjectConsoleCommand as C;
use Hp2ObjectConsoleLeaf as L;
use Hp2ObjectConsoleLog as E;
const TAG: usize = 0x10;
const MARK: usize = 0x0800_0000;
impl Flow<'_> {
    unsafe fn call(self, leaf: L, a: usize, b: usize) -> Result<usize, i32> {
        let mut out = 0;
        let status = unsafe { (self.services.call)(self.context, leaf, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
    unsafe fn command(self, command: C) -> Result<bool, i32> {
        let mut out = 0;
        let status = unsafe { (self.services.command)(self.context, command, &mut out) };
        if status == 0 {
            Ok(out != 0)
        } else {
            Err(status)
        }
    }
    unsafe fn log(self, event: E, row: Hp2ObjectConsoleRow) -> Result<(), i32> {
        let status = unsafe { (self.services.log)(self.context, event, &row) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn object_log(self, event: E, object: usize, count: i32) -> Result<(), i32> {
        unsafe {
            self.log(
                event,
                Hp2ObjectConsoleRow {
                    object,
                    count,
                    ..Default::default()
                },
            )
        }
    }
    unsafe fn each(self, mut body: impl FnMut(usize) -> Result<(), i32>) -> Result<(), i32> {
        let mut index = 0;
        // Reload the live table after every native callback; no table slice crosses reentry.
        while index < unsafe { self.call(L::ObjectLen, 0, 0)? } {
            let object = unsafe { self.call(L::ObjectAt, index, 0)? };
            if object != 0 {
                body(object)?;
            }
            index += 1;
        }
        Ok(())
    }
    unsafe fn archive(
        self,
        object: usize,
        mode: i32,
        state: *mut Reference<'_>,
    ) -> Result<(), i32> {
        let status = unsafe {
            (self.services.archive)(self.context, object, mode, state.cast(), self.visitor)
        };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}
#[derive(Clone, Copy)]
struct Route {
    depth: i32,
    previous: usize,
}
enum References {
    Count {
        target: usize,
        count: i32,
    },
    Show {
        parent: usize,
        excluded: Vec<usize>,
        shown: bool,
    },
    Trace {
        routes: HashMap<usize, Route>,
        depth: i32,
        previous: usize,
    },
}
struct Reference<'a> {
    flow: Flow<'a>,
    mode: References,
}

/// # Safety
/// The opaque state is owned by the synchronous archive invocation. In particular,
/// no reference into its map or enum may survive a native Serialize callback.
pub unsafe fn reference(
    state: *mut c_void,
    object: usize,
    archive: *mut c_void,
) -> Result<(), i32> {
    let state = state.cast::<Reference<'_>>();
    let flow = unsafe { (*state).flow };
    unsafe {
        match &mut (*state).mode {
            References::Count { target, count } => {
                if object == *target {
                    *count = count.wrapping_add(1);
                }
                return Ok(());
            }
            References::Show { .. } => {}
            References::Trace {
                routes,
                depth,
                previous,
            } => {
                if object != 0 && routes.get(&object).is_none_or(|r| *depth < r.depth) {
                    routes.insert(
                        object,
                        Route {
                            depth: *depth,
                            previous: *previous,
                        },
                    );
                }
            }
        }
        if matches!((*state).mode, References::Show { .. }) {
            if object == 0 {
                return Ok(());
            }
            let outer = flow.call(L::Outer, object, 0)?;
            let (show, heading) = match &(*state).mode {
                References::Show {
                    parent,
                    excluded,
                    shown,
                } => (outer != *parent && !excluded.contains(&outer), !*shown),
                _ => unreachable!(),
            };
            if show {
                // Deliberately use the referenced object in the heading (legacy shadowing).
                if heading {
                    flow.object_log(E::ReferenceHeading, object, 0)?;
                }
                flow.object_log(E::Reference, object, 0)?;
                if let References::Show { shown, .. } = &mut (*state).mode {
                    *shown = true;
                }
            }
            return Ok(());
        }
        if object != 0 && flow.call(L::Flags, object, 0)? & TAG != 0 {
            flow.call(L::ClearFlags, object, TAG)?;
            let saved = match &mut (*state).mode {
                References::Trace {
                    depth, previous, ..
                } => {
                    let saved = *previous;
                    *previous = object;
                    *depth = depth.wrapping_add(1);
                    saved
                }
                _ => unreachable!(),
            };
            flow.call(L::Serialize, object, archive as usize)?;
            // No cleanup on failure: preserve the original partial traversal/tag state.
            if let References::Trace {
                depth, previous, ..
            } = &mut (*state).mode
            {
                *depth = depth.wrapping_sub(1);
                *previous = saved;
            }
        }
        Ok(())
    }
}
unsafe fn classes(f: Flow<'_>, class: usize, indent: i32) -> Result<(), i32> {
    unsafe {
        f.object_log(E::ClassTree, class, indent)?;
        f.each(|object| {
            if f.call(L::IsKind, object, 0)? != 0 && f.call(L::SuperClass, object, 0)? == class {
                classes(f, object, indent.wrapping_add(2))?;
            }
            Ok(())
        })
    }
}
unsafe fn refs(f: Flow<'_>, target: usize) -> Result<(), i32> {
    unsafe {
        f.object_log(E::Blank, 0, 0)?;
        f.object_log(E::Referencers, target, 0)?;
        f.each(|object| {
            let mut state = Reference {
                flow: f,
                mode: References::Count { target, count: 0 },
            };
            f.archive(object, 0, &mut state)?;
            if let References::Count { count, .. } = state.mode {
                if count != 0 {
                    f.object_log(E::Referencer, object, 0)?;
                }
            }
            Ok(())
        })?;
        f.object_log(E::Blank, 0, 0)?;
        f.object_log(E::Reachability, target, 0)?;
        let mut state = Reference {
            flow: f,
            mode: References::Trace {
                routes: HashMap::new(),
                depth: 0,
                previous: 0,
            },
        };
        f.each(|object| {
            f.call(L::SetFlags, object, TAG)?;
            Ok(())
        })?;
        f.archive(0, 1, &mut state)?;
        f.each(|object| {
            f.call(L::ClearFlags, object, TAG)?;
            Ok(())
        })?;
        let References::Trace { routes, .. } = state.mode else {
            unreachable!()
        };
        let mut path = Vec::new();
        if routes.contains_key(&target) {
            let mut object = target;
            loop {
                path.push(object);
                let record = routes[&object];
                if record.depth == 0 {
                    break;
                }
                object = record.previous;
            }
        }
        if path.is_empty() {
            f.object_log(E::Unrooted, 0, 0)?;
        }
        for (i, object) in path.into_iter().rev().enumerate() {
            f.object_log(E::Route, object, (i == 0) as i32)?;
        }
        f.object_log(E::Blank, 0, 0)
    }
}
unsafe fn list(f: Flow<'_>) -> Result<(), i32> {
    unsafe {
        f.object_log(E::Objects, 0, 0)?;
        f.object_log(E::Blank, 0, 0)?;
        let class = f.call(L::ParseObject, 0, 0)?;
        let package = f.call(L::ParseObject, 1, 0)?;
        let inside = f.call(L::ParseObject, 2, 0)?;
        let filtered = class != 0 || package != 0 || inside != 0;
        let mut groups: Vec<Hp2ObjectConsoleRow> = Vec::new();
        let mut objects: Vec<Hp2ObjectConsoleRow> = Vec::new();
        let mut total = Hp2ObjectConsoleRow::default();
        f.each(|object| {
            if !filtered
                || (class != 0
                    && crate::object_model::is_a(object as *const c_void, class as *const c_void))
                || (package != 0 && f.call(L::Outer, object, 0)? == package)
                || (inside != 0 && f.call(L::IsIn, object, inside)? != 0)
            {
                let mut row = Hp2ObjectConsoleRow {
                    object,
                    count: 1,
                    ..Default::default()
                };
                f.call(L::CountMem, object, &mut row as *mut _ as usize)?;
                let class = f.call(L::ObjectClass, object, 0)?;
                let index = groups
                    .iter()
                    .position(|r| r.object == class)
                    .unwrap_or_else(|| {
                        groups.push(Hp2ObjectConsoleRow {
                            object: class,
                            ..Default::default()
                        });
                        groups.len() - 1
                    });
                if filtered {
                    objects.push(row);
                }
                for aggregate in [&mut groups[index], &mut total] {
                    aggregate.count = aggregate.count.wrapping_add(1);
                    aggregate.num = aggregate.num.wrapping_add(row.num);
                    aggregate.max = aggregate.max.wrapping_add(row.max);
                }
            }
            Ok(())
        })?;
        let mut deleted: i32 = 0;
        if !objects.is_empty() {
            f.call(L::SortObjects, objects.as_mut_ptr() as usize, objects.len())?;
            f.object_log(E::Space, 0, 0)?;
            f.object_log(E::ObjectColumns, 0, 0)?;
            let verbose = f.command(C::Verbose)?;
            for row in objects {
                if verbose || f.call(L::PendingKill, row.object, 0)? == 0 {
                    f.log(E::ObjectRow, row)?;
                }
                if f.call(L::PendingKill, row.object, 0)? != 0 {
                    deleted = deleted.wrapping_add(1);
                }
            }
            f.object_log(E::Space, 0, 0)?;
        }
        if !groups.is_empty() {
            f.call(L::SortClasses, groups.as_mut_ptr() as usize, groups.len())?;
            f.object_log(E::ClassColumns, 0, 0)?;
            for row in groups {
                f.log(E::ClassRow, row)?;
            }
            f.object_log(E::Space, 0, 0)?;
        }
        f.log(E::Total, total)?;
        f.object_log(E::Deleted, 0, deleted)
    }
}
/// Native qsort retains its exact-tie behavior. Legacy suffix subtraction wraps.
pub fn compare_max(a: usize, b: usize) -> i32 {
    if b > a {
        1
    } else if b < a {
        -1
    } else {
        0
    }
}
/// # Safety
/// Object pointers and service leaves must be valid for the call.
pub unsafe fn compare_names(
    services: &Hp2ObjectConsoleServices,
    context: *mut c_void,
    a: usize,
    b: usize,
) -> Result<i32, i32> {
    let mut out = 0;
    let status = unsafe { (services.call)(context, L::ClassCompare, a, b, &mut out) };
    if status != 0 {
        return Err(status);
    }
    if out as i32 != 0 {
        return Ok(out as i32);
    }
    let mut x = 0;
    let mut y = 0;
    let status = unsafe { (services.call)(context, L::NameNumber, a, 0, &mut x) };
    if status != 0 {
        return Err(status);
    }
    let status = unsafe { (services.call)(context, L::NameNumber, b, 0, &mut y) };
    if status != 0 {
        return Err(status);
    }
    Ok((x as i32).wrapping_sub(y as i32))
}
/// # Safety
/// Both row objects and the native comparison services must remain valid.
pub unsafe fn compare_objects(
    services: &Hp2ObjectConsoleServices,
    context: *mut c_void,
    a: &Hp2ObjectConsoleRow,
    b: &Hp2ObjectConsoleRow,
) -> Result<i32, i32> {
    let order = compare_max(a.max, b.max);
    if order != 0 {
        Ok(order)
    } else {
        unsafe { compare_names(services, context, a.object, b.object) }
    }
}
/// # Safety
/// Callbacks must capture native exceptions. Context and services remain live
/// through synchronous reentry; visitor is the guarded FFI reference adapter.
pub unsafe fn exec(
    context: *mut c_void,
    services: &Hp2ObjectConsoleServices,
    visitor: Hp2ObjectConsoleVisitor,
) -> Result<i32, i32> {
    let f = Flow {
        context,
        services,
        visitor,
    };
    unsafe {
        if f.command(C::Mem)? {
            f.call(L::DumpAllocs, 0, 0)?;
        } else if f.call(L::ProfileEnabled, 0, 0)? != 0 && f.command(C::ResetProfile)? {
            f.each(|o| {
                if f.call(L::IsKind, o, 1)? != 0 {
                    f.call(L::ResetFunction, o, 0)?;
                }
                Ok(())
            })?;
            f.call(L::ResetTicks, 0, 0)?;
        } else if f.command(C::DumpNatives)? { /* Recognized/no output on macOS and Linux. */
        } else {
            let get = f.command(C::Get)?;
            if get || f.command(C::Set)? {
                let token = f.call(L::Token, 0, 0)?;
                let class = if token != 0 {
                    f.call(L::FindClass, 0, 0)?
                } else {
                    0
                };
                if class == 0 {
                    f.object_log(E::ClassWarning, 0, 0)?;
                } else {
                    let token = f.call(L::Token, 1, 0)?;
                    let property = if token != 0 {
                        f.call(L::FindProperty, class, 0)?
                    } else {
                        0
                    };
                    if property == 0 {
                        f.object_log(E::PropertyWarning, 0, 0)?;
                    } else if get {
                        f.call(L::ExportDefault, class, property)?;
                    } else {
                        f.call(L::SkipSpaces, 0, 0)?;
                        f.call(L::SetProperty, class, property)?;
                    }
                }
            } else if f.command(C::Obj)? {
                if f.command(C::Garbage)? {
                    let saved = f.call(L::NoGc, 0, 0)?;
                    f.call(L::SetNoGc, 0, 0)?;
                    f.call(L::CollectGarbage, 0, 0)?;
                    f.call(L::SetNoGc, saved, 0)?;
                } else if f.command(C::Mark)? {
                    f.object_log(E::Marking, 0, 0)?;
                    f.each(|o| {
                        f.call(L::SetFlags, o, MARK)?;
                        Ok(())
                    })?;
                } else if f.command(C::MarkCheck)? {
                    f.object_log(E::Unmarked, 0, 0)?;
                    f.each(|o| {
                        if f.call(L::Flags, o, 0)? & MARK == 0 {
                            f.object_log(E::DebugObject, o, 0)?;
                        }
                        Ok(())
                    })?;
                } else if f.command(C::Refs)? {
                    let class = f.call(L::ParseObject, 0, 0)?;
                    if class != 0 {
                        let object = f.call(L::ParseObject, 3, class)?;
                        if object != 0 {
                            refs(f, object)?;
                        }
                    }
                } else if f.command(C::Hash)? {
                    f.call(L::DisplayHash, 0, 0)?;
                } else if f.command(C::Classes)? {
                    classes(f, f.call(L::ObjectStaticClass, 0, 0)?, 0)?;
                } else if f.command(C::Dependencies)? {
                    let parent = f.call(L::ParseObject, 1, 0)?;
                    if parent != 0 {
                        let mut excluded = Vec::new();
                        for i in 0..16 {
                            let package = f.call(L::ExcludePackage, i, 0)?;
                            if package != 0 {
                                excluded.push(package);
                            }
                        }
                        f.object_log(E::Dependencies, parent, 0)?;
                        let mut state = Reference {
                            flow: f,
                            mode: References::Show {
                                parent,
                                excluded,
                                shown: false,
                            },
                        };
                        f.each(|o| {
                            if f.call(L::Outer, o, 0)? == parent {
                                if let References::Show { shown, .. } = &mut state.mode {
                                    *shown = false;
                                }
                                f.archive(o, 2, &mut state)?;
                            }
                            Ok(())
                        })?;
                    }
                } else if f.command(C::List)? {
                    list(f)?;
                } else if f.command(C::VfHash)? {
                    f.object_log(E::Hashes, 0, 0)?;
                    f.each(|o| {
                        if f.call(L::IsKind, o, 2)? != 0 {
                            f.object_log(E::StateName, o, 0)?;
                            for i in 0..f.call(L::HashCount, 0, 0)? {
                                let mut count: i32 = 0;
                                let mut field = f.call(L::HashHead, o, i)?;
                                while field != 0 {
                                    count = count.wrapping_add(1);
                                    field = f.call(L::HashNext, field, 0)?;
                                }
                                f.object_log(E::HashBucket, i, count)?;
                            }
                        }
                        Ok(())
                    })?;
                } else if f.command(C::Linkers)? {
                    f.object_log(E::Linkers, 0, 0)?;
                    let mut i = 0;
                    while i < f.call(L::LoaderLen, 0, 0)? {
                        let linker = f.call(L::LoaderAt, i, 0)?;
                        let mut name_size: i32 = 0;
                        let mut n = 0;
                        while n < f.call(L::NameLen, linker, 0)? {
                            name_size =
                                name_size.wrapping_add(f.call(L::NameSize, linker, n)? as i32);
                            n += 1;
                        }
                        f.call(L::LinkerRow, linker, name_size as usize)?;
                        i += 1;
                    }
                } else {
                    return Ok(0);
                }
            } else if f.command(C::GTime)? {
                f.object_log(E::Time, 0, 0)?;
            } else {
                return Ok(0);
            }
        }
        Ok(1)
    }
}
