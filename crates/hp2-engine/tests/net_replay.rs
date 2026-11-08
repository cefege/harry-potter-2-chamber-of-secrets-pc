//! Replay test for the `FReplay` port (`net_replay.rs`).
//!
//! `fixtures/net_replay.txt` is generated from the ORIGINAL `UnReplay.cpp` by
//! `Build/net_fixture_harness/replay_fixture.cpp` (`gen_net.sh <builddir> replay`).
//! Each line is one FReplay call plus the object state after it. This test drives
//! the Rust port with the same inputs through services mocks that reproduce the
//! harness engine stubs (in-memory file store, archive, log capture, reseed
//! counter), rebuilds each line in the harness format and requires byte equality.
//!
//! Fault-injection tests make one mock service report status 1 (the C++ thunk
//! caught a throw). They check that the port returns `Err` at that service, makes
//! no further service call, and leaves `FReplay` in the state the C++ throw left.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

use hp2_abi::engine::generated::{FReplay, FReplayInputEvent, TArrayRaw};
use hp2_engine::net_replay::{self, Hp2NetReplayServices, Thrown};

const FIXTURE: &str = include_str!("fixtures/net_replay.txt");
/// Number of records the harness emits (one per scenario op).
const RECORD_COUNT: usize = 28;

/// Harness file store, archive and log state. Thread-local because the services
/// are plain function pointers; each test runs on its own thread.
#[derive(Default)]
struct Mock {
    /// Harness `files` map: archive contents keyed by file name.
    files: BTreeMap<String, Vec<u8>>,
    /// Log text produced by the current op (cleared after each record).
    log: String,
    /// Number of `appResetRandTraceForReplay` calls.
    reseeds: u32,
    /// Backing storage for FString buffers handed to the port.
    strings: Vec<Box<[u32]>>,
    /// Services entered, in order (fault-injection tests).
    calls: Vec<&'static str>,
    /// Service whose next call reports status 1 with no effect.
    throw_on: Option<&'static str>,
}

/// Archive handle returned by `replay_create_file`.
struct Archive {
    name: String,
    loading: bool,
    pos: usize,
}

thread_local! {
    static MOCK: RefCell<Mock> = RefCell::new(Mock::default());
}

fn with_mock<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

/// Records entry into a service. Returns true when the injected throw fires.
fn enter(name: &'static str) -> bool {
    with_mock(|m| {
        m.calls.push(name);
        m.throw_on == Some(name)
    })
}

/// Clears the call log and sets (or clears) the injected throw.
fn begin(throw: Option<&'static str>) {
    with_mock(|m| {
        m.calls.clear();
        m.throw_on = throw;
    });
}

fn calls() -> Vec<&'static str> {
    with_mock(|m| m.calls.clone())
}

fn wide(s: &str) -> Vec<u32> {
    s.bytes().map(u32::from).collect()
}

fn units_string(units: &[u32]) -> String {
    units
        .iter()
        .map(|&u| char::from_u32(u).expect("ascii unit"))
        .collect()
}

/// Reads a NUL-terminated `TCHAR` string (4-byte units).
///
/// # Safety
/// `p` must point to a NUL-terminated buffer of `u32` units.
unsafe fn units_z(p: *const c_void) -> Vec<u32> {
    let mut out = Vec::new();
    let mut q = p.cast::<u32>();
    loop {
        // SAFETY: services contract; reads stop at the terminator.
        let u = unsafe { *q };
        if u == 0 {
            break;
        }
        out.push(u);
        // SAFETY: stays within the NUL-terminated buffer.
        q = unsafe { q.add(1) };
    }
    out
}

/// Reads the characters of an FString (`TArrayRaw`), without the terminator.
///
/// # Safety
/// `p` must point to a live `TArrayRaw` whose buffer holds `num` units.
unsafe fn fstring_units(p: *const c_void) -> Vec<u32> {
    // SAFETY: services contract.
    let arr = unsafe { &*p.cast::<TArrayRaw>() };
    if arr.num <= 1 {
        return Vec::new();
    }
    // SAFETY: services contract; `num` units including the terminator.
    unsafe { std::slice::from_raw_parts(arr.data.cast::<u32>(), (arr.num - 1) as usize) }.to_vec()
}

/// `FString::operator=`: stores `units` with a terminator, or clears the string.
///
/// # Safety
/// `p` must point to a live `TArrayRaw`.
unsafe fn fstring_set(p: *mut c_void, units: &[u32]) {
    // SAFETY: services contract.
    let arr = unsafe { &mut *p.cast::<TArrayRaw>() };
    if units.is_empty() {
        arr.num = 0;
        arr.data = null_mut();
        return;
    }
    let mut buf: Box<[u32]> = units.iter().copied().chain(std::iter::once(0)).collect();
    arr.num = buf.len() as i32;
    arr.data = buf.as_mut_ptr().cast();
    with_mock(|m| m.strings.push(buf));
}

/// # Safety
/// `ar` must be a live `Archive` handle.
unsafe fn archive<'a>(ar: *mut c_void) -> &'a mut Archive {
    // SAFETY: services contract.
    unsafe { &mut *ar.cast::<Archive>() }
}

fn archive_write(m: &mut Mock, a: &mut Archive, bytes: &[u8]) {
    m.files
        .get_mut(&a.name)
        .expect("archive file")
        .extend_from_slice(bytes);
}

fn archive_read(m: &Mock, a: &mut Archive, n: usize) -> Vec<u8> {
    let data = &m.files[&a.name];
    let out = data[a.pos..a.pos + n].to_vec();
    a.pos += n;
    out
}

fn read_i32(m: &Mock, a: &mut Archive) -> i32 {
    let b = archive_read(m, a, 4);
    i32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn read_f32(m: &Mock, a: &mut Archive) -> f32 {
    let b = archive_read(m, a, 4);
    f32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn read_u8(m: &Mock, a: &mut Archive) -> u8 {
    archive_read(m, a, 1)[0]
}

// ---- services (mirror the harness engine stubs) ---------------------------------

/// # Safety
/// `file_name` must be a live FString.
unsafe extern "C" fn log_already_recording(_ar: *mut c_void, file_name: *const c_void) -> i32 {
    if enter("log_already_recording") {
        return 1;
    }
    // SAFETY: services contract.
    let name = unsafe { fstring_units(file_name) };
    with_mock(|m| m.log = format!("Already recording: {}", units_string(&name)));
    0
}

/// # Safety
/// `file_name` must be a live FString.
unsafe extern "C" fn log_already_replaying(_ar: *mut c_void, file_name: *const c_void) -> i32 {
    if enter("log_already_replaying") {
        return 1;
    }
    // SAFETY: services contract.
    let name = unsafe { fstring_units(file_name) };
    with_mock(|m| m.log = format!("Already replaying: {}", units_string(&name)));
    0
}

/// # Safety
/// `file_name` must be a live FString.
unsafe extern "C" fn log_cannot_open(
    _ar: *mut c_void,
    record: i32,
    file_name: *const c_void,
) -> i32 {
    if enter("log_cannot_open") {
        return 1;
    }
    // SAFETY: services contract.
    let name = unsafe { fstring_units(file_name) };
    let verb = if record != 0 { "create" } else { "open" };
    with_mock(|m| m.log = format!("Cannot {verb} file {}", units_string(&name)));
    0
}

/// # Safety
/// `file_name` must be a live FString; `in_name` a NUL-terminated string.
unsafe extern "C" fn file_name_assign(file_name: *mut c_void, in_name: *const c_void) -> i32 {
    if enter("file_name_assign") {
        return 1;
    }
    // SAFETY: services contract.
    let mut s = wide("UserDir/");
    s.extend(unsafe { units_z(in_name) });
    s.extend(wide(".rep"));
    // SAFETY: services contract.
    unsafe { fstring_set(file_name, &s) };
    0
}

/// # Safety
/// `url_str` must be a live FString; `start_url` NULL or NUL-terminated.
unsafe extern "C" fn url_assign(url_str: *mut c_void, start_url: *const c_void) -> i32 {
    if enter("url_assign") {
        return 1;
    }
    // A NULL start URL leaves an empty FString empty (`operator=` no-op).
    let units = if start_url.is_null() {
        Vec::new()
    } else {
        // SAFETY: services contract.
        unsafe { units_z(start_url) }
    };
    // SAFETY: services contract.
    unsafe { fstring_set(url_str, &units) };
    0
}

/// # Safety
/// `file_name` must be a live FString; `out` must be a live out-pointer.
unsafe extern "C" fn create_file(
    file_name: *const c_void,
    record: i32,
    out: *mut *mut c_void,
) -> i32 {
    if enter("create_file") {
        return 1;
    }
    // SAFETY: services contract.
    let name = units_string(&unsafe { fstring_units(file_name) });
    let ar = with_mock(|m| {
        if record != 0 {
            if name.contains("bad") {
                return null_mut();
            }
            m.files.insert(name.clone(), Vec::new());
            Box::into_raw(Box::new(Archive {
                name,
                loading: false,
                pos: 0,
            }))
            .cast::<c_void>()
        } else {
            if !m.files.contains_key(&name) {
                return null_mut();
            }
            Box::into_raw(Box::new(Archive {
                name,
                loading: true,
                pos: 0,
            }))
            .cast::<c_void>()
        }
    });
    // SAFETY: services contract.
    unsafe { *out = ar };
    0
}

/// `FArchive << URLStr`: INT count, then one 4-byte unit per character.
///
/// # Safety
/// `ar` a live archive handle; `url_str` a live FString.
unsafe extern "C" fn archive_serialize_url(ar: *mut c_void, url_str: *mut c_void) -> i32 {
    if enter("archive_serialize_url") {
        return 1;
    }
    // SAFETY: services contract.
    let a = unsafe { archive(ar) };
    if a.loading {
        let units = with_mock(|m| {
            let n = read_i32(m, a) as usize;
            let mut units = Vec::with_capacity(n);
            for _ in 0..n {
                let b = archive_read(m, a, 4);
                units.push(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            }
            units
        });
        // SAFETY: services contract; `fstring_set` takes the mock borrow itself.
        unsafe { fstring_set(url_str, &units) };
    } else {
        // SAFETY: services contract.
        let units = unsafe { fstring_units(url_str) };
        with_mock(|m| {
            archive_write(m, a, &(units.len() as i32).to_le_bytes());
            for u in units {
                archive_write(m, a, &u.to_le_bytes());
            }
        });
    }
    0
}

/// `operator<<(FArchive&, FReplay::FInputEvent&)` from `UnReplayWire.cpp`.
///
/// # Safety
/// `ar` a live archive handle; `ie` a live `FReplayInputEvent`.
unsafe extern "C" fn archive_serialize_input(ar: *mut c_void, ie: *mut c_void) -> i32 {
    if enter("archive_serialize_input") {
        return 1;
    }
    // SAFETY: services contract.
    let a = unsafe { archive(ar) };
    // SAFETY: services contract.
    let ie = unsafe { &mut *ie.cast::<FReplayInputEvent>() };
    with_mock(|m| {
        let mut key = ie.i_key as u8;
        let mut state = ie.state as u8;
        if ie.delta != 0.0 {
            state |= 0x80;
        }
        if a.loading {
            key = read_u8(m, a);
            state = read_u8(m, a);
        } else {
            archive_write(m, a, &[key, state]);
        }
        ie.i_key = i32::from(key);
        ie.state = i32::from(state & 0x7f);
        if state & 0x80 != 0 {
            if a.loading {
                ie.delta = read_f32(m, a);
            } else {
                archive_write(m, a, &ie.delta.to_bits().to_le_bytes());
            }
        } else if a.loading {
            ie.delta = 0.0;
        }
    });
    0
}

/// # Safety
/// `ar` a live archive handle; `out` a live out-pointer.
unsafe extern "C" fn archive_at_end(ar: *mut c_void, out: *mut i32) -> i32 {
    if enter("archive_at_end") {
        return 1;
    }
    // SAFETY: services contract.
    let a = unsafe { archive(ar) };
    let at_end = with_mock(|m| i32::from(a.pos >= m.files[&a.name].len()));
    // SAFETY: services contract.
    unsafe { *out = at_end };
    0
}

unsafe extern "C" fn archive_flush(_ar: *mut c_void) -> i32 {
    if enter("archive_flush") {
        return 1;
    }
    0
}

/// # Safety
/// `ar` must be NULL or a live archive handle that is not used afterwards.
unsafe extern "C" fn archive_delete(ar: *mut c_void) -> i32 {
    if enter("archive_delete") {
        return 1;
    }
    if !ar.is_null() {
        // SAFETY: services contract; handle came from `create_file`.
        drop(unsafe { Box::from_raw(ar.cast::<Archive>()) });
    }
    0
}

unsafe extern "C" fn reseed_for_replay() -> i32 {
    if enter("reseed_for_replay") {
        return 1;
    }
    with_mock(|m| m.reseeds += 1);
    0
}

fn services() -> Hp2NetReplayServices {
    Hp2NetReplayServices {
        replay_log_already_recording: log_already_recording,
        replay_log_already_replaying: log_already_replaying,
        replay_log_cannot_open: log_cannot_open,
        replay_file_name_assign: file_name_assign,
        replay_url_assign: url_assign,
        replay_create_file: create_file,
        replay_archive_serialize_url: archive_serialize_url,
        replay_archive_serialize_input: archive_serialize_input,
        replay_archive_at_end: archive_at_end,
        replay_archive_flush: archive_flush,
        replay_archive_delete: archive_delete,
        replay_reseed_for_replay: reseed_for_replay,
    }
}

/// FNV-1a 64 over (name, 0, bytes, 0xff) per entry in byte order, as in the harness.
fn digest(m: &Mock) -> String {
    fn step(h: &mut u64, b: u8) {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(1_099_511_628_211);
    }
    let mut h: u64 = 14_695_981_039_346_656_037;
    for (name, bytes) in &m.files {
        for &b in name.as_bytes() {
            step(&mut h, b);
        }
        step(&mut h, 0);
        for &b in bytes {
            step(&mut h, b);
        }
        step(&mut h, 0xff);
    }
    format!("{}:{h:016x}", m.files.len())
}

fn hex_bits(s: &str) -> u32 {
    u32::from_str_radix(s, 16).expect("hex float bits")
}

fn arg<'a>(args: &BTreeMap<&str, &'a str>, key: &str) -> &'a str {
    args.get(key)
        .copied()
        .unwrap_or_else(|| panic!("missing arg {key}"))
}

/// `FReplay::OpenFile` with a NUL-terminated name and no start URL.
fn open(
    r: *mut FReplay,
    svc: &Hp2NetReplayServices,
    name: &str,
    record: bool,
) -> net_replay::Res<bool> {
    let mut in_name = wide(name);
    in_name.push(0);
    // SAFETY: in_name is NUL-terminated and outlives the call; r is live.
    unsafe { net_replay::open_file(r, svc, in_name.as_ptr().cast(), record, null(), null_mut()) }
}

fn zeroed_replay() -> FReplay {
    // SAFETY: FReplay is plain data; all-zero is the harness's static `ReplayPeek` state.
    unsafe { std::mem::zeroed() }
}

#[test]
fn replay_fixture_matches_harness() {
    let svc = services();
    // SAFETY: FReplay is plain data; the all-zero state is the harness's static
    // `ReplayPeek` state (NULL FileAr, cleared flags, zero counters).
    let mut reps: [FReplay; 3] = unsafe { std::mem::zeroed() };

    let lines: Vec<&str> = FIXTURE.lines().collect();
    assert_eq!(lines.len(), RECORD_COUNT, "fixture record count");

    for (idx, line) in lines.iter().enumerate() {
        let toks: Vec<&str> = line.split(' ').collect();
        let obj: usize = toks[0]
            .strip_prefix("obj=")
            .expect("obj=")
            .parse()
            .expect("obj index");
        let op = toks[1].strip_prefix("op=").expect("op=");
        let args: BTreeMap<&str, &str> = toks[2..]
            .iter()
            .take_while(|t| !t.starts_with("ret="))
            .map(|t| t.split_once('=').expect("key=value"))
            .collect();
        let r: *mut FReplay = &mut reps[obj];

        let (op_text, ret): (String, &str) = match op {
            "open" => {
                let name = arg(&args, "name");
                let rec = arg(&args, "rec") == "1";
                let url = arg(&args, "url");
                let mut in_name = wide(name);
                in_name.push(0);
                let start_url: Option<Vec<u32>> = if url == "-" {
                    None
                } else {
                    let mut u = wide(url);
                    u.push(0);
                    Some(u)
                };
                let url_ptr = start_url
                    .as_ref()
                    .map_or(null_mut(), |u| u.as_ptr().cast_mut().cast::<c_void>());
                // SAFETY: buffers live across the call; ar is unused by the mocks.
                let ok = unsafe {
                    net_replay::open_file(
                        r,
                        &svc,
                        in_name.as_ptr().cast(),
                        rec,
                        url_ptr.cast_const(),
                        null_mut(),
                    )
                }
                .expect("no injected throw");
                (
                    format!("open name={name} rec={} url={url}", u8::from(rec)),
                    if ok { "1" } else { "0" },
                )
            }
            "stop" => {
                // SAFETY: r points into `reps`.
                let ok = unsafe { net_replay::stop(r, &svc) }.expect("no injected throw");
                ("stop".to_string(), if ok { "1" } else { "0" })
            }
            "tick" => {
                let delta = f32::from_bits(hex_bits(arg(&args, "delta")));
                let mut td = delta;
                // SAFETY: r points into `reps`; td is a live f32.
                let ok = unsafe { net_replay::serialize_frame_tick(r, &svc, &mut td) }
                    .expect("no injected throw");
                (
                    format!(
                        "tick delta={:08x} out={:08x}",
                        delta.to_bits(),
                        td.to_bits()
                    ),
                    if ok { "1" } else { "0" },
                )
            }
            "input" => {
                let key: i32 = arg(&args, "key").parse().expect("key");
                let state: i32 = arg(&args, "state").parse().expect("state");
                let delta = f32::from_bits(hex_bits(arg(&args, "delta")));
                // SAFETY: r points into `reps`.
                let ok =
                    unsafe { net_replay::serialize_frame_input_write(r, &svc, key, state, delta) }
                        .expect("no injected throw");
                (
                    format!(
                        "input key={key} state={state} delta={:08x}",
                        delta.to_bits()
                    ),
                    if ok { "1" } else { "0" },
                )
            }
            "input_ev" => {
                let key: i32 = arg(&args, "key").parse().expect("key");
                let state: i32 = arg(&args, "state").parse().expect("state");
                let delta = f32::from_bits(hex_bits(arg(&args, "delta")));
                let mut ie = FReplayInputEvent {
                    i_key: key,
                    state,
                    delta,
                };
                // SAFETY: r points into `reps`; ie is a live event.
                let ok = unsafe { net_replay::serialize_frame_input(r, &svc, &mut ie) }
                    .expect("no injected throw");
                (
                    format!(
                        "input_ev key={key} state={state} delta={:08x} out_key={} out_state={} out_delta={:08x}",
                        delta.to_bits(),
                        ie.i_key,
                        ie.state,
                        ie.delta.to_bits()
                    ),
                    if ok { "1" } else { "0" },
                )
            }
            "pause" => {
                let on = arg(&args, "on");
                let count: i32 = arg(&args, "count").parse().expect("count");
                // SAFETY: r points into `reps`.
                unsafe { net_replay::pause(r, on == "1", count) };
                (format!("pause on={on} count={count}"), "-")
            }
            other => panic!("unknown op {other} at line {}", idx + 1),
        };

        // SAFETY: r points into `reps`.
        let (rec_s, rep_s, in_s, pau_s, frame, pc, next) = unsafe {
            (
                u8::from(net_replay::recording(r)),
                u8::from(net_replay::replaying(r)),
                u8::from(net_replay::in_replay(r)),
                u8::from(net_replay::paused(r)),
                net_replay::frame_num(r),
                (*r).pause_count,
                (*r).next_tick,
            )
        };
        let (files, resets, log) = with_mock(|m| {
            let log = std::mem::take(&mut m.log);
            m.calls.clear();
            (
                digest(m),
                m.reseeds,
                if log.is_empty() { "-".to_string() } else { log },
            )
        });

        let rebuilt = format!(
            "obj={obj} op={op_text} ret={ret} recording={rec_s} replaying={rep_s} in_replay={in_s} paused={pau_s} frame={frame} pause_count={pc} next={:08x} files={files} resets={resets} log={log}",
            next.to_bits(),
        );
        assert_eq!(rebuilt, *line, "fixture line {} diverges", idx + 1);
    }
}

#[test]
fn open_file_serialize_url_throw_keeps_partial_state() {
    let svc = services();
    let mut rep = zeroed_replay();
    let r: *mut FReplay = &mut rep;

    begin(Some("archive_serialize_url"));
    assert_eq!(open(r, &svc, "Demo", true), Err(Thrown));
    // State the C++ throw left: archive created, recording set, frame reset,
    // seed not yet reset (the reseed call comes after the URL serialise).
    assert!(!rep.file_ar.is_null());
    assert_eq!(rep.b_recording, 1);
    assert_eq!(rep.frame_num, 0);
    assert_eq!(
        calls(),
        [
            "file_name_assign",
            "create_file",
            "url_assign",
            "archive_serialize_url"
        ]
    );
    assert_eq!(with_mock(|m| m.reseeds), 0);

    // Cleanup: no injected throw, stop releases the archive.
    begin(None);
    assert_eq!(unsafe { net_replay::stop(r, &svc) }, Ok(true));
    assert!(rep.file_ar.is_null());
}

#[test]
fn serialize_frame_tick_input_throw_leaves_frame_unchanged() {
    let svc = services();
    let mut rep = zeroed_replay();
    let r: *mut FReplay = &mut rep;

    begin(None);
    assert_eq!(open(r, &svc, "Tick", true), Ok(true));

    begin(Some("archive_serialize_input"));
    let mut td = 0.5f32;
    // SAFETY: r points at a live FReplay; td is a live f32.
    assert_eq!(
        unsafe { net_replay::serialize_frame_tick(r, &svc, &mut td) },
        Err(Thrown)
    );
    assert_eq!(calls(), ["archive_serialize_input"]);
    assert_eq!(
        rep.frame_num, 0,
        "FrameNum is not incremented after a throw"
    );
    assert_eq!(rep.b_recording, 1);

    begin(None);
    assert_eq!(unsafe { net_replay::stop(r, &svc) }, Ok(true));
}

#[test]
fn open_file_log_throw_keeps_file_ar() {
    let svc = services();
    let mut rep = zeroed_replay();
    let r: *mut FReplay = &mut rep;

    begin(None);
    assert_eq!(open(r, &svc, "First", true), Ok(true));
    let before = rep.file_ar;
    assert!(!before.is_null());

    // Opening while already recording logs and returns false; the log throws.
    begin(Some("log_already_recording"));
    assert_eq!(open(r, &svc, "Second", true), Err(Thrown));
    assert_eq!(calls(), ["log_already_recording"]);
    assert_eq!(rep.file_ar, before, "FileAr untouched by the throwing log");

    begin(None);
    assert_eq!(unsafe { net_replay::stop(r, &svc) }, Ok(true));
}

#[test]
fn open_file_first_tick_throw_skips_reseed() {
    let svc = services();
    let mut rep = zeroed_replay();
    let r: *mut FReplay = &mut rep;

    // Record a short file so the playback open has data to read.
    begin(None);
    assert_eq!(open(r, &svc, "Play", true), Ok(true));
    // One frame, so the playback open does not hit the end-of-file path first.
    assert_eq!(
        unsafe { net_replay::serialize_frame_input_write(r, &svc, 3, 0, 0.5) },
        Ok(true)
    );
    assert_eq!(unsafe { net_replay::stop(r, &svc) }, Ok(true));

    // Playback open: the first-tick read throws, so the reseed after it must not run.
    let reseeds_before = with_mock(|m| m.reseeds);
    begin(Some("archive_serialize_input"));
    assert_eq!(open(r, &svc, "Play", false), Err(Thrown));
    assert_eq!(calls().last().copied(), Some("archive_serialize_input"));
    assert_eq!(with_mock(|m| m.reseeds), reseeds_before);
    assert_eq!(rep.b_recording, 0);
    assert!(!rep.file_ar.is_null(), "archive kept after the throw");

    // Cleanup: no injected throw, stop releases the archive.
    begin(None);
    assert_eq!(unsafe { net_replay::stop(r, &svc) }, Ok(true));
    assert!(rep.file_ar.is_null());
}
