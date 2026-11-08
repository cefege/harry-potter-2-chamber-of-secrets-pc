use hp2_abi::engine::generated::{self, UObject, USound};
use hp2_core::{
    core_access::{CoreAccess, CoreOptions, RuntimeError, RuntimeResult},
    object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive},
    package_archive::ExportArchive,
    name_owner::FindName,
};
use hp2_engine::EngineOwner;
use std::{path::{Path, PathBuf}, ptr};


fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn isolated_root() -> PathBuf {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR must name the pre-created test artifact directory"),
    );
    assert!(artifact.is_dir(), "HP2_ARTIFACT_DIR exists before the test starts");
    let root = artifact.join(format!("sound-owner-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove this test's previous isolated runtime");
    }
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    std::fs::write(root.join("System/Default.ini"), "[Core.System]\nPaths=../System/*.u\n").expect("write default configuration");
    std::fs::write(root.join("System/DefUser.ini"), "").expect("write user configuration");
    let prototype = PathBuf::from(
        std::env::var_os("HP2_CORE_DATA_ROOT")
            .expect("HP2_CORE_DATA_ROOT must name the data-prototype runtime"),
    );
    for package in ["Core.u", "Engine.u"] {
        std::fs::copy(
            prototype.join("System").join(package),
            root.join("System").join(package),
        )
        .expect("copy actual prototype metadata before constructing Sound");
    }
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

#[derive(Default)]
struct Mapping;
impl ArchiveMapping for Mapping {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new("unexpected object load in sound transaction"))
    }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> {
        Err(RuntimeError::new("unexpected object save in sound transaction"))
    }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new("unexpected name load in sound transaction"))
    }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new("unexpected name save in sound transaction"))
    }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> {
        Ok(())
    }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> {
        Ok(())
    }
}

fn save_sound(access: CoreAccess<'_>, sound: *mut UObject, version: u16) -> Vec<u8> {
    let mapping = Mapping::default();
    let archive = ObjectArchive::transaction_saving(
        ArchiveFlags::new(version, 0, true),
        &mapping,
    );
    unsafe { access.serialize_object(sound, &archive) }
        .expect("serialize canonical Sound through its genuine ClassOps");
    archive.into_written_bytes().expect("finish sound transaction archive")
}

#[test]
fn canonical_sound_preserves_file_type_name_and_obeys_archive_version_boundaries() {
    let root = isolated_root();
    let owner = EngineOwner::new(options(&root)).expect("initialize canonical engine owner");
    let access = owner.access();
    let package_name = access.intern_str("SoundFixture", FindName::Add).expect("intern Sound package");
    let package = unsafe {
        access.construct_object(access.native_class("Package").expect("Core.Package"), ptr::null_mut(), package_name, 4, ptr::null_mut())
    }.expect("construct canonical Sound package");
    let class = access.native_class("Engine.Sound").expect("canonical Engine.Sound");
    let name = access.intern_str("CanonicalSound", FindName::Add).expect("intern Sound name");
    let object = unsafe { access.construct_object(class, package, name, 4, ptr::null_mut()) }
        .expect("construct canonical Sound");
    let sound = object.cast::<USound>();
    let file_type = access.intern_str("CanonicalSoundFormat", FindName::Add).expect("intern the Sound FileType");
    unsafe { (*sound).file_type = file_type.to_ne_bytes(); }
    unsafe { access.add_root(object); }
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect with canonical Sound rooted");
    assert_eq!(
        access.intern_str("CanonicalSoundFormat", FindName::Find).expect("find retained Sound format"),
        file_type,
        "Sound's non-reflected FileType survives real name collection",
    );

    for version in [76, 77, 78, 79] {
        unsafe {
            (*sound).raw_num_samples = 13;
            (*sound).raw_bits_per_sample = 24;
            (*sound).raw_num_channels = 35;
            (*sound).raw_sample_rate = 46;
        }
        let bytes = save_sound(access, object, version);
        unsafe {
            (*sound).raw_num_samples = -11;
            (*sound).raw_bits_per_sample = -22;
            (*sound).raw_num_channels = -33;
            (*sound).raw_sample_rate = -44;
        }
        let mapping = Mapping;
        let loading = ObjectArchive::transaction_loading(
            ExportArchive::from_slice(&bytes), ArchiveFlags::new(version, 0, true), &mapping,
        );
        unsafe { access.serialize_object(object, &loading) }.expect("load versioned Sound raw format");
        assert_eq!(
            unsafe { [(*sound).raw_num_samples, (*sound).raw_bits_per_sample, (*sound).raw_num_channels, (*sound).raw_sample_rate] },
            [if version > 76 { 13 } else { -11 }, if version > 77 { 24 } else { -22 },
             if version > 77 { 35 } else { -33 }, if version > 78 { 46 } else { -44 }],
            "raw format fields obey their actual archive-version boundaries",
        );
    }

    access.remove_root(object);

    owner.shutdown().expect("destroy canonical Sound members");
}
