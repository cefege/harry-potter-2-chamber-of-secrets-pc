//! Replays the `actor_rep` oracle cases (`Build/actor_rep_fixture_harness`) through the
//! Rust `actor_rep` module on mirror-sized objects and checks the emitted records against
//! `fixtures/actor_rep_replication.txt`, record for record, and the per-family counts.
//!
//! Object images are raw buffers. Field offsets come from the fixture `layout` lines, so
//! each token is written with the same tag, offset, size, count and mask as the oracle.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::fs;
use std::mem::size_of;
use std::ptr::{self, NonNull};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, AAmmo, AGameReplicationInfo, AInventory, ALevelInfo, AMover, APawn, APickup,
    APlayerPawn, APlayerReplicationInfo, AWeapon, AZoneInfo,
};
use hp2_engine::actor_rep::{
    Hp2ActorRepServices, Rep, RepCache, actor_get_optimized_rep_list, actor_update_frequency,
    ammo_get_optimized_rep_list, game_replication_info_get_optimized_rep_list,
    inventory_get_optimized_rep_list, inventory_no_variables_to_replicate,
    inventory_should_do_script_replication, inventory_update_frequency,
    level_info_get_optimized_rep_list, mover_get_optimized_rep_list, pawn_get_optimized_rep_list,
    pickup_get_optimized_rep_list, player_pawn_get_optimized_rep_list,
    player_replication_info_get_optimized_rep_list, weapon_get_optimized_rep_list,
    zone_info_get_optimized_rep_list,
};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/actor_rep_replication.txt"
);

/// `kObjBytes` in the harness: every object image is this large.
const OBJ_WORDS: usize = 16384 / 8;

/// Kind codes, matching `actor_rep.rs` and the harness `K_*` order.
const K_ACTOR: usize = 0;
const K_PAWN: usize = 1;
const K_PLAYERPAWN: usize = 2;
const K_MOVER: usize = 3;
const K_ZONE: usize = 4;
const K_LEVEL: usize = 5;
const K_PRI: usize = 6;
const K_GRI: usize = 7;
const K_INV: usize = 8;
const K_WEAPON: usize = 9;
const K_AMMO: usize = 10;
const K_PICKUP: usize = 11;

const TYPE_NAMES: [&str; 12] = [
    "AActor",
    "APawn",
    "APlayerPawn",
    "AMover",
    "AZoneInfo",
    "ALevelInfo",
    "APlayerReplicationInfo",
    "AGameReplicationInfo",
    "AInventory",
    "AWeapon",
    "AAmmo",
    "APickup",
];

/// Parent kind of each owned class (-1 for AActor), as the harness `kParent` table.
const K_PARENT: [i32; 12] = [-1, 0, 1, 0, 0, 4, 0, 0, 0, 8, 11, 8];

/// `CLASS_NativeReplication` from `UnObjBas.h`.
const CLASS_NATIVE_REPLICATION: u32 = 0x0000_0800;

/// Buffer slots inside `State::bufs`.
const THIS: usize = 0;
const RECENT: usize = 1;
const PAWN: usize = 2;
const DEFAULTS: usize = 3;

/// Empty `FString` data, as the harness `kEmptyStr`.
static EMPTY_STR: [u32; 1] = [0];

/// Fake `UClass` identities: the address of element k is the class pointer for kind k.
static CLASS_TAG: [u64; 12] = [0; 12];

fn class_ptr(kind: usize) -> usize {
    CLASS_TAG.as_ptr() as usize + 8 * kind
}

#[derive(Default)]
struct World {
    native: u32,
    reject: usize,
    defaults: usize,
    class_addr: [usize; 12],
    class_off: usize,
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::default());
}

// ---- services ------------------------------------------------------------------------------

unsafe extern "C" fn static_class_flags(kind: u32) -> u32 {
    let native = WORLD.with(|w| w.borrow().native);
    if (native >> kind) & 1 != 0 {
        CLASS_NATIVE_REPLICATION
    } else {
        0
    }
}

unsafe extern "C" fn find_rep_index(kind: u32, name: *const c_char) -> u32 {
    // Mirrors the harness `StaticFindObjectChecked`: FNV-1a over the name, then
    // `kind * 4096 + hash % 4055 + 1` (per-kind band; the harness asserts no collision).
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    let mut h: u32 = 2_166_136_261;
    for &b in bytes {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    (kind * 4096 + h % 4055 + 1) as u16 as u32
}

unsafe extern "C" fn can_serialize_object(_map: *mut c_void, obj: *mut c_void) -> i32 {
    let reject = WORLD.with(|w| w.borrow().reject);
    i32::from(obj as usize != reject)
}

unsafe extern "C" fn fstring_neq(a: *const c_void, b: *const c_void) -> i32 {
    // TArrayRaw's data pointer is at +0 and num at +8; `FString::operator*` yields "" when num is 0.
    let pa = unsafe { fstring_text(a) };
    let pb = unsafe { fstring_text(b) };
    i32::from(unsafe { stricmp_differs(pa, pb) })
}

unsafe fn fstring_text(f: *const c_void) -> *const u32 {
    let data = unsafe { ptr::read_unaligned(f.cast::<usize>()) } as *const u32;
    let num = unsafe { ptr::read_unaligned(f.cast::<u8>().add(8).cast::<i32>()) };
    if num == 0 { EMPTY_STR.as_ptr() } else { data }
}

unsafe fn stricmp_differs(mut a: *const u32, mut b: *const u32) -> bool {
    loop {
        let x = fold_ascii(unsafe { *a });
        let y = fold_ascii(unsafe { *b });
        if x != y {
            return true;
        }
        if x == 0 {
            return false;
        }
        a = unsafe { a.add(1) };
        b = unsafe { b.add(1) };
    }
}

fn fold_ascii(c: u32) -> u32 {
    if (u32::from(b'A')..=u32::from(b'Z')).contains(&c) {
        c + 32
    } else {
        c
    }
}

unsafe extern "C" fn object_is_a(obj: *const c_void, kind: u32) -> i32 {
    let (off, classes) = WORLD.with(|w| {
        let w = w.borrow();
        (w.class_off, w.class_addr)
    });
    let cls = unsafe { ptr::read_unaligned(obj.cast::<u8>().add(off).cast::<usize>()) };
    let Some(mut k) = classes.iter().position(|&c| c == cls) else {
        return 0;
    };
    loop {
        if k as u32 == kind {
            return 1;
        }
        let parent = K_PARENT[k];
        if parent < 0 {
            return 0;
        }
        k = parent as usize;
    }
}

unsafe extern "C" fn default_actor_of(_actor: *const c_void) -> *const c_void {
    WORLD.with(|w| w.borrow().defaults) as *const c_void
}

fn services() -> Hp2ActorRepServices {
    Hp2ActorRepServices {
        static_class_flags,
        find_rep_index,
        can_serialize_object,
        fstring_neq,
        object_is_a,
        default_actor_of,
    }
}

// ---- fixture -------------------------------------------------------------------------------

struct Layout {
    tag: u8,
    off: usize,
    size: usize,
    count: usize,
    mask: u8,
}

struct Case {
    family: String,
    idx: usize,
    tokens: Vec<String>,
    raw: String,
    res: String,
}

struct Fixture {
    sizes: HashMap<String, usize>,
    layout: HashMap<(String, String), Layout>,
    cases: Vec<Case>,
}

fn num(s: Option<&str>) -> usize {
    s.expect("missing field").parse().expect("number")
}

impl Fixture {
    fn class_off(&self) -> usize {
        self.layout[&("AActor".to_owned(), "Class".to_owned())].off
    }

    fn family_text(&self, family: &str) -> String {
        let mut s = String::new();
        for c in self.cases.iter().filter(|c| c.family == family) {
            s.push_str(&c.raw);
            s.push('\n');
            s.push_str(&c.res);
            s.push('\n');
        }
        s
    }
}

fn fixture() -> Fixture {
    let text = fs::read_to_string(FIXTURE).expect("read actor_rep fixture");
    let mut fx = Fixture {
        sizes: HashMap::new(),
        layout: HashMap::new(),
        cases: Vec::new(),
    };
    for line in text.lines() {
        let mut f = line.split(' ');
        match f.next() {
            Some("sizeof") => {
                let ty = f.next().expect("sizeof type").to_owned();
                fx.sizes.insert(ty, num(f.next()));
            }
            Some("layout") => {
                let key = (
                    f.next().expect("layout type").to_owned(),
                    f.next().expect("layout name").to_owned(),
                );
                let tag = f.next().expect("layout tag").as_bytes()[0];
                let off = num(f.next());
                let size = num(f.next());
                let count = num(f.next());
                let mask = num(f.next()) as u8;
                fx.layout.entry(key).or_insert(Layout {
                    tag,
                    off,
                    size,
                    count,
                    mask,
                });
            }
            Some("case") => {
                let family = f.next().expect("case family").to_owned();
                let idx = num(f.next());
                let tokens = f.map(str::to_owned).collect();
                fx.cases.push(Case {
                    family,
                    idx,
                    tokens,
                    raw: line.to_owned(),
                    res: String::new(),
                });
            }
            Some("res") => {
                fx.cases.last_mut().expect("res before any case").res = line.to_owned();
            }
            _ => {}
        }
    }
    fx
}

fn case_line(c: &Case) -> String {
    let mut s = format!("case {} {}", c.family, c.idx);
    for t in &c.tokens {
        s.push(' ');
        s.push_str(t);
    }
    s
}

// ---- object images -------------------------------------------------------------------------

struct State {
    /// THIS, RECENT, PAWN, DEFAULTS images.
    bufs: [Vec<u64>; 4],
    /// Pointer selectors: 0 null, 1 this, 2 recent, 3 pawn, 4 defaults.
    addrs: [usize; 5],
    /// Backing storage for FString data, kept alive for the case.
    strings: Vec<Box<[u32]>>,
}

unsafe fn put(base: *mut u8, off: usize, bytes: &[u8]) {
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(off), bytes.len()) };
}

impl State {
    fn new(kind: usize, class_off: usize) -> Self {
        let mut st = State {
            bufs: std::array::from_fn(|_| vec![0u64; OBJ_WORDS]),
            addrs: [0; 5],
            strings: Vec::new(),
        };
        for i in 0..4 {
            st.addrs[i + 1] = st.bufs[i].as_ptr() as usize;
        }
        // Class of this and recent is the family class; the pawn is an APawn.
        let this_cls = class_ptr(kind).to_le_bytes();
        let pawn_cls = class_ptr(K_PAWN).to_le_bytes();
        unsafe {
            put(st.bufs[THIS].as_mut_ptr().cast(), class_off, &this_cls);
            put(st.bufs[RECENT].as_mut_ptr().cast(), class_off, &this_cls);
            put(st.bufs[PAWN].as_mut_ptr().cast(), class_off, &pawn_cls);
        }
        st
    }
}

/// Pointer selector from the harness `null|t|r|o` spelling.
fn ptr_index(val: &str) -> usize {
    match val {
        "null" => 0,
        "t" => 1,
        "r" => 2,
        "o" => 3,
        other => panic!("unknown pointer selector {other}"),
    }
}

fn payload(v: &str) -> &str {
    v.split_once(':').expect("tagged value").1
}

fn hex_words(p: &str) -> Vec<u32> {
    p.split(',')
        .map(|h| u32::from_str_radix(h, 16).expect("hex word"))
        .collect()
}

/// Writes one value at its layout entry, as the harness `write_entry`.
///
/// # Safety
/// `base` must address a live object image with room for `e.off + e.size` bytes.
unsafe fn write_entry(
    base: *mut u8,
    e: &Layout,
    v: &str,
    addrs: &[usize; 5],
    strings: &mut Vec<Box<[u32]>>,
) {
    let off = e.off;
    if e.tag != b'x' {
        unsafe { ptr::write_bytes(base.add(off), 0, e.size) };
    }
    match e.tag {
        b'x' => {
            let p = unsafe { base.add(off) };
            match v {
                "0" => unsafe { *p &= !e.mask },
                "1" => unsafe { *p |= e.mask },
                other => panic!("bitfield value {other}"),
            }
        }
        b'b' | b'w' | b'i' => {
            assert_eq!(e.count, 1, "scalar tag on array entry");
            let x: i64 = v.parse().expect("integer");
            unsafe { put(base, off, &x.to_le_bytes()[..e.size]) };
        }
        b'f' | b'n' => {
            assert_eq!(e.size, 4, "4-byte tag size");
            let w = u32::from_str_radix(payload(v), 16).expect("hex bits");
            unsafe { put(base, off, &w.to_le_bytes()) };
        }
        b'v' | b'p' => {
            let words = hex_words(payload(v));
            assert_eq!(words.len() * 4, e.size, "vector size");
            for (i, w) in words.iter().enumerate() {
                unsafe { put(base, off + 4 * i, &w.to_le_bytes()) };
            }
        }
        b'r' => {
            let vals: Vec<i32> = payload(v)
                .split(',')
                .map(|s| s.parse().expect("rotator component"))
                .collect();
            assert_eq!(vals.len() * 4, e.size, "rotator size");
            for (i, x) in vals.iter().enumerate() {
                unsafe { put(base, off + 4 * i, &x.to_le_bytes()) };
            }
        }
        b'c' => {
            let vals: Vec<i64> = payload(v)
                .split(',')
                .map(|s| s.parse().expect("color component"))
                .collect();
            assert_eq!(vals.len(), e.size, "color size");
            for (i, x) in vals.iter().enumerate() {
                unsafe { put(base, off + i, &[*x as u8]) };
            }
        }
        b's' => {
            assert_eq!(e.size, 16, "FString size");
            let text = payload(v);
            let (data, num) = if text.is_empty() {
                (EMPTY_STR.as_ptr() as usize, 0i32)
            } else {
                let units: Box<[u32]> = text
                    .bytes()
                    .map(u32::from)
                    .chain(std::iter::once(0))
                    .collect();
                let n = units.len() as i32;
                let data = units.as_ptr() as usize;
                strings.push(units);
                (data, n)
            };
            unsafe {
                put(base, off, &data.to_le_bytes());
                put(base, off + 8, &num.to_le_bytes());
                put(base, off + 12, &num.to_le_bytes());
            }
        }
        b'o' => {
            let ptrs: Vec<usize> = v.split(',').map(|s| addrs[ptr_index(s)]).collect();
            assert_eq!(ptrs.len(), e.count, "pointer array count");
            assert_eq!(e.size, 8 * e.count, "pointer array size");
            for (i, p) in ptrs.iter().enumerate() {
                unsafe { put(base, off + 8 * i, &p.to_le_bytes()) };
            }
        }
        other => panic!("unknown layout tag {}", other as char),
    }
}

fn write_to(st: &mut State, buf: usize, e: &Layout, val: &str) {
    let base = st.bufs[buf].as_mut_ptr().cast::<u8>();
    // SAFETY: `base` addresses a live OBJ_WORDS image; the layout entry lies inside it.
    unsafe { write_entry(base, e, val, &st.addrs, &mut st.strings) };
}

// ---- case replay ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Rep,
    Freq,
    Script,
    Novars,
}

struct Ctx {
    num: i32,
    native: u32,
    viewer: usize,
    old: usize,
    reject: bool,
    vdir: [u32; 3],
    vpos: [u32; 3],
}

impl Ctx {
    fn new() -> Self {
        Ctx {
            num: 0,
            native: 0,
            viewer: 0,
            old: 2,
            reject: false,
            vdir: [0; 3],
            vpos: [0; 3],
        }
    }
}

fn hex3(val: &str) -> [u32; 3] {
    let w = hex_words(payload(val));
    assert_eq!(w.len(), 3, "vector");
    [w[0], w[1], w[2]]
}

/// Applies one knob or field token, as the harness `apply`.
fn apply_token(fx: &Fixture, kind: usize, st: &mut State, ctx: &mut Ctx, tok: &str) {
    let (key, val) = tok.split_once('=').expect("token without '='");
    match key {
        "num" => ctx.num = val.parse().expect("num"),
        "native" => ctx.native = u32::from_str_radix(val, 16).expect("native bits"),
        "viewer" => ctx.viewer = ptr_index(val),
        "old" => ctx.old = ptr_index(val),
        "reject" => ctx.reject = val == "1",
        "vdir" => ctx.vdir = hex3(val),
        "vpos" => ctx.vpos = hex3(val),
        _ => {
            let (bufs, field, ty): (&[usize], &str, &str) = match key.split_once('.') {
                None => (&[THIS, RECENT], key, TYPE_NAMES[kind]),
                Some(("t", f)) => (&[THIS], f, TYPE_NAMES[kind]),
                Some(("r", f)) => (&[RECENT], f, TYPE_NAMES[kind]),
                Some(("d", f)) => (&[DEFAULTS], f, TYPE_NAMES[kind]),
                Some(("o", f)) => (&[PAWN], f, "APawn"),
                Some((p, _)) => panic!("unknown target prefix {p}"),
            };
            let entry = fx
                .layout
                .get(&(ty.to_owned(), field.to_owned()))
                .unwrap_or_else(|| panic!("no layout entry {ty}.{field}"));
            for &b in bufs {
                write_to(st, b, entry, val);
            }
        }
    }
}

fn set_world(fx: &Fixture, st: &State, ctx: &Ctx) {
    WORLD.with(|w| {
        let mut w = w.borrow_mut();
        w.native = ctx.native;
        w.reject = if ctx.reject { st.addrs[3] } else { 1 };
        w.defaults = st.addrs[4];
        w.class_addr = std::array::from_fn(class_ptr);
        w.class_off = fx.class_off();
    });
}

/// Views an object image as `T`. The image is at least `size_of::<T>()` bytes, checked by
/// `fixture_sizes_match_mirrors`, and 8-byte aligned, which covers the packed(4) mirrors.
fn obj<'a, T>(p: *const u8) -> &'a T {
    unsafe { &*p.cast::<T>() }
}

/// Runs a GetOptimizedRepList and returns the written indices.
fn run_rep(
    svc: &Hp2ActorRepServices,
    cache: &mut RepCache,
    kind: usize,
    this: *const u8,
    recent: *const u8,
    num: i32,
    out: &mut [i32; 256],
) -> Vec<i32> {
    let map = NonNull::<c_void>::dangling().as_ptr();
    let base = out.as_mut_ptr();
    let end = unsafe {
        let mut rep = Rep::new(svc, cache, map, base);
        match kind {
            K_ACTOR => {
                actor_get_optimized_rep_list(&mut rep, obj::<AActor>(this), obj::<AActor>(recent))
            }
            K_PAWN => {
                pawn_get_optimized_rep_list(&mut rep, obj::<APawn>(this), obj::<APawn>(recent), num)
            }
            K_PLAYERPAWN => player_pawn_get_optimized_rep_list(
                &mut rep,
                obj::<APlayerPawn>(this),
                obj::<APlayerPawn>(recent),
                num,
            ),
            K_MOVER => {
                mover_get_optimized_rep_list(&mut rep, obj::<AMover>(this), obj::<AMover>(recent))
            }
            K_ZONE => zone_info_get_optimized_rep_list(
                &mut rep,
                obj::<AZoneInfo>(this),
                obj::<AZoneInfo>(recent),
            ),
            K_LEVEL => level_info_get_optimized_rep_list(
                &mut rep,
                obj::<ALevelInfo>(this),
                obj::<ALevelInfo>(recent),
            ),
            K_PRI => player_replication_info_get_optimized_rep_list(
                &mut rep,
                obj::<APlayerReplicationInfo>(this),
                obj::<APlayerReplicationInfo>(recent),
            ),
            K_GRI => game_replication_info_get_optimized_rep_list(
                &mut rep,
                obj::<AGameReplicationInfo>(this),
                obj::<AGameReplicationInfo>(recent),
            ),
            K_INV => inventory_get_optimized_rep_list(
                &mut rep,
                obj::<AInventory>(this),
                obj::<AInventory>(recent),
                num,
            ),
            K_WEAPON => weapon_get_optimized_rep_list(
                &mut rep,
                obj::<AWeapon>(this),
                obj::<AWeapon>(recent),
                num,
            ),
            K_AMMO => {
                ammo_get_optimized_rep_list(&mut rep, obj::<AAmmo>(this), obj::<AAmmo>(recent), num)
            }
            K_PICKUP => pickup_get_optimized_rep_list(
                &mut rep,
                obj::<APickup>(this),
                obj::<APickup>(recent),
                num,
            ),
            other => unreachable!("kind {other} has no rep function"),
        }
        rep.finish()
    };
    let n = (end as usize - base as usize) / size_of::<i32>();
    out[..n].to_vec()
}

fn run_freq(
    svc: &Hp2ActorRepServices,
    kind: usize,
    this: *const u8,
    viewer: usize,
    ctx: &Ctx,
) -> f32 {
    let dir = FVector {
        x: f32::from_bits(ctx.vdir[0]),
        y: f32::from_bits(ctx.vdir[1]),
        z: f32::from_bits(ctx.vdir[2]),
    };
    let pos = FVector {
        x: f32::from_bits(ctx.vpos[0]),
        y: f32::from_bits(ctx.vpos[1]),
        z: f32::from_bits(ctx.vpos[2]),
    };
    match kind {
        K_ACTOR => actor_update_frequency(svc, obj::<AActor>(this), &dir, &pos),
        K_INV => inventory_update_frequency(svc, obj::<AInventory>(this), viewer as *const c_void),
        other => unreachable!("kind {other} has no frequency function"),
    }
}

fn run_case(
    svc: &Hp2ActorRepServices,
    cache: &mut RepCache,
    kind: usize,
    mode: Mode,
    st: &State,
    ctx: &Ctx,
) -> String {
    let this = st.addrs[THIS + 1] as *const u8;
    let recent = st.addrs[RECENT + 1] as *const u8;
    match mode {
        Mode::Rep => {
            let mut out = [0i32; 256];
            let idx = run_rep(svc, cache, kind, this, recent, ctx.num, &mut out);
            let list: Vec<String> = idx.iter().map(i32::to_string).collect();
            format!("res n={} idx={}", idx.len(), list.join(","))
        }
        Mode::Freq => {
            let f = run_freq(svc, kind, this, st.addrs[ctx.viewer], ctx);
            format!("res f={:08x}", f.to_bits())
        }
        Mode::Script => {
            let b = inventory_should_do_script_replication(obj::<AInventory>(this));
            format!("res b={}", i32::from(b))
        }
        Mode::Novars => {
            let old = st.addrs[ctx.old];
            assert!(old != 0, "old=null cannot be replayed");
            let b = inventory_no_variables_to_replicate(
                obj::<AInventory>(this),
                obj::<AActor>(old as *const u8),
            );
            format!("res b={}", i32::from(b))
        }
    }
}

fn replay_family(family: &str, kind: usize, mode: Mode, expected: usize) {
    let fx = fixture();
    let svc = services();
    let mut cache = RepCache::default();
    let mut got = String::new();
    let mut count = 0usize;
    for case in fx.cases.iter().filter(|c| c.family == family) {
        assert_eq!(case.idx, count, "{family}: case index out of order");
        let mut st = State::new(kind, fx.class_off());
        let mut ctx = Ctx::new();
        for tok in &case.tokens {
            apply_token(&fx, kind, &mut st, &mut ctx, tok);
        }
        set_world(&fx, &st, &ctx);
        got.push_str(&case_line(case));
        got.push('\n');
        got.push_str(&run_case(&svc, &mut cache, kind, mode, &st, &ctx));
        got.push('\n');
        count += 1;
    }
    assert_eq!(count, expected, "{family}: record count");
    assert_eq!(
        got,
        fx.family_text(family),
        "{family}: replay diverges from fixture"
    );
}

// ---- tests ---------------------------------------------------------------------------------

macro_rules! check_size {
    ($fx:expr, $($ty:ident),+ $(,)?) => {
        $(
            assert_eq!(
                $fx.sizes[stringify!($ty)],
                size_of::<$ty>(),
                "sizeof {} disagrees with the oracle",
                stringify!($ty)
            );
        )+
    };
}

#[test]
fn fixture_sizes_match_mirrors() {
    let fx = fixture();
    assert_eq!(fx.cases.len(), 4615, "total record count");
    check_size!(
        fx,
        AActor,
        APawn,
        APlayerPawn,
        AMover,
        AZoneInfo,
        ALevelInfo,
        APlayerReplicationInfo,
        AGameReplicationInfo,
        AInventory,
        AWeapon,
        AAmmo,
        APickup,
    );
}

macro_rules! family_test {
    ($name:ident, $family:literal, $kind:expr, $mode:expr, $count:literal) => {
        #[test]
        fn $name() {
            replay_family($family, $kind, $mode, $count);
        }
    };
}

family_test!(replay_actor_rep, "actor_rep", K_ACTOR, Mode::Rep, 246);
family_test!(replay_actor_freq, "actor_freq", K_ACTOR, Mode::Freq, 313);
family_test!(replay_pawn_rep, "pawn_rep", K_PAWN, Mode::Rep, 297);
family_test!(
    replay_player_pawn_rep,
    "player_pawn_rep",
    K_PLAYERPAWN,
    Mode::Rep,
    339
);
family_test!(replay_mover_rep, "mover_rep", K_MOVER, Mode::Rep, 267);
family_test!(replay_zone_rep, "zone_rep", K_ZONE, Mode::Rep, 273);
family_test!(replay_level_rep, "level_rep", K_LEVEL, Mode::Rep, 288);
family_test!(replay_pri_rep, "pri_rep", K_PRI, Mode::Rep, 315);
family_test!(replay_gri_rep, "gri_rep", K_GRI, Mode::Rep, 306);
family_test!(replay_inv_rep, "inv_rep", K_INV, Mode::Rep, 279);
family_test!(replay_inv_freq, "inv_freq", K_INV, Mode::Freq, 279);
family_test!(replay_inv_script, "inv_script", K_INV, Mode::Script, 279);
family_test!(replay_inv_novars, "inv_novars", K_INV, Mode::Novars, 279);
family_test!(replay_weapon_rep, "weapon_rep", K_WEAPON, Mode::Rep, 288);
family_test!(replay_ammo_rep, "ammo_rep", K_AMMO, Mode::Rep, 285);
family_test!(replay_pickup_rep, "pickup_rep", K_PICKUP, Mode::Rep, 282);
