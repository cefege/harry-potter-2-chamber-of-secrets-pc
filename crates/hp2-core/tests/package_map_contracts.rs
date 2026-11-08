use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_core::package_map::{
    Hp2PackageMapLoadServices, Hp2PackageMapServices, index_to_object, object_to_index,
    supports_package,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Linker,
    LocalIndex,
    Find,
    Len,
    At(i32),
    Parent(i32),
    Count(i32),
    Base(i32),
    Export(i32, i32),
    Begin,
    Create(i32, i32),
    End,
}

#[derive(Default)]
struct Trace {
    calls: Vec<Call>,
    failure: Option<Call>,
    swap: Option<(*mut Package, *mut Linker)>,
}

thread_local! {
    static TRACE: RefCell<Trace> = RefCell::new(Trace::default());
}

fn record(call: Call) -> i32 {
    TRACE.with(|trace| {
        let mut trace = trace.borrow_mut();
        let status = if trace.failure.as_ref() == Some(&call) {
            29
        } else {
            0
        };
        trace.calls.push(call);
        status
    })
}

fn reset() {
    TRACE.with(|trace| *trace.borrow_mut() = Trace::default());
}

fn calls() -> Vec<Call> {
    TRACE.with(|trace| trace.borrow().calls.clone())
}

fn identity(value: usize) -> *mut c_void {
    value as *mut c_void
}

struct Object {
    linker: *mut c_void,
    index: i32,
}

struct Linker {
    id: i32,
    exports: Vec<*mut c_void>,
    created: *mut c_void,
}

struct Package {
    id: i32,
    parent: *mut c_void,
    linker: *mut Linker,
    count: i32,
    base: i32,
}

struct Map {
    packages: Vec<Package>,
    linkers: Vec<(*mut c_void, i32)>,
}

unsafe extern "C" fn object_linker(object: *mut c_void) -> *mut c_void {
    record(Call::Linker);
    unsafe { (*object.cast::<Object>()).linker }
}

unsafe extern "C" fn object_linker_index(object: *mut c_void) -> i32 {
    record(Call::LocalIndex);
    unsafe { (*object.cast::<Object>()).index }
}

unsafe extern "C" fn find_linker(
    map: *mut c_void,
    linker: *mut c_void,
    found: *mut *const i32,
) -> i32 {
    let status = record(Call::Find);
    if status == 0 {
        let map = unsafe { &*map.cast::<Map>() };
        unsafe {
            *found = map
                .linkers
                .iter()
                .find(|entry| entry.0 == linker)
                .map_or(std::ptr::null(), |entry| &entry.1);
        }
    }
    status
}

unsafe extern "C" fn package_len(packages: *mut c_void) -> i32 {
    record(Call::Len);
    let map = unsafe { &*packages.cast::<Map>() };
    map.packages.len() as i32
}

unsafe extern "C" fn package_at(
    packages: *mut c_void,
    index: i32,
    package: *mut *mut c_void,
) -> i32 {
    let status = record(Call::At(index));
    if status == 0 {
        let map = unsafe { &mut *packages.cast::<Map>() };
        unsafe { *package = (&mut map.packages[index as usize] as *mut Package).cast() };
    }
    status
}

unsafe extern "C" fn package_parent(package: *mut c_void) -> *mut c_void {
    let package = unsafe { &*package.cast::<Package>() };
    record(Call::Parent(package.id));
    package.parent
}

unsafe extern "C" fn package_object_count(package: *mut c_void) -> i32 {
    let package = unsafe { &*package.cast::<Package>() };
    record(Call::Count(package.id));
    package.count
}

unsafe extern "C" fn package_object_base(package: *mut c_void) -> i32 {
    let package = unsafe { &*package.cast::<Package>() };
    record(Call::Base(package.id));
    package.base
}

unsafe extern "C" fn export_object(
    package: *mut c_void,
    index: i32,
    object: *mut *mut c_void,
) -> i32 {
    let linker = unsafe { &*(*package.cast::<Package>()).linker };
    let status = record(Call::Export(linker.id, index));
    if status == 0 {
        unsafe { *object = linker.exports[index as usize] };
    }
    status
}

unsafe extern "C" fn begin_load() -> i32 {
    let status = record(Call::Begin);
    if status == 0 {
        TRACE.with(|trace| {
            if let Some((package, linker)) = trace.borrow_mut().swap.take() {
                unsafe { (*package).linker = linker };
            }
        });
    }
    status
}

unsafe extern "C" fn create_export(
    package: *mut c_void,
    index: i32,
    object: *mut *mut c_void,
) -> i32 {
    let linker = unsafe { &mut *(*package.cast::<Package>()).linker };
    let status = record(Call::Create(linker.id, index));
    if status == 0 {
        linker.exports[index as usize] = linker.created;
        unsafe { *object = linker.created };
    }
    status
}

unsafe extern "C" fn end_load() -> i32 {
    record(Call::End)
}

const SERVICES: Hp2PackageMapServices = Hp2PackageMapServices {
    object_linker,
    object_linker_index,
    find_linker,
    package_len,
    package_at,
    package_parent,
    package_object_count,
    package_object_base,
    export_object,
};
const LOADING: Hp2PackageMapLoadServices = Hp2PackageMapLoadServices {
    begin_load,
    create_export,
    end_load,
};

struct World {
    // Stable addresses model live C++ linkers independently of package records.
    first: Box<Linker>,
    last: Box<Linker>,
    map: Map,
}

impl World {
    fn new() -> Self {
        reset();
        let mut first = Box::new(Linker {
            id: 10,
            exports: vec![identity(100), identity(101)],
            created: identity(110),
        });
        let mut last = Box::new(Linker {
            id: 20,
            exports: vec![identity(200), null_mut(), identity(202)],
            created: identity(210),
        });
        let first_ptr = &mut *first as *mut Linker;
        let last_ptr = &mut *last as *mut Linker;
        let map = Map {
            packages: vec![
                Package {
                    id: 0,
                    parent: identity(1),
                    linker: first_ptr,
                    count: 2,
                    base: 40,
                },
                Package {
                    id: 1,
                    parent: null_mut(),
                    linker: null_mut(),
                    count: 0,
                    base: 70,
                },
                Package {
                    id: 2,
                    parent: identity(2),
                    linker: last_ptr,
                    count: 3,
                    base: 90,
                },
            ],
            linkers: vec![(first_ptr.cast(), 0), (last_ptr.cast(), 2)],
        };
        Self { first, last, map }
    }

    fn resolve(&mut self, index: i32, load: i32) -> Result<*mut c_void, i32> {
        unsafe {
            index_to_object(
                (&mut self.map as *mut Map).cast(),
                index,
                load,
                &SERVICES,
                &LOADING,
            )
        }
    }

    fn object_index(&mut self, object: *mut Object) -> Result<i32, i32> {
        let map = (&mut self.map as *mut Map).cast();
        unsafe { object_to_index(object.cast(), map, map, &SERVICES) }
    }

    fn supports(&mut self, outer: *mut c_void) -> Result<i32, i32> {
        unsafe { supports_package((&mut self.map as *mut Map).cast(), outer, &SERVICES) }
    }
}

#[test]
fn concatenated_indices_cross_packages_and_skip_zero_size_null_linker() {
    let mut world = World::new();
    for (index, expected) in [(0, 100), (1, 101), (2, 200), (3, 0), (4, 202)] {
        reset();
        assert_eq!(world.resolve(index, 0), Ok(identity(expected)));
        assert!(!calls().contains(&Call::Begin));
        assert!(
            !calls()
                .iter()
                .any(|call| matches!(call, Call::Export(0, _)))
        );
    }
    // ObjectBase is deliberately unrelated: reverse resolution uses counts.
    for index in [5, 6, 40, i32::MAX] {
        reset();
        assert_eq!(world.resolve(index, 1), Ok(null_mut()));
        assert!(
            !calls()
                .iter()
                .any(|call| matches!(call, Call::Export(_, _) | Call::Begin))
        );
    }
    for index in [-1, -2, i32::MIN] {
        reset();
        assert_eq!(world.resolve(index, 1), Ok(null_mut()));
        assert!(calls().is_empty());
    }
}

#[test]
fn object_mapping_uses_linker_lookup_strict_bound_and_stored_base() {
    let mut world = World::new();
    let first = (&mut *world.first as *mut Linker).cast();
    let last = (&mut *world.last as *mut Linker).cast();
    for (linker, local, expected) in [
        (first, 0, 40),
        (first, 1, 41),
        (first, 2, -1),
        (last, 0, 90),
        (last, 2, 92),
        (last, 3, -1),
        (last, i32::MAX, -1),
        // Legacy checks INDEX_NONE, not all negative local indices.
        (first, -2, 38),
    ] {
        reset();
        assert_eq!(
            world.object_index(&mut Object {
                linker,
                index: local
            }),
            Ok(expected)
        );
    }
    reset();
    assert_eq!(
        world.object_index(&mut Object {
            linker: last,
            index: 2
        }),
        Ok(92)
    );
    assert_eq!(
        calls(),
        [
            Call::Linker,
            Call::LocalIndex,
            Call::Linker,
            Call::Find,
            Call::At(2),
            Call::LocalIndex,
            Call::Count(2),
            Call::Base(2),
            Call::LocalIndex,
        ]
    );
    world.map.packages[2].count = 0;
    assert_eq!(
        world.object_index(&mut Object {
            linker: last,
            index: 0
        }),
        Ok(-1)
    );
}

#[test]
fn absent_objects_linkers_and_local_indices_short_circuit() {
    let mut world = World::new();
    assert_eq!(world.object_index(null_mut()), Ok(-1));
    assert!(calls().is_empty());
    assert_eq!(
        world.object_index(&mut Object {
            linker: null_mut(),
            index: 0
        }),
        Ok(-1)
    );
    assert_eq!(calls(), [Call::Linker]);
    reset();
    assert_eq!(
        world.object_index(&mut Object {
            linker: identity(999),
            index: -1
        }),
        Ok(-1)
    );
    assert_eq!(calls(), [Call::Linker, Call::LocalIndex]);
    reset();
    assert_eq!(
        world.object_index(&mut Object {
            linker: identity(999),
            index: 0
        }),
        Ok(-1)
    );
    assert_eq!(
        calls(),
        [Call::Linker, Call::LocalIndex, Call::Linker, Call::Find]
    );
}

#[test]
fn support_search_preserves_order_first_match_and_null_semantics() {
    let mut world = World::new();
    assert_eq!(world.supports(identity(1)), Ok(1));
    assert_eq!(calls(), [Call::Len, Call::At(0), Call::Parent(0)]);
    reset();
    assert_eq!(world.supports(null_mut()), Ok(1));
    assert_eq!(
        calls(),
        [
            Call::Len,
            Call::At(0),
            Call::Parent(0),
            Call::Len,
            Call::At(1),
            Call::Parent(1),
        ]
    );
    reset();
    assert_eq!(world.supports(identity(2)), Ok(1));
    assert_eq!(calls().last(), Some(&Call::Parent(2)));
    reset();
    assert_eq!(world.supports(identity(999)), Ok(0));
    assert_eq!(
        calls(),
        [
            Call::Len,
            Call::At(0),
            Call::Parent(0),
            Call::Len,
            Call::At(1),
            Call::Parent(1),
            Call::Len,
            Call::At(2),
            Call::Parent(2),
            Call::Len,
        ]
    );
    world.map.packages[1].parent = identity(1);
    reset();
    assert_eq!(world.supports(identity(1)), Ok(1));
    assert_eq!(calls(), [Call::Len, Call::At(0), Call::Parent(0)]);
    assert_eq!(world.supports(null_mut()), Ok(0));
    world.map.packages.clear();
    reset();
    assert_eq!(world.supports(null_mut()), Ok(0));
    assert_eq!(calls(), [Call::Len]);
    reset();
    assert_eq!(world.resolve(0, 1), Ok(null_mut()));
    assert_eq!(calls(), [Call::Len]);
}

fn selected_last_export_trace() -> Vec<Call> {
    vec![
        Call::Len,
        Call::At(0),
        Call::Count(0),
        Call::Len,
        Call::At(1),
        Call::Count(1),
        Call::Len,
        Call::At(2),
        Call::Count(2),
        Call::Export(20, 1),
    ]
}

#[test]
fn loading_is_conditional_ordered_and_persists_the_created_export() {
    let mut world = World::new();
    assert_eq!(world.resolve(3, 0), Ok(null_mut()));
    assert_eq!(calls(), selected_last_export_trace());
    reset();
    assert_eq!(world.resolve(3, -7), Ok(identity(210)));
    let mut expected = selected_last_export_trace();
    expected.extend([Call::Begin, Call::Create(20, 1), Call::End]);
    assert_eq!(calls(), expected);
    assert_eq!(world.last.exports[1], identity(210));
    reset();
    assert_eq!(world.resolve(3, 1), Ok(identity(210)));
    assert_eq!(calls(), selected_last_export_trace());
    reset();
    assert_eq!(world.resolve(0, 1), Ok(identity(100)));
    assert_eq!(
        calls(),
        [Call::Len, Call::At(0), Call::Count(0), Call::Export(10, 0)]
    );
}

#[test]
fn null_creation_still_ends_load_and_begin_load_can_change_the_live_linker() {
    let mut world = World::new();
    world.last.created = null_mut();
    assert_eq!(world.resolve(3, 1), Ok(null_mut()));
    assert_eq!(
        &calls()[10..],
        [Call::Begin, Call::Create(20, 1), Call::End]
    );
    reset();
    let package = &mut world.map.packages[2] as *mut Package;
    let replacement = &mut *world.first as *mut Linker;
    TRACE.with(|trace| trace.borrow_mut().swap = Some((package, replacement)));
    assert_eq!(world.resolve(3, 1), Ok(identity(110)));
    let mut expected = selected_last_export_trace();
    expected.extend([Call::Begin, Call::Create(10, 1), Call::End]);
    assert_eq!(calls(), expected);
}

#[test]
fn failing_load_leaves_stop_without_added_cleanup_or_retry() {
    for failure in [
        Call::Export(20, 1),
        Call::Begin,
        Call::Create(20, 1),
        Call::End,
    ] {
        let mut world = World::new();
        TRACE.with(|trace| trace.borrow_mut().failure = Some(failure.clone()));
        assert_eq!(world.resolve(3, 1), Err(29));
        let mut expected = selected_last_export_trace();
        expected.extend([Call::Begin, Call::Create(20, 1), Call::End]);
        expected.truncate(expected.iter().position(|call| *call == failure).unwrap() + 1);
        assert_eq!(calls(), expected);
        assert_eq!(
            world.last.exports[1],
            if failure == Call::End {
                identity(210)
            } else {
                null_mut()
            }
        );
    }
}

#[test]
fn container_failures_stop_before_following_queries() {
    let mut world = World::new();
    let linker = (&mut *world.first as *mut Linker).cast();
    TRACE.with(|trace| trace.borrow_mut().failure = Some(Call::Find));
    assert_eq!(
        world.object_index(&mut Object { linker, index: 0 }),
        Err(29)
    );
    assert_eq!(
        calls(),
        [Call::Linker, Call::LocalIndex, Call::Linker, Call::Find]
    );
    reset();
    TRACE.with(|trace| trace.borrow_mut().failure = Some(Call::At(0)));
    assert_eq!(
        world.object_index(&mut Object { linker, index: 0 }),
        Err(29)
    );
    assert_eq!(calls().last(), Some(&Call::At(0)));
    reset();
    TRACE.with(|trace| trace.borrow_mut().failure = Some(Call::At(1)));
    assert_eq!(world.supports(identity(2)), Err(29));
    assert_eq!(
        calls(),
        [
            Call::Len,
            Call::At(0),
            Call::Parent(0),
            Call::Len,
            Call::At(1)
        ]
    );
    reset();
    TRACE.with(|trace| trace.borrow_mut().failure = Some(Call::At(1)));
    assert_eq!(world.resolve(3, 1), Err(29));
    assert_eq!(
        calls(),
        [
            Call::Len,
            Call::At(0),
            Call::Count(0),
            Call::Len,
            Call::At(1)
        ]
    );
}

mod construction {
    use super::{RefCell, c_void, identity, null_mut};
    use hp2_core::object_collections::ObjectCollection;
    use hp2_core::package_map::{
        self, Hp2PackageImportView, Hp2PackageInfoView, Hp2PackageMapBuildServices,
        Hp2PackageMapBuildView,
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        Package(i32),
        Append(i32),
        Import(i32, i32),
        Add(i32),
        Compute,
        Check,
        EmptyNames,
        AddNames,
        Slot(i32),
        EmptyLinkers,
        GenerationExports(i32),
        GenerationNames(i32),
        LinkerName(i32, i32),
        SetLinker(i32),
        Serialize,
        Loading,
        Error,
        None,
        Select(i32, i32),
    }

    #[derive(Default)]
    struct Log {
        events: Vec<Event>,
        fail: Option<Event>,
        // Mutations from virtual admission model live lists changing mid-traversal.
        grow_source: Option<(*mut BuildMap, *mut BuildLinker, i32)>,
        grow_loaders: Option<(*mut BuildMap, *mut BuildLinker, i32)>,
        replace_import: Option<(*mut BuildLinker, i32, i32)>,
    }
    thread_local! { static LOG: RefCell<Log> = RefCell::new(Log::default()); }
    fn reset() {
        LOG.with(|log| *log.borrow_mut() = Log::default());
    }
    fn event(value: Event) -> i32 {
        LOG.with(|log| {
            let mut log = log.borrow_mut();
            let status = if log.fail.as_ref() == Some(&value) {
                61
            } else {
                0
            };
            log.events.push(value);
            status
        })
    }
    fn events() -> Vec<Event> {
        LOG.with(|log| log.borrow().events.clone())
    }
    fn fail(value: Event) {
        LOG.with(|log| log.borrow_mut().fail = Some(value));
    }

    struct BuildLinker {
        root: i32,
        flags: u32,
        exports: i32,
        names: Vec<i32>,
        generations: Vec<(i32, i32)>,
        imports: Vec<(bool, i32, i32)>,
    }
    impl BuildLinker {
        fn new(root: i32, names: &[i32]) -> Box<Self> {
            Box::new(Self {
                root,
                flags: 0,
                exports: 5,
                names: names.to_vec(),
                generations: vec![(2, 1), (5, names.len() as i32)],
                imports: vec![],
            })
        }
        fn ptr(&mut self) -> *mut c_void {
            (self as *mut Self).cast()
        }
    }
    struct BuildPackage {
        linker: *mut c_void,
        parent: *mut c_void,
        object_base: i32,
        object_count: i32,
        name_base: i32,
        name_count: i32,
        local: i32,
        remote: i32,
    }
    impl BuildPackage {
        fn new(linker: &mut BuildLinker) -> Self {
            Self {
                linker: linker.ptr(),
                parent: identity(linker.root as usize),
                object_base: -1,
                object_count: -1,
                name_base: -1,
                name_count: -1,
                local: 0,
                remote: 0,
            }
        }
    }
    #[derive(Default)]
    struct BuildMap {
        packages: Vec<BuildPackage>,
        names: Vec<i32>,
        linkers: Vec<(*mut c_void, i32)>,
        max_objects: u32,
        max_names: u32,
        loaders: ObjectCollection,
    }
    impl BuildMap {
        fn view(&mut self) -> Hp2PackageMapBuildView {
            let owner = (self as *mut Self).cast();
            Hp2PackageMapBuildView {
                owner,
                packages: owner,
                linker_map: owner,
                names: owner,
                loaders: &mut self.loaders,
                max_objects: &mut self.max_objects,
                max_names: &mut self.max_names,
            }
        }
        fn add(&mut self, linker: &mut BuildLinker) -> Result<i32, i32> {
            unsafe { package_map::add_linker(&self.view(), linker.ptr(), &SERVICES) }
        }
        fn compute(&mut self) -> Result<(), i32> {
            unsafe { package_map::compute(&self.view(), &SERVICES) }
        }
        fn serialize(&mut self, archive: &mut Archive, name: &mut i32) -> Result<i32, i32> {
            unsafe {
                package_map::serialize_name(
                    &self.view(),
                    (archive as *mut Archive).cast(),
                    (name as *mut i32).cast(),
                    &SERVICES,
                )
            }
        }
    }
    struct Archive {
        loading: bool,
        error: bool,
        wire: u32,
        seen: Option<(u32, u32)>,
    }
    impl Archive {
        fn new(loading: bool, wire: u32) -> Self {
            Self {
                loading,
                error: false,
                wire,
                seen: None,
            }
        }
    }
    unsafe extern "C" fn package_len(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildMap>()).packages.len() as i32 }
    }
    unsafe extern "C" fn package_at(
        p: *mut c_void,
        i: i32,
        output: *mut Hp2PackageInfoView,
    ) -> i32 {
        let status = event(Event::Package(i));
        if status != 0 {
            return status;
        }
        unsafe {
            let map = &mut *p.cast::<BuildMap>();
            let Some(info) = map.packages.get_mut(i as usize) else {
                return 61;
            };
            *output = Hp2PackageInfoView {
                linker: &mut info.linker,
                parent: info.parent,
                object_base: &mut info.object_base,
                object_count: &mut info.object_count,
                name_base: &mut info.name_base,
                name_count: &mut info.name_count,
                local_generation: &mut info.local,
                remote_generation: &mut info.remote,
            };
        }
        0
    }
    unsafe extern "C" fn linker_flags(p: *mut c_void) -> u32 {
        unsafe { (*p.cast::<BuildLinker>()).flags }
    }
    unsafe extern "C" fn linker_root(p: *mut c_void) -> *mut c_void {
        unsafe { identity((*p.cast::<BuildLinker>()).root as usize) }
    }
    unsafe extern "C" fn append_package(p: *mut c_void, linker: *mut c_void) -> i32 {
        unsafe {
            let linker = &mut *linker.cast::<BuildLinker>();
            let status = event(Event::Append(linker.root));
            if status == 0 {
                (*p.cast::<BuildMap>())
                    .packages
                    .push(BuildPackage::new(linker));
            }
            status
        }
    }
    unsafe extern "C" fn import_len(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildLinker>()).imports.len() as i32 }
    }
    unsafe extern "C" fn import_at(
        p: *mut c_void,
        i: i32,
        output: *mut Hp2PackageImportView,
    ) -> i32 {
        unsafe {
            let linker = &*p.cast::<BuildLinker>();
            let status = event(Event::Import(linker.root, i));
            if status != 0 {
                return status;
            }
            let Some(&(package, package_index, object_name)) = linker.imports.get(i as usize)
            else {
                return 61;
            };
            *output = Hp2PackageImportView {
                is_package: i32::from(package),
                package_index,
                object_name,
            };
            0
        }
    }
    unsafe extern "C" fn root_name(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildLinker>()).root }
    }
    unsafe extern "C" fn add(p: *mut c_void, linker: *mut c_void, result: *mut i32) -> i32 {
        unsafe {
            let root = (*linker.cast::<BuildLinker>()).root;
            let status = event(Event::Add(root));
            if status != 0 {
                return status;
            }
            let view = (*p.cast::<BuildMap>()).view();
            match package_map::add_linker(&view, linker, &SERVICES) {
                Ok(index) => *result = index,
                Err(status) => return status,
            }
            LOG.with(|log| {
                let mut log = log.borrow_mut();
                if let Some((map, extra, trigger)) = log.grow_source {
                    if trigger == root {
                        log.grow_source = None;
                        (*map).packages.push(BuildPackage::new(&mut *extra));
                    }
                }
                if let Some((map, extra, trigger)) = log.grow_loaders {
                    if trigger == root {
                        log.grow_loaders = None;
                        (*map).loaders.append((*extra).ptr());
                    }
                }
                if let Some((owner, name, trigger)) = log.replace_import {
                    if trigger == root {
                        log.replace_import = None;
                        (&mut (*owner).imports)[0].2 = name;
                    }
                }
            });
            0
        }
    }
    unsafe extern "C" fn compute(p: *mut c_void) -> i32 {
        let status = event(Event::Compute);
        if status != 0 {
            return status;
        }
        unsafe {
            match package_map::compute(&(*p.cast::<BuildMap>()).view(), &SERVICES) {
                Ok(()) => 0,
                Err(status) => status,
            }
        }
    }
    unsafe extern "C" fn check_linker(p: *mut c_void) -> i32 {
        let status = event(Event::Check);
        if status != 0 {
            status
        } else if p.is_null() {
            61
        } else {
            0
        }
    }
    unsafe extern "C" fn max_global_names() -> i32 {
        16
    }
    unsafe extern "C" fn names_empty(p: *mut c_void, _: i32) -> i32 {
        let status = event(Event::EmptyNames);
        if status == 0 {
            unsafe {
                (*p.cast::<BuildMap>()).names.clear();
            }
        }
        status
    }
    unsafe extern "C" fn names_add(p: *mut c_void, count: i32) -> i32 {
        let status = event(Event::AddNames);
        if status == 0 {
            unsafe {
                (*p.cast::<BuildMap>()).names.resize(count as usize, 123);
            }
        }
        status
    }
    unsafe extern "C" fn names_len(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildMap>()).names.len() as i32 }
    }
    unsafe extern "C" fn name_slot(p: *mut c_void, i: i32, output: *mut *mut i32) -> i32 {
        let status = event(Event::Slot(i));
        if status == 0 {
            unsafe {
                let map = &mut *p.cast::<BuildMap>();
                let Some(slot) = map.names.get_mut(i as usize) else {
                    return 61;
                };
                *output = slot;
            }
        }
        status
    }
    unsafe extern "C" fn linkers_empty(p: *mut c_void) -> i32 {
        let status = event(Event::EmptyLinkers);
        if status == 0 {
            unsafe {
                (*p.cast::<BuildMap>()).linkers.clear();
            }
        }
        status
    }
    unsafe extern "C" fn export_len(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildLinker>()).exports }
    }
    unsafe extern "C" fn name_len(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildLinker>()).names.len() as i32 }
    }
    unsafe extern "C" fn generation_len(p: *mut c_void) -> i32 {
        unsafe { (*p.cast::<BuildLinker>()).generations.len() as i32 }
    }
    unsafe extern "C" fn generation_exports(p: *mut c_void, i: i32, out: *mut i32) -> i32 {
        let status = event(Event::GenerationExports(i));
        if status == 0 {
            unsafe {
                let linker = &*p.cast::<BuildLinker>();
                let Some(&(exports, _)) = linker.generations.get(i as usize) else {
                    return 61;
                };
                *out = exports;
            }
        }
        status
    }
    unsafe extern "C" fn generation_names(p: *mut c_void, i: i32, out: *mut i32) -> i32 {
        let status = event(Event::GenerationNames(i));
        if status == 0 {
            unsafe {
                let linker = &*p.cast::<BuildLinker>();
                let Some(&(_, names)) = linker.generations.get(i as usize) else {
                    return 61;
                };
                *out = names;
            }
        }
        status
    }
    unsafe extern "C" fn linker_name(p: *mut c_void, i: i32, out: *mut i32) -> i32 {
        unsafe {
            let linker = &*p.cast::<BuildLinker>();
            let status = event(Event::LinkerName(linker.root, i));
            if status != 0 {
                return status;
            }
            let Some(&name) = linker.names.get(i as usize) else {
                return 61;
            };
            *out = name;
            0
        }
    }
    unsafe extern "C" fn set_linker(p: *mut c_void, linker: *mut c_void, i: i32) -> i32 {
        let status = event(Event::SetLinker(i));
        if status == 0 {
            unsafe {
                (*p.cast::<BuildMap>()).linkers.push((linker, i));
            }
        }
        status
    }
    unsafe extern "C" fn name_index(p: *mut c_void) -> i32 {
        unsafe { *p.cast::<i32>() }
    }
    unsafe extern "C" fn serialize_int(p: *mut c_void, index: *mut u32, maximum: u32) -> i32 {
        let status = event(Event::Serialize);
        if status == 0 {
            unsafe {
                let archive = &mut *p.cast::<Archive>();
                archive.seen = Some((*index, maximum));
                if archive.loading {
                    *index = archive.wire;
                } else {
                    archive.wire = *index;
                }
            }
        }
        status
    }
    unsafe extern "C" fn is_loading(p: *mut c_void) -> i32 {
        event(Event::Loading);
        unsafe { i32::from((*p.cast::<Archive>()).loading) }
    }
    unsafe extern "C" fn is_error(p: *mut c_void) -> i32 {
        event(Event::Error);
        unsafe { i32::from((*p.cast::<Archive>()).error) }
    }
    unsafe extern "C" fn name_none(p: *mut c_void) {
        event(Event::None);
        unsafe {
            *p.cast::<i32>() = 0;
        }
    }
    unsafe extern "C" fn select_name(p: *mut c_void, i: i32, out: *mut c_void) -> i32 {
        unsafe {
            let linker = &*p.cast::<BuildLinker>();
            let status = event(Event::Select(linker.root, i));
            if status != 0 {
                return status;
            }
            let Some(&name) = linker.names.get(i as usize) else {
                return 61;
            };
            *out.cast::<i32>() = name;
            0
        }
    }
    const SERVICES: Hp2PackageMapBuildServices = Hp2PackageMapBuildServices {
        package_len,
        package_at,
        linker_flags,
        linker_root,
        append_package,
        import_len,
        import_at,
        root_name,
        add_linker: add,
        compute,
        check_linker,
        max_global_names,
        names_empty,
        names_add,
        names_len,
        name_slot,
        linkers_empty,
        export_len,
        name_len,
        generation_len,
        generation_exports,
        generation_names,
        linker_name,
        set_linker,
        name_index,
        serialize_int,
        is_loading,
        is_error,
        name_none,
        select_name,
    };

    #[test]
    fn admission_handles_cycles_duplicate_roots_server_only_and_import_filters() {
        reset();
        let mut a = BuildLinker::new(1, &[2]);
        let mut b = BuildLinker::new(2, &[3]);
        let mut duplicate = BuildLinker::new(2, &[4]);
        let mut server = BuildLinker::new(3, &[5]);
        let mut ignored = BuildLinker::new(4, &[6]);
        server.flags = 4;
        a.imports = vec![(true, 0, 2), (true, 0, 3), (false, 0, 4), (true, 1, 4)];
        b.imports = vec![(true, 0, 1)];
        let mut map = BuildMap::default();
        for loader in [
            a.ptr(),
            b.ptr(),
            duplicate.ptr(),
            server.ptr(),
            ignored.ptr(),
        ] {
            map.loaders.append(loader);
        }
        assert_eq!(map.add(&mut a), Ok(0));
        assert_eq!(
            map.packages.iter().map(|p| p.parent).collect::<Vec<_>>(),
            vec![identity(1), identity(2)]
        );
        assert_eq!(map.packages[1].linker, b.ptr());
        assert_eq!(map.add(&mut duplicate), Ok(1));
        // Server-only rejection precedes even an already-present root match.
        duplicate.flags = 4;
        reset();
        assert_eq!(map.add(&mut duplicate), Ok(-1));
        assert!(events().is_empty());
    }

    #[test]
    fn recursive_admission_rereads_live_loader_length_and_import_name() {
        reset();
        let mut a = BuildLinker::new(1, &[1]);
        let mut b = BuildLinker::new(2, &[2]);
        let mut c = BuildLinker::new(3, &[3]);
        a.imports = vec![(true, 0, 2)];
        let mut map = BuildMap::default();
        map.loaders.append(b.ptr());
        LOG.with(|log| {
            let mut log = log.borrow_mut();
            log.grow_loaders = Some((&mut map, &mut *c, 2));
            log.replace_import = Some((&mut *a, 3, 2));
        });
        assert_eq!(map.add(&mut a), Ok(0));
        assert_eq!(
            map.packages.iter().map(|p| p.parent).collect::<Vec<_>>(),
            vec![identity(1), identity(2), identity(3)]
        );
    }

    #[test]
    fn compute_clamps_generations_and_keeps_first_global_name_index() {
        reset();
        let mut a = BuildLinker::new(1, &[3, 4, 5]);
        let mut b = BuildLinker::new(2, &[3, 6, 6]);
        let mut c = BuildLinker::new(3, &[7]);
        let mut map = BuildMap::default();
        assert_eq!(map.add(&mut a), Ok(0));
        assert_eq!(map.add(&mut b), Ok(1));
        assert_eq!(map.add(&mut c), Ok(2));
        map.packages[0].remote = 1;
        map.packages[2].remote = 9; // Future remote generations are not clamped down.
        assert_eq!(map.compute(), Ok(()));
        assert_eq!((map.max_objects, map.max_names), (12, 5));
        let ranges: Vec<_> = map
            .packages
            .iter()
            .map(|p| {
                (
                    p.object_base,
                    p.object_count,
                    p.name_base,
                    p.name_count,
                    p.local,
                    p.remote,
                )
            })
            .collect();
        assert_eq!(
            ranges,
            vec![(0, 2, 0, 1, 2, 1), (2, 5, 1, 3, 2, 2), (7, 5, 4, 1, 2, 9)]
        );
        assert_eq!(
            (
                map.names[3],
                map.names[4],
                map.names[5],
                map.names[6],
                map.names[7]
            ),
            (0, -1, -1, 2, 4)
        );
        assert_eq!(map.linkers, vec![(a.ptr(), 0), (b.ptr(), 1), (c.ptr(), 2)]);
    }

    #[test]
    fn serialize_name_preserves_sentinel_unmapped_slot_and_load_selection() {
        reset();
        let mut a = BuildLinker::new(1, &[2, 3]);
        let mut b = BuildLinker::new(2, &[4]);
        let mut map = BuildMap::default();
        assert_eq!(map.add(&mut a), Ok(0));
        assert_eq!(map.add(&mut b), Ok(1));
        assert_eq!(map.compute(), Ok(()));
        let mut save = Archive::new(false, 0);
        let mut name = 4;
        assert_eq!(map.serialize(&mut save, &mut name), Ok(1));
        assert_eq!(save.seen, Some((2, 4)));
        name = 16; // Outside the global table uses MaxNameIndex.
        assert_eq!(map.serialize(&mut save, &mut name), Ok(0));
        assert_eq!(save.wire, 3);
        name = 8; // An in-table -1 is deliberately not changed to the sentinel.
        assert_eq!(map.serialize(&mut save, &mut name), Ok(1));
        assert_eq!(save.wire, u32::MAX);
        for (wire, error, expected) in [
            (0, false, 2),
            (2, false, 4),
            (3, false, 0),
            (99, false, 0),
            (u32::MAX, false, 0),
            (1, true, 0),
        ] {
            reset();
            let mut load = Archive::new(true, wire);
            load.error = error;
            name = 7;
            assert_eq!(map.serialize(&mut load, &mut name), Ok(1));
            assert_eq!(name, expected);
            if expected == 0 {
                assert!(
                    !events()
                        .iter()
                        .any(|e| matches!(e, Event::Package(_) | Event::Select(..)))
                );
            }
            if wire >= 3 {
                assert!(!events().contains(&Event::Error));
            }
        }
    }

    #[test]
    fn copy_linkers_skips_nulls_rereads_source_and_computes_last() {
        reset();
        let mut a = BuildLinker::new(1, &[2]);
        let mut b = BuildLinker::new(2, &[3]);
        let mut source = BuildMap::default();
        source.packages.push(BuildPackage::new(&mut a));
        let mut missing = BuildPackage::new(&mut a);
        missing.linker = null_mut();
        source.packages.push(missing);
        let mut destination = BuildMap::default();
        LOG.with(|log| log.borrow_mut().grow_source = Some((&mut source, &mut *b, 1)));
        assert_eq!(
            unsafe {
                package_map::copy_linkers(&destination.view(), source.view().packages, &SERVICES)
            },
            Ok(())
        );
        assert_eq!(destination.packages.len(), 2);
        assert_eq!((destination.max_objects, destination.max_names), (10, 2));
        let log = events();
        let compute_at = log.iter().position(|e| *e == Event::Compute).unwrap();
        assert_eq!(
            log[..compute_at]
                .iter()
                .filter(|e| matches!(e, Event::Add(_)))
                .count(),
            2
        );
        assert!(!log[compute_at..].iter().any(|e| matches!(e, Event::Add(_))));
    }

    #[test]
    fn admission_and_copy_failures_do_not_continue_or_compute() {
        reset();
        let mut a = BuildLinker::new(1, &[1]);
        let mut b = BuildLinker::new(2, &[2]);
        a.imports = vec![(true, 0, 2)];
        let mut map = BuildMap::default();
        map.loaders.append(b.ptr());
        fail(Event::Append(1));
        assert_eq!(map.add(&mut a), Err(61));
        assert!(map.packages.is_empty());
        assert_eq!(events(), vec![Event::Append(1)]);
        reset();
        fail(Event::Add(2));
        assert_eq!(map.add(&mut a), Err(61));
        assert_eq!(map.packages.len(), 1); // No invented rollback.
        assert_eq!(events().last(), Some(&Event::Add(2)));
        reset();
        let mut source = BuildMap::default();
        source.packages = vec![BuildPackage::new(&mut a), BuildPackage::new(&mut b)];
        let mut destination = BuildMap::default();
        fail(Event::Add(1));
        assert_eq!(
            unsafe {
                package_map::copy_linkers(&destination.view(), source.view().packages, &SERVICES)
            },
            Err(61)
        );
        assert_eq!(
            events(),
            vec![Event::Package(0), Event::Package(0), Event::Add(1)]
        );
        assert!(destination.packages.is_empty());
    }

    #[test]
    fn compute_failure_preserves_exact_partial_state_and_stops() {
        reset();
        let mut a = BuildLinker::new(1, &[2, 3]);
        let mut b = BuildLinker::new(2, &[4]);
        let mut map = BuildMap::default();
        assert_eq!(map.add(&mut a), Ok(0));
        assert_eq!(map.add(&mut b), Ok(1));
        map.packages[0].remote = 1;
        fail(Event::GenerationNames(0));
        assert_eq!(map.compute(), Err(61));
        assert_eq!(events().last(), Some(&Event::GenerationNames(0)));
        assert_eq!(
            (map.packages[0].object_count, map.packages[0].name_count),
            (2, 2)
        );
        assert_eq!((map.max_objects, map.max_names), (0, 0));
        assert_eq!(map.packages[1].object_base, -1);
        assert!(map.linkers.is_empty());
        assert!(map.names.iter().all(|&n| n == -1));
        reset();
        fail(Event::SetLinker(0));
        assert_eq!(map.compute(), Err(61));
        assert_eq!(events().last(), Some(&Event::SetLinker(0)));
        assert_eq!((map.max_objects, map.max_names), (2, 1));
        assert_eq!(map.names[2], 0);
        assert_eq!(map.packages[1].object_base, -1);
        assert!(map.linkers.is_empty());
    }

    #[test]
    fn archive_and_selected_name_failures_stop_at_the_failed_leaf() {
        reset();
        let mut a = BuildLinker::new(1, &[2]);
        let mut map = BuildMap::default();
        assert_eq!(map.add(&mut a), Ok(0));
        assert_eq!(map.compute(), Ok(()));
        reset();
        fail(Event::Serialize);
        let mut load = Archive::new(true, 0);
        let mut name = 16;
        assert_eq!(map.serialize(&mut load, &mut name), Err(61));
        assert_eq!(name, 16);
        assert_eq!(events(), vec![Event::Serialize]);
        reset();
        fail(Event::Select(1, 0));
        assert_eq!(map.serialize(&mut load, &mut name), Err(61));
        assert_eq!(name, 0);
        assert_eq!(events().last(), Some(&Event::Select(1, 0)));
    }

    #[test]
    fn compute_checks_all_linkers_before_reset_and_retains_generation_preconditions() {
        reset();
        let mut a = BuildLinker::new(1, &[2, 3]);
        let mut map = BuildMap::default();
        map.packages.push(BuildPackage::new(&mut a));
        let mut absent = BuildPackage::new(&mut a);
        absent.linker = null_mut();
        map.packages.push(absent);
        map.names = vec![42];
        map.max_objects = 17;
        map.max_names = 19;
        assert_eq!(map.compute(), Err(61));
        assert_eq!(
            events(),
            vec![
                Event::Package(0),
                Event::Check,
                Event::Package(1),
                Event::Check
            ]
        );
        assert_eq!(map.names, vec![42]);
        assert_eq!((map.max_objects, map.max_names), (17, 19));
        map.packages.pop();
        map.packages[0].remote = -1;
        reset();
        // The checked test leaf reports the original invalid generation access;
        // Rust must not silently substitute the local generation or zero counts.
        assert_eq!(map.compute(), Err(61));
        assert_eq!(events().last(), Some(&Event::GenerationExports(-2)));
        assert_eq!(map.packages[0].remote, -1);
        assert_eq!(
            (map.packages[0].object_count, map.packages[0].name_count),
            (5, 2)
        );
    }
}

mod class_cache {
    use super::{RefCell, c_void, null_mut};
    use hp2_core::package_map::{
        Hp2ClassNetCacheServices, Hp2ClassNetCacheView, Hp2ClassNetFieldView, get_class_net_cache,
    };
    use std::collections::HashMap;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Event(&'static str, i32);
    #[derive(Default)]
    struct Log {
        events: Vec<Event>,
        fail_at: Option<usize>,
    }
    thread_local! { static LOG: RefCell<Log> = RefCell::new(Log::default()); }
    fn reset(fail_at: Option<usize>) {
        LOG.with(|log| {
            *log.borrow_mut() = Log {
                events: vec![],
                fail_at,
            }
        });
    }
    fn event(name: &'static str, id: i32) -> i32 {
        LOG.with(|log| {
            let mut log = log.borrow_mut();
            let fail = log.fail_at == Some(log.events.len());
            log.events.push(Event(name, id));
            if fail { 73 } else { 0 }
        })
    }
    fn events() -> Vec<Event> {
        LOG.with(|log| log.borrow().events.clone())
    }

    struct Object {
        id: i32,
        admitted: bool,
        property: bool,
        owner: *mut c_void,
        native_owner: bool,
        superclass: *mut c_void,
        fields: Vec<*mut c_void>,
    }
    impl Object {
        fn new(id: i32, property: bool) -> Box<Self> {
            let mut value = Box::new(Self {
                id,
                admitted: true,
                property,
                owner: null_mut(),
                native_owner: false,
                superclass: null_mut(),
                fields: vec![],
            });
            value.owner = value.ptr();
            value
        }
        fn ptr(&mut self) -> *mut c_void {
            (self as *mut Self).cast()
        }
    }
    #[derive(Clone)]
    struct Entry {
        field: *mut c_void,
        index: i32,
        condition: i32,
    }
    struct Cache {
        class: *mut c_void,
        super_cache: *mut c_void,
        conditions: i32,
        base: i32,
        fields: Vec<Entry>,
        field_map: HashMap<usize, *mut Entry>,
        rep_properties: Vec<*mut Entry>,
        shrunk: bool,
    }
    impl Cache {
        fn new(class: *mut c_void) -> Self {
            Self {
                class,
                super_cache: null_mut(),
                conditions: -99,
                base: -99,
                fields: vec![],
                field_map: HashMap::new(),
                rep_properties: vec![],
                shrunk: false,
            }
        }
        fn ptr(&mut self) -> *mut c_void {
            (self as *mut Self).cast()
        }
        unsafe fn from_field(&self, field: *mut c_void) -> *mut Entry {
            if let Some(entry) = self.field_map.get(&(field as usize)) {
                *entry
            } else if !self.super_cache.is_null() {
                unsafe { (&*self.super_cache.cast::<Cache>()).from_field(field) }
            } else {
                null_mut()
            }
        }
        unsafe fn from_index(&self, index: i32) -> Option<&Entry> {
            if index >= self.base && index < self.base + self.fields.len() as i32 {
                Some(&self.fields[(index - self.base) as usize])
            } else if !self.super_cache.is_null() {
                unsafe { (&*self.super_cache.cast::<Cache>()).from_index(index) }
            } else {
                None
            }
        }
    }
    #[derive(Default)]
    struct Map {
        caches: HashMap<usize, Box<Cache>>,
        recursive_probe: *mut c_void,
        observed_published: bool,
        override_cache: *mut c_void,
    }
    impl Map {
        fn get(&mut self, class: *mut c_void) -> Result<*mut c_void, i32> {
            unsafe { get_class_net_cache((self as *mut Self).cast(), class, &SERVICES) }
        }
    }
    unsafe fn object<'a>(p: *mut c_void) -> &'a mut Object {
        unsafe { &mut *p.cast::<Object>() }
    }
    unsafe fn cache<'a>(p: *mut c_void) -> &'a mut Cache {
        unsafe { &mut *p.cast::<Cache>() }
    }
    unsafe fn cache_event(name: &'static str, p: *mut c_void) -> i32 {
        unsafe { event(name, object(cache(p).class).id) }
    }
    unsafe extern "C" fn find_cache(m: *mut c_void, c: *mut c_void, out: *mut *mut c_void) -> i32 {
        unsafe {
            let status = event("find", object(c).id);
            if status != 0 {
                return status;
            }
            *out = (*m.cast::<Map>())
                .caches
                .get_mut(&(c as usize))
                .map_or(null_mut(), |cache| cache.ptr());
            0
        }
    }
    unsafe extern "C" fn object_to_index(_: *mut c_void, o: *mut c_void, out: *mut i32) -> i32 {
        unsafe {
            let status = event("index", object(o).id);
            if status != 0 {
                return status;
            }
            *out = if object(o).admitted { object(o).id } else { -1 };
            0
        }
    }
    unsafe extern "C" fn publish_cache(
        m: *mut c_void,
        c: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            let status = event("publish", object(c).id);
            if status != 0 {
                return status;
            }
            let mut value = Box::new(Cache::new(c));
            *out = value.ptr();
            (*m.cast::<Map>()).caches.insert(c as usize, value);
            0
        }
    }
    unsafe extern "C" fn cache_view(c: *mut c_void, out: *mut Hp2ClassNetCacheView) {
        unsafe {
            let c = cache(c);
            *out = Hp2ClassNetCacheView {
                super_cache: &mut c.super_cache,
                condition_count: &mut c.conditions,
                fields_base: &mut c.base,
            };
        }
    }
    unsafe extern "C" fn super_class(c: *mut c_void, out: *mut *mut c_void) -> i32 {
        unsafe {
            let status = event("super", object(c).id);
            if status != 0 {
                return status;
            }
            *out = object(c).superclass;
            0
        }
    }
    unsafe extern "C" fn get_class_cache(
        m: *mut c_void,
        c: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            let status = event("virtual_cache", object(c).id);
            if status != 0 {
                return status;
            }
            let probe = (*m.cast::<Map>()).recursive_probe;
            if !probe.is_null() {
                (*m.cast::<Map>()).recursive_probe = null_mut();
                let result = get_class_net_cache(m, probe, &SERVICES);
                let found = match result {
                    Ok(value) => value,
                    Err(status) => return status,
                };
                (*m.cast::<Map>()).observed_published = !found.is_null()
                    && cache(found).super_cache.is_null()
                    && cache(found).conditions == 0
                    && cache(found).base == 0;
            }
            if !(*m.cast::<Map>()).override_cache.is_null() {
                *out = (*m.cast::<Map>()).override_cache;
                return 0;
            }
            match get_class_net_cache(m, c, &SERVICES) {
                Ok(value) => {
                    *out = value;
                    0
                }
                Err(status) => status,
            }
        }
    }
    unsafe extern "C" fn copy_rep_properties(c: *mut c_void, s: *mut c_void) -> i32 {
        unsafe {
            let status = cache_event("copy", c);
            if status != 0 {
                return status;
            }
            let properties = cache(s).rep_properties.clone();
            cache(c).rep_properties = properties;
            0
        }
    }
    unsafe extern "C" fn net_fields_len(c: *mut c_void) -> i32 {
        unsafe { object(c).fields.len() as i32 }
    }
    unsafe extern "C" fn net_field_at(c: *mut c_void, i: i32, out: *mut *mut c_void) -> i32 {
        unsafe {
            let field = object(c).fields[i as usize];
            let status = event("net_field", object(field).id);
            if status != 0 {
                return status;
            }
            *out = field;
            0
        }
    }
    unsafe extern "C" fn fields_empty(c: *mut c_void, count: i32) -> i32 {
        unsafe {
            let status = cache_event("empty", c);
            if status != 0 {
                return status;
            }
            cache(c).fields = Vec::with_capacity(count as usize);
            0
        }
    }
    unsafe extern "C" fn fields_len(c: *mut c_void) -> i32 {
        unsafe { cache(c).fields.len() as i32 }
    }
    unsafe extern "C" fn field_property(f: *mut c_void, out: *mut *mut c_void) -> i32 {
        unsafe {
            let status = event("property", object(f).id);
            if status != 0 {
                return status;
            }
            *out = if object(f).property { f } else { null_mut() };
            0
        }
    }
    unsafe extern "C" fn rep_owner(p: *mut c_void) -> *mut c_void {
        unsafe { object(p).owner }
    }
    unsafe extern "C" fn append_field(
        c: *mut c_void,
        f: *mut c_void,
        index: i32,
        condition: i32,
    ) -> i32 {
        unsafe {
            let status = event("append", object(f).id);
            if status != 0 {
                return status;
            }
            cache(c).fields.push(Entry {
                field: f,
                index,
                condition,
            });
            0
        }
    }
    unsafe extern "C" fn fields_shrink(c: *mut c_void) -> i32 {
        unsafe {
            let status = cache_event("shrink", c);
            if status != 0 {
                return status;
            }
            // Force relocation so a prematurely published field pointer is stale.
            let relocated = cache(c).fields.clone();
            cache(c).fields = relocated;
            cache(c).shrunk = true;
            0
        }
    }
    unsafe extern "C" fn field_at(c: *mut c_void, i: i32, out: *mut Hp2ClassNetFieldView) -> i32 {
        unsafe {
            let field = &mut cache(c).fields[i as usize];
            let status = event("field", object(field.field).id);
            if status != 0 {
                return status;
            }
            *out = Hp2ClassNetFieldView {
                field: field.field,
                condition_index: &mut field.condition,
            };
            0
        }
    }
    unsafe extern "C" fn set_field_map(c: *mut c_void, i: i32) -> i32 {
        unsafe {
            let c = cache(c);
            let field = &mut c.fields[i as usize];
            let status = event("field_map", object(field.field).id);
            if status != 0 {
                return status;
            }
            if !c.shrunk {
                return 74;
            }
            c.field_map.insert(field.field as usize, field);
            0
        }
    }
    unsafe extern "C" fn owner_condition(c: *mut c_void, o: *mut c_void, out: *mut i32) -> i32 {
        unsafe {
            let status = event("owner_condition", object(o).id);
            if status != 0 {
                return status;
            }
            let entry = cache(c).from_field(o);
            if entry.is_null() {
                return 75;
            }
            *out = (*entry).condition;
            0
        }
    }
    unsafe extern "C" fn native_replication(p: *mut c_void, out: *mut i32) -> i32 {
        unsafe {
            let status = event("native", object(p).id);
            if status != 0 {
                return status;
            }
            *out = i32::from(object(p).native_owner);
            0
        }
    }
    unsafe extern "C" fn add_rep_property(c: *mut c_void, i: i32) -> i32 {
        unsafe {
            let c = cache(c);
            let field = &mut c.fields[i as usize];
            let status = event("rep_property", object(field.field).id);
            if status != 0 {
                return status;
            }
            c.rep_properties.push(field);
            0
        }
    }
    const SERVICES: Hp2ClassNetCacheServices = Hp2ClassNetCacheServices {
        find_cache,
        object_to_index,
        publish_cache,
        cache_view,
        super_class,
        get_class_cache,
        copy_rep_properties,
        net_fields_len,
        net_field_at,
        fields_empty,
        fields_len,
        field_property,
        rep_owner,
        append_field,
        fields_shrink,
        field_at,
        set_field_map,
        owner_condition,
        native_replication,
        add_rep_property,
    };

    struct Fixture {
        // Boxes keep opaque object addresses stable throughout recursive calls.
        objects: Vec<Box<Object>>,
        map: Map,
        base: *mut c_void,
        derived: *mut c_void,
    }
    impl Fixture {
        fn new() -> Self {
            let mut objects: Vec<_> = (1..=14)
                .map(|id| Object::new(id, id != 2 && id != 10))
                .collect();
            let fields: Vec<_> = objects.iter_mut().map(|o| o.ptr()).collect();
            objects[3].admitted = false;
            objects[8].admitted = false;
            objects[2].native_owner = true;
            objects[10].native_owner = true;
            for (property, owner) in [(5, 1), (6, 7), (8, 9), (12, 11), (13, 9), (14, 3)] {
                objects[property - 1].owner = fields[owner - 1];
            }
            let mut base = Object::new(100, false);
            base.fields = fields[..3].to_vec();
            let base_ptr = base.ptr();
            let mut derived = Object::new(200, false);
            derived.superclass = base_ptr;
            derived.fields = [4, 5, 6, 7, 8, 10, 11, 12, 13, 14]
                .into_iter()
                .map(|id| fields[id - 1])
                .collect();
            let derived_ptr = derived.ptr();
            objects.push(base);
            objects.push(derived);
            Self {
                objects,
                map: Map {
                    recursive_probe: derived_ptr,
                    ..Map::default()
                },
                base: base_ptr,
                derived: derived_ptr,
            }
        }
        fn get(&mut self) -> Result<*mut c_void, i32> {
            self.map.get(self.derived)
        }
    }

    #[test]
    fn inheritance_indices_conditions_and_replication_are_consumer_visible() {
        reset(None);
        let mut f = Fixture::new();
        let result = f.get().unwrap();
        assert!(f.map.observed_published);
        let c = unsafe { cache(result) };
        assert_eq!((c.base, c.conditions, c.fields.len()), (3, 6, 9));
        let expected = [
            (1, 0, 0),
            (2, 1, -1),
            (3, 2, 1),
            (5, 3, 0),
            (6, 4, 2),
            (7, 5, 2),
            (8, 6, 3),
            (10, 7, -1),
            (11, 8, 4),
            (12, 9, 4),
            (13, 10, 5),
            (14, 11, 1),
        ];
        for (id, index, condition) in expected {
            let field = f.objects[(id - 1) as usize].ptr();
            let entry = unsafe { c.from_index(index).unwrap() };
            assert_eq!(
                (entry.field, entry.index, entry.condition),
                (field, index, condition)
            );
            assert_eq!(
                unsafe { c.from_field(field) }.cast_const(),
                entry as *const Entry
            );
        }
        for id in [4, 9] {
            assert!(unsafe { c.from_field(f.objects[id - 1].ptr()) }.is_null());
        }
        assert!(unsafe { c.from_index(12) }.is_none());
        let rep_ids: Vec<_> = c
            .rep_properties
            .iter()
            .map(|p| unsafe { object((**p).field).id })
            .collect();
        assert_eq!(rep_ids, [1, 5, 6, 7, 8, 12, 13, 14]);
        for p in &c.rep_properties {
            assert_eq!(unsafe { c.from_field((**p).field) }, *p);
        }
        let log = events();
        assert_eq!(log.iter().filter(|e| **e == Event("index", 7)).count(), 2);
        assert_eq!(log.iter().filter(|e| **e == Event("index", 9)).count(), 2);
        assert!(!log.contains(&Event("owner_condition", 9)));
        assert!(!log.contains(&Event("property", 4)));
        let last_map = log.iter().rposition(|e| e.0 == "field_map").unwrap();
        let first_owner = log.iter().position(|e| e.0 == "owner_condition").unwrap();
        assert!(last_map < first_owner);
        // Existing caches bypass object admission even after the sandbox changes.
        unsafe {
            object(f.derived).admitted = false;
        }
        reset(None);
        assert_eq!(f.get(), Ok(result));
        assert_eq!(events(), [Event("find", 200)]);
    }

    #[test]
    fn rejected_class_is_not_published_and_lookup_errors_do_not_admit() {
        let mut f = Fixture::new();
        unsafe {
            object(f.derived).admitted = false;
        }
        reset(None);
        assert_eq!(f.get(), Ok(null_mut()));
        assert!(f.map.caches.is_empty());
        assert_eq!(events(), [Event("find", 200), Event("index", 200)]);
        reset(Some(0));
        assert_eq!(f.get(), Err(73));
        assert_eq!(events(), [Event("find", 200)]);
        assert!(f.map.caches.is_empty());
    }

    #[test]
    fn virtual_superclass_override_supplies_inherited_state() {
        reset(None);
        let mut f = Fixture::new();
        unsafe {
            object(f.derived).fields.clear();
        }
        let mut supplied = Box::new(Cache::new(f.base));
        supplied.conditions = 17;
        supplied.base = 8;
        supplied.fields.push(Entry {
            field: f.objects[0].ptr(),
            index: 8,
            condition: 16,
        });
        let field = &mut supplied.fields[0] as *mut Entry;
        supplied
            .field_map
            .insert(f.objects[0].ptr() as usize, field);
        supplied.rep_properties.push(field);
        f.map.override_cache = supplied.ptr();
        let result = f.get().unwrap();
        let c = unsafe { cache(result) };
        assert_eq!((c.base, c.conditions), (9, 17));
        assert_eq!(c.super_cache, supplied.ptr());
        assert_eq!(c.rep_properties, [field]);
        assert_eq!(unsafe { c.from_index(8).unwrap().condition }, 16);
        assert_eq!(f.map.caches.len(), 1);
        assert!(f.map.observed_published);
        assert!(!events().contains(&Event("find", 100)));
    }

    #[test]
    fn every_fallible_cache_leaf_stops_on_its_first_error() {
        reset(None);
        let mut f = Fixture::new();
        f.get().unwrap();
        let baseline = events();
        for failed in 0..baseline.len() {
            let mut f = Fixture::new();
            reset(Some(failed));
            assert_eq!(f.get(), Err(73), "failure at {:?}", baseline[failed]);
            assert_eq!(
                events(),
                baseline[..=failed],
                "failure at {:?}",
                baseline[failed]
            );
            // Published partial caches must remain visible, never retried or removed.
            if let Some(published) = f.map.caches.get_mut(&(f.derived as usize)) {
                let published = published.ptr();
                reset(None);
                assert_eq!(f.get(), Ok(published));
                assert_eq!(events(), [Event("find", 200)]);
            }
        }
    }

    #[test]
    fn failures_retain_condition_increments_and_inheritance_assignment_order() {
        reset(None);
        let mut baseline_fixture = Fixture::new();
        baseline_fixture.get().unwrap();
        let baseline = events();
        for (failed_event, expected_base, expected_conditions, expected_fields, expected_reps) in [
            (Event("copy", 200), 0, 0, 0, 0),
            (Event("empty", 200), 3, 2, 0, 1),
            (Event("append", 7), 3, 3, 2, 1),
            (Event("shrink", 200), 3, 6, 9, 1),
            (Event("field_map", 6), 3, 6, 9, 1),
            (Event("native", 6), 3, 6, 9, 2),
            (Event("rep_property", 8), 3, 6, 9, 4),
        ] {
            let failed = baseline
                .iter()
                .position(|event| *event == failed_event)
                .unwrap();
            let mut f = Fixture::new();
            reset(Some(failed));
            assert_eq!(f.get(), Err(73));
            let c = &f.map.caches[&(f.derived as usize)];
            assert!(!c.super_cache.is_null(), "{failed_event:?}");
            assert_eq!(
                (c.base, c.conditions, c.fields.len(), c.rep_properties.len()),
                (
                    expected_base,
                    expected_conditions,
                    expected_fields,
                    expected_reps
                ),
                "{failed_event:?}",
            );
            if failed_event == Event("field_map", 6) {
                assert_eq!(c.field_map.len(), 1);
            }
            if failed_event == Event("native", 6) {
                assert_eq!(c.fields[1].condition, 2);
            }
        }
    }

    #[test]
    fn condition_owner_resolution_is_one_ordered_pass_not_a_recursive_fixup() {
        reset(None);
        let mut f = Fixture::new();
        // 5 -> 6 -> 7, with both owners later in NetFields. Only 7 owns a
        // condition initially; legacy resolves 5 before 6 and leaves 5 at -1.
        f.objects[4].owner = f.objects[5].ptr();
        let result = f.get().unwrap();
        let c = unsafe { cache(result) };
        assert_eq!(unsafe { c.from_index(3).unwrap().condition }, -1);
        assert_eq!(unsafe { c.from_index(4).unwrap().condition }, 2);
        assert_eq!(unsafe { c.from_index(5).unwrap().condition }, 2);
        assert_eq!(c.conditions, 6);
        let owners: Vec<_> = events()
            .into_iter()
            .filter(|event| event.0 == "owner_condition")
            .collect();
        assert_eq!(
            owners,
            [
                Event("owner_condition", 6),
                Event("owner_condition", 7),
                Event("owner_condition", 11),
                Event("owner_condition", 3),
            ]
        );
    }

    #[test]
    fn admitted_but_absent_condition_owner_preserves_lookup_precondition() {
        reset(None);
        let mut f = Fixture::new();
        // An admitted RepOwner absent from this hierarchy is not treated as an
        // unmapped owner, and must not synthesize an independent condition.
        f.objects[8].admitted = true;
        assert_eq!(f.get(), Err(75));
        assert_eq!(events().last(), Some(&Event("owner_condition", 9)));
        let c = &f.map.caches[&(f.derived as usize)];
        assert_eq!(c.conditions, 4);
        assert_eq!(c.fields[3].condition, -1);
        let rep_ids: Vec<_> = c
            .rep_properties
            .iter()
            .map(|p| unsafe { object((**p).field).id })
            .collect();
        assert_eq!(rep_ids, [1, 5, 6, 7]);
    }
}
