//! Real prototype Engine.Bitmap.USize metadata in one canonical EngineOwner.
//! This test does not explicitly read an instance property, execute a fixture VM body, or draw.

use hp2_abi::engine::generated::{
    CORE_LINKER_CONSTANTS, FObjectExport, UBitmap, ULinkerLoad, UObject, UProperty, UStruct,
};
use hp2_core::{
    core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult},
    name_owner::FindName,
    object_lifecycle::{RF_DEBUG_DESTROY, RF_DESTROYED, RF_UNREACHABLE},
    sha256::{self, Sha256Context},
};
use hp2_engine::EngineOwner;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    mem::{MaybeUninit, align_of, offset_of},
    path::{Path, PathBuf},
    ptr,
};

const CORE_SHA256: &str = "e0812a1931322d99d8b85db2f42fb2dacc8df33e919ea566a64e7a834e0d56e8";
const ENGINE_SHA256: &str = "c75c3e849bd07a3a315d3e87f24bb5d8960b5ce32f8fc1438d2c1c3906f93182";
const LOAD_NO_FAIL: u32 = 1;

fn units(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn output(access: CoreAccess<'_>, category: i32, text: &[u32]) -> RuntimeResult<()> {
    access.write_log_record(&access.name_text(category)?, text)
}

fn append(path: &Path, message: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().append(true).open(path)?;
    writeln!(file, "{message}")
}

// A cleanup reporting failure must not replace an earlier panic or shutdown error.
fn append_secondary(path: &Path, message: &str) {
    if let Err(error) = append(path, message) {
        let _ = writeln!(io::stderr().lock(), "secondary journal error: {error}; {message}");
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    // SAFETY: Sha256Context contains only integers and integer arrays; zero is
    // a valid initialized value for every field before the init call.
    let mut context = unsafe { MaybeUninit::<Sha256Context>::zeroed().assume_init() };
    // SAFETY: context is initialized writable storage, and every chunk is live,
    // disjoint from it, and at most 65536 bytes.
    unsafe {
        sha256::init(&mut context);
        for chunk in bytes.chunks(65536) {
            sha256::update(&mut context, chunk.as_ptr(), chunk.len() as u32);
        }
    }
    let mut digest = [0u8; 32];
    // SAFETY: digest contains 32 writable bytes and context remains live.
    unsafe { sha256::final_digest(digest.as_mut_ptr(), &mut context) };
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn copy_pinned(source: &Path, destination: &Path, expected: &str, journal: &Path) {
    let bytes = fs::read(source).unwrap_or_else(|error| {
        panic!("read prototype package {}: {error}", source.display())
    });
    let actual = sha256_hex(&bytes);
    append(
        journal,
        &format!(
            "source={} bytes={} sha256={} expected={}",
            source.display(),
            bytes.len(),
            actual,
            expected
        ),
    )
    .expect("record source package provenance");
    assert_eq!(actual, expected, "prototype package identity: {}", source.display());

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap_or_else(|error| panic!("create package copy {}: {error}", destination.display()));
    file.write_all(&bytes).expect("write package copy");
    file.flush().expect("flush package copy");
    drop(file);
    let copied = fs::read(destination).expect("read back package copy");
    let copy_hash = sha256_hex(&copied);
    append(
        journal,
        &format!("copy={} bytes={} sha256={copy_hash}", destination.display(), copied.len()),
    )
    .expect("record copied package provenance");
    assert_eq!(copy_hash, expected, "copied package identity: {}", destination.display());
}

struct OwnerGuard {
    owner: Option<EngineOwner>,
    journal: PathBuf,
}

impl OwnerGuard {
    fn access(&self) -> CoreAccess<'_> {
        self.owner.as_ref().expect("EngineOwner remains live").access()
    }

    fn finish(mut self) {
        let owner = self.owner.take().expect("EngineOwner remains live for shutdown");
        match owner.shutdown() {
            Ok(()) => append(&self.journal, "owner_shutdown=ok")
                .expect("record successful owner shutdown"),
            Err(error) => {
                append_secondary(&self.journal, &format!("owner_shutdown=error {error}"));
                panic!("EngineOwner shutdown failed: {error}");
            }
        }
    }
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            let message = match owner.shutdown() {
                Ok(()) => "owner_shutdown_during_unwind=ok".to_owned(),
                Err(error) => format!("owner_shutdown_during_unwind=error {error}"),
            };
            append_secondary(&self.journal, &message);
        }
    }
}

// SAFETY: object comes from this owner's completed canonical load or native
// class lookup; no GC, loader reset, or teardown occurs during this test.
unsafe fn checked_index(access: CoreAccess<'_>, object: *mut UObject, label: &str) -> i32 {
    assert!(!object.is_null(), "{label} is non-null");
    let index = unsafe { ptr::addr_of!((*object).index).read_unaligned() };
    assert!(index >= 0 && index < access.object_count(), "{label} index {index} is in range");
    assert_eq!(access.object_at(index), object, "{label} is in this owner's table");
    index
}

// SAFETY: object is a live canonical UObject from this owner's completed load.
unsafe fn assert_loaded(object: *mut UObject, label: &str) {
    let flags = unsafe { ptr::addr_of!((*object).object_flags).read_unaligned() };
    let invalid = CORE_LINKER_CONSTANTS.need_load
        | CORE_LINKER_CONSTANTS.preloading
        | CORE_LINKER_CONSTANTS.need_post_load
        | RF_UNREACHABLE
        | RF_DESTROYED
        | RF_DEBUG_DESTROY;
    assert_eq!(flags & invalid, 0, "{label} has load/destroy flags {flags:#x}");
}

#[test]
#[ignore = "requires isolated HP2_ARTIFACT_DIR and prototype HP2_CORE_DATA_ROOT"]
fn prototype_engine_bitmap_usize_descriptor_contract() {
    let artifacts = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR is required"),
    );
    assert!(artifacts.is_dir(), "artifact parent exists: {}", artifacts.display());
    let data = fs::canonicalize(
        PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype data root is required")),
    )
    .expect("resolve prototype data root");
    let root = artifacts.join(format!("bitmap-usize-metadata-{}", std::process::id()));
    fs::create_dir(&root).expect("create a fresh artifact root without deleting any existing data");
    let system = root.join("System");
    fs::create_dir(&system).expect("create isolated System");
    let journal = root.join("contract.txt");
    OpenOptions::new().write(true).create_new(true).open(&journal)
        .expect("create contract journal");
    append(&journal, &format!("profile=data-prototype source_root={}", data.display()))
        .expect("record prototype root");
    for (name, hash) in [("Core.u", CORE_SHA256), ("Engine.u", ENGINE_SHA256)] {
        copy_pinned(&data.join("System").join(name), &system.join(name), hash, &journal);
    }
    fs::write(system.join("Default.ini"), "[Core.System]\nPaths=../System/*.u\n")
        .expect("write isolated system configuration");
    fs::write(system.join("DefUser.ini"), "").expect("write isolated user configuration");
    let mut log = OpenOptions::new().write(true).create_new(true).open(root.join("runtime.log"))
        .expect("create isolated runtime log");
    log.write_all(&[0xff, 0xfe]).expect("write UTF-16LE runtime log marker");
    log.flush().expect("flush runtime log marker");

    append(&journal, "creating=EngineOwner").expect("record owner construction");
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: system.join("Default.ini"),
        user_ini: system.join("DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: Some(log),
    })
    .expect("construct EngineOwner with the copied prototype packages");
    let guard = OwnerGuard { owner: Some(owner), journal: journal.clone() };
    let access = guard.access();
    access.set_process_mode(CoreProcessMode { is_client: true, ..CoreProcessMode::default() });
    append(&journal, "acquiring=System/Core.u flags=LOAD_NoFail parent=null guid=none")
        .expect("record Core linker acquisition");
    // SAFETY: bootstrap is complete, the parent is null, and the engine-thread
    // owner has compatible declarations for every class the linker constructs.
    let core_linker = unsafe {
        access.get_package_linker(&system.join("Core.u"), LOAD_NO_FAIL, ptr::null_mut(), None)
    }
    .expect("acquire prototype Core.u linker in this owner");
    let core_package = access.core_package();
    // SAFETY: the linker and Core package are live canonical records in this owner.
    let (core_linker_index, core_package_index) = unsafe {
        (
            checked_index(access, core_linker, "Core linker"),
            checked_index(access, core_package, "Core package"),
        )
    };
    // SAFETY: the returned linker has the canonical ULinkerLoad prefix.
    let core_root = unsafe {
        ptr::addr_of!((*core_linker.cast::<ULinkerLoad>()).base.linker_root).read_unaligned()
    };
    assert_eq!(core_root, core_package.cast(), "Core.u binds this owner's Core package");
    append(&journal, &format!("core_linker_index={core_linker_index} core_package_index={core_package_index}"))
        .expect("record Core linker identity");
    append(&journal, "loading=System/Engine.u flags=LOAD_NoFail").expect("record Engine package load");
    // SAFETY: owner bootstrap is complete. This uses the existing prototype
    // package-loading pattern and the engine-thread capability.
    let linker = unsafe { access.load_package(&system.join("Engine.u"), LOAD_NO_FAIL) }
        .expect("load prototype Engine.u into this owner");
    // SAFETY: the loader returned a live canonical linker in this owner.
    let linker_index = unsafe { checked_index(access, linker, "Engine linker") };
    let base = access.native_class("Core.Object").expect("Core.Object class");
    // SAFETY: base belongs to this owner; the name resolves through its loaded linker.
    let bitmap = unsafe {
        access.static_load_class(base, ptr::null_mut(), &units("Engine.Bitmap"), LOAD_NO_FAIL)
    }
    .expect("materialize prototype Engine.Bitmap class");
    assert!(!bitmap.is_null(), "Engine.Bitmap class is present");
    assert_eq!(bitmap, access.native_class("Engine.Bitmap").expect("native Bitmap identity"));
    let int_class = access.native_class("Core.IntProperty").expect("Core.IntProperty class");
    // SAFETY: the classes were returned by this same live owner.
    let bitmap_index = unsafe { checked_index(access, bitmap.cast(), "Bitmap class") };
    let int_class_index = unsafe { checked_index(access, int_class.cast(), "IntProperty class") };
    // SAFETY: the name lookup only searches the owner's already-loaded objects.
    let object = unsafe { access.find_object_by_name(ptr::null_mut(), &units("Engine.Bitmap.USize")) }
        .expect("look up Engine.Bitmap.USize")
        .expect("Engine.Bitmap.USize descriptor must exist");
    // SAFETY: the successful lookup returned an object in this live owner.
    let index = unsafe { checked_index(access, object, "USize descriptor") };
    // SAFETY: these are UObject prefix fields in the returned live records.
    let (actual_class, actual_outer, actual_linker, export_row) = unsafe {
        (
            ptr::addr_of!((*object).class).read_unaligned(),
            ptr::addr_of!((*object).outer).read_unaligned(),
            ptr::addr_of!((*object)._linker).read_unaligned(),
            ptr::addr_of!((*object)._linker_index).read_unaligned(),
        )
    };
    append(
        &journal,
        &format!(
            "linker_index={linker_index} bitmap_index={bitmap_index} int_class_index={int_class_index} descriptor_index={index} export_row={export_row} class={actual_class:p} outer={actual_outer:p} linker={actual_linker:p}"
        ),
    )
    .expect("record descriptor identity before assertions");
    // SAFETY: both records were returned by the completed owner load.
    unsafe {
        assert_loaded(bitmap.cast(), "Bitmap class");
        assert_loaded(object, "USize descriptor");
    }
    assert_eq!(actual_class, int_class.cast(), "USize has exact Core.IntProperty class");
    assert_eq!(actual_outer, bitmap.cast(), "USize outer is the loaded Bitmap class");
    assert_eq!(actual_linker, linker.cast(), "USize belongs to this Engine.u linker");
    // SAFETY: the owner supplies the live linker prefix and Bitmap UObject prefix.
    let (package, bitmap_package, bitmap_linker) = unsafe {
        (
            ptr::addr_of!((*linker.cast::<ULinkerLoad>()).base.linker_root).read_unaligned(),
            ptr::addr_of!((*bitmap.cast::<UObject>()).outer).read_unaligned(),
            ptr::addr_of!((*bitmap.cast::<UObject>())._linker).read_unaligned(),
        )
    };
    assert!(!package.is_null(), "Engine.u linker has a package root");
    assert_eq!(bitmap_package, package, "Bitmap belongs to the Engine package");
    assert_eq!(bitmap_linker, linker.cast(), "Bitmap is bound to the same Engine.u linker");
    // SAFETY: class, property, package, and outer chain are live in this owner.
    let path = unsafe { access.object_path_text(object) }.expect("read property path");
    assert_eq!(path, units("Engine.Bitmap.USize"), "exact property path");
    // SAFETY: the class-filtered lookup only reads the same owner's object table.
    let typed = unsafe { access.find_object_by_name(int_class, &units("Engine.Bitmap.USize")) }
        .expect("look up typed USize descriptor");
    assert_eq!(typed, Some(object), "typed and untyped lookups identify the same descriptor");
    let name = access.intern_str("USize", FindName::Find).expect("find loaded USize name");
    assert_ne!(name, 0, "USize name is present");
    // SAFETY: Bitmap and its already-linked field chain remain live in this owner.
    let field = unsafe { access.find_field(bitmap.cast::<UStruct>(), name, int_class) };
    assert_eq!(field.cast::<UObject>(), object, "Bitmap field chain contains this descriptor");

    // SAFETY: class equality above establishes the UProperty-family allocation.
    let property = object.cast::<UProperty>();
    let (dim, element_size, offset) = unsafe {
        (
            ptr::addr_of!((*property).array_dim).read_unaligned(),
            ptr::addr_of!((*property).element_size).read_unaligned(),
            ptr::addr_of!((*property).offset).read_unaligned(),
        )
    };
    // SAFETY: Bitmap is the same owner's live UClass record.
    let bitmap_size = unsafe { ptr::addr_of!((*bitmap).properties_size).read_unaligned() };
    append(
        &journal,
        &format!("array_dim={dim} element_size={element_size} offset={offset} bitmap_properties_size={bitmap_size}"),
    )
    .expect("record live descriptor ABI before assertions");
    let native_offset = i32::try_from(offset_of!(UBitmap, u_size)).expect("native offset fits i32");
    assert_eq!(native_offset, 88, "compiled UBitmap member offset");
    assert_eq!(dim, 1, "USize is scalar");
    assert_eq!(element_size, 4, "USize element is four bytes");
    assert_eq!(offset, native_offset, "USize reflection agrees with the native member");
    assert_eq!(offset as usize % align_of::<i32>(), 0, "USize offset is i32-aligned");
    let end = offset.checked_add(element_size).expect("USize extent does not overflow");
    assert!(end <= bitmap_size, "USize ends at {end}, Bitmap properties size is {bitmap_size}");

    // SAFETY: load_package returned this live ULinkerLoad and its exported array.
    let exports = unsafe { ptr::addr_of!((*linker.cast::<ULinkerLoad>()).base.export_map).read_unaligned() };
    assert!(exports.num > 0 && exports.max >= exports.num && !exports.data.is_null(), "Engine export map is valid");
    assert!(export_row >= 0 && export_row < exports.num, "USize export row {export_row} is in range");
    // SAFETY: the checked row indexes the live export map owned by this linker.
    let export = unsafe { ptr::read_unaligned(exports.data.cast::<FObjectExport>().add(export_row as usize)) };
    assert_eq!(export._object, object.cast(), "Engine export points to the descriptor");
    assert_eq!(i32::from_ne_bytes(export.object_name), name, "Engine export has the USize name");
    append(&journal, "metadata_assertions=ok").expect("record successful metadata assertions");
    guard.finish();
}
