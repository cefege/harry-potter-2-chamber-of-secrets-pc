use Hp2ActorAudioInteger::*;
use Hp2ActorAudioObject::*;
use hp2_engine::actor_audio::*;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::null_mut;

type V = Hp2ActorAudioVector;
fn ptr(id: usize) -> *mut c_void {
    id as *mut c_void
}
const ACTOR: usize = 1;
const HEARER: usize = 2;
const OTHER: usize = 3;
const TARGET: usize = 4;
const LEVEL: usize = 10;
const CLIENT: usize = 11;
const DRIVER: usize = 12;
const AUDIO: usize = 13;
const PLAYER: usize = 14;
const CALLER: usize = 15;
const SOUND: usize = 16;

#[derive(Debug, PartialEq)]
struct Heard {
    hearer: usize,
    actor: usize,
    id: i32,
    sound: usize,
    location: V,
    parameters: V,
    disable: i32,
    looping: i32,
}
struct World {
    objects: HashMap<(usize, u32), usize>,
    integers: HashMap<(usize, u32), i32>,
    locations: HashMap<usize, V>,
    viewports: Vec<usize>,
    visible: i32,
    calls: Vec<String>,
    heard: Vec<Heard>,
    demos: Vec<Hp2ActorAudioSound>,
    commands: Vec<(u32, usize, i32, usize, u8, f32)>,
    fail_at: Option<usize>,
    demo_mode: Option<i32>,
    demo_index: Option<i32>,
}
impl World {
    fn new() -> Self {
        let mut w = Self {
            objects: HashMap::new(),
            integers: HashMap::new(),
            locations: HashMap::new(),
            viewports: vec![HEARER],
            visible: 1,
            calls: vec![],
            heard: vec![],
            demos: vec![],
            commands: vec![],
            fail_at: None,
            demo_mode: None,
            demo_index: None,
        };
        for a in [ACTOR, HEARER, OTHER, TARGET] {
            w.object(a, AudioLevel, LEVEL);
            w.locations.insert(a, V::default());
        }
        w.object(LEVEL, AudioClient, CLIENT);
        w.object(LEVEL, AudioSubsystem, AUDIO);
        w.object(ACTOR, AudioPawnList, HEARER);
        w.object(HEARER, AudioNextPawn, OTHER);
        w.integer(ACTOR, AudioActorIndex, 7);
        for a in [HEARER, OTHER] {
            w.integer(a, AudioIsPlayer, 1);
            w.integer(a, AudioIsPlayerPawn, 1);
        }
        w
    }
    fn object(&mut self, a: usize, f: Hp2ActorAudioObject, v: usize) {
        self.objects.insert((a, f as u32), v);
    }
    fn integer(&mut self, a: usize, f: Hp2ActorAudioInteger, v: i32) {
        self.integers.insert((a, f as u32), v);
    }
    fn call(&mut self, name: String) -> i32 {
        self.calls.push(name);
        if self.fail_at == Some(self.calls.len() - 1) {
            73
        } else {
            0
        }
    }
    fn services(&mut self) -> Hp2ActorAudioServices {
        Hp2ActorAudioServices {
            context: (self as *mut Self).cast(),
            object,
            integer,
            location,
            viewport_actor,
            line_check,
            hear,
            demo,
            play_music,
            stop_music,
            stop_all_music,
            modify_sound,
            stop_sound,
        }
    }
    fn play(&mut self, kind: u32, args: &Hp2ActorAudioSound) -> Result<(), i32> {
        unsafe { play_sound(ptr(ACTOR), ptr(CALLER), kind, args, &self.services()) }
    }
    fn check(&mut self, radius: f32) -> Result<(), i32> {
        unsafe {
            check_hear_sound(
                ptr(ACTOR),
                ptr(HEARER),
                31,
                ptr(SOUND),
                V {
                    x: 80.0,
                    y: 22.0,
                    z: 120.0,
                },
                radius,
                1,
                1,
                &self.services(),
            )
        }
    }
    fn command(&mut self, kind: u32, out: &mut i32) -> Result<(), i32> {
        unsafe {
            audio_command(
                ptr(ACTOR),
                kind,
                ptr(SOUND),
                3,
                9,
                42,
                0.75,
                out,
                &self.services(),
            )
        }
    }
}
unsafe fn world<'a>(c: *mut c_void) -> &'a mut World {
    unsafe { &mut *c.cast::<World>() }
}
unsafe extern "C" fn object(
    c: *mut c_void,
    a: *mut c_void,
    f: Hp2ActorAudioObject,
    out: *mut *mut c_void,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call(format!("object:{a:?}:{f:?}"));
    if status == 0 {
        unsafe {
            *out = ptr(*w.objects.get(&(a as usize, f as u32)).unwrap_or(&0));
        }
    }
    status
}
unsafe extern "C" fn integer(
    c: *mut c_void,
    a: *mut c_void,
    f: Hp2ActorAudioInteger,
    out: *mut i32,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call(format!("integer:{a:?}:{f:?}"));
    if status == 0 {
        unsafe {
            *out = if f == AudioViewportCount {
                w.viewports.len() as i32
            } else {
                *w.integers.get(&(a as usize, f as u32)).unwrap_or(&0)
            };
        }
    }
    status
}
unsafe extern "C" fn location(c: *mut c_void, a: *mut c_void, out: *mut V) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call(format!("location:{a:?}"));
    if status == 0 {
        unsafe {
            *out = w.locations[&(a as usize)];
        }
    }
    status
}
unsafe extern "C" fn viewport_actor(
    c: *mut c_void,
    _: *mut c_void,
    i: i32,
    out: *mut *mut c_void,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call(format!("viewport:{i}"));
    if status == 0 {
        unsafe {
            *out = ptr(w.viewports[i as usize]);
        }
    }
    status
}
unsafe extern "C" fn line_check(
    c: *mut c_void,
    _: *mut c_void,
    _: *const V,
    _: *const V,
    out: *mut i32,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("line".into());
    if status == 0 {
        unsafe {
            *out = w.visible;
        }
    }
    status
}
unsafe extern "C" fn hear(
    c: *mut c_void,
    h: *mut c_void,
    a: *mut c_void,
    id: i32,
    s: *mut c_void,
    l: *const V,
    p: *const V,
    d: i32,
    looping: i32,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("hear".into());
    if status == 0 {
        w.heard.push(Heard {
            hearer: h as usize,
            actor: a as usize,
            id,
            sound: s as usize,
            location: unsafe { *l },
            parameters: unsafe { *p },
            disable: d,
            looping,
        });
    }
    status
}
unsafe extern "C" fn demo(c: *mut c_void, _: *mut c_void, a: *const Hp2ActorAudioSound) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("demo".into());
    if status == 0 {
        w.demos.push(unsafe { *a });
        if let Some(mode) = w.demo_mode {
            w.integer(LEVEL, AudioNetMode, mode);
        }
        if let Some(index) = w.demo_index {
            w.integer(ACTOR, AudioActorIndex, index);
        }
    }
    status
}
unsafe extern "C" fn play_music(
    c: *mut c_void,
    a: *mut c_void,
    song: *mut c_void,
    fade: f32,
    out: *mut i32,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("play_music".into());
    if status == 0 {
        w.commands.push((0, a as usize, 0, song as usize, 0, fade));
        unsafe {
            *out = -17;
        }
    }
    status
}
unsafe extern "C" fn stop_music(c: *mut c_void, a: *mut c_void, handle: i32, fade: f32) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("stop_music".into());
    if status == 0 {
        w.commands.push((1, a as usize, handle, 0, 0, fade));
    }
    status
}
unsafe extern "C" fn stop_all_music(c: *mut c_void, a: *mut c_void, fade: f32) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("stop_all_music".into());
    if status == 0 {
        w.commands.push((2, a as usize, 0, 0, 0, fade));
    }
    status
}
unsafe extern "C" fn modify_sound(
    c: *mut c_void,
    a: *mut c_void,
    _: *mut c_void,
    id: i32,
    sound: *mut c_void,
    param: u8,
    value: f32,
    out: *mut i32,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("modify".into());
    if status == 0 {
        w.commands
            .push((3, a as usize, id, sound as usize, param, value));
        unsafe {
            *out = 7;
        }
    }
    status
}
unsafe extern "C" fn stop_sound(
    c: *mut c_void,
    a: *mut c_void,
    _: *mut c_void,
    id: i32,
    sound: *mut c_void,
    value: f32,
) -> i32 {
    let w = unsafe { world(c) };
    let status = w.call("stop".into());
    if status == 0 {
        w.commands
            .push((4, a as usize, id, sound as usize, 0, value));
    }
    status
}
fn args() -> Hp2ActorAudioSound {
    Hp2ActorAudioSound {
        sound: ptr(SOUND),
        slot: 3,
        volume: 0.8,
        no_override: 1,
        radius: 0.0,
        pitch: 1.2,
        disable_3d: 1,
        looping: 1,
    }
}
fn recipients(w: &World) -> Vec<usize> {
    w.heard.iter().map(|h| h.hearer).collect()
}

#[test]
fn strict_initial_cutoff_and_inclusive_occluded_cutoff() {
    // The initial comparison rejects equality before any line check.
    for (x, expected) in [(9.0, true), (10.0, false), (11.0, false)] {
        let mut w = World::new();
        w.locations.insert(HEARER, V { x, y: 0.0, z: 0.0 });
        w.check(130.0).unwrap();
        assert_eq!(!w.heard.is_empty(), expected);
        assert_eq!(w.calls.iter().any(|v| v == "line"), expected);
    }
    // These f32 operations give an exact reduced squared radius of six.
    assert_eq!((13.0_f32 / 1.3_f32) * 0.6_f32, 6.0);
    for (position, expected) in [
        (
            V {
                x: 2.0,
                y: 1.0,
                z: 1.0,
            },
            true,
        ),
        (
            V {
                x: 2.0,
                y: 1.0,
                z: 2.0,
            },
            false,
        ),
    ] {
        let mut w = World::new();
        w.visible = 0;
        w.locations.insert(HEARER, position);
        w.check(13.0).unwrap();
        assert_eq!(!w.heard.is_empty(), expected);
        if expected {
            assert_eq!(
                w.heard[0].parameters,
                V {
                    x: 28.0,
                    y: 22.0,
                    z: 120.0
                }
            );
        }
    }
    for radius in [0.0, -1.0, f32::NAN] {
        let mut w = World::new();
        w.check(radius).unwrap();
        assert!(w.heard.is_empty());
    }
}

#[test]
fn instigator_keeps_occluded_radius_but_not_volume() {
    for instigator in [0, HEARER] {
        let mut w = World::new();
        w.visible = 0;
        w.object(ACTOR, AudioInstigator, instigator);
        w.locations.insert(
            HEARER,
            V {
                x: 9.0,
                y: 0.0,
                z: 0.0,
            },
        );
        w.check(130.0).unwrap();
        assert_eq!(w.heard.len(), usize::from(instigator == HEARER));
        if instigator == HEARER {
            assert_eq!(w.heard[0].parameters.x, 28.0);
        }
    }
}

#[test]
fn listener_uses_view_target_only_for_player_pawn_with_target() {
    for player_pawn in [0, 1] {
        for target in [0, TARGET] {
            let mut w = World::new();
            w.integer(HEARER, AudioIsPlayerPawn, player_pawn);
            w.object(HEARER, AudioViewTarget, target);
            w.locations.insert(
                HEARER,
                V {
                    x: 100.0,
                    y: 0.0,
                    z: 0.0,
                },
            );
            w.check(130.0).unwrap();
            assert_eq!(w.heard.len(), usize::from(player_pawn != 0 && target != 0));
            if !w.heard.is_empty() {
                assert_eq!(w.heard[0].hearer, HEARER);
            }
        }
    }
}

#[test]
fn routes_and_demo_conditions_preserve_all_modes() {
    for kind in 0..3 {
        for mode in 0..4 {
            for simulated in [0, 1] {
                for recording in [false, true] {
                    for server_connection in [false, true] {
                        let mut w = World::new();
                        w.integer(LEVEL, AudioNetMode, mode);
                        w.integer(CALLER, AudioCallerSimulated, simulated);
                        if recording {
                            w.object(LEVEL, AudioDemoDriver, DRIVER);
                        }
                        if server_connection {
                            w.object(DRIVER, AudioServerConnection, 99);
                        }
                        w.play(kind, &args()).unwrap();
                        let local = kind == 0 || mode == 3 || (kind == 1 && simulated != 0);
                        assert_eq!(
                            recipients(&w),
                            if local {
                                vec![HEARER]
                            } else {
                                vec![HEARER, OTHER]
                            }
                        );
                        let demo = kind != 0
                            && recording
                            && !server_connection
                            && (kind == 2 || mode != 3);
                        assert_eq!(w.demos.len(), usize::from(demo));
                        if demo {
                            assert!(
                                w.calls.iter().position(|s| s == "demo").unwrap()
                                    < w.calls.iter().position(|s| s == "hear").unwrap()
                            );
                            let d = w.demos[0];
                            let a = args();
                            assert_eq!(
                                (
                                    d.sound,
                                    d.slot,
                                    d.volume,
                                    d.no_override,
                                    d.radius,
                                    d.pitch,
                                    d.disable_3d,
                                    d.looping
                                ),
                                (
                                    a.sound,
                                    a.slot,
                                    a.volume,
                                    a.no_override,
                                    a.radius,
                                    a.pitch,
                                    a.disable_3d,
                                    a.looping
                                )
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn demo_event_mutations_precede_id_and_route_selection() {
    let mut w = World::new();
    w.object(LEVEL, AudioDemoDriver, DRIVER);
    w.demo_mode = Some(3);
    w.demo_index = Some(20);
    w.play(1, &args()).unwrap();
    assert_eq!(recipients(&w), vec![HEARER]);
    assert_eq!(w.heard[0].id, 327);
}

#[test]
fn local_null_sound_missing_client_and_other_level_do_not_emit() {
    for kind in 0..3 {
        let mut w = World::new();
        let mut a = args();
        a.sound = null_mut();
        w.play(kind, &a).unwrap();
        assert!(w.calls.is_empty());
        let mut w = World::new();
        w.integer(LEVEL, AudioNetMode, 3);
        w.object(LEVEL, AudioClient, 0);
        w.play(kind, &args()).unwrap();
        assert!(w.heard.is_empty());
        let mut w = World::new();
        w.integer(LEVEL, AudioNetMode, 3);
        w.viewports = vec![0, HEARER, OTHER];
        w.object(HEARER, AudioLevel, 99);
        w.play(kind, &args()).unwrap();
        assert_eq!(recipients(&w), vec![OTHER]);
    }
    let mut w = World::new();
    w.integer(HEARER, AudioIsPlayer, 0);
    w.play(1, &args()).unwrap();
    assert_eq!(recipients(&w), vec![OTHER]);
}

#[test]
fn only_owned_sound_excludes_remote_owner_and_not_standalone_or_viewports() {
    for kind in [1, 2] {
        for mode in [0, 1, 2] {
            for viewport in [0, 1] {
                for player_present in [false, true] {
                    let mut w = World::new();
                    w.integer(LEVEL, AudioNetMode, mode);
                    w.object(ACTOR, AudioOwner, HEARER);
                    if player_present {
                        w.object(HEARER, AudioPlayer, PLAYER);
                    }
                    w.integer(PLAYER, AudioIsViewport, viewport);
                    w.play(kind, &args()).unwrap();
                    let excluded = kind == 2 && mode != 0 && viewport == 0 && player_present;
                    assert_eq!(
                        recipients(&w),
                        if excluded {
                            vec![OTHER]
                        } else {
                            vec![HEARER, OTHER]
                        }
                    );
                }
            }
        }
    }
    // A player-pawn source never falls through to its Owner, even without a Player.
    let mut w = World::new();
    w.integer(LEVEL, AudioNetMode, 2);
    w.integer(ACTOR, AudioIsPlayerPawn, 1);
    w.object(ACTOR, AudioOwner, HEARER);
    w.object(HEARER, AudioPlayer, PLAYER);
    w.play(2, &args()).unwrap();
    assert_eq!(recipients(&w), vec![HEARER, OTHER]);
    let mut w = World::new();
    w.integer(LEVEL, AudioNetMode, 2);
    w.integer(ACTOR, AudioIsPlayerPawn, 1);
    w.integer(ACTOR, AudioIsPlayer, 1);
    w.object(ACTOR, AudioPlayer, PLAYER);
    w.object(ACTOR, AudioPawnList, ACTOR);
    w.object(ACTOR, AudioNextPawn, HEARER);
    w.play(2, &args()).unwrap();
    assert_eq!(recipients(&w), vec![HEARER, OTHER]);
}

#[test]
fn ids_parameter_scaling_and_zero_radius_default() {
    assert_eq!(sound_id(7, 3, 1), 119);
    assert_eq!(sound_id(7, 3, 0), 118);
    for radius in [0.0, -0.0, 20.0, -20.0] {
        let mut w = World::new();
        let mut a = args();
        a.radius = radius;
        w.locations.insert(
            HEARER,
            V {
                x: 100.0,
                y: 0.0,
                z: 0.0,
            },
        );
        w.play(0, &a).unwrap();
        assert_eq!(w.heard.len(), usize::from(radius == 0.0));
        if radius == 0.0 {
            assert_eq!(
                w.heard[0],
                Heard {
                    hearer: HEARER,
                    actor: ACTOR,
                    id: 119,
                    sound: SOUND,
                    location: V::default(),
                    parameters: V {
                        x: 80.0,
                        y: radius,
                        z: 100.0 * 1.2
                    },
                    disable: 1,
                    looping: 1
                }
            );
        }
    }
}

#[test]
fn audio_absence_defaults_and_present_results_are_exact() {
    for present in [false, true] {
        for kind in 0..5 {
            let mut w = World::new();
            if !present {
                w.object(LEVEL, AudioSubsystem, 0);
            }
            let mut out = 991;
            w.command(kind, &mut out).unwrap();
            assert_eq!(
                out,
                match kind {
                    0 =>
                        if present {
                            -17
                        } else {
                            0
                        },
                    3 =>
                        if present {
                            7
                        } else {
                            0
                        },
                    _ => 991,
                }
            );
            if present {
                let expected = match kind {
                    0 => (0, AUDIO, 0, SOUND, 0, 0.75),
                    1 => (1, AUDIO, 42, 0, 0, 0.75),
                    2 => (2, AUDIO, 0, 0, 0, 0.75),
                    3 => (3, AUDIO, 118, SOUND, 9, 0.75),
                    _ => (4, AUDIO, 118, SOUND, 0, 0.75),
                };
                assert_eq!(w.commands, vec![expected]);
            } else {
                assert!(w.commands.is_empty());
            }
            assert_eq!(
                w.calls.iter().any(|s| s.ends_with("AudioActorIndex")),
                kind == 4 || (kind == 3 && present)
            );
        }
    }
}

#[test]
fn first_leaf_failure_stops_without_later_callbacks_or_result_writes() {
    for kind in 0..3 {
        let setup = || {
            let mut w = World::new();
            w.object(LEVEL, AudioDemoDriver, DRIVER);
            w.object(ACTOR, AudioOwner, HEARER);
            w.object(HEARER, AudioPlayer, PLAYER);
            w.integer(LEVEL, AudioNetMode, 2);
            w.visible = 0;
            w
        };
        let mut baseline = setup();
        baseline.play(kind, &args()).unwrap();
        for index in 0..baseline.calls.len() {
            let mut w = setup();
            w.fail_at = Some(index);
            assert_eq!(w.play(kind, &args()), Err(73));
            assert_eq!(w.calls, baseline.calls[..=index]);
        }
    }
    for kind in 0..5 {
        let mut baseline = World::new();
        baseline.command(kind, &mut 0).unwrap();
        for index in 0..baseline.calls.len() {
            let mut w = World::new();
            w.fail_at = Some(index);
            let mut out = 991;
            assert_eq!(w.command(kind, &mut out), Err(73));
            assert_eq!(w.calls, baseline.calls[..=index]);
            assert_eq!(out, 991);
        }
    }
}
