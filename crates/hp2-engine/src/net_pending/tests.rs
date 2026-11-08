use super::*;
use std::cell::{Cell, RefCell};

use hp2_abi::engine::generated::APlayerPawn;

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static URL: Cell<usize> = const { Cell::new(0) };
    static PAWN: Cell<usize> = const { Cell::new(0) };
    static SECRET: Cell<usize> = const { Cell::new(0) };
    static INIT_RESULT: Cell<i32> = const { Cell::new(1) };
    static SKIP_RESULT: Cell<i32> = const { Cell::new(0) };
    /// Owns every mock string and option array; freed when the test thread exits.
    static TEXTS: RefCell<Vec<Box<[u32]>>> = const { RefCell::new(Vec::new()) };
    static OPS: RefCell<Vec<Box<[TArrayRaw]>>> = const { RefCell::new(Vec::new()) };
}

const ENGINE: usize = 0xe1;
const DRIVER_CLASS: usize = 0xc1;

fn note(line: String) {
    LOG.with(|log| log.borrow_mut().push(line));
}
fn take_log() -> Vec<String> {
    LOG.with(|log| log.take())
}

fn wide(text: &str) -> Vec<u32> {
    text.chars().map(|c| c as u32).chain([0]).collect()
}
/// A NUL-terminated copy of `text` that lives as long as the test thread.
fn keep(text: &str) -> *const u32 {
    let text = wide(text).into_boxed_slice();
    let data = text.as_ptr();
    TEXTS.with(|texts| texts.borrow_mut().push(text));
    data
}
unsafe fn read(text: *const u32) -> String {
    let mut out = String::new();
    let mut p = text;
    while unsafe { *p } != 0 {
        out.push(char::from_u32(unsafe { *p }).unwrap());
        p = unsafe { p.add(1) };
    }
    out
}
unsafe fn set_fstring(string: *mut TArrayRaw, text: &str) {
    let raw = if text.is_empty() {
        TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        }
    } else {
        let len = text.chars().count() as i32 + 1;
        TArrayRaw {
            data: keep(text).cast_mut().cast(),
            num: len,
            max: len,
        }
    };
    unsafe { ptr::write_unaligned(string, raw) };
}
unsafe fn get_fstring(string: *const TArrayRaw) -> String {
    unsafe { read(fstring_text(string)) }
}
unsafe fn set_ops(url: *mut c_void, ops: &[&str]) {
    let mut storage: Box<[TArrayRaw]> = (0..8).map(|_| unsafe { mem::zeroed() }).collect();
    for (slot, op) in storage.iter_mut().zip(ops) {
        unsafe { set_fstring(slot, op) };
    }
    let raw = TArrayRaw {
        data: storage.as_mut_ptr().cast(),
        num: ops.len() as i32,
        max: 8,
    };
    OPS.with(|arrays| arrays.borrow_mut().push(storage));
    unsafe { ptr::write_unaligned(url_ops(url), raw) };
}
unsafe fn get_ops(url: *mut c_void) -> Vec<String> {
    let ops = unsafe { ptr::read_unaligned(url_ops(url)) };
    (0..ops.num as usize)
        .map(|i| unsafe { get_fstring(ops.data.cast::<TArrayRaw>().add(i)) })
        .collect()
}
/// `TArray::Remove(index)` on a mock array of `size`-byte elements.
unsafe fn remove(array: *mut TArrayRaw, index: i32, size: usize) {
    let mut raw = unsafe { ptr::read_unaligned(array) };
    let data = raw.data.cast::<u8>();
    let tail = (raw.num - index - 1) as usize * size;
    unsafe {
        ptr::copy(
            data.add((index as usize + 1) * size),
            data.add(index as usize * size),
            tail,
        )
    };
    raw.num -= 1;
    unsafe { ptr::write_unaligned(array, raw) };
}
/// Mock `Parse`: the value after `key`, up to the next space.
unsafe fn find(text: *const u32, key: *const u32) -> Option<String> {
    let (text, key) = unsafe { (read(text), read(key)) };
    let start = text.find(&key)? + key.len();
    Some(text[start..].split(' ').next().unwrap().to_owned())
}
fn found(value: &Option<String>) -> i32 {
    i32::from(value.is_some())
}

unsafe extern "C" fn log_dev_net(text: *const u32) -> i32 {
    note(format!("devnet {}", unsafe { read(text) }));
    0
}
unsafe extern "C" fn log_dev_net_format(format: *const u32) -> i32 {
    note(format!("devnet-format {}", unsafe { read(format) }));
    0
}
unsafe extern "C" fn check_failed(expr: *const c_char, line: i32) -> i32 {
    let expr = unsafe { CStr::from_ptr(expr) }.to_str().unwrap();
    note(format!("check {expr} {line}"));
    1
}
unsafe extern "C" fn localize_error(key: *const c_char, result: *mut *const u32) -> i32 {
    let key = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
    let text = if key == "DownloadFailed" {
        "failed %s: %s".to_owned()
    } else {
        format!("<{key}>")
    };
    unsafe { *result = keep(&text) };
    0
}
unsafe extern "C" fn fstring_assign(string: *mut c_void, text: *const u32) -> i32 {
    unsafe { set_fstring(string.cast(), &read(text)) };
    0
}
unsafe extern "C" fn fstring_printf_texts(
    string: *mut c_void,
    format: *const u32,
    first: *const u32,
    second: *const u32,
) -> i32 {
    let text = unsafe {
        read(format)
            .replacen("%s", &read(first), 1)
            .replacen("%s", &read(second), 1)
    };
    unsafe { set_fstring(string.cast(), &text) };
    0
}
unsafe extern "C" fn app_atoi(text: *const u32, result: *mut i32) -> i32 {
    unsafe { *result = read(text).trim().parse().unwrap_or(0) };
    0
}
unsafe extern "C" fn pending_engine(_pending: *mut c_void, result: *mut *mut c_void) -> i32 {
    unsafe { *result = ENGINE as *mut c_void };
    0
}
unsafe extern "C" fn pending_url(_pending: *mut c_void, result: *mut *mut c_void) -> i32 {
    unsafe { *result = URL.with(Cell::get) as *mut c_void };
    0
}
unsafe extern "C" fn static_load_driver_class(name: *const u32, result: *mut *mut c_void) -> i32 {
    note(format!("load-driver {}", unsafe { read(name) }));
    unsafe { *result = DRIVER_CLASS as *mut c_void };
    0
}
unsafe extern "C" fn construct_driver(class: *mut c_void, result: *mut *mut c_void) -> i32 {
    assert_eq!(class as usize, DRIVER_CLASS);
    // The driver the world already wired to its server connection.
    unsafe { *result = PENDING_DRIVER.with(Cell::get) as *mut c_void };
    0
}
thread_local! {
    static PENDING_DRIVER: Cell<usize> = const { Cell::new(0) };
}
unsafe extern "C" fn driver_init_connect(
    driver: *mut c_void,
    pending: *mut c_void,
    result: *mut i32,
) -> i32 {
    assert_eq!(unsafe { net_driver(pending) }, driver);
    unsafe { *result = INIT_RESULT.with(Cell::get) };
    0
}
unsafe extern "C" fn driver_delete(_driver: *mut c_void) -> i32 {
    note("delete-driver".to_owned());
    0
}
unsafe extern "C" fn driver_tick_dispatch(_driver: *mut c_void, delta: f32) -> i32 {
    note(format!("dispatch {delta}"));
    0
}
unsafe extern "C" fn driver_tick_flush(_driver: *mut c_void) -> i32 {
    note("tick-flush".to_owned());
    0
}
unsafe extern "C" fn connection_log(_conn: *mut c_void, text: *const u32) -> i32 {
    note(format!("send {}", unsafe { read(text) }));
    0
}
unsafe extern "C" fn connection_flush_net(_conn: *mut c_void) -> i32 {
    note("flush".to_owned());
    0
}
unsafe extern "C" fn connection_receive_file(_conn: *mut c_void, index: i32) -> i32 {
    note(format!("receive {index}"));
    0
}
unsafe extern "C" fn channel_close(_channel: *mut c_void) -> i32 {
    note("close".to_owned());
    0
}
unsafe extern "C" fn download_try_skip_file(_download: *mut c_void, result: *mut i32) -> i32 {
    unsafe { *result = SKIP_RESULT.with(Cell::get) };
    0
}
unsafe extern "C" fn download_delete(_download: *mut c_void) -> i32 {
    note("delete-download".to_owned());
    0
}
unsafe extern "C" fn download_info_add_zeroed(conn: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        let slot = ptr::addr_of_mut!((*connection(conn)).download_info);
        let mut raw = ptr::read_unaligned(slot);
        ptr::write_bytes(raw.data.cast::<FDownloadInfo>().add(raw.num as usize), 0, 1);
        *result = raw.num;
        raw.num += 1;
        ptr::write_unaligned(slot, raw);
    }
    0
}
unsafe extern "C" fn download_info_remove(conn: *mut c_void, index: i32) -> i32 {
    note(format!("remove-download-info {index}"));
    unsafe {
        remove(
            ptr::addr_of_mut!((*connection(conn)).download_info),
            index,
            size_of::<FDownloadInfo>(),
        )
    };
    0
}
unsafe extern "C" fn load_download_class(name: *const u32, result: *mut *mut c_void) -> i32 {
    let known = unsafe { read(name) } == "IpDrv.HTTPDownload";
    unsafe {
        *result = if known {
            0xd1 as *mut c_void
        } else {
            ptr::null_mut()
        }
    };
    0
}
unsafe extern "C" fn package_list_add(map: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        let slot = ptr::addr_of_mut!((*map.cast::<UPackageMap>()).list);
        let mut raw = ptr::read_unaligned(slot);
        ptr::write_bytes(raw.data.cast::<FPackageInfo>().add(raw.num as usize), 0, 1);
        *result = raw.num;
        raw.num += 1;
        ptr::write_unaligned(slot, raw);
    }
    0
}
unsafe extern "C" fn package_list_remove(map: *mut c_void, index: i32) -> i32 {
    note(format!("remove-package {index}"));
    unsafe {
        remove(
            ptr::addr_of_mut!((*map.cast::<UPackageMap>()).list),
            index,
            size_of::<FPackageInfo>(),
        )
    };
    0
}
/// A mock package is its own NUL-terminated name.
unsafe extern "C" fn create_package(name: *const u32, result: *mut *mut c_void) -> i32 {
    unsafe { *result = keep(&read(name)).cast_mut().cast() };
    0
}
unsafe extern "C" fn object_name(object: *mut c_void, result: *mut *const u32) -> i32 {
    unsafe { *result = object.cast_const().cast() };
    0
}
unsafe extern "C" fn find_package_file(
    name: *const u32,
    _guid: *const c_void,
    _filename: *mut u32,
    max: i32,
    result: *mut i32,
) -> i32 {
    assert_eq!(max, FILENAME_LEN as i32);
    unsafe { *result = i32::from(read(name).starts_with("Have")) };
    0
}
unsafe extern "C" fn dll_ext(result: *mut *const u32) -> i32 {
    unsafe { *result = lit!(".dylib").as_ptr() };
    0
}
unsafe extern "C" fn file_size(filename: *const u32, result: *mut i32) -> i32 {
    let filename = unsafe { read(filename) };
    note(format!("file-size {filename}"));
    unsafe {
        *result = if filename.starts_with("Native") {
            10
        } else {
            -1
        }
    };
    0
}
unsafe extern "C" fn parse_int(
    text: *const u32,
    key: *const u32,
    value: *mut i32,
    parsed: *mut i32,
) -> i32 {
    let found_value = unsafe { find(text, key) };
    if let Some(v) = &found_value {
        unsafe { *value = v.parse().unwrap() };
    }
    unsafe { *parsed = found(&found_value) };
    0
}
unsafe extern "C" fn parse_dword(
    text: *const u32,
    key: *const u32,
    value: *mut u32,
    parsed: *mut i32,
) -> i32 {
    let found_value = unsafe { find(text, key) };
    if let Some(v) = &found_value {
        unsafe { *value = v.parse().unwrap() };
    }
    unsafe { *parsed = found(&found_value) };
    0
}
unsafe extern "C" fn parse_guid(
    text: *const u32,
    key: *const u32,
    value: *mut c_void,
    parsed: *mut i32,
) -> i32 {
    let found_value = unsafe { find(text, key) };
    if let Some(v) = &found_value {
        unsafe { *value.cast::<[u32; 4]>() = [u32::from_str_radix(v, 16).unwrap(), 0, 0, 0] };
    }
    unsafe { *parsed = found(&found_value) };
    0
}
unsafe extern "C" fn parse_text(
    text: *const u32,
    key: *const u32,
    value: *mut u32,
    max: i32,
    parsed: *mut i32,
) -> i32 {
    assert_eq!(max, NAME_SIZE as i32);
    let found_value = unsafe { find(text, key) };
    if let Some(v) = &found_value {
        let v = wide(v);
        unsafe { ptr::copy_nonoverlapping(v.as_ptr(), value, v.len()) };
    }
    unsafe { *parsed = found(&found_value) };
    0
}
unsafe extern "C" fn parse_fstring(
    text: *const u32,
    key: *const u32,
    value: *mut c_void,
    parsed: *mut i32,
) -> i32 {
    let found_value = unsafe { find(text, key) };
    if let Some(v) = &found_value {
        unsafe { set_fstring(value.cast(), v) };
    }
    unsafe { *parsed = found(&found_value) };
    0
}
unsafe extern "C" fn parse_ubool(
    text: *const u32,
    key: *const u32,
    value: *mut i32,
    parsed: *mut i32,
) -> i32 {
    let found_value = unsafe { find(text, key) };
    if let Some(v) = &found_value {
        unsafe { *value = i32::from(v == "1" || v == "True") };
    }
    unsafe { *parsed = found(&found_value) };
    0
}
unsafe extern "C" fn url_copy_scope(
    url: *mut c_void,
    context: *mut c_void,
    resume: Hp2NetPendingContinuation,
) -> i32 {
    unsafe {
        let mut copy: FURL = ptr::read(url.cast());
        let copy_ptr = (&raw mut copy).cast::<c_void>();
        let ops = get_ops(url);
        set_ops(
            copy_ptr,
            &ops.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        note("url-copy".to_owned());
        let status = resume(context, copy_ptr);
        note(format!(
            "url-copy-end host={:?}",
            get_fstring(ptr::addr_of!(copy.host))
        ));
        status
    }
}
unsafe fn url_text(url: *mut c_void) -> String {
    unsafe {
        format!(
            "host={};ops={}",
            get_fstring(ptr::addr_of!((*url.cast::<FURL>()).host)),
            get_ops(url).join(",")
        )
    }
}
unsafe extern "C" fn url_string_scope(
    url: *mut c_void,
    context: *mut c_void,
    resume: Hp2NetPendingContinuation,
) -> i32 {
    let text = wide(&unsafe { url_text(url) });
    unsafe { resume(context, text.as_ptr().cast_mut().cast()) }
}
unsafe extern "C" fn url_string_assign(url: *mut c_void, string: *mut c_void) -> i32 {
    unsafe { set_fstring(string.cast(), &url_text(url)) };
    0
}
unsafe extern "C" fn url_op_remove(url: *mut c_void, index: i32) -> i32 {
    unsafe { remove(url_ops(url), index, size_of::<TArrayRaw>()) };
    0
}
unsafe extern "C" fn url_add_option(url: *mut c_void, text: *const u32) -> i32 {
    unsafe {
        let mut ops = get_ops(url);
        ops.push(read(text));
        set_ops(url, &ops.iter().map(String::as_str).collect::<Vec<_>>());
    }
    0
}
unsafe extern "C" fn engine_challenge_response(
    engine: *mut c_void,
    challenge: i32,
    result: *mut i32,
) -> i32 {
    assert_eq!(engine as usize, ENGINE);
    unsafe { *result = challenge * 2 };
    0
}
unsafe extern "C" fn engine_viewport_actor(engine: *mut c_void, result: *mut *mut c_void) -> i32 {
    assert_eq!(engine as usize, ENGINE);
    unsafe { *result = PAWN.with(Cell::get) as *mut c_void };
    0
}
unsafe extern "C" fn pawn_world_secret(_pawn: *mut c_void, result: *mut *mut c_void) -> i32 {
    unsafe { *result = SECRET.with(Cell::get) as *mut c_void };
    0
}
unsafe extern "C" fn md5_overflow() -> i32 {
    panic!("net_pending test: unexpected MD5 overflow")
}

fn services() -> Hp2NetPendingServices {
    Hp2NetPendingServices {
        log_dev_net,
        log_dev_net_format,
        check_failed,
        localize_error,
        fstring_assign,
        fstring_printf_texts,
        app_atoi,
        pending_engine,
        pending_url,
        static_load_driver_class,
        construct_driver,
        driver_init_connect,
        driver_delete,
        driver_tick_dispatch,
        driver_tick_flush,
        connection_log,
        connection_flush_net,
        connection_receive_file,
        channel_close,
        download_try_skip_file,
        download_delete,
        download_info_add_zeroed,
        download_info_remove,
        load_download_class,
        package_list_add,
        package_list_remove,
        create_package,
        object_name,
        find_package_file,
        dll_ext,
        file_size,
        parse_int,
        parse_dword,
        parse_guid,
        parse_text,
        parse_fstring,
        parse_ubool,
        url_copy_scope,
        url_string_scope,
        url_string_assign,
        url_op_remove,
        url_add_option,
        engine_challenge_response,
        engine_viewport_actor,
        pawn_world_secret,
        md5_overflow,
    }
}

/// A pending level wired to a driver, server connection, package map and URL.
struct World {
    pending: Box<UNetPendingLevel>,
    driver: Box<UNetDriver>,
    conn: Box<UNetConnection>,
    map: Box<UPackageMap>,
    url: Box<FURL>,
    _packages: Vec<FPackageInfo>,
    _downloads: Vec<FDownloadInfo>,
}

impl World {
    fn new() -> Self {
        let mut world = World {
            pending: Box::new(unsafe { mem::zeroed() }),
            driver: Box::new(unsafe { mem::zeroed() }),
            conn: Box::new(unsafe { mem::zeroed() }),
            map: Box::new(unsafe { mem::zeroed() }),
            url: Box::new(unsafe { mem::zeroed() }),
            _packages: Vec::with_capacity(8),
            _downloads: Vec::with_capacity(8),
        };
        world.map.list = TArrayRaw {
            data: world._packages.as_mut_ptr().cast(),
            num: 0,
            max: 8,
        };
        world.conn.download_info = TArrayRaw {
            data: world._downloads.as_mut_ptr().cast(),
            num: 0,
            max: 8,
        };
        world.conn.package_map = (&raw mut *world.map).cast();
        world.conn.state = 3;
        world.driver.server_connection = world.conn();
        world.pending.base.net_driver = (&raw mut *world.driver).cast();
        URL.with(|url| url.set(&raw mut *world.url as usize));
        PENDING_DRIVER.with(|driver| driver.set(&raw mut *world.driver as usize));
        take_log();
        world
    }
    fn this(&mut self) -> *mut c_void {
        (&raw mut *self.pending).cast()
    }
    fn conn(&mut self) -> *mut c_void {
        (&raw mut *self.conn).cast()
    }
    fn url(&mut self) -> *mut c_void {
        (&raw mut *self.url).cast()
    }
    fn receive(&mut self, text: &str) -> R<()> {
        let s = services();
        let text = wide(text);
        let (this, conn) = (self.this(), self.conn());
        unsafe { notify_received_text(&s, this, conn, text.as_ptr()) }
    }
    fn error(&mut self) -> String {
        unsafe { get_fstring(error_slot(self.this())) }
    }
    fn add_package(&mut self, name: &str, flags: u32) {
        let s = services();
        let map = (&raw mut *self.map).cast();
        let index = unsafe { value_of(s.package_list_add, map) };
        let info = unsafe { package_info(map, index) };
        unsafe {
            (*info).parent = keep(name).cast_mut().cast();
            (*info).package_flags = flags;
        }
    }
    fn package_flags(&mut self, index: i32) -> u32 {
        unsafe { (*package_info((&raw mut *self.map).cast(), index)).package_flags }
    }
}

unsafe fn value_of(
    leaf: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    object: *mut c_void,
) -> i32 {
    let mut out = 0;
    assert_eq!(unsafe { leaf(object, &mut out) }, 0);
    out
}

#[test]
fn constructor_sends_hello_or_discards_the_driver() {
    let s = services();
    let mut world = World::new();
    unsafe { set_fstring(error_slot(world.this()), "stale") };
    INIT_RESULT.with(|r| r.set(1));
    unsafe { constructor(&s, world.this()).unwrap() };
    assert_eq!(
        take_log(),
        [
            "load-driver ini:Engine.Engine.NetworkDevice",
            "send HELLO REVISION=0 MINVER=400 VER=433",
            "flush"
        ]
    );
    assert_eq!(world.error(), "");
    assert_eq!(
        unsafe { net_driver(world.this()) },
        (&raw mut *world.driver).cast()
    );

    INIT_RESULT.with(|r| r.set(0));
    unsafe { constructor(&s, world.this()).unwrap() };
    assert_eq!(
        take_log(),
        [
            "load-driver ini:Engine.Engine.NetworkDevice",
            "delete-driver"
        ]
    );
    assert!(unsafe { net_driver(world.this()) }.is_null());
}

#[test]
fn received_text_requires_the_server_connection() {
    let s = services();
    let mut world = World::new();
    let mut other: UNetConnection = unsafe { mem::zeroed() };
    let text = wide("FAILURE");
    let result =
        unsafe { notify_received_text(&s, world.this(), (&raw mut other).cast(), text.as_ptr()) };
    assert!(result.is_err());
    assert_eq!(
        take_log(),
        ["check Connection==NetDriver->ServerConnection 87"]
    );
    assert_eq!(world.error(), "");
}

#[test]
fn upgrade_reports_which_side_is_outdated() {
    let mut world = World::new();
    world.receive("UPGRADE MINVER=500 VER=500").unwrap();
    assert_eq!(world.error(), "<ClientOutdated>");
    assert_eq!(
        take_log(),
        ["devnet PendingLevel received: UPGRADE MINVER=500 VER=500"]
    );
    world.receive("UPGRADE MINVER=433 VER=500").unwrap();
    assert_eq!(world.error(), "<ServerOutdated>");
    world.receive("FAILURE").unwrap();
    assert_eq!(world.error(), "<RejectedByServer>");
}

#[test]
fn failcode_records_the_url_without_passwords() {
    let mut world = World::new();
    let url = world.url();
    unsafe {
        set_ops(
            url,
            &[
                "Name=Harry",
                "password=x",
                "PASSWORD",
                "Pass=1",
                "PaSsWoRd=y",
            ],
        )
    };
    world.receive("FAILCODE NEEDPW").unwrap();
    let this = world.this();
    unsafe {
        assert_eq!(get_fstring(fail_code_slot(this)), "NEEDPW");
        assert_eq!(
            get_fstring(fail_url_slot(this)),
            "host=;ops=Name=Harry,PASSWORD,Pass=1"
        );
        // Only the copy loses its options.
        assert_eq!(get_ops(url).len(), 5);
    }
}

#[test]
fn uses_appends_package_info() {
    let mut world = World::new();
    world
        .receive("USES GUID=abc GEN=3 SIZE=1234 FLAGS=4294934529 PKG=Botpack FNAME=Botpack.u")
        .unwrap();
    let info = unsafe { package_info((&raw mut *world.map).cast(), 0) };
    unsafe {
        assert_eq!(world.map.list.num, 1);
        assert_eq!((*info).guid, [0xabc, 0, 0, 0]);
        assert_eq!((*info).remote_generation, 3);
        assert_eq!((*info).file_size, 1234);
        assert_eq!((*info).package_flags, 0xffff_8001);
        assert_eq!(read((*info).parent.cast()), "Botpack");
        assert_eq!(get_fstring(ptr::addr_of!((*info).url)), "Botpack.u");
    }
}

#[test]
fn userflag_sets_connection_flags() {
    let mut world = World::new();
    world.receive("USERFLAG 17").unwrap();
    assert_eq!(world.conn.user_flags, 17);
}

#[test]
fn challenge_logs_in_with_the_partial_url() {
    let mut world = World::new();
    world.conn.current_net_speed = 2600;
    let url = world.url();
    unsafe {
        set_fstring(ptr::addr_of_mut!((*url.cast::<FURL>()).host), "server");
        set_ops(url, &["game=Botpack.DM", "Name=Harry", "GAME=x"]);
    }
    world
        .receive("CHALLENGE VER=432 CHALLENGE=21 STATS=0")
        .unwrap();
    assert_eq!(world.conn.negotiated_ver, 432);
    assert_eq!(world.conn.challenge, 21);
    assert_eq!(
        take_log(),
        [
            "devnet PendingLevel received: CHALLENGE VER=432 CHALLENGE=21 STATS=0",
            "url-copy",
            "send NETSPEED 2600",
            "send LOGIN RESPONSE=42 URL=host=;ops=game=Botpack.DM,Name=Harry,GAME=x",
            "flush",
            "url-copy-end host=\"\"",
        ]
    );
    // The level URL itself loses its game options and keeps its host.
    assert_eq!(unsafe { get_ops(url) }, ["Name=Harry"]);
    assert_eq!(
        unsafe { get_fstring(ptr::addr_of!((*url.cast::<FURL>()).host)) },
        "server"
    );
}

#[test]
fn challenge_with_stats_adds_the_player_checksum() {
    let mut world = World::new();
    let mut pawn: Box<APlayerPawn> = Box::new(unsafe { mem::zeroed() });
    let mut secret: Box<TArrayRaw> = Box::new(unsafe { mem::zeroed() });
    let mut pri: Box<APlayerReplicationInfo> = Box::new(unsafe { mem::zeroed() });
    let (pawn, secret, pri) = (&raw mut *pawn, &raw mut *secret, &raw mut *pri);
    let option = |world: &mut World| {
        let url = world.url();
        unsafe { set_ops(url, &[]) };
        world.receive("CHALLENGE CHALLENGE=1 STATS=1").unwrap();
        unsafe { get_ops(url) }.pop().unwrap()
    };

    PAWN.with(|p| p.set(0));
    assert_eq!(option(&mut world), "Checksum=NoChecksum");

    PAWN.with(|p| p.set(pawn as usize));
    SECRET.with(|p| p.set(secret as usize));
    assert_eq!(option(&mut world), "Checksum=NoChecksum");

    unsafe { set_fstring(secret, "secret") };
    assert_eq!(
        option(&mut world),
        "Checksum=09433e1853385270b51511571e35eeca"
    );

    unsafe {
        set_fstring(ptr::addr_of_mut!((*pri).player_name), "Harry");
        (*pawn).base.player_replication_info = pri.cast();
    }
    assert_eq!(
        option(&mut world),
        "Checksum=8e6d5a81e5d3044fe3fe4885973cbc68"
    );
}

#[test]
fn dlmgr_keeps_only_loadable_download_classes() {
    let mut world = World::new();
    world
        .receive("DLMGR CLASS=IpDrv.HTTPDownload PARAMS=http://x COMPRESSION=1")
        .unwrap();
    world.receive("DLMGR CLASS=Missing.Download").unwrap();
    assert_eq!(world.conn.download_info.num, 1);
    let info = unsafe { download_info(world.conn(), 0) };
    unsafe {
        assert_eq!((*info).class as usize, 0xd1);
        assert_eq!(
            get_fstring(ptr::addr_of!((*info).class_name)),
            "IpDrv.HTTPDownload"
        );
        assert_eq!(get_fstring(ptr::addr_of!((*info).params)), "http://x");
        assert_eq!((*info).compression, 1);
    }
    assert!(take_log().contains(&"remove-download-info 1".to_owned()));
}

#[test]
fn welcome_rejects_a_package_that_may_not_download() {
    let mut world = World::new();
    let mut channel = 0u8;
    world.conn.channels[0] = (&raw mut channel).cast();
    world.conn.remote_version = 420;
    world.driver.allow_downloads = 1;
    world.add_package("HaveCore", 0);
    world.add_package("Native", 0);
    world.add_package("Secret", 0);
    world
        .receive("WELCOME LEVEL=DM-Deck16 LONE=1 CHALLENGE=9")
        .unwrap();
    assert_eq!(world.error(), "Downloading 'Secret' not allowed");
    assert_eq!(
        take_log(),
        [
            "devnet PendingLevel received: WELCOME LEVEL=DM-Deck16 LONE=1 CHALLENGE=9",
            "devnet Welcomed by server: LEVEL=DM-Deck16 LONE=1 CHALLENGE=9",
            "file-size Native.dylib",
            "file-size Secret.dylib",
            "close",
            "flush",
        ]
    );
    assert_eq!(world.conn.state, USOCK_CLOSED);
    assert_eq!(world.pending.base.files_needed, 1);
    assert_eq!(world.package_flags(2), PKG_NEED);
    assert_eq!(world.pending.base.success, 0);
    assert_eq!(world.pending.base.lone_player, 1);
    assert_eq!(world.conn.challenge, 9);
    assert_eq!(
        unsafe { get_fstring(ptr::addr_of!(world.url.map)) },
        "DM-Deck16"
    );

    // A current server closes the socket itself.
    let mut world = World::new();
    world.conn.channels[0] = (&raw mut channel).cast();
    world.conn.remote_version = 421;
    world.add_package("Secret", PKG_ALLOW_DOWNLOAD);
    world.receive("WELCOME").unwrap();
    assert_eq!(world.conn.state, 3);
}

#[test]
fn welcome_requests_the_first_needed_package() {
    let mut world = World::new();
    world.driver.allow_downloads = 1;
    world.add_package("HaveCore", 0);
    world.add_package("Textures", PKG_ALLOW_DOWNLOAD);
    world.receive("WELCOME").unwrap();
    assert_eq!(world.pending.base.files_needed, 1);
    assert_eq!(world.pending.base.success, 1);
    assert_eq!(world.package_flags(1), PKG_ALLOW_DOWNLOAD | PKG_NEED);
    assert_eq!(take_log().last().unwrap(), "receive 1");
}

#[test]
fn received_file_error_tries_the_next_method_then_reports() {
    let s = services();
    let mut world = World::new();
    world.add_package("Textures", PKG_NEED);
    let (this, conn) = (world.this(), world.conn());
    let mut download = 0u8;
    world.conn.download = (&raw mut download).cast();
    unsafe {
        value_of(download_info_add_zeroed, conn);
        value_of(download_info_add_zeroed, conn);
    }
    let error = wide("timeout");
    unsafe { notify_received_file(&s, this, conn, 0, error.as_ptr(), 0).unwrap() };
    assert_eq!(take_log(), ["remove-download-info 0", "receive 0"]);

    unsafe { notify_received_file(&s, this, conn, 0, error.as_ptr(), 0).unwrap() };
    assert_eq!(world.error(), "failed Textures: timeout");
    // An earlier error wins.
    unsafe { notify_received_file(&s, this, conn, 0, wide("later").as_ptr(), 0).unwrap() };
    assert_eq!(world.error(), "failed Textures: timeout");

    let result = unsafe { notify_received_file(&s, this, conn, 1, error.as_ptr(), 0) };
    assert!(result.is_err());
    assert_eq!(
        take_log(),
        ["check Connection->PackageMap->List.IsValidIndex(PackageIndex) 251"]
    );
}

#[test]
fn received_file_success_clears_need_and_moves_on() {
    let s = services();
    let mut world = World::new();
    world.pending.base.files_needed = 2;
    world.add_package("First", PKG_NEED | PKG_ALLOW_DOWNLOAD);
    world.add_package("Second", PKG_NEED);
    let (this, conn) = (world.this(), world.conn());
    let none = wide("");
    unsafe { notify_received_file(&s, this, conn, 0, none.as_ptr(), 0).unwrap() };
    assert_eq!(world.package_flags(0), PKG_ALLOW_DOWNLOAD);
    assert_eq!(world.pending.base.files_needed, 1);
    assert_eq!(take_log(), ["receive 1"]);

    // A skipped package leaves the map; with nothing needed the download ends.
    let mut download = 0u8;
    world.conn.download = (&raw mut download).cast();
    unsafe { notify_received_file(&s, this, conn, 1, none.as_ptr(), 1).unwrap() };
    assert_eq!(world.map.list.num, 1);
    assert_eq!(world.pending.base.files_needed, 0);
    assert_eq!(take_log(), ["remove-package 1", "delete-download"]);

    let result = unsafe { notify_received_file(&s, this, conn, 0, none.as_ptr(), 0) };
    assert!(result.is_err());
    assert_eq!(take_log(), ["check Info.PackageFlags&PKG_Need 274"]);
}

#[test]
fn tick_fails_a_closed_connection_once() {
    let s = services();
    let mut world = World::new();
    let this = world.this();
    unsafe { tick(&s, this, 0.5).unwrap() };
    assert_eq!(take_log(), ["dispatch 0.5", "tick-flush"]);

    world.conn.state = USOCK_CLOSED;
    unsafe { tick(&s, this, 0.5).unwrap() };
    assert_eq!(world.error(), "<ConnectionFailed>");
    assert!(take_log().is_empty());
    // With an error recorded the closed driver still ticks.
    unsafe { tick(&s, this, 0.5).unwrap() };
    assert_eq!(take_log(), ["dispatch 0.5", "tick-flush"]);

    world.driver.server_connection = ptr::null_mut();
    assert!(unsafe { tick(&s, this, 0.5) }.is_err());
    world.pending.base.net_driver = ptr::null_mut();
    assert!(unsafe { tick(&s, this, 0.5) }.is_err());
    assert_eq!(
        take_log(),
        [
            "check NetDriver->ServerConnection 313",
            "check NetDriver 312"
        ]
    );
}

#[test]
fn join_skip_and_file_requests() {
    let s = services();
    let mut world = World::new();
    let this = world.this();
    unsafe { send_join(&s, this).unwrap() };
    assert_eq!(world.pending.base.sent_join, 1);
    assert_eq!(take_log(), ["send JOIN", "flush"]);

    assert_eq!(unsafe { try_skip_file(&s, this) }, Ok(0));
    let mut download = 0u8;
    world.conn.download = (&raw mut download).cast();
    SKIP_RESULT.with(|r| r.set(7));
    assert_eq!(unsafe { try_skip_file(&s, this) }, Ok(1));
    SKIP_RESULT.with(|r| r.set(0));
    assert_eq!(unsafe { try_skip_file(&s, this) }, Ok(0));

    assert_eq!(unsafe { notify_sending_file(&s) }, Ok(0));
    assert_eq!(take_log(), ["devnet-format <RequestDenied>"]);
    assert_eq!(notify_accepting_connection(), ACCEPTC_REJECT);
    assert_eq!(notify_accepting_channel(), 0);
    assert!(notify_get_level().is_null());
}
