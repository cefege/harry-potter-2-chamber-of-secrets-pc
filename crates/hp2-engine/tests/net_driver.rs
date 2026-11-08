//! Replays the `driver_fixture.cpp` oracle cases through the Rust `UNetDriver`
//! port in `hp2_engine::net_driver`, comparing the emitted trace against the
//! pinned `fixtures/net_driver.txt` records.
//!
//! Real `UNetDriver`, `UNetConnection`, `UChannel`, and `UPackageMapLevel`
//! memory is used for the fields the port reads directly. Every other object is
//! a registry entry keyed by address. Mock thunks emit the same record formats
//! as the C++ harness.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::zeroed;
use std::ptr::null_mut;

use hp2_abi::engine::generated::{
    AActor, FArchive, TArrayRaw, UActorChannel, UArrayProperty, UChannel, UNetConnection,
    UNetDriver, UObject, UPackageMapLevel,
};
use hp2_engine::net_driver::{self, Thrown};
use hp2_engine::net_driver_abi::Hp2NetDriverServices;

const FIXTURE: &str = include_str!("fixtures/net_driver.txt");
const EXPECTED_RECORDS: usize = 190;

const USOCK_CLOSED: i32 = 1;
const USOCK_PENDING: i32 = 2;
const USOCK_OPEN: i32 = 3;
const CHTYPE_CONTROL: i32 = 1;
const CHTYPE_ACTOR: i32 = 2;

#[derive(Clone)]
struct Obj {
    ptr: usize,
    name: String,
    class: String,
    full: String,
}

#[derive(Default)]
struct Emulated {
    trace: Vec<String>,
    objects: Vec<Obj>,
    /// `(connection, actor, channel)` triples mirroring `ActorChannels`.
    actor_channels: Vec<(usize, usize, usize)>,
    loading: bool,
    is_error: bool,
    script: VecDeque<u32>,
    max_object_index: i32,
    cmdline: String,
    driver_class: usize,
    archive_ptr: usize,
    /// Owns every test-allocated object so pointers stay valid until reset.
    owners: Vec<Box<dyn std::any::Any>>,
    /// Owns `TArray` backing storage.
    arrays: Vec<Vec<*mut c_void>>,
    /// Owns the NUL-terminated wide command line handed to the port.
    cmd_wide: Vec<u32>,
}

thread_local! {
    static EMU: RefCell<Emulated> = RefCell::new(Emulated::default());
}

fn with<R>(f: impl FnOnce(&mut Emulated) -> R) -> R {
    EMU.with(|e| f(&mut e.borrow_mut()))
}

fn rec(line: String) {
    with(|e| e.trace.push(line));
}

fn flush(lines: &mut Vec<String>) {
    with(|e| lines.append(&mut e.trace));
}

fn obj<R>(ptr: *mut c_void, f: impl FnOnce(&Obj) -> R) -> R {
    let addr = ptr as usize;
    with(|e| {
        let o = e
            .objects
            .iter()
            .rev()
            .find(|o| o.ptr == addr)
            .unwrap_or_else(|| panic!("net_driver fixture: unregistered object {addr:#x}"));
        f(o)
    })
}

fn name_of(ptr: *mut c_void) -> String {
    if ptr.is_null() {
        "NULL".to_string()
    } else {
        obj(ptr, |o| o.name.clone())
    }
}

fn class_of(ptr: *mut c_void) -> String {
    obj(ptr, |o| o.class.clone())
}

fn full_of(ptr: *mut c_void) -> String {
    obj(ptr, |o| o.full.clone())
}

fn find_latest(name: &str, class: Option<&str>) -> Option<usize> {
    with(|e| {
        e.objects
            .iter()
            .rev()
            .find(|o| o.name == name && class.is_none_or(|c| o.class == c))
            .map(|o| o.ptr)
    })
}

fn register(ptr: usize, name: &str, class: &str) {
    let full = if class == "UClass" {
        format!("Engine.Class {name}")
    } else {
        format!("Engine.Other {name}")
    };
    with(|e| {
        e.objects.push(Obj {
            ptr,
            name: name.to_string(),
            class: class.to_string(),
            full,
        })
    });
}

fn own<T: 'static>(value: T) -> *mut T {
    let p = Box::into_raw(Box::new(value));
    with(|e| e.owners.push(unsafe { Box::from_raw(p) }));
    p
}

fn new_obj(name: &str, class: &str) -> *mut c_void {
    let p = own(0u64).cast::<c_void>();
    register(p as usize, name, class);
    p
}

fn new_actor(name: &str, b_static: bool, b_no_delete: bool, b_net_temporary: bool) -> *mut c_void {
    let p = alloc_struct::<AActor>(name, "AActor");
    unsafe {
        (*p).set_b_static(b_static);
        (*p).set_b_no_delete(b_no_delete);
        (*p).set_b_net_temporary(b_net_temporary);
    }
    p.cast::<c_void>()
}

fn alloc_struct<T: 'static>(name: &str, class: &str) -> *mut T {
    let p = own(unsafe { zeroed::<T>() });
    register(p as usize, name, class);
    p
}

fn new_driver() -> *mut UNetDriver {
    let driver = alloc_struct::<UNetDriver>("driver", "UNetDriver");
    let class = with(|e| e.driver_class);
    unsafe { (*driver.cast::<UObject>()).class = class as *mut c_void };
    driver
}

fn new_conn(name: &str, driver: *mut UNetDriver, state: i32) -> *mut UNetConnection {
    let c = alloc_struct::<UNetConnection>(name, "UNetConnection");
    unsafe {
        (*c).driver = driver.cast();
        (*c).state = state;
    }
    c
}

fn new_map(name: &str, conn: *mut UNetConnection) -> *mut UPackageMapLevel {
    let m = alloc_struct::<UPackageMapLevel>(name, "UPackageMap");
    unsafe {
        (*m).connection = conn.cast();
    }
    m
}

struct ChannelSetup {
    ch_index: i32,
    ch_type: i32,
    closing: i32,
    open_acked: i32,
    opened_locally: i32,
}

impl ChannelSetup {
    const fn new(
        ch_index: i32,
        ch_type: i32,
        closing: i32,
        open_acked: i32,
        opened_locally: i32,
    ) -> Self {
        Self {
            ch_index,
            ch_type,
            closing,
            open_acked,
            opened_locally,
        }
    }
}

fn new_channel(
    name: &str,
    conn: *mut UNetConnection,
    setup: ChannelSetup,
    actor: *mut c_void,
) -> *mut UChannel {
    let full = alloc_struct::<UActorChannel>(name, "UChannel");
    let ch = full.cast::<UChannel>();
    unsafe {
        (*ch).connection = conn.cast();
        (*ch).ch_index = setup.ch_index;
        (*ch).ch_type = setup.ch_type;
        (*ch).closing = setup.closing;
        (*ch).open_acked = setup.open_acked;
        (*ch).opened_locally = setup.opened_locally;
        (*conn).channels[setup.ch_index as usize] = ch.cast();
        (*full).actor = actor;
    }
    with(|e| {
        e.actor_channels
            .push((conn as usize, actor as usize, ch as usize))
    });
    ch
}

fn tarray(items: &[*mut c_void]) -> TArrayRaw {
    let mut storage = items.to_vec();
    let len = storage.len() as i32;
    let data = storage.as_mut_ptr().cast::<c_void>();
    with(|e| e.arrays.push(storage));
    TArrayRaw {
        data,
        num: len,
        max: len,
    }
}

unsafe fn tarray_items(arr: TArrayRaw) -> Vec<*mut c_void> {
    (0..arr.num as usize)
        .map(|i| unsafe { *arr.data.cast::<*mut c_void>().add(i) })
        .collect()
}

unsafe fn tarray_remove(arr: *mut TArrayRaw, item: *mut c_void) {
    unsafe {
        let n = (*arr).num as usize;
        let data = (*arr).data.cast::<*mut c_void>();
        if let Some(pos) = (0..n).find(|&i| *data.add(i) == item) {
            for i in pos..n - 1 {
                *data.add(i) = *data.add(i + 1);
            }
            (*arr).num -= 1;
        }
    }
}

unsafe fn wstr(p: *const u32) -> String {
    let mut s = String::new();
    let mut i = 0;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            break;
        }
        s.push(char::from_u32(c).expect("wide char"));
        i += 1;
    }
    s
}

fn wide_z(text: &str) -> Vec<u32> {
    text.chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0))
        .collect()
}

fn archive() -> *mut c_void {
    with(|e| e.archive_ptr) as *mut c_void
}

fn output() -> *mut c_void {
    static OUTPUT: u8 = 0;
    (&OUTPUT as *const u8).cast_mut().cast()
}

fn reset_world() {
    with(|e| *e = Emulated::default());
    let driver_class = new_obj("UNetDriver", "UClass") as usize;
    let ar = alloc_struct::<FArchive>("archive", "FArchive") as usize;
    with(|e| {
        e.driver_class = driver_class;
        e.archive_ptr = ar;
    });
}

fn set_ar(loading: bool, is_error: bool, script: &[u32], max_object_index: i32) {
    with(|e| {
        e.loading = loading;
        e.is_error = is_error;
        e.script = script.iter().copied().collect();
        e.max_object_index = max_object_index;
    });
    unsafe {
        let ar = archive().cast::<FArchive>();
        (*ar).ar_is_loading = loading as i32;
        (*ar).ar_is_error = is_error as i32;
    }
}

fn set_cmdline(cmdline: &str) {
    with(|e| {
        e.cmdline = cmdline.to_string();
        e.cmd_wide = wide_z(cmdline);
    });
}

fn begin(lines: &mut Vec<String>, name: &str) {
    flush(lines);
    reset_world();
    lines.push(format!("case {name}"));
}

fn finish_result(lines: &mut Vec<String>, value: i32) {
    flush(lines);
    lines.push(format!("result={value}"));
}

fn ok<T>(r: Result<T, Thrown>) -> T {
    r.expect("port threw")
}

// ---- Archive mocks -------------------------------------------------------

unsafe extern "C" fn drv_ar_serialize_bits(_ar: *mut c_void, b: *mut u8, bits: i32) -> i32 {
    let loading = with(|e| e.loading);
    let value = unsafe {
        if loading {
            *b = with(|e| e.script.pop_front().unwrap_or(0)) as u8;
        }
        *b
    };
    rec(format!(
        "serialize_bits bits={bits} value={value} loading={}",
        loading as i32
    ));
    0
}

unsafe extern "C" fn drv_ar_serialize_int(_ar: *mut c_void, v: *mut u32, max: i32) -> i32 {
    let loading = with(|e| e.loading);
    let value = unsafe {
        if loading {
            *v = with(|e| e.script.pop_front().unwrap_or(0));
        }
        *v
    };
    rec(format!(
        "serialize_int max={max} value={value} loading={}",
        loading as i32
    ));
    0
}

unsafe extern "C" fn drv_super_serialize(driver: *mut c_void, _ar: *mut c_void) -> i32 {
    rec(format!("object_serialize {}", name_of(driver)));
    0
}

unsafe extern "C" fn drv_serialize_references(_ar: *mut c_void, driver: *mut c_void) -> i32 {
    let loading = with(|e| e.loading) as i32;
    let d = driver.cast::<UNetDriver>();
    let (clients, server, master, role, remote) = unsafe {
        (
            tarray_items((*d).client_connections),
            (*d).server_connection,
            (*d).master_map,
            (*d).role_property,
            (*d).remote_role_property,
        )
    };
    rec(format!(
        "serialize_bytes len=1 first={} loading={loading}",
        clients.len()
    ));
    for c in clients {
        rec(format!("serialize_object obj={}", name_of(c)));
    }
    rec(format!("serialize_object obj={}", name_of(server)));
    rec(format!("serialize_object obj={}", name_of(master)));
    rec(format!("serialize_object obj={}", name_of(role)));
    rec(format!("serialize_object obj={}", name_of(remote)));
    0
}

// ---- Object mocks --------------------------------------------------------

unsafe extern "C" fn drv_max_object_index(_map: *mut c_void) -> i32 {
    with(|e| e.max_object_index)
}

unsafe extern "C" fn drv_index_to_object(
    _map: *mut c_void,
    index: i32,
    load: i32,
    out: *mut *mut c_void,
) -> i32 {
    let want = format!("idx{index}");
    let ptr = find_latest(&want, None).unwrap_or(0) as *mut c_void;
    rec(format!(
        "index_to_object index={index} load={load} -> {}",
        name_of(ptr)
    ));
    unsafe { *out = ptr };
    0
}

unsafe extern "C" fn drv_object_to_index(
    _map: *mut c_void,
    object: *mut c_void,
    out: *mut i32,
) -> i32 {
    let name = name_of(object);
    let index = name
        .strip_prefix("idx")
        .and_then(|n| n.parse::<i32>().ok())
        .unwrap_or(-1);
    rec(format!("object_to_index obj={name} -> {index}"));
    unsafe { *out = index };
    0
}

unsafe extern "C" fn drv_object_is_a(object: *mut c_void, class: *mut c_void) -> i32 {
    (class_of(object) == name_of(class)) as i32
}

unsafe extern "C" fn drv_log_forged_object(object: *mut c_void, class: *mut c_void) -> i32 {
    let (oname, ofull) = (name_of(object), full_of(object));
    let (cname, cfull) = (name_of(class), full_of(class));
    rec(format!("full_name obj={oname} -> {ofull}"));
    rec(format!("full_name obj={cname} -> {cfull}"));
    rec(format!(
        "logf 'Forged object: got {ofull}, expecting {cfull}'"
    ));
    0
}

// ---- Actor / channel mocks -----------------------------------------------

unsafe extern "C" fn drv_cast_actor(object: *mut c_void) -> *mut c_void {
    if object.is_null() || class_of(object) != "AActor" {
        null_mut()
    } else {
        object
    }
}

unsafe extern "C" fn drv_conn_find_actor_channel(
    conn: *mut c_void,
    actor: *mut c_void,
) -> *mut c_void {
    with(|e| {
        e.actor_channels
            .iter()
            .rev()
            .find(|t| t.0 == conn as usize && t.1 == actor as usize)
            .map(|t| t.2)
    })
    .unwrap_or(0) as *mut c_void
}

unsafe extern "C" fn drv_channel_close_checked(channel: *mut c_void) -> i32 {
    let opened = unsafe { (*channel.cast::<UChannel>()).opened_locally };
    if opened == 0 {
        panic!("net_driver fixture: channel_close on a channel not opened locally");
    }
    rec(format!("channel_close ch={}", name_of(channel)));
    0
}

unsafe extern "C" fn drv_conn_remove_sent_temporary(
    _conn: *mut c_void,
    _actor: *mut c_void,
) -> i32 {
    0
}

// ---- Connection / package-map mocks -------------------------------------

unsafe extern "C" fn drv_conn_tick(conn: *mut c_void) -> i32 {
    rec(format!("connection_tick conn={}", name_of(conn)));
    0
}

unsafe extern "C" fn drv_delete_connection(conn: *mut c_void) -> i32 {
    let n = name_of(conn);
    rec(format!("destroy {n}"));
    rec(format!("connection_destroy conn={n}"));
    unsafe {
        let driver = (*conn.cast::<UNetConnection>()).driver.cast::<UNetDriver>();
        if (*driver).server_connection == conn {
            (*driver).server_connection = null_mut();
        } else {
            tarray_remove(&mut (*driver).client_connections, conn);
        }
    }
    rec(format!("object_dtor {n}"));
    0
}

unsafe extern "C" fn drv_delete_package_map(map: *mut c_void) -> i32 {
    let n = name_of(map);
    rec(format!("destroy {n}"));
    rec(format!("package_map_destroy {n}"));
    rec(format!("object_dtor {n}"));
    0
}

unsafe extern "C" fn drv_new_package_map(out: *mut *mut c_void) -> i32 {
    rec("static_allocate class=UPackageMap outer=Transient name=None flags=0".to_string());
    let p = new_obj("None", "UPackageMap");
    unsafe { *out = p };
    0
}

// ---- Driver / property / command mocks ----------------------------------

unsafe extern "C" fn drv_find_actor_property(name: *const u32, out: *mut *mut c_void) -> i32 {
    let n = unsafe { wstr(name) };
    let p = find_latest(&n, Some("UProperty"))
        .unwrap_or_else(|| panic!("net_driver fixture: property {n} not found"))
        as *mut c_void;
    rec(format!(
        "find_object_checked class=UProperty name={n} -> {}",
        name_of(p)
    ));
    unsafe { *out = p };
    0
}

unsafe extern "C" fn drv_app_cmdline() -> *const u32 {
    let cmd = with(|e| e.cmdline.clone());
    rec(format!("app_cmdline -> '{cmd}'"));
    with(|e| e.cmd_wide.as_ptr())
}

unsafe extern "C" fn drv_parse_param(stream: *const u32, param: *const u32) -> i32 {
    let s = unsafe { wstr(stream) };
    let k = unsafe { wstr(param) };
    let r = s.contains(&k) as i32;
    rec(format!("parse_param key={k} -> {r}"));
    r
}

unsafe extern "C" fn drv_parse_command(stream: *mut *const u32, command: *const u32) -> i32 {
    let cur = unsafe { *stream };
    let s = unsafe { wstr(cur) };
    let want = unsafe { wstr(command) };
    let prefix_ok = s
        .get(..want.len())
        .is_some_and(|h| h.eq_ignore_ascii_case(&want));
    let boundary_ok = s.len() == want.len() || s.as_bytes().get(want.len()) == Some(&b' ');
    let matched = prefix_ok && boundary_ok;
    if matched {
        unsafe {
            let mut p = cur.add(want.len());
            if *p == b' ' as u32 {
                p = p.add(1);
            }
            *stream = p;
        }
    }
    rec(format!("parse_command match={want} -> {}", matched as i32));
    matched as i32
}

// ---- Static property mocks ----------------------------------------------

unsafe fn static_prop(
    class: &str,
    outer: *mut c_void,
    name: *const u32,
    flags: u32,
    out: *mut *mut c_void,
) -> i32 {
    let n = unsafe { wstr(name) };
    rec(format!(
        "static_allocate class={class} outer={} name={n} flags={flags}",
        name_of(outer)
    ));
    let p = if class == "UArrayProperty" {
        alloc_struct::<UArrayProperty>(&n, class).cast::<c_void>()
    } else {
        new_obj(&n, class)
    };
    unsafe { *out = p };
    0
}

unsafe extern "C" fn drv_static_float(
    outer: *mut c_void,
    name: *const u32,
    flags: u32,
    _offset: i32,
    _category: *const u32,
    _cpf: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe { static_prop("UFloatProperty", outer, name, flags, out) }
}

unsafe extern "C" fn drv_static_int(
    outer: *mut c_void,
    name: *const u32,
    flags: u32,
    _offset: i32,
    _category: *const u32,
    _cpf: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe { static_prop("UIntProperty", outer, name, flags, out) }
}

unsafe extern "C" fn drv_static_bool(
    outer: *mut c_void,
    name: *const u32,
    flags: u32,
    _offset: i32,
    _category: *const u32,
    _cpf: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe { static_prop("UBoolProperty", outer, name, flags, out) }
}

unsafe extern "C" fn drv_static_array(
    outer: *mut c_void,
    name: *const u32,
    flags: u32,
    _offset: i32,
    _category: *const u32,
    _cpf: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe { static_prop("UArrayProperty", outer, name, flags, out) }
}

unsafe extern "C" fn drv_static_str(
    outer: *mut c_void,
    name: *const u32,
    flags: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe { static_prop("UStrProperty", outer, name, flags, out) }
}

// ---- Teardown mocks ------------------------------------------------------

unsafe extern "C" fn drv_low_level_destroy(driver: *mut c_void) -> i32 {
    rec(format!("low_level_destroy driver={}", name_of(driver)));
    0
}

unsafe extern "C" fn drv_super_destroy(driver: *mut c_void) -> i32 {
    rec(format!("super_destroy {}", name_of(driver)));
    0
}

// ---- Exec / logging mocks ------------------------------------------------

unsafe extern "C" fn drv_exec_logf(_out: *mut c_void, text: *const u32) -> i32 {
    let t = unsafe { wstr(text) };
    rec(format!("logf '{t}'"));
    0
}

unsafe extern "C" fn drv_exec_log_server(_out: *mut c_void, conn: *mut c_void) -> i32 {
    rec(format!(
        "connection_low_level_describe conn={}",
        name_of(conn)
    ));
    rec("logf '   Server conn'".to_string());
    0
}

unsafe extern "C" fn drv_exec_log_client(_out: *mut c_void, conn: *mut c_void) -> i32 {
    rec(format!(
        "connection_low_level_describe conn={}",
        name_of(conn)
    ));
    rec("logf '   Client conn'".to_string());
    0
}

unsafe extern "C" fn drv_exec_log_channel(_out: *mut c_void, channel: *mut c_void) -> i32 {
    rec(format!("channel_describe ch={}", name_of(channel)));
    let idx = unsafe { (*channel.cast::<UChannel>()).ch_index };
    rec(format!("logf '      Channel {idx}: chan'"));
    0
}

fn services() -> Hp2NetDriverServices {
    Hp2NetDriverServices {
        drv_ar_serialize_bits,
        drv_ar_serialize_int,
        drv_super_serialize,
        drv_serialize_references,
        drv_max_object_index,
        drv_index_to_object,
        drv_object_to_index,
        drv_object_is_a,
        drv_log_forged_object,
        drv_cast_actor,
        drv_conn_find_actor_channel,
        drv_channel_close_checked,
        drv_conn_remove_sent_temporary,
        drv_conn_tick,
        drv_delete_connection,
        drv_delete_package_map,
        drv_new_package_map,
        drv_find_actor_property,
        drv_app_cmdline,
        drv_parse_param,
        drv_parse_command,
        drv_static_float,
        drv_static_int,
        drv_static_bool,
        drv_static_array,
        drv_static_str,
        drv_low_level_destroy,
        drv_super_destroy,
        drv_exec_logf,
        drv_exec_log_server,
        drv_exec_log_client,
        drv_exec_log_channel,
    }
}

// ---- Cases ---------------------------------------------------------------

type ConnSetup = fn(*mut UNetConnection) -> *mut c_void;
type LoadSetup = fn(*mut UNetConnection);

fn case_can_serialize(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    let scenarios: [(&str, ConnSetup); 6] = [
        ("null", |_| null_mut()),
        ("nonactor", |_| new_obj("plain_obj", "UObject")),
        ("static", |_| new_actor("actor_static", true, false, false)),
        ("nodelete", |_| {
            new_actor("actor_nodelete", false, true, false)
        }),
        ("channel", |c| {
            let a = new_actor("actor_ch", false, false, false);
            new_channel("ch", c, ChannelSetup::new(2, CHTYPE_ACTOR, 0, 1, 1), a);
            a
        }),
        ("no_channel", |_| {
            new_actor("actor_free", false, false, false)
        }),
    ];
    for (name, setup) in scenarios {
        begin(lines, &format!("can_serialize_{name}"));
        let conn = new_conn("conn", null_mut(), USOCK_OPEN);
        let object = setup(conn);
        let map = new_map("map", conn);
        let value = unsafe { net_driver::can_serialize_object(s, map.cast(), object) };
        finish_result(lines, value as i32);
    }
}

fn load_case(
    s: &Hp2NetDriverServices,
    lines: &mut Vec<String>,
    name: &str,
    script: &[u32],
    is_error: bool,
    max_object_index: i32,
    setup: LoadSetup,
) {
    begin(lines, &format!("load_{name}"));
    let conn = new_conn("conn", null_mut(), USOCK_OPEN);
    setup(conn);
    let map = new_map("map", conn);
    set_ar(true, is_error, script, max_object_index);
    let class = new_obj("AActor", "UClass");
    let mut object: *mut c_void = null_mut();
    let result =
        ok(unsafe { net_driver::serialize_object(s, map.cast(), archive(), class, &mut object) });
    flush(lines);
    lines.push(format!(
        "result={} object={}",
        result as i32,
        name_of(object)
    ));
}

fn case_serialize_load(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    load_case(s, lines, "static_good", &[0, 5], false, 100, |_| {
        new_actor("idx5", false, false, false);
    });
    load_case(s, lines, "static_forged", &[0, 6], false, 100, |_| {
        new_obj("idx6", "OtherClass");
    });
    load_case(s, lines, "static_error", &[0, 5], true, 100, |_| {});
    load_case(s, lines, "static_missing", &[0, 99], false, 100, |_| {});
    load_case(s, lines, "dynamic_none", &[1, 0], false, 0, |_| {});
    load_case(s, lines, "dynamic_actor", &[1, 2], false, 0, |c| {
        let a = new_actor("actor_ch2", false, false, false);
        new_channel("ch2", c, ChannelSetup::new(2, CHTYPE_ACTOR, 0, 1, 1), a);
    });
    load_case(s, lines, "dynamic_closing", &[1, 3], false, 0, |c| {
        let a = new_actor("actor_ch3", false, false, false);
        new_channel("ch3", c, ChannelSetup::new(3, CHTYPE_ACTOR, 1, 1, 1), a);
    });
    load_case(s, lines, "dynamic_nonactor", &[1, 4], false, 0, |c| {
        new_channel(
            "ch4",
            c,
            ChannelSetup::new(4, CHTYPE_CONTROL, 0, 1, 1),
            null_mut(),
        );
    });
    load_case(s, lines, "dynamic_empty", &[1, 5], false, 0, |_| {});
    load_case(s, lines, "dynamic_range", &[1, 1023], false, 0, |_| {});
    load_case(
        s,
        lines,
        "dynamic_wrapped",
        &[1, 0xFFFF_FFFF],
        false,
        0,
        |_| {},
    );
    load_case(s, lines, "dynamic_error", &[1, 2], true, 0, |c| {
        let a = new_actor("actor_ch2", false, false, false);
        new_channel("ch2", c, ChannelSetup::new(2, CHTYPE_ACTOR, 0, 1, 1), a);
    });
}

fn save_case(
    s: &Hp2NetDriverServices,
    lines: &mut Vec<String>,
    name: &str,
    max_object_index: i32,
    setup: ConnSetup,
) {
    begin(lines, &format!("save_{name}"));
    let conn = new_conn("conn", null_mut(), USOCK_OPEN);
    let mut object = setup(conn);
    let map = new_map("map", conn);
    set_ar(false, false, &[], max_object_index);
    let class = new_obj("AActor", "UClass");
    let result =
        ok(unsafe { net_driver::serialize_object(s, map.cast(), archive(), class, &mut object) });
    finish_result(lines, result as i32);
}

fn case_serialize_save(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    save_case(s, lines, "dynamic_open", 0, |c| {
        let a = new_actor("actor_open", false, false, false);
        new_channel("ch_open", c, ChannelSetup::new(7, CHTYPE_ACTOR, 0, 1, 1), a);
        a
    });
    save_case(s, lines, "dynamic_pending", 0, |c| {
        let a = new_actor("actor_pending", false, false, false);
        new_channel(
            "ch_pending",
            c,
            ChannelSetup::new(8, CHTYPE_ACTOR, 0, 0, 1),
            a,
        );
        a
    });
    save_case(s, lines, "dynamic_unmapped", 0, |_| {
        new_actor("actor_unmapped", false, false, false)
    });
    save_case(s, lines, "static_actor_nonindexed", 0, |_| {
        new_actor("actor_static_save", true, false, false)
    });
    save_case(s, lines, "null", 0, |_| null_mut());
    save_case(s, lines, "nonactor_nonindexed", 0, |_| {
        new_obj("plain_obj", "UObject")
    });
    save_case(s, lines, "nonactor_indexed", 100, |_| {
        new_obj("idx9", "UObject")
    });
}

fn case_driver_ctor(s: &Hp2NetDriverServices, lines: &mut Vec<String>, name: &str, cmdline: &str) {
    begin(lines, name);
    set_cmdline(cmdline);
    let driver = new_driver();
    new_obj("Role", "UProperty");
    new_obj("RemoteRole", "UProperty");
    ok(unsafe { net_driver::construct(s, driver) });
    flush(lines);
    let line = unsafe {
        format!(
            "role={} remote={} master={} profile={}",
            name_of((*driver).role_property),
            name_of((*driver).remote_role_property),
            if (*driver).master_map.is_null() {
                "NULL"
            } else {
                "set"
            },
            (*driver).profile_stats
        )
    };
    lines.push(line);
}

fn case_static_ctor(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    begin(lines, "static_ctor");
    let driver = new_driver();
    ok(unsafe { net_driver::static_constructor(s, driver) });
    flush(lines);
    let rate = unsafe { (*driver).max_client_rate };
    lines.push(format!("max_client_rate={rate}"));
}

fn case_tick_flush(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    let cases: [(&str, bool, &[&str]); 4] = [
        ("flush_none", false, &[]),
        ("flush_server_only", true, &[]),
        ("flush_clients", false, &["cli0", "cli1"]),
        ("flush_both", true, &["cli0", "cli1"]),
    ];
    for (name, server, clients) in cases {
        begin(lines, name);
        let driver = new_driver();
        if server {
            let srv = new_conn("srv", driver, USOCK_OPEN);
            unsafe { (*driver).server_connection = srv.cast() };
        }
        let list: Vec<*mut c_void> = clients
            .iter()
            .map(|n| new_conn(n, driver, USOCK_OPEN).cast())
            .collect();
        unsafe { (*driver).client_connections = tarray(&list) };
        ok(unsafe { net_driver::tick_flush(s, driver) });
        flush(lines);
    }
}

fn case_init(lines: &mut Vec<String>, name: &str) {
    begin(lines, name);
    let driver = new_driver();
    let notify = new_obj("notify", "UObject");
    let accepted = unsafe { net_driver::init_notify(driver, notify) };
    let notify_set = unsafe { (*driver).notify == notify };
    flush(lines);
    lines.push(format!(
        "result={} notify_set={}",
        accepted as i32, notify_set as i32
    ));
}

fn case_tick_dispatch(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    begin(lines, "dispatch_server_keeps_closed");
    let driver = new_driver();
    let srv = new_conn("srv", driver, USOCK_OPEN);
    let cli = new_conn("cli_closed0", driver, USOCK_CLOSED);
    unsafe {
        (*driver).server_connection = srv.cast();
        (*driver).client_connections = tarray(&[cli.cast()]);
        (*driver).send_cycles = 7;
        (*driver).recv_cycles = 9;
    }
    ok(unsafe { net_driver::tick_dispatch(s, driver, 0.5) });
    flush(lines);
    let line = unsafe {
        format!(
            "send={} recv={} clients={}",
            (*driver).send_cycles,
            (*driver).recv_cycles,
            (*driver).client_connections.num
        )
    };
    lines.push(line);

    begin(lines, "dispatch_delete_closed");
    let driver = new_driver();
    let clients = [
        new_conn("cli0", driver, USOCK_OPEN),
        new_conn("cli_closed1", driver, USOCK_CLOSED),
        new_conn("cli_closed2", driver, USOCK_CLOSED),
        new_conn("cli_pending3", driver, USOCK_PENDING),
    ];
    let list: Vec<*mut c_void> = clients.iter().map(|c| c.cast()).collect();
    unsafe { (*driver).client_connections = tarray(&list) };
    ok(unsafe { net_driver::tick_dispatch(s, driver, 0.5) });
    flush(lines);
    let remaining = unsafe { (*driver).client_connections.num };
    lines.push(format!("clients={remaining}"));
}

fn case_serialize(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    begin(lines, "driver_serialize");
    let driver = new_driver();
    let srv = new_conn("srv", driver, USOCK_OPEN);
    let cli0 = new_conn("cli0", driver, USOCK_OPEN);
    let master = new_map("master_map", null_mut());
    let role = new_obj("Role", "UProperty");
    let remote = new_obj("RemoteRole", "UProperty");
    unsafe {
        (*driver).server_connection = srv.cast();
        (*driver).client_connections = tarray(&[cli0.cast()]);
        (*driver).master_map = master.cast();
        (*driver).role_property = role;
        (*driver).remote_role_property = remote;
    }
    set_ar(false, false, &[], 0);
    ok(unsafe { net_driver::serialize(s, driver, archive()) });
    flush(lines);
}

fn case_destroy(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    begin(lines, "driver_destroy");
    let driver = new_driver();
    let srv = new_conn("srv", driver, USOCK_OPEN);
    let cli0 = new_conn("cli0", driver, USOCK_OPEN);
    let cli1 = new_conn("cli1", driver, USOCK_OPEN);
    let master = new_map("master_map", null_mut());
    unsafe {
        (*driver).server_connection = srv.cast();
        (*driver).client_connections = tarray(&[cli0.cast(), cli1.cast()]);
        (*driver).master_map = master.cast();
    }
    ok(unsafe { net_driver::destroy(s, driver) });
    flush(lines);
}

fn case_exec(
    s: &Hp2NetDriverServices,
    lines: &mut Vec<String>,
    name: &str,
    command: &str,
    populated: bool,
) {
    begin(lines, name);
    let driver = new_driver();
    if populated {
        let srv = new_conn("srv", driver, USOCK_OPEN);
        let cli0 = new_conn("cli0", driver, USOCK_OPEN);
        let ch = new_channel(
            "srv_chan",
            srv,
            ChannelSetup::new(1, CHTYPE_ACTOR, 0, 1, 1),
            null_mut(),
        );
        unsafe {
            (*driver).server_connection = srv.cast();
            (*driver).client_connections = tarray(&[cli0.cast()]);
            (*srv).open_channels = tarray(&[ch.cast()]);
        }
    }
    let cmd = wide_z(command);
    let result = ok(unsafe { net_driver::exec_sockets(s, driver, cmd.as_ptr(), output()) });
    finish_result(lines, result as i32);
}

fn case_notify_actor_destroyed(s: &Hp2NetDriverServices, lines: &mut Vec<String>) {
    begin(lines, "notify_no_channel");
    let driver = new_driver();
    let cli0 = new_conn("cli0", driver, USOCK_OPEN);
    unsafe { (*driver).client_connections = tarray(&[cli0.cast()]) };
    let gone = new_actor("actor_gone", false, false, true);
    ok(unsafe { net_driver::notify_actor_destroyed(s, driver, gone) });
    flush(lines);

    begin(lines, "notify_closes_channel");
    let driver = new_driver();
    let cli0 = new_conn("cli0", driver, USOCK_OPEN);
    let cli1 = new_conn("cli1", driver, USOCK_OPEN);
    let live = new_actor("actor_live", false, false, false);
    new_channel(
        "chan_live",
        cli1,
        ChannelSetup::new(4, CHTYPE_ACTOR, 0, 1, 1),
        live,
    );
    unsafe { (*driver).client_connections = tarray(&[cli0.cast(), cli1.cast()]) };
    ok(unsafe { net_driver::notify_actor_destroyed(s, driver, live) });
    flush(lines);
}

fn rebuild() -> Vec<String> {
    let s = services();
    let mut lines = Vec::with_capacity(EXPECTED_RECORDS);
    case_can_serialize(&s, &mut lines);
    case_serialize_load(&s, &mut lines);
    case_serialize_save(&s, &mut lines);
    case_driver_ctor(&s, &mut lines, "ctor_plain", "");
    case_driver_ctor(&s, &mut lines, "ctor_profilestats", "-profilestats");
    case_static_ctor(&s, &mut lines);
    case_tick_flush(&s, &mut lines);
    case_init(&mut lines, "init_connect");
    case_init(&mut lines, "init_listen");
    case_tick_dispatch(&s, &mut lines);
    case_serialize(&s, &mut lines);
    case_destroy(&s, &mut lines);
    case_exec(&s, &mut lines, "exec_sockets", "SOCKETS", true);
    case_exec(&s, &mut lines, "exec_unknown", "FOO", false);
    case_notify_actor_destroyed(&s, &mut lines);
    flush(&mut lines);
    lines.push("driver_fixture end".to_string());
    lines
}

#[test]
fn net_driver_fixture_matches_cpp() {
    let expected: Vec<&str> = FIXTURE.lines().collect();
    assert_eq!(expected.len(), EXPECTED_RECORDS);
    let actual = rebuild();
    assert_eq!(actual.len(), expected.len());
    for (i, (want, got)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_eq!(got, want, "record {}", i + 1);
    }
}
