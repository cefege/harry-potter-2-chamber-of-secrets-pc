//! Probe: does a script-side write to `PlayerPawn.bLumosOn` (as the VM does
//! in `LumosLight::TurnOn`) land in the native struct bit that the Rust
//! renderer reads at `APlayerPawn` offset 2912 / `f_lumos_radius` 2916?
//!
//! Run with `HP2_CORE_DATA_ROOT` pointing at the data root under test
//! (prototype `HarryPotter2/Unreal` or the retail overlay root).
use hp2_abi::engine::generated::{APlayerPawn, UBoolProperty, UClass, UObject, UProperty};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_model::is_a;
use hp2_engine::EngineOwner;
use std::{fs, path::{Path, PathBuf}, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn utf32(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}

fn data_root() -> PathBuf {
    for name in ["HP2_CORE_DATA_ROOT", "HP2_TEST_DATA_ROOT"] {
        if let Some(root) = std::env::var_os(name) {
            return PathBuf::from(root);
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal")
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn isolated_root() -> PathBuf {
    let artifact = match std::env::var_os("HP2_ARTIFACT_DIR") {
        Some(path) => PathBuf::from(path),
        None => std::env::temp_dir().join(format!("hp2-lumos-probe-{}", std::process::id())),
    };
    fs::create_dir_all(&artifact).expect("create the isolated lumos probe artifact directory");
    let root = artifact.join(format!("lumos-probe-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove this test's previous isolated runtime");
    }
    let data = data_root();
    assert!(
        data.join("System/HGame.u").is_file(),
        "the data root must carry System/HGame.u: {}",
        data.display()
    );
    copy_tree(&data.join("System"), &root.join("System"));
    for directory in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(directory);
        if source.is_dir() {
            std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(directory))
                .expect("link the package dependency directory");
        }
    }
    root
}

fn probe_owner() -> (EngineOwner, PathBuf) {
    let root = isolated_root();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("initialize canonical Core and Engine graph");
    let access = owner.access();
    access.set_process_mode(CoreProcessMode {
        is_client: true,
        is_server: true,
        is_editor: true,
        is_scriptable: true,
        lazy_load: false,
    });
    (owner, root)
}

#[test]
fn lumos_bool_round_trip_reaches_native_struct_bit() {
    let (owner, root) = probe_owner();
    let access = owner.access();

    // Load the script packages, then resolve HGame.Harry as `StaticLoadClass`
    // does for the player pawn.
    for package in ["Engine.u", "HGame.u"] {
        let linker = unsafe { access.load_package(&root.join("System").join(package), 1) };
        assert!(linker.is_ok(), "load {}", package);
    }
    let player_pawn = access
        .native_class("Engine.PlayerPawn")
        .expect("Engine.PlayerPawn native class");
    let harry = unsafe {
        access.static_load_class(player_pawn, ptr::null_mut(), &utf32("HGame.Harry"), 2)
    }
    .ok()
    .filter(|class| !class.is_null())
    .expect("HGame.Harry class available")
    .cast::<UClass>();

    // Construct a scratch Harry object (as spawn does) to write into.
    let name = access
        .intern_str("LumosProbeHarry", FindName::Add)
        .expect("intern scratch name");
    let scratch = unsafe {
        access.construct_object(harry, harry.cast(), name, 0x0000_0004 /* PUBLIC */, ptr::null_mut())
    }
    .expect("construct scratch Harry object");
    assert!(unsafe { is_a(scratch.cast(), harry.cast()) });
    let pawn = unsafe { scratch.cast::<APlayerPawn>() };

    let bool_property = access
        .native_class("Core.BoolProperty")
        .expect("Core.BoolProperty native class");
    let name_index = access
        .intern_str("bLumosOn", FindName::Find)
        .expect("intern bLumosOn");
    let property = unsafe { access.find_field(harry.cast(), name_index, bool_property) };
    assert!(
        !property.is_null(),
        "PlayerPawn.bLumosOn property is not found on HGame.Harry"
    );

    let prop = unsafe { &*property.cast::<UProperty>() };
    let bit_mask = unsafe { (*property.cast::<UBoolProperty>()).bit_mask };
    let address = unsafe { access.property_address(scratch, property.cast()) }
        .expect("bLumosOn property address");
    let struct_addr = (pawn as *mut APlayerPawn as usize) + 2912;
    let struct_value_before = unsafe { (*pawn).b_lumos_on() };
    eprintln!(
        "PROBE bLumosOn offset={} bit_mask={} prop_addr=0x{:x} struct_bits_addr=0x{:x} before={}",
        prop.offset,
        bit_mask,
        address as usize,
        struct_addr,
        struct_value_before
    );

    // VM-style write: OR the bit into the property byte.
    unsafe {
        *address.cast::<u8>() |= bit_mask as u8;
    }
    let struct_after = unsafe { (*pawn).b_lumos_on() };
    let reflected_after = unsafe { *address.cast::<u8>() & bit_mask as u8 != 0 };
    let struct_byte = unsafe { *(struct_addr as *const u8) };
    eprintln!(
        "PROBE after write: struct_b_lumos_on={} reflected={} struct_byte={:#04x}",
        struct_after, reflected_after, struct_byte
    );

    // Same round trip for fLumosRadius (the lumos surface-fade radius read).
    let float_property = access
        .native_class("Core.FloatProperty")
        .expect("Core.FloatProperty native class");
    let radius_index = access
        .intern_str("fLumosRadius", FindName::Find)
        .expect("intern fLumosRadius");
    let radius_prop = unsafe { access.find_field(harry.cast(), radius_index, float_property) };
    assert!(!radius_prop.is_null(), "fLumosRadius property missing");
    let radius_addr = unsafe { access.property_address(scratch, radius_prop.cast()) }
        .expect("fLumosRadius property address");
    unsafe {
        *(radius_addr.cast::<f32>()) = 1234.0f32;
    }
    let struct_radius = unsafe { (*pawn).f_lumos_radius };
    let reflected_radius = unsafe { *radius_addr.cast::<f32>() };
    eprintln!(
        "PROBE fLumosRadius struct={} reflected={}",
        struct_radius, reflected_radius
    );

    assert!(
        struct_after,
        "FAIL: the VM-style script write did not reach the native struct bit the renderer reads"
    );
    assert_eq!(
        struct_radius, reflected_radius,
        "FAIL: fLumosRadius script write did not reach the native struct field"
    );
    owner.shutdown().expect("shutdown");
    let _ = fs::remove_dir_all(&root);
}