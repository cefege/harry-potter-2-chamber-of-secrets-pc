//! Replays the original `AStatLog.cpp` (@ 69c3263) fixture through the Rust port.
//! `tests/fixtures/stat_log_cases.txt` is produced by
//! `Build/stat_log_fixture_harness/gen_stat_log.sh` from the unmodified TU. This
//! test drives `hp2_engine::stat_log` with mock services that print the same
//! trace tokens as the harness, then compares every `ret=`, `out=` and `trace=`
//! field. The `in=` echo is not replayed; the same inputs are set in code.
//!
//! The MD5 mock implements the UE state machine exactly, including
//! `appMD5Final` zeroizing the context. Two `GetChecksum` calls on one context
//! therefore produce identical digests, as in the fixture.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

use hp2_engine::params::TChar;
use hp2_engine::stat_log::{
    StatLogError, StatLogResult, StatLogServices, batch_local, browse_relative_local_url,
    checksum_commandlet_main, close_log, execute_local_log_batcher, execute_silent_log_batcher,
    execute_world_log_batcher, file_flush, file_log, get_checksum, get_player_checksum,
    initial_check, log_mutator, open_log, watermark,
};

const EXPECTED: &str = include_str!("fixtures/stat_log_cases.txt");
const EXPECTED_RECORDS: usize = 31;

type P = *mut c_void;

/// Harness object. Every mock object is leaked for the life of the test process.
#[derive(Default)]
struct Obj {
    /// NUL-terminated name. Classes: `Class <pkg>.<name>`; packages: `Package <name>`.
    full: Vec<TChar>,
    outer: *mut Obj,
    class: *mut Obj,
    /// Actor's level, level's game, or pawn's PlayerReplicationInfo.
    level: *mut Obj,
    /// Game: external batcher. Level: not-dedicated. Stat: world batcher error.
    /// File: bWorld. Default object: world batcher error.
    flag: i32,
    /// Stat: local URL, local log dir, world URL, world params.
    /// File: log file name, final name.
    s: [FStr; 4],
}

#[derive(Default)]
struct FStr {
    units: Vec<TChar>,
}

struct Md5 {
    state: [u32; 4],
    count: [u32; 2],
    buffer: [u8; 64],
}

impl Default for Md5 {
    fn default() -> Self {
        Md5 {
            state: [0; 4],
            count: [0; 2],
            buffer: [0; 64],
        }
    }
}

struct Archive {
    id: usize,
}

#[derive(Default)]
struct Mock {
    trace: Vec<String>,
    handle_seq: usize,
    archive_seq: usize,
    cpnull: bool,
    gpr: bool,
    gpr_rc: i32,
    files: HashMap<String, i32>,
    exit: bool,
    iter_classes: Vec<*mut Obj>,
    default_obj: *mut Obj,
    packages: HashMap<String, *mut Obj>,
    classes: HashMap<String, *mut Obj>,
}

thread_local! {
    static MOCK: RefCell<Mock> = RefCell::new(Mock::default());
}

fn with_mock<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

fn tok(t: String) {
    with_mock(|m| m.trace.push(t));
}

fn wide(s: &str) -> Vec<TChar> {
    s.chars().map(|c| c as TChar).collect()
}

fn nul_units(s: &str) -> Vec<TChar> {
    let mut v = wide(s);
    v.push(0);
    v
}

fn text(units: &[TChar]) -> String {
    units
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| char::from_u32(c as u32).unwrap_or('?'))
        .collect()
}

/// # Safety
/// `p` must point to a NUL-terminated `TChar` string.
unsafe fn read_nul(p: *const TChar) -> Vec<TChar> {
    let mut v = Vec::new();
    let mut i = 0;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            return v;
        }
        v.push(c);
        i += 1;
    }
}

/// # Safety
/// `p` must point to a NUL-terminated `TChar` string.
unsafe fn text_at(p: *const TChar) -> String {
    let units = unsafe { read_nul(p) };
    text(&units)
}

fn new_obj(class: *mut Obj, full: &str) -> *mut Obj {
    let obj = Obj {
        full: if full.is_empty() {
            Vec::new()
        } else {
            nul_units(full)
        },
        class,
        ..Obj::default()
    };
    Box::into_raw(Box::new(obj))
}

fn package_obj(name: &str) -> *mut Obj {
    if let Some(p) = with_mock(|m| m.packages.get(name).copied()) {
        return p;
    }
    let p = new_obj(std::ptr::null_mut(), &format!("Package {name}"));
    with_mock(|m| m.packages.insert(name.to_string(), p));
    p
}

fn class_obj(full: &str) -> *mut Obj {
    if let Some(c) = with_mock(|m| m.classes.get(full).copied()) {
        return c;
    }
    let pkg = full.split('.').next().unwrap_or("");
    let c = new_obj(std::ptr::null_mut(), &format!("Class {full}"));
    unsafe { (*c).outer = package_obj(pkg) };
    with_mock(|m| m.classes.insert(full.to_string(), c));
    c
}

fn o(p: P) -> *mut Obj {
    p.cast()
}

fn fs(p: P) -> *mut FStr {
    p.cast()
}

fn s_of(obj: *mut Obj, i: usize) -> *mut FStr {
    unsafe { &raw mut (*obj).s[i] }
}

fn set_str(f: *mut FStr, s: &str) {
    unsafe { (*f).units = wide(s) };
}

// ---------------------------------------------------------------------------
// MD5, UE state machine (Core/Src/UnMisc.cpp).
// ---------------------------------------------------------------------------

const PADDING: [u8; 64] = {
    let mut p = [0u8; 64];
    p[0] = 0x80;
    p
};

fn md5_k(i: usize) -> u32 {
    (((i + 1) as f64).sin().abs() * 4_294_967_296.0) as u32
}

const MD5_S: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];

impl Md5 {
    fn new() -> Self {
        Md5 {
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            count: [0, 0],
            buffer: [0; 64],
        }
    }

    fn transform(state: &mut [u32; 4], block: &[u8]) {
        let mut x = [0u32; 16];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        let [mut a, mut b, mut c, mut d] = *state;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let s = MD5_S[(i / 16) * 4 + i % 4];
            let f = f.wrapping_add(a).wrapping_add(md5_k(i)).wrapping_add(x[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(s));
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }

    fn update(&mut self, input: &[u8]) {
        let len = input.len() as u32;
        let mut index = ((self.count[0] >> 3) & 0x3f) as usize;
        let (sum, carry) = self.count[0].overflowing_add(len << 3);
        self.count[0] = sum;
        if carry {
            self.count[1] = self.count[1].wrapping_add(1);
        }
        self.count[1] = self.count[1].wrapping_add(len >> 29);

        let part_len = 64 - index;
        let mut i = 0;
        if input.len() >= part_len {
            self.buffer[index..].copy_from_slice(&input[..part_len]);
            Self::transform(&mut self.state, &self.buffer);
            i = part_len;
            while i + 63 < input.len() {
                Self::transform(&mut self.state, &input[i..i + 64]);
                i += 64;
            }
            index = 0;
        }
        let rest = &input[i..];
        self.buffer[index..index + rest.len()].copy_from_slice(rest);
    }

    fn finish(&mut self) -> [u8; 16] {
        let mut bits = [0u8; 8];
        bits[..4].copy_from_slice(&self.count[0].to_le_bytes());
        bits[4..].copy_from_slice(&self.count[1].to_le_bytes());
        let index = ((self.count[0] >> 3) & 0x3f) as usize;
        let pad_len = if index < 56 { 56 - index } else { 120 - index };
        self.update(&PADDING[..pad_len]);
        self.update(&bits);
        let mut digest = [0u8; 16];
        for (i, w) in self.state.iter().enumerate() {
            digest[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
        *self = Md5::default();
        digest
    }
}

// ---------------------------------------------------------------------------
// Services mock (the trace is the same token format as the harness).
// ---------------------------------------------------------------------------

unsafe extern "C" fn svc_actor_level(actor: P) -> P {
    unsafe { (*o(actor)).level.cast() }
}

unsafe extern "C" fn svc_level_game(level: P) -> P {
    unsafe { (*o(level)).level.cast() }
}

unsafe extern "C" fn svc_level_not_dedicated(level: P) -> i32 {
    unsafe { (*o(level)).flag }
}

unsafe extern "C" fn svc_game_external(game: P) -> i32 {
    unsafe { (*o(game)).flag }
}

unsafe extern "C" fn svc_game_local_log_file_name(game: P) -> P {
    s_of(o(game), 0).cast()
}

unsafe extern "C" fn svc_fstr_chars(f: P) -> *const TChar {
    unsafe { (*fs(f)).units.as_ptr() }
}

unsafe extern "C" fn svc_fstr_len(f: P) -> i32 {
    unsafe { (*fs(f)).units.len() as i32 }
}

unsafe extern "C" fn svc_fstr_assign(f: P, s: *const TChar) {
    unsafe { (*fs(f)).units = read_nul(s) };
}

unsafe extern "C" fn svc_fstr_append(f: P, s: *const TChar) {
    unsafe {
        let add = read_nul(s);
        (*fs(f)).units.extend(add);
    };
}

unsafe extern "C" fn svc_stat_local_batcher_url(this: P) -> P {
    s_of(o(this), 0).cast()
}

unsafe extern "C" fn svc_stat_local_log_dir(this: P) -> P {
    s_of(o(this), 1).cast()
}

unsafe extern "C" fn svc_stat_world_batcher_url(this: P) -> P {
    s_of(o(this), 2).cast()
}

unsafe extern "C" fn svc_stat_world_batcher_params(this: P) -> P {
    s_of(o(this), 3).cast()
}

unsafe extern "C" fn svc_stat_class_default_object(_this: P) -> P {
    with_mock(|m| m.default_obj).cast()
}

unsafe extern "C" fn svc_stat_static_default_object() -> P {
    with_mock(|m| m.default_obj).cast()
}

unsafe extern "C" fn svc_stat_get_world_batcher_error(this: P) -> i32 {
    unsafe { (*o(this)).flag }
}

unsafe extern "C" fn svc_stat_set_world_batcher_error(this: P, v: i32) {
    unsafe { (*o(this)).flag = v };
}

unsafe extern "C" fn svc_stat_save_config(_this: P) {
    tok("SC".into());
}

unsafe extern "C" fn svc_stat_get_world(this: P) -> i32 {
    unsafe { (*o(this)).flag }
}

unsafe extern "C" fn svc_stat_file_name(this: P) -> P {
    s_of(o(this), 0).cast()
}

unsafe extern "C" fn svc_stat_file_final(this: P) -> P {
    s_of(o(this), 1).cast()
}

unsafe extern "C" fn svc_stat_player_secret(pawn: P) -> P {
    s_of(o(pawn), 0).cast()
}

unsafe extern "C" fn svc_stat_player_name(pawn: P) -> P {
    unsafe { s_of((*o(pawn)).level, 0).cast() }
}

unsafe extern "C" fn svc_stat_event_special_class(_this: P, id: *const TChar, target: P) {
    unsafe {
        let param = text(&(*(*o(target)).class).full);
        tok(format!("EV({},{})", text(&read_nul(id)), param));
    };
}

unsafe extern "C" fn svc_stat_event_special2(
    _this: P,
    id: *const TChar,
    p1: *const TChar,
    p2: *const TChar,
) {
    unsafe {
        tok(format!(
            "EV2({},{},{})",
            text(&read_nul(id)),
            text(&read_nul(p1)),
            text(&read_nul(p2))
        ));
    };
}

unsafe extern "C" fn svc_class_iterate_packages(
    callback: unsafe extern "C" fn(*mut c_void, *mut c_void),
    user: *mut c_void,
) {
    let classes = with_mock(|m| m.iter_classes.clone());
    for c in classes {
        unsafe { callback(user, (*c).outer.cast()) };
    }
}

unsafe extern "C" fn svc_package_full_name(pkg: P) -> *const TChar {
    unsafe { (*o(pkg)).full.as_ptr() }
}

static DLL_EXT: [TChar; 7] = [46, 100, 121, 108, 105, 98, 0];

unsafe extern "C" fn svc_dll_ext() -> *const TChar {
    DLL_EXT.as_ptr()
}

unsafe extern "C" fn svc_file_size(name: *const TChar) -> i32 {
    let n = unsafe { text_at(name) };
    let size = with_mock(|m| m.files.get(&n).copied().unwrap_or(-1));
    tok(format!("FSZ({n})={size}"));
    size
}

unsafe extern "C" fn svc_file_make_directory(name: *const TChar) {
    tok(format!("MKD({})", unsafe { text_at(name) }));
}

unsafe extern "C" fn svc_file_move(dst: *const TChar, src: *const TChar) {
    let dst = unsafe { text_at(dst) };
    let src = unsafe { text_at(src) };
    tok(format!("MV({dst},{src})"));
}

unsafe extern "C" fn svc_file_create_writer(name: *const TChar) -> P {
    let n = unsafe { text_at(name) };
    let id = with_mock(|m| {
        m.archive_seq += 1;
        m.archive_seq
    });
    tok(format!("CFW({n})=a{id}"));
    Box::into_raw(Box::new(Archive { id })).cast()
}

unsafe extern "C" fn svc_archive_serialize(ar: P, data: *const u8, len: i32) {
    let id = unsafe { (*ar.cast::<Archive>()).id };
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    tok(format!("SER(a{id},{hex})"));
}

unsafe extern "C" fn svc_archive_flush(ar: P) {
    let id = unsafe { (*ar.cast::<Archive>()).id };
    tok(format!("FLU(a{id})"));
}

unsafe extern "C" fn svc_archive_delete(ar: P) {
    if ar.is_null() {
        return;
    }
    let archive = unsafe { Box::from_raw(ar.cast::<Archive>()) };
    tok(format!("ARD(a{})", archive.id));
}

unsafe extern "C" fn svc_md5_context_new() -> P {
    Box::into_raw(Box::new(Md5::default())).cast()
}

unsafe extern "C" fn svc_md5_context_init(ctx: P) {
    unsafe { *ctx.cast::<Md5>() = Md5::new() };
}

unsafe extern "C" fn svc_md5_context_update(ctx: P, data: *const u8, len: i32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    unsafe { (*ctx.cast::<Md5>()).update(bytes) };
}

unsafe extern "C" fn svc_md5_context_final(digest: *mut u8, ctx: P) {
    let out = unsafe { (*ctx.cast::<Md5>()).finish() };
    unsafe { std::ptr::copy_nonoverlapping(out.as_ptr(), digest, 16) };
}

unsafe extern "C" fn svc_md5_context_delete(ctx: P) {
    if !ctx.is_null() {
        drop(unsafe { Box::from_raw(ctx.cast::<Md5>()) });
    }
}

unsafe extern "C" fn svc_debugf(s: *const TChar) {
    tok(format!("DBG({})", unsafe { text_at(s) }));
}

unsafe extern "C" fn svc_warn_text(s: *const TChar) {
    tok(format!("LOG({})", unsafe { text_at(s) }));
}

unsafe extern "C" fn svc_set_requesting_exit() {
    with_mock(|m| m.exit = true);
}

unsafe extern "C" fn svc_proc_create(url: *const TChar, params: *const TChar, show: i32) -> P {
    let url = unsafe { text_at(url) };
    let params = unsafe { text_at(params) };
    let show = i32::from(show != 0);
    let (h, null) = with_mock(|m| {
        m.handle_seq += 1;
        (m.handle_seq, m.cpnull)
    });
    let shown = if null { 0 } else { h };
    tok(format!("CP({url},{params},{show})=h{shown}"));
    if null { std::ptr::null_mut() } else { h as P }
}

unsafe extern "C" fn svc_proc_return_code(handle: P, rc: *mut i32) -> i32 {
    let h = handle as usize;
    let (gpr, value) = with_mock(|m| (m.gpr, m.gpr_rc));
    if !gpr {
        tok(format!("GPR(h{h})=0"));
        return 0;
    }
    tok(format!("GPR(h{h})=1/{value}"));
    unsafe { *rc = value };
    1
}

unsafe extern "C" fn svc_launch_url(url: *const TChar) {
    let url = unsafe { text_at(url) };
    tok(format!("LU(/work/{url})"));
}

static SVC: StatLogServices = StatLogServices {
    actor_level: svc_actor_level,
    level_game: svc_level_game,
    level_not_dedicated: svc_level_not_dedicated,
    game_external_batcher: svc_game_external,
    game_local_log_file_name: svc_game_local_log_file_name,
    fstr_chars: svc_fstr_chars,
    fstr_len: svc_fstr_len,
    fstr_assign: svc_fstr_assign,
    fstr_append: svc_fstr_append,
    stat_log_local_batcher_url: svc_stat_local_batcher_url,
    stat_log_local_log_dir: svc_stat_local_log_dir,
    stat_log_world_batcher_url: svc_stat_world_batcher_url,
    stat_log_world_batcher_params: svc_stat_world_batcher_params,
    stat_log_class_default_object: svc_stat_class_default_object,
    stat_log_static_default_object: svc_stat_static_default_object,
    stat_log_get_world_batcher_error: svc_stat_get_world_batcher_error,
    stat_log_set_world_batcher_error: svc_stat_set_world_batcher_error,
    stat_log_save_config: svc_stat_save_config,
    stat_log_get_world: svc_stat_get_world,
    stat_log_file_name: svc_stat_file_name,
    stat_log_file_final: svc_stat_file_final,
    stat_log_player_secret: svc_stat_player_secret,
    stat_log_player_name: svc_stat_player_name,
    stat_log_event_special_class: svc_stat_event_special_class,
    stat_log_event_special2: svc_stat_event_special2,
    class_iterate_packages: svc_class_iterate_packages,
    package_full_name: svc_package_full_name,
    dll_ext: svc_dll_ext,
    file_size: svc_file_size,
    file_make_directory: svc_file_make_directory,
    file_move: svc_file_move,
    file_create_writer: svc_file_create_writer,
    archive_serialize: svc_archive_serialize,
    archive_flush: svc_archive_flush,
    archive_delete: svc_archive_delete,
    md5_context_new: svc_md5_context_new,
    md5_context_init: svc_md5_context_init,
    md5_context_update: svc_md5_context_update,
    md5_context_final: svc_md5_context_final,
    md5_context_delete: svc_md5_context_delete,
    debugf_text: svc_debugf,
    warn_text: svc_warn_text,
    set_requesting_exit: svc_set_requesting_exit,
    proc_create: svc_proc_create,
    proc_return_code: svc_proc_return_code,
    launch_url: svc_launch_url,
};

// ---------------------------------------------------------------------------
// Fixture scenario. Objects persist across cases, as in the harness.
// ---------------------------------------------------------------------------

struct Env {
    stat: *mut Obj,
    files: [*mut Obj; 3],
    level: *mut Obj,
    game: *mut Obj,
    pawn: *mut Obj,
    mutator: *mut Obj,
    checksum: *mut FStr,
}

fn setup() -> Env {
    let stat_class = class_obj("Engine.StatLog");
    let default_obj = new_obj(stat_class, "");
    with_mock(|m| m.default_obj = default_obj);

    let level = new_obj(class_obj("Engine.LevelInfo"), "");
    let game = new_obj(class_obj("Engine.GameInfo"), "");
    let stat = new_obj(stat_class, "");
    let pri = new_obj(class_obj("Engine.PlayerReplicationInfo"), "");
    let pawn = new_obj(class_obj("Engine.PlayerPawn"), "");
    let mutator = new_obj(class_obj("Engine.Mutator"), "");
    let files = [
        std::ptr::null_mut(),
        new_obj(class_obj("Engine.StatLogFile"), ""),
        new_obj(class_obj("Engine.StatLogFile"), ""),
    ];
    unsafe {
        (*level).flag = 1;
        (*level).level = game;
        (*stat).level = level;
        (*pawn).level = pri;
        for f in files.iter().skip(1) {
            (**f).level = level;
        }
    }
    set_str(s_of(default_obj, 0), "/bin/dl");
    set_str(s_of(default_obj, 1), "/logs");
    set_str(s_of(stat, 2), "/bin/wb");
    set_str(s_of(stat, 3), "wp");
    Env {
        stat,
        files,
        level,
        game,
        pawn,
        mutator,
        checksum: Box::into_raw(Box::new(FStr::default())),
    }
}

fn reset(env: &Env) {
    with_mock(|m| {
        m.trace.clear();
        m.cpnull = false;
        m.gpr = false;
        m.gpr_rc = 0;
        m.exit = false;
        m.files.clear();
    });
    unsafe { (*env.checksum).units.clear() };
}

fn set_files(spec: &str) {
    with_mock(|m| {
        for item in spec.split(',') {
            if let Some((name, size)) = item.rsplit_once(':') {
                m.files
                    .insert(name.to_string(), size.parse().expect("file size"));
            }
        }
    });
}

fn set_classes(spec: &str) {
    let classes: Vec<*mut Obj> = spec.split(',').map(class_obj).collect();
    with_mock(|m| m.iter_classes = classes);
}

fn dedicated(env: &Env, d: bool) {
    unsafe { (*env.level).flag = i32::from(!d) };
}

fn external(env: &Env, e: bool) {
    unsafe { (*env.game).flag = i32::from(e) };
}

fn set_gpr(v: bool, rc: i32) {
    with_mock(|m| {
        m.gpr = v;
        m.gpr_rc = rc;
    });
}

fn set_cpnull(v: bool) {
    with_mock(|m| m.cpnull = v);
}

fn world_setup(env: &Env, ext: bool) {
    dedicated(env, false);
    external(env, ext);
    set_str(s_of(env.stat, 2), "/bin/wb");
    set_str(s_of(env.stat, 3), "wp");
}

fn set_bwbe(env: &Env, v: i32) {
    unsafe { (*env.stat).flag = v };
}

fn set_dbwbe(v: i32) {
    with_mock(|m| unsafe { (*m.default_obj).flag = v });
}

fn file_case(env: &Env, inst: usize, bworld: bool) -> *mut Obj {
    let f = env.files[inst];
    unsafe { (*f).flag = i32::from(bworld) };
    f
}

fn ev_str(s: &str) -> *mut FStr {
    Box::into_raw(Box::new(FStr { units: wide(s) }))
}

/// Maps a native error to the shell's behaviour: `appFailAssert` or `appErrorf`.
fn raise(err: StatLogError, file: &str) {
    match err {
        StatLogError::CheckFailed(expr) => tok(format!("ASSERT({expr})")),
        StatLogError::FileMissing => tok(format!("ERR(Could not open file: {file})")),
        other => panic!("{other:?} is not exercised by the fixture"),
    }
}

fn check(r: StatLogResult) {
    if let Err(e) = r {
        raise(e, "");
    }
}

// Case runners. Each returns the `ret=` value.

fn c_local_show(e: &Env) -> &'static str {
    dedicated(e, false);
    set_str(s_of(e.game, 0), "Game.log");
    set_str(s_of(e.stat, 0), "/bin/batcher");
    unsafe { execute_local_log_batcher(&SVC, e.stat.cast()) };
    "-"
}

fn c_local_dedicated(e: &Env) -> &'static str {
    dedicated(e, true);
    set_str(s_of(e.game, 0), "Game.log");
    set_str(s_of(e.stat, 0), "/bin/batcher");
    unsafe { execute_local_log_batcher(&SVC, e.stat.cast()) };
    "-"
}

fn c_silent_show(e: &Env) -> &'static str {
    dedicated(e, false);
    set_str(s_of(e.game, 0), "Game.log");
    set_str(s_of(e.stat, 0), "/bin/batcher");
    unsafe { execute_silent_log_batcher(&SVC, e.stat.cast()) };
    "-"
}

fn c_batch_local(e: &Env) -> &'static str {
    dedicated(e, false);
    unsafe { batch_local(&SVC, e.stat.cast()) };
    "-"
}

fn c_browse_relative(_e: &Env) -> &'static str {
    let url = ev_str("Maps/A.u");
    unsafe { browse_relative_local_url(&SVC, url.cast()) };
    "-"
}

fn c_world_first(e: &Env) -> &'static str {
    world_setup(e, false);
    set_bwbe(e, 0);
    set_dbwbe(0);
    set_cpnull(false);
    check(unsafe { execute_world_log_batcher(&SVC, e.stat.cast()) });
    "-"
}

fn c_world_running(e: &Env) -> &'static str {
    world_setup(e, false);
    set_bwbe(e, 0);
    set_dbwbe(0);
    set_cpnull(false);
    set_gpr(false, 0);
    check(unsafe { execute_world_log_batcher(&SVC, e.stat.cast()) });
    "-"
}

fn c_world_prev_fail(e: &Env) -> &'static str {
    world_setup(e, false);
    set_bwbe(e, 0);
    set_dbwbe(0);
    set_cpnull(false);
    set_gpr(true, -1);
    check(unsafe { execute_world_log_batcher(&SVC, e.stat.cast()) });
    "-"
}

fn c_world_prev_ok(e: &Env) -> &'static str {
    world_setup(e, false);
    set_bwbe(e, 0);
    set_dbwbe(0);
    set_cpnull(false);
    set_gpr(true, 0);
    check(unsafe { execute_world_log_batcher(&SVC, e.stat.cast()) });
    "-"
}

fn c_world_create_fail(e: &Env) -> &'static str {
    world_setup(e, false);
    set_bwbe(e, 0);
    set_dbwbe(0);
    set_cpnull(true);
    set_gpr(true, 0);
    check(unsafe { execute_world_log_batcher(&SVC, e.stat.cast()) });
    "-"
}

fn c_world_external(e: &Env) -> &'static str {
    external(e, true);
    set_bwbe(e, 1);
    set_dbwbe(0);
    check(unsafe { execute_world_log_batcher(&SVC, e.stat.cast()) });
    "-"
}

fn c_initial_basic(e: &Env) -> &'static str {
    set_classes("Engine.GameInfo,Engine.Actor,Core.Object");
    unsafe { (*e.game).class = class_obj("Engine.GameInfo") };
    set_files("Engine.u:1234,Engine.dylib:777,Core.u:55");
    check(unsafe { initial_check(&SVC, e.stat.cast(), e.game.cast()) });
    "-"
}

fn c_initial_dedup(e: &Env) -> &'static str {
    set_classes("Engine.GameInfo,Engine.Actor");
    unsafe { (*e.game).class = class_obj("Engine.GameInfo") };
    set_files("Engine.u:1234,Engine.dylib:777");
    check(unsafe { initial_check(&SVC, e.stat.cast(), e.game.cast()) });
    "-"
}

fn c_log_mutator(e: &Env) -> &'static str {
    unsafe { (*e.mutator).class = class_obj("Engine.Mutator") };
    unsafe { log_mutator(&SVC, e.stat.cast(), e.mutator.cast()) };
    "-"
}

fn c_pcs_nosecret(e: &Env) -> &'static str {
    set_str(s_of(e.pawn, 0), "");
    set_str(s_of(e.level_pri(), 0), "Player One");
    check(unsafe { get_player_checksum(&SVC, e.pawn.cast(), e.checksum.cast()) });
    "-"
}

fn c_pcs_secret(e: &Env) -> &'static str {
    set_str(s_of(e.pawn, 0), "xyz");
    set_str(s_of(e.level_pri(), 0), "Player One");
    check(unsafe { get_player_checksum(&SVC, e.pawn.cast(), e.checksum.cast()) });
    "-"
}

fn c_open_nonworld(e: &Env) -> &'static str {
    let f = file_case(e, 1, false);
    set_str(s_of(f, 0), "Logs/x.log");
    let dir: Vec<TChar> = "../Logs\0".encode_utf16().map(|u| u as TChar).collect();
    unsafe { open_log(&SVC, f.cast(), dir.as_ptr()) };
    "-"
}

fn c_filelog_nonworld(e: &Env) -> &'static str {
    let f = file_case(e, 1, false);
    let ev = ev_str("Hello");
    check(unsafe { file_log(&SVC, f.cast(), ev.cast()) });
    "-"
}

fn c_file_flush(e: &Env) -> &'static str {
    let f = file_case(e, 1, false);
    unsafe { file_flush(&SVC, f.cast()) };
    "-"
}

fn c_close_nonworld(e: &Env) -> &'static str {
    let f = file_case(e, 1, false);
    set_str(s_of(f, 0), "Logs/x.log");
    set_str(s_of(f, 1), "Logs/x.final.log");
    unsafe { close_log(&SVC, f.cast()) };
    "-"
}

fn c_open_world(e: &Env) -> &'static str {
    let f = file_case(e, 1, true);
    set_str(s_of(f, 0), "Logs/w.log");
    let dir: Vec<TChar> = "../Logs\0".encode_utf16().map(|u| u as TChar).collect();
    unsafe { open_log(&SVC, f.cast(), dir.as_ptr()) };
    "-"
}

fn c_watermark(e: &Env) -> &'static str {
    let f = file_case(e, 1, true);
    let ev = ev_str("Mark");
    check(unsafe { watermark(&SVC, f.cast(), ev.cast()) });
    "-"
}

fn c_get_checksum(e: &Env) -> &'static str {
    let f = file_case(e, 1, true);
    check(unsafe { get_checksum(&SVC, f.cast(), e.checksum.cast()) });
    "-"
}

fn c_get_checksum_twice(e: &Env) -> &'static str {
    let f = file_case(e, 1, true);
    check(unsafe { get_checksum(&SVC, f.cast(), e.checksum.cast()) });
    check(unsafe { get_checksum(&SVC, f.cast(), e.checksum.cast()) });
    "-"
}

fn c_file_log_world(e: &Env) -> &'static str {
    let f = file_case(e, 1, true);
    let ev = ev_str("Hi");
    check(unsafe { file_log(&SVC, f.cast(), ev.cast()) });
    "-"
}

fn c_watermark_nocontext(e: &Env) -> &'static str {
    let f = file_case(e, 2, false);
    let ev = ev_str("Mark");
    check(unsafe { watermark(&SVC, f.cast(), ev.cast()) });
    "-"
}

fn c_get_checksum_nocontext(e: &Env) -> &'static str {
    let f = file_case(e, 2, false);
    check(unsafe { get_checksum(&SVC, f.cast(), e.checksum.cast()) });
    "-"
}

fn c_close_world(e: &Env) -> &'static str {
    let f = file_case(e, 1, true);
    set_str(s_of(f, 0), "Logs/w.log");
    set_str(s_of(f, 1), "Logs/w.final.log");
    unsafe { close_log(&SVC, f.cast()) };
    "-"
}

fn run_commandlet(file: &str, files: &str) -> &'static str {
    set_files(files);
    let name = nul_units(file);
    match unsafe { checksum_commandlet_main(&SVC, name.as_ptr()) } {
        Ok(()) => "0",
        Err(e) => {
            raise(e, file);
            "-"
        }
    }
}

fn c_checksum_backslash(_e: &Env) -> &'static str {
    run_commandlet("..\\Maps\\Engine.u", "..\\Maps\\Engine.u:4096")
}

fn c_checksum_slash(_e: &Env) -> &'static str {
    run_commandlet("Maps/Foo.u", "Maps/Foo.u:2048")
}

fn c_checksum_missing(_e: &Env) -> &'static str {
    run_commandlet("Nope.u", "Maps/Other.u:10")
}

impl Env {
    fn level_pri(&self) -> *mut Obj {
        unsafe { (*self.pawn).level }
    }
}

struct Case {
    name: &'static str,
    run: fn(&Env) -> &'static str,
    /// Whether the record prints `DefBWBE=`.
    defbwbe: bool,
    /// Whether the record prints `Checksum=`.
    checksum: bool,
}

const CASES: &[Case] = &[
    Case {
        name: "LocalBatcher_show",
        run: c_local_show,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "LocalBatcher_dedicated",
        run: c_local_dedicated,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "SilentBatcher_show",
        run: c_silent_show,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "BatchLocal",
        run: c_batch_local,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "BrowseRelative",
        run: c_browse_relative,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "WorldBatcher_first",
        run: c_world_first,
        defbwbe: true,
        checksum: false,
    },
    Case {
        name: "WorldBatcher_running",
        run: c_world_running,
        defbwbe: true,
        checksum: false,
    },
    Case {
        name: "WorldBatcher_prev_fail",
        run: c_world_prev_fail,
        defbwbe: true,
        checksum: false,
    },
    Case {
        name: "WorldBatcher_prev_ok",
        run: c_world_prev_ok,
        defbwbe: true,
        checksum: false,
    },
    Case {
        name: "WorldBatcher_create_fail",
        run: c_world_create_fail,
        defbwbe: true,
        checksum: false,
    },
    Case {
        name: "WorldBatcher_external",
        run: c_world_external,
        defbwbe: true,
        checksum: false,
    },
    Case {
        name: "InitialCheck_basic",
        run: c_initial_basic,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "InitialCheck_dedup",
        run: c_initial_dedup,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "LogMutator",
        run: c_log_mutator,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "GetPlayerChecksum_nosecret",
        run: c_pcs_nosecret,
        defbwbe: false,
        checksum: true,
    },
    Case {
        name: "GetPlayerChecksum_secret",
        run: c_pcs_secret,
        defbwbe: false,
        checksum: true,
    },
    Case {
        name: "OpenLog_nonworld",
        run: c_open_nonworld,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "FileLog_nonworld",
        run: c_filelog_nonworld,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "FileFlush",
        run: c_file_flush,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "CloseLog_nonworld",
        run: c_close_nonworld,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "OpenLog_world",
        run: c_open_world,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "Watermark",
        run: c_watermark,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "GetChecksum",
        run: c_get_checksum,
        defbwbe: false,
        checksum: true,
    },
    Case {
        name: "GetChecksum_twice",
        run: c_get_checksum_twice,
        defbwbe: false,
        checksum: true,
    },
    Case {
        name: "FileLog_world",
        run: c_file_log_world,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "Watermark_nocontext",
        run: c_watermark_nocontext,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "GetChecksum_nocontext",
        run: c_get_checksum_nocontext,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "CloseLog_world",
        run: c_close_world,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "Checksum_backslash",
        run: c_checksum_backslash,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "Checksum_slash",
        run: c_checksum_slash,
        defbwbe: false,
        checksum: false,
    },
    Case {
        name: "Checksum_missing",
        run: c_checksum_missing,
        defbwbe: false,
        checksum: false,
    },
];

#[derive(Debug, PartialEq)]
struct Record {
    name: String,
    ret: String,
    out: String,
    trace: String,
}

fn parse_fixture(line: &str) -> Record {
    let rest = line.strip_prefix("case=").expect("case= prefix");
    let (name, rest) = rest.split_once(" in=").expect(" in=");
    let (_in, rest) = rest.split_once(" ret=").expect(" ret=");
    let (ret, rest) = rest.split_once(" out=").expect(" out=");
    let (out, trace) = rest.split_once(" trace=").expect(" trace=");
    Record {
        name: name.to_string(),
        ret: ret.to_string(),
        out: out.to_string(),
        trace: trace.to_string(),
    }
}

#[test]
fn stat_log_replays_original_fixture() {
    let env = setup();
    let expected: Vec<Record> = EXPECTED.lines().map(parse_fixture).collect();
    assert_eq!(expected.len(), EXPECTED_RECORDS, "fixture record count");
    assert_eq!(CASES.len(), EXPECTED_RECORDS, "case table count");

    let mut actual = Vec::with_capacity(CASES.len());
    for case in CASES {
        reset(&env);
        let ret = (case.run)(&env);
        let mut out = Vec::new();
        if case.checksum {
            let c = unsafe { &(*env.checksum).units };
            out.push(format!("Checksum={}", text(c)));
        }
        if case.defbwbe {
            let d = with_mock(|m| unsafe { (*m.default_obj).flag });
            out.push(format!("DefBWBE={d}"));
        }
        let exit = with_mock(|m| m.exit);
        out.push(format!("GIsRequestingExit={}", i32::from(exit)));
        let trace = with_mock(|m| {
            if m.trace.is_empty() {
                "-".to_string()
            } else {
                m.trace.join("|")
            }
        });
        actual.push(Record {
            name: case.name.to_string(),
            ret: ret.to_string(),
            out: out.join(";"),
            trace,
        });
    }

    assert_eq!(actual.len(), expected.len());
    for (i, (act, exp)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(act, exp, "record {} ({})", i + 1, exp.name);
    }
}
