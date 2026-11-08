use hp2_core::ansi::Tchar;
use hp2_core::file_persistence::*;
use std::ffi::c_void;

struct Host {
    input: Vec<u8>,
    written: Vec<u8>,
    scratch: Vec<u8>,
    binary: Vec<u8>,
    text: Vec<Tchar>,
    size: i32,
    opened: i32,
    error: i32,
    closed: i32,
    writing: bool,
    events: Vec<&'static str>,
    counts: Vec<i32>,
    fail_at: Option<usize>,
}

impl Host {
    fn new(input: &[u8]) -> Self {
        Self {
            input: input.into(),
            written: vec![],
            scratch: vec![],
            binary: vec![9, 8],
            text: vec![77, 0],
            size: input.len() as i32,
            opened: 1,
            error: 0,
            closed: 1,
            writing: false,
            events: vec![],
            counts: vec![],
            fail_at: None,
        }
    }
    fn event(&mut self, event: &'static str) -> i32 {
        let index = self.events.len();
        self.events.push(event);
        if self.fail_at == Some(index) { 73 } else { 0 }
    }
    fn context(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
    fn load_text(&mut self) -> Result<i32, i32> {
        unsafe { load_string(&SERVICES, self.context()) }
    }
    fn save_text(&mut self, text: &[Tchar]) -> Result<i32, i32> {
        unsafe { save_string(&SERVICES, self.context(), text.as_ptr(), text.len() as i32) }
    }
    fn load_binary(&mut self) -> Result<i32, i32> {
        unsafe { load_array(&SERVICES, self.context()) }
    }
    fn save_binary(&mut self) -> Result<i32, i32> {
        unsafe { save_array(&SERVICES, self.context(), self.binary.len() as i32) }
    }
}

unsafe extern "C" fn open(c: *mut c_void, writing: i32, out: *mut i32) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event(if writing != 0 { "writer" } else { "reader" });
    if status == 0 {
        h.writing = writing != 0;
        unsafe { *out = h.opened };
    }
    status
}
unsafe extern "C" fn size(c: *mut c_void, out: *mut i32) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event("size");
    if status == 0 {
        unsafe { *out = h.size };
    }
    status
}
unsafe extern "C" fn serialize(c: *mut c_void, data: *mut u8, count: i32) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event("serialize");
    h.counts.push(count);
    if status == 0 {
        if h.writing {
            h.written = unsafe { std::slice::from_raw_parts(data, count as usize) }.into();
        } else {
            unsafe { std::ptr::copy_nonoverlapping(h.input.as_ptr(), data, count as usize) };
        }
    }
    status
}
unsafe extern "C" fn is_error(c: *mut c_void, out: *mut i32) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event("error");
    if status == 0 {
        unsafe { *out = h.error };
    }
    status
}
unsafe extern "C" fn close(c: *mut c_void, out: *mut i32) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event("close");
    if status == 0 {
        unsafe { *out = h.closed };
    }
    status
}
unsafe extern "C" fn destroy(c: *mut c_void) -> i32 {
    unsafe { &mut *c.cast::<Host>() }.event("destroy")
}
unsafe extern "C" fn delete_file(c: *mut c_void) -> i32 {
    unsafe { &mut *c.cast::<Host>() }.event("delete")
}
unsafe extern "C" fn scratch(c: *mut c_void, count: i32, out: *mut *mut u8) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event("scratch");
    h.counts.push(count);
    if status == 0 {
        h.scratch.resize(count as usize, 0xa5);
        unsafe { *out = h.scratch.as_mut_ptr() };
    }
    status
}
unsafe extern "C" fn binary(c: *mut c_void, count: i32, resize: i32, out: *mut *mut u8) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event(if resize != 0 {
        "binary_set"
    } else {
        "binary_data"
    });
    h.counts.push(count);
    if status != 0 {
        return status;
    }
    if count < 0 {
        return 91;
    } // Native Set owns invalid-size behavior, not Rust.
    if resize != 0 {
        h.binary.resize(count as usize, 0);
    }
    unsafe { *out = h.binary.as_mut_ptr() };
    0
}
unsafe extern "C" fn text(c: *mut c_void, count: i32, out: *mut *mut c_void) -> i32 {
    let h = unsafe { &mut *c.cast::<Host>() };
    let status = h.event("text");
    h.counts.push(count);
    if status == 0 {
        h.text.clear();
        h.text.resize(count as usize, 0x55);
        unsafe { *out = h.text.as_mut_ptr().cast() };
    }
    status
}
unsafe extern "C" fn from_ansi(value: u8) -> u32 {
    u32::from(value)
}
unsafe extern "C" fn from_unicode(value: u16) -> u32 {
    u32::from(value)
}
unsafe extern "C" fn to_ansi(value: u32) -> u8 {
    value as u8
}

const SERVICES: Hp2FilePersistenceServices = Hp2FilePersistenceServices {
    open,
    size,
    serialize,
    is_error,
    close,
    destroy,
    delete_file,
    scratch,
    binary,
    text,
    from_ansi,
    from_unicode,
    to_ansi,
};

#[test]
fn ansi_preserves_all_bytes_embedded_nuls_and_does_not_decode_utf8() {
    let bytes: Vec<u8> = (0..=255).collect();
    let mut h = Host::new(&bytes);
    assert_eq!(h.load_text(), Ok(1));
    let expected: Vec<Tchar> = bytes.iter().map(|&b| b as Tchar).chain([0]).collect();
    assert_eq!(h.text, expected);
    assert_eq!(h.counts, [258, 256, 257]);
    assert_eq!(&h.scratch[256..], &[0, 0]);
    assert_eq!(
        h.events,
        [
            "reader",
            "size",
            "scratch",
            "serialize",
            "error",
            "close",
            "destroy",
            "text"
        ]
    );
    let mut h = Host::new(&[0xef, 0xbb, 0xbf, 0xc3, 0xa9]);
    assert_eq!(h.load_text(), Ok(1));
    assert_eq!(h.text, [0xef, 0xbb, 0xbf, 0xc3, 0xa9, 0]);
}

fn utf16(little: bool, units: &[u16]) -> Vec<u8> {
    let mut bytes = if little {
        vec![0xff, 0xfe]
    } else {
        vec![0xfe, 0xff]
    };
    for &unit in units {
        bytes.extend(if little {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    bytes
}

#[test]
fn utf16_both_endians_scalar_boundaries_and_embedded_nuls() {
    let units = [
        0, 0xff, 0x100, 0xd7ff, 0xe000, 0xffff, 0xd800, 0xdc00, 0xdbff, 0xdfff, 0x41,
    ];
    for little in [true, false] {
        let mut h = Host::new(&utf16(little, &units));
        assert_eq!(h.load_text(), Ok(1));
        assert_eq!(
            h.text,
            [
                0, 0xff, 0x100, 0xd7ff, 0xe000, 0xffff, 0x10000, 0x10ffff, 0x41, 0
            ]
        );
        assert_eq!(h.counts.last(), Some(&10));
    }
}

#[test]
fn malformed_utf16_never_partially_mutates_destination() {
    for little in [true, false] {
        for units in [
            &[0x41, 0xd800][..],
            &[0x41, 0xd800, 0x42],
            &[0x41, 0xdc00],
            &[0xdbff, 0xdbff],
            &[0xdc00, 0xd800],
        ] {
            let mut h = Host::new(&utf16(little, units));
            assert_eq!(h.load_text(), Ok(0), "{units:x?}");
            assert_eq!(h.text, [77, 0]);
            assert!(!h.events.contains(&"text"));
            assert_eq!(&h.events[h.events.len() - 2..], &["close", "destroy"]);
        }
        let mut odd = utf16(little, &[0x41]);
        odd.push(0);
        let mut h = Host::new(&odd);
        assert_eq!(h.load_text(), Ok(0));
        assert_eq!(h.text, [77, 0]);
    }
}

#[test]
fn empty_files_bom_only_and_single_bom_byte_keep_native_storage_counts() {
    for bytes in [vec![], vec![0xff, 0xfe], vec![0xfe, 0xff]] {
        let mut h = Host::new(&bytes);
        assert_eq!(h.load_text(), Ok(1));
        assert_eq!(h.text, [0]);
        assert_eq!(h.events.contains(&"serialize"), !bytes.is_empty());
    }
    let mut h = Host::new(&[0xff]);
    assert_eq!(h.load_text(), Ok(1));
    assert_eq!(h.text, [0xff, 0]);
}

#[test]
fn text_load_size_and_archive_failures_preserve_output() {
    for invalid in [-1, i32::MIN, i32::MAX - 1, i32::MAX] {
        let mut h = Host::new(&[]);
        h.size = invalid;
        assert_eq!(h.load_text(), Ok(0));
        assert_eq!(h.events, ["reader", "size", "close", "destroy"]);
        assert_eq!(h.text, [77, 0]);
    }
    for (error, closed) in [(1, 1), (0, 0), (1, 0)] {
        let mut h = Host::new(b"abc");
        h.error = error;
        h.closed = closed;
        assert_eq!(h.load_text(), Ok(0));
        assert_eq!(h.text, [77, 0]);
        assert_eq!(
            h.events,
            [
                "reader",
                "size",
                "scratch",
                "serialize",
                "error",
                "close",
                "destroy"
            ]
        );
    }
}

#[test]
fn saves_ansi_through_ff_without_bom_and_unicode_as_utf16_le() {
    let text: Vec<Tchar> = (0..=255).collect();
    let mut h = Host::new(&[]);
    assert_eq!(h.save_text(&text), Ok(1));
    assert_eq!(h.written, (0..=255).collect::<Vec<u8>>());
    assert_eq!(
        h.events,
        [
            "scratch",
            "writer",
            "serialize",
            "error",
            "close",
            "destroy"
        ]
    );
    assert_eq!(h.counts, [256, 256]);
    let mut h = Host::new(&[]);
    assert_eq!(
        h.save_text(&[0x41, 0, 0x100, 0xd7ff, 0xe000, 0xffff, 0x10000, 0x10ffff]),
        Ok(1)
    );
    assert_eq!(
        h.written,
        utf16(
            true,
            &[
                0x41, 0, 0x100, 0xd7ff, 0xe000, 0xffff, 0xd800, 0xdc00, 0xdbff, 0xdfff
            ]
        )
    );
}

#[test]
fn save_rejects_empty_or_invalid_scalar_before_allocation_or_open() {
    for text in [
        vec![],
        vec![0xd800],
        vec![0xdfff],
        vec![0x110000],
        vec![u32::MAX as Tchar],
        vec![65, 0x100, 0xdc00],
        vec![0xd800, 0xdc00],
    ] {
        let mut h = Host::new(&[]);
        assert_eq!(h.save_text(&text), Ok(0));
        assert!(h.events.is_empty());
    }
}

#[test]
fn text_save_closes_and_destroys_before_deleting_on_either_error() {
    for (error, closed) in [(0, 0), (1, 1), (1, 0)] {
        let mut h = Host::new(&[]);
        h.error = error;
        h.closed = closed;
        assert_eq!(h.save_text(&[65]), Ok(0));
        assert_eq!(
            h.events,
            [
                "scratch",
                "writer",
                "serialize",
                "error",
                "close",
                "destroy",
                "delete"
            ]
        );
    }
    let mut h = Host::new(&[]);
    h.closed = 7;
    assert_eq!(h.save_text(&[65]), Ok(1));
    assert!(!h.events.contains(&"delete"));
}

#[test]
fn open_failure_stops_at_native_precedence_points() {
    for operation in 0..4 {
        let mut h = Host::new(&[65]);
        h.opened = 0;
        let result = match operation {
            0 => h.load_text(),
            1 => h.load_binary(),
            2 => h.save_text(&[0x100]),
            _ => h.save_binary(),
        };
        assert_eq!(result, Ok(0));
        assert_eq!(
            h.events,
            match operation {
                0 | 1 => vec!["reader"],
                2 => vec!["scratch", "writer"],
                _ => vec!["writer"],
            }
        );
        assert_eq!(h.text, [77, 0]);
        assert_eq!(h.binary, [9, 8]);
    }
}

#[test]
fn binary_legacy_load_ignores_error_and_returns_raw_close_while_save_only_destroys() {
    let mut h = Host::new(&[1, 0, 0xff]);
    h.error = 1;
    h.closed = 7;
    assert_eq!(h.load_binary(), Ok(7));
    assert_eq!(h.binary, [1, 0, 0xff]);
    assert_eq!(
        h.events,
        [
            "reader",
            "size",
            "binary_set",
            "serialize",
            "close",
            "destroy"
        ]
    );
    let mut h = Host::new(&[1]);
    h.closed = 0;
    assert_eq!(h.load_binary(), Ok(0));
    assert_eq!(h.binary, [1]);
    let mut h = Host::new(&[]);
    h.error = 1;
    h.closed = 0;
    assert_eq!(h.save_binary(), Ok(1));
    assert_eq!(h.written, [9, 8]);
    assert_eq!(h.events, ["writer", "binary_data", "serialize", "destroy"]);
}

#[test]
fn binary_empty_and_invalid_size_still_reach_native_array_leaf() {
    let mut h = Host::new(&[]);
    assert_eq!(h.load_binary(), Ok(1));
    assert_eq!(h.counts, [0, 0]);
    assert!(h.events.contains(&"serialize"));
    let mut h = Host::new(&[]);
    h.binary.clear();
    assert_eq!(h.save_binary(), Ok(1));
    assert_eq!(h.counts, [0, 0]);
    let mut h = Host::new(&[]);
    h.size = -1;
    assert_eq!(h.load_binary(), Err(91));
    assert_eq!(h.events, ["reader", "size", "binary_set"]);
    assert_eq!(h.counts, [-1]);
}

#[test]
fn every_native_exception_status_stops_policy_without_followup_callbacks() {
    for operation in 0..6 {
        let run = |h: &mut Host| match operation {
            0 => h.load_text(),
            1 => h.save_text(&[0x100]),
            2 => h.load_binary(),
            3 => h.save_binary(),
            4 => {
                h.error = 1;
                h.save_text(&[65])
            }
            _ => {
                h.size = -1;
                h.load_text()
            }
        };
        let mut baseline = Host::new(&[65]);
        assert!(run(&mut baseline).is_ok());
        for index in 0..baseline.events.len() {
            let mut h = Host::new(&[65]);
            h.fail_at = Some(index);
            assert_eq!(
                run(&mut h),
                Err(73),
                "operation {operation} callback {index}"
            );
            assert_eq!(h.events, baseline.events[..=index]);
        }
    }
}

#[test]
fn conversion_services_are_used_only_for_their_native_codepoint_paths() {
    unsafe extern "C" fn ansi_in(value: u8) -> u32 {
        u32::from(value) + 0x1000
    }
    unsafe extern "C" fn unicode_in(value: u16) -> u32 {
        u32::from(value) + 1
    }
    unsafe extern "C" fn ansi_out(value: u32) -> u8 {
        (value as u8) ^ 0x80
    }
    let services = Hp2FilePersistenceServices {
        from_ansi: ansi_in,
        from_unicode: unicode_in,
        to_ansi: ansi_out,
        ..SERVICES
    };
    let mut h = Host::new(&[0xff]);
    assert_eq!(unsafe { load_string(&services, h.context()) }, Ok(1));
    assert_eq!(h.text, [0x10ff, 0]);
    let mut h = Host::new(&utf16(true, &[0x41, 0xd800, 0xdc00]));
    assert_eq!(unsafe { load_string(&services, h.context()) }, Ok(1));
    assert_eq!(h.text, [0x42, 0x10000, 0]);
    let mut h = Host::new(&[]);
    assert_eq!(
        unsafe { save_string(&services, h.context(), [0xff].as_ptr(), 1) },
        Ok(1)
    );
    assert_eq!(h.written, [0x7f]);
}
