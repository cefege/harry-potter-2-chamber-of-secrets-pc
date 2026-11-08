//! First-failure propagation through the checked input policy.
//! Synthetic unit adapters are deliberately separate from genuine native proof.
//! Runs every callback failure in an isolated process by default.
use hp2_abi::engine::generated::{AActor, APlayerPawn};
use hp2_engine::pawn_native_input::*;
use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::sync::LazyLock;

const CALLBACK_FAILURES: [&str; 12] = [
    "find_bool",
    "find_object",
    "property_offset",
    "bool_property_mask",
    "object_class",
    "class_super",
    "is_a",
    "class_path_equals",
    "state_node",
    "state_super",
    "state_name_equals",
    "log_missing_property",
];

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
    fault: String,
    fault_hits: usize,
    after_fault: usize,
    failed_bytes: Vec<u8>,
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

fn checkpoint(name: &str) -> i32 {
    with(|w| {
        if w.fault_hits != 0 {
            w.after_fault += 1;
        }
        if w.fault == name && w.fault_hits == 0 {
            w.fault_hits += 1;
            w.failed_bytes = unsafe {
                std::slice::from_raw_parts(
                    w.pawn as *const u8,
                    std::mem::size_of::<APlayerPawn>() + PAWN_TAIL,
                )
            }
            .to_vec();
            return 71;
        }
        0
    })
}

// Checked fault-return unit adapters; these are not native consumer evidence.
unsafe extern "C" fn find_bool(
    klass: *mut c_void,
    name: *const c_char,
    out: *mut *mut c_void,
) -> i32 {
    if checkpoint("find_bool") != 0 {
        return 71;
    }
    let value = (|| {
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
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn find_object(
    klass: *mut c_void,
    name: *const c_char,
    out: *mut *mut c_void,
) -> i32 {
    if checkpoint("find_object") != 0 {
        return 71;
    }
    let value = (|| {
        let klass = klass as usize;
        if klass != HARRY && klass != HARRY2 {
            return std::ptr::null_mut();
        }
        assert_eq!(cstr(name), "BossTarget");
        (PROP_BASE + BOSS_IDX) as *mut c_void
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn property_offset(prop: *mut c_void, out: *mut i32) -> i32 {
    if checkpoint("property_offset") != 0 {
        return 71;
    }
    let value = (|| {
        let idx = prop as usize - PROP_BASE;
        (std::mem::size_of::<APlayerPawn>() + BOOL_STRIDE * idx) as i32
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn bool_property_mask(_prop: *mut c_void, out: *mut u32) -> i32 {
    if checkpoint("bool_property_mask") != 0 {
        return 71;
    }
    let value = (|| 1)();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn object_class(_obj: *mut c_void, out: *mut *mut c_void) -> i32 {
    if checkpoint("object_class") != 0 {
        return 71;
    }
    let value = (|| with(|w| w.obj_class) as *mut c_void)();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn class_super(klass: *mut c_void, out: *mut *mut c_void) -> i32 {
    if checkpoint("class_super") != 0 {
        return 71;
    }
    let value = (|| {
        with(|w| {
            if klass as usize == BASE {
                w.base_super
            } else {
                0
            }
        }) as *mut c_void
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn is_a(_obj: *mut c_void, klass: *mut c_void, out: *mut i32) -> i32 {
    if checkpoint("is_a") != 0 {
        return 71;
    }
    let value = (|| with(|w| (klass as usize == w.obj_class) as i32))();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn class_path_equals(
    klass: *mut c_void,
    path: *const c_char,
    out: *mut i32,
) -> i32 {
    if checkpoint("class_path_equals") != 0 {
        return 71;
    }
    let value = (|| {
        let klass = klass as usize;
        ((klass == HARRY || klass == HARRY2) && cstr(path) == "HGame.Harry") as i32
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn state_node(obj: *mut c_void, out: *mut *mut c_void) -> i32 {
    if checkpoint("state_node") != 0 {
        return 71;
    }
    let value = (|| {
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
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn state_super(state: *mut c_void, out: *mut *mut c_void) -> i32 {
    if checkpoint("state_super") != 0 {
        return 71;
    }
    let value = (|| {
        (if state as usize == STATE_WALK {
            STATE_BASE
        } else {
            0
        }) as *mut c_void
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn state_name_equals(
    state: *mut c_void,
    name: *const c_char,
    out: *mut i32,
) -> i32 {
    if checkpoint("state_name_equals") != 0 {
        return 71;
    }
    let value = (|| {
        if with(|w| w.fault == "state_super") {
            return 0;
        }
        let expected = match state as usize {
            STATE_WALK => "PlayerWalking",
            STATE_BASE => "PlayerPawnBase",
            STATE_CAM => "StateStandardCam",
            _ => return 0,
        };
        (cstr(name) == expected) as i32
    })();
    unsafe {
        *out = value;
    }
    0
}

unsafe extern "C" fn log_missing_property(name: *const c_char) -> i32 {
    if checkpoint("log_missing_property") != 0 {
        return 71;
    }
    with(|w| {
        w.logs += 1;
        w.last_log = cstr(name).to_string();
    });

    0
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
fn checked_native_input_first_failure() {
    let target = match std::env::var("HP2_INPUT_UNIT_FAULT") {
        Ok(target) => target,
        Err(_) => {
            let executable = std::env::current_exe().unwrap();
            for target in CALLBACK_FAILURES {
                let output = std::process::Command::new(&executable)
                    .args([
                        "--exact",
                        "checked_native_input_first_failure",
                        "--nocapture",
                    ])
                    .env("HP2_INPUT_UNIT_FAULT", target)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{target}: status {}\n{}\n{}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
    };
    let svc = services();
    let mut rig = Rig::new();
    rig.configure(0, 0, 0, true);
    let mut raw = [7.0, 8.0, 9.0, 10.0];
    unsafe {
        if target == "is_a" {
            begin_modern_third_person_input(
                &svc,
                rig.pawn,
                raw.as_mut_ptr(),
                raw.as_mut_ptr().add(1),
                raw.as_mut_ptr().add(2),
                raw.as_mut_ptr().add(3),
            )
            .unwrap();
        }
        (*rig.pawn).a_base_x = 0.75;
        (*rig.pawn).a_strafe = -0.5;
        (*rig.pawn).a_turn = 0.25;
        (*rig.pawn).a_look_up = -0.125;
    }
    with(|w| {
        if target == "class_super" {
            w.obj_class = BASE;
            w.base_super = HARRY;
        }
        if target == "log_missing_property" {
            w.obj_class = HARRY2;
            w.missing_mask = 1 << 3;
        }
        w.fault = target.clone();
    });
    let result = unsafe {
        begin_modern_third_person_input(
            &svc,
            rig.pawn,
            raw.as_mut_ptr(),
            raw.as_mut_ptr().add(1),
            raw.as_mut_ptr().add(2),
            raw.as_mut_ptr().add(3),
        )
    };
    assert_eq!(
        result,
        Err(71),
        "first checked failure propagates: {target}"
    );
    assert_eq!(
        raw,
        [0.75, -0.5, 0.25, -0.125],
        "pre-reflection native argument assignments retained"
    );
    with(|w| {
        assert_eq!(w.fault_hits, 1, "requested fault reached: {target}");
        assert_eq!(w.after_fault, 0, "no callback after failure: {target}");
        let current = unsafe {
            std::slice::from_raw_parts(
                w.pawn as *const u8,
                std::mem::size_of::<APlayerPawn>() + PAWN_TAIL,
            )
        };
        assert_eq!(
            current,
            w.failed_bytes.as_slice(),
            "no policy writes after failure: {target}"
        );
    });
}
