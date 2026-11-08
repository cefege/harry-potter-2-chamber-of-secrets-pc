//! Checked policy for the native FURL API. Native storage is never borrowed as Rust
//! references. Native stack continuations own parser/config buffers and FString
//! temporaries; every callback returns its first exception as a status.
use crate::params::TChar;

#[repr(C)]
pub struct UrlServices {
    pub call: unsafe extern "C" fn(u32, usize, usize, usize, usize, *mut usize) -> i32,
    pub scope: unsafe extern "C" fn(u32, *const UrlArgs, *mut usize) -> i32,
}



#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct UrlArgs {
    pub url: usize,
    pub other: usize,
    pub text: usize,
    pub extra: usize,
    pub output: usize,
    pub mode: usize,
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Op {
    StaticText,
    StaticSet,
    DefaultPort,
    SetDefaultPort,
    ConfigGet,
    ConfigSet,
    Icmp,
    Nicmp,
    Cmp,
    Atoi,
    Len,
    Chr,
    Char,
    SetChar,
    Field,
    SetField,
    Port,
    SetPort,
    Count,
    Option,
    Push,
    SetOption,
    Reset,
    SetValid,
    Append,
    AppendPort,
    LogDefault,
    Internal,
    AddOption,
    GetOption,
    Literal,
    CopyField,
    CopyOption,
    FieldBool,
    FieldLen,
    StaticLen,
    CopyStaticField,
    CheckBase,
    MatchLength,
}

type Result = std::result::Result<usize, i32>;

/// Supplies storage, string, settings, and scoped-buffer operations to URL policy.
pub trait UrlOps {
    /// # Safety
    /// Every raw handle is valid for the operation identified by `code`.
    unsafe fn call(
        &self,
        code: u32,
        a: usize,
        b: usize,
        c: usize,
        d: usize,
    ) -> std::result::Result<usize, i32>;

    /// # Safety
    /// `args` and all handles it contains are valid for the requested scope.
    unsafe fn scope(&self, entry: u32, args: &UrlArgs) -> std::result::Result<usize, i32>;
}

impl UrlOps for UrlServices {
    unsafe fn call(
        &self,
        code: u32,
        a: usize,
        b: usize,
        c: usize,
        d: usize,
    ) -> Result {
        let mut out = 0;
        let status = unsafe { (self.call)(code, a, b, c, d, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }

    unsafe fn scope(&self, entry: u32, args: &UrlArgs) -> Result {
        let mut out = 0;
        let status = unsafe { (self.scope)(entry, args, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
}

struct Policy<'a, O: UrlOps>(&'a O);
impl<O: UrlOps> Policy<'_, O> {
    fn call(&self, op: Op, a: usize, b: usize, c: usize, d: usize) -> Result {
        unsafe { self.0.call(op as u32, a, b, c, d) }
    }
    fn one(&self, op: Op, a: usize) -> Result {
        self.call(op, a, 0, 0, 0)
    }
    fn two(&self, op: Op, a: usize, b: usize) -> Result {
        self.call(op, a, b, 0, 0)
    }
    fn scope(&self, entry: u32, args: &UrlArgs) -> Result {
        unsafe { self.0.scope(entry, args) }
    }
    fn field(&self, u: usize, field: usize) -> Result {
        self.two(Op::Field, u, field)
    }
    fn set(&self, u: usize, field: usize, text: usize) -> Result {
        self.call(Op::SetField, u, field, text, 0)
    }
    fn literal(&self, id: usize) -> Result {
        self.one(Op::Literal, id)
    }
    fn chr(&self, text: usize, ch: u8) -> Result {
        self.two(Op::Chr, text, ch as usize)
    }
    fn char(&self, text: usize) -> Result {
        self.one(Op::Char, text)
    }
    fn terminate(&self, text: usize) -> Result {
        self.two(Op::SetChar, text, 0)
    }
    fn split(&self, text: usize) -> Result {
        let a = self.chr(text, b'?')?;
        let b = self.chr(text, b'#')?;
        Ok(if a != 0 && (b == 0 || a < b) { a } else { b })
    }
    fn invalid(&self, u: usize) -> Result {
        self.one(Op::Reset, u)?;
        self.two(Op::SetValid, u, 0)
    }
    fn same_default(&self, u: usize, field: usize, id: usize) -> std::result::Result<bool, i32> {
        Ok(self.two(
            Op::Icmp,
            self.field(u, field)?,
            self.one(Op::StaticText, id)?,
        )? == 0)
    }
    fn ext_mismatch(&self, dot: usize, id: usize) -> std::result::Result<bool, i32> {
        let ext = self.one(Op::StaticText, id)?;
        let len = self.one(Op::StaticLen, id)?;
        if self.call(Op::Nicmp, next(dot), ext, len, 0)? != 0 {
            return Ok(true);
        }
        let n = self.one(Op::StaticLen, id)?;
        let ch = self.char(advance(dot, n + 1))?;
        Ok((b'a' as usize..=b'z' as usize).contains(&ch)
            || (b'A' as usize..=b'Z' as usize).contains(&ch)
            || (b'0' as usize..=b'9' as usize).contains(&ch))
    }
    fn parse(&self, a: &UrlArgs, buffer: usize) -> Result {
        let u = a.url;
        if a.mode == 2 {
            self.one(Op::CheckBase, a.other)?;
            for field in 0..4 {
                self.call(Op::CopyField, u, a.other, field, 0)?;
            }
            self.two(Op::SetPort, u, self.one(Op::Port, a.other)?)?;
        }
        if a.mode == 2 || a.mode == 1 {
            self.one(Op::CheckBase, a.other)?;
            let mut i = 0;
            while i < self.one(Op::Count, a.other)? {
                let mut skip = false;
                for word in 14..18 {
                    if self.two(
                        Op::Icmp,
                        self.two(Op::Option, a.other, i)?,
                        self.literal(word)?,
                    )? == 0
                    {
                        skip = true;
                        break;
                    }
                }
                if !skip {
                    self.call(Op::CopyOption, u, a.other, i, 0)?;
                }
                i += 1;
            }
        }
        let mut text = buffer;
        while self.char(text)? == b' ' as usize {
            text = next(text);
        }
        let first = self.split(text)?;
        if first != 0 {
            let mut kind = self.char(first)?;
            let mut next_kind = 0;
            self.terminate(first)?;
            let mut s = next(first);
            loop {
                let mut t = self.split(s)?;
                if t != 0 {
                    next_kind = self.char(t)?;
                    self.terminate(t)?;
                    t = next(t);
                }
                if self.chr(s, b' ')? != 0 {
                    return self.invalid(u);
                }
                if kind == b'?' as usize {
                    self.two(Op::AddOption, u, s)?;
                } else {
                    self.set(u, 3, s)?;
                }
                s = t;
                kind = next_kind;
                if s == 0 {
                    break;
                }
            }
        }
        let mut far_host = false;
        let mut far_map = false;
        if self.one(Op::Len, text)? > 2 && self.char(next(text))? == b':' as usize {
            self.call(Op::CopyStaticField, u, 0, 0, 0)?;
            self.call(Op::CopyStaticField, u, 1, 5, 0)?;
            self.set(u, 2, text)?;
            self.call(Op::CopyStaticField, u, 3, 6, 0)?;
            text = 0;
            far_host = true;
            far_map = true;
            self.set(u, 1, self.literal(0)?)?;
        } else {
            let colon = self.chr(text, b':')?;
            if colon != 0 && colon > next(text) {
                let dot = self.chr(text, b'.')?;
                if dot == 0 || colon < dot {
                    let s = text;
                    text = next(colon);
                    self.terminate(colon)?;
                    self.set(u, 0, s)?;
                }
            }
            if self.char(text)? == b'/' as usize {
                text = next(text);
                let ch = self.char(text)?;
                text = next(text);
                if ch != b'/' as usize {
                    return self.invalid(u);
                }
                far_host = true;
                self.set(u, 1, self.literal(0)?)?;
            }
            let dot = self.chr(text, b'.')?;
            if dot != 0 && dot > text && self.ext_mismatch(dot, 7)? && self.ext_mismatch(dot, 8)? {
                let s = text;
                text = self.chr(text, b'/')?;
                if text != 0 {
                    self.terminate(text)?;
                    text = next(text);
                }
                let t = self.chr(s, b':')?;
                if t != 0 {
                    self.terminate(t)?;
                    self.two(Op::SetPort, u, self.one(Op::Atoi, next(t))?)?;
                }
                self.set(u, 1, s)?;
                if self.same_default(u, 0, 0)? {
                    self.call(Op::CopyStaticField, u, 2, 3, 0)?;
                } else {
                    self.set(u, 2, self.literal(0)?)?;
                }
                far_host = true;
            }
        }
        if a.mode == 0 && a.other != 0 && self.one(Op::Internal, u)? != 0 {
            let mut i = 0;
            while i < self.one(Op::Count, a.other)? {
                let mut inherit = false;
                for (word, len) in [
                    (18, 5),
                    (19, 5),
                    (20, 6),
                    (21, 5),
                    (22, 5),
                    (23, 6),
                    (24, 14),
                ] {
                    if self.call(
                        Op::Nicmp,
                        self.two(Op::Option, a.other, i)?,
                        self.literal(word)?,
                        len,
                        0,
                    )? == 0
                    {
                        inherit = true;
                        break;
                    }
                }
                if inherit {
                    self.scope(4, &UrlArgs { text: i, ..*a })?;
                }
                i += 1;
            }
        }
        if text != 0 && self.char(text)? != 0 {
            if self.one(Op::Internal, u)? != 0 {
                far_map = true;
                let t = self.chr(text, b'/')?;
                if t != 0 {
                    self.terminate(t)?;
                    let t = next(t);
                    let end = self.chr(t, b'/')?;
                    if end != 0 {
                        self.terminate(end)?;
                        if self.char(next(end))? != 0 {
                            return self.invalid(u);
                        }
                    }
                    self.set(u, 3, t)?;
                }
            }
            self.set(u, 2, text)?;
        }
        for field in [0, 1, 3] {
            if self.chr(self.field(u, field)?, b' ')? != 0 {
                return self.invalid(u);
            }
        }
        if !far_host && !far_map && self.one(Op::Count, u)? == 0 {
            return self.invalid(u);
        }
        Ok(0)
    }
    fn string(&self, a: &UrlArgs) -> Result {
        let u = a.url;
        let out = a.output;
        if !self.same_default(u, 0, 0)? || a.mode != 0 {
            self.two(Op::Append, out, self.field(u, 0)?)?;
            self.two(Op::Append, out, self.literal(10)?)?;
            if !self.same_default(u, 1, 5)? {
                self.two(Op::Append, out, self.literal(11)?)?;
            }
        }
        if !self.same_default(u, 1, 5)? || self.one(Op::Port, u)? != self.one(Op::DefaultPort, 0)? {
            self.two(Op::Append, out, self.field(u, 1)?)?;
            if self.one(Op::Port, u)? != self.one(Op::DefaultPort, 0)? {
                self.two(Op::Append, out, self.literal(10)?)?;
                self.two(Op::AppendPort, out, u)?;
            }
            self.two(Op::Append, out, self.literal(12)?)?;
        }
        if self.two(Op::FieldBool, u, 2)? != 0 {
            self.two(Op::Append, out, self.field(u, 2)?)?;
        }
        let mut i = 0;
        while i < self.one(Op::Count, u)? {
            self.two(Op::Append, out, self.literal(13)?)?;
            self.two(Op::Append, out, self.two(Op::Option, u, i)?)?;
            i += 1;
        }
        if self.two(Op::FieldBool, u, 3)? != 0 {
            self.two(Op::Append, out, self.literal(25)?)?;
            self.two(Op::Append, out, self.field(u, 3)?)?;
        }
        Ok(0)
    }
    fn add_option(&self, a: &UrlArgs) -> Result {
        let sep = self.chr(a.text, b'=')?;
        let len = if sep != 0 {
            self.two(Op::MatchLength, sep, a.text)?
        } else {
            self.one(Op::Len, a.text)? + 1
        };
        let mut i = 0;
        while i < self.one(Op::Count, a.url)? {
            if self.call(Op::Nicmp, self.two(Op::Option, a.url, i)?, a.text, len, 0)? == 0 {
                break;
            }
            i += 1;
        }
        if i == self.one(Op::Count, a.url)? {
            self.two(Op::Push, a.url, a.text)
        } else {
            self.call(Op::SetOption, a.url, i, a.text, 0)
        }
    }
    fn equal(&self, a: &UrlArgs) -> Result {
        for field in 0..3 {
            if self.two(
                Op::Icmp,
                self.field(a.url, field)?,
                self.field(a.other, field)?,
            )? != 0
            {
                return Ok(0);
            }
        }
        if self.one(Op::Port, a.url)? != self.one(Op::Port, a.other)?
            || self.one(Op::Count, a.url)? != self.one(Op::Count, a.other)?
        {
            return Ok(0);
        }
        let mut i = 0;
        while i < self.one(Op::Count, a.url)? {
            if self.two(
                Op::Icmp,
                self.two(Op::Option, a.url, i)?,
                self.two(Op::Option, a.other, i)?,
            )? != 0
            {
                return Ok(0);
            }
            i += 1;
        }
        Ok(1)
    }
}
fn advance(p: usize, n: usize) -> usize {
    p.wrapping_add(n.wrapping_mul(std::mem::size_of::<TChar>()))
}
fn next(p: usize) -> usize {
    advance(p, 1)
}

/// Runs the native FURL policy using the original service table.
///
/// # Safety
/// The service table and handles required by `entry` must be live.
pub unsafe fn run(svc: &UrlServices, entry: u32, args: &UrlArgs) -> Result {
    unsafe { run_with(svc, entry, args) }
}

/// Runs the shared FURL policy with any genuine operation backend.
///
/// # Safety
/// The backend and all handles required by `entry` must be live.
pub unsafe fn run_with<O: UrlOps>(ops: &O, entry: u32, a: &UrlArgs) -> Result {
    let p = Policy(ops);
    match entry {
        0 => {
            for id in 0..9 {
                p.two(Op::StaticSet, id, p.one(Op::ConfigGet, id)?)?;
            }
            p.one(
                Op::SetDefaultPort,
                p.one(Op::Atoi, p.one(Op::ConfigGet, 9)?)?,
            )
        }
        1 => {
            for id in 0..9 {
                p.two(Op::StaticSet, id, p.literal(0)?)?;
            }
            Ok(0)
        }
        2 => p.scope(1, a),
        3 => p.string(a),
        4 => Ok(usize::from(p.same_default(a.url, 0, 0)?)),
        5 => Ok(usize::from(
            p.one(Op::Internal, a.url)? != 0 && p.two(Op::FieldLen, a.url, 1)? == 0,
        )),
        6 => p.add_option(a),
        7 => p.scope(2, a),
        8 => {
            let mut i = 0;
            while i < p.one(Op::Count, a.url)? {
                p.scope(3, &UrlArgs { other: i, ..*a })?;
                i += 1;
            }
            Ok(0)
        }
        9 | 10 => {
            let mut i = 0;
            while i < p.one(Op::Count, a.url)? {
                let op = p.two(Op::Option, a.url, i)?;
                let equal = if entry == 9 {
                    p.two(Op::Icmp, op, a.text)? == 0
                } else {
                    p.call(Op::Nicmp, op, a.text, p.one(Op::Len, a.text)?, 0)? == 0
                };
                if equal {
                    return if entry == 9 {
                        Ok(1)
                    } else {
                        Ok(advance(
                            p.two(Op::Option, a.url, i)?,
                            p.one(Op::Len, a.text)?,
                        ))
                    };
                }
                i += 1;
            }
            Ok(if entry == 9 { 0 } else { a.extra })
        }
        11 => p.equal(a),
        _ => Err(-1),
    }
}

/// Continues the original native-service policy inside its matching stack scope.
///
/// # Safety
/// Called only by the matching `UrlServices` scope with a live buffer.
pub unsafe fn resume(svc: &UrlServices, entry: u32, args: &UrlArgs, buffer: usize) -> Result {
    unsafe { resume_with(svc, entry, args, buffer) }
}

/// Continues shared URL policy using a caller-owned scope buffer.
///
/// # Safety
/// `ops`, `args`, and `buffer` must remain live for the continuation.
pub unsafe fn resume_with<O: UrlOps>(ops: &O, entry: u32, a: &UrlArgs, buffer: usize) -> Result {
    let p = Policy(ops);
    match entry {
        1 => p.parse(a, buffer),
        2 => {
            let mut text = buffer;
            while p.char(text)? != 0 {
                p.two(Op::AddOption, a.url, text)?;
                text = advance(text, p.one(Op::Len, text)? + 1);
            }
            Ok(0)
        }
        3 => {
            let value = p.chr(buffer, b'=')?;
            if value != 0 {
                p.terminate(value)?;
                if p.two(Op::Icmp, buffer, a.extra)? == 0 {
                    p.call(Op::ConfigSet, a.text, buffer, next(value), a.output)?;
                }
            }
            Ok(0)
        }
        4 => {
            let value = p.call(Op::GetOption, a.url, buffer, p.literal(0)?, 0)?;
            if p.two(Op::Cmp, value, p.literal(0)?)? == 0 {
                p.one(Op::LogDefault, p.two(Op::Option, a.other, a.text)?)?;
                p.call(Op::CopyOption, a.url, a.other, a.text, 0)?;
            }
            Ok(0)
        }
        _ => Err(-1),
    }
}

/// Original path-token loop, without an owned copy or exclusive native borrow.
/// # Safety
/// `path` is null or NUL terminated; `out` has `capacity` writable characters.
pub unsafe fn file_path_to_token(path: *const TChar, out: *mut TChar, capacity: i32) -> bool {
    if out.is_null() || capacity <= 0 {
        return false;
    }
    unsafe {
        *out = 0;
    }
    if path.is_null() || unsafe { *path } == 0 {
        return false;
    }
    let mut n = 0;
    while unsafe { *path.add(n) } != 0 {
        if unsafe { *path.add(n) } == b':' as TChar
            && unsafe { *path.add(n + 1) } == b'/' as TChar
            && unsafe { *path.add(n + 2) } == b'/' as TChar
        {
            return false;
        }
        n += 1;
    }
    let mut in_options = false;
    let mut i = 0;
    while unsafe { *path.add(i) } != 0 {
        let mut ch = unsafe { *path.add(i) };
        if i >= capacity as usize - 1 || ch == b'#' as TChar {
            unsafe {
                *out = 0;
            }
            return false;
        }
        if ch == b'?' as TChar {
            in_options = true;
        } else if !in_options && (ch == b'/' as TChar || ch == b'\\' as TChar) {
            ch = b'\\' as TChar;
        }
        unsafe {
            *out.add(i) = ch;
        }
        i += 1;
    }
    unsafe {
        *out.add(i) = 0;
    }
    i > 0 && unsafe { *out } != b'?' as TChar
}
