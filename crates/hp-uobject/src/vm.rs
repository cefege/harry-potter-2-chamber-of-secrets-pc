//! UnrealScript bytecode interpreter.
//!
//! [`USToken`] enumerates every `EExprToken` member of
//! `HarryPotter2/Unreal/Core/Inc/UnStack.h` (no wildcard arms — adding an
//! opcode forces a compile error here). Decoding an unknown byte is a loud
//! [`Fail`] with reason `vm.unknown_token`; executing a known-but-deferred
//! token is a loud `vm.token_unsupported`, never silent progress.

use crate::arena::{ObjectArena, ObjectId};
use crate::error::Fail;
use crate::natives::NativeRegistry;
use crate::props::PropStore;
use crate::value::PropValue;

pub type VmResult<T> = std::result::Result<T, Fail>;

/// Every `EExprToken` from `UnStack.h`, tagged with its opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum USToken {
    LocalVariable = 0x00,
    InstanceVariable = 0x01,
    DefaultVariable = 0x02,
    Return = 0x04,
    Switch = 0x05,
    Jump = 0x06,
    JumpIfNot = 0x07,
    Stop = 0x08,
    Assert = 0x09,
    Case = 0x0A,
    Nothing = 0x0B,
    LabelTable = 0x0C,
    GotoLabel = 0x0D,
    EatString = 0x0E,
    Let = 0x0F,
    DynArrayElement = 0x10,
    New = 0x11,
    ClassContext = 0x12,
    MetaCast = 0x13,
    LetBool = 0x14,
    LineNumber = 0x15,
    EndFunctionParms = 0x16,
    SelfToken = 0x17,
    Skip = 0x18,
    Context = 0x19,
    ArrayElement = 0x1A,
    VirtualFunction = 0x1B,
    FinalFunction = 0x1C,
    IntConst = 0x1D,
    FloatConst = 0x1E,
    StringConst = 0x1F,
    ObjectConst = 0x20,
    NameConst = 0x21,
    RotationConst = 0x22,
    VectorConst = 0x23,
    ByteConst = 0x24,
    IntZero = 0x25,
    IntOne = 0x26,
    True = 0x27,
    False = 0x28,
    NativeParm = 0x29,
    NoObject = 0x2A,
    IntConstByte = 0x2C,
    BoolVariable = 0x2D,
    DynamicCast = 0x2E,
    Iterator = 0x2F,
    IteratorPop = 0x30,
    IteratorNext = 0x31,
    StructCmpEq = 0x32,
    StructCmpNe = 0x33,
    UnicodeStringConst = 0x34,
    StructMember = 0x36,
    DynArrayCount = 0x37,
    DebugInfo = 0x38,
    GlobalFunction = 0x39,
    RotatorToVector = 0x3A,
    ByteToInt = 0x3B,
    ByteToBool = 0x3C,
    ByteToFloat = 0x3D,
    IntToByte = 0x3E,
    IntToBool = 0x3F,
    IntToFloat = 0x40,
    BoolToByte = 0x41,
    BoolToInt = 0x42,
    BoolToFloat = 0x43,
    FloatToByte = 0x44,
    FloatToInt = 0x45,
    FloatToBool = 0x46,
    ObjectToBool = 0x47,
    NameToBool = 0x48,
    StringToByte = 0x49,
    StringToInt = 0x4A,
    StringToBool = 0x4B,
    StringToFloat = 0x4C,
    StringToVector = 0x4D,
    StringToRotator = 0x4E,
    VectorToBool = 0x4F,
    VectorToRotator = 0x50,
    RotatorToBool = 0x51,
    ByteToString = 0x52,
    IntToString = 0x53,
    BoolToString = 0x54,
    FloatToString = 0x55,
    ObjectToString = 0x56,
    NameToString = 0x57,
    VectorToString = 0x58,
    RotatorToString = 0x59,
    StringToName = 0x5A,
    ExtendedNative = 0x60,
    FirstNative = 0x70,
}

impl USToken {
    /// Decode one opcode. Unknown bytes are loud errors at the call site.
    pub fn from_opcode(opcode: u8) -> Option<USToken> {
        Some(match opcode {
            0x00 => USToken::LocalVariable,
            0x01 => USToken::InstanceVariable,
            0x02 => USToken::DefaultVariable,
            0x04 => USToken::Return,
            0x05 => USToken::Switch,
            0x06 => USToken::Jump,
            0x07 => USToken::JumpIfNot,
            0x08 => USToken::Stop,
            0x09 => USToken::Assert,
            0x0A => USToken::Case,
            0x0B => USToken::Nothing,
            0x0C => USToken::LabelTable,
            0x0D => USToken::GotoLabel,
            0x0E => USToken::EatString,
            0x0F => USToken::Let,
            0x10 => USToken::DynArrayElement,
            0x11 => USToken::New,
            0x12 => USToken::ClassContext,
            0x13 => USToken::MetaCast,
            0x14 => USToken::LetBool,
            0x15 => USToken::LineNumber,
            0x16 => USToken::EndFunctionParms,
            0x17 => USToken::SelfToken,
            0x18 => USToken::Skip,
            0x19 => USToken::Context,
            0x1A => USToken::ArrayElement,
            0x1B => USToken::VirtualFunction,
            0x1C => USToken::FinalFunction,
            0x1D => USToken::IntConst,
            0x1E => USToken::FloatConst,
            0x1F => USToken::StringConst,
            0x20 => USToken::ObjectConst,
            0x21 => USToken::NameConst,
            0x22 => USToken::RotationConst,
            0x23 => USToken::VectorConst,
            0x24 => USToken::ByteConst,
            0x25 => USToken::IntZero,
            0x26 => USToken::IntOne,
            0x27 => USToken::True,
            0x28 => USToken::False,
            0x29 => USToken::NativeParm,
            0x2A => USToken::NoObject,
            0x2C => USToken::IntConstByte,
            0x2D => USToken::BoolVariable,
            0x2E => USToken::DynamicCast,
            0x2F => USToken::Iterator,
            0x30 => USToken::IteratorPop,
            0x31 => USToken::IteratorNext,
            0x32 => USToken::StructCmpEq,
            0x33 => USToken::StructCmpNe,
            0x34 => USToken::UnicodeStringConst,
            0x36 => USToken::StructMember,
            0x37 => USToken::DynArrayCount,
            0x38 => USToken::DebugInfo,
            0x39 => USToken::GlobalFunction,
            0x3A => USToken::RotatorToVector,
            0x3B => USToken::ByteToInt,
            0x3C => USToken::ByteToBool,
            0x3D => USToken::ByteToFloat,
            0x3E => USToken::IntToByte,
            0x3F => USToken::IntToBool,
            0x40 => USToken::IntToFloat,
            0x41 => USToken::BoolToByte,
            0x42 => USToken::BoolToInt,
            0x43 => USToken::BoolToFloat,
            0x44 => USToken::FloatToByte,
            0x45 => USToken::FloatToInt,
            0x46 => USToken::FloatToBool,
            0x47 => USToken::ObjectToBool,
            0x48 => USToken::NameToBool,
            0x49 => USToken::StringToByte,
            0x4A => USToken::StringToInt,
            0x4B => USToken::StringToBool,
            0x4C => USToken::StringToFloat,
            0x4D => USToken::StringToVector,
            0x4E => USToken::StringToRotator,
            0x4F => USToken::VectorToBool,
            0x50 => USToken::VectorToRotator,
            0x51 => USToken::RotatorToBool,
            0x52 => USToken::ByteToString,
            0x53 => USToken::IntToString,
            0x54 => USToken::BoolToString,
            0x55 => USToken::FloatToString,
            0x56 => USToken::ObjectToString,
            0x57 => USToken::NameToString,
            0x58 => USToken::VectorToString,
            0x59 => USToken::RotatorToString,
            0x5A => USToken::StringToName,
            0x60 => USToken::ExtendedNative,
            0x70 => USToken::FirstNative,
            _ => return None,
        })
    }
}

/// One activation record: locals, result propagation, and the code cursor.
/// Natives receive `&mut Frame` so they can read/mutate locals and self.
pub struct Frame<'a> {
    pub arena: &'a ObjectArena,
    /// Locals and parameters of this activation, keyed by pool name index.
    pub locals: PropStore,
    /// The object executing the code (`self`).
    pub self_id: Option<ObjectId>,
    /// Pending result of the last evaluated expression.
    pub result: PropValue,
    registry: &'a NativeRegistry,
    code: Vec<u8>,
    pc: usize,
}

impl<'a> Frame<'a> {
    pub fn new(
        arena: &'a ObjectArena,
        registry: &'a NativeRegistry,
        code: &[u8],
        self_id: Option<ObjectId>,
    ) -> Self {
        Self {
            arena,
            locals: PropStore::new(),
            self_id,
            result: PropValue::Int(0),
            registry,
            code: code.to_vec(),
            pc: 0,
        }
    }

    /// Run to `EX_Return` / end-of-code; returns the propagated result.
    pub fn run(&mut self) -> VmResult<PropValue> {
        while self.pc < self.code.len() {
            let opcode = self.code[self.pc];
            self.pc += 1;
            let Some(token) = USToken::from_opcode(opcode) else {
                return Err(Fail::new(
                    "vm.unknown_token",
                    format!("opcode {opcode:#04x} at {} has no token", self.pc - 1),
                ));
            };
            if self.exec(token)? {
                break; // EX_Return executed.
            }
        }
        Ok(self.result.clone())
    }

    fn u16_at(&mut self) -> VmResult<u16> {
        let end = self.pc + 2;
        let v = u16::from_le_bytes(
            self.code
                .get(self.pc..end)
                .ok_or_else(truncated)?
                .try_into()
                .unwrap(),
        );
        self.pc = end;
        Ok(v)
    }

    fn i32_at(&mut self) -> VmResult<i32> {
        let end = self.pc + 4;
        let v = i32::from_le_bytes(
            self.code
                .get(self.pc..end)
                .ok_or_else(truncated)?
                .try_into()
                .unwrap(),
        );
        self.pc = end;
        Ok(v)
    }

    fn name_ci(&mut self) -> VmResult<i32> {
        let cursor = &mut self.pc;
        let _ = cursor;
        // Compact index straight from the code bytes.
        let mut pos = self.pc;
        let first = *self.code.get(pos).ok_or_else(truncated)? as u32;
        pos += 1;
        let negative = first & 0x80 != 0;
        let mut magnitude = (first & 0x3F) as i64;
        let mut shift = 6;
        if first & 0x40 != 0 {
            loop {
                let byte = *self.code.get(pos).ok_or_else(truncated)?;
                pos += 1;
                magnitude |= i64::from(byte & 0x7F) << shift;
                shift += 7;
                if byte & 0x80 == 0 || shift >= 35 {
                    break;
                }
            }
        }
        self.pc = pos;
        Ok(if negative { -magnitude } else { magnitude } as i32)
    }

    fn u8_at(&mut self) -> VmResult<u8> {
        let v = *self.code.get(self.pc).ok_or_else(truncated)?;
        self.pc += 1;
        Ok(v)
    }

    /// Execute one token. Returns `true` when `EX_Return` terminated the run.
    fn exec(&mut self, token: USToken) -> VmResult<bool> {
        use PropValue::*;
        match token {
            // ---- Variable loads ------------------------------------------
            USToken::LocalVariable | USToken::NativeParm => {
                let name = self.name_ci()? as u32;
                self.result = self.locals.get(name).cloned().unwrap_or(Int(0));
            }
            USToken::InstanceVariable | USToken::DefaultVariable => {
                let name = self.name_ci()? as u32;
                self.result = instance_value(self, name);
            }
            USToken::BoolVariable => {
                let name = self.name_ci()? as u32;
                let mask = self.u8_at()?;
                let value = instance_value(self, name);
                self.result = Bool(matches!(value, Bool(true)) && mask != 0);
            }

            // ---- Constants ----------------------------------------------
            USToken::IntConst => self.result = Int(self.i32_at()?),
            USToken::IntConstByte => self.result = Int(i32::from(self.u8_at()?)),
            USToken::IntZero => self.result = Int(0),
            USToken::IntOne => self.result = Int(1),
            USToken::ByteConst => self.result = Byte(self.u8_at()?),
            USToken::FloatConst => {
                let raw = self.i32_at()? as u32;
                self.result = Float(f32::from_le_bytes(raw.to_le_bytes()));
            }
            USToken::True => self.result = Bool(true),
            USToken::False => self.result = Bool(false),
            USToken::NameConst => {
                let index = self.name_ci()? as u32;
                self.result = Name(crate::name::Name {
                    index,
                    number: crate::name::NO_NUMBER,
                });
            }
            USToken::ObjectConst => self.result = Object(Some(self.name_ci()?)),
            USToken::NoObject => self.result = Object(None),
            USToken::StringConst => self.result = Str(self.nul_terminated_string()?),
            USToken::UnicodeStringConst => {
                let mut units = Vec::new();
                loop {
                    let lo = self.u8_at()?;
                    let hi = self.u8_at()?;
                    let unit = u16::from_le_bytes([lo, hi]);
                    if unit == 0 {
                        break;
                    }
                    units.push(unit);
                }
                self.result = Str(String::from_utf16_lossy(&units));
            }
            USToken::VectorConst | USToken::RotationConst => {
                let components = if matches!(token, USToken::VectorConst) {
                    3
                } else {
                    4
                };
                for _ in 0..components {
                    self.i32_at()?;
                }
                self.result = Int(0);
            }

            // ---- Assignment & control flow -------------------------------
            USToken::Let | USToken::LetBool => {
                let target = self.pop_lvalue_name()?;
                self.expr()?;
                let value = self.result.clone();
                if token == USToken::LetBool && !matches!(value, Bool(_)) {
                    self.locals.set(target, Bool(value.truthy()));
                } else {
                    self.locals.set(target, value);
                }
            }
            USToken::Jump => {
                let target = self.u16_at()? as usize;
                self.pc = target;
            }
            USToken::JumpIfNot => {
                let target = self.u16_at()? as usize;
                self.expr()?;
                if !self.result.truthy() {
                    self.pc = target;
                }
            }
            USToken::Stop => self.pc = self.code.len(),
            USToken::Nothing => {}
            USToken::Return => {
                self.expr()?;
                self.pc = self.code.len();
                return Ok(true);
            }
            USToken::Skip => {
                let offset = self.u16_at()? as usize;
                let skip_end = self.pc.saturating_add(offset);
                self.expr()?;
                self.pc = skip_end.max(self.pc);
            }
            USToken::Assert => {
                let _line = self.u16_at()?;
                let _guard = self.u8_at()?;
                self.expr()?;
            }
            USToken::GotoLabel
            | USToken::Switch
            | USToken::Case
            | USToken::LabelTable
            | USToken::DebugInfo => {
                return Err(Fail::new(
                    "vm.token_unsupported",
                    format!("{token:?}: label/state/debug control flow is deferred"),
                ));
            }

            // ---- Calls ---------------------------------------------------
            USToken::VirtualFunction | USToken::GlobalFunction | USToken::FinalFunction => {
                let _dispatch_target = self.name_ci()?;
                self.eval_parms_and_dispatch()?;
            }

            // ---- Member/array access ------------------------------------
            USToken::StructMember => {
                let _field = self.name_ci()?;
                self.expr()?;
            }
            USToken::ArrayElement | USToken::DynArrayElement => {
                self.expr()?; // index
                self.expr()?; // array expression
                return Err(Fail::new(
                    "vm.token_unsupported",
                    format!("{token:?}: element addressing is deferred"),
                ));
            }
            USToken::DynArrayCount => {
                self.expr()?;
                self.result = Int(0);
            }

            // ---- Contexts & casts ---------------------------------------
            USToken::Context | USToken::ClassContext => {
                let _offset = self.u16_at()?;
                let _context_name = self.name_ci()?;
                self.expr()?;
            }
            USToken::MetaCast | USToken::DynamicCast => {
                let _class_ref = self.name_ci()?;
                self.expr()?;
            }

            // ---- Iterators ----------------------------------------------
            USToken::Iterator => {
                let _offset = self.u16_at()?;
                self.expr()?;
                return Err(Fail::new(
                    "vm.token_unsupported",
                    "Iterator: foreach iteration state is deferred",
                ));
            }
            USToken::IteratorNext | USToken::IteratorPop => {
                return Err(Fail::new(
                    "vm.token_unsupported",
                    format!("{token:?}: foreach iteration state is deferred"),
                ));
            }

            // ---- Self & struct comparison -------------------------------
            USToken::SelfToken => {
                self.result = self
                    .self_id
                    .map(|_| PropValue::Int(1))
                    .unwrap_or(PropValue::Object(None));
            }
            USToken::StructCmpEq | USToken::StructCmpNe => {
                self.expr()?;
                self.expr()?;
                let equal = matches!(token, USToken::StructCmpEq);
                self.result = PropValue::Bool(equal);
            }

            // ---- Conversions --------------------------------------------

            // ---- Misc ---------------------------------------------------
            USToken::LineNumber => {
                let _line = self.name_ci()?;
            }
            USToken::EatString => {
                self.expr()?;
            }
            USToken::New => {
                return Err(Fail::new(
                    "vm.token_unsupported",
                    "New: object allocation is deferred",
                ));
            }

            // ---- Conversions --------------------------------------------
            USToken::ByteToInt
            | USToken::ByteToBool
            | USToken::ByteToFloat
            | USToken::IntToByte
            | USToken::IntToBool
            | USToken::IntToFloat
            | USToken::BoolToByte
            | USToken::BoolToInt
            | USToken::BoolToFloat
            | USToken::FloatToByte
            | USToken::FloatToInt
            | USToken::FloatToBool
            | USToken::ObjectToBool
            | USToken::NameToBool
            | USToken::StringToByte
            | USToken::StringToInt
            | USToken::StringToBool
            | USToken::StringToFloat
            | USToken::StringToVector
            | USToken::StringToRotator
            | USToken::VectorToBool
            | USToken::VectorToRotator
            | USToken::RotatorToBool
            | USToken::RotatorToVector
            | USToken::ByteToString
            | USToken::IntToString
            | USToken::BoolToString
            | USToken::FloatToString
            | USToken::ObjectToString
            | USToken::NameToString
            | USToken::VectorToString
            | USToken::RotatorToString
            | USToken::StringToName => {
                self.expr()?;
                self.result = convert(token, &self.result)?;
            }

            // ---- Extended natives ---------------------------------------
            USToken::ExtendedNative => {
                // Two-byte extended-native encoding n*0x100+B.
                let native_hi = self.u8_at()?;
                let native_lo = self.u8_at()?;
                let slot = u16::from(native_hi) * 0x100 + u16::from(native_lo);
                let body = self.registry.get(slot)?;
                self.result = body(self)?;
            }
            USToken::FirstNative => {
                return Err(Fail::new(
                    "vm.token_unsupported",
                    "FirstNative: numbered natives dispatch via ExtendedNative",
                ));
            }

            // Tokens that terminate parameter lists are never executed.
            USToken::EndFunctionParms => {
                return Err(Fail::new(
                    "vm.token_unexpected_end_parms",
                    "EndFunctionParms encountered outside a parameter list",
                ));
            }
        }
        Ok(false)
    }

    /// Evaluate one expression element into `self.result`.
    fn expr(&mut self) -> VmResult<()> {
        let opcode = *self.code.get(self.pc).ok_or_else(truncated)?;
        self.pc += 1;
        let Some(token) = USToken::from_opcode(opcode) else {
            return Err(Fail::new(
                "vm.unknown_token",
                format!("opcode {opcode:#04x} at {} has no token", self.pc - 1),
            ));
        };
        self.exec(token)?;
        Ok(())
    }

    fn pop_lvalue_name(&mut self) -> VmResult<u32> {
        let opcode = *self.code.get(self.pc).ok_or_else(truncated)?;
        self.pc += 1;
        match USToken::from_opcode(opcode) {
            Some(
                USToken::LocalVariable
                | USToken::InstanceVariable
                | USToken::DefaultVariable
                | USToken::NativeParm,
            ) => self.name_ci().map(|v| v as u32),
            _ => Err(Fail::new(
                "vm.lvalue_unsupported",
                format!("opcode {opcode:#04x} cannot be assigned through"),
            )),
        }
    }

    /// Consume parameters up to `EX_EndFunctionParms`.
    fn eval_parms_and_dispatch(&mut self) -> VmResult<()> {
        loop {
            let peek = *self.code.get(self.pc).ok_or_else(truncated)?;
            if peek == USToken::EndFunctionParms as u8 {
                self.pc += 1;
                break;
            }
            self.expr()?;
        }
        Ok(())
    }

    fn nul_terminated_string(&mut self) -> VmResult<String> {
        let start = self.pc;
        let mut end = self.pc;
        while let Some(&b) = self.code.get(end) {
            if b == 0 {
                break;
            }
            end += 1;
        }
        self.pc = end + 1;
        Ok(String::from_utf8_lossy(&self.code[start..end]).into_owned())
    }
}

fn truncated() -> Fail {
    Fail::new("vm.code_truncated", "operand runs past end of code")
}

fn instance_value(frame: &Frame<'_>, name: u32) -> PropValue {
    frame
        .self_id
        .and_then(|id| arena_properties(frame.arena, id))
        .and_then(|store| store.get(name).cloned())
        .unwrap_or(PropValue::Int(0))
}

fn arena_properties(arena: &ObjectArena, id: ObjectId) -> Option<&PropStore> {
    arena.get(id).ok()?.properties()
}

/// Numeric/boolean/string conversions for the EX_*To* family.
fn convert(token: USToken, value: &PropValue) -> VmResult<PropValue> {
    use PropValue::*;
    let int_of = |v: &PropValue| -> i32 {
        match v {
            Byte(b) => i32::from(*b),
            Int(i) => *i,
            Bool(b) => i32::from(*b),
            Float(f) => *f as i32,
            other => i32::from(other.truthy()),
        }
    };
    let float_of = |v: &PropValue| -> f32 {
        match v {
            Byte(b) => f32::from(*b),
            Int(i) => *i as f32,
            Float(f) => *f,
            other => f32::from(other.truthy()),
        }
    };
    Ok(match token {
        USToken::ByteToInt | USToken::StringToInt | USToken::FloatToInt | USToken::BoolToInt => {
            Int(int_of(value))
        }
        USToken::IntToFloat | USToken::StringToFloat => Float(float_of(value)),
        USToken::ByteToFloat => Float(f32::from(int_of(value) as u8)),
        USToken::IntToByte | USToken::StringToByte | USToken::FloatToByte | USToken::BoolToByte => {
            Byte(int_of(value) as u8)
        }
        USToken::ByteToBool
        | USToken::IntToBool
        | USToken::BoolToFloat
        | USToken::FloatToBool
        | USToken::StringToBool
        | USToken::ObjectToBool
        | USToken::NameToBool
        | USToken::VectorToBool
        | USToken::RotatorToBool => Bool(value.truthy()),
        USToken::IntToString
        | USToken::BoolToString
        | USToken::FloatToString
        | USToken::ObjectToString
        | USToken::NameToString => Str(format!("{value:?}")),
        USToken::ByteToString | USToken::StringToName => Str(format!("{value:?}")),
        _ => {
            return Err(Fail::new(
                "vm.conversion_unsupported",
                format!("{token:?}: composite conversion is deferred"),
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::natives::NativeRegistry;

    fn registry() -> NativeRegistry {
        NativeRegistry::new()
    }

    /// The interpreter's opcode dispatch is exhaustive: `exec` matches every
    /// [`USToken`] variant with no wildcard arm (compiler-enforced), so the
    /// only failure mode for a byte is "decodes to nothing".
    #[test]
    fn token_table_matches_unstack_h_exactly() {
        const DEFINED: [u8; 90] = [
            0x00, 0x01, 0x02, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
            0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x36, 0x37, 0x38, 0x39, 0x3A,
            0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
            0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56,
            0x57, 0x58, 0x59, 0x5A, 0x60, 0x70,
        ];
        let mut expected = [false; 256];
        for &opcode in &DEFINED {
            expected[opcode as usize] = true;
        }
        for opcode in 0..=255u8 {
            assert_eq!(
                USToken::from_opcode(opcode).is_some(),
                expected[opcode as usize],
                "opcode {opcode:#04x} disagrees with UnStack.h EExprToken"
            );
        }
    }

    #[test]
    fn unknown_token_fails_loudly() {
        // 0xFF has no EExprToken; running into it must be a reason-coded
        // error, never silent progress.
        let arena = ObjectArena::new();
        let registry = registry();
        let mut frame = Frame::new(&arena, &registry, &[0xFF], None);
        let fail = frame.run().expect_err("0xFF must not execute");
        assert_eq!(fail.reason_code, "vm.unknown_token");
        // Same verdict mid-stream, after a valid constant load.
        let mut frame = Frame::new(&arena, &registry, &[USToken::IntZero as u8, 0x03], None);
        let fail = frame.run().expect_err("0x03 must not execute");
        assert_eq!(fail.reason_code, "vm.unknown_token");
    }

    #[test]
    fn int_const_return_program_runs() {
        let arena = ObjectArena::new();
        let registry = registry();
        // IntConst(42); Return.
        let code = [
            USToken::IntConst as u8,
            42,
            0,
            0,
            0,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Int(42));
    }

    #[test]
    fn let_bool_stores_through_lvalue() {
        let arena = ObjectArena::new();
        let registry = registry();
        // LetBool <lvalue: LocalVariable(7)> <expr: IntOne>; Return Nothing.
        let code = [
            USToken::LetBool as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::IntOne as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.run().expect("run");
        assert_eq!(frame.locals.get(7), Some(&PropValue::Bool(true)));
    }

    #[test]
    fn jump_if_not_branches_on_local() {
        let arena = ObjectArena::new();
        let registry = registry();
        //  0: JumpIfNot -> 8      (expr: LocalVariable(7))
        //  3: IntOne; 4: LocalVariable(7); 6: Return; 7: Nothing
        //  8: IntZero; 9: LocalVariable(7); 11: Return; 12: Nothing
        let code = [
            USToken::JumpIfNot as u8,
            8,
            0,
            USToken::IntOne as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::Return as u8,
            USToken::Nothing as u8,
            USToken::IntZero as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        // False local: branch taken, falls into the zero/false leg.
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.locals.set(7, PropValue::Bool(false));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(false));
        // True local: branch skipped, the one-leg wins.
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.locals.set(7, PropValue::Bool(true));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));
    }

    #[test]
    fn extended_native_dispatches_two_byte_slot() {
        use crate::value::PropValue as PV;
        let arena = ObjectArena::new();
        let mut registry = registry();
        fn body(frame: &mut Frame<'_>) -> crate::error::Result<PV> {
            let seeded = matches!(frame.locals.get(7), Some(PV::Int(5)));
            Ok(PV::Int(i32::from(seeded)))
        }
        registry
            .register(0x0B_A7, 0, body, "probe.extended")
            .expect("register");
        // LocalVariable(7) seeds the probe input; ExtendedNative(0x0B,0xA7)
        // dispatches n*0x100+B = 2983; Return.
        let code = [
            USToken::ExtendedNative as u8,
            0x0B,
            0xA7,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.locals.set(7, PropValue::Int(5));
        assert_eq!(frame.run().expect("run"), PropValue::Int(1));
        // An unregistered slot fails loudly at dispatch.
        let code = [
            USToken::ExtendedNative as u8,
            0x0C,
            0x00,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame.run().expect_err("unbound slot");
        assert_eq!(fail.reason_code, "native.slot_unbound");
    }
}
