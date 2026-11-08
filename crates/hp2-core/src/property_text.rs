//! Legacy property text policy over live native reflection and storage primitives.
use crate::ansi::{self, Hp2AnsiServices, Tchar, wide};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
type Object = *mut c_void;
type Result<T = (), E = i32> = std::result::Result<T, E>;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2TextKind {
    TextGeneric,
    TextByte,
    TextInt,
    TextBool,
    TextFloat,
    TextObject,
    TextClass,
    TextName,
    TextString,
    TextFixed,
    TextArray,
    TextStruct,
    TextMap,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2TextOperation {
    TextImport,
    TextExportItem,
    TextExport,
    TextCppItem,
    TextCpp,
    TextPort,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2TextInteger {
    TextDim,
    TextSize,
    TextOffset,
    TextMask,
    TextCount,
    TextArrayNum,
    TextHasEnum,
    TextNameId,
    TextCategory,
    TextFlags,
    TextIsProperty,
    TextIsBool,
    TextIsString,
    TextEnumCount,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2TextPointer {
    TextInner,
    TextStructType,
    TextChildren,
    TextNext,
    TextSuper,
    TextData,
    TextStringData,
    TextNameData,
    TextPropertyName,
    TextCppName,
    TextPropertyClass,
    TextMetaClass,
    TextClassName,
    TextObjectPath,
    TextEnumName,
    TextTrue,
    TextFalse,
    TextKey,
    TextValue,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2TextCall {
    TextMatches,
    TextVirtualExport,
    TextVirtualExportItem,
    TextVirtualImport,
    TextVirtualCpp,
    TextVirtualPort,
    TextFindName,
    TextAssignName,
    TextAssignString,
    TextFindEnum,
    TextFindObject,
    TextFindClass,
    TextIsClass,
    TextIsChild,
    TextArrayEmpty,
    TextArrayAdd,
    TextCheckInner,
    TextCheckMap,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2TextFormat {
    TextFormatInt,
    TextFormatFloat,
    TextFormatIndex,
    TextFormatObject,
    TextFormatField,
    TextFormatArrayField,
    TextFormatCppObject,
    TextFormatCppBool,
    TextFormatCppReference,
    TextFormatCppValue,
    TextFormatCppPointer,
    TextFormatCppString,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2TextWarning {
    TextQuotedOverflow,
    TextBadQuote,
    TextTokenOverflow,
    TextArrayElement,
    TextMissingKey,
    TextUnknownElement,
    TextStructBadQuote,
    TextBadParenthesis,
    TextIncompleteStruct,
    TextBadTermination,
    TextMissingParenthesis,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PropertyTextServices {
    pub none_name: i32,
    pub class_name: i32,
    pub delimited: i32,
    pub localized: i32,
    pub transient: u32,
    pub native: u32,
    pub out_parm: u32,
    pub ansi: *const Hp2AnsiServices,
    pub atoi_file: *const c_void,
    pub atoi_line: i32,
    pub integer: unsafe extern "C" fn(Object, Hp2TextInteger) -> i32,
    pub pointer: unsafe extern "C" fn(Object, Hp2TextPointer, i32, *mut Object) -> i32,
    pub call: unsafe extern "C" fn(
        Object,
        Hp2TextCall,
        Object,
        Object,
        Object,
        i32,
        i32,
        *mut Object,
        *mut i32,
    ) -> i32,
    pub format: unsafe extern "C" fn(
        Object,
        Object,
        Hp2TextFormat,
        *const c_void,
        *const c_void,
        i32,
        f64,
        *mut i32,
    ) -> i32,
    pub log: unsafe extern "C" fn(Object, *const c_void) -> i32,
    pub warning: unsafe extern "C" fn(Object, Hp2TextWarning, *const c_void) -> i32,
}

/// Operations supplied by either the live Core adapter or the legacy service bridge.
pub trait PropertyTextOps {
    type Error;

    fn none_name(&self) -> i32;
    fn class_name(&self) -> i32;
    fn delimited(&self) -> i32;
    fn localized(&self) -> i32;
    fn transient(&self) -> u32;
    fn native(&self) -> u32;
    fn out_parm(&self) -> u32;
    fn integer(&self, property: Object, kind: Hp2TextInteger) -> i32;
    fn pointer(&self, property: Object, kind: Hp2TextPointer, index: i32) -> Result<Object, Self::Error>;
    fn call(&self, property: Object, operation: Hp2TextCall, a: Object, b: Object, c: Object, n: i32, flags: i32) -> Result<(Object, i32), Self::Error>;
    fn format(&self, out: Object, dest: *mut Tchar, kind: Hp2TextFormat, a: *const Tchar, b: *const Tchar, n: i32, f: f64) -> Result<i32, Self::Error>;
    fn log(&self, out: Object, text: *const Tchar) -> Result<(), Self::Error>;
    fn warning(&self, property: Object, kind: Hp2TextWarning, name: *const Tchar) -> Result<(), Self::Error>;
    fn atoi(&self, text: *const Tchar) -> Result<i32, Self::Error>;
    fn atof(&self, text: *const Tchar) -> Result<f32, Self::Error>;
}

struct LegacyOps<'a>(&'a Hp2PropertyTextServices);
impl PropertyTextOps for LegacyOps<'_> {
    type Error = i32;
    fn none_name(&self) -> i32 { self.0.none_name }
    fn class_name(&self) -> i32 { self.0.class_name }
    fn delimited(&self) -> i32 { self.0.delimited }
    fn localized(&self) -> i32 { self.0.localized }
    fn transient(&self) -> u32 { self.0.transient }
    fn native(&self) -> u32 { self.0.native }
    fn out_parm(&self) -> u32 { self.0.out_parm }
    fn integer(&self, property: Object, kind: Hp2TextInteger) -> i32 {
        unsafe { (self.0.integer)(property, kind) }
    }
    fn pointer(&self, property: Object, kind: Hp2TextPointer, index: i32) -> Result<Object> {
        let mut result = null_mut();
        status(unsafe { (self.0.pointer)(property, kind, index, &mut result) })?;
        Ok(result)
    }
    fn call(&self, property: Object, operation: Hp2TextCall, a: Object, b: Object, c: Object, n: i32, flags: i32) -> Result<(Object, i32)> {
        let mut result = null_mut();
        let mut number = 0;
        status(unsafe { (self.0.call)(property, operation, a, b, c, n, flags, &mut result, &mut number) })?;
        Ok((result, number))
    }
    fn format(&self, out: Object, dest: *mut Tchar, kind: Hp2TextFormat, a: *const Tchar, b: *const Tchar, n: i32, f: f64) -> Result<i32> {
        let mut count = 0;
        status(unsafe { (self.0.format)(out, dest.cast(), kind, a.cast(), b.cast(), n, f, &mut count) })?;
        Ok(count)
    }
    fn log(&self, out: Object, text: *const Tchar) -> Result<()> {
        status(unsafe { (self.0.log)(out, text.cast()) })
    }
    fn warning(&self, property: Object, kind: Hp2TextWarning, name: *const Tchar) -> Result<()> {
        status(unsafe { (self.0.warning)(property, kind, name.cast()) })
    }
    fn atoi(&self, text: *const Tchar) -> Result<i32> {
        unsafe { ansi::atoi(&*self.0.ansi, text, self.0.atoi_file, self.0.atoi_line).map_err(|_| 1) }
    }
    fn atof(&self, text: *const Tchar) -> Result<f32> {
        unsafe { ansi::atof(&*self.0.ansi, text).map_err(|_| 1) }
    }
}

macro_rules! w {
    ($s:literal) => {
        wide($s).as_ptr()
    };
}
fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}
fn at(p: Object, offset: i32) -> Object {
    p.cast::<u8>().wrapping_offset(offset as isize).cast()
}
fn optional_at(p: Object, offset: i32) -> Object {
    if p.is_null() { p } else { at(p, offset) }
}
fn digit(c: Tchar) -> bool {
    c >= 48 && c <= 57
}
fn alnum(c: Tchar) -> bool {
    digit(c) || (c >= 65 && c <= 90) || (c >= 97 && c <= 122)
}
fn hex(c: Tchar) -> Tchar {
    match c {
        48..=57 => c - 48,
        65..=70 => c - 55,
        97..=102 => c - 87,
        _ => 0,
    }
}
unsafe fn len(mut p: *const Tchar) -> usize {
    unsafe {
        let start = p;
        while *p != 0 {
            p = p.add(1);
        }
        p.offset_from(start) as usize
    }
}
unsafe fn ptr<O: PropertyTextOps>(s: &O, p: Object, f: Hp2TextPointer, index: i32) -> Result<Object, O::Error> {
    s.pointer(p, f, index)
}
unsafe fn call<O: PropertyTextOps>(
    s: &O, p: Object, op: Hp2TextCall, a: Object, b: Object, c: Object, n: i32, flags: i32,
) -> Result<(Object, i32), O::Error> {
    s.call(p, op, a, b, c, n, flags)
}
unsafe fn format<O: PropertyTextOps>(
    s: &O, out: Object, dest: *mut Tchar, kind: Hp2TextFormat,
    a: *const Tchar, b: *const Tchar, n: i32, f: f64,
) -> Result<i32, O::Error> {
    s.format(out, dest, kind, a, b, n, f)
}
unsafe fn warning<O: PropertyTextOps>(
    s: &O, p: Object, kind: Hp2TextWarning, name: *const Tchar,
) -> Result<(), O::Error> {
    s.warning(p, kind, name)
}
unsafe fn log<O: PropertyTextOps>(s: &O, out: Object, text: *const Tchar) -> Result<(), O::Error> {
    s.log(out, text)
}
unsafe fn atoi<O: PropertyTextOps>(s: &O, text: *const Tchar) -> Result<i32, O::Error> {
    s.atoi(text)
}

/// Native service adapter for the shared token grammar.
///
/// # Safety
/// Source and destination satisfy the token-buffer contract of read_token_with.
pub unsafe fn read_token(
    buffer: *const Tchar,
    string: *mut Tchar,
    maximum: i32,
    dotted: bool,
    services: &Hp2PropertyTextServices,
) -> Result<*const Tchar, i32> {
    unsafe { read_token_with(buffer, string, maximum, dotted, &LegacyOps(services)) }
}

/// The caller provides the original writable token buffer and readable source, including
/// the legacy two-character escape lookahead. Errors deliberately leave partial output
/// unterminated. A punctuation token deliberately does not advance the source.
pub unsafe fn read_token_with<O: PropertyTextOps>(
    mut buffer: *const Tchar,
    string: *mut Tchar,
    maximum: i32,
    dotted: bool,
    s: &O,
) -> Result<*const Tchar, O::Error> {
    unsafe {
        let mut count = 0i32;
        if *buffer == 34 {
            buffer = buffer.add(1);
            while *buffer != 0
                && *buffer != 34
                && *buffer != 13
                && *buffer != 10
                && count < maximum - 1
            {
                if *buffer != 92 {
                    *string.offset(count as isize) = *buffer;
                    buffer = buffer.add(1);
                } else {
                    buffer = buffer.add(1);
                    if *buffer == 92 {
                        *string.offset(count as isize) = 92;
                        buffer = buffer.add(1);
                    } else {
                        *string.offset(count as isize) = hex(*buffer) * 16 + hex(*buffer.add(1));
                        buffer = buffer.add(2);
                    }
                }
                count += 1;
            }
            if count == maximum - 1 {
                warning(s, null_mut(), Hp2TextWarning::TextQuotedOverflow, null())?;
                return Ok(null());
            }
            let end = *buffer;
            buffer = buffer.add(1);
            if end != 34 {
                warning(s, null_mut(), Hp2TextWarning::TextBadQuote, null())?;
                return Ok(null());
            }
        } else if alnum(*buffer) {
            while (alnum(*buffer) || *buffer == 95 || *buffer == 45 || (dotted && *buffer == 46))
                && count < maximum - 1
            {
                *string.offset(count as isize) = *buffer;
                count += 1;
                buffer = buffer.add(1);
            }
            if count == maximum - 1 {
                warning(s, null_mut(), Hp2TextWarning::TextTokenOverflow, null())?;
                return Ok(null());
            }
        } else {
            *string = *buffer;
            count = 1;
        }
        *string.offset(count as isize) = 0;
        Ok(buffer)
    }
}

// TFieldIterator's cursor remains live across virtual calls; no metadata snapshot or
// references into engine-owned memory are retained across reentry.
struct Fields { owner: Object, field: Object }
impl Fields {
    unsafe fn new<O: PropertyTextOps>(owner: Object, s: &O) -> Result<Self, O::Error> {
        let mut f = Self { owner, field: if owner.is_null() { null_mut() } else {
            unsafe { ptr(s, owner, Hp2TextPointer::TextChildren, 0)? }
        }};
        unsafe { f.seek(s)?; }
        Ok(f)
    }
    unsafe fn seek<O: PropertyTextOps>(&mut self, s: &O) -> Result<(), O::Error> {
        unsafe {
            while !self.owner.is_null() {
                while !self.field.is_null() {
                    if s.integer(self.field, Hp2TextInteger::TextIsProperty) != 0 { return Ok(()); }
                    self.field = ptr(s, self.field, Hp2TextPointer::TextNext, 0)?;
                }
                self.owner = ptr(s, self.owner, Hp2TextPointer::TextSuper, 0)?;
                if !self.owner.is_null() { self.field = ptr(s, self.owner, Hp2TextPointer::TextChildren, 0)?; }
            }
            Ok(())
        }
    }
    unsafe fn advance<O: PropertyTextOps>(&mut self, s: &O) -> Result<(), O::Error> {
        unsafe { self.field = ptr(s, self.field, Hp2TextPointer::TextNext, 0)?; self.seek(s) }
    }
}
unsafe fn value_offset<O: PropertyTextOps>(s: &O, p: Object, index: i32) -> i32 {
    s.integer(p, Hp2TextInteger::TextOffset)
        .wrapping_add(index.wrapping_mul(s.integer(p, Hp2TextInteger::TextSize)))
}

/// # Safety
/// Native values, output storage and reflected objects must satisfy the original
/// property's lifetime, alignment and capacity contracts throughout synchronous reentry.
pub unsafe fn run(
    kind: Hp2TextKind, op: Hp2TextOperation, p: Object, a: Object, b: Object, c: Object,
    index: i32, flags: i32, services: &Hp2PropertyTextServices,
) -> Result<(Object, i32)> {
    unsafe { run_with(kind, op, p, a, b, c, index, flags, &LegacyOps(services)) }
}

/// Runs property text grammar over generic operations.
///
/// # Safety
/// Native values and reflected metadata must remain valid for every synchronous operation.
pub unsafe fn run_with<O: PropertyTextOps>(
    kind: Hp2TextKind, op: Hp2TextOperation, p: Object, a: Object, b: Object, c: Object,
    index: i32, flags: i32, s: &O,
) -> Result<(Object, i32), O::Error> {
    use Hp2TextOperation::*;
    unsafe {
        match op {
            TextImport => Ok((import(kind, p, a.cast(), b, flags, s)?.cast_mut().cast(), 0)),
            TextExportItem => { export_item(kind, p, a.cast(), b, c, flags, s)?; Ok((null_mut(), 0)) }
            TextExport => {
                *a.cast::<Tchar>() = 0;
                if b == c || call(s, p, Hp2TextCall::TextMatches, b, c, null_mut(), index, 0)?.1 == 0 {
                    let offset = value_offset(s, p, index);
                    call(s, p, Hp2TextCall::TextVirtualExportItem, a, at(b, offset), optional_at(c, offset), 0, flags)?;
                    Ok((null_mut(), 1))
                } else { Ok((null_mut(), 0)) }
            }
            TextCppItem => { cpp_item(kind, p, a, s)?; Ok((null_mut(), 0)) }
            TextCpp => { cpp(p, a, index != 0, flags != 0, s)?; Ok((null_mut(), 0)) }
            TextPort => {
                let size = s.integer(p, Hp2TextInteger::TextSize).wrapping_mul(s.integer(p, Hp2TextInteger::TextDim));
                let result = size != 0
                    && (s.integer(p, Hp2TextInteger::TextCategory) != s.none_name()
                        || s.integer(p, Hp2TextInteger::TextFlags) as u32 & (s.transient() | s.native()) == 0)
                    && s.integer(p, Hp2TextInteger::TextNameId) != s.class_name();
                Ok((null_mut(), result as i32))
            }
        }
    }
}
unsafe fn import<O: PropertyTextOps>(
    kind: Hp2TextKind, p: Object, mut buffer: *const Tchar, data: Object, flags: i32, s: &O,
) -> Result<*const Tchar, O::Error> {
    use Hp2TextCall::*;
    use Hp2TextInteger::*;
    use Hp2TextKind::*;
    use Hp2TextPointer::*;
    unsafe {
        match kind {
            TextByte => {
                if s.integer(p, TextHasEnum) != 0 {
                    let mut temp = [0; 1024];
                    buffer = read_token_with(buffer, temp.as_mut_ptr(), 1024, false, s)?;
                    if buffer.is_null() {
                        return Ok(null());
                    }
                    let name = call(
                        s,
                        p,
                        TextFindName,
                        temp.as_mut_ptr().cast(),
                        null_mut(),
                        null_mut(),
                        0,
                        0,
                    )?
                    .1;
                    if name != s.none_name() {
                        let found = call(
                            s,
                            p,
                            TextFindEnum,
                            null_mut(),
                            null_mut(),
                            null_mut(),
                            name,
                            0,
                        )?
                        .1;
                        if found >= 0 {
                            *data.cast::<u8>() = found as u8;
                            return Ok(buffer);
                        }
                    }
                }
                if !digit(*buffer) {
                    return Ok(null());
                }
                *data.cast::<u8>() = atoi(s, buffer)? as u8;
                while digit(*buffer) {
                    buffer = buffer.add(1);
                }
            }
            TextInt => {
                if *buffer == 45 || digit(*buffer) {
                    *data.cast::<i32>() = atoi(s, buffer)?;
                }
                while *buffer == 45 || digit(*buffer) {
                    buffer = buffer.add(1);
                }
            }
            TextFloat => {
                *data.cast::<f32>() = s.atof(buffer)?;
                while *buffer != 0
                    && *buffer != 44
                    && *buffer != 41
                    && *buffer != 13
                    && *buffer != 10
                {
                    buffer = buffer.add(1);
                }
            }
            TextBool => {
                let mut temp = [0; 1024];
                buffer = read_token_with(buffer, temp.as_mut_ptr(), 1024, false, s)?;
                if buffer.is_null() {
                    return Ok(null());
                }
                let t = temp.as_ptr();
                if ansi::stricmp(t, w!(b"1\0")) == 0
                    || ansi::stricmp(t, w!(b"True\0")) == 0
                    || ansi::stricmp(t, ptr(s, p, TextTrue, 0)?.cast()) == 0
                {
                    *data.cast::<u32>() |= s.integer(p, TextMask) as u32;
                } else if ansi::stricmp(t, w!(b"0\0")) == 0
                    || ansi::stricmp(t, w!(b"False\0")) == 0
                    || ansi::stricmp(t, ptr(s, p, TextFalse, 0)?.cast()) == 0
                {
                    *data.cast::<u32>() &= !(s.integer(p, TextMask) as u32);
                } else {
                    return Ok(null());
                }
            }
            TextObject | TextClass => {
                let mut temp = [0; 1024];
                buffer = read_token_with(buffer, temp.as_mut_ptr(), 1024, true, s)?;
                if buffer.is_null() {
                    return Ok(null());
                }
                if ansi::stricmp(temp.as_ptr(), w!(b"None\0")) == 0 {
                    *data.cast::<Object>() = null_mut();
                } else {
                    while *buffer == 32 {
                        buffer = buffer.add(1);
                    }
                    let quote = *buffer;
                    buffer = buffer.add(1);
                    let object = if quote != 39 {
                        let class = ptr(s, p, TextPropertyClass, 0)?;
                        call(
                            s,
                            class,
                            TextFindObject,
                            temp.as_mut_ptr().cast(),
                            null_mut(),
                            null_mut(),
                            0,
                            0,
                        )?
                        .0
                    } else {
                        let mut other = [0; 1024];
                        buffer = read_token_with(buffer, other.as_mut_ptr(), 1024, true, s)?;
                        if buffer.is_null() {
                            return Ok(null());
                        }
                        let end = *buffer;
                        buffer = buffer.add(1);
                        if end != 39 {
                            return Ok(null());
                        }
                        let class = call(
                            s,
                            p,
                            TextFindClass,
                            temp.as_mut_ptr().cast(),
                            null_mut(),
                            null_mut(),
                            0,
                            0,
                        )?
                        .0;
                        if class.is_null() {
                            return Ok(null());
                        }
                        call(
                            s,
                            class,
                            TextFindObject,
                            other.as_mut_ptr().cast(),
                            null_mut(),
                            null_mut(),
                            0,
                            0,
                        )?
                        .0
                    };
                    *data.cast::<Object>() = object;
                    if object.is_null() {
                        return Ok(null());
                    }
                }
                if kind == TextClass {
                    let class = *data.cast::<Object>();
                    // Keep the native precedence: IsChildOf is evaluated even for NULL.
                    // Its native traversal treats a NULL receiver as an empty ancestry.
                    if (!class.is_null()
                        && call(
                            s,
                            class,
                            TextIsClass,
                            null_mut(),
                            null_mut(),
                            null_mut(),
                            0,
                            0,
                        )?
                        .1 == 0)
                        || call(
                            s,
                            class,
                            TextIsChild,
                            ptr(s, p, TextMetaClass, 0)?,
                            null_mut(),
                            null_mut(),
                            0,
                            0,
                        )?
                        .1 == 0
                    {
                        *data.cast::<Object>() = null_mut();
                    }
                }
            }
            TextName => {
                let mut temp = [0; 1024];
                buffer = read_token_with(buffer, temp.as_mut_ptr(), 1024, false, s)?;
                if buffer.is_null() {
                    return Ok(null());
                }
                call(
                    s,
                    p,
                    TextAssignName,
                    data,
                    temp.as_mut_ptr().cast(),
                    null_mut(),
                    0,
                    0,
                )?;
            }
            TextString => {
                if flags & s.delimited() == 0 {
                    call(
                        s,
                        p,
                        TextAssignString,
                        data,
                        buffer.cast_mut().cast(),
                        null_mut(),
                        0,
                        0,
                    )?;
                } else {
                    let mut temp = [0; 4096];
                    buffer = read_token_with(buffer, temp.as_mut_ptr(), 4096, false, s)?;
                    if buffer.is_null() {
                        return Ok(null());
                    }
                    call(
                        s,
                        p,
                        TextAssignString,
                        data,
                        temp.as_mut_ptr().cast(),
                        null_mut(),
                        0,
                        0,
                    )?;
                }
            }
            TextFixed | TextArray => {
                call(
                    s,
                    p,
                    TextCheckInner,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    0,
                    0,
                )?;
                let open = *buffer;
                buffer = buffer.add(1);
                if open != 40 {
                    return Ok(null());
                }
                if kind == TextFixed {
                    data.cast::<u8>().write_bytes(0, s.integer(p, TextSize) as usize);
                    let mut i = 0;
                    while i < s.integer(p, TextCount) {
                        let inner = ptr(s, p, TextInner, 0)?;
                        buffer = call(
                            s,
                            inner,
                            TextVirtualImport,
                            buffer.cast_mut().cast(),
                            at(data, i.wrapping_mul(s.integer(inner, TextSize))),
                            null_mut(),
                            0,
                            flags | s.delimited(),
                        )?
                        .0
                        .cast();
                        if buffer.is_null() {
                            return Ok(null());
                        }
                        if *buffer != 44 && i != s.integer(p, TextCount) - 1 {
                            return Ok(null());
                        }
                        // Includes the last element: the historical parser requires the
                        // extra character before its closing-parenthesis check.
                        buffer = buffer.add(1);
                        i += 1;
                    }
                } else {
                    let size = s.integer(ptr(s, p, TextInner, 0)?, TextSize);
                    call(s, p, TextArrayEmpty, data, null_mut(), null_mut(), size, 0)?;
                    while *buffer != 41 {
                        let i = call(s, p, TextArrayAdd, data, null_mut(), null_mut(), size, 0)?.1;
                        let slot = at(ptr(s, data, TextData, 0)?, i.wrapping_mul(size));
                        slot.cast::<u8>().write_bytes(0, size as usize);
                        let inner = ptr(s, p, TextInner, 0)?;
                        buffer = call(
                            s,
                            inner,
                            TextVirtualImport,
                            buffer.cast_mut().cast(),
                            slot,
                            null_mut(),
                            0,
                            flags | s.delimited(),
                        )?
                        .0
                        .cast();
                        if buffer.is_null() {
                            return Ok(null());
                        }
                        if *buffer != 44 {
                            break;
                        }
                        buffer = buffer.add(1);
                    }
                }
                let close = *buffer;
                buffer = buffer.add(1);
                if close != 41 {
                    return Ok(null());
                }
            }
            TextStruct => return import_struct(p, buffer, data, flags, s),
            TextMap | TextGeneric => return Ok(null()),
        }
        Ok(buffer)
    }
}
unsafe fn import_struct<O: PropertyTextOps>(
    p: Object, mut buffer: *const Tchar, data: Object, flags: i32, s: &O,
) -> Result<*const Tchar, O::Error> {
    use Hp2TextCall::*;
    use Hp2TextInteger::*;
    use Hp2TextPointer::*;
    use Hp2TextWarning::*;
    unsafe {
        let open = *buffer;
        buffer = buffer.add(1);
        if open != 40 {
            warning(s, p, TextMissingParenthesis, null())?;
            return Ok(null());
        }
        while *buffer != 41 {
            let mut name = [0; 128];
            let mut count = 0;
            while count < 127 && *buffer != 0 && *buffer != 61 && *buffer != 91 {
                name[count] = *buffer;
                count += 1;
                buffer = buffer.add(1);
            }
            name[count] = 0;
            let mut element = 0;
            if *buffer == 91 {
                buffer = buffer.add(1);
                let start = buffer;
                while digit(*buffer) {
                    buffer = buffer.add(1);
                }
                let close = *buffer;
                buffer = buffer.add(1);
                if close != 93 {
                    warning(s, p, TextArrayElement, null())?;
                    return Ok(null());
                }
                element = atoi(s, start)?;
            }
            let eq = *buffer;
            buffer = buffer.add(1);
            if eq != 61 {
                warning(s, p, TextMissingKey, null())?;
                return Ok(null());
            }
            let got = call(
                s,
                p,
                TextFindName,
                name.as_mut_ptr().cast(),
                null_mut(),
                null_mut(),
                0,
                0,
            )?
            .1;
            let mut parsed = false;
            if got != s.none_name() {
                let mut fields = Fields::new(ptr(s, p, TextStructType, 0)?, s)?;
                while !fields.field.is_null() {
                    let field = fields.field;
                    if s.integer(field, TextNameId) == got
                        && element >= 0
                        && element < s.integer(field, TextDim)
                        && s.integer(field, TextSize).wrapping_mul(s.integer(field, TextDim)) != 0
                        && call(
                            s,
                            field,
                            TextVirtualPort,
                            null_mut(),
                            null_mut(),
                            null_mut(),
                            0,
                            0,
                        )?
                        .1 != 0
                    {
                        buffer = call(
                            s,
                            field,
                            TextVirtualImport,
                            buffer.cast_mut().cast(),
                            at(data, value_offset(s, field, element)),
                            null_mut(),
                            0,
                            flags | s.delimited(),
                        )?
                        .0
                        .cast();
                        if buffer.is_null() {
                            return Ok(null());
                        }
                        parsed = true;
                    }
                    fields.advance(s)?;
                }
            }
            if !parsed {
                warning(s, p, TextUnknownElement, name.as_ptr())?;
                let mut sub = 0;
                while *buffer != 0
                    && *buffer != 10
                    && *buffer != 13
                    && (sub > 0 || *buffer != 41)
                    && (sub > 0 || *buffer != 44)
                {
                    // The old quote branch never advances before checking its opening
                    // quote. Keep commas/parentheses inside quotes visible to the skip.
                    if *buffer == 34 {
                        while *buffer != 0 && *buffer != 34 && *buffer != 10 && *buffer != 13 {
                            buffer = buffer.add(1);
                        }
                        if *buffer != 34 {
                            warning(s, p, TextStructBadQuote, null())?;
                            return Ok(null());
                        }
                    } else if *buffer == 40 {
                        sub += 1;
                    } else if *buffer == 41 {
                        sub -= 1;
                        if sub < 0 {
                            warning(s, p, TextBadParenthesis, null())?;
                            return Ok(null());
                        }
                    }
                    buffer = buffer.add(1);
                }
                if sub > 0 {
                    warning(s, p, TextIncompleteStruct, null())?;
                    return Ok(null());
                }
            }
            if *buffer == 44 {
                buffer = buffer.add(1);
            } else if *buffer != 41 {
                warning(s, p, TextBadTermination, null())?;
                return Ok(null());
            }
        }
        Ok(buffer.add(1))
    }
}
unsafe fn export_item<O: PropertyTextOps>(
    kind: Hp2TextKind, p: Object, mut dest: *mut Tchar, data: Object, default: Object, flags: i32, s: &O,
) -> Result<(), O::Error> {
    use Hp2TextCall::*;
    use Hp2TextFormat::*;
    use Hp2TextInteger::*;
    use Hp2TextKind::*;
    use Hp2TextPointer::*;
    unsafe {
        match kind {
            TextByte => {
                let value = *data.cast::<u8>();
                // Rust policy: a value past the enum's name table (UE1 indexed it
                // unchecked) exports as its decimal value, like a non-enum byte.
                if s.integer(p, TextHasEnum) != 0
                    && i32::from(value) < s.integer(p, TextEnumCount)
                {
                    ansi::strcpy(dest, ptr(s, p, TextEnumName, value as i32)?.cast());
                } else {
                    format(
                        s,
                        null_mut(),
                        dest,
                        TextFormatInt,
                        null(),
                        null(),
                        value as i32,
                        0.0,
                    )?;
                }
            }
            TextInt => {
                format(
                    s,
                    null_mut(),
                    dest,
                    TextFormatInt,
                    null(),
                    null(),
                    *data.cast::<i32>(),
                    0.0,
                )?;
            }
            TextFloat => {
                format(
                    s,
                    null_mut(),
                    dest,
                    TextFormatFloat,
                    null(),
                    null(),
                    0,
                    *data.cast::<f32>() as f64,
                )?;
            }
            TextBool => {
                let value = *data.cast::<u32>() & s.integer(p, TextMask) as u32 != 0;
                if flags & s.localized() != 0 {
                    ansi::strcpy(
                        dest,
                        ptr(s, p, if value { TextTrue } else { TextFalse }, 0)?.cast(),
                    );
                } else if value {
                    ansi::strcpy(dest, w!(b"True\0"));
                } else {
                    ansi::strcpy(dest, w!(b"False\0"));
                }
            }
            TextObject | TextClass => {
                let object = *data.cast::<Object>();
                if object.is_null() {
                    ansi::strcpy(dest, w!(b"None\0"));
                } else {
                    let class = ptr(s, object, TextClassName, 0)?;
                    let path = ptr(s, object, TextObjectPath, 0)?;
                    format(
                        s,
                        null_mut(),
                        dest,
                        TextFormatObject,
                        class.cast(),
                        path.cast(),
                        0,
                        0.0,
                    )?;
                }
            }
            TextName | TextString => {
                let text = ptr(
                    s,
                    data,
                    if kind == TextName {
                        TextNameData
                    } else {
                        TextStringData
                    },
                    0,
                )?
                .cast::<Tchar>();
                let mut quoted = kind == TextString && flags & s.delimited() != 0;
                if kind == TextName {
                    let mut scan = text;
                    while *scan != 0 {
                        if *scan == 32 || *scan == 44 {
                            quoted = true;
                            break;
                        }
                        scan = scan.add(1);
                    }
                }
                if quoted {
                    *dest = 34;
                    dest = dest.add(1);
                }
                ansi::strcpy(dest, text);
                if quoted {
                    dest = dest.add(len(dest));
                    *dest = 34;
                    *dest.add(1) = 0;
                }
            }
            TextFixed | TextArray => {
                call(
                    s,
                    p,
                    TextCheckInner,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    0,
                    0,
                )?;
                *dest = 40;
                dest = dest.add(1);
                let size = if kind == TextArray {
                    s.integer(ptr(s, p, TextInner, 0)?, TextSize)
                } else {
                    0
                };
                let mut i = 0;
                while i < if kind == TextArray {
                    s.integer(data, TextArrayNum)
                } else {
                    s.integer(p, TextCount)
                } {
                    if i > 0 {
                        *dest = 44;
                        dest = dest.add(1);
                    }
                    let inner = ptr(s, p, TextInner, 0)?;
                    let (value, delta) = if kind == TextArray {
                        (
                            at(ptr(s, data, TextData, 0)?, i.wrapping_mul(size)),
                            if default.is_null() {
                                null_mut()
                            } else {
                                at(ptr(s, default, TextData, 0)?, i.wrapping_mul(size))
                            },
                        )
                    } else {
                        let offset = i.wrapping_mul(s.integer(inner, TextSize));
                        (at(data, offset), optional_at(default, offset))
                    };
                    call(
                        s,
                        inner,
                        TextVirtualExportItem,
                        dest.cast(),
                        value,
                        delta,
                        0,
                        flags | s.delimited(),
                    )?;
                    dest = dest.add(len(dest));
                    i += 1;
                }
                *dest = 41;
                *dest.add(1) = 0;
            }
            TextStruct => {
                let mut count = 0;
                let mut fields = Fields::new(ptr(s, p, TextStructType, 0)?, s)?;
                while !fields.field.is_null() {
                    let field = fields.field;
                    if call(
                        s,
                        field,
                        TextVirtualPort,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                        0,
                        0,
                    )?
                    .1 != 0
                    {
                        let mut index = 0;
                        while index < s.integer(field, TextDim) {
                            let mut value = [0; 1024];
                            if call(
                                s,
                                field,
                                TextVirtualExport,
                                value.as_mut_ptr().cast(),
                                data,
                                default,
                                index,
                                s.delimited(),
                            )?
                            .1 != 0
                            {
                                count += 1;
                                *dest = if count == 1 { 40 } else { 44 };
                                dest = dest.add(1);
                                let name = ptr(s, field, TextPropertyName, 0)?;
                                let kind = if s.integer(field, TextDim) == 1 {
                                    TextFormatField
                                } else {
                                    TextFormatArrayField
                                };
                                dest = dest.offset(format(
                                    s,
                                    null_mut(),
                                    dest,
                                    kind,
                                    name.cast(),
                                    null(),
                                    index,
                                    0.0,
                                )? as isize);
                                ansi::strcpy(dest, value.as_ptr());
                                dest = dest.add(len(dest));
                            }
                            index += 1;
                        }
                    }
                    fields.advance(s)?;
                }
                // No fields means no write at all, including no terminator.
                if count > 0 {
                    *dest = 41;
                    *dest.add(1) = 0;
                }
            }
            TextGeneric | TextMap => {}
        }
        Ok(())
    }
}
unsafe fn cpp_item<O: PropertyTextOps>(
    kind: Hp2TextKind, p: Object, out: Object, s: &O,
) -> Result<(), O::Error> {
    use Hp2TextCall::*;
    use Hp2TextKind::*;
    use Hp2TextPointer::*;
    unsafe {
        match kind {
            TextByte => log(s, out, w!(b"BYTE\0"))?,
            TextInt => log(s, out, w!(b"INT\0"))?,
            TextBool => log(s, out, w!(b"BITFIELD\0"))?,
            TextFloat => log(s, out, w!(b"FLOAT\0"))?,
            TextName => log(s, out, w!(b"FName\0"))?,
            TextString => log(s, out, w!(b"FString\0"))?,
            TextObject | TextClass => {
                let class = ptr(s, p, TextPropertyClass, 0)?;
                let name = ptr(s, class, TextCppName, 0)?;
                format(
                    s,
                    out,
                    null_mut(),
                    Hp2TextFormat::TextFormatCppObject,
                    name.cast(),
                    null(),
                    0,
                    0.0,
                )?;
            }
            TextStruct => {
                let owner = ptr(s, p, TextStructType, 0)?;
                let name = ptr(s, owner, TextCppName, 0)?;
                log(s, out, name.cast())?;
            }
            TextFixed | TextArray => {
                call(
                    s,
                    p,
                    TextCheckInner,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    0,
                    0,
                )?;
                if kind == TextArray {
                    log(s, out, w!(b"TArray<\0"))?;
                }
                let inner = ptr(s, p, TextInner, 0)?;
                call(s, inner, TextVirtualCpp, out, null_mut(), null_mut(), 0, 0)?;
                if kind == TextArray {
                    log(s, out, w!(b">\0"))?;
                } else {
                    format(
                        s,
                        out,
                        null_mut(),
                        Hp2TextFormat::TextFormatIndex,
                        null(),
                        null(),
                        s.integer(p, Hp2TextInteger::TextCount),
                        0.0,
                    )?;
                }
            }
            TextMap => {
                call(s, p, TextCheckMap, null_mut(), null_mut(), null_mut(), 0, 0)?;
                log(s, out, w!(b"TMap<\0"))?;
                call(
                    s,
                    ptr(s, p, TextKey, 0)?,
                    TextVirtualCpp,
                    out,
                    null_mut(),
                    null_mut(),
                    0,
                    0,
                )?;
                log(s, out, w!(b",\0"))?;
                call(
                    s,
                    ptr(s, p, TextValue, 0)?,
                    TextVirtualCpp,
                    out,
                    null_mut(),
                    null_mut(),
                    0,
                    0,
                )?;
                log(s, out, w!(b">\0"))?;
            }
            TextGeneric => {}
        }
        Ok(())
    }
}
unsafe fn cpp<O: PropertyTextOps>(
    p: Object, out: Object, local: bool, parm: bool, s: &O,
) -> Result<(), O::Error> {
    use Hp2TextFormat::*;
    use Hp2TextInteger::*;
    unsafe {
        let mut array = [0; 80];
        if parm
            && s.integer(p, TextIsString) != 0
            && s.integer(p, TextFlags) as u32 & s.out_parm() == 0
        {
            log(s, out, w!(b"const \0"))?;
        }
        call(
            s,
            p,
            Hp2TextCall::TextVirtualCpp,
            out,
            null_mut(),
            null_mut(),
            0,
            0,
        )?;
        if s.integer(p, TextDim) != 1 {
            format(
                s,
                null_mut(),
                array.as_mut_ptr(),
                TextFormatIndex,
                null(),
                null(),
                s.integer(p, TextDim),
                0.0,
            )?;
        }
        let style = if s.integer(p, TextIsBool) != 0 {
            if s.integer(p, TextDim) == 1 && !local && !parm {
                TextFormatCppBool
            } else if parm && s.integer(p, TextFlags) as u32 & s.out_parm() != 0 {
                TextFormatCppReference
            } else {
                TextFormatCppValue
            }
        } else if s.integer(p, TextIsString) != 0 {
            if parm && s.integer(p, TextDim) > 1 {
                TextFormatCppPointer
            } else if parm {
                array[0] = 0;
                TextFormatCppReference
            } else if local {
                array[0] = 0;
                TextFormatCppValue
            } else {
                TextFormatCppString
            }
        } else if parm && s.integer(p, TextDim) > 1 {
            TextFormatCppPointer
        } else if parm && s.integer(p, TextFlags) as u32 & s.out_parm() != 0 {
            TextFormatCppReference
        } else {
            TextFormatCppValue
        };
        let name = ptr(s, p, Hp2TextPointer::TextPropertyName, 0)?;
        format(
            s,
            out,
            null_mut(),
            style,
            name.cast(),
            array.as_ptr(),
            0,
            0.0,
        )?;
        Ok(())
    }
}
