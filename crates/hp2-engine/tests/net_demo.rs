//! Record-level replay of `fixtures/net_demo.txt` for the download port (`hp2_engine::net_download`).
//!
//! `demo_fixture.cpp` drives the original `UDownload` / `UChannelDownload` through sixteen cases and prints one
//! record per leaf call. This test drives the Rust port through the same cases. Each service in the table emits
//! the record its harness hook prints, and writes the struct fields its C++ leaf would write. The port's own
//! control flow is what the fixture checks. Additional cases observe individual codec, string, archive,
//! notification and bunch leaves; no mock implements a download algorithm or completion sequence.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::mem::zeroed;
use std::ptr::null_mut;
use std::sync::LazyLock;

use hp2_abi::engine::generated::{FArchive, FBitWriter, FInBunch, FOutBunch, UChannel, UChannelDownload, UDownload, UFileChannel, UNetConnection};
use hp2_engine::net_demo::Hp2NetDemoServices;
use hp2_engine::net_download::{
    PKG_CLIENT_OPTIONAL, chdl_destroy, chdl_receive_file as chdl_receive_file_with_options,
    chdl_try_skip_file, download_destroy, download_done, download_receive_data,
    download_receive_file as download_receive_file_with_options, download_try_skip_file,
};

unsafe fn download_receive_file(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    conn: *mut c_void,
    index: i32,
) -> Result<(), hp2_engine::net_demo::Thrown> {
    unsafe { download_receive_file_with_options(svc, this, conn, index, std::ptr::null(), 0) }
}

unsafe fn chdl_receive_file(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    conn: *mut c_void,
    index: i32,
) -> Result<(), hp2_engine::net_demo::Thrown> {
    unsafe { chdl_receive_file_with_options(svc, this, conn, index, std::ptr::null(), 0) }
}

const FIXTURE: &str = include_str!("fixtures/net_demo.txt");
const EXPECTED_RECORDS: usize = 144;

/// Per-family record counts of the fixture, keyed by the first token of each line.
const FAMILY_COUNTS: &[(&str, usize)] = &[
    ("archive_delete", 5),
    ("archive_serialize", 11),
    ("case", 16),
    ("chdl_destroy", 1),
    ("chdl_no_channel", 2),
    ("chdl_receive_file", 2),
    ("chdl_try_skip", 2),
    ("create_channel", 3),
    ("create_channel_slot", 2),
    ("create_writer", 20),
    ("debugf", 10),
    ("delete", 6),
    ("destroy_with_writer", 1),
    ("done_move_fail", 1),
    ("done_refused", 1),
    ("done_size_mismatch", 1),
    ("done_skipped", 1),
    ("done_success", 1),
    ("download", 1),
    ("file_size", 3),
    ("load_file_to_string", 1),
    ("logf", 1),
    ("make_directory", 10),
    ("move", 2),
    ("notify_progress", 12),
    ("notify_received_file", 6),
    ("receive_data_ok", 1),
    ("receive_data_open_fail", 1),
    ("receive_data_write_error", 1),
    ("receive_file_index0", 1),
    ("save_string_to_file", 1),
    ("send_bunch", 3),
    ("temp_filename", 10),
    ("try_skip_optional", 2),
    ("try_skip_required", 2),
];

/// Temp path written by `appCreateTempFilename` under the `/cache` system path.
const TEMP: &str = "/cache/tmp0001.tmp";
/// Package GUID destinations (`demo_fixture.cpp` `make_package_map`, GUIDs at lines 493 and 498).
const DEST: [&str; 2] = [
    "/cache/11111111222222223333333344444444.uxx",
    "/cache/55555555666666667777777788888888.uxx",
];
/// Package sizes and flags (`demo_fixture.cpp` `make_package_map`, lines 491 and 495-496).
const PKG_SIZE: [i32; 2] = [4096, 1000];
const PKG_FLAGS: [u32; 2] = [0, PKG_CLIENT_OPTIONAL];
/// Stand-in `FPackageInfo` objects. The byte value is the package index.
static INFO_TAGS: [u8; 2] = [0, 1];
include!("net_demo_unused.txt");

/// Leaf state the harness keeps in globals (`g_*` in `demo_fixture.cpp`).
/// `begin_case` resets it the same way as `reset_leaves()`.
struct Mock {
    /// Records in harness order.
    trace: Vec<String>,
    strings: Vec<Box<[u32]>>,
    events: Vec<String>,
    parent: String,
    url: String,
    localization: BTreeMap<String, String>,
    temp_count: usize,
    reader_fails: bool,
    signature: u32,
    override_error: bool,
    override_done: bool,
    virtual_error_fails: bool,
    bunch_error: bool,
    payloads: Vec<Vec<u8>>,

    /// `g_file_size`: `GFileManager->FileSize` of the temp file.
    file_size: i32,
    /// `g_move_ok`: `GFileManager->Move` result.
    move_ok: i32,
    /// `g_create_writer_fails`.
    create_writer_fails: bool,
    /// `g_error_on_write`: serialize call index that sets `ArIsError`. Zero disables it.
    error_on_write: i32,
    /// `g_channel_fails`: `UNetConnection::CreateChannel` returns null.
    channel_fails: bool,
    /// `g_serialize_calls`: serialize calls since the last writer was created.
    serialize_calls: i32,
}

impl Mock {
    const fn new() -> Self {
        Self {
            trace: Vec::new(),
            strings: Vec::new(),
            events: Vec::new(),
            parent: String::new(),
            url: String::new(),
            localization: BTreeMap::new(),
            temp_count: 0,
            reader_fails: false,
            signature: 1234,
            override_error: false,
            override_done: false,
            virtual_error_fails: false,
            bunch_error: false,
            payloads: Vec::new(),

            file_size: 4096,
            move_ok: 1,
            create_writer_fails: false,
            error_on_write: 0,
            channel_fails: false,
            serialize_calls: 0,
        }
    }

    /// Mirrors `reset_leaves()`. The trace is kept.
    fn reset_leaves(&mut self) {
        self.temp_count = 0;
        self.create_writer_fails = false;
        self.error_on_write = 0;
        self.serialize_calls = 0;
        self.file_size = 4096;
        self.move_ok = 1;
        self.channel_fails = false;
    }
}

thread_local! {
    static MOCK: RefCell<Mock> = const { RefCell::new(Mock::new()) };
}

fn emit(line: impl Into<String>) {
    let line = line.into();
    MOCK.with(|m| m.borrow_mut().trace.push(line));
}

fn with_mock<T>(f: impl FnOnce(&mut Mock) -> T) -> T {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

/// `FixtureWriter`: `written` sums serialized bytes. `ar.ar_is_error` mirrors `ArIsError`,
/// matching the real `FArchive` layout so the port's direct field reads work on the mock.
struct Writer {
    ar: FArchive,
    written: i32,
}

/// Writes a TCHAR string into a `[u32; 256]` field, zero-terminated.
fn set_text(dst: &mut [u32; 256], text: &str) {
    assert!(text.chars().count() < 256, "text too long for TCHAR field");
    dst.fill(0);
    for (slot, ch) in dst.iter_mut().zip(text.chars()) {
        *slot = u32::from(ch);
    }
}

/// Reads a zero-terminated TCHAR field as a string.
fn read_text(src: &[u32; 256]) -> String {
    src.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| char::from_u32(c).expect("TCHAR is a scalar value"))
        .collect()
}

/// Package index of a `FPackageInfo` stand-in pointer.
///
/// # Safety
/// `info` is one of `INFO_TAGS`.
unsafe fn pkg_index(info: *mut c_void) -> usize {
    INFO_TAGS
        .iter()
        .position(|tag| std::ptr::eq(tag, info.cast::<u8>()))
        .expect("leaf requires FPackageInfo*, not UDownload*")
}

/// # Safety
/// `this` is a live `UDownload`.
unsafe fn set_error(this: *mut c_void, text: &str) {
    unsafe { set_text(&mut (*this.cast::<UDownload>()).error, text) }
}

// ---- archive leaves ----------------------------------------------------------------------------

unsafe extern "C" fn mock_obj_super_destroy(_this: *mut c_void) -> i32 {
    0
}

/// `FixtureWriter` destructor: `archive_delete written=N`.
unsafe extern "C" fn mock_ar_delete(ar: *mut c_void) -> i32 {
    unsafe {
        let w = Box::from_raw(ar.cast::<Writer>());
        event(format!("archive_delete:{}", w.written));
        emit(format!("archive_delete written={}", w.written));
    }
    0
}

/// `FixtureWriter::Serialize`: counts the call, and sets `ArIsError` on the call `g_error_on_write` names.
unsafe extern "C" fn mock_ar_serialize(ar: *mut c_void, _data: *mut c_void, count: i32) -> i32 {
    let calls = with_mock(|m| {
        m.serialize_calls += 1;
        m.serialize_calls
    });
    emit(format!("archive_serialize count={count}"));
    unsafe {
        let w = &mut *ar.cast::<Writer>();
        w.written += count;
        if with_mock(|m| m.error_on_write) == calls {
            w.ar.ar_is_error = 1;
        }
    }
    0
}

// ---- connection / package-info leaves ----------------------------------------------------------

/// `UNetConnection::CreateChannel(CHTYPE_File, 1, INDEX_NONE)`: records the call, then picks the first free
/// slot from 1 (`demo_fixture.cpp` lines 425-441, UnConn.cpp 603-614, 622-623).
unsafe extern "C" fn mock_conn_create_file_channel(
    conn: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    emit("create_channel type=3 local=1 index=-1");
    if with_mock(|m| m.channel_fails) {
        unsafe { *out = null_mut() };
        return 0;
    }
    unsafe {
        let conn = &mut *conn.cast::<UNetConnection>();
        let slot = (1..conn.channels.len())
            .find(|&i| conn.channels[i].is_null())
            .expect("a free channel slot");
        emit(format!("create_channel_slot index={slot}"));
        if conn.max_packet == 0 { conn.max_packet = 512; }
        let connection = conn as *mut UNetConnection;
        let mut channel = Box::new(zeroed::<Channel>());
        channel.package_index = -1;
        let base = (&raw mut *channel).cast::<UChannel>();
        (*base).connection = connection.cast();
        (*base).ch_index = slot as i32;
        (*base).ch_type = 3;
        let ch = Box::into_raw(channel).cast::<c_void>();
        conn.channels[slot] = ch;
        *out = ch;
    }
    0
}

include!("net_demo_leaves.txt");

fn services() -> Hp2NetDemoServices {
    static ALLOCATOR: std::sync::Once = std::sync::Once::new();
    ALLOCATOR.call_once(publish_bunch_allocator);
    Hp2NetDemoServices {
        demo_app_fail_assert: mock_app_fail_assert,
        demo_app_seconds: unused_demo_app_seconds,
        demo_obj_super_destroy: mock_obj_super_destroy,
        demo_ar_delete: mock_ar_delete,
        demo_ar_at_end: unused_demo_ar_at_end,
        demo_ar_tell: unused_demo_ar_tell,
        demo_ar_seek: unused_demo_ar_seek,
        demo_ar_serialize: mock_ar_serialize,
        demo_ar_write_i32: unused_demo_ar_write_i32,
        demo_drv_write_time: unused_demo_drv_write_time,
        demo_ar_read_i32: mock_ar_read_i32,
        demo_ar_read_u32: mock_ar_read_u32,
        demo_ar_read_ftime: unused_demo_ar_read_ftime,
        demo_ar_read_fstring_discard: mock_ar_read_fstring_discard,
        demo_url_has_option: unused_demo_url_has_option,
        demo_parse_command: unused_demo_parse_command,
        demo_app_strcpy: mock_app_strcpy,
        demo_fstring_new: mock_fstring_new,
        demo_fstring_delete: mock_fstring_delete,
        demo_fstring_chars: mock_fstring_chars,
        demo_fstring_assign: mock_fstring_assign,
        demo_localize_error_ansi: mock_localize_error_ansi,
        demo_localize_error_wide: mock_localize_error_wide,
        demo_localize_progress_ansi: mock_localize_error_ansi,
        demo_debugf_text: mock_debugf_text,
        demo_debugf_devnet_text: mock_debugf_devnet_text,
        demo_out_logf_text: unused_demo_out_logf_text,
        demo_static_load_actor_class: unused_demo_static_load_actor_class,
        demo_static_load_player_pawn_class: unused_demo_static_load_player_pawn_class,
        demo_sys_cache_path: mock_sys_cache_path,
        demo_app_create_temp_filename: mock_app_create_temp_filename,
        demo_fm_create_reader: mock_fm_create_reader,
        demo_fm_create_writer: mock_fm_create_writer,
        demo_fm_make_directory: mock_fm_make_directory,
        demo_fm_file_size: mock_fm_file_size,
        demo_fm_delete: mock_fm_delete,
        demo_fm_move: mock_fm_move,
        demo_codec_new: mock_codec_new,
        demo_codec_add_rle: mock_codec_add_rle,
        demo_codec_add_bwt: mock_codec_add_bwt,
        demo_codec_add_mtf: mock_codec_add_mtf,
        demo_codec_add_huffman: mock_codec_add_huffman,
        demo_codec_decode: mock_codec_decode,
        demo_codec_delete: mock_codec_delete,
        demo_cache_ini_new: mock_cache_ini_new,
        demo_cache_ini_delete: mock_cache_ini_delete,
        demo_cache_ini_set_string: mock_cache_ini_set_string,
        bunch: hp2_engine::net_bunch::Hp2NetBunchServices {
            out_construct: mock_bunch_construct,
            out_destruct: mock_bunch_destruct,
            in_construct: mock_in_construct,
            in_destruct: mock_in_destruct,
            check_failed: mock_app_fail_assert,
        },
        demo_bunch_write_text: mock_bunch_write_text,
        demo_bunch_write_guid: mock_bunch_write_guid,
        demo_bunch_set_reliable: mock_bunch_set_reliable,
        demo_bunch_is_error: mock_bunch_is_error,
        demo_bunch_send: mock_bunch_send,
        demo_drv_super_init_connect: unused_demo_drv_super_init_connect,
        demo_drv_super_init_listen: unused_demo_drv_super_init_listen,
        demo_drv_set_demo_filename: unused_demo_drv_set_demo_filename,
        demo_drv_set_time_zero: unused_demo_drv_set_time_zero,
        demo_drv_new_connection: unused_demo_drv_new_connection,
        demo_drv_set_loop_url: unused_demo_drv_set_loop_url,
        demo_drv_create_reader: unused_demo_drv_create_reader,
        demo_drv_create_writer: unused_demo_drv_create_writer,
        demo_drv_demo_filename_chars: unused_demo_drv_demo_filename_chars,
        demo_drv_spectator_class_chars: unused_demo_drv_spectator_class_chars,
        demo_drv_set_spectator_class: unused_demo_drv_set_spectator_class,
        demo_drv_loop_url_to_fstring: unused_demo_drv_loop_url_to_fstring,
        demo_drv_listen_add_client: unused_demo_drv_listen_add_client,
        demo_drv_super_tick_dispatch: unused_demo_drv_super_tick_dispatch,
        demo_drv_time_raw: unused_demo_drv_time_raw,
        demo_drv_notify_get_level: unused_demo_drv_notify_get_level,
        demo_drv_delete_self: unused_demo_drv_delete_self,
        demo_drv_master_copy_linkers: unused_demo_drv_master_copy_linkers,
        demo_drv_add_string_property: unused_demo_drv_add_string_property,
        demo_level_engine: unused_demo_level_engine,
        demo_level_netmode: unused_demo_level_netmode,
        demo_level_net_server_package_map: unused_demo_level_net_server_package_map,
        demo_level_welcome_player: unused_demo_level_welcome_player,
        demo_level_spawn_actor: unused_demo_level_spawn_actor,
        demo_level_clear_demo_rec_driver: unused_demo_level_clear_demo_rec_driver,
        demo_level_exec: unused_demo_level_exec,
        demo_engine_build_server_master_map: unused_demo_engine_build_server_master_map,
        demo_conn_received_raw_packet: unused_demo_conn_received_raw_packet,
        demo_conn_super_flush_net: unused_demo_conn_super_flush_net,
        demo_conn_super_handle_client_player: unused_demo_conn_super_handle_client_player,
        demo_conn_create_file_channel: mock_conn_create_file_channel,
        demo_conn_create_control_channel: unused_demo_conn_create_control_channel,
        demo_conn_init_out: unused_demo_conn_init_out,
        demo_conn_download: mock_conn_download,
        demo_conn_set_download: mock_conn_set_download,
        demo_conn_notify_progress: mock_conn_notify_progress,
        demo_conn_notify_received_file: mock_conn_notify_received_file,
        demo_pawn_actor_count: unused_demo_pawn_actor_count,
        demo_pawn_actor_at: unused_demo_pawn_actor_at,
        demo_actor_is_player_start: unused_demo_actor_is_player_start,
        demo_pawn_spawn_spectator: unused_demo_pawn_spawn_spectator,
        demo_pawn_engine_client: unused_demo_pawn_engine_client,
        demo_client_viewport0: unused_demo_client_viewport0,
        demo_pawn_set_player: unused_demo_pawn_set_player,
        demo_pawn_set_admin: unused_demo_pawn_set_admin,
        demo_viewport_reset_input: unused_demo_viewport_reset_input,
        demo_pawn_copy_song: unused_demo_pawn_copy_song,
        demo_pawn_set_transition_fade: unused_demo_pawn_set_transition_fade,
        demo_viewport_console: unused_demo_viewport_console,
        demo_console_is_time_demo: unused_demo_console_is_time_demo,
        demo_dl_resolve_info: mock_dl_resolve_info,
        demo_info_package_flags: mock_info_package_flags,
        demo_info_file_size: mock_info_file_size,
        demo_info_url_chars: mock_info_url_chars,
        demo_info_parent_name: mock_info_parent_name,
        demo_info_guid_string: mock_info_guid_string,
        demo_chdl_set_package_index: mock_chdl_set_package_index,
        demo_chdl_ch_download: mock_chdl_ch_download,
        demo_chdl_set_ch_download: mock_chdl_set_ch_download,
        demo_dl_error_virtual: mock_dl_error_virtual,
        demo_dl_done_virtual: mock_dl_done_virtual,
    }
}

static SVC: LazyLock<Hp2NetDemoServices> = LazyLock::new(services);

// ---- driver ------------------------------------------------------------------------------------

/// Mirrors `reset_leaves()` and prints the harness `case` line.
fn begin_case(name: &str, conn: *mut UNetConnection) {
    with_mock(Mock::reset_leaves);
    unsafe { (*conn).channels = [null_mut(); 1023] };
    emit(format!("case {name}"));
}

/// Fresh zeroed download (`new FixtureDownload()`). Leaked for the test's lifetime.
fn new_download() -> *mut UDownload {
    Box::into_raw(Box::new(unsafe { zeroed::<UDownload>() }))
}

/// Fresh zeroed channel download (`new FixtureChannelDownload()`). Leaked for the test's lifetime.
fn new_chdl() -> *mut UChannelDownload {
    Box::into_raw(Box::new(unsafe { zeroed::<UChannelDownload>() }))
}

/// `print_state`: reads the download's own fields after the call.
///
/// # Safety
/// `d` is a live `UDownload`.
unsafe fn state_line(label: &str, d: *const UDownload) -> String {
    unsafe {
        let d = &*d;
        format!(
            "{label} state index={} transfered={} skipped={} compressed={} recv={} error='{}' temp='{}'",
            d.package_index,
            d.transfered,
            d.skipped_file,
            d.is_compressed,
            if d.recv_file_ar.is_null() {
                "null"
            } else {
                "set"
            },
            read_text(&d.error),
            read_text(&d.temp_filename),
        )
    }
}

/// `print_chdl_state`: `ch=set` or `ch=null`.
///
/// # Safety
/// `c` is a live `UChannelDownload`.
unsafe fn chdl_line(label: &str, c: *const UChannelDownload) -> String {
    let ch = unsafe { (*c).ch };
    format!(
        "{label} chdl ch={}",
        if ch.is_null() { "null" } else { "set" }
    )
}

#[test]
fn net_demo_fixture_matches_cpp() {
    reset();
    unsafe {
        let svc: &Hp2NetDemoServices = &SVC;
        let conn: *mut UNetConnection = Box::into_raw(Box::new(zeroed::<UNetConnection>()));
        let c = conn.cast::<c_void>();
        let payload = [0u8; 4096];
        let data = payload.as_ptr().cast_mut().cast::<c_void>();

        // Case 1: ReceiveFile binds the connection, index, and package info.
        begin_case("receive_file_index0", conn);
        let d0 = new_download();
        download_receive_file(svc, d0.cast(), c, 0).expect("ReceiveFile does not throw");
        emit(state_line("receive_file_index0", d0));

        // Case 2: two data calls on a required package; the second reuses the open writer.
        begin_case("receive_data_ok", conn);
        let d1 = new_download();
        download_receive_file(svc, d1.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d1.cast(), data, 100).expect("ReceiveData does not throw");
        download_receive_data(svc, d1.cast(), data, 50).expect("ReceiveData does not throw");
        emit(state_line("receive_data_ok", d1));

        // Case 3: writer creation fails, so the open error is recorded and nothing is written.
        begin_case("receive_data_open_fail", conn);
        with_mock(|m| m.create_writer_fails = true);
        let d2 = new_download();
        download_receive_file(svc, d2.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d2.cast(), data, 10).expect("ReceiveData does not throw");
        emit(state_line("receive_data_open_fail", d2));

        // Case 4: the second serialize marks the archive errored, so the write error is recorded.
        begin_case("receive_data_write_error", conn);
        with_mock(|m| m.error_on_write = 2);
        let d3 = new_download();
        download_receive_file(svc, d3.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d3.cast(), data, 10).expect("ReceiveData does not throw");
        download_receive_data(svc, d3.cast(), data, 10).expect("ReceiveData does not throw");
        emit(state_line("receive_data_write_error", d3));

        // Case 5: TrySkipFile on a client-optional package with an open writer skips it.
        begin_case("try_skip_optional", conn);
        let d4 = new_download();
        download_receive_file(svc, d4.cast(), c, 1).expect("ReceiveFile does not throw");
        download_receive_data(svc, d4.cast(), data, 10).expect("ReceiveData does not throw");
        let skip4 = download_try_skip_file(svc, d4.cast()).expect("TrySkipFile does not throw");
        emit(format!("try_skip_optional result={skip4}"));
        emit(state_line("try_skip_optional", d4));

        // Case 6: TrySkipFile on a required package does not skip.
        begin_case("try_skip_required", conn);
        let d5 = new_download();
        download_receive_file(svc, d5.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d5.cast(), data, 10).expect("ReceiveData does not throw");
        let skip5 = download_try_skip_file(svc, d5.cast()).expect("TrySkipFile does not throw");
        emit(format!("try_skip_required result={skip5}"));
        emit(state_line("try_skip_required", d5));

        // Case 7: DownloadDone after a skip reports the skipped file and deletes the temp file.
        begin_case("done_skipped", conn);
        download_done(svc, d4.cast()).expect("DownloadDone does not throw");
        emit(state_line("done_skipped", d4));

        // Case 8: DownloadDone with nothing received reports the refusal.
        begin_case("done_refused", conn);
        let d6 = new_download();
        download_receive_file(svc, d6.cast(), c, 0).expect("ReceiveFile does not throw");
        download_done(svc, d6.cast()).expect("DownloadDone does not throw");
        emit(state_line("done_refused", d6));

        // Case 9: DownloadDone on a complete required download moves the file and records success.
        begin_case("done_success", conn);
        let d7 = new_download();
        download_receive_file(svc, d7.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d7.cast(), data, 4096).expect("ReceiveData does not throw");
        download_done(svc, d7.cast()).expect("DownloadDone does not throw");
        emit(state_line("done_success", d7));

        // Case 10: DownloadDone with a size mismatch reports NetSize and deletes the temp file.
        begin_case("done_size_mismatch", conn);
        with_mock(|m| m.file_size = 1);
        let d8 = new_download();
        download_receive_file(svc, d8.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d8.cast(), data, 4096).expect("ReceiveData does not throw");
        download_done(svc, d8.cast()).expect("DownloadDone does not throw");
        emit(state_line("done_size_mismatch", d8));

        // Case 11: DownloadDone with a failed move reports NetMove.
        begin_case("done_move_fail", conn);
        with_mock(|m| m.move_ok = 0);
        let d9 = new_download();
        download_receive_file(svc, d9.cast(), c, 0).expect("ReceiveFile does not throw");
        download_receive_data(svc, d9.cast(), data, 4096).expect("ReceiveData does not throw");
        download_done(svc, d9.cast()).expect("DownloadDone does not throw");
        emit(state_line("done_move_fail", d9));

        // Case 12: Destroy with an open writer deletes the writer and the temp file, then detaches.
        begin_case("destroy_with_writer", conn);
        let d10 = new_download();
        download_receive_file(svc, d10.cast(), c, 0).expect("ReceiveFile does not throw");
        (*conn).download = d10.cast();
        download_receive_data(svc, d10.cast(), data, 10).expect("ReceiveData does not throw");
        download_destroy(svc, d10.cast()).expect("Destroy does not throw");
        emit(format!(
            "destroy_with_writer connection={} download={}",
            if (*d10).connection.is_null() {
                "null"
            } else {
                "set"
            },
            if (*conn).download.is_null() {
                "null"
            } else {
                "set"
            },
        ));

        // Case 13: UChannelDownload::ReceiveFile creates the file channel and sends the guid request.
        begin_case("chdl_receive_file", conn);
        let c1 = new_chdl();
        chdl_receive_file(svc, c1.cast(), c, 0).expect("ReceiveFile does not throw");
        emit(chdl_line("chdl_receive_file", c1));
        emit(state_line("chdl_receive_file", c1.cast()));

        // Case 14: channel creation fails, so ChAllocate is recorded and DownloadDone runs.
        begin_case("chdl_no_channel", conn);
        with_mock(|m| m.channel_fails = true);
        let c2 = new_chdl();
        chdl_receive_file(svc, c2.cast(), c, 0).expect("ReceiveFile does not throw");
        emit(chdl_line("chdl_no_channel", c2));
        emit(state_line("chdl_no_channel", c2.cast()));

        // Case 15: a channel download with a client-optional package and an open writer skips it.
        begin_case("chdl_try_skip", conn);
        let c3 = new_chdl();
        chdl_receive_file(svc, c3.cast(), c, 1).expect("ReceiveFile does not throw");
        download_receive_data(svc, c3.cast(), data, 10).expect("ReceiveData does not throw");
        let skip3 = chdl_try_skip_file(svc, c3.cast()).expect("TrySkipFile does not throw");
        emit(format!("chdl_try_skip result={skip3}"));
        emit(state_line("chdl_try_skip", c3.cast()));

        // Case 16: Destroy clears the channel's download back-reference.
        begin_case("chdl_destroy", conn);
        chdl_destroy(svc, c1.cast()).expect("Destroy does not throw");
        let ch = (*c1).ch;
        emit(format!(
            "chdl_destroy ch={}",
            if ch.is_null() { "null" } else { "set" }
        ));

        emit("download fixture ends");
    }

    let trace = with_mock(|m| std::mem::take(&mut m.trace));
    let got: Vec<&str> = trace.iter().map(String::as_str).collect();
    let want: Vec<&str> = FIXTURE.lines().collect();
    assert_eq!(want.len(), EXPECTED_RECORDS, "fixture record count");
    assert_eq!(got.len(), EXPECTED_RECORDS, "replayed record count");
    assert_eq!(got, want, "record sequence differs from fixture");

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for line in &got {
        let family = line.split(' ').next().unwrap_or("");
        *counts.entry(family).or_insert(0) += 1;
    }
    let expected: BTreeMap<&str, usize> = FAMILY_COUNTS.iter().copied().collect();
    assert_eq!(counts, expected, "per-family record counts");
}

include!("net_demo_download_cases.txt");
