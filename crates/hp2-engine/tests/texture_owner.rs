use hp2_abi::engine::generated::{
    self, FColor, FMipmap, TArrayRaw, UClass, UObject, UPalette, UTexture,
};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult};
use hp2_core::lazy_array::{self, LazyArraySlots};
use hp2_core::name_owner::FindName;
use hp2_engine::EngineOwner;
use std::path::{Path, PathBuf};
use std::ptr;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn isolated_root() -> PathBuf {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR")
            .expect("HP2_ARTIFACT_DIR must name the pre-created test artifact directory"),
    );
    assert!(
        artifact.is_dir(),
        "HP2_ARTIFACT_DIR exists before the test starts"
    );
    let root = artifact.join(format!("texture-owner-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove this test's previous isolated runtime");
    }
    std::fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    std::fs::write(root.join("System/Default.ini"), "[Core.System]\nPaths=../System/*.u\n").expect("write default configuration");
    std::fs::write(root.join("System/DefUser.ini"), "").expect("write user configuration");
    let prototype = PathBuf::from(
        std::env::var_os("HP2_CORE_DATA_ROOT")
            .expect("HP2_CORE_DATA_ROOT must name the data-prototype runtime"),
    );
    for package in ["Core.u", "Engine.u"] {
        std::fs::copy(
            prototype.join("System").join(package),
            root.join("System").join(package),
        )
        .expect("copy actual prototype metadata before constructing texture classes");
    }
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

fn name(access: CoreAccess<'_>, value: &str) -> i32 {
    access
        .intern_str(value, FindName::Add)
        .expect("intern canonical fixture name")
}

fn utf32(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain([0]).collect()
}

unsafe fn set_values<T: Copy>(array: *mut TArrayRaw, values: &[T]) {
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*array).data) },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    };
    assert_eq!(
        unsafe { slots.num.read_unaligned() },
        0,
        "fresh native array is empty"
    );
    assert!(
        unsafe { slots.data.read_unaligned() }.is_null(),
        "fresh native array has no storage"
    );
    let count = i32::try_from(values.len()).expect("test array count fits native INT");
    if count == 0 {
        return;
    }
    unsafe { slots.max.write_unaligned(count) };
    unsafe {
        array_storage::realloc(
            slots,
            i32::try_from(std::mem::size_of::<T>()).expect("element width fits native INT"),
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .expect("allocate native array with Core's paired allocator");
    let storage = unsafe { slots.data.read_unaligned().cast::<T>() };
    for (index, value) in values.iter().copied().enumerate() {
        unsafe { storage.add(index).write(value) };
    }
    unsafe { slots.num.write_unaligned(count) };
}

unsafe fn array_values<T: Copy>(array: *const TArrayRaw) -> Vec<T> {
    let raw = unsafe { ptr::read_unaligned(array) };
    assert!(raw.num >= 0, "native array count is nonnegative");
    if raw.num == 0 {
        return Vec::new();
    }
    assert!(!raw.data.is_null(), "nonempty native array has storage");
    unsafe { std::slice::from_raw_parts(raw.data.cast::<T>(), raw.num as usize).to_vec() }
}

fn class(access: CoreAccess<'_>, path: &str) -> *mut UClass {
    access
        .native_class(path)
        .unwrap_or_else(|error| panic!("resolve {path}: {error}"))
}

fn texture_mip_slots(mip: *mut FMipmap) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mip).data_array) };
    LazyArraySlots {
        array: unsafe {
            lazy.cast::<u8>()
                .add(generated::TLazyBytes::TARRAY_BYTE_OFFSET)
                .cast()
        },
        loader: unsafe {
            lazy.cast::<u8>()
                .add(generated::TLazyBytes::FLAZYLOADER_OFFSET)
                .cast()
        },
        saved_archive: unsafe { ptr::addr_of_mut!((*mip).data_array.saved_ar) },
        saved_position: unsafe { ptr::addr_of_mut!((*mip).data_array.saved_pos) },
    }
}

fn set_mips(
    access: CoreAccess<'_>,
    mips_array: *mut TArrayRaw,
    records: &[(i32, i32, u8, u8, &[u8])],
) {
    let mut mips = Vec::with_capacity(records.len());
    for &(u_size, v_size, u_bits, v_bits, _) in records {
        let mut mip: FMipmap = unsafe { std::mem::zeroed() };
        mip.base.u_size = u_size;
        mip.base.v_size = v_size;
        mip.base.u_bits = u_bits;
        mip.base.v_bits = v_bits;
        mips.push(mip);
    }
    // Publish mip records only after each embedded lazy header and payload are initialized.
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*mips_array).data) },
        num: unsafe { ptr::addr_of_mut!((*mips_array).num) },
        max: unsafe { ptr::addr_of_mut!((*mips_array).max) },
    };
    let count = i32::try_from(mips.len()).expect("mip count fits native INT");
    if count != 0 {
        unsafe { slots.max.write_unaligned(count) };
        unsafe {
            array_storage::realloc(
                slots,
                i32::try_from(std::mem::size_of::<FMipmap>())
                    .expect("FMipmap width fits native INT"),
                ptr::null(),
                array_storage::system_allocator(),
            )
        }
        .expect("allocate FMipmap records with Core's paired allocator");
        let storage = unsafe { slots.data.read_unaligned().cast::<FMipmap>() };
        for (index, mip) in mips.into_iter().enumerate() {
            unsafe { storage.add(index).write(mip) };
        }
        for (index, &(_, _, _, _, bytes)) in records.iter().enumerate() {
            let mip = unsafe { storage.add(index) };
            let lazy = unsafe { ptr::addr_of_mut!((*mip).data_array) };
            let lazy_slots = texture_mip_slots(mip);
            unsafe { lazy_array::lazy_array_initialize(access, lazy_slots) }
                .expect("initialize actual TLazyBytes base fields");
            if !bytes.is_empty() {
                let raw = unsafe {
                    lazy.cast::<u8>()
                        .add(generated::TLazyBytes::TARRAY_BYTE_OFFSET)
                        .cast::<TArrayRaw>()
                };
                unsafe { set_values(raw, bytes) };
            }
        }
        unsafe { slots.num.write_unaligned(count) };
    }
}

#[test]
fn canonical_texture_graph_roundtrips_real_properties_and_deferred_mip_bodies() {
    let root = isolated_root();
    let saved_path = root.join("System/TextureGraph.u");
    let owner =
        EngineOwner::new(options(&root)).expect("initialize owner with prototype metadata present");
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, ..CoreProcessMode::default() });
    let metadata_flags = 1;
    unsafe { access.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("load actual Engine.Texture script metadata");
    let texture_class = unsafe {
        access.static_load_class(
            class(access, "Object"),
            ptr::null_mut(),
            &utf32("Engine.Texture"),
            1,
        )
    }
    .expect("materialize genuine Engine.Texture properties and defaults");
    let palette_class = class(access, "Engine.Palette");

    let package = unsafe {
        access.construct_object(
            class(access, "Package"),
            ptr::null_mut(),
            name(access, "TextureGraph"),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct canonical texture package");
    let palette = unsafe {
        access.construct_object(
            palette_class,
            package,
            name(access, "GraphPalette"),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct canonical palette")
    .cast::<UPalette>();
    let colors = [
        FColor {
            r: 17,
            g: 31,
            b: 47,
            a: 0,
        },
        FColor {
            r: 79,
            g: 97,
            b: 113,
            a: 0,
        },
    ];
    unsafe { set_values(ptr::addr_of_mut!((*palette).colors).cast(), &colors) };

    let texture = unsafe {
        access.construct_object(
            texture_class,
            package,
            name(access, "MipTexture"),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct genuine Engine.Texture")
    .cast::<UTexture>();
    let neighbor = unsafe {
        access.construct_object(
            texture_class,
            package,
            name(access, "NeighborTexture"),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct second texture export for bounded-body cursor coverage")
    .cast::<UTexture>();
    unsafe {
        (*texture).base.palette = palette.cast();
        (*texture).bump_map = neighbor.cast();
        (*texture).base.format = 1;
        (*texture).base.u_size = 4;
        (*texture).base.v_size = 2;
        (*texture).base.u_bits = 2;
        (*texture).base.v_bits = 1;
        (*texture).base.u_clamp = -3;
        (*texture).base.v_clamp = 7;
        (*texture).base.mip_zero = [colors[0].r, colors[0].g, colors[0].b, colors[0].a];
    }
    let mip0 = [3, 8, 13, 21, 34, 55, 89, 144];
    let mip1 = [201, 202];
    unsafe {
        set_mips(
            access,
            ptr::addr_of_mut!((*texture).mips),
            &[(4, 2, 2, 1, &mip0), (2, 1, 1, 0, &mip1)],
        );
    }
    let compressed = [240, 225, 210];
    unsafe {
        (*texture).comp_format = 0;
        (*texture).set_b_has_comp(true);
    }
    unsafe {
        set_mips(
            access,
            ptr::addr_of_mut!((*texture).comp_mips),
            &[(2, 1, 1, 0, &compressed)],
        );
    }
    let parametric = unsafe {
        access.construct_object(
            texture_class,
            package,
            name(access, "ProceduralTexture"),
            4,
            ptr::null_mut(),
        )
    }
    .expect("construct parametric texture export")
    .cast::<UTexture>();
    unsafe {
        (*parametric).base.u_size = 2;
        (*parametric).base.v_size = 2;
        (*parametric).base.u_clamp = -5;
        (*parametric).base.v_clamp = 9;
        (*parametric).set_b_parametric(true);
    }
    let procedural_source = [91, 72, 53, 34];
    unsafe {
        set_mips(
            access,
            ptr::addr_of_mut!((*parametric).mips),
            &[(2, 2, 1, 1, &procedural_source)],
        );
    }
    unsafe {
        access.add_root(package);
        access.add_root(texture.cast());
    }
    assert!(
        unsafe { access.save_package(package, texture.cast(), 4, &saved_path, None) }
            .expect("save canonical texture and its reflected palette graph")
    );
    let texture_index = unsafe { (*texture.cast::<UObject>()).index };
    let palette_index = unsafe { (*palette.cast::<UObject>()).index };
    access
        .collect_garbage(0, 0, &|_| Ok(()))
        .expect("collect while texture graph is rooted");
    assert_eq!(
        access.object_at(palette_index),
        palette.cast(),
        "rooted texture's reflected Palette reference retains its target"
    );
    access.remove_root(texture.cast());
    access
        .collect_garbage(0, 0, &|_| Ok(()))
        .expect("collect unrooted texture and palette graph");
    assert!(
        access.object_at(texture_index).is_null(),
        "unrooted texture is collectible"
    );
    access.remove_root(package);
    owner
        .shutdown()
        .expect("destroy canonical texture and mip storage");

    let restored_owner =
        EngineOwner::new(options(&root)).expect("initialize second real EngineOwner");
    let restored = restored_owner.access();
    restored.set_process_mode(CoreProcessMode {
        is_client: true,
        lazy_load: true,
        ..CoreProcessMode::default()
    });
    unsafe { restored.load_package(&root.join("System/Engine.u"), metadata_flags) }
        .expect("load real Engine metadata before resolving native classes");
    let restored_texture_class = unsafe {
        restored.static_load_class(
            class(restored, "Object"),
            ptr::null_mut(),
            &utf32("Engine.Texture"),
            1,
        )
    }
    .expect("materialize real Texture metadata in the reader owner");
    let restored_palette_class = class(restored, "Engine.Palette");
    unsafe { restored.load_package(&saved_path, metadata_flags) }
        .expect("open saved texture package");
    let neighbor_path = utf32("TextureGraph.NeighborTexture");
    let loaded_neighbor = unsafe {
        restored.static_load_object(restored_texture_class, ptr::null_mut(), &neighbor_path, 1)
    }
    .expect("load neighboring export before materializing the first mip body");
    let texture_path = utf32("TextureGraph.MipTexture");
    let loaded = unsafe {
        restored.static_load_object(restored_texture_class, ptr::null_mut(), &texture_path, 1)
    }
    .expect("load genuine Texture export")
    .cast::<UTexture>();
    assert_eq!(
        unsafe { (*loaded).bump_map },
        loaded_neighbor.cast(),
        "reflected Texture-to-Texture property resolves to the canonical loaded export"
    );
    let parametric_path = utf32("TextureGraph.ProceduralTexture");
    let loaded_parametric = unsafe {
        restored.static_load_object(restored_texture_class, ptr::null_mut(), &parametric_path, 1)
    }
    .expect("load parametric Texture export")
    .cast::<UTexture>();
    assert!(
        unsafe { (*loaded_parametric).b_parametric() },
        "parametric property survives package reload"
    );
    assert_eq!(
        unsafe { (*loaded_parametric).base.u_clamp },
        0,
        "PostLoad clamps negative UClamp"
    );
    assert_eq!(
        unsafe { (*loaded_parametric).base.v_clamp },
        2,
        "PostLoad clamps VClamp to VSize"
    );
    let generated_palette = unsafe { (*loaded_parametric).base.palette.cast::<UPalette>() };
    assert!(
        !generated_palette.is_null(),
        "PostLoad creates a missing palette"
    );
    let gray = unsafe { array_values::<FColor>(ptr::addr_of!((*generated_palette).colors)) };
    assert_eq!(
        gray.len(),
        256,
        "missing palette is filled with native grayscale values"
    );
    assert_eq!([gray[0].r, gray[0].g, gray[0].b, gray[0].a], [0, 0, 0, 0]);
    assert_eq!(
        [gray[255].r, gray[255].g, gray[255].b, gray[255].a],
        [255, 255, 255, 0]
    );
    let parametric_mips =
        unsafe { array_values::<FMipmap>(ptr::addr_of!((*loaded_parametric).mips)) };
    assert_eq!(parametric_mips.len(), 1);
    let parametric_slots = texture_mip_slots(unsafe {
        ptr::addr_of_mut!((*loaded_parametric).mips.data)
            .read_unaligned()
            .cast::<FMipmap>()
    });
    assert_eq!(
        unsafe { array_values::<u8>(parametric_slots.array) },
        [0, 0, 0, 0],
        "parametric reload discards source data and recreates U*V zeroes"
    );
    assert_eq!(unsafe { (*loaded).base.format }, 1);
    assert_eq!(unsafe { (*loaded).base.u_size }, 4);
    assert_eq!(unsafe { (*loaded).base.v_size }, 2);
    assert_eq!(unsafe { (*loaded).base.u_bits }, 2);
    assert_eq!(unsafe { (*loaded).base.v_bits }, 1);
    assert!(
        unsafe { (*loaded).b_has_comp() },
        "compressed-mip script bit survives loading"
    );
    assert_eq!(
        unsafe { (*loaded).base.u_clamp },
        0,
        "PostLoad clamps negative UClamp"
    );
    assert_eq!(
        unsafe { (*loaded).base.v_clamp },
        2,
        "PostLoad clamps VClamp to VSize"
    );
    let loaded_palette = unsafe { (*loaded).base.palette.cast::<UPalette>() };
    assert_eq!(
        unsafe { (*loaded_palette.cast::<UObject>()).class },
        restored_palette_class.cast()
    );
    assert_eq!(
        unsafe { (*loaded_palette.cast::<UObject>()).name },
        name(restored, "GraphPalette").to_ne_bytes()
    );
    let loaded_colors = unsafe { array_values::<FColor>(ptr::addr_of!((*loaded_palette).colors)) };
    assert_eq!(loaded_colors.len(), colors.len());
    assert_eq!(
        loaded_colors
            .iter()
            .map(|c| [c.r, c.g, c.b, c.a])
            .collect::<Vec<_>>(),
        [[17, 31, 47, 0], [79, 97, 113, 0]],
        "palette entries and alpha-zero RGBA bytes survive the reflected reference"
    );
    let loaded_mips = unsafe { array_values::<FMipmap>(ptr::addr_of!((*loaded).mips)) };
    assert_eq!(
        loaded_mips.len(),
        2,
        "normal mip count and ordering survive archive reload"
    );
    let live_mips = unsafe {
        ptr::addr_of_mut!((*loaded).mips.data)
            .read_unaligned()
            .cast::<FMipmap>()
    };
    for (index, expected) in [mip0.as_slice(), mip1.as_slice()].into_iter().enumerate() {
        let mip = unsafe { live_mips.add(index) };
        assert_eq!(unsafe { (*mip).base.u_size }, [4, 2][index]);
        assert_eq!(unsafe { (*mip).base.v_size }, [2, 1][index]);
        assert_eq!(unsafe { (*mip).base.u_bits }, [2, 1][index]);
        assert_eq!(unsafe { (*mip).base.v_bits }, [1, 0][index]);
        let slots = texture_mip_slots(mip);
        let saved_pos = unsafe { slots.saved_position.read_unaligned() };
        assert!(
            saved_pos > 0,
            "mip body remains deferred at its actual source position"
        );
        assert_eq!(
            unsafe { slots.array.read_unaligned().num },
            0,
            "mip data remains unallocated until requested"
        );
        unsafe { lazy_array::lazy_array_load(restored, slots) }
            .expect("materialize bounded deferred mip body");
        let got = unsafe { array_values::<u8>(slots.array) };
        assert_eq!(got, expected, "mip {index} bytes preserve their body order");
        unsafe { lazy_array::lazy_array_unload(restored, slots, None) }
            .expect("unload materialized byte mip");
        unsafe { lazy_array::lazy_array_load(restored, slots) }
            .expect("reload mip from its original saved position");
        assert_eq!(
            unsafe { array_values::<u8>(slots.array) },
            expected,
            "mip {index} reloads identically"
        );
    }
    let compressed_mips = unsafe { array_values::<FMipmap>(ptr::addr_of!((*loaded).comp_mips)) };
    assert_eq!(
        compressed_mips.len(),
        1,
        "compressed mip chain survives archive reload"
    );
    let compressed_live = unsafe {
        ptr::addr_of_mut!((*loaded).comp_mips.data)
            .read_unaligned()
            .cast::<FMipmap>()
    };
    assert_eq!(unsafe { (*compressed_live).base.u_size }, 2);
    assert_eq!(unsafe { (*compressed_live).base.v_size }, 1);
    let compressed_slots = texture_mip_slots(compressed_live);
    assert!(
        unsafe { compressed_slots.saved_position.read_unaligned() } > 0,
        "compressed mip body is independently deferred"
    );
    unsafe { lazy_array::lazy_array_load(restored, compressed_slots) }
        .expect("materialize compressed mip body through its bounded source");
    assert_eq!(
        unsafe { array_values::<u8>(compressed_slots.array) },
        compressed
    );
    unsafe { lazy_array::lazy_array_destroy(restored, compressed_slots) }
        .expect("detach compressed mip source before owner teardown");
    // Explicitly detach source registrations before the owner's ordinary object teardown.
    for index in 0..loaded_mips.len() {
        let mip = unsafe { live_mips.add(index) };
        unsafe { lazy_array::lazy_array_destroy(restored, texture_mip_slots(mip)) }
            .expect("detach initialized lazy mip source");
    }
    restored.remove_root(loaded.cast());
    restored_owner
        .shutdown()
        .expect("destroy reloaded texture graph with mip bodies initialized");
}
