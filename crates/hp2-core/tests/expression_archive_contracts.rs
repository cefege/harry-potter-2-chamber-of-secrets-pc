use hp2_core::expression_archive::{serialize, token::*, Hp2ExprContinuation, Hp2ExprOperand, Hp2ExpressionArchiveServices};
use std::ffi::c_void;

struct Archive {
    script: Vec<u8>,
    wire: Vec<u8>,
    position: usize,
    loading: bool,
    version: i32,
    transfers: Vec<Hp2ExprOperand>,
    seeks: Vec<i32>,
    fail_at: usize,
    name: Vec<u32>,
    scopes: usize,
    move_cursor_at: usize,
}
impl Archive {
    fn new(bytes: &[u8], loading: bool) -> Self {
        let mut script = vec![0xcd; bytes.len() + 32];
        if !loading { script[..bytes.len()].copy_from_slice(bytes); }
        Self {
            script, wire: if loading { bytes.to_vec() } else { Vec::new() },
            position: 0, loading, version: 79, transfers: Vec::new(), seeks: Vec::new(),
            fail_at: usize::MAX, name: "Function Test.Normal".bytes().map(u32::from).chain([0]).collect(),
            scopes: 0, move_cursor_at: usize::MAX,
        }
    }
    fn run(&mut self, code: &mut i32) -> Result<u8, i32> {
        let object = (self as *mut Self).cast();
        unsafe { serialize(object, object, code, &SERVICES) }
    }
}
unsafe extern "C" fn transfer(object: *mut c_void, _: *mut c_void, code: *mut i32, kind: Hp2ExprOperand, none: *mut i32) -> i32 {
    use Hp2ExprOperand::*;
    let a = unsafe { &mut *object.cast::<Archive>() };
    a.transfers.push(kind);
    if a.transfers.len() == a.fail_at { return 8173; }
    let size = match kind { ExprByte => 1, ExprWord => 2, _ => 4 };
    let mut value = [0; 4];
    let old = unsafe { code.read() } as usize;
    if a.loading { value[..size].copy_from_slice(&a.wire[a.position..a.position + size]); }
    else {
        value[..size].copy_from_slice(&a.script[old..old + size]);
        a.wire.resize(a.wire.len().max(a.position + size), 0);
        a.wire[a.position..a.position + size].copy_from_slice(&value[..size]);
    }
    a.position += size;
    // Archive callbacks may replace Script and change the live native cursor.
    let replacement = a.script.clone();
    a.script = replacement;
    if a.transfers.len() == a.move_cursor_at { unsafe { code.write(code.read() + 3) }; }
    let current = unsafe { code.read() } as usize;
    a.script[current..current + size].copy_from_slice(&value[..size]);
    unsafe { code.write(code.read() + size as i32); none.write(i32::from(u32::from_ne_bytes(value) == 0)); }
    0
}
unsafe extern "C" fn scope(body: Hp2ExprContinuation, user: *mut c_void, token: *mut u8) -> i32 {
    unsafe { body(user, token) }
}
unsafe extern "C" fn read_script(object: *mut c_void, offset: i32, size: i32, result: *mut u32) -> i32 {
    let a = unsafe { &*object.cast::<Archive>() };
    let mut bytes = [0; 4];
    bytes[..size as usize].copy_from_slice(&a.script[offset as usize..(offset + size) as usize]);
    unsafe { result.write(u32::from_ne_bytes(bytes)) };
    0
}
unsafe extern "C" fn version(object: *mut c_void) -> i32 { unsafe { (*object.cast::<Archive>()).version } }
unsafe extern "C" fn tell(object: *mut c_void, result: *mut i32) -> i32 {
    unsafe { result.write((*object.cast::<Archive>()).position as i32) }; 0
}
unsafe extern "C" fn seek(object: *mut c_void, position: i32) -> i32 {
    let a = unsafe { &mut *object.cast::<Archive>() };
    a.position = position as usize;
    a.seeks.push(position);
    0
}
unsafe extern "C" fn full_name(object: *mut c_void, name: *mut *const c_void, width: *mut i32) -> i32 {
    let a = unsafe { &mut *object.cast::<Archive>() };
    a.scopes += 1;
    unsafe { name.write(a.name.as_ptr().cast()); width.write(4); } 0
}
unsafe extern "C" fn alignment(code: i32) -> i32 { if code & 3 == 0 { 0 } else { 9001 } }
unsafe extern "C" fn bad_token(_: u8) -> i32 { 9002 }
const SERVICES: Hp2ExpressionArchiveServices = Hp2ExpressionArchiveServices {
    transfer, scope, read_script, version, tell, seek, full_name, check_alignment: alignment, bad_token,
};

#[test]
fn recursion_operand_order_and_live_cursor_replacement() {
    use Hp2ExprOperand::*;
    let bytes = [CONTEXT, SELF, 7, 0, 4, LET, INT_CONST, 1, 2, 3, 4, ITERATOR, TRUE, 9, 0];
    for loading in [false, true] {
        let mut a = Archive::new(&bytes, loading);
        let mut code = 0;
        assert_eq!(a.run(&mut code), Ok(CONTEXT));
        assert_eq!(code, bytes.len() as i32);
        assert_eq!(a.transfers, [ExprByte, ExprByte, ExprWord, ExprByte, ExprByte, ExprByte, ExprInt, ExprByte, ExprByte, ExprWord]);
        assert_eq!(&a.script[..bytes.len()], &bytes);
    }
    let mut a = Archive::new(&[RETURN, TRUE], true);
    a.move_cursor_at = 1;
    let mut code = 0;
    assert_eq!(a.run(&mut code), Ok(RETURN));
    assert_eq!(code, 5);
    assert_eq!(a.script[3..5], [RETURN, TRUE]);
}

#[test]
fn legacy_conversion_changes_dispatch_not_stored_token() {
    for version in [76, 77] {
        let bytes = [0x3a, TRUE];
        let mut a = Archive::new(&bytes, true);
        a.version = version;
        let mut code = 0;
        assert_eq!(a.run(&mut code), Ok(if version == 76 { 0x3b } else { 0x3a }));
        assert_eq!(a.script[0], 0x3a);
        assert_eq!(code, 2);
    }
    let mut a = Archive::new(&[0x39, TRUE], true);
    a.version = 76;
    assert_eq!(a.run(&mut 0), Ok(MIN_CONVERSION));
}

#[test]
fn optional_debug_rewinds_positions_but_retains_peek_mutation() {
    let bytes = [FIRST_NATIVE, END_FUNCTION_PARMS, DEBUG_INFO, 99, 0, 0, 0];
    let mut a = Archive::new(&bytes, true);
    let mut code = 0;
    assert_eq!(a.run(&mut code), Ok(FIRST_NATIVE));
    assert_eq!(code, 2);
    assert_eq!(a.position, 2);
    assert_eq!(a.seeks, [2]);
    assert_eq!(&a.script[..bytes.len()], &bytes);

    let bytes = [FIRST_NATIVE, END_FUNCTION_PARMS, DEBUG_INFO, 100, 0, 0, 0, 7, 0, 0, 0, 9, 0, 0, 0, b'x', 0];
    let mut a = Archive::new(&bytes, true);
    code = 0;
    assert_eq!(a.run(&mut code), Ok(FIRST_NATIVE));
    assert_eq!(code, bytes.len() as i32);
    assert_eq!(a.position, bytes.len());
    assert_eq!(a.seeks, [2]);
    assert_eq!(&a.script[..bytes.len()], &bytes);
}

#[test]
fn teleporter_exception_is_exact_and_only_for_native_130() {
    for name in ["Function Engine.Teleporter.SpecialHandling", "Class Engine.Teleporter", "Class Engine.TeleporterExtra"] {
        for token in [130, 131] {
            let mut a = Archive::new(&[token, END_FUNCTION_PARMS, NOTHING], true);
            a.name = name.bytes().map(u32::from).chain([0]).collect();
            let mut code = 0;
            assert_eq!(a.run(&mut code), Ok(token));
            let skip = token == 130 && !name.ends_with("Extra");
            assert_eq!(a.seeks.is_empty(), skip);
            assert_eq!(a.scopes, usize::from(token == 130));
        }
    }
}

#[test]
fn first_native_error_stops_before_rewind_and_recovers() {
    let bytes = [FIRST_NATIVE, END_FUNCTION_PARMS, DEBUG_INFO, 100, 0, 0, 0];
    let mut a = Archive::new(&bytes, true);
    a.fail_at = 4;
    let mut code = 0;
    assert_eq!(a.run(&mut code), Err(8173));
    assert_eq!(code, 3);
    assert_eq!(a.position, 3);
    assert!(a.seeks.is_empty());
    assert_eq!(a.transfers.len(), 4);
    let mut good = Archive::new(&[RETURN, TRUE], true);
    assert_eq!(good.run(&mut 0), Ok(RETURN));
    let mut bad = Archive::new(&[0x03], true);
    assert_eq!(bad.run(&mut 0), Err(9002));
}

#[test]
fn unicode_last_byte_case_default_and_label_termination() {
    let mut unicode = Archive::new(&[UNICODE_STRING_CONST, b'A', 0, b'B', 0], true);
    let mut code = 0;
    assert_eq!(unicode.run(&mut code), Ok(UNICODE_STRING_CONST));
    assert_eq!(code, 3);
    let mut default = Archive::new(&[CASE, 0xff, 0xff, TRUE], true);
    code = 0;
    assert_eq!(default.run(&mut code), Ok(CASE));
    assert_eq!(code, 3);
    let bytes = [LABEL_TABLE, 9, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0];
    let mut labels = Archive::new(&bytes, true);
    code = 3;
    assert_eq!(labels.run(&mut code), Ok(LABEL_TABLE));
    assert_eq!(code, 20);
    assert_eq!(labels.position, 17);
    let mut misaligned = Archive::new(&bytes, true);
    assert_eq!(misaligned.run(&mut 0), Err(9001));
    assert_eq!(misaligned.transfers.len(), 1);
}
