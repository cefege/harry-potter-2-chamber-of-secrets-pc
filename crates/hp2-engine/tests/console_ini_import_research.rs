use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_engine::EngineOwner;
use std::{fs, path::PathBuf, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
#[test]
fn reflected_float_import_reports_consumption_for_nonnumeric_text() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join("console-property-import-research");
    fs::create_dir_all(&root).unwrap();
    let system = root.join("Default.ini");
    let user = root.join("DefUser.ini");
    fs::write(&system, "").unwrap(); fs::write(&user, "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root, system_ini: system, user_ini: user,
        command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    let class = access.native_class("SDLDrv.SDLClient").unwrap();
    let name = access.intern_str("Brightness", FindName::Find).unwrap();
    let property: *mut hp2_abi::engine::generated::UProperty = unsafe { access.find_field(class.cast(), name, access.native_class("Core.Property").unwrap()).cast() };
    assert!(!property.is_null());
    let mut value = 0.75_f32;
    let input: Vec<u32> = "invalid".chars().map(u32::from).collect();
    let consumed = unsafe { access.property_import_text_at(property, (&mut value as *mut f32).cast(), &input, ptr::null_mut(), 1) }.unwrap();
    // This is not a validator: atof maps nonnumeric text to zero and the
    // parser still consumes it. Transactional SET must reject it separately.
    assert_eq!(consumed, input.len());
    assert_eq!(value, 0.0);

    // A post-publication notification can itself change the reflected value.
    // Persistence of the unnotified value would diverge from the live device.
    let audio_class = access.native_class("ALAudio.ALAudioSubsystem").unwrap();
    let audio = unsafe { access.construct_object(audio_class, access.transient_package(), 0, 0, ptr::null_mut()) }
        .unwrap().cast::<hp2_abi::engine::generated::UALAudioSubsystem>();
    unsafe {
        (*audio).music_volume = 1.5;
        access.post_edit_change(audio.cast()).unwrap();
        assert_eq!((*audio).music_volume, 1.0);
    }
    owner.shutdown().unwrap();
}
