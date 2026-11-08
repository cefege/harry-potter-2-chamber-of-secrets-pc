use hp2_abi::engine::generated::{
    self, FMeshAnimNotify, FMeshAnimSeq, FMeshFace, FMeshMaterial, FMeshTri, FMeshUV,
    FMeshVertConnect, FMeshWedge, TArrayRaw, UClass, ULodMesh, UMesh, UObject, UPrimitive,
};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{
    CoreAccess, CoreOptions, CoreProcessMode, RuntimeError, RuntimeResult,
};
use hp2_core::lazy_array::{self, LazyArraySlots};
use hp2_core::name_owner::FindName;
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_core::package_archive::ExportArchive;
use hp2_engine::EngineOwner;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::ptr;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn data_root() -> PathBuf {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR must name the pre-created test artifact directory"),
    );
    assert!(
        artifact.is_dir(),
        "HP2_ARTIFACT_DIR must exist before the consumer starts"
    );
    let root = artifact.join(format!("mesh-owner-{}", std::process::id()));
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

fn text(access: CoreAccess<'_>, name: i32) -> String {
    access
        .name_text(name)
        .expect("resolve canonical name")
        .into_iter()
        .filter_map(char::from_u32)
        .collect()
}

unsafe fn set_values<T: Copy>(array: *mut TArrayRaw, values: &[T]) {
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*array).data) },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    };
    assert_eq!(
        unsafe { slots.num.read_unaligned() },
        0,
        "fresh fixture array is empty"
    );
    assert!(
        unsafe { slots.data.read_unaligned() }.is_null(),
        "fresh fixture array has no storage"
    );
    let count = i32::try_from(values.len()).expect("fixture count fits native INT");
    if count == 0 {
        return;
    }
    unsafe {
        slots.max.write_unaligned(count);
    }
    unsafe {
        array_storage::realloc(
            slots,
            i32::try_from(std::mem::size_of::<T>()).expect("fixture element width fits native INT"),
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .expect("allocate fixture storage with Core's paired system allocator");
    let destination = unsafe { slots.data.read_unaligned().cast::<T>() };
    for (index, value) in values.iter().copied().enumerate() {
        unsafe {
            destination.add(index).write(value);
        }
    }
    unsafe {
        slots.num.write_unaligned(count);
    }
}

unsafe fn values<T: Copy>(array: *const TArrayRaw) -> Vec<T> {
    let raw = unsafe { ptr::read_unaligned(array) };
    assert!(raw.num >= 0, "native array count must not be negative");
    if raw.num == 0 {
        return Vec::new();
    }
    assert!(
        !raw.data.is_null(),
        "nonempty native array must have storage"
    );
    unsafe { std::slice::from_raw_parts(raw.data.cast::<T>(), raw.num as usize).to_vec() }
}

unsafe fn lazy_array(lazy: *mut u8, offset: usize) -> *mut TArrayRaw {
    unsafe { lazy.add(offset).cast::<TArrayRaw>() }
}

struct FixtureMapping<'a> {
    access: CoreAccess<'a>,
    visited_objects: RefCell<Vec<i32>>,
    visited_names: RefCell<Vec<i32>>,
}

impl<'a> FixtureMapping<'a> {
    fn new(access: CoreAccess<'a>) -> Self {
        Self {
            access,
            visited_objects: RefCell::new(Vec::new()),
            visited_names: RefCell::new(Vec::new()),
        }
    }
}

impl ArchiveMapping for FixtureMapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 {
            return Ok(ptr::null_mut());
        }
        let object = self.access.object_at(index - 1);
        if object.is_null() {
            return Err(RuntimeError::new("Invalid mesh fixture object index"));
        }
        Ok(object)
    }

    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() {
            return Ok(0);
        }
        Ok(unsafe { (*object).index + 1 })
    }

    fn load_name(&self, index: i32) -> RuntimeResult<i32> {
        self.access.name_text(index)?;
        Ok(index)
    }

    fn save_name(&self, index: i32) -> RuntimeResult<i32> {
        self.load_name(index)
    }

    fn visit_object(&self, slot: *mut *mut UObject) -> RuntimeResult<()> {
        let object = unsafe { slot.read_unaligned() };
        if !object.is_null() {
            self.visited_objects
                .borrow_mut()
                .push(unsafe { (*object).index });
        }
        Ok(())
    }

    fn visit_name(&self, name: i32) -> RuntimeResult<()> {
        self.access.name_text(name)?;
        self.visited_names.borrow_mut().push(name);
        Ok(())
    }
}

fn name(access: CoreAccess<'_>, value: &str) -> i32 {
    access
        .intern_str(value, FindName::Add)
        .expect("intern fixture name")
}
fn utf32(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain([0]).collect()
}

fn create_package(access: CoreAccess<'_>) -> *mut UObject {
    let package_name = name(access, "MeshArchive");
    unsafe {
        access.construct_object(
            access
                .native_class("Package")
                .expect("resolve Core.Package"),
            ptr::null_mut(),
            package_name,
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct fixture package")
}

fn construct_mesh(
    access: CoreAccess<'_>,
    package: *mut UObject,
    class: *mut UClass,
    object_name: &str,
) -> *mut UMesh {
    unsafe {
        access.construct_object(
            class,
            package,
            name(access, object_name),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct canonical mesh object")
    .cast()
}

fn setup_animation(access: CoreAccess<'_>, mesh: *mut UMesh) -> (i32, i32, i32) {
    let sequence_name = name(access, "WalkCycle");
    let group_name = name(access, "Movement");
    let function_name = name(access, "StepNotify");
    let mut notify: FMeshAnimNotify = unsafe { std::mem::zeroed() };
    notify.time = 0.375;
    notify.function = function_name.to_ne_bytes();
    let sequence = unsafe { ptr::addr_of_mut!((*mesh).anim_seqs).cast::<TArrayRaw>() };
    let mut animation: FMeshAnimSeq = unsafe { std::mem::zeroed() };
    animation.name = sequence_name.to_ne_bytes();
    animation.group = group_name.to_ne_bytes();
    animation.start_frame = 2;
    animation.num_frames = 4;
    animation.rate = 30.0;
    unsafe {
        set_values(ptr::addr_of_mut!(animation.notifys), &[notify]);
        set_values(sequence, &[animation]);
    }
    (sequence_name, group_name, function_name)
}

fn object_class(access: CoreAccess<'_>, name: &str) -> *mut UClass {
    access
        .native_class(name)
        .unwrap_or_else(|error| panic!("resolve {name}: {error}"))
}

#[test]
fn canonical_mesh_and_lod_mesh_archive_geometry_references_and_remapping() {
    let root = data_root();
    let owner = EngineOwner::new(options(&root)).expect("initialize canonical mesh owner");
    let access = owner.access();
    let mesh_class = object_class(access, "Engine.Mesh");
    let lod_class = object_class(access, "Engine.LodMesh");
    assert_eq!(
        unsafe { (*mesh_class).super_field },
        access.native_class("Engine.BoxPrim").unwrap().cast()
    );
    assert_eq!(unsafe { (*lod_class).super_field }, mesh_class.cast());
    let package = create_package(access);
    let mesh = construct_mesh(access, package, mesh_class, "OrdinaryMesh");
    let lod = construct_mesh(access, package, lod_class, "RemappedLod").cast::<ULodMesh>();
    assert_eq!(unsafe { (*mesh).scale.x }, 1.0);
    assert_eq!(unsafe { (*mesh).scale.y }, 1.0);
    assert_eq!(unsafe { (*mesh).scale.z }, 1.0);
    assert_eq!(unsafe { (*mesh).and_flags }, u32::MAX);
    assert_eq!(unsafe { (*mesh).or_flags }, 0);
    assert_eq!(unsafe { (*lod).lod_min_verts }, 0);
    assert_eq!(unsafe { (*lod).lod_strength }, 0.0);
    let (sequence_name, group_name, function_name) = setup_animation(access, mesh);

    let triangle = FMeshTri {
        i_vertex: [0, 1, 2],
        tex: [
            FMeshUV { u: 4, v: 9 },
            FMeshUV { u: 17, v: 23 },
            FMeshUV { u: 31, v: 47 },
        ],
        poly_flags: 0x120,
        texture_index: -1,
    };
    let mut bounds: generated::FBox = unsafe { std::mem::zeroed() };
    bounds.min = hp2_abi::FVector {
        x: -3.0,
        y: -5.0,
        z: -7.0,
    };
    bounds.max = hp2_abi::FVector {
        x: 11.0,
        y: 13.0,
        z: 17.0,
    };
    bounds.is_valid = 1;
    unsafe {
        let primitive = mesh.cast::<UPrimitive>();
        (*primitive).bounding_box = bounds;
        ptr::addr_of_mut!((*primitive).bounding_sphere)
            .cast::<f32>()
            .add(3)
            .write(19.0);
        set_values(
            lazy_array(
                ptr::addr_of_mut!((*mesh).verts).cast(),
                generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
            ),
            &[0x1122_3344u32, 0x5566_7788, 0x99AA_BBCC],
        );
        set_values(
            lazy_array(
                ptr::addr_of_mut!((*mesh).tris).cast(),
                generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
            ),
            &[triangle],
        );
        set_values(
            lazy_array(
                ptr::addr_of_mut!((*mesh).connects).cast(),
                generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET,
            ),
            &[FMeshVertConnect {
                num_vert_triangles: 1,
                triangle_list_offset: 0,
            }],
        );
        set_values(
            lazy_array(
                ptr::addr_of_mut!((*mesh).vert_links).cast(),
                generated::TLazyMeshLinks::TARRAY_INT_OFFSET,
            ),
            &[0i32, 0, 0],
        );
        set_values(ptr::addr_of_mut!((*mesh).bounding_boxes), &[bounds]);
        set_values(
            ptr::addr_of_mut!((*mesh).bounding_spheres).cast(),
            &[[1.0f32, 2.0, 3.0, 19.0]],
        );
        set_values(ptr::addr_of_mut!((*mesh).textures), &[] as &[*mut UObject]);
        set_values(ptr::addr_of_mut!((*mesh).texture_lod), &[0.25f32, 0.75]);
        (*mesh).frame_verts = 3;
        (*mesh).anim_frames = 1;
        (*mesh).scale = hp2_abi::FVector {
            x: 1.5,
            y: 2.0,
            z: 0.5,
        };
        (*mesh).and_flags = 0xFFFF_FF00;
        (*mesh).or_flags = 0x80;
    }

    let mapping = FixtureMapping::new(access);
    let reference = ObjectArchive::reference(ArchiveFlags::new(79, 0, true), &mapping);
    unsafe { access.serialize_object(mesh.cast(), &reference) }
        .expect("visit mesh references and names");
    let visited_names = mapping.visited_names.borrow();
    assert!(
        visited_names.contains(&sequence_name),
        "mesh reference traversal visits animation names"
    );
    assert!(
        visited_names.contains(&group_name),
        "mesh reference traversal visits animation groups"
    );
    assert!(
        visited_names.contains(&function_name),
        "mesh reference traversal visits nested notify functions"
    );
    drop(visited_names);

    let lod_mesh = unsafe { ptr::addr_of_mut!((*lod).base) };
    unsafe {
        set_values(
            lazy_array(
                ptr::addr_of_mut!((*lod_mesh).verts).cast(),
                generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
            ),
            &[10u32, 20, 30, 40, 50, 60],
        );
        set_values(
            lazy_array(
                ptr::addr_of_mut!((*lod_mesh).tris).cast(),
                generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
            ),
            &[triangle],
        );
        set_values(ptr::addr_of_mut!((*lod).collapse_point_thus), &[1u16, 2, 2]);
        set_values(ptr::addr_of_mut!((*lod).face_level), &[1u16]);
        let face = FMeshFace {
            i_wedge: [0, 1, 2],
            material_index: 0,
        };
        set_values(ptr::addr_of_mut!((*lod).faces), &[face]);
        set_values(ptr::addr_of_mut!((*lod).collapse_wedge_thus), &[1u16, 2, 2]);
        set_values(
            ptr::addr_of_mut!((*lod).wedges),
            &[
                FMeshWedge {
                    i_vertex: 0,
                    tex_uv: FMeshUV { u: 1, v: 2 },
                },
                FMeshWedge {
                    i_vertex: 1,
                    tex_uv: FMeshUV { u: 3, v: 4 },
                },
                FMeshWedge {
                    i_vertex: 2,
                    tex_uv: FMeshUV { u: 5, v: 6 },
                },
            ],
        );
        set_values(
            ptr::addr_of_mut!((*lod).materials),
            &[FMeshMaterial {
                poly_flags: 0x80,
                texture_index: -1,
            }],
        );
        set_values(ptr::addr_of_mut!((*lod).special_faces), &[face]);
        set_values(ptr::addr_of_mut!((*lod).remap_anim_verts), &[2u16, 0]);
        (*lod_mesh).frame_verts = 2;
        (*lod_mesh).anim_frames = 2;
        (*lod).model_verts = 3;
        (*lod).special_verts = 0;
        (*lod).mesh_scale_max = 1.25;
        (*lod).lod_strength = 0.75;
        (*lod).old_frame_verts = 3;
        (*lod).lod_min_verts = 9;
        (*lod).lod_morph = 0.2;
        (*lod).lodz_displace = 0.3;
        (*lod).lod_hysteresis = 0.1;
    }
    let mesh_transaction =
        ObjectArchive::transaction_saving(ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(mesh.cast(), &mesh_transaction) }
        .expect("serialize Mesh transaction through resident lazy-array bodies");
    let transaction_bytes = mesh_transaction
        .into_written_bytes()
        .expect("collect Mesh transaction bytes");
    unsafe {
        let vertices = lazy_array(
            ptr::addr_of_mut!((*mesh).verts).cast(),
            generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
        );
        ptr::addr_of!((*vertices).data)
            .read_unaligned()
            .cast::<u32>()
            .write(0xDEAD_BEEF);
        let triangles = lazy_array(
            ptr::addr_of_mut!((*mesh).tris).cast(),
            generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
        );
        ptr::addr_of_mut!((*triangles).data)
            .read_unaligned()
            .cast::<FMeshTri>()
            .as_mut()
            .unwrap()
            .i_vertex[0] = 2;
        let connects = lazy_array(
            ptr::addr_of_mut!((*mesh).connects).cast(),
            generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET,
        );
        ptr::addr_of_mut!((*connects).data)
            .read_unaligned()
            .cast::<FMeshVertConnect>()
            .as_mut()
            .unwrap()
            .num_vert_triangles = 7;
        let links = lazy_array(
            ptr::addr_of_mut!((*mesh).vert_links).cast(),
            generated::TLazyMeshLinks::TARRAY_INT_OFFSET,
        );
        ptr::addr_of_mut!((*links).data)
            .read_unaligned()
            .cast::<i32>()
            .write(9);
        (*mesh).scale.x = 99.0;
        let sequences =
            ptr::read_unaligned(ptr::addr_of!((*mesh).anim_seqs.data)).cast::<FMeshAnimSeq>();
        (*sequences).name = name(access, "ChangedName").to_ne_bytes();
    }
    let transaction_reader = ObjectArchive::transaction_loading(
        ExportArchive::from_slice(&transaction_bytes),
        ArchiveFlags::new(79, 0, false),
        &mapping,
    );
    unsafe { access.serialize_object(mesh.cast(), &transaction_reader) }
        .expect("restore eager transaction fields without fabricating lazy sources");
    drop(transaction_reader);
    assert_eq!(unsafe { (*mesh).scale.x }, 1.5);
    let resident_vertices = unsafe {
        values::<u32>(lazy_array(
            ptr::addr_of_mut!((*mesh).verts).cast(),
            generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
        ))
    };
    assert_eq!(
        resident_vertices,
        [0xDEAD_BEEF, 0x5566_7788, 0x99AA_BBCC],
        "transaction loading skips lazy vertex bodies and preserves resident storage"
    );
    let resident_triangle = unsafe {
        values::<FMeshTri>(lazy_array(
            ptr::addr_of_mut!((*mesh).tris).cast(),
            generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
        ))
    }[0];
    assert_eq!(resident_triangle.i_vertex[0], 2);
    assert_eq!(
        unsafe {
            values::<FMeshVertConnect>(lazy_array(
                ptr::addr_of_mut!((*mesh).connects).cast(),
                generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET,
            ))[0]
                .num_vert_triangles
        },
        7
    );
    assert_eq!(
        unsafe {
            values::<i32>(lazy_array(
                ptr::addr_of_mut!((*mesh).vert_links).cast(),
                generated::TLazyMeshLinks::TARRAY_INT_OFFSET,
            ))
        },
        [9, 0, 0]
    );
    let restored_sequence = unsafe { values::<FMeshAnimSeq>(ptr::addr_of!((*mesh).anim_seqs)) }[0];
    assert_eq!(
        text(access, i32::from_ne_bytes(restored_sequence.name)),
        "WalkCycle"
    );
    let restored_notify =
        unsafe { values::<FMeshAnimNotify>(ptr::addr_of!(restored_sequence.notifys)) }[0];
    assert_eq!(
        text(access, i32::from_ne_bytes(restored_notify.function)),
        "StepNotify"
    );
    unsafe {
        let vertices = lazy_array(
            ptr::addr_of_mut!((*mesh).verts).cast(),
            generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
        );
        ptr::addr_of!((*vertices).data)
            .read_unaligned()
            .cast::<u32>()
            .write(0x1122_3344);
        let triangles = lazy_array(
            ptr::addr_of_mut!((*mesh).tris).cast(),
            generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
        );
        ptr::addr_of!((*triangles).data)
            .read_unaligned()
            .cast::<FMeshTri>()
            .as_mut()
            .unwrap()
            .i_vertex[0] = 0;
        let connects = lazy_array(
            ptr::addr_of_mut!((*mesh).connects).cast(),
            generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET,
        );
        ptr::addr_of!((*connects).data)
            .read_unaligned()
            .cast::<FMeshVertConnect>()
            .as_mut()
            .unwrap()
            .num_vert_triangles = 1;
        let links = lazy_array(
            ptr::addr_of_mut!((*mesh).vert_links).cast(),
            generated::TLazyMeshLinks::TARRAY_INT_OFFSET,
        );
        ptr::addr_of!((*links).data)
            .read_unaligned()
            .cast::<i32>()
            .write(0);
    }
    let package_path = root.join("System/MeshArchive.u");
    unsafe {
        access.add_root(package);
        access.add_root(mesh.cast());
        access.add_root(lod.cast());
    }
    assert!(
        unsafe { access.save_package(package, mesh.cast(), 4, &package_path, None) }
            .expect("save canonical mesh and LOD package exports")
    );
    assert!(
        unsafe {
            values::<FMeshTri>(lazy_array(
                ptr::addr_of_mut!((*lod_mesh).tris).cast(),
                generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
            ))
        }
        .is_empty(),
        "native LOD saving empties inherited triangles before serialization"
    );
    unsafe {
        access.remove_root(lod.cast());
        access.remove_root(mesh.cast());
        access.remove_root(package);
    }
    drop(reference);
    drop(mapping);
    owner
        .shutdown()
        .expect("destroy canonical mesh and LOD graph");

    let prototype = PathBuf::from(
        std::env::var_os("HP2_CORE_DATA_ROOT")
            .expect("HP2_CORE_DATA_ROOT must name prototype Unreal package metadata"),
    );
    for name in ["Core.u", "Engine.u"] {
        std::fs::copy(
            prototype.join("System").join(name),
            root.join("System").join(name),
        )
        .expect("copy real prototype metadata for native class imports");
    }
    let restored_owner =
        EngineOwner::new(options(&root)).expect("initialize fresh mesh owner for linker reload");
    let restored = restored_owner.access();
    restored.set_process_mode(CoreProcessMode {
        is_client: true,
        lazy_load: true,
        ..CoreProcessMode::default()
    });
    let restored_mesh_class = object_class(restored, "Engine.Mesh");
    let restored_lod_class = object_class(restored, "Engine.LodMesh");
    let metadata_flags =
        1 | generated::CORE_LINKER_CONSTANTS.verify | generated::CORE_LINKER_CONSTANTS.no_verify;
    unsafe { restored.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("open real Engine metadata for native class imports");
    unsafe { restored.load_package(&package_path, metadata_flags) }
        .expect("open saved MeshArchive through the real package linker");
    let mesh_path = utf32("MeshArchive.OrdinaryMesh");
    let loaded_mesh =
        unsafe { restored.static_load_object(restored_mesh_class, ptr::null_mut(), &mesh_path, 1) }
            .expect("load canonical Mesh export from linker-backed package")
            .cast::<UMesh>();
    let lod_path = utf32("MeshArchive.RemappedLod");
    let loaded_lod =
        unsafe { restored.static_load_object(restored_lod_class, ptr::null_mut(), &lod_path, 1) }
            .expect("load canonical LodMesh export from linker-backed package")
            .cast::<ULodMesh>();
    let loaded_triangles = unsafe {
        values::<FMeshTri>(lazy_array(
            ptr::addr_of_mut!((*loaded_mesh).tris).cast(),
            generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
        ))
    };
    let triangle_saved_position = unsafe { (*loaded_mesh).tris.saved_pos };
    assert!(
        triangle_saved_position > 0,
        "lazy-enabled package loading retains the real deferred source"
    );
    assert!(
        loaded_triangles.is_empty(),
        "triangle bytes remain deferred until loading or linker teardown"
    );
    assert_eq!(unsafe { (*loaded_mesh).scale.x }, 1.5);
    let loaded_bounds =
        unsafe { values::<generated::FBox>(ptr::addr_of!((*loaded_mesh).bounding_boxes)) }[0];
    assert_eq!(
        loaded_bounds.min,
        hp2_abi::FVector {
            x: -3.0,
            y: -5.0,
            z: -7.0
        }
    );
    assert_eq!(
        loaded_bounds.max,
        hp2_abi::FVector {
            x: 11.0,
            y: 13.0,
            z: 17.0
        }
    );
    assert_eq!(
        unsafe { values::<[f32; 4]>(ptr::addr_of!((*loaded_mesh).bounding_spheres).cast()) },
        [[1.0, 2.0, 3.0, 19.0]]
    );
    let loaded_primitive = loaded_mesh.cast::<UPrimitive>();
    assert_eq!(
        unsafe { (*loaded_primitive).bounding_box.min },
        hp2_abi::FVector {
            x: -3.0,
            y: -5.0,
            z: -7.0
        }
    );
    assert_eq!(
        unsafe { (*loaded_primitive).bounding_box.max },
        hp2_abi::FVector {
            x: 11.0,
            y: 13.0,
            z: 17.0
        }
    );
    assert_eq!(
        unsafe {
            ptr::addr_of!((*loaded_primitive).bounding_sphere)
                .cast::<f32>()
                .add(3)
                .read()
        },
        19.0
    );
    assert_eq!(
        unsafe { values::<f32>(ptr::addr_of!((*loaded_mesh).texture_lod)) },
        [0.25, 0.75]
    );
    assert!(
        unsafe { values::<*mut UObject>(ptr::addr_of!((*loaded_mesh).textures)) }.is_empty(),
        "fixture intentionally contains no Texture reference"
    );
    assert_eq!(unsafe { (*loaded_mesh).frame_verts }, 3);
    assert_eq!(unsafe { (*loaded_mesh).anim_frames }, 1);
    assert_eq!(unsafe { (*loaded_mesh).and_flags }, 0xFFFF_FF00);
    assert_eq!(unsafe { (*loaded_mesh).or_flags }, 0x80);
    let loaded_sequence =
        unsafe { values::<FMeshAnimSeq>(ptr::addr_of!((*loaded_mesh).anim_seqs)) }[0];
    assert_eq!(
        text(restored, i32::from_ne_bytes(loaded_sequence.name)),
        "WalkCycle"
    );
    assert_eq!(
        text(restored, i32::from_ne_bytes(loaded_sequence.group)),
        "Movement"
    );
    let loaded_notify =
        unsafe { values::<FMeshAnimNotify>(ptr::addr_of!(loaded_sequence.notifys)) }[0];
    assert_eq!(
        text(restored, i32::from_ne_bytes(loaded_notify.function)),
        "StepNotify"
    );
    assert_eq!(loaded_notify.time, 0.375);
    assert_eq!(
        unsafe { (*loaded_lod).lod_min_verts },
        2,
        "LOD minimum clamps to the loaded frame vertex count"
    );
    assert_eq!(unsafe { (*loaded_lod).old_frame_verts }, 3);
    let loaded_lod_mesh = unsafe { ptr::addr_of_mut!((*loaded_lod).base) };
    assert_eq!(unsafe { (*loaded_lod_mesh).frame_verts }, 2);
    assert_eq!(unsafe { (*loaded_lod_mesh).anim_frames }, 2);
    assert_eq!(
        unsafe {
            values::<u32>(lazy_array(
                ptr::addr_of_mut!((*loaded_lod_mesh).verts).cast(),
                generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
            ))
        },
        [30, 10, 60, 40],
        "linker-backed lazy vertices pass through the existing per-frame LOD remap policy"
    );
    assert!(
        unsafe { values::<u16>(ptr::addr_of!((*loaded_lod).remap_anim_verts)) }.is_empty(),
        "successful remapping releases the temporary remap array"
    );
    assert!(
        unsafe {
            values::<FMeshTri>(lazy_array(
                ptr::addr_of_mut!((*loaded_lod_mesh).tris).cast(),
                generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
            ))
        }
        .is_empty()
    );
    let restored_face = unsafe { values::<FMeshFace>(ptr::addr_of!((*loaded_lod).faces)) }[0];
    assert_eq!(restored_face.i_wedge, [0, 1, 2]);
    let restored_wedges = unsafe { values::<FMeshWedge>(ptr::addr_of!((*loaded_lod).wedges)) };
    assert_eq!(restored_wedges[2].i_vertex, 2);
    assert_eq!(restored_wedges[2].tex_uv.v, 6);
    let restored_material =
        unsafe { values::<FMeshMaterial>(ptr::addr_of!((*loaded_lod).materials)) }[0];
    assert_eq!(restored_material.poly_flags, 0x80);
    assert_eq!(restored_material.texture_index, -1);
    assert_eq!(
        unsafe { values::<u16>(ptr::addr_of!((*loaded_lod).collapse_point_thus)) },
        [1, 2, 2]
    );
    assert_eq!(
        unsafe { values::<u16>(ptr::addr_of!((*loaded_lod).face_level)) },
        [1]
    );
    assert_eq!(
        unsafe { values::<u16>(ptr::addr_of!((*loaded_lod).collapse_wedge_thus)) },
        [1, 2, 2]
    );
    let restored_special_face =
        unsafe { values::<FMeshFace>(ptr::addr_of!((*loaded_lod).special_faces)) }[0];
    assert_eq!(restored_special_face.i_wedge, [0, 1, 2]);
    assert_eq!(unsafe { (*loaded_lod).mesh_scale_max }, 1.25);
    assert_eq!(unsafe { (*loaded_lod).lod_strength }, 0.75);
    assert_eq!(unsafe { (*loaded_lod).lod_morph }, 0.2);
    assert_eq!(unsafe { (*loaded_lod).lodz_displace }, 0.3);
    assert_eq!(unsafe { (*loaded_lod).model_verts }, 3);
    assert_eq!(unsafe { (*loaded_lod).special_verts }, 0);
    assert_eq!(unsafe { (*loaded_lod).lod_hysteresis }, 0.1);

    // Loading LOD moved the shared linker cursor into a different export.
    // Mesh's deferred body must load its own bytes and restore that file cursor.
    let mesh_vertices = unsafe { ptr::addr_of_mut!((*loaded_mesh).verts) };
    let mesh_vertices_slots = unsafe {
        LazyArraySlots {
            array: lazy_array(
                mesh_vertices.cast(),
                generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
            ),
            loader: mesh_vertices
                .cast::<u8>()
                .add(generated::TLazyMeshVerts::FLAZYLOADER_OFFSET)
                .cast(),
            saved_archive: ptr::addr_of_mut!((*mesh_vertices).saved_ar),
            saved_position: ptr::addr_of_mut!((*mesh_vertices).saved_pos),
        }
    };
    assert!(unsafe { mesh_vertices_slots.saved_position.read_unaligned() } > 0);
    unsafe { lazy_array::lazy_array_load(restored, mesh_vertices_slots) }
        .expect("load Mesh source after another export moved the linker cursor");
    assert_eq!(
        unsafe { values::<u32>(mesh_vertices_slots.array) },
        [0x1122_3344, 0x5566_7788, 0x99AA_BBCC]
    );
    let lod_vertices = unsafe { ptr::addr_of_mut!((*loaded_lod_mesh).verts) };
    let lod_vertices_slots = unsafe {
        LazyArraySlots {
            array: lazy_array(
                lod_vertices.cast(),
                generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
            ),
            loader: lod_vertices
                .cast::<u8>()
                .add(generated::TLazyMeshVerts::FLAZYLOADER_OFFSET)
                .cast(),
            saved_archive: ptr::addr_of_mut!((*lod_vertices).saved_ar),
            saved_position: ptr::addr_of_mut!((*lod_vertices).saved_pos),
        }
    };
    let saved_archive = unsafe { lod_vertices_slots.saved_archive.read_unaligned() };
    assert!(
        !saved_archive.is_null(),
        "remapped LodMesh vertices remain attached to their package linker"
    );
    let linker = unsafe {
        saved_archive
            .cast::<u8>()
            .sub(generated::ULinkerLoad::FARCHIVE_OFFSET)
            .cast::<generated::ULinkerLoad>()
    };
    let loaders_before = unsafe { ptr::read_unaligned(ptr::addr_of!((*linker).lazy_loaders.num)) };
    assert!(loaders_before > 0);
    assert!(
        unsafe { lod_vertices_slots.saved_position.read_unaligned() } < 0,
        "LOD remapping materialized the attached source before unload"
    );
    unsafe { lazy_array::lazy_array_unload(restored, lod_vertices_slots, None) }
        .expect("unload trivial packed vertices through the canonical lazy policy");
    let position_after_unload = unsafe { lod_vertices_slots.saved_position.read_unaligned() };
    assert!(
        position_after_unload > 0,
        "lazy-enabled unloading restores the positive source position"
    );
    let vertices_after_unload = unsafe {
        values::<u32>(lazy_array(
            lod_vertices.cast(),
            generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
        ))
    };
    assert!(
        vertices_after_unload.is_empty(),
        "lazy-enabled unloading releases resident vertices"
    );
    assert_eq!(
        unsafe { ptr::read_unaligned(ptr::addr_of!((*linker).lazy_loaders.num)) },
        loaders_before,
        "unloading preserves its source attachment"
    );
    unsafe { lazy_array::lazy_array_load(restored, lod_vertices_slots) }
        .expect("reload packed vertices through their registered typed source");
    assert_eq!(
        unsafe { values::<u32>(lod_vertices_slots.array) },
        [10, 20, 30, 40, 50, 60],
        "reloading reads the original source, not the discarded LOD-remapped resident array"
    );
    assert!(unsafe { lod_vertices_slots.saved_position.read_unaligned() } < 0);
    unsafe { lazy_array::lazy_array_destroy(restored, lod_vertices_slots) }
        .expect("destroy the lazy source registration before object teardown");
    assert!(unsafe { lod_vertices_slots.saved_archive.read_unaligned() }.is_null());
    assert_eq!(
        unsafe { lod_vertices_slots.saved_position.read_unaligned() },
        0
    );
    assert_eq!(
        unsafe { ptr::read_unaligned(ptr::addr_of!((*linker).lazy_loaders.num)) },
        loaders_before - 1,
        "lazy destruction removes exactly this measured loader identity"
    );
    restored_owner
        .shutdown()
        .expect("destroy linker-backed mesh graph");

    // A valid export can defer a malformed array body until linker destruction.
    // Corrupt the compact count, not its skip boundary, so ordinary loading must
    // retain the real source and final teardown must report the delayed error.
    let mut malformed_bytes = std::fs::read(&package_path).expect("read saved mesh package");
    malformed_bytes
        [usize::try_from(triangle_saved_position).expect("positive triangle body offset")] = 0x81;
    let malformed_dir = root.join("Malformed");
    std::fs::create_dir_all(&malformed_dir).expect("create malformed package artifact directory");
    let malformed_path = malformed_dir.join("MeshArchive.u");
    std::fs::write(&malformed_path, malformed_bytes)
        .expect("write malformed compact negative count");
    let malformed_owner =
        EngineOwner::new(options(&root)).expect("initialize malformed-source owner");
    let malformed = malformed_owner.access();
    malformed.set_process_mode(CoreProcessMode {
        is_client: true,
        lazy_load: true,
        ..CoreProcessMode::default()
    });
    let malformed_mesh_class = object_class(malformed, "Engine.Mesh");
    unsafe { malformed.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("open genuine Engine metadata in malformed-source owner");
    unsafe { malformed.load_package(&malformed_path, metadata_flags) }
        .expect("open package without reading its deferred triangle body");
    let malformed_mesh = unsafe {
        malformed.static_load_object(malformed_mesh_class, ptr::null_mut(), &mesh_path, 1)
    }
    .expect("load ordinary fields while leaving malformed triangles deferred")
    .cast::<UMesh>();
    let malformed_archive = unsafe { (*malformed_mesh).tris.saved_ar };
    assert!(!malformed_archive.is_null());
    assert!(unsafe { (*malformed_mesh).tris.saved_pos } > 0);
    let malformed_linker = unsafe {
        malformed_archive
            .cast::<u8>()
            .sub(generated::ULinkerLoad::FARCHIVE_OFFSET)
            .cast::<UObject>()
    };
    let malformed_linker_index = unsafe { (*malformed_linker).index };
    unsafe { malformed.delete_object(malformed_linker) }
        .expect_err("final linker destruction must report the malformed deferred body");
    assert!(
        malformed.object_at(malformed_linker_index).is_null(),
        "failed loading does not prevent final owned linker allocation release"
    );
    let saved_slots = unsafe {
        [
            (
                ptr::addr_of!((*malformed_mesh).verts.saved_ar),
                ptr::addr_of!((*malformed_mesh).verts.saved_pos),
            ),
            (
                ptr::addr_of!((*malformed_mesh).tris.saved_ar),
                ptr::addr_of!((*malformed_mesh).tris.saved_pos),
            ),
            (
                ptr::addr_of!((*malformed_mesh).connects.saved_ar),
                ptr::addr_of!((*malformed_mesh).connects.saved_pos),
            ),
            (
                ptr::addr_of!((*malformed_mesh).vert_links.saved_ar),
                ptr::addr_of!((*malformed_mesh).vert_links.saved_pos),
            ),
        ]
    };
    for (archive, position) in saved_slots {
        assert!(
            unsafe { archive.read_unaligned() }.is_null(),
            "all lazy fields detach before the failing linker is freed"
        );
        assert_eq!(unsafe { position.read_unaligned() }, 0);
    }
    malformed_owner
        .shutdown()
        .expect("destroy surviving mesh after failed source teardown");
}
