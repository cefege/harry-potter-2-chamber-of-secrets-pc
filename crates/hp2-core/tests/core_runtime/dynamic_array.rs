use super::*;
use std::{mem::size_of, ptr};

fn indexed(property: *mut UProperty, index: i32) -> Vec<u8> {
    let mut code = vec![0x10, 0x1d]; // DynArrayElement, IntConst.
    code.extend(index.to_le_bytes());
    code.push(0x01); // InstanceVariable.
    code.extend(unsafe { (*property.cast::<UObject>()).index }.to_le_bytes());
    code
}

unsafe fn evaluate(access: CoreAccess<'_>, object: *mut UObject, mut code: Vec<u8>, result: *mut c_void) {
    let function = unsafe {
        access.construct_object(access.native_class("Function").unwrap(), (*object).class.cast(), 0, 0x4000, ptr::null_mut()).unwrap()
    }.cast::<UFunction>();
    unsafe {
        (*function).function_flags = 0;
        (*function).parms_size = 0;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_size = 0;
        (*function.cast::<UStruct>()).properties_align = 4;
        (*function.cast::<UStruct>()).script = TArrayRaw { data: code.as_mut_ptr().cast(), num: code.len() as i32, max: code.len() as i32 };
    }
    let outcome = unsafe { access.execute_event(object, function, ptr::null_mut(), result) };
    // The function only borrows the vector during synchronous event execution.
    unsafe { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    outcome.expect("evaluate actual dynamic-array expression through production Core transport");
}

unsafe fn clear(access: CoreAccess<'_>, object: *mut UObject, property: *mut UProperty) {
    let address = unsafe { access.property_address(object, property).unwrap() };
    unsafe {
        access.property_value(Hp2ValueOperation::ValueDestroy, property, address.cast(), ptr::null_mut(), 0, None).unwrap();
        address.cast::<TArrayRaw>().write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 });
    }
}

unsafe fn string(access: CoreAccess<'_>, object: *mut UObject, mut expression: Vec<u8>) -> TArrayRaw {
    expression.insert(0, 0x04); // Return.
    let mut result = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    unsafe { evaluate(access, object, expression, (&raw mut result).cast()); }
    result
}

unsafe fn text(value: &TArrayRaw) -> String {
    if value.num == 0 { return String::new(); }
    unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) }
        .iter().take_while(|&&unit| unit != 0).map(|&unit| char::from_u32(unit).unwrap()).collect()
}

#[test]
fn production_dynamic_name_and_string_arrays_preserve_indexed_values_and_copy_ownership() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("core-dynamic-array-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).unwrap();
    fs::write(root.join("System/System.ini"), "").unwrap();
    fs::write(root.join("System/User.ini"), "").unwrap();
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"),
        command_line: Vec::new(), output: |_, _, _| Ok(()), log_file: None,
    }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..CoreProcessMode::default() });
    unsafe {
        let class = access.native_class("System").unwrap();
        let object = access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap();
        let names = find_field(access, class.cast(), "Suppress").unwrap();
        let strings = find_field(access, class.cast(), "Paths").unwrap();
        clear(access, object, names);
        clear(access, object, strings);
        assert_ne!((*names).element_size as usize, size_of::<i32>(), "array header and name-element stride differ");
        for (index, expected) in [(2, "BookmarkTwo"), (5, "BookmarkFive"), (2, "UpdatedTwo")] {
            let name = access.intern_str(expected, FindName::Add).unwrap();
            let mut code = vec![0x0f]; // Let.
            code.extend(indexed(names, index));
            code.push(0x21); // NameConst.
            code.extend(name.to_le_bytes());
            code.extend([0x04, 0x0b]); // Return Nothing: this writer has no result slot.
            evaluate(access, object, code, ptr::null_mut());
        }
        for (index, expected) in [(0, "None"), (2, "UpdatedTwo"), (5, "BookmarkFive")] {
            let mut expression = vec![0x57]; // NameToString, as used by bookmark PaintColumn.
            expression.extend(indexed(names, index));
            let mut copied = string(access, object, expression);
            assert_eq!(text(&copied), expected);
            access.destroy_owned_fstring(&mut copied).unwrap();
        }
        for (index, expected) in [(2, "two paths"), (5, "five paths"), (2, "updated two paths")] {
            let mut code = vec![0x0f];
            code.extend(indexed(strings, index));
            code.push(0x1f); // StringConst.
            code.extend(expected.bytes());
            code.extend([0, 0x04, 0x0b]);
            evaluate(access, object, code, ptr::null_mut());
        }
        for (index, expected) in [(0, ""), (2, "updated two paths"), (5, "five paths")] {
            let mut copied = string(access, object, indexed(strings, index));
            assert_eq!(text(&copied), expected);
            access.assign_fstring(&mut copied, &"independent replacement".chars().map(u32::from).chain([0]).collect::<Vec<_>>()).unwrap();
            let mut original = string(access, object, indexed(strings, index));
            assert_eq!(text(&original), expected, "mutating a returned string must not modify its array element");
            access.destroy_owned_fstring(&mut copied).unwrap();
            access.destroy_owned_fstring(&mut original).unwrap();
        }
    }
    owner.shutdown().unwrap();
}
