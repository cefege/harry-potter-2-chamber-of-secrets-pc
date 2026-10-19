//! Rust port of `HarryPotter2/Unreal/Engine/Src/UnURL.cpp` (`FURL`, pinned at `42a1e81`).
//!
//! FURL members, the `FString` storage, `GConfig` and the static defaults stay in C++.
//! Every access goes through [`UrlServices`], filled in by `Hp2UrlServices.cpp`.
//! Constructor parsing runs on an owned [`Draft`] and is committed with one store.
//! `guard`/`unguard`, the trivial `FURL(const TCHAR*)` constructor and `operator<<`
//! stay in the C++ shells.
//!
//! Divergences, all intentional:
//! - `SaveURLConfig` copies each option into `TCHAR Temp[1024]` with unbounded
//!   `appStrcpy`. Past 1023 characters that is undefined behaviour. The port truncates
//!   to 1023 characters.
//! - `LoadURLConfig` leaves `TCHAR Text[32767]` uninitialised before `GetSection`. The
//!   port zero-fills it and stops the scan at the buffer end.
//! - `appFilePathToFURLToken` reads `Path` into an owned buffer before scanning it.
//! - The `Base` dereferences in the Relative and Partial constructors have no null
//!   guard. The original `check(Base)` is compiled out in release builds, so a null
//!   `Base` is undefined behaviour there too.
//! - FMA: `UnURL.o` (-O3 -ffp-contract=on) has no floating-point ops, so there are no
//!   fused sites.

use std::ffi::c_void;

use crate::params::TChar;

/// C++ services (`Hp2UrlServices` in `Hp2UrlServices.h`), same field order.
/// Each entry is a static, config, string or FURL member access the port cannot own.
#[repr(C)]
pub struct UrlServices {
    /// `FURL::Default*` by static id; returns the NUL-terminated chars.
    pub static_chars: unsafe extern "C" fn(id: i32) -> *const TChar,
    /// Assign a `FURL::Default*` static from NUL-terminated chars.
    pub static_set_chars: unsafe extern "C" fn(id: i32, chars: *const TChar),
    /// `FURL::DefaultPort`.
    pub default_port: unsafe extern "C" fn() -> i32,
    /// Assign `FURL::DefaultPort`.
    pub default_port_set: unsafe extern "C" fn(port: i32),
    /// `GConfig->GetStr(Section, Key)`.
    pub config_get_str:
        unsafe extern "C" fn(section: *const TChar, key: *const TChar) -> *const TChar,
    /// `GConfig->GetSection(Section, Value, Size, Filename)`.
    pub config_get_section: unsafe extern "C" fn(
        section: *const TChar,
        value: *mut TChar,
        size: i32,
        filename: *const TChar,
    ),
    /// `GConfig->SetString(Section, Key, Value, Filename)`.
    pub config_set_string: unsafe extern "C" fn(
        section: *const TChar,
        key: *const TChar,
        value: *const TChar,
        filename: *const TChar,
    ),
    /// `appStricmp`.
    pub str_icmp: unsafe extern "C" fn(a: *const TChar, b: *const TChar) -> i32,
    /// `appStrnicmp`.
    pub str_nicmp: unsafe extern "C" fn(a: *const TChar, b: *const TChar, count: i32) -> i32,
    /// `appAtoi`.
    pub str_atoi: unsafe extern "C" fn(s: *const TChar) -> i32,
    /// `FURL` text member by field id (0 Protocol, 1 Host, 2 Map, 3 Portal).
    pub url_field: unsafe extern "C" fn(url: *const c_void, field: i32) -> *const TChar,
    /// `FURL::Port`.
    pub url_port: unsafe extern "C" fn(url: *const c_void) -> i32,
    /// `FURL::Valid`.
    pub url_valid: unsafe extern "C" fn(url: *const c_void) -> i32,
    /// `FURL::Op.Num()`.
    pub url_op_count: unsafe extern "C" fn(url: *const c_void) -> i32,
    /// `*FURL::Op(index)`.
    pub url_op: unsafe extern "C" fn(url: *const c_void, index: i32) -> *const TChar,
    /// Assign a `FURL` text member by field id.
    pub url_set_field: unsafe extern "C" fn(url: *mut c_void, field: i32, chars: *const TChar),
    /// Assign `FURL::Port`.
    pub url_set_port: unsafe extern "C" fn(url: *mut c_void, port: i32),
    /// Assign `FURL::Valid`.
    pub url_set_valid: unsafe extern "C" fn(url: *mut c_void, valid: i32),
    /// `FURL::Op.Empty()`.
    pub url_op_clear: unsafe extern "C" fn(url: *mut c_void),
    /// `new(Op)FString(chars)`.
    pub url_op_push: unsafe extern "C" fn(url: *mut c_void, chars: *const TChar),
    /// `Op(index) = chars`.
    pub url_op_set: unsafe extern "C" fn(url: *mut c_void, index: i32, chars: *const TChar),
    /// `*(FString*)str = chars`.
    pub fstr_set: unsafe extern "C" fn(s: *mut c_void, chars: *const TChar),
    /// `debugf(TEXT("URL: Adding default option %s"), chars)`.
    pub log_default_option: unsafe extern "C" fn(chars: *const TChar),
}

// Static ids (`Hp2UrlServices.h`): the nine `FURL::Default*` FStrings, in `StaticInit` order.
const STATIC_PROTOCOL: i32 = 0;
const STATIC_MAP: i32 = 3;
const STATIC_HOST: i32 = 5;
const STATIC_PORTAL: i32 = 6;
const STATIC_MAP_EXT: i32 = 7;
const STATIC_SAVE_EXT: i32 = 8;

/// `[URL]` keys read by `StaticInit`, index = static id.
const STATIC_KEYS: [&str; 9] = [
    "Protocol",
    "ProtocolDescription",
    "Name",
    "Map",
    "LocalMap",
    "Host",
    "Portal",
    "MapExt",
    "SaveExt",
];

// Field ids (`Hp2UrlServices.h`).
const FIELD_PROTOCOL: i32 = 0;
const FIELD_HOST: i32 = 1;
const FIELD_MAP: i32 = 2;
const FIELD_PORTAL: i32 = 3;

// `ETravelType` (UnURL.h).
const TRAVEL_ABSOLUTE: i32 = 0;
const TRAVEL_PARTIAL: i32 = 1;
const TRAVEL_RELATIVE: i32 = 2;

/// `Temp[1024]` in the constructor and `SaveURLConfig`.
const TEMP_LEN: usize = 1024;
/// `Text[32767]` in `LoadURLConfig`.
const CONFIG_TEXT_LEN: usize = 32767;

/// Base options that an Absolute internal URL inherits (UnURL.cpp:296-302).
const INHERITED_PREFIXES: [&str; 7] = [
    "Name=",
    "Team=",
    "Class=",
    "Skin=",
    "Face=",
    "Voice=",
    "OverrideClass=",
];

/// Option names a Relative or Partial URL does not inherit (UnURL.cpp:178-181).
const SKIPPED_BASE_OPTIONS: [&str; 4] = ["PUSH", "POP", "PEER", "LOAD"];

const SPACE: TChar = b' ' as TChar;
const QMARK: TChar = b'?' as TChar;
const HASH: TChar = b'#' as TChar;
const COLON: TChar = b':' as TChar;
const DOT: TChar = b'.' as TChar;
const SLASH: TChar = b'/' as TChar;
const BACKSLASH: TChar = b'\\' as TChar;
const EQ: TChar = b'=' as TChar;

/// NUL-terminated `TChar` string.
type Wide = Vec<TChar>;

fn wide(s: &str) -> Wide {
    let mut v: Wide = s.bytes().map(TChar::from).collect();
    v.push(0);
    v
}

/// Length up to the first NUL, or the whole slice when there is none.
fn cstr_len(s: &[TChar]) -> usize {
    s.iter().position(|&c| c == 0).unwrap_or(s.len())
}

/// NUL-terminated copy of `s` up to its first NUL.
fn owned(s: &[TChar]) -> Wide {
    let n = cstr_len(s);
    let mut v = s[..n].to_vec();
    v.push(0);
    v
}

fn push_text(out: &mut Vec<TChar>, s: &[TChar]) {
    out.extend_from_slice(&s[..cstr_len(s)]);
}

fn push_lit(out: &mut Vec<TChar>, s: &str) {
    out.extend(s.bytes().map(TChar::from));
}

/// Reads a NUL-terminated string from service memory into an owned `Wide`.
///
/// SAFETY: `p` must point at a NUL-terminated buffer.
unsafe fn read_str(p: *const TChar) -> Wide {
    let mut v = Vec::new();
    let mut i = 0;
    loop {
        let c = unsafe { *p.add(i) };
        v.push(c);
        if c == 0 {
            return v;
        }
        i += 1;
    }
}

/// Length of a NUL-terminated buffer read through a raw pointer.
///
/// SAFETY: `p` must point at a NUL-terminated buffer.
unsafe fn raw_len(p: *const TChar) -> usize {
    let mut n = 0;
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    n
}

/// `appStricmp`. Both operands are `Wide` (NUL-terminated).
fn icmp(svc: &UrlServices, a: &[TChar], b: &[TChar]) -> i32 {
    // SAFETY: `Wide` operands always end in NUL, and the service only reads them.
    unsafe { (svc.str_icmp)(a.as_ptr(), b.as_ptr()) }
}

/// `appStrnicmp`. Both operands are `Wide` (NUL-terminated).
fn nicmp(svc: &UrlServices, a: &[TChar], b: &[TChar], count: usize) -> i32 {
    // SAFETY: `Wide` operands always end in NUL, and the service only reads them.
    unsafe { (svc.str_nicmp)(a.as_ptr(), b.as_ptr(), count as i32) }
}

/// `appStrchr(s, c)` over `s[from..]`, stopping at the NUL. Returns an index into `s`.
fn strchr(s: &[TChar], from: usize, c: TChar) -> Option<usize> {
    let mut i = from;
    while i < s.len() && s[i] != 0 {
        if s[i] == c {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// File-scope `appStrchr(Src, A, B)` (UnURL.cpp:136): the earlier of the first `A` or `B`.
fn strchr2(s: &[TChar], from: usize, a: TChar, b: TChar) -> Option<usize> {
    let mut i = from;
    while i < s.len() && s[i] != 0 {
        if s[i] == a || s[i] == b {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `appIsAlnum` (UnFile.h): ASCII letters and digits only.
fn is_alnum(c: TChar) -> bool {
    (c >= b'a' as TChar && c <= b'z' as TChar)
        || (c >= b'A' as TChar && c <= b'Z' as TChar)
        || (c >= b'0' as TChar && c <= b'9' as TChar)
}

/// `ValidNetChar` (UnURL.cpp): no space before the NUL.
fn valid_net_char(s: &[TChar]) -> bool {
    !s[..cstr_len(s)].contains(&SPACE)
}

/// `TCHAR* FURL` static default by id.
///
/// SAFETY: the service returns a NUL-terminated buffer.
unsafe fn static_str(svc: &UrlServices, id: i32) -> Wide {
    unsafe { read_str((svc.static_chars)(id)) }
}

/// `Protocol == DefaultProtocol` (`FString::operator==` is `appStricmp`).
fn is_default_protocol(svc: &UrlServices, protocol: &[TChar]) -> bool {
    // SAFETY: the static is read through the service; `protocol` is a `Wide`.
    let default = unsafe { static_str(svc, STATIC_PROTOCOL) };
    icmp(svc, protocol, &default) == 0
}

/// `*Options` value check: `*GetOption(name, "") == 0`. True when no option matches `name`,
/// or when the matching value is empty.
fn option_value_empty(svc: &UrlServices, ops: &[Wide], name: &[TChar]) -> bool {
    let n = cstr_len(name);
    match ops.iter().find(|op| nicmp(svc, op, name, n) == 0) {
        // The match means `op[..n]` has no NUL, so `op[n]` is in range.
        Some(op) => op[n] == 0,
        None => true,
    }
}

/// `appStrncpy(Temp, TextURL, ARRAY_COUNT(Temp))`: copies at most 1023 characters and leaves
/// the buffer zero-terminated.
///
/// SAFETY: `text` must be NUL-terminated or have at least 1023 readable characters.
unsafe fn copy_text(temp: &mut [TChar], text: *const TChar) {
    temp.fill(0);
    let mut i = 0;
    while i < TEMP_LEN - 1 {
        let c = unsafe { *text.add(i) };
        if c == 0 {
            break;
        }
        temp[i] = c;
        i += 1;
    }
}

/// `appStrstr(s, needle)` on a NUL-terminated `s` slice.
fn contains_text(s: &[TChar], needle: &str) -> bool {
    let needle = wide(needle);
    let n = cstr_len(&needle);
    s.windows(n).any(|w| w == &needle[..n])
}

/// Owned FURL state. Mirrors the FURL members; committed back through [`Draft::store`].
#[derive(Clone)]
struct Draft {
    protocol: Wide,
    host: Wide,
    map: Wide,
    portal: Wide,
    port: i32,
    ops: Vec<Wide>,
    valid: bool,
}

/// `FURL::Op` as owned `Wide` strings.
///
/// SAFETY: `url` must be a valid FURL object.
unsafe fn load_ops(svc: &UrlServices, url: *const c_void) -> Vec<Wide> {
    unsafe {
        let count = (svc.url_op_count)(url);
        (0..count).map(|i| read_str((svc.url_op)(url, i))).collect()
    }
}

/// Replaces `FURL::Op` with `ops`.
///
/// SAFETY: `url` must be a valid, writable FURL object.
unsafe fn store_ops(svc: &UrlServices, url: *mut c_void, ops: &[Wide]) {
    unsafe {
        (svc.url_op_clear)(url);
        for op in ops {
            (svc.url_op_push)(url, op.as_ptr());
        }
    }
}

impl Draft {
    /// `FURL()`: member initialisers from the statics, no options, `Valid = 1`.
    ///
    /// SAFETY: the services must be initialised (`static_chars`, `default_port`).
    unsafe fn defaults(svc: &UrlServices) -> Self {
        unsafe {
            Self {
                protocol: static_str(svc, STATIC_PROTOCOL),
                host: static_str(svc, STATIC_HOST),
                map: static_str(svc, STATIC_MAP),
                portal: static_str(svc, STATIC_PORTAL),
                port: (svc.default_port)(),
                ops: Vec::new(),
                valid: true,
            }
        }
    }

    /// Reads every FURL member from `url`.
    ///
    /// SAFETY: `url` must be a valid FURL object.
    unsafe fn load(svc: &UrlServices, url: *const c_void) -> Self {
        unsafe {
            Self {
                protocol: read_str((svc.url_field)(url, FIELD_PROTOCOL)),
                host: read_str((svc.url_field)(url, FIELD_HOST)),
                map: read_str((svc.url_field)(url, FIELD_MAP)),
                portal: read_str((svc.url_field)(url, FIELD_PORTAL)),
                port: (svc.url_port)(url),
                ops: load_ops(svc, url),
                valid: (svc.url_valid)(url) != 0,
            }
        }
    }

    /// Writes every FURL member back to `url`.
    ///
    /// SAFETY: `url` must be a valid, writable FURL object.
    unsafe fn store(&self, svc: &UrlServices, url: *mut c_void) {
        unsafe {
            (svc.url_set_field)(url, FIELD_PROTOCOL, self.protocol.as_ptr());
            (svc.url_set_field)(url, FIELD_HOST, self.host.as_ptr());
            (svc.url_set_field)(url, FIELD_MAP, self.map.as_ptr());
            (svc.url_set_field)(url, FIELD_PORTAL, self.portal.as_ptr());
            (svc.url_set_port)(url, self.port);
            (svc.url_set_valid)(url, self.valid as i32);
            store_ops(svc, url, &self.ops);
        }
    }
}

/// `FURL::AddOption` on an owned option list: replaces the first option whose name (up to and
/// including `=`) matches case-insensitively, otherwise appends.
fn push_option(svc: &UrlServices, ops: &mut Vec<Wide>, s: &[TChar]) {
    let n = cstr_len(s);
    let match_len = match s[..n].iter().position(|&c| c == EQ) {
        Some(sep) => sep + 1,
        None => n + 1,
    };
    let entry = owned(s);
    match ops
        .iter()
        .position(|op| nicmp(svc, op, &entry, match_len) == 0)
    {
        Some(i) => ops[i] = entry,
        None => ops.push(entry),
    }
}

/// `FURL::FURL(FURL* Base, const TCHAR* TextURL, ETravelType Type)`, after `Temp` is filled.
/// Returns `false` where the original resets to `FURL()` with `Valid = 0`.
///
/// SAFETY: `base` must be a valid FURL for Relative and Partial construction, and `base` may
/// be null only for Absolute construction.
unsafe fn parse(
    svc: &UrlServices,
    d: &mut Draft,
    base: *const c_void,
    temp: &mut [TChar],
    ty: i32,
) -> bool {
    let mut far_host = false;
    let mut far_map = false;

    // Copy Base.
    if ty == TRAVEL_RELATIVE {
        unsafe {
            d.protocol = read_str((svc.url_field)(base, FIELD_PROTOCOL));
            d.host = read_str((svc.url_field)(base, FIELD_HOST));
            d.map = read_str((svc.url_field)(base, FIELD_MAP));
            d.portal = read_str((svc.url_field)(base, FIELD_PORTAL));
            d.port = (svc.url_port)(base);
        }
    }
    if ty == TRAVEL_RELATIVE || ty == TRAVEL_PARTIAL {
        let base_ops = unsafe { load_ops(svc, base) };
        for op in base_ops {
            let skipped = SKIPPED_BASE_OPTIONS
                .iter()
                .any(|word| icmp(svc, &op, &wide(word)) == 0);
            if !skipped {
                d.ops.push(op);
            }
        }
    }

    // Skip leading blanks.
    let mut u = 0;
    while temp[u] == SPACE {
        u += 1;
    }

    // Options: '?' adds an option, '#' sets the portal.
    if let Some(first) = strchr2(temp, u, QMARK, HASH) {
        let mut option_char = temp[first];
        let mut next_option_char: TChar = 0;
        temp[first] = 0;
        let mut cur = Some(first + 1);
        while let Some(s) = cur {
            let t = strchr2(temp, s, QMARK, HASH);
            if let Some(t) = t {
                next_option_char = temp[t];
                temp[t] = 0;
            }
            let segment = owned(&temp[s..]);
            if !valid_net_char(&segment) {
                return false;
            }
            if option_char == QMARK {
                push_option(svc, &mut d.ops, &segment);
            } else {
                d.portal = segment;
            }
            cur = t.map(|t| t + 1);
            option_char = next_option_char;
        }
    }

    // `cur` is the remaining URL text, or None once the host consumed it.
    let mut cur: Option<usize>;
    if cstr_len(&temp[u..]) > 2 && temp[u + 1] == COLON {
        // Pure filename.
        unsafe {
            d.protocol = static_str(svc, STATIC_PROTOCOL);
            d.portal = static_str(svc, STATIC_PORTAL);
        }
        d.map = owned(&temp[u..]);
        d.host = wide("");
        cur = None;
        far_host = true;
        far_map = true;
    } else {
        // Parse protocol: a ':' past index 1 that comes before any '.'.
        let mut p = u;
        let colon = strchr(temp, u, COLON);
        let dot = strchr(temp, u, DOT);
        if let Some(c) = colon
            && c > u + 1
            && dot.is_none_or(|dt| c < dt)
        {
            d.protocol = owned(&temp[u..c]);
            p = c + 1;
        }

        // Optional leading slashes.
        if temp[p] == SLASH {
            p += 1;
            let next = temp[p];
            p += 1;
            if next != SLASH {
                return false;
            }
            far_host = true;
            d.host = wide("");
        }

        // Optional host name and port, when the dot is followed by neither map nor save ext.
        cur = Some(p);
        let dot = strchr(temp, p, DOT);
        let (map_ext, save_ext) = unsafe {
            (
                static_str(svc, STATIC_MAP_EXT),
                static_str(svc, STATIC_SAVE_EXT),
            )
        };
        if let Some(dt) = dot
            && dt > p
            && ext_mismatch(svc, temp, dt, &map_ext)
            && ext_mismatch(svc, temp, dt, &save_ext)
        {
            let host_start = p;
            match strchr(temp, host_start, SLASH) {
                Some(sl) => {
                    temp[sl] = 0;
                    cur = Some(sl + 1);
                }
                None => cur = None,
            }
            if let Some(t) = strchr(temp, host_start, COLON) {
                temp[t] = 0;
                let port_text = owned(&temp[t + 1..]);
                d.port = unsafe { (svc.str_atoi)(port_text.as_ptr()) };
            }
            d.host = owned(&temp[host_start..]);
            d.map = if is_default_protocol(svc, &d.protocol) {
                unsafe { static_str(svc, STATIC_MAP) }
            } else {
                wide("")
            };
            far_host = true;
        }
    }

    // Copy inherited options from Base (Absolute, internal URL only).
    if ty == TRAVEL_ABSOLUTE && !base.is_null() && is_default_protocol(svc, &d.protocol) {
        let base_ops = unsafe { load_ops(svc, base) };
        for op in base_ops {
            let inherited = INHERITED_PREFIXES
                .iter()
                .any(|prefix| nicmp(svc, &op, &wide(prefix), prefix.len()) == 0);
            if !inherited {
                continue;
            }
            let name = match op[..cstr_len(&op)].iter().position(|&c| c == EQ) {
                Some(eq) => owned(&op[..eq]),
                None => owned(&op),
            };
            if option_value_empty(svc, &d.ops, &name) {
                unsafe { (svc.log_default_option)(op.as_ptr()) };
                d.ops.push(op);
            }
        }
    }

    // Map and portal.
    if let Some(u) = cur
        && temp[u] != 0
    {
        if is_default_protocol(svc, &d.protocol) {
            far_map = true;
            if let Some(t) = strchr(temp, u, SLASH) {
                temp[t] = 0;
                let t1 = t + 1;
                if let Some(uu) = strchr(temp, t1, SLASH) {
                    temp[uu] = 0;
                    if temp[uu + 1] != 0 {
                        return false;
                    }
                }
                d.portal = owned(&temp[t1..]);
            }
        }
        d.map = owned(&temp[u..]);
    }

    // Validate. The Map check is commented out in the original.
    valid_net_char(&d.protocol)
        && valid_net_char(&d.host)
        && valid_net_char(&d.portal)
        && (far_host || far_map || !d.ops.is_empty())
}

/// `(appStrnicmp(Dot+1, Ext, Ext.Len()) != 0 || appIsAlnum(Dot[Ext.Len()+1]))`. Returns true
/// when the dot does not start this extension, or when a letter or digit follows it.
fn ext_mismatch(svc: &UrlServices, temp: &[TChar], dot: usize, ext: &[TChar]) -> bool {
    let n = cstr_len(ext);
    let after = temp.get(dot + n + 1).copied().unwrap_or(0);
    nicmp(svc, &owned(&temp[dot + 1..]), ext, n) != 0 || is_alnum(after)
}

/// `FURL::FURL(FURL* Base, const TCHAR* TextURL, ETravelType Type)`, storing the result into `this`.
///
/// # Safety
/// `this` must be a writable FURL. `text` must be NUL-terminated. `base` must be a valid
/// FURL for Relative and Partial construction.
pub unsafe fn construct(
    svc: &UrlServices,
    this: *mut c_void,
    base: *const c_void,
    text: *const TChar,
    ty: i32,
) {
    let mut temp = vec![0 as TChar; TEMP_LEN];
    unsafe { copy_text(&mut temp, text) };
    let mut d = unsafe { Draft::defaults(svc) };
    if !unsafe { parse(svc, &mut d, base, &mut temp, ty) } {
        d = unsafe { Draft::defaults(svc) };
        d.valid = false;
    }
    unsafe { d.store(svc, this) };
}

/// `FURL::String(FullyQualified)`: writes the text into the FString at `out` through `fstr_set`.
///
/// # Safety
/// `url` must be a valid FURL, and `out` must address a valid FString.
pub unsafe fn string(
    svc: &UrlServices,
    url: *const c_void,
    fully_qualified: bool,
    out: *mut c_void,
) {
    let d = unsafe { Draft::load(svc, url) };
    let (default_protocol, default_host, default_port) = unsafe {
        (
            static_str(svc, STATIC_PROTOCOL),
            static_str(svc, STATIC_HOST),
            (svc.default_port)(),
        )
    };

    let mut r: Vec<TChar> = Vec::new();
    if icmp(svc, &d.protocol, &default_protocol) != 0 || fully_qualified {
        push_text(&mut r, &d.protocol);
        push_lit(&mut r, ":");
        if icmp(svc, &d.host, &default_host) != 0 {
            push_lit(&mut r, "//");
        }
    }
    if icmp(svc, &d.host, &default_host) != 0 || d.port != default_port {
        push_text(&mut r, &d.host);
        if d.port != default_port {
            push_lit(&mut r, ":");
            push_lit(&mut r, &d.port.to_string());
        }
        push_lit(&mut r, "/");
    }
    push_text(&mut r, &d.map);
    for op in &d.ops {
        push_lit(&mut r, "?");
        push_text(&mut r, op);
    }
    if cstr_len(&d.portal) > 0 {
        push_lit(&mut r, "#");
        push_text(&mut r, &d.portal);
    }
    r.push(0);
    unsafe { (svc.fstr_set)(out, r.as_ptr()) };
}

/// `FURL::IsInternal`: `Protocol == DefaultProtocol`.
///
/// # Safety
/// `url` must be a valid FURL.
pub unsafe fn is_internal(svc: &UrlServices, url: *const c_void) -> bool {
    let protocol = unsafe { read_str((svc.url_field)(url, FIELD_PROTOCOL)) };
    is_default_protocol(svc, &protocol)
}

/// `FURL::IsLocalInternal`: `IsInternal() && Host.Len() == 0`.
///
/// # Safety
/// `url` must be a valid FURL.
pub unsafe fn is_local_internal(svc: &UrlServices, url: *const c_void) -> bool {
    if !unsafe { is_internal(svc, url) } {
        return false;
    }
    let host = unsafe { read_str((svc.url_field)(url, FIELD_HOST)) };
    cstr_len(&host) == 0
}

/// `FURL::AddOption(Str)`.
///
/// # Safety
/// `url` must be a valid, writable FURL. `s` must be NUL-terminated.
pub unsafe fn add_option(svc: &UrlServices, url: *mut c_void, s: *const TChar) {
    unsafe {
        let mut ops = load_ops(svc, url);
        let entry = read_str(s);
        push_option(svc, &mut ops, &entry);
        store_ops(svc, url, &ops);
    }
}

/// `FURL::LoadURLConfig(Section, Filename)`: adds each NUL-separated `[Section]` entry.
///
/// # Safety
/// `url` must be a valid, writable FURL. `section` and `filename` must be NUL-terminated
/// (`filename` may be null).
pub unsafe fn load_config(
    svc: &UrlServices,
    url: *mut c_void,
    section: *const TChar,
    filename: *const TChar,
) {
    unsafe {
        let mut ops = load_ops(svc, url);
        let mut text = vec![0 as TChar; CONFIG_TEXT_LEN];
        (svc.config_get_section)(section, text.as_mut_ptr(), CONFIG_TEXT_LEN as i32, filename);
        let mut i = 0;
        while i < text.len() && text[i] != 0 {
            let entry = owned(&text[i..]);
            push_option(svc, &mut ops, &entry);
            i += cstr_len(&text[i..]) + 1;
        }
        store_ops(svc, url, &ops);
    }
}

/// `FURL::SaveURLConfig(Section, Item, Filename) const`: writes each `Item=Value` option.
///
/// # Safety
/// `url` must be a valid FURL. `section`, `item` must be NUL-terminated. `filename` may
/// be null.
pub unsafe fn save_config(
    svc: &UrlServices,
    url: *const c_void,
    section: *const TChar,
    item: *const TChar,
    filename: *const TChar,
) {
    unsafe {
        let count = (svc.url_op_count)(url);
        for i in 0..count {
            let op = read_str((svc.url_op)(url, i));
            let n = cstr_len(&op).min(TEMP_LEN - 1);
            let mut temp = op[..n].to_vec();
            temp.push(0);
            if let Some(eq) = temp[..n].iter().position(|&c| c == EQ) {
                temp[eq] = 0;
                if (svc.str_icmp)(temp.as_ptr(), item) == 0 {
                    let value = owned(&temp[eq + 1..]);
                    (svc.config_set_string)(section, temp.as_ptr(), value.as_ptr(), filename);
                }
            }
        }
    }
}

/// `FURL::HasOption(Test) const`.
///
/// # Safety
/// `url` must be a valid FURL. `test` must be NUL-terminated.
pub unsafe fn has_option(svc: &UrlServices, url: *const c_void, test: *const TChar) -> bool {
    unsafe {
        let count = (svc.url_op_count)(url);
        (0..count).any(|i| (svc.str_icmp)((svc.url_op)(url, i), test) == 0)
    }
}

/// `FURL::GetOption(Match, Default) const`: returns a pointer into the matching option's text,
/// or `default`.
///
/// # Safety
/// `url` must be a valid FURL. `m` must be NUL-terminated. `default` must stay valid.
/// The returned pointer is only valid while the FURL's options are unchanged.
pub unsafe fn get_option(
    svc: &UrlServices,
    url: *const c_void,
    m: *const TChar,
    default: *const TChar,
) -> *const TChar {
    unsafe {
        let n = raw_len(m);
        let count = (svc.url_op_count)(url);
        for i in 0..count {
            let op = (svc.url_op)(url, i);
            if (svc.str_nicmp)(op, m, n as i32) == 0 {
                return op.add(n);
            }
        }
        default
    }
}

/// `FURL::operator==(const FURL& Other) const`.
///
/// # Safety
/// `a` and `b` must be valid FURL objects.
pub unsafe fn equal(svc: &UrlServices, a: *const c_void, b: *const c_void) -> bool {
    unsafe {
        for field in [FIELD_PROTOCOL, FIELD_HOST, FIELD_MAP] {
            if (svc.str_icmp)((svc.url_field)(a, field), (svc.url_field)(b, field)) != 0 {
                return false;
            }
        }
        if (svc.url_port)(a) != (svc.url_port)(b) {
            return false;
        }
        let count = (svc.url_op_count)(a);
        if count != (svc.url_op_count)(b) {
            return false;
        }
        (0..count).all(|i| (svc.str_icmp)((svc.url_op)(a, i), (svc.url_op)(b, i)) == 0)
    }
}

/// `FURL::StaticInit()`: reads the `[URL]` defaults and `Port`.
///
/// # Safety
/// the services must be initialised and GConfig must be loaded.
pub unsafe fn static_init(svc: &UrlServices) {
    unsafe {
        let section = wide("URL");
        for (id, key) in STATIC_KEYS.iter().enumerate() {
            let key = wide(key);
            (svc.static_set_chars)(
                id as i32,
                (svc.config_get_str)(section.as_ptr(), key.as_ptr()),
            );
        }
        let key = wide("Port");
        let port = (svc.config_get_str)(section.as_ptr(), key.as_ptr());
        (svc.default_port_set)((svc.str_atoi)(port));
    }
}

/// `FURL::StaticExit()`: clears the nine `FURL::Default*` strings. `DefaultPort` is kept.
///
/// # Safety
/// the services must be initialised.
pub unsafe fn static_exit(svc: &UrlServices) {
    unsafe {
        let empty = wide("");
        for id in 0..STATIC_KEYS.len() {
            (svc.static_set_chars)(id as i32, empty.as_ptr());
        }
    }
}

/// `appFilePathToFURLToken(Path, Out, OutCapacity)`: writes the token into `out[..capacity]`.
///
/// # Safety
/// when `capacity > 0`, `out` must address `capacity` writable `TChar`s. `path` must be
/// NUL-terminated or null.
pub unsafe fn file_path_to_token(path: *const TChar, out: *mut TChar, capacity: i32) -> bool {
    if out.is_null() || capacity <= 0 {
        return false;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(out, capacity as usize) };
    out[0] = 0;
    if path.is_null() {
        return false;
    }
    let path = unsafe { read_str(path) };
    let len = cstr_len(&path);
    if len == 0 || contains_text(&path[..len], "://") {
        return false;
    }

    // `Index >= OutCapacity - 1` rejects the token before a terminator would overflow.
    let limit = (capacity - 1) as usize;
    let mut in_options = false;
    let mut index = 0usize;
    while index < len {
        if index >= limit || path[index] == HASH {
            out[0] = 0;
            return false;
        }
        let mut ch = path[index];
        if ch == QMARK {
            in_options = true;
        } else if !in_options && (ch == SLASH || ch == BACKSLASH) {
            ch = BACKSLASH;
        }
        out[index] = ch;
        index += 1;
    }
    out[index] = 0;
    index > 0 && out[0] != QMARK
}
