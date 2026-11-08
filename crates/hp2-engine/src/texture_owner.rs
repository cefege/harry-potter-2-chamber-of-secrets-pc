//! Canonical Engine Bitmap and Texture object ownership and archive operations.
//!
//! Reflected Bitmap/Texture properties require the real Engine.u metadata; native
//! mip arrays remain separately owned and serialized here.

#[path = "texture_image_owner.rs"]
pub(crate) mod image;

use crate::{
    animation_owner,
    core_owner::EngineState,
    texture_runtime::{
        self, TextureArchiveIo, TextureRuntimeArgs, TextureRuntimeField, TextureRuntimeLeaf,
        TextureRuntimeLock, TextureRuntimeOp, TextureRuntimeServices,
    },
};
use hp2_abi::engine::generated::{
    self, FColor, FMipmap, FTextureInfo, TArrayRaw, UBitmap, UObject, UClass, UPalette,
    URenderDevice, UTexture,
};
use hp2_core::{
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    lazy_array::{self, LazyArraySlots},
    object_archive::{ArchiveMode, ObjectArchive},
    object_owner::ClassOps,
};
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const ARRAYS: Hp2ArrayServices = array_storage::system_allocator();
const BITMAP: ClassOps = {
    let mut ops = OBJECT;
    ops.construct = construct_bitmap;
    ops
};
pub(crate) const TEXTURE: ClassOps = {
    let mut ops = BITMAP;
    ops.construct = construct_texture;
    ops.finalize_members = finalize_texture;
    ops.serialize = serialize_texture;
    ops.post_load = post_load_texture;
    ops
};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    spec(
        "Bitmap",
        "Object",
        native_layout::<UBitmap>(UBitmap::NATIVE_ALIGNMENT),
        BITMAP,
        generated::BITMAP_CLASS_FLAGS,
    ),
    spec(
        "Texture",
        "Bitmap",
        native_layout::<UTexture>(UTexture::NATIVE_ALIGNMENT),
        TEXTURE,
        generated::TEXTURE_CLASS_FLAGS,
    ),
];

const fn native_layout<T>(alignment: usize) -> Layout {
    match Layout::from_size_align(size_of::<T>(), alignment) {
        Ok(layout) => layout,
        Err(_) => panic!("compiler-probed native alignment is invalid"),
    }
}

const fn spec(
    name: &'static str,
    superclass: &'static str,
    layout: Layout,
    operations: ClassOps,
    flags: u32,
) -> NativeClassSpec {
    NativeClassSpec {
        short_name: name,
        package: "Engine",
        super_name: Some(superclass),
        within: "Object",
        flags,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(layout),
        operations: Some(operations),
        properties: &[],
        defaults: &[],
    }
}

fn error(message: &'static str) -> RuntimeError {
    RuntimeError::new(message)
}
fn combine(first: RuntimeResult<()>, next: RuntimeResult<()>) -> RuntimeResult<()> {
    match (first, next) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}
fn slots(raw: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*raw).data),
            num: ptr::addr_of_mut!((*raw).num),
            max: ptr::addr_of_mut!((*raw).max),
        }
    }
}

unsafe fn construct_bitmap(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.construct)(access, object)?;
    }
    let bitmap = object.cast::<UBitmap>();
    unsafe {
        ptr::addr_of_mut!((*bitmap).format).write_unaligned(0); // P8
        ptr::addr_of_mut!((*bitmap).palette).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*bitmap).u_bits).write_unaligned(0);
        ptr::addr_of_mut!((*bitmap).v_bits).write_unaligned(0);
        ptr::addr_of_mut!((*bitmap).u_size).write_unaligned(0);
        ptr::addr_of_mut!((*bitmap).v_size).write_unaligned(0);
        ptr::addr_of_mut!((*bitmap).mip_zero).write_unaligned([64, 128, 64, 0]);
        ptr::addr_of_mut!((*bitmap).max_color).write_unaligned([255; 4]);
        ptr::addr_of_mut!((*bitmap).__last_update_time).write_unaligned(
            hp2_core::time::from_double(
                hp2_audio::verification::visual_seconds()
                    .unwrap_or_else(hp2_core::unix_clock::seconds_slow),
            ).to_ne_bytes(),
        );
    }
    Ok(())
}

unsafe fn construct_texture(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        construct_bitmap(access, object)?;
    }
    let texture = object.cast::<UTexture>();
    unsafe {
        ptr::addr_of_mut!((*texture).diffuse).write_unaligned(1.0);
        ptr::addr_of_mut!((*texture).specular).write_unaligned(1.0);
        ptr::addr_of_mut!((*texture).alpha).write_unaligned(0.0);
        ptr::addr_of_mut!((*texture).scale).write_unaligned(1.0);
        ptr::addr_of_mut!((*texture).friction).write_unaligned(1.0);
        ptr::addr_of_mut!((*texture).mip_mult).write_unaligned(1.0);
        initialize_array(ptr::addr_of_mut!((*texture).mips));
        initialize_array(ptr::addr_of_mut!((*texture).comp_mips));
    }
    Ok(())
}

unsafe fn initialize_array(raw: *mut TArrayRaw) {
    unsafe {
        raw.write_unaligned(TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        });
    }
}

unsafe fn destroy_texture(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.destroy)(access, object) }
}

unsafe fn finalize_texture(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let texture = object.cast::<UTexture>();
    let compressed = unsafe { destroy_mip_array(access, ptr::addr_of_mut!((*texture).comp_mips)) };
    let normal = unsafe { destroy_mip_array(access, ptr::addr_of_mut!((*texture).mips)) };
    combine(compressed, normal)
}

unsafe fn destroy_mip_array(access: CoreAccess<'_>, raw: *mut TArrayRaw) -> RuntimeResult<()> {
    let value = unsafe { ptr::read_unaligned(raw) };
    let mut result = Ok(());
    if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
        result = Err(error("Invalid texture mip storage"));
    } else if !value.data.is_null() {
        for i in (0..value.num as usize).rev() {
            let mip = unsafe {
                value
                    .data
                    .cast::<u8>()
                    .add(i * size_of::<FMipmap>())
                    .cast::<FMipmap>()
            };
            result = combine(result, unsafe { destroy_mip(access, mip) });
        }
    }
    let released = unsafe { array_storage::release(slots(raw), ARRAYS) }
        .map_err(|_| error("Texture mip storage release failed"));
    combine(result, released)
}

unsafe fn destroy_mip(access: CoreAccess<'_>, mip: *mut FMipmap) -> RuntimeResult<()> {
    let lazy = unsafe { mip_slots(mip) };
    let detached = unsafe { lazy_array::lazy_array_destroy(access, lazy) };
    let released = unsafe { array_storage::release(slots(lazy.array), ARRAYS) }
        .map_err(|_| error("Texture mip byte storage release failed"));
    combine(detached, released)
}

unsafe fn mip_slots(mip: *mut FMipmap) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mip).data_array) };
    unsafe {
        LazyArraySlots {
            array: lazy
                .cast::<u8>()
                .add(generated::TLazyBytes::TARRAY_BYTE_OFFSET)
                .cast(),
            loader: lazy
                .cast::<u8>()
                .add(generated::TLazyBytes::FLAZYLOADER_OFFSET)
                .cast(),
            saved_archive: ptr::addr_of_mut!((*lazy).saved_ar),
            saved_position: ptr::addr_of_mut!((*lazy).saved_pos),
        }
    }
}

unsafe fn serialize_texture(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    if is_reference_mode(archive) {
        return unsafe { (OBJECT.serialize)(access, object, archive) };
    }
    texture_runtime::serialize_texture(&mut TextureIo {
        access,
        object,
        archive: Some(archive),
    })
}

unsafe fn post_load_texture(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    texture_runtime::post_load(&mut TextureIo {
        access,
        object,
        archive: None,
    })
}

/// Calls the native UTexture superclass serializer without derived dispatch.
pub(crate) unsafe fn serialize_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { serialize_texture(access, object, archive) }
}

/// Calls the native UTexture superclass post-load without derived dispatch.
pub(crate) unsafe fn post_load_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe { post_load_texture(access, object) }
}

/// Calls native UTexture destruction without releasing its automatic members.
pub(crate) unsafe fn destroy_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe { destroy_texture(access, object) }
}

/// Resolves mip zero, loading ordinary pixels but retaining generated parametric output.
pub(crate) unsafe fn mip_zero(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
) -> RuntimeResult<(*mut u8, usize)> {
    if texture.is_null() {
        return Err(error("Texture mip zero requires a live UTexture"));
    }
    let mip = unsafe { mip_at(texture, false, 0)? };
    if !unsafe { (*texture).b_parametric() } {
        unsafe { lazy_array::lazy_array_load(access, mip_slots(mip))? };
    }
    let bytes = unsafe { ptr::read_unaligned(mip_slots(mip).array) };
    if bytes.num < 0 || bytes.max < bytes.num || (bytes.num > 0 && bytes.data.is_null()) {
        return Err(error("Invalid canonical texture mip-zero bytes"));
    }
    Ok((bytes.data.cast(), bytes.num as usize))
}

fn is_reference_mode(archive: &ObjectArchive<'_>) -> bool {
    matches!(
        archive.mode(),
        ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports
    )
}

struct TextureIo<'a, 'b, 'd> {
    access: CoreAccess<'a>,
    object: *mut UObject,
    archive: Option<&'b ObjectArchive<'d>>,
}

impl<'core, 'archive, 'data> TextureIo<'core, 'archive, 'data> {
    fn archive(&self) -> RuntimeResult<&'archive ObjectArchive<'data>> {
        self.archive
            .ok_or_else(|| error("Texture archive operation has no archive"))
    }
}

impl TextureArchiveIo for TextureIo<'_, '_, '_> {
    type Error = RuntimeError;
    fn is_loading(&self) -> Result<bool, Self::Error> {
        Ok(self.archive()?.is_loading())
    }
    fn is_saving(&self) -> Result<bool, Self::Error> {
        Ok(self.archive()?.is_saving())
    }
    fn is_parametric(&self) -> Result<bool, Self::Error> {
        Ok(unsafe { (*self.object.cast::<UTexture>()).b_parametric() })
    }
    fn has_compressed_mips(&self) -> Result<bool, Self::Error> {
        Ok(unsafe { (*self.object.cast::<UTexture>()).b_has_comp() })
    }
    fn serialize_super(&mut self) -> Result<(), Self::Error> {
        unsafe { (OBJECT.serialize)(self.access, self.object, self.archive()?) }
    }
    fn mip_count_bytes(&mut self, compressed: bool) -> Result<(), Self::Error> {
        let raw = unsafe { mip_array(self.object.cast(), compressed) };
        let value = unsafe { ptr::read_unaligned(raw) };
        if value.num < 0 || value.max < value.num {
            return Err(error("Invalid texture mip count"));
        }
        self.archive()?.count_bytes(
            (value.num as usize)
                .checked_mul(size_of::<FMipmap>())
                .ok_or_else(|| error("Texture mip byte count overflow"))?,
            (value.max as usize)
                .checked_mul(size_of::<FMipmap>())
                .ok_or_else(|| error("Texture mip capacity overflow"))?,
        )
    }
    fn mip_count(&self, compressed: bool) -> Result<i32, Self::Error> {
        let n = unsafe {
            ptr::read_unaligned(ptr::addr_of!(
                (*mip_array(self.object.cast(), compressed)).num
            ))
        };
        if n < 0 {
            Err(error("Invalid texture mip count"))
        } else {
            Ok(n)
        }
    }
    fn empty_mips(&mut self, compressed: bool, reserve: i32) -> Result<(), Self::Error> {
        unsafe {
            empty_mips(
                self.access,
                mip_array(self.object.cast(), compressed),
                reserve,
            )
        }
    }
    fn append_mip(&mut self, compressed: bool) -> Result<(), Self::Error> {
        unsafe { append_mip(self.access, mip_array(self.object.cast(), compressed)) }
    }
    fn archive_mip(&mut self, compressed: bool, index: i32) -> Result<(), Self::Error> {
        unsafe { serialize_mip(self.access, self.object, self.archive()?, compressed, index) }
    }
    fn archive_mip_index(&mut self, count: Option<i32>) -> Result<i32, Self::Error> {
        let mut n = count.unwrap_or(0);
        self.archive()?.serialize_compact_index(&mut n)?;
        if n < 0 {
            return Err(error("Negative texture mip count"));
        }
        Ok(n)
    }
    fn client_lod(&self) -> Result<i32, Self::Error> {
        let state = self
            .access
            .owner_context::<EngineState>()
            .ok_or_else(|| error("Texture class operations require their Engine owner context"))?;
        let Some(client) = state.bitmap_client.get() else {
            return Ok(0);
        };
        let lod_set = unsafe {
            ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UTexture>()).lod_set))
        } as usize;
        if lod_set >= 8 {
            return Err(error("Texture LOD set is outside native range"));
        }
        let values = unsafe { ptr::addr_of!((*client.as_ptr()).texture_lod_set) };
        Ok(unsafe { ptr::read_unaligned(values.cast::<i32>().add(lod_set)) })
    }
    fn lazy_load(&self) -> Result<bool, Self::Error> {
        Ok(self.access.process_mode().lazy_load)
    }
    fn set_lazy_load(&mut self, lazy: bool) -> Result<(), Self::Error> {
        let mut mode = self.access.process_mode();
        mode.lazy_load = lazy;
        self.access.set_process_mode(mode);
        Ok(())
    }
    fn parametric_data_empty(&mut self, index: i32) -> Result<(), Self::Error> {
        unsafe { clear_mip_bytes(self.object.cast(), false, index, None) }
    }
    fn mip_dimensions(&self, index: i32) -> Result<(i32, i32), Self::Error> {
        unsafe {
            let m = mip_at(self.object.cast(), false, index)?;
            Ok((
                ptr::read_unaligned(ptr::addr_of!((*m).base.u_size)),
                ptr::read_unaligned(ptr::addr_of!((*m).base.v_size)),
            ))
        }
    }
    fn parametric_data_empty_sized(&mut self, index: i32, len: i32) -> Result<(), Self::Error> {
        unsafe { clear_mip_bytes(self.object.cast(), false, index, Some(len)) }
    }
    fn parametric_data_zeroed(&mut self, index: i32, len: i32) -> Result<(), Self::Error> {
        unsafe { zero_mip_bytes(self.object.cast(), index, len) }
    }
    fn post_load_super(&mut self) -> Result<(), Self::Error> {
        unsafe { (OBJECT.post_load)(self.access, self.object) }
    }
    fn palette_exists(&self) -> Result<bool, Self::Error> {
        Ok(
            !unsafe {
                ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UBitmap>()).palette))
            }
            .is_null(),
        )
    }
    fn create_palette(&mut self) -> Result<(), Self::Error> {
        let class = self.access.native_class("Engine.Palette")?;
        let outer = unsafe { ptr::read_unaligned(ptr::addr_of!((*self.object).outer)) };
        let palette = unsafe {
            self.access
                .construct_object(class, outer.cast(), 0, 0, ptr::null_mut())?
        };
        unsafe {
            ptr::addr_of_mut!((*self.object.cast::<UBitmap>()).palette)
                .write_unaligned(palette.cast())
        };
        Ok(())
    }
    fn append_gray_palette_entry(&mut self, gray: u8, alpha: u8) -> Result<(), Self::Error> {
        let palette =
            unsafe { ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UBitmap>()).palette)) }
                .cast::<UPalette>();
        let raw = unsafe { ptr::addr_of_mut!((*palette).colors) };
        let index = unsafe {
            array_storage::add(
                slots(raw),
                1,
                size_of::<FColor>() as i32,
                ptr::null(),
                ARRAYS,
            )
        }
        .map_err(|_| error("Palette color allocation failed"))?;
        let data = unsafe { ptr::read_unaligned(ptr::addr_of!((*raw).data)) }.cast::<FColor>();
        unsafe {
            data.add(index as usize).write_unaligned(FColor {
                r: gray,
                g: gray,
                b: gray,
                a: alpha,
            });
        }
        Ok(())
    }
    fn u_clamp(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UBitmap>()).u_clamp)) })
    }
    fn set_u_clamp(&mut self, value: i32) -> Result<(), Self::Error> {
        unsafe {
            ptr::addr_of_mut!((*self.object.cast::<UBitmap>()).u_clamp).write_unaligned(value)
        };
        Ok(())
    }
    fn u_size(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UBitmap>()).u_size)) })
    }
    fn v_clamp(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UBitmap>()).v_clamp)) })
    }
    fn set_v_clamp(&mut self, value: i32) -> Result<(), Self::Error> {
        unsafe {
            ptr::addr_of_mut!((*self.object.cast::<UBitmap>()).v_clamp).write_unaligned(value)
        };
        Ok(())
    }
    fn v_size(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*self.object.cast::<UBitmap>()).v_size)) })
    }
    fn reset_accumulator(&mut self) -> Result<(), Self::Error> {
        unsafe {
            ptr::addr_of_mut!((*self.object.cast::<UTexture>()).accumulator).write_unaligned(0.0)
        };
        Ok(())
    }
    fn seconds(&mut self) -> Result<i64, Self::Error> {
        Ok(hp2_core::time::from_double(
            hp2_audio::verification::visual_seconds()
                .unwrap_or_else(hp2_core::unix_clock::seconds_slow),
        ))
    }
    fn set_last_time(&mut self, value: i64) -> Result<(), Self::Error> {
        unsafe {
            ptr::addr_of_mut!((*self.object.cast::<UBitmap>()).__last_update_time)
                .write_unaligned(value.to_ne_bytes())
        };
        Ok(())
    }
}

unsafe fn mip_array(texture: *mut UTexture, compressed: bool) -> *mut TArrayRaw {
    unsafe {
        if compressed {
            ptr::addr_of_mut!((*texture).comp_mips)
        } else {
            ptr::addr_of_mut!((*texture).mips)
        }
    }
}
unsafe fn empty_mips(
    access: CoreAccess<'_>,
    raw: *mut TArrayRaw,
    reserve: i32,
) -> RuntimeResult<()> {
    if reserve < 0 {
        return Err(error("Negative texture mip count"));
    }
    let old = unsafe { ptr::read_unaligned(raw) };
    if old.num < 0 || old.max < old.num || (old.num > 0 && old.data.is_null()) {
        return Err(error("Invalid texture mip storage"));
    }
    let mut first = Ok(());
    if !old.data.is_null() {
        for i in (0..old.num as usize).rev() {
            let m = unsafe {
                old.data
                    .cast::<u8>()
                    .add(i * size_of::<FMipmap>())
                    .cast::<FMipmap>()
            };
            first = combine(first, unsafe { destroy_mip(access, m) });
        }
    }
    let released = unsafe { array_storage::release(slots(raw), ARRAYS) }
        .map_err(|_| error("Texture mip release failed"));
    first = combine(first, released);
    first?;
    if reserve > 0 {
        unsafe { ptr::addr_of_mut!((*raw).max).write(reserve) };
        unsafe {
            array_storage::realloc(slots(raw), size_of::<FMipmap>() as i32, ptr::null(), ARRAYS)
        }
        .map_err(|_| error("Texture mip reserve failed"))?;
    }
    Ok(())
}

unsafe fn append_mip(access: CoreAccess<'_>, raw: *mut TArrayRaw) -> RuntimeResult<()> {
    let index = unsafe {
        array_storage::add(
            slots(raw),
            1,
            size_of::<FMipmap>() as i32,
            ptr::null(),
            ARRAYS,
        )
    }
    .map_err(|_| error("Texture mip allocation failed"))?;
    let data = unsafe { ptr::read_unaligned(ptr::addr_of!((*raw).data)) }.cast::<u8>();
    let mip = unsafe {
        data.add(index as usize * size_of::<FMipmap>())
            .cast::<FMipmap>()
    };
    // Initialize the complete Rust view, including opaque lazy-loader bytes,
    // before publishing a source or borrowing scalar fields during decoding.
    unsafe {
        mip.cast::<u8>().write_bytes(0, size_of::<FMipmap>());
    }
    let lazy = unsafe { mip_slots(mip) };
    if let Err(error) = unsafe { lazy_array::lazy_array_initialize(access, lazy) } {
        unsafe { ptr::addr_of_mut!((*raw).num).write(index) };
        return Err(error);
    }
    Ok(())
}

unsafe fn mip_at(
    texture: *mut UTexture,
    compressed: bool,
    index: i32,
) -> RuntimeResult<*mut FMipmap> {
    let raw = unsafe { ptr::read_unaligned(mip_array(texture, compressed)) };
    if index < 0 || index >= raw.num || raw.data.is_null() {
        return Err(error("Texture mip index out of range"));
    }
    Ok(unsafe {
        raw.data
            .cast::<u8>()
            .add(index as usize * size_of::<FMipmap>())
            .cast()
    })
}

/// Loads one canonical bitmap mip through its registered archive lazy loader.
///
/// # Safety
/// `texture` must be a live canonical texture and `index` a valid ordinary mip.
pub(crate) unsafe fn load_mip(
    access: CoreAccess<'_>, texture: *mut UTexture, index: i32,
) -> RuntimeResult<()> {
    unsafe { lazy_array::lazy_array_load(access, mip_slots(mip_at(texture, false, index)?)) }
}

unsafe fn serialize_mip(
    access: CoreAccess<'_>,
    owner: *mut UObject,
    archive: &ObjectArchive<'_>,
    compressed: bool,
    index: i32,
) -> RuntimeResult<()> {
    let mip = unsafe { mip_at(owner.cast(), compressed, index)? };
    let lazy = unsafe { mip_slots(mip) };
    unsafe {
        lazy_array::lazy_array_serialize(access, owner, lazy, archive, 1, serialize_bytes)?;
    }
    unsafe {
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mip).base.u_size))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mip).base.v_size))?;
        archive.serialize_u8(&mut *ptr::addr_of_mut!((*mip).base.u_bits))?;
        archive.serialize_u8(&mut *ptr::addr_of_mut!((*mip).base.v_bits))?;
    }
    Ok(())
}

unsafe fn serialize_bytes(
    _: CoreAccess<'_>,
    array: *mut c_void,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        animation_owner::serialize_array(
            archive,
            array.cast(),
            1,
            None,
            None,
            generated::SCALAR_FAST_SERIALIZE != 0,
        )
    }
}

unsafe fn clear_mip_bytes(
    texture: *mut UTexture,
    compressed: bool,
    index: i32,
    len: Option<i32>,
) -> RuntimeResult<()> {
    let mip = unsafe { mip_at(texture, compressed, index)? };
    let lazy = unsafe { mip_slots(mip) };
    let slots = slots(lazy.array);
    let old = unsafe { ptr::read_unaligned(lazy.array) };
    if old.num < 0 || old.max < old.num {
        return Err(error("Invalid texture mip data"));
    }
    unsafe { array_storage::release(slots, ARRAYS) }
        .map_err(|_| error("Texture mip data release failed"))?;
    if let Some(count) = len {
        if count < 0 {
            return Err(error("Negative texture mip data size"));
        }
        unsafe { ptr::addr_of_mut!((*lazy.array).max).write(count) };
        if count > 0 {
            unsafe { array_storage::realloc(slots, 1, ptr::null(), ARRAYS) }
                .map_err(|_| error("Texture mip data allocation failed"))?;
        }
    }
    Ok(())
}

unsafe fn zero_mip_bytes(texture: *mut UTexture, index: i32, len: i32) -> RuntimeResult<()> {
    if len < 0 {
        return Err(error("Negative texture mip data size"));
    }
    let mip = unsafe { mip_at(texture, false, index)? };
    let lazy = unsafe { mip_slots(mip) };
    if len > 0 {
        unsafe {
            ptr::addr_of_mut!((*lazy.array).num).write(len);
            ptr::write_bytes(
                ptr::read_unaligned(ptr::addr_of!((*lazy.array).data)).cast::<u8>(),
                0,
                len as usize,
            );
        }
    }
    Ok(())
}
/// Builds the complete source `FTextureInfo` through the shared
/// `TextureRuntimeOp::Lock` policy and Core-owned texture/mip records.
///
/// # Safety
/// `texture` and `info` must be live canonical records; `device` may be null
/// or a live canonical render-device record. The returned info borrows the
/// texture and its mip buffers until `unlock` returns.
pub(crate) unsafe fn lock(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    info: *mut FTextureInfo,
    time: i64,
    lod: i32,
    device: *mut URenderDevice,
) -> RuntimeResult<()> {
    if unsafe { crate::fractal_owner::lock_virtual(access, texture, info, time, lod, device)? } {
        return Ok(());
    }
    unsafe { lock_base(access, texture, info, time, lod, device) }
}

/// Executes the non-virtual UTexture::Lock body for derived native leaves.
pub(crate) unsafe fn lock_base(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    info: *mut FTextureInfo,
    time: i64,
    lod: i32,
    device: *mut URenderDevice,
) -> RuntimeResult<()> {
    if texture.is_null() || info.is_null() {
        return Err(error("Invalid canonical texture Lock arguments"));
    }
    unsafe { info.write_unaligned(std::mem::zeroed()) };
    let mut context = TextureLockContext {
        access,
        texture,
        info,
        device,
        error: None,
    };
    let services = TextureRuntimeServices { call: texture_lock_leaf };
    let args = TextureRuntimeArgs {
        native: (&mut context as *mut TextureLockContext<'_>).cast(),
        a: lod as usize,
        b: 0,
        time,
    };
    let result = texture_runtime::run(&services, args, TextureRuntimeOp::Lock);
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    result.map(|_| ()).map_err(|_| error("Canonical texture Lock policy failed"))
}

/// Runs a base texture lifecycle operation without recursively dispatching a
/// derived class virtual override.
pub(crate) unsafe fn runtime_operation(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    operation: TextureRuntimeOp,
    a: usize,
    b: usize,
    time: i64,
    info: *mut FTextureInfo,
    device: *mut URenderDevice,
) -> RuntimeResult<usize> {
    if texture.is_null() {
        return Err(error("Texture runtime operation requires a live UTexture"));
    }
    let mut context = TextureLockContext { access, texture, info, device, error: None };
    let services = TextureRuntimeServices { call: texture_lock_leaf };
    let args = TextureRuntimeArgs {
        native: (&mut context as *mut TextureLockContext<'_>).cast(),
        a,
        b,
        time,
    };
    let result = texture_runtime::run(&services, args, operation);
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    result.map_err(|_| error("Canonical texture runtime operation failed"))
}

/// Source `UTexture::Unlock(FTextureInfo&)` has no body. This checked Core
/// boundary preserves that behavior; the caller-owned info record and its
/// borrowed mip pointers expire at the caller's scope boundary.
///
/// # Safety
/// `texture` and `info` must be live records produced by `lock`.
pub(crate) unsafe fn unlock(
    _access: CoreAccess<'_>,
    texture: *mut UTexture,
    info: *mut FTextureInfo,
) -> RuntimeResult<()> {
    if texture.is_null() || info.is_null() {
        return Err(error("Invalid canonical texture Unlock arguments"));
    }
    let locked_texture = unsafe { ptr::read_unaligned(ptr::addr_of!((*info).texture)) };
    if locked_texture != texture.cast() {
        return Err(error("Canonical texture Unlock does not match its Lock"));
    }
    Ok(())
}
/// Applies the shared source `TextureRuntimeOp::Get` animation policy.
///
/// # Safety
/// `texture` must be a live canonical UTexture record.
pub(crate) unsafe fn get(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    time: i64,
) -> RuntimeResult<*mut UTexture> {
    if texture.is_null() {
        return Err(error("Null canonical texture Get receiver"));
    }
    let mut context = TextureLockContext {
        access,
        texture,
        info: ptr::null_mut(),
        device: ptr::null_mut(),
        error: None,
    };
    let services = TextureRuntimeServices { call: texture_lock_leaf };
    let args = TextureRuntimeArgs {
        native: (&mut context as *mut TextureLockContext<'_>).cast(),
        a: 0,
        b: 0,
        time,
    };
    let result = texture_runtime::run(&services, args, TextureRuntimeOp::Get);
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    let texture = result
        .map(|texture| texture as *mut UTexture)
        .map_err(|_| error("Canonical texture Get policy failed"))?;
    if texture.is_null() {
        return Err(error("Canonical texture Get returned null"));
    }
    Ok(texture)
}


struct TextureLockContext<'a> {
    access: CoreAccess<'a>,
    texture: *mut UTexture,
    info: *mut FTextureInfo,
    device: *mut URenderDevice,
    error: Option<RuntimeError>,
}

unsafe extern "C" fn texture_lock_leaf(
    native: *mut c_void,
    leaf: texture_runtime::TextureRuntimeLeaf,
    a: usize,
    b: usize,
    result: *mut usize,
) -> i32 {
    if native.is_null() || result.is_null() {
        return 1;
    }
    let context = native.cast::<TextureLockContext<'static>>();
    match unsafe { texture_lock_dispatch(context, leaf, a, b) } {
        Ok(value) => {
            unsafe { result.write_unaligned(value) };
            0
        }
        Err(error) => {
            unsafe { ptr::addr_of_mut!((*context).error).write(Some(error)) };
            1
        }
    }
}

fn texture_runtime_field(tag: usize) -> RuntimeResult<TextureRuntimeField> {
    if tag > TextureRuntimeField::FactoryCreateNew as usize {
        return Err(error("Texture runtime has an invalid field tag"));
    }
    // SAFETY: TextureRuntimeField is repr(C) with consecutive tags starting at
    // zero; FactoryCreateNew is its final variant.
    Ok(unsafe { std::mem::transmute::<u32, TextureRuntimeField>(tag as u32) })
}

unsafe fn texture_lock_dispatch(
    context: *mut TextureLockContext<'_>,
    leaf: texture_runtime::TextureRuntimeLeaf,
    a: usize,
    b: usize,
) -> RuntimeResult<usize> {
    use texture_runtime::TextureRuntimeLeaf as L;
    let texture = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).texture)) };
    let device = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).device)) };
    let info = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).info)) };
    match leaf {
        L::Read => unsafe { texture_lock_read(context, texture, device, info, texture_runtime_field(a)?, b) },
        L::Write => unsafe { texture_lock_write(context, texture, texture_runtime_field(a)?, b) }.map(|_| 0),
        L::MipCount => {
            let raw = unsafe { ptr::read_unaligned(mip_array(texture, a != 0)) };
            if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
                return Err(error("Invalid canonical texture mip array"));
            }
            Ok(raw.num as usize)
        }
        L::MipEmpty => {
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            unsafe { empty_mips(access, mip_array(texture, a != 0), b as i32)? };
            Ok(0)
        }
        L::MipAppend => {
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            let raw = unsafe { mip_array(texture, a != 0) };
            unsafe { append_mip(access, raw)? };
            let count = unsafe { ptr::read_unaligned(ptr::addr_of!((*raw).num)) };
            let mip = unsafe { mip_at(texture, a != 0, count - 1)? };
            if b != usize::MAX {
                let u_bits = ((b >> 8) & 0xff) as u32;
                let v_bits = (b & 0xff) as u32;
                let u = 1i32.checked_shl(u_bits).ok_or_else(|| error("Texture mip U bits exceed range"))?;
                let v = 1i32.checked_shl(v_bits).ok_or_else(|| error("Texture mip V bits exceed range"))?;
                unsafe {
                    ptr::addr_of_mut!((*mip).base.u_size).write_unaligned(u);
                    ptr::addr_of_mut!((*mip).base.v_size).write_unaligned(v);
                    ptr::addr_of_mut!((*mip).base.u_bits).write_unaligned(u_bits as u8);
                    ptr::addr_of_mut!((*mip).base.v_bits).write_unaligned(v_bits as u8);
                    let len = crate::tex::mip_size(u, v);
                    clear_mip_bytes(texture, a != 0, count - 1, Some(len))?;
                    ptr::addr_of_mut!((*mip_slots(mip).array).num).write_unaligned(len);
                }
            }
            Ok(0)
        }
        L::MipAppendSized => {
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            let raw = unsafe { mip_array(texture, false) };
            unsafe { append_mip(access, raw)? };
            let count = unsafe { ptr::read_unaligned(ptr::addr_of!((*raw).num)) };
            let mip = unsafe { mip_at(texture, false, count - 1)? };
            let u_bits = ((a >> 8) & 0xff) as u32;
            let v_bits = (a & 0xff) as u32;
            let u = 1i32.checked_shl(u_bits).ok_or_else(|| error("Texture mip U bits exceed range"))?;
            let v = 1i32.checked_shl(v_bits).ok_or_else(|| error("Texture mip V bits exceed range"))?;
            unsafe {
                ptr::addr_of_mut!((*mip).base.u_size).write_unaligned(u);
                ptr::addr_of_mut!((*mip).base.v_size).write_unaligned(v);
                ptr::addr_of_mut!((*mip).base.u_bits).write_unaligned(u_bits as u8);
                ptr::addr_of_mut!((*mip).base.v_bits).write_unaligned(v_bits as u8);
                clear_mip_bytes(texture, false, count - 1, Some(b as i32))?;
                ptr::addr_of_mut!((*mip_slots(mip).array).num).write_unaligned(b as i32);
            }
            Ok(0)
        }
        L::DataClear => {
            let mip = unsafe { mip_at(texture, false, a as i32)? };
            let raw = unsafe { mip_slots(mip).array.read_unaligned() };
            if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
                return Err(error("Invalid texture mip data during Clear"));
            }
            if raw.num > 0 {
                crate::tex::mip_clear(unsafe { std::slice::from_raw_parts_mut(raw.data.cast::<u8>(), raw.num as usize) });
            }
            Ok(0)
        }
        L::CheckInit => {
            let bitmap = unsafe { ptr::addr_of!((*texture).base) };
            let (u, v) = unsafe {
                (
                    ptr::read_unaligned(ptr::addr_of!((*bitmap).u_size)),
                    ptr::read_unaligned(ptr::addr_of!((*bitmap).v_size)),
                )
            };
            if u < 0 || v < 0 || (u & (u - 1)) != 0 || (v & (v - 1)) != 0 {
                return Err(error("Canonical texture dimensions are not powers of two"));
            }
            Ok(0)
        }
        L::AssignMips => {
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            unsafe { assign_mips(access, texture, a != 0)? };
            Ok(0)
        }
        L::VirtualInit => {
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            if let Some(value) = unsafe {
                crate::fractal_owner::virtual_call(
                    access, texture, TextureRuntimeOp::Init, a, b, 0, info, device,
                )?
            } {
                Ok(value)
            } else {
                texture_lock_nested(context, TextureRuntimeOp::Init, a, b, 0)
            }
        }
        L::DataView | L::MipDimensions => {
            let compressed = a >> 32 != 0;
            let index = a as u32 as i32;
            let mip = unsafe { mip_at(texture, compressed, index)? };
            let dimensions = unsafe {
                (
                    ptr::read_unaligned(ptr::addr_of!((*mip).base.u_size)),
                    ptr::read_unaligned(ptr::addr_of!((*mip).base.v_size)),
                )
            };
            let mut data = texture_runtime::TextureRuntimeData {
                data: ptr::null_mut(),
                len: 0,
                u: dimensions.0,
                v: dimensions.1,
            };
            if matches!(leaf, L::DataView) {
                let array = unsafe { ptr::read_unaligned(mip_slots(mip).array) };
                if array.num < 0 || array.max < array.num || (array.num > 0 && array.data.is_null()) {
                    return Err(error("Invalid canonical texture mip bytes"));
                }
                data.data = array.data.cast();
                data.len = array.num;
            }
            unsafe { (b as *mut texture_runtime::TextureRuntimeData).write_unaligned(data) };
            Ok(0)
        }
        L::DataLoad => {
            let mip = unsafe { mip_at(texture, a != 0, b as i32)? };
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            unsafe { lazy_array::lazy_array_load(access, mip_slots(mip))? };
            Ok(0)
        }
        L::InfoBegin => {
            if info.is_null() {
                return Err(error("Texture Lock has no FTextureInfo"));
            }
            unsafe {
                ptr::addr_of_mut!((*info).lod).write_unaligned(a as i32);
                ptr::addr_of_mut!((*info).texture).write_unaligned(texture.cast());
                ptr::addr_of_mut!((*info).pan).write_unaligned(hp2_abi::FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
                ptr::addr_of_mut!((*info).max_color)
                    .write_unaligned(ptr::addr_of_mut!((*texture).base.max_color).cast());
            }
            Ok(0)
        }
        L::InfoPublish => {
            if info.is_null() {
                return Err(error("Texture Lock has no FTextureInfo"));
            }
            let lock = unsafe { (a as *const TextureRuntimeLock).read_unaligned() };
            if lock.count <= 0 || lock.count as usize > 12 {
                return Err(error("Texture Lock has an invalid FTextureInfo mip count"));
            }
            if lock.lod < 0 || lock.u <= 0 || lock.v <= 0 || !lock.scale.is_finite() {
                return Err(error("Texture Lock has invalid canonical mip dimensions"));
            }
            let palette = unsafe {
                ptr::read_unaligned(ptr::addr_of!((*texture).base.palette)).cast::<UPalette>()
            };
            let palette_colors: *mut c_void = if palette.is_null() {
                ptr::null_mut()
            } else {
                let colors = unsafe { ptr::read_unaligned(ptr::addr_of!((*palette).colors)) };
                if colors.num < 0 || colors.max < colors.num || (colors.num > 0 && colors.data.is_null()) {
                    return Err(error("Invalid canonical texture palette"));
                }
                colors.data
            };
            let texture_index = unsafe {
                ptr::read_unaligned(ptr::addr_of!((*texture).base.base.index))
            } as u32;
            let palette_index = if palette.is_null() {
                0
            } else {
                unsafe { ptr::read_unaligned(ptr::addr_of!((*palette.cast::<UObject>()).index)) as u32 }
            };
            unsafe {
                ptr::addr_of_mut!((*info).u_scale).write_unaligned(lock.scale);
                ptr::addr_of_mut!((*info).v_scale).write_unaligned(lock.scale);
                ptr::addr_of_mut!((*info).palette_cache_id)
                    .write_unaligned(hp2_core::cache_id::frame(0x25, palette_index));
                ptr::addr_of_mut!((*info).palette).write_unaligned(palette_colors.cast());
                ptr::addr_of_mut!((*info).cache_id)
                    .write_unaligned(hp2_core::cache_id::object(0xE0u32.wrapping_add(lock.lod as u32), texture_index, 0));
                ptr::addr_of_mut!((*info).u_size).write_unaligned(lock.u);
                ptr::addr_of_mut!((*info).u_clamp).write_unaligned(lock.u);
                ptr::addr_of_mut!((*info).v_size).write_unaligned(lock.v);
                ptr::addr_of_mut!((*info).v_clamp).write_unaligned(lock.v);
                ptr::addr_of_mut!((*info).num_mips).write_unaligned(lock.count);
                ptr::addr_of_mut!((*info).format).write_unaligned(lock.format);
            }
            Ok(0)
        }
        L::InfoFlags => {
            if info.is_null() {
                return Err(error("Texture Lock has no FTextureInfo"));
            }
            let texture = unsafe { &*texture };
            let info = unsafe { &mut *info };
            info.set_b_high_color_quality(texture.b_high_color_quality());
            info.set_b_high_texture_quality(texture.b_high_texture_quality());
            info.set_b_realtime(texture.b_realtime());
            info.set_b_parametric(texture.b_parametric());
            info.set_b_realtime_changed(texture.b_realtime_changed());
            Ok(0)
        }
        L::InfoMipPublish => {
            if info.is_null() || b == 0 || b > 12 {
                return Err(error("Invalid canonical texture mip slot"));
            }
            let compressed = a >> 32 != 0;
            let index = a as u32 as i32;
            let mip = unsafe { mip_at(texture, compressed, index)? };
            // Lock loads ordinary mips; parametric bytes belong to their generator.
            let data_array = unsafe { ptr::read_unaligned(mip_slots(mip).array) };
            if data_array.num <= 0 || data_array.data.is_null() {
                return Err(error("Canonical texture mip data is unavailable"));
            }
            unsafe {
                ptr::addr_of_mut!((*mip).base.data_ptr).write_unaligned(data_array.data.cast());
                ptr::addr_of_mut!((*info).mips).cast::<*mut c_void>().add(b - 1).write_unaligned(mip.cast());
            }
            Ok(0)
        }
        L::VirtualTick => {
            let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
            if let Some(value) = unsafe {
                crate::fractal_owner::virtual_call(
                    access, texture, TextureRuntimeOp::Tick, a, 0, 0, info, device,
                )?
            } {
                return Ok(value);
            }
            let scripted = is_scripted_texture(access, texture)?;
            if scripted { unsafe { crate::scripted_texture_owner::begin_tick(texture.cast())?; } }
            let result = texture_lock_nested(context, TextureRuntimeOp::Tick, a, 0, 0)?;
            if scripted { unsafe { crate::scripted_texture_owner::tick(access, texture.cast())?; } }
            Ok(result)
        }
        L::VirtualUpdate | L::VirtualPrime | L::VirtualConstantTick | L::RippedScope => {
            let operation = match leaf {
                L::VirtualPrime => Some(TextureRuntimeOp::Prime),
                L::VirtualConstantTick => Some(TextureRuntimeOp::ConstantTick),
                _ => None,
            };
            if let Some(operation) = operation {
                let access = unsafe { ptr::read_unaligned(ptr::addr_of!((*context).access)) };
                if let Some(value) = unsafe {
                    crate::fractal_owner::virtual_call(
                        access, texture, operation, 0, 0, 0, info, device,
                    )?
                } {
                    return Ok(value);
                }
            }
            let (operation, first, second, time) = match leaf {
                L::VirtualUpdate => (TextureRuntimeOp::Update, 0, 0, a as i64),
                L::VirtualPrime => (TextureRuntimeOp::Prime, 0, 0, 0),
                L::VirtualConstantTick => (TextureRuntimeOp::ConstantTick, 0, 0, 0),
                L::RippedScope => (TextureRuntimeOp::RippedFixup, a, 0, 0),
                _ => unreachable!(),
            };
            texture_lock_nested(context, operation, first, second, time)
        }
        _ => Err(error("Unsupported Core texture runtime leaf during Lock")),
    }
}
fn is_scripted_texture(access: CoreAccess<'_>, texture: *mut UTexture) -> RuntimeResult<bool> {
    if texture.is_null() { return Ok(false); }
    let scripted = access.native_class("Engine.ScriptedTexture")?.cast::<UClass>();
    let mut class = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture.cast::<UObject>()).class)).cast::<UClass>() };
    while !class.is_null() {
        if class == scripted { return Ok(true); }
        class = unsafe { ptr::read_unaligned(ptr::addr_of!((*class).super_field)).cast::<UClass>() };
    }
    Ok(false)
}

unsafe fn texture_lock_read(
    context: *mut TextureLockContext<'_>,
    texture: *mut UTexture,
    device: *mut URenderDevice,
    info: *mut FTextureInfo,
    field: texture_runtime::TextureRuntimeField,
    argument: usize,
) -> RuntimeResult<usize> {
    use texture_runtime::TextureRuntimeField as F;
    let bitmap = unsafe { ptr::addr_of_mut!((*texture).base) };
    Ok(match field {
        F::Format => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).format)) as usize },
        F::Palette => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).palette)) as usize },
        F::UBits => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).u_bits)) as usize },
        F::VBits => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).v_bits)) as usize },
        F::USize => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).u_size)) as usize },
        F::VSize => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).v_size)) as usize },
        F::UClamp => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).u_clamp)) as usize },
        F::VClamp => unsafe { ptr::read_unaligned(ptr::addr_of!((*bitmap).v_clamp)) as usize },
        F::MipZero => unsafe { u32::from_ne_bytes((*bitmap).mip_zero) as usize },
        F::MaxColor => unsafe { u32::from_ne_bytes((*bitmap).max_color) as usize },
        F::LastTime => unsafe { i64::from_ne_bytes((*bitmap).__last_update_time) as usize },
        F::SelfTexture => texture as usize,
        F::Diffuse => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).diffuse)).to_bits() as usize },
        F::Specular => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).specular)).to_bits() as usize },
        F::Alpha => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).alpha)).to_bits() as usize },
        F::Scale => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).scale)).to_bits() as usize },
        F::Friction => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).friction)).to_bits() as usize },
        F::MipMult => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).mip_mult)).to_bits() as usize },
        F::Realtime => unsafe { (*texture).b_realtime() as usize },
        F::Changed => unsafe { (*texture).b_realtime_changed() as usize },
        F::Parametric => unsafe { (*texture).b_parametric() as usize },
        F::HasComp => unsafe { (*texture).b_has_comp() as usize },
        F::CompFormat => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).comp_format)) as usize },
        F::HighColor => unsafe { (*texture).b_high_color_quality() as usize },
        F::HighTexture => unsafe { (*texture).b_high_texture_quality() as usize },
        F::PrimeCurrent => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).prime_current)) as usize },
        F::PrimeCount => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).prime_count)) as usize },
        F::MinRate => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).min_frame_rate)).to_bits() as usize },
        F::MaxRate => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).max_frame_rate)).to_bits() as usize },
        F::Accumulator => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).accumulator)).to_bits() as usize },
        F::AnimCur => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).anim_cur)) as usize },
        F::AnimNext => {
            let animation = argument as *mut UTexture;
            if animation.is_null() { 0 } else { unsafe { ptr::read_unaligned(ptr::addr_of!((*animation).anim_next)) as usize } }
        }
        F::ClientLod => unsafe { texture_client_lod((*context).access, texture)? as usize },
        F::HasClient => unsafe {
            (*context).access.owner_context::<EngineState>().and_then(|s| s.bitmap_client.get()).is_some() as usize
        },
        F::RecommendedLod => if device.is_null() { 0 } else { unsafe { ptr::read_unaligned(ptr::addr_of!((*device).recommended_lod)) as usize } },
        F::SupportsTc => if device.is_null() { 0 } else { unsafe { (ptr::read_unaligned(ptr::addr_of!((*device).supports_tc)) != 0) as usize } },
        F::Deferred => if device.is_null() { 0 } else { unsafe { (ptr::read_unaligned(ptr::addr_of!((*device).prefers_deferred_load)) != 0) as usize } },
        F::HasDevice => (!device.is_null()) as usize,
        F::Lazy => unsafe { (*context).access.process_mode().lazy_load as usize },
        F::Loading | F::Saving | F::Version => 0,
        F::InfoTexture => if info.is_null() { 0 } else { unsafe { ptr::read_unaligned(ptr::addr_of!((*info).texture)) as usize } },
        F::InfoParametric => if info.is_null() { 0 } else { unsafe { (&*info).b_parametric() as usize } },
        F::InfoCount => if info.is_null() { 0 } else { unsafe { ptr::read_unaligned(ptr::addr_of!((*info).num_mips)) as usize } },
        F::PolyFlags => unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).poly_flags)) as usize },
        F::FactoryCreateNew => 0,
    })
}

unsafe fn texture_lock_write(
    context: *mut TextureLockContext<'_>,
    texture: *mut UTexture,
    field: texture_runtime::TextureRuntimeField,
    value: usize,
) -> RuntimeResult<()> {
    use texture_runtime::TextureRuntimeField as F;
    let bitmap = unsafe { ptr::addr_of_mut!((*texture).base) };
    match field {
        F::Format => unsafe { ptr::addr_of_mut!((*bitmap).format).write_unaligned(value as u8) },
        F::Palette => unsafe { ptr::addr_of_mut!((*bitmap).palette).write_unaligned(value as *mut c_void) },
        F::UBits => unsafe { ptr::addr_of_mut!((*bitmap).u_bits).write_unaligned(value as u8) },
        F::VBits => unsafe { ptr::addr_of_mut!((*bitmap).v_bits).write_unaligned(value as u8) },
        F::USize => unsafe { ptr::addr_of_mut!((*bitmap).u_size).write_unaligned(value as i32) },
        F::VSize => unsafe { ptr::addr_of_mut!((*bitmap).v_size).write_unaligned(value as i32) },
        F::UClamp => unsafe { ptr::addr_of_mut!((*bitmap).u_clamp).write_unaligned(value as i32) },
        F::VClamp => unsafe { ptr::addr_of_mut!((*bitmap).v_clamp).write_unaligned(value as i32) },
        F::MipZero => unsafe { ptr::addr_of_mut!((*bitmap).mip_zero).write_unaligned((value as u32).to_ne_bytes()) },
        F::MaxColor => unsafe { ptr::addr_of_mut!((*bitmap).max_color).write_unaligned((value as u32).to_ne_bytes()) },
        F::LastTime => unsafe { ptr::addr_of_mut!((*bitmap).__last_update_time).write_unaligned((value as i64).to_ne_bytes()) },
        F::Diffuse => unsafe { ptr::addr_of_mut!((*texture).diffuse).write_unaligned(f32::from_bits(value as u32)) },
        F::Specular => unsafe { ptr::addr_of_mut!((*texture).specular).write_unaligned(f32::from_bits(value as u32)) },
        F::Alpha => unsafe { ptr::addr_of_mut!((*texture).alpha).write_unaligned(f32::from_bits(value as u32)) },
        F::Scale => unsafe { ptr::addr_of_mut!((*texture).scale).write_unaligned(f32::from_bits(value as u32)) },
        F::Friction => unsafe { ptr::addr_of_mut!((*texture).friction).write_unaligned(f32::from_bits(value as u32)) },
        F::MipMult => unsafe { ptr::addr_of_mut!((*texture).mip_mult).write_unaligned(f32::from_bits(value as u32)) },
        F::Changed => unsafe { (*texture).set_b_realtime_changed(value != 0) },
        F::Parametric => unsafe { (*texture).set_b_parametric(value != 0) },
        F::Realtime => unsafe { (*texture).set_b_realtime(value != 0) },
        F::HighColor => unsafe { (*texture).set_b_high_color_quality(value != 0) },
        F::HighTexture => unsafe { (*texture).set_b_high_texture_quality(value != 0) },
        F::PrimeCurrent => unsafe { ptr::addr_of_mut!((*texture).prime_current).write_unaligned(value as u8) },
        F::PrimeCount => unsafe { ptr::addr_of_mut!((*texture).prime_count).write_unaligned(value as u8) },
        F::MinRate => unsafe { ptr::addr_of_mut!((*texture).min_frame_rate).write_unaligned(f32::from_bits(value as u32)) },
        F::MaxRate => unsafe { ptr::addr_of_mut!((*texture).max_frame_rate).write_unaligned(f32::from_bits(value as u32)) },
        // The native SelfTexture leaf only returns the pointer, even on Write.
        F::SelfTexture => (),
        F::Accumulator => unsafe { ptr::addr_of_mut!((*texture).accumulator).write_unaligned(f32::from_bits(value as u32)) },
        F::AnimCur => unsafe { ptr::addr_of_mut!((*texture).anim_cur).write_unaligned(value as *mut c_void) },
        F::PolyFlags => unsafe { ptr::addr_of_mut!((*texture).poly_flags).write_unaligned(value as u32) },
        F::CompFormat | F::HasComp | F::AnimNext | F::ClientLod | F::HasClient
        | F::RecommendedLod | F::SupportsTc | F::Deferred | F::HasDevice | F::Lazy
        | F::Loading | F::Saving | F::Version | F::InfoTexture | F::InfoParametric
        | F::InfoCount | F::FactoryCreateNew => {
            return Err(error("Texture Lock attempted to write a read-only Core field"));
        }
    }
    let _ = context;
    Ok(())
}
unsafe fn texture_lock_nested(
    context: *mut TextureLockContext<'_>,
    operation: TextureRuntimeOp,
    a: usize,
    b: usize,
    time: i64,
) -> RuntimeResult<usize> {
    let services = TextureRuntimeServices { call: texture_lock_leaf };
    let args = TextureRuntimeArgs {
        native: context.cast(),
        a,
        b,
        time,
    };
    texture_runtime::run(&services, args, operation)
        .map_err(|_| error("Nested canonical texture policy failed"))
}

unsafe fn assign_mips(
    access: CoreAccess<'_>,
    texture: *mut UTexture,
    compressed: bool,
) -> RuntimeResult<()> {
    if !compressed {
        return Ok(());
    }
    let source = unsafe { ptr::read_unaligned(mip_array(texture, false)) };
    if source.num < 0 || source.max < source.num || (source.num > 0 && source.data.is_null()) {
        return Err(error("Invalid source texture mip array"));
    }
    let target = unsafe { mip_array(texture, true) };
    unsafe { empty_mips(access, target, source.num)? };
    for index in 0..source.num {
        let src = unsafe { mip_at(texture, false, index)? };
        unsafe { lazy_array::lazy_array_load(access, mip_slots(src))? };
        let data = unsafe { ptr::read_unaligned(mip_slots(src).array) };
        if data.num < 0 || data.max < data.num || (data.num > 0 && data.data.is_null()) {
            return Err(error("Invalid source texture mip bytes"));
        }
        unsafe { append_mip(access, target)? };
        let dst = unsafe { mip_at(texture, true, index)? };
        let (u, v) = unsafe {
            (
                ptr::read_unaligned(ptr::addr_of!((*src).base.u_size)),
                ptr::read_unaligned(ptr::addr_of!((*src).base.v_size)),
            )
        };
        unsafe {
            ptr::addr_of_mut!((*dst).base.u_size).write_unaligned(u);
            ptr::addr_of_mut!((*dst).base.v_size).write_unaligned(v);
        }
        if data.num > 0 {
            let dst_slots = unsafe { mip_slots(dst) };
            unsafe {
                array_storage::add(
                    slots(dst_slots.array),
                    data.num,
                    1,
                    ptr::null(),
                    ARRAYS,
                )
                .map_err(|_| error("Compressed texture mip allocation failed"))?;
            }
            let destination = unsafe { ptr::read_unaligned(dst_slots.array) };
            unsafe {
                ptr::copy_nonoverlapping(
                    data.data,
                    destination.data,
                    data.num as usize,
                );
                ptr::addr_of_mut!((*dst).base.data_ptr).write_unaligned(destination.data.cast());
            }
        }
    }
    Ok(())
}

unsafe fn texture_client_lod(access: CoreAccess<'_>, texture: *mut UTexture) -> RuntimeResult<i32> {
    let state = access
        .owner_context::<EngineState>()
        .ok_or_else(|| error("Texture Lock requires the Engine owner context"))?;
    let Some(client) = state.bitmap_client.get() else {
        return Ok(0);
    };
    let lod_set = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).lod_set)) } as usize;
    if lod_set >= 8 {
        return Err(error("Texture LOD set is outside native range"));
    }
    let lods = unsafe { ptr::addr_of!((*client.as_ptr()).texture_lod_set) };
    Ok(unsafe { ptr::read_unaligned(lods.cast::<i32>().add(lod_set)) })
}

