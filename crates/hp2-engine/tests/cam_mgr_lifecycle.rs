//! Lifecycle sequencing contracts for the Rust-owned non-reflection UClient policy.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, transmute};

use hp2_engine::cam_mgr::{self, CamMgrServices};

thread_local! {
    static EVENTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn emit(event: impl Into<String>) {
    EVENTS.with(|events| events.borrow_mut().push(event.into()));
}

fn take_events() -> Vec<String> {
    EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
}

unsafe extern "C" fn unexpected_service() {
    panic!("unexpected CamMgr service call")
}

unsafe extern "C" fn bitmap(client: *mut c_void) -> i32 {
    emit(format!("bitmap:{client:p}"));
    0
}
unsafe extern "C" fn clear() -> i32 {
    emit("clear");
    0
}
unsafe extern "C" fn maintain(_client: *mut c_void, value: i32) -> i32 {
    emit(format!("maintain:{value}"));
    0
}
unsafe extern "C" fn native_text(_client: *mut c_void, value: i32) -> i32 {
    emit(format!("native:{value}"));
    0
}
unsafe extern "C" fn native_text_failure(_client: *mut c_void, value: i32) -> i32 {
    emit(format!("native:{value}"));
    9
}
unsafe extern "C" fn show_fps(_client: *mut c_void, value: i32) -> i32 {
    emit(format!("fps:{value}"));
    0
}
unsafe extern "C" fn engine(client: *mut c_void, value: *mut c_void) -> i32 {
    emit(format!("engine:{client:p}:{value:p}"));
    0
}
unsafe extern "C" fn archive(client: *mut c_void, value: *mut c_void) -> i32 {
    emit(format!("archive:{client:p}:{value:p}"));
    0
}

fn services(native: unsafe extern "C" fn(*mut c_void, i32) -> i32) -> CamMgrServices {
    const WORDS: usize = size_of::<CamMgrServices>() / size_of::<usize>();
    let filler = unexpected_service as unsafe extern "C" fn() as usize;
    let table: [usize; WORDS] = [filler; WORDS];
    // SAFETY: the service table contains function pointers only. Every entry invoked by
    // these tests is replaced below; all others intentionally fail immediately.
    let mut svc: CamMgrServices = unsafe { transmute(table) };
    svc.client_set_bitmap_client = bitmap;
    svc.client_clear_bitmap_client = clear;
    svc.client_set_maintain_vertical_fov = maintain;
    svc.client_set_native_text = native;
    svc.client_set_show_fps = show_fps;
    svc.client_set_engine = engine;
    svc.client_serialize_viewports = archive;
    svc
}

#[test]
fn client_lifecycle_preserves_constructor_default_and_teardown_order() {
    let svc = services(native_text);
    let client = 0x1000usize as *mut c_void;
    let engine_ptr = 0x2000usize as *mut c_void;
    let archive_ptr = 0x3000usize as *mut c_void;

    unsafe { cam_mgr::construct(&svc, client) }.unwrap();
    unsafe { cam_mgr::set_static_defaults(&svc, client) }.unwrap();
    unsafe { cam_mgr::init(&svc, client, engine_ptr) }.unwrap();
    unsafe { cam_mgr::serialize_viewports(&svc, client, archive_ptr) }.unwrap();
    unsafe { cam_mgr::destroy(&svc) }.unwrap();

    assert_eq!(
        take_events(),
        [
            "bitmap:0x1000",
            "maintain:1",
            "native:1",
            "fps:0",
            "engine:0x1000:0x2000",
            "archive:0x1000:0x3000",
            "clear",
        ]
    );
}

#[test]
fn default_failure_stops_before_later_publication() {
    let svc = services(native_text_failure);
    assert_eq!(
        unsafe { cam_mgr::set_static_defaults(&svc, 0x1000usize as *mut c_void) },
        Err(-9)
    );
    assert_eq!(take_events(), ["maintain:1", "native:1"]);
}
