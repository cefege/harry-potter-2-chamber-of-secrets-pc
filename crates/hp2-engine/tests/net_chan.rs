//! Replays `tests/fixtures/net_chan.txt` (generated from `Build/net_fixture_harness/chan_fixture.cpp`)
//! against the Rust port in `net_chan.rs` / `net_bunch.rs`.
//!
//! Every fixture line is rebuilt here with the harness printf format and compared exactly.
//! The mock services emulate only the connection / bunch / channel state the harness exercises.
//! Any service that the exercised paths never reach aborts the process if it is called.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{c_char, c_void};
use std::mem::zeroed;
use std::ptr::addr_of_mut;

use hp2_abi::engine::generated::{
    FInBunch, FOutBunch, UChannel, UDownload, UNetConnection, UNetDriver, UPackageMap,
};
use hp2_abi::engine::generated::{TArrayRaw, UFileChannel};
use hp2_engine::net_bunch::{
    Hp2NetBunchServices,
    in_serialize_name, in_serialize_object, out_serialize_name,
    out_serialize_object,
};
use hp2_engine::net_chan::{
    Hp2NetChanServices, channel_close, channel_init, channel_is_known_type, channel_is_net_ready,
    channel_max_send_bytes, channel_received_acks, channel_received_nak, channel_received_raw,
    channel_send_bunch, channel_set_closing_flag, channel_tick, control_describe,
    control_received_bunch, control_serialize, file_describe, file_destroy, file_received_bunch,
    file_tick,
};

use hp2_core::array_storage::{self, Hp2ArrayServices};
use hp2_abi::engine::generated::FBitWriter;

unsafe extern "C" {
    fn realloc(data: *mut c_void, bytes: usize) -> *mut c_void;
    fn free(data: *mut c_void);
}
struct BunchEvents {
    freed: usize,
    destroyed: usize,
    fail_allocation: bool,
    fail_free: bool,
    relocate_source: *mut FBitWriter,
    resize_source: Option<i32>,
}
impl Default for BunchEvents {
    fn default() -> Self {
        Self { freed: 0, destroyed: 0, fail_allocation: false, fail_free: false, relocate_source: std::ptr::null_mut(), resize_source: None }
    }
}
thread_local! {
    static BUNCH_EVENTS: RefCell<BunchEvents> = RefCell::new(BunchEvents::default());
}
unsafe extern "C" fn array_realloc(_: *mut c_void, data: *mut c_void, bytes: usize, _: *const i32, _: i32, _: *const c_void, out: *mut *mut c_void) -> i32 {
    let (fail, source, resize) = BUNCH_EVENTS.with(|state| {
        let mut state = state.borrow_mut();
        let pair = (state.fail_allocation, state.relocate_source, state.resize_source.take());
        state.fail_allocation = false;
        state.relocate_source = std::ptr::null_mut();
        pair
    });
    if fail { return 1; }
    if !source.is_null() {
        unsafe {
            // Simulate allocator callback reentry replacing a source's live buffer.
            let count = resize.unwrap_or((*source).buffer.num);
            let replacement = realloc(std::ptr::null_mut(), count as usize);
            std::ptr::write_bytes(replacement.cast::<u8>(), 0x5a, count as usize);
            free((*source).buffer.data);
            (*source).buffer.data = replacement;
            if resize.is_some() {
                (*source).buffer.num = count;
                (*source).buffer.max = count;
                (*source).num = count * 8;
                (*source).max = count * 8;
            }
        }
    }
    // Match FMallocAnsi::Realloc: an empty array releases storage and returns null.
    unsafe {
        if bytes == 0 {
            free(data);
            out.write(std::ptr::null_mut());
        } else {
            out.write(realloc(data, bytes));
        }
    }
    0
}
unsafe extern "C" fn checked_size(_: *mut c_void, bytes: usize, out: *mut i32) -> i32 {
    match i32::try_from(bytes) {
        Ok(value) => { unsafe { out.write(value); } 0 }
        Err(_) => 1,
    }
}
unsafe extern "C" fn array_free(_: *mut c_void, data: *mut c_void) -> i32 {
    let fail = BUNCH_EVENTS.with(|state| {
        let mut state = state.borrow_mut();
        state.freed += 1;
        std::mem::take(&mut state.fail_free)
    });
    unsafe { free(data); }
    i32::from(fail)
}
fn publish_allocator() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        array_storage::publish_allocator(Hp2ArrayServices {
            context: std::ptr::null_mut(), realloc: array_realloc,
            checked_int_size: checked_size, free: array_free,
        });
    });
}

unsafe extern "C" fn out_construct(b: *mut FOutBunch) -> i32 {
    unsafe {
        std::ptr::write_bytes(b.cast::<u8>(), 0, std::mem::size_of::<FOutBunch>());
    }
    0
}
unsafe extern "C" fn out_destruct(b: *mut FOutBunch) -> i32 {
    let release = unsafe { hp2_engine::net_bunch::release_buffer(&raw mut (*b).base.buffer) };
    BUNCH_EVENTS.with(|state| state.borrow_mut().destroyed += 1);
    i32::from(release.is_err())
}

fn bunch_services() -> Hp2NetBunchServices {
    Hp2NetBunchServices {
        out_construct,
        out_destruct,
        in_construct,
        in_destruct,
        check_failed: chan_check_failed,
    }
}

unsafe extern "C" fn in_construct(b: *mut FInBunch) -> i32 {
    unsafe { std::ptr::write_bytes(b.cast::<u8>(), 0, std::mem::size_of::<FInBunch>()); }
    0
}
unsafe extern "C" fn in_destruct(_b: *mut FInBunch) -> i32 {
    BUNCH_EVENTS.with(|state| state.borrow_mut().destroyed += 1);
    0
}

unsafe fn string_units(value: *mut c_void) -> Vec<u32> {
    let view = unsafe { hp2_core::vm_string::view(value.cast()) };
    if view.num == 0 { Vec::new() } else {
        unsafe { read_tchar(view.data.cast()) }
    }
}
const FIXTURE: &str = include_str!("fixtures/net_chan.txt");
const EXPECTED_RECORDS: usize = 103;

/// `CHTYPE_Actor` from `UnChan.h`.
const CHTYPE_ACTOR: i32 = 2;
/// `RELIABLE_BUFFER` from `UnBunch.h`.
const RELIABLE_BUFFER: i32 = 128;

/// Emulated C++ `UChannel` subclass with the receive hook of the harness channel.
///
/// `UChannel` has no virtual-table field in the mirror, so the harness's `ReceivedBunch`
/// override is emulated by the `chan_virtual_received_bunch` mock below, which records
/// `ChSequence` values into `received`. This struct is `repr(C)` with `UChannel` first,
/// so a `*mut FixtureChannel` and its `*mut UChannel` are the same address.
#[repr(C)]
struct FixtureChannel {
    base: UChannel,
    received: [i32; 8],
    received_count: i32,
}

/// Mock state that the C++ harness keeps in globals, plus the leaf state the send path reads back.
struct Emulated {
    /// Bit count returned by `chan_conn_out_num_bits` (the harness `Out` writer is empty).
    out_bits: i32,
    /// Value returned by `chan_conn_is_net_ready` (the harness `IsNetReady` script).
    net_ready: i32,
    /// Scripted `Driver->Time - Out->Time` results, consumed in call order.
    time_since: VecDeque<f32>,
    /// Next packet id returned by `SendRawBunch` (the harness counter starts at 100).
    next_packet: i32,
    /// Storage behind `UNetConnection::LastOut`.
    last_out: Option<Box<FOutBunch>>,
    /// `UNetConnection::LastOutBunch` as set by the port.
    last_out_bunch: *mut FOutBunch,
    /// `UNetConnection::LastEnd` as set by the port.
    last_end: *mut c_void,
    /// Leaf records (`send_raw`, `net_ready`) in call order, drained into the fixture stream.
    trace: Vec<String>,
    /// Scripted `FInBunch << FString` values, consumed in call order (the harness `Serialize` script).
    in_script: VecDeque<Vec<u32>>,
    /// Set when the scripted reads run out: the bunch reports `IsError()`.
    in_error: bool,
    /// `GFileManager->FileSize` result (the harness `g_file_size`).
    file_size: i32,
    /// `NotifySendingFile` return value (the harness `g_notify_sending`).
    notify_ret: i32,
    /// Set when `CreateFileReader` should fail (the harness `g_reader_fails`).
    reader_fails: bool,
    /// Scripted `FGuid` reads, consumed in call order; `None` makes the bunch report an error.
    in_guid: VecDeque<Option<[u32; 4]>>,
    /// Payload behind `chan_in_bunch_num_bytes` / `chan_in_bunch_data`.
    in_data: Vec<u8>,
    /// Stand-in for `UNetConnection::OpenChannels`, used by `chan_tarray_remove_channel`.
    open_channels: Vec<usize>,
}

thread_local! {
    static EMULATED: RefCell<Emulated> = const {
        RefCell::new(Emulated {
            out_bits: 0,
            net_ready: 0,
            time_since: VecDeque::new(),
            next_packet: 100,
            last_out: None,
            last_out_bunch: std::ptr::null_mut(),
            last_end: std::ptr::null_mut(),
            trace: Vec::new(),
            in_script: VecDeque::new(),
            in_error: false,
            file_size: 0,
            notify_ret: 0,
            reader_fails: false,
            in_guid: VecDeque::new(),
            in_data: Vec::new(),
            open_channels: Vec::new(),
        })
    };
}

fn with_emulated<R>(f: impl FnOnce(&mut Emulated) -> R) -> R {
    EMULATED.with(|e| f(&mut e.borrow_mut()))
}

/// `UNetConnection::SendRawBunch` hook: records the bunch fields and returns the next packet id.
unsafe extern "C" fn chan_conn_send_raw_bunch(
    conn: *mut c_void,
    bunch: *mut FOutBunch,
    merge: i32,
    out: *mut i32,
) -> i32 {
    let (ch_index, seq, reliable, open, close) = unsafe {
        let b = &*bunch;
        (b.ch_index, b.ch_sequence, b.b_reliable, b.b_open, b.b_close)
    };
    let packet = with_emulated(|e| {
        e.trace.push(format!(
            "send_raw ch={ch_index} seq={seq} reliable={reliable} open={open} close={close} merge={merge}"
        ));
        let packet = e.next_packet;
        e.next_packet += 1;
        packet
    });
    unsafe {
        let conn = &mut *conn.cast::<UNetConnection>();
        conn.out_bun_acc = conn.out_bun_acc.wrapping_add(1);
        *out = packet;
    }
    0
}

/// `UNetConnection::IsNetReady` hook: records the saturate flag and returns the scripted value.
unsafe extern "C" fn chan_conn_is_net_ready(
    _conn: *mut c_void,
    saturate: i32,
    out: *mut i32,
) -> i32 {
    let ready = with_emulated(|e| {
        e.trace.push(format!("net_ready saturate={saturate}"));
        e.net_ready
    });
    unsafe { *out = ready };
    0
}

/// `UNetConnection::LastOut` storage: lazily allocates the zeroed bunch the port copies into.
unsafe extern "C" fn chan_conn_last_out(_conn: *mut c_void, out: *mut *mut FOutBunch) -> i32 {
    let p = with_emulated(|e| {
        let last = e
            .last_out
            .get_or_insert_with(|| Box::new(unsafe { zeroed() }));
        &mut **last as *mut FOutBunch
    });
    unsafe { *out = p };
    0
}

unsafe extern "C" fn chan_conn_last_end(_conn: *mut c_void, out: *mut *mut c_void) -> i32 {
    let end = with_emulated(|e| e.last_end);
    unsafe { *out = end };
    0
}

unsafe extern "C" fn chan_conn_last_out_bunch_set(
    _conn: *mut c_void,
    bunch: *mut FOutBunch,
) -> i32 {
    with_emulated(|e| e.last_out_bunch = bunch);
    0
}
unsafe extern "C" fn chan_conn_last_out_bunch_get(
    _conn: *mut c_void,
    out: *mut *mut FOutBunch,
) -> i32 {
    let bunch = with_emulated(|e| e.last_out_bunch);
    unsafe { *out = bunch };
    0
}

unsafe extern "C" fn chan_mark_pop_last_start(_conn: *mut c_void, _end: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn chan_out_bunch_num_bits(_bunch: *mut FOutBunch, out: *mut i32) -> i32 {
    unsafe { *out = with_emulated(|e| e.out_bits) };
    0
}

unsafe extern "C" fn chan_out_bunch_num_bytes(_bunch: *mut FOutBunch, out: *mut i32) -> i32 {
    unsafe { *out = (with_emulated(|e| e.out_bits) + 7) / 8 };
    0
}

static MERGE_DATA: u8 = 0;

unsafe extern "C" fn chan_out_bunch_data(_bunch: *mut FOutBunch, out: *mut *const u8) -> i32 {
    unsafe { *out = std::ptr::addr_of!(MERGE_DATA) };
    0
}

unsafe extern "C" fn chan_out_bunch_serialize_bits(
    _bunch: *mut FOutBunch,
    _data: *const u8,
    _bits: i32,
) -> i32 {
    0
}

/// `UNetConnection::OutBytes`-style leaf: derives the byte count from the scripted bit count.
unsafe extern "C" fn chan_conn_out_num_bytes(_conn: *mut c_void, out: *mut i32) -> i32 {
    let bits = with_emulated(|e| e.out_bits);
    unsafe { *out = (bits + 7) / 8 };
    0
}

unsafe extern "C" fn chan_mark_set_last_end(_conn: *mut c_void, end: *mut c_void) -> i32 {
    with_emulated(|e| e.last_end = end);
    0
}


/// `UChannel::SetClosingFlag` (the base body is `Closing = 1`).
unsafe extern "C" fn chan_virtual_set_closing_flag(ch: *mut c_void) -> i32 {
    unsafe { (*ch.cast::<UChannel>()).closing = 1 };
    0
}

/// `delete this` of a channel: the harness channel objects are heap `FixtureChannel`s.
unsafe extern "C" fn chan_channel_delete(ch: *mut c_void) -> i32 {
    drop(unsafe { Box::from_raw(ch.cast::<FixtureChannel>()) });
    0
}

/// `Driver->Time - Out->Time` as a float: the fixed-point FTime subtraction is a leaf, so the
/// test scripts its results in call order.
unsafe extern "C" fn chan_driver_time_since(
    _driver: *mut c_void,
    _time: *const u8,
    out: *mut f32,
) -> i32 {
    let v = with_emulated(|e| e.time_since.pop_front())
        .expect("net_chan fixture: unexpected driver time query");
    unsafe { *out = v };
    0
}

/// Abort-on-use for every service the exercised paths never reach.
///
/// Each field gets its own named function so the panic message names the field.
/// The transmute only changes the function pointer type; the body always panics.
macro_rules! unused {
    ($field:ident) => {{
        unsafe extern "C" fn unused_service() -> i32 {
            panic!(concat!(
                "net_chan fixture: unused service called: ",
                stringify!($field)
            ));
        }
        // SAFETY: fn pointer casts between `unsafe extern "C" fn` types with identical ABI;
        // the function never returns, so the declared signature is never observed.
        unsafe { std::mem::transmute::<unsafe extern "C" fn() -> i32, _>(unused_service) }
    }};
}

unsafe extern "C" fn chan_check_failed(expr: *const c_char, file: *const c_char, line: i32) -> i32 {
    let expr = unsafe { std::ffi::CStr::from_ptr(expr) }.to_string_lossy();
    let file = unsafe { std::ffi::CStr::from_ptr(file) }.to_string_lossy();
    panic!("net_chan fixture: check failed `{expr}` at {file}:{line}");
}

unsafe extern "C" fn chan_conn_out_num_bits(_conn: *mut c_void, out: *mut i32) -> i32 {
    unsafe { *out = with_emulated(|e| e.out_bits) };
    0
}

unsafe extern "C" fn chan_channel_class_at(ty: i32, out: *mut *mut c_void) -> i32 {
    // Mirrors the harness: CHTYPE_Control..=CHTYPE_File carry a non-null sentinel class; the rest stay null.
    static CLASS_SENTINEL: u8 = 0;
    let class: *mut c_void = if (1..=3).contains(&ty) {
        std::ptr::addr_of!(CLASS_SENTINEL).cast_mut().cast()
    } else {
        std::ptr::null_mut()
    };
    unsafe { *out = class };
    0
}


unsafe extern "C" fn chan_virtual_received_bunch(ch: *mut c_void, bunch: *mut FInBunch) -> i32 {
    let fc = ch.cast::<FixtureChannel>();
    let seq = unsafe { (*bunch).ch_sequence };
    unsafe {
        let n = (*fc).received_count;
        if (n as usize) < (*fc).received.len() {
            (*fc).received[n as usize] = seq;
            (*fc).received_count = n + 1;
        }
    }
    0
}


unsafe extern "C" fn chan_out_bunch_is_error(bunch: *mut FOutBunch, out: *mut i32) -> i32 {
    unsafe { *out = (*bunch).base.ar_is_error; }
    0
}

/// Package-map state for the bunch serialize records: the `UPackageMap` token the connection
/// points at, the two bunch addresses that identify the direction, and the emitted records.
#[derive(Default)]
struct PackageMapLog {
    pm_token: usize,
    in_bunch: usize,
    out_bunch: usize,
    records: Vec<String>,
}


unsafe extern "C" fn chan_string_set_tchar(s: *mut c_void, data: *const u32) -> i32 {
    match unsafe { hp2_engine::net_bunch::assign_string(s, data) } {
        Ok(()) => 0, Err(_) => 1,
    }
}

unsafe extern "C" fn chan_string_append(out: *mut c_void, base: *mut c_void) -> i32 {
    let mut text = unsafe { string_units(out) };
    text.extend(unsafe { string_units(base) });
    text.push(0);
    return unsafe { chan_string_set_tchar(out, text.as_ptr()) };
}

/// `FInBunch << FString`: the harness hook pops the next scripted value, or sets `ArIsError`.
unsafe extern "C" fn chan_in_bunch_serialize_string(
    _bunch: *mut FInBunch,
    text: *mut c_void,
) -> i32 {
    with_emulated(|e| match e.in_script.pop_front() {
        Some(value) => {
            e.trace
                .push(format!("string_ser dir=in value={}", wide_string(&value)));
            let mut value = value;
            value.push(0);
            assert_eq!(unsafe { chan_string_set_tchar(text, value.as_ptr()) }, 0);
        }
        None => {
            e.in_error = true;
            e.trace.push("string_ser dir=in error".to_string());
        }
    });
    0
}

unsafe extern "C" fn chan_in_bunch_is_error(_bunch: *mut FInBunch, out: *mut i32) -> i32 {
    unsafe { *out = i32::from(with_emulated(|e| e.in_error)) };
    0
}

unsafe extern "C" fn chan_out_bunch_serialize_string(
    _bunch: *mut FOutBunch,
    text: *mut c_void,
) -> i32 {
    let value = unsafe { string_units(text) };
    with_emulated(|e| {
        e.trace
            .push(format!("string_ser dir=out value={}", wide_string(&value)))
    });
    0
}

unsafe extern "C" fn chan_notify_received_text(_conn: *mut c_void, text: *mut c_void) -> i32 {
    let value = unsafe { string_units(text) };
    with_emulated(|e| e.trace.push(format!("notify_text {}", wide_string(&value))));
    0
}

thread_local! {
    static PACKAGE_MAP: RefCell<PackageMapLog> = RefCell::new(PackageMapLog::default());
}

/// Stand-in object the package map writes into an inbound object slot; never dereferenced.
static OBJECT_SENTINEL: u8 = 0;

/// Checks the package map token and maps the bunch address to the harness direction tag.
fn package_map_dir(pm: *mut c_void, bunch: *mut c_void) -> &'static str {
    PACKAGE_MAP.with(|p| {
        let p = p.borrow();
        assert_eq!(
            pm as usize, p.pm_token,
            "net_chan fixture: package map token"
        );
        if bunch as usize == p.in_bunch {
            "in"
        } else if bunch as usize == p.out_bunch {
            "out"
        } else {
            panic!("net_chan fixture: package map called with an unknown bunch");
        }
    })
}

unsafe extern "C" fn chan_pm_serialize_object(
    pm: *mut c_void,
    bunch: *mut c_void,
    slot: *mut *mut c_void,
) -> i32 {
    let dir = package_map_dir(pm, bunch);
    PACKAGE_MAP.with(|p| {
        p.borrow_mut()
            .records
            .push(format!("pm_serialize_object dir={dir}"))
    });
    if dir == "in" {
        // SAFETY: `slot` is the caller's object slot, valid for the write as in `FInBunch::operator<<`.
        unsafe { *slot = std::ptr::addr_of!(OBJECT_SENTINEL).cast_mut().cast() };
    }
    0
}

unsafe extern "C" fn chan_pm_serialize_name(
    pm: *mut c_void,
    bunch: *mut c_void,
    _name: *mut c_void,
) -> i32 {
    let dir = package_map_dir(pm, bunch);
    PACKAGE_MAP.with(|p| {
        p.borrow_mut()
            .records
            .push(format!("pm_serialize_name dir={dir}"))
    });
    0
}

fn slot_state(obj: *mut c_void) -> &'static str {
    if obj.is_null() { "null" } else { "set" }
}

/// Moves the package-map records emitted by the last serialize call into `lines`.
fn drain_package_map(lines: &mut Vec<String>) {
    PACKAGE_MAP.with(|p| lines.append(&mut p.borrow_mut().records));
}

/// Records the `FInBunch`/`FOutBunch` serialize cases in harness order: object then name, inbound
/// then outbound. Each call's package-map record is followed by the harness's slot line.
fn record_bunch_serialize(s: &Hp2NetChanServices) -> Vec<String> {
    let mut pm_token = 0u8;
    let mut conn = new_connection(7, 1200);
    conn.package_map = addr_of_mut!(pm_token).cast();
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    let mut in_bunch: FInBunch = unsafe { zeroed() };
    let mut out_bunch: FOutBunch = unsafe { zeroed() };
    unsafe {
        channel_init(ch, cp, 5, 1);
        (*cp).channels[5] = ch.cast();
        in_bunch.connection = cp.cast();
        construct_out_bunch(s, ch, 0, addr_of_mut!(out_bunch));
    }
    PACKAGE_MAP.with(|p| {
        *p.borrow_mut() = PackageMapLog {
            pm_token: addr_of_mut!(pm_token) as usize,
            in_bunch: addr_of_mut!(in_bunch) as usize,
            out_bunch: addr_of_mut!(out_bunch) as usize,
            records: Vec::new(),
        }
    });
    let mut name = 0u64;
    let name_ptr = addr_of_mut!(name).cast::<c_void>();
    let mut lines = Vec::with_capacity(6);
    let mut obj: *mut c_void = std::ptr::null_mut();
    unsafe {
        in_serialize_object(s, addr_of_mut!(in_bunch), addr_of_mut!(obj)).expect("in object");
    }
    drain_package_map(&mut lines);
    lines.push(format!("in_object slot={}", slot_state(obj)));
    unsafe { in_serialize_name(s, addr_of_mut!(in_bunch), name_ptr).expect("in name") };
    drain_package_map(&mut lines);
    obj = std::ptr::null_mut();
    unsafe {
        out_serialize_object(s, addr_of_mut!(out_bunch), addr_of_mut!(obj)).expect("out object");
    }
    drain_package_map(&mut lines);
    lines.push(format!("out_object slot={}", slot_state(obj)));
    unsafe { out_serialize_name(s, addr_of_mut!(out_bunch), name_ptr).expect("out name") };
    drain_package_map(&mut lines);
    unsafe { hp2_engine::net_bunch::release_buffer(&raw mut (*(&raw mut out_bunch).cast::<FBitWriter>()).buffer).unwrap(); }
    lines
}

/// Package-map stand-in: `FPackageMap` entries as `FPackageInfo` exposes them to the port.
struct MockPackage {
    url: Vec<u32>,
    // Borrowed canonical FString view; `url` retains the terminated backing.
    url_view: TArrayRaw,
    file_size: i32,
    guid: [u32; 4],
    has_linker: i32,
}

#[repr(C)]
struct MockPackageMap {
    base: UPackageMap,
    list: Vec<MockPackage>,
}

/// Scripted package list used by the file cases: entry 0 is the map, entry 1 has no URL.
fn reset_packages(pm: &mut MockPackageMap, size: i32) {
    pm.list = vec![
        MockPackage {
            url: wide_z("Maps/Demo.utx"),
            url_view: unsafe { zeroed() },
            file_size: size,
            guid: [1, 2, 3, 4],
            has_linker: 1,
        },
        MockPackage {
            url: Vec::new(),
            url_view: unsafe { zeroed() },
            file_size: 0,
            guid: [9, 9, 9, 9],
            has_linker: 0,
        },
    ];
    pm.base.list.num = pm.list.len() as i32;
}

/// Backs the download pointer with a zeroed `UDownload` mirror so the direct field reads in
/// `net_chan.rs` (`ChanDownloadTempFilename`/`ChanDownloadTransferred` leaves, now eliminated)
/// see real memory instead of a type-punned stand-in.
fn new_download(temp: &str, transferred: i32) -> *mut c_void {
    let mut dl: UDownload = unsafe { zeroed() };
    let wide = wide_z(temp);
    assert!(
        wide.len() <= dl.temp_filename.len(),
        "net_chan fixture: temp filename too long"
    );
    dl.temp_filename[..wide.len()].copy_from_slice(&wide);
    dl.transfered = transferred;
    Box::into_raw(Box::new(dl)).cast()
}

/// Reader stand-in for the `FArchive` returned by `CreateFileReader`; `pos` mirrors `g_reader_pos`.
struct MockReader {
    pos: u32,
}

unsafe extern "C" fn chan_tarray_remove_channel(
    _array: *mut TArrayRaw,
    ch: *mut c_void,
    out: *mut i32,
) -> i32 {
    let removed = with_emulated(|e| {
        let before = e.open_channels.len();
        e.open_channels.retain(|&c| c != ch as usize);
        before - e.open_channels.len()
    });
    unsafe { *out = removed as i32 };
    0
}

unsafe extern "C" fn chan_conn_flush_net(_conn: *mut c_void) -> i32 {
    with_emulated(|e| e.trace.push("flush_net".to_string()));
    0
}

unsafe extern "C" fn chan_download_receive_data(_dl: *mut c_void, data: *const u8, n: i32) -> i32 {
    let first = if n > 0 {
        i32::from(unsafe { *data })
    } else {
        -1
    };
    with_emulated(|e| {
        e.trace
            .push(format!("download_receive n={n} first={first}"))
    });
    0
}

unsafe extern "C" fn chan_download_done(_dl: *mut c_void) -> i32 {
    with_emulated(|e| e.trace.push("download_done".to_string()));
    0
}

unsafe extern "C" fn chan_download_delete(dl: *mut c_void) -> i32 {
    with_emulated(|e| e.trace.push("download_delete".to_string()));
    drop(unsafe { Box::from_raw(dl.cast::<UDownload>()) });
    0
}

unsafe extern "C" fn chan_file_create_reader(name: *const u32, out: *mut *mut c_void) -> i32 {
    let name = wide_string(&unsafe { read_tchar(name) });
    let ok = with_emulated(|e| {
        let ok = !e.reader_fails;
        let result = if ok { "set" } else { "null" };
        e.trace
            .push(format!("create_reader '{name}' result={result}"));
        ok
    });
    let reader = if ok {
        Box::into_raw(Box::new(MockReader { pos: 0 })).cast()
    } else {
        std::ptr::null_mut()
    };
    unsafe { *out = reader };
    0
}

unsafe extern "C" fn chan_file_archive_serialize(ar: *mut c_void, buf: *mut u8, n: i32) -> i32 {
    let reader = unsafe { &mut *ar.cast::<MockReader>() };
    let n = n as usize;
    let first = (reader.pos & 0xff) as i32;
    for i in 0..n {
        unsafe { *buf.add(i) = ((reader.pos as usize + i) & 0xff) as u8 };
    }
    with_emulated(|e| {
        e.trace
            .push(format!("reader_serialize n={n} first={first}"))
    });
    reader.pos += n as u32;
    0
}

unsafe extern "C" fn chan_file_archive_is_error(_ar: *mut c_void, out: *mut i32) -> i32 {
    unsafe { *out = 0 };
    0
}

unsafe extern "C" fn chan_file_archive_delete(ar: *mut c_void) -> i32 {
    with_emulated(|e| e.trace.push("reader_delete".to_string()));
    drop(unsafe { Box::from_raw(ar.cast::<MockReader>()) });
    0
}

unsafe extern "C" fn chan_package_info(
    map: *mut c_void,
    index: i32,
    size: *mut i32,
    url: *mut *mut c_void,
    guid: *mut c_void,
) -> i32 {
    let pm = unsafe { &mut *map.cast::<MockPackageMap>() };
    let pkg = &mut pm.list[index as usize];
    unsafe {
        *size = pkg.file_size;
        pkg.url_view = TArrayRaw {
            data: pkg.url.as_mut_ptr().cast(),
            num: pkg.url.len() as i32,
            max: pkg.url.capacity() as i32,
        };
        *url = (&raw mut pkg.url_view).cast();
        for (i, g) in pkg.guid.iter().enumerate() {
            guid.cast::<u32>().add(i).write_unaligned(*g);
        }
    }
    0
}

unsafe extern "C" fn chan_package_remove(map: *mut c_void, index: i32) -> i32 {
    let pm = unsafe { &mut *map.cast::<MockPackageMap>() };
    pm.list.remove(index as usize);
    pm.base.list.num = pm.list.len() as i32;
    0
}

unsafe extern "C" fn chan_package_file_info(
    map: *mut c_void,
    index: i32,
    size: *mut i32,
    unused: *mut i32,
) -> i32 {
    let pm = unsafe { &*map.cast::<MockPackageMap>() };
    unsafe {
        *size = pm.list[index as usize].file_size;
        *unused = 0;
    }
    0
}

unsafe extern "C" fn chan_package_has_linker(map: *mut c_void, index: i32, out: *mut i32) -> i32 {
    let pm = unsafe { &*map.cast::<MockPackageMap>() };
    unsafe { *out = pm.list[index as usize].has_linker };
    0
}

unsafe extern "C" fn chan_string_equals_tchar(
    s: *mut c_void,
    text: *const u32,
    out: *mut i32,
) -> i32 {
    let eq = unsafe { string_units(s) == read_tchar(text) };
    unsafe { *out = i32::from(eq) };
    0
}

unsafe extern "C" fn chan_file_size(url: *const u32, out: *mut i32) -> i32 {
    let name = wide_string(&unsafe { string_units(url.cast_mut().cast()) });
    let size = with_emulated(|e| {
        e.trace
            .push(format!("file_size '{name}' = {}", e.file_size));
        e.file_size
    });
    unsafe { *out = size };
    0
}

unsafe extern "C" fn chan_file_copy_src_filename(
    ch: *mut UFileChannel,
    url: *mut c_void,
    _index: i32,
) -> i32 {
    let text = unsafe { string_units(url) };
    let dst = unsafe { &mut (*ch).src_filename };
    dst.fill(0);
    let n = text.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&text[..n]);
    0
}

unsafe extern "C" fn chan_notify_sending_file(
    _conn: *mut c_void,
    guid: *mut c_void,
    out: *mut i32,
) -> i32 {
    let g = unsafe { *guid.cast::<[u32; 4]>() };
    let ret = with_emulated(|e| {
        e.trace.push(format!(
            "notify_sending_file guid={},{},{},{} ret={}",
            g[0], g[1], g[2], g[3], e.notify_ret
        ));
        e.notify_ret
    });
    unsafe { *out = ret };
    0
}

unsafe extern "C" fn chan_log_file_net_invalid() -> i32 {
    with_emulated(|e| e.trace.push("log NetInvalid".to_string()));
    0
}

unsafe extern "C" fn chan_log_file_skip(src: *const u32) -> i32 {
    let name = wide_string(&unsafe { read_tchar(src) });
    with_emulated(|e| {
        e.trace
            .push(format!("log User skipped download of '{name}'"))
    });
    0
}

unsafe extern "C" fn chan_log_file_net_send(src: *const u32) -> i32 {
    let name = wide_string(&unsafe { read_tchar(src) });
    with_emulated(|e| e.trace.push(format!("log NetSend {name}")));
    0
}

unsafe extern "C" fn chan_in_bunch_num_bytes(_bunch: *mut FInBunch, out: *mut i32) -> i32 {
    unsafe { *out = with_emulated(|e| e.in_data.len() as i32) };
    0
}

unsafe extern "C" fn chan_in_bunch_data(_bunch: *mut FInBunch, out: *mut *const u8) -> i32 {
    unsafe { *out = with_emulated(|e| e.in_data.as_ptr()) };
    0
}

/// `FInBunch << FGuid`: pops the next scripted guid; an absent or `None` entry sets `ArIsError`.
unsafe extern "C" fn chan_in_bunch_serialize_guid(_bunch: *mut FInBunch, out: *mut u8) -> i32 {
    let guid = with_emulated(|e| {
        let g = e.in_guid.pop_front().flatten();
        if g.is_none() {
            e.in_error = true;
        }
        g
    });
    if let Some(g) = guid {
        for (i, v) in g.iter().enumerate() {
            unsafe { out.cast::<u32>().add(i).write_unaligned(*v) };
        }
    }
    0
}

unsafe extern "C" fn chan_out_bunch_serialize_bytes(
    _bunch: *mut FOutBunch,
    _data: *mut u8,
    _n: i32,
) -> i32 {
    0
}

/// `UObject::GetFlags` on the harness's zero-initialised objects: a plain data read with no hook,
/// so it emits no record. The harness never sets object flags, so the value is 0.
unsafe extern "C" fn chan_object_get_flags(_obj: *mut c_void, out: *mut u32) -> i32 {
    unsafe { *out = 0 };
    0
}

/// `UObject::Destroy` is the harness's empty stub (`void UObject::Destroy() {}`): no record.
unsafe extern "C" fn chan_uobject_destroy(_ch: *mut c_void) -> i32 {
    0
}

/// Each abort stub's transmute target is inferred from its struct field type; the
/// identity transmutes are kept so every field uses the same `unused!` form.
#[allow(clippy::missing_transmute_annotations, clippy::useless_transmute)]
fn services() -> Hp2NetChanServices {
    publish_allocator();
    Hp2NetChanServices {
        chan_check_failed,
        chan_conn_out_num_bits,
        chan_conn_out_num_bytes,
        chan_conn_last_out,
        chan_conn_last_start: unused!(chan_conn_last_start),
        chan_conn_last_end,
        chan_conn_last_out_bunch_get,
        chan_conn_last_out_bunch_set,
        chan_conn_send_raw_bunch,
        chan_conn_is_net_ready,
        chan_tarray_remove_channel,
        bunches: bunch_services(),
        chan_mark_pop_last_start,
        chan_mark_set_last_end,
        chan_out_bunch_is_error,
        chan_out_bunch_num_bits,
        chan_out_bunch_num_bytes,
        chan_out_bunch_data,
        chan_out_bunch_serialize_bits,
        chan_pm_serialize_object,
        chan_pm_serialize_name,
        chan_virtual_received_bunch,
        chan_virtual_set_closing_flag,
        chan_channel_delete,
        chan_uobject_destroy,
        chan_object_get_flags,
        chan_object_clear_flags: unused!(chan_object_clear_flags),
        chan_object_set_flags: unused!(chan_object_set_flags),
        chan_conn_conditional_destroy: unused!(chan_conn_conditional_destroy),
        chan_driver_time_since,
        chan_driver_notify_get_level: unused!(chan_driver_notify_get_level),
        chan_driver_time_into: unused!(chan_driver_time_into),
        chan_driver_time_minus_spawn_priority: unused!(chan_driver_time_minus_spawn_priority),
        chan_channel_class_at,
        chan_in_bunch_serialize_string,
        chan_in_bunch_is_error,
        chan_notify_received_text,
        chan_out_bunch_serialize_string,
        chan_conn_actor_channels_remove: unused!(chan_conn_actor_channels_remove),
        chan_array_num: unused!(chan_array_num),
        chan_array_add: unused!(chan_array_add),
        chan_array_add_zeroed: unused!(chan_array_add_zeroed),
        chan_array_empty: unused!(chan_array_empty),
        chan_array_remove_item_i32: unused!(chan_array_remove_item_i32),
        chan_array_add_unique_i32: unused!(chan_array_add_unique_i32),
        chan_array_i32_at: unused!(chan_array_i32_at),
        chan_array_i32_set: unused!(chan_array_i32_set),
        chan_array_ptr_at: unused!(chan_array_ptr_at),
        chan_array_data: unused!(chan_array_data),
        chan_actor_get_class: unused!(chan_actor_get_class),
        chan_actor_flags: unused!(chan_actor_flags),
        chan_actor_get_roles: unused!(chan_actor_get_roles),
        chan_actor_pre_net_receive: unused!(chan_actor_pre_net_receive),
        chan_actor_post_net_receive: unused!(chan_actor_post_net_receive),
        chan_actor_post_net_begin_play: unused!(chan_actor_post_net_begin_play),
        chan_actor_should_script_replicate: unused!(chan_actor_should_script_replicate),
        chan_actor_optimized_rep_list: unused!(chan_actor_optimized_rep_list),
        chan_actor_process_event: unused!(chan_actor_process_event),
        chan_actor_level_num_reps_add: unused!(chan_actor_level_num_reps_add),
        chan_actor_is_player_pawn: unused!(chan_actor_is_player_pawn),
        chan_actor_handle_client_player: unused!(chan_actor_handle_client_player),
        chan_actor_channels_set: unused!(chan_actor_channels_set),
        chan_conn_remove_sent_temporary: unused!(chan_conn_remove_sent_temporary),
        chan_conn_add_sent_temporary: unused!(chan_conn_add_sent_temporary),
        chan_conn_flush_net,
        chan_driver_recording_demo: unused!(chan_driver_recording_demo),
        chan_map_class_cache: unused!(chan_map_class_cache),
        chan_cache_rep_condition_count: unused!(chan_cache_rep_condition_count),
        chan_cache_max_index: unused!(chan_cache_max_index),
        chan_cache_from_index: unused!(chan_cache_from_index),
        chan_cache_from_field: unused!(chan_cache_from_field),
        chan_cache_rep_property_at: unused!(chan_cache_rep_property_at),
        chan_field_kind: unused!(chan_field_kind),
        chan_field_name: unused!(chan_field_name),
        chan_object_init_properties: unused!(chan_object_init_properties),
        chan_class_rep_record: unused!(chan_class_rep_record),
        chan_rep_record_metadata: unused!(chan_rep_record_metadata),
        chan_property_metadata: unused!(chan_property_metadata),
        chan_property_replicated_metadata: unused!(chan_property_replicated_metadata),
        chan_property_net_serialize: unused!(chan_property_net_serialize),
        chan_property_copy_single: unused!(chan_property_copy_single),
        chan_property_destroy_value: unused!(chan_property_destroy_value),
        chan_property_exit_recent: unused!(chan_property_exit_recent),
        chan_property_identical: unused!(chan_property_identical),
        chan_property_is_object: unused!(chan_property_is_object),
        chan_property_name_kind: unused!(chan_property_name_kind),
        chan_property_is_bool: unused!(chan_property_is_bool),
        chan_property_owner_class: unused!(chan_property_owner_class),
        chan_function_metadata: unused!(chan_function_metadata),
        chan_function_find: unused!(chan_function_find),
        chan_function_super: unused!(chan_function_super),
        chan_function_param_first: unused!(chan_function_param_first),
        chan_function_param_next: unused!(chan_function_param_next),
        chan_frame_step_condition: unused!(chan_frame_step_condition),
        chan_bunch_read_int: unused!(chan_bunch_read_int),
        chan_bunch_read_bit: unused!(chan_bunch_read_bit),
        chan_bunch_is_error: unused!(chan_bunch_is_error),
        chan_bunch_serialize_object: unused!(chan_bunch_serialize_object),
        chan_bunch_serialize_vector: unused!(chan_bunch_serialize_vector),
        chan_bunch_write_int: unused!(chan_bunch_write_int),
        chan_bunch_serialize_byte: unused!(chan_bunch_serialize_byte),
        chan_bunch_write_byte: unused!(chan_bunch_write_byte),
        chan_bunch_mark_pop: unused!(chan_bunch_mark_pop),
        chan_retirement_get: unused!(chan_retirement_get),
        chan_retirement_in_packet: unused!(chan_retirement_in_packet),
        chan_retirement_set_in_packet: unused!(chan_retirement_set_in_packet),
        chan_retirement_out_packet: unused!(chan_retirement_out_packet),
        chan_retirement_set_out: unused!(chan_retirement_set_out),
        chan_retirement_reliable: unused!(chan_retirement_reliable),
        chan_retirement_set_reliable: unused!(chan_retirement_set_reliable),
        chan_object_to_index: unused!(chan_object_to_index),
        chan_can_serialize_object: unused!(chan_can_serialize_object),
        chan_level_destroy_actor: unused!(chan_level_destroy_actor),
        chan_actor_net_mode_client: unused!(chan_actor_net_mode_client),
        chan_download_receive_data,
        chan_download_done,
        chan_download_delete,
        chan_file_archive_serialize,
        chan_file_archive_is_error,
        chan_file_archive_delete,
        chan_package_info,
        chan_package_find_guid: unused!(chan_package_find_guid),
        chan_package_remove,
        chan_file_create_reader,
        chan_notify_sending_file,
        chan_string_set_tchar,
        chan_string_append,
        chan_actor_describe_format: unused!(chan_actor_describe_format),
        chan_object_is_valid: unused!(chan_object_is_valid),
        chan_object_is_actor: unused!(chan_object_is_actor),
        chan_object_is_class: unused!(chan_object_is_class),
        chan_class_is_actor_subclass: unused!(chan_class_is_actor_subclass),
        chan_actor_player: unused!(chan_actor_player),
        chan_mem_stack: unused!(chan_mem_stack),
        chan_mem_new_zeroed: unused!(chan_mem_new_zeroed),
        chan_mem_new_int: unused!(chan_mem_new_int),
        chan_log_invalid_new_channel_packet: unused!(chan_log_invalid_new_channel_packet),
        chan_log_invalid_actor_class: unused!(chan_log_invalid_actor_class),
        chan_log_actor_not_spawned: unused!(chan_log_actor_not_spawned),
        chan_recent_add: unused!(chan_recent_add),
        chan_retirement_empty: unused!(chan_retirement_empty),
        chan_retirement_add: unused!(chan_retirement_add),
        chan_mark_num_bits: unused!(chan_mark_num_bits),
        chan_log_received_unwanted_function: unused!(chan_log_received_unwanted_function),
        chan_log_control_overflow: unused!(chan_log_control_overflow),
        chan_log_file_net_invalid,
        chan_log_file_skip,
        chan_log_file_net_send,
        chan_in_bunch_num_bytes,
        chan_in_bunch_data,
        chan_in_bunch_serialize_guid,
        chan_string_equals_tchar,
        chan_package_file_info,
        chan_package_has_linker,
        chan_file_copy_src_filename,
        chan_out_bunch_serialize_bytes,
        chan_out_bunch_serialize_vector: unused!(chan_out_bunch_serialize_vector),
        chan_actor_level: unused!(chan_actor_level),
        chan_player_is_viewport: unused!(chan_player_is_viewport),
        chan_mem_zero: unused!(chan_mem_zero),
        chan_file_size,
    }
}

fn new_connection(negotiated_ver: i32, max_packet: i32) -> UNetConnection {
    // The harness `make_connection` sets only `NegotiatedVer` and `MaxPacket`; the rest is zero.
    let mut conn: UNetConnection = unsafe { zeroed() };
    conn.negotiated_ver = negotiated_ver;
    conn.max_packet = max_packet;
    conn
}

/// Mirrors the harness: `FOutBunch(Ch, bClose)` with a fresh `Out` (no bits written).
///
/// The C++ ctor initialises the members first, then runs the checks and the overflow test.
/// The Rust port performs the same work in `initialize_out`.
unsafe fn construct_out_bunch(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
    close: u8,
    bunch: *mut FOutBunch,
) {
    unsafe {
        hp2_engine::net_bunch::initialize_out(&s.bunches, bunch, ch, i32::from(close)).expect("FOutBunch constructor");
    }
}

/// `base_init index=3 local=1 version=7 open_packet=%d closing=%d maxsend=%d,%d,%d`.
fn record_base_init(s: &Hp2NetChanServices) -> String {
    let max_packets = [1200, 80, 1];
    let mut maxsend = [0i32; 3];
    let mut open_packet = 0;
    let mut closing = 0;
    for (slot, max_packet) in maxsend.iter_mut().zip(max_packets) {
        let mut conn = new_connection(7, max_packet);
        let cp = addr_of_mut!(conn);
        let mut fc: FixtureChannel = unsafe { zeroed() };
        let ch = addr_of_mut!(fc).cast::<UChannel>();
        unsafe {
            channel_init(ch, cp, 3, 1);
            *slot = channel_max_send_bytes(s, ch).expect("MaxSendBytes");
            open_packet = (*ch).open_packet_id;
            closing = (*ch).closing;
        }
    }
    format!(
        "base_init index=3 local=1 version=7 open_packet={open_packet} closing={closing} maxsend={},{},{}",
        maxsend[0], maxsend[1], maxsend[2]
    )
}

/// `closing before=0 after=%d`.
fn record_closing(_s: &Hp2NetChanServices) -> String {
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    let before = unsafe { (*ch).closing };
    unsafe { channel_set_closing_flag(ch) };
    let after = unsafe { (*ch).closing };
    format!("closing before={before} after={after}")
}

/// `known type=-1:%d,0:%d,1:%d,2:%d,3:%d,7:%d,8:%d`.
fn record_known(s: &Hp2NetChanServices) -> String {
    let probe = |ty: i32| unsafe { channel_is_known_type(s, ty) }.expect("IsKnownChannelType");
    format!(
        "known type=-1:{},0:{},1:{},2:{},3:{},7:{},8:{}",
        probe(-1),
        probe(0),
        probe(1),
        probe(2),
        probe(3),
        probe(7),
        probe(8)
    )
}

/// `out_ctor close=%d error=%d channel=%d type=%d open=%d close=%d reliable=%d`
/// (the harness prints the same `close` value twice).
fn record_out_ctor(s: &Hp2NetChanServices, close: u8) -> String {
    let mut conn = new_connection(1, 1200);
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    let mut bunch: FOutBunch = unsafe { zeroed() };
    let bp = addr_of_mut!(bunch);
    unsafe {
        channel_init(ch, cp, 4, 1);
        (*ch).ch_type = CHTYPE_ACTOR;
        (*cp).channels[4] = ch.cast();
        construct_out_bunch(s, ch, close, bp);
    }
    let mut error = 0;
    let status = unsafe { (s.chan_out_bunch_is_error)(bp, &mut error) };
    assert_eq!(status, 0, "chan_out_bunch_is_error status");
    unsafe { hp2_engine::net_bunch::release_buffer(&raw mut (*bp.cast::<FBitWriter>()).buffer).unwrap(); }
    format!(
        "out_ctor close={close} error={error} channel={} type={} open={} close={} reliable={}",
        bunch.ch_index, bunch.ch_type, bunch.b_open, bunch.b_close, bunch.b_reliable
    )
}

/// `out_ctor overflow close=%d count=127 error=%d` (close=0) and the close=1 variant.
///
/// `NumOutRec` is set to 127 before construction. The overflow test is
/// `NumOutRec >= RELIABLE_BUFFER - 1 + bClose`, so close=0 overflows (127 >= 127) and
/// close=1 does not (127 >= 128 is false). The harness prints the literal `count=127`.
fn record_out_ctor_overflow(s: &Hp2NetChanServices, close: u8) -> String {
    let mut conn = new_connection(1, 1200);
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    let mut bunch: FOutBunch = unsafe { zeroed() };
    let bp = addr_of_mut!(bunch);
    let count = RELIABLE_BUFFER - 1;
    unsafe {
        channel_init(ch, cp, 4, 1);
        (*ch).ch_type = CHTYPE_ACTOR;
        (*ch).num_out_rec = count;
        (*cp).channels[4] = ch.cast();
        construct_out_bunch(s, ch, close, bp);
    }
    let mut error = 0;
    let status = unsafe { (s.chan_out_bunch_is_error)(bp, &mut error) };
    assert_eq!(status, 0, "chan_out_bunch_is_error status");
    unsafe { hp2_engine::net_bunch::release_buffer(&raw mut (*bp.cast::<FBitWriter>()).buffer).unwrap(); }
    format!("out_ctor overflow close={close} count={count} error={error}")
}

/// Moves the leaf records traced by the mocks since the last flush into `lines`.
fn flush_trace(lines: &mut Vec<String>) {
    with_emulated(|e| lines.append(&mut e.trace));
}

/// Heap `FOutBunch` as the harness builds it with `new FOutBunch(&Ch, close)`. The caller frees it.
fn boxed_out_bunch(ch: *mut UChannel, close: u8) -> *mut FOutBunch {
    let mut bunch: Box<FOutBunch> = Box::new(unsafe { zeroed() });
    unsafe {
        bunch.channel = ch.cast();
        bunch.ch_index = (*ch).ch_index;
        bunch.ch_type = (*ch).ch_type;
        bunch.b_close = close;
    }
    Box::into_raw(bunch)
}

/// Harness `link_out`: appends a reliable heap bunch to `OutRec` and bumps `NumOutRec`.
unsafe fn link_out(
    ch: *mut UChannel,
    seq: i32,
    packet: i32,
    acked: i32,
    close: u8,
) -> *mut FOutBunch {
    let bunch = boxed_out_bunch(ch, close);
    unsafe {
        (*bunch).b_reliable = 1;
        (*bunch).ch_sequence = seq;
        (*bunch).packet_id = packet;
        (*bunch).received_ack = acked;
        let mut slot: *mut *mut c_void = addr_of_mut!((*ch).out_rec).cast();
        while !(*slot).is_null() {
            slot = addr_of_mut!((*(*slot).cast::<FOutBunch>()).next).cast();
        }
        *slot = bunch.cast();
        (*ch).num_out_rec += 1;
    }
    bunch
}

/// Frees the bunches still linked into `OutRec` (the harness leaks them).
unsafe fn free_out_rec(s: &Hp2NetChanServices, ch: *mut UChannel) {
    unsafe {
        let mut p = (*ch).out_rec.cast::<FOutBunch>();
        while !p.is_null() {
            let next = (*p).next.cast::<FOutBunch>();
            hp2_engine::net_bunch::delete_out(&s.bunches, p).expect("queued bunch cleanup");
            p = next;
        }
        (*ch).out_rec = std::ptr::null_mut();
        (*ch).num_out_rec = 0;
    }
}

/// `UChannel::Close`: the first close on an open channel queues the reliable close bunch, a repeat is a
/// no-op, and a channel on a closed connection never sends.
fn record_close(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    unsafe {
        channel_init(ch, cp, 2, 1);
        (*cp).channels[2] = ch.cast();
        channel_close(s, ch).expect("Close open");
        flush_trace(lines);
        lines.push(format!("close open closing={}", (*ch).closing));
        channel_close(s, ch).expect("Close repeat");
        lines.push(format!("close repeat closing={}", (*ch).closing));
        free_out_rec(s, ch);
    }
    let mut closed_conn = new_connection(1, 1200);
    closed_conn.state = 1;
    let closed_cp = addr_of_mut!(closed_conn);
    let mut cl: FixtureChannel = unsafe { zeroed() };
    let clp = addr_of_mut!(cl).cast::<UChannel>();
    unsafe {
        channel_init(clp, closed_cp, 3, 1);
        (*closed_cp).channels[3] = clp.cast();
        channel_close(s, clp).expect("Close closed-state");
        lines.push(format!("close closed-state closing={}", (*clp).closing));
    }
}

/// `UChannel::SendBunch`: a reliable open copies into `LastOut`, an unreliable open is a temporary open,
/// and a close bunch sent after the channel started closing marks it closing.
fn record_send(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    let cp = addr_of_mut!(conn);
    unsafe {
        let mut r: FixtureChannel = zeroed();
        let rp = addr_of_mut!(r).cast::<UChannel>();
        channel_init(rp, cp, 4, 1);
        (*cp).channels[4] = rp.cast();
        let b = boxed_out_bunch(rp, 0);
        (*b).b_reliable = 1;
        let packet = channel_send_bunch(s, rp, b, 0).expect("SendBunch reliable");
        drop(Box::from_raw(b));
        flush_trace(lines);
        lines.push(format!(
            "send reliable_open ret={packet} open_packet={} open_temp={} closing={}",
            (*rp).open_packet_id,
            (*rp).open_temporary,
            (*rp).closing
        ));
        free_out_rec(s, rp);

        let mut u: FixtureChannel = zeroed();
        let up = addr_of_mut!(u).cast::<UChannel>();
        channel_init(up, cp, 5, 1);
        (*cp).channels[5] = up.cast();
        let b = boxed_out_bunch(up, 0);
        let packet = channel_send_bunch(s, up, b, 0).expect("SendBunch unreliable");
        flush_trace(lines);
        lines.push(format!(
            "send unreliable_open ret={packet} open_packet={} open_temp={}",
            (*up).open_packet_id,
            (*up).open_temporary
        ));
        drop(Box::from_raw(b));

        let b = boxed_out_bunch(up, 1);
        let packet = channel_send_bunch(s, up, b, 0).expect("SendBunch close");
        flush_trace(lines);
        lines.push(format!(
            "send close_follow ret={packet} closing={}",
            (*up).closing
        ));
        drop(Box::from_raw(b));
    }
}

/// `UChannel::IsNetReady`: the connection answer is forwarded, and a full reliable buffer returns 0
/// without asking the connection.
fn record_is_net_ready(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    unsafe {
        channel_init(ch, cp, 2, 0);
        (*cp).channels[2] = ch.cast();
    }
    with_emulated(|e| e.net_ready = 1);
    let r = unsafe { channel_is_net_ready(s, ch, 1) }.expect("IsNetReady saturate");
    flush_trace(lines);
    lines.push(format!("is_net_ready saturate=1 result={r}"));
    with_emulated(|e| e.net_ready = 0);
    let r = unsafe { channel_is_net_ready(s, ch, 0) }.expect("IsNetReady no saturate");
    flush_trace(lines);
    lines.push(format!("is_net_ready saturate=0 result={r}"));
    unsafe { (*ch).num_out_rec = RELIABLE_BUFFER - 1 };
    let r = unsafe { channel_is_net_ready(s, ch, 1) }.expect("IsNetReady full");
    flush_trace(lines);
    lines.push(format!("is_net_ready full result={r}"));
}

/// `UChannel::ReceivedAcks` and `ReceivedNak` on an open channel with reliable bunches queued.
fn record_acks_nak(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    unsafe {
        channel_init(ch, cp, 2, 1);
        (*cp).channels[2] = ch.cast();
        link_out(ch, 1, 7, 1, 0);
        link_out(ch, 2, 7, 1, 0);
        link_out(ch, 3, 8, 0, 0);
        channel_received_acks(s, ch).expect("ReceivedAcks");
        let head = (*ch).out_rec.cast::<FOutBunch>();
        let (remaining, seq) = if head.is_null() {
            (0, -1)
        } else {
            (1, (*head).ch_sequence)
        };
        lines.push(format!(
            "acks remaining={remaining} seq={seq} count={}",
            (*ch).num_out_rec
        ));
        channel_received_nak(s, ch, 8).expect("ReceivedNak 8");
        flush_trace(lines);
        lines.push("nak packet=8 done".to_string());
        link_out(ch, 4, 9, 0, 0);
        channel_received_nak(s, ch, 9).expect("ReceivedNak 9");
        flush_trace(lines);
        lines.push("nak packet=9 done".to_string());
        free_out_rec(s, ch);
        (*cp).channels[2] = std::ptr::null_mut();
    }
}

/// `UChannel::ReceivedAcks` on an acknowledged close bunch: the channel deletes itself.
fn record_acks_close(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    let cp = addr_of_mut!(conn);
    unsafe {
        let hp: *mut FixtureChannel = Box::into_raw(Box::new(zeroed()));
        let hch = hp.cast::<UChannel>();
        channel_init(hch, cp, 3, 1);
        (*cp).channels[3] = hch.cast();
        link_out(hch, 1, 10, 1, 1);
        lines.push("acks close before".to_string());
        channel_received_acks(s, hch).expect("ReceivedAcks close");
        lines.push("acks close deleted".to_string());
        (*cp).channels[3] = std::ptr::null_mut();
    }
}

/// `UChannel::Tick` on channel 0: only an unacked bunch older than one second is resent.
fn record_tick(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fc).cast::<UChannel>();
    unsafe {
        channel_init(ch, cp, 0, 1);
        (*cp).channels[0] = ch.cast();
        let late = link_out(ch, 1, 11, 0, 0);
        with_emulated(|e| e.time_since.push_back(2.0));
        channel_tick(s, ch).expect("Tick stale");
        flush_trace(lines);
        lines.push("tick stale done".to_string());
        with_emulated(|e| e.time_since.push_back(0.5));
        channel_tick(s, ch).expect("Tick fresh");
        flush_trace(lines);
        lines.push("tick fresh done".to_string());
        (*late).received_ack = 1;
        channel_tick(s, ch).expect("Tick acked");
        flush_trace(lines);
        lines.push("tick acked done".to_string());
        free_out_rec(s, ch);
        (*cp).channels[0] = std::ptr::null_mut();
    }
}

fn join(values: &[i32]) -> String {
    values
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Stack `FInBunch` as the harness builds it: `ChIndex=2`, `ChType=0`, reliable, on `conn`.
fn reliable_bunch(conn: *mut UNetConnection, seq: i32) -> FInBunch {
    let mut bunch: FInBunch = unsafe { zeroed() };
    bunch.connection = conn.cast();
    bunch.ch_index = 2;
    bunch.ch_type = 0;
    bunch.ch_sequence = seq;
    bunch.b_reliable = 1;
    bunch
}

/// Walks `UChannel::InRec` and returns each queued bunch's `ChSequence` in list order.
unsafe fn queued_sequences(ch: *mut UChannel) -> Vec<i32> {
    let mut out = Vec::new();
    let mut p = unsafe { (*ch).in_rec }.cast::<FInBunch>();
    while !p.is_null() {
        out.push(unsafe { (*p).ch_sequence });
        p = unsafe { (*p).next }.cast::<FInBunch>();
    }
    out
}

/// Receives seq 3, 2, 3 (reliable, gap at 1), then seq 1 which releases the queue.
/// Produces the `receive_queue` and `receive_release` lines.
fn record_receive(s: &Hp2NetChanServices) -> (String, String) {
    let mut conn = new_connection(1, 1200);
    let cp = addr_of_mut!(conn);
    let mut fc: FixtureChannel = unsafe { zeroed() };
    let fcp = addr_of_mut!(fc);
    let ch = fcp.cast::<UChannel>();
    unsafe {
        channel_init(ch, cp, 2, 0);
        (*cp).channels[2] = ch.cast();
    }

    let mut b3 = reliable_bunch(cp, 3);
    let mut b2 = reliable_bunch(cp, 2);
    let mut b3_again = reliable_bunch(cp, 3);
    let mut b1 = reliable_bunch(cp, 1);

    unsafe {
        channel_received_raw(s, ch, addr_of_mut!(b3)).expect("ReceivedRawBunch seq 3");
        channel_received_raw(s, ch, addr_of_mut!(b2)).expect("ReceivedRawBunch seq 2");
        channel_received_raw(s, ch, addr_of_mut!(b3_again)).expect("ReceivedRawBunch seq 3 dup");
    }

    let (queued, queue_order, dispatched_before) = unsafe {
        (
            (*ch).num_in_rec,
            queued_sequences(ch),
            (*fcp).received_count,
        )
    };
    let receive_queue = format!(
        "receive_queue seq=3,2,3 reliable=1 queued={queued} order={} dispatched={dispatched_before}",
        join(&queue_order)
    );

    unsafe {
        channel_received_raw(s, ch, addr_of_mut!(b1)).expect("ReceivedRawBunch seq 1");
    }

    let (in_reliable, queued_after, dispatched_after, order) = unsafe {
        let n = (*fcp).received_count as usize;
        (
            (*cp).in_reliable[2],
            (*ch).num_in_rec,
            (*fcp).received_count,
            (&(*fcp).received)[..n].to_vec(),
        )
    };
    let receive_release = format!(
        "receive_release seq=1 reliable=1 in_reliable={in_reliable} queued={queued_after} dispatched={dispatched_after} order={}",
        join(&order)
    );

    (receive_queue, receive_release)
}

fn wide(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

fn wide_z(s: &str) -> Vec<u32> {
    let mut v = wide(s);
    v.push(0);
    v
}

fn wide_string(v: &[u32]) -> String {
    v.iter()
        .map(|&c| char::from_u32(c).unwrap_or('?'))
        .collect()
}

/// Reads a NUL-terminated `TCHAR` buffer, as the C++ `FString(const TCHAR*)` constructor does.
unsafe fn read_tchar(p: *const u32) -> Vec<u32> {
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

/// Harness `emit_control_cases`: `UControlChannel::ReceivedBunch` over the scripted pair, then
/// `Serialize` (a reliable control bunch), then `Describe`.
fn record_control(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    let cp = addr_of_mut!(conn);
    unsafe {
        let mut c: FixtureChannel = zeroed();
        let chp = addr_of_mut!(c).cast::<UChannel>();
        channel_init(chp, cp, 0, 1);
        (*cp).channels[0] = chp.cast();

        with_emulated(|e| {
            e.in_script = VecDeque::from([wide("alpha"), wide("beta")]);
            e.in_error = false;
        });
        let mut inbunch: FInBunch = zeroed();
        control_received_bunch(s, chp.cast(), addr_of_mut!(inbunch)).expect("ControlReceivedBunch");
        let strings = with_emulated(|e| {
            e.trace
                .iter()
                .filter(|l| l.starts_with("string_ser dir=in value="))
                .count()
        });
        flush_trace(lines);
        lines.push(format!("control received strings={strings}"));

        control_serialize(s, chp.cast(), wide_z("hi").as_ptr(), 0).expect("ControlSerialize");
        flush_trace(lines);
        lines.push("control serialize done".to_string());

        let mut out = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
        control_describe(s, chp.cast(), (&raw mut out).cast()).expect("ControlDescribe");
        flush_trace(lines);
        lines.push(format!("control describe={}", wide_string(&string_units((&raw mut out).cast()))));
        hp2_engine::net_bunch::release_buffer(&raw mut out).expect("description cleanup");
    }
}

/// Scripts the bunch reads for the next file-channel case and clears the bunch error flag.
fn script_bunch(in_script: Vec<Vec<u32>>, in_guid: Vec<Option<[u32; 4]>>, in_data: Vec<u8>) {
    with_emulated(|e| {
        e.in_script = in_script.into();
        e.in_guid = in_guid.into();
        e.in_data = in_data;
        e.in_error = false;
    });
}

/// Zeroed `UFileChannel` initialised on slot 2 of `conn`, as the harness `Init(C, 2, local)` does.
unsafe fn new_file_channel(conn: *mut UNetConnection, local: i32) -> Box<UFileChannel> {
    let mut ch: Box<UFileChannel> = Box::new(unsafe { zeroed() });
    let chp = (&mut *ch as *mut UFileChannel).cast::<UChannel>();
    unsafe {
        channel_init(chp, conn, 2, local);
        (*conn).channels[2] = chp.cast();
    }
    ch
}

/// Clears the connection's channel slot 2 after a file case.
unsafe fn unwire(conn: *mut UNetConnection) {
    unsafe { (*conn).channels[2] = std::ptr::null_mut() };
}

/// `FReader` stand-in: the harness `new FixtureReader()` emits nothing on construction.
fn new_reader() -> *mut c_void {
    Box::into_raw(Box::new(MockReader { pos: 0 })).cast()
}

/// Frees a reader the oracle leaks (no destroy record exists for it).
unsafe fn drop_reader_silently(ch: *mut UFileChannel) {
    unsafe {
        let ar = std::mem::replace(&mut (*ch).send_file_ar, std::ptr::null_mut());
        if !ar.is_null() {
            drop(Box::from_raw(ar.cast::<MockReader>()));
        }
    }
}

/// Frees a download the oracle leaks (no destroy record exists for it).
unsafe fn drop_download_silently(ch: *mut UFileChannel) {
    unsafe {
        let dl = std::mem::replace(&mut (*ch).download, std::ptr::null_mut());
        if !dl.is_null() {
            drop(Box::from_raw(dl.cast::<UDownload>()));
        }
    }
}

/// Sets `UFileChannel::SrcFilename` the way the harness assigns it.
unsafe fn set_src(ch: *mut UFileChannel, text: &str) {
    let v = wide(text);
    unsafe {
        let dst = &mut (*ch).src_filename;
        dst.fill(0);
        dst[..v.len()].copy_from_slice(&v);
    }
}

/// `FileReceivedBunch` with a scratch bunch; flushes the leaf records it emitted.
unsafe fn receive_case(s: &Hp2NetChanServices, ch: *mut UFileChannel, lines: &mut Vec<String>) {
    let mut bunch: FInBunch = unsafe { zeroed() };
    unsafe { file_received_bunch(s, ch, addr_of_mut!(bunch)) }.expect("FileReceivedBunch");
    flush_trace(lines);
}

/// Receive-as-request: records the request line with the post-call state.
unsafe fn request_case(
    s: &Hp2NetChanServices,
    ch: *mut UFileChannel,
    name: &str,
    lines: &mut Vec<String>,
) {
    unsafe {
        receive_case(s, ch, lines);
        let sendfile = i32::from(!(*ch).send_file_ar.is_null());
        let src = wide_string(&read_tchar((*ch).src_filename.as_ptr()));
        lines.push(format!(
            "file request {name} sendfile={sendfile} index={} src={src}",
            (*ch).package_index
        ));
    }
}

/// `FileTick` with the given lan_play; records the tick line with the post-call state.
unsafe fn tick_case(
    s: &Hp2NetChanServices,
    ch: *mut UFileChannel,
    name: &str,
    lines: &mut Vec<String>,
) {
    unsafe {
        file_tick(s, ch, 0).expect("FileTick");
        flush_trace(lines);
        let sent = (*ch).sent_data;
        let sendfile = i32::from(!(*ch).send_file_ar.is_null());
        lines.push(format!("file tick {name} sent={sent} sendfile={sendfile}"));
    }
}

/// `FileDescribe` into a fresh string; flushes the leaf records it emitted.
unsafe fn describe_case(
    s: &Hp2NetChanServices,
    ch: *mut UFileChannel,
    lines: &mut Vec<String>,
) -> String {
    let mut out = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    unsafe {
        file_describe(s, ch, (&raw mut out).cast()).expect("FileDescribe");
        flush_trace(lines);
        let text = wide_string(&string_units((&raw mut out).cast()));
        hp2_engine::net_bunch::release_buffer(&raw mut out).expect("description cleanup");
        text
    }
}

/// Records the file-channel cases in harness order (`emit_file_cases`, chan_fixture.cpp).
fn record_file(s: &Hp2NetChanServices, lines: &mut Vec<String>) {
    let pm: *mut MockPackageMap =
        Box::into_raw(Box::new(MockPackageMap { base: unsafe { zeroed() }, list: Vec::new() }));
    let dp: *mut UNetDriver = Box::into_raw(Box::new(unsafe { zeroed() }));
    unsafe { (*dp).allow_downloads = 1 };
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    conn.driver = dp.cast();
    conn.package_map = pm.cast();
    let cp = addr_of_mut!(conn);
    unsafe {
        // local_receive: the bunch payload goes straight to the download.
        script_bunch(Vec::new(), Vec::new(), vec![7, 8, 9]);
        let mut ch = new_file_channel(cp, 1);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).download = new_download("", 0);
        receive_case(s, chp, lines);
        lines.push("file local_receive done".to_string());
        drop_download_silently(chp);
        unwire(cp);

        // refuse_disabled: downloads disabled, so the request is refused with NetInvalid.
        (*dp).allow_downloads = 0;
        script_bunch(Vec::new(), Vec::new(), Vec::new());
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        receive_case(s, chp, lines);
        lines.push("file refuse_disabled done".to_string());
        (*dp).allow_downloads = 1;
        unwire(cp);

        // skip: the SKIP command removes the package entry.
        reset_packages(&mut *pm, 10);
        script_bunch(vec![wide("SKIP")], Vec::new(), Vec::new());
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).send_file_ar = new_reader();
        set_src(chp, "Maps/Demo.utx");
        receive_case(s, chp, lines);
        lines.push(format!("file skip packages={}", (*pm).list.len()));
        drop_reader_silently(chp);
        unwire(cp);

        // accepted: the guid matches entry 0, so the file is offered and a reader opened.
        reset_packages(&mut *pm, 10);
        script_bunch(Vec::new(), vec![Some([1, 2, 3, 4])], Vec::new());
        with_emulated(|e| e.notify_ret = 1);
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        request_case(s, chp, "accepted", lines);
        drop_reader_silently(chp);
        unwire(cp);

        // too_large: the package exceeds MaxDownloadSize, so no offer is made.
        (*dp).max_download_size = 5;
        with_emulated(|e| {
            e.file_size = 10;
        });
        script_bunch(Vec::new(), vec![Some([1, 2, 3, 4])], Vec::new());
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        request_case(s, chp, "too_large", lines);
        (*dp).max_download_size = 0;
        unwire(cp);

        // notify_refused: the notify hook refuses the file.
        with_emulated(|e| e.notify_ret = 0);
        script_bunch(Vec::new(), vec![Some([1, 2, 3, 4])], Vec::new());
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        request_case(s, chp, "notify_refused", lines);
        with_emulated(|e| e.notify_ret = 1);
        unwire(cp);

        // reader_failed: notify accepts but the reader cannot be created.
        with_emulated(|e| e.reader_fails = true);
        script_bunch(Vec::new(), vec![Some([1, 2, 3, 4])], Vec::new());
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        request_case(s, chp, "reader_failed", lines);
        with_emulated(|e| e.reader_fails = false);
        unwire(cp);

        // guid_error: the guid bunch read fails.
        script_bunch(Vec::new(), vec![None], Vec::new());
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        request_case(s, chp, "guid_error", lines);
        unwire(cp);

        // tick single: one 10-byte chunk, then the reader closes.
        reset_packages(&mut *pm, 10);
        with_emulated(|e| e.net_ready = 1);
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).send_file_ar = new_reader();
        set_src(chp, "Maps/Demo.utx");
        tick_case(s, chp, "single", lines);
        unwire(cp);

        // tick multi: 2000 bytes in two chunks.
        reset_packages(&mut *pm, 2000);
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).send_file_ar = new_reader();
        set_src(chp, "Maps/Demo.utx");
        tick_case(s, chp, "multi", lines);
        with_emulated(|e| e.net_ready = 0);
        unwire(cp);

        // destroy_send: the owned reader is deleted; no download exists.
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).send_file_ar = new_reader();
        with_emulated(|e| e.open_channels.push(chp as usize));
        file_destroy(s, chp).expect("FileDestroy");
        flush_trace(lines);
        lines.push("file destroy_send done".to_string());
        unwire(cp);

        // destroy_received: the download is finished and deleted.
        let mut ch = new_file_channel(cp, 1);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).download = new_download("", 0);
        with_emulated(|e| e.open_channels.push(chp as usize));
        file_destroy(s, chp).expect("FileDestroy");
        flush_trace(lines);
        lines.push("file destroy_received done".to_string());
        unwire(cp);

        // describe_sent: sent-side progress with the package size.
        reset_packages(&mut *pm, 10);
        let mut ch = new_file_channel(cp, 0);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).sent_data = 512;
        set_src(chp, "Maps/Demo.utx");
        let text = describe_case(s, chp, lines);
        lines.push(format!("file describe_sent={text}"));
        unwire(cp);

        // describe_received: the download supplies the temp filename and progress.
        let mut ch = new_file_channel(cp, 1);
        let chp: *mut UFileChannel = &mut *ch;
        (*chp).download = new_download("Temp.tmp", 77);
        let text = describe_case(s, chp, lines);
        lines.push(format!("file describe_received={text}"));
        drop_download_silently(chp);
        unwire(cp);

        // describe_received_nodl: no download object at all.
        let mut ch = new_file_channel(cp, 1);
        let chp: *mut UFileChannel = &mut *ch;
        let text = describe_case(s, chp, lines);
        lines.push(format!("file describe_received_nodl={text}"));
        unwire(cp);
    }
    drop(unsafe { Box::from_raw(pm) });
}

/// Rebuilds the 103 fixture records in harness order.
fn rebuild(s: &Hp2NetChanServices) -> Vec<String> {
    let mut lines = Vec::with_capacity(EXPECTED_RECORDS);
    lines.push(record_base_init(s));
    lines.push(record_closing(s));
    lines.push(record_known(s));
    lines.push(record_out_ctor(s, 0));
    lines.push(record_out_ctor(s, 1));
    lines.push(record_out_ctor_overflow(s, 0));
    lines.push(record_out_ctor_overflow(s, 1));
    lines.extend(record_bunch_serialize(s));
    let (receive_queue, receive_release) = record_receive(s);
    lines.push(receive_queue);
    lines.push(receive_release);
    record_close(s, &mut lines);
    record_send(s, &mut lines);
    record_is_net_ready(s, &mut lines);
    record_acks_nak(s, &mut lines);
    record_acks_close(s, &mut lines);
    record_tick(s, &mut lines);
    record_control(s, &mut lines);
    record_file(s, &mut lines);
    lines
}

#[test]
fn net_chan_fixture_matches_cpp() {
    let s = services();
    let lines: Vec<&str> = FIXTURE.lines().collect();
    assert_eq!(
        lines.len(),
        EXPECTED_RECORDS,
        "fixture record count (fixtures/net_chan.txt)"
    );

    let rebuilt = rebuild(&s);
    assert_eq!(rebuilt.len(), EXPECTED_RECORDS, "rebuilt record count");

    for (index, (want, got)) in lines.iter().zip(rebuilt.iter()).enumerate() {
        assert_eq!(got.as_str(), *want, "fixture record {index} mismatch");
    }
}

#[test]
fn merged_send_preserves_output_bunch_accounting() {
    let s = services();
    let mut conn = new_connection(1, 1200);
    conn.state = 3;
    conn.allow_merge = 1;
    let cp = addr_of_mut!(conn);
    let mut fixture_ch: FixtureChannel = unsafe { zeroed() };
    let ch = addr_of_mut!(fixture_ch).cast::<UChannel>();
    unsafe {
        channel_init(ch, cp, 4, 1);
        (*cp).channels[4] = ch.cast();
        with_emulated(|e| {
            e.out_bits = 8;
            e.open_channels.push(ch as usize);
        });

        let first = boxed_out_bunch(ch, 0);
        (*first).b_reliable = 1;
        channel_send_bunch(&s, ch, first, 0).expect("first SendBunch");
        assert_eq!((*cp).out_bun_acc, 1);
        drop(Box::from_raw(first));

        let merged = boxed_out_bunch(ch, 0);
        (*merged).b_reliable = 1;

        channel_send_bunch(&s, ch, merged, 1).expect("merged SendBunch");
        assert_eq!((*cp).out_bun_acc, 1);
        drop(Box::from_raw(merged));
        hp2_engine::net_chan::channel_destroy(&s, ch).expect("retire reliable send history");
    }
}

#[test]
fn bunch_copies_own_independent_buffers() {
    let s = services();
    let mut conn = new_connection(1, 1200);
    let mut ch: UChannel = unsafe { zeroed() };
    unsafe {
        let cp = &raw mut conn;
        let channel = &raw mut ch;
        channel_init(channel, cp, 3, 1);
        (*cp).channels[3] = channel.cast();
        hp2_engine::net_bunch::with_out(&s.bunches, channel, 1, |source| {
            (*source).base.num = 13;
            (*source).base.ar_is_error = 1;
            (*source).b_reliable = 1;
            (*source).ch_sequence = 19;
            *(*source).base.buffer.data.cast::<u8>() = 0x5a;
            let copied = hp2_engine::net_bunch::copy_out(&s.bunches, source)?;
            let writer_data = (*source).base.buffer.data.cast::<u8>();
            let copied_data = (*copied).base.buffer.data.cast::<u8>();
            assert_ne!(writer_data, copied_data);
            assert_eq!(*copied_data, 0x5a);
            assert_eq!((*copied).base.num, 13);
            assert_eq!((*copied).base.ar_is_error, 1);
            assert_eq!((*copied).ch_sequence, 19);
            assert_eq!((*copied).b_close, 1);
            *writer_data = 0x11;
            assert_eq!(*copied_data, 0x5a);
            hp2_engine::net_bunch::assign_out(copied, copied)?;
            hp2_engine::net_bunch::delete_out(&s.bunches, copied)?;
            Ok(())
        }).expect("outgoing ownership");
        let mut incoming: FInBunch = zeroed();
        let source = &raw mut incoming;
        hp2_engine::net_bunch::initialize_in(source, cp);
        hp2_engine::net_bunch::initialize_reader(source.cast(), [0x31, 0x02].as_ptr(), 10).unwrap();
        (*source).base.pos = 3;
        (*source).base.ar_is_error = 1;
        (*source).packet_id = 23;
        (*source).ch_sequence = 17;
        let copied = hp2_engine::net_bunch::copy_in(&s.bunches, source).unwrap();
        let reader_data = (*source).base.buffer.data.cast::<u8>();
        let copied_data = (*copied).base.buffer.data.cast::<u8>();
        assert_ne!(reader_data, copied_data);
        *reader_data = 0x44;
        assert_eq!(*copied_data, 0x31);
        assert_eq!((*copied).base.num, 10);
        assert_eq!((*copied).base.pos, 3);
        assert_eq!((*copied).base.ar_is_error, 1);
        assert_eq!((*copied).packet_id, 23);
        assert_eq!((*copied).ch_sequence, 17);
        hp2_engine::net_bunch::delete_in(&s.bunches, copied).unwrap();
        hp2_engine::net_bunch::release_buffer(&raw mut (*source).base.buffer).unwrap();
    }
}

#[test]
fn bunch_scope_cleans_callback_failure_and_partial_buffer_construction() {
    let s = services();
    let mut conn = new_connection(1, 1200);
    let mut ch: UChannel = unsafe { zeroed() };
    unsafe {
        let cp = &raw mut conn;
        let channel = &raw mut ch;
        channel_init(channel, cp, 2, 1);
        (*cp).channels[2] = channel.cast();
        let failed: Result<(), _> = hp2_engine::net_bunch::with_out(&s.bunches, channel, 1, |bunch| {
            assert_eq!((*bunch).b_close, 1);
            assert_eq!((*bunch).base.max, hp2_engine::net_bunch::out_bunch_capacity((*cp).max_packet));
            Err(hp2_engine::net_chan::Thrown)
        });
        assert!(failed.is_err());
        let completed = hp2_engine::net_bunch::with_out(&s.bunches, channel, 0, |bunch| {
            assert_eq!((*bunch).b_close, 0);
            Ok(())
        });
        assert!(completed.is_ok());
    }
}

#[test]
fn bunch_assignment_preserves_writer_buffer_with_core_allocator() {
    publish_allocator();
    unsafe {
        let mut source: FOutBunch = zeroed();
        let mut destination: FOutBunch = zeroed();
        let src = &raw mut source;
        let dst = &raw mut destination;
        hp2_engine::net_bunch::initialize_writer(src.cast(), 24).unwrap();
        (*src).base.num = 24;
        std::ptr::write_bytes((*src).base.buffer.data.cast::<u8>(), 0x5a, 3);
        hp2_engine::net_bunch::assign_out(dst, src).unwrap();
        assert_eq!((*dst).base.buffer.num, 3);
        assert!((*dst).base.buffer.max >= 3);
        assert_eq!((*dst).base.num, 24);
        assert_eq!((*dst).base.max, 24);
        let destination_data = (*dst).base.buffer.data.cast::<u8>();
        let source_data = (*src).base.buffer.data.cast::<u8>();
        assert_ne!(destination_data, source_data);
        for index in 0..3 {
            assert_eq!(*destination_data.add(index), 0x5a);
        }
        hp2_engine::net_bunch::release_buffer(&raw mut (*dst).base.buffer).unwrap();
        hp2_engine::net_bunch::release_buffer(&raw mut (*src).base.buffer).unwrap();
    }
}
