//! Strict canonical Engine package acceptance for the G1 prototype gate.
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_engine::EngineOwner;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    ptr,
};

fn output(access: CoreAccess<'_>, category: i32, text: &[u32]) -> RuntimeResult<()> {
    access.write_log_record(&access.name_text(category)?, text)
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
fn utf32(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}

fn prototype_owner(label: &str) -> (EngineOwner, PathBuf) {
    let data = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype data root"));
    let root =
        PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifact directory"))
            .join(format!("canonical-engine-{label}-{}", std::process::id()));
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
    let mut log = fs::File::create(root.join("runtime.log")).unwrap();
    log.write_all(&[0xff, 0xfe]).unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: utf32("-CONFLICTS"),
        output,
        log_file: Some(log),
    })
    .expect("construct canonical Engine graph");
    let access = owner.access();
    access.set_process_mode(CoreProcessMode {
        is_client: true,
        is_server: true,
        is_scriptable: true,
        ..access.process_mode()
    });
    (owner, root)
}

#[test]
fn every_prototype_script_package_loads_into_the_canonical_engine() {
    let (owner, root) = prototype_owner("packages");
    let access = owner.access();
    let mut packages: Vec<_> = fs::read_dir(root.join("System"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "u"))
        .collect();
    packages.sort_by(|a, b| {
        a.file_name()
            .unwrap()
            .as_encoded_bytes()
            .cmp(b.file_name().unwrap().as_encoded_bytes())
    });
    let engine = root.join("System/Engine.u");
    assert!(packages.contains(&engine), "prototype Engine.u must exist");
    packages.retain(|path| path != &engine);
    packages.insert(0, engine);
    let mut report = String::from("data_profile=data-prototype\n");
    for path in packages {
        // LOAD_NoFail only. Metadata-only VERIFY/NO_VERIFY would hide missing natives.
        match unsafe { access.load_package(&path, 1) } {
            Ok(_) => report.push_str(&format!(
                "{}=ok\n",
                path.file_name().unwrap().to_str().unwrap()
            )),
            Err(error) => {
                report.push_str(&format!(
                    "{}=fail\n{error}\n",
                    path.file_name().unwrap().to_str().unwrap()
                ));
                fs::write(root.join("report.txt"), &report).unwrap();
                panic!("strict canonical package {}: {error}", path.display());
            }
        }
    }
    let function_class = access.native_class("Core.Function").unwrap();
    for index in 0..access.object_count() {
        let object = access.object_at(index);
        if !unsafe { hp2_core::object_model::is_a(object.cast(), function_class.cast()) } {
            continue;
        }
        let function = object.cast::<hp2_abi::engine::generated::UStruct>();
        let mut property = unsafe { (*function).constructor_link.cast::<hp2_abi::engine::generated::UProperty>() };
        while !property.is_null() {
            unsafe {
                assert_eq!((*property.cast::<hp2_abi::engine::generated::UObject>()).outer, object.cast(),
                    "function {index} must not construct a super-function's local property");
                let extent = (*property).element_size.checked_mul((*property).array_dim).unwrap();
                let end = (*property).offset.checked_add(extent).unwrap();
                assert!((*property).offset >= 0 && extent >= 0 && end <= (*function).properties_size,
                    "function {index} constructor property must fit its own locals");
                property = (*property).constructor_link_next.cast();
            }
        }
    }
    let enumeration = access.native_class("Core.Enum").unwrap();
    assert!(
        unsafe { access.find_object_by_name(enumeration, &utf32("Engine.Actor.EInputKey")) }
            .unwrap()
            .is_some(),
        "Engine.Actor.EInputKey must be loaded as a Core.Enum"
    );
    unsafe {
        access.construct_object(
            access.native_class("Engine.GameEngine").unwrap(),
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )
    }
    .expect("construct Engine.GameEngine after strict package binding");
    owner.shutdown().expect("shutdown loaded canonical graph");
    fs::write(root.join("report.txt"), report).unwrap();
}

#[test]
fn player_pawn_mouse_history_offsets_match_prototype_reflection() {
    use hp2_abi::engine::generated::{APlayerPawn, UProperty};
    let (owner, root) = prototype_owner("mouse-history-layout");
    let access = owner.access();
    unsafe { access.load_package(&root.join("System/Engine.u"), 1) }
        .expect("load prototype Engine.u");
    let float_class = access.native_class("Core.FloatProperty").unwrap();
    let fields = [
        ("SmoothMouseX", std::mem::offset_of!(APlayerPawn, smooth_mouse_x)),
        ("SmoothMouseY", std::mem::offset_of!(APlayerPawn, smooth_mouse_y)),
        ("BorrowedMouseX", std::mem::offset_of!(APlayerPawn, borrowed_mouse_x)),
        ("BorrowedMouseY", std::mem::offset_of!(APlayerPawn, borrowed_mouse_y)),
    ];
    let mut report = String::from("data_profile=data-prototype\n");
    for (name, expected) in fields {
        let full_name = utf32(&format!("Engine.PlayerPawn.{name}"));
        let property = unsafe { access.find_object_by_name(float_class, &full_name) }
            .unwrap().unwrap_or_else(|| panic!("missing reflected float {name}"))
            .cast::<UProperty>();
        let offset = unsafe { (*property).offset };
        report.push_str(&format!("{name}: reflected={offset} abi={expected}\n"));
        fs::write(root.join("mouse-history-offsets.txt"), &report).unwrap();
        assert_eq!(offset, expected as i32, "{name} offset");
    }
    owner.shutdown().expect("shutdown canonical graph");
}

#[test]
fn explicit_map_filename_uses_the_existing_package_and_checks_cached_guid() {
    use hp2_abi::engine::generated::{AActor, ULinkerLoad, UObject};
    let (owner, root) = prototype_owner("map-filename");
    let access = owner.access();
    unsafe {
        let info = access.static_load_object_from_file(
            access.native_class("Engine.LevelInfo").unwrap(), ptr::null_mut(),
            &utf32("LevelInfo0"), &utf32("Entry.unr"), 1,
        ).expect("load LevelInfo0 from an explicit map filename");
        let package = (*info).outer.cast::<UObject>();
        assert!((*package).outer.is_null(), "file-backed packages use the same top-level namespace as native packages");
        let level = access.static_load_object_from_file(
            access.native_class("Engine.Level").unwrap(), package,
            &utf32("MyLevel"), &utf32("Entry.unr"), 1,
        ).expect("load MyLevel into its supplied package, not a nested duplicate");
        assert_eq!((*level).outer.cast::<UObject>(), package);
        let actor_class = access.native_class("Engine.Actor").unwrap();
        let mut index = -1;
        access.next_object_index(&mut index, actor_class).unwrap();
        while index < access.object_count() {
            let actor = access.object_at(index).cast::<AActor>();
            if (*actor.cast::<UObject>()).outer.cast::<UObject>() == package {
                let x_level = ptr::addr_of!((*actor).x_level).read_unaligned();
                assert_eq!(x_level, level.cast(), "level PostLoad retargets every actor in its package");
            }
            access.next_object_index(&mut index, actor_class).unwrap();
        }
        let linker = (*level)._linker.cast::<ULinkerLoad>();
        assert!(!linker.is_null(), "loaded level retains its canonical linker");
        let guid = ptr::addr_of!((*linker).base.summary.guid).read_unaligned();
        let filename = root.join("Maps/Entry.unr");
        assert_eq!(
            access.get_package_linker(&filename, 1, package, Some(&guid)).unwrap(),
            linker.cast(),
        );
        let mut wrong_guid = guid;
        wrong_guid[0] ^= 1;
        access.get_package_linker(&filename, 1, package, Some(&wrong_guid))
            .expect_err("a cached linker must not bypass GUID validation");
        let buffer_class = access.native_class("Core.TextBuffer").unwrap();
        let collision_name = access.intern_str("TextBuffer123", FindName::Add).unwrap();
        access.construct_object(buffer_class, package, collision_name, 0x4000, ptr::null_mut()).unwrap();
        (*buffer_class).class_unique = 123;
        let first = access.construct_object(buffer_class, package, 0, 0x4000, ptr::null_mut()).unwrap();
        let second = access.construct_object(buffer_class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap();
        assert_eq!(
            access.name_text(i32::from_ne_bytes((*first).name)).unwrap(), "TextBuffer124".chars().map(u32::from).collect::<Vec<_>>(),
            "source naming skips a collision without adding a separator",
        );
        assert_eq!(
            access.name_text(i32::from_ne_bytes((*second).name)).unwrap(), "TextBuffer125".chars().map(u32::from).collect::<Vec<_>>(),
            "the native class counter continues across different outers",
        );
    }
    owner.shutdown().expect("shutdown explicit-filename map graph");
}

#[test]
fn browse_keeps_caller_urls_owned_and_applies_restart_link_and_invalid_url_policy() {
    use hp2_abi::engine::generated::{FURL, TArrayRaw, UGameEngine};
    use hp2_engine::{game_travel_owner, url_owner::UrlOwner};
    use std::mem::MaybeUninit;

    unsafe fn string_units(value: &TArrayRaw) -> &[u32] {
        if value.num <= 1 { &[] }
        else { unsafe { std::slice::from_raw_parts(value.data.cast(), value.num as usize - 1) } }
    }
    let (owner, root) = prototype_owner("browse");
    let access = owner.access();
    unsafe {
        access.load_package(&root.join("System/Engine.u"), 1).unwrap();
        UrlOwner::static_init(access).unwrap();
        let defaults = UrlOwner::current_defaults(access).unwrap();
        let engine = access.construct_object(
            access.native_class("Engine.GameEngine").unwrap(), access.transient_package(),
            0, 0, ptr::null_mut(),
        ).unwrap().cast::<UGameEngine>();
        let mut url = MaybeUninit::<FURL>::uninit();
        UrlOwner::construct_default(access, &defaults, url.as_mut_ptr()).unwrap();
        let mut url = url.assume_init();
        let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };

        UrlOwner::parse_update(
            access, &defaults, ptr::addr_of_mut!((*engine).last_url), ptr::null_mut(),
            utf32("http://example.invalid/restarted").as_ptr(), 0,
        ).unwrap();
        UrlOwner::parse_update(
            access, &defaults, &mut url, ptr::null_mut(),
            utf32("OriginalMap?restart#OriginalPortal").as_ptr(), 0,
        ).unwrap();
        let mut before = UrlOwner::string(access, &defaults, &url, true).unwrap();
        assert!(!game_travel_owner::browse(access, engine, &url, ptr::null(), &mut error).unwrap());
        let mut after = UrlOwner::string(access, &defaults, &url, true).unwrap();
        assert_eq!(string_units(&before), string_units(&after), "restart replaces Browse's by-value URL, not the caller's owned strings/options");
        assert!(string_units(&error).is_empty(), "the source external-URL path does not manufacture an error");
        access.destroy_owned_fstring(&mut before).unwrap();
        access.destroy_owned_fstring(&mut after).unwrap();

        let invalid_map = utf32("DistinctInvalidTravelMap");
        UrlOwner::parse_update(access, &defaults, &mut url, ptr::null_mut(), invalid_map.as_ptr(), 0).unwrap();
        url.valid = 0;
        assert!(!game_travel_owner::browse(access, engine, &url, ptr::null(), &mut error).unwrap());
        assert!(string_units(&error).windows(invalid_map.len() - 1).any(|part| part == &invalid_map[..invalid_map.len() - 1]), "invalid-URL diagnostics include the rejected URL argument");
        let valid = url.valid;
        assert_eq!(valid, 0, "Browse does not repair the caller's invalid URL in place");

        let link = root.join("System/canonical-travel.unreal");
        let link_units = utf32(link.to_str().unwrap());
        access.assign_fstring(ptr::addr_of_mut!(url.map), &link_units).unwrap();
        url.valid = 1;
        assert!(!game_travel_owner::browse(access, engine, &url, ptr::null(), &mut error).unwrap());
        assert!(string_units(&error).windows(link_units.len() - 1).any(|part| part == &link_units[..link_units.len() - 1]), "a missing link reports its filename instead of attempting LoadMap");

        let usable_link = root.join("System/canonical-travel-usable.unreal");
        // Native config Find appends .ini to extensions longer than four units.
        fs::write(usable_link.with_extension("unreal.ini"), "[Link]\nServer=http://example.invalid/linked\n").unwrap();
        let usable_units = utf32(usable_link.to_str().unwrap());
        access.assign_fstring(ptr::addr_of_mut!(url.map), &usable_units).unwrap();
        assert!(!game_travel_owner::browse(access, engine, &url, ptr::null(), &mut error).unwrap());
        assert!(string_units(&error).is_empty(), "a valid file-backed link clears the preceding failure and follows its external destination");
        assert_eq!(string_units(&url.map), &usable_units[..usable_units.len() - 1], "link parsing owns a separate URL");

        UrlOwner::parse_update(access, &defaults, &mut url, ptr::null_mut(), utf32("UnusedMap?pop").as_ptr(), 0).unwrap();
        assert!(!game_travel_owner::browse(access, engine, &url, ptr::null(), &mut error).unwrap(), "pop without a current hub level does not load a map");
        assert!(string_units(&error).is_empty());
        access.destroy_owned_fstring(&mut error).unwrap();
        UrlOwner::destroy(access, &mut url).unwrap();
    }
    owner.shutdown().expect("shutdown canonical Browse graph without double-freeing caller URL members");
}
