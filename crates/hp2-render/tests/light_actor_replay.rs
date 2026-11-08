#![cfg(target_arch = "aarch64")]
//! Replays `fixtures/light_actor.txt`, written by `Build/light_fixture_harness/sec_actor.inc`
//! from the oracle actor paths in `UnLight.cpp` (c9fc381). Floats are compared by bit pattern.

use std::cell::RefCell;
use std::ffi::{c_char, c_void};
use std::ptr::null_mut;
use std::sync::{LazyLock, Mutex};

use hp2_abi::engine::generated::{AActor, TArrayRaw, ULevel, UModel};
use hp2_abi::render_scene::{FActorLink, FVolActorLink};
use hp2_render::light::PF_RENDER_FOG;
use hp2_render::light_actor::{
    ActorBeginIo, CONSIDER_CAP, CandidateIo, LightAddIo, LightConsiderSet, LightRec,
    MAX_ACTOR_LIGHTS, SenderEntry, SetupTailIo, actor_begin, add_light_decide, collect_candidates,
    color_scale, finish_actor, particle_accumulate, setup_tail,
};
use hp2_render::light_manager;
use hp2_render::services_light::Hp2LightServices;

const FIXTURE: &str = include_str!("fixtures/light_actor.txt");

// Sentinel address for `io.actor` in the addlight test: never dereferenced,
// only compared by identity against `io.light`.
const ACTOR: usize = 0x10;

fn records(tag: &str) -> Vec<&'static str> {
    FIXTURE
        .lines()
        .filter(|l| l.split_whitespace().next() == Some(tag))
        .collect()
}

struct Cur<'a>(std::str::SplitWhitespace<'a>);

impl<'a> Cur<'a> {
    fn new(line: &'a str) -> Self {
        Cur(line.split_whitespace())
    }
    fn s(&mut self) -> &'a str {
        self.0.next().expect("truncated fixture record")
    }
    fn i(&mut self) -> i64 {
        self.s().parse().expect("fixture int")
    }
    fn u(&mut self) -> u32 {
        self.s().parse().expect("fixture uint")
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.s(), 16).expect("fixture float bits"))
    }
    fn expect(&mut self, tok: &str) {
        assert_eq!(self.s(), tok, "fixture token");
    }
}

fn bits<const N: usize>(v: [f32; N]) -> [u32; N] {
    v.map(f32::to_bits)
}

#[derive(Clone, Copy)]
struct LightState {
    location: [f32; 3],
    radius: f32,
    extra: i32,
    lighting: i32,
    special: i32,
    index: i32,
    brightness: u32,
    special_lit: u32,
    is_static: u32,
    is_movable: u32,
    delete_me: u32,
    visible: u32,
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Line(usize, bool),
}

#[derive(Default)]
struct Replay {
    lights: Vec<LightState>,

    events: Vec<Event>,
    /// `finish_actor` callbacks in call order.
    calls: Vec<Call>,
}

#[derive(Debug, PartialEq, Eq)]
enum Call {
    MarkPop,
    Unlock,
}

thread_local! {
    static REPLAY: RefCell<Replay> = RefCell::new(Replay::default());
}

fn with<R>(f: impl FnOnce(&mut Replay) -> R) -> R {
    REPLAY.with(|r| f(&mut r.borrow_mut()))
}

/// Backing capacity for `light_ptr`/`light_pos`: large enough for the fixture's
/// largest `cand` record (130 lights, positions up to 129).
const LIGHT_CAP: usize = 256;

thread_local! {
    /// Real `AActor` memory `light_ptr(p)` points into: `fill_rec`/`write_back`/
    /// `add_light_decide` now dereference candidate/light pointers directly, so
    /// they can no longer be opaque sentinel addresses. Fixed length, filled in
    /// place by `seed_lights`, so the backing buffer's address never moves.
    static LIGHTS: RefCell<Vec<AActor>> =
        RefCell::new(unsafe { vec![std::mem::zeroed(); LIGHT_CAP] });
}

fn light_ptr(p: usize) -> *mut c_void {
    assert!(p < LIGHT_CAP, "light position {p} exceeds LIGHT_CAP");
    LIGHTS.with(|l| unsafe { l.borrow_mut().as_mut_ptr().add(p) as *mut c_void })
}

fn light_pos(ptr: *mut c_void) -> usize {
    LIGHTS.with(|l| {
        let base = l.borrow().as_ptr() as usize;
        let stride = std::mem::size_of::<AActor>();
        let a = ptr as usize;
        assert!(
            a >= base && (a - base).is_multiple_of(stride),
            "not a light pointer: {a:#x}"
        );
        (a - base) / stride
    })
}

/// Writes each fixture `LightState` into its position's backing `AActor`
/// (`fill_rec`'s direct-mirror reads: location, tags, index, brightness, and
/// the special-lit/static/movable bitfields. `radius` stays a service call).
fn seed_lights(lights: &[LightState]) {
    LIGHTS.with(|l| {
        let mut backing = l.borrow_mut();
        for (p, state) in lights.iter().enumerate() {
            let a = &mut backing[p];
            a.location = hp2_abi::FVector {
                x: state.location[0],
                y: state.location[1],
                z: state.location[2],
            };
            a.extra_tag = state.extra;
            a.lighting_tag = state.lighting;
            a.special_tag = state.special;
            a.index = state.index;
            a.light_brightness = state.brightness as u8;
            a.set_b_special_lit(state.special_lit != 0);
            a.set_b_static(state.is_static != 0);
            a.set_b_movable(state.is_movable != 0);
        }
    });
}

fn blank_rec() -> LightRec {
    LightRec {
        actor: null_mut(),
        location: [0.0; 3],
        radius: 0.0,
        extra_tag: 0,
        lighting_tag: 0,
        special_tag: 0,
        index: 0,
        brightness: 0,
        special_lit: 0,
        is_static: 0,
        is_movable: 0,
    }
}

// Candidate replay uses only collector services. These default callbacks panic if accidentally
// reached; the setup-tail tests override the tail callbacks with their own state.
macro_rules! stub {
    ($($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {$(
        #[allow(unused_variables)]
        unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? {
            panic!("light_actor replay: {} is not on the actor path", stringify!($name));
        }
    )*};
}

stub! {
    check_failed(expr: *const c_char, line: i32);
    owner_life_fraction(owner: *mut c_void) -> f32;
    world_volumetric_radius(actor: *mut c_void) -> f32;
    view_vector(actor: *mut c_void, out: *mut c_void);
    map_uncoords(map_coords: *const c_void, out: *mut c_void);
    global_hsv(hue: u8, sat: u8, out: *mut c_void);
    cache_get(kind: u32, actor: *mut c_void, out: *mut *mut c_void) -> u32;
    cache_create(kind: u32, actor: *mut c_void, size: u32, out: *mut *mut c_void);
    stat_clock();
    stat_unclock();
    surf_map_setup(ctx: *mut c_void, which: u32);
    surf_zone_ambient(out: *mut c_void);
    surf_cache_get(id: u64) -> *mut c_void;
    surf_cache_create(id: u64, size: u64, pad: u64, check: u32) -> *mut c_void;
    surf_gmem(bytes: u64) -> *mut c_void;
    surf_gmem_mark(slot: u32, pop: u32);
    surf_light_compute(i: u32, which: u32);
    surf_light_spatial_fx(i: u32, shadow: *mut c_void);
    surf_shadow_gen(i: u32, use_bits: u32, dest: *mut c_void);
    random_base_data() -> *const f32;
    random_data() -> *const f32;
    light_compute(index: u32, frame: *mut c_void);
    stat_vtric_inc();
    stat_mesh_light_add(n: u32);
}

// `WorldLightRadius`: `fill_rec` now reads this directly off the service (the
// only `LightRec` field the `AActor` mirror doesn't provide), keyed by the
// fixture's `LightState.radius` through the same position lookup `line_check` uses.
unsafe extern "C" fn svc_world_light_radius(actor: *mut c_void) -> f32 {
    with(|r| r.lights[light_pos(actor)].radius)
}

// `finish_actor` callbacks, recorded in call order. `pop_unlock` underflows `TopItemToUnlock`
// silently (raw pointer subtraction, not a checked counter), so `svc_unlock_one` asserts a
// pending item first to still catch an extra `unlock_one` call as a clean test failure.
unsafe extern "C" fn svc_mark_pop() {
    with(|r| r.calls.push(Call::MarkPop));
}

// `unlock_pending` now reads `light_manager::State.TopItemToUnlock` directly; `unlock_one`
// mirrors the real leaf (`hp2_rs_light_manager_pop_unlock`), popping the same stack so the
// `finish_actor` loop below still terminates.
unsafe extern "C" fn svc_unlock_one() {
    assert!(
        unsafe { light_manager::unlock_pending() },
        "unlock_one called with nothing pending"
    );
    unsafe { light_manager::pop_unlock() };
    with(|r| r.calls.push(Call::Unlock));
}

// `UnLight.cpp` `LineCheck` stand-in: the fixture records the oracle's result and call order.
unsafe extern "C" fn svc_line_check(_actor: *mut c_void, light: *mut c_void) -> u32 {
    with(|r| {
        let p = light_pos(light);
        let visible = r.lights[p].visible != 0;
        r.events.push(Event::Line(p, visible));
        visible as u32
    })
}

unsafe extern "C" fn svc_add_light(_actor: *mut c_void, _light: *mut c_void) -> u32 {
    1
}

// `GetIndexedObject(index)==actor && !actor->bDeleteMe`: the first light with this Index.
unsafe extern "C" fn svc_sender_alive(index: i32, actor: *mut c_void) -> u32 {
    with(|r| match r.lights.iter().position(|l| l.index == index) {
        Some(p) => (light_ptr(p) == actor && r.lights[p].delete_me == 0) as u32,
        None => 0,
    })
}

fn services() -> Hp2LightServices {
    Hp2LightServices {
        check_failed,
        line_check: svc_line_check,
        add_light: svc_add_light,
        sender_alive: svc_sender_alive,
        owner_life_fraction,
        world_light_radius: svc_world_light_radius,
        world_volumetric_radius,
        view_vector,
        map_uncoords,
        global_hsv,
        cache_get,
        cache_create,
        stat_clock,
        stat_unclock,
        surf_map_setup,
        surf_zone_ambient,
        surf_cache_get,
        surf_cache_create,
        surf_gmem,
        surf_gmem_mark,
        surf_light_compute,
        surf_light_spatial_fx,
        surf_shadow_gen,
        random_base_data,
        random_data,
        light_compute,
        stat_vtric_inc,
        stat_mesh_light_add,
        unlock_one: svc_unlock_one,
        mark_pop: svc_mark_pop,
    }
}

#[test]
fn begin_matches_oracle() {
    let recs = records("begin");
    assert_eq!(recs.len(), 48);
    for line in recs {
        let mut c = Cur::new(line);
        c.s();
        let mut io = ActorBeginIo {
            scale_glow: c.f(),
            specular_glow: c.f(),
            specular_width: c.u(),
            ambient_glow: c.u(),
            time: c.f(),
            hsv: [c.f(), c.f(), c.f()],
            diffuse: 0.0,
            specular: 0.0,
            spec_min_cos: 0.0,
            ambient: [0.0; 4],
        };
        c.expect("->");
        let want = [c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f()];
        actor_begin(&mut io);
        let got = [
            io.diffuse,
            io.specular,
            io.spec_min_cos,
            io.ambient[0],
            io.ambient[1],
            io.ambient[2],
            io.ambient[3],
        ];
        assert_eq!(bits(got), bits(want), "{line}");
    }
}

#[test]
fn addlight_matches_oracle() {
    let recs = records("addlight");
    assert_eq!(recs.len(), 1024);
    // Real backing memory for `io.light`: `add_light_decide` now dereferences it
    // directly as an `AActor` (`LightType`/`LightEffect`/`LightBrightness`/the
    // dynamic/static/no-delete/changed bitfields). Reused and overwritten per
    // record; `io.actor` stays a never-dereferenced sentinel (only pointer
    // identity against `io.light` matters for it).
    let mut light_actor: AActor = unsafe { std::mem::zeroed() };
    for line in recs {
        let mut c = Cur::new(line);
        c.s();
        let has = c.u() != 0;
        let same = c.u() != 0;
        let light_type = c.u();
        let effect = c.u();
        let brightness = c.u();
        let dynamic = c.u();
        let stat = c.u();
        let nodel = c.u();
        let changed = c.u();
        let last_full = c.u() != 0;
        let spatial = c.u() != 0;
        let merge = c.u() != 0;
        let no_dyn = c.u() != 0;
        let slc0 = c.u();
        c.expect("->");
        let want = [c.u(), c.u(), c.u(), c.u(), c.u(), c.u()];

        light_actor.light_type = light_type as u8;
        light_actor.light_effect = effect as u8;
        light_actor.light_brightness = brightness as u8;
        light_actor.set_b_dynamic_light(dynamic != 0);
        light_actor.set_b_static(stat != 0);
        light_actor.set_b_no_delete(nodel != 0);
        light_actor.set_b_light_changed(changed != 0);
        let light_ptr = &mut light_actor as *mut AActor as *mut c_void;

        let mut io = LightAddIo {
            actor: if has {
                if same {
                    light_ptr
                } else {
                    ACTOR as *mut c_void
                }
            } else {
                null_mut()
            },
            light: light_ptr,
            accept: 0,
            opt: 0,
            moving_delta: 0,
            static_delta: 0,
            dynamic_delta: 0,
            static_lighting_changed: 0,
        };
        // The shell refuses before AddLight runs when the list is full (`actor_last >= actor_final`).
        if !last_full {
            let flags = (spatial as u32) | ((merge as u32) << 1);
            add_light_decide(&mut io, || no_dyn, move |_| flags);
        }
        let accept = io.accept;
        // Opt is written only on accept; the oracle keeps its 255 sentinel otherwise.
        let opt = if accept != 0 { io.opt } else { 255 };
        let got = [
            accept,
            opt,
            io.moving_delta,
            io.static_delta,
            io.dynamic_delta,
            slc0 | io.static_lighting_changed,
        ];
        assert_eq!(got, want, "{line}");
    }
}

#[test]
fn color_matches_oracle() {
    let recs = records("color");
    assert_eq!(recs.len(), 64);
    for line in recs {
        let mut c = Cur::new(line);
        c.s();
        let special = c.i() as i32;
        let dark = c.u() != 0;
        let mut color = [c.f(), c.f(), c.f(), c.f()];
        c.expect("->");
        let want = [c.f(), c.f(), c.f(), c.f()];
        color_scale(&mut color, special, dark);
        assert_eq!(bits(color), bits(want), "{line}");
    }
}

#[test]
fn particle_matches_oracle() {
    let recs = records("particle");
    assert_eq!(recs.len(), 64);
    for line in recs {
        let mut c = Cur::new(line);
        c.s();
        let max_spec = [c.f(), c.f(), c.f()];
        let light_dir = [c.f(), c.f(), c.f()];
        let scaled = [c.f(), c.f(), c.f(), c.f()];
        let mut acc = [c.f(), c.f(), c.f(), c.f()];
        c.expect("->");
        let want = [c.f(), c.f(), c.f(), c.f()];
        particle_accumulate(&mut acc, &scaled, max_spec, light_dir);
        assert_eq!(bits(acc), bits(want), "{line}");
    }
}

#[test]

fn cand_matches_oracle() {
    let recs = records("cand");
    assert_eq!(recs.len(), 110);
    let svc = services();
    // `model_light` now reads `light_manager::State.Level` directly.
    let _lock = LIGHT_MANAGER_STATE_LOCK.lock().unwrap();
    for line in recs {
        let mut c = Cur::new(line);
        c.s();
        let tag = c.i() as i32;
        let delta = c.i() as i32;
        let frame = c.i() as i32;
        let first_see = c.u();
        let permeating = c.i() as i32;
        let actor_location = [c.f(), c.f(), c.f()];
        let actor_special_lit = c.u();
        let nl = c.i() as usize;
        let mut lights = Vec::with_capacity(nl);
        for _ in 0..nl {
            let location = [c.f(), c.f(), c.f()];
            let radius = c.f();
            lights.push(LightState {
                location,
                radius,
                extra: c.i() as i32,
                lighting: c.i() as i32,
                special: c.i() as i32,
                index: c.i() as i32,
                brightness: c.u(),
                special_lit: c.u(),
                is_static: c.u(),
                is_movable: c.u(),
                delete_me: c.u(),
                visible: c.u(),
            });
        }
        let nm = c.i() as usize;
        let model: Vec<usize> = (0..nm).map(|_| c.i() as usize).collect();
        let nk = c.i() as usize;
        let links: Vec<usize> = (0..nk).map(|_| c.i() as usize).collect();
        let ns = c.i() as usize;
        let mut senders = Vec::with_capacity(ns);
        for _ in 0..ns {
            let pos = c.i() as i32;
            let index = c.i() as i32;
            let factor = c.i() as u8;
            senders.push((pos, index, factor));
        }
        c.expect("->");
        let ne = c.i() as usize;
        let mut want_events = Vec::with_capacity(ne);
        for _ in 0..ne {
            let ev = match c.s() {
                "C" => {
                    let p = c.i() as usize;
                    Event::Line(p, c.i() != 0)
                }
                other => panic!("bad event kind {other}"),
            };
            want_events.push(ev);
        }
        let ncons = c.i() as usize;
        let want_cons: Vec<usize> = (0..ncons).map(|_| c.i() as usize).collect();
        let nsnd = c.i() as usize;
        let mut want_send = Vec::with_capacity(nsnd);
        for _ in 0..nsnd {
            let pos = c.i() as usize;
            let factor = c.i() as u8;
            want_send.push((pos, factor));
        }
        c.expect("F");
        let want_f: Vec<(i32, i32, i32)> = (0..nl)
            .map(|_| (c.i() as i32, c.i() as i32, c.i() as i32))
            .collect();

        // Sender block: a cache miss (FirstSeeActor) starts with no senders.
        let mut entries = [SenderEntry {
            actor: null_mut(),
            factor: 0,
            index: 0,
        }; MAX_ACTOR_LIGHTS];
        for (slot, &(pos, index, factor)) in entries.iter_mut().zip(&senders) {
            *slot = SenderEntry {
                actor: if pos >= 0 {
                    light_ptr(pos as usize)
                } else {
                    null_mut()
                },
                factor,
                index,
            };
        }
        let mut num = if first_see != 0 { 0 } else { ns as i32 };

        // `collect_candidates` now reads the leaf-light list via
        // `light_manager::read_vol_actor_link`, which casts the pointer straight to
        // `FVolActorLink` and dereferences it: build a real chain instead of the opaque-index
        // scheme the removed `link_read` service used to decode.
        let mut link_nodes: Vec<FVolActorLink> = links
            .iter()
            .map(|&p| FVolActorLink {
                location: hp2_abi::FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                actor: light_ptr(p),
                next: null_mut(),
                volumetric: 0,
            })
            .collect();
        let link_base = link_nodes.as_mut_ptr();
        for i in 0..link_nodes.len() {
            let next = if i + 1 < link_nodes.len() {
                unsafe { link_base.add(i + 1) }
            } else {
                null_mut()
            };
            unsafe { (*link_base.add(i)).next = next };
        }

        let mut set = LightConsiderSet {
            tag,
            num: 0,
            actor_location: [0.0; 3],
            actor_special_lit: 0,
            recs: [blank_rec(); CONSIDER_CAP],
        };
        let io = CandidateIo {
            actor: ACTOR as *mut c_void,
            link_head: if nk > 0 { link_base.cast() } else { null_mut() },
            actor_location,
            actor_special_lit,
            permeating,
            delta,
            frame_count: frame,
            first_see,
        };
        // `fill_rec`/`write_back` now dereference candidate pointers directly as
        // `AActor`; seed the real backing memory `light_ptr` hands out.
        seed_lights(&lights);
        // `model_light` now reads `Level->Model->Lights(i)` directly: build a real
        // `ULevel`/`UModel` chain whose `Lights` TArray holds the same `light_ptr`
        // addresses the removed `model_light` service used to return by position.
        // The old mock's `r.model.get(i)` naturally returned `None` (null) past
        // `nm`; `model_light`'s unchecked index needs real memory there too, so
        // the loop still finds its terminator one past the real entries.
        let nm = model.len();
        let mut model_ptrs: Vec<*mut c_void> = model.iter().map(|&p| light_ptr(p)).collect();
        model_ptrs.resize(nm + 8, null_mut());
        let mut model_obj: UModel = unsafe { std::mem::zeroed() };
        model_obj.lights = TArrayRaw {
            data: model_ptrs.as_mut_ptr().cast(),
            num: nm as i32,
            max: nm as i32,
        };
        let mut level_obj: ULevel = unsafe { std::mem::zeroed() };
        level_obj.model = (&raw mut model_obj).cast();
        unsafe { (*light_manager::state()).Level = (&raw mut level_obj).cast() };
        with(|r| {
            *r = Replay {
                lights,

                events: Vec::new(),
                ..Default::default()
            }
        });

        collect_candidates(&svc, &mut set, &io, &mut entries, &mut num);

        let got_events = with(|r| std::mem::take(&mut r.events));
        assert_eq!(got_events, want_events, "events: {line}");
        let got_cons: Vec<usize> = set.recs[..set.num as usize]
            .iter()
            .map(|rec| light_pos(rec.actor))
            .collect();
        assert_eq!(got_cons, want_cons, "considered: {line}");
        let got_send: Vec<(usize, u8)> = entries[..num as usize]
            .iter()
            .map(|e| (light_pos(e.actor), e.factor))
            .collect();
        assert_eq!(got_send, want_send, "senders: {line}");
        // `write_back` now writes `extra_tag`/`lighting_tag`/`special_tag` straight
        // into the `AActor` mirror, not into `Replay.lights` (which stays the
        // untouched input): read the backing memory `light_ptr` points at.
        let got_f: Vec<(i32, i32, i32)> = (0..nl)
            .map(|p| {
                let a = unsafe { &*(light_ptr(p) as *const AActor) };
                (a.extra_tag, a.lighting_tag, a.special_tag)
            })
            .collect();
        assert_eq!(got_f, want_f, "tags: {line}");
    }
}

/// Tests below touch the single global `light_manager::State`: `finish_actor`'s
/// `unlock_pending`/`unlock_one` and `setup_tail`'s light list. Serialize them against each
/// other the same way `light_surf_replay.rs` does.
static LIGHT_MANAGER_STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Pushes `count` dummy entries onto `State.ItemsToUnlock`, mirroring the cache locks
/// `FLightManager::SetupForActor` would have accumulated before `FinishActor` runs.
unsafe fn seed_unlock_pending(count: u32) {
    unsafe {
        let s = light_manager::state();
        (*s).TopItemToUnlock = std::ptr::addr_of_mut!((*s).ItemsToUnlock)
            .cast::<*mut c_void>()
            .add(count as usize);
    }
}

#[test]
fn finish_releases_then_unlocks_every_pending_item() {
    let _lock = LIGHT_MANAGER_STATE_LOCK.lock().unwrap();
    for locked in [0u32, 1, 3] {
        with(|r| {
            *r = Replay::default();
        });
        unsafe { seed_unlock_pending(locked) };
        finish_actor(&services());
        let calls = with(|r| std::mem::take(&mut r.calls));
        let mut want = vec![Call::MarkPop];
        want.extend((0..locked).map(|_| Call::Unlock));
        assert_eq!(calls, want, "locked={locked}");
        assert!(
            !unsafe { light_manager::unlock_pending() },
            "locked={locked}"
        );
    }
}
const TAIL_FRAME: usize = 0x30_0000;
const ALO_NOT_LIGHT: u32 = 3;

#[derive(Debug, PartialEq, Eq)]
enum TailEvent {
    Compute(u32, usize),
}

/// `light_count`, `light_has_room`, `light_actor_at`, `light_append`, `light_set_volumetric`,
/// `light_is_volumetric`, `light_float_color`, `vtric_reset`, `vtric_push`, `vtric_count`,
/// `vtric_sort` and `actor_link_read` now read/write `light_manager::State` directly instead of
/// going through mockable leaves. `light_tags` was removed: `setup_tail` now reads
/// `special_tag`/`b_dark_light` straight off the `AActor` mirror (`light_actor_at(i)`), so test
/// actors must be real, dereferenceable `AActor` memory instead of opaque addresses. Only
/// `light_compute`, `stat_vtric_inc` and `stat_mesh_light_add` remain real callbacks.
#[derive(Default)]
struct TailReplay {
    events: Vec<TailEvent>,
    stat_vtric: u32,
    stat_mesh_light: u32,
}

thread_local! {
    static TAIL: RefCell<TailReplay> = RefCell::new(TailReplay::default());
}

fn with_tail<R>(f: impl FnOnce(&mut TailReplay) -> R) -> R {
    TAIL.with(|r| f(&mut r.borrow_mut()))
}

/// Builds a real `FActorLink` chain (`read_actor_link`'s target), one node per actor, in test
/// memory instead of the opaque-index scheme the removed `actor_link_read` service used to decode.
fn build_actor_link_chain(actors: &[usize]) -> Vec<FActorLink> {
    let mut nodes: Vec<FActorLink> = actors
        .iter()
        .map(|&a| FActorLink {
            actor: a as *mut c_void,
            next: null_mut(),
        })
        .collect();
    let base = nodes.as_mut_ptr();
    for i in 0..nodes.len() {
        let next = if i + 1 < nodes.len() {
            unsafe { base.add(i + 1) }
        } else {
            null_mut()
        };
        unsafe { (*base.add(i)).next = next };
    }
    nodes
}

/// A zeroed `AActor` with `SpecialTag`/`bDarkLight` set, kept alive by the caller, so
/// `setup_tail`'s direct mirror read (`light_actor_at(i)` dereferenced as `AActor`) has real
/// memory to read instead of an opaque test address.
fn make_actor(special_tag: i32, dark_light: bool) -> Box<AActor> {
    let mut a: Box<AActor> = unsafe { Box::new(std::mem::zeroed()) };
    a.special_tag = special_tag;
    a.set_b_dark_light(dark_light);
    a
}

/// Seeds the `FirstLight` prefix `FLightManager::SetupForActor`'s candidate pass would have
/// gathered before `SetupTail` runs: actor identity (a real `AActor*`) and starting float color.
unsafe fn seed_tail_light_manager(existing: &[(usize, [f32; 4])], capacity: usize) {
    unsafe {
        let s = light_manager::state();
        (*s).FirstLight = std::mem::zeroed();
        for (i, &(actor, color)) in existing.iter().enumerate() {
            let slot = &mut (*s).FirstLight[i];
            slot.Actor = actor as *mut c_void;
            slot.FloatColor = color;
            // Distinct, strictly-positive Z per pre-existing slot (descending by index) so
            // `vtric_sort`'s comparator (`2*(a.z<b.z)-1`, never 0, not antisymmetric for ties)
            // never compares equal Z values against a freshly-appended light's zeroed Location
            // (`light_append` never sets `Location`); the sort order is then determined by the
            // comparator's logic rather than by an unspecified qsort tie-break.
            slot.Location = [0.0, 0.0, (existing.len() - i) as f32];
        }
        (*s).LastLight = std::ptr::addr_of_mut!((*s).FirstLight)
            .cast::<light_manager::LightInfo>()
            .add(existing.len());
        (*s).FinalLight = std::ptr::addr_of_mut!((*s).FirstLight)
            .cast::<light_manager::LightInfo>()
            .add(capacity);
    }
}

unsafe extern "C" fn tail_light_compute(index: u32, frame: *mut c_void) {
    with_tail(|r| r.events.push(TailEvent::Compute(index, frame as usize)));
}

unsafe extern "C" fn tail_stat_vtric_inc() {
    with_tail(|r| r.stat_vtric += 1);
}

unsafe extern "C" fn tail_stat_mesh_light_add(count: u32) {
    with_tail(|r| r.stat_mesh_light += count);
}

fn tail_services() -> Hp2LightServices {
    let mut svc = services();
    svc.light_compute = tail_light_compute;
    svc.stat_vtric_inc = tail_stat_vtric_inc;
    svc.stat_mesh_light_add = tail_stat_mesh_light_add;
    svc
}

/// Reads the `i`-th real `FirstVtric` entry back to its `FirstLight` index.
unsafe fn vtric_index_at(i: u32) -> u32 {
    unsafe {
        let s = light_manager::state();
        (*s).FirstVtric[i as usize].offset_from(std::ptr::addr_of!((*s).FirstLight).cast()) as u32
    }
}

#[test]
fn setup_tail_replays_volumetric_append_and_room_limit() {
    let _lock = LIGHT_MANAGER_STATE_LOCK.lock().unwrap();
    let mut actor_a = make_actor(128, false);
    let mut actor_b = make_actor(64, true);
    // The appended slot's tags are read after `light_append` (zeroed `FloatColor`), straight off
    // this actor's `AActor` mirror; special=255 (s=1.0) keeps `color_scale` a no-op either way.
    let mut actor_appended = make_actor(255, false);
    let actor_unprocessed = make_actor(0, false);
    let existing_a = std::ptr::addr_of_mut!(*actor_a) as usize;
    let existing_b = std::ptr::addr_of_mut!(*actor_b) as usize;
    let appended = std::ptr::addr_of_mut!(*actor_appended) as usize;
    // Room runs out before this one is ever reached, so it is never dereferenced.
    let unprocessed = std::ptr::addr_of!(*actor_unprocessed) as usize;

    with_tail(|r| *r = TailReplay::default());
    unsafe {
        seed_tail_light_manager(
            &[
                (existing_a, [1.0, 2.0, -3.0, 4.0]),
                (existing_b, [-1.0, 0.5, 2.0, -4.0]),
            ],
            3,
        );
    }

    let link_nodes = build_actor_link_chain(&[existing_b, appended, unprocessed]);
    let mut io = SetupTailIo {
        frame: TAIL_FRAME as *mut c_void,
        volumetrics: link_nodes.as_ptr() as *mut c_void,
        result: 0x20,
        pad: 0,
    };
    setup_tail(&tail_services(), &mut io);

    assert_eq!(io.result, 0x20 | PF_RENDER_FOG);
    let (actors, volumetric, opts, colors) = unsafe {
        let s = light_manager::state();
        let n = light_manager::light_count();
        let actors: Vec<usize> = (0..n).map(|i| (*s).FirstLight[i].Actor as usize).collect();
        let volumetric: Vec<bool> = (0..n)
            .map(|i| (*s).FirstLight[i].IsVolumetric != 0)
            .collect();
        let opts: Vec<i32> = (0..n).map(|i| (*s).FirstLight[i].Opt).collect();
        let colors: Vec<[f32; 4]> = (0..n).map(|i| (*s).FirstLight[i].FloatColor).collect();
        (actors, volumetric, opts, colors)
    };
    assert_eq!(actors, [existing_a, existing_b, appended]);
    assert_eq!(volumetric, [false, true, true]);
    assert_eq!(opts, [0, 0, ALO_NOT_LIGHT as i32]);

    let mut expected_a = [1.0, 2.0, -3.0, 4.0];
    color_scale(&mut expected_a, 128, false);
    let mut expected_b = [-1.0, 0.5, 2.0, -4.0];
    color_scale(&mut expected_b, 64, true);
    assert_eq!(bits(colors[0]), bits(expected_a));
    assert_eq!(bits(colors[1]), bits(expected_b));
    assert_eq!(bits(colors[2]), bits([0.0; 4]));

    let vtric_n = unsafe { light_manager::vtric_count() };
    let vtrics: Vec<u32> = (0..vtric_n).map(|i| unsafe { vtric_index_at(i) }).collect();
    assert_eq!(vtrics, [1, 2]);

    let (events, stats) = with_tail(|r| {
        (
            std::mem::take(&mut r.events),
            (r.stat_vtric, r.stat_mesh_light),
        )
    });
    assert_eq!(stats, (1, 3));
    assert_eq!(
        events,
        [
            TailEvent::Compute(0, TAIL_FRAME),
            TailEvent::Compute(1, TAIL_FRAME),
            TailEvent::Compute(2, TAIL_FRAME),
        ]
    );
}

#[test]
fn setup_tail_preserves_result_when_full_and_skips_empty_vtric_sort() {
    let _lock = LIGHT_MANAGER_STATE_LOCK.lock().unwrap();
    let mut actor = make_actor(255, false);
    let existing = std::ptr::addr_of_mut!(*actor) as usize;
    with_tail(|r| *r = TailReplay::default());
    unsafe { seed_tail_light_manager(&[(existing, [0.5, 1.0, 2.0, 4.0])], 1) };

    // Room is already full, so this target is never dereferenced.
    let link_nodes = build_actor_link_chain(&[0x600]);
    let mut io = SetupTailIo {
        frame: TAIL_FRAME as *mut c_void,
        volumetrics: link_nodes.as_ptr() as *mut c_void,
        result: 0x20,
        pad: 0,
    };
    setup_tail(&tail_services(), &mut io);
    assert_eq!(io.result, 0x20);
    assert_eq!(unsafe { light_manager::vtric_count() }, 0);
    assert_eq!(with_tail(|r| r.stat_vtric), 0);
    assert_eq!(with_tail(|r| r.stat_mesh_light), 1);
    assert_eq!(
        with_tail(|r| std::mem::take(&mut r.events)),
        [TailEvent::Compute(0, TAIL_FRAME)]
    );
}
