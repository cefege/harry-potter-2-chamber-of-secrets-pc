use hp2_abi::engine::generated::{FFontCharacter, FFontPage, TArrayRaw, TMapCharChar, TMapCharCharPair, UFont, UObject};
use hp2_core::{core_access::{CoreAccess, CoreOptions, RuntimeError, RuntimeResult}, object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive}, package_archive::ExportArchive};
use hp2_engine::EngineOwner;
use std::{cell::RefCell, mem::size_of, path::PathBuf, ptr};

struct Mapping<'a> { access: CoreAccess<'a>, visited: RefCell<Vec<*mut UObject>> }
impl ArchiveMapping for Mapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 { return Ok(ptr::null_mut()); }
        let object = self.access.object_at(index - 1);
        if object.is_null() { return Err(RuntimeError::new("Unknown canonical archive object")); }
        Ok(object)
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        Ok(if object.is_null() { 0 } else { unsafe { (*object).index + 1 } })
    }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> { self.access.name_text(index)?; Ok(index) }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> { self.access.name_text(index)?; Ok(index) }
    fn visit_object(&self, slot: *mut *mut UObject) -> RuntimeResult<()> { self.visited.borrow_mut().push(unsafe { slot.read_unaligned() }); Ok(()) }
    fn visit_name(&self, index: i32) -> RuntimeResult<()> { self.access.name_text(index)?; Ok(()) }
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
fn text(value: &str) -> Vec<u32> { value.chars().map(u32::from).chain([0]).collect() }
unsafe fn construct(access: CoreAccess<'_>, name: &str) -> *mut UObject {
    unsafe { access.construct_object(access.native_class(name).unwrap(), access.transient_package(), 0, 0, ptr::null_mut()) }.unwrap()
}
unsafe fn set_name(access: CoreAccess<'_>, font: *mut UFont, value: &str) {
    unsafe { access.assign_fstring(ptr::addr_of_mut!((*font).font_name), &text(value)).unwrap(); }
}
unsafe fn name(font: *mut UFont) -> Vec<u32> {
    let value = unsafe { ptr::addr_of!((*font).font_name).read_unaligned() };
    if value.num == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize - 1).to_vec() } }
}
unsafe fn save(access: CoreAccess<'_>, font: *mut UFont, version: u16, mapping: &Mapping<'_>) -> Vec<u8> {
    let archive = ObjectArchive::saving(ArchiveFlags::new(version, 0, false), mapping);
    unsafe { access.serialize_object(font.cast(), &archive).unwrap(); }
    archive.into_written_bytes().unwrap()
}
unsafe fn load(access: CoreAccess<'_>, font: *mut UFont, version: u16, bytes: &[u8], mapping: &Mapping<'_>) {
    let archive = ObjectArchive::loading(ExportArchive::from_slice(bytes), ArchiveFlags::new(version, 0, false), mapping);
    unsafe { access.serialize_object(font.cast(), &archive).unwrap(); }
    assert_eq!(archive.tell().unwrap(), bytes.len());
}

pub fn exercise_font_archives() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("font-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    let mapping = Mapping { access, visited: RefCell::new(Vec::new()) };
    let mut mode = access.process_mode(); mode.lazy_load = false; access.set_process_mode(mode);
    unsafe {
        for version in [68, 69] {
            let source = construct(access, "Engine.Font").cast::<UFont>();
            let copy = construct(access, "Engine.Font").cast::<UFont>();
            set_name(access, source, "Modern Ω");
            (*source).font_height = 21;
            (*source).is_remapped = 1;
            let map = ptr::addr_of_mut!((*source).char_remap).cast::<TMapCharChar>();
            let pairs = libc::calloc(2, size_of::<TMapCharCharPair>()).cast::<TMapCharCharPair>();
            assert!(!pairs.is_null());
            (*pairs).key = 65; (*pairs).value = 66;
            (*pairs.add(1)).key = 73; (*pairs.add(1)).value = 32;
            ptr::addr_of_mut!((*map).pairs).write_unaligned(TArrayRaw { data: pairs.cast(), num: 2, max: 2 });
            (*pairs).hash_next = -1; (*pairs.add(1)).hash_next = 0;
            let hash = ptr::addr_of!((*map).hash).read_unaligned().cast::<i32>();
            hash.add(1).write(1);
            let bytes = save(access, source, version, &mapping);
            // Independent package79 tail: pages/count, optional TCHAR32 map,
            // Unicode FString (-9 compact length), then INT font height.
            let mut expected = vec![0, 0, 0, 0, 0];
            if version >= 69 {
                expected.push(2);
                for value in [65u32, 66, 73, 32, 1] { expected.extend_from_slice(&value.to_le_bytes()); }
            }
            expected.push(0x89);
            for unit in "Modern Ω".encode_utf16().chain([0]) { expected.extend_from_slice(&unit.to_le_bytes()); }
            expected.extend_from_slice(&21i32.to_le_bytes());
            assert!(bytes.ends_with(&expected), "version {version}: canonical Font wire differs from native field order/width");
            load(access, copy, version, &bytes, &mapping);
            assert_eq!(name(copy), "Modern Ω".chars().map(u32::from).collect::<Vec<_>>());
            assert_eq!((*copy).font_height, 21);
            assert_eq!((*copy).is_remapped, i32::from(version >= 69));
            assert!(!access.process_mode().lazy_load, "normal serialization must restore GLazyLoad");
            let copied = ptr::addr_of!((*copy).char_remap).cast::<TMapCharChar>();
            let header = ptr::addr_of!((*copied).pairs).read_unaligned();
            assert_eq!(header.num, if version >= 69 { 2 } else { 0 });
            if version >= 69 {
                let pairs = header.data.cast::<TMapCharCharPair>();
                let hash = ptr::addr_of!((*copied).hash).read_unaligned().cast::<i32>();
                assert_eq!(*hash.add(1), 1, "colliding keys must be rehashed after load");
                assert_eq!((*pairs.add(1)).hash_next, 0);
                assert_eq!((*pairs).hash_next, -1);
            } else { assert_eq!((*source).is_remapped, 0, "old-version saving also clears IsRemapped"); }
        }
        let source = construct(access, "Engine.Font").cast::<UFont>();
        let copy = construct(access, "Engine.Font").cast::<UFont>();
        let texture = construct(access, "Engine.Texture");
        let pages = libc::calloc(1, size_of::<FFontPage>()).cast::<FFontPage>();
        let characters = libc::calloc(256, size_of::<FFontCharacter>()).cast::<FFontCharacter>();
        assert!(!pages.is_null() && !characters.is_null());
        (*pages).texture = texture.cast();
        (*pages).characters = TArrayRaw { data: characters.cast(), num: 256, max: 256 };
        (*characters.add(65)).start_u = 17; (*characters.add(65)).start_v = 23;
        (*characters.add(65)).u_size = 9; (*characters.add(65)).v_size = 18;
        (*source).pages = TArrayRaw { data: pages.cast(), num: 1, max: 1 };
        (*source).characters_per_page = 256;
        set_name(access, source, "bitmap must not serialize native fields");
        set_name(access, copy, "retained native name"); (*copy).font_height = 91;
        let bytes = save(access, source, 69, &mapping);
        load(access, copy, 69, &bytes, &mapping);
        assert_eq!((*copy).characters_per_page, 256);
        let loaded = (*copy).pages.data.cast::<FFontPage>();
        assert_eq!((*loaded).texture, texture.cast());
        let glyph = (*loaded).characters.data.cast::<FFontCharacter>().add(65);
        assert_eq!(((*glyph).start_u, (*glyph).start_v, (*glyph).u_size, (*glyph).v_size), (17, 23, 9, 18));
        assert_eq!(name(copy), "retained native name".chars().map(u32::from).collect::<Vec<_>>());
        assert_eq!((*copy).font_height, 91);
        let visitor = ObjectArchive::reference(ArchiveFlags::new(69, 0, false), &mapping);
        access.serialize_object(copy.cast(), &visitor).unwrap();
        assert!(mapping.visited.borrow().contains(&texture), "bitmap texture must remain visible to Core GC/reference traversal");

        let partial = construct(access, "Engine.Font").cast::<UFont>();
        let archive = ObjectArchive::loading(ExportArchive::from_slice(&bytes[..bytes.len() - 15]), ArchiveFlags::new(69, 0, false), &mapping);
        access.serialize_object(partial.cast(), &archive).expect_err("truncated glyph must stop checked serialization");
        assert!(access.process_mode().lazy_load, "checked page failure preserves source's not-yet-restored global");
        assert_eq!((*partial).pages.num, 1, "partially loaded page is owned and destructible");
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nversion_68_69_native_font_wire=ok\ncharacter_map_collision_rehash=ok\nbitmap_glyphs_and_texture_reference=ok\nnative_font_fallback_guard=ok\nnormal_and_checked_error_lazy_load=ok\npartial_page_shutdown=ok\n").unwrap();
}
#[test]
fn canonical_font_archives_preserve_version_boundaries_bitmap_pages_and_partial_lifetimes() { exercise_font_archives(); }
