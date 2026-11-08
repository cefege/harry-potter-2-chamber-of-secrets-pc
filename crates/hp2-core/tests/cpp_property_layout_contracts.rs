use hp2_abi::engine::generated::{FName, TArrayRaw};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{mem::{offset_of, size_of}, path::PathBuf, ptr};

#[repr(C)]
struct AliasValue {
    leading: [u64; 2],
    alias: FName,
    middle: [u32; 3],
    command: TArrayRaw,
    trailing: [u64; 2],
}

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

#[test]
fn dynamic_cpp_struct_relinks_authored_offsets_and_writes_only_its_fields() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("cpp-layout-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output,
        log_file: None,
    }).unwrap();
    let access = owner.access();
    let outer = access.native_class("Core.Object").unwrap();
    let name = access.intern_str("AliasLayoutProbe", FindName::Add).unwrap();
    let alias_name = access.intern_str("Alias", FindName::Add).unwrap();
    let command_name = access.intern_str("Command", FindName::Add).unwrap();
    unsafe {
        let structure = access.create_cpp_struct(outer.cast(), name, size_of::<AliasValue>(), 4).unwrap();
        let alias = access.add_cpp_property(structure, "NameProperty", alias_name, 0,
            offset_of!(AliasValue, alias), 1, 0, 4, ptr::null_mut()).unwrap();
        let command = access.add_cpp_property(structure, "StrProperty", command_name, 0,
            offset_of!(AliasValue, command), 1, 0, 4, ptr::null_mut()).unwrap();
        access.link_cpp_struct(structure).expect("link actual dynamic native declarations");
        // A package/relink must recover constructor-authored offsets, not trust
        // overwritten live slots or recalculate a contiguous script layout.
        ptr::addr_of_mut!((*alias).offset).write_unaligned(-777);
        ptr::addr_of_mut!((*command).offset).write_unaligned(-999);
        access.link_cpp_struct(structure).unwrap();
        assert_eq!(ptr::addr_of!((*alias).offset).read_unaligned() as usize, offset_of!(AliasValue, alias));
        assert_eq!(ptr::addr_of!((*command).offset).read_unaligned() as usize, offset_of!(AliasValue, command));

        let mut value = AliasValue {
            leading: [0x1122_3344_5566_7788, 0x8877_6655_4433_2211], alias: FName { index: 0 },
            middle: [0xaa55_0099; 3], command: std::mem::zeroed(),
            trailing: [0x1357_9bdf_2468_ace0; 2],
        };
        let bytes = ptr::addr_of_mut!(value).cast::<u8>();
        let alias_text: Vec<u32> = "MoveForward".chars().map(u32::from).collect();
        let command_text: Vec<u32> = "Axis aBaseY Speed=+300.0".chars().map(u32::from).collect();
        access.property_import_text_at(alias, bytes.add((*alias).offset as usize), &alias_text, ptr::null_mut(), 0).unwrap();
        access.property_import_text_at(command, bytes.add((*command).offset as usize), &command_text, ptr::null_mut(), 0).unwrap();
        assert_eq!(access.name_text(value.alias.index).unwrap(), alias_text);
        let actual = std::slice::from_raw_parts(value.command.data.cast::<u32>(), value.command.num as usize);
        assert_eq!(&actual[..actual.len() - 1], command_text);
        assert_eq!(actual.last(), Some(&0));
        assert_eq!(value.leading, [0x1122_3344_5566_7788, 0x8877_6655_4433_2211]);
        assert_eq!(value.middle, [0xaa55_0099; 3]);
        assert_eq!(value.trailing, [0x1357_9bdf_2468_ace0; 2]);
        access.destroy_owned_fstring(ptr::addr_of_mut!(value.command)).unwrap();
    }
    owner.shutdown().unwrap();
}
