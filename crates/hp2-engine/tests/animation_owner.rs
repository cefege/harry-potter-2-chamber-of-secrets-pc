use hp2_abi::engine::generated::{FAnalogTrack, FAnimVec, FMeshAnimNotify, FMeshAnimSeq, FNamedBone, MotionChunk, TArrayRaw, UAnimation, UGesture, UObject};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_core::package_archive::ExportArchive;
use hp2_engine::EngineOwner;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::ptr;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
fn root() -> PathBuf {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR names a pre-created artifact directory"));
    assert!(artifact.is_dir(), "HP2_ARTIFACT_DIR exists");
    let root = artifact.join(format!("animation-owner-{}", std::process::id()));
    if root.exists() { std::fs::remove_dir_all(&root).expect("remove this test's previous fixture"); }
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    std::fs::write(root.join("System/Default.ini"), "").expect("write default configuration");
    std::fs::write(root.join("System/DefUser.ini"), "").expect("write user configuration");
    root
}
fn options(root: &Path) -> CoreOptions {
    CoreOptions {
        data_root: root.to_path_buf(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }
}

struct Mapping<'a>(CoreAccess<'a>, RefCell<Vec<i32>>);
impl ArchiveMapping for Mapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 { return Ok(ptr::null_mut()); }
        let object = self.0.object_at(index - 1);
        if object.is_null() { return Err(hp2_core::core_access::RuntimeError::new("invalid fixture object index")); }
        Ok(object)
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { Ok(0) } else { Ok(unsafe { (*object).index + 1 }) }
    }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> { self.0.name_text(index).map(|_| index) }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> { self.load_name(index) }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> { Ok(()) }
    fn visit_name(&self, index: i32) -> RuntimeResult<()> {
        self.1.borrow_mut().push(index);
        Ok(())
    }
}

unsafe fn set_values<T: Copy>(raw: *mut TArrayRaw, values: &[T]) {
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).num) },
        max: unsafe { ptr::addr_of_mut!((*raw).max) },
    };
    let count = i32::try_from(values.len()).expect("fixture array count fits native INT");
    if count == 0 { return; }
    unsafe { slots.max.write(count); }
    unsafe { array_storage::realloc(slots, size_of::<T>() as i32, ptr::null(), array_storage::system_allocator()) }
        .expect("allocate fixture array with Core's paired system allocator");
    let data = unsafe { slots.data.read_unaligned().cast::<T>() };
    for (index, item) in values.iter().copied().enumerate() { unsafe { data.add(index).write(item); } }
    unsafe { slots.num.write(count); }
}
unsafe fn values<T: Copy>(raw: *const TArrayRaw) -> Vec<T> {
    let value = unsafe { ptr::read_unaligned(raw) };
    assert!(value.num >= 0, "array count is nonnegative");
    if value.num == 0 { return Vec::new(); }
    assert!(!value.data.is_null(), "nonempty array owns storage");
    unsafe { std::slice::from_raw_parts(value.data.cast::<T>(), value.num as usize).to_vec() }
}
fn name(access: CoreAccess<'_>, text: &str) -> [u8; 4] {
    access.intern_str(text, FindName::Add).expect("intern animation fixture name").to_ne_bytes()
}

#[test]
fn canonical_animation_nested_wire_roundtrip_and_truncated_load_cleanup() {
    let root = root();
    let owner = EngineOwner::new(options(&root)).expect("initialize canonical engine owner");
    let access = owner.access();
    let mapping = Mapping(access, RefCell::new(Vec::new()));
    let class = access.native_class("Engine.Animation").expect("resolve canonical Animation");
    let package_name = access.intern_str("AnimationFixture", FindName::Add).expect("intern fixture package");
    let package = unsafe {
        access.construct_object(access.native_class("Package").expect("Core.Package"),
            ptr::null_mut(), package_name, 4, ptr::null_mut())
    }.expect("construct canonical fixture package");
    let first_name = access.intern_str("AnimationSource", FindName::Add).expect("intern source object name");
    let source = unsafe { access.construct_object(class, package, first_name, 4, ptr::null_mut()) }
        .expect("construct canonical animation source").cast::<UAnimation>();

    let bone = FNamedBone { name: name(access, "Root"), flags: 0x20, parent_index: 0 };
    unsafe { set_values(ptr::addr_of_mut!((*source).ref_bones), &[bone]); }
    let mut motion: MotionChunk = unsafe { std::mem::zeroed() };
    motion.root_speed3_d = hp2_abi::FVector { x: 1.25, y: -2.5, z: 8.0 };
    motion.track_time = 2.75;
    motion.start_bone = 0;
    motion.flags = 0x31;
    unsafe { set_values(ptr::addr_of_mut!(motion.bone_indices), &[0i32]); }
    let mut track: FAnalogTrack = unsafe { std::mem::zeroed() };
    track.flags = 0x42;
    track.pos_scale = 0.75;
    track.time_scale = 1.5;
    unsafe { set_values(ptr::addr_of_mut!(motion.anim_tracks), &[track]); }
    unsafe { set_values(ptr::addr_of_mut!((*source).moves), &[motion]); }
    let master_quat = [FAnimVec { x: 101, y: -202, z: 303 }];
    let master_pos = [FAnimVec { x: -404, y: 505, z: -606 }];
    unsafe {
        set_values(ptr::addr_of_mut!((*source).master_track.key_quat), &master_quat);
        set_values(ptr::addr_of_mut!((*source).master_track.key_pos), &master_pos);
        set_values(ptr::addr_of_mut!((*source).master_track.key_delta), &[7u8]);
        let moves = ptr::addr_of_mut!((*source).moves);
        let move_data = ptr::read_unaligned(ptr::addr_of!((*moves).data)).cast::<MotionChunk>();
        let tracks = ptr::addr_of_mut!((*move_data).anim_tracks);
        let track_data = ptr::read_unaligned(ptr::addr_of!((*tracks).data)).cast::<FAnalogTrack>();
        let quat_data = ptr::read_unaligned(ptr::addr_of!((*source).master_track.key_quat.data));
        let pos_data = ptr::read_unaligned(ptr::addr_of!((*source).master_track.key_pos.data));
        let delta_data = ptr::read_unaligned(ptr::addr_of!((*source).master_track.key_delta.data));
        ptr::addr_of_mut!((*track_data).key_quat.data).write_unaligned(quat_data);
        ptr::addr_of_mut!((*track_data).key_quat.num).write_unaligned(1);
        ptr::addr_of_mut!((*track_data).key_pos.data).write_unaligned(pos_data);
        ptr::addr_of_mut!((*track_data).key_pos.num).write_unaligned(1);
        ptr::addr_of_mut!((*track_data).key_delta.data).write_unaligned(delta_data);
        ptr::addr_of_mut!((*track_data).key_delta.num).write_unaligned(1);
    }
    let mut sequence: FMeshAnimSeq = unsafe { std::mem::zeroed() };
    sequence.name = name(access, "Walk");
    sequence.group = name(access, "Locomotion");
    sequence.start_frame = 4;
    sequence.num_frames = 11;
    sequence.rate = 29.5;
    let notify = FMeshAnimNotify { time: 0.375, function: name(access, "Step") };
    unsafe {
        set_values(ptr::addr_of_mut!(sequence.notifys), &[notify]);
        set_values(ptr::addr_of_mut!((*source).anim_seqs), &[sequence]);
    }

    let saving = ObjectArchive::saving(ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(source.cast(), &saving) }.expect("serialize nested canonical animation");
    let wire = saving.into_written_bytes().expect("collect animation wire bytes");

    let target_name = access.intern_str("AnimationTarget", FindName::Add).expect("intern target object name");
    let target = unsafe { access.construct_object(class, package, target_name, 4, ptr::null_mut()) }
        .expect("construct canonical animation target").cast::<UAnimation>();
    let loading = ObjectArchive::loading(ExportArchive::from_slice(&wire), ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(target.cast(), &loading) }.expect("load nested canonical animation");
    let loaded_bones = unsafe { values::<FNamedBone>(ptr::addr_of!((*target).ref_bones)) };
    assert_eq!(loaded_bones[0].name, bone.name);
    assert_eq!(loaded_bones[0].parent_index, 0);
    let loaded_moves = unsafe { values::<MotionChunk>(ptr::addr_of!((*target).moves)) };
    assert_eq!(loaded_moves[0].track_time, 2.75);
    assert_eq!(unsafe { values::<i32>(ptr::addr_of!(loaded_moves[0].bone_indices)) }, [0]);
    let loaded_tracks = unsafe { values::<FAnalogTrack>(ptr::addr_of!(loaded_moves[0].anim_tracks)) };
    assert_eq!(loaded_tracks[0].flags, 0x42);
    let quaternion = unsafe { ptr::read_unaligned(loaded_tracks[0].key_quat.data.cast::<FAnimVec>()) };
    let position = unsafe { ptr::read_unaligned(loaded_tracks[0].key_pos.data.cast::<FAnimVec>()) };
    assert_eq!((quaternion.x, quaternion.y, quaternion.z), (101, -202, 303));
    assert_eq!((position.x, position.y, position.z), (-404, 505, -606));
    assert_eq!(unsafe { ptr::read_unaligned(loaded_tracks[0].key_delta.data.cast::<u8>()) }, 7);
    let loaded_sequences = unsafe { values::<FMeshAnimSeq>(ptr::addr_of!((*target).anim_seqs)) };
    assert_eq!((loaded_sequences[0].start_frame, loaded_sequences[0].num_frames, loaded_sequences[0].rate), (4, 11, 29.5));
    let loaded_notifies = unsafe { values::<FMeshAnimNotify>(ptr::addr_of!(loaded_sequences[0].notifys)) };
    assert_eq!(loaded_notifies[0].time, 0.375);
    assert_eq!(loaded_notifies[0].function, notify.function);

    let expected_names = [bone.name, sequence.name, sequence.group, notify.function]
        .map(i32::from_ne_bytes);
    mapping.1.borrow_mut().clear();
    let tagging = ObjectArchive::tag_imports(ArchiveFlags::new(79, 0, true), &mapping);
    unsafe { access.serialize_object(source.cast(), &tagging) }.expect("tag nested animation imports");
    for name in expected_names {
        assert!(mapping.1.borrow().contains(&name), "package tagging retains every nested animation name");
    }

    let transaction = ObjectArchive::transaction_saving(ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(source.cast(), &transaction) }.expect("save animation undo state");
    let transaction = transaction.into_written_bytes().expect("retain animation undo bytes");
    unsafe {
        let data = ptr::read_unaligned(ptr::addr_of!((*source).master_track.key_quat.data)).cast::<FAnimVec>();
        ptr::addr_of_mut!((*data).x).write_unaligned(19001);
    }
    let undo = ObjectArchive::transaction_loading(
        ExportArchive::from_slice(&transaction), ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(source.cast(), &undo) }.expect("restore animation undo state");
    let restored_moves = unsafe { values::<MotionChunk>(ptr::addr_of!((*source).moves)) };
    let restored_tracks = unsafe { values::<FAnalogTrack>(ptr::addr_of!(restored_moves[0].anim_tracks)) };
    let restored_key = unsafe { ptr::read_unaligned(restored_tracks[0].key_quat.data.cast::<FAnimVec>()) };
    assert_eq!((restored_key.x, restored_key.y, restored_key.z), (101, -202, 303),
        "undo restores master keys and rebinds track aliases, not just reflected properties");

    let truncated_name = access.intern_str("TruncatedAnimation", FindName::Add).expect("intern malformed target name");
    let truncated = unsafe { access.construct_object(class, package, truncated_name, 4, ptr::null_mut()) }
        .expect("construct malformed-input target");
    let malformed = &wire[..wire.len() - 1];
    let loading = ObjectArchive::loading(ExportArchive::from_slice(malformed), ArchiveFlags::new(79, 0, false), &mapping);
    assert!(unsafe { access.serialize_object(truncated, &loading) }.is_err(), "truncated master track must be rejected");
    let gesture_class = access.native_class("Engine.Gesture").expect("resolve canonical Gesture");
    let gesture_name = access.intern_str("GestureFixture", FindName::Add).expect("intern gesture object name");
    let gesture = unsafe { access.construct_object(gesture_class, package, gesture_name, 4, ptr::null_mut()) }
        .expect("construct canonical gesture").cast::<UGesture>();
    unsafe {
        set_values(ptr::addr_of_mut!((*gesture).points), &[
            hp2_abi::FVector { x: 0.25, y: -0.5, z: 1.0 },
            hp2_abi::FVector { x: 2.0, y: 3.0, z: 4.0 },
        ]);
        set_values(ptr::addr_of_mut!((*gesture).segments), &[1i32, 0]);
    }
    let source_index = unsafe { (*source.cast::<UObject>()).index };
    unsafe { access.add_root(source.cast()); }
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect partial loads and runtime gesture storage");
    for name in expected_names {
        assert!(access.name_text(name).is_ok(), "rooted animation retains nested names through GC");
    }
    access.remove_root(source.cast());
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect the released animation and its storage");
    assert!(access.object_at(source_index).is_null(), "animation does not become a permanent global root");
    owner.shutdown().expect("destroy all canonical animation and gesture storage after success and malformed load");
}
