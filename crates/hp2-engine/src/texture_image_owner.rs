//! Canonical image import and viewport capture over the shared texture/factory policies.
//! This is a child of texture_owner so all mip allocation and field operations
//! remain the same operations used by ordinary canonical textures.
use super::{TEXTURE, TextureLockContext, texture_lock_dispatch};
use crate::texture_runtime::{
    self, TextureRuntimeArgs, TextureRuntimeData, TextureRuntimeField as F,
    TextureRuntimeLeaf as L, TextureRuntimeOp as O, TextureRuntimeServices,
};
use hp2_abi::engine::generated::{
    FFrame, TArrayRaw, UClass, UObject, UPalette, UTexture, UViewport,
};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupDefault},
    core_records::FactoryRecord,
    factory_exporter::{
        self, Hp2FactoryExporterArgs, Hp2FactoryExporterLeaf as FL,
        Hp2FactoryExporterOperation as FO, Hp2FactoryExporterServices,
    },
    name_owner::FindName,
    native_text::{self, TextPart},
};
use std::{alloc::Layout, ffi::c_void, path::PathBuf, ptr};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "EngineTextureFactory",
    package: "Editor",
    super_name: Some("Factory"),
    within: "Object",
    flags: 0,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Factory,
    layout: Some(Layout::new::<FactoryRecord>()),
    operations: None,
    properties: &[],
    defaults: &[
        StartupDefault::SetClass {
            field: "SupportedClass",
            class: "Engine.Texture",
        },
        StartupDefault::AppendString {
            field: "Formats",
            value: "bmp;Bitmap files;pcx;PC Painbrush files",
        },
        StartupDefault::SetBool {
            field: "bCreateNew",
            value: false,
        },
    ],
}];

fn object_name(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
    let name = unsafe { i32::from_ne_bytes((*object).name) };
    let mut value = access.name_text(name)?;
    value.push(0);
    Ok(value)
}
unsafe fn log_parts(
    access: CoreAccess<'_>,
    category: &str,
    parts: &[TextPart],
) -> RuntimeResult<()> {
    let category = access.intern_str(category, FindName::Add)?;
    if access.name_suppressed(category) {
        return Ok(());
    }
    let mut message = unsafe { native_text::fmt(parts) };
    if message.last() == Some(&0) {
        message.pop();
    }
    if message.len() >= 4096 {
        message.clear();
    }
    access.emit_log(category, &message)
}
fn path_from_string(access: CoreAccess<'_>, value: *const TArrayRaw) -> RuntimeResult<PathBuf> {
    let units = unsafe { native_text::code_units(native_text::fstring_units(value)) };
    let mut text = String::with_capacity(units.len());
    for &unit in units.iter().take_while(|&&unit| unit != 0) {
        text.push(
            char::from_u32(unit)
                .ok_or_else(|| RuntimeError::new("Invalid image filename TCHAR"))?,
        );
    }
    let path = PathBuf::from(text);
    Ok(if path.is_absolute() {
        path
    } else {
        access.data_root().join("System").join(path)
    })
}

struct ImageContext<'a> {
    base: TextureLockContext<'a>,
    viewport: *mut UViewport,
    palette: *mut UPalette,
    palette_only: bool,
    image: *const u8,
    image_len: i32,
    cursor: *const u8,
    class: *mut UClass,
    parent: *mut UObject,
    name: i32,
    flags: u32,
    palette_cursor: i32,
    name_text: Vec<u32>,
    animation_name: [u32; 256],
}
impl<'a> ImageContext<'a> {
    fn new(access: CoreAccess<'a>, texture: *mut UTexture) -> Self {
        Self {
            base: TextureLockContext {
                access,
                texture,
                info: ptr::null_mut(),
                device: ptr::null_mut(),
                error: None,
            },
            viewport: ptr::null_mut(),
            palette: ptr::null_mut(),
            palette_only: false,
            image: ptr::null(),
            image_len: 0,
            cursor: ptr::null(),
            class: ptr::null_mut(),
            parent: ptr::null_mut(),
            name: 0,
            flags: 0,
            palette_cursor: 0,
            name_text: Vec::new(),
            animation_name: [0; 256],
        }
    }
}
unsafe fn run_image(
    context: *mut ImageContext<'_>,
    operation: O,
    a: usize,
    b: usize,
) -> RuntimeResult<usize> {
    let result = texture_runtime::run(
        &TextureRuntimeServices { call: image_leaf },
        TextureRuntimeArgs {
            native: context.cast(),
            a,
            b,
            time: 0,
        },
        operation,
    );
    if let Some(error) = unsafe { (*context).base.error.take() } {
        return Err(error);
    }
    result.map_err(|status| {
        RuntimeError::new("Canonical image policy failed").with_context(&status.to_string())
    })
}
unsafe extern "C" fn image_leaf(
    raw: *mut c_void,
    leaf: L,
    a: usize,
    b: usize,
    out: *mut usize,
) -> i32 {
    let context = raw.cast::<ImageContext<'_>>();
    match unsafe { image_dispatch(context, leaf, a, b) } {
        Ok(value) => {
            unsafe { out.write(value) };
            0
        }
        Err(error) => {
            unsafe { (*context).base.error.get_or_insert(error) };
            1
        }
    }
}
unsafe fn selected_palette(context: *mut ImageContext<'_>, argument: usize) -> *mut UPalette {
    if argument != 0 {
        return argument as *mut UPalette;
    }
    if unsafe { (*context).palette_only } {
        unsafe { (*context).palette }
    } else {
        unsafe {
            ptr::addr_of!((*(*context).base.texture).base.palette)
                .read_unaligned()
                .cast()
        }
    }
}
unsafe fn next_palette(context: *mut ImageContext<'_>) -> RuntimeResult<usize> {
    let access = unsafe { (*context).base.access };
    let class = access.native_class("Engine.Palette")?;
    while unsafe { (*context).palette_cursor } < access.object_count() {
        let index = unsafe { (*context).palette_cursor };
        unsafe {
            (*context).palette_cursor += 1;
        }
        let object = access.object_at(index);
        if !object.is_null() && unsafe { hp2_core::object_model::is_a(object.cast(), class.cast()) }
        {
            return Ok(object as usize);
        }
    }
    Ok(0)
}
unsafe fn image_dispatch(
    context: *mut ImageContext<'_>,
    leaf: L,
    a: usize,
    b: usize,
) -> RuntimeResult<usize> {
    let access = unsafe { (*context).base.access };
    let texture = unsafe { (*context).base.texture };
    match leaf {
        L::Read if a == F::Palette as usize && unsafe { (*context).palette_only } => {
            Ok(unsafe { (*context).palette as usize })
        }
        L::SnapView => {
            let viewport = unsafe { (*context).viewport };
            let array = unsafe { ptr::addr_of!((*viewport).snap_buff).read_unaligned() };
            let value = TextureRuntimeData {
                data: array.data.cast(),
                len: array.num,
                u: unsafe { (*viewport).snap_u },
                v: unsafe { (*viewport).snap_v },
            };
            unsafe { (b as *mut TextureRuntimeData).write_unaligned(value) };
            Ok(0)
        }
        L::ImageView => {
            let value = TextureRuntimeData {
                data: unsafe { (*context).image.cast_mut() },
                len: unsafe { (*context).image_len },
                u: 0,
                v: 0,
            };
            unsafe { (b as *mut TextureRuntimeData).write_unaligned(value) };
            Ok(0)
        }
        L::ImageCursor => Ok(unsafe { ptr::addr_of_mut!((*context).cursor) as usize }),
        L::FactoryConstruct => {
            let (class, parent, name, flags) = unsafe {
                (
                    (*context).class,
                    (*context).parent,
                    (*context).name,
                    (*context).flags,
                )
            };
            let object =
                unsafe { access.construct_object(class, parent, name, flags, ptr::null_mut())? };
            let expected = access.native_class("Engine.Texture")?;
            if !unsafe { hp2_core::object_model::is_a(object.cast(), expected.cast()) } {
                return Err(RuntimeError::new(
                    "Texture factory constructed a non-Texture",
                ));
            }
            unsafe { (*context).base.texture = object.cast() };
            Ok(0)
        }
        L::FactoryPostLoad => {
            unsafe { (TEXTURE.post_load)(access, texture.cast())? };
            Ok(0)
        }
        L::FactoryPaletteNew => {
            let parent = unsafe { (*context).parent };
            let palette = unsafe {
                access.construct_object(
                    access.native_class("Engine.Palette")?,
                    parent,
                    0,
                    4,
                    ptr::null_mut(),
                )?
            };
            unsafe { (*context).palette = palette.cast() };
            Ok(0)
        }
        L::PaletteEmpty => {
            let palette = unsafe { (*context).palette };
            let slots = unsafe {
                array_storage::Hp2ArraySlots {
                    data: ptr::addr_of_mut!((*palette).colors.data),
                    num: ptr::addr_of_mut!((*palette).colors.num),
                    max: ptr::addr_of_mut!((*palette).colors.max),
                }
            };
            unsafe {
                array_storage::empty(slots, 0, 4, ptr::null(), array_storage::system_allocator())
            }
            .map_err(|_| RuntimeError::new("Unable to empty imported palette"))?;
            Ok(0)
        }
        L::PaletteAppend => {
            let palette = unsafe { (*context).palette };
            let raw = unsafe { ptr::addr_of_mut!((*palette).colors) };
            let slots = unsafe {
                Hp2ArraySlots {
                    data: ptr::addr_of_mut!((*raw).data),
                    num: ptr::addr_of_mut!((*raw).num),
                    max: ptr::addr_of_mut!((*raw).max),
                }
            };
            let index = unsafe {
                array_storage::add(slots, 1, 4, ptr::null(), array_storage::system_allocator())
            }
            .map_err(|_| RuntimeError::new("Unable to append imported palette"))?;
            unsafe {
                slots
                    .data
                    .read_unaligned()
                    .cast::<u32>()
                    .add(index as usize)
                    .write_unaligned(a as u32)
            };
            Ok(0)
        }
        L::PaletteCount => Ok(unsafe { (*(*context).palette).colors.num as usize }),
        L::PaletteView => {
            let palette = unsafe { selected_palette(context, a) };
            if palette.is_null() {
                return Err(RuntimeError::new("Image operation requires a palette"));
            }
            let colors = unsafe { ptr::addr_of!((*palette).colors).read_unaligned() };
            if colors.num < 256 || colors.data.is_null() {
                return Err(RuntimeError::new("Image palette has fewer than 256 colors"));
            }
            unsafe {
                (b as *mut TextureRuntimeData).write_unaligned(TextureRuntimeData {
                    data: colors.data.cast(),
                    len: colors.num,
                    u: 0,
                    v: 0,
                })
            };
            Ok(0)
        }
        L::PaletteOuter => {
            let palette = if a != 0 {
                a as *mut UPalette
            } else {
                unsafe { (*context).palette }
            };
            Ok(unsafe { (*palette.cast::<UObject>()).outer as usize })
        }
        L::PaletteIterator => {
            unsafe { (*context).palette_cursor = 0 };
            unsafe { next_palette(context) }
        }
        L::PaletteAdvance => unsafe { next_palette(context) },
        L::PaletteLogDelete => {
            let palette = unsafe { (*context).palette };
            let old = object_name(access, palette.cast())?;
            let replacement = object_name(access, (a as *mut UPalette).cast())?;
            unsafe {
                log_parts(
                    access,
                    "Log",
                    &[
                        TextPart::L("Replaced palette "),
                        TextPart::P(old.as_ptr()),
                        TextPart::L(" with "),
                        TextPart::P(replacement.as_ptr()),
                    ],
                )?;
                access.delete_object(palette.cast())?;
            }
            Ok(a)
        }
        L::FactoryPalettePublish => {
            let palette = unsafe { (*context).palette };
            let mut nested = ImageContext::new(access, ptr::null_mut());
            nested.palette = palette;
            nested.palette_only = true;
            let replacement = unsafe { run_image(&raw mut nested, O::PaletteReplace, 0, 0)? };
            unsafe {
                ptr::addr_of_mut!((*texture).base.palette)
                    .write_unaligned(replacement as *mut c_void);
                (*context).palette = ptr::null_mut();
            }
            Ok(0)
        }
        L::FactoryWarn => {
            match a {
                0 => unsafe {
                    log_parts(
                        access,
                        "Log",
                        &[TextPart::L("Texture dimensions are not powers of two")],
                    )?
                },
                1 => unsafe {
                    log_parts(
                        access,
                        "Log",
                        &[TextPart::L("RLE compression of BMP images not supported")],
                    )?
                },
                2 => unsafe {
                    log_parts(
                        access,
                        "Log",
                        &[
                            TextPart::L("BMP uses an unsupported format ("),
                            TextPart::I((b >> 16) as i32),
                            TextPart::L("/"),
                            TextPart::I((b & 65535) as i32),
                            TextPart::L(")"),
                        ],
                    )?
                },
                3 => unsafe {
                    log_parts(
                        access,
                        "Log",
                        &[TextPart::L("Bad image format for texture import")],
                    )?
                },
                _ => return Err(RuntimeError::new("Unknown texture factory warning")),
            };
            Ok(0)
        }
        L::FactoryAnimationName => {
            let text = object_name(access, texture.cast())?;
            unsafe { (*context).name_text = text };
            let name = unsafe { &(*context).name_text };
            unsafe {
                (b as *mut TextureRuntimeData).write_unaligned(TextureRuntimeData {
                    data: name.as_ptr().cast_mut().cast(),
                    len: (name.len() - 1) as i32,
                    u: 0,
                    v: 0,
                })
            };
            Ok(0)
        }
        L::FactoryAnimationCopy => {
            let name = unsafe { &(*context).name_text };
            unsafe {
                ptr::copy_nonoverlapping(
                    name.as_ptr(),
                    ptr::addr_of_mut!((*context).animation_name).cast::<u32>(),
                    name.len(),
                );
                (*(b as *mut TextureRuntimeData)).data =
                    ptr::addr_of_mut!((*context).animation_name).cast();
            }
            Ok(0)
        }
        L::FactoryAnimationFind => {
            let frame = a as u32;
            let position = b;
            if position + 2 >= 256 {
                return Err(RuntimeError::new(
                    "Texture animation name exceeds native buffer",
                ));
            }
            unsafe {
                (*context).animation_name[position] = u32::from(b'0') + frame / 10;
                (*context).animation_name[position + 1] = u32::from(b'0') + frame % 10;
                (*context).animation_name[position + 2] = 0;
            }
            let name = unsafe { &(*context).animation_name };
            let index = access.intern(
                &name[..name.iter().position(|v| *v == 0).unwrap()],
                FindName::Find,
            )?;
            let outer = unsafe { (*texture.cast::<UObject>()).outer.cast() };
            let class = access.native_class("Engine.Texture")?;
            Ok(unsafe { access.find_object(class, outer, index, false) } as usize)
        }
        L::FactoryAnimationLink => {
            let other = a as *mut UTexture;
            if b != 0 {
                unsafe { ptr::addr_of_mut!((*texture).anim_next).write_unaligned(other.cast()) };
            } else {
                unsafe { ptr::addr_of_mut!((*other).anim_next).write_unaligned(texture.cast()) };
            }
            let name = object_name(access, other.cast())?;
            unsafe {
                log_parts(
                    access,
                    "Log",
                    &[
                        TextPart::L(if b != 0 {
                            "   Linked to next: "
                        } else {
                            "   Linked to previous: "
                        }),
                        TextPart::P(name.as_ptr()),
                    ],
                )?
            };
            Ok(0)
        }
        L::FactoryAnimationLog => {
            let name = object_name(access, texture.cast())?;
            unsafe {
                log_parts(
                    access,
                    "Log",
                    &[
                        TextPart::L("Texture animation frame "),
                        TextPart::I(a as i32),
                        TextPart::L(": "),
                        TextPart::P(name.as_ptr()),
                    ],
                )?
            };
            Ok(0)
        }
        L::FactoryCompressParam => Ok(unsafe {
            hp2_core::search::parse_param(
                access.command_line().as_ptr().cast(),
                [67, 79, 77, 80, 82, 69, 83, 83, 68, 88, 84, 0].as_ptr(),
            )
        } as usize),
        // UnTex.cpp's UTexture::Compress/Decompress explicitly return false.
        L::Compress | L::Decompress => Ok(0),
        L::FactoryResult => Ok(unsafe {
            if !(*context).palette.is_null() {
                (*context).palette as usize
            } else {
                texture as usize
            }
        }),
        _ => unsafe { texture_lock_dispatch(ptr::addr_of_mut!((*context).base), leaf, a, b) },
    }
}

struct BinaryFileLoad<'a> {
    access: CoreAccess<'a>,
    path: &'a std::path::Path,
    archive: Option<hp2_core::runtime_archive::RuntimeArchive>,
    bytes: Vec<u8>,
}
impl hp2_core::file_persistence::LoadArrayOps for BinaryFileLoad<'_> {
    type Error = RuntimeError;
    fn open(&mut self) -> RuntimeResult<i32> {
        self.archive = hp2_core::unix_file_manager_owner::open_archive(self.access, self.path, false)?;
        Ok(i32::from(self.archive.is_some()))
    }
    fn size(&mut self) -> RuntimeResult<i32> {
        self.archive
            .as_mut()
            .expect("opened binary archive")
            .total_size()
            .map_err(|status| {
                RuntimeError::new("Binary import archive size failed")
                    .with_context(&status.to_string())
            })
    }
    unsafe fn binary(&mut self, count: i32) -> RuntimeResult<*mut u8> {
        self.bytes.clear();
        if count <= 0 {
            return Err(RuntimeError::new("Binary file array has no element zero"));
        }
        // ImportBinary immediately appends its sentinel; keep that in this one
        // allocation, without initializing bytes that fread will overwrite.
        self.bytes.reserve_exact(count as usize + 1);
        Ok(self.bytes.as_mut_ptr())
    }
    unsafe fn serialize(&mut self, bytes: *mut u8, count: i32) -> RuntimeResult<()> {
        let archive = self.archive.as_mut().expect("opened binary archive");
        let status = unsafe { archive.serialize(bytes.cast(), count) };
        if status != 0 {
            return Err(RuntimeError::new("Binary import archive callback failed")
                .with_context(&status.to_string()));
        }
        if status == 0 && archive.error().is_none() {
            unsafe { self.bytes.set_len(count as usize) };
        }
        // The native GNull reader records ordinary I/O failure in ArIsError;
        // appLoadFileToArray returns its Close result rather than throwing it.
        Ok(())
    }
    fn close(&mut self) -> RuntimeResult<i32> {
        let archive = self.archive.as_mut().expect("opened binary archive");
        let closed = archive.close().is_ok();
        Ok(i32::from(closed && archive.error().is_none()))
    }
    fn destroy(&mut self) -> RuntimeResult<()> {
        self.archive.take();
        Ok(())
    }
}

struct ImportContext<'a> {
    access: CoreAccess<'a>,
    class: *mut UClass,
    parent: *mut UObject,
    name: i32,
    flags: u32,
    path: PathBuf,
    filename: *const u32,
    class_cursor: i32,
    binary: Vec<u8>,
    error: Option<RuntimeError>,
}
unsafe fn run_import(
    context: *mut ImportContext<'_>,
    operation: FO,
    input: usize,
) -> RuntimeResult<usize> {
    let result = unsafe {
        factory_exporter::execute(
            &Hp2FactoryExporterServices { call: import_leaf },
            operation,
            Hp2FactoryExporterArgs {
                native: context.cast(),
                input,
                indent: 0,
                no_replace: 0,
                prompt: 0,
            },
        )
    };
    if let Some(error) = unsafe { (*context).error.take() } {
        return Err(error);
    }
    result.map_err(|status| {
        RuntimeError::new("Canonical texture import failed").with_context(&status.to_string())
    })
}
unsafe extern "C" fn import_leaf(
    raw: *mut c_void,
    leaf: FL,
    a: usize,
    b: usize,
    out: *mut usize,
) -> i32 {
    let context = raw.cast::<ImportContext<'_>>();
    match unsafe { import_dispatch(context, leaf, a, b) } {
        Ok(value) => {
            unsafe { out.write(value) };
            0
        }
        Err(error) => {
            unsafe { (*context).error.get_or_insert(error) };
            1
        }
    }
}
unsafe fn import_dispatch(
    context: *mut ImportContext<'_>,
    leaf: FL,
    a: usize,
    b: usize,
) -> RuntimeResult<usize> {
    let access = unsafe { (*context).access };
    let factory = a as *mut FactoryRecord;
    match leaf {
        FL::CheckInput => {
            if unsafe { (*context).class.is_null() } {
                Err(RuntimeError::new("Import requires a class"))
            } else {
                Ok(0)
            }
        }
        FL::ClassCurrent => {
            let class_type = access.native_class("Core.Class")?;
            loop {
                let index = unsafe { (*context).class_cursor };
                if index >= access.object_count() {
                    return Ok(0);
                }
                let object = access.object_at(index);
                if !object.is_null()
                    && unsafe { hp2_core::object_model::is_a(object.cast(), class_type.cast()) }
                {
                    return Ok(object as usize);
                }
                unsafe {
                    (*context).class_cursor += 1;
                }
            }
        }
        FL::ClassAdvance => {
            unsafe { (*context).class_cursor += 1 };
            Ok(0)
        }
        FL::ClassDefault => {
            let base = access.native_class("Core.Factory")?;
            if unsafe { hp2_core::object_model::is_child_of(a as *mut c_void, base.cast()) } {
                Ok(unsafe { access.default_object(a as *mut UClass)? } as usize)
            } else {
                Ok(0)
            }
        }
        FL::Supported => {
            Ok(unsafe { (*factory).supported_class == (*context).class.cast() } as usize)
        }
        FL::Priority => Ok(unsafe { (*factory).auto_priority as usize }),
        FL::Construct => Ok(unsafe {
            access.construct_object(
                a as *mut UClass,
                access.transient_package(),
                0,
                0,
                ptr::null_mut(),
            )?
        } as usize),
        FL::Delete => {
            unsafe { access.delete_object(a as *mut UObject)? };
            Ok(0)
        }
        FL::CreateNew => Ok(unsafe { ((*factory).factory_bits & 1) != 0 } as usize),
        FL::IsText => Ok(unsafe { ((*factory).factory_bits & 8) != 0 } as usize),
        FL::FilenameEmpty => Ok(unsafe { *(*context).filename == 0 } as usize),
        FL::ImportLog => {
            let class_name = object_name(access, unsafe { (*context).class.cast() })?;
            let factory_class = unsafe { (*factory).base.class.cast::<UObject>() };
            let factory_name = object_name(access, factory_class)?;
            unsafe {
                log_parts(
                    access,
                    "Log",
                    &[
                        TextPart::L(if b == 0 {
                            "FactoryCreateNew: "
                        } else if b == 1 {
                            "FactoryCreateText: "
                        } else {
                            "FactoryCreateBinary: "
                        }),
                        TextPart::P(class_name.as_ptr()),
                        TextPart::L(" with "),
                        TextPart::P(factory_name.as_ptr()),
                        TextPart::L(" ("),
                        TextPart::I(((*factory).factory_bits & 1 != 0) as i32),
                        TextPart::L(" "),
                        TextPart::I(((*factory).factory_bits & 8 != 0) as i32),
                        TextPart::L(" "),
                        TextPart::P((*context).filename),
                        TextPart::L(")"),
                    ],
                )?
            };
            Ok(0)
        }
        // ImportObject passes NULL Parms; UObject::ParseParms has no input to parse.
        FL::Parse => Ok(0),
        FL::BinaryScope => unsafe { run_import(context, FO::ImportBinary, a) },
        FL::LoadBinary => {
            let mut load = BinaryFileLoad {
                access,
                path: unsafe { &(*context).path },
                archive: None,
                bytes: unsafe { std::mem::take(&mut (*context).binary) },
            };
            let loaded = unsafe { hp2_core::file_persistence::load_array_with(&mut load)? };
            unsafe { (*context).binary = load.bytes };
            Ok(loaded as usize)
        }
        FL::BinarySentinel => {
            unsafe { (*context).binary.push(0) };
            Ok(0)
        }
        FL::InvokeBinaryImport => {
            let class = access.native_class("Editor.EngineTextureFactory")?;
            if !unsafe { hp2_core::object_model::is_a(factory.cast(), class.cast()) } {
                // UFactory's inherited FactoryCreateBinary is source-empty.
                return Ok(0);
            }
            let mut image = ImageContext::new(access, ptr::null_mut());
            unsafe {
                image.image = (*context).binary.as_ptr();
                image.image_len = ((*context).binary.len() - 1) as i32;
                image.cursor = image.image;
                image.class = (*context).class;
                image.parent = (*context).parent;
                image.name = (*context).name;
                image.flags = (*context).flags;
            }
            unsafe { run_image(&raw mut image, O::Factory, 0, 0) }
        }
        // UTexture inherits UObject::ImportPostProcess, whose source body is empty.
        FL::PostProcess => Ok(0),
        FL::CheckResult => {
            if unsafe { hp2_core::object_model::is_a(a as *mut c_void, (*context).class.cast()) } {
                Ok(0)
            } else {
                Err(RuntimeError::new("Imported object has the wrong class"))
            }
        }
        FL::ImportMissing => {
            let template = access.localize_text(
                &[69, 114, 114, 111, 114],
                &[78, 111, 70, 105, 110, 100, 73, 109, 112, 111, 114, 116],
                Some(&[67, 111, 114, 101]),
                None,
                false,
            )?;
            let filename = unsafe { (*context).filename };
            let mut length = 0;
            while unsafe { *filename.add(length) } != 0 {
                length += 1;
            }
            let filename = unsafe { std::slice::from_raw_parts(filename, length) };
            let mut message = Vec::with_capacity(template.len() + filename.len());
            let mut index = 0;
            while index < template.len() {
                if template[index] == 37 && template.get(index + 1) == Some(&115) {
                    message.extend_from_slice(filename);
                    index += 2;
                } else {
                    message.push(template[index]);
                    index += 1;
                }
            }
            let category = access.intern_str("Log", FindName::Add)?;
            access.emit_log(category, &message)?;
            Ok(0)
        }
        _ => Err(RuntimeError::new(
            "Invalid canonical texture-import operation",
        )),
    }
}

/// The source ImportObject call creates/uses SavePics, then runs the UFactory
/// selection and binary-import transaction (not UTexture::LoadFromBMP).
pub(crate) unsafe fn from_bmp(
    access: CoreAccess<'_>,
    name: *const TArrayRaw,
    text: *const TArrayRaw,
) -> RuntimeResult<*mut UObject> {
    let package_class = access.native_class("Core.Package")?;
    let package =
        match unsafe { access.find_object_by_name(package_class, hp2_core::lit!("SavePics"))? } {
            Some(value) => value,
            None => unsafe {
                access.construct_object(
                    package_class,
                    ptr::null_mut(),
                    access.intern_str("SavePics", FindName::Add)?,
                    0,
                    ptr::null_mut(),
                )?
            },
        };
    let name = unsafe { native_text::code_units(native_text::fstring_units(name)) };
    let name = access.intern(name, FindName::Add)?;
    let filename = unsafe { native_text::fstring_text(text) };
    let path = path_from_string(access, text)?;
    let mut context = ImportContext {
        access,
        class: access.native_class("Engine.Texture")?,
        parent: package,
        name,
        flags: hp2_core::class_metadata::METADATA_RF_PUBLIC_STANDALONE,
        path,
        filename,
        class_cursor: 0,
        binary: Vec::new(),
        error: None,
    };
    Ok(unsafe { run_import(&raw mut context, FO::Import, 0)? } as *mut UObject)
}

/// Runs the real viewport SNAP command, then the shared 128x64 LoadFromSnap policy.
pub(crate) unsafe fn from_screen_shot(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    viewport: *mut UViewport,
) -> RuntimeResult<*mut UObject> {
    if viewport.is_null() {
        let message = hp2_core::lit!("CreateTextureFromScreenShot: Missing Viewport");
        unsafe {
            hp2_core::canonical_vm_native::log_frame(access, frame, &message[..message.len() - 1])?
        };
        return Ok(ptr::null_mut());
    }
    let category = access.intern_str("Log", FindName::Add)?;
    unsafe {
        crate::camera_owner::exec(
            access,
            viewport,
            hp2_core::lit!("snap 8").as_ptr(),
            &mut |message| access.emit_log(category, message),
        )?;
    }
    let texture = unsafe {
        access.construct_object(
            access.native_class("Engine.Texture")?,
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )?
    };
    let mut context = ImageContext::new(access, texture.cast());
    context.viewport = viewport;
    unsafe { run_image(&raw mut context, O::Snap, 128, 64)? };
    Ok(texture)
}
