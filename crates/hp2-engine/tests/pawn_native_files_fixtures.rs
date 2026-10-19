//! Replays the file, URL, skin, map, console and screen natives against the
//! oracle fixture `fixtures/pawn_native_files.txt`.
//!
//! The fixture is produced by `Build/pawn_native_files_fixture_harness`, which
//! compiles the oracle `UnPawn.cpp` with the real flags and routes each engine
//! call through a `PawnFiles*` hook. Each hook here mirrors that harness hook
//! exactly: it pushes the same `fx` tag and returns the same value from the
//! same world state. The C++ shell glue (registry, cache section iteration) is
//! mirrored in the replay functions below.

use hp2_abi::FVector;
use hp2_abi::engine::generated::FRotator;
use hp2_engine::pawn_native_files::{self as files, Hp2PawnNativeFilesServices};
use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::ptr;
use std::sync::LazyLock;

const FIXTURE: &str = include_str!("fixtures/pawn_native_files.txt");
const SIN_TABLE_LEN: usize = 16384;

// TrigFLOAT[i] = appSin((FLOAT)i * 2.0f * PI / (FLOAT)NUM_ANGLES) in UnMath.cpp.
static TRIG: LazyLock<Vec<f32>> = LazyLock::new(|| {
    (0..SIN_TABLE_LEN)
        .map(|i| {
            let angle = i as f32 * 2.0f32 * std::f32::consts::PI / SIN_TABLE_LEN as f32;
            (angle as f64).sin() as f32
        })
        .collect()
});

/// Harness pointer codes: 0 is null, 1 is `ActorB`, 2 is `Goal`.
const CODE_ACTOR_B: u8 = 1;
const CODE_GOAL: u8 = 2;
const CODE_ENTRY_LEVEL: u8 = 3;

struct World {
    fx: Vec<String>,
    arena: Vec<Box<[u16]>>,
    registry: Vec<(String, String)>,
    skins: Vec<(String, String)>,
    ini: Vec<(String, String)>,
    cache_section: bool,
    url_defaults: Vec<(String, String)>,
    files: Vec<(String, Vec<String>)>,
    found: Vec<String>,
    cd_path: String,
    paths: Vec<String>,
    has_gameengine: bool,
    has_serverpkg: bool,
    defaults_num: i32,
    export_text: String,
    move_ok: i32,
    has_player: bool,
    has_viewport: bool,
    has_input: bool,
    vp: (i32, i32),
    pawn_fov: f32,
    maintain: i32,
    eff_fov: f32,
    view_loc: FVector,
    view_rot: FRotator,
    exec_out: String,
    best: u8,
    best_weight: f32,
    probing: i32,
    pause: f32,
    shoot: i32,
    goal: u8,
    url_map: String,
}

fn fresh() -> World {
    World {
        fx: Vec::new(),
        arena: Vec::new(),
        registry: vec![
            ("Harry.A".into(), "Desc A".into()),
            ("Harry.B".into(), "Desc B".into()),
            ("Harry.C".into(), "Desc C".into()),
        ],
        skins: vec![
            ("HarryA".into(), "dA".into()),
            ("HarryB".into(), "dB".into()),
            ("Other".into(), "dO".into()),
            ("HarryC".into(), "dC".into()),
        ],
        ini: Vec::new(),
        cache_section: true,
        url_defaults: vec![("Name".into(), "Val".into()), ("Other".into(), "X".into())],
        files: Vec::new(),
        found: Vec::new(),
        cd_path: String::new(),
        paths: Vec::new(),
        has_gameengine: true,
        has_serverpkg: true,
        defaults_num: 1,
        export_text: "HARRYSKINS,OTHER".into(),
        move_ok: 1,
        has_player: true,
        has_viewport: true,
        has_input: true,
        vp: (0, 0),
        pawn_fov: 90.0,
        maintain: 0,
        eff_fov: 90.0,
        view_loc: FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        view_rot: FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        },
        exec_out: String::new(),
        best: 0,
        best_weight: 0.0,
        probing: 0,
        pause: 7.5,
        shoot: 1,
        goal: 0,
        url_map: String::new(),
    }
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(fresh());
}

fn setup(f: impl FnOnce(&mut World)) {
    WORLD.with_borrow_mut(|w| {
        *w = fresh();
        f(w);
    });
}

fn push(s: impl Into<String>) {
    let s = s.into();
    WORLD.with_borrow_mut(|w| w.fx.push(s));
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Copies `s` into a NUL-terminated buffer that outlives the call.
fn keep(s: &str) -> *const u16 {
    let mut units = wide(s);
    units.push(0);
    let boxed = units.into_boxed_slice();
    let p = boxed.as_ptr();
    WORLD.with_borrow_mut(|w| w.arena.push(boxed));
    p
}

/// Reads a NUL-terminated buffer as a `String`.
///
/// # Safety
/// `p` must be null or a readable NUL-terminated buffer.
unsafe fn s16(p: *const u16) -> String {
    // SAFETY: the caller guarantees a readable NUL-terminated buffer.
    String::from_utf16_lossy(&unsafe { files::units(p) })
}

fn hf(x: f32) -> String {
    format!("{:08x}", x.to_bits())
}

fn qs(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn code_ptr(code: u8) -> *mut c_void {
    ptr::without_provenance_mut(code as usize)
}

fn set_ini(w: &mut World, key: &str, value: &str) {
    match w.ini.iter_mut().find(|(k, _)| k == key) {
        Some(entry) => entry.1 = value.to_owned(),
        None => w.ini.push((key.to_owned(), value.to_owned())),
    }
}

// ---------------------------------------------------------------- hooks

unsafe extern "C" fn cache_get_string(guid: *const u16) -> *const u16 {
    // SAFETY: the Rust helpers pass NUL-terminated keys.
    let key = unsafe { s16(guid) };
    push(format!("cacheget:{key}"));
    let found = WORLD.with_borrow(|w| {
        w.ini
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
    });
    match found {
        Some(value) => keep(&value),
        None => ptr::null(),
    }
}

unsafe extern "C" fn cache_clear(guid: *const u16) {
    // SAFETY: the Rust helpers pass NUL-terminated keys.
    let key = unsafe { s16(guid) };
    push(format!("cacheset:{key}="));
    WORLD.with_borrow_mut(|w| set_ini(w, &key, ""));
}

unsafe extern "C" fn cache_path() -> *const u16 {
    keep("Cache")
}

unsafe extern "C" fn cache_ext() -> *const u16 {
    keep(".tmp")
}

unsafe extern "C" fn path_separator() -> *const u16 {
    keep("/")
}

unsafe extern "C" fn paths_count() -> i32 {
    WORLD.with_borrow(|w| w.paths.len() as i32)
}

unsafe extern "C" fn path_at(i: i32) -> *const u16 {
    let path = WORLD.with_borrow(|w| w.paths[i as usize].clone());
    keep(&path)
}

unsafe extern "C" fn file_move(dest: *const u16, src: *const u16) -> i32 {
    // SAFETY: the Rust helpers pass NUL-terminated paths.
    let (dest, src) = unsafe { (s16(dest), s16(src)) };
    let ok = WORLD.with_borrow(|w| w.move_ok);
    push(format!("move:{src}->{dest}={}", i32::from(ok != 0)));
    ok
}

unsafe extern "C" fn log_no_extension(name: *const u16) {
    // SAFETY: the Rust helpers pass a NUL-terminated name.
    let name = unsafe { s16(name) };
    push(format!("log:MoveCacheEntry: No extension: {name}"));
}

unsafe extern "C" fn log_move(src: *const u16, dest: *const u16) {
    // SAFETY: the Rust helpers pass NUL-terminated paths.
    let (src, dest) = unsafe { (s16(src), s16(dest)) };
    push(format!("log:MoveCacheEntry: {src} -> {dest}"));
}

unsafe extern "C" fn url_default_option(option: *const u16) -> *const u16 {
    // SAFETY: the Rust helpers pass a NUL-terminated option.
    let opt = unsafe { s16(option) };
    push("loadcfg:DefaultPlayer/User");
    push(format!("getopt:{opt}"));
    let value = WORLD.with_borrow(|w| {
        w.url_defaults
            .iter()
            .find(|(k, _)| format!("{k}=") == opt)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    });
    keep(&value)
}

unsafe extern "C" fn url_add_option(_this: *mut c_void, text: *const u16) {
    // SAFETY: the Rust helpers pass a NUL-terminated option text.
    let text = unsafe { s16(text) };
    push(format!("addopt:{text}"));
}

unsafe extern "C" fn url_save_default(_this: *mut c_void, option: *const u16) {
    // SAFETY: the Rust helpers pass a NUL-terminated option name.
    let option = unsafe { s16(option) };
    push(format!("save:DefaultPlayer/{option}/User"));
}

unsafe extern "C" fn server_packages() -> *const u16 {
    push("findclass:engine.gameengine");
    if !WORLD.with_borrow(|w| w.has_gameengine) {
        return ptr::null();
    }
    push("findfield:serverpackages");
    if !WORLD.with_borrow(|w| w.has_serverpkg) {
        return ptr::null();
    }
    let text = WORLD.with_borrow(|w| {
        if w.defaults_num != 0 {
            Some(w.export_text.clone())
        } else {
            None
        }
    });
    match text {
        Some(text) => {
            push("exporttext");
            keep(&text)
        }
        None => keep(""),
    }
}

unsafe extern "C" fn find_files(pattern: *const u16) -> i32 {
    // SAFETY: the Rust helpers pass a NUL-terminated pattern.
    let pattern = unsafe { s16(pattern) };
    push(format!("find:{pattern}"));
    let names = WORLD.with_borrow(|w| {
        w.files
            .iter()
            .find(|(p, _)| *p == pattern)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    });
    let count = names.len() as i32;
    WORLD.with_borrow_mut(|w| w.found = names);
    count
}

unsafe extern "C" fn found_file_at(i: i32) -> *const u16 {
    let name = WORLD.with_borrow(|w| w.found[i as usize].clone());
    keep(&name)
}

unsafe extern "C" fn map_ext() -> *const u16 {
    keep("unr")
}

unsafe extern "C" fn cd_path() -> *const u16 {
    let cd = WORLD.with_borrow(|w| w.cd_path.clone());
    keep(&cd)
}

unsafe extern "C" fn find_path_toward_best(
    _this: *mut c_void,
    _weight: f32,
    predict: i32,
    new_path: *mut *mut c_void,
) -> f32 {
    push(format!(
        "findpath:clear=1,predict={}",
        i32::from(predict != 0)
    ));
    let (best, best_weight) = WORLD.with_borrow(|w| (w.best, w.best_weight));
    // SAFETY: the Rust helper passes a valid out slot.
    unsafe { *new_path = code_ptr(best) };
    best_weight
}

unsafe extern "C" fn reset_special_state(_this: *mut c_void) {
    WORLD.with_borrow_mut(|w| {
        w.pause = 0.0;
        w.shoot = 0;
    });
}

unsafe extern "C" fn is_probing_special(_path: *mut c_void) -> i32 {
    push("probing");
    WORLD.with_borrow(|w| w.probing)
}

unsafe extern "C" fn handle_special(_this: *mut c_void, _path: *mut c_void) {
    push("handle");
}

unsafe extern "C" fn special_goal(_this: *mut c_void) -> *mut c_void {
    code_ptr(WORLD.with_borrow(|w| w.goal))
}

unsafe extern "C" fn clear_special_goal(_this: *mut c_void) {
    WORLD.with_borrow_mut(|w| w.goal = 0);
}

unsafe extern "C" fn keyboard_config_class(_this: *mut c_void) -> *mut c_void {
    push("castviewport");
    let (vp, input) = WORLD.with_borrow(|w| (w.has_viewport, w.has_input));
    if vp && input {
        code_ptr(1)
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn reset_config(_klass: *mut c_void) {
    push("reset");
}

unsafe extern "C" fn has_player(_this: *mut c_void) -> i32 {
    i32::from(WORLD.with_borrow(|w| w.has_player))
}

unsafe extern "C" fn player_exec(_this: *mut c_void, command: *const u16) -> *const u16 {
    // SAFETY: the Rust helpers pass a NUL-terminated command.
    let cmd = unsafe { s16(command) };
    push(format!("exec_player:{cmd}"));
    let out = WORLD.with_borrow(|w| w.exec_out.clone());
    keep(&out)
}

unsafe extern "C" fn engine_exec(_this: *mut c_void, command: *const u16) -> *const u16 {
    // SAFETY: the Rust helpers pass a NUL-terminated command.
    let cmd = unsafe { s16(command) };
    push(format!("exec_engine:{cmd}"));
    let out = WORLD.with_borrow(|w| w.exec_out.clone());
    keep(&out)
}

unsafe extern "C" fn viewport_size(_this: *mut c_void, size_x: *mut i32, size_y: *mut i32) {
    let (x, y) = WORLD.with_borrow(|w| w.vp);
    // SAFETY: the Rust helper passes valid out slots.
    unsafe {
        *size_x = x;
        *size_y = y;
    }
}

unsafe extern "C" fn pawn_fov(_this: *mut c_void) -> f32 {
    WORLD.with_borrow(|w| w.pawn_fov)
}

unsafe extern "C" fn maintain_vertical_fov(_this: *mut c_void) -> i32 {
    WORLD.with_borrow(|w| w.maintain)
}

unsafe extern "C" fn effective_fov(fov: f32, size_x: i32, size_y: i32, maintain: i32) -> f32 {
    push(format!(
        "eff:auth={},x={size_x},y={size_y},m={maintain}",
        hf(fov)
    ));
    WORLD.with_borrow(|w| w.eff_fov)
}

unsafe extern "C" fn pawn_location(_this: *mut c_void, out: *mut c_void) {
    // SAFETY: the Rust helper passes a valid FVector slot.
    unsafe {
        *out.cast::<FVector>() = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    };
}

unsafe extern "C" fn pawn_rotation(_this: *mut c_void, out: *mut c_void) {
    // SAFETY: the Rust helper passes a valid FRotator slot.
    unsafe {
        *out.cast::<FRotator>() = FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        }
    };
}

unsafe extern "C" fn calc_view(_this: *mut c_void, loc: *mut c_void, rot: *mut c_void) {
    push("calcview");
    let (view_loc, view_rot) = WORLD.with_borrow(|w| {
        (
            FVector {
                x: w.view_loc.x,
                y: w.view_loc.y,
                z: w.view_loc.z,
            },
            FRotator {
                pitch: w.view_rot.pitch,
                yaw: w.view_rot.yaw,
                roll: w.view_rot.roll,
            },
        )
    });
    // SAFETY: the Rust helper passes valid FVector and FRotator slots.
    unsafe {
        *loc.cast::<FVector>() = view_loc;
        *rot.cast::<FRotator>() = view_rot;
    }
}

unsafe extern "C" fn sin_tab(angle: i32) -> f32 {
    TRIG[((angle >> 2) & 16383) as usize]
}

unsafe extern "C" fn cos_tab(angle: i32) -> f32 {
    TRIG[((angle.wrapping_add(16384) >> 2) & 16383) as usize]
}

unsafe extern "C" fn last_url_map(_this: *mut c_void) -> *const u16 {
    push("lasturl");
    let map = WORLD.with_borrow(|w| w.url_map.clone());
    keep(&map)
}

unsafe extern "C" fn entry_level(_this: *mut c_void) -> *mut c_void {
    push("entrylevel");
    code_ptr(CODE_ENTRY_LEVEL)
}

fn services() -> Hp2PawnNativeFilesServices {
    Hp2PawnNativeFilesServices {
        cache_get_string,
        cache_clear,
        cache_path,
        cache_ext,
        path_separator,
        paths_count,
        path_at,
        file_move,
        log_no_extension,
        log_move,
        url_default_option,
        url_add_option,
        url_save_default,
        server_packages,
        find_files,
        found_file_at,
        map_ext,
        cd_path,
        find_path_toward_best,
        reset_special_state,
        is_probing_special,
        handle_special,
        special_goal,
        clear_special_goal,
        keyboard_config_class,
        reset_config,
        has_player,
        player_exec,
        engine_exec,
        viewport_size,
        pawn_fov,
        maintain_vertical_fov,
        effective_fov,
        pawn_location,
        pawn_rotation,
        calc_view,
        sin_tab,
        cos_tab,
        last_url_map,
        entry_level,
    }
}

// ---------------------------------------------------------------- replay

type Out = Vec<String>;

fn rec(out: &mut Out, entry: &str, case: &str, body: &str) {
    let fx = WORLD
        .with_borrow_mut(|w| std::mem::take(&mut w.fx))
        .join(";");
    out.push(format!("entry={entry} case={case} {body} fx={}", qs(&fx)));
}

/// `GetNextInt` shell: `FindObjectChecked<UClass>` then `GetRegistryObjects`.
fn next_int(out: &mut Out, case: &str, current: i32) {
    push("findclass:Harry");
    let list = WORLD.with_borrow(|w| w.registry.clone());
    push("registry:class");
    let index = files::registry_index(list.len() as i32, current);
    let result = if index >= 0 {
        list[index as usize].0.clone()
    } else {
        String::new()
    };
    rec(
        out,
        "get_next_int",
        case,
        &format!("result={}", qs(&result)),
    );
}

/// `GetNextIntDesc` shell: same lookup, copies both the object and description.
fn next_int_desc(out: &mut Out, case: &str, current: i32) {
    push("findclass:Harry");
    let list = WORLD.with_borrow(|w| w.registry.clone());
    push("registry:class");
    let index = files::registry_index(list.len() as i32, current);
    let (entry, desc) = if index >= 0 {
        list[index as usize].clone()
    } else {
        (String::new(), String::new())
    };
    rec(
        out,
        "get_next_int_desc",
        case,
        &format!("entry={} desc={}", qs(&entry), qs(&desc)),
    );
}

/// `GetCacheEntry` shell: iterates the `Cache` section, then `cache_entry_index`.
fn cache_entry(out: &mut Out, case: &str, num: i32) {
    push("cachesec:Cache");
    let ini = WORLD.with_borrow(|w| {
        if w.cache_section {
            w.ini.clone()
        } else {
            Vec::new()
        }
    });
    let empty: Vec<bool> = ini.iter().map(|(_, v)| v.is_empty()).collect();
    let index = files::cache_entry_index(num, &empty);
    let (result, guid, filename) = if index >= 0 {
        let (k, v) = &ini[index as usize];
        (1, k.clone(), v.clone())
    } else {
        (0, String::new(), String::new())
    };
    let mut order = String::new();
    for (k, v) in &ini {
        let _ = write!(order, "{k}={v};");
    }
    rec(
        out,
        "get_cache_entry",
        case,
        &format!(
            "result={result} guid={} filename={} order={}",
            qs(&guid),
            qs(&filename),
            qs(&order)
        ),
    );
}

fn move_cache(out: &mut Out, case: &str, guid: &str, new_name: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let moved = unsafe { files::move_cache_entry(&svc, &wide(guid), &wide(new_name)) };
    rec(
        out,
        "move_cache_entry",
        case,
        &format!("result={}", i32::from(moved)),
    );
}

fn check_valid(out: &mut Out, case: &str, pkg: &str, mesh: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let valid = unsafe { files::check_valid_skin_package(&svc, &wide(pkg), &wide(mesh)) };
    rec(
        out,
        "check_valid_skin_package",
        case,
        &format!("result={}", i32::from(valid)),
    );
}

fn update_url(out: &mut Out, case: &str, option: &str, value: &str, save: bool) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    unsafe { files::update_url(&svc, ptr::null_mut(), &wide(option), &wide(value), save) };
    rec(out, "update_url", case, "done=1");
}

fn get_default_url(out: &mut Out, case: &str, option: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let value = unsafe { files::get_default_url(&svc, &wide(option)) };
    rec(
        out,
        "get_default_url",
        case,
        &format!("result={}", qs(&String::from_utf16_lossy(&value))),
    );
}

/// `GetNextSkin` shell: `GetRegistryObjects(texture class)` then `next_skin_index`.
fn next_skin(out: &mut Out, case: &str, prefix: &str, current: &str, dir: i32) {
    push("registry:tex");
    let skins = WORLD.with_borrow(|w| w.skins.clone());
    let names: Vec<Vec<u16>> = skins.iter().map(|(n, _)| wide(n)).collect();
    let (name, desc) = match files::next_skin_index(&names, &wide(prefix), &wide(current), dir) {
        Some(index) => skins[index].clone(),
        None => (String::new(), String::new()),
    };
    rec(
        out,
        "get_next_skin",
        case,
        &format!("name={} desc={}", qs(&name), qs(&desc)),
    );
}

fn reset_keyboard(out: &mut Out, case: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    unsafe { files::reset_keyboard(&svc, ptr::null_mut()) };
    rec(out, "reset_keyboard", case, "done=1");
}

fn find_best(out: &mut Out, case: &str, weight: f32, predict: bool) {
    let svc = services();
    let mut weight_out = weight;
    // SAFETY: the mock services are valid for the call; `weight_out` is a valid slot.
    let result = unsafe {
        files::find_best_inventory_path(&svc, ptr::null_mut(), weight, predict, &mut weight_out)
    };
    let result_tag = match result as usize {
        0 => "null",
        1 => "b",
        2 => "goal",
        _ => "other",
    };
    let (pause, shoot, goal) = WORLD.with_borrow(|w| (w.pause, w.shoot, w.goal));
    let goal_tag = match goal {
        0 => "null",
        CODE_ACTOR_B => "b",
        CODE_GOAL => "goal",
        _ => "other",
    };
    rec(
        out,
        "find_best_inventory_path",
        case,
        &format!(
            "weight={} result={result_tag} pause={} shoot={shoot} goal={goal_tag}",
            hf(weight_out),
            hf(pause)
        ),
    );
}

fn get_map_name(out: &mut Out, case: &str, prefix: &str, map: &str, dir: i32) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let result = unsafe { files::get_map_name(&svc, &wide(prefix), &wide(map), dir) };
    rec(
        out,
        "get_map_name",
        case,
        &format!("result={}", qs(&String::from_utf16_lossy(&result))),
    );
}

fn console_command(out: &mut Out, case: &str, command: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let result = unsafe { files::console_command(&svc, ptr::null_mut(), &wide(command)) };
    rec(
        out,
        "console_command",
        case,
        &format!("result={}", qs(&String::from_utf16_lossy(&result))),
    );
}

fn screen_to_world(out: &mut Out, case: &str, sx: f32, sy: f32, sz: f32) {
    let svc = services();
    let mut result = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    // SAFETY: the mock services are valid for the call; `result` is a valid slot.
    unsafe {
        files::screen_to_world(
            &svc,
            ptr::null_mut(),
            FVector {
                x: sx,
                y: sy,
                z: sz,
            },
            &mut result,
        )
    };
    rec(
        out,
        "screen_to_world",
        case,
        &format!(
            "result=({},{},{})",
            hf(result.x),
            hf(result.y),
            hf(result.z)
        ),
    );
}

fn get_url_map(out: &mut Out, case: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let value = unsafe { files::url_map(&svc, ptr::null_mut()) };
    // SAFETY: the mock returns a buffer from `keep`, valid until the next call.
    let map = unsafe { files::units(value) };
    rec(
        out,
        "get_url_map",
        case,
        &format!("result={}", qs(&String::from_utf16_lossy(&map))),
    );
}

fn get_entry_level(out: &mut Out, case: &str) {
    let svc = services();
    // SAFETY: the mock services are valid for the call.
    let level = unsafe { files::entry_level(&svc, ptr::null_mut()) };
    let tag = if level == code_ptr(CODE_ENTRY_LEVEL) {
        "entry"
    } else {
        "other"
    };
    rec(out, "get_entry_level", case, &format!("result={tag}"));
}

// ---------------------------------------------------------------- cases

fn default_ini() -> Vec<(String, String)> {
    vec![
        ("g1".into(), "a.unr".into()),
        ("g2".into(), String::new()),
        ("g3".into(), "c.unr".into()),
    ]
}

fn map_world(paths: &[&str], files: &[(&str, &[&str])]) -> impl FnOnce(&mut World) {
    let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
    let files: Vec<(String, Vec<String>)> = files
        .iter()
        .map(|(p, names)| {
            (
                (*p).to_owned(),
                names.iter().map(|n| (*n).to_owned()).collect(),
            )
        })
        .collect();
    move |w: &mut World| {
        w.paths = paths;
        w.files = files;
    }
}

fn run_all(out: &mut Out) {
    // get_next_int
    setup(|_| {});
    next_int(out, "i0", 0);
    setup(|_| {});
    next_int(out, "i2", 2);
    setup(|_| {});
    next_int(out, "i3", 3);
    setup(|w| w.registry.clear());
    next_int(out, "i_empty", 0);

    // get_next_int_desc
    setup(|_| {});
    next_int_desc(out, "d1", 1);
    setup(|_| {});
    next_int_desc(out, "d3", 3);
    setup(|w| w.registry.clear());
    next_int_desc(out, "d_empty", 0);

    // get_cache_entry
    setup(|w| w.ini = default_ini());
    cache_entry(out, "n0", 0);
    setup(|w| w.ini = default_ini());
    cache_entry(out, "n1", 1);
    setup(|w| w.ini = default_ini());
    cache_entry(out, "n2", 2);
    setup(|w| {
        w.ini = default_ini();
        w.cache_section = false;
    });
    cache_entry(out, "n_nosec", 0);

    // move_cache_entry
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into(), "System\\Engine.u".into()];
    });
    move_cache(out, "m1", "g1", "");
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into(), "System\\Engine.u".into()];
    });
    move_cache(out, "m2", "g1", "Dir/New.unr");
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into()];
    });
    move_cache(out, "m3", "g1", "Nodot");
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into()];
        w.move_ok = 0;
    });
    move_cache(out, "m4", "g1", "");
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into()];
    });
    move_cache(out, "m5", "zz", "");
    setup(|w| {
        w.ini = vec![("g3".into(), String::new())];
        w.paths = vec!["Maps\\Start.unr".into()];
    });
    move_cache(out, "m6", "g3", "");
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into()];
    });
    move_cache(out, "m7", "g1", "x.bin");
    setup(|w| {
        w.ini = vec![("g1".into(), "Orig.unr".into())];
        w.paths = vec!["Maps\\Start.unr".into()];
    });
    move_cache(out, "m8", "g1", "C:\\Dir\\q.unr");

    // check_valid_skin_package
    setup(|w| w.has_gameengine = false);
    check_valid(out, "c1", "HARRYSKINS", "HARRY");
    setup(|w| w.has_serverpkg = false);
    check_valid(out, "c2", "HARRYSKINS", "HARRY");
    setup(|w| w.defaults_num = 0);
    check_valid(out, "c3", "HARRYSKINS", "HARRY");
    setup(|_| {});
    check_valid(out, "c4", "UNREALSHARE", "HARRY");
    setup(|_| {});
    check_valid(out, "c5", "BOTPACK", "HARRY");
    setup(|_| {});
    check_valid(out, "c6", "HARRYSKINS", "HARRY");
    setup(|_| {});
    check_valid(out, "c7", "BOSSSKINS", "HARRY");
    setup(|_| {});
    check_valid(out, "c8", "TCOWMESHSKINS", "HARRY");
    setup(|_| {});
    check_valid(out, "c9", "FOO", "HARRY");
    setup(|w| w.export_text = "HARRYSKINS".into());
    check_valid(out, "c10", "HARRYSKINS", "HARRY");
    setup(|_| {});
    check_valid(out, "c11", "harryskins", "harry");

    // update_url
    setup(|_| {});
    update_url(out, "u0", "Name", "Val", false);
    setup(|_| {});
    update_url(out, "u1", "Name", "Val", true);

    // get_default_url
    setup(|_| {});
    get_default_url(out, "found", "Name");
    setup(|_| {});
    get_default_url(out, "missing", "Missing");
    setup(|_| {});
    get_default_url(out, "prefix", "Nam");

    // get_next_skin
    setup(|_| {});
    next_skin(out, "s_next", "Harry", "HarryA", 1);
    setup(|_| {});
    next_skin(out, "s_prev", "Harry", "HarryB", -1);
    setup(|_| {});
    next_skin(out, "s_cur", "Harry", "HarryB", 0);
    setup(|_| {});
    next_skin(out, "s_wrap", "Harry", "HarryC", 1);
    setup(|_| {});
    next_skin(out, "s_nocur", "Harry", "Nope", 1);
    setup(|_| {});
    next_skin(out, "s_noprefix", "Zzz", "HarryA", 1);
    setup(|w| w.skins.clear());
    next_skin(out, "s_empty", "Harry", "HarryA", 1);

    // reset_keyboard
    setup(|_| {});
    reset_keyboard(out, "r_input");
    setup(|w| w.has_input = false);
    reset_keyboard(out, "r_noinput");
    setup(|w| w.has_viewport = false);
    reset_keyboard(out, "r_noviewport");

    // find_best_inventory_path
    setup(|w| {
        w.best = CODE_ACTOR_B;
        w.best_weight = 2.0;
        w.goal = CODE_GOAL;
    });
    find_best(out, "f_improved", 1.0, false);
    setup(|w| {
        w.best = CODE_GOAL;
        w.best_weight = 2.0;
        w.goal = CODE_GOAL;
    });
    find_best(out, "f_goal", 1.0, false);
    setup(|w| {
        w.best = CODE_ACTOR_B;
        w.best_weight = 0.5;
    });
    find_best(out, "f_notimproved", 1.0, false);
    setup(|w| {
        w.best = CODE_ACTOR_B;
        w.best_weight = 2.0;
        w.probing = 1;
    });
    find_best(out, "f_handle", 1.0, false);
    setup(|w| {
        w.best = CODE_ACTOR_B;
        w.best_weight = 2.0;
    });
    find_best(out, "f_predict", 1.0, true);
    setup(|w| {
        w.best = 0;
        w.best_weight = 0.0;
    });
    find_best(out, "f_none", 0.0, false);

    // get_map_name
    setup(map_world(
        &["Maps\\*.unr"],
        &[("Maps\\*.unr", &["Maps\\Alpha.unr", "Maps\\Beta.unr"])],
    ));
    get_map_name(out, "g_basic", "", "Alpha.unr", 0);
    setup(map_world(
        &["Maps\\*.unr"],
        &[("Maps\\*.unr", &["Maps\\Alpha.unr", "Maps\\Beta.unr"])],
    ));
    get_map_name(out, "g_dir_plus", "", "Beta.unr", 1);
    setup(map_world(
        &["Maps\\*.unr"],
        &[("Maps\\*.unr", &["Maps\\Alpha.unr", "Maps\\Beta.unr"])],
    ));
    get_map_name(out, "g_dir_minus", "", "Alpha.unr", -1);
    setup(map_world(
        &["Maps\\*.unr"],
        &[("Maps\\*.unr", &["Maps\\Alpha.unr", "Maps\\Beta.unr"])],
    ));
    get_map_name(out, "g_nomatch", "", "Zeta.unr", 0);
    setup(|w| {
        let base = map_world(
            &["Maps\\*.unr"],
            &[
                ("Maps\\*.unr", &["Maps\\Alpha.unr"]),
                ("C:\\Hp2\\System/Maps\\*.unr", &["Maps\\Gamma.unr"]),
            ],
        );
        base(w);
        w.cd_path = "C:\\Hp2\\".into();
    });
    get_map_name(out, "g_cd", "", "Gamma.unr", 0);
    setup(map_world(
        &["Maps\\*.unr", "Maps\\*.unr"],
        &[("Maps\\*.unr", &["Maps\\Alpha.unr", "Maps\\Beta.unr"])],
    ));
    get_map_name(out, "g_dedupe", "", "Beta.unr", 0);
    setup(|_| {});
    get_map_name(out, "g_empty", "", "x.unr", 0);
    setup(map_world(
        &["Maps\\*.unr"],
        &[("Maps\\Al*.unr", &["Maps\\Alpha.unr"])],
    ));
    get_map_name(out, "g_prefix", "Al", "Alpha.unr", 0);
    setup(map_world(&["Maps\\*.txt"], &[]));
    get_map_name(out, "g_nonunr", "", "Alpha.unr", 0);
    setup(map_world(
        &["Maps\\*.unr"],
        &[("Maps\\*.unr", &["X\\Y\\Alpha.unr"])],
    ));
    get_map_name(out, "g_stripdir", "", "Alpha.unr", 0);

    // console_command
    setup(|w| w.exec_out = "Result text".into());
    console_command(out, "cc_player", "Test Cmd");
    setup(|w| {
        w.has_player = false;
        w.exec_out = "Engine out".into();
    });
    console_command(out, "cc_engine", "Test Cmd");
    setup(|w| w.exec_out = String::new());
    console_command(out, "cc_empty", "Test Cmd");

    // screen_to_world
    setup(|w| {
        w.vp = (640, 480);
        w.eff_fov = 90.0;
        w.maintain = 0;
        w.pawn_fov = 90.0;
    });
    screen_to_world(out, "sw1", 100.0, 50.0, 2.0);
    setup(|w| {
        w.vp = (800, 600);
        w.eff_fov = 60.0;
        w.maintain = 0;
        w.view_loc = FVector {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        w.view_rot = FRotator {
            pitch: 1000,
            yaw: 2000,
            roll: 3000,
        };
        w.pawn_fov = 60.0;
    });
    screen_to_world(out, "sw2", 0.0, 0.0, 1.0);
    setup(|w| {
        w.vp = (640, 480);
        w.eff_fov = 90.0;
        w.maintain = 0;
        w.pawn_fov = 90.0;
    });
    screen_to_world(out, "sw3", 320.0, 240.0, 0.5);
    setup(|w| {
        w.vp = (1024, 768);
        w.eff_fov = 75.0;
        w.maintain = 1;
        w.pawn_fov = 75.0;
    });
    screen_to_world(out, "sw4", -50.5, 700.0, 10.0);
    setup(|w| w.url_map = "Start".into());
    get_url_map(out, "u_map");
    setup(|w| w.url_map = "Maps\\Start.unr".into());
    get_url_map(out, "u_path");
    setup(|w| w.url_map = String::new());
    get_url_map(out, "u_empty");
    setup(|_| {});
    get_entry_level(out, "e_level");
}

fn entry_of(line: &str) -> &str {
    line.split_whitespace()
        .next()
        .and_then(|field| field.strip_prefix("entry="))
        .unwrap_or("")
}

#[test]
fn pawn_native_files_fixtures_match_oracle() {
    let mut out = Vec::new();
    run_all(&mut out);

    let expected: Vec<&str> = FIXTURE.lines().collect();
    assert_eq!(out.len(), expected.len(), "record count");
    for (got, want) in out.iter().zip(&expected) {
        assert_eq!(got, want);
    }

    let expected_counts = [
        ("get_next_int", 4),
        ("get_next_int_desc", 3),
        ("get_cache_entry", 4),
        ("move_cache_entry", 8),
        ("check_valid_skin_package", 11),
        ("update_url", 2),
        ("get_default_url", 3),
        ("get_next_skin", 7),
        ("reset_keyboard", 3),
        ("find_best_inventory_path", 6),
        ("get_map_name", 10),
        ("console_command", 3),
        ("screen_to_world", 4),
        ("get_url_map", 3),
        ("get_entry_level", 1),
    ];
    for (entry, count) in expected_counts {
        let got = out.iter().filter(|line| entry_of(line) == entry).count();
        assert_eq!(got, count, "record count for {entry}");
    }
    assert!(!out.is_empty(), "fixture replay produced no records");
}
