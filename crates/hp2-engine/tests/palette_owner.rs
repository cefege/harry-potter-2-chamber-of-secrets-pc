use hp2_abi::engine::generated::{FColor, UObject, UPalette};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_core::package_archive::ExportArchive;
use hp2_engine::EngineOwner;
use std::path::{Path, PathBuf};
use std::{mem::size_of, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
fn root() -> PathBuf {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR names a pre-created test artifact directory"),
    );
    assert!(artifact.is_dir(), "HP2_ARTIFACT_DIR exists");
    let root = artifact.join(format!("palette-owner-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove this test's previous fixture");
    }
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    std::fs::write(root.join("System/Default.ini"), "").expect("write default configuration");
    std::fs::write(root.join("System/DefUser.ini"), "").expect("write user configuration");
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
struct Mapping<'a>(CoreAccess<'a>);
impl ArchiveMapping for Mapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 {
            return Ok(ptr::null_mut());
        }
        let object = self.0.object_at(index - 1);
        if object.is_null() {
            return Err(hp2_core::core_access::RuntimeError::new(
                "invalid fixture object index",
            ));
        }
        Ok(object)
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() {
            Ok(0)
        } else {
            Ok(unsafe { (*object).index + 1 })
        }
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
    fn visit_name(&self, index: i32) -> RuntimeResult<()> {
        self.0.name_text(index).map(|_| ())
    }
}
unsafe fn set_colors(palette: *mut UPalette, colors: &[FColor]) {
    let raw = ptr::addr_of_mut!((*palette).colors);
    let slots = Hp2ArraySlots {
        data: ptr::addr_of_mut!((*raw).data),
        num: ptr::addr_of_mut!((*raw).num),
        max: ptr::addr_of_mut!((*raw).max),
    };
    let count = i32::try_from(colors.len()).expect("color count fits native INT");
    assert_eq!(
        unsafe { slots.num.read_unaligned() },
        0,
        "fresh or emptied palette has no live colors"
    );
    if count == 0 {
        return;
    }
    unsafe {
        slots.max.write(count);
    }
    unsafe {
        array_storage::realloc(
            slots,
            size_of::<FColor>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .expect("allocate colors using Core's paired array allocator");
    let data = unsafe { slots.data.read_unaligned().cast::<FColor>() };
    for (index, color) in colors.iter().copied().enumerate() {
        unsafe {
            data.add(index).write(color);
        }
    }
    unsafe {
        slots.num.write(count);
    }
}
unsafe fn colors(palette: *mut UPalette) -> Vec<[u8; 4]> {
    let raw = unsafe { ptr::read_unaligned(ptr::addr_of!((*palette).colors)) };
    assert!(raw.num >= 0, "color array count is nonnegative");
    if raw.num == 0 {
        return Vec::new();
    }
    assert!(!raw.data.is_null(), "nonempty color array owns storage");
    unsafe { std::slice::from_raw_parts(raw.data.cast::<FColor>(), raw.num as usize) }
        .iter()
        .map(|c| [c.r, c.g, c.b, c.a])
        .collect()
}

#[test]
fn canonical_palette_rgba_transaction_roundtrip_and_truncated_load_cleanup() {
    let root = root();
    let owner = EngineOwner::new(options(&root)).expect("initialize canonical engine owner");
    let access = owner.access();
    let mapping = Mapping(access);
    let package_name = access
        .intern_str("PaletteFixture", FindName::Add)
        .expect("intern fixture package name");
    let package = unsafe {
        access.construct_object(
            access.native_class("Package").expect("Core.Package"),
            ptr::null_mut(),
            package_name,
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct canonical package");
    let class = access
        .native_class("Engine.Palette")
        .expect("canonical Engine.Palette");
    let palette_name = access
        .intern_str("CanonicalPalette", FindName::Add)
        .expect("intern palette name");
    let palette =
        unsafe { access.construct_object(class, package, palette_name, 4, ptr::null_mut()) }
            .expect("construct canonical palette")
            .cast::<UPalette>();
    let expected = [
        FColor {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0x78,
        },
        FColor {
            r: 0x9a,
            g: 0xbc,
            b: 0xde,
            a: 0xf0,
        },
        FColor {
            r: 0x01,
            g: 0x23,
            b: 0x45,
            a: 0x67,
        },
    ];
    unsafe {
        set_colors(palette, &expected);
    }
    let saved = ObjectArchive::transaction_saving(ArchiveFlags::new(79, 0, false), &mapping);
    unsafe { access.serialize_object(palette.cast(), &saved) }
        .expect("save canonical palette transaction");
    let wire = saved
        .into_written_bytes()
        .expect("collect palette transaction bytes");
    assert!(
        wire.ends_with(&[
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x01, 0x23, 0x45, 0x67
        ]),
        "palette wire data ends with native R,G,B,A channels, not host packed-color order"
    );
    // A transaction restores this instance, including its Name/Outer identity.
    // Loading it into a differently named object would corrupt that object's hash.
    unsafe {
        (*palette).colors.data.cast::<FColor>().write(FColor { r: 7, g: 8, b: 9, a: 10 });
    }
    let undo = ObjectArchive::transaction_loading(
        ExportArchive::from_slice(&wire),
        ArchiveFlags::new(79, 0, false),
        &mapping,
    );
    unsafe { access.serialize_object(palette.cast(), &undo) }
        .expect("restore canonical palette transaction");
    assert_eq!(
        unsafe { colors(palette) },
        expected.map(|c| [c.r, c.g, c.b, c.a]),
        "transaction restores distinct RGBA channels"
    );

    let truncated = &wire[..wire.len() - 2];
    let loading = ObjectArchive::transaction_loading(
        ExportArchive::from_slice(truncated),
        ArchiveFlags::new(79, 0, false),
        &mapping,
    );
    assert!(
        unsafe { access.serialize_object(palette.cast(), &loading) }.is_err(),
        "truncated color bytes are rejected"
    );
    let recovered = ObjectArchive::transaction_loading(
        ExportArchive::from_slice(&wire),
        ArchiveFlags::new(79, 0, false),
        &mapping,
    );
    unsafe { access.serialize_object(palette.cast(), &recovered) }
        .expect("restore intact transaction after partial color load");
    assert_eq!(
        unsafe { colors(palette) },
        expected.map(|c| [c.r, c.g, c.b, c.a]),
        "a truncated color load does not poison subsequent restoration"
    );
    for version in [65, 66] {
        let legacy = ObjectArchive::transaction_loading(
            ExportArchive::from_slice(&wire),
            ArchiveFlags::new(version, 0, false),
            &mapping,
        );
        unsafe { access.serialize_object(palette.cast(), &legacy) }
            .expect("restore palette at the legacy alpha boundary");
        assert_eq!(
            unsafe { colors(palette) },
            expected.map(|c| [c.r, c.g, c.b, if version < 66 { 255 } else { c.a }]),
            "versions before 66 force opaque alpha; version 66 preserves stored alpha"
        );
    }
    let legacy_save = ObjectArchive::transaction_saving(ArchiveFlags::new(65, 0, false), &mapping);
    unsafe { access.serialize_object(palette.cast(), &legacy_save) }
        .expect("save palette before the legacy alpha fixup");
    let legacy_wire = legacy_save.into_written_bytes().expect("collect legacy palette bytes");
    assert!(
        legacy_wire.ends_with(&[
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x01, 0x23, 0x45, 0x67
        ]),
        "legacy fixup occurs after serializing the original RGBA channels"
    );
    assert_eq!(
        unsafe { colors(palette) },
        expected.map(|c| [c.r, c.g, c.b, 255]),
        "legacy saving also makes the live palette opaque"
    );
    owner
        .shutdown()
        .expect("destroy palette buffers and their canonical owner");
}
