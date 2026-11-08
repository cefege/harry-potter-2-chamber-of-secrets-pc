use hp2_abi::engine::generated::{self, FPoly, TArrayRaw, UClass, UObject, UModel, UPolys, UPrimitive};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_engine::EngineOwner;
use hp2_abi::render_scene::{FBspNode, FBspSurf};
use hp2_core::package_archive::ExportArchive;
use std::path::{Path, PathBuf};
use std::ptr;

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

static TRACKER_POOLS_RESTORED: AtomicBool = AtomicBool::new(false);
static TRACKER_MODEL_INDEX: AtomicI32 = AtomicI32::new(-1);
static TRACKER_LEVEL_INDEX: AtomicI32 = AtomicI32::new(-1);
static NEXT_MODEL_TEST_ROOT: AtomicUsize = AtomicUsize::new(0);

fn tracker_output(access: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    let model = access.object_at(TRACKER_MODEL_INDEX.load(Ordering::SeqCst)).cast::<UModel>();
    let level = access.object_at(TRACKER_LEVEL_INDEX.load(Ordering::SeqCst)).cast::<generated::ULevel>();
    if !model.is_null() && !level.is_null() && unsafe { (*level).brush_tracker.is_null() } {
        let pools = unsafe { [
            ptr::addr_of!((*model).surfs).cast::<TArrayRaw>().read_unaligned(),
            ptr::addr_of!((*model).vectors).cast::<TArrayRaw>().read_unaligned(),
            ptr::addr_of!((*model).points).cast::<TArrayRaw>().read_unaligned(),
            ptr::addr_of!((*model).nodes).cast::<TArrayRaw>().read_unaligned(),
            ptr::addr_of!((*model).verts).cast::<TArrayRaw>().read_unaligned(),
        ] };
        if pools.iter().all(|pool| pool.num == 1 && !pool.data.is_null()) {
            let vector = unsafe { pools[1].data.cast::<hp2_abi::FVector>().read_unaligned() };
            let point = unsafe { pools[2].data.cast::<hp2_abi::FVector>().read_unaligned() };
            if vector == (hp2_abi::FVector { x: 1.0, y: 2.0, z: 3.0 })
                && point == (hp2_abi::FVector { x: -4.0, y: 5.0, z: -6.0 })
            {
                TRACKER_POOLS_RESTORED.store(true, Ordering::SeqCst);
            }
        }
    }
    Ok(())
}

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn data_root() -> PathBuf {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR must name the pre-created test artifact directory"),
    );
    assert!(artifact.is_dir(), "HP2_ARTIFACT_DIR must exist before the consumer starts");
    let serial = NEXT_MODEL_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = artifact.join(format!("model-owner-{}-{serial}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove this test's previous synthetic fixture");
    }
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    std::fs::write(root.join("System/Default.ini"), "[Core.System]\nPaths=../System/*.u\n").expect("write empty default configuration");
    std::fs::write(root.join("System/DefUser.ini"), "").expect("write empty user configuration");
    root
}

fn options(root: &Path) -> CoreOptions {
    CoreOptions {
        data_root: root.to_path_buf(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    }
}

fn object_name(access: CoreAccess<'_>, object: *mut UObject) -> String {
    // SAFETY: the object is a live canonical UObject in this owner.
    let name = unsafe { ptr::read_unaligned(ptr::addr_of!((*object).name).cast::<i32>()) };
    access.name_text(name).expect("read canonical object name").into_iter()
        .filter_map(char::from_u32).collect()
}

fn assert_class(access: CoreAccess<'_>, name: &str) -> *mut UClass {
    let class = access.native_class(name).unwrap_or_else(|error| panic!("resolve {name}: {error}"));
    assert_eq!(object_name(access, class.cast()), name.split('.').nth(1).unwrap_or(name));
    // SAFETY: class is a live canonical UClass whose outer is its package.
    let package = unsafe { (*class.cast::<UObject>()).outer.cast() };
    assert_eq!(object_name(access, package), name.split('.').next().unwrap_or("Core"));
    class
}

unsafe fn set_values<T: Copy>(array: *mut TArrayRaw, values: &[T]) {
    let slots = Hp2ArraySlots {
        // SAFETY: the caller passes the address of a live native TArrayRaw.
        data: unsafe { ptr::addr_of_mut!((*array).data) },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    };
    assert_eq!(unsafe { slots.num.read() }, 0, "fresh model array has no live elements");
    assert!(unsafe { slots.data.read_unaligned() }.is_null(), "fresh model array has no allocation");
    let count = i32::try_from(values.len()).expect("fixture array count fits native INT");
    unsafe { slots.max.write(count); }
    unsafe {
        array_storage::realloc(
            slots,
            std::mem::size_of::<T>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }.expect("allocate model geometry with Core's paired system allocator");
    // SAFETY: allocation has `count` slots and no elements are live until below.
    let destination = unsafe { slots.data.read_unaligned().cast::<T>() };
    for (index, value) in values.iter().copied().enumerate() {
        unsafe { destination.add(index).write(value) };
    }
    unsafe { slots.num.write(count); }
}

unsafe fn array_values<T: Copy>(array: *const TArrayRaw) -> Vec<T> {
    // SAFETY: the caller passes initialized array storage owned by a live object.
    let raw = unsafe { ptr::read_unaligned(array) };
    assert!(raw.num >= 0, "native array count must not be negative");
    if raw.num == 0 {
        return Vec::new();
    }
    assert!(!raw.data.is_null(), "nonempty native array must have storage");
    // SAFETY: all live elements were initialized by set_values or archive loading.
    unsafe { std::slice::from_raw_parts(raw.data.cast::<T>(), raw.num as usize).to_vec() }
}

// In-memory export fixtures use this owner's canonical IDs. Package roundtrips
// use Core's real package mapping; GC uses its real reachability mapping.
struct MemoryMapping<'a>(CoreAccess<'a>);
impl ArchiveMapping for MemoryMapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        let object = self.0.object_at(index - 1);
        if object.is_null() { return Err(hp2_core::core_access::RuntimeError::new("Invalid fixture object index")); }
        Ok(object)
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        Ok(unsafe { (*object).index + 1 })
    }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> {
        self.0.name_text(index)?;
        Ok(index)
    }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> { self.load_name(index) }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> {
        Err(hp2_core::core_access::RuntimeError::new("Memory fixture is not a GC mapping"))
    }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> {
        Err(hp2_core::core_access::RuntimeError::new("Memory fixture is not a GC mapping"))
    }
}

fn utf32(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain([0]).collect()
}

fn rejected_counts_preserve_gc_safe_geometry(access: CoreAccess<'_>, package: *mut UObject, model_class: *mut UClass) {
    let mapping = MemoryMapping(access);
    let prefix_name = access.intern_str("ArchivePrefix", FindName::Add).expect("intern prefix name");
    let prefix_object = unsafe {
        access.construct_object(access.native_class("Engine.BoxPrim").expect("BoxPrim class"), package, prefix_name, 0, ptr::null_mut())
    }.expect("construct actual model superclass for malformed export prefix");
    let writer = ObjectArchive::saving(ArchiveFlags::new(79, 0, true), &mapping);
    unsafe { access.serialize_object(prefix_object, &writer) }.expect("serialize actual model superclass prefix");
    let prefix = writer.into_written_bytes().expect("collect superclass prefix");
    for mut count in [-1, i32::MAX] {
        let name = access.intern_str(&format!("RejectedCount{count}"), FindName::Add).expect("intern malformed target");
        let model = unsafe { access.construct_object(model_class, package, name, 0, ptr::null_mut()) }
            .expect("construct malformed archive target").cast::<UModel>();
        let retained = hp2_abi::FVector { x: 41.0, y: -43.0, z: 47.0 };
        unsafe {
            set_values(ptr::addr_of_mut!((*model).vectors).cast(), &[retained]);
            access.add_root(model.cast());
        }
        let writer = ObjectArchive::saving(ArchiveFlags::new(79, 0, true), &mapping);
        writer.write(&prefix).expect("copy real superclass wire prefix");
        writer.serialize_compact_index(&mut count).expect("write malformed first array count");
        let bytes = writer.into_written_bytes().expect("collect malformed export");
        let reader = ObjectArchive::loading(ExportArchive::from_slice(&bytes), ArchiveFlags::new(79, 0, true), &mapping);
        assert!(unsafe { access.serialize_object(model.cast(), &reader) }.is_err(), "reject invalid geometry allocation count");
        let raw = unsafe { ptr::addr_of!((*model).vectors).cast::<TArrayRaw>().read_unaligned() };
        assert_eq!((raw.num, raw.max), (1, 1), "rejected count must not corrupt the live allocation header");
        assert_eq!(unsafe { array_values::<hp2_abi::FVector>(ptr::addr_of!((*model).vectors).cast()) }, [retained]);
        access.collect_garbage(0, 0, &|_| Ok(())).expect("GC remains usable after a malformed model archive");
        access.remove_root(model.cast());
    }
}

#[test]
fn canonical_model_graph_saves_populated_geometry_and_survives_gc_by_reference() {
    let root = data_root();
    let saved_path = root.join("System/SavedModel.u");
    let owner = EngineOwner::new(options(&root)).expect("initialize canonical model owner without prototype data");
    let access = owner.access();

    let model_class = assert_class(access, "Engine.Model");
    let polys_class = assert_class(access, "Engine.Polys");
    assert_class(access, "Engine.BspNodes");
    assert_class(access, "Engine.BspSurfs");
    assert_class(access, "Engine.Vectors");
    assert_class(access, "Engine.Verts");
    let box_prim = assert_class(access, "Engine.BoxPrim");
    let box_class = assert_class(access, "Engine.Box");
    let primitive = assert_class(access, "Engine.Primitive");
    // SAFETY: all classes are live canonical UClass records in the owner.
    assert_eq!(unsafe { (*model_class).super_field }, box_prim.cast());
    assert_eq!(unsafe { (*box_prim).super_field }, box_class.cast());
    assert_eq!(unsafe { (*box_class).super_field }, primitive.cast());

    let package_name = access.intern_str("SavedModel", FindName::Add).expect("intern package name");
    let package = unsafe {
        access.construct_object(access.native_class("Package").expect("Core.Package"), ptr::null_mut(), package_name, 4, ptr::null_mut())
    }.expect("construct isolated model package");
    let model_name = access.intern_str("Geometry", FindName::Add).expect("intern model name");
    let model = unsafe { access.construct_object(model_class, package, model_name, 4, ptr::null_mut()) }
        .expect("construct canonical Engine.Model").cast::<UModel>();
    let polys_name = access.intern_str("GeometryPolys", FindName::Add).expect("intern Polys name");
    let polys = unsafe { access.construct_object(polys_class, package, polys_name, 4, ptr::null_mut()) }
        .expect("construct canonical Engine.Polys").cast::<UPolys>();
    // SAFETY: both records are live, matching canonical class layouts.
    let item_name = access.intern_str("OwnershipTriangle", FindName::Add).expect("intern polygon item name");
    unsafe {
        (*model).polys = polys.cast();
        set_values(ptr::addr_of_mut!((*model).vectors).cast::<TArrayRaw>(), &[
            hp2_abi::FVector { x: 1.25, y: -2.5, z: 8.0 },
            hp2_abi::FVector { x: -13.0, y: 0.5, z: 21.75 },
        ]);
        set_values(ptr::addr_of_mut!((*model).points).cast::<TArrayRaw>(), &[
            hp2_abi::FVector { x: 3.0, y: 5.0, z: -7.0 },
        ]);
        let mut surface: FBspSurf = std::mem::zeroed();
        surface.poly_flags = 1 | generated::POLYFLAG_TRANSIENT;
        surface.pan_u = -37;
        set_values(ptr::addr_of_mut!((*model).surfs).cast(), &[surface]);
        let mut node: FBspNode = std::mem::zeroed();
        node.i_surf = 0;
        node.i_front = -1;
        node.node_flags = (generated::NODEFLAG_IS_NEW | generated::NODEFLAG_IS_FRONT | generated::NODEFLAG_IS_BACK) as u8;
        set_values(ptr::addr_of_mut!((*model).nodes).cast(), &[node, node]);
        let mut polygon: FPoly = std::mem::zeroed();
        polygon.num_vertices = 3;
        polygon.vertex[0] = hp2_abi::FVector { x: 11.0, y: 7.0, z: -3.0 };
        polygon.vertex[1] = hp2_abi::FVector { x: -2.0, y: 17.0, z: 5.0 };
        polygon.vertex[2] = hp2_abi::FVector { x: 19.0, y: 23.0, z: 29.0 };
        polygon.item_name = item_name.to_ne_bytes();
        polygon.poly_flags = 1 | generated::POLYFLAG_TRANSIENT;
        set_values(ptr::addr_of_mut!((*polys).element).cast(), &[polygon]);
        let primitive = model.cast::<UPrimitive>();
        (*primitive).bounding_box.min.x = -101.5;
        (*primitive).bounding_box.max.z = 209.25;
        (*primitive).bounding_box.is_valid = 1;
        ptr::addr_of_mut!((*primitive).bounding_sphere).cast::<f32>().add(3).write(71.5);
        (*model).root_outside = 0;
    }
 
    let mapping = MemoryMapping(access);
    let transaction = ObjectArchive::transaction_saving(ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(model.cast(), &transaction) }
        .expect("serialize model through public transaction archive");
    let transaction_bytes = transaction.into_written_bytes().expect("collect transaction payload");
    // UObject's transaction payload includes identity. Undo restores the same
    // canonical object, not an unrelated object under a different hash key.
    let retained = hp2_abi::FVector { x: 301.0, y: -307.0, z: 311.0 };
    unsafe {
        let vectors = ptr::addr_of_mut!((*model).vectors).cast::<TArrayRaw>();
        ptr::addr_of!((*vectors).data).read_unaligned().cast::<hp2_abi::FVector>().write(retained);
        (*model).root_outside = 1;
        (*model).polys = ptr::null_mut();
    }
    let transaction_read = ObjectArchive::transaction_loading(
        ExportArchive::from_slice(&transaction_bytes), ArchiveFlags::new(79, 0, false), &mapping,
    );
    unsafe { access.serialize_object(model.cast(), &transaction_read) }.expect("restore transaction metadata");
    assert_eq!(unsafe { array_values::<hp2_abi::FVector>(ptr::addr_of!((*model).vectors).cast()) }[0], retained,
        "model transactions leave independently owned geometry unchanged");
    assert_eq!(unsafe { (*model).root_outside }, 0, "transaction restores model metadata");
    assert_eq!(unsafe { (*model).polys }, polys.cast(), "transaction restores the genuine Polys identity");
 
    unsafe {
        access.add_root(package);
        access.add_root(model.cast());
    }
    assert!(unsafe { access.save_package(package, model.cast(), 4, &saved_path, None) }
        .expect("save populated canonical model package"));
    let polys_index = unsafe { (*polys.cast::<UObject>()).index };
    let package_index = unsafe { (*package).index };
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect while the model is rooted");
    assert_eq!(access.object_at(polys_index), polys.cast(), "rooted model's genuine Polys reference retains canonical identity");
    assert_eq!(unsafe { (*model).polys }, polys.cast(), "model retains the genuine Polys pointer");
    rejected_counts_preserve_gc_safe_geometry(access, package, model_class);
 
    access.remove_root(model.cast());
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect unrooted model geometry graph");
    assert!(access.object_at(polys_index).is_null(), "unrooted referenced Polys object is destroyed");
    let recreated_name = access.intern_str("GeometryPolys", FindName::Add).expect("re-intern collected Polys name");
    let recreated = unsafe { access.construct_object(polys_class, package, recreated_name, 4, ptr::null_mut()) }
        .expect("recreate collected canonical Polys object");
    assert_eq!(access.object_at(unsafe { (*recreated.cast::<UObject>()).index }), recreated);
 
    access.remove_root(package);
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect recreated Polys and package");
    assert!(access.object_at(package_index).is_null(), "test package is not leaked as a global root");
    owner.shutdown().expect("destroy canonical model graph");
    // External native class imports still require their real package metadata.
    // Keep the construction/save/GC phase data-none; reload uses prototype Core/Engine.
    let prototype = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT")
        .expect("HP2_CORE_DATA_ROOT must name the prototype root"));
    for name in ["Core.u", "Engine.u"] {
        std::fs::copy(prototype.join("System").join(name), root.join("System").join(name))
            .expect("copy real prototype class metadata for import verification");
    }
 
    let restored_owner = EngineOwner::new(options(&root)).expect("initialize fresh owner for package reload");
    let restored = restored_owner.access();
    restored.set_process_mode(hp2_core::core_access::CoreProcessMode {
        is_client: true,
        ..hp2_core::core_access::CoreProcessMode::default()
    });
    let restored_model_class = restored.native_class("Engine.Model").expect("resolve model class in fresh owner");
    let restored_polys_class = restored.native_class("Engine.Polys").expect("resolve Polys class in fresh owner");
    let metadata_flags = 1 | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.verify
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.no_verify;
    // LOAD_Propagate is zero here: select Engine metadata explicitly, as the
    // LevelSummary consumer does, while resolving all selected model imports.
    unsafe { restored.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("open real Engine metadata for the selected model classes");
    unsafe { restored.load_package(&saved_path, metadata_flags) }.expect("open saved synthetic model package");
    let object_path = utf32("SavedModel.Geometry");
    let loaded = unsafe { restored.static_load_object(restored_model_class, ptr::null_mut(), &object_path, 1) }
        .expect("load saved canonical model export").cast::<UModel>();
    // SAFETY: the object came from Core's loader using the registered Model layout.
    let loaded_polys = unsafe { (*loaded).polys.cast::<UObject>() };
    assert!(!loaded_polys.is_null(), "saved model restores its Polys reference");
    assert_eq!(unsafe { (*loaded_polys).class }, restored_polys_class.cast());
    assert_eq!(object_name(restored, loaded_polys), "GeometryPolys");
    let polys_path = utf32("SavedModel.GeometryPolys");
    let loaded_export = unsafe {
        restored.static_load_object(restored_polys_class, ptr::null_mut(), &polys_path, 1)
    }.expect("resolve saved canonical Polys export");
    assert_eq!(loaded_polys, loaded_export, "Model.Polys points to the canonical loaded export");
    let loaded_vectors = unsafe { array_values::<hp2_abi::FVector>(ptr::addr_of!((*loaded).vectors).cast()) };
    assert_eq!(loaded_vectors, [
        hp2_abi::FVector { x: 301.0, y: -307.0, z: 311.0 },
        hp2_abi::FVector { x: -13.0, y: 0.5, z: 21.75 },
    ]);
    let loaded_points = unsafe { array_values::<hp2_abi::FVector>(ptr::addr_of!((*loaded).points).cast()) };
    assert_eq!(loaded_points, [hp2_abi::FVector { x: 3.0, y: 5.0, z: -7.0 }]);
    let loaded_surfaces = unsafe { array_values::<FBspSurf>(ptr::addr_of!((*loaded).surfs).cast()) };
    assert_eq!(loaded_surfaces[0].pan_u, -37);
    assert_eq!(loaded_surfaces[0].poly_flags, 1, "loading clears transient surface flags");
    assert_eq!(unsafe { array_values::<i32>(ptr::addr_of!(loaded_surfaces[0].nodes).cast()) }, [0, 1],
        "PostLoad reconstructs the ordered surface-to-node adjacency");
    let loaded_nodes = unsafe { array_values::<FBspNode>(ptr::addr_of!((*loaded).nodes).cast()) };
    assert_eq!(loaded_nodes[1].i_front, -1);
    assert_eq!(loaded_nodes[1].node_flags, 0, "loading clears transient node flags");
    let loaded_polygons = unsafe { array_values::<FPoly>(ptr::addr_of!((*loaded_polys.cast::<UPolys>()).element).cast()) };
    assert_eq!(loaded_polygons[0].num_vertices, 3);
    assert_eq!(loaded_polygons[0].vertex[1], hp2_abi::FVector { x: -2.0, y: 17.0, z: 5.0 });
    assert_eq!(loaded_polygons[0].poly_flags, 1, "loading clears transient polygon flags");
    assert_eq!(restored.name_text(i32::from_ne_bytes(loaded_polygons[0].item_name)).expect("resolve loaded item name"),
        "OwnershipTriangle".chars().map(u32::from).collect::<Vec<_>>());
    let loaded_primitive = loaded.cast::<UPrimitive>();
    assert_eq!(unsafe { (*loaded_primitive).bounding_box.min.x }, -101.5);
    assert_eq!(unsafe { (*loaded_primitive).bounding_box.max.z }, 209.25);
    assert_eq!(unsafe { ptr::addr_of!((*loaded_primitive).bounding_sphere).cast::<f32>().add(3).read() }, 71.5);
    restored_owner.shutdown().expect("destroy reloaded canonical package graph");
}

#[test]
fn canonical_shutdown_restores_level_tracker_before_later_model_member_release() {
    TRACKER_POOLS_RESTORED.store(false, Ordering::SeqCst);
    let root = data_root();
    let mut core_options = options(&root);
    core_options.output = tracker_output;
    let owner = EngineOwner::new(core_options)
        .expect("initialize canonical Engine owner without prototype data");
    let access = owner.access();

    let package_name = access.intern_str("TrackerShutdown", FindName::Add).expect("intern package name");
    let package = unsafe {
        access.construct_object(
            access.native_class("Package").expect("Core.Package"),
            ptr::null_mut(),
            package_name,
            4,
            ptr::null_mut(),
        )
    }.expect("construct tracker test package");
    let level_class = assert_class(access, "Engine.Level");
    let model_class = assert_class(access, "Engine.Model");
    let level_name = access.intern_str("TrackerLevel", FindName::Add).expect("intern level name");
    let level = unsafe { access.construct_object(level_class, package, level_name, 4, ptr::null_mut()) }
        .expect("construct Level before its Model").cast::<generated::ULevel>();
    let model_name = access.intern_str("TrackerModel", FindName::Add).expect("intern model name");
    let model = unsafe { access.construct_object(model_class, level.cast(), model_name, 4, ptr::null_mut()) }
        .expect("construct Model after its Level").cast::<UModel>();

    unsafe {
        (*level).model = model.cast();
        set_values(ptr::addr_of_mut!((*model).vectors).cast(), &[
            hp2_abi::FVector { x: 1.0, y: 2.0, z: 3.0 },
        ]);
        set_values(ptr::addr_of_mut!((*model).points).cast(), &[
            hp2_abi::FVector { x: -4.0, y: 5.0, z: -6.0 },
        ]);
        set_values(ptr::addr_of_mut!((*model).surfs).cast(), &[std::mem::zeroed::<FBspSurf>()]);
        let mut node = std::mem::zeroed::<FBspNode>();
        node.i_front = -1;
        node.i_back = -1;
        node.i_plane = -1;
        node.i_collision_bound = -1;
        node.i_render_bound = -1;
        set_values(ptr::addr_of_mut!((*model).nodes).cast(), &[node]);
        set_values(ptr::addr_of_mut!((*model).verts).cast(), &[
            std::mem::zeroed::<hp2_abi::render_scene::FVert>(),
        ]);
    }
    let original_surfs = unsafe {
        ptr::addr_of!((*model).surfs).cast::<TArrayRaw>().read_unaligned().num
    };
    let original_vectors = unsafe {
        ptr::addr_of!((*model).vectors).cast::<TArrayRaw>().read_unaligned().num
    };
    let original_points = unsafe {
        ptr::addr_of!((*model).points).cast::<TArrayRaw>().read_unaligned().num
    };
    let original_nodes = unsafe {
        ptr::addr_of!((*model).nodes).cast::<TArrayRaw>().read_unaligned().num
    };
    let original_verts = unsafe {
        ptr::addr_of!((*model).verts).cast::<TArrayRaw>().read_unaligned().num
    };
    assert!(
        [original_surfs, original_vectors, original_points, original_nodes, original_verts]
            .iter().all(|count| *count > 0),
        "the original model BSP must populate every tracker pool"
    );
    unsafe { hp2_engine::dynbsp_owner::initialize(access, level) }
        .expect("initialize moving-brush tracker over the original model BSP");
    assert!(!unsafe { hp2_engine::dynbsp_owner::surf_is_dynamic(level, 0) }
        .expect("query original BSP surface"));
    let tracker_nodes = unsafe {
        ptr::addr_of!((*model).nodes).cast::<TArrayRaw>().read_unaligned().num
    };
    let tracker_verts = unsafe {
        ptr::addr_of!((*model).verts).cast::<TArrayRaw>().read_unaligned().num
    };
    assert!(tracker_nodes > original_nodes, "tracker must reserve its transient node pool");
    assert!(tracker_verts > original_verts, "tracker must reserve its transient vertex pool");
    TRACKER_MODEL_INDEX.store(unsafe { (*model.cast::<UObject>()).index }, Ordering::SeqCst);
    TRACKER_LEVEL_INDEX.store(unsafe { (*level.cast::<UObject>()).index }, Ordering::SeqCst);

    owner.shutdown()
        .expect("Level tracker must restore original BSP pools before Model finalization");
    assert!(
        TRACKER_POOLS_RESTORED.load(Ordering::SeqCst),
        "tracker shutdown must finish restoring the model pools before final member release"
    );
    TRACKER_MODEL_INDEX.store(-1, Ordering::SeqCst);
    TRACKER_LEVEL_INDEX.store(-1, Ordering::SeqCst);
}
