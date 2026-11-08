//! Replays the `persist.*` scenarios in `tests/fixtures/level_oracle.txt` against
//! the Rust port of `ULevel::SavePersistentActors` / `LoadPersistentActors`.
//!
//! The services are mocks that emit the same trace records as the C++ oracle
//! (`archive_serialize_bytes\t<len>\t<W|R>` for every int, sqword, and wire unit).
//! Each scenario's trace must match exactly, and its return value and saved byte
//! count must match too. Non-`persist.*` scenarios (`core.*`, `net.*`) are skipped.

use std::ffi::c_void;
use std::sync::Mutex;

use hp2_abi::engine::generated::AActor;
use hp2_engine::level_persist::{self, Hp2LevelPersistServices};

const FIXTURE: &str = include_str!("fixtures/level_oracle.txt");

#[derive(Clone)]
struct ActorRec {
    idx: i32,
    name: Vec<u32>,
    /// Real `AActor`-shaped memory: `actor_b_persistent`/`actor_set_b_script_initialized`
    /// are now direct mirror field accesses, so the handle must be a real pointer.
    mem: Box<AActor>,
}

fn new_actor_rec(idx: i32, name: Vec<u32>, persistent: bool) -> ActorRec {
    let mut mem: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
    mem.set_b_persistent(persistent);
    ActorRec { idx, name, mem }
}

fn actor_handle(rec: &ActorRec) -> *mut c_void {
    rec.mem.as_ref() as *const AActor as *mut c_void
}

fn actor_lookup(m: &Mock, actor: *mut c_void) -> &ActorRec {
    m.actors
        .iter()
        .find(|a| actor_handle(a) as usize == actor as usize)
        .expect("actor handle installed")
}

struct Ar {
    writer: bool,
    buf: Vec<u8>,
    pos: usize,
}

#[derive(Default)]
struct Mock {
    trace: Vec<String>,
    /// Actor handles are real `AActor`-mirror pointers (`new_actor_rec`); 0 is null.
    actors: Vec<ActorRec>,
    /// Level actor handles, in level order.
    level: Vec<usize>,
    /// TMultiMap handles are `index + 1`; entries are `(name, actor handle)`.
    maps: Vec<Vec<(Vec<u32>, usize)>>,
    /// Iterator handles are `index + 1`; entries are `(map index, position)`.
    iters: Vec<(usize, usize)>,
    /// Archive handles are `index + 1`.
    ars: Vec<Ar>,
    pa_in: Vec<u8>,
    file_size: i32,
    global_time: i64,
    reader_missing: bool,
    last_close_size: Option<usize>,
}

// SAFETY: heap-owned engine-shaped memory, only ever accessed through MOCK's mutex.
unsafe impl Send for Mock {}

static MOCK: Mutex<Option<Mock>> = Mutex::new(None);

fn install(m: Mock) {
    *MOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
}

fn take() -> Mock {
    MOCK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .expect("mock installed")
}

fn with<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    let mut g = MOCK.lock().unwrap_or_else(|e| e.into_inner());
    f(g.as_mut().expect("mock installed"))
}

fn rec(m: &mut Mock, line: String) {
    m.trace.push(line);
}

/// Reads a NUL-terminated `TCHAR` string.
unsafe fn read_z(p: *const u32) -> Vec<u32> {
    let mut v = Vec::new();
    let mut i = 0usize;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            return v;
        }
        v.push(c);
        i += 1;
    }
}

fn to_s(v: &[u32]) -> String {
    v.iter().map(|&c| char::from_u32(c).unwrap()).collect()
}

fn t(s: &str) -> Vec<u32> {
    s.chars().map(|c| c as u32).collect()
}

unsafe fn is_editor() -> i32 {
    0
}

unsafe fn fstring_ne(a: *const u32, b: *const u32) -> i32 {
    let (a, b) = unsafe { (read_z(a), read_z(b)) };
    (a != b) as i32
}

unsafe fn app_strcmp(a: *const u32, b: *const u32) -> i32 {
    let (a, b) = unsafe { (read_z(a), read_z(b)) };
    a.cmp(&b) as i32
}

unsafe fn level_actors_num(_level: *mut c_void) -> i32 {
    with(|m| m.level.len() as i32)
}

unsafe fn level_actor_at(_level: *mut c_void, i: i32) -> *mut c_void {
    with(|m| m.level[i as usize] as *mut c_void)
}

unsafe fn load_map_add(map: *mut c_void, name: *const u32, actor: *mut c_void) {
    let name = unsafe { read_z(name) };
    with(|m| m.maps[map as usize - 1].push((name, actor as usize)));
}

unsafe fn load_map_find(map: *mut c_void, name: *const u32) -> *mut c_void {
    let name = unsafe { read_z(name) };
    with(|m| {
        m.maps[map as usize - 1]
            .iter()
            .find(|(k, _)| *k == name)
            .map_or(std::ptr::null_mut(), |(_, a)| *a as *mut c_void)
    })
}

unsafe fn load_map_remove(map: *mut c_void, name: *const u32) {
    let name = unsafe { read_z(name) };
    with(|m| m.maps[map as usize - 1].retain(|(k, _)| *k != name));
}

unsafe fn load_map_iter_valid(it: *mut c_void) -> i32 {
    with(|m| {
        let (mi, pos) = m.iters[it as usize - 1];
        (pos < m.maps[mi].len()) as i32
    })
}

unsafe fn load_map_iter_value(it: *mut c_void) -> *mut c_void {
    with(|m| {
        let (mi, pos) = m.iters[it as usize - 1];
        m.maps[mi][pos].1 as *mut c_void
    })
}

unsafe fn load_map_iter_next(it: *mut c_void) {
    with(|m| m.iters[it as usize - 1].1 += 1);
}

unsafe fn archive_writer_create(path: *const u32) -> *mut c_void {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("archive_writer_create\t{path}"));
        m.ars.push(Ar {
            writer: true,
            buf: Vec::new(),
            pos: 0,
        });
        m.ars.len() as *mut c_void
    })
}

unsafe fn archive_reader_create(path: *const u32) -> *mut c_void {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("archive_reader_create\t{path}"));
        if m.reader_missing {
            return std::ptr::null_mut();
        }
        let buf = m.pa_in.clone();
        m.ars.push(Ar {
            writer: false,
            buf,
            pos: 0,
        });
        m.ars.len() as *mut c_void
    })
}

unsafe fn archive_close(ar: *mut c_void) {
    with(|m| {
        let (writer, size) = {
            let a = &m.ars[ar as usize - 1];
            (a.writer, a.buf.len())
        };
        if writer {
            rec(m, format!("archive_close\t{size}"));
            m.last_close_size = Some(size);
        }
    });
}

unsafe fn archive_tell(ar: *mut c_void) -> i32 {
    with(|m| m.ars[ar as usize - 1].pos as i32)
}

unsafe fn archive_seek(ar: *mut c_void, pos: i32) {
    with(|m| {
        rec(m, format!("archive_seek\t{pos}"));
        m.ars[ar as usize - 1].pos = pos as usize;
    });
}

unsafe fn archive_total_size(ar: *mut c_void) -> i32 {
    with(|m| m.ars[ar as usize - 1].buf.len() as i32)
}

unsafe fn archive_get_error(_ar: *mut c_void) -> i32 {
    0
}

/// Moves `len` bytes between the archive and `data`, recording one
/// `archive_serialize_bytes` line. Returns `true` for a writer.
unsafe fn serialize(ar: *mut c_void, data: *mut u8, len: usize) -> bool {
    with(|m| {
        let a = &mut m.ars[ar as usize - 1];
        let writer = a.writer;
        let dir = if writer { "W" } else { "R" };
        if writer {
            if a.buf.len() < a.pos + len {
                a.buf.resize(a.pos + len, 0);
            }
            let src = unsafe { std::slice::from_raw_parts(data, len) };
            a.buf[a.pos..a.pos + len].copy_from_slice(src);
        } else {
            let dst = unsafe { std::slice::from_raw_parts_mut(data, len) };
            for (i, d) in dst.iter_mut().enumerate() {
                *d = a.buf.get(a.pos + i).copied().unwrap_or(0);
            }
        }
        a.pos += len;
        rec(m, format!("archive_serialize_bytes\t{len}\t{dir}"));
        writer
    })
}

unsafe fn archive_serialize_bytes(ar: *mut c_void, data: *mut c_void, len: i32) {
    unsafe { serialize(ar, data as *mut u8, len as usize) };
}

unsafe fn archive_serialize_int(ar: *mut c_void, v: *mut i32) {
    let mut b = unsafe { *v }.to_le_bytes();
    let writer = unsafe { serialize(ar, b.as_mut_ptr(), 4) };
    if !writer {
        unsafe { *v = i32::from_le_bytes(b) };
    }
}

unsafe fn archive_serialize_sqword(ar: *mut c_void, v: *mut i64) {
    let mut b = unsafe { *v }.to_le_bytes();
    let writer = unsafe { serialize(ar, b.as_mut_ptr(), 8) };
    if !writer {
        unsafe { *v = i64::from_le_bytes(b) };
    }
}

unsafe fn file_manager_file_size(path: *const u32) -> i32 {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("file_size\t{path}\t{}", m.file_size));
        m.file_size
    })
}

unsafe fn file_manager_global_time(path: *const u32) -> i64 {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("global_time\t{path}"));
        m.global_time
    })
}

unsafe fn actor_serialize_persistence(ar: *mut c_void, actor: *mut c_void) {
    // Mirrors harness `AActor::SerializePersistence`: trace the call, then write
    // one INT `0x1000 + idx` through the archive (recorded as serialize_bytes 4).
    let idx = with(|m| {
        let idx = actor_lookup(m, actor).idx;
        rec(m, format!("actor_serialize_persistence\t{idx}"));
        idx
    });
    let mut v = (0x1000 + idx).to_le_bytes();
    unsafe { serialize(ar, v.as_mut_ptr(), 4) };
}

unsafe fn actor_destroy(_level: *mut c_void, actor: *mut c_void) {
    with(|m| {
        let idx = actor_lookup(m, actor).idx;
        rec(m, format!("level_destroy_actor\t{idx}"));
    });
}

unsafe fn debugf(name: i32, msg: *const u32) {
    // Mirrors harness `EngSink::Serialize` category names.
    let name = if name == 0 { "log" } else { "error" };
    let msg = escape_fixture(&to_s(&unsafe { read_z(msg) }));
    with(|m| rec(m, format!("debugf\t{name}\t{msg}")));
}

/// Escapes `\n`, `\r`, `\t` only, as the fixture writer does. Backslashes are
/// left alone so `Save\/cache` stays literal.
fn escape_fixture(s: &str) -> String {
    s.replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[derive(Default)]
struct PersistContext {
    names: [Vec<u32>; 3],
    save: Vec<*mut c_void>,
    map: usize,
}
unsafe fn context_map(context: *mut c_void) -> *mut c_void {
    unsafe { (*(context as *const PersistContext)).map as *mut c_void }
}
unsafe extern "C" fn checked_is_editor(out: *mut i32) -> i32 {
    unsafe {
        *out = is_editor();
    }
    0
}
unsafe extern "C" fn checked_build_filenames(context: *mut c_void, map_name: *const u32) -> i32 {
    let map = unsafe { read_z(map_name) };
    let mut leaf = to_s(&map).rsplit('/').next().unwrap().to_owned();
    if !leaf.contains("unr") {
        leaf.push_str(".unr");
    }
    let n = leaf.find("unr").unwrap() as i32 - 1;
    let stem: String = leaf.chars().take(n.max(0) as usize).collect();
    let names = [
        leaf.clone(),
        format!("Maps\\/{leaf}"),
        format!("Save\\/cache/{stem}_pa.usa"),
    ];
    for (i, name) in names.iter().enumerate() {
        unsafe {
            (*(context as *mut PersistContext)).names[i] =
                t(name).into_iter().chain(Some(0)).collect();
        }
    }
    0
}
unsafe extern "C" fn checked_filename(
    context: *mut c_void,
    index: i32,
    out: *mut *const u32,
) -> i32 {
    unsafe {
        *out = (*(context as *const PersistContext)).names[index as usize].as_ptr();
    }
    0
}
unsafe extern "C" fn checked_fstring_ne(context: *mut c_void, b: *const u32, out: *mut i32) -> i32 {
    let a = unsafe { (*(context as *const PersistContext)).names[0].as_ptr() };
    unsafe {
        *out = fstring_ne(a, b);
    }
    0
}
unsafe extern "C" fn checked_app_strcmp(a: *const u32, b: *const u32, out: *mut i32) -> i32 {
    unsafe {
        *out = app_strcmp(a, b);
    }
    0
}
unsafe extern "C" fn checked_level_actors_num(level: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = level_actors_num(level);
    }
    0
}
unsafe extern "C" fn checked_level_actor_at(
    level: *mut c_void,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = level_actor_at(level, index);
    }
    0
}
unsafe extern "C" fn checked_actor_name(actor: *mut c_void, out: *mut *const u32) -> i32 {
    unsafe {
        *out = with(|m| actor_lookup(m, actor).name.as_ptr());
    }
    0
}
unsafe extern "C" fn checked_save_array_add(context: *mut c_void, actor: *mut c_void) -> i32 {
    unsafe {
        (*(context as *mut PersistContext)).save.push(actor);
    }
    0
}
unsafe extern "C" fn checked_save_array_num(context: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = (*(context as *const PersistContext)).save.len() as i32;
    }
    0
}
unsafe extern "C" fn checked_save_array_at(
    context: *mut c_void,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = (&(*(context as *const PersistContext)).save)[index as usize];
    }
    0
}
unsafe extern "C" fn checked_save_array_empty(context: *mut c_void) -> i32 {
    unsafe {
        (*(context as *mut PersistContext)).save.clear();
    }
    0
}
unsafe extern "C" fn checked_load_map_add(
    context: *mut c_void,
    name: *const u32,
    actor: *mut c_void,
) -> i32 {
    unsafe {
        load_map_add(context_map(context), name, actor);
    }
    0
}
unsafe extern "C" fn checked_load_map_find(
    context: *mut c_void,
    name: *const u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = load_map_find(context_map(context), name);
    }
    0
}
unsafe extern "C" fn checked_load_map_remove(context: *mut c_void, name: *const u32) -> i32 {
    unsafe {
        load_map_remove(context_map(context), name);
    }
    0
}
unsafe extern "C" fn checked_load_map_empty(context: *mut c_void) -> i32 {
    let map = unsafe { context_map(context) } as usize;
    with(|m| m.maps[map - 1].clear());
    0
}
unsafe extern "C" fn checked_destroy_remaining(
    services: *const Hp2LevelPersistServices,
    level: *mut c_void,
    context: *mut c_void,
) -> i32 {
    let map = unsafe { context_map(context) } as usize;
    let it = with(|m| {
        m.iters.push((map - 1, 0));
        m.iters.len() as *mut c_void
    });
    match unsafe { level_persist::destroy_remaining(&*services, level, it) } {
        Ok(()) => 0,
        Err(()) => 1,
    }
}
unsafe extern "C" fn checked_load_map_iter_valid(iterator: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = load_map_iter_valid(iterator);
    }
    0
}
unsafe extern "C" fn checked_load_map_iter_value(
    iterator: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = load_map_iter_value(iterator);
    }
    0
}
unsafe extern "C" fn checked_load_map_iter_next(iterator: *mut c_void) -> i32 {
    unsafe {
        load_map_iter_next(iterator);
    }
    0
}
unsafe extern "C" fn checked_archive_writer_create(path: *const u32, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = archive_writer_create(path);
    }
    0
}
unsafe extern "C" fn checked_archive_reader_create(path: *const u32, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = archive_reader_create(path);
    }
    0
}
unsafe extern "C" fn checked_archive_close(archive: *mut c_void) -> i32 {
    unsafe {
        archive_close(archive);
    }
    0
}
unsafe extern "C" fn checked_archive_tell(archive: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = archive_tell(archive);
    }
    0
}
unsafe extern "C" fn checked_archive_seek(archive: *mut c_void, position: i32) -> i32 {
    unsafe {
        archive_seek(archive, position);
    }
    0
}
unsafe extern "C" fn checked_archive_total_size(archive: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = archive_total_size(archive);
    }
    0
}
unsafe extern "C" fn checked_archive_get_error(archive: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = archive_get_error(archive);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_bytes(
    archive: *mut c_void,
    data: *mut c_void,
    len: i32,
) -> i32 {
    unsafe {
        archive_serialize_bytes(archive, data, len);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_int(archive: *mut c_void, value: *mut i32) -> i32 {
    unsafe {
        archive_serialize_int(archive, value);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_sqword(
    archive: *mut c_void,
    value: *mut i64,
) -> i32 {
    unsafe {
        archive_serialize_sqword(archive, value);
    }
    0
}
unsafe extern "C" fn checked_file_manager_file_size(path: *const u32, out: *mut i32) -> i32 {
    unsafe {
        *out = file_manager_file_size(path);
    }
    0
}
unsafe extern "C" fn checked_file_manager_global_time(path: *const u32, out: *mut i64) -> i32 {
    unsafe {
        *out = file_manager_global_time(path);
    }
    0
}
unsafe extern "C" fn checked_actor_serialize_persistence(
    archive: *mut c_void,
    actor: *mut c_void,
) -> i32 {
    unsafe {
        actor_serialize_persistence(archive, actor);
    }
    0
}
unsafe extern "C" fn checked_actor_destroy(level: *mut c_void, actor: *mut c_void) -> i32 {
    unsafe {
        actor_destroy(level, actor);
    }
    0
}
unsafe extern "C" fn checked_debug_log(context: *mut c_void, message: i32) -> i32 {
    let c = unsafe { &*(context as *const PersistContext) };
    let message = match message {
        0 => (0, format!("*** Could not Save Persistent Actor file {}, CreateFileWriter had an error!!!!.", to_s(&c.names[2][..c.names[2].len()-1]))),
        1 => (0, format!("*** Starting to Save Persistent Actors for map {}", to_s(&c.names[0][..c.names[0].len()-1]))),
        2 => (1, "ULevelBase::SavePersistentActors ERROR: Saving map PA file identifier!".to_owned()),
        3 => (1, "ULevelBase::SavePersistentActors ERROR: Saving map name as UTF-16LE!".to_owned()),
        4 => (1, "ULevelBase::SavePersistentActors ERROR: Saving map FileSize that this PA file is for!".to_owned()),
        5 => (1, "ULevelBase::SavePersistentActors ERROR: Saving map FileTime that this PA file is for!".to_owned()),
        6 => (1, "ULevelBase::SavePersistentActors ERROR: Saving Number of persistent Actors!".to_owned()),
        7 => (1, "ULevelBase::SavePersistentActors ERROR: Saving actor name as UTF-16LE!".to_owned()),
        8 => (1, "ULevelBase::SavePersistentActors ERROR: Saving temp size of taggedProperties!".to_owned()),
        9 => (1, "ULevelBase::SavePersistentActors ERROR: Saving tagged properties of actor!".to_owned()),
        10 => (1, "ULevelBase::SavePersistentActors ERROR: Saving real size of taggedProperties!".to_owned()),
        11 => (1, "ULevelBase::SavePersistentActors ERROR: Saving EOF token as UTF-16LE!".to_owned()),
        12 => (0, format!("*** Finished Saving Persistent Actors for map {}", to_s(&c.names[0][..c.names[0].len()-1]))),
        13 => (0, format!("*** Could not load Persistent Actor file {} (file is probably missing).", to_s(&c.names[2][..c.names[2].len()-1]))),
        14 => (0, format!("*** Starting to Load Persistent Actors for map {}", to_s(&c.names[0][..c.names[0].len()-1]))),
        15 => (1, "ULevelBase::LoadPersistentActors ERROR: Invalid or truncated PA file identifier!".to_owned()),
        16 => (1, "ULevelBase::LoadPersistentActors ERROR: Invalid map name UTF-16LE string!".to_owned()),
        17 => (1, "ULevelBase::LoadPersistentActors ERROR: FinalMapName != Saved map name!".to_owned()),
        18 => (1, "ULevelBase::LoadPersistentActors ERROR: Loading map FileSize that this PA file is for!".to_owned()),
        19 => (1, "ULevelBase::LoadPersistentActors ERROR: Saving map FileTime that this PA file is for!".to_owned()),
        20 => (1, "ULevelBase::LoadPersistentActors ERROR: Loading total number of persistent actors!".to_owned()),
        21 => (1, "ULevelBase::LoadPersistentActors ERROR: Invalid actor name UTF-16LE string!".to_owned()),
        22 => (1, "ULevelBase::LoadPersistentActors ERROR: Loading size of tagged properties!".to_owned()),
        23 => (0, format!("*** Actor is NOT in level {} but is in pa file {}! \n. ", to_s(&c.names[0][..c.names[0].len()-1]), to_s(&c.names[2][..c.names[2].len()-1]))),
        24 => (1, "Level actor's name != persistent actor's name!!!!!".to_owned()),
        25 => (1, "ULevelBase::LoadPersistentActors ERROR: Loading tagged properties!".to_owned()),
        26 => (1, "ULevelBase::LoadPersistentActors ERROR: Invalid EOF UTF-16LE string!".to_owned()),
        27 => (1, "The EOF token is corrupt in the persistent actor file!!!!!".to_owned()),
        28 => (0, format!("*** Finished Loading Persistent Actors for map {}", to_s(&c.names[0][..c.names[0].len()-1]))),
        _ => panic!("invalid persistence log"),
    };
    let text: Vec<u32> = t(&message.1).into_iter().chain(Some(0)).collect();
    unsafe {
        debugf(message.0, text.as_ptr());
    }
    0
}
fn services() -> Hp2LevelPersistServices {
    Hp2LevelPersistServices {
        is_editor: checked_is_editor,
        build_filenames: checked_build_filenames,
        filename: checked_filename,
        fstring_ne: checked_fstring_ne,
        app_strcmp: checked_app_strcmp,
        level_actors_num: checked_level_actors_num,
        level_actor_at: checked_level_actor_at,
        actor_name: checked_actor_name,
        save_array_add: checked_save_array_add,
        save_array_num: checked_save_array_num,
        save_array_at: checked_save_array_at,
        save_array_empty: checked_save_array_empty,
        load_map_add: checked_load_map_add,
        load_map_find: checked_load_map_find,
        load_map_remove: checked_load_map_remove,
        load_map_empty: checked_load_map_empty,
        destroy_remaining: checked_destroy_remaining,
        load_map_iter_valid: checked_load_map_iter_valid,
        load_map_iter_value: checked_load_map_iter_value,
        load_map_iter_next: checked_load_map_iter_next,
        archive_writer_create: checked_archive_writer_create,
        archive_reader_create: checked_archive_reader_create,
        archive_close: checked_archive_close,
        archive_tell: checked_archive_tell,
        archive_seek: checked_archive_seek,
        archive_total_size: checked_archive_total_size,
        archive_get_error: checked_archive_get_error,
        archive_serialize_bytes: checked_archive_serialize_bytes,
        archive_serialize_int: checked_archive_serialize_int,
        archive_serialize_sqword: checked_archive_serialize_sqword,
        file_manager_file_size: checked_file_manager_file_size,
        file_manager_global_time: checked_file_manager_global_time,
        actor_serialize_persistence: checked_actor_serialize_persistence,
        actor_destroy: checked_actor_destroy,
        debug_log: checked_debug_log,
    }
}

struct Scenario {
    name: String,
    inputs: Vec<Vec<String>>,
    trace: Vec<String>,
    ret: Option<i32>,
    saved: Option<usize>,
}

/// Parses the fixture verbatim. Paths keep the oracle's literal `Save\/cache` form.
fn parse(text: &str) -> Vec<Scenario> {
    let mut out: Vec<Scenario> = Vec::new();
    for raw in text.lines() {
        if raw.is_empty() {
            continue;
        }
        let line = raw.to_string();
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "case" => continue,
            "scenario" => out.push(Scenario {
                name: f[1].to_string(),
                inputs: Vec::new(),
                trace: Vec::new(),
                ret: None,
                saved: None,
            }),
            "input" => out
                .last_mut()
                .unwrap()
                .inputs
                .push(f[1..].iter().map(|s| s.to_string()).collect()),
            "=>" => {
                let sc = out.last_mut().unwrap();
                if sc.name.starts_with("persist.") {
                    sc.ret = Some(f[1].parse().unwrap());
                }
            }
            "saved_bytes" => out.last_mut().unwrap().saved = Some(f[1].parse().unwrap()),
            _ => out.last_mut().unwrap().trace.push(line.clone()),
        }
    }
    out
}

fn decode_hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn run(sc: &Scenario) {
    let mut m = Mock {
        file_size: -1,
        ..Mock::default()
    };
    let mut map_name = String::new();
    for inp in &sc.inputs {
        match inp[0].as_str() {
            "map" => map_name = inp[1].clone(),
            "actor" => {
                m.actors.push(new_actor_rec(
                    inp[1].parse().unwrap(),
                    t(&inp[2]).into_iter().chain(Some(0)).collect(),
                    inp[3] == "1",
                ));
                if inp[4] == "1" {
                    m.level.push(actor_handle(m.actors.last().unwrap()) as usize);
                }
            }
            "pa_bytes" => {
                let n: usize = inp[1].parse().unwrap();
                m.pa_in = decode_hex(inp.get(2).map_or("", |s| s.as_str()));
                assert_eq!(m.pa_in.len(), n, "scenario {}: pa_bytes length", sc.name);
            }
            "file_size" => m.file_size = inp[1].parse().unwrap(),
            "global_time" => m.global_time = inp[1].parse().unwrap(),
            "reader_null" => m.reader_missing = inp[1] == "1",
            other => panic!("scenario {}: unknown input {other}", sc.name),
        }
    }
    install(m);

    let svc = services();
    let map: Vec<u32> = t(&map_name).into_iter().chain(std::iter::once(0)).collect();
    let mut level_byte = 0u8;
    let level = &mut level_byte as *mut u8 as *mut c_void;
    let map_index = with(|m| {
        m.maps.push(Vec::new());
        m.maps.len()
    });
    let mut context = PersistContext {
        map: map_index,
        ..Default::default()
    };
    let context = &mut context as *mut PersistContext as *mut c_void;
    let ret = unsafe {
        if sc.name.starts_with("persist.save_") {
            level_persist::save_persistent_actors(&svc, level, map.as_ptr(), context)
        } else {
            level_persist::load_persistent_actors(&svc, level, map.as_ptr(), context)
        }
    }
    .expect("checked persistence succeeds");
    let m = take();

    assert_eq!(m.trace, sc.trace, "scenario {}: trace", sc.name);
    assert_eq!(
        ret as i32,
        sc.ret.expect("scenario has =>"),
        "scenario {}: return",
        sc.name
    );
    if let Some(n) = sc.saved {
        assert_eq!(
            m.last_close_size,
            Some(n),
            "scenario {}: saved bytes",
            sc.name
        );
    }
}

/// Expected record counts per persist scenario: (name, trace records, return).
/// Pinned from the oracle fixture so a silent regeneration fails here.
const EXPECTED: &[(&str, usize, i32)] = &[
    ("persist.save_two_persistent", 56, 0),
    ("persist.load_roundtrip", 49, 0),
    ("persist.load_missing_file", 2, 0),
    ("persist.load_empty_input", 2, 0),
    ("persist.load_bad_identifier", 7, 1),
    ("persist.load_map_mismatch", 17, 0),
    ("persist.load_size_mismatch", 19, 0),
    ("persist.load_missing_actor", 49, 0),
    ("persist.load_leftover_destroy", 50, 0),
    ("persist.load_bad_eof", 45, 1),
    ("persist.load_wire_error", 7, 1),
];

#[test]
fn persist_scenarios_replay() {
    let scenarios = parse(FIXTURE);
    let mut ran = 0;
    for sc in scenarios.iter().filter(|s| s.name.starts_with("persist.")) {
        let &(_, count, ret) = EXPECTED
            .iter()
            .find(|(n, _, _)| *n == sc.name)
            .unwrap_or_else(|| panic!("unexpected persist scenario {}", sc.name));
        assert_eq!(sc.trace.len(), count, "scenario {}: record count", sc.name);
        assert_eq!(sc.ret, Some(ret), "scenario {}: expected return", sc.name);
        run(sc);
        ran += 1;
    }
    assert_eq!(
        ran,
        EXPECTED.len(),
        "expected 11 persist scenarios in the fixture"
    );
    checked_fault_matrix();
}

// First-failure matrix shares the replay's serial invocation.
thread_local! {
    static FAIL_AT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CALL_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn fail_next() -> bool {
    let count = CALL_COUNT.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        n
    });
    FAIL_AT.with(|f| f.get() == count)
}
unsafe extern "C" fn fault_is_editor(out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_is_editor(out) }
}
unsafe extern "C" fn fault_build_filenames(context: *mut c_void, map_name: *const u32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_build_filenames(context, map_name) }
}
unsafe extern "C" fn fault_filename(context: *mut c_void, index: i32, out: *mut *const u32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_filename(context, index, out) }
}
unsafe extern "C" fn fault_fstring_ne(context: *mut c_void, b: *const u32, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_fstring_ne(context, b, out) }
}
unsafe extern "C" fn fault_app_strcmp(a: *const u32, b: *const u32, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_app_strcmp(a, b, out) }
}
unsafe extern "C" fn fault_level_actors_num(level: *mut c_void, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_level_actors_num(level, out) }
}
unsafe extern "C" fn fault_level_actor_at(
    level: *mut c_void,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_level_actor_at(level, index, out) }
}
unsafe extern "C" fn fault_actor_name(actor: *mut c_void, out: *mut *const u32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_actor_name(actor, out) }
}
unsafe extern "C" fn fault_save_array_add(context: *mut c_void, actor: *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_save_array_add(context, actor) }
}
unsafe extern "C" fn fault_save_array_num(context: *mut c_void, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_save_array_num(context, out) }
}
unsafe extern "C" fn fault_save_array_at(
    context: *mut c_void,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_save_array_at(context, index, out) }
}
unsafe extern "C" fn fault_save_array_empty(context: *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_save_array_empty(context) }
}
unsafe extern "C" fn fault_load_map_add(
    context: *mut c_void,
    name: *const u32,
    actor: *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_add(context, name, actor) }
}
unsafe extern "C" fn fault_load_map_find(
    context: *mut c_void,
    name: *const u32,
    out: *mut *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_find(context, name, out) }
}
unsafe extern "C" fn fault_load_map_remove(context: *mut c_void, name: *const u32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_remove(context, name) }
}
unsafe extern "C" fn fault_load_map_empty(context: *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_empty(context) }
}
unsafe extern "C" fn fault_destroy_remaining(
    services: *const Hp2LevelPersistServices,
    level: *mut c_void,
    context: *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_destroy_remaining(services, level, context) }
}
unsafe extern "C" fn fault_load_map_iter_valid(iterator: *mut c_void, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_iter_valid(iterator, out) }
}
unsafe extern "C" fn fault_load_map_iter_value(
    iterator: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_iter_value(iterator, out) }
}
unsafe extern "C" fn fault_load_map_iter_next(iterator: *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_load_map_iter_next(iterator) }
}
unsafe extern "C" fn fault_archive_writer_create(path: *const u32, out: *mut *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_writer_create(path, out) }
}
unsafe extern "C" fn fault_archive_reader_create(path: *const u32, out: *mut *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_reader_create(path, out) }
}
unsafe extern "C" fn fault_archive_close(archive: *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_close(archive) }
}
unsafe extern "C" fn fault_archive_tell(archive: *mut c_void, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_tell(archive, out) }
}
unsafe extern "C" fn fault_archive_seek(archive: *mut c_void, position: i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_seek(archive, position) }
}
unsafe extern "C" fn fault_archive_total_size(archive: *mut c_void, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_total_size(archive, out) }
}
unsafe extern "C" fn fault_archive_get_error(archive: *mut c_void, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_get_error(archive, out) }
}
unsafe extern "C" fn fault_archive_serialize_bytes(
    archive: *mut c_void,
    data: *mut c_void,
    len: i32,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_serialize_bytes(archive, data, len) }
}
unsafe extern "C" fn fault_archive_serialize_int(archive: *mut c_void, value: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_serialize_int(archive, value) }
}
unsafe extern "C" fn fault_archive_serialize_sqword(archive: *mut c_void, value: *mut i64) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_archive_serialize_sqword(archive, value) }
}
unsafe extern "C" fn fault_file_manager_file_size(path: *const u32, out: *mut i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_file_manager_file_size(path, out) }
}
unsafe extern "C" fn fault_file_manager_global_time(path: *const u32, out: *mut i64) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_file_manager_global_time(path, out) }
}
unsafe extern "C" fn fault_actor_serialize_persistence(
    archive: *mut c_void,
    actor: *mut c_void,
) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_actor_serialize_persistence(archive, actor) }
}
unsafe extern "C" fn fault_actor_destroy(level: *mut c_void, actor: *mut c_void) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_actor_destroy(level, actor) }
}
unsafe extern "C" fn fault_debug_log(context: *mut c_void, message: i32) -> i32 {
    if fail_next() {
        return 1;
    }
    unsafe { checked_debug_log(context, message) }
}
fn fault_services() -> Hp2LevelPersistServices {
    Hp2LevelPersistServices {
        is_editor: fault_is_editor,
        build_filenames: fault_build_filenames,
        filename: fault_filename,
        fstring_ne: fault_fstring_ne,
        app_strcmp: fault_app_strcmp,
        level_actors_num: fault_level_actors_num,
        level_actor_at: fault_level_actor_at,
        actor_name: fault_actor_name,
        save_array_add: fault_save_array_add,
        save_array_num: fault_save_array_num,
        save_array_at: fault_save_array_at,
        save_array_empty: fault_save_array_empty,
        load_map_add: fault_load_map_add,
        load_map_find: fault_load_map_find,
        load_map_remove: fault_load_map_remove,
        load_map_empty: fault_load_map_empty,
        destroy_remaining: fault_destroy_remaining,
        load_map_iter_valid: fault_load_map_iter_valid,
        load_map_iter_value: fault_load_map_iter_value,
        load_map_iter_next: fault_load_map_iter_next,
        archive_writer_create: fault_archive_writer_create,
        archive_reader_create: fault_archive_reader_create,
        archive_close: fault_archive_close,
        archive_tell: fault_archive_tell,
        archive_seek: fault_archive_seek,
        archive_total_size: fault_archive_total_size,
        archive_get_error: fault_archive_get_error,
        archive_serialize_bytes: fault_archive_serialize_bytes,
        archive_serialize_int: fault_archive_serialize_int,
        archive_serialize_sqword: fault_archive_serialize_sqword,
        file_manager_file_size: fault_file_manager_file_size,
        file_manager_global_time: fault_file_manager_global_time,
        actor_serialize_persistence: fault_actor_serialize_persistence,
        actor_destroy: fault_actor_destroy,
        debug_log: fault_debug_log,
    }
}
fn checked_fault_matrix() {
    let mut saved = Vec::new();
    for loading in [false, true] {
        let mut total = 0;
        let mut fault = 0;
        loop {
            let zname = |s: &str| t(s).into_iter().chain(Some(0)).collect();
            let actors = vec![
                new_actor_rec(1, zname("Actor01"), true),
                new_actor_rec(2, zname("Actor02"), true),
            ];
            let level = actors.iter().map(|a| actor_handle(a) as usize).collect();
            install(Mock {
                actors,
                level,
                maps: vec![Vec::new()],
                pa_in: saved.clone(),
                file_size: 123,
                global_time: 456,
                ..Default::default()
            });
            if loading {
                with(|m| {
                    m.actors.push(new_actor_rec(3, zname("Actor03"), true));
                    m.level.push(actor_handle(m.actors.last().unwrap()) as usize);
                });
            }
            let mut context = PersistContext {
                map: 1,
                ..Default::default()
            };
            let context = &mut context as *mut PersistContext as *mut c_void;
            let services = fault_services();
            let name: Vec<u32> = zname("Fixture");
            FAIL_AT.with(|v| v.set(fault));
            CALL_COUNT.with(|v| v.set(0));
            let result = unsafe {
                if loading {
                    level_persist::load_persistent_actors(
                        &services,
                        std::ptr::null_mut(),
                        name.as_ptr(),
                        context,
                    )
                } else {
                    level_persist::save_persistent_actors(
                        &services,
                        std::ptr::null_mut(),
                        name.as_ptr(),
                        context,
                    )
                }
            };
            let count = CALL_COUNT.with(|v| v.get());
            FAIL_AT.with(|v| v.set(0));
            let mock = take();
            if fault == 0 {
                assert_eq!(result, Ok(false), "matrix normal path loading={loading}");
                total = count;
                if !loading {
                    saved = mock.ars[0].buf.clone();
                }
            } else {
                assert_eq!(result, Err(()), "fault {fault}/{total}, loading={loading}");
                assert_eq!(count, fault, "no later callback after first failure");
                // Only an explicit successful close can populate last_close_size.
                if !loading {
                    assert_eq!(mock.last_close_size, None);
                }
            }
            fault += 1;
            if fault > total {
                break;
            }
        }
    }
}
