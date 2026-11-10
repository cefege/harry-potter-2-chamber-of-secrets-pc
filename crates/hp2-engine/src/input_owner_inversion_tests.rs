use super::*;

#[test]
fn package_inversion_handler_repairs_only_persistence_operand() {
    let owner = crate::render_owner::render_test_support::owner("stock-inversion-probe");
    let access = owner.access();
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("render-stock-inversion-probe-{}", std::process::id()));
    unsafe {
        access.load_package(&root.join("System/HGame.u"), 1).unwrap();
        let name: Vec<u32> = "HGame.FEInputPage".chars().map(u32::from).chain([0]).collect();
        let class = access.static_load_class(access.native_class("Core.Object").unwrap(), ptr::null_mut(), &name, 1).unwrap();
        let function = access.find_field(class.cast(), access.intern_str("MouseInvertChanged", FindName::Find).unwrap(), access.native_class("Core.Function").unwrap()).cast::<UStruct>();
        assert!(!function.is_null());
        let script = (*function).script;
        assert!(script.num > 0 && !script.data.is_null());
        let bytes = std::slice::from_raw_parts(script.data.cast::<u8>(), script.num as usize);
        assert_eq!(bytes.len(), 154);
        let mut fields = Vec::new();
        for key in ["MouseInvertCheck", "AutoCenterCamCheck"] {
            let field = access.find_field(class.cast(), access.intern_str(key, FindName::Find).unwrap(), access.native_class("Core.ObjectProperty").unwrap());
            assert!(!field.is_null(), "missing {key}");
            fields.push((*field.cast::<UObject>()).index);
        }
        let get_index = |at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(get_index(17), fields[0], "live inversion operand");
        assert_eq!(get_index(92), fields[0], "persistence operand must use the invert checkbox");
        assert_ne!(fields[0], fields[1]);
        assert_eq!(get_index(137), fields[0], "logging operand");
    }
}
