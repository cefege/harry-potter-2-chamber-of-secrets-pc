//! Pre-appInit path selection and overlay identity policy. Unlike the import
//! manifest writer, this reader preserves the original bootstrap's permissive
//! JSON grammar (first duplicate member wins, saturated C integers, raw bytes).
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

type Bytes = Vec<u8>;
const MANIFEST: &[u8] = b"overlay-manifest.json";
const CHECKSUMS: &[u8] = b"overlay-checksums.txt";
pub const REASONS: [&[u8]; 8] = [
    b"",
    b"data.manifest_missing",
    b"data.checksums_missing",
    b"data.manifest_invalid",
    b"data.path_set_mismatch",
    b"data.size_mismatch",
    b"data.hash_mismatch",
    b"data.profile_unknown",
];
#[derive(Default)]
pub struct Outcome {
    pub success: bool,
    pub profile: i32,
    pub reason: usize,
    // canonical root, detail/error, retail, searched, prototype.
    pub text: [Bytes; 5],
}
fn cat(parts: &[&[u8]]) -> Bytes {
    parts.concat()
}
fn cbytes(s: &[u8]) -> &[u8] {
    s.split(|b| *b == 0).next().unwrap_or_default()
}
fn path(s: &[u8]) -> &Path {
    Path::new(OsStr::from_bytes(cbytes(s)))
}
fn bytes(p: &Path) -> Bytes {
    p.as_os_str().as_bytes().to_vec()
}
fn join(a: &[u8], b: &[u8]) -> Bytes {
    cat(&[
        a,
        if !a.is_empty() && a.last() != Some(&b'/') {
            b"/"
        } else {
            b""
        },
        b,
    ])
}
fn canonical(s: &[u8]) -> Option<Bytes> {
    let p = path(s);
    if p.as_os_str().is_empty() {
        return None;
    }
    // weakly_canonical resolves the existing prefix and lexically normalizes
    // the remainder; canonicalize alone rejects missing/../existing.
    let mut prefix = PathBuf::new();
    let mut suffix = Vec::new();
    let mut missing = false;
    for component in p.components() {
        if !missing {
            let candidate = prefix.join(component);
            match fs::metadata(&candidate) {
                Ok(_) => {
                    prefix = candidate;
                    continue;
                }
                Err(e) if matches!(e.raw_os_error(), Some(libc::ENOENT) | Some(libc::ENOTDIR)) => {
                    missing = true
                }
                Err(_) => return None,
            }
        }
        suffix.push(component);
    }
    let mut resolved = if prefix.as_os_str().is_empty() {
        PathBuf::new()
    } else {
        fs::canonicalize(prefix).ok()?
    };
    // The unresolved suffix retains lexical directory terminators; an
    // entirely existing path goes through canonicalize and drops them.
    let raw = p.as_os_str().as_bytes();
    let trailing_directory = missing
        && (matches!(suffix.last(), Some(Component::ParentDir))
            || raw.ends_with(b"/")
            || raw.ends_with(b"/."));
    for component in suffix {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match resolved.components().next_back() {
                Some(Component::Normal(_)) => {
                    resolved.pop();
                }
                Some(Component::RootDir) => {}
                _ => resolved.push(".."),
            },
            other => resolved.push(other),
        }
    }
    if resolved.as_os_str().is_empty() {
        resolved.push(".");
    } else if trailing_directory && resolved.file_name().is_some() {
        resolved.push("");
    }
    resolved.is_dir().then(|| bytes(&resolved))
}
fn readable(s: &[u8]) -> bool {
    fs::metadata(path(s)).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o444 != 0)
}
fn writable(s: &[u8]) -> bool {
    fs::metadata(path(s)).is_ok_and(|m| {
        let p = m.permissions().mode();
        m.is_dir() && (p & 0o300 == 0o300 || p & 0o030 == 0o030 || p & 0o003 == 0o003)
    })
}
fn ensure(s: &[u8], mode: u32) -> bool {
    let p = path(s);
    if fs::create_dir(p).is_ok() {
        return fs::metadata(p)
            .and_then(|m| {
                fs::set_permissions(p, fs::Permissions::from_mode(m.permissions().mode() & mode))
            })
            .is_ok();
    }
    p.is_dir()
}
fn tree(s: &[u8], mode: u32) -> bool {
    let p = path(s);
    if !p.is_absolute() {
        return false;
    }
    let mut current = PathBuf::new();
    for component in p.components() {
        current.push(component);
        if !ensure(&bytes(&current), mode) {
            return false;
        }
    }
    true
}
fn env(name: &str) -> Option<Bytes> {
    std::env::var_os(name).map(|s| s.as_bytes().to_vec())
}
fn home() -> Option<Bytes> {
    #[cfg(target_os = "macos")]
    {
        canonical(&env("HOME")?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(xdg) = env("XDG_DATA_HOME").filter(|s| s.first() == Some(&b'/')) {
            return Some(xdg);
        }
        Some(join(&canonical(&env("HOME")?)?, b".local/share"))
    }
}
fn app_suffix() -> &'static [u8] {
    if cfg!(target_os = "macos") {
        b"Library/Application Support/Harry Potter 2"
    } else {
        b"harry-potter-2"
    }
}
fn conventional(home: &[u8], leaf: &[u8]) -> Bytes {
    join(&join(home, app_suffix()), leaf)
}
fn launcher(home: &[u8]) -> Option<Bytes> {
    let app = join(home, app_suffix());
    let user = join(&app, b"User");
    if !tree(&app, 0o755) || !ensure(&user, 0o700) {
        return None;
    }
    canonical(&user).filter(|s| writable(s))
}
fn active(user: &[u8]) -> Option<Bytes> {
    if !tree(user, 0o700) {
        return None;
    }
    let root = canonical(user).filter(|s| writable(s))?;
    for child in [b"Save".as_slice(), b"Save/cache", b"Cache"] {
        let p = join(&root, child);
        if !ensure(&p, 0o700) || !writable(&p) {
            return None;
        }
    }
    Some(root)
}
fn descendant(p: &[u8], parent: &[u8]) -> bool {
    p == parent
        || (p.starts_with(parent)
            && p.len() > parent.len()
            && (parent.len() == 1 || matches!(p[parent.len()], b'/' | b'\\')))
}
pub fn argument(argument: &[u8]) -> Option<usize> {
    argument
        .get(..9)
        .filter(|p| p.eq_ignore_ascii_case(b"-datadir="))
        .map(|_| 9)
}
fn report(reason: usize, detail: &[u8]) {
    let safe: Bytes = detail
        .iter()
        .map(|&b| if b < 32 || b == 127 { b' ' } else { b })
        .collect();
    let _ = std::io::stderr().write_all(&cat(&[
        b"HP2_DATA_IDENTITY_REJECT ",
        REASONS[reason],
        b" ",
        &safe,
        b"\n",
    ]));
}
fn stderr(s: &[u8]) {
    let _ = std::io::stderr().write_all(s);
}

// Keep members ordered, including duplicates: the original FindMember returns
// the first member, not the last one a generic JSON object map would retain.
enum Json {
    Other,
    Number(Option<i64>),
    String(Bytes),
    Array(Vec<Json>),
    Object(Vec<(Bytes, Json)>),
}
impl Json {
    fn member(&self, key: &[u8]) -> Option<&Json> {
        if let Self::Object(m) = self {
            m.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
    fn string(&self) -> Option<&[u8]> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn integer(&self) -> Option<i64> {
        if let Self::Number(n) = self { *n } else { None }
    }
}
struct Parser {
    data: Bytes,
    pos: usize,
}
impl Parser {
    fn peek(&self) -> u8 {
        self.data.get(self.pos).copied().unwrap_or(0)
    }
    fn whitespace(&mut self) {
        while matches!(self.peek(), b' ' | b'\t' | b'\n' | b'\r') {
            self.pos += 1;
        }
    }
    fn hex4(&mut self) -> Option<u32> {
        let mut n = 0;
        for _ in 0..4 {
            let digit = (self.peek() as char).to_digit(16)?;
            n = (n << 4) | digit;
            self.pos += 1;
        }
        Some(n)
    }
    fn string(&mut self) -> Result<Bytes, &'static str> {
        if self.peek() != b'"' {
            return Err("expected a JSON string");
        }
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let b = self.peek();
            if b == 0 {
                return Err("unterminated JSON string");
            }
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = self.peek();
                    self.pos += 1;
                    match escape {
                        b'"' | b'\\' | b'/' => out.push(escape),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'u' => {
                            let mut cp = self.hex4().ok_or("malformed JSON unicode escape")?;
                            if (0xd800..=0xdbff).contains(&cp)
                                && self.data.get(self.pos..self.pos + 2) == Some(b"\\u")
                            {
                                self.pos += 2;
                                let low = self
                                    .hex4()
                                    .filter(|v| (0xdc00..=0xdfff).contains(v))
                                    .ok_or("unpaired JSON surrogate escape")?;
                                cp = 0x10000 + ((cp - 0xd800) << 10) + low - 0xdc00;
                            }
                            // Deliberately also encodes lone surrogate code units.
                            if cp < 0x80 {
                                out.push(cp as u8);
                            } else if cp < 0x800 {
                                out.extend_from_slice(&[
                                    (0xc0 | cp >> 6) as u8,
                                    (0x80 | cp & 63) as u8,
                                ]);
                            } else if cp < 0x10000 {
                                out.extend_from_slice(&[
                                    (0xe0 | cp >> 12) as u8,
                                    (0x80 | (cp >> 6) & 63) as u8,
                                    (0x80 | cp & 63) as u8,
                                ]);
                            } else {
                                out.extend_from_slice(&[
                                    (0xf0 | cp >> 18) as u8,
                                    (0x80 | (cp >> 12) & 63) as u8,
                                    (0x80 | (cp >> 6) & 63) as u8,
                                    (0x80 | cp & 63) as u8,
                                ]);
                            }
                        }
                        _ => return Err("invalid JSON escape"),
                    }
                }
                _ => out.push(b),
            }
        }
    }
    fn value(&mut self) -> Result<Json, &'static str> {
        self.whitespace();
        match self.peek() {
            b'{' => {
                self.pos += 1;
                self.whitespace();
                let mut members = Vec::new();
                if self.peek() == b'}' {
                    self.pos += 1;
                    return Ok(Json::Object(members));
                }
                loop {
                    self.whitespace();
                    let key = self.string()?;
                    self.whitespace();
                    if self.peek() != b':' {
                        return Err("expected ':' in JSON object");
                    }
                    self.pos += 1;
                    members.push((key, self.value()?));
                    self.whitespace();
                    match self.peek() {
                        b',' => self.pos += 1,
                        b'}' => {
                            self.pos += 1;
                            return Ok(Json::Object(members));
                        }
                        _ => return Err("expected ',' or '}' in JSON object"),
                    }
                }
            }
            b'[' => {
                self.pos += 1;
                self.whitespace();
                let mut items = Vec::new();
                if self.peek() == b']' {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.whitespace();
                    match self.peek() {
                        b',' => self.pos += 1,
                        b']' => {
                            self.pos += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err("expected ',' or ']' in JSON array"),
                    }
                }
            }
            b'"' => self.string().map(Json::String),
            b't' | b'f' | b'n' => {
                let literal: &[u8] = match self.peek() {
                    b't' => b"true",
                    b'f' => b"false",
                    _ => b"null",
                };
                if self.data.get(self.pos..self.pos + literal.len()) != Some(literal) {
                    return Err("malformed JSON literal");
                }
                self.pos += literal.len();
                Ok(Json::Other)
            }
            b'-' | b'0'..=b'9' => {
                // libc is the native numeric-conversion leaf, not a JSON parser.
                let start = unsafe { self.data.as_ptr().add(self.pos).cast::<libc::c_char>() };
                let mut end = std::ptr::null_mut();
                let integer = unsafe { libc::strtoll(start, &mut end, 10) };
                if end.cast_const() == start {
                    return Err("malformed JSON number");
                }
                let integral = !matches!(unsafe { *end } as u8, b'.' | b'e' | b'E');
                if !integral {
                    unsafe {
                        libc::strtod(start, &mut end);
                    }
                }
                self.pos += unsafe { end.offset_from(start) } as usize;
                Ok(Json::Number(integral.then_some(integer)))
            }
            _ => Err("expected a JSON value"),
        }
    }
    fn parse(contents: &[u8]) -> Result<Json, &'static str> {
        let mut data = cbytes(contents).to_vec();
        data.push(0);
        let mut p = Self { data, pos: 0 };
        let root = p.value()?;
        p.whitespace();
        if p.peek() != 0 {
            return Err("trailing characters after JSON document");
        }
        Ok(root)
    }
}
#[derive(Clone)]
struct Identity {
    size: u64,
    digest: Bytes,
}
type Index = BTreeMap<Bytes, Identity>;
type Reject = (usize, Bytes);
fn reject(reason: usize, detail: &[u8]) -> Reject {
    (reason, detail.to_vec())
}
fn safe(p: &[u8]) -> bool {
    !p.is_empty()
        && !p.contains(&b'\\')
        && p.split(|b| *b == b'/')
            .all(|c| !c.is_empty() && c != b"." && c != b"..")
}
fn hex(s: &[u8]) -> bool {
    s.len() == 64 && s.iter().all(u8::is_ascii_hexdigit)
}
fn manifest(contents: &[u8]) -> Result<(i32, Index), Reject> {
    let root = Parser::parse(contents).map_err(|e| {
        (
            3,
            cat(&[b"overlay-manifest.json is not valid JSON: ", e.as_bytes()]),
        )
    })?;
    if !root
        .member(b"schema_version")
        .and_then(Json::integer)
        .is_some_and(|n| (2..=3).contains(&n))
    {
        return Err(reject(
            3,
            b"overlay-manifest.json has a missing or unsupported schema_version",
        ));
    }
    let profile = root
        .member(b"profile")
        .and_then(Json::string)
        .ok_or_else(|| reject(7, b"overlay-manifest.json has no string profile"))?;
    let profile = match profile {
        b"safe" => 2,
        b"full" => 3,
        b"retail-only" => 4,
        _ => return Err((7, cat(&[b"unknown overlay profile: ", profile]))),
    };
    let entries = match root.member(b"entries") {
        Some(Json::Array(a)) if !a.is_empty() => a,
        _ => {
            return Err(reject(
                3,
                b"overlay-manifest.json has no non-empty entries array",
            ));
        }
    };
    let mut index = Index::new();
    for entry in entries {
        if !matches!(entry, Json::Object(_)) {
            return Err(reject(3, b"overlay manifest entry is not an object"));
        }
        let p = entry
            .member(b"path")
            .and_then(Json::string)
            .filter(|s| safe(s))
            .ok_or_else(|| reject(3, b"overlay manifest entry has a missing or unsafe path"))?;
        let size = entry
            .member(b"size")
            .and_then(Json::integer)
            .filter(|n| *n >= 0)
            .ok_or_else(|| {
                (
                    3,
                    cat(&[b"overlay manifest entry ", p, b" has no integer size"]),
                )
            })? as u64;
        let digest = entry
            .member(b"sha256")
            .and_then(Json::string)
            .filter(|s| hex(s))
            .ok_or_else(|| {
                (
                    3,
                    cat(&[b"overlay manifest entry ", p, b" has a malformed sha256"]),
                )
            })?;
        if index
            .insert(
                p.to_vec(),
                Identity {
                    size,
                    digest: digest.to_ascii_lowercase(),
                },
            )
            .is_some()
        {
            return Err((3, cat(&[b"duplicate overlay manifest entry for path: ", p])));
        }
    }
    Ok((profile, index))
}
fn checksums(contents: &[u8]) -> Result<Index, Reject> {
    if contents.is_empty() {
        return Err(reject(2, b"overlay-checksums.txt is empty"));
    }
    if contents.last() != Some(&b'\n') {
        return Err(reject(
            2,
            b"overlay-checksums.txt does not end with a newline",
        ));
    }
    let mut index = Index::new();
    for (n, line) in contents[..contents.len() - 1]
        .split(|b| *b == b'\n')
        .enumerate()
    {
        let prefix = format!("checksum line {}", n + 1);
        let fail = |s: &[u8]| (2, cat(&[prefix.as_bytes(), s]));
        if line.contains(&b'\r') {
            return Err(fail(b" contains a carriage return"));
        }
        let fields: Vec<_> = line.split(|b| *b == b'\t').collect();
        if fields.len() != 3 {
            return Err(fail(b" is not '<path>\\t<size>\\t<sha256>'"));
        }
        let (p, size, digest) = (fields[0], fields[1], fields[2]);
        if !safe(p) {
            return Err(fail(b" has an unsafe path"));
        }
        if size.is_empty() {
            return Err(fail(b" has an empty size"));
        }
        if !size.iter().all(u8::is_ascii_digit) {
            return Err(fail(b" has a non-numeric size"));
        }
        if !hex(digest) {
            return Err(fail(b" has a malformed sha256"));
        }
        let size = size.iter().fold(0u64, |n, b| {
            n.saturating_mul(10).saturating_add((b - b'0') as u64)
        });
        if index
            .insert(
                p.to_vec(),
                Identity {
                    size,
                    digest: digest.to_ascii_lowercase(),
                },
            )
            .is_some()
        {
            return Err((2, cat(&[b"duplicate checksum line for path: ", p])));
        }
    }
    Ok(index)
}
fn verify(root: &[u8]) -> Result<i32, Reject> {
    let mp = join(root, MANIFEST);
    let cp = join(root, CHECKSUMS);
    if !readable(&mp) {
        return Err(reject(
            1,
            b"overlay-checksums.txt present without overlay-manifest.json",
        ));
    }
    if !readable(&cp) {
        return Err(reject(
            2,
            b"overlay-manifest.json without sibling overlay-checksums.txt",
        ));
    }
    let m = fs::read(path(&mp));
    let c = fs::read(path(&cp));
    let m = m.map_err(|_| reject(3, b"cannot read overlay-manifest.json"))?;
    let c = c.map_err(|_| reject(2, b"cannot read overlay-checksums.txt"))?;
    let (profile, m) = manifest(&m)?;
    let c = checksums(&c)?;
    // Index disagreements take precedence over a path-set difference, even if
    // the differing key sorts earlier. Both native maps are byte-ordered.
    for (p, a) in &m {
        if let Some(b) = c.get(p) {
            if a.size != b.size {
                return Err((
                    5,
                    cat(&[
                        b"index disagreement for ",
                        p,
                        format!(": manifest size {} vs checksums size {}", a.size, b.size)
                            .as_bytes(),
                    ]),
                ));
            }
            if a.digest != b.digest {
                return Err((
                    6,
                    cat(&[
                        b"index disagreement for ",
                        p,
                        b": manifest sha256 differs from checksums sha256",
                    ]),
                ));
            }
        }
    }
    let mo: Vec<&[u8]> = m
        .keys()
        .filter(|p| !c.contains_key(*p))
        .take(3)
        .map(Vec::as_slice)
        .collect();
    let co: Vec<&[u8]> = c
        .keys()
        .filter(|p| !m.contains_key(*p))
        .take(3)
        .map(Vec::as_slice)
        .collect();
    if !mo.is_empty() || !co.is_empty() {
        return Err((
            4,
            cat(&[
                b"manifest paths: [",
                &mo.join(b", ".as_slice()),
                b"]; checksums paths: [",
                &co.join(b", ".as_slice()),
                b"]",
            ]),
        ));
    }
    for (p, entry) in &m {
        let filename = join(root, cbytes(p));
        let (size, digest) = hp2_file_hash::hash(path(&filename))
            .map_err(|_| (4, cat(&[b"overlay entry missing from tree: ", p])))?;
        if size != entry.size {
            return Err((
                5,
                cat(&[
                    b"size mismatch for ",
                    p,
                    format!(": expected {}, found {}", entry.size, size).as_bytes(),
                ]),
            ));
        }
        if digest.as_bytes() != entry.digest {
            return Err((
                6,
                cat(&[
                    b"sha256 mismatch for ",
                    p,
                    b": expected ",
                    &entry.digest,
                    b", found ",
                    digest.as_bytes(),
                ]),
            ));
        }
    }
    Ok(profile)
}
pub fn validate(candidate: &[u8]) -> Outcome {
    let mut out = Outcome::default();
    let Some(root) = canonical(candidate) else {
        out.text[1] = b"candidate does not name an existing directory".to_vec();
        return out;
    };
    if !readable(&join(&root, b"System/Default.ini")) {
        out.text[1] = b"expected a readable System/Default.ini".to_vec();
        return out;
    }
    if !readable(&join(&root, MANIFEST)) && !readable(&join(&root, CHECKSUMS)) {
        out.profile = 1;
        out.text[1] = b"legacy development root accepted by System/Default.ini".to_vec();
    } else {
        match verify(&root) {
            Ok(profile) => {
                out.profile = profile;
                out.text[1] = b"overlay identity verified".to_vec();
            }
            Err((reason, detail)) => {
                report(reason, &detail);
                out.reason = reason;
                out.text[1] = detail;
                return out;
            }
        }
    }
    out.success = true;
    out.text[0] = root;
    out
}
fn validation_error(out: &Outcome) -> Bytes {
    if out.reason != 0 {
        cat(&[
            b"data identity rejected: ",
            REASONS[out.reason],
            b": ",
            &out.text[1],
        ])
    } else {
        out.text[1].clone()
    }
}
fn discover(suffixes: &[&[u8]]) -> Option<Bytes> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        for suffix in suffixes {
            let out = validate(&join(&bytes(&current), suffix));
            if out.success {
                return Some(out.text[0].clone());
            }
        }
        if !current.pop() {
            return None;
        }
    }
}
fn external(home: &[u8], suffixes: &[&[u8]]) -> Option<Bytes> {
    for suffix in suffixes {
        let mut out = validate(&conventional(home, suffix));
        if out.success {
            return Some(std::mem::take(&mut out.text[0]));
        }
    }
    None
}
fn install<F: FnMut(&[u8], &[u8]) -> i32>(
    data: &[u8],
    user: &[u8],
    installer: &mut F,
) -> Result<bool, Bytes> {
    if descendant(data, user) || descendant(user, data) {
        return Err(b"data and user directories must be separate".to_vec());
    }
    let system = canonical(&join(data, b"System"))
        .ok_or_else(|| b"could not canonicalize the validated System directory".to_vec())?;
    let mut base = join(&system, b"");
    let mut user = join(user, b"");
    base.push(0);
    user.push(0);
    match installer(&base, &user) {
        1 => Ok(true),
        -1 => Ok(false),
        _ => Err(b"filesystem paths are not valid UTF-8".to_vec()),
    }
}
/// Operations correspond to the stable native public API; only the typed
/// TCHAR conversion and publication leaf remains native. -1 stops on its
/// captured exception, which the native caller rethrows after this returns.
/// Installer spans include the terminating NUL and remain live for the call.
pub fn run<F: FnMut(&[u8], &[u8]) -> i32>(
    operation: u32,
    first: &[u8],
    second: &[u8],
    args: &[&[u8]],
    mut installer: F,
) -> Outcome {
    if operation == 1 {
        return validate(first);
    }
    let mut out = Outcome::default();
    match operation {
        2 | 3 => {
            let validated = validate(first);
            if !validated.success {
                out.text[1] = validation_error(&validated);
                return out;
            }
            if operation == 2 {
                out.success = true;
                out.text[0] = validated.text[0].clone();
                return out;
            }
            if second.is_empty() || !path(second).is_absolute() {
                out.text[1] = b"user directory must be an absolute path".to_vec();
                return out;
            }
            let Some(user) = active(second) else {
                out.text[1] = b"could not create writable user directory".to_vec();
                return out;
            };
            match install(&validated.text[0], &user, &mut installer) {
                Ok(ok) => out.success = ok,
                Err(e) => out.text[1] = e,
            }
        }
        4 => {
            let Some(home) = home() else {
                out.text[1] = b"HOME does not name an existing directory".to_vec();
                return out;
            };
            if let Some(root) = launcher(&home) {
                out.success = true;
                out.text[0] = root;
            } else {
                out.text[1] = b"could not create writable launcher directory".to_vec();
            }
        }
        5 | 6 => {
            if let Some(home) = home() {
                out.success = true;
                out.text[0] = conventional(
                    &home,
                    if operation == 5 {
                        b"Data/Retail"
                    } else {
                        b"Data/Prototype"
                    },
                );
            }
        }
        7 | 8 => {
            let root = if operation == 8 {
                discover(&[b"HarryPotter2/Unreal"])
                    .or_else(|| external(&home()?, &[b"Data/Prototype"]))
            } else {
                home().and_then(|h| external(&h, &[b"Data/Retail", b"Data/Unreal"]))
            };
            if let Some(root) = root {
                out.success = true;
                out.text[0] = root;
            }
        }
        0 => {
            let explicit = args
                .iter()
                .skip(1)
                .find_map(|arg| argument(arg).map(|n| &arg[n..]));
            let Some(home) = home() else {
                stderr(b"hp2: local application data directory is unavailable\n");
                return out;
            };
            out.text[2] = conventional(&home, b"Data/Retail");
            out.text[3] = conventional(&home, b"Data/Unreal");
            out.text[4] = conventional(&home, b"Data/Prototype");
            let Some(user) = launcher(&home).and_then(|u| active(&u)) else {
                stderr(&cat(&[
                    b"hp2: could not create writable user directory '",
                    &conventional(&home, b"User"),
                    b"'\n",
                ]));
                return out;
            };
            let root = if let Some(candidate) = explicit {
                let validated = validate(candidate);
                if !validated.success {
                    stderr(&cat(&[
                        b"hp2: invalid -datadir '",
                        candidate,
                        if validated.reason != 0 {
                            b"': refused by data identity verification\n"
                        } else {
                            b"': expected a readable System/Default.ini\n"
                        },
                    ]));
                    return out;
                }
                Some(validated.text[0].clone())
            } else {
                discover(&[b"out/retail-data", b"HarryPotter2/Unreal"]).or_else(|| {
                    external(&home, &[b"Data/Retail", b"Data/Unreal", b"Data/Prototype"])
                })
            };
            let Some(root) = root else {
                stderr(&cat(&[b"hp2: no game data installed yet; the launcher will offer to import it.\nhp2: expected HarryPotter2/Unreal/System/Default.ini from the working directory or ", &out.text[3], b"/System/Default.ini\n"]));
                return out;
            };
            match install(&root, &user, &mut installer) {
                Ok(ok) => out.success = ok,
                Err(e) => stderr(&cat(&[b"hp2: ", &e, b"\n"])),
            }
        }
        _ => {}
    }
    out
}

/// Active files and the platform user directory selected during runtime bootstrap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfigurationPaths {
    pub user_root: PathBuf,
    pub system_ini: PathBuf,
    pub user_ini: PathBuf,
}

/// Resolves UCC's active INI files from the native appInit command-line rules,
/// installing shipped defaults when the active file is missing or stale.
///
/// The appInit TCHAR command line is UTF-8 encoded; its NUL terminator is optional.
pub fn runtime_configuration_paths(
    data_root: &Path,
    command_line: &[u8],
) -> Result<RuntimeConfigurationPaths, String> {
    let user = run(4, b"", b"", &[], |_, _| 0);
    if !user.success {
        return Err(String::from_utf8_lossy(&user.text[1]).into_owned());
    }
    let user_root = PathBuf::from(OsStr::from_bytes(&user.text[0]));
    let system_default = data_root.join("System").join("Default.ini");
    let user_default = data_root.join("System").join("DefUser.ini");
    let system = active_ini(command_line, b"INI=", user_root.join("HP.ini"))?;
    let user = active_ini(command_line, b"USERINI=", user_root.join("User.ini"))?;
    let system_changed = initialize_active_ini(&system_default, &system, false)?;
    initialize_active_ini(&user_default, &user, system_changed)?;
    Ok(RuntimeConfigurationPaths { user_root, system_ini: system, user_ini: user })
}

fn active_ini(command_line: &[u8], key: &[u8], default: PathBuf) -> Result<PathBuf, String> {
    let Some(value) = parse_command_value(command_line, key) else { return Ok(default) };
    if value.is_empty() {
        return Err(format!("empty {} command-line value", String::from_utf8_lossy(key)));
    }
    let path = PathBuf::from(OsStr::from_bytes(value));
    if path.is_absolute() { return Ok(path); }
    let base = std::env::current_dir().map_err(|error| format!("appBaseDir is unavailable: {error}"))?;
    Ok(base.join(path))
}
fn parse_command_value<'a>(command_line: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let end = command_line.iter().position(|byte| *byte == 0).unwrap_or(command_line.len());
    let source = &command_line[..end];
    let start = source.windows(key.len()).enumerate()
        .find(|(index, window)| {
            window.eq_ignore_ascii_case(key)
                && (*index == 0 || !source[*index - 1].is_ascii_alphanumeric())
        })
        .map(|(index, _)| index + key.len())?;
    let mut value = &source[start..];
    if value.first() == Some(&b'"') {
        value = &value[1..];
        let end = value.iter().position(|byte| *byte == b'"').unwrap_or(value.len());
        return Some(&value[..end]);
    }
    let end = value.iter().position(|byte| matches!(*byte, b' ' | b'\r' | b'\n' | b'\t' | b','))
        .unwrap_or(value.len());
    Some(&value[..end])
}

fn initialize_active_ini(default: &Path, active: &Path, force: bool) -> Result<bool, String> {
    let default_time = fs::metadata(default).and_then(|metadata| metadata.modified()).ok();
    let active_time = fs::metadata(active).and_then(|metadata| metadata.modified()).ok();
    let copy = force || active_time.is_none()
        || default_time.zip(active_time).is_some_and(|(default, active)| default > active);
    if !copy { return Ok(false); }
    let contents = fs::read(default).map_err(|error| {
        format!("required default configuration '{}' is unavailable: {error}", default.display())
    })?;
    fs::write(active, contents).map_err(|error| {
        format!("failed to initialize configuration '{}': {error}", active.display())
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Tree(PathBuf);
    impl Tree {
        fn new() -> Self {
            loop {
                let p = std::env::temp_dir().join(format!(
                    "hp2-bootstrap-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&p) {
                    Ok(()) => return Self(p),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => panic!("{e}"),
                }
            }
        }
        fn put(&self, p: &str, content: &[u8]) {
            let p = self.0.join(p);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
        }
    }
    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // The bootstrap publishes rejection detail on stderr and reads HOME/cwd;
    // serializing these tests keeps the stderr capture below deterministic
    // even when the harness runs tests concurrently.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|error| error.into_inner())
    }

    // Mirrors HP2DataProfileKind (HP2Paths.h:67).
    const DEVELOPMENT: i32 = 1;
    const SAFE: i32 = 2;
    const FULL: i32 = 3;
    const RETAIL_ONLY: i32 = 4;

    // StandardFiles() from DataIdentityTests.cpp:245.
    const DEFAULT_INI: &[u8] = b"[Engine.Engine]\r\nGameEngine=Engine.GameEngine\r\n";
    const ENTRY_MAP: &[u8] = b"fake map payload 1234567890\n";
    fn standard_files() -> [(&'static str, &'static [u8]); 2] {
        [("System/Default.ini", DEFAULT_INI), ("Maps/Entry.unr", ENTRY_MAP)]
    }

    // ManifestJson() from DataIdentityTests.cpp:186.
    fn manifest_json(files: &[(&str, &[u8])], profile: &str, schema_version: i32) -> Bytes {
        let mut json = b"{\n  \"entries\": [\n".to_vec();
        for (index, (path, content)) in files.iter().enumerate() {
            json.extend_from_slice(
                format!(
                    "    {{\"path\": \"{path}\", \"sha256\": \"{}\", \"size\": {}}}",
                    hp2_file_hash::hash_bytes(content),
                    content.len()
                )
                .as_bytes(),
            );
            if index + 1 < files.len() {
                json.push(b',');
            }
            json.push(b'\n');
        }
        json.extend_from_slice(
            format!("  ],\n  \"profile\": \"{profile}\",\n  \"schema_version\": {schema_version}\n}}\n")
                .as_bytes(),
        );
        json
    }

    // ChecksumsText() from DataIdentityTests.cpp:216: '<path>\t<size>\t<sha256>'
    // lines, LF-terminated, sorted by path bytes.
    fn checksums_text(files: &[(&str, &[u8])]) -> Bytes {
        let mut sorted = files.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        let mut text = Bytes::new();
        for (path, content) in sorted {
            text.extend_from_slice(
                format!(
                    "{path}\t{}\t{}\n",
                    content.len(),
                    hp2_file_hash::hash_bytes(content)
                )
                .as_bytes(),
            );
        }
        text
    }

    // ExpectReason() from DataIdentityTests.cpp:87.
    fn expect_reason(outcome: &Outcome, reason: usize, code: &str, message: &str) {
        assert!(!outcome.success, "{message}");
        assert_eq!(REASONS[outcome.reason], code.as_bytes(), "{message}");
        assert_eq!(outcome.reason, reason, "{message}");
    }

    // CaptureStderr() from DataIdentityTests.cpp:260.
    fn capture_stderr<F: FnOnce()>(body: F) -> Bytes {
        let file = std::env::temp_dir().join(format!(
            "hp2-bootstrap-stderr-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let handle = fs::File::create(&file).unwrap();
        let saved = unsafe { libc::dup(libc::STDERR_FILENO) };
        assert!(saved >= 0, "cannot duplicate stderr");
        assert!(
            unsafe { libc::dup2(handle.as_raw_fd(), libc::STDERR_FILENO) } >= 0,
            "cannot redirect stderr"
        );
        body();
        assert!(
            unsafe { libc::dup2(saved, libc::STDERR_FILENO) } >= 0,
            "cannot restore stderr"
        );
        unsafe { libc::close(saved) };
        drop(handle);
        let text = fs::read(&file).unwrap();
        let _ = fs::remove_file(&file);
        text
    }

    // HOME/cwd restoration for the PrepareHP2Paths contracts below.
    struct ProcessEnvironment {
        saved_home: Option<std::ffi::OsString>,
        saved_cwd: PathBuf,
    }
    impl ProcessEnvironment {
        fn new() -> Self {
            Self {
                saved_home: std::env::var_os("HOME"),
                saved_cwd: std::env::current_dir().unwrap(),
            }
        }
        fn set_home(home: &Path) {
            unsafe { std::env::set_var("HOME", home) };
        }
        fn set_cwd(cwd: &Path) {
            std::env::set_current_dir(cwd).unwrap();
        }
    }
    impl Drop for ProcessEnvironment {
        fn drop(&mut self) {
            match &self.saved_home {
                Some(home) => unsafe { std::env::set_var("HOME", home) },
                None => unsafe { std::env::remove_var("HOME") },
            }
            let _ = std::env::set_current_dir(&self.saved_cwd);
        }
    }

    #[test]
    fn weak_canonical_preserves_unresolved_directory_terminators() {
        let tree = Tree::new();
        tree.put("legacy/System/Default.ini", b"[Core]\n");
        // Both normalized directory targets are valid legacy data roots.
        tree.put("legacy/System/System/Default.ini", b"[Core]\n");
        let root = bytes(&fs::canonicalize(tree.0.join("legacy")).unwrap());
        let cases: &[(&[u8], &[u8], bool)] = &[
            (b"", b"", false),
            (b"/", b"", false),
            (b"/missing/..", b"", true),
            (b"/missing/../.", b"", true),
            (b"/missing/../System", b"/System", false),
            (b"/missing/../System/", b"/System", true),
            (b"/missing/../System/..", b"", true),
            (b"/System/..", b"", false),
        ];
        for &(suffix, child, trailing) in cases {
            let outcome = validate(&cat(&[&root, suffix]));
            assert!(outcome.success, "rejected suffix {suffix:?}");
            let mut expected = cat(&[&root, child]);
            if trailing {
                expected.push(b'/');
            }
            assert_eq!(outcome.text[0], expected, "suffix {suffix:?}");
        }
    }

    #[test]
    fn native_json_first_member_and_byte_escape_contract() {
        let root = Parser::parse(br#"{"n":02,"n":9,"raw":"\ud800","max":999999999999999999999}"#)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(root.member(b"n").and_then(Json::integer), Some(2));
        assert_eq!(root.member(b"max").and_then(Json::integer), Some(i64::MAX));
        assert_eq!(
            root.member(b"raw").and_then(Json::string),
            Some([0xed, 0xa0, 0x80].as_slice())
        );
        assert!(Parser::parse(br#"{"raw":"\ud800\u0041"}"#).is_err());
    }
    #[test]
    fn real_stream_identity_and_rejection_precedence() {
        let _serial = serial();
        let tree = Tree::new();
        let data = vec![b'x'; 262_147];
        tree.put("System/Default.ini", &data);
        let digest = hp2_file_hash::hash_bytes(&data);
        let manifest = format!(
            r#"{{"schema_version":2,"profile":"safe","entries":[{{"path":"System/Default.ini","size":{},"sha256":"{}"}}]}}"#,
            data.len(),
            digest
        );
        let checksum = format!("System/Default.ini\t{}\t{}\n", data.len(), digest);
        tree.put("overlay-manifest.json", manifest.as_bytes());
        tree.put("overlay-checksums.txt", checksum.as_bytes());
        let root = bytes(&tree.0);
        let accepted = validate(&root);
        assert!(accepted.success);
        assert_eq!(accepted.profile, 2);
        tree.put(
            "overlay-checksums.txt",
            format!(
                "a\t0\t{}\nSystem/Default.ini\t0\t{}\n",
                hp2_file_hash::hash_bytes(b""),
                digest
            )
            .as_bytes(),
        );
        let rejected = validate(&root);
        assert_eq!(rejected.reason, 5); // index size before earlier path-set difference
        tree.put("overlay-checksums.txt", checksum.as_bytes());
        let mut changed = data;
        changed[65_536] = b'z';
        tree.put("System/Default.ini", &changed);
        assert_eq!(validate(&root).reason, 6);
        changed.push(b'z');
        tree.put("System/Default.ini", &changed);
        assert_eq!(validate(&root).reason, 5);
    }

    #[test]
    fn legacy_root_without_overlay_files_selects_the_development_profile() {
        let _serial = serial();
        let tree = Tree::new();
        tree.put("System/Default.ini", b"[Engine.Engine]\n");
        let root = bytes(&tree.0);
        let outcome = validate(&root);
        assert!(outcome.success, "legacy root without overlay files is accepted");
        assert_eq!(
            outcome.profile, DEVELOPMENT,
            "legacy root reports the development profile"
        );
        assert_eq!(
            outcome.reason, 0,
            "accepted legacy root has no rejection reason code"
        );
        assert_eq!(REASONS[outcome.reason], b"");
        // ValidateHP2DataRoot (operation 2) keeps accepting legacy roots and
        // leaves the error string empty.
        let plain = run(2, &root, b"", &[], |_, _| 0);
        assert!(plain.success, "ValidateHP2DataRoot still accepts legacy roots");
        assert!(
            plain.text[1].is_empty(),
            "ValidateHP2DataRoot sets no error for accepted roots"
        );
    }

    #[test]
    fn directory_without_system_default_ini_is_refused_without_identity_reason() {
        let _serial = serial();
        let tree = Tree::new();
        let outcome = validate(&bytes(&tree.0));
        assert!(
            !outcome.success,
            "directory without System/Default.ini is refused"
        );
        assert_eq!(
            outcome.reason, 0,
            "non-data directory carries no identity reason code"
        );
    }

    #[test]
    fn stray_checksums_index_reports_data_manifest_missing() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        for (p, content) in files {
            tree.put(p, content);
        }
        tree.put("overlay-checksums.txt", &checksums_text(&files));
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            1,
            "data.manifest_missing",
            "stray checksums index reports data.manifest_missing",
        );
    }

    #[test]
    fn manifest_without_sibling_checksums_reports_data_checksums_missing() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        for (p, content) in files {
            tree.put(p, content);
        }
        tree.put("overlay-manifest.json", &manifest_json(&files, "safe", 3));
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            2,
            "data.checksums_missing",
            "manifest without checksums reports data.checksums_missing",
        );
    }

    #[test]
    fn malformed_and_unsupported_manifest_report_data_manifest_invalid() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        for (p, content) in files {
            tree.put(p, content);
        }
        tree.put("overlay-checksums.txt", &checksums_text(&files));
        tree.put("overlay-manifest.json", b"{\"entries\": [ not json");
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            3,
            "data.manifest_invalid",
            "malformed manifest reports data.manifest_invalid",
        );
        tree.put("overlay-manifest.json", &manifest_json(&files, "safe", 99));
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            3,
            "data.manifest_invalid",
            "unsupported schema version reports data.manifest_invalid",
        );
    }

    #[test]
    fn unknown_overlay_profile_reports_data_profile_unknown() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        for (p, content) in files {
            tree.put(p, content);
        }
        tree.put("overlay-checksums.txt", &checksums_text(&files));
        tree.put(
            "overlay-manifest.json",
            &manifest_json(&files, "prototype", 3),
        );
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            7,
            "data.profile_unknown",
            "unknown profile reports data.profile_unknown",
        );
    }

    #[test]
    fn path_set_disagreement_reports_data_path_set_mismatch() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        for (p, content) in files {
            tree.put(p, content);
        }
        // Checksums index loses Maps/Entry.unr.
        tree.put("overlay-checksums.txt", &checksums_text(&files[..1]));
        tree.put("overlay-manifest.json", &manifest_json(&files, "safe", 3));
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            4,
            "data.path_set_mismatch",
            "path set mismatch between manifest and checksums is refused",
        );
    }

    #[test]
    fn truncated_overlay_file_reports_data_size_mismatch() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        tree.put("System/Default.ini", DEFAULT_INI);
        tree.put("Maps/Entry.unr", &ENTRY_MAP[..ENTRY_MAP.len() - 4]);
        tree.put("overlay-checksums.txt", &checksums_text(&files));
        tree.put("overlay-manifest.json", &manifest_json(&files, "safe", 3));
        let outcome = validate(&bytes(&tree.0));
        expect_reason(
            &outcome,
            5,
            "data.size_mismatch",
            "size drift reports data.size_mismatch",
        );
    }

    #[test]
    fn same_length_corruption_reports_data_hash_mismatch_and_reject_line() {
        let _serial = serial();
        let files = standard_files();
        let mut corrupted = ENTRY_MAP.to_vec();
        corrupted[8] ^= 0x5a;
        let tree = Tree::new();
        tree.put("System/Default.ini", DEFAULT_INI);
        tree.put("Maps/Entry.unr", &corrupted);
        tree.put("overlay-checksums.txt", &checksums_text(&files));
        tree.put("overlay-manifest.json", &manifest_json(&files, "safe", 3));
        let root = bytes(&tree.0);
        let reject_line = capture_stderr(|| {
            validate(&root);
        });
        assert!(
            reject_line.starts_with(b"HP2_DATA_IDENTITY_REJECT data.hash_mismatch "),
            "stderr carries 'HP2_DATA_IDENTITY_REJECT data.hash_mismatch <detail>'"
        );
        // Re-run outside capture to check the structured status too.
        let status_tree = Tree::new();
        status_tree.put("System/Default.ini", DEFAULT_INI);
        status_tree.put("Maps/Entry.unr", &corrupted);
        status_tree.put("overlay-checksums.txt", &checksums_text(&files));
        status_tree.put("overlay-manifest.json", &manifest_json(&files, "safe", 3));
        let outcome = validate(&bytes(&status_tree.0));
        expect_reason(
            &outcome,
            6,
            "data.hash_mismatch",
            "same-length corruption reports data.hash_mismatch",
        );
    }

    #[test]
    fn consistent_overlays_report_safe_full_and_retail_only_profiles() {
        let _serial = serial();
        let files = standard_files();
        let cases: &[(&str, i32, i32, &str)] = &[
            ("safe", 3, SAFE, "consistent safe overlay is accepted"),
            ("full", 2, FULL, "full overlay maps to the full profile"),
            ("retail-only", 3, RETAIL_ONLY, "retail-only overlay maps to the retail-only profile"),
        ];
        for &(profile, schema_version, expected_profile, message) in cases {
            let tree = Tree::new();
            for (p, content) in files {
                tree.put(p, content);
            }
            tree.put("overlay-checksums.txt", &checksums_text(&files));
            tree.put(
                "overlay-manifest.json",
                &manifest_json(&files, profile, schema_version),
            );
            let outcome = validate(&bytes(&tree.0));
            assert!(outcome.success, "{message}");
            assert_eq!(outcome.profile, expected_profile, "{message}");
            if profile == "safe" {
                assert_eq!(outcome.reason, 0, "accepted overlay has no rejection reason code");
                assert_eq!(REASONS[outcome.reason], b"");
            }
        }
    }

    #[test]
    fn prepare_paths_accepts_an_explicit_legacy_data_root() {
        let _serial = serial();
        let tree = Tree::new();
        tree.put("legacy/System/Default.ini", b"[Engine.Engine]\n");
        tree.put("Home/.keep", b"");
        let environment = ProcessEnvironment::new();
        ProcessEnvironment::set_home(&tree.0.join("Home"));
        let mut argument = b"-datadir=".to_vec();
        argument.extend_from_slice(&bytes(&tree.0.join("legacy")));
        let args: [&[u8]; 2] = [b"hp2-data-identity-tests", &argument];
        let outcome = run(0, b"", b"", &args, |_, _| 1);
        drop(environment);
        assert!(
            outcome.success,
            "PrepareHP2Paths accepts an explicit legacy data root"
        );
    }

    #[test]
    fn prepare_paths_discovers_a_retail_overlay_from_the_working_directory() {
        let _serial = serial();
        let tree = Tree::new();
        let files = standard_files();
        for (p, content) in files {
            tree.put(&format!("out/retail-data/{p}"), content);
        }
        tree.put("out/retail-data/overlay-checksums.txt", &checksums_text(&files));
        tree.put(
            "out/retail-data/overlay-manifest.json",
            &manifest_json(&files, "safe", 3),
        );
        tree.put("Home/.keep", b"");
        let environment = ProcessEnvironment::new();
        ProcessEnvironment::set_home(&tree.0.join("Home"));
        ProcessEnvironment::set_cwd(&tree.0);
        let outcome = run(0, b"", b"", &[b"hp2-data-identity-tests"], |_, _| 1);
        drop(environment);
        assert!(
            outcome.success,
            "PrepareHP2Paths discovers out/retail-data from the cwd"
        );
    }
}
