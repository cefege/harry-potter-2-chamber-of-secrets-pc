use super::*;
use std::ptr;

#[test]
fn production_console_string_arguments_consume_one_token_and_preserve_the_remainder() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("core-console-arguments-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).unwrap();
    fs::write(root.join("System/System.ini"), "").unwrap();
    fs::write(root.join("System/User.ini"), "").unwrap();
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/System.ini"),
        user_ini: root.join("System/User.ini"),
        command_line: Vec::new(),
        output: |_, _, _| Ok(()),
        log_file: None,
    }).unwrap();
    let access = owner.access();
    let class = access.native_class("System").unwrap();
    // This is the actual native StrProperty descriptor, not an adapter double.
    let property = unsafe { find_field(access, class.cast(), "CachePath").unwrap() };
    for (input, expected, consumed) in [
        ("GSTATE020 next", "GSTATE020", "GSTATE020".len()),
        ("\"a path with spaces\" next", "a path with spaces", "\"a path with spaces\"".len()),
        ("\"\" next", "", 2),
    ] {
        let input: Vec<u32> = input.chars().map(u32::from).chain([0]).collect();
        let mut value = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        // SAFETY: the descriptor and zero-initialized FString live through import and destruction.
        let actual_consumed = unsafe { access.import_console_argument(property, (&raw mut value).cast(), &input).unwrap() };
        let actual = if value.num == 0 { String::new() } else {
            unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) }
                .iter().take_while(|&&unit| unit != 0).map(|&unit| char::from_u32(unit).unwrap()).collect::<String>()
        };
        unsafe { access.destroy_owned_fstring(&mut value).unwrap(); }
        assert_eq!(actual_consumed, consumed, "leave the next console argument for its own property");
        assert_eq!(actual, expected);
        assert_eq!(input[actual_consumed], u32::from(' '));
    }
    owner.shutdown().unwrap();
}
