//! Real-package loader contract for the retail confirmation dialog.
use hp2_core::name_owner::FindName;
use std::ptr;

#[test]
#[ignore = "requires verified retail data in HP2_CORE_DATA_ROOT"]
fn retail_dialog_textures_resolve_through_nonexistent_group() {
    let owner = crate::render_owner::render_test_support::owner("retail-dialog-textures");
    let access = owner.access();
    let texture = access.native_class("Engine.Texture").unwrap();
    for name in ["Hp2ConfirmLoad", "Hp2ConfirmYes", "Hp2ConfirmYes_Highlight"] {
        let path: Vec<u32> = format!("HP2_Menu.icon.{name}").chars().map(u32::from).collect();
        let object = unsafe { access.static_load_object(texture, ptr::null_mut(), &path, 1) }.unwrap();
        assert!(!object.is_null(), "retail object {name} must resolve despite the group spelling");
        let actual = access.intern_str(name, FindName::Find).unwrap();
        assert_eq!(i32::from_ne_bytes(unsafe { (*object).name }), actual);
        let outer = unsafe { (*object).outer.cast::<hp2_abi::engine::generated::UObject>() };
        let icons = access.intern_str("Icons", FindName::Find).unwrap();
        assert_eq!(i32::from_ne_bytes(unsafe { (*outer).name }), icons, "return the genuine texture in its genuine group");
    }
}
