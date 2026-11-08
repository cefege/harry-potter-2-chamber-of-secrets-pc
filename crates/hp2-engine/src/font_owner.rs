//! Canonical `Engine.Font` storage, archive leaves, and bitmap-page references.

use crate::{animation_owner, font::{self, FontArchiveOps}, texture_owner};
use hp2_abi::engine::generated::{self, FFontCharacter, FFontPage, TMapCharChar, TMapCharCharPair, UFont, UObject, UTexture};
use hp2_core::{array_storage, map_storage::{self, Hp2MapServices, Hp2MapSlots}};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_archive::{ArchiveMode, ObjectArchive};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const FONT: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct;
    operations.finalize_members = finalize_members;
    operations.serialize = serialize;
    operations.visit_references = visit;
    operations
};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Font", package: "Engine", super_name: Some("Core.Object"),
    within: "Object", flags: 0,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    // SAFETY: both size and alignment are generated from the same native class.
    layout: Some(unsafe { Layout::from_size_align_unchecked(size_of::<UFont>(), UFont::NATIVE_ALIGNMENT) }),
    operations: Some(FONT), properties: &[], defaults: &[],
}];

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.construct)(access, object)?;
        let font = object.cast::<UFont>();
        ptr::addr_of_mut!((*font).pages).write_unaligned(std::mem::zeroed());
        ptr::addr_of_mut!((*font).font_name).write_unaligned(std::mem::zeroed());
        ptr::addr_of_mut!((*font).native_font).write_unaligned(ptr::null_mut());
        map_storage::initialize_owned_pod(ptr::addr_of_mut!((*font).char_remap).cast())
    }
}

unsafe fn destroy_page(value: *mut c_void) -> RuntimeResult<()> {
    let page = value.cast::<FFontPage>();
    unsafe { animation_owner::destroy_array(ptr::addr_of_mut!((*page).characters), size_of::<FFontCharacter>(), None) }
}

unsafe fn finalize_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let font = object.cast::<UFont>();
    let pages = unsafe { animation_owner::destroy_array(ptr::addr_of_mut!((*font).pages), size_of::<FFontPage>(), Some(destroy_page)) };
    unsafe { map_storage::destroy_owned_pod(ptr::addr_of_mut!((*font).char_remap).cast()); }
    let name = unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*font).font_name)) };
    // NativeFont is a borrowed OS handle in UFont; the native class has no handle destructor.
    pages.and(name)
}

unsafe fn serialize(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    if matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
        return unsafe { visit(access, object, archive) };
    }
    font::serialize_with(&mut FontArchive { access, font: object.cast(), archive })
}

struct FontArchive<'a, 'archive, 'data> {
    access: CoreAccess<'a>,
    font: *mut UFont,
    archive: &'archive ObjectArchive<'data>,
}

impl FontArchiveOps for FontArchive<'_, '_, '_> {
    type Error = RuntimeError;
    type Texture = *mut UTexture;

    fn super_serialize(&mut self) -> RuntimeResult<()> {
        unsafe { (OBJECT.serialize)(self.access, self.font.cast(), self.archive) }
    }
    fn lazy_load(&self) -> i32 { i32::from(self.access.process_mode().lazy_load) }
    fn set_lazy_load(&mut self, value: i32) {
        let mut mode = self.access.process_mode();
        mode.lazy_load = value != 0;
        self.access.set_process_mode(mode);
    }
    fn serialize_pages(&mut self) -> RuntimeResult<()> {
        unsafe {
            animation_owner::serialize_array(self.archive, ptr::addr_of_mut!((*self.font).pages), size_of::<FFontPage>(), Some(serialize_page), Some(destroy_page), false)?;
            self.archive.serialize_i32(&mut *ptr::addr_of_mut!((*self.font).characters_per_page))
        }
    }
    fn check_characters_per_page(&self) -> RuntimeResult<()> {
        let count = self.characters_per_page();
        if generated::CORE_LINKER_CONSTANTS.checks != 0 && count & count.wrapping_sub(1) != 0 {
            return Err(RuntimeError::new("Font CharactersPerPage is not a power of two"));
        }
        Ok(())
    }
    fn pages_num(&self) -> i32 { unsafe { ptr::addr_of!((*self.font).pages.num).read_unaligned() } }
    fn characters_per_page(&self) -> i32 { unsafe { ptr::addr_of!((*self.font).characters_per_page).read_unaligned() } }
    fn page_texture(&self, index: i32) -> RuntimeResult<Option<*mut UTexture>> {
        let page = unsafe { page_at(self.font, index)? };
        let texture = unsafe { ptr::addr_of!((*page).texture).read_unaligned().cast::<UTexture>() };
        Ok((!texture.is_null()).then_some(texture))
    }
    fn texture_mips_num(&self, texture: *mut UTexture) -> RuntimeResult<i32> {
        Ok(unsafe { ptr::addr_of!((*texture).mips.num).read_unaligned() })
    }
    fn texture_mip_load(&mut self, texture: *mut UTexture, index: i32) -> RuntimeResult<()> {
        unsafe { texture_owner::load_mip(self.access, texture, index) }
    }
    fn archive_version(&self) -> i32 { i32::from(self.archive.flags().version) }
    fn serialize_char_remap(&mut self) -> RuntimeResult<()> {
        let map = unsafe { ptr::addr_of_mut!((*self.font).char_remap).cast::<TMapCharChar>() };
        unsafe {
            animation_owner::serialize_array(self.archive, ptr::addr_of_mut!((*map).pairs), size_of::<TMapCharCharPair>(), Some(serialize_remap_pair), None, false)?;
            if self.archive.is_loading() { rehash_remap(map)?; }
            self.archive.serialize_i32(&mut *ptr::addr_of_mut!((*self.font).is_remapped))
        }
    }
    fn set_is_remapped(&mut self, value: i32) { unsafe { ptr::addr_of_mut!((*self.font).is_remapped).write_unaligned(value); } }
    fn serialize_native_fields(&mut self) -> RuntimeResult<()> {
        unsafe {
            self.access.serialize_owned_fstring(self.archive, ptr::addr_of_mut!((*self.font).font_name))?;
            self.archive.serialize_i32(&mut *ptr::addr_of_mut!((*self.font).font_height))
        }
    }
}

unsafe fn page_at(font: *mut UFont, index: i32) -> RuntimeResult<*mut FFontPage> {
    let pages = unsafe { ptr::addr_of!((*font).pages).read_unaligned() };
    if index < 0 || index >= pages.num || pages.data.is_null() {
        return Err(RuntimeError::new("Font page index out of range"));
    }
    Ok(unsafe { pages.data.cast::<FFontPage>().add(index as usize) })
}

unsafe fn serialize_character(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let value = value.cast::<FFontCharacter>();
    unsafe {
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).start_u))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).start_v))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).u_size))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).v_size))
    }
}

unsafe fn serialize_page(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let page = value.cast::<FFontPage>();
    unsafe {
        archive.serialize_object_slot(ptr::addr_of_mut!((*page).texture).cast())?;
        animation_owner::serialize_array(archive, ptr::addr_of_mut!((*page).characters), size_of::<FFontCharacter>(), Some(serialize_character), None, false)
    }
}

unsafe fn serialize_remap_pair(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let pair = value.cast::<TMapCharCharPair>();
    unsafe {
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*pair).key))?;
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*pair).value))
    }
}

unsafe extern "C" fn remap_leaf(context: *mut c_void, operation: i32, index: i32, argument: i32, output: *mut i32) -> i32 {
    let map = context.cast::<TMapCharChar>();
    let pairs = unsafe { ptr::addr_of!((*map).pairs).read_unaligned() };
    let value = if operation == map_storage::leaf::NUM { pairs.num } else {
        if index < 0 || index >= pairs.num || pairs.data.is_null() { return 1; }
        let pair = unsafe { pairs.data.cast::<TMapCharCharPair>().add(index as usize) };
        match operation {
            map_storage::leaf::HASH => unsafe { ptr::addr_of!((*pair).key).read_unaligned() as i32 },
            map_storage::leaf::LINK => { unsafe { ptr::addr_of_mut!((*pair).hash_next).write_unaligned(argument); } 0 },
            _ => return 1,
        }
    };
    unsafe { output.write_unaligned(value); }
    0
}

unsafe extern "C" fn unsupported_query(_: *mut c_void, _: i32, _: i32, _: i32, _: *mut i32) -> i32 { 1 }

unsafe fn rehash_remap(map: *mut TMapCharChar) -> RuntimeResult<()> {
    let slots = unsafe { Hp2MapSlots { hash: ptr::addr_of_mut!((*map).hash).cast(), count: ptr::addr_of_mut!((*map).hash_count) } };
    let services = Hp2MapServices { context: map.cast(), leaf: remap_leaf, query: unsupported_query, allocator: array_storage::system_allocator(), tag: ptr::null() };
    unsafe { map_storage::rehash(slots, services) }.map_err(|_| RuntimeError::new("Unable to rehash Font CharRemap"))
}

unsafe fn visit(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.visit_references)(access, object, archive)?; }
    let font = object.cast::<UFont>();
    let count = unsafe { ptr::addr_of!((*font).pages.num).read_unaligned() };
    for index in 0..count {
        let page = unsafe { page_at(font, index)? };
        unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*page).texture).cast())?; }
    }
    Ok(())
}
