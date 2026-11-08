//! Data-prototype spell runtime contracts: the stock Core/Engine/HGame
//! package-load boundary (`Tests/SpellRuntimeTests.cpp`).
//!
//! The prototype data root follows the engine-suite convention: the CTest
//! runner exports `HP2_CORE_DATA_ROOT` (with `HP2_TEST_DATA_ROOT` accepted as
//! the CMake-side spelling), and a bare `cargo test` falls back to the
//! checkout's `HarryPotter2/Unreal`. A libtest harness rejects the C++
//! `-datadir=` argument, so the root is never taken from `argv`.
use hp2_abi::engine::generated::{ULinkerLoad, UObject, UStruct};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_model::is_a;
use hp2_engine::EngineOwner;
use std::{
    fs,
    path::{Path, PathBuf},
    ptr,
};

/// `RequiredHarryBools` from `Tests/SpellRuntimeTests.cpp`.
const REQUIRED_HARRY_BOOLS: [&str; 13] = [
    "bScreenRelativeMovement",
    "bAutoCenterCamera",
    "bLockedOnTarget",
    "bFixedFaceDirection",
    "bInDuelingMode",
    "bReverseInput",
    "bKeepStationary",
    "bLockOutForward",
    "bLockOutBackward",
    "bLockOutStrafeLeft",
    "bLockOutStrafeRight",
    "bIsAiming",
    "bE3DemoLockout",
];

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
        None => std::env::temp_dir().join(format!("hp2-spell-runtime-{}", std::process::id())),
    };
    fs::create_dir_all(&artifact).expect("create the isolated spell runtime artifact directory");
    let root = artifact.join(format!("spell-runtime-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove this test's previous isolated runtime");
    }
    let data = data_root();
    assert!(
        data.join("System/HGame.u").is_file(),
        "the prototype data root must carry System/HGame.u"
    );
    copy_tree(&data.join("System"), &root.join("System"));
    // Script defaults import real asset packages through Core.System's Paths.
    // Keep writable System isolated; these dependency roots are read-only inputs.
    for directory in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(directory);
        if source.is_dir() {
            std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(directory))
                .expect("link the prototype package dependency directory");
        }
    }
    root
}

fn prototype_owner() -> (EngineOwner, PathBuf) {
    let root = isolated_root();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("initialize canonical Core and Engine spell runtime graph");
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

/// `Cast<UPackage>(LoadPackage(...))`: the linker must own a package root of
/// the canonical `Core.Package` class.
unsafe fn loaded_package(access: CoreAccess<'_>, path: &Path) -> Option<*mut UObject> {
    let linker = unsafe { access.load_package(path, 1) }.ok()?;
    let package = unsafe {
        let root = (*linker.cast::<ULinkerLoad>())
            .base
            .linker_root
            .cast::<UObject>();
        if root.is_null() {
            (*linker).outer.cast::<UObject>()
        } else {
            root
        }
    };
    if package.is_null() {
        return None;
    }
    let package_class = access.native_class("Core.Package").expect("Core.Package native class");
    unsafe { is_a(package.cast(), package_class.cast()) }.then_some(package)
}

#[test]
fn stock_core_engine_hgame_package_load_boundary() {
    let (owner, root) = prototype_owner();
    let access = owner.access();
    let engine = unsafe { loaded_package(access, &root.join("System/Engine.u")) };
    assert!(engine.is_some(), "Engine package load failed");
    let hgame = unsafe { loaded_package(access, &root.join("System/HGame.u")) };
    assert!(hgame.is_some(), "HGame package load failed");

    let player_pawn = access
        .native_class("Engine.PlayerPawn")
        .expect("Engine.PlayerPawn native class");
    // `StaticLoadClass(APlayerPawn::StaticClass(), NULL, TEXT("HGame.Harry"), NULL, LOAD_NoWarn, NULL)`.
    let harry = unsafe {
        access.static_load_class(player_pawn, ptr::null_mut(), &utf32("HGame.Harry"), 2)
    }
    .ok()
    .filter(|class| !class.is_null());
    assert!(
        harry.is_some(),
        "HGame.Harry class is unavailable after package load"
    );
    let harry = harry.unwrap().cast::<UStruct>();

    let bool_property = access
        .native_class("Core.BoolProperty")
        .expect("Core.BoolProperty native class");
    for name in REQUIRED_HARRY_BOOLS {
        let index = access
            .intern_str(name, FindName::Find)
            .expect("intern a required HGame.Harry bool name");
        let field = unsafe { access.find_field(harry, index, bool_property) };
        assert!(
            !field.is_null(),
            "HGame.Harry.{name} is missing or is not a bool property"
        );
    }

    let object_property = access
        .native_class("Core.ObjectProperty")
        .expect("Core.ObjectProperty native class");
    let boss_index = access
        .intern_str("BossTarget", FindName::Find)
        .expect("intern the HGame.Harry.BossTarget name");
    let boss_target = unsafe { access.find_field(harry, boss_index, object_property) };
    assert!(
        !boss_target.is_null(),
        "HGame.Harry.BossTarget is missing or is not an object property"
    );

    // `StaticFindObject(UClass::StaticClass(), ANY_PACKAGE, TEXT("Engine.Mover"), 1)`.
    let class_class = access.native_class("Core.Class").expect("Core.Class native class");
    let mover = unsafe { access.find_object_by_name(class_class, &utf32("Engine.Mover")) }
        .expect("resolve Engine.Mover");
    assert!(
        mover.is_some_and(|mover| unsafe { (*mover.cast::<UObject>()).class == class_class.cast() }),
        "Engine.Mover class is unavailable after package load"
    );

    owner.shutdown().expect("shutdown loaded canonical spell runtime graph");
    fs::remove_dir_all(&root).expect("remove the isolated spell runtime root");
}