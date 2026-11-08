use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    FMeshBone, TArrayRaw, UObject, USkeletalMesh, VBoneInfIndex, VBoneInfluence,
};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_engine::EngineOwner;
use std::{
    path::{Path, PathBuf},
    ptr,
};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
fn fixture_root() -> PathBuf {
    let artifacts = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR is supplied by CTest"),
    );
    let root = artifacts.join(format!("skeletal-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    for ini in ["Default.ini", "DefUser.ini"] {
        std::fs::write(root.join("System").join(ini), "").expect("write empty test configuration");
    }
    root
}
fn options(root: &Path) -> CoreOptions {
    CoreOptions {
        data_root: root.into(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    }
}
fn name(access: CoreAccess<'_>, text: &str) -> i32 {
    access
        .intern_str(text, FindName::Add)
        .expect("intern fixture name")
}
unsafe fn create(
    access: CoreAccess<'_>,
    package: *mut UObject,
    class: &str,
    object_name: &str,
) -> *mut UObject {
    // SAFETY: each class is registered by this owner; the outer belongs to that owner.
    unsafe {
        access.construct_object(
            access.native_class(class).expect("canonical class"),
            package,
            name(access, object_name),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct canonical fixture object")
}
unsafe fn assign<T: Copy>(raw: *mut TArrayRaw, items: &[T]) {
    // SAFETY: callers provide a canonical member with the matching element layout.
    let slots = unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*raw).data),
            num: ptr::addr_of_mut!((*raw).num),
            max: ptr::addr_of_mut!((*raw).max),
        }
    };
    // SAFETY: Core's allocator is paired with the canonical owner's final release.
    unsafe {
        array_storage::set_exact(
            slots,
            items.len() as i32,
            size_of::<T>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
        .expect("allocate fixture member data");
        if !items.is_empty() {
            ptr::copy_nonoverlapping(
                items.as_ptr(),
                slots.data.read_unaligned().cast(),
                items.len(),
            );
        }
    }
}
struct Mapping<'a>(CoreAccess<'a>);
impl ArchiveMapping for Mapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 {
            return Ok(ptr::null_mut());
        }
        let object = self.0.object_at(index - 1);
        if object.is_null() {
            Err(RuntimeError::new("Invalid fixture object index"))
        } else {
            Ok(object)
        }
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        // SAFETY: this mapping only receives references from this canonical owner.
        Ok(if object.is_null() {
            0
        } else {
            unsafe { (*object).index + 1 }
        })
    }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> {
        self.0.name_text(index).map(|_| index)
    }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> {
        self.load_name(index)
    }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> {
        Ok(())
    }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> {
        Ok(())
    }
}

#[test]
fn skeletal_wire_preserves_joint_quirks_and_gc_follows_animation_and_bone_names() {
    let root = fixture_root();
    let owner = EngineOwner::new(options(&root)).expect("initialize canonical engine owner");
    let access = owner.access();
    // SAFETY: Package permits a null outer; all other records use that real package.
    let package = unsafe { create(access, ptr::null_mut(), "Package", "SkeletalFixture") };
    // SAFETY: these registered classes use the canonical compiler-probed layouts.
    let mesh = unsafe { create(access, package, "Engine.SkeletalMesh", "Character") }
        .cast::<USkeletalMesh>();
    let animation = unsafe { create(access, package, "Engine.Animation", "Idle") };
    let bone_name = name(access, "CharacterRootBone");
    let mut bone: FMeshBone = unsafe { std::mem::zeroed() };
    bone.name = bone_name.to_ne_bytes();
    bone.flags = 0x7428_91c5;
    bone.bone_pos.place.quat = [0.125, -0.25, 0.5, 1.0];
    bone.bone_pos.place.pos = FVector {
        x: -11.5,
        y: 21.25,
        z: 31.75,
    };
    bone.bone_pos.length = 43.5;
    bone.bone_pos.x_size = 57.25;
    bone.bone_pos.y_size = 89.75;
    bone.bone_pos.z_size = 63.5;
    bone.num_children = 7;
    bone.parent_index = -1;
    bone.depth = 123;
    let influence_index = VBoneInfIndex {
        weight_index: 3,
        number: 2,
        detail_a: 5,
        detail_b: 7,
    };
    let influence = VBoneInfluence {
        point_index: 19,
        bone_weight: 32767,
    };
    // SAFETY: member slots belong to this live canonical object. Each assignment
    // transfers an independent, correctly typed allocation to the owner.
    unsafe {
        assign(ptr::addr_of_mut!((*mesh).ref_skeleton), &[bone]);
        assign(
            ptr::addr_of_mut!((*mesh).bone_weight_idx),
            &[influence_index],
        );
        assign(ptr::addr_of_mut!((*mesh).bone_weights), &[influence]);
        assign(
            ptr::addr_of_mut!((*mesh).points),
            &[FVector {
                x: 2.0,
                y: 4.0,
                z: 8.0,
            }],
        );
        assign(
            ptr::addr_of_mut!((*mesh).local_points),
            &[FVector {
                x: -2.0,
                y: -4.0,
                z: -8.0,
            }],
        );
        assign(
            ptr::addr_of_mut!((*mesh).debug_pivots),
            &[FVector {
                x: 1.0,
                y: 3.0,
                z: 5.0,
            }],
        );
        assign(ptr::addr_of_mut!((*mesh).debug_parents), &[-1i32, 0]);
        assign(ptr::addr_of_mut!((*mesh).debug_verts), &[3u8, 9]);
        (*mesh).default_animation = animation.cast();
        (*mesh).skeletal_depth = 5;
        (*mesh).weapon_bone_index = -1;
    }
    let mapping = Mapping(access);
    let saving = ObjectArchive::saving(ArchiveFlags::new(79, 0, false), &mapping);
    // SAFETY: saving does not require a lazy source attachment; the object is canonical.
    unsafe { access.serialize_object(mesh.cast(), &saving) }
        .expect("serialize skeletal native payload");
    let wire = saving
        .into_written_bytes()
        .expect("retain skeletal wire bytes");
    let mut expected = Vec::new();
    expected.extend(bone.flags.to_le_bytes());
    for value in [
        0.125f32, -0.25, 0.5, 1.0, -11.5, 21.25, 31.75, 43.5, 57.25, 57.25, 63.5,
    ] {
        expected.extend(value.to_le_bytes());
    }
    expected.extend(7i32.to_le_bytes());
    expected.extend((-1i32).to_le_bytes());
    expected.push(1); // The next array's compact count follows ParentIndex, not Depth.
    for value in [3u16, 2, 5, 7] {
        expected.extend(value.to_le_bytes());
    }
    expected.push(1);
    for value in [19u16, 32767] {
        expected.extend(value.to_le_bytes());
    }
    assert!(
        wire.windows(expected.len()).any(|bytes| bytes == expected),
        "package79 skeletal wire repeats XSize, omits YSize/Depth, and writes children before parent and influence arrays"
    );

    let animation_index = unsafe { (*animation).index };
    let mesh_index = unsafe { (*mesh.cast::<UObject>()).index };
    unsafe {
        access.add_root(mesh.cast());
    }
    access
        .collect_garbage(0, 0, &|_| Ok(()))
        .expect("trace rooted skeletal graph");
    assert_eq!(
        access.object_at(animation_index),
        animation,
        "DefaultAnimation is a real owning-graph reference, not an untraced native pointer"
    );
    assert!(
        access.name_text(bone_name).is_ok(),
        "RefSkeleton names survive with their rooted mesh"
    );
    access.remove_root(mesh.cast());
    access
        .collect_garbage(0, 0, &|_| Ok(()))
        .expect("collect released skeletal graph and transient member storage");
    assert!(
        access.object_at(mesh_index).is_null(),
        "skeletal mesh is not a permanent root"
    );
    assert!(
        access.object_at(animation_index).is_null(),
        "animation dies when its last rooted mesh reference disappears"
    );
    owner
        .shutdown()
        .expect("destroy remaining canonical storage");
}
