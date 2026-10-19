//! Bit-exact scripted replay for the modern third-person input entrypoints
//! (`begin`, `finish`, `apply`) of `pawn_native_input`.
//!
//! The mock services mirror the `PawnInput*` hooks in
//! `Build/pawn_native_fixture_harness/pawn_native_input_harness.cpp`. The
//! scenario loop mirrors its `main`: 128 modern contexts (class mode x bit
//! variant x state) plus one modern-off context. The process-wide Rust state
//! (Harry cache, reported-failure flag, auto-center override) evolves in the
//! same order because this is the only test in the file.
use hp2_abi::{
    FVector,
    engine::generated::{AActor, APlayerPawn},
};
use hp2_engine::pawn_native_input::*;
use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::fmt::Write as _;
use std::sync::LazyLock;

const FIXTURE: &str = include_str!("fixtures/pawn_native_input.txt");
const NUM_BOOLS: usize = 13;
const BOOL_STRIDE: usize = 16;
const BOSS_IDX: usize = NUM_BOOLS;
const BOOL_NAMES: [&str; NUM_BOOLS] = [
    "bScreenRelativeMovement",
    "bLockedOnTarget",
    "bFixedFaceDirection",
    "bInDuelingMode",
    "bReverseInput",
    "bKeepStationary",
    "bLockOutForward",
    "bLockOutBackward",
    "bLockOutStrafeLeft",
    "bLockOutStrafeRight",
    "bIsAiming",
    "bE3DemoLockout",
    "bAutoCenterCamera",
];
const BITS_VARIANTS: [u32; 8] = [
    0,
    1 << 5,
    1 << 1,
    1 << 10,
    (1 << 6) | (1 << 8),
    (1 << 9) | (1 << 7),
    1 << 12,
    (1 << 2) | (1 << 4),
];
// Opaque class, state, and property tokens. Only identity matters.
const HARRY: usize = 0x1000;
const HARRY2: usize = 0x2000;
const BASE: usize = 0x3000;
const STATE_WALK: usize = 0x5000;
const STATE_BASE: usize = 0x5100;
const STATE_CAM: usize = 0x5200;
const PROP_BASE: usize = 0x100;
const PAWN_TAIL: usize = 512;
const SIN_TABLE_LEN: usize = 16384;
const FINISH_ROWS: [[f32; 6]; 2] = [
    [1.0, 0.5, -0.25, 0.3, -0.2, 0.016666668],
    [1.0, -0.9, 0.7, 12.5, 3.0, 0.25],
];

#[derive(Default)]
struct World {
    obj_class: usize,
    base_super: usize,
    missing_mask: u32,
    walk: bool,
    cam: bool,
    pawn: usize,
    view: usize,
    logs: usize,
    last_log: String,
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::default());
}

fn with<R>(f: impl FnOnce(&mut World) -> R) -> R {
    WORLD.with(|w| f(&mut w.borrow_mut()))
}

// TrigFLOAT[i] = appSin((FLOAT)i * 2.0f * PI / (FLOAT)NUM_ANGLES) in UnMath.cpp.
static TRIG: LazyLock<Vec<f32>> = LazyLock::new(|| {
    (0..SIN_TABLE_LEN)
        .map(|i| {
            let angle = i as f32 * 2.0f32 * std::f32::consts::PI / SIN_TABLE_LEN as f32;
            (angle as f64).sin() as f32
        })
        .collect()
});

fn cstr(p: *const c_char) -> &'static str {
    // SAFETY: the services adapters pass NUL-terminated names that outlive the call.
    unsafe { CStr::from_ptr(p) }.to_str().unwrap()
}

// Hooks mirroring the PawnInput* functions of the harness.
unsafe extern "C" fn find_bool(klass: *mut c_void, name: *const c_char) -> *mut c_void {
    let klass = klass as usize;
    if klass != HARRY && klass != HARRY2 {
        return std::ptr::null_mut();
    }
    let name = cstr(name);
    let idx = BOOL_NAMES
        .iter()
        .position(|&n| n == name)
        .unwrap_or_else(|| panic!("unknown bool property {name}"));
    let missing = with(|w| klass == HARRY2 && (w.missing_mask >> idx) & 1 != 0);
    if missing {
        std::ptr::null_mut()
    } else {
        (PROP_BASE + idx) as *mut c_void
    }
}

unsafe extern "C" fn find_object(klass: *mut c_void, name: *const c_char) -> *mut c_void {
    let klass = klass as usize;
    if klass != HARRY && klass != HARRY2 {
        return std::ptr::null_mut();
    }
    assert_eq!(cstr(name), "BossTarget");
    (PROP_BASE + BOSS_IDX) as *mut c_void
}

unsafe extern "C" fn property_offset(prop: *mut c_void) -> i32 {
    let idx = prop as usize - PROP_BASE;
    (std::mem::size_of::<APlayerPawn>() + BOOL_STRIDE * idx) as i32
}

unsafe extern "C" fn bool_property_mask(_prop: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn object_class(_obj: *mut c_void) -> *mut c_void {
    with(|w| w.obj_class) as *mut c_void
}

unsafe extern "C" fn class_super(klass: *mut c_void) -> *mut c_void {
    with(|w| {
        if klass as usize == BASE {
            w.base_super
        } else {
            0
        }
    }) as *mut c_void
}

unsafe extern "C" fn is_a(_obj: *mut c_void, klass: *mut c_void) -> i32 {
    with(|w| (klass as usize == w.obj_class) as i32)
}

unsafe extern "C" fn class_path_equals(klass: *mut c_void, path: *const c_char) -> i32 {
    let klass = klass as usize;
    ((klass == HARRY || klass == HARRY2) && cstr(path) == "HGame.Harry") as i32
}

unsafe extern "C" fn state_node(obj: *mut c_void) -> *mut c_void {
    with(|w| {
        let obj = obj as usize;
        if obj == w.pawn {
            if w.walk { STATE_WALK } else { 0 }
        } else if obj == w.view {
            if w.cam { STATE_CAM } else { 0 }
        } else {
            0
        }
    }) as *mut c_void
}

unsafe extern "C" fn state_super(state: *mut c_void) -> *mut c_void {
    (if state as usize == STATE_WALK {
        STATE_BASE
    } else {
        0
    }) as *mut c_void
}

unsafe extern "C" fn state_name_equals(state: *mut c_void, name: *const c_char) -> i32 {
    let expected = match state as usize {
        STATE_WALK => "PlayerWalking",
        STATE_BASE => "PlayerPawnBase",
        STATE_CAM => "StateStandardCam",
        _ => return 0,
    };
    (cstr(name) == expected) as i32
}

unsafe extern "C" fn log_missing_property(name: *const c_char) {
    with(|w| {
        w.logs += 1;
        w.last_log = cstr(name).to_string();
    });
}

unsafe extern "C" fn sin_tab(angle: i32) -> f32 {
    TRIG[((angle >> 2) & 16383) as usize]
}

unsafe extern "C" fn cos_tab(angle: i32) -> f32 {
    TRIG[((angle.wrapping_add(16384) >> 2) & 16383) as usize]
}

fn services() -> Hp2PawnNativeInputServices {
    Hp2PawnNativeInputServices {
        find_bool_property: find_bool,
        find_object_property: find_object,
        property_offset,
        bool_property_mask,
        object_class,
        class_super,
        is_a,
        class_path_equals,
        state_node,
        state_super,
        state_name_equals,
        log_missing_property,
        sin_tab,
        cos_tab,
    }
}

/// One pawn buffer (struct plus Harry bit tail) and its view target.
struct Rig {
    // Owners only: keep the allocations alive for `pawn` and `view_ptr`.
    _buf: Vec<u128>,
    _view: Box<AActor>,
    pawn: *mut APlayerPawn,
    view_ptr: *mut AActor,
}

impl Rig {
    fn new() -> Self {
        let bytes = std::mem::size_of::<APlayerPawn>() + PAWN_TAIL;
        let mut buf = vec![0u128; bytes.div_ceil(16)];
        let pawn = buf.as_mut_ptr().cast::<APlayerPawn>();
        // SAFETY: AActor is plain old data (pointers, scalars, bit words).
        let mut view: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
        let view_ptr: *mut AActor = &mut *view;
        with(|w| {
            w.pawn = pawn as usize;
            w.view = view_ptr as usize;
        });
        Self {
            _buf: buf,
            _view: view,
            pawn,
            view_ptr,
        }
    }

    fn bool_off(i: usize) -> usize {
        std::mem::size_of::<APlayerPawn>() + BOOL_STRIDE * i
    }

    fn read_u32(&self, off: usize) -> u32 {
        // SAFETY: offsets lie inside the buffer allocated in `new`.
        unsafe {
            (self.pawn.cast::<u8>().add(off))
                .cast::<u32>()
                .read_unaligned()
        }
    }

    fn write_u32(&mut self, off: usize, value: u32) {
        // SAFETY: offsets lie inside the buffer allocated in `new`.
        unsafe {
            (self.pawn.cast::<u8>().add(off))
                .cast::<u32>()
                .write_unaligned(value)
        }
    }

    fn flags(&self) -> u32 {
        (0..NUM_BOOLS).fold(0, |f, i| f | ((self.read_u32(Self::bool_off(i)) & 1) << i))
    }

    fn configure(&mut self, klass: usize, bits: u32, state: usize, modern: bool) {
        with(|w| {
            w.obj_class = BASE;
            w.base_super = 0;
            w.missing_mask = 0;
            match klass {
                0 => w.obj_class = HARRY,
                1 => w.base_super = HARRY,
                2 => w.base_super = 0,
                _ => {
                    w.base_super = HARRY2;
                    w.missing_mask = 1 << 3;
                }
            }
            w.walk = state != 1;
            w.cam = state != 2;
        });
        let boss = state == 3;
        let size = std::mem::size_of::<APlayerPawn>();
        // SAFETY: the buffer spans sizeof(APlayerPawn) + PAWN_TAIL bytes.
        unsafe {
            std::ptr::write_bytes(self.pawn.cast::<u8>(), 0, size + PAWN_TAIL);
            (*self.pawn).set_b_modern_third_person_controls(modern);
            (*self.pawn).view_target = self.view_ptr.cast::<c_void>();
            (*self.view_ptr).rotation.yaw = 0;
        }
        for i in 0..NUM_BOOLS {
            self.write_u32(Self::bool_off(i), (bits >> i) & 1);
        }
        if boss {
            let off = Self::bool_off(BOSS_IDX);
            // SAFETY: the boss slot is a pointer-sized field in the tail.
            unsafe {
                (self.pawn.cast::<u8>().add(off))
                    .cast::<usize>()
                    .write_unaligned(self.view_ptr as usize)
            };
        }
    }
}

#[test]
fn pawn_native_input_replay() {
    let svc = services();
    let mut rig = Rig::new();
    let mut out = String::new();

    // Context 128 disables modern controls; the other 128 cover klass x bits x state.
    for pass in 0..129usize {
        let modern = pass < 128;
        let klass = if modern { pass / 32 } else { 0 };
        let bits_idx = if modern { (pass / 4) % 8 } else { 0 };
        let state = if modern { pass % 4 } else { 0 };
        let bits = BITS_VARIANTS[bits_idx];
        let c = pass;

        // Begin: raw refs start at sentinels; only the gated-in path writes them.
        rig.configure(klass, bits, state, modern);
        // SAFETY: rig.pawn is the live buffer; the services stay valid for the call.
        unsafe {
            (*rig.pawn).a_base_x = 0.75;
            (*rig.pawn).a_strafe = -0.5;
            (*rig.pawn).a_turn = 0.25;
            (*rig.pawn).a_look_up = -0.125;
        }
        let mut raw = [7.0f32, 7.5, 8.0, 8.5];
        let before = with(|w| w.logs);
        // SAFETY: rig.pawn is a live APlayerPawn; svc matches the mock hooks.
        let ret = unsafe { begin_modern_third_person_input(&svc, rig.pawn, &mut raw) };
        let dlogs = with(|w| w.logs) - before;
        let p = rig.pawn;
        // SAFETY: as above.
        let (base_x, strafe, turn, look_up) =
            unsafe { ((*p).a_base_x, (*p).a_strafe, (*p).a_turn, (*p).a_look_up) };
        write!(
            out,
            "begin ctx={c:03} k={klass} bits={bits_idx:02} state={state} modern={} ret={} raw={:08x}:{:08x}:{:08x}:{:08x} pawn={:08x}:{:08x}:{:08x}:{:08x} flags={:04x} logs={dlogs}",
            modern as u32,
            ret as u32,
            raw[0].to_bits(),
            raw[1].to_bits(),
            raw[2].to_bits(),
            raw[3].to_bits(),
            base_x.to_bits(),
            strafe.to_bits(),
            turn.to_bits(),
            look_up.to_bits(),
            rig.flags(),
        )
        .unwrap();
        if dlogs > 0 {
            write!(out, " missing={}", with(|w| w.last_log.clone())).unwrap();
        }
        out.push('\n');

        // Finish: two input vectors, each from a freshly configured pawn.
        for (v, row) in FINISH_ROWS.iter().enumerate() {
            rig.configure(klass, bits, state, modern);
            // SAFETY: rig.pawn is a live APlayerPawn.
            unsafe {
                (*rig.pawn).a_forward = 0.6;
                (*rig.pawn).a_side_move = 0.2;
                (*rig.pawn).a_strafe = 0.1;
                (*rig.pawn).a_base_x = 0.3;
                (*rig.pawn).a_turn = 0.0;
                (*rig.pawn).a_look_up = 0.0;
                (*rig.pawn).smooth_mouse_x = 0.125;
                (*rig.pawn).smooth_mouse_y = -0.5;
                (*rig.pawn).desired_fov = 90.0;
            }
            let before = with(|w| w.logs);
            // SAFETY: rig.pawn is a live APlayerPawn; svc matches the mock hooks.
            unsafe {
                finish_modern_third_person_input(
                    &svc,
                    rig.pawn,
                    row[0] != 0.0,
                    row[1],
                    row[2],
                    row[3],
                    row[4],
                    row[5],
                );
            }
            let dlogs = with(|w| w.logs) - before;
            let p = rig.pawn;
            // SAFETY: as above.
            let (fwd, strafe, side, base_x, smooth_x, smooth_y) = unsafe {
                (
                    (*p).a_forward,
                    (*p).a_strafe,
                    (*p).a_side_move,
                    (*p).a_base_x,
                    (*p).smooth_mouse_x,
                    (*p).smooth_mouse_y,
                )
            };
            write!(
                out,
                "finish ctx={c:03} vec={v} aforward={:08x} astrafe={:08x} aside={:08x} abasex={:08x} smoothx={:08x} smoothy={:08x} flags={:04x} logs={dlogs}",
                fwd.to_bits(),
                strafe.to_bits(),
                side.to_bits(),
                base_x.to_bits(),
                smooth_x.to_bits(),
                smooth_y.to_bits(),
                rig.flags(),
            )
            .unwrap();
            if dlogs > 0 {
                write!(out, " missing={}", with(|w| w.last_log.clone())).unwrap();
            }
            out.push('\n');
        }

        // Apply: bit 0 (screen-relative) is set so the movement gate can pass.
        rig.configure(klass, bits | 1, state, modern);
        let zero_move = bits_idx == 7;
        // SAFETY: rig.pawn is a live APlayerPawn.
        unsafe {
            (*rig.pawn).a_forward = if zero_move { 0.0 } else { 0.7 };
            (*rig.pawn).a_side_move = if zero_move { 0.0 } else { -0.4 };
            (*rig.pawn).base.base.velocity = if bits_idx == 5 {
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }
            } else {
                FVector {
                    x: 120.0,
                    y: -30.0,
                    z: 50.0,
                }
            };
            (*rig.pawn).base.base.rotation.yaw = 12345;
            (*rig.pawn).base.base.desired_rotation.yaw = 1111;
            (*rig.view_ptr).rotation.yaw = 9000;
        }
        let before = with(|w| w.logs);
        // SAFETY: rig.pawn is a live APlayerPawn; svc matches the mock hooks.
        unsafe { apply_modern_third_person_movement(&svc, rig.pawn) };
        let dlogs = with(|w| w.logs) - before;
        let p = rig.pawn;
        // SAFETY: as above.
        let (acc, desired) = unsafe {
            (
                (*p).base.base.acceleration,
                (*p).base.base.desired_rotation.yaw,
            )
        };
        writeln!(
            out,
            "apply ctx={c:03} acc={:08x}:{:08x}:{:08x} desired={desired} flags={:04x} logs={dlogs}",
            acc.x.to_bits(),
            acc.y.to_bits(),
            acc.z.to_bits(),
            rig.flags(),
        )
        .unwrap();
    }

    let count = |prefix: &str| out.lines().filter(|l| l.starts_with(prefix)).count();
    assert_eq!(count("begin "), 129, "begin record count");
    assert_eq!(count("finish "), 258, "finish record count");
    assert_eq!(count("apply "), 129, "apply record count");
    assert_eq!(out.lines().count(), 516, "total record count");
    assert_eq!(
        out, FIXTURE,
        "pawn_native_input replay diverged from fixture"
    );
}
