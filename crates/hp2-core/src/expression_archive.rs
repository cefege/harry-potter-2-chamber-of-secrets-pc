//! Recursive script/archive traversal. Native leaves own typed archive operators,
//! indexed object/name conversion, live Script access, and engine diagnostics.

use std::ffi::c_void;

type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2ExprOperand {
    ExprByte,
    ExprWord,
    ExprInt,
    ExprFloat,
    ExprProperty,
    ExprStruct,
    ExprClass,
    ExprObject,
    ExprName,
}
pub type Hp2ExprContinuation = unsafe extern "C" fn(*mut c_void, *mut u8) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ExpressionArchiveServices {
    pub transfer: unsafe extern "C" fn(Object, Object, *mut i32, Hp2ExprOperand, *mut i32) -> i32,
    pub scope: unsafe extern "C" fn(Hp2ExprContinuation, *mut c_void, *mut u8) -> i32,
    pub read_script: unsafe extern "C" fn(Object, i32, i32, *mut u32) -> i32,
    pub version: unsafe extern "C" fn(Object) -> i32,
    pub tell: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub seek: unsafe extern "C" fn(Object, i32) -> i32,
    pub full_name: unsafe extern "C" fn(Object, *mut *const c_void, *mut i32) -> i32,
    pub check_alignment: unsafe extern "C" fn(i32) -> i32,
    pub bad_token: unsafe extern "C" fn(u8) -> i32,
}

// EExprToken from Core/Inc/UnStack.h. There is no existing shared Rust token
// definition; native static assertions keep this archive vocabulary in sync.
pub mod token {
    pub const LOCAL_VARIABLE: u8 = 0x00;
    pub const INSTANCE_VARIABLE: u8 = 0x01;
    pub const DEFAULT_VARIABLE: u8 = 0x02;
    pub const RETURN: u8 = 0x04;
    pub const SWITCH: u8 = 0x05;
    pub const JUMP: u8 = 0x06;
    pub const JUMP_IF_NOT: u8 = 0x07;
    pub const STOP: u8 = 0x08;
    pub const ASSERT: u8 = 0x09;
    pub const CASE: u8 = 0x0a;
    pub const NOTHING: u8 = 0x0b;
    pub const LABEL_TABLE: u8 = 0x0c;
    pub const GOTO_LABEL: u8 = 0x0d;
    pub const EAT_STRING: u8 = 0x0e;
    pub const LET: u8 = 0x0f;
    pub const DYN_ARRAY_ELEMENT: u8 = 0x10;
    pub const NEW: u8 = 0x11;
    pub const CLASS_CONTEXT: u8 = 0x12;
    pub const META_CAST: u8 = 0x13;
    pub const LET_BOOL: u8 = 0x14;
    pub const END_FUNCTION_PARMS: u8 = 0x16;
    pub const SELF: u8 = 0x17;
    pub const SKIP: u8 = 0x18;
    pub const CONTEXT: u8 = 0x19;
    pub const ARRAY_ELEMENT: u8 = 0x1a;
    pub const VIRTUAL_FUNCTION: u8 = 0x1b;
    pub const FINAL_FUNCTION: u8 = 0x1c;
    pub const INT_CONST: u8 = 0x1d;
    pub const FLOAT_CONST: u8 = 0x1e;
    pub const STRING_CONST: u8 = 0x1f;
    pub const OBJECT_CONST: u8 = 0x20;
    pub const NAME_CONST: u8 = 0x21;
    pub const ROTATION_CONST: u8 = 0x22;
    pub const VECTOR_CONST: u8 = 0x23;
    pub const BYTE_CONST: u8 = 0x24;
    pub const INT_ZERO: u8 = 0x25;
    pub const INT_ONE: u8 = 0x26;
    pub const TRUE: u8 = 0x27;
    pub const FALSE: u8 = 0x28;
    pub const NATIVE_PARM: u8 = 0x29;
    pub const NO_OBJECT: u8 = 0x2a;
    pub const INT_CONST_BYTE: u8 = 0x2c;
    pub const BOOL_VARIABLE: u8 = 0x2d;
    pub const DYNAMIC_CAST: u8 = 0x2e;
    pub const ITERATOR: u8 = 0x2f;
    pub const ITERATOR_POP: u8 = 0x30;
    pub const ITERATOR_NEXT: u8 = 0x31;
    pub const STRUCT_CMP_EQ: u8 = 0x32;
    pub const STRUCT_CMP_NE: u8 = 0x33;
    pub const UNICODE_STRING_CONST: u8 = 0x34;
    pub const STRUCT_MEMBER: u8 = 0x36;
    pub const DYN_ARRAY_COUNT: u8 = 0x37;
    pub const DEBUG_INFO: u8 = 0x38;
    pub const GLOBAL_FUNCTION: u8 = 0x39;
    pub const MIN_CONVERSION: u8 = 0x3a;
    pub const MAX_CONVERSION: u8 = 0x60;
    pub const EXTENDED_NATIVE: u8 = 0x60;
    pub const FIRST_NATIVE: u8 = 0x70;
}

fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}

pub(crate) trait ExpressionOps {
    type Error;
    unsafe fn transfer(&self, code: *mut i32, kind: Hp2ExprOperand) -> std::result::Result<bool, Self::Error>;
    unsafe fn read_script(&self, offset: i32, width: i32) -> std::result::Result<u32, Self::Error>;
    fn version(&self) -> i32;
    fn tell(&self) -> std::result::Result<i32, Self::Error>;
    fn seek(&self, position: i32) -> std::result::Result<(), Self::Error>;
    fn teleporter(&self) -> std::result::Result<bool, Self::Error>;
    fn scope<F: FnMut() -> std::result::Result<u8, Self::Error>>(&self, body: F) -> std::result::Result<u8, Self::Error>;
    /// Only the optional token/version probe may use initialized storage after
    /// the logical script end; ordinary expression operands remain bounded.
    fn debug_lookahead<F: FnOnce() -> std::result::Result<i32, Self::Error>>(
        &self, body: F,
    ) -> std::result::Result<i32, Self::Error> { body() }
    fn check_alignment(&self, code: i32) -> std::result::Result<(), Self::Error>;
    fn bad_token(&self, token: u8) -> std::result::Result<(), Self::Error>;
}

struct LegacyExpressionOps<'a> {
    object: Object,
    archive: Object,
    services: &'a Hp2ExpressionArchiveServices,
}

impl ExpressionOps for LegacyExpressionOps<'_> {
    type Error = i32;
    unsafe fn transfer(&self, code: *mut i32, kind: Hp2ExprOperand) -> Result<bool> {
        let mut name_none = 0;
        status(unsafe { (self.services.transfer)(self.object, self.archive, code, kind, &mut name_none) })?;
        Ok(name_none != 0)
    }
    unsafe fn read_script(&self, offset: i32, width: i32) -> Result<u32> {
        let mut value = 0;
        status(unsafe { (self.services.read_script)(self.object, offset, width, &mut value) })?;
        Ok(value)
    }
    fn version(&self) -> i32 { unsafe { (self.services.version)(self.archive) } }
    fn tell(&self) -> Result<i32> {
        let mut value = 0;
        status(unsafe { (self.services.tell)(self.archive, &mut value) })?;
        Ok(value)
    }
    fn seek(&self, position: i32) -> Result { status(unsafe { (self.services.seek)(self.archive, position) }) }
    fn teleporter(&self) -> Result<bool> {
        let mut name = std::ptr::null();
        let mut width = 0;
        status(unsafe { (self.services.full_name)(self.object, &mut name, &mut width) })?;
        // Consume native GetFullName storage before another native callback.
        let equal = |expected: &[u8]| unsafe {
            expected.iter().copied().chain(std::iter::once(0)).enumerate().all(|(i, byte)| {
                let value = if width == 2 { name.cast::<u16>().add(i).read_unaligned() as u32 }
                    else { name.cast::<u32>().add(i).read_unaligned() };
                value == u32::from(byte)
            })
        };
        Ok(equal(b"Function Engine.Teleporter.SpecialHandling") || equal(b"Class Engine.Teleporter"))
    }
    fn scope<F: FnMut() -> Result<u8>>(&self, mut body: F) -> Result<u8> {
        unsafe extern "C" fn invoke<F: FnMut() -> Result<u8>>(user: Object, token: *mut u8) -> i32 {
            match unsafe { (&mut *user.cast::<F>())() } {
                Ok(value) => { unsafe { token.write(value); } 0 }
                Err(error) => error,
            }
        }
        let mut token = 0;
        status(unsafe { (self.services.scope)(invoke::<F>, (&mut body as *mut F).cast(), &mut token) })?;
        Ok(token)
    }
    fn check_alignment(&self, code: i32) -> Result { status(unsafe { (self.services.check_alignment)(code) }) }
    fn bad_token(&self, token: u8) -> Result { status(unsafe { (self.services.bad_token)(token) }) }
}

struct Walk<'a, O: ExpressionOps> {
    code: *mut i32,
    ops: &'a O,
}

impl<O: ExpressionOps> Walk<'_, O> {
    unsafe fn transfer(&self, kind: Hp2ExprOperand) -> std::result::Result<bool, O::Error> {
        unsafe { self.ops.transfer(self.code, kind) }
    }

    unsafe fn preceding(&self, bytes: i32) -> std::result::Result<u32, O::Error> {
        unsafe { self.ops.read_script(self.code.read() - bytes, bytes) }
    }

    unsafe fn parameters(&self) -> std::result::Result<(), O::Error> {
        while unsafe { self.expression()? } != token::END_FUNCTION_PARMS {}
        Ok(())
    }

    unsafe fn string(&self, kind: Hp2ExprOperand) -> std::result::Result<(), O::Error> {
        loop {
            unsafe { self.transfer(kind)? };
            // Intentionally inspect the LAST BYTE, also for Unicode strings.
            if unsafe { self.preceding(1)? } == 0 {
                return Ok(());
            }
        }
    }

    unsafe fn optional_debug(&self) -> std::result::Result<(), O::Error> {
        let version = self.ops.debug_lookahead(|| unsafe {
            use Hp2ExprOperand::*;
            let position = self.ops.tell()?;
            let old_code = self.code.read();
            self.transfer(ExprByte)?;
            let mut version = -1;
            if self.preceding(1)? == u32::from(token::DEBUG_INFO) {
                self.transfer(ExprInt)?;
                version = self.preceding(4)? as i32;
            }
            // Rewind positions, not bytes mutated by speculative transfers.
            self.code.write(old_code);
            self.ops.seek(position)?;
            Ok(version)
        })?;
        if version == 100 {
            // This is a real expression, not speculative storage access.
            unsafe { self.expression()?; }
        }
        Ok(())
    }

    unsafe fn expression(&self) -> std::result::Result<u8, O::Error> {
        self.ops.scope(|| {
            let mut token = 0;
            unsafe { self.expression_body(&mut token)?; }
            Ok(token)
        })
    }

    unsafe fn expression_body(&self, result: *mut u8) -> std::result::Result<(), O::Error> {
        use Hp2ExprOperand::*;
        use token::*;
        unsafe {
            self.transfer(ExprByte)?;
            let mut expr = self.preceding(1)? as u8;
            result.write(expr);
            if self.ops.version() <= 76 && (0x38..0x5b).contains(&expr) {
                // Original code adjusts dispatch/return only, NOT Script bytes.
                expr += 1;
                result.write(expr);
            }
            if (MIN_CONVERSION..MAX_CONVERSION).contains(&expr) {
                self.expression()?;
            } else if expr >= FIRST_NATIVE {
                self.parameters()?;
                if expr != 130 || !self.ops.teleporter()? {
                    self.optional_debug()?;
                }
            } else if expr >= EXTENDED_NATIVE {
                self.transfer(ExprByte)?;
                self.parameters()?;
                self.optional_debug()?;
            } else {
                match expr {
                    LET | LET_BOOL | ARRAY_ELEMENT | DYN_ARRAY_ELEMENT => {
                        self.expression()?;
                        self.expression()?;
                    }
                    JUMP => {
                        self.transfer(ExprWord)?;
                    }
                    LOCAL_VARIABLE | INSTANCE_VARIABLE | DEFAULT_VARIABLE | NATIVE_PARM => {
                        self.transfer(ExprProperty)?;
                    }
                    DEBUG_INFO => {
                        self.transfer(ExprInt)?;
                        self.transfer(ExprInt)?;
                        self.transfer(ExprInt)?;
                        self.string(ExprByte)?;
                    }
                    BOOL_VARIABLE | NOTHING | END_FUNCTION_PARMS | INT_ZERO | INT_ONE | TRUE
                    | FALSE | NO_OBJECT | SELF | ITERATOR_POP | STOP | ITERATOR_NEXT => {}
                    EAT_STRING | RETURN | GOTO_LABEL | DYN_ARRAY_COUNT => {
                        self.expression()?;
                    }
                    FINAL_FUNCTION => {
                        self.transfer(ExprStruct)?;
                        self.parameters()?;
                        self.optional_debug()?;
                    }
                    VIRTUAL_FUNCTION | GLOBAL_FUNCTION => {
                        self.transfer(ExprName)?;
                        self.parameters()?;
                        self.optional_debug()?;
                    }
                    CLASS_CONTEXT | CONTEXT => {
                        self.expression()?;
                        self.transfer(ExprWord)?;
                        self.transfer(ExprByte)?;
                        self.expression()?;
                    }
                    NEW => {
                        for _ in 0..4 {
                            self.expression()?;
                        }
                    }
                    INT_CONST => {
                        self.transfer(ExprInt)?;
                    }
                    FLOAT_CONST => {
                        self.transfer(ExprFloat)?;
                    }
                    STRING_CONST => {
                        self.string(ExprByte)?;
                    }
                    UNICODE_STRING_CONST => {
                        self.string(ExprWord)?;
                    }
                    OBJECT_CONST => {
                        self.transfer(ExprObject)?;
                    }
                    NAME_CONST => {
                        self.transfer(ExprName)?;
                    }
                    ROTATION_CONST => {
                        for _ in 0..3 {
                            self.transfer(ExprInt)?;
                        }
                    }
                    VECTOR_CONST => {
                        for _ in 0..3 {
                            self.transfer(ExprFloat)?;
                        }
                    }
                    BYTE_CONST | INT_CONST_BYTE => {
                        self.transfer(ExprByte)?;
                    }
                    META_CAST | DYNAMIC_CAST => {
                        self.transfer(ExprClass)?;
                        self.expression()?;
                    }
                    JUMP_IF_NOT | ASSERT | SKIP => {
                        self.transfer(ExprWord)?;
                        self.expression()?;
                    }
                    ITERATOR => {
                        self.expression()?;
                        self.transfer(ExprWord)?;
                    }
                    SWITCH => {
                        self.transfer(ExprByte)?;
                        self.expression()?;
                    }
                    CASE => {
                        self.transfer(ExprWord)?;
                        if self.preceding(2)? != 0xffff {
                            self.expression()?;
                        }
                    }
                    LABEL_TABLE => {
                        self.ops.check_alignment(self.code.read())?;
                        loop {
                            let name_none = self.transfer(ExprName)?;
                            self.transfer(ExprInt)?;
                            if name_none {
                                break;
                            }
                        }
                    }
                    STRUCT_CMP_EQ | STRUCT_CMP_NE => {
                        self.transfer(ExprStruct)?;
                        self.expression()?;
                        self.expression()?;
                    }
                    STRUCT_MEMBER => {
                        self.transfer(ExprProperty)?;
                        self.expression()?;
                    }
                    _ => {
                        self.ops.bad_token(expr)?;
                    }
                }
            }
            Ok(())
        }
    }
}

/// # Safety
/// Native handles, cursor, and callbacks must remain live through recursion.
/// Callbacks must contain native exceptions and return their first error status.
/// The script/cursor may be replaced by callbacks; no native storage is borrowed.
pub unsafe fn serialize(
    object: Object,
    archive: Object,
    code: *mut i32,
    services: &Hp2ExpressionArchiveServices,
) -> Result<u8> {
    unsafe { serialize_with(code, &LegacyExpressionOps { object, archive, services }) }
}

/// # Safety
/// Cursor and operand storage remain live through recursive owner operations.
pub(crate) unsafe fn serialize_with<O: ExpressionOps>(code: *mut i32, ops: &O) -> std::result::Result<u8, O::Error> {
    unsafe { Walk { code, ops }.expression() }
}
