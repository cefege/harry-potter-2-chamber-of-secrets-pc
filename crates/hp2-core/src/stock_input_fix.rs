//! Fail-closed in-memory correction for the original stock input-page script.
//! The package stays untouched. A changed handler must be reviewed separately.
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_abi::engine::generated::{UObject, UStruct};

fn named(access: CoreAccess<'_>, object: *mut UObject, name: &str) -> RuntimeResult<bool> {
    if object.is_null() { return Ok(false); }
    let index = i32::from_ne_bytes(unsafe { (*object).name });
    Ok(access.name_text(index)? == name.chars().map(u32::from).collect::<Vec<_>>())
}

/// Called after a function's complete script has been deserialized.
///
/// # Safety
/// The function and its loaded script belong to this owner and are exclusively
/// being serialized. The referenced reflection objects must remain live.
pub(crate) unsafe fn fix(access: CoreAccess<'_>, function: *mut UObject) -> RuntimeResult<()> {
    if !named(access, function, "MouseInvertChanged")? { return Ok(()); }
    let class = unsafe { (*function).outer.cast::<UObject>() };
    if !named(access, class, "FEInputPage")? { return Ok(()); }
    let package = unsafe { (*class).outer.cast::<UObject>() };
    if !named(access, package, "HGame")? { return Ok(()); }
    let script = unsafe { (*function.cast::<UStruct>()).script };
    if script.num != 154 || script.max < script.num || script.data.is_null() {
        return Err(RuntimeError::new("Unsupported HGame.FEInputPage.MouseInvertChanged bytecode shape"));
    }
    let bytes = unsafe { std::slice::from_raw_parts_mut(script.data.cast::<u8>(), 154) };
    // Only serialized object/name indices differ between the known prototype
    // and retail handlers. Compare all opcodes, offsets, strings and terminators.
    const EXPECTED: &[u8] = &[
        0x19,0x1c,0,0,0,0,0x16,0x15,0,0,0x1b,0,0,0,0,0x19,0x01,0,0,0,0,0x06,0,0x04,0x2d,0x01,0,0,0,0,0x16,0x0e,
        0x19,0x1c,0,0,0,0,0x16,0x41,0,0,0x1b,0,0,0,0,0x70,0x1f,
        b's',b'e',b't',b' ',b'i',b'n',b'i',b':',b'E',b'n',b'g',b'i',b'n',b'e',b'.',b'P',b'l',b'a',b'y',b'e',b'r',b'P',b'a',b'w',b'n',b' ',b'b',b'I',b'n',b'v',b'e',b'r',b't',b'M',b'o',b'u',b's',b'e',b' ',0,
        0x54,0x19,0x01,0,0,0,0,0x06,0,0x04,0x2d,0x01,0,0,0,0,0x16,0x16,0xe7,0x70,0x1f,
        b'M',b'o',b'u',b's',b'e',b'I',b'n',b'v',b'e',b'r',b't',b' ',b'c',b'h',b'a',b'n',b'g',b'e',b'd',b' ',b't',b'o',b' ',0,
        0x54,0x19,0x01,0,0,0,0,0x06,0,0x04,0x2d,0x01,0,0,0,0,0x16,0x16,0x04,0x0b,
    ];
    const SLOTS: [usize; 10] = [2,11,17,26,34,43,92,101,137,146];
    if !matches_shape(bytes, EXPECTED, &SLOTS) {
        return Err(RuntimeError::new("Unsupported HGame.FEInputPage.MouseInvertChanged bytecode shape"));
    }
    let index = |pos: usize| i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
    let object = |pos: usize| access.object_at(index(pos)).cast::<UObject>();
    let get_owner = object(2);
    let invert = object(17);
    let center = object(92);
    let checked = object(26);
    let property_class = access.native_class("Core.ObjectProperty")?;
    let bool_property_class = access.native_class("Core.BoolProperty")?;
    if get_owner.is_null() || !named(access, get_owner, "GetPlayerOwner")?
        || !named(access, invert, "MouseInvertCheck")? || !named(access, center, "AutoCenterCamCheck")?
        || !named(access, checked, "bChecked")?
        || unsafe { (*invert).outer } != class.cast() || unsafe { (*center).outer } != class.cast()
        || !unsafe { crate::object_model::is_a(invert.cast(), property_class.cast()) }
        || !unsafe { crate::object_model::is_a(center.cast(), property_class.cast()) }
        || !unsafe { crate::object_model::is_a(checked.cast(), bool_property_class.cast()) }
        || index(2) != index(34) || index(17) != index(137)
        || index(26) != index(101) || index(26) != index(146)
        || access.name_text(index(11))? != "InvertMouse".chars().map(u32::from).collect::<Vec<_>>()
        || access.name_text(index(43))? != "ConsoleCommand".chars().map(u32::from).collect::<Vec<_>>()
    {
        return Err(RuntimeError::new("Unsupported HGame.FEInputPage.MouseInvertChanged reflection references"));
    }
    let replacement = index(17).to_le_bytes();
    bytes[92..96].copy_from_slice(&replacement);
    Ok(())
}

fn matches_shape(bytes: &[u8], expected: &[u8], slots: &[usize]) -> bool {
    expected.len() == bytes.len() && bytes.iter().enumerate().all(|(i, &b)| {
        slots.iter().any(|&s| (s..s + 4).contains(&i)) || b == expected[i]
    })
}

#[cfg(test)]
mod tests {
    use super::matches_shape;
    #[test]
    fn checked_shape_rejects_changed_opcodes_and_lengths() {
        let template = [1, 2, 0, 0, 0, 0, 3, 4];
        assert!(matches_shape(&[1, 2, 9, 8, 7, 6, 3, 4], &template, &[2]));
        assert!(!matches_shape(&[1, 9, 9, 8, 7, 6, 3, 4], &template, &[2]));
        assert!(!matches_shape(&[1, 2, 0, 0], &template, &[2]));
    }
}
