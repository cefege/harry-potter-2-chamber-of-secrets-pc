use hp2_abi::engine::generated::{TArrayRaw, UObject, ULevelSummary};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_engine::EngineOwner;

use std::path::{Path, PathBuf};
use std::ptr;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn data_root() -> PathBuf {
    std::env::var_os("HP2_CORE_DATA_ROOT")
        .map(PathBuf::from)
        .expect("HP2_CORE_DATA_ROOT must point to the prototype Unreal data root")
}

fn sandbox() -> PathBuf {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR must name the pre-created test artifact directory"),
    );
    assert!(artifact.is_dir(), "HP2_ARTIFACT_DIR must exist before the consumer starts");
    let root = artifact.join(format!("level-summary-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    std::fs::create_dir_all(root.join("Maps")).expect("create isolated Maps directory");
    let source = data_root();
    for name in ["Core.u", "Engine.u", "Default.ini", "DefUser.ini"] {
        std::fs::copy(source.join("System").join(name), root.join("System").join(name))
            .expect("copy real prototype class packages and configuration");
    }
    std::fs::copy(source.join("Maps/Entry.unr"), root.join("Maps/Entry.unr"))
        .expect("copy prototype Entry map");
    std::fs::write(
        root.join("System/Entry.int"),
        "[LevelInfo0]\nTitle=Rust-owned Entry title\nIdealPlayerCount=Rust-owned ideal count\n",
    )
    .expect("write isolated Entry localization override");
    root
}

fn make_owner(root: &Path) -> EngineOwner {
    EngineOwner::new(CoreOptions {
        data_root: root.to_path_buf(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .expect("initialize canonical Core and Engine.LevelSummary owner")
}

fn utf32(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain([0]).collect()
}
/// # Safety
/// The value must point to initialized FString storage owned by a live object.

unsafe fn read_string(value: *const TArrayRaw) -> Vec<u32> {
    let raw = unsafe { ptr::read_unaligned(value) };
    assert!(raw.num >= 0, "FString count must not be negative");
    if raw.num == 0 {
        return Vec::new();
    }
    assert!(!raw.data.is_null(), "nonempty FString must have data");
    let units = unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) };
    units.iter().copied().take_while(|&unit| unit != 0).collect()
}

fn text(value: &[u32]) -> String {
    value.iter().filter_map(|&unit| char::from_u32(unit)).collect()
}


#[test]
fn prototype_entry_level_summary_localizes_saves_reloads_and_destroys() {
    let root = sandbox();
    let saved = root.join("Maps/Entry.unr");
    let owner = make_owner(&root);
    let access = owner.access();
    access.set_process_mode(hp2_core::core_access::CoreProcessMode {
        is_client: true,
        ..hp2_core::core_access::CoreProcessMode::default()
    });
    let class = access.native_class("Engine.LevelSummary").expect("qualified Engine class lookup");

    // Open metadata with lazy imports: this owner constructs LevelSummary, not
    // the unrelated native world/resource types referenced by the original map.
    let metadata_flags = 1 | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.verify
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.no_verify;
    // LOAD_Propagate is zero in this ABI: open the dependency explicitly in the
    // same metadata mode instead of accidentally requesting a full Engine load.
    unsafe { access.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("open Engine metadata for the selected native class");
    unsafe { access.load_package(&saved, metadata_flags) }
        .expect("open prototype Entry metadata");

    let name = utf32("Entry.LevelSummary");
    let summary = unsafe { access.static_load_object(class, ptr::null_mut(), &name, 1) }
        .expect("load the real prototype Entry LevelSummary through the Rust owner")
        .cast::<ULevelSummary>();
    assert_eq!(text(&unsafe { read_string(ptr::addr_of!((*summary).title)) }), "Rust-owned Entry title");
    assert_eq!(text(&unsafe { read_string(ptr::addr_of!((*summary).ideal_player_count)) }), "Rust-owned ideal count");

    let replacement = utf32("Saved by the Rust LevelSummary owner");
    let author = unsafe { ptr::addr_of_mut!((*summary).author) };
    // SAFETY: Author is an initialized, exclusive FString field in the live
    // canonical LevelSummary record; replacement is terminated and disjoint.
    unsafe { access.assign_fstring(author, &replacement) }
        .expect("change reflected Author FString using Core's Rust allocation policy");

    let index = unsafe { (*summary).base.index };
    unsafe { access.add_root(summary.cast()); }
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect with the summary rooted");
    assert_eq!(access.object_at(index), summary.cast(), "rooted summary retains canonical identity");
    assert_eq!(text(&unsafe { read_string(ptr::addr_of!((*summary).author)) }), "Saved by the Rust LevelSummary owner");

    let package = unsafe { (*summary).base.outer.cast::<UObject>() };
    assert!(unsafe { access.save_package(package, summary.cast(), 0, &saved, None) }
        .expect("save the Rust-owned Entry package"));
    access.remove_root(summary.cast());
    access.collect_garbage(0, 0, &|_| Ok(())).expect("collect the unrooted summary and its strings");
    assert!(access.object_at(index).is_null(), "unrooted summary storage is retired");
    owner.shutdown().expect("destroy first Rust-owned package graph");

    let reloaded = make_owner(&root);
    let access = reloaded.access();
    access.set_process_mode(hp2_core::core_access::CoreProcessMode {
        is_client: true,
        ..hp2_core::core_access::CoreProcessMode::default()
    });
    let class = access.native_class("Engine.LevelSummary").expect("qualified class in reloaded graph");
    unsafe { access.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("open Engine metadata in the fresh owner");
    unsafe { access.load_package(&saved, metadata_flags) }
        .expect("open saved Entry metadata");
    let name = utf32("Entry.LevelSummary");
    let summary = unsafe { access.static_load_object(class, ptr::null_mut(), &name, 1) }
        .expect("reload saved LevelSummary from the Rust-owned package")
        .cast::<ULevelSummary>();
    assert_eq!(text(&unsafe { read_string(ptr::addr_of!((*summary).author)) }), "Saved by the Rust LevelSummary owner");
    assert_eq!(text(&unsafe { read_string(ptr::addr_of!((*summary).title)) }), "Rust-owned Entry title");
    reloaded.shutdown().expect("destroy reloaded Rust-owned package graph");
}
