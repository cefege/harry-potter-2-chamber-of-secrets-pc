//! `UFont::Serialize` from `Engine/Src/UnFont.cpp`.
//!
//! Native and canonical Core objects share the same archive state machine;
//! adapters supply only their object, archive, and lazy-load operations.

use std::ffi::c_void;

/// C++ object/archive operations required by `UFont::Serialize`.
#[repr(C)]
pub struct Hp2FontServices {
    /// `UObject::Serialize` on this font.
    pub font_super_serialize: unsafe extern "C" fn(font: *mut c_void, ar: *mut c_void) -> i32,
    /// Read `GLazyLoad`.
    pub lazy_load_get: unsafe extern "C" fn() -> i32,
    /// Write `GLazyLoad`.
    pub lazy_load_set: unsafe extern "C" fn(value: i32),
    /// `Ar << Pages << CharactersPerPage`.
    pub font_serialize_pages: unsafe extern "C" fn(font: *mut c_void, ar: *mut c_void) -> i32,
    /// `check(!(CharactersPerPage & (CharactersPerPage - 1)))`.
    pub font_check_characters_per_page: unsafe extern "C" fn(font: *mut c_void) -> i32,
    /// `Pages.Num()`.
    pub font_pages_num: unsafe extern "C" fn(font: *const c_void) -> i32,
    /// `CharactersPerPage`.
    pub font_characters_per_page: unsafe extern "C" fn(font: *const c_void) -> i32,
    /// `Pages(index).Texture`.
    pub font_page_texture: unsafe extern "C" fn(font: *const c_void, index: i32) -> *mut c_void,
    /// `Texture->Mips.Num()`.
    pub texture_mips_num: unsafe extern "C" fn(texture: *const c_void) -> i32,
    /// `Texture->Mips(index).DataArray.Load()`.
    pub texture_mip_load: unsafe extern "C" fn(texture: *mut c_void, index: i32) -> i32,
    pub archive_version: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar << CharRemap << IsRemapped`.
    pub font_serialize_char_remap: unsafe extern "C" fn(font: *mut c_void, ar: *mut c_void) -> i32,
    /// Set `IsRemapped`.
    pub font_set_is_remapped: unsafe extern "C" fn(font: *mut c_void, value: i32),
    /// `Ar << FontName << FontHeight`.
    pub font_serialize_native_fields:
        unsafe extern "C" fn(font: *mut c_void, ar: *mut c_void) -> i32,
}

/// Object/archive leaves for the source-shared font serializer.
pub trait FontArchiveOps {
    type Error;
    type Texture: Copy;

    fn super_serialize(&mut self) -> Result<(), Self::Error>;
    fn lazy_load(&self) -> i32;
    fn set_lazy_load(&mut self, value: i32);
    fn serialize_pages(&mut self) -> Result<(), Self::Error>;
    fn check_characters_per_page(&self) -> Result<(), Self::Error>;
    fn pages_num(&self) -> i32;
    fn characters_per_page(&self) -> i32;
    fn page_texture(&self, index: i32) -> Result<Option<Self::Texture>, Self::Error>;
    fn texture_mips_num(&self, texture: Self::Texture) -> Result<i32, Self::Error>;
    fn texture_mip_load(&mut self, texture: Self::Texture, index: i32) -> Result<(), Self::Error>;
    fn archive_version(&self) -> i32;
    fn serialize_char_remap(&mut self) -> Result<(), Self::Error>;
    fn set_is_remapped(&mut self, value: i32);
    fn serialize_native_fields(&mut self) -> Result<(), Self::Error>;
}

/// Runs `UFont::Serialize`, including its live `GLazyLoad` test and non-RAII
/// restoration: an error before restoration deliberately leaves it changed.
///
/// # Errors
/// Returns the first object/archive failure without running subsequent leaves.
pub fn serialize_with<O: FontArchiveOps>(ops: &mut O) -> Result<(), O::Error> {
    ops.super_serialize()?;
    let saved_lazy_load = ops.lazy_load();
    ops.set_lazy_load(1);
    ops.serialize_pages()?;
    ops.check_characters_per_page()?;

    // The source tests the live global, not its saved value. Reentrant archive
    // work can change it while loading the page textures.
    if ops.lazy_load() == 0 {
        let characters_per_page = ops.characters_per_page();
        let mut character = 0i32;
        let mut page = 0i32;
        while character < 256 {
            if page < ops.pages_num() {
                if let Some(texture) = ops.page_texture(page)? {
                    let mip_count = ops.texture_mips_num(texture)?;
                    for mip in 0..mip_count {
                        ops.texture_mip_load(texture, mip)?;
                    }
                }
            }
            character = character.wrapping_add(characters_per_page);
            page = page.wrapping_add(1);
        }
    }
    ops.set_lazy_load(saved_lazy_load);
    if ops.archive_version() >= 69 {
        ops.serialize_char_remap()?;
    } else {
        ops.set_is_remapped(0);
    }
    if ops.pages_num() == 0 && ops.characters_per_page() == 0 {
        ops.serialize_native_fields()?;
    }
    Ok(())
}

struct NativeFont<'a> {
    font: *mut c_void,
    archive: *mut c_void,
    services: &'a Hp2FontServices,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

impl FontArchiveOps for NativeFont<'_> {
    type Error = i32;
    type Texture = *mut c_void;
    fn super_serialize(&mut self) -> Result<(), i32> {
        status(unsafe { (self.services.font_super_serialize)(self.font, self.archive) })
    }
    fn lazy_load(&self) -> i32 { unsafe { (self.services.lazy_load_get)() } }
    fn set_lazy_load(&mut self, value: i32) { unsafe { (self.services.lazy_load_set)(value); } }
    fn serialize_pages(&mut self) -> Result<(), i32> {
        status(unsafe { (self.services.font_serialize_pages)(self.font, self.archive) })
    }
    fn check_characters_per_page(&self) -> Result<(), i32> {
        status(unsafe { (self.services.font_check_characters_per_page)(self.font) })
    }
    fn pages_num(&self) -> i32 { unsafe { (self.services.font_pages_num)(self.font) } }
    fn characters_per_page(&self) -> i32 { unsafe { (self.services.font_characters_per_page)(self.font) } }
    fn page_texture(&self, index: i32) -> Result<Option<Self::Texture>, i32> {
        let texture = unsafe { (self.services.font_page_texture)(self.font, index) };
        Ok((!texture.is_null()).then_some(texture))
    }
    fn texture_mips_num(&self, texture: Self::Texture) -> Result<i32, i32> {
        Ok(unsafe { (self.services.texture_mips_num)(texture) })
    }
    fn texture_mip_load(&mut self, texture: Self::Texture, index: i32) -> Result<(), i32> {
        status(unsafe { (self.services.texture_mip_load)(texture, index) })
    }
    fn archive_version(&self) -> i32 { unsafe { (self.services.archive_version)(self.archive) } }
    fn serialize_char_remap(&mut self) -> Result<(), i32> {
        status(unsafe { (self.services.font_serialize_char_remap)(self.font, self.archive) })
    }
    fn set_is_remapped(&mut self, value: i32) { unsafe { (self.services.font_set_is_remapped)(self.font, value); } }
    fn serialize_native_fields(&mut self) -> Result<(), i32> {
        status(unsafe { (self.services.font_serialize_native_fields)(self.font, self.archive) })
    }
}

/// Native object/archive adapter. Nonzero status is rethrown by the C++ shell.
///
/// # Safety
/// All object/archive pointers and the service table must remain valid through
/// synchronous callbacks, under the native serializer's reentry contract.
pub unsafe fn serialize(font: *mut c_void, ar: *mut c_void, svc: &Hp2FontServices) -> i32 {
    serialize_with(&mut NativeFont { font, archive: ar, services: svc }).err().unwrap_or(0)
}
