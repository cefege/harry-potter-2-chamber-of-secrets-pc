use hp2_abi::engine::generated::{AActor, FFrame, UFunction, UGameSaveInfo, UObject, UStruct};
use hp2_core::core_access::{CoreAccess, CoreOptions, RuntimeResult};
use hp2_engine::EngineOwner;
use std::{fs, path::PathBuf, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
fn wide(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}
struct LiveNames<'a>(CoreAccess<'a>);
impl hp2_core::runtime_archive::RuntimeNameMapping for LiveNames<'_> {
    fn name_text(&self, index: i32) -> Result<Vec<u32>, i32> {
        self.0.name_text(index).map_err(|_| 1)
    }
    fn add_name(&self, text: &[u32]) -> Result<i32, i32> {
        self.0
            .intern(text, hp2_core::name_owner::FindName::Add)
            .map_err(|_| 1)
    }
}
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe {
        access.construct_object(
            access.native_class(class).unwrap(),
            outer,
            0,
            0,
            ptr::null_mut(),
        )
    }
    .unwrap_or_else(|error| panic!("construct {class}: {error:?}"))
}
fn string_arg(value: &str) -> Vec<u8> {
    let mut code = vec![0x34];
    for unit in value.encode_utf16().chain([0]) {
        code.extend(unit.to_le_bytes());
    }
    code
}
unsafe fn object_arg(object: *mut UObject) -> Vec<u8> {
    if object.is_null() {
        vec![0x2a]
    } else {
        let mut code = vec![0x20];
        code.extend(unsafe { (*object).index }.to_le_bytes());
        code
    }
}
unsafe fn call_file(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    native: u32,
    path: &str,
    object: *mut UObject,
) -> RuntimeResult<bool> {
    unsafe {
        let function = construct(
            access,
            "Core.Function",
            (*actor.cast::<UObject>()).class.cast(),
        )
        .cast::<UFunction>();
        let mut code = string_arg(path);
        code.extend(object_arg(object));
        code.push(0x16);
        hp2_core::array_storage::set(
            hp2_core::array_storage::Hp2ArraySlots {
                data: &raw mut (*function.cast::<UStruct>()).script.data,
                num: &raw mut (*function.cast::<UStruct>()).script.num,
                max: &raw mut (*function.cast::<UStruct>()).script.max,
            },
            code.len() as i32,
            1,
            ptr::null(),
            hp2_core::array_storage::system_allocator(),
        )
        .unwrap();
        ptr::copy_nonoverlapping(
            code.as_ptr(),
            (*function.cast::<UStruct>()).script.data.cast(),
            code.len(),
        );
        let start = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut frame = FFrame {
            node: function.cast(),
            object: actor.cast(),
            code: start,
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        let mut result = 0xa5_u8;
        access.execute_vm_native_index(
            actor.cast(),
            native,
            &raw mut frame,
            (&raw mut result).cast(),
        )?;
        assert_eq!(
            frame.code,
            start.add(code.len()),
            "native {native} must consume both arguments and P_FINISH"
        );
        Ok(result != 0)
    }
}

#[test]
fn actor_file_natives_roundtrip_real_reflected_objects_and_save_info() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("artifact directory"))
        .join(format!("actor-file-transfer-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).unwrap();
    fs::create_dir_all(root.join("Slots")).unwrap();
    fs::write(root.join("System/Default.ini"), "[Engine.Engine]\n").unwrap();
    fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: Some(fs::File::create(root.join("runtime.log")).unwrap()),
    })
    .expect("construct canonical Engine graph");
    let access = owner.access();
    unsafe {
        let actor = construct(access, "Engine.Actor", access.transient_package()).cast::<AActor>();
        let info = construct(access, "Engine.GameSaveInfo", access.transient_package())
            .cast::<UGameSaveInfo>();
        (*info).num_beans = 17;
        (*info).num_stars = -3;
        (*info).num_points = i32::MIN + 9;
        (*info).save_point_id = 2048;
        access
            .assign_fstring(
                ptr::addr_of_mut!((*info).current_level_string),
                &wide("Level_Ω"),
            )
            .unwrap();
        let object_path = root.join("object.usa").to_string_lossy().into_owned();

        assert!(call_file(access, actor, 323, &object_path, info.cast()).unwrap());
        (*info).num_beans = 900;
        (*info).num_stars = 901;
        (*info).num_points = 902;
        (*info).save_point_id = 903;
        access
            .assign_fstring(
                ptr::addr_of_mut!((*info).current_level_string),
                &wide("mutated"),
            )
            .unwrap();
        assert!(call_file(access, actor, 324, &object_path, info.cast()).unwrap());
        assert_eq!(
            (*info).num_beans,
            17,
            "generic UObject::Serialize restores reflected consumer state"
        );
        assert_eq!((*info).num_stars, -3);
        assert_eq!((*info).num_points, i32::MIN + 9);
        assert_eq!((*info).save_point_id, 2048);
        assert_eq!(
            hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(ptr::addr_of!(
                (*info).current_level_string
            ),)),
            wide("Level_Ω")
                .into_iter()
                .take_while(|&unit| unit != 0)
                .collect::<Vec<_>>()
        );

        let system = access.system().unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*system).save_slot_path),
                &wide(root.join("Slots").to_string_lossy().as_ref()),
            )
            .unwrap();
        (*info).num_beans = 21;
        (*info).num_stars = 22;
        (*info).num_points = 23;
        (*info).save_point_id = 24;
        access
            .assign_fstring(
                ptr::addr_of_mut!((*info).current_level_string),
                &wide("Ch2Skurge_Ω"),
            )
            .unwrap();
        assert!(call_file(access, actor, 325, "progress.sav", info.cast()).unwrap());
        let wire = fs::read(root.join("Slots/progress.sav")).unwrap();
        let mut expected = vec![21, 0, 0, 0, 22, 0, 0, 0, 23, 0, 0, 0, 24, 0, 0, 0, 0x8c];
        for unit in "Ch2Skurge_Ω".encode_utf16().chain([0]) {
            expected.extend(unit.to_le_bytes());
        }
        assert_eq!(
            wire, expected,
            "GameSaveInfo source wire is four INTs followed by the FString archive encoding"
        );
        (*info).num_beans = 0;
        (*info).num_stars = 0;
        (*info).num_points = 0;
        (*info).save_point_id = 0;
        access
            .assign_fstring(
                ptr::addr_of_mut!((*info).current_level_string),
                &wide("changed"),
            )
            .unwrap();
        assert!(call_file(access, actor, 326, "progress.sav", info.cast()).unwrap());
        assert_eq!(
            (
                (*info).num_beans,
                (*info).num_stars,
                (*info).num_points,
                (*info).save_point_id
            ),
            (21, 22, 23, 24)
        );
        assert_eq!(
            hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(ptr::addr_of!(
                (*info).current_level_string
            ))),
            wide("Ch2Skurge_Ω")
                .into_iter()
                .take_while(|&unit| unit != 0)
                .collect::<Vec<_>>()
        );

        let absolute_leaf = root.join("absolute-leaf.sav");
        let source_join = root
            .join("Slots")
            .join(absolute_leaf.strip_prefix("/").unwrap());
        fs::create_dir_all(source_join.parent().unwrap()).unwrap();
        assert!(
            call_file(
                access,
                actor,
                325,
                absolute_leaf.to_str().unwrap(),
                info.cast()
            )
            .unwrap()
        );
        assert_eq!(
            fs::read(source_join).unwrap(),
            expected,
            "FString path concatenation retains SaveSlotPath even for an absolute-looking leaf"
        );
        access
            .assign_fstring(ptr::addr_of_mut!((*system).save_slot_path), &wide(""))
            .unwrap();
        assert!(call_file(access, actor, 325, "empty-slot.sav", info.cast()).unwrap());
        assert_eq!(
            fs::read(root.join("System/empty-slot.sav")).unwrap(),
            expected,
            "empty FString SaveSlotPath uses the canonical System working base"
        );
        access
            .assign_fstring(
                ptr::addr_of_mut!((*system).save_slot_path),
                &wide(root.join("Slots").to_str().unwrap()),
            )
            .unwrap();

        let relative = format!("system-relative-{}/object.usa", std::process::id());
        fs::create_dir_all(root.join("System").join(&relative).parent().unwrap()).unwrap();
        assert!(call_file(access, actor, 323, &relative, info.cast()).unwrap());
        (*info).num_beans = -99;
        assert!(call_file(access, actor, 324, &relative, info.cast()).unwrap());
        assert_eq!(
            (*info).num_beans,
            21,
            "relative object files round-trip through the canonical System directory"
        );

        let missing = root.join("missing.usa").to_string_lossy().into_owned();
        assert!(
            !call_file(access, actor, 324, &missing, info.cast()).unwrap(),
            "missing reader returns false"
        );
        let oversized = root.join("oversized.usa");
        fs::File::create(&oversized)
            .unwrap()
            .set_len(i32::MAX as u64 + 1)
            .unwrap();
        assert!(
            !call_file(access, actor, 324, oversized.to_str().unwrap(), info.cast()).unwrap(),
            "Unix manager rejects a captured reader size outside INT before object decoding"
        );
        fs::remove_file(oversized).unwrap();
        assert!(
            !call_file(access, actor, 323, &object_path, ptr::null_mut()).unwrap(),
            "null UObject returns false without opening a file"
        );
        fs::write(root.join("Slots/truncated.sav"), [0x01, 0x02, 0x03]).unwrap();
        assert!(
            call_file(access, actor, 326, "truncated.sav", info.cast()).unwrap(),
            "native GNull archive I/O flags do not change the opened-transfer return value"
        );
        assert_eq!(
            (
                (*info).num_beans,
                (*info).num_stars,
                (*info).num_points,
                (*info).save_point_id
            ),
            (21, 22, 23, 24),
            "the failed first INT read leaves initialized consumer fields unchanged"
        );
        assert!(
            hp2_core::native_text::fstring_units(ptr::addr_of!((*info).current_level_string))
                .is_empty(),
            "the source compact count initializes to zero after ArIsError"
        );
        assert!(call_file(access, actor, 325, "empty-info.sav", info.cast()).unwrap());
        let mut empty_wire = expected[..16].to_vec();
        empty_wire.push(0);
        assert_eq!(
            fs::read(root.join("Slots/empty-info.sav")).unwrap(),
            empty_wire,
            "native empty FString storage saves count zero with no terminator byte"
        );

        let mut archive = hp2_core::runtime_archive::RuntimeArchive::create(
            &root.join("gnull-exception-precedence.bin"),
        )
        .unwrap();
        archive.use_null_error_device();
        archive.seek(i32::MAX).unwrap();
        let mut word = 0x1234_i32;
        assert_eq!(
            archive.serialize((&raw mut word).cast(), 4),
            0,
            "source GNull records the INT-range I/O flag instead of throwing"
        );
        assert!(archive.error().is_some());
        assert_eq!(
            archive.tell().unwrap(),
            i32::MAX,
            "source Tell remains usable after an ordinary archive error"
        );
        let mut invalid_name = -1;
        assert!(
            archive
                .serialize_name_with(&raw mut invalid_name, &LiveNames(access))
                .is_err(),
            "an actual name-table failure still throws after an earlier GNull I/O flag"
        );
    }
}
