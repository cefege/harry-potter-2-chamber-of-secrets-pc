use super::*;
use hp2_abi::engine::generated::{ALevelInfo, APlayerPawn, UBoolProperty, ULevel};

fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).chain([0]).collect() }

#[test]
fn prototype_compiled_autojump_and_invert_mouse_save_pawn_config() {
    let owner = crate::render_owner::render_test_support::owner("stock-input-gameplay");
    let access = owner.access();
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
        .join(format!("render-stock-input-gameplay-{}", std::process::id()));
    unsafe {
        let pawn = crate::render_owner::render_test_support::construct(access, "Engine.Camera").cast::<APlayerPawn>();
        let level = crate::render_owner::render_test_support::construct(access, "Engine.Level").cast::<ULevel>();
        let info = crate::render_owner::render_test_support::construct(access, "Engine.LevelInfo").cast::<ALevelInfo>();
        (*info).set_b_begun_play(true);
        (*pawn.cast::<AActor>()).level = info.cast();
        (*pawn.cast::<AActor>()).x_level = level.cast();
        let class = (*pawn.cast::<UObject>()).class;
        let property = |name: &str| {
            let id = access.intern_str(name, FindName::Find).unwrap();
            let prop = access.find_field(class.cast(), id, access.native_class("Core.BoolProperty").unwrap()).cast::<UBoolProperty>();
            assert!(!prop.is_null(), "missing {name}");
            prop
        };
        let invert = property("bInvertMouse");
        let read_invert = || {
            let p = invert.cast::<UProperty>();
            let word = pawn.cast::<u8>().add((*p).offset as usize).cast::<u32>().read_unaligned();
            word & (*invert).bit_mask != 0
        };
        let mut sink = |_: &[u32]| Ok(());
        for (value, expected) in [("True", true), ("False", false)] {
            assert!(crate::engine_exec_owner::process_console_exec(access, pawn.cast(), pawn.cast(), wide(&format!("AutoJump {value}")).as_ptr(), &mut sink).unwrap());
            assert_eq!((*pawn).b_auto_jump(), expected);
            assert!(crate::engine_exec_owner::process_console_exec(access, pawn.cast(), pawn.cast(), wide(&format!("InvertMouse {value}")).as_ptr(), &mut sink).unwrap());
            assert_eq!(read_invert(), expected);
            let mut reopened = hp2_core::CoreConfig::new(root.clone(), root.join("System/Default.ini"), root.join("System/DefUser.ini"));
            for key in ["bAutoJump", "bInvertMouse"] {
                let cached = access.config_get_units_from(Some(&wide("User")[..4]), &wide("Engine.PlayerPawn")[..17], &wide(key)[..key.len()]).unwrap().unwrap();
                let disk = reopened.get(Some(&wide("User")[..4]), &wide("Engine.PlayerPawn")[..17], &wide(key)[..key.len()]).unwrap().unwrap();
                let text: String = cached.into_iter().map(|c| char::from_u32(c).unwrap()).collect();
                let disk_text: String = disk.into_iter().map(|c| char::from_u32(c).unwrap()).collect();
                assert_eq!(text.to_ascii_lowercase(), value.to_ascii_lowercase(), "cached {key}");
                assert_eq!(disk_text.to_ascii_lowercase(), value.to_ascii_lowercase(), "persisted {key}");
            }
        }
    }
}
