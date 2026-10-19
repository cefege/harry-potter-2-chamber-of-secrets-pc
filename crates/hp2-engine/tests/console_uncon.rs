//! Replays the original `UConsole` (UnCon.cpp @ 4914c7e) fixture through the
//! Rust port. `tests/fixtures/console_uncon.txt` is produced by
//! `Build/console_fixture_harness/gen_console.sh` from the unmodified TU; this
//! test drives `hp2_engine::console` with the same scenario and mock services
//! that print the same lines, then compares byte-for-byte.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char, c_void};

use hp2_abi::engine::generated::{FSceneNode, UConsole};
use hp2_engine::console::{self, Hp2ConsoleServices};

const EXPECTED: &str = include_str!("fixtures/console_uncon.txt");
const EXPECTED_RECORDS: usize = 12116;

/// `TEXT("...")` under `HP2_HOST_WCHAR_TCHAR`: one `u32` per character.
const fn wide<const N: usize>(s: &[u8]) -> [u32; N] {
    let mut out = [0u32; N];
    let mut i = 0;
    while i < s.len() {
        out[i] = s[i] as u32;
        i += 1;
    }
    out
}

static ENGINE_TEXT: [u32; 12] = wide(b"Engine|Core");
static COPYRIGHT_TEXT: [u32; 15] = wide(b"Copyright|Core");
static HELLO: [u32; 6] = wide(b"hello");
static OPEN_MAP: [u32; 9] = wide(b"open map");
static BOGUS: [u32; 6] = wide(b"bogus");
static FPS_DUMP: [u32; 9] = wide(b"fps dump");
static ARIAL: [u32; 6] = wide(b"Arial");
static TINY: [u32; 5] = wide(b"Tiny");
/// Stand-in for a `UFont` returned by `CreateNativeFont`.
static FONT_OBJECT: u64 = 0;

thread_local! {
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
    static VIEWPORT: Cell<usize> = const { Cell::new(0) };
    static CONSOLE: Cell<usize> = const { Cell::new(0) };
    static DELTA: Cell<f32> = const { Cell::new(0.0) };
    static EXEC_RET: Cell<i32> = const { Cell::new(0) };
    static FONT_SET: Cell<bool> = const { Cell::new(true) };
}

fn emit(line: String) {
    OUT.with_borrow_mut(|o| {
        o.push_str(&line);
        o.push('\n');
    });
}

/// `FTime(float)` (`Core.h:49`, `FromSeconds`): `FIXTIME` is `4294967296.f` and
/// `long double` is 64-bit on arm64, so the scale is exact in `f64`. The
/// saturating `as` cast matches the clamps at `Core.h:43-46`.
fn ftime_from_seconds(seconds: f32) -> i64 {
    (f64::from(seconds) * 4_294_967_296.0) as i64
}

/// `FTime::operator-` (`Core.h:53`): `(v - t.v) / FIXTIME`. The i64 difference
/// rounds to `float` before the exact power-of-two divide.
fn ftime_delta(current: f32, last: f32) -> f32 {
    let diff = ftime_from_seconds(current).wrapping_sub(ftime_from_seconds(last));
    diff as f32 / 4_294_967_296.0_f32
}

/// Reads a NUL-terminated `TCHAR*` (`wchar_t`, 4 bytes) as text.
///
/// # Safety
/// `p` must point to a NUL-terminated array of `u32` code units.
unsafe fn wstr(p: *const c_void) -> String {
    let mut s = String::new();
    let mut q = p.cast::<u32>();
    loop {
        let c = unsafe { *q };
        if c == 0 {
            break;
        }
        s.push(char::from_u32(c).expect("valid code point"));
        q = unsafe { q.add(1) };
    }
    s
}

unsafe extern "C" fn m_init_execution(_console: *mut c_void) {
    emit("init_exec".into());
}

unsafe extern "C" fn m_localize_general(
    key: *const c_char,
    package: *const c_void,
) -> *const c_void {
    assert_eq!(unsafe { wstr(package) }, "Core");
    match unsafe { CStr::from_ptr(key) }.to_bytes() {
        b"Engine" => ENGINE_TEXT.as_ptr().cast(),
        b"Copyright" => COPYRIGHT_TEXT.as_ptr().cast(),
        other => panic!("unexpected localize key {other:?}"),
    }
}

unsafe extern "C" fn m_log_format(_console: *mut c_void, format: *const c_void) {
    emit(format!("log {}", unsafe { wstr(format) }));
}

unsafe extern "C" fn m_event_message(_console: *mut c_void, data: *const c_void, name: i32) {
    emit(format!("event name={name} data={}", unsafe { wstr(data) }));
}

unsafe extern "C" fn m_event_tick(_console: *mut c_void, delta_time: f32) {
    emit(format!("tick {:08x}", delta_time.to_bits()));
}

unsafe extern "C" fn m_viewport_exec(
    viewport: *mut c_void,
    cmd: *const c_void,
    console: *mut c_void,
) -> i32 {
    let ret = EXEC_RET.get();
    emit(format!(
        "exec vp_ok={} ar_console={} cmd={} ret={ret}",
        i32::from(viewport as usize == VIEWPORT.get()),
        i32::from(console as usize == CONSOLE.get()),
        unsafe { wstr(cmd) },
    ));
    ret
}

unsafe extern "C" fn m_viewport_create_native_font(
    viewport: *mut c_void,
    name: *const c_void,
    height: i32,
) -> *mut c_void {
    assert_eq!(viewport as usize, VIEWPORT.get());
    let set = FONT_SET.get();
    emit(format!(
        "font name={} height={height} -> {}",
        unsafe { wstr(name) },
        if set { "set" } else { "null" },
    ));
    if set {
        (&FONT_OBJECT as *const u64).cast_mut().cast()
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "C" fn m_viewport_time_delta(_viewport: *mut c_void) -> f32 {
    DELTA.get()
}

unsafe extern "C" fn m_save_string_to_file(string: *const c_void, filename: *const c_void) {
    emit(format!(
        "save filename={} string={}",
        unsafe { wstr(filename) },
        unsafe { wstr(string) },
    ));
}

unsafe extern "C" fn m_frame_compute_render_size(frame: *mut c_void) {
    let f = unsafe { &*frame.cast::<FSceneNode>() };
    emit(format!(
        "compute x={} y={} xb={} yb={}",
        f.x, f.y, f.xb, f.yb
    ));
}

static SVC: Hp2ConsoleServices = Hp2ConsoleServices {
    init_execution: m_init_execution,
    localize_general: m_localize_general,
    log_format: m_log_format,
    event_message: m_event_message,
    event_tick: m_event_tick,
    viewport_exec: m_viewport_exec,
    viewport_create_native_font: m_viewport_create_native_font,
    viewport_time_delta: m_viewport_time_delta,
    save_string_to_file: m_save_string_to_file,
    frame_compute_render_size: m_frame_compute_render_size,
};

/// `execConsoleCommand` through the port, storing the `UBOOL` result as the
/// C++ native would (`*(DWORD*)Result`).
unsafe fn run_exec(cp: *mut UConsole, cmd: &[u32], ret: i32) {
    EXEC_RET.set(ret);
    let result = unsafe { console::exec_command(&SVC, cp, cmd.as_ptr().cast()) } as u32;
    emit(format!("exec_result={result}"));
}

/// `execCreateNativeFont` through the port.
unsafe fn run_font(cp: *mut UConsole, name: &[u32], height: i32, set: bool) {
    FONT_SET.set(set);
    let out = unsafe { console::create_native_font(&SVC, cp, name.as_ptr().cast(), height) };
    emit(format!(
        "font_result={}",
        if out.is_null() { "null" } else { "set" }
    ));
}

#[test]
fn console_uncon_replays_original_fixture() {
    const BORDERS: [i32; 9] = [-3, 0, 1, 2, 3, 4, 5, 6, 7];
    const POS: [f32; 6] = [-0.5, 0.0, 0.25, 0.5, 1.0, 1.5];
    const FRAMES: [[i32; 4]; 7] = [
        [640, 480, 0, 0],
        [800, 600, 10, 20],
        [1024, 768, -4, 0],
        [0, 0, 0, 0],
        [-1, -2, 3, 4],
        [7, 9, 0, 0],
        [4096, 2160, 1, 2],
    ];
    // (CurrentTime, LastUpdateTime) pairs, converted as `FTime(float)` then differenced.
    const TIMES: [[f32; 2]; 4] = [[1.5, 0.25], [0.125, 0.125], [1000.3, 999.7], [0.1, 0.3]];

    // Opaque UViewport: the port only forwards its address.
    let mut viewport = [0u64; 8];
    let mut console: UConsole = unsafe { std::mem::zeroed() };
    let mut frame: FSceneNode = unsafe { std::mem::zeroed() };
    let vp: *mut c_void = viewport.as_mut_ptr().cast();
    let cp: *mut UConsole = &mut console;
    let fp: *mut FSceneNode = &mut frame;
    VIEWPORT.set(vp as usize);
    CONSOLE.set(cp as usize);

    unsafe {
        // _Init.
        console::init(&SVC, cp, vp);
        emit(format!(
            "init top={} border={} vp={}",
            (*cp).top_line,
            (*cp).border_size,
            i32::from((*cp).viewport == vp),
        ));

        // Serialize -> eventMessage.
        for name in [0, 1, 37] {
            console::serialize(&SVC, cp, HELLO.as_ptr().cast(), name);
        }

        // execConsoleCommand.
        run_exec(cp, &OPEN_MAP, 1);
        run_exec(cp, &BOGUS, 0);

        // execSaveTimeDemo.
        console::save_time_demo(&SVC, FPS_DUMP.as_ptr().cast());

        // execCreateNativeFont.
        run_font(cp, &ARIAL, 12, true);
        run_font(cp, &TINY, 6, false);

        // GetDrawWorld.
        for nd in 0..=1 {
            (*cp).set_b_no_draw_world(nd == 1);
            emit(format!(
                "draw_world nd={nd} ret={}",
                console::get_draw_world(cp)
            ));
        }

        // PreRender / PostRender grid.
        for bs in BORDERS {
            for pos in POS {
                for f in FRAMES {
                    for nd in 0..=1 {
                        for t in TIMES {
                            (*cp).border_size = bs;
                            (*cp).console_pos = pos;
                            (*cp).set_b_no_draw_world(nd == 1);
                            DELTA.set(ftime_delta(t[0], t[1]));
                            (*fp).x = f[0];
                            (*fp).y = f[1];
                            (*fp).xb = f[2];
                            (*fp).yb = f[3];

                            console::pre_render(&SVC, cp, fp);
                            emit(format!(
                                "pre bs={bs} cp={:08x} out={},{},{},{} cl={} bl={} bp={}",
                                pos.to_bits(),
                                (*fp).x,
                                (*fp).y,
                                (*fp).xb,
                                (*fp).yb,
                                (*cp).console_lines,
                                (*cp).border_lines,
                                (*cp).border_pixels,
                            ));
                            console::post_render(cp, fp);
                            emit(format!(
                                "post fx={:08x} fy={:08x} drew={} frame={},{},{},{}",
                                (*cp).frame_x.to_bits(),
                                (*cp).frame_y.to_bits(),
                                i32::from((*cp).b_drew_world()),
                                (*fp).x,
                                (*fp).y,
                                (*fp).xb,
                                (*fp).yb,
                            ));
                        }
                    }
                }
            }
        }
    }

    let out = OUT.with_borrow(Clone::clone);
    assert_eq!(out.lines().count(), EXPECTED_RECORDS, "record count");
    assert_eq!(
        EXPECTED.lines().count(),
        EXPECTED_RECORDS,
        "fixture record count"
    );
    assert_eq!(
        out, EXPECTED,
        "console replay diverges from original fixture"
    );
}
