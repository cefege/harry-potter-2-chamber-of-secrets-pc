//! Standalone executable probe. Run exactly one scenario in each process.
//! `picker.rs` must be an unmodified copy of the source under examination.
#[path = "picker.rs"]
mod picker;

use std::ffi::{CStr, c_char, c_int};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

unsafe extern "C" {
    fn pthread_main_np() -> c_int;
    fn fake_setup(result: c_int, callback_stage: c_int, init_fail: c_int);
    fn fake_count() -> c_int;
    fn fake_event(index: c_int) -> c_int;
    fn fake_policy() -> c_int;
    fn fake_callbacks() -> c_int;
    fn fake_last_format() -> *const c_char;
    fn fake_filter_count() -> u32;
    fn fake_filter_spec() -> *const c_char;
}

static NESTED_FILE: AtomicBool = AtomicBool::new(false);
static NESTED_RESULT: AtomicI32 = AtomicI32::new(-1);

// The fake shim calls this synchronously. Do not assert or explicitly panic
// across the C ABI; inspect the captured result after the outer call returns.
#[unsafe(no_mangle)]
pub extern "C" fn picker_test_nested() {
    let result = std::panic::catch_unwind(|| {
        if NESTED_FILE.load(Ordering::SeqCst) {
            picker::pick_file(&["zip"])
        } else {
            picker::pick_folder()
        }
    });
    let observed = result.map_or(-2, |value| i32::from(value.is_some()));
    NESTED_RESULT.store(observed, Ordering::SeqCst);
}

fn pick(file: bool) -> Option<Vec<u8>> {
    if file {
        picker::pick_file(&["zip"])
    } else {
        picker::pick_folder()
    }
}

fn events() -> Vec<i32> {
    // SAFETY: The fake shim's atomic event count takes no arguments.
    let count = unsafe { fake_count() };
    (0..count)
        // SAFETY: The shim checks every index and reads an atomic slot.
        .map(|index| unsafe { fake_event(index) })
        .collect()
}

fn main() {
    let scenario = std::env::args().nth(1).expect("scenario required");
    let fields: Vec<_> = scenario.split('-').collect();
    if scenario == "pthread" {
        // SAFETY: pthread_main_np takes no arguments and reports this thread.
        assert_eq!(unsafe { pthread_main_np() }, 1, "process entry thread");
        let child = std::thread::spawn(|| {
            // SAFETY: The same argument-free query is valid on the child.
            unsafe { pthread_main_np() }
        })
        .join()
        .unwrap();
        assert_eq!(child, 0, "child thread");
        return;
    }
    let file = *fields.last().expect("picker kind") == "file";
    assert!(
        file || *fields.last().unwrap() == "folder",
        "bad picker kind"
    );
    let dialog = if file { 3 } else { 2 };
    let cross_kind = fields.get(1) == Some(&"cross");
    NESTED_FILE.store(if cross_kind { !file } else { file }, Ordering::SeqCst);
    NESTED_RESULT.store(-1, Ordering::SeqCst);
    let initial = fields[0];
    let stage_name = if cross_kind {
        fields.get(2)
    } else {
        fields.get(1)
    };
    let (response, stage, fail) = match initial {
        "success" | "filter" => (1, 0, 0),
        "cancel" => (2, 0, 0),
        "error" => (0, 0, 0),
        "null" => (3, 0, 0),
        "initfail" => (1, 0, 1),
        "initlog" => (1, 7, 1),
        "wrong" => (2, 0, 0),
        "nested" => match stage_name.copied() {
            Some("dialog") => (1, dialog, 0),
            Some("free") => (1, 4, 0),
            Some("quit") => (1, 5, 0),
            Some("log") => (0, 7, 0),
            _ => panic!("bad nested stage"),
        },
        _ => panic!("bad scenario"),
    };
    // SAFETY: The fake shim resets its own state before any picker call.
    unsafe { fake_setup(response, stage, fail) };
    if initial == "wrong" {
        let result = std::thread::spawn(move || pick(file)).join().unwrap();
        assert_eq!(result, None);
        assert_eq!(
            events(),
            Vec::<i32>::new(),
            "wrong-thread call reached fake NFD"
        );
        return;
    }
    let result = if initial == "filter" {
        assert!(file);
        match fields.get(1).copied() {
            Some("empty") => picker::pick_file(&[]),
            Some("order") => picker::pick_file(&["zip", "", "tar.gz", "7z"]),
            Some("nul") => picker::pick_file(&["zip\0tar"]),
            _ => panic!("bad filter scenario"),
        }
    } else {
        pick(file)
    };
    if initial == "filter" && fields.get(1) == Some(&"nul") {
        assert_eq!(result, None);
        assert_eq!(events(), Vec::<i32>::new(), "embedded NUL reached fake NFD");
        return;
    }
    if initial == "initfail" || initial == "initlog" {
        assert_eq!(result, None);
        let retry = pick(file);
        assert_eq!(retry, Some(vec![b'x', 0xff, b'z']));
        let expected = if initial == "initlog" {
            vec![1, 6, 7, 8, 1, dialog, 4, 5]
        } else {
            vec![1, 6, 7, 1, dialog, 4, 5]
        };
        assert_eq!(
            events(),
            expected,
            "unexpected NFD operations or cleanup order"
        );
    } else {
        let expected_result = if response == 1 {
            Some(vec![b'x', 0xff, b'z'])
        } else {
            None
        };
        assert_eq!(result, expected_result);
        let expected = match (initial, stage) {
            ("nested", s) if s == dialog => vec![1, dialog, 8, 4, 5],
            ("nested", 4) => vec![1, dialog, 4, 8, 5],
            ("nested", 5) => vec![1, dialog, 4, 5, 8],
            ("nested", 7) => vec![1, dialog, 6, 7, 8, 5],
            ("success" | "filter", _) => vec![1, dialog, 4, 5],
            ("cancel" | "null", _) => vec![1, dialog, 5],
            ("error", _) => vec![1, dialog, 6, 7, 5],
            _ => unreachable!(),
        };
        assert_eq!(
            events(),
            expected,
            "unexpected NFD operations or cleanup order"
        );
    }
    // SAFETY: This argument-free fake-shim getter reads an atomic value.
    assert_eq!(
        unsafe { fake_policy() },
        0,
        "activation policy was not restored"
    );
    if initial == "nested" || initial == "initlog" {
        // SAFETY: This argument-free fake-shim getter reads an atomic value.
        assert_eq!(unsafe { fake_callbacks() }, 1);
        assert_eq!(
            NESTED_RESULT.load(Ordering::SeqCst),
            0,
            "nested picker admitted"
        );
    }
    if initial == "filter" {
        // SAFETY: The fake shim owns a persistent NUL-terminated buffer and
        // no callbacks run while this result is inspected.
        let spec = unsafe { CStr::from_ptr(fake_filter_spec()) }.to_bytes();
        // SAFETY: This argument-free fake-shim getter reads an atomic value.
        let count = unsafe { fake_filter_count() };
        if fields.get(1) == Some(&"empty") {
            assert_eq!((count, spec), (0, &b""[..]));
        } else {
            assert_eq!((count, spec), (1, &b"zip,tar.gz,7z"[..]));
        }
    }
    if initial == "error"
        || (initial == "nested" && stage == 7)
        || initial == "initfail"
        || initial == "initlog"
    {
        // SAFETY: The fake shim returns its persistent NUL-terminated buffer;
        // no concurrent fake logger runs while this value is borrowed.
        let actual = unsafe { CStr::from_ptr(fake_last_format()) }.to_bytes();
        let expected: &[u8] = if initial == "initfail" || initial == "initlog" {
            b"HP2Launcher: native file dialog initialization failed: %s"
        } else if file {
            b"HP2Launcher: file picker failed: %s"
        } else {
            b"HP2Launcher: folder picker failed: %s"
        };
        assert_eq!(actual, expected);
    }
}
