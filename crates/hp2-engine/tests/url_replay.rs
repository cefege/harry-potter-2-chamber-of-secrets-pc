//! Replays `tests/fixtures/url_fixture.txt` against `hp2_engine::url`.
//!
//! The fixture is the ORIGINAL `UnURL.cpp` (ref 42a1e81) driven by `Build/url_fixture_harness`
//! against a mock GConfig/GLog. Each line is one record: `KIND=<k>`, then the inputs, then the
//! original outputs. Every output field must match the fixture byte for byte.
//!
//! `KIND=N`, `C` and `S` records start from the trivial `FURL(const TCHAR*)` constructor. That
//! constructor stays in C++ and has no Rust entry point, so the replay rebuilds its result from
//! the mock defaults.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;
use std::ptr;

use hp2_engine::params::TChar;
use hp2_engine::url::{self, UrlServices};

const FIXTURE: &str = include_str!("fixtures/url_fixture.txt");

// `FURL::Default*` static ids (see `Hp2UrlServices.h`).
const STATIC_PROTOCOL: usize = 0;
const STATIC_MAP: usize = 3;
const STATIC_HOST: usize = 5;
const STATIC_PORTAL: usize = 6;

// `FURL` text member ids.
const FIELD_PROTOCOL: i32 = 0;
const FIELD_HOST: i32 = 1;
const FIELD_MAP: i32 = 2;
const FIELD_PORTAL: i32 = 3;

const CFG_FIELDS: [&str; 10] = [
    "CFG_PROTOCOL",
    "CFG_DESC",
    "CFG_NAME",
    "CFG_MAP",
    "CFG_LOCALMAP",
    "CFG_HOST",
    "CFG_PORTAL",
    "CFG_MAPEXT",
    "CFG_SAVEEXT",
    "CFG_PORT",
];
const CFG_KEYS: [&str; 10] = [
    "Protocol",
    "ProtocolDescription",
    "Name",
    "Map",
    "LocalMap",
    "Host",
    "Portal",
    "MapExt",
    "SaveExt",
    "Port",
];

const DEFAULT_FIELDS: [(&str, i32); 9] = [
    ("DEF_PROTOCOL", 0),
    ("DEF_DESC", 1),
    ("DEF_NAME", 2),
    ("DEF_MAP", 3),
    ("DEF_LOCALMAP", 4),
    ("DEF_HOST", 5),
    ("DEF_PORTAL", 6),
    ("DEF_MAPEXT", 7),
    ("DEF_SAVEEXT", 8),
];

type Wide = Vec<TChar>;

static EMPTY: [TChar; 1] = [0];

/// Mock `FURL`: the four text members, the port, the valid flag and the option list.
#[derive(Clone)]
struct Furl {
    fields: [Wide; 4],
    port: i32,
    valid: i32,
    ops: Vec<Wide>,
}

/// Mock GConfig, the `FURL` defaults, and the GLog trace for the current record.
struct Mock {
    defaults: [Wide; 9],
    default_port: i32,
    /// `(Section, Key)` -> value.
    config: BTreeMap<(String, String), Wide>,
    /// `Section` -> `Key=Value` items.
    sections: BTreeMap<String, Vec<String>>,
    trace: Vec<String>,
}

impl Mock {
    fn new() -> Self {
        Self {
            defaults: std::array::from_fn(|_| wide("")),
            default_port: 0,
            config: BTreeMap::new(),
            sections: BTreeMap::new(),
            trace: Vec::new(),
        }
    }
}

thread_local! {
    static MOCK: RefCell<Mock> = RefCell::new(Mock::new());
}

fn with_mock<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

fn take_trace() -> Vec<String> {
    with_mock(|m| std::mem::take(&mut m.trace))
}

fn wide(s: &str) -> Wide {
    let mut v: Wide = s.chars().map(|c| c as TChar).collect();
    v.push(0);
    v
}

/// Reads a NUL-terminated string.
///
/// SAFETY: `p` must point to a NUL-terminated string.
unsafe fn narrow(p: *const TChar) -> String {
    let mut s = String::new();
    let mut i = 0;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            return s;
        }
        s.push(char::from_u32(c as u32).expect("valid char"));
        i += 1;
    }
}

/// Reads the units up to the first NUL, or the whole slice.
fn narrow_slice(units: &[TChar]) -> String {
    units
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| char::from_u32(c as u32).expect("valid char"))
        .collect()
}

/// Copies a NUL-terminated string, including the NUL.
///
/// SAFETY: `p` must point to a NUL-terminated string.
unsafe fn read_wide(p: *const TChar) -> Wide {
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

fn rp(f: &Furl) -> *const c_void {
    f as *const Furl as *const c_void
}

fn rm(f: &mut Furl) -> *mut c_void {
    f as *mut Furl as *mut c_void
}

/// `FURL(const TCHAR* LocalFilename)`: the trivial constructor, kept in C++.
fn trivial(local: Option<&str>) -> Furl {
    with_mock(|m| Furl {
        fields: [
            m.defaults[STATIC_PROTOCOL].clone(),
            m.defaults[STATIC_HOST].clone(),
            local.map_or_else(|| m.defaults[STATIC_MAP].clone(), wide),
            m.defaults[STATIC_PORTAL].clone(),
        ],
        port: m.default_port,
        valid: 1,
        ops: Vec::new(),
    })
}

fn blank() -> Furl {
    Furl {
        fields: std::array::from_fn(|_| wide("")),
        port: 0,
        valid: 0,
        ops: Vec::new(),
    }
}

/// `appStricmp`/`appStrnicmp` over at most `limit` units, ASCII case folding.
///
/// SAFETY: `a` and `b` must be NUL-terminated, or readable for `limit` units or until NUL.
unsafe fn compare_units(a: *const TChar, b: *const TChar, limit: usize) -> i32 {
    fn fold(c: TChar) -> TChar {
        if (b'A' as TChar..=b'Z' as TChar).contains(&c) {
            c + 32
        } else {
            c
        }
    }
    for i in 0..limit {
        let (x, y) = unsafe { (fold(*a.add(i)), fold(*b.add(i))) };
        if x != y {
            return if x < y { -1 } else { 1 };
        }
        if x == 0 {
            return 0;
        }
    }
    0
}

/// `appAtoi`: optional leading whitespace, sign, then decimal digits up to the first non-digit.
fn atoi_units(units: &[TChar]) -> i32 {
    let s: Vec<u8> = units
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    let mut i = 0;
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut value: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        value = (value * 10 + i64::from(s[i] - b'0')).min(i64::from(i32::MAX) + 1);
        i += 1;
    }
    let value = if neg { -value } else { value };
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

unsafe extern "C" fn m_static_chars(id: i32) -> *const TChar {
    with_mock(|m| m.defaults[id as usize].as_ptr())
}

unsafe extern "C" fn m_static_set_chars(id: i32, chars: *const TChar) {
    let v = unsafe { read_wide(chars) };
    with_mock(|m| m.defaults[id as usize] = v);
}

unsafe extern "C" fn m_default_port() -> i32 {
    with_mock(|m| m.default_port)
}

unsafe extern "C" fn m_default_port_set(port: i32) {
    with_mock(|m| m.default_port = port);
}

unsafe extern "C" fn m_config_get_str(section: *const TChar, key: *const TChar) -> *const TChar {
    let k = unsafe { (narrow(section), narrow(key)) };
    with_mock(|m| m.config.get(&k).map_or(EMPTY.as_ptr(), |v| v.as_ptr()))
}

unsafe extern "C" fn m_config_get_section(
    section: *const TChar,
    value: *mut TChar,
    size: i32,
    _filename: *const TChar,
) {
    unsafe {
        let sec = narrow(section);
        with_mock(|m| {
            m.trace.push(format!("GS:{sec}"));
            let items = m.sections.get(&sec).cloned().unwrap_or_default();
            let mut pos = 0usize;
            for item in &items {
                let units: Vec<TChar> = item.chars().map(|c| c as TChar).collect();
                assert!(
                    pos + units.len() + 2 <= size as usize,
                    "GetSection overflow"
                );
                for u in units {
                    value.add(pos).write(u);
                    pos += 1;
                }
                value.add(pos).write(0);
                pos += 1;
            }
            value.add(pos).write(0);
        });
    }
}

unsafe extern "C" fn m_config_set_string(
    section: *const TChar,
    key: *const TChar,
    value: *const TChar,
    _filename: *const TChar,
) {
    let entry = unsafe { format!("SS:{}:{}={}", narrow(section), narrow(key), narrow(value)) };
    with_mock(|m| m.trace.push(entry));
}

unsafe extern "C" fn m_str_icmp(a: *const TChar, b: *const TChar) -> i32 {
    unsafe { compare_units(a, b, usize::MAX) }
}

unsafe extern "C" fn m_str_nicmp(a: *const TChar, b: *const TChar, count: i32) -> i32 {
    unsafe { compare_units(a, b, count.max(0) as usize) }
}

unsafe extern "C" fn m_str_atoi(s: *const TChar) -> i32 {
    let units = unsafe { read_wide(s) };
    atoi_units(&units)
}

unsafe extern "C" fn m_url_field(url: *const c_void, field: i32) -> *const TChar {
    let f = unsafe { &*(url as *const Furl) };
    f.fields[field as usize].as_ptr()
}

unsafe extern "C" fn m_url_port(url: *const c_void) -> i32 {
    unsafe { (*(url as *const Furl)).port }
}

unsafe extern "C" fn m_url_valid(url: *const c_void) -> i32 {
    unsafe { (*(url as *const Furl)).valid }
}

unsafe extern "C" fn m_url_op_count(url: *const c_void) -> i32 {
    unsafe { (*(url as *const Furl)).ops.len() as i32 }
}

unsafe extern "C" fn m_url_op(url: *const c_void, index: i32) -> *const TChar {
    let f = unsafe { &*(url as *const Furl) };
    f.ops[index as usize].as_ptr()
}

unsafe extern "C" fn m_url_set_field(url: *mut c_void, field: i32, chars: *const TChar) {
    let v = unsafe { read_wide(chars) };
    let f = unsafe { &mut *(url as *mut Furl) };
    f.fields[field as usize] = v;
}

unsafe extern "C" fn m_url_set_port(url: *mut c_void, port: i32) {
    unsafe { (*(url as *mut Furl)).port = port };
}

unsafe extern "C" fn m_url_set_valid(url: *mut c_void, valid: i32) {
    unsafe { (*(url as *mut Furl)).valid = valid };
}

unsafe extern "C" fn m_url_op_clear(url: *mut c_void) {
    unsafe { (*(url as *mut Furl)).ops.clear() };
}

unsafe extern "C" fn m_url_op_push(url: *mut c_void, chars: *const TChar) {
    let v = unsafe { read_wide(chars) };
    unsafe { (*(url as *mut Furl)).ops.push(v) };
}

unsafe extern "C" fn m_url_op_set(url: *mut c_void, index: i32, chars: *const TChar) {
    let v = unsafe { read_wide(chars) };
    let f = unsafe { &mut *(url as *mut Furl) };
    f.ops[index as usize] = v;
}

unsafe extern "C" fn m_fstr_set(s: *mut c_void, chars: *const TChar) {
    let v = unsafe { read_wide(chars) };
    unsafe { *(s as *mut Wide) = v };
}

unsafe extern "C" fn m_log_default_option(chars: *const TChar) {
    let entry = format!("URL: Adding default option {}", unsafe { narrow(chars) });
    with_mock(|m| m.trace.push(entry));
}

fn services() -> UrlServices {
    UrlServices {
        static_chars: m_static_chars,
        static_set_chars: m_static_set_chars,
        default_port: m_default_port,
        default_port_set: m_default_port_set,
        config_get_str: m_config_get_str,
        config_get_section: m_config_get_section,
        config_set_string: m_config_set_string,
        str_icmp: m_str_icmp,
        str_nicmp: m_str_nicmp,
        str_atoi: m_str_atoi,
        url_field: m_url_field,
        url_port: m_url_port,
        url_valid: m_url_valid,
        url_op_count: m_url_op_count,
        url_op: m_url_op,
        url_set_field: m_url_set_field,
        url_set_port: m_url_set_port,
        url_set_valid: m_url_set_valid,
        url_op_clear: m_url_op_clear,
        url_op_push: m_url_op_push,
        url_op_set: m_url_op_set,
        fstr_set: m_fstr_set,
        log_default_option: m_log_default_option,
    }
}

// ---- Fixture record grammar ------------------------------------------------------------------

/// Splits a `[a][b]...` value into its groups. An empty value is an empty list.
fn groups(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(stripped) = rest.strip_prefix('[') {
        let end = stripped
            .find(']')
            .unwrap_or_else(|| panic!("unterminated group in {raw:?}"));
        out.push(stripped[..end].to_string());
        rest = &stripped[end + 1..];
    }
    assert!(rest.is_empty(), "trailing text {rest:?} in {raw:?}");
    out
}

struct Rec<'a> {
    line: usize,
    fields: HashMap<&'a str, &'a str>,
}

impl<'a> Rec<'a> {
    fn parse(line: usize, text: &'a str) -> Self {
        let fields = text
            .split('\t')
            .map(|f| {
                f.split_once('=')
                    .unwrap_or_else(|| panic!("line {line}: field without '=': {f:?}"))
            })
            .collect();
        Self { line, fields }
    }

    fn raw(&self, key: &str) -> &'a str {
        self.fields
            .get(key)
            .copied()
            .unwrap_or_else(|| panic!("line {}: missing {key}", self.line))
    }

    fn int(&self, key: &str) -> i32 {
        let raw = self.raw(key);
        raw.parse::<i64>()
            .unwrap_or_else(|_| panic!("line {}: {key}={raw:?} is not an integer", self.line))
            as i32
    }

    fn group(&self, key: &str) -> String {
        let mut g = groups(self.raw(key));
        assert_eq!(g.len(), 1, "line {}: {key} is not one group", self.line);
        g.pop().expect("one group")
    }

    fn list(&self, key: &str) -> Vec<String> {
        groups(self.raw(key))
    }
}

/// Encodes a string as one `[..]` group.
fn g(s: &str) -> String {
    format!("[{s}]")
}

/// Encodes a list as consecutive `[..]` groups.
fn gl(items: &[String]) -> String {
    items.iter().map(|s| g(s)).collect()
}

fn bool_i(b: bool) -> String {
    (b as i32).to_string()
}

fn check(rec: &Rec, expected: &[(&str, String)]) {
    for (key, want) in expected {
        let got = rec.raw(key);
        assert_eq!(got, want.as_str(), "line {}: {key}", rec.line);
    }
}

/// Fixture keys that the replay reads as inputs. Every other key in a record is an output.
fn is_input_key(key: &str) -> bool {
    key.starts_with("CFG_")
        || matches!(
            key,
            "KIND"
                | "BPRESENT"
                | "BVALID"
                | "BPROTOCOL"
                | "BHOST"
                | "BPORT"
                | "BMAP"
                | "BPORTAL"
                | "BOPS"
                | "TYPE"
                | "TEXT"
                | "T2"
                | "T3"
                | "HASNAME"
                | "NAME"
                | "SECTION"
                | "SECITEMS"
                | "ITEM"
                | "OPS0"
                | "CAP"
                | "MODE"
        )
}

/// Every output key in the record must be produced by its `replay_*` function, so no output
/// goes unchecked.
fn check_covered(rec: &Rec, expected: &[(&str, String)]) {
    for key in rec.fields.keys() {
        if is_input_key(key) {
            continue;
        }
        assert!(
            expected.iter().any(|(k, _)| *k == *key),
            "line {}: fixture output {key} is never compared",
            rec.line
        );
    }
}

// ---- Services helpers used by the replay -----------------------------------------------------

/// Installs the record's config and runs `StaticInit`, as the harness does.
fn install(svc: &UrlServices, rec: &Rec) {
    let values: Vec<Wide> = CFG_FIELDS.iter().map(|f| wide(&rec.group(f))).collect();
    with_mock(|m| {
        m.config.clear();
        for (key, value) in CFG_KEYS.iter().zip(values) {
            m.config.insert(("URL".to_string(), key.to_string()), value);
        }
    });
    unsafe { url::static_init(svc) };
}

/// Reads the option list through the services.
fn op_list(svc: &UrlServices, p: *const c_void) -> Vec<String> {
    unsafe {
        let count = (svc.url_op_count)(p);
        (0..count).map(|i| narrow((svc.url_op)(p, i))).collect()
    }
}

/// `FURL::String(FullyQualified)` read back through `fstr_set`.
fn out_string(svc: &UrlServices, p: *const c_void, fully_qualified: bool) -> String {
    let mut out: Wide = wide("");
    unsafe {
        url::string(
            svc,
            p,
            fully_qualified,
            &mut out as *mut Wide as *mut c_void,
        )
    };
    narrow_slice(&out)
}

/// The `emit_state` fields, read through the services.
fn state_fields(svc: &UrlServices, p: *const c_void) -> Vec<(&'static str, String)> {
    unsafe {
        let text = |id: i32| g(&narrow((svc.url_field)(p, id)));
        vec![
            ("VALID", (svc.url_valid)(p).to_string()),
            ("PROTOCOL", text(FIELD_PROTOCOL)),
            ("HOST", text(FIELD_HOST)),
            ("PORT", (svc.url_port)(p).to_string()),
            ("MAP", text(FIELD_MAP)),
            ("PORTAL", text(FIELD_PORTAL)),
            ("OPS", gl(&op_list(svc, p))),
            ("STR0", g(&out_string(svc, p, false))),
            ("STR1", g(&out_string(svc, p, true))),
            ("ISINT", bool_i(url::is_internal(svc, p))),
            ("ISLOCAL", bool_i(url::is_local_internal(svc, p))),
        ]
    }
}

// ---- Record replays --------------------------------------------------------------------------

/// `KIND=U`: the base-taking constructor, then `HasOption`, `GetOption`, `AddOption` and `==`.
fn replay_u(svc: &UrlServices, rec: &Rec) -> Vec<(&'static str, String)> {
    install(svc, rec);

    let mut base = trivial(None);
    base.valid = rec.int("BVALID");
    base.fields[FIELD_PROTOCOL as usize] = wide(&rec.group("BPROTOCOL"));
    base.fields[FIELD_HOST as usize] = wide(&rec.group("BHOST"));
    base.port = rec.int("BPORT");
    base.fields[FIELD_MAP as usize] = wide(&rec.group("BMAP"));
    base.fields[FIELD_PORTAL as usize] = wide(&rec.group("BPORTAL"));
    base.ops = rec.list("BOPS").iter().map(|s| wide(s)).collect();

    let base_ptr = if rec.int("BPRESENT") != 0 {
        rp(&base)
    } else {
        ptr::null()
    };
    let text = wide(&rec.group("TEXT"));
    let ty = rec.int("TYPE");
    let t2 = wide(&rec.group("T2"));
    let t3 = wide(&rec.group("T3"));
    let def = wide("DEF");

    with_mock(|m| m.trace.clear());
    let mut r = blank();
    unsafe { url::construct(svc, rm(&mut r), base_ptr, text.as_ptr(), ty) };
    let trace = take_trace();

    let mut out = vec![("TRACE", gl(&trace))];
    out.extend(state_fields(svc, rp(&r)));
    let has = unsafe { url::has_option(svc, rp(&r), t2.as_ptr()) };
    let get = unsafe { narrow(url::get_option(svc, rp(&r), t2.as_ptr(), def.as_ptr())) };

    let mut r2 = r.clone();
    unsafe { url::add_option(svc, rm(&mut r2), t3.as_ptr()) };
    out.push(("HAS", bool_i(has)));
    out.push(("GET", g(&get)));
    out.push(("OPS2", gl(&op_list(svc, rp(&r2)))));
    out.push(("EQSELF", bool_i(unsafe { url::equal(svc, rp(&r), rp(&r)) })));
    out.push(("EQ2", bool_i(unsafe { url::equal(svc, rp(&r), rp(&r2)) })));
    out
}

/// `KIND=N`: the trivial constructor from a filename, then the state fields.
fn replay_n(svc: &UrlServices, rec: &Rec) -> Vec<(&'static str, String)> {
    install(svc, rec);
    let local = if rec.int("HASNAME") != 0 {
        Some(rec.group("NAME"))
    } else {
        None
    };
    let r = trivial(local.as_deref());
    state_fields(svc, rp(&r))
}

/// `KIND=C`: `LoadURLConfig` over the mock section.
fn replay_c(svc: &UrlServices, rec: &Rec) -> Vec<(&'static str, String)> {
    install(svc, rec);
    let section = rec.group("SECTION");
    let items = rec.list("SECITEMS");
    let ops0: Vec<Wide> = rec.list("OPS0").iter().map(|s| wide(s)).collect();
    let sec_w = wide(&section);

    with_mock(|m| {
        m.sections.clear();
        m.sections.insert(section, items);
    });
    let mut r = trivial(None);
    r.ops = ops0;

    with_mock(|m| m.trace.clear());
    unsafe { url::load_config(svc, rm(&mut r), sec_w.as_ptr(), ptr::null()) };
    let trace = take_trace();

    let mut out = vec![("TRACE", gl(&trace))];
    out.extend(state_fields(svc, rp(&r)));
    out
}

/// `KIND=S`: `SaveURLConfig` over an option list.
fn replay_s(svc: &UrlServices, rec: &Rec) -> Vec<(&'static str, String)> {
    install(svc, rec);
    let sec_w = wide(&rec.group("SECTION"));
    let item_w = wide(&rec.group("ITEM"));
    let ops0: Vec<Wide> = rec.list("OPS0").iter().map(|s| wide(s)).collect();
    let mut r = trivial(None);
    r.ops = ops0;

    with_mock(|m| m.trace.clear());
    unsafe { url::save_config(svc, rp(&r), sec_w.as_ptr(), item_w.as_ptr(), ptr::null()) };
    let trace = take_trace();
    vec![("TRACE", gl(&trace)), ("OPS", gl(&op_list(svc, rp(&r))))]
}

/// `KIND=F`: `appFilePathToFURLToken` into a 300-unit buffer.
fn replay_f(rec: &Rec) -> Vec<(&'static str, String)> {
    let text = wide(&rec.group("TEXT"));
    let cap = rec.int("CAP");
    let mut out = vec![0 as TChar; 300];
    let ret = unsafe { url::file_path_to_token(text.as_ptr(), out.as_mut_ptr(), cap) };
    vec![("RET", bool_i(ret)), ("OUT", g(&narrow_slice(&out)))]
}

/// `KIND=X`: `StaticInit` (MODE 0) or `StaticInit` then `StaticExit` (MODE 1), then the defaults.
fn replay_x(svc: &UrlServices, rec: &Rec) -> Vec<(&'static str, String)> {
    install(svc, rec);
    if rec.int("MODE") == 1 {
        unsafe { url::static_exit(svc) };
    }
    let mut out: Vec<(&'static str, String)> = DEFAULT_FIELDS
        .iter()
        .map(|&(key, id)| {
            let chars = unsafe { (svc.static_chars)(id) };
            (key, g(&unsafe { narrow(chars) }))
        })
        .collect();
    out.push(("DEF_PORT", unsafe { (svc.default_port)() }.to_string()));
    out
}

#[test]
fn url_fixture_replays_against_original_records() {
    let svc = services();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();

    for (index, text) in FIXTURE.lines().enumerate() {
        let rec = Rec::parse(index + 1, text);
        let kind = rec.raw("KIND");
        let expected = match kind {
            "U" => replay_u(&svc, &rec),
            "N" => replay_n(&svc, &rec),
            "C" => replay_c(&svc, &rec),
            "S" => replay_s(&svc, &rec),
            "F" => replay_f(&rec),
            "X" => replay_x(&svc, &rec),
            other => panic!("line {}: unknown KIND {other}", rec.line),
        };
        check(&rec, &expected);
        check_covered(&rec, &expected);
        *counts.entry(kind).or_default() += 1;
    }

    let want = [
        ("U", 1200),
        ("N", 200),
        ("C", 300),
        ("S", 300),
        ("F", 400),
        ("X", 2),
    ];
    for (kind, n) in want {
        assert_eq!(
            counts.get(kind).copied().unwrap_or(0),
            n,
            "record count for KIND={kind}"
        );
    }
    assert_eq!(counts.values().sum::<usize>(), 2402);
}
