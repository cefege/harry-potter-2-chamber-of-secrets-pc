//! M2/M3/M4 — tick orchestration, input dispatch, and the console path,
//! owned by one plain main-loop struct. No scheduler trait mediates ticks.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use hp_format::mesh::decode_export;
use hp_format::package79::{ByteCursor, PackageArchive, read_compact_index, read_property_tags};
use hp_format::psa::read_psa;
use hp_ini::{IniSet, load_pair};
use hp_uobject::arena::{BytecodeResolver, ObjectArena, ObjectData, ObjectId, UObject};
use hp_uobject::bootstrap::World;
use hp_uobject::name::{NO_NUMBER, Name};
use hp_uobject::props::PropStore;
use hp_uobject::value::PropValue;
use hp_uobject::vm::{
    Frame, IteratorCursor, LatentRequest, PendingExpression, ScriptEffect, StateChange, Suspend,
    SuspendedCall, state_entry_pc,
};

use crate::console::{Bindings, binding_key_name, expand_segment};
use crate::error::{EngineError, Result};
use crate::input::{InputScript, action};
use crate::level::{Level, load_level};
use crate::pawn::{CollisionPoly, CollisionWorld, PawnInput, PlayerPawn};
use crate::rng::{DEFAULT_SEED, SimRng};
use crate::scene::{ResolvedProp, SceneCamera, effective_prop};
use hp_audio::ogg::OggSource;
use hp_audio::source::StreamKind;
use hp_audio::xa::XaSource;

/// Explicit `FinishAnim` fallback when neither the mesh nor its source PSA
/// exposes timing metadata. This is never used when authored duration exists.
pub const FINISH_ANIM_PLACEHOLDER_SECS: f64 = 1.0;

/// Bootstrap options for one process.
#[derive(Debug, Clone)]
pub struct EngineOptions {
    /// Process RNG seed (`--rng-seed`; default [`DEFAULT_SEED`]).
    pub rng_seed: u64,
    /// Deterministic tick override (`--fixed-dt=<secs>`).
    pub fixed_dt: Option<f32>,
    /// Editor-mode cap flag (`GetMaxTickRate` returns 30 instead of 0).
    pub editor_mode: bool,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            rng_seed: DEFAULT_SEED,
            fixed_dt: None,
            editor_mode: false,
        }
    }
}

/// Per-run observable counters (deterministic; part of the run summary).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RunCounters {
    pub ticks: u64,
    pub input_events: u64,
    pub input_handled: u64,
    pub console_commands: u64,
    /// Actor events skipped under `engine.script_event_deferred`.
    pub script_deferrals: u64,
    /// `GotoState` switches that resolved to a state with code.
    pub states_entered: u64,
    /// `Sleep` suspensions parked by the frame scheduler.
    pub latent_sleeps: u64,
    /// `FinishAnim` suspensions (nominal placeholder wake).
    pub latent_finish_anims: u64,
    /// `SetTimer` schedules installed into the timer wheel.
    pub set_timers: u64,
    /// `Timer` events fired this run.
    pub timers_fired: u64,
}

/// Wake model for one parked actor frame. Times are absolute simulated
/// seconds ([`Engine::tick`] advances the clock by `dt` once per tick).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wake {
    /// Due on the scheduler's very next pass (e.g. freshly entered state
    /// code that has not had a first slice yet).
    Ready,
    /// Due when simulated time reaches this instant.
    At(f64),
    /// Re-polled each scheduler pass until the actor's `bInterpolating`
    /// property is false.
    Interpolation,
    /// Blocked forever on an external condition the headless sim cannot
    /// observe yet (reserved for animation/PSA-completion wakes).
    Never,
}

/// One actor's parked script continuation: exactly where a suspended
/// state-code or event frame stopped and what it needs to resume from
/// that point — never a restart.
#[derive(Debug, Clone)]
pub struct ScriptFrame {
    /// The code stream this continuation runs in (state body, or the
    /// event function's body for parked event continuations).
    pub code: Vec<u8>,
    /// Package-local compact object resolver for this exact code stream.
    pub resolver: BytecodeResolver,
    /// Resume cursor: already PAST the suspending latent call.
    pub pc: usize,
    /// The object bound as `self` when the frame resumes. It is explicit
    /// rather than implied by the map key so the persisted-frame ABI is
    /// inspectable and a corrupt continuation cannot run as another actor.
    pub self_id: ObjectId,
    /// The state whose code this is (`None` for parked event frames).
    pub state: Option<ObjectId>,
    /// Locals snapshot of the suspended activation.
    pub locals: PropStore,
    /// A suspended script callee that must resume before this frame can
    /// execute past the outer call site.
    pub nested_call: Option<SuspendedCall>,
    /// Active foreach cursor stack. It survives a latent body suspension so
    /// resuming continues at the next iterator body rather than restarting
    /// the native enumeration.
    pub iterators: Vec<IteratorCursor>,
    pub wake: Wake,
    /// Event name owning a parked event continuation (`None` = state
    /// code). UE1 semantics: an active state frame replaces the actor's
    /// `Tick` event; likewise a parked event continuation suppresses
    /// re-dispatch of its own event until it resumes.
    pub origin: Option<String>,
}

/// One entry of the per-actor timer wheel (`AActor::SetTimer`). Kept in
/// a plain Vec and sorted by `(at, actor)` at fire time so due-timer
/// order stays deterministic regardless of insertion order.
#[derive(Debug, Clone, Copy)]
struct TimerEntry {
    actor: ObjectId,
    at: f64,
    interval: f32,
    repeating: bool,
}

#[derive(Debug, Clone)]
struct SplineMotion {
    actor: ObjectId,
    points: Vec<ObjectId>,
    segment_lengths: Vec<f32>,
    total_length: f32,
    elapsed: f32,
    duration: f32,
    accel: f32,
    ease: SplineEase,
    align: bool,
    cue: String,
    loops: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplineEase {
    Linear,
    From,
    To,
    Between,
}

/// Last script-requested animation state for one actor. Both timestamps are
/// simulation-owned: render cadence never participates in playback.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorAnimation {
    pub sequence: hp_uobject::name::Name,
    pub started_at: f64,
    pub elapsed_seconds: f32,
    pub duration_seconds: Option<f32>,
    /// First authored last-frame crossing, already adjusted by play rate:
    /// `(frame_count - 1) / (source_fps * abs(rate))`.
    pub finish_after_seconds: Option<f32>,
    pub mesh_path: Option<String>,
    pub animation_path: Option<String>,
    pub rate: f32,
    pub looped: bool,
    pub tweening: bool,
}

/// Deterministic render-facing animation state. [`Engine::actor_animation_snapshots`]
/// keys these by stable arena identity in a [`BTreeMap`].
#[derive(Debug, Clone, PartialEq)]
pub struct ActorAnimationSnapshot {
    pub actor: ObjectId,
    pub mesh_path: Option<String>,
    pub animation_path: Option<String>,
    pub sequence: String,
    pub started_at: f64,
    pub elapsed_seconds: f32,
    pub duration_seconds: Option<f32>,
    pub finish_after_seconds: Option<f32>,
    pub rate: f32,
    pub looped: bool,
    pub tweening: bool,
}

/// Save-game animation state keyed by an actor's stable map path.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistentAnimationState {
    pub actor_path: String,
    pub sequence: String,
    pub actor_identity: String,
    pub elapsed_seconds: f32,
    pub rate: f32,
    pub looped: bool,
    pub tweening: bool,
}

/// Save-safe representation of a tagged actor property. Text names and
/// object paths replace arena-local indices so a fresh bootstrap can restore
/// the same value graph.
#[derive(Debug, Clone, PartialEq)]
pub enum PersistentPropertyValue {
    Byte(u8),
    Int(i32),
    Bool(bool),
    Float(f32),
    Object {
        path: Option<String>,
        raw: Option<i32>,
    },
    Name { text: String, number: i32 },
    Str(String),
    Struct {
        struct_name: String,
        fields: Vec<(String, PersistentPropertyValue)>,
    },
    Array(Vec<PersistentPropertyValue>),
    FixedArray(Vec<PersistentPropertyValue>),
    Map(Vec<(PersistentPropertyValue, PersistentPropertyValue)>),
}

/// Save-game state for one authored actor marked `bPersistent`.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistentActorState {
    pub actor_path: String,
    pub actor_identity: String,
    pub class_path: String,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub hidden: bool,
    pub properties: Vec<(String, PersistentPropertyValue)>,
}

/// Cross-map state serialized into the tagged `SaveN.usa` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct CampaignState {
    pub active_map: String,
    pub current_game_state: Option<String>,
    pub persistent_actors: Vec<PersistentActorState>,
    pub animations: Vec<PersistentAnimationState>,
}
/// Deterministic authoritative pose for one active actor. The renderer uses
/// this instead of a second transform clock, so scripted movement and hiding
/// become visible on the same tick that mutates the simulation.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorTransformSnapshot {
    pub actor: ObjectId,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub hidden: bool,
}

/// Live state of an authored `HPawn.DoFlyTo` operation.  BaseCam and its
/// target both inherit these fields; the controller remains UnrealScript
/// authority while the simulation only snapshots it.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamFlyToSnapshot {
    pub actor: ObjectId,
    pub elapsed_seconds: Option<f32>,
    pub duration_seconds: Option<f32>,
    pub start: Option<[f32; 3]>,
    pub controller: Option<ObjectId>,
    pub controller_enabled: Option<bool>,
    pub destination: Option<[f32; 3]>,
    pub destination_actor: Option<ObjectId>,
    pub destination_offset: Option<[f32; 3]>,
    pub fixed_to_destination_actor: Option<bool>,
    pub stay_locked_to_destination_actor: Option<bool>,
}

/// Simulation-owned continuation for one authored FollowSpline command.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamSplineSnapshot {
    pub actor: ObjectId,
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
    pub cue: String,
    pub aligns_to_spline: bool,
}

/// An effect interval backed by a live UnrealScript controller actor.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamControllerInterval {
    pub controller: ObjectId,
    pub elapsed_seconds: Option<f32>,
    pub duration_seconds: Option<f32>,
    /// FOV uses `[end, start, 0, 0]`; fades use the controller's authored
    /// `(A,R,G,B)` destination plane.
    pub values: Option<[f32; 4]>,
}

/// Whether an authored effect has observable controller state.  `Available`
/// may be empty when no interval is active.  Native effects whose controller
/// state is not exposed must stay explicit rather than being fabricated.
#[derive(Debug, Clone, PartialEq)]
pub enum BaseCamEffectIntervals {
    Available(Vec<BaseCamControllerInterval>),
    Unavailable { reason_code: &'static str },
}

/// Simulation-owned `PlayerPawn.ShakeView` interval applied by Harry's
/// `BaseCam.rExtraRotation` path. Times use the scheduler clock; rotation is
/// the current decaying UE rotator delta consumed by camera selection.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamShakeInterval {
    pub magnitude: f32,
    pub max_shake: f32,
    pub duration_seconds: f32,
    pub started_at_seconds: f64,
    pub ends_at_seconds: f64,
    pub elapsed_seconds: f32,
    pub progress: f32,
    pub rotation_delta: [i32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamTargetSnapshot {
    pub actor: ObjectId,
    pub attached_to: Option<ObjectId>,
    pub offset: Option<[f32; 3]>,
    pub relative: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamSettingsSnapshot {
    pub look_at_offset: Option<[f32; 3]>,
    pub look_at_distance: Option<f32>,
    pub rotation_tightness: Option<f32>,
    pub unavailable_reason_code: Option<&'static str>,
    pub rotation_speed: Option<f32>,
    pub movement_tightness: Option<f32>,
    pub movement_speed: Option<f32>,
}

/// Source-facing BaseCam state at the simulation/render boundary.  It is
/// read from the live UnrealScript object rather than inferred from camera
/// selection, so CutScript command transitions remain observable.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseCamSnapshot {
    pub actor: ObjectId,
    pub camera_mode: Option<i32>,
    pub transition_mode: Option<i32>,
    pub target: Option<ObjectId>,
    pub cut_notify_actor: Option<ObjectId>,
    pub sync_position_with_target: Option<bool>,
    pub sync_rotation_with_target: Option<bool>,
    pub ignore_target: Option<bool>,
    pub current_position: Option<[f32; 3]>,
    pub destination_position: Option<[f32; 3]>,
    pub current_rotation: Option<[i32; 3]>,
    pub destination_rotation: Option<[i32; 3]>,
    /// `Locked` and `UnLock` directly control
    /// `bSyncPositionWithTarget`; retain the named view for telemetry.
    pub locked_to_target: Option<bool>,
    pub target_state: Option<BaseCamTargetSnapshot>,
    pub settings: Option<BaseCamSettingsSnapshot>,
    pub camera_fly_to: BaseCamFlyToSnapshot,
    pub target_fly_to: Option<BaseCamFlyToSnapshot>,
    pub follow_splines: Vec<BaseCamSplineSnapshot>,
    pub fov_intervals: BaseCamEffectIntervals,
    pub shake_intervals: Vec<BaseCamShakeInterval>,
    pub flash_intervals: BaseCamEffectIntervals,
    pub fade_intervals: BaseCamEffectIntervals,
}


/// Deferral blacklist key: which bytecode stream failed loudly. Events
/// key by class + event name; state bodies by class + state object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ScriptKey {
    Event(ObjectId, String),
    State(ObjectId, ObjectId),
}

/// Runtime-only join of a canonical mesh asset to the metadata used by
/// `AActor::HasAnim` and `USkeletalMesh::BoneIndex`. Bone order is the
/// serialized RefSkeleton order; animation names combine the mesh's own
/// sequences with its optional companion PSA.
#[derive(Debug, Clone, Default)]
pub struct SkeletalAssetMetadata {
    pub skeletal: bool,
    pub bone_names: Vec<String>,
    pub animation_names: HashSet<String>,
    /// Case-folded sequence name -> authored PSA/mesh group name.
    pub animation_groups: HashMap<String, String>,
    pub bone_children: Vec<usize>,
}

/// PSA metadata selected by an actor's explicit `SkelAnim` property.
#[derive(Debug, Clone, Default)]
struct AnimationAssetMetadata {
    sequence_names: HashSet<String>,
    sequence_groups: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct BaseCamShakeState {
    roll_magnitude: f32,
    max_shake: f32,
    duration_seconds: f32,
    started_at_seconds: f64,
    ends_at_seconds: f64,
    elapsed_seconds: f32,
    progress: f32,
    rotation_delta: [i32; 3],
}

/// The headless simulation: world + level + loop policy.
const SCRIPT_LOG_BUDGET: usize = 256;

pub struct Engine {
    /// The data root every map load resolves against.
    pub data_root: PathBuf,
    pub world: World,
    pub level: Level,
    pub rng: SimRng,
    pub fixed_dt: Option<f32>,
    pub editor_mode: bool,
    /// `[Engine.GameEngine] FrameRateLimit` (<= 0 means uncapped).
    pub frame_rate_limit: f32,
    bindings: Bindings,
    ini: IniSet,
    /// Camera/scripted cues executed through the console path, in order.
    pub cues: Vec<String>,
    /// Ordered UnrealScript `log(...)` messages retained for fidelity
    /// diagnostics; process stderr remains a secondary presentation sink.
    pub script_logs: Vec<String>,
    counters: RunCounters,
    /// Sound cues routed through [`Engine::play_sound_cue`], in order
    /// (interactive mode only; headless paths never enable sounds so the
    /// deterministic output stays byte-stable).
    pub sound_cues: Vec<String>,
    /// Whether interactive audio is wired up (set by the windowed app at
    /// boot unless `-nosound`). Default `false`.
    pub sounds_enabled: bool,
    /// Gameplay audio host once [`Engine::attach_audio`] receives one
    /// (interactive only; headless never attaches so deterministic output
    /// stays byte-stable).
    pub audio: Option<crate::music::AudioHost>,
    /// Player pawn spawned at the loaded map's PlayerStart (Phase D);
    /// `None` when the map carries no PlayerStart or no map is loaded.
    /// Stepped by the shell via [`Engine::pawn_step`] BEFORE
    /// [`Engine::tick`], never inside it.
    pub pawn: Option<PlayerPawn>,
    /// Static collision world (brush + BSP level-model planes) built
    /// alongside [`Engine::pawn`].
    pub collision: Option<CollisionWorld>,
    /// First-person toggle for [`Engine::pawn_camera`] (shell F5).
    pub pawn_first_person: bool,
    /// Set once the pawn has entered a level-change trigger this run;
    /// consumed via [`Engine::take_level_exit`] by the binary layer's
    /// transition path.
    pub pending_level_exit: bool,
    /// Destination map name (`NewMapName`, else `URL`) captured by the
    /// trigger that set [`Engine::pending_level_exit`]; `None` when the
    /// trigger carried neither property. Consumed via
    /// [`Engine::take_level_exit`] by the binary layer's transition path.
    pub level_exit_destination: Option<String>,
    /// Level-exit triggers already fired this map load; each actor fires
    /// at most once per load.
    fired_triggers: HashSet<ObjectId>,
    /// Ambient-sound classes already reported as `[engine.sound_deferred]`
    /// this run — their payloads cannot change, so re-running them would
    /// just repeat.
    deferred_sounds: HashSet<String>,
    quit_requested: bool,
    /// Memoized `(class, event)` -> function id lookups.
    script_lookup: HashMap<(ObjectId, String), Option<ObjectId>>,
    /// `(class, event)` pairs already reported as deferred this run; their
    /// bytecode cannot change, so re-running them would just repeat.
    deferred_scripts: HashSet<(ObjectId, String)>,
    /// Distinct deferral sites already printed (print once, count always).
    deferral_reported: HashSet<(ObjectId, String)>,
    deferral_reason_codes: HashSet<&'static str>,
    /// Exact deferred-native diagnostics grouped by registered slot/subject.
    native_deferral_census: BTreeMap<String, u64>,
    /// Simulated seconds since boot; advanced by `dt` once per tick.
    /// Every wake time and timer deadline is an absolute point on this
    /// clock, which keeps wake ordering a pure `(time, actor)` sort.
    sim_time: f64,
    /// Monotonic request id assigned to VM effects that suspend awaiting an
    /// engine result (for example Spawn).
    next_script_request_id: u64,
    /// Parked script continuations, keyed by actor. An actor holds at
    /// most one (UE1 gives it one script CPU); entering a new state or
    /// suspending elsewhere replaces the entry.
    actor_scripts: HashMap<ObjectId, ScriptFrame>,
    /// State membership, kept even when no code is parked: the actor's
    /// `GetStateName`/`IsInState` view and what `GotoState` mutates.
    actor_states: HashMap<ObjectId, ObjectId>,
    /// Per-actor timer wheel (`SetTimer`).
    timers: Vec<TimerEntry>,
    /// Native PHYS_Interpolating continuations started by the authored
    /// Pawn.CutCommand_FollowSpline function.
    spline_motions: HashMap<ObjectId, SplineMotion>,
    /// Active native `PlayerPawn.ShakeView` state. This is owned by the
    /// simulation because the C++ native has no UnrealScript controller actor.
    base_cam_shake: Option<BaseCamShakeState>,
    /// State bodies already reported as deferred this run; their
    /// bytecode cannot change, so re-running them would just repeat.
    deferred_states: HashSet<(ObjectId, ObjectId)>,
    /// Actors destroyed by a script effect this map. Arena handles remain
    /// stable, but these ids no longer participate in active dispatch.
    destroyed_actors: HashSet<ObjectId>,
    /// Current script animation request per actor, retained for PSA playback
    /// when the renderer consumes runtime animation state.
    pub actor_animations: HashMap<ObjectId, ActorAnimation>,
    /// Probe events disabled on an actor's active state frame by
    /// `Disable(name)`. `Enable` removes the same case-folded key; a state
    /// transition replaces its frame and clears this state-local control.
    disabled_events: HashMap<ObjectId, HashSet<String>>,
    /// Canonical case-folded `Package.Group.Mesh` -> decoded skeletal/native
    /// metadata. Entries are populated lazily on the first native query.
    skeletal_assets: HashMap<String, SkeletalAssetMetadata>,
    /// Canonical case-folded animation-object path -> PSA sequence metadata.
    /// Non-null actor `SkelAnim` entries use this cache instead of the mesh's
    /// `DefaultAnimation`, matching `AActor::GetAnim`.
    animation_assets: HashMap<String, AnimationAssetMetadata>,
    /// Per-map bounded diagnostics for animation requests whose serialized
    /// spawned-actor asset references cannot be resolved.
    animation_metadata_reported: HashSet<(ObjectId, String)>,
    finish_animation_fallback_reported: HashSet<ObjectId>,
    /// Test-only trace geometry, kept separate from map archives so synthetic
    /// script dispatch can exercise the shared collision resolver.
    #[cfg(test)]
    test_trace_collision_polys: Option<Vec<CollisionPoly>>,
}

impl Engine {
    /// Bootstrap stock packages from `<root>/System`, resolve the tick
    /// policy out of the merged INI set, and prepare the RNG stream.
    pub fn bootstrap(data_root: &Path, ini: IniSet, options: EngineOptions) -> Result<Self> {
        let world = World::load(data_root)?;
        let frame_rate_limit = ini
            .get("Engine.GameEngine", "FrameRateLimit")
            .map(hp_ini::atof_quirk)
            .unwrap_or(0.0);
        let aliases: Vec<&str> = ini.get_array("Engine.Input", "Aliases");
        Ok(Self {
            world,
            data_root: data_root.to_path_buf(),
            level: Level {
                root: ObjectId(u32::MAX),
                actors: Vec::new(),
                model_count: 0,
                export_ids: Vec::new(),
                archive: None,
            },
            rng: SimRng::seeded(options.rng_seed),
            fixed_dt: options.fixed_dt.filter(|dt| dt.is_finite() && *dt > 0.0),
            editor_mode: options.editor_mode,
            frame_rate_limit,
            bindings: Bindings::parse(&aliases),
            ini,
            cues: Vec::new(),
            script_logs: Vec::new(),
            counters: RunCounters::default(),
            sound_cues: Vec::new(),
            sounds_enabled: false,
            audio: None,
            pawn: None,
            collision: None,
            pawn_first_person: false,
            pending_level_exit: false,
            level_exit_destination: None,
            fired_triggers: HashSet::new(),
            sim_time: 0.0,
            next_script_request_id: 0,
            destroyed_actors: HashSet::new(),
            actor_animations: HashMap::new(),
            actor_scripts: HashMap::new(),
            actor_states: HashMap::new(),
            timers: Vec::new(),
            spline_motions: HashMap::new(),
            base_cam_shake: None,
            deferred_states: HashSet::new(),
            disabled_events: HashMap::new(),
            skeletal_assets: HashMap::new(),
            animation_assets: HashMap::new(),
            animation_metadata_reported: HashSet::new(),
            finish_animation_fallback_reported: HashSet::new(),
            deferred_sounds: HashSet::new(),
            quit_requested: false,
            script_lookup: HashMap::new(),
            deferred_scripts: HashSet::new(),
            deferral_reported: HashSet::new(),
            deferral_reason_codes: HashSet::new(),
            native_deferral_census: BTreeMap::new(),
            #[cfg(test)]
            test_trace_collision_polys: None,
        })
    }

    /// Load a map by harness token and bring every serialized actor up for
    /// play using UE1's four-phase startup lifecycle. With an audio host
    /// attached (interactive mode), also start the level's triggered music
    /// and resolve ambient sound actors.

    /// Preserve `bPersistent` actors while applying a new map.
    pub fn load_map(&mut self, map_token: &str) -> Result<()> {
        let mut transfer = (!self.level.actors.is_empty())
            .then(|| self.campaign_state())
            .transpose()?;
        if let Some(transfer) = transfer.as_mut() {
            transfer.animations.retain(|animation| {
                transfer
                    .persistent_actors
                    .iter()
                    .any(|actor| actor.actor_identity.eq_ignore_ascii_case(&animation.actor_identity))
            });
        }
        self.load_map_with_campaign(map_token, transfer.as_ref())
    }
    pub fn load_map_with_campaign(
        &mut self,
        map_token: &str,
        campaign: Option<&CampaignState>,
    ) -> Result<()> {
        let level = {
            let arena = &mut self.world.arena;
            load_level(arena, &self.data_root, map_token)?
        };
        // `decode_value` retains linker-local indices inside NameProperty
        // payloads. Actor.Tag participates in global FName comparisons
        // (`AllActors(..., Tag)` and TriggerEvent), so remap it through the
        // level linker's local-name text before any startup script runs.
        if let (Some(archive), Some(tag_property)) = (
            level.archive.as_ref(),
            self.world.arena.names.find_index("Tag"),
        ) {
            let remapped: Vec<_> = level
                .actors
                .iter()
                .filter_map(|actor| {
                    let PropValue::Name(local) = self
                        .world
                        .arena
                        .get(*actor)
                        .ok()?
                        .properties()?
                        .get(tag_property)?
                    else {
                        return None;
                    };
                    let text = archive.names.get(local.index as usize)?.text.as_str();
                    let index = self.world.arena.names.find_index(text)?;
                    Some((
                        *actor,
                        hp_uobject::name::Name {
                            index,
                            number: local.number,
                        },
                    ))
                })
                .collect();
            for (actor, tag) in remapped {
                if let Some(properties) = self.world.arena.get_mut(actor)?.properties_mut() {
                    properties.set(tag_property, PropValue::Name(tag));
                }
            }
        }
        // Per-map state resets on every load (startup and map
        // transitions): trigger one-shots, the exit latch, and deferral
        self.script_logs.clear();
        // reports all describe the previous map's actors.
        self.fired_triggers.clear();
        self.pending_level_exit = false;
        self.level_exit_destination = None;
        self.deferred_scripts.clear();
        self.animation_metadata_reported.clear();
        self.finish_animation_fallback_reported.clear();
        self.deferral_reported.clear();
        self.deferral_reason_codes.clear();
        self.native_deferral_census.clear();
        // The scheduler describes the previous map's actors entirely.
        self.actor_scripts.clear();
        self.actor_states.clear();
        self.timers.clear();
        self.base_cam_shake = None;
        self.deferred_states.clear();
        self.disabled_events.clear();
        self.destroyed_actors.clear();
        self.actor_animations.clear();
        self.level = level;
        self.resolve_level_class_references()?;
        self.bind_runtime_game_context()?;
        if let Some(campaign) = campaign {
            self.restore_campaign_properties(campaign)?;
        }
        // Phase D: a map carrying a PlayerStart spawns Harry there with
        // a collision world built from the same geometry the renderer
        // shows. Maps without one keep no pawn at all — a transition
        // into such a map must not inherit the previous map's pawn.
        if let Some(pose) = crate::scene::player_start_pose(&self.world.arena, &self.level) {
            self.collision = Some(CollisionWorld::from_level(&self.world.arena, &self.level));
            self.pawn = Some(PlayerPawn::spawn_at(pose.location, pose.rotation));
        } else {
            self.collision = None;
            self.pawn = None;
        }
        self.run_startup_lifecycle()?;
        if let Some(campaign) = campaign {
            self.restore_campaign_animations(campaign)?;
        }
        self.bind_runtime_cutscene_camera()?;
        if let Some(game_state) = self.player_game_state() {
            self.screen_actors_by_game_state(Some(&game_state))?;
        }
        if self.audio.is_some() {
            self.start_level_music();
            self.scan_ambient_sounds();
        }

        // Lazy-resolve map-referenced stock packages (e.g. HProps.u) so
        // the caller's render-scene extraction sees every mesh source.
        self.ensure_referenced_archives();
        Ok(())
    }

    /// Capture all campaign-bearing runtime state in deterministic actor
    /// order. Transform state is limited to actors whose effective
    /// `bPersistent` property is true; authored animation state is retained
    fn persistent_name_text(&self, index: u32) -> String {
        self.world
            .arena
            .names
            .text(index)
            .or_else(|| {
                self.level
                    .archive
                    .as_ref()
                    .and_then(|archive| archive.names.get(index as usize))
                    .map(|entry| entry.text.as_str())
            })
            .unwrap_or("None")
            .to_string()
    }

    fn persistent_value_name_text(&self, index: u32, map_local: bool) -> String {
        if map_local
            && let Some(text) = self
                .level
                .archive
                .as_ref()
                .and_then(|archive| archive.names.get(index as usize))
                .map(|entry| entry.text.clone())
        {
            return text;
        }
        self.persistent_name_text(index)
    }

    fn persistent_object_path(&self, raw: i32, map_local: bool) -> Option<String> {
        if raw == 0 {
            return None;
        }
        if map_local {
            if raw > 0 {
                if let Some(object) = self.level.export_ids.get(raw as usize - 1) {
                    return self.world.arena.path_of(*object).ok();
                }
            } else if let Some(archive) = self.level.archive.as_ref()
                && let Some(label) = crate::scene::archive_object_label(archive, raw)
            {
                let canonical = label.split('.').rev().collect::<Vec<_>>().join(".");
                if let Some(object) = self.world.arena.find_by_path(&canonical) {
                    return self.world.arena.path_of(object).ok();
                }
            }
        }
        (raw >= 0)
            .then(|| self.world.arena.path_of(ObjectId(raw as u32)).ok())
            .flatten()
    }

    fn capture_persistent_value(
        &self,
        value: &PropValue,
        map_local_names: bool,
        map_local_objects: bool,
    ) -> PersistentPropertyValue {
        match value {
            PropValue::Byte(value) => PersistentPropertyValue::Byte(*value),
            PropValue::Int(value) => PersistentPropertyValue::Int(*value),
            PropValue::Bool(value) => PersistentPropertyValue::Bool(*value),
            PropValue::Float(value) => PersistentPropertyValue::Float(*value),
            PropValue::Object(raw) => PersistentPropertyValue::Object {
                path: raw.and_then(|raw| self.persistent_object_path(raw, map_local_objects)),
                raw: *raw,
            },
            PropValue::Name(name) => PersistentPropertyValue::Name {
                text: self.persistent_value_name_text(name.index, map_local_names),
                number: name.number,
            },
            PropValue::Str(value) => PersistentPropertyValue::Str(value.clone()),
            PropValue::Struct {
                struct_name,
                fields,
            } => PersistentPropertyValue::Struct {
                struct_name: self.persistent_name_text(*struct_name),
                fields: fields
                    .iter()
                    .map(|(name, value)| {
                        (
                            self.persistent_name_text(*name),
                            self.capture_persistent_value(
                                value,
                                map_local_names,
                                map_local_objects,
                            ),
                        )
                    })
                    .collect(),
            },
            PropValue::Array(values) => PersistentPropertyValue::Array(
                values
                    .iter()
                    .map(|value| {
                        self.capture_persistent_value(
                            value,
                            map_local_names,
                            map_local_objects,
                        )
                    })
                    .collect(),
            ),
            PropValue::FixedArray(values) => PersistentPropertyValue::FixedArray(
                values
                    .iter()
                    .map(|value| {
                        self.capture_persistent_value(
                            value,
                            map_local_names,
                            map_local_objects,
                        )
                    })
                    .collect(),
            ),
            PropValue::Map(values) => PersistentPropertyValue::Map(
                values
                    .iter()
                    .map(|(key, value)| {
                        (
                            self.capture_persistent_value(
                                key,
                                map_local_names,
                                map_local_objects,
                            ),
                            self.capture_persistent_value(
                                value,
                                map_local_names,
                                map_local_objects,
                            ),
                        )
                    })
                    .collect(),
            ),
        }
    }

    fn restore_persistent_value(&mut self, value: &PersistentPropertyValue) -> PropValue {
        match value {
            PersistentPropertyValue::Byte(value) => PropValue::Byte(*value),
            PersistentPropertyValue::Int(value) => PropValue::Int(*value),
            PersistentPropertyValue::Bool(value) => PropValue::Bool(*value),
            PersistentPropertyValue::Float(value) => PropValue::Float(*value),
            PersistentPropertyValue::Object { path, raw } => {
                let resolved = path
                    .as_ref()
                    .and_then(|path| self.world.arena.find_by_path(path))
                    .map(|object| object.0 as i32)
                    .or_else(|| raw.filter(|raw| *raw < 0));
                PropValue::Object(resolved)
            }
            PersistentPropertyValue::Name { text, number } => {
                PropValue::Name(hp_uobject::name::Name {
                    index: self.world.arena.names.intern(text),
                    number: *number,
                })
            }
            PersistentPropertyValue::Str(value) => PropValue::Str(value.clone()),
            PersistentPropertyValue::Struct {
                struct_name,
                fields,
            } => PropValue::Struct {
                struct_name: self.world.arena.names.intern(struct_name),
                fields: fields
                    .iter()
                    .map(|(name, value)| {
                        let name = self.world.arena.names.intern(name);
                        let value = self.restore_persistent_value(value);
                        (name, value)
                    })
                    .collect(),
            },
            PersistentPropertyValue::Array(values) => PropValue::Array(
                values
                    .iter()
                    .map(|value| self.restore_persistent_value(value))
                    .collect(),
            ),
            PersistentPropertyValue::FixedArray(values) => PropValue::FixedArray(
                values
                    .iter()
                    .map(|value| self.restore_persistent_value(value))
                    .collect(),
            ),
            PersistentPropertyValue::Map(values) => PropValue::Map(
                values
                    .iter()
                    .map(|(key, value)| {
                        (
                            self.restore_persistent_value(key),
                            self.restore_persistent_value(value),
                        )
                    })
                    .collect(),
            ),
        }
    }

    /// Capture persistent tagged actor properties and authored animation state
    fn persistent_property_uses_runtime_object_ref(
        &self,
        actor: ObjectId,
        property_name: u32,
    ) -> bool {
        let Some(class) = self
            .world
            .arena
            .get(actor)
            .ok()
            .and_then(|object| object.class_id)
        else {
            return false;
        };
        self.world
            .arena
            .class_chain(class)
            .ok()
            .into_iter()
            .flatten()
            .flat_map(|owner| self.world.arena.children_of(owner))
            .any(|(_, field)| {
                field.name_index == property_name
                    && matches!(
                        &field.data,
                        ObjectData::Property(data)
                            if matches!(
                                &data.kind,
                                hp_uobject::value::PropertyKind::Class { .. }
                            )
                    )
            })
    }

    /// Capture persistent tagged actor properties and authored animation state
    /// for the active map.
    pub fn campaign_state(&self) -> Result<CampaignState> {
        let persistent_name = self.world.arena.names.find_index("bPersistent");
        let hidden_name = self.world.arena.names.find_index("bHidden");
        let mut actors = Vec::new();
        let mut animations = Vec::new();
        for &actor in &self.level.actors {
            let actor_path = self.world.arena.path_of(actor)?;
            let object = self.world.arena.get(actor)?;
            let actor_identity = self
                .world
                .arena
                .names
                .text(object.name_index)
                .unwrap_or("Actor")
                .to_string();
            let class_path = object
                .class_id
                .and_then(|class| self.world.arena.path_of(class).ok())
                .unwrap_or_else(|| "Engine.Actor".to_string());
            if let Some(animation) = self.actor_animations.get(&actor) {
                animations.push(PersistentAnimationState {
                    actor_path: actor_path.clone(),
                    actor_identity: actor_identity.clone(),
                    sequence: self
                        .world
                        .arena
                        .names
                        .text(animation.sequence.index)
                        .unwrap_or("None")
                        .to_string(),
                    elapsed_seconds: animation.elapsed_seconds,
                    rate: animation.rate,
                    looped: animation.looped,
                    tweening: animation.tweening,
                });
            }
            let persistent = persistent_name.is_some_and(|name| {
                matches!(
                    self.actor_effective_value(actor, name),
                    Ok(Some(PropValue::Bool(true)))
                )
            });
            if !persistent {
                continue;
            }
            let Some(location) = self.actor_location(actor) else {
                continue;
            };
            let rotation = self.actor_rotation(actor).unwrap_or([0; 3]);
            let hidden = hidden_name.is_some_and(|name| {
                matches!(
                    self.actor_effective_value(actor, name),
                    Ok(Some(PropValue::Bool(true)))
                )
            });
            let properties = self
                .world
                .arena
                .get(actor)?
                .properties()
                .map(|store| {
                    store
                        .iter()
                        .filter_map(|(name, value)| {
                            let name_text = self.persistent_name_text(name);
                            (!name_text.eq_ignore_ascii_case("AuxAnims")
                                && !name_text.eq_ignore_ascii_case("Level")
                                && !name_text.eq_ignore_ascii_case("bScriptInitialized"))
                            .then(|| {
                                let map_local_names = !name_text.eq_ignore_ascii_case("Tag");
                                let map_local_objects =
                                    !self.persistent_property_uses_runtime_object_ref(actor, name);
                                (
                                    name_text,
                                    self.capture_persistent_value(
                                        value,
                                        map_local_names,
                                        map_local_objects,
                                    ),
                                )
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            actors.push(PersistentActorState {
                actor_path,
                actor_identity,
                class_path,
                location,
                rotation,
                hidden,
                properties,
            });
        }
        Ok(CampaignState {
            active_map: format!(
                "Maps/{}.unr",
                self.world
                    .arena
                    .names
                    .text(self.world.arena.get(self.level.root)?.name_index)
                    .unwrap_or("Map")
            ),
            current_game_state: self.player_game_state(),
            persistent_actors: actors,
            animations,
        })
    }

    fn find_campaign_actor(&self, path: &str, identity: &str) -> Option<ObjectId> {
        self.world
            .arena
            .find_by_path(path)
            .filter(|actor| self.level.actors.contains(actor))
            .or_else(|| {
                self.level.actors.iter().copied().find(|actor| {
                    self.world
                        .arena
                        .get(*actor)
                        .ok()
                        .and_then(|object| self.world.arena.names.text(object.name_index))
                        .is_some_and(|name| name.eq_ignore_ascii_case(identity))
                })
            })
    }


    fn restore_campaign_properties(&mut self, campaign: &CampaignState) -> Result<()> {
        if let Some(game_state) = campaign.current_game_state.as_ref() {
            let level_info_class = self.world.arena.find_by_path("Engine.LevelInfo");
            let level_info = level_info_class.and_then(|class| {
                self.level.actors.iter().copied().find(|actor| {
                    self.world
                        .arena
                        .get(*actor)
                        .ok()
                        .and_then(|object| object.class_id)
                        .is_some_and(|actor_class| {
                            self.world
                                .arena
                                .class_is_a(actor_class, class)
                                .unwrap_or(false)
                        })
                })
            });
            if let Some(harry) =
                level_info.and_then(|level_info| self.actor_object_property(level_info, "PlayerHarryActor"))
            {
                let name = self.world.arena.names.intern("CurrentGameState");
                self.actor_store_mut(harry, "campaign_restore")?
                    .set(name, PropValue::Str(game_state.clone()));
            }
        }
        for saved in &campaign.persistent_actors {
            let actor = if let Some(actor) =
                self.find_campaign_actor(&saved.actor_path, &saved.actor_identity)
            {
                actor
            } else {
                let Some(class) = self.world.arena.find_by_path(&saved.class_path) else {
                    eprintln!(
                        "hp-engine: [engine.save_persistent_class_missing] {} ({})",
                        saved.actor_identity, saved.class_path
                    );
                    continue;
                };
                let actor =
                    self.spawn_persistent_actor(class, saved.location, saved.rotation)?;
                let name = self.world.arena.names.intern(&saved.actor_identity);
                self.world.arena.get_mut(actor)?.name_index = name;
                actor
            };
            let restored_properties: Vec<_> = saved
                .properties
                .iter()
                .filter(|(name, _)| {
                    !name.eq_ignore_ascii_case("AuxAnims")
                        && !name.eq_ignore_ascii_case("Level")
                        && !name.eq_ignore_ascii_case("bScriptInitialized")
                })
                .map(|(name, value)| {
                    let name = self.world.arena.names.intern(name);
                    let value = self.restore_persistent_value(value);
                    (name, value)
                })
                .collect();
            {
            let initialized = self.world.arena.names.intern("bScriptInitialized");
            self.actor_store_mut(actor, "campaign_restore")?
                .set(initialized, PropValue::Bool(false));
                let store = self.actor_store_mut(actor, "campaign_restore")?;
                for (name, value) in restored_properties {
                    store.set(name, value);
                }
            }
            self.set_actor_vector(actor, "Location", saved.location)?;
            self.set_actor_rotator(actor, "Rotation", saved.rotation)?;
            let hidden = self.world.arena.names.intern("bHidden");
            self.actor_store_mut(actor, "campaign_restore")?
                .set(hidden, PropValue::Bool(saved.hidden));
        }
        Ok(())
    }

    fn restore_campaign_animations(&mut self, campaign: &CampaignState) -> Result<()> {
        for saved in &campaign.animations {
            let Some(actor) =
                self.find_campaign_actor(&saved.actor_path, &saved.actor_identity)
            else {
                eprintln!(
                    "hp-engine: [engine.save_animation_actor_missing] {}",
                    saved.actor_identity
                );
                continue;
            };
            let sequence = hp_uobject::name::Name {
                index: self.world.arena.names.intern(&saved.sequence),
                number: 0,
            };
            let (timing, mesh_path, animation_path) =
                self.animation_request_metadata(actor, sequence, saved.rate);
            self.actor_animations.insert(
                actor,
                ActorAnimation {
                    sequence,
                    started_at: self.sim_time - f64::from(saved.elapsed_seconds),
                    elapsed_seconds: saved.elapsed_seconds,
                    duration_seconds: timing.map(|value| value.0),
                    finish_after_seconds: timing.map(|value| value.1),
                    mesh_path,
                    animation_path,
                    rate: saved.rate,
                    looped: saved.looped,
                    tweening: saved.tweening,
                },
            );
        }
        Ok(())
    }

    /// Map-instance object values retain signed references into the map
    /// linker. Resolve class-typed properties while that archive context is
    /// available so calls crossing into an Engine helper (for example
    /// `SpawnThingy.Trigger -> Actor.FancySpawn`) do not lose the originating
    /// package before `Spawn` validates the class.
    fn resolve_level_class_references(&mut self) -> Result<()> {
        let Some(archive) = self.level.archive.as_ref() else {
            return Ok(());
        };
        let mut updates = Vec::new();
        for actor in &self.level.actors {
            let object = self.world.arena.get(*actor)?;
            let Some(class) = object.class_id else {
                continue;
            };
            let Some(properties) = object.properties() else {
                continue;
            };
            for (name, value) in properties.iter() {
                let PropValue::Object(Some(raw)) = value else {
                    continue;
                };
                if *raw >= 0 {
                    continue;
                }
                let is_class_property = self
                    .world
                    .arena
                    .class_chain(class)?
                    .into_iter()
                    .flat_map(|owner| self.world.arena.children_of(owner))
                    .any(|(_, field)| {
                        field.name_index == name
                            && matches!(
                                &field.data,
                                ObjectData::Property(data)
                                    if matches!(data.kind, hp_uobject::value::PropertyKind::Class { .. })
                            )
                    });
                if !is_class_property {
                    continue;
                }
                let Some(label) = crate::scene::archive_object_label(archive, *raw) else {
                    continue;
                };
                // Import outer chains are reported leaf-first by the archive
                // walker; arena paths are package-first.
                let canonical = label.split('.').rev().collect::<Vec<_>>().join(".");
                let Some(resolved) = self.world.arena.find_by_path(&canonical) else {
                    continue;
                };
                if !matches!(self.world.arena.get(resolved)?.data, ObjectData::Class(_)) {
                    return Err(EngineError::new(
                        "native.spawn_class_invalid",
                        format!(
                            "map class property {} raw {raw} resolved to non-class {canonical}",
                            self.world.arena.names.text(name).unwrap_or("?")
                        ),
                    ));
                }
                let resolved_raw = i32::try_from(resolved.0).map_err(|_| {
                    EngineError::new(
                        "native.object_invalid",
                        format!("resolved map class {canonical} does not fit an object value"),
                    )
                })?;
                updates.push((*actor, name, resolved_raw));
            }
        }
        for (actor, name, resolved) in updates {
            self.world
                .arena
                .get_mut(actor)?
                .properties_mut()
                .expect("collected from property object")
                .set(name, PropValue::Object(Some(resolved)));
        }
        Ok(())
    }

    /// Bring serialized actors up for play exactly as
    /// `UGameEngine::BeginPlay` does: one complete export-order pass per
    /// event, with state selection last.
    fn run_startup_lifecycle(&mut self) -> Result<()> {
        // Each source pass independently tests bScriptInitialized. An actor
        // preset by native construction, or one that sets the bit during an
        // earlier phase, receives none of the later startup events.
        for event in [
            "PreBeginPlay",
            "BeginPlay",
            "PostBeginPlay",
            "SetInitialState",
        ] {
            let actors = self.level.actors.clone();
            for actor in actors {
                if self.destroyed_actors.contains(&actor)
                    || !self.level.actors.contains(&actor)
                    || self.actor_script_initialized(actor)?
                {
                    continue;
                }
                self.run_named_event_on(actor, event, Some(0.0))?;
            }
        }
        Ok(())
    }

    fn actor_script_initialized(&self, actor: ObjectId) -> Result<bool> {
        let Some(name) = self.world.arena.names.find_index("bScriptInitialized") else {
            return Ok(false);
        };
        let object = self.world.arena.get(actor)?;
        if let Some(value) = object.properties().and_then(|store| store.get(name)) {
            return Ok(matches!(value, PropValue::Bool(true)));
        }
        let Some(class_id) = object.class_id else {
            return Ok(false);
        };
        for class in self.world.arena.class_chain(class_id)? {
            let default_object = match &self.world.arena.get(class)?.data {
                ObjectData::Class(data) => data.default_object,
                _ => None,
            };
            if let Some(value) = default_object
                .and_then(|id| self.world.arena.get(id).ok())
                .and_then(|object| object.properties())
                .and_then(|store| store.get(name))
            {
                return Ok(matches!(value, PropValue::Bool(true)));
            }
        }
        Ok(false)
    }

    /// Port of `ULevel::ScreenActorsByGameState` (`UnLevel.cpp:171-244`).
    /// Exclusions win over groups; `OnResolveGameState` runs only when the
    /// retail `bHasGState` guard is set by a valid GSTATE group or a matching
    /// exclusion.
    fn screen_actors_by_game_state(&mut self, current: Option<&str>) -> Result<()> {
        let Some(current) = current.map(str::to_ascii_uppercase) else {
            return Ok(());
        };
        let group_name = self.world.arena.names.find_index("Group");
        let exclude_name = self.world.arena.names.find_index("ExcludeGameStates");
        let in_state_name = self.world.arena.names.intern("bInCurrentGameState");
        let actors = self.level.actors.clone();
        for actor in actors {
            if self.destroyed_actors.contains(&actor) || !self.level.actors.contains(&actor) {
                continue;
            }
            let property_text = |property: Option<u32>| -> Option<String> {
                let value = self
                    .world
                    .arena
                    .get(actor)
                    .ok()?
                    .properties()?
                    .get(property?)?;
                match value {
                    PropValue::Str(text) => Some(text.to_ascii_uppercase()),
                    PropValue::Name(name) => self
                        .level
                        .archive
                        .as_ref()
                        .and_then(|archive| archive.names.get(name.index as usize))
                        .map(|entry| entry.text.to_ascii_uppercase())
                        .or_else(|| {
                            self.world
                                .arena
                                .names
                                .text(name.index)
                                .map(str::to_ascii_uppercase)
                        }),
                    _ => None,
                }
            };
            let group = property_text(group_name).unwrap_or_default();
            let excluded_states = property_text(exclude_name).unwrap_or_default();
            let has_game_state_group = group
                .split(',')
                .map(str::trim)
                .any(|token| token.contains("GSTATE"));
            let group_matches = group
                .split(',')
                .map(str::trim)
                .any(|token| token == current);
            let excluded = excluded_states
                .split(',')
                .map(str::trim)
                .any(|token| token == current);
            if !has_game_state_group && !excluded {
                continue;
            }
            let included = (!has_game_state_group || group_matches) && !excluded;
            if let Some(store) = self.world.arena.get_mut(actor)?.properties_mut() {
                store.set(in_state_name, PropValue::Bool(included));
            }
            self.run_named_event_on(actor, "OnResolveGameState", None)?;
        }
        Ok(())
    }
    fn player_game_state(&self) -> Option<String> {
        let level_info_class = self.world.arena.find_by_path("Engine.LevelInfo")?;
        let level_info = self.level.actors.iter().copied().find(|actor| {
            self.world
                .arena
                .get(*actor)
                .ok()
                .and_then(|object| object.class_id)
                .is_some_and(|class| {
                    self.world
                        .arena
                        .class_is_a(class, level_info_class)
                        .unwrap_or(false)
                })
        })?;
        let harry = self.actor_object_property(level_info, "PlayerHarryActor")?;
        let game_state = self.world.arena.names.find_index("CurrentGameState")?;
        match self.actor_effective_value(harry, game_state).ok()?? {
            PropValue::Str(value) => Some(value),
            PropValue::Name(name) => self
                .world
                .arena
                .names
                .text(name.index)
                .map(str::to_string),
            _ => None,
        }
    }


    /// Install the gameplay actors before startup, matching `UnGame.cpp`
    /// `InitGameInfo` (1329-1344) and `BeginPlay` (1419-1426): GameInfo and
    /// its base Mutator exist before any actor receives `PreBeginPlay`.
    ///
    /// With no URL options, `GameInfo.InitGame`'s material runtime result is
    /// `BaseMutator = Spawn(MutatorClass)`. Constructing both from the shipped
    /// CDOs produces that result while leaving relevance to the authored VM
    /// implementations of `GameInfo.IsRelevant` and `Mutator.IsRelevant`.
    fn bind_runtime_game_context(&mut self) -> Result<()> {
        let arena = &mut self.world.arena;
        let level_info_class = arena.find_by_path("Engine.LevelInfo").ok_or_else(|| {
            EngineError::new(
                "engine.level_info_missing",
                "Engine.LevelInfo is unavailable",
            )
        })?;
        let level_info = self
            .level
            .actors
            .iter()
            .copied()
            .find(|actor| {
                arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .and_then(|class| arena.class_is_a(class, level_info_class).ok())
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                EngineError::new(
                    "engine.level_info_missing",
                    "loaded map has no ALevelInfo actor",
                )
            })?;
        let property = |arena: &ObjectArena, name: &str, code: &'static str| {
            arena
                .names
                .find_index(name)
                .ok_or_else(|| EngineError::new(code, format!("{name} is unavailable")))
        };
        let level_property = property(arena, "Level", "engine.level_property_missing")?;
        let net_mode_property = property(arena, "NetMode", "engine.net_mode_property_missing")?;
        let game_property = property(arena, "Game", "engine.game_property_missing")?;
        let base_mutator_property =
            property(arena, "BaseMutator", "engine.base_mutator_property_missing")?;
        let pawn_list_property = property(arena, "PawnList", "engine.pawn_list_property_missing")?;
        let next_pawn_property = property(arena, "nextPawn", "engine.next_pawn_property_missing")?;
        let script_initialized_property = property(
            arena,
            "bScriptInitialized",
            "engine.script_initialized_property_missing",
        )?;

        let configured_game = self
            .ini
            .get("Engine.Engine", "DefaultGame")
            .unwrap_or("Engine.GameInfo");
        let game_class = arena.find_by_path(configured_game).ok_or_else(|| {
            EngineError::new(
                "engine.game_class_missing",
                format!("configured GameInfo class {configured_game:?} is unavailable"),
            )
        })?;
        let game_info_class = arena.find_by_path("Engine.GameInfo").ok_or_else(|| {
            EngineError::new(
                "engine.game_info_class_missing",
                "Engine.GameInfo is unavailable",
            )
        })?;
        if !arena.class_is_a(game_class, game_info_class)? {
            return Err(EngineError::new(
                "engine.game_class_invalid",
                format!("{configured_game:?} is not a GameInfo class"),
            ));
        }
        let mutator_class = arena.find_by_path("Engine.Mutator").ok_or_else(|| {
            EngineError::new(
                "engine.mutator_class_missing",
                "Engine.Mutator is unavailable",
            )
        })?;
        let pawn_class = arena.find_by_path("Engine.Pawn").ok_or_else(|| {
            EngineError::new("engine.pawn_class_missing", "Engine.Pawn is unavailable")
        })?;

        let instantiate = |arena: &mut ObjectArena, class, outer, name: &str| -> Result<ObjectId> {
            let defaults = match &arena.get(class)?.data {
                ObjectData::Class(data) => data
                    .default_object
                    .and_then(|id| arena.get(id).ok())
                    .and_then(UObject::properties)
                    .cloned()
                    .unwrap_or_default(),
                _ => {
                    return Err(EngineError::new(
                        "engine.runtime_class_invalid",
                        format!("{class:?} is not a class"),
                    ));
                }
            };
            let name_index = arena.names.intern(name);
            Ok(arena.alloc(UObject {
                class_id: Some(class),
                name_index,
                outer: Some(outer),
                flags: 0,
                data: ObjectData::Properties(defaults),
            }))
        };
        // Function child metadata is linked through UField::Next. Bind the
        // authored relevance signatures so script arguments (including the
        // Mutator out byte) land in their declared locals.
        for (class, function) in [
            (game_info_class, "IsRelevant"),
            (mutator_class, "AlwaysKeep"),
            (mutator_class, "IsRelevant"),
            (mutator_class, "CheckReplacement"),
        ] {
            let Some(function_id) = arena.find_function(class, function)? else {
                continue;
            };
            let fields: Vec<_> = arena
                .children_of(function_id)
                .filter_map(|(id, object)| match &object.data {
                    ObjectData::Property(data) => Some((id, data.links.next, data.property_flags)),
                    _ => None,
                })
                .collect();
            let mut params = Vec::new();
            let mut locals = Vec::new();
            let mut current = fields
                .iter()
                .find(|(id, _, _)| !fields.iter().any(|(_, next, _)| next == &Some(*id)))
                .map(|(id, _, _)| *id);
            while let Some(id) = current {
                let Some((_, next, flags)) =
                    fields.iter().find(|(candidate, _, _)| *candidate == id)
                else {
                    break;
                };
                if flags & 0x80 != 0 {
                    if flags & 0x400 == 0 {
                        params.push(id);
                    }
                } else {
                    locals.push(id);
                }
                current = *next;
            }
            if let ObjectData::Function(data) = &mut arena.get_mut(function_id)?.data {
                data.params = params;
                data.locals = locals;
            }
        }
        let game = instantiate(arena, game_class, self.level.root, "GameInfo")?;
        let mutator = instantiate(arena, mutator_class, self.level.root, "BaseMutator")?;
        // Both source operations are SpawnActor calls, so they enter the
        // level actor array. Their native spawn startup has completed.
        self.level.actors.push(game);
        self.level.actors.push(mutator);

        let object_value = |id: ObjectId, subject: &'static str| -> Result<PropValue> {
            let raw = i32::try_from(id.0).map_err(|_| {
                EngineError::new(
                    "engine.runtime_object_id_range",
                    format!("{subject} arena id {id:?} exceeds PropValue::Object"),
                )
            })?;
            Ok(PropValue::Object(Some(raw)))
        };
        let level_value = object_value(level_info, "LevelInfo")?;
        if let Some(properties) = arena.get_mut(level_info)?.properties_mut() {
            properties.set(net_mode_property, PropValue::Byte(0));
            properties.set(pawn_list_property, PropValue::Object(None));
            properties.set(game_property, object_value(game, "GameInfo")?);
        }
        if let Some(properties) = arena.get_mut(game)?.properties_mut() {
            properties.set(level_property, level_value.clone());
            properties.set(base_mutator_property, object_value(mutator, "BaseMutator")?);
            properties.set(script_initialized_property, PropValue::Bool(true));
        }
        if let Some(properties) = arena.get_mut(mutator)?.properties_mut() {
            properties.set(level_property, level_value.clone());
            properties.set(script_initialized_property, PropValue::Bool(true));
        }
        for actor in &self.level.actors {
            let is_pawn = arena
                .get(*actor)?
                .class_id
                .is_some_and(|class| arena.class_is_a(class, pawn_class).unwrap_or(false));
            if let Some(properties) = arena.get_mut(*actor)?.properties_mut() {
                properties.set(level_property, level_value.clone());
                if is_pawn {
                    properties.set(next_pawn_property, PropValue::Object(None));
                }
            }
        }
        // The map already serializes its playable Harry pawn. Native
        // `SpawnPlayActor` publishes that actor through LevelInfo before
        // startup; scripts deliberately use this pointer (rather than an
        // AllActors search) to reach Harry and his BaseCam.
        if let Some(player_pawn_class) = arena
            .find_by_path("HGame.Harry")
            .or_else(|| arena.find_by_path("Engine.PlayerPawn"))
            && let Some(player_actor) = self.level.actors.iter().copied().find(|actor| {
                arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .is_some_and(|class| {
                        arena.class_is_a(class, player_pawn_class).unwrap_or(false)
                    })
            })
        {
            let player_property = arena.names.intern("PlayerHarryActor");
            if let Some(properties) = arena.get_mut(level_info)?.properties_mut() {
                properties.set(
                    player_property,
                    object_value(player_actor, "PlayerHarryActor")?,
                );
            }
        }
        Ok(())
    }
    fn bind_runtime_cutscene_camera(&mut self) -> Result<()> {
        let Some(cut_name) = self.world.arena.names.find_index("CutName") else {
            return Ok(());
        };
        let Some(base_cam) = self.level.actors.iter().copied().find(|actor| {
            matches!(
                self.actor_effective_value(*actor, cut_name),
                Ok(Some(PropValue::Str(name))) if name.eq_ignore_ascii_case("BaseCam")
            )
        }) else {
            return Ok(());
        };
        let Some(player_class) = self
            .world
            .arena
            .find_by_path("HGame.Harry")
            .or_else(|| self.world.arena.find_by_path("Engine.PlayerPawn"))
        else {
            return Ok(());
        };
        let Some(player) = self.level.actors.iter().copied().find(|actor| {
            self.world
                .arena
                .get(*actor)
                .ok()
                .and_then(|object| object.class_id)
                .is_some_and(|class| {
                    self.world
                        .arena
                        .class_is_a(class, player_class)
                        .unwrap_or(false)
                })
        }) else {
            return Ok(());
        };
        let Some(level_info_class) = self.world.arena.find_by_path("Engine.LevelInfo") else {
            return Ok(());
        };
        if let Some(level_info) = self.level.actors.iter().copied().find(|actor| {
            self.world
                .arena
                .get(*actor)
                .ok()
                .and_then(|object| object.class_id)
                .is_some_and(|class| {
                    self.world
                        .arena
                        .class_is_a(class, level_info_class)
                        .unwrap_or(false)
                })
        }) {
            // LevelInfo.PreBeginPlay performs the source AllActors lookup,
            // but runtime publication is authoritative after startup: every
            // CutScript thread and BaseCam's authored special case traverse
            // this exact LevelInfo -> Harry -> cam relationship.
            self.set_actor_object(level_info, "PlayerHarryActor", Some(player))?;
        }
        self.set_actor_object(player, "cam", Some(base_cam))?;
        self.set_actor_object(base_cam, "playerHarry", Some(player))?;
        Ok(())
    }

    /// Transitively lazy-load stock packages referenced by root imports
    /// but absent from `World::archives` (e.g. `HProps.u`: the map may
    /// only import `HGame`, whose OWN root imports pull `HProps`). The
    /// closure starts from the loaded map's archive plus every archive
    /// already held; each archive's ROOT imports (`outer_ref == 0`) name
    /// package stems, read from `<data_root>/System/<stem>.u`, inserted
    /// under the stem AS WRITTEN IN THE IMPORT TABLE (authored case,
    /// like the bootstrap), and pushed onto the worklist so THEIR imports
    /// are scanned too. Each stem is parsed at most once; sources are
    /// visited in sorted order (FIFO over sorted batches) so output is
    /// deterministic. A missing/unreadable/undecodable package is one
    /// loud `[engine.archive_lazy]` note — never a load failure; its
    /// subtree simply stays unresolved.
    pub fn ensure_referenced_archives(&mut self) {
        // Case-folded coverage set: present keys plus attempted failures,
        // so nothing is scanned or retried twice.
        let mut scanned: HashSet<String> = self
            .world
            .archives
            .keys()
            .map(|key| key.to_lowercase())
            .collect();
        let mut queue: VecDeque<String> = VecDeque::new();

        // Seed batch 1: the loaded map's own root imports.
        if let Some(archive) = self.level.archive.as_ref() {
            for stem in root_import_stems(archive) {
                if scanned.insert(stem.to_lowercase()) {
                    queue.push_back(stem);
                }
            }
        }
        // Seed batch 2: root imports of everything already held (stock
        // archives may reference further packages themselves).
        let mut held: Vec<String> = self.world.archives.keys().cloned().collect();
        held.sort_by_key(|key| key.to_lowercase());
        for key in held {
            let Some(archive) = self.world.archives.get(&key) else {
                continue;
            };
            for stem in root_import_stems(archive) {
                if scanned.insert(stem.to_lowercase()) {
                    queue.push_back(stem);
                }
            }
        }

        while let Some(stem) = queue.pop_front() {
            let path = self.data_root.join("System").join(format!("{stem}.u"));
            let loaded = std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    hp_format::package79::read_package(&bytes)
                        .map_err(|error| format!("package decode: {error}"))
                });
            let archive = match loaded {
                Ok(archive) => archive,
                Err(reason) => {
                    eprintln!("hp-engine: [engine.archive_lazy] {stem}: {reason}");
                    continue;
                }
            };
            // The freshly parsed archive extends the closure; scan it
            // before the insert hands ownership to the map.
            for next in root_import_stems(&archive) {
                if scanned.insert(next.to_lowercase()) {
                    queue.push_back(next);
                }
            }
            self.world.archives.insert(stem, archive);
        }
    }

    /// Take ownership of the gameplay audio host. Called once at windowed
    /// boot; headless paths never call this.
    pub fn attach_audio(&mut self, host: crate::music::AudioHost) {
        self.audio = Some(host);
    }

    /// Resolve and open the loaded level's music the way the native game
    /// does: the first `MusicTrigger` actor's `Song` string names
    /// `Music/<Song>.ogg` (`ALAudioSubsystem::PlayMusic`). A missing song
    /// file is the shipped data's own gap — the reference game logs a
    /// warning and stays silent, and so do we, loudly. No substitutes.
    pub fn start_level_music(&mut self) {
        let Some(host) = self.audio.as_mut() else {
            return;
        };
        let Some(song) = crate::music::extract_triggered_song(&self.level, &self.world.arena)
        else {
            eprintln!(
                "hp-engine: [engine.music_missing] map carries no MusicTrigger Song — continuing silent"
            );
            return;
        };
        let Some(path) = crate::music::resolve_song_file(&self.data_root, &song) else {
            eprintln!(
                "hp-engine: [engine.music_missing] song \"{song}\" not found under {} (native PlayMusic behavior: silence)",
                self.data_root.join("Music").display()
            );
            return;
        };
        // Keep the file handle alive so `lsof -p <pid>` shows the playing
        // asset.
        let source_file = std::fs::File::open(&path).ok();
        match OggSource::open(&path, StreamKind::OggLooping) {
            Ok(source) => match host.music.open(Box::new(source), source_file) {
                Ok((id, info)) => println!(
                    "hp-engine: [engine.music_started] stream {id}: {} — song \"{song}\" ({}ch @ {} Hz, looping)",
                    path.display(),
                    info.channels,
                    info.sample_rate
                ),
                Err(error) => eprintln!("hp-engine: [audio.music_open_failed] {error}"),
            },
            Err(error) => {
                eprintln!(
                    "hp-engine: [engine.music_missing] {}: {error}",
                    path.display()
                )
            }
        }
    }
    fn start_script_music(&mut self, song: &str) {
        let Some(path) = crate::music::resolve_song_file(&self.data_root, song) else {
            eprintln!(
                "hp-engine: [engine.music_missing] PlayMusic \"{song}\": no file under {}",
                self.data_root.join("Music").display()
            );
            return;
        };
        let source_file = std::fs::File::open(&path).ok();
        let source = match OggSource::open(&path, StreamKind::OggLooping) {
            Ok(source) => source,
            Err(error) => {
                eprintln!(
                    "hp-engine: [engine.music_missing] PlayMusic \"{song}\" at {}: {error}",
                    path.display()
                );
                return;
            }
        };
        let Some(host) = self.audio.as_mut() else {
            eprintln!(
                "hp-engine: [engine.music_unavailable] PlayMusic \"{song}\": no audio host attached"
            );
            return;
        };
        match host.music.open(Box::new(source), source_file) {
            Ok((id, info)) => println!(
                "hp-engine: [engine.music_started] stream {id}: {} — song \"{song}\" ({}ch @ {} Hz, looping)",
                path.display(),
                info.channels,
                info.sample_rate
            ),
            Err(error) => {
                eprintln!("hp-engine: [audio.music_open_failed] PlayMusic \"{song}\": {error}")
            }
        }
    }

    /// Resolve AmbientSound actors' sound references far enough to decide
    /// playability: only plain-Ogg payloads inside `Sounds/*.uax` open as
    /// loops; everything else reports `[engine.sound_deferred]` once per
    /// class.
    pub fn scan_ambient_sounds(&mut self) {
        let candidates = crate::music::collect_ambient_candidates(&self.level, &self.world.arena);
        for candidate in candidates {
            let reference = format!("{}.{}", candidate.package, candidate.object);
            match crate::music::resolve_uax_sound(
                &self.data_root,
                &candidate.package,
                &candidate.object,
            ) {
                crate::music::ResolvedSound::PlainOgg(bytes) => {
                    let opened = OggSource::from_bytes(bytes, StreamKind::OggLooping).ok();
                    let Some(source) = opened else {
                        continue;
                    };
                    if let Some(host) = self.audio.as_mut()
                        && let Ok((id, info)) = host.ambient.open(Box::new(source), None)
                    {
                        println!(
                            "hp-engine: [engine.ambient_started] stream {id}: {reference} ({}ch @ {} Hz, looping @ {:.0}% gain)",
                            info.channels,
                            info.sample_rate,
                            crate::music::AMBIENT_GAIN * 100.0
                        );
                    }
                }
                crate::music::ResolvedSound::EaXa {
                    encoded,
                    num_samples,
                    sample_rate,
                    looping,
                } => {
                    let kind = if looping {
                        StreamKind::XaLooping
                    } else {
                        StreamKind::Xa
                    };
                    match XaSource::new(encoded, num_samples, sample_rate, kind) {
                        Ok(source) => {
                            if let Some(host) = self.audio.as_mut()
                                && let Ok((id, info)) = host.ambient.open(Box::new(source), None)
                            {
                                println!(
                                    "hp-engine: [engine.ambient_started] stream {id}: {reference} ({}ch @ {} Hz, {} @ {:.0}% gain)",
                                    info.channels,
                                    info.sample_rate,
                                    if looping { "looping" } else { "finite" },
                                    crate::music::AMBIENT_GAIN * 100.0
                                );
                            }
                        }
                        Err(error) => {
                            if self.deferred_sounds.insert(candidate.class.clone()) {
                                eprintln!(
                                    "hp-engine: [engine.sound_deferred] class={} ref={reference}: {error}",
                                    candidate.class
                                );
                            }
                        }
                    }
                }
                crate::music::ResolvedSound::Deferred(reason) => {
                    if self.deferred_sounds.insert(candidate.class.clone()) {
                        eprintln!(
                            "hp-engine: [engine.sound_deferred] class={} ref={reference}: {reason}",
                            candidate.class
                        );
                    }
                }
            }
        }
    }

    /// `UEngine::GetMaxTickRate` mirror: 30 in editor mode, uncapped (0)
    /// otherwise, honoring the `FrameRateLimit` ini key when set.
    pub fn get_max_tick_rate(&self) -> f32 {
        if self.editor_mode {
            return 30.0;
        }
        if self.frame_rate_limit > 0.0 {
            return self.frame_rate_limit;
        }
        0.0
    }

    /// Effective delta seconds for one tick: `--fixed-dt` override, else
    /// the capped cadence `1 / max_tick_rate`, else the nominal 60 Hz
    /// frame the uncapped loop targets on this hardware class.
    pub fn next_delta(&self) -> f32 {
        if let Some(dt) = self.fixed_dt {
            return dt;
        }
        let cap = self.get_max_tick_rate();
        if cap > 0.0 { 1.0 / cap } else { 1.0 / 60.0 }
    }

    /// Advance the player pawn one tick with shell-gathered input.
    ///
    /// The windowed shell (and the headless `--pawn-script` loop) calls
    /// this BEFORE [`Engine::tick`], keeping the frame order input
    /// dispatch → PAWN PHYSICS → rng → actor ticks while `Engine::tick`
    /// itself never moves the pawn. No-op `Ok` when no pawn/collision
    /// world exists (map without a PlayerStart, headless fly paths).
    pub fn pawn_step(&mut self, input: PawnInput) -> Result<()> {
        // Delta first: `next_delta(&self)` cannot run under the later
        // exclusive loan on `self.pawn`.
        let dt = self.next_delta();
        let Some(pawn) = self.pawn.as_mut() else {
            return Ok(());
        };
        let Some(world) = self.collision.as_ref() else {
            return Ok(());
        };
        pawn.step(world, input, dt);
        // Phase F (plan F16): after movement, walk the level's trigger
        // actors against the pawn's new pose. Once-per-actor per map load.
        self.check_level_triggers();
        Ok(())
    }

    /// Spawn the player pawn at an explicit pose instead of the map's
    /// PlayerStart (plan F17: `-LOAD=` respawn). Builds the collision
    /// world when the default PlayerStart path skipped it, so physics
    /// holds at the restored location too.
    pub fn spawn_pawn_from_save(&mut self, location: [f32; 3], rotation: [i32; 3]) {
        if self.collision.is_none() {
            self.collision = Some(CollisionWorld::from_level(&self.world.arena, &self.level));
        }
        self.pawn = Some(PlayerPawn::spawn_at(location, rotation));
    }

    /// Trigger walk (plan F16): cylinder-enter test of the pawn against
    /// every loaded actor whose class name folds to `TriggerChangeLevel`
    /// (HP2's shipped exit actor). First entry logs
    /// `[engine.level_exit] <url>`, sets [`Engine::pending_level_exit`],
    /// and captures the destination map name for
    /// [`Engine::take_level_exit`].
    fn check_level_triggers(&mut self) {
        let Some(pawn) = self.pawn.as_ref() else {
            return;
        };
        // Cylinder center: the feet position raised half the capsule height
        // (`pawn.location` is FEET; see PlayerPawn).
        let center = [
            pawn.location[0],
            pawn.location[1],
            pawn.location[2] + pawn.height * 0.5,
        ];
        let arena = &self.world.arena;
        let mut exits: Vec<(ObjectId, String)> = Vec::new();
        for &actor in &self.level.actors {
            if self.fired_triggers.contains(&actor) {
                continue;
            }
            let Ok(object) = arena.get(actor) else {
                continue;
            };
            let Some(class_id) = object.class_id else {
                continue;
            };
            let is_trigger = arena
                .get(class_id)
                .ok()
                .and_then(|class| arena.names.text(class.name_index))
                .is_some_and(|name| {
                    TRIGGER_CLASSES
                        .iter()
                        .any(|candidate| name.eq_ignore_ascii_case(candidate))
                });
            if !is_trigger {
                continue;
            }
            let Some(store) = object.properties() else {
                continue;
            };
            let Some(location) = actor_vec3(store, arena, "Location") else {
                continue;
            };
            // Radius/height resolve instance-first then class-chain
            // defaults via `effective_prop` (the Trigger classes carry
            // their own CollisionRadius/CollisionHeight defaults); 64 uu
            // is the sane fallback when neither exists.
            let radius = effective_prop(arena, &self.world, actor, "CollisionRadius")
                .and_then(prop_scalar)
                .unwrap_or(64.0);
            let height = effective_prop(arena, &self.world, actor, "CollisionHeight")
                .and_then(prop_scalar)
                .unwrap_or(64.0);
            let dx = center[0] - location[0];
            let dy = center[1] - location[1];
            let dz = center[2] - location[2];
            if (dx * dx + dy * dy).sqrt() >= radius || dz.abs() >= height {
                continue;
            }
            // Exit target: the shipped TriggerChangeLevel carries its
            // destination in NewMapName (TriggerChangeLevel.uc); URL is
            // the fallback alias, `<none>` when neither exists.
            let url = ["NewMapName", "URL"]
                .iter()
                .find_map(
                    |name| match effective_prop(arena, &self.world, actor, name) {
                        Some(ResolvedProp::Str(text)) => Some(text),
                        _ => None,
                    },
                )
                .unwrap_or_else(|| "<none>".to_string());
            exits.push((actor, url));
        }
        if exits.is_empty() {
            return;
        }
        self.pending_level_exit = true;
        let mut destination: Option<String> = None;
        for (actor, url) in exits {
            println!("hp-engine: [engine.level_exit] {url}");
            if url != "<none>" {
                destination.get_or_insert(url);
            }
            self.fired_triggers.insert(actor);
        }
        self.level_exit_destination = destination;
    }

    /// Consume the level-exit latch set by [`Engine::check_level_triggers`]
    /// (the binary layer's transition path calls this once per tick): the
    /// flag clears and the captured destination map name returns. `None`
    /// when no trigger fired this run, or when it carried neither
    /// `NewMapName` nor `URL`.
    pub fn take_level_exit(&mut self) -> Option<String> {
        if !self.pending_level_exit {
            return None;
        }
        self.pending_level_exit = false;
        self.level_exit_destination.take()
    }

    /// Camera pose following the player pawn: third-person BaseCam
    /// follow by default, first-person eye when
    /// [`Engine::pawn_first_person`] is set. `None` without a pawn.
    pub fn pawn_camera(&self) -> Option<SceneCamera> {
        let pawn = self.pawn.as_ref()?;
        Some(if self.pawn_first_person {
            pawn.first_person_camera()
        } else {
            pawn.third_person_camera()
        })
    }
    /// Active CutScene camera pose. The camera identity follows the authored
    /// `LevelInfo.PlayerHarryActor -> Harry.cam` path; `CutNotifyActor` only
    /// decides whether that camera currently overrides pawn control.
    pub fn cutscene_camera(&self) -> Option<SceneCamera> {
        let level_info_class = self.world.arena.find_by_path("Engine.LevelInfo")?;
        let level_info = self.level.actors.iter().copied().find(|actor| {
            self.world
                .arena
                .get(*actor)
                .ok()
                .and_then(|object| object.class_id)
                .is_some_and(|class| {
                    self.world
                        .arena
                        .class_is_a(class, level_info_class)
                        .unwrap_or(false)
                })
        })?;
        let harry = self.actor_object_property(level_info, "PlayerHarryActor")?;
        let camera = self.actor_object_property(harry, "cam")?;
        let owner_name = self.world.arena.names.find_index("CutNotifyActor")?;
        matches!(
            self.actor_effective_value(camera, owner_name),
            Ok(Some(PropValue::Object(Some(owner)))) if owner >= 0
        )
        .then(|| {
            let mut rotation = self.actor_rotation(camera)?;
            if let Some(shake) = &self.base_cam_shake {
                for (component, delta) in rotation.iter_mut().zip(shake.rotation_delta) {
                    *component = component.wrapping_add(delta);
                }
            }
            Some(SceneCamera {
                location: self.actor_location(camera)?,
                rotation,
            })
        })
        .flatten()
    }
    /// Complete live state of the authored `Harry.cam` object. Camera and
    /// target properties remain authoritative; controller and spline state is
    /// joined here solely to expose one simulation-owned telemetry boundary.
    pub fn base_cam_snapshot(&self) -> Option<BaseCamSnapshot> {
        let level_info_class = self.world.arena.find_by_path("Engine.LevelInfo")?;
        let level_info = self.level.actors.iter().copied().find(|actor| {
            self.world
                .arena
                .get(*actor)
                .ok()
                .and_then(|object| object.class_id)
                .is_some_and(|class| {
                    self.world
                        .arena
                        .class_is_a(class, level_info_class)
                        .unwrap_or(false)
                })
        })?;
        let harry = self.actor_object_property(level_info, "PlayerHarryActor")?;
        let actor = self.actor_object_property(harry, "cam")?;
        let value = |subject: ObjectId, property: &str| {
            self.world
                .arena
                .names
                .find_index(property)
                .and_then(|name| self.actor_effective_value(subject, name).ok().flatten())
        };
        let number = |subject, property| match value(subject, property) {
            Some(PropValue::Byte(value)) => Some(i32::from(value)),
            Some(PropValue::Int(value)) => Some(value),
            _ => None,
        };
        let float = |subject, property| match value(subject, property) {
            Some(PropValue::Byte(value)) => Some(f32::from(value)),
            Some(PropValue::Int(value)) => Some(value as f32),
            Some(PropValue::Float(value)) => Some(value),
            _ => None,
        };
        let boolean = |subject, property| match value(subject, property) {
            Some(PropValue::Bool(value)) => Some(value),
            Some(PropValue::Byte(value)) => Some(value != 0),
            _ => None,
        };
        let object = |subject, property| match value(subject, property) {
            Some(PropValue::Object(Some(value))) if value >= 0 => {
                Some(ObjectId(value as u32))
            }
            _ => None,
        };
        let struct_number = |fields: &[(u32, PropValue)], wanted: &str| {
            fields.iter().find_map(|(name, value)| {
                self.world
                    .arena
                    .names
                    .text(*name)
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                    .then(|| match value {
                        PropValue::Byte(value) => Some(f32::from(*value)),
                        PropValue::Int(value) => Some(*value as f32),
                        PropValue::Float(value) => Some(*value),
                        _ => None,
                    })
                    .flatten()
            })
        };
        let vector = |subject, property| match value(subject, property) {
            Some(PropValue::Struct { fields, .. }) => Some([
                struct_number(&fields, "X")?,
                struct_number(&fields, "Y")?,
                struct_number(&fields, "Z")?,
            ]),
            _ => None,
        };
        let plane = |subject, property| match value(subject, property) {
            Some(PropValue::Struct { fields, .. }) => Some([
                struct_number(&fields, "X")?,
                struct_number(&fields, "Y")?,
                struct_number(&fields, "Z")?,
                struct_number(&fields, "W")?,
            ]),
            _ => None,
        };
        let rotator = |subject, property| match value(subject, property) {
            Some(PropValue::Struct { fields, .. }) => {
                let component = |wanted: &str| {
                    struct_number(&fields, wanted).map(|value| value as i32)
                };
                Some([
                    component("Pitch")?,
                    component("Yaw")?,
                    component("Roll")?,
                ])
            }
            _ => None,
        };
        let fly_to = |subject| {
            let controller = object(subject, "TickParent");
            BaseCamFlyToSnapshot {
                actor: subject,
                elapsed_seconds: float(subject, "fFlyToTime"),
                duration_seconds: float(subject, "fFlyToTimeSpan"),
                start: vector(subject, "vFlyToStart"),
                controller,
                controller_enabled: controller
                    .and_then(|controller| boolean(controller, "bEnabled")),
                destination: vector(subject, "vFlyToDest"),
                destination_actor: object(subject, "aFlyToActor"),
                destination_offset: vector(subject, "vFlyToDestOffset"),
                fixed_to_destination_actor: boolean(subject, "bFlyToFixedToDestActor"),
                stay_locked_to_destination_actor: boolean(subject, "bFlyToStayLockedToActor"),
            }
        };
        let target = object(actor, "CamTarget");
        let target_state = target.map(|target| BaseCamTargetSnapshot {
            actor: target,
            attached_to: object(target, "aAttachedTo"),
            // BaseCamTarget declares neither field in defaultproperties:
            // UE1 initializes the vector to zero and the flag to false.
            offset: Some(vector(target, "vOffset").unwrap_or([0.0; 3])),
            relative: Some(boolean(target, "bRelative").unwrap_or(false)),
        });
        let settings = match value(actor, "CurrentSet") {
            Some(PropValue::Struct { fields, .. }) => {
                let field_value = |wanted: &str| {
                    fields.iter().find_map(|(name, value)| {
                        self.world
                            .arena
                            .names
                            .text(*name)
                            .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                            .then_some(value)
                    })
                };
                let field_vector = |wanted: &str| match field_value(wanted) {
                    Some(PropValue::Struct { fields, .. }) => Some([
                        struct_number(fields, "X")?,
                        struct_number(fields, "Y")?,
                        struct_number(fields, "Z")?,
                    ]),
                    _ => None,
                };
                Some(BaseCamSettingsSnapshot {
                    look_at_offset: field_vector("vLookAtOffset"),
                    look_at_distance: struct_number(&fields, "fLookAtDistance"),
                    rotation_tightness: struct_number(&fields, "fRotTightness"),
                    unavailable_reason_code: self
                        .deferral_reason_codes
                        .contains("vm.lvalue_unsupported")
                        .then_some("base_cam.locked_settings_lvalue_unavailable"),
                    rotation_speed: struct_number(&fields, "fRotSpeed"),
                    movement_tightness: struct_number(&fields, "fMoveTightness"),
                    movement_speed: struct_number(&fields, "fMoveSpeed"),
                })
            }
            _ => None,
        };
        let mut follow_splines = self
            .spline_motions
            .values()
            .filter(|motion| motion.actor == actor || target == Some(motion.actor))
            .map(|motion| BaseCamSplineSnapshot {
                actor: motion.actor,
                elapsed_seconds: motion.elapsed,
                duration_seconds: motion.duration,
                cue: motion.cue.clone(),
                aligns_to_spline: motion.align,
            })
            .collect::<Vec<_>>();
        follow_splines.sort_by_key(|motion| motion.actor);
        let controller_actors = |class_path: &str| {
            let Some(controller_class) = self.world.arena.find_by_path(class_path) else {
                return Vec::new();
            };
            self.level
                .actors
                .iter()
                .copied()
                .filter(|candidate| !self.destroyed_actors.contains(candidate))
                .filter(|candidate| {
                    self.world
                        .arena
                        .get(*candidate)
                        .ok()
                        .and_then(|object| object.class_id)
                        .is_some_and(|class| {
                            self.world
                                .arena
                                .class_is_a(class, controller_class)
                                .unwrap_or(false)
                        })
                })
                .collect::<Vec<_>>()
        };
        let fov_intervals = BaseCamEffectIntervals::Available(
            controller_actors("HGame.FOVController")
                .into_iter()
                .map(|controller| {
                    let values = match (
                        float(controller, "FOVEnd"),
                        float(controller, "FOVStart"),
                    ) {
                        (Some(end), Some(start)) => Some([end, start, 0.0, 0.0]),
                        _ => None,
                    };
                    BaseCamControllerInterval {
                        controller,
                        elapsed_seconds: float(controller, "CurTime"),
                        duration_seconds: float(controller, "FOVTime"),
                        values,
                    }
                })
                .collect(),
        );
        let mut flash_intervals = Vec::new();
        let mut fade_intervals = Vec::new();
        for controller in controller_actors("HGame.FadeViewController") {
            let interval = BaseCamControllerInterval {
                controller,
                elapsed_seconds: float(controller, "CurTime"),
                duration_seconds: float(controller, "FadeTime"),
                values: plane(controller, "FadeEnd"),
            };
            if boolean(controller, "bFadeFlash") == Some(true) {
                flash_intervals.push(interval);
            } else {
                fade_intervals.push(interval);
            }
        }
        let locked_to_target = boolean(actor, "bSyncPositionWithTarget");
        Some(BaseCamSnapshot {
            actor,
            camera_mode: number(actor, "CameraMode"),
            transition_mode: number(actor, "CameraModeTransition"),
            target,
            cut_notify_actor: object(actor, "CutNotifyActor"),
            sync_position_with_target: locked_to_target,
            sync_rotation_with_target: boolean(actor, "bSyncRotationWithTarget"),
            ignore_target: boolean(actor, "bIgnoreTarget"),
            current_position: vector(actor, "vCurrPosition"),
            destination_position: vector(actor, "vDestPosition"),
            current_rotation: rotator(actor, "rCurrRotation"),
            destination_rotation: rotator(actor, "rDestRotation"),
            locked_to_target,
            target_state,
            settings,
            camera_fly_to: fly_to(actor),
            target_fly_to: target.map(fly_to),
            follow_splines,
            fov_intervals,
            shake_intervals: self
                .base_cam_shake
                .iter()
                .map(|shake| BaseCamShakeInterval {
                    magnitude: shake.roll_magnitude,
                    max_shake: shake.max_shake,
                    duration_seconds: shake.duration_seconds,
                    started_at_seconds: shake.started_at_seconds,
                    ends_at_seconds: shake.ends_at_seconds,
                    elapsed_seconds: shake.elapsed_seconds,
                    progress: shake.progress,
                    rotation_delta: shake.rotation_delta,
                })
                .collect(),
            flash_intervals: BaseCamEffectIntervals::Available(flash_intervals),
            fade_intervals: BaseCamEffectIntervals::Available(fade_intervals),
        })
    }
    /// Runtime state label for an active actor, if that actor currently owns
    /// a state frame. This is diagnostics-only and never drives scheduling.
    pub fn actor_state_name(&self, actor: ObjectId) -> Option<String> {
        let state = self.active_state_name(actor)?;
        self.world.arena.names.text(state).map(str::to_string)
    }


    /// Active actor poses in stable arena identity order. The level actor
    /// list is the sole source; deleted or non-actor exports never appear.
    pub fn actor_transform_snapshots(&self) -> BTreeMap<ObjectId, ActorTransformSnapshot> {
        let hidden_name = self.world.arena.names.find_index("bHidden");
        self.level
            .actors
            .iter()
            .copied()
            .filter_map(|actor| {
                let location = self.actor_location(actor)?;
                let rotation = self.actor_rotation(actor)?;
                let hidden = hidden_name.is_some_and(|name| {
                    matches!(
                        self.actor_effective_value(actor, name),
                        Ok(Some(PropValue::Bool(true)))
                    )
                });
                Some((
                    actor,
                    ActorTransformSnapshot {
                        actor,
                        location,
                        rotation,
                        hidden,
                    },
                ))
            })
            .collect()
    }


    /// Absolute simulation time since process bootstrap. Rendering reads this
    /// clock but never advances it.
    pub fn simulation_time(&self) -> f64 {
        self.sim_time
    }

    /// Stable actor-keyed animation state for the renderer and traces.
    pub fn actor_animation_snapshots(&self) -> Result<BTreeMap<ObjectId, ActorAnimationSnapshot>> {
        let mut snapshots = BTreeMap::new();
        for &actor in &self.level.actors {
            let Some(animation) = self.actor_animations.get(&actor) else {
                continue;
            };
            let mesh_path = animation.mesh_path.clone();
            let animation_path = animation.animation_path.clone();
            let sequence = self
                .world
                .arena
                .names
                .text(animation.sequence.index)
                .unwrap_or("None")
                .to_string();
            snapshots.insert(
                actor,
                ActorAnimationSnapshot {
                    actor,
                    mesh_path,
                    animation_path,
                    sequence,
                    started_at: animation.started_at,
                    elapsed_seconds: animation.elapsed_seconds,
                    duration_seconds: animation.duration_seconds,
                    finish_after_seconds: animation.finish_after_seconds,
                    rate: animation.rate,
                    looped: animation.looped,
                    tweening: animation.tweening,
                },
            );
        }
        Ok(snapshots)
    }

    fn begin_base_cam_shake(
        &mut self,
        roll_magnitude: f32,
        max_shake: f32,
        duration_seconds: f32,
    ) {
        let duration_seconds = duration_seconds.max(0.0);
        if duration_seconds == 0.0 {
            self.base_cam_shake = None;
            return;
        }
        self.base_cam_shake = Some(BaseCamShakeState {
            roll_magnitude,
            max_shake,
            duration_seconds,
            started_at_seconds: self.sim_time,
            ends_at_seconds: self.sim_time + f64::from(duration_seconds),
            elapsed_seconds: 0.0,
            progress: 0.0,
            rotation_delta: [0; 3],
        });
    }

    fn advance_base_cam_shake(&mut self) {
        let Some(shake) = self.base_cam_shake.as_mut() else {
            return;
        };
        shake.elapsed_seconds =
            (self.sim_time - shake.started_at_seconds).max(0.0) as f32;
        if shake.elapsed_seconds >= shake.duration_seconds {
            self.base_cam_shake = None;
            return;
        }
        shake.progress = (shake.elapsed_seconds / shake.duration_seconds).clamp(0.0, 1.0);
        // PlayerPawn.ViewShake samples yaw, pitch, then roll from the one
        // authoritative stream; rotators store components as pitch/yaw/roll.
        let envelope = 1.0 - shake.progress;
        let yaw = sim_rand_range(&mut self.rng, -shake.max_shake, shake.max_shake) * envelope;
        let pitch = sim_rand_range(&mut self.rng, -shake.max_shake, shake.max_shake) * envelope;
        let roll =
            sim_rand_range(&mut self.rng, -shake.roll_magnitude, shake.roll_magnitude) * envelope;
        shake.rotation_delta = [pitch as i32, yaw as i32, roll as i32];
    }

    /// Observe the real `PlayerPawn.ShakeView` property transaction. The
    /// script body writes `ShakeMag`, `ShakeTimer`, `MaxShake`, `VertTimer`,
    /// and `ShakeVert` in one activation; `ShakeTimer` is the unambiguous
    /// start edge and carries the authored duration after argument parsing.
    fn begin_shake_view_writes(&mut self, writes: &[hp_uobject::vm::InstanceWrite]) {
        let number = |value: &PropValue| match value {
            PropValue::Byte(value) => Some(f32::from(*value)),
            PropValue::Int(value) => Some(*value as f32),
            PropValue::Float(value) => Some(*value),
            _ => None,
        };
        let property_is = |name: u32, wanted: &str| {
            self.world
                .arena
                .names
                .text(name)
                .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
        };
        let Some((player, duration)) = writes.iter().find_map(|write| {
            property_is(write.name, "ShakeTimer")
                .then(|| number(&write.value).map(|value| (write.object, value)))
                .flatten()
        }) else {
            return;
        };
        let Some(player_class) = self.world.arena.find_by_path("Engine.PlayerPawn") else {
            return;
        };
        let is_player = self
            .world
            .arena
            .get(player)
            .ok()
            .and_then(|object| object.class_id)
            .is_some_and(|class| {
                self.world
                    .arena
                    .class_is_a(class, player_class)
                    .unwrap_or(false)
            });
        if !is_player {
            return;
        }
        let roll_magnitude = writes
            .iter()
            .find(|write| write.object == player && property_is(write.name, "ShakeMag"))
            .and_then(|write| number(&write.value))
            .unwrap_or(0.0);
        let max_shake = writes
            .iter()
            .find(|write| write.object == player && property_is(write.name, "MaxShake"))
            .and_then(|write| number(&write.value))
            .unwrap_or(0.0);
        self.begin_base_cam_shake(roll_magnitude, max_shake, duration);
    }

    /// One variable-delta tick: dispatch queued input, advance simulated
    /// time, fire due timers, resume parked actor frames, and run actor
    /// `Tick` scripts.
    ///
    /// Tick order: input dispatch → PAWN PHYSICS (in [`Engine::pawn_step`],
    /// before this method) → rng → TIMER events due this tick → resume
    /// active actor frames → actor Tick events → counters. State code and
    /// the active state's `Tick` probe are independent UE1 execution paths:
    /// processing (or parking) the former must not suppress the latter.
    pub fn tick(&mut self, pending_input: &[(u8, u8, f32)]) -> Result<()> {
        let dt = self.next_delta();
        if self.audio.is_some()
            && let Some(host) = self.audio.as_mut()
        {
            host.pump();
        }
        for &(key, act, delta) in pending_input {
            self.dispatch_input(key, act, delta);
        }
        let _ = self.rng.next_f32(); // gameplay randomness consumes the stream in fixed order
        self.sim_time += f64::from(dt);
        self.advance_base_cam_shake();
        for animation in self.actor_animations.values_mut() {
            animation.elapsed_seconds += dt;
        }
        self.advance_splines(dt)?;
        self.fire_due_timers()?;
        self.resume_actor_frames()?;
        self.run_actor_event_with_delta("Tick", dt)?;
        self.counters.ticks += 1;
        Ok(())
    }

    /// Run a newly spawned actor through UE1's source-ordered lifecycle
    /// before `Spawn` returns to script. This ordering matters: the caller
    /// may immediately set fields and call a method which enters a state.
    /// Delaying `SetInitialState` until the next engine tick would then
    /// incorrectly overwrite that explicit transition.
    fn run_spawn_startup(&mut self, actor: ObjectId) -> Result<()> {
        for event in [
            "PreBeginPlay",
            "BeginPlay",
            "PostBeginPlay",
            "SetInitialState",
        ] {
            if self.destroyed_actors.contains(&actor)
                || !self.level.actors.contains(&actor)
                || self.actor_script_initialized(actor)?
            {
                break;
            }
            self.run_named_event_on(actor, event, Some(0.0))?;
        }
        Ok(())
    }
    /// Fire every timer whose deadline has passed, ordered by
    /// `(deadline, actor id)` for determinism. Each timer fires at most
    /// once per tick; repeating entries re-arm from their scheduled
    /// instant (`AActor::SetTimer` semantics).
    fn fire_due_timers(&mut self) -> Result<()> {
        let now = self.sim_time;
        let mut due: Vec<TimerEntry> = Vec::new();
        let mut keep: Vec<TimerEntry> = Vec::new();
        for entry in std::mem::take(&mut self.timers) {
            if entry.at <= now {
                due.push(entry);
            } else {
                keep.push(entry);
            }
        }
        self.timers = keep;
        due.sort_by(|a, b| {
            a.at.partial_cmp(&b.at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.actor.cmp(&b.actor))
        });
        for entry in due {
            if entry.repeating && entry.interval > 0.0 {
                self.timers.push(TimerEntry {
                    at: entry.at + f64::from(entry.interval),
                    ..entry
                });
            }
            self.counters.timers_fired += 1;
            // The fired event runs like any explicit event; if IT sleeps,
            // the frame parks under origin "Timer" and further Timer
            // dispatches for this actor hold until it resumes.
            self.run_named_event_on(entry.actor, "Timer", None)?;
        }
        Ok(())
    }

    /// Resume every parked frame whose wake time or condition has arrived, in
    /// `(wake time, actor id)` order — the scheduler's determinism contract.
    /// `Wake::Interpolation` is evaluated once per scheduler pass; `Never`
    /// frames stay parked until something external clears them.
    fn resume_actor_frames(&mut self) -> Result<()> {
        let now = self.sim_time;
        let mut due = Vec::new();
        for (&actor, script) in &self.actor_scripts {
            let wake = match script.wake {
                Wake::At(t) if t <= now => Some(t),
                Wake::At(_) | Wake::Never => None,
                Wake::Ready => Some(now),
                Wake::Interpolation if !self.actor_is_interpolating(actor)? => Some(now),
                Wake::Interpolation => None,
            };
            if let Some(wake) = wake {
                due.push((wake, actor));
            }
        }
        due.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        for (_, actor) in due {
            // Removed/replace while an earlier resume ran? Then the
            // continuation is gone and this entry is stale.
            let Some(script) = self.actor_scripts.remove(&actor) else {
                continue;
            };
            self.run_parked_frame(actor, script)?;
        }
        Ok(())
    }

    /// `CauseInputEvent → UEngine::InputEvent`: route one normalized key
    /// event through the config binding chain into console commands.
    pub fn dispatch_input(&mut self, key: u8, act: u8, delta: f32) {
        self.counters.input_events += 1;
        if act == action::NONE {
            return;
        }
        let Some(name) = binding_key_name(key) else {
            return;
        };
        let Some(raw) = self.ini.get("Engine.Input", &name) else {
            return;
        };
        let raw = raw.to_string();
        let segments: Vec<String> = self
            .bindings
            .pipeline_for(&raw)
            .into_iter()
            .flat_map(|segment| expand_segment(&self.bindings, segment))
            .collect();
        let mut handled = false;
        for command in segments {
            if matches!(
                self.exec_console(&command),
                Ok(crate::console::ConsoleOutcome::Handled)
            ) {
                handled = true;
            }
        }
        if handled {
            self.counters.input_handled += 1;
        }
        let _ = delta; // axis deltas reach bound Axis cues verbatim
    }

    /// M4 — the console execution path. Returns whether the command was
    /// recognized. Unknown commands are `Unhandled` (the exec-chain
    /// contract), never silent successes.
    pub fn exec_console(&mut self, command: &str) -> Result<crate::console::ConsoleOutcome> {
        use crate::console::ConsoleOutcome;
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Ok(ConsoleOutcome::Unhandled);
        }
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((verb, rest)) => (verb, rest.trim()),
            None => (trimmed, ""),
        };
        self.counters.console_commands += 1;
        match verb.to_ascii_lowercase().as_str() {
            "quit" | "exit" => {
                self.quit_requested = true;
                Ok(ConsoleOutcome::Handled)
            }
            "flush" => Ok(ConsoleOutcome::Handled),
            "flyto" | "cammove" | "moveto" | "lookat" => {
                self.cues
                    .push(format!("{verb} {rest}").trim_end().to_string());
                Ok(ConsoleOutcome::Handled)
            }
            "get" => Ok(ConsoleOutcome::Handled),
            "set" => {
                let _ = rest; // headless store keeps no user vars yet
                Ok(ConsoleOutcome::Handled)
            }
            _ if self.sounds_enabled
                && matches!(
                    verb.to_ascii_lowercase().as_str(),
                    "playsound" | "startsound" | "stopsound" | "stopsoundmap"
                ) =>
            {
                // Interactive-only routing: with sounds disabled (headless)
                // these fall through to `Unhandled`, keeping the
                // deterministic output byte-stable.
                let cue = format!("{verb} {rest}").trim_end().to_string();
                self.play_sound_cue(&cue);
                Ok(ConsoleOutcome::Handled)
            }
            _ => Ok(ConsoleOutcome::Unhandled),
        }
    }

    /// Interactive-mode sound cue hook. Script-driven cues route here when
    /// sounds are enabled: the cue name is recorded in
    /// [`Engine::sound_cues`] and logged with an `[audio.cue]` marker so it
    /// is observable on stdout/stderr and to whatever sink the host wired
    /// into the audio graph. With sounds disabled (headless default,
    /// `-nosound`) this only records — no output, no side effects.
    pub fn play_sound_cue(&mut self, name: &str) {
        self.sound_cues.push(name.to_string());
        if self.sounds_enabled {
            println!("hp-engine: [audio.cue] {name}");
        }
    }

    fn run_actor_event_with_delta(&mut self, event: &str, dt: f32) -> Result<()> {
        self.run_actor_event_on(event, dt)
    }

    /// Run one named script event on every active loaded actor, in export
    /// order. Deliberately unsupported VM constructs use the reason-coded
    /// deferral policy; malformed code aborts.
    fn run_actor_event_on(&mut self, event: &str, delta: f32) -> Result<()> {
        let actors = self.level.actors.clone();
        for &actor in &actors {
            // A script effect may have destroyed this actor in an earlier
            // lifecycle phase (or an earlier actor's event in this phase).
            // Retail nulls its level-array slot, so subsequent dispatches
            // must skip the stable arena handle too.
            if self.destroyed_actors.contains(&actor) || !self.level.actors.contains(&actor) {
                continue;
            }
            // UE1 interplay: state code and state event probes are separate
            // execution paths. Only a parked continuation of THIS event
            // suppresses re-dispatch until that continuation resumes.
            if self.frame_suppresses(actor, event) {
                continue;
            }
            self.run_named_event_on(actor, event, Some(delta))?;
        }
        Ok(())
    }

    /// True when `actor` has a parked continuation which owns `event`.
    /// State-body continuations never suppress state event probes: UE1 runs
    /// `ProcessState` and `eventTick` independently during `AActor::Tick`.
    fn frame_suppresses(&self, actor: ObjectId, event: &str) -> bool {
        self.actor_scripts
            .get(&actor)
            .is_some_and(|script| match &script.origin {
                Some(owner) => owner.eq_ignore_ascii_case(event),
                None => false,
            })
    }

    fn event_is_disabled(&self, actor: ObjectId, event: &str) -> bool {
        self.disabled_events
            .get(&actor)
            .is_some_and(|events| events.contains(&hp_uobject::vm::fold_key(event)))
    }

    /// Resolve and run one named event function on ONE actor (explicit
    /// events, `BeginPlay`, fired timers). Missing functions and empty
    /// bodies are ordinary no-ops; deferral bookkeeping follows the
    /// event path.
    fn run_named_event_on(
        &mut self,
        actor: ObjectId,
        event: &str,
        delta: Option<f32>,
    ) -> Result<()> {
        let Some(function_id) = self.resolve_actor_event_function(actor, event)? else {
            return Ok(());
        };
        self.run_function_frame(actor, function_id, event, delta, &[])
    }

    /// Resolve one actor event through the same state-first path regardless
    /// of whether the caller supplies native event arguments.
    fn resolve_actor_event_function(
        &mut self,
        actor: ObjectId,
        event: &str,
    ) -> Result<Option<ObjectId>> {
        let Some(class_id) = self.world.arena.get(actor)?.class_id else {
            return Ok(None);
        };
        if self.event_is_disabled(actor, event)
            || self.deferred_scripts.contains(&(class_id, event.to_string()))
        {
            return Ok(None);
        }
        if let Some(state) = self.actor_states.get(&actor).copied()
            && let Some(function) = self.find_state_event_function(state, event)?
        {
            return Ok(Some(function));
        }
        self.resolve_event_function(class_id, event)
    }

    /// Memoized `(class, event)` → function id resolution.
    fn resolve_event_function(
        &mut self,
        class_id: ObjectId,
        event: &str,
    ) -> Result<Option<ObjectId>> {
        let key = (class_id, event.to_string());
        if let Some(found) = self.script_lookup.get(&key) {
            return Ok(*found);
        }
        let found = self.world.arena.find_function(class_id, event)?;
        self.script_lookup.insert(key, found);
        Ok(found)
    }

    fn run_function_frame(
        &mut self,
        actor: ObjectId,
        function_id: ObjectId,
        event: &str,
        delta: Option<f32>,
        arguments: &[PropValue],
    ) -> Result<()> {
        let (code, params, resolver) = match &self.world.arena.get(function_id)?.data {
            ObjectData::Function(function) => (
                function.code.clone(),
                function.params.clone(),
                function.resolver.clone(),
            ),
            _ => return Ok(()),
        };
        if code.is_empty() {
            return Ok(());
        }
        let mut locals = PropStore::new();
        for (parameter, value) in params.iter().zip(arguments.iter()) {
            let name = self.world.arena.get(*parameter)?.name_index;
            locals.set(name, value.clone());
        }
        if event.eq_ignore_ascii_case("Trigger") && arguments.len() == 2 {
            // Engine.Actor.Trigger has the stable `(Actor Other, Pawn
            // EventInstigator)` ABI. Some package function records omit
            // parameter metadata, but their bytecode still addresses these
            // lexical locals by name.
            for (name, value) in [("Other", &arguments[0]), ("EventInstigator", &arguments[1])] {
                if let Some(index) = self.world.arena.names.find_index(name) {
                    locals.set(index, value.clone());
                }
            }
        }
        if event.eq_ignore_ascii_case("HearNoise") && arguments.len() == 2 {
            // `event HearNoise(float Loudness, Actor NoiseMaker)`: preserve
            // these lexical locals even when a package's parameter links are
            // incomplete, just as the Trigger ABI does above.
            for (name, value) in [("Loudness", &arguments[0]), ("NoiseMaker", &arguments[1])] {
                if let Some(index) = self.world.arena.names.find_index(name) {
                    locals.set(index, value.clone());
                }
            }
        }

        if arguments.is_empty()
            && let (Some(delta), Some(parameter)) = (delta, params.first())
        {
            // `event Tick(float DeltaTime)` — bind the delta by the
            // parameter's declared name.
            let param_name = self.world.arena.get(*parameter)?.name_index;
            locals.set(param_name, PropValue::Float(delta));
        }
        let Some(class_id) = self.world.arena.get(actor)?.class_id else {
            return Ok(());
        };
        let key = ScriptKey::Event(class_id, event.to_string());
        self.execute_frame(
            actor,
            code,
            resolver,
            0,
            locals,
            None,
            None,
            None,
            Vec::new(),
            None,
            Some(event.to_string()),
            key,
            event,
        )
        .map(|_| ())
    }

    /// Resume one parked continuation taken out of [`Engine::actor_scripts`]
    /// (the caller removed it, so a suspension inside can simply
    /// re-insert). Blacklisting keys off the owning state body or, for
    /// parked event continuations, the owning event.
    fn run_parked_frame(&mut self, actor: ObjectId, script: ScriptFrame) -> Result<()> {
        if script.self_id != actor {
            return Err(EngineError::new(
                "vm.frame_self_mismatch",
                format!(
                    "parked frame self {:?} does not match scheduler actor {:?}",
                    script.self_id, actor
                ),
            ));
        }
        let class_id = self.world.arena.get(actor)?.class_id;
        let (key, context) = match (script.origin.clone(), script.state, class_id) {
            (Some(origin), _, Some(class)) => (ScriptKey::Event(class, origin.clone()), origin),
            (None, Some(state), Some(class)) => {
                if self.deferred_states.contains(&(class, state)) {
                    return Ok(());
                }
                let context = self
                    .world
                    .arena
                    .display_name(state)
                    .map(|n| format!("state {n}"))
                    .unwrap_or_else(|_| format!("state {state:?}"));
                (ScriptKey::State(class, state), context)
            }
            _ => return Ok(()),
        };
        self.execute_frame(
            actor,
            script.code,
            script.resolver,
            script.pc,
            script.locals,
            script.nested_call,
            None,
            None,
            script.iterators,
            script.state,
            script.origin,
            key,
            &context,
        )
        .map(|_| ())
    }

    /// The core frame runner every script path funnels through. Runs one
    /// activation from `pc` with `locals`, then lands its effects:
    /// queued instance writes applied to the arena, scheduler requests
    /// (`SetTimer`/`GotoState`) drained, and a latent suspension parked
    /// into [`Engine::actor_scripts`] with the program cursor already
    /// PAST the suspending call — resume continues, never restarts.
    ///
    /// Deferrable failures are reported loudly once per site and
    /// blacklist the stream under `key` (an accepted reason-coded
    /// deferral). `Actor.Error` destroys only the receiving actor and
    /// discards its active frame; any other failure aborts the run. Returns
    /// `true` when the frame parked, `false` when it completed or was
    /// discarded.
    #[allow(clippy::too_many_arguments)]
    fn execute_frame(
        &mut self,
        actor: ObjectId,
        code: Vec<u8>,
        resolver: BytecodeResolver,
        pc: usize,
        locals: PropStore,
        nested_call: Option<SuspendedCall>,
        pending_expression: Option<PendingExpression>,
        injected_effect_result: Option<(u64, PropValue)>,
        iterators: Vec<IteratorCursor>,
        state_id: Option<ObjectId>,
        origin: Option<String>,
        key: ScriptKey,
        context: &str,
    ) -> Result<bool> {
        struct Outcome {
            pc: usize,
            locals: PropStore,
            nested_call: Option<SuspendedCall>,
            pending_expression: Option<PendingExpression>,
            iterators: Vec<IteratorCursor>,
            writes: Vec<hp_uobject::vm::InstanceWrite>,
            effects: Vec<ScriptEffect>,
            requests: Vec<LatentRequest>,
            suspend: Option<Suspend>,
            next_effect_request_id: u64,
        }
        let mut pc = pc;
        let mut locals = locals;
        let mut nested_call = nested_call;
        let mut pending_expression = pending_expression;
        let mut injected_effect_result = injected_effect_result;
        let mut iterators = iterators;
        loop {
            let active_state = self.active_state_name(actor);
            let active_state_object = state_id.or(self.actor_states.get(&actor).copied());
            let outcome: std::result::Result<
                Outcome,
                (hp_uobject::error::Fail, Outcome),
            > = {
                let arena = &self.world.arena;
                let registry = &self.world.registry;
                let mut frame =
                    Frame::with_resolver(arena, registry, &code, Some(actor), resolver.clone());
                frame.set_pc(pc);
                frame.set_actor_scope(&self.level.actors);
                frame.set_next_effect_request_id(self.next_script_request_id);
                frame.locals = std::mem::take(&mut locals);
                frame.restore_iterators(std::mem::take(&mut iterators));
                if let Some(call) = nested_call.take() {
                    frame.restore_suspended_call(call);
                }
                frame.restore_pending_expression(pending_expression.take());
                if let Some((request_id, value)) = injected_effect_result.take() {
                    frame.inject_effect_result(request_id, value)?;
                }
                frame.set_active_state_context(active_state, active_state_object);
                let ran = frame.run();
                let collected = Outcome {
                    pc: frame.pc(),
                    locals: std::mem::take(&mut frame.locals),
                    nested_call: frame.take_suspended_call(),
                    pending_expression: frame.take_pending_expression(),
                    writes: frame.take_instance_writes(),
                    effects: frame.take_effects(),
                    iterators: frame.take_iterators(),
                    requests: frame.take_latent_requests(),
                    suspend: frame.take_suspend(),
                    next_effect_request_id: frame.next_effect_request_id(),
                };
                match ran {
                    Ok(_) => Ok(collected),
                    Err(fail) => Err((fail, collected)),
                }
            };
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err((fail, partial)) => {
                    if Self::is_deferred_reason(fail.reason_code) {
                        self.next_script_request_id = partial.next_effect_request_id;
                        // A deferred native is a harness limitation, not an
                        // UnrealScript transaction rollback. Preserve writes
                        // and effects executed before the unsupported point;
                        // BaseCam.Locked, for example, sets its sync flags
                        // before its nested processor reaches such a point.
                        self.begin_shake_view_writes(&partial.writes);
                        for write in partial.writes {
                            if let Ok(object) = self.world.arena.get_mut(write.object)
                                && let Some(store) = object.properties_mut()
                            {
                                store.set(write.name, write.value);
                            }
                        }
                        self.apply_script_effects(partial.effects)?;
                        for request in partial.requests {
                            match request {
                                LatentRequest::SetTimer {
                                    actor: target,
                                    seconds,
                                    repeating,
                                } => self.schedule_timer(target, seconds, repeating),
                                LatentRequest::SetEventEnabled {
                                    actor: target,
                                    event,
                                    enabled,
                                } => {
                                    let event = hp_uobject::vm::fold_key(&event);
                                    if enabled {
                                        if let Some(events) = self.disabled_events.get_mut(&target) {
                                            events.remove(&event);
                                            if events.is_empty() {
                                                self.disabled_events.remove(&target);
                                            }
                                        }
                                    } else {
                                        self.disabled_events
                                            .entry(target)
                                            .or_default()
                                            .insert(event);
                                    }
                                }
                                LatentRequest::GotoState {
                                    actor: target,
                                    change,
                                } => self.enter_state(target, change)?,
                            }
                        }
                        let reason = fail.reason_code;
                        let error = EngineError::new(reason, fail.message.clone()).annotate(
                            actor,
                            &self.world.arena,
                            context,
                        );
                        let tag = if matches!(key, ScriptKey::State(..)) {
                            "engine.script_state_deferred"
                        } else {
                            "engine.script_event_deferred"
                        };
                        self.deferral_reason_codes.insert(reason);
                        if reason == "native.body_deferred" {
                            *self
                                .native_deferral_census
                                .entry(fail.message.clone())
                                .or_insert(0) += 1;
                        }
                        if self
                            .deferral_reported
                            .insert((actor, error.message.clone()))
                        {
                            eprintln!("hp-engine: [{tag}] {error}");
                        }
                        match key {
                            ScriptKey::Event(class, event) => {
                                self.deferred_scripts.insert((class, event));
                            }
                            ScriptKey::State(class, state) => {
                                // Kill any parked continuation of the dead body.
                                self.deferred_states.insert((class, state));
                                self.actor_scripts.remove(&actor);
                            }
                        }
                        self.counters.script_deferrals += 1;
                        return Ok(false);
                    }
                    if fail.reason_code == "engine.actor_error" {
                        self.destroy_script_actor(actor);
                        return Ok(false);
                    }
                    return Err(EngineError::from(fail).annotate(
                        actor,
                        &self.world.arena,
                        context,
                    ));
                }
            };
            self.next_script_request_id = outcome.next_effect_request_id;
            if let Some(Suspend::EffectResult { request_id }) = outcome.suspend {
                // Preserve S1's writes-before-effects landing order. The
                // suspending Spawn expression has not yet continued, so these
                // writes can only precede the request.
                self.begin_shake_view_writes(&outcome.writes);
                for write in outcome.writes {
                    if let Ok(object) = self.world.arena.get_mut(write.object)
                        && let Some(store) = object.properties_mut()
                    {
                        store.set(write.name, write.value);
                    }
                }
                // Apply effects at their exact queue positions and resolve the
                // one effect whose scalar/object result this activation awaits.
                let mut result = None;
                for effect in outcome.effects {
                    let resolved = match effect {
                        ScriptEffect::SpawnRequest {
                            request_id: effect_id,
                            class,
                            owner,
                            location,
                            rotation,
                            ..
                        } if effect_id == request_id => Some(PropValue::Object(Some(
                            self.spawn_script_actor(class, owner, location, rotation)?.0 as i32,
                        ))),
                        ScriptEffect::SkeletalQuery {
                            request_id: effect_id,
                            actor,
                            query,
                        } if effect_id == request_id => {
                            Some(self.resolve_skeletal_query(actor, query)?)
                        }
                        ScriptEffect::TraceActorsRequest {
                            request_id: effect_id,
                            source,
                            base_class,
                            start,
                            end,
                            extent,
                        } if effect_id == request_id => {
                            Some(self.resolve_trace_actors(source, base_class, start, end, extent, true)?)
                        }
                        ScriptEffect::TraceRequest {
                            request_id: effect_id,
                            source,
                            start,
                            end,
                            trace_actors,
                            extent,
                        } if effect_id == request_id => {
                            Some(self.resolve_trace(source, start, end, trace_actors, extent)?)
                        }
                        ScriptEffect::FastTraceRequest {
                            request_id: effect_id,
                            source,
                            start,
                            end,
                        } if effect_id == request_id => {
                            Some(self.resolve_fast_trace(source, start, end)?)
                        }
                        ScriptEffect::LocalizeRequest {
                            request_id: effect_id,
                            section,
                            key,
                            package,
                        } if effect_id == request_id => Some(PropValue::Str(
                            self.resolve_localize(&section, &key, &package),
                        )),
                        ScriptEffect::CreateAnimChannelRequest {
                            request_id: effect_id,
                            reserved,
                            actor,
                            class,
                            anim_type,
                            root_bone,
                            transient,
                            not_replaceable,
                        } if effect_id == request_id => {
                            let channel = self.create_anim_channel(
                                reserved,
                                actor,
                                class,
                                anim_type,
                                root_bone,
                                transient,
                                not_replaceable,
                            )?;
                            Some(PropValue::Object(channel.map(|id| id.0 as i32)))
                        }
                        other => {
                            self.apply_script_effects(vec![other])?;
                            None
                        }
                    };
                    if let Some(value) = resolved {
                        if result.replace(value).is_some() {
                            return Err(EngineError::new(
                                "native.effect_result_duplicate",
                                format!("duplicate effect result for request {request_id}"),
                            ));
                        }
                    }
                }
                let result = result.ok_or_else(|| {
                    EngineError::new(
                        "native.effect_result_missing",
                        format!("effect-result suspension {request_id} has no matching request"),
                    )
                })?;
                for request in outcome.requests {
                    match request {
                        LatentRequest::SetTimer {
                            actor: target,
                            seconds,
                            repeating,
                        } => self.schedule_timer(target, seconds, repeating),
                        LatentRequest::SetEventEnabled {
                            actor: target,
                            event,
                            enabled,
                        } => {
                            let event = hp_uobject::vm::fold_key(&event);
                            if enabled {
                                if let Some(events) = self.disabled_events.get_mut(&target) {
                                    events.remove(&event);
                                    if events.is_empty() {
                                        self.disabled_events.remove(&target);
                                    }
                                }
                            } else {
                                self.disabled_events
                                    .entry(target)
                                    .or_default()
                                    .insert(event);
                            }
                        }
                        LatentRequest::GotoState {
                            actor: target,
                            change,
                        } => {
                            if target == actor {
                                return Err(EngineError::new(
                                    "vm.effect_result_state_change_conflict",
                                    "effect-result slice also changed its own state",
                                ));
                            }
                            self.enter_state(target, change)?;
                        }
                    }
                }
                pc = outcome.pc;
                locals = outcome.locals;
                nested_call = outcome.nested_call;
                pending_expression = outcome.pending_expression;
                injected_effect_result = Some((request_id, result));
                iterators = outcome.iterators;
                continue;
            }
            // Instance/default writes only exist once applied (S1 contract).
            self.begin_shake_view_writes(&outcome.writes);
            for write in outcome.writes {
                if let Ok(object) = self.world.arena.get_mut(write.object)
                    && let Some(store) = object.properties_mut()
                {
                    store.set(write.name, write.value);
                }
            }
            self.apply_script_effects(outcome.effects)?;
            let mut goto_states = Vec::new();
            for request in outcome.requests {
                match request {
                    LatentRequest::SetTimer {
                        actor: target,
                        seconds,
                        repeating,
                    } => self.schedule_timer(target, seconds, repeating),
                    LatentRequest::GotoState {
                        actor: target,
                        change,
                    } => goto_states.push((target, change)),
                    LatentRequest::SetEventEnabled {
                        actor: target,
                        event,
                        enabled,
                    } => {
                        let event = hp_uobject::vm::fold_key(&event);
                        if enabled {
                            if let Some(events) = self.disabled_events.get_mut(&target) {
                                events.remove(&event);
                                if events.is_empty() {
                                    self.disabled_events.remove(&target);
                                }
                            }
                        } else {
                            self.disabled_events
                                .entry(target)
                                .or_default()
                                .insert(event);
                        }
                    }
                }
            }
            let mut changed_current_actor = false;
            for (target, change) in goto_states {
                self.enter_state(target, change)?;
                changed_current_actor |= target == actor;
            }
            if changed_current_actor {
                return Ok(false);
            }
            if outcome.suspend == Some(Suspend::StateChange) {
                pc = outcome.pc;
                locals = outcome.locals;
                nested_call = outcome.nested_call;
                pending_expression = outcome.pending_expression;
                iterators = outcome.iterators;
                continue;
            }
            if let Some(suspend) = outcome.suspend {
                let wake = match suspend {
                    Suspend::Sleep(seconds) => {
                        self.counters.latent_sleeps += 1;
                        Wake::At(self.sim_time + seconds)
                    }
                    Suspend::Anim => {
                        self.counters.latent_finish_anims += 1;
                        let delay = self
                            .actor_animations
                            .get(&actor)
                            .and_then(animation_finish_delay);
                        let seconds = delay.unwrap_or_else(|| {
                            if self.finish_animation_fallback_reported.insert(actor) {
                                eprintln!(
                                    "hp-engine: note [engine.finish_anim_timing_unavailable] actor {actor:?}: source animation duration unavailable; using explicit {FINISH_ANIM_PLACEHOLDER_SECS:.3}s fallback"
                                );
                            }
                            FINISH_ANIM_PLACEHOLDER_SECS
                        });
                        Wake::At(self.sim_time + seconds)
                    }
                    Suspend::Interpolation => Wake::Interpolation,
                    Suspend::StateChange => {
                        return Err(EngineError::new(
                            "vm.state_change_request_missing",
                            "GotoState suspended without queuing a state request",
                        ));
                    }
                    Suspend::EffectResult { request_id } => {
                        return Err(EngineError::new(
                            "native.spawn_request_missing",
                            format!("unconsumed effect result request {request_id}"),
                        ));
                    }
                };
                self.actor_scripts.insert(
                    actor,
                    ScriptFrame {
                        code,
                        pc: outcome.pc,
                        self_id: actor,
                        state: state_id,
                        locals: outcome.locals,
                        nested_call: outcome.nested_call,
                        iterators: outcome.iterators,
                        wake,
                        resolver,
                        origin,
                    },
                );
                return Ok(true);
            }
            return Ok(false);
        }
    }
    fn trace_collision_polys(&self) -> Vec<CollisionPoly> {
        #[cfg(test)]
        if let Some(polys) = &self.test_trace_collision_polys {
            return polys.clone();
        }
        crate::scene::collect_collision_polys(&self.world.arena, &self.level)
    }

    #[cfg(test)]
    fn set_test_trace_collision_polys(&mut self, polys: Vec<CollisionPoly>) {
        self.test_trace_collision_polys = Some(polys);
    }

    fn resolve_trace_actors(
        &self,
        source: ObjectId,
        base_class: Option<ObjectId>,
        start: [f32; 3],
        end: [f32; 3],
        extent: [f32; 3],
        include_actors: bool,
    ) -> Result<PropValue> {
        let arena = &self.world.arena;
        let actor_class = arena.find_by_path("Engine.Actor").ok_or_else(|| {
            EngineError::new(
                "native.trace_collision_unavailable",
                "Engine.Actor is unavailable for TraceActors",
            )
        })?;
        let base_class = base_class.unwrap_or(actor_class);
        if !matches!(arena.get(base_class)?.data, ObjectData::Class(_))
            || !arena.class_is_a(base_class, actor_class)?
        {
            return Err(EngineError::new(
                "native.trace_collision_unavailable",
                format!(
                    "TraceActors source {source:?} has invalid BaseClass {base_class:?} for range {start:?}->{end:?}"
                ),
            ));
        }

        let level_info_class = arena.find_by_path("Engine.LevelInfo");
        let level_actor = level_info_class.and_then(|class| {
            self.level.actors.iter().copied().find(|actor| {
                arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .is_some_and(|actual| arena.class_is_a(actual, class).unwrap_or(false))
            })
        });
        let polys = self.trace_collision_polys();
        if polys.is_empty() {
            return Err(EngineError::new(
                "native.trace_collision_unavailable",
                format!(
                    "TraceActors source {source:?} has no level collision geometry for range {start:?}->{end:?}"
                ),
            ));
        }

        let world_hit = trace_level_polys(&polys, start, end, extent);
        let world_limit = world_hit.map_or(1.0, |(time, _, _)| time);
        let collide_name = arena.names.find_index("bCollideActors");
        let mut hits = Vec::new();
        for (order, actor) in self.level.actors.iter().copied().enumerate() {
            let Some(class) = arena.get(actor)?.class_id else {
                continue;
            };
            if !arena.class_is_a(class, base_class)? {
                continue;
            }
            if Some(actor) == level_actor {
                if let Some((time, location, normal)) = world_hit {
                    hits.push((time, order, actor, location, normal));
                }
                continue;
            }
            if !include_actors {
                continue;
            }
            let collides = match collide_name {
                Some(name) => matches!(
                    self.actor_effective_value(actor, name)?,
                    Some(PropValue::Bool(true))
                        | Some(PropValue::Byte(1..))
                        | Some(PropValue::Int(1..))
                ),
                None => false,
            };
            if !collides {
                continue;
            }
            let Some(location) = self.actor_location(actor) else {
                continue;
            };
            let radius = effective_prop(arena, &self.world, actor, "CollisionRadius")
                .and_then(prop_scalar)
                .unwrap_or(0.0);
            let half_height = effective_prop(arena, &self.world, actor, "CollisionHeight")
                .and_then(prop_scalar)
                .unwrap_or(0.0);
            if let Some((time, hit_location, normal)) =
                trace_actor_cylinder(start, end, extent, location, radius, half_height)
                && time <= world_limit
            {
                hits.push((time, order, actor, hit_location, normal));
            }
        }
        hits.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
        });
        let values = hits
            .into_iter()
            .map(|(_, _, actor, location, normal)| {
                Ok(PropValue::FixedArray(vec![
                    PropValue::Object(Some(actor.0 as i32)),
                    trace_vector_value(arena, location)?,
                    trace_vector_value(arena, normal)?,
                ]))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(PropValue::Array(values))
    }
    /// Resolve `AActor::Trace` from the shared TraceActors collision path,
    /// retaining only its first hit and clearing both vectors on a clear ray.
    fn resolve_trace(
        &self,
        source: ObjectId,
        start: [f32; 3],
        end: [f32; 3],
        trace_actors: bool,
        extent: [f32; 3],
    ) -> Result<PropValue> {
        let arena = &self.world.arena;
        let actor_class = arena.find_by_path("Engine.Actor").ok_or_else(|| {
            EngineError::new(
                "native.trace_collision_unavailable",
                "Engine.Actor is unavailable for Trace",
            )
        })?;
        let PropValue::Array(mut hits) =
            self.resolve_trace_actors(source, Some(actor_class), start, end, extent, trace_actors)?
        else {
            unreachable!("TraceActors resolver always returns an array")
        };
        if let Some(hit) = hits.drain(..).next() {
            return Ok(hit);
        }
        Ok(PropValue::FixedArray(vec![
            PropValue::Object(None),
            trace_vector_value(arena, [0.0; 3])?,
            trace_vector_value(arena, [0.0; 3])?,
        ]))
    }

    fn resolve_fast_trace(
        &self,
        source: ObjectId,
        start: [f32; 3],
        end: [f32; 3],
    ) -> Result<PropValue> {
        let polys = self.trace_collision_polys();
        if polys.is_empty() {
            return Err(EngineError::new(
                "native.trace_collision_unavailable",
                format!(
                    "FastTrace source {source:?} has no level collision geometry for range {start:?}->{end:?}"
                ),
            ));
        }
        Ok(PropValue::Bool(fast_trace_clear(&polys, start, end)))
    }

    fn resolve_skeletal_query(
        &mut self,
        actor: ObjectId,
        query: hp_uobject::vm::SkeletalQuery,
    ) -> Result<PropValue> {
        if let hp_uobject::vm::SkeletalQuery::IsAnimating { root_bone } = query
            && root_bone.is_none()
        {
            return Ok(PropValue::Bool(self.animation_is_active(actor)));
        }

        let Some((key, package, archive, export_index, class_name)) = self.mesh_asset(actor)?
        else {
            return Ok(match query {
                hp_uobject::vm::SkeletalQuery::HasAnim { .. }
                | hp_uobject::vm::SkeletalQuery::IsAnimating { .. } => PropValue::Bool(false),
                hp_uobject::vm::SkeletalQuery::BoneNumber { .. } => PropValue::Int(0),
                hp_uobject::vm::SkeletalQuery::BoneName { .. }
                | hp_uobject::vm::SkeletalQuery::AnimGroup { .. } => PropValue::Name(Name::none()),
            });
        };
        if !self.skeletal_assets.contains_key(&key) {
            let metadata = load_skeletal_asset_metadata(
                &self.data_root,
                &package,
                archive,
                export_index,
                &class_name,
            )?;
            self.skeletal_assets.insert(key.clone(), metadata);
        }
        let selects_animation_asset = matches!(
            query,
            hp_uobject::vm::SkeletalQuery::HasAnim { .. }
                | hp_uobject::vm::SkeletalQuery::AnimGroup { .. }
        );
        let override_key = if selects_animation_asset
            && self
                .skeletal_assets
                .get(&key)
                .is_some_and(|metadata| metadata.skeletal)
        {
            self.ensure_animation_override(actor)?
        } else {
            None
        };
        let metadata = self
            .skeletal_assets
            .get(&key)
            .expect("metadata inserted above");
        match query {
            hp_uobject::vm::SkeletalQuery::HasAnim { sequence } => {
                let sequence = self.world.arena.names.text(sequence.index).unwrap_or("");
                let override_sequences = override_key
                    .as_ref()
                    .and_then(|key| self.animation_assets.get(key))
                    .map(|metadata| &metadata.sequence_names);
                let present = skeletal_has_anim_selected(metadata, override_sequences, sequence);
                if !present && std::env::var_os("HP2_TRACE").is_some() {
                    let actor_path = self
                        .world
                        .arena
                        .path_of(actor)
                        .unwrap_or_else(|_| format!("{actor:?}"));
                    let class_path = self
                        .world
                        .arena
                        .get(actor)
                        .ok()
                        .and_then(|object| object.class_id)
                        .and_then(|class| self.world.arena.path_of(class).ok())
                        .unwrap_or_else(|| "None".to_string());
                    let selected = override_key.as_deref().unwrap_or(&key);
                    eprintln!(
                        "hp-engine: HasAnim miss actor={actor_path} class={class_path} \
                         mesh={key} selected={selected} sequence={sequence}"
                    );
                }
                Ok(PropValue::Bool(present))
            }
            hp_uobject::vm::SkeletalQuery::BoneNumber { bone } => {
                let bone = self.world.arena.names.text(bone.index).unwrap_or("");
                Ok(PropValue::Int(skeletal_bone_number(metadata, bone)))
            }
            hp_uobject::vm::SkeletalQuery::BoneName { index } => {
                let name = usize::try_from(index)
                    .ok()
                    .filter(|_| metadata.skeletal)
                    .and_then(|index| metadata.bone_names.get(index));
                Ok(PropValue::Name(name.map_or_else(Name::none, |name| Name {
                    index: self.world.arena.names.intern(name),
                    number: NO_NUMBER,
                })))
            }
            hp_uobject::vm::SkeletalQuery::IsAnimating { root_bone } => {
                if root_bone.is_none() {
                    return Ok(PropValue::Bool(self.animation_is_active(actor)));
                }
                let root_bone = self.world.arena.names.text(root_bone.index).unwrap_or("");
                let Some(bone) = metadata
                    .bone_names
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(root_bone))
                else {
                    return Ok(PropValue::Bool(false));
                };
                Ok(PropValue::Bool(
                    self.auxiliary_animation_channel(actor, bone)
                        .is_some_and(|channel| self.animation_is_active(channel)),
                ))
            }
            hp_uobject::vm::SkeletalQuery::AnimGroup { sequence } => {
                let sequence = self.world.arena.names.text(sequence.index).unwrap_or("");
                let sequence_key = hp_uobject::vm::fold_key(sequence);
                let group = override_key
                    .as_ref()
                    .and_then(|key| self.animation_assets.get(key))
                    .and_then(|metadata| metadata.sequence_groups.get(&sequence_key))
                    .or_else(|| metadata.animation_groups.get(&sequence_key))
                    .cloned();
                Ok(PropValue::Name(group.map_or_else(Name::none, |group| Name {
                    index: self.world.arena.names.intern(&group),
                    number: NO_NUMBER,
                })))
            }
        }
    }

    fn animation_is_active(&self, actor: ObjectId) -> bool {
        self.actor_animations.get(&actor).is_some_and(|animation| {
            !animation.sequence.is_none()
                && (animation.looped
                    || animation
                        .finish_after_seconds
                        .is_none_or(|finish| animation.elapsed_seconds < finish))
        })
    }

    fn auxiliary_animation_channel(&self, actor: ObjectId, bone: usize) -> Option<ObjectId> {
        let aux_name = self.world.arena.names.find_index("AuxAnims")?;
        let anim_bone_name = self.world.arena.names.find_index("AnimBone")?;
        let channels = self
            .world
            .arena
            .get(actor)
            .ok()?
            .properties()?
            .get(aux_name)?;
        let PropValue::Array(channels) = channels else {
            return None;
        };
        channels.iter().find_map(|value| {
            let PropValue::Object(Some(raw)) = value else {
                return None;
            };
            let channel = ObjectId(u32::try_from(*raw).ok()?);
            let anim_bone = self
                .world
                .arena
                .get(channel)
                .ok()?
                .properties()?
                .get(anim_bone_name)?;
            match anim_bone {
                PropValue::Byte(value) if usize::from(*value) == bone => Some(channel),
                PropValue::Int(value) if *value == bone as i32 => Some(channel),
                _ => None,
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn create_anim_channel(
        &mut self,
        reserved: ObjectId,
        actor: ObjectId,
        class: Option<ObjectId>,
        anim_type: i32,
        root_bone: hp_uobject::name::Name,
        transient: bool,
        not_replaceable: bool,
    ) -> Result<Option<ObjectId>> {
        if root_bone.is_none() {
            return Ok(None);
        }
        let Some((key, package, archive, export_index, class_name)) = self.mesh_asset(actor)?
        else {
            return Ok(None);
        };
        if !self.skeletal_assets.contains_key(&key) {
            let metadata = load_skeletal_asset_metadata(
                &self.data_root,
                &package,
                archive,
                export_index,
                &class_name,
            )?;
            self.skeletal_assets.insert(key.clone(), metadata);
        }
        let root_text = self.world.arena.names.text(root_bone.index).unwrap_or("");
        let metadata = self
            .skeletal_assets
            .get(&key)
            .expect("metadata inserted above");
        if !metadata.skeletal {
            return Ok(None);
        }
        let Some(bone_start) = metadata
            .bone_names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(root_text))
        else {
            return Ok(None);
        };
        let num_children = metadata.bone_children.get(bone_start).copied().unwrap_or(0);

        let aux_name = self.world.arena.names.intern("AuxAnims");
        let anim_bone_name = self.world.arena.names.intern("AnimBone");
        let transient_name = self.world.arena.names.intern("bAnimTransient");
        let not_replaceable_name = self.world.arena.names.intern("bAnimNotReplaceable");
        let mut channels = self
            .world
            .arena
            .get(actor)?
            .properties()
            .and_then(|props| props.get(aux_name))
            .and_then(|value| match value {
                PropValue::Array(items) => Some(
                    items
                        .iter()
                        .filter_map(|item| match item {
                            PropValue::Object(Some(raw)) if *raw >= 0 => {
                                Some(ObjectId(*raw as u32))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        for channel in &channels {
            let Some(props) = self
                .world
                .arena
                .get(*channel)
                .ok()
                .and_then(|object| object.properties())
            else {
                continue;
            };
            let same_bone = matches!(
                props.get(anim_bone_name),
                Some(PropValue::Byte(value)) if usize::from(*value) == bone_start
            ) || matches!(
                props.get(anim_bone_name),
                Some(PropValue::Int(value)) if *value == bone_start as i32
            );
            if same_bone
                && prop_bool(props.get(transient_name)) == transient
                && prop_bool(props.get(not_replaceable_name)) == not_replaceable
            {
                return Ok(Some(*channel));
            }
        }

        let Some(class) = class else {
            return Ok(None);
        };
        let actor_class = self
            .world
            .arena
            .find_by_path("Engine.Actor")
            .ok_or_else(|| EngineError::new("native.spawn_actor_class_missing", "Engine.Actor"))?;
        if !matches!(self.world.arena.get(class)?.data, ObjectData::Class(_))
            || !self.world.arena.class_is_a(class, actor_class)?
        {
            return Ok(None);
        }

        let location = self.actor_location(actor).unwrap_or([0.0; 3]);
        let channel = self.spawn_script_actor(class, Some(actor), location, [0; 3])?;
        if channel != reserved {
            return Err(EngineError::new(
                "native.anim_channel_allocation_failed",
                format!("reserved {reserved:?}, allocated {channel:?}"),
            ));
        }
        self.copy_anim_channel_properties(actor, channel)?;
        if let Some(props) = self.world.arena.get_mut(channel)?.properties_mut() {
            props.set(anim_bone_name, PropValue::Byte(bone_start as u8));
            props.set(transient_name, PropValue::Bool(transient));
            props.set(not_replaceable_name, PropValue::Bool(not_replaceable));
        }

        if anim_type == 1 {
            channels.insert(0, channel);
        } else {
            let mut retained = Vec::with_capacity(channels.len() + 1);
            for old in channels {
                let (old_bone, protected) = self
                    .world
                    .arena
                    .get(old)
                    .ok()
                    .and_then(|object| object.properties())
                    .map(|props| {
                        let bone = match props.get(anim_bone_name) {
                            Some(PropValue::Byte(value)) => usize::from(*value),
                            Some(PropValue::Int(value)) if *value >= 0 => *value as usize,
                            _ => usize::MAX,
                        };
                        (bone, prop_bool(props.get(not_replaceable_name)))
                    })
                    .unwrap_or((usize::MAX, false));
                if anim_bone_is_subset(old_bone, bone_start, num_children) && !protected {
                    self.destroy_script_actor(old);
                } else {
                    retained.push(old);
                }
            }
            retained.push(channel);
            channels = retained;
        }
        let props = self
            .world
            .arena
            .get_mut(actor)?
            .properties_mut()
            .ok_or_else(|| {
                EngineError::new("native.anim_channel_owner_invalid", format!("{actor:?}"))
            })?;
        props.set(
            aux_name,
            PropValue::Array(
                channels
                    .into_iter()
                    .map(|id| PropValue::Object(Some(id.0 as i32)))
                    .collect(),
            ),
        );
        Ok(Some(channel))
    }

    fn mesh_asset(
        &self,
        actor: ObjectId,
    ) -> Result<Option<(String, String, &PackageArchive, usize, String)>> {
        let Some((source, package, path)) = self.actor_asset_target(actor, "Mesh")? else {
            return Ok(None);
        };
        let candidate = self
            .archive_for(&package)
            .and_then(|archive| mesh_export_by_path(archive, &path).map(|index| (archive, index)))
            .or_else(|| mesh_export_by_path(source, &path).map(|index| (source, index)))
            .ok_or_else(|| {
                skeletal_metadata_error(format!(
                    "actor {actor:?} mesh {package}.{path} has no export payload"
                ))
            })?;
        let class_name = mesh_export_class(candidate.0, candidate.1).ok_or_else(|| {
            skeletal_metadata_error(format!(
                "actor {actor:?} mesh {package}.{path} has no resolvable class"
            ))
        })?;
        Ok(Some((
            hp_uobject::vm::fold_key(&format!("{package}.{path}")),
            package,
            candidate.0,
            candidate.1,
            class_name.to_string(),
        )))
    }
    fn ensure_animation_override(&mut self, actor: ObjectId) -> Result<Option<String>> {
        let Some((source, package, path)) = self.actor_asset_target(actor, "SkelAnim")? else {
            return Ok(None);
        };
        let exists = self
            .archive_for(&package)
            .is_some_and(|archive| mesh_export_by_path(archive, &path).is_some())
            || mesh_export_by_path(source, &path).is_some();
        if !exists {
            return Err(skeletal_metadata_error(format!(
                "actor {actor:?} SkelAnim {package}.{path} has no export"
            )));
        }
        let key = hp_uobject::vm::fold_key(&format!("{package}.{path}"));
        if !self.animation_assets.contains_key(&key) {
            let animation_stem = path.rsplit('.').next().unwrap_or(&path);
            let metadata = load_psa_sequence_metadata(&self.data_root, &package, animation_stem)?
                .ok_or_else(|| {
                    skeletal_metadata_error(format!(
                        "actor {actor:?} SkelAnim {package}.{path} has no resolvable companion PSA"
                    ))
                })?;
            self.animation_assets.insert(key.clone(), metadata);
        }
        Ok(Some(key))
    }

    fn actor_asset_target(
        &self,
        actor: ObjectId,
        property: &str,
    ) -> Result<Option<(&PackageArchive, String, String)>> {
        let Some(ResolvedProp::Object(asset)) =
            effective_prop(&self.world.arena, &self.world, actor, property)
        else {
            return Ok(None);
        };
        match asset {
            crate::scene::ResolvedObject::Null
            | crate::scene::ResolvedObject::Raw { raw: 0, .. } => Ok(None),
            crate::scene::ResolvedObject::Raw { package, raw } => {
                let source = self.archive_for(&package).ok_or_else(|| {
                    skeletal_metadata_error(format!(
                        "actor {actor:?} {property} source archive {package:?} is unavailable"
                    ))
                })?;
                let owner = if package == "__level__" {
                    self.level_package_name().unwrap_or("__level__")
                } else {
                    package.as_str()
                };
                let (target_package, path) =
                    mesh_object_ref_parts(source, owner, raw).ok_or_else(|| {
                        skeletal_metadata_error(format!(
                            "actor {actor:?} {property} reference {raw} cannot be resolved"
                        ))
                    })?;
                Ok(Some((source, target_package, path)))
            }
            crate::scene::ResolvedObject::Arena(id) => {
                let canonical = self.world.arena.path_of(id).map_err(|error| {
                    skeletal_metadata_error(format!(
                        "actor {actor:?} {property} {id:?} has no canonical path: {error}"
                    ))
                })?;
                let (package, path) = canonical.split_once('.').ok_or_else(|| {
                    skeletal_metadata_error(format!(
                        "actor {actor:?} {property} path {canonical:?} has no package"
                    ))
                })?;
                let source = self.archive_for(package).ok_or_else(|| {
                    skeletal_metadata_error(format!(
                        "actor {actor:?} {property} archive {package:?} is unavailable"
                    ))
                })?;
                Ok(Some((source, package.to_string(), path.to_string())))
            }
        }
    }

    fn actor_render_asset_paths(
        &self,
        actor: ObjectId,
    ) -> Result<(Option<String>, Option<String>)> {
        let mesh_path = self
            .actor_asset_target(actor, "Mesh")?
            .map(|(_, package, path)| format!("{package}.{path}"));
        let mut animation_path = self
            .actor_asset_target(actor, "SkelAnim")?
            .map(|(_, package, path)| format!("{package}.{path}"));
        if animation_path.is_none()
            && let Some((_, package, archive, export_index, class_name)) = self.mesh_asset(actor)?
        {
            let payload = archive.export_payload(export_index).ok_or_else(|| {
                skeletal_metadata_error(format!("mesh export {export_index} has no payload"))
            })?;
            let mesh = decode_export(&class_name, payload, archive)
                .map_err(|error| skeletal_metadata_error(error.to_string()))?;
            animation_path = mesh.skeletal_default_animation.map(|asset| {
                if asset.path.contains('.') {
                    asset.path
                } else {
                    format!("{package}.{}", asset.path)
                }
            });
        }
        Ok((mesh_path, animation_path))
    }

    fn animation_request_metadata(
        &mut self,
        actor: ObjectId,
        sequence: hp_uobject::name::Name,
        rate: f32,
    ) -> (Option<(f32, f32)>, Option<String>, Option<String>) {
        let timing = self.actor_animation_timing(actor, sequence, rate);
        let paths = self.actor_render_asset_paths(actor);
        if let Err(error) = timing.as_ref().map(|_| ()).and(paths.as_ref().map(|_| ())) {
            let sequence_text = self
                .world
                .arena
                .names
                .text(sequence.index)
                .unwrap_or("None")
                .to_string();
            if self
                .animation_metadata_reported
                .insert((actor, sequence_text))
            {
                eprintln!(
                    "hp-engine: note [engine.animation_metadata_unavailable] actor {actor:?}: {error}; retaining bind pose and fallback FinishAnim timing"
                );
            }
        }
        let timing = timing.ok().flatten();
        let (mesh_path, animation_path) = paths.unwrap_or((None, None));
        (timing, mesh_path, animation_path)
    }

    fn actor_animation_timing(
        &self,
        actor: ObjectId,
        sequence: hp_uobject::name::Name,
        play_rate: f32,
    ) -> Result<Option<(f32, f32)>> {
        let sequence = self
            .world
            .arena
            .names
            .text(sequence.index)
            .unwrap_or("None");
        let rate = play_rate.abs();
        if !rate.is_finite() || rate <= 0.0 {
            return Ok(None);
        }
        let mut default_animation_path = None;
        if let Some((_, package, archive, export_index, class_name)) = self.mesh_asset(actor)? {
            let payload = archive.export_payload(export_index).ok_or_else(|| {
                skeletal_metadata_error(format!("mesh export {export_index} has no payload"))
            })?;
            let mesh = decode_export(&class_name, payload, archive)
                .map_err(|error| skeletal_metadata_error(error.to_string()))?;
            if let Some(anim) = mesh
                .animations
                .iter()
                .find(|anim| anim.name.eq_ignore_ascii_case(sequence))
                && anim.frame_count > 0
                && anim.rate.is_finite()
                && anim.rate > 0.0
            {
                return Ok(Some((
                    anim.frame_count as f32 / anim.rate,
                    anim.frame_count.saturating_sub(1) as f32 / (anim.rate * rate),
                )));
            }
            default_animation_path = mesh.skeletal_default_animation.map(|asset| {
                if asset.path.contains('.') {
                    asset.path
                } else {
                    format!("{package}.{}", asset.path)
                }
            });
        }
        let animation_path = match self.actor_asset_target(actor, "SkelAnim")? {
            Some((_, package, path)) => format!("{package}.{path}"),
            None => match default_animation_path {
                Some(path) => path,
                None => return Ok(None),
            },
        };
        let Some((package, stem)) = animation_path.split_once('.') else {
            return Ok(None);
        };
        Ok(
            load_psa_sequence_timing(&self.data_root, package, stem, sequence)?
                .map(|(duration, finish)| (duration, finish / rate)),
        )
    }

    fn archive_for(&self, package: &str) -> Option<&PackageArchive> {
        if package == "__level__"
            || self
                .level_package_name()
                .is_some_and(|level| level.eq_ignore_ascii_case(package))
        {
            return self.level.archive.as_ref();
        }
        self.world
            .archives
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(package))
            .map(|(_, archive)| archive)
    }

    fn level_package_name(&self) -> Option<&str> {
        self.world
            .arena
            .get(self.level.root)
            .ok()
            .and_then(|object| self.world.arena.names.text(object.name_index))
    }

    /// Apply a frame's engine-facing mutations in emission order. This is
    /// the sole mutable engine boundary for native bodies, which only receive
    /// `&mut Frame`.
    pub fn apply_script_effects(&mut self, effects: Vec<ScriptEffect>) -> Result<()> {
        for effect in effects {
            match effect {
                ScriptEffect::Log { message } if self.script_logs.len() < SCRIPT_LOG_BUDGET => {
                    self.script_logs.push(message)
                }
                ScriptEffect::Log { .. } => {}
                ScriptEffect::SetLocation { actor, location } => {
                    self.require_active_actor(actor, "set_location")?;
                    self.set_actor_vector(actor, "Location", location)?;
                }
                ScriptEffect::SetRotation { actor, rotation } => {
                    self.require_active_actor(actor, "set_rotation")?;
                    self.set_actor_rotator(actor, "Rotation", rotation)?;
                }
                ScriptEffect::SetPhysics { actor, physics } => {
                    self.require_active_actor(actor, "set_physics")?;
                    self.set_actor_int(actor, "Physics", physics)?;
                }
                ScriptEffect::SetCollision {
                    actor,
                    colliding_actors,
                    block_actors,
                    block_players,
                } => {
                    self.require_active_actor(actor, "set_collision")?;
                    self.set_actor_bool(actor, "bCollideActors", colliding_actors)?;
                    self.set_actor_bool(actor, "bBlockActors", block_actors)?;
                    self.set_actor_bool(actor, "bBlockPlayers", block_players)?;
                }
                ScriptEffect::SetCollisionSize {
                    actor,
                    radius,
                    height,
                    width,
                } => {
                    self.require_active_actor(actor, "set_collision_size")?;
                    self.set_actor_float(actor, "CollisionRadius", radius)?;
                    self.set_actor_float(actor, "CollisionHeight", height)?;
                    self.set_actor_float(actor, "CollisionWidth", width)?;
                }
                ScriptEffect::LocalizeRequest { request_id, .. } => {
                    return Err(EngineError::new(
                        "native.effect_result_unconsumed",
                        format!("localization request {request_id} was not awaited"),
                    ));
                }
                ScriptEffect::SkeletalQuery { request_id, .. } => {
                    return Err(EngineError::new(
                        "native.effect_result_unconsumed",
                        format!("skeletal metadata request {request_id} was not awaited"),
                    ));
                }
                ScriptEffect::CreateAnimChannelRequest { request_id, .. } => {
                    return Err(EngineError::new(
                        "native.effect_result_unconsumed",
                        format!("animation channel request {request_id} was not awaited"),
                    ));
                }
                ScriptEffect::TraceActorsRequest { request_id, .. } => {
                    return Err(EngineError::new(
                        "native.effect_result_unconsumed",
                        format!("TraceActors request {request_id} was not awaited"),
                    ));
                }
                ScriptEffect::TraceRequest { request_id, .. } => {
                    return Err(EngineError::new(
                        "native.effect_result_unconsumed",
                        format!("Trace request {request_id} was not awaited"),
                    ));
                }
                ScriptEffect::FastTraceRequest { request_id, .. } => {
                    return Err(EngineError::new(
                        "native.effect_result_unconsumed",
                        format!("FastTrace request {request_id} was not awaited"),
                    ));
                }
                ScriptEffect::SetOwner { actor, owner } => {
                    self.require_active_actor(actor, "set_owner")?;
                    self.set_actor_object(actor, "Owner", owner)?;
                }
                ScriptEffect::SetBase { actor, base } => {
                    self.require_active_actor(actor, "set_base")?;
                    self.set_actor_object(actor, "Base", base)?;
                }
                ScriptEffect::MakeNoise { actor, loudness } => {
                    self.dispatch_make_noise(actor, loudness)?
                }

                ScriptEffect::AddPawn { pawn } => self.apply_add_pawn(pawn)?,
                ScriptEffect::SpawnRequest {
                    request_id,
                    reserved,
                    class,
                    owner,
                    location,
                    rotation,
                } => {
                    let spawned = self.spawn_script_actor(class, owner, location, rotation)?;
                    if spawned != reserved {
                        return Err(EngineError::new(
                            "native.spawn_allocation_failed",
                            format!(
                                "request {request_id} reserved {reserved:?}, allocated {spawned:?}"
                            ),
                        ));
                    }
                    debug_assert!(self.level.actors.last() == Some(&spawned));
                }
                ScriptEffect::Destroy { actor } => self.destroy_script_actor(actor),
                ScriptEffect::PlaySound { actor, cue } => {
                    self.require_active_actor(actor, "play_sound")?;
                    let cue = self
                        .world
                        .arena
                        .path_of(cue)
                        .unwrap_or_else(|_| format!("{cue:?}"));
                    self.play_sound_cue(&format!("PlaySound {cue}"));
                }
                ScriptEffect::StopSound { actor, cue } => {
                    self.require_active_actor(actor, "stop_sound")?;
                    let cue = cue
                        .and_then(|cue| self.world.arena.path_of(cue).ok())
                        .unwrap_or_else(|| "None".to_string());
                    self.play_sound_cue(&format!("StopSound {cue}"));
                }
                ScriptEffect::PlayMusic {
                    actor,
                    song,
                    handle,
                } => {
                    self.require_active_actor(actor, "play_music")?;
                    self.sound_cues
                        .push(format!("PlayMusic {song} handle={handle}"));
                    if self.audio.is_some() {
                        self.start_script_music(&song);
                    } else {
                        eprintln!(
                            "hp-engine: [engine.music_unavailable] PlayMusic \"{song}\" handle {handle}: no audio host attached"
                        );
                    }
                }
                ScriptEffect::StopMusic { actor, handle } => {
                    self.require_active_actor(actor, "stop_music")?;
                    self.sound_cues.push(format!("StopMusic handle={handle}"));
                    eprintln!(
                        "hp-engine: [engine.music_unavailable] StopMusic handle {handle}: stream stop is unavailable"
                    );
                }
                ScriptEffect::StopAllMusic { actor } => {
                    self.require_active_actor(actor, "stop_all_music")?;
                    self.sound_cues.push("StopAllMusic".to_string());
                    eprintln!(
                        "hp-engine: [engine.music_unavailable] StopAllMusic: stream stop is unavailable"
                    );
                }
                ScriptEffect::AudioUnavailable { actor, source } => {
                    let actor = actor
                        .and_then(|actor| self.world.arena.path_of(actor).ok())
                        .unwrap_or_else(|| "<static>".to_string());
                    let key = format!("{actor}: {source}");
                    if self.deferred_sounds.insert(key.clone()) {
                        eprintln!("hp-engine: [engine.audio_unavailable] {key}");
                    }
                    self.sound_cues.push(format!("Unavailable {key}"));
                }
                ScriptEffect::PlayAnim {
                    actor,
                    sequence,
                    rate,
                    looped,
                } => {
                    self.require_active_actor(actor, "play_anim")?;
                    let (timing, mesh_path, animation_path) =
                        self.animation_request_metadata(actor, sequence, rate);
                    self.actor_animations.insert(
                        actor,
                        ActorAnimation {
                            sequence,
                            started_at: self.sim_time,
                            elapsed_seconds: 0.0,
                            duration_seconds: timing.map(|timing| timing.0),
                            finish_after_seconds: timing.map(|timing| timing.1),
                            mesh_path,
                            animation_path,
                            rate,
                            looped,
                            tweening: false,
                        },
                    );
                }
                ScriptEffect::LoopAnim {
                    actor,
                    sequence,
                    rate,
                    looped,
                } => {
                    self.require_active_actor(actor, "loop_anim")?;
                    let (timing, mesh_path, animation_path) =
                        self.animation_request_metadata(actor, sequence, rate);
                    self.actor_animations.insert(
                        actor,
                        ActorAnimation {
                            sequence,
                            started_at: self.sim_time,
                            elapsed_seconds: 0.0,
                            duration_seconds: timing.map(|timing| timing.0),
                            finish_after_seconds: timing.map(|timing| timing.1),
                            mesh_path,
                            animation_path,
                            rate,
                            looped,
                            tweening: false,
                        },
                    );
                }
                ScriptEffect::TweenAnim {
                    actor,
                    sequence,
                    rate,
                    looped,
                } => {
                    self.require_active_actor(actor, "tween_anim")?;
                    let (timing, mesh_path, animation_path) =
                        self.animation_request_metadata(actor, sequence, rate);
                    self.actor_animations.insert(
                        actor,
                        ActorAnimation {
                            sequence,
                            started_at: self.sim_time,
                            elapsed_seconds: 0.0,
                            duration_seconds: timing.map(|timing| timing.0),
                            finish_after_seconds: timing.map(|timing| timing.1),
                            mesh_path,
                            animation_path,
                            rate,
                            looped,
                            tweening: true,
                        },
                    );
                }
                ScriptEffect::MoveSmooth { actor, delta } => {
                    self.require_active_actor(actor, "move_smooth")?;
                    let location = self.actor_location(actor).ok_or_else(|| {
                        EngineError::new(
                            "native.move_smooth_location_missing",
                            format!("actor {actor:?} has no Location"),
                        )
                    })?;
                    self.set_actor_vector(
                        actor,
                        "Location",
                        [
                            location[0] + delta[0],
                            location[1] + delta[1],
                            location[2] + delta[2],
                        ],
                    )?;
                }
                ScriptEffect::MoveTo { actor, destination } => {
                    self.require_active_actor(actor, "move_to")?;
                    self.set_actor_vector(actor, "Location", destination)?;
                }
                ScriptEffect::MoveToward { actor, target } => {
                    self.require_active_actor(actor, "move_toward")?;
                    let location = self.actor_location(target).ok_or_else(|| {
                        EngineError::new(
                            "native.move_toward_target_location_missing",
                            format!("target {target:?} has no Location"),
                        )
                    })?;
                    self.set_actor_vector(actor, "Location", location)?;
                }
                ScriptEffect::TurnTo { actor, focus } => {
                    self.require_active_actor(actor, "turn_to")?;
                    let location = self.actor_location(actor).ok_or_else(|| {
                        EngineError::new(
                            "native.turn_to_location_missing",
                            format!("actor {actor:?} has no Location"),
                        )
                    })?;
                    self.set_actor_rotator(actor, "Rotation", focus_rotation(location, focus))?;
                }
                ScriptEffect::TurnToward { actor, target } => {
                    self.require_active_actor(actor, "turn_toward")?;
                    let location = self.actor_location(actor).ok_or_else(|| {
                        EngineError::new(
                            "native.turn_toward_location_missing",
                            format!("actor {actor:?} has no Location"),
                        )
                    })?;
                    let focus = self.actor_location(target).ok_or_else(|| {
                        EngineError::new(
                            "native.turn_toward_target_location_missing",
                            format!("target {target:?} has no Location"),
                        )
                    })?;
                    self.set_actor_rotator(actor, "Rotation", focus_rotation(location, focus))?;
                }
                ScriptEffect::TriggerEvent {
                    name,
                    other,
                    instigator,
                } => self.dispatch_trigger_event(name, other, instigator)?,
                ScriptEffect::FollowSpline {
                    actor,
                    command,
                    cue,
                } => self.start_follow_spline(actor, &command, cue)?,
            }
        }
        Ok(())
    }

    fn start_follow_spline(&mut self, actor: ObjectId, command: &str, cue: String) -> Result<()> {
        self.require_active_actor(actor, "follow_spline")?;
        let mut words = command.split_ascii_whitespace();
        if !words
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("FollowSpline"))
        {
            return Ok(());
        }
        let path_tag = words.next().unwrap_or_default().to_string();
        let mut start = None;
        let mut dest = None;
        let mut speed = 0.0f32;
        let mut time = 0.0f32;
        let mut accel = 0.0f32;
        let mut ease = SplineEase::Linear;
        let mut align = false;
        for word in words {
            let (key, value) = word.split_once('=').unwrap_or((word, ""));
            match key.to_ascii_lowercase().as_str() {
                "start" => start = self.find_actor_by_cut_name(value),
                "dest" => dest = self.find_actor_by_cut_name(value),
                "speed" => speed = value.parse().unwrap_or(0.0),
                "time" => time = value.parse().unwrap_or(0.0),
                "accel" => accel = value.parse().unwrap_or(0.0),
                "easefrom" => ease = SplineEase::From,
                "easeto" => ease = SplineEase::To,
                "easebetween" => ease = SplineEase::Between,
                "align" => align = true,
                _ => {}
            }
        }
        let start = start.or_else(|| {
            self.level.actors.iter().copied().find(|candidate| {
                self.prop_int(*candidate, "Position") == Some(0)
                    && self
                        .prop_text(*candidate, "Tag")
                        .is_some_and(|tag| tag.eq_ignore_ascii_case(&path_tag))
            })
        });
        let Some(start) = start else {
            return Ok(());
        };
        let mut points = vec![start];
        let mut cursor = start;
        let mut seen = HashSet::from([start]);
        while dest != Some(cursor) {
            let Some(next) = self.prop_object(cursor, "Next") else {
                break;
            };
            if !seen.insert(next) {
                break;
            }
            points.push(next);
            cursor = next;
        }
        if points.len() < 2 {
            return Ok(());
        }
        let mut segment_lengths = Vec::with_capacity(points.len() - 1);
        for pair in points.windows(2) {
            segment_lengths.push(self.spline_segment_length(pair[0], pair[1]));
        }
        let total_length: f32 = segment_lengths.iter().sum();
        if total_length <= f32::EPSILON {
            return Ok(());
        }
        if speed <= 0.0 {
            speed = points
                .iter()
                .find_map(|point| self.prop_float(*point, "DesiredSpeed").filter(|v| *v > 0.0))
                .unwrap_or(100.0);
        }
        let duration = if time > 0.0 {
            time
        } else {
            total_length / speed.max(f32::EPSILON)
        };
        self.spline_motions.insert(
            actor,
            SplineMotion {
                actor,
                points,
                segment_lengths,
                total_length,
                elapsed: 0.0,
                duration,
                accel: if accel > 0.0 { accel } else { 3000.0 },
                ease,
                align,
                cue,
                loops: dest.is_none(),
            },
        );
        Ok(())
    }

    fn advance_splines(&mut self, dt: f32) -> Result<()> {
        let mut motions = std::mem::take(&mut self.spline_motions);
        let mut finished = Vec::new();
        for motion in motions.values_mut() {
            motion.elapsed += dt;
            let reached_end = motion.elapsed >= motion.duration;
            let cycle_elapsed = if motion.loops {
                motion.elapsed % motion.duration
            } else {
                motion.elapsed.min(motion.duration)
            };
            let mut alpha = cycle_elapsed / motion.duration;
            // Pawn's authored move type determines acceleration at the ends.
            // The ratio limits the easing window for high authored accel while
            // retaining the exact Time deadline.
            let window = (motion.total_length / motion.duration / motion.accel).clamp(0.0, 0.5);
            alpha = spline_ease_alpha(alpha, motion.ease, window);
            let distance = alpha * motion.total_length;
            let mut accumulated = 0.0;
            let mut segment = motion.segment_lengths.len() - 1;
            for (index, length) in motion.segment_lengths.iter().enumerate() {
                if distance <= accumulated + length {
                    segment = index;
                    break;
                }
                accumulated += length;
            }
            let segment_length = motion.segment_lengths[segment].max(f32::EPSILON);
            let local = ((distance - accumulated) / segment_length).clamp(0.0, 1.0);
            let location =
                self.spline_position(motion.points[segment], motion.points[segment + 1], local);
            self.set_actor_vector(motion.actor, "Location", location)?;
            if motion.align {
                let ahead = self.spline_position(
                    motion.points[segment],
                    motion.points[segment + 1],
                    (local + 0.001).min(1.0),
                );
                self.set_actor_rotator(motion.actor, "Rotation", focus_rotation(location, ahead))?;
            }
            if reached_end && !motion.loops {
                finished.push((motion.actor, motion.cue.clone()));
            }
        }
        for (actor, _) in &finished {
            motions.remove(actor);
        }
        self.spline_motions = motions;
        for (actor, cue) in finished {
            self.set_actor_int(actor, "Physics", 0)?;
            self.set_actor_bool(actor, "bInterpolating", false)?;
            if !cue.is_empty() {
                self.cues.push(cue.clone());
            }
            // DoCutCueNotify reads the source-owned CutNotifyActor and
            // sCutNotifyCue fields; never bypass the capture table.
            if !cue.is_empty() {
                self.run_named_event_on(actor, "DoCutCueNotify", None)?;
            }
        }
        Ok(())
    }

    fn spline_segment_length(&self, from: ObjectId, to: ObjectId) -> f32 {
        let mut length = 0.0;
        let mut previous = self.spline_position(from, to, 0.0);
        for step in 1..=16 {
            let current = self.spline_position(from, to, step as f32 / 16.0);
            length += vector_distance(previous, current);
            previous = current;
        }
        length
    }

    fn spline_position(&self, from: ObjectId, to: ObjectId, alpha: f32) -> [f32; 3] {
        let p0 = self.actor_location(from).unwrap_or([0.0; 3]);
        let p3 = self.actor_location(to).unwrap_or(p0);
        let start = self
            .prop_vector(from, "StartControlPoint")
            .unwrap_or([0.0; 3]);
        let end = self.prop_vector(to, "EndControlPoint").unwrap_or([0.0; 3]);
        let p1 = add3(p0, start);
        let p2 = add3(p3, end);
        let r = 1.0 - alpha;
        let weights = [
            r * r * r,
            3.0 * alpha * r * r,
            3.0 * alpha * alpha * r,
            alpha * alpha * alpha,
        ];
        [
            weights[0] * p0[0] + weights[1] * p1[0] + weights[2] * p2[0] + weights[3] * p3[0],
            weights[0] * p0[1] + weights[1] * p1[1] + weights[2] * p2[1] + weights[3] * p3[1],
            weights[0] * p0[2] + weights[1] * p1[2] + weights[2] * p2[2] + weights[3] * p3[2],
        ]
    }

    fn find_actor_by_cut_name(&self, wanted: &str) -> Option<ObjectId> {
        self.level.actors.iter().copied().find(|actor| {
            self.prop_text(*actor, "CutName")
                .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
        })
    }

    fn prop_value(&self, actor: ObjectId, property: &str) -> Option<PropValue> {
        let name = self.world.arena.names.find_index(property)?;
        self.actor_effective_value(actor, name).ok().flatten()
    }

    fn prop_text(&self, actor: ObjectId, property: &str) -> Option<String> {
        match self.prop_value(actor, property)? {
            PropValue::Str(value) => Some(value),
            PropValue::Name(value) => self
                .level
                .archive
                .as_ref()
                .and_then(|archive| archive.names.get(value.index as usize))
                .map(|entry| entry.text.clone())
                .or_else(|| self.world.arena.names.text(value.index).map(str::to_string)),
            _ => None,
        }
    }

    fn prop_object(&self, actor: ObjectId, property: &str) -> Option<ObjectId> {
        match self.prop_value(actor, property)? {
            PropValue::Object(Some(raw)) if raw >= 0 => Some(ObjectId(raw as u32)),
            _ => None,
        }
    }

    fn prop_int(&self, actor: ObjectId, property: &str) -> Option<i32> {
        match self.prop_value(actor, property)? {
            PropValue::Byte(value) => Some(i32::from(value)),
            PropValue::Int(value) => Some(value),
            _ => None,
        }
    }

    fn prop_float(&self, actor: ObjectId, property: &str) -> Option<f32> {
        match self.prop_value(actor, property)? {
            PropValue::Byte(value) => Some(f32::from(value)),
            PropValue::Int(value) => Some(value as f32),
            PropValue::Float(value) => Some(value),
            _ => None,
        }
    }

    fn prop_vector(&self, actor: ObjectId, property: &str) -> Option<[f32; 3]> {
        let PropValue::Struct { fields, .. } = self.prop_value(actor, property)? else {
            return None;
        };
        let component = |wanted: &str| {
            let name = self.world.arena.names.find_index(wanted)?;
            match &fields.iter().find(|(field, _)| *field == name)?.1 {
                PropValue::Byte(value) => Some(f32::from(*value)),
                PropValue::Int(value) => Some(*value as f32),
                PropValue::Float(value) => Some(*value),
                _ => None,
            }
        };
        Some([component("X")?, component("Y")?, component("Z")?])
    }

    /// `Core::Localize` (`UnMisc.cpp:1326-1352`): read
    /// `<Package>.<language>`, fall back to `.int`, and return the diagnostic
    /// marker used by script callers to detect a missing key.
    fn resolve_localize(&self, section: &str, key: &str, package: &str) -> String {
        // CutScriptDisk.uc declares `lineArray[4096]` but its authored load
        // loop asks Localize for up to 9999 keys. Treat the exclusive endpoint
        // as the same missing-key control-flow sentinel used by the source
        // guard; never let it become a lineArray lvalue.
        if !cutscript_disk_line_in_bounds(package, key) {
            return format!("<?int?{package}.{section}.{key}?>");
        }
        let normalized = package.replace('\\', "/");
        let relative = PathBuf::from(&normalized);
        let with_int = relative
            .extension()
            .is_none()
            .then(|| relative.with_extension("int"));
        let cutscene_int = normalized
            .to_ascii_lowercase()
            .strip_prefix("cutscenes/")
            .map(|tail| PathBuf::from("CUTSCENES").join(tail).with_extension("int"));
        let mut candidates = Vec::with_capacity(6);
        candidates.push(self.data_root.join("System").join(&relative));
        candidates.push(self.data_root.join(&relative));
        if let Some(path) = cutscene_int {
            // Retail HP2 stores authored disk scripts in the uppercase
            // `System/CUTSCENES` directory on case-sensitive filesystems.
            candidates.push(self.data_root.join("System").join(path));
        }
        if let Some(path) = with_int {
            candidates.push(self.data_root.join("System").join(&path));
            candidates.push(self.data_root.join(path));
        }
        for path in candidates {
            if let Ok(ini) = load_pair(&path, None)
                && let Some(value) = ini.get(section, key)
            {
                return value.to_string();
            }
        }
        format!("<?int?{package}.{section}.{key}?>")
    }

    fn require_active_actor(&self, actor: ObjectId, native: &str) -> Result<()> {
        if self.destroyed_actors.contains(&actor) || !self.level.actors.contains(&actor) {
            return Err(EngineError::new(
                "native.actor_inactive",
                format!("{native}: actor {actor:?} is not active in this level"),
            ));
        }
        Ok(())
    }
    /// Apply `APawn::AddPawn` exactly as `UnPawn.cpp:1105-1113`: read the
    /// runtime `Actor.Level` binding, copy `LevelInfo.PawnList` into
    /// `Pawn.nextPawn`, then install the pawn as the new head. Deliberately no
    /// duplicate guard; repeated calls reproduce the native's pointer writes.
    fn apply_add_pawn(&mut self, pawn: ObjectId) -> Result<()> {
        self.require_active_actor(pawn, "add_pawn")?;
        let level_name = self.world.arena.names.find_index("Level").ok_or_else(|| {
            EngineError::new(
                "native.add_pawn_level_property_missing",
                "AActor.Level property name is unavailable",
            )
        })?;
        let pawn_list_name = self
            .world
            .arena
            .names
            .find_index("PawnList")
            .ok_or_else(|| {
                EngineError::new(
                    "native.add_pawn_pawn_list_property_missing",
                    "ALevelInfo.PawnList property name is unavailable",
                )
            })?;
        let next_pawn_name = self
            .world
            .arena
            .names
            .find_index("nextPawn")
            .ok_or_else(|| {
                EngineError::new(
                    "native.add_pawn_next_pawn_property_missing",
                    "APawn.nextPawn property name is unavailable",
                )
            })?;
        let level_info = match self
            .world
            .arena
            .get(pawn)?
            .properties()
            .and_then(|store| store.get(level_name))
        {
            Some(PropValue::Object(Some(raw))) if *raw >= 0 => ObjectId(*raw as u32),
            other => {
                return Err(EngineError::new(
                    "native.add_pawn_level_missing",
                    format!("pawn {pawn:?} has no runtime LevelInfo binding: {other:?}"),
                ));
            }
        };
        if !self.world.arena.contains(level_info) {
            return Err(EngineError::new(
                "native.add_pawn_level_missing",
                format!("pawn {pawn:?} Level points outside the arena: {level_info:?}"),
            ));
        }
        let has_declared_property = |object: ObjectId, property: u32| -> Result<bool> {
            let class = self.world.arena.get(object)?.class_id.ok_or_else(|| {
                EngineError::new(
                    "native.add_pawn_property_owner_invalid",
                    format!("object {object:?} has no runtime class"),
                )
            })?;
            Ok(self
                .world
                .arena
                .class_chain(class)?
                .into_iter()
                .any(|class| {
                    self.world.arena.children_of(class).any(|(_, child)| {
                        child.name_index == property
                            && matches!(child.data, ObjectData::Property(_))
                    })
                }))
        };
        if !has_declared_property(pawn, next_pawn_name)? {
            return Err(EngineError::new(
                "native.add_pawn_next_pawn_property_missing",
                format!("pawn {pawn:?} class has no nextPawn property"),
            ));
        }
        if !has_declared_property(level_info, pawn_list_name)? {
            return Err(EngineError::new(
                "native.add_pawn_pawn_list_property_missing",
                format!("LevelInfo {level_info:?} class has no PawnList property"),
            ));
        }
        let old_head = match self
            .world
            .arena
            .get(level_info)?
            .properties()
            .and_then(|store| store.get(pawn_list_name))
        {
            Some(PropValue::Object(raw)) => *raw,
            None => None,
            other => {
                return Err(EngineError::new(
                    "native.add_pawn_pawn_list_property_invalid",
                    format!("LevelInfo {level_info:?} PawnList is not an object: {other:?}"),
                ));
            }
        };
        match self
            .world
            .arena
            .get(pawn)?
            .properties()
            .and_then(|store| store.get(next_pawn_name))
        {
            Some(PropValue::Object(_)) | None => {}
            other => {
                return Err(EngineError::new(
                    "native.add_pawn_next_pawn_property_invalid",
                    format!("pawn {pawn:?} nextPawn is not an object: {other:?}"),
                ));
            }
        }
        let pawn_raw = i32::try_from(pawn.0).map_err(|_| {
            EngineError::new(
                "native.add_pawn_object_id_range",
                format!("pawn id {pawn:?} exceeds the script object representation"),
            )
        })?;
        self.actor_store_mut(pawn, "add_pawn")?
            .set(next_pawn_name, PropValue::Object(old_head));
        self.world
            .arena
            .get_mut(level_info)?
            .properties_mut()
            .ok_or_else(|| {
                EngineError::new(
                    "native.add_pawn_level_properties_missing",
                    format!("LevelInfo {level_info:?} has no property store"),
                )
            })?
            .set(pawn_list_name, PropValue::Object(Some(pawn_raw)));
        Ok(())
    }

    fn actor_store_mut(&mut self, actor: ObjectId, native: &str) -> Result<&mut PropStore> {
        self.world
            .arena
            .get_mut(actor)
            .map_err(EngineError::from)?
            .properties_mut()
            .ok_or_else(|| {
                EngineError::new(
                    "native.actor_properties_missing",
                    format!("{native}: actor {actor:?} has no property store"),
                )
            })
    }

    fn set_actor_bool(&mut self, actor: ObjectId, property: &str, value: bool) -> Result<()> {
        let name = self.world.arena.names.intern(property);
        self.actor_store_mut(actor, "property")?
            .set(name, PropValue::Bool(value));
        Ok(())
    }

    fn set_actor_int(&mut self, actor: ObjectId, property: &str, value: i32) -> Result<()> {
        let name = self.world.arena.names.intern(property);
        let stored = match self.actor_store_mut(actor, "property")?.get(name) {
            Some(PropValue::Byte(_)) => PropValue::Byte(value.clamp(0, u8::MAX as i32) as u8),
            Some(PropValue::Float(_)) => PropValue::Float(value as f32),
            _ => PropValue::Int(value),
        };
        self.actor_store_mut(actor, "property")?.set(name, stored);
        Ok(())
    }

    fn set_actor_float(&mut self, actor: ObjectId, property: &str, value: f32) -> Result<()> {
        let name = self.world.arena.names.intern(property);
        let stored = match self.actor_store_mut(actor, "property")?.get(name) {
            Some(PropValue::Byte(_)) => PropValue::Byte(value.clamp(0.0, f32::from(u8::MAX)) as u8),
            Some(PropValue::Int(_)) => PropValue::Int(value as i32),
            _ => PropValue::Float(value),
        };
        self.actor_store_mut(actor, "property")?.set(name, stored);
        Ok(())
    }

    fn set_actor_object(
        &mut self,
        actor: ObjectId,
        property: &str,
        value: Option<ObjectId>,
    ) -> Result<()> {
        let name = self.world.arena.names.intern(property);
        self.actor_store_mut(actor, "property")?
            .set(name, PropValue::Object(value.map(|id| id.0 as i32)));
        Ok(())
    }

    fn set_actor_vector(&mut self, actor: ObjectId, property: &str, value: [f32; 3]) -> Result<()> {
        let property_name = self.world.arena.names.intern(property);
        let struct_name = self
            .world
            .arena
            .find_by_path("Core.Vector")
            .and_then(|id| {
                self.world
                    .arena
                    .get(id)
                    .ok()
                    .map(|object| object.name_index)
            })
            .unwrap_or_else(|| self.world.arena.names.intern("Vector"));
        let fields = [
            self.world.arena.names.intern("X"),
            self.world.arena.names.intern("Y"),
            self.world.arena.names.intern("Z"),
        ];
        let store = self.actor_store_mut(actor, "set_location")?;
        let (struct_name, mut values) = match store.get(property_name) {
            Some(PropValue::Struct {
                struct_name,
                fields: current,
            }) => (*struct_name, current.clone()),
            _ => (
                struct_name,
                fields
                    .into_iter()
                    .map(|field| (field, PropValue::Float(0.0)))
                    .collect(),
            ),
        };
        for (name, component) in fields.into_iter().zip(value) {
            if let Some((_, field)) = values
                .iter_mut()
                .find(|(field_name, _)| *field_name == name)
            {
                *field = PropValue::Float(component);
            } else {
                values.push((name, PropValue::Float(component)));
            }
        }
        store.set(
            property_name,
            PropValue::Struct {
                struct_name,
                fields: values,
            },
        );
        Ok(())
    }

    fn set_actor_rotator(
        &mut self,
        actor: ObjectId,
        property: &str,
        value: [i32; 3],
    ) -> Result<()> {
        let property_name = self.world.arena.names.intern(property);
        let struct_name = self
            .world
            .arena
            .find_by_path("Core.Rotator")
            .and_then(|id| {
                self.world
                    .arena
                    .get(id)
                    .ok()
                    .map(|object| object.name_index)
            })
            .unwrap_or_else(|| self.world.arena.names.intern("Rotator"));
        let fields = [
            self.world.arena.names.intern("Pitch"),
            self.world.arena.names.intern("Yaw"),
            self.world.arena.names.intern("Roll"),
        ];
        let store = self.actor_store_mut(actor, "set_rotation")?;
        let (struct_name, mut values) = match store.get(property_name) {
            Some(PropValue::Struct {
                struct_name,
                fields: current,
            }) => (*struct_name, current.clone()),
            _ => (
                struct_name,
                fields
                    .into_iter()
                    .map(|field| (field, PropValue::Int(0)))
                    .collect(),
            ),
        };
        for (name, component) in fields.into_iter().zip(value) {
            if let Some((_, field)) = values
                .iter_mut()
                .find(|(field_name, _)| *field_name == name)
            {
                *field = PropValue::Int(component);
            } else {
                values.push((name, PropValue::Int(component)));
            }
        }
        store.set(
            property_name,
            PropValue::Struct {
                struct_name,
                fields: values,
            },
        );
        Ok(())
    }

    fn actor_location(&self, actor: ObjectId) -> Option<[f32; 3]> {
        self.world
            .arena
            .get(actor)
            .ok()
            .and_then(|object| object.properties())
            .and_then(|store| actor_vec3(store, &self.world.arena, "Location"))
    }
    fn actor_rotation(&self, actor: ObjectId) -> Option<[i32; 3]> {
        let rotation_name = self.world.arena.names.find_index("Rotation")?;
        let PropValue::Struct { fields, .. } =
            self.actor_effective_value(actor, rotation_name).ok()??
        else {
            return None;
        };
        let component = |wanted: &str| {
            fields.iter().find_map(|(name, value)| {
                self.world
                    .arena
                    .names
                    .text(*name)
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                    .then(|| match value {
                        PropValue::Int(value) => Some(*value),
                        _ => None,
                    })
                    .flatten()
            })
        };
        Some([
            component("Pitch").unwrap_or_default(),
            component("Yaw").unwrap_or_default(),
            component("Roll").unwrap_or_default(),
        ])
    }
    fn actor_object_property(&self, actor: ObjectId, property: &str) -> Option<ObjectId> {
        let property = self.world.arena.names.find_index(property)?;
        match self.actor_effective_value(actor, property).ok()?? {
            PropValue::Object(Some(raw)) if raw >= 0 => Some(ObjectId(raw as u32)),
            _ => None,
        }
    }



    fn actor_is_interpolating(&self, actor: ObjectId) -> Result<bool> {
        let Some(name) = self.world.arena.names.find_index("bInterpolating") else {
            return Ok(false);
        };
        Ok(matches!(
            self.actor_effective_value(actor, name)?,
            Some(PropValue::Bool(true))
        ))
    }

    fn actor_effective_value(&self, actor: ObjectId, name: u32) -> Result<Option<PropValue>> {
        let object = self.world.arena.get(actor)?;
        if let Some(value) = object.properties().and_then(|store| store.get(name)) {
            return Ok(Some(value.clone()));
        }
        let Some(class_id) = object.class_id else {
            return Ok(None);
        };
        for class in self.world.arena.class_chain(class_id)? {
            let default_object = match &self.world.arena.get(class)?.data {
                ObjectData::Class(data) => data.default_object,
                _ => None,
            };
            if let Some(value) = default_object
                .and_then(|id| self.world.arena.get(id).ok())
                .and_then(|object| object.properties())
                .and_then(|store| store.get(name))
            {
                return Ok(Some(value.clone()));
            }
        }
        Ok(None)
    }

    fn copy_anim_channel_properties(&mut self, actor: ObjectId, channel: ObjectId) -> Result<()> {
        let mut copied = Vec::with_capacity(3);
        for property in ["Rotation", "Mesh", "SkelAnim"] {
            if let Some(name) = self.world.arena.names.find_index(property)
                && let Some(value) = self.actor_effective_value(actor, name)?
            {
                copied.push((name, value));
            }
        }
        let props = self
            .world
            .arena
            .get_mut(channel)?
            .properties_mut()
            .ok_or_else(|| {
                EngineError::new("native.anim_channel_object_invalid", format!("{channel:?}"))
            })?;
        for (name, value) in copied {
            props.set(name, value);
        }
        Ok(())
    }

    fn spawn_script_actor(
        &mut self,
        class: ObjectId,
        owner: Option<ObjectId>,
        location: [f32; 3],
        rotation: [i32; 3],
    ) -> Result<ObjectId> {
        self.spawn_script_actor_inner(class, owner, location, rotation, true)
    }

    fn spawn_persistent_actor(
        &mut self,
        class: ObjectId,
        location: [f32; 3],
        rotation: [i32; 3],
    ) -> Result<ObjectId> {
        self.spawn_script_actor_inner(class, None, location, rotation, false)
    }

    fn spawn_script_actor_inner(
        &mut self,
        class: ObjectId,
        owner: Option<ObjectId>,
        location: [f32; 3],
        rotation: [i32; 3],
        initialize: bool,
    ) -> Result<ObjectId> {
        let actor_class = self
            .world
            .arena
            .find_by_path("Engine.Actor")
            .ok_or_else(|| EngineError::new("native.spawn_actor_class_missing", "Engine.Actor"))?;
        if !matches!(self.world.arena.get(class)?.data, ObjectData::Class(_))
            || !self.world.arena.class_is_a(class, actor_class)?
        {
            return Err(EngineError::new(
                "native.spawn_class_invalid",
                format!("class {class:?} is not an Engine.Actor class"),
            ));
        }
        let level_info = if let Some(level_name) = self.world.arena.names.find_index("Level") {
            Some(
                self.level
                    .actors
                    .iter()
                    .find_map(|candidate| {
                        match self
                            .world
                            .arena
                            .get(*candidate)
                            .ok()?
                            .properties()?
                            .get(level_name)?
                        {
                            PropValue::Object(Some(raw)) if *raw >= 0 => {
                                Some(ObjectId(*raw as u32))
                            }
                            _ => None,
                        }
                    })
                    .ok_or_else(|| {
                        EngineError::new(
                            "native.spawn_level_info_missing",
                            "active level has no runtime LevelInfo binding",
                        )
                    })?,
            )
        } else {
            None
        };
        let defaults = match &self.world.arena.get(class)?.data {
            ObjectData::Class(class) => class
                .default_object
                .and_then(|id| self.world.arena.get(id).ok())
                .and_then(|object| object.properties())
                .cloned()
                .unwrap_or_default(),
            _ => unreachable!("validated class above"),
        };
        let name_index = self
            .world
            .arena
            .names
            .intern(&format!("ScriptSpawn_{}", self.world.arena.len()));
        let actor = self.world.arena.alloc(UObject {
            class_id: Some(class),
            name_index,
            outer: Some(self.level.root),
            flags: 0,
            data: ObjectData::Properties(defaults),
        });
        self.level.actors.push(actor);
        self.set_actor_object(actor, "Owner", owner)?;
        self.set_actor_vector(actor, "Location", location)?;
        self.set_actor_rotator(actor, "Rotation", rotation)?;
        if let Some(level_info) = level_info {
            self.set_actor_object(actor, "Level", Some(level_info))?;
        }
        if let Some(level_info) = level_info
            && let Some(player_pawn_class) = self.world.arena.find_by_path("Engine.PlayerPawn")
            && self.world.arena.class_is_a(class, player_pawn_class)?
        {
            self.set_actor_object(level_info, "PlayerHarryActor", Some(actor))?;
        }
        if initialize {
            self.run_spawn_startup(actor)?;
        }
        Ok(actor)
    }

    fn destroy_script_actor(&mut self, actor: ObjectId) {
        if !self.level.actors.contains(&actor) {
            return;
        }
        self.destroyed_actors.insert(actor);
        self.level.actors.retain(|candidate| *candidate != actor);
        self.actor_scripts.remove(&actor);
        self.actor_states.remove(&actor);
        self.disabled_events.remove(&actor);
        self.timers.retain(|timer| timer.actor != actor);
        self.actor_animations.remove(&actor);
    }

    /// Deliver `AActor::MakeNoise` to every other active Pawn in the loaded
    /// level's export/ObjectId order. The candidate list is snapshotted before
    /// invoking any listener so listener-side effects cannot reorder this
    /// native's dispatch.
    fn dispatch_make_noise(&mut self, source: ObjectId, loudness: f32) -> Result<()> {
        self.require_active_actor(source, "make_noise")?;
        let pawn_class = self.world.arena.find_by_path("Engine.Pawn").ok_or_else(|| {
            EngineError::new(
                "native.make_noise_pawn_class_missing",
                "Engine.Pawn is unavailable",
            )
        })?;
        let listeners: Vec<ObjectId> = self
            .level
            .actors
            .iter()
            .copied()
            .filter(|listener| {
                *listener != source
                    && !self.destroyed_actors.contains(listener)
                    && self
                        .world
                        .arena
                        .get(*listener)
                        .ok()
                        .and_then(|object| object.class_id)
                        .is_some_and(|class| {
                            self.world.arena.class_is_a(class, pawn_class).unwrap_or(false)
                        })
            })
            .collect();
        let arguments = [
            PropValue::Float(loudness),
            PropValue::Object(Some(source.0 as i32)),
        ];
        for listener in listeners {
            let Some(function) = self.resolve_actor_event_function(listener, "HearNoise")? else {
                continue;
            };
            self.run_function_frame(listener, function, "HearNoise", None, &arguments)?;
        }
        Ok(())
    }

    fn dispatch_trigger_event(
        &mut self,
        name: hp_uobject::name::Name,
        other: Option<ObjectId>,
        instigator: Option<ObjectId>,
    ) -> Result<()> {
        if name.is_none() {
            return Ok(());
        }
        let targets: Vec<ObjectId> = self
            .level
            .actors
            .iter()
            .copied()
            .filter(|actor| self.actor_has_tag(*actor, name))
            .collect();
        let arguments = [
            PropValue::Object(other.map(|value| value.0 as i32)),
            PropValue::Object(instigator.map(|value| value.0 as i32)),
        ];
        for target in targets {
            let Some(function) = self.resolve_actor_event_function(target, "Trigger")? else {
                continue;
            };
            self.run_function_frame(target, function, "Trigger", None, &arguments)?;
        }
        Ok(())
    }

    fn actor_has_tag(&self, actor: ObjectId, expected: hp_uobject::name::Name) -> bool {
        let Some(tag_name) = self.world.arena.names.find_index("Tag") else {
            return false;
        };
        let Ok(Some(PropValue::Name(candidate))) = self.actor_effective_value(actor, tag_name)
        else {
            return false;
        };
        if candidate == expected {
            return true;
        }
        if candidate.number != expected.number {
            return false;
        }
        let Some(expected_text) = self.world.arena.names.text(expected.index) else {
            return false;
        };
        // Instance names originate in the level linker's local table while
        // inherited defaults already carry process-global FName indices.
        // Accept either provenance textually; an index collision alone is
        // never treated as identity.
        self.level
            .archive
            .as_ref()
            .and_then(|archive| archive.names.get(candidate.index as usize))
            .is_some_and(|entry| entry.text.eq_ignore_ascii_case(expected_text))
            || self
                .world
                .arena
                .names
                .text(candidate.index)
                .is_some_and(|text| text.eq_ignore_ascii_case(expected_text))
    }

    /// Install one `SetTimer` schedule. A later call for the same actor
    /// overrides its prior schedule (retail behavior); non-positive
    /// intervals clamp to "due now" and fire next pass.
    fn schedule_timer(&mut self, actor: ObjectId, seconds: f32, repeating: bool) {
        self.timers.retain(|entry| entry.actor != actor);
        self.counters.set_timers += 1;
        let interval = seconds.max(0.0);
        self.timers.push(TimerEntry {
            actor,
            at: self.sim_time + f64::from(interval),
            interval,
            repeating,
        });
    }

    /// Replace `actor`'s current state frame. Resolution and `Begin:`
    /// decoding complete before the old continuation is discarded, so a
    /// missing/unreadable target fails loudly with a `vm.state_*` error
    /// instead of silently stranding the actor.
    fn find_state_event_function(
        &self,
        mut state_id: ObjectId,
        event: &str,
    ) -> Result<Option<ObjectId>> {
        let wanted = hp_uobject::vm::fold_key(event);
        loop {
            for (id, object) in self.world.arena.children_of(state_id) {
                if matches!(object.data, ObjectData::Function(_))
                    && self
                        .world
                        .arena
                        .names
                        .text(object.name_index)
                        .is_some_and(|name| hp_uobject::vm::fold_key(name) == wanted)
                {
                    return Ok(Some(id));
                }
            }
            let ObjectData::State(state) = &self.world.arena.get(state_id)?.data else {
                return Ok(None);
            };
            let Some(super_state) = state.links.super_field else {
                return Ok(None);
            };
            state_id = super_state;
        }
    }

    fn enter_state(&mut self, actor: ObjectId, change: StateChange) -> Result<()> {
        if change == StateChange::None {
            self.actor_scripts.remove(&actor);
            self.actor_states.remove(&actor);
            self.disabled_events.remove(&actor);
            return Ok(());
        }
        let Some(class_id) = self.world.arena.get(actor)?.class_id else {
            return Err(EngineError::from(hp_uobject::error::Fail::new(
                "vm.state_class_missing",
                "GotoState target has no class chain",
            )));
        };
        let current_state = self.actor_states.get(&actor).copied();
        let resolved: Option<(ObjectId, Box<hp_uobject::arena::StateData>)> = match &change {
            StateChange::Current => {
                let Some(state_id) = current_state else {
                    self.actor_scripts.remove(&actor);
                    return Ok(());
                };
                match &self.world.arena.get(state_id)?.data {
                    ObjectData::State(state) => Some((state_id, state.clone())),
                    _ => {
                        return Err(EngineError::from(hp_uobject::error::Fail::new(
                            "vm.state_identity_invalid",
                            format!("active state {state_id:?} is not a state object"),
                        )));
                    }
                }
            }
            StateChange::Auto | StateChange::Named(_) => {
                let wanted = match &change {
                    StateChange::Named(name) => Some(hp_uobject::vm::fold_key(name)),
                    StateChange::Auto => None,
                    _ => unreachable!(),
                };
                let mut found = None;
                {
                    let arena = &self.world.arena;
                    'chain: for class in arena.class_chain(class_id)? {
                        for (id, object) in arena.children_of(class) {
                            let ObjectData::State(state) = &object.data else {
                                continue;
                            };
                            let matches = match &wanted {
                                Some(name) => arena
                                    .names
                                    .text(object.name_index)
                                    .is_some_and(|text| hp_uobject::vm::fold_key(text) == *name),
                                None => state.state_flags & 0x0000_0002 != 0,
                            };
                            if matches {
                                found = Some((id, state.clone()));
                                break 'chain;
                            }
                        }
                    }
                }
                found
            }
            StateChange::None => unreachable!(),
        };
        let Some((state_id, state)) = resolved else {
            // UObject::GotoState sends every unresolved target to the class
            // (no-state) frame and returns GOTOSTATE_NotFound. This includes
            // authored InitialState names unavailable on a particular class,
            // not only NAME_Auto with no auto state.
            self.actor_scripts.remove(&actor);
            self.actor_states.remove(&actor);
            self.disabled_events.remove(&actor);
            return Ok(());
        };
        if self.deferred_states.contains(&(class_id, state_id)) {
            return Err(EngineError::from(hp_uobject::error::Fail::new(
                "vm.state_previously_deferred",
                format!("state {state_id:?} previously failed during this run"),
            )));
        }
        // MAXWORD means the compiler emitted no label table. Retail still
        // enters the state (GetStateName/IsInState change) but leaves
        // StateFrame->Code null. Likewise a valid table without `Begin`
        // yields idle state membership rather than an offset-zero frame.
        let entry = if state.label_table_offset == u16::MAX {
            None
        } else {
            match state_entry_pc(
                &self.world.arena,
                &state.resolver,
                &state.code,
                usize::from(state.label_table_offset),
            ) {
                Ok(entry) => Some(entry),
                Err(fail) if fail.reason_code == "vm.state_begin_missing" => None,
                Err(fail) => {
                    return Err(EngineError::from(fail).annotate(
                        actor,
                        &self.world.arena,
                        "state entry",
                    ));
                }
            }
        };

        // All validation succeeded: abandon the old activation and establish
        // membership before BeginState, exactly like UObject::GotoState.
        self.actor_scripts.remove(&actor);
        self.disabled_events.remove(&actor);
        self.actor_states.insert(actor, state_id);
        self.counters.states_entered += 1;

        // Retail invokes the new state's BeginState notification with Code
        // still null. It may immediately issue another GotoState; in that
        // case the nested transition owns the resulting frame.
        if current_state != Some(state_id)
            && let Some(begin_state) = self.find_state_event_function(state_id, "BeginState")?
        {
            self.run_function_frame(actor, begin_state, "BeginState", None, &[])?;
            if self.actor_states.get(&actor).copied() != Some(state_id) {
                return Ok(());
            }
        }
        if let Some(entry) = entry {
            self.actor_scripts.insert(
                actor,
                ScriptFrame {
                    code: state.code,
                    resolver: state.resolver,
                    pc: entry,
                    self_id: actor,
                    state: Some(state_id),
                    locals: PropStore::new(),
                    nested_call: None,
                    iterators: Vec::new(),
                    wake: Wake::Ready,
                    origin: None,
                },
            );
        }
        Ok(())
    }

    /// Name-pool index of the state `actor` currently belongs to (the
    /// `GetStateName`/`IsInState` view), independent of whether any of
    /// its code is parked right now.
    fn active_state_name(&self, actor: ObjectId) -> Option<u32> {
        let state_id = self.actor_states.get(&actor).copied()?;
        self.world.arena.get(state_id).ok().map(|o| o.name_index)
    }

    /// Feed a whole parsed script's frames through the tick loop.
    pub fn run_script_ticks(
        &mut self,
        script: &InputScript,
        extra_frames: usize,
        mut on_frame: impl FnMut(u64, f64),
    ) -> Result<()> {
        for (index, frame) in script.frames().iter().enumerate() {
            let started = std::time::Instant::now();
            let events: Vec<(u8, u8, f32)> = frame
                .iter()
                .map(|&(key, act)| (key, act, script.tick_delta))
                .collect();

            self.tick(&events)?;
            on_frame(index as u64 + 1, started.elapsed().as_secs_f64() * 1000.0);
        }
        let base = script.frames().len() as u64;
        for offset in 0..extra_frames {
            let started = std::time::Instant::now();
            self.tick(&[])?;
            on_frame(
                base + offset as u64 + 1,
                started.elapsed().as_secs_f64() * 1000.0,
            );
        }
        Ok(())
    }

    pub fn counters(&self) -> RunCounters {
        self.counters
    }

    pub fn script_deferral_reason_codes(&self) -> &HashSet<&'static str> {
        &self.deferral_reason_codes
    }

    /// Deferred native ABI entries observed in this map run. Keys begin with
    /// `slot=0xNNNN` and include the registry's resolved function path.
    pub fn native_deferral_census(&self) -> &BTreeMap<String, u64> {
        &self.native_deferral_census
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }
}

impl Engine {
    /// Failure classes covered by the accepted `engine.script_event_deferred`
    /// policy: constructs the VM deliberately defers (iterator state,
    /// unbound/deferred native bodies, conversions or assignments without
    /// runtime support, and operand walks that outrun a body cut). Everything
    /// else — unknown opcodes, bad data — aborts.
    fn is_deferred_reason(reason_code: &str) -> bool {
        reason_code == "vm.token_unsupported"
            || reason_code == "vm.unknown_token_deferrable"
            || reason_code == "vm.lvalue_unsupported"
            || reason_code == "vm.conversion_unsupported"
            || reason_code == "vm.code_truncated"
            || reason_code == "vm.function_unresolved"
            || reason_code == "vm.array_out_of_bounds"
            || reason_code.starts_with("native.")
    }
}

fn sim_rand_range(rng: &mut SimRng, min: f32, max: f32) -> f32 {
    min + (max - min) * rng.next_f32()
}

fn vector_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let delta = sub3(b, a);
    dot3(delta, delta).sqrt()
}

fn spline_ease_alpha(alpha: f32, ease: SplineEase, window: f32) -> f32 {
    if window <= f32::EPSILON || ease == SplineEase::Linear {
        return alpha;
    }
    let accelerate = |value: f32| {
        let total = 1.0 - window / 2.0;
        if value < window {
            value * value / (2.0 * window * total)
        } else {
            (value - window / 2.0) / total
        }
    };
    match ease {
        SplineEase::Linear => alpha,
        SplineEase::From => accelerate(alpha),
        SplineEase::To => 1.0 - accelerate(1.0 - alpha),
        SplineEase::Between => {
            let cruise = 1.0 - window;
            if alpha < window {
                alpha * alpha / (2.0 * window * cruise)
            } else if alpha > 1.0 - window {
                let remaining = 1.0 - alpha;
                1.0 - remaining * remaining / (2.0 * window * cruise)
            } else {
                (alpha - window / 2.0) / cruise
            }
        }
    }
    .clamp(0.0, 1.0)
}

/// Deterministic immediate-facing rotation for S3's initially non-latent
/// `TurnTo`/`TurnToward` effects. UE rotators use 65536 units per turn.
fn focus_rotation(location: [f32; 3], focus: [f32; 3]) -> [i32; 3] {
    let dx = focus[0] - location[0];
    let dy = focus[1] - location[1];
    let dz = focus[2] - location[2];
    let planar = (dx * dx + dy * dy).sqrt();
    let units = 65536.0 / std::f32::consts::TAU;
    [
        (-dz.atan2(planar) * units).round() as i32,
        (dy.atan2(dx) * units).round() as i32,
        0,
    ]
}

impl EngineError {
    fn annotate(mut self, actor: ObjectId, arena: &ObjectArena, event: &str) -> Self {
        let subject = arena
            .path_of(actor)
            .unwrap_or_else(|_| format!("{actor:?}"));
        self.message = format!("{}, while running {event} on {subject}", self.message);
        self
    }
}
fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn mul3(value: [f32; 3], scale: f32) -> [f32; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn trace_vector_value(arena: &ObjectArena, value: [f32; 3]) -> Result<PropValue> {
    let struct_name = arena.names.find_index("Vector").ok_or_else(|| {
        EngineError::new(
            "native.trace_collision_unavailable",
            "Core.Vector name is unavailable for TraceActors results",
        )
    })?;
    let fields = ["X", "Y", "Z"]
        .into_iter()
        .map(|name| {
            arena.names.find_index(name).ok_or_else(|| {
                EngineError::new(
                    "native.trace_collision_unavailable",
                    format!("Core.Vector.{name} is unavailable for TraceActors results"),
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PropValue::Struct {
        struct_name,
        fields: fields
            .into_iter()
            .zip(value)
            .map(|(name, component)| (name, PropValue::Float(component)))
            .collect(),
    })
}

/// Earliest contact against the CollisionWorld's shared plane+AABB geometry.
fn trace_level_polys(
    polys: &[CollisionPoly],
    start: [f32; 3],
    end: [f32; 3],
    extent: [f32; 3],
) -> Option<(f32, [f32; 3], [f32; 3])> {
    let delta = sub3(end, start);
    let mut best: Option<(f32, [f32; 3], [f32; 3])> = None;
    for poly in polys {
        let support = poly.normal[0].abs() * extent[0]
            + poly.normal[1].abs() * extent[1]
            + poly.normal[2].abs() * extent[2];
        let start_dist = dot3(poly.normal, sub3(start, poly.base)) - support;
        let end_dist = dot3(poly.normal, sub3(end, poly.base)) - support;
        if start_dist < 0.0 || end_dist >= 0.0 || start_dist <= end_dist {
            continue;
        }
        let time = start_dist / (start_dist - end_dist);
        let location = add3(start, mul3(delta, time));
        if (0..3).any(|axis| {
            location[axis] < poly.aabb[0][axis] - extent[axis] - 1.0
                || location[axis] > poly.aabb[1][axis] + extent[axis] + 1.0
        }) {
            continue;
        }
        if best.is_none_or(|(old, _, _)| time < old) {
            best = Some((time, location, poly.normal));
        }
    }
    best
}

/// `AActor::FastTrace` mirrors `UModel::FastLineCheck`: it ignores actors
/// and succeeds only when the level-model line has no blocking polygon.
fn fast_trace_clear(polys: &[CollisionPoly], start: [f32; 3], end: [f32; 3]) -> bool {
    trace_level_polys(polys, start, end, [0.0; 3]).is_none()
}

/// Segment against the upright collision cylinder used by UE1 actors.
fn trace_actor_cylinder(
    start: [f32; 3],
    end: [f32; 3],
    extent: [f32; 3],
    center: [f32; 3],
    radius: f32,
    half_height: f32,
) -> Option<(f32, [f32; 3], [f32; 3])> {
    let radius = radius.max(0.0) + extent[0].max(extent[1]).max(0.0);
    let half_height = half_height.max(0.0) + extent[2].max(0.0);
    if radius == 0.0 && half_height == 0.0 {
        return None;
    }
    let delta = sub3(end, start);
    let rel = sub3(start, center);
    let mut candidates = Vec::with_capacity(4);
    let a = delta[0] * delta[0] + delta[1] * delta[1];
    let b = 2.0 * (rel[0] * delta[0] + rel[1] * delta[1]);
    let c = rel[0] * rel[0] + rel[1] * rel[1] - radius * radius;
    if a > f32::EPSILON {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let root = disc.sqrt();
            candidates.push(((-b - root) / (2.0 * a), false));
            candidates.push(((-b + root) / (2.0 * a), false));
        }
    }
    if delta[2].abs() > f32::EPSILON {
        candidates.push(((center[2] - half_height - start[2]) / delta[2], true));
        candidates.push(((center[2] + half_height - start[2]) / delta[2], true));
    }
    candidates.sort_by(|left, right| left.0.total_cmp(&right.0));
    for (time, cap) in candidates {
        if !(0.0..=1.0).contains(&time) {
            continue;
        }
        let location = add3(start, mul3(delta, time));
        let dx = location[0] - center[0];
        let dy = location[1] - center[1];
        if cap {
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            return Some((
                time,
                location,
                [0.0, 0.0, if location[2] < center[2] { -1.0 } else { 1.0 }],
            ));
        }
        if (location[2] - center[2]).abs() > half_height {
            continue;
        }
        let length = (dx * dx + dy * dy).sqrt();
        if length > f32::EPSILON {
            return Some((time, location, [dx / length, dy / length, 0.0]));
        }
    }
    None
}

/// Trigger classes whose cylinder-enter fires a level exit; matched
/// case-insensitively against the actor's class name. HP2's shipped exit
/// actor is `TriggerChangeLevel` (HGame/Classes/Triggers/
/// TriggerChangeLevel.uc) — the generic `Trigger` class fires on gameplay
/// events and must NOT exit the level, so it is deliberately excluded.
const TRIGGER_CLASSES: [&str; 1] = ["triggerchangelevel"];

/// Instance-store `Vector` property of one actor (`Location`): the same
/// X/Y/Z field walk the render scene uses, kept local so sim does not
/// depend on scene.rs internals.
fn actor_vec3(store: &PropStore, arena: &ObjectArena, prop: &str) -> Option<[f32; 3]> {
    let value = store.get(arena.names.find_index(prop)?)?;
    let PropValue::Struct { fields, .. } = value else {
        return None;
    };
    let mut out = [0.0f32; 3];
    for (field_index, field_value) in fields {
        let slot = match arena
            .names
            .text(*field_index)?
            .to_ascii_uppercase()
            .as_str()
        {
            "X" => 0,
            "Y" => 1,
            "Z" => 2,
            _ => continue,
        };
        if let PropValue::Float(f) = field_value {
            out[slot] = *f;
        }
    }
    Some(out)
}

/// Numeric view of one resolved scalar property (`CollisionRadius` and
/// friends arrive as Byte/Int/Float depending on declaring class).
fn prop_scalar(prop: ResolvedProp) -> Option<f32> {
    match prop {
        ResolvedProp::Byte(b) => Some(f32::from(b)),
        ResolvedProp::Float(f) => Some(f),
        ResolvedProp::Int(i) => Some(i as f32),
        _ => None,
    }
}

fn prop_bool(value: Option<&PropValue>) -> bool {
    match value {
        Some(PropValue::Bool(value)) => *value,
        Some(PropValue::Byte(value)) => *value != 0,
        Some(PropValue::Int(value)) => *value != 0,
        _ => false,
    }
}

fn anim_bone_is_subset(bone: usize, root: usize, num_children: usize) -> bool {
    bone >= root && bone < root.saturating_add(num_children)
}

/// Root import stems (`outer_ref == 0` entries) of one archive, sorted
/// case-insensitively so closure traversal is deterministic.
fn root_import_stems(archive: &hp_format::package79::PackageArchive) -> Vec<String> {
    let mut stems: Vec<String> = archive
        .imports
        .iter()
        .filter(|entry| entry.outer_ref == 0)
        .filter_map(|entry| {
            archive
                .names
                .get(entry.object_name_index.max(0) as usize)
                .map(|name| name.text.clone())
        })
        .collect();
    stems.sort_by_key(|stem| stem.to_lowercase());
    stems
}

fn skeletal_has_anim(metadata: &SkeletalAssetMetadata, sequence: &str) -> bool {
    metadata
        .animation_names
        .contains(&hp_uobject::vm::fold_key(sequence))
}
fn skeletal_has_anim_selected(
    metadata: &SkeletalAssetMetadata,
    override_sequences: Option<&HashSet<String>>,
    sequence: &str,
) -> bool {
    if metadata.skeletal {
        override_sequences.map_or_else(
            || skeletal_has_anim(metadata, sequence),
            |names| names.contains(&hp_uobject::vm::fold_key(sequence)),
        )
    } else {
        skeletal_has_anim(metadata, sequence)
    }
}

fn skeletal_bone_number(metadata: &SkeletalAssetMetadata, bone: &str) -> i32 {
    if !metadata.skeletal {
        return 0;
    }
    metadata
        .bone_names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(bone))
        .map(|index| index as i32)
        .unwrap_or(-1)
}

fn skeletal_metadata_error(message: impl Into<String>) -> EngineError {
    EngineError::new("native.skeletal_metadata_unavailable", message)
}

fn mesh_wire<T>(result: hp_format::package79::PackageResult<T>) -> Result<T> {
    result.map_err(|error| skeletal_metadata_error(error.to_string()))
}

fn mesh_count(cur: &mut ByteCursor<'_>) -> Result<usize> {
    let at = cur.position();
    let count = read_compact_index(cur)
        .map_err(|error| skeletal_metadata_error(format!("mesh count at byte {at}: {error}")))?;
    usize::try_from(count)
        .map_err(|_| skeletal_metadata_error(format!("negative mesh array count {count}")))
}

fn skip_mesh_records(cur: &mut ByteCursor<'_>, count: usize, size: usize) -> Result<()> {
    let bytes = count
        .checked_mul(size)
        .ok_or_else(|| skeletal_metadata_error("mesh array byte size overflow"))?;
    mesh_wire(cur.take(bytes)).map(|_| ())
}

fn skip_lazy_mesh_records(cur: &mut ByteCursor<'_>, version: i32, size: usize) -> Result<()> {
    if version > 61 {
        mesh_wire(cur.take(4))?;
    }
    let count = mesh_count(cur)?;
    skip_mesh_records(cur, count, size)
}

/// Read the RefSkeleton names/subtree sizes and DefaultAnimation reference
/// from an already fully validated skeletal payload. This mirrors the
/// serialized field walk but retains no geometry, weights, or poses.
fn skeletal_tail_metadata(
    payload: &[u8],
    archive: &PackageArchive,
) -> Result<(Vec<String>, Vec<usize>, i32)> {
    let version = archive.summary.version;
    let mut cur = ByteCursor::new(payload);
    mesh_wire(read_property_tags(&mut cur, &archive.names))?;
    skip_mesh_records(&mut cur, 1, 25)?;
    skip_mesh_records(&mut cur, 1, if version > 61 { 16 } else { 12 })?;
    skip_lazy_mesh_records(&mut cur, version, 4)?;
    skip_lazy_mesh_records(&mut cur, version, 20)?;

    let sequence_count = mesh_count(&mut cur)?;
    for _ in 0..sequence_count {
        mesh_wire(read_compact_index(&mut cur))?;
        mesh_wire(read_compact_index(&mut cur))?;
        skip_mesh_records(&mut cur, 1, 8)?;
        let notification_count = mesh_count(&mut cur)?;
        for _ in 0..notification_count {
            skip_mesh_records(&mut cur, 1, 4)?;
            mesh_wire(read_compact_index(&mut cur))?;
        }
        skip_mesh_records(&mut cur, 1, 4)?;
    }
    skip_lazy_mesh_records(&mut cur, version, 8)?;
    skip_mesh_records(&mut cur, 1, 25)?;
    skip_mesh_records(&mut cur, 1, if version > 61 { 16 } else { 12 })?;
    skip_lazy_mesh_records(&mut cur, version, 4)?;
    let texture_count = mesh_count(&mut cur)?;
    for _ in 0..texture_count {
        mesh_wire(read_compact_index(&mut cur))?;
    }
    let boxes = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, boxes, 25)?;
    let spheres = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, spheres, if version > 61 { 16 } else { 12 })?;
    skip_mesh_records(&mut cur, 1, 60)?;
    match version {
        65 => skip_mesh_records(&mut cur, 1, 4)?,
        66.. => {
            let count = mesh_count(&mut cur)?;
            skip_mesh_records(&mut cur, count, 4)?;
        }
        _ => {}
    }

    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 2)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 2)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 8)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 2)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 4)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 8)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 8)?;
    skip_mesh_records(&mut cur, 1, 32)?;
    let count = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, count, 2)?;
    skip_mesh_records(&mut cur, 1, 4)?;

    let extended = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, extended, 12)?;
    let points = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, points, 12)?;
    let bone_count = mesh_count(&mut cur)?;
    let mut bone_names = Vec::with_capacity(bone_count.min(1024));
    let mut bone_children = Vec::with_capacity(bone_count.min(1024));
    for _ in 0..bone_count {
        let name = mesh_wire(read_compact_index(&mut cur))?;
        let name = usize::try_from(name)
            .ok()
            .and_then(|index| archive.names.get(index))
            .ok_or_else(|| {
                skeletal_metadata_error(format!("invalid skeleton name index {name}"))
            })?;
        bone_names.push(name.text.clone());
        // FMeshBone serializes Flags, BonePos, NumChildren, ParentIndex.
        skip_mesh_records(&mut cur, 1, 48)?;
        let children = mesh_wire(cur.take(4))?;
        bone_children.push(u32::from_le_bytes(children.try_into().expect("four bytes")) as usize);
        skip_mesh_records(&mut cur, 1, 4)?;
    }
    let weight_indices = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, weight_indices, 8)?;
    let weights = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, weights, 4)?;
    let local_points = mesh_count(&mut cur)?;
    skip_mesh_records(&mut cur, local_points, 12)?;
    skip_mesh_records(&mut cur, 1, 4)?;
    let default_animation = mesh_wire(read_compact_index(&mut cur))?;
    Ok((bone_names, bone_children, default_animation))
}

const CUTSCRIPT_DISK_LINE_CAP: usize = 4096;

fn cutscript_disk_line_in_bounds(package: &str, key: &str) -> bool {
    let is_cutscene = package
        .replace('\\', "/")
        .split('/')
        .any(|component| component.eq_ignore_ascii_case("Cutscenes"));
    if !is_cutscene {
        return true;
    }
    let Some(suffix) = key
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("line_"))
        .then(|| key.get(5..))
        .flatten()
    else {
        return true;
    };
    suffix
        .parse::<usize>()
        .map_or(true, |index| index < CUTSCRIPT_DISK_LINE_CAP)
}

fn animation_finish_delay(animation: &ActorAnimation) -> Option<f64> {
    let finish = f64::from(animation.finish_after_seconds?);
    if !finish.is_finite() || finish < 0.0 {
        return None;
    }
    Some((finish - f64::from(animation.elapsed_seconds.max(0.0))).max(0.0))
}

// The named packages embed the corresponding `#exec ANIM IMPORT`
// declarations; these package/object/PSA joins are data-authored rather
// than stem guesses.
const VERIFIED_ANIMATION_IMPORTS: [(&str, &str, &str); 4] = [
    ("HPModels", "skHarryAnims", "skHarryAnim"),
    ("HPModels", "skGenMaleAnims", "skgenmale"),
    ("HPModels", "skdobbyAnims", "skdobby"),
    ("HProps", "skArmTorchAnims", "skArmTorch"),
];

fn unique_psa_path(
    data_root: &Path,
    source_package: &str,
    animation_stem: &str,
) -> Option<PathBuf> {
    let psa_stem = VERIFIED_ANIMATION_IMPORTS
        .iter()
        .find(|(package, animation, _)| {
            package.eq_ignore_ascii_case(source_package)
                && animation.eq_ignore_ascii_case(animation_stem)
        })
        .map_or(animation_stem, |(_, _, psa)| *psa);
    let mut matches = std::fs::read_dir(data_root.join(source_package).join("Models"))
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("psa"))
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.eq_ignore_ascii_case(psa_stem))
        });
    let path = matches.next()?;
    matches.next().is_none().then_some(path)
}

fn load_psa_sequence_names(
    data_root: &Path,
    source_package: &str,
    animation_stem: &str,
) -> Result<Option<HashSet<String>>> {
    Ok(load_psa_sequence_metadata(data_root, source_package, animation_stem)?
        .map(|metadata| metadata.sequence_names))
}

fn load_psa_sequence_metadata(
    data_root: &Path,
    source_package: &str,
    animation_stem: &str,
) -> Result<Option<AnimationAssetMetadata>> {
    let Some(psa_path) = unique_psa_path(data_root, source_package, animation_stem) else {
        return Ok(None);
    };
    let bytes = std::fs::read(&psa_path)
        .map_err(|error| skeletal_metadata_error(format!("{}: {error}", psa_path.display())))?;
    let psa = read_psa(&bytes)
        .map_err(|error| skeletal_metadata_error(format!("{}: {error}", psa_path.display())))?;
    let mut metadata = AnimationAssetMetadata::default();
    for sequence in psa.sequences {
        let name = hp_uobject::vm::fold_key(&sequence.name);
        metadata.sequence_names.insert(name.clone());
        metadata.sequence_groups.insert(name, sequence.group);
    }
    Ok(Some(metadata))
}

fn load_psa_sequence_timing(
    data_root: &Path,
    source_package: &str,
    animation_stem: &str,
    sequence: &str,
) -> Result<Option<(f32, f32)>> {
    let Some(path) = unique_psa_path(data_root, source_package, animation_stem) else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path)
        .map_err(|error| skeletal_metadata_error(format!("{}: {error}", path.display())))?;
    let psa = read_psa(&bytes)
        .map_err(|error| skeletal_metadata_error(format!("{}: {error}", path.display())))?;
    Ok(psa
        .sequences
        .iter()
        .find(|anim| anim.name.eq_ignore_ascii_case(sequence))
        .filter(|anim| anim.num_raw_frames > 0 && anim.anim_rate > 0.0)
        .map(|anim| {
            (
                anim.num_raw_frames as f32 / anim.anim_rate,
                anim.num_raw_frames.saturating_sub(1) as f32 / anim.anim_rate,
            )
        }))
}

fn load_skeletal_asset_metadata(
    data_root: &Path,
    source_package: &str,
    archive: &PackageArchive,
    export_index: usize,
    class_name: &str,
) -> Result<SkeletalAssetMetadata> {
    let payload = archive.export_payload(export_index).ok_or_else(|| {
        skeletal_metadata_error(format!("mesh export {export_index} has no payload"))
    })?;
    let mesh = decode_export(class_name, payload, archive)
        .map_err(|error| skeletal_metadata_error(error.to_string()))?;
    let skeletal = class_name.eq_ignore_ascii_case("SkeletalMesh");
    let mut metadata = SkeletalAssetMetadata {
        skeletal,
        animation_names: if skeletal {
            HashSet::new()
        } else {
            mesh.animations
                .iter()
                .map(|sequence| hp_uobject::vm::fold_key(&sequence.name))
                .collect()
        },
        ..SkeletalAssetMetadata::default()
    };
    if !metadata.skeletal {
        return Ok(metadata);
    }
    let (bone_names, bone_children, default_animation) = skeletal_tail_metadata(payload, archive)?;
    metadata.bone_names = bone_names;
    metadata.bone_children = bone_children;
    if default_animation == 0 {
        return Ok(metadata);
    }
    let animation_label = mesh_object_label(archive, default_animation).ok_or_else(|| {
        skeletal_metadata_error(format!(
            "default animation reference {default_animation} is unresolved"
        ))
    })?;
    let animation_stem = animation_label
        .rsplit('.')
        .next()
        .unwrap_or(&animation_label);
    if let Some(sequences) = load_psa_sequence_names(data_root, source_package, animation_stem)? {
        metadata.animation_names.extend(sequences);
    }
    // A serialized RefSkeleton is sufficient for BoneNumber. When the
    // selected default animation has no companion PSA, HasAnim simply has no
    // companion sequences to find.
    Ok(metadata)
}

fn mesh_export_class(archive: &PackageArchive, export_index: usize) -> Option<&str> {
    let class_ref = archive.exports.get(export_index)?.class_ref;
    let name_index = if class_ref > 0 {
        archive
            .exports
            .get(class_ref as usize - 1)?
            .object_name_index
    } else if class_ref < 0 {
        archive
            .imports
            .get((-class_ref - 1) as usize)?
            .object_name_index
    } else {
        return None;
    };
    archive
        .names
        .get(name_index.max(0) as usize)
        .map(|name| name.text.as_str())
}

fn mesh_export_by_path(archive: &PackageArchive, path: &str) -> Option<usize> {
    let mut parts = path.split('.');
    let root = parts.next()?;
    let mut current = archive.exports.iter().position(|entry| {
        entry.outer_ref == 0
            && archive
                .names
                .get(entry.object_name_index.max(0) as usize)
                .is_some_and(|name| name.text.eq_ignore_ascii_case(root))
    })?;
    for part in parts {
        current = archive.exports.iter().position(|entry| {
            entry.outer_ref == current as i32 + 1
                && archive
                    .names
                    .get(entry.object_name_index.max(0) as usize)
                    .is_some_and(|name| name.text.eq_ignore_ascii_case(part))
        })?;
    }
    Some(current)
}

fn mesh_outer_chain(archive: &PackageArchive, mut reference: i32) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    for _ in 0..=256 {
        if reference == 0 {
            parts.reverse();
            return Some(parts);
        }
        let (name_index, outer) = if reference > 0 {
            let entry = archive.exports.get(reference as usize - 1)?;
            (entry.object_name_index, entry.outer_ref)
        } else {
            let entry = archive.imports.get((-reference - 1) as usize)?;
            (entry.object_name_index, entry.outer_ref)
        };
        parts.push(archive.names.get(name_index.max(0) as usize)?.text.clone());
        reference = outer;
    }
    None
}

fn mesh_object_label(archive: &PackageArchive, reference: i32) -> Option<String> {
    let parts = mesh_outer_chain(archive, reference)?;
    (!parts.is_empty()).then(|| parts.join("."))
}

fn mesh_object_ref_parts(
    archive: &PackageArchive,
    owner: &str,
    reference: i32,
) -> Option<(String, String)> {
    let mut parts = mesh_outer_chain(archive, reference)?;
    if reference > 0 {
        return Some((owner.to_string(), parts.join(".")));
    }
    if reference < 0 && parts.len() >= 2 {
        let package = parts.remove(0);
        return Some((package, parts.join(".")));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Level;
    use hp_uobject::arena::ClassData;
    use hp_uobject::natives::NativeRegistry;
    use hp_uobject::vm::USToken;

    const DT: f32 = 0.1;
    #[test]
    fn fast_trace_reports_blocked_and_clear_world_segments() {
        let wall = CollisionPoly {
            normal: [1.0, 0.0, 0.0],
            base: [0.0, 0.0, 0.0],
            aabb: [[0.0, -100.0, -100.0], [0.0, 100.0, 100.0]],
        };
        assert!(
            !fast_trace_clear(std::slice::from_ref(&wall), [10.0, 0.0, 0.0], [-10.0, 0.0, 0.0]),
            "front-to-back segment intersects the blocking level polygon"
        );
        assert!(
            fast_trace_clear(std::slice::from_ref(&wall), [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]),
            "segment remaining in front of the polygon is clear"
        );
    }

    #[test]
    fn trace_request_dispatches_world_collision_into_return_and_out_vectors() {
        use hp_uobject::natives::natives_impl::lookup;

        let Synth {
            mut engine,
            class,
            actor_a: source,
            ..
        } = synth_world();
        let wall = CollisionPoly {
            normal: [1.0, 0.0, 0.0],
            base: [0.0; 3],
            aabb: [[0.0, -100.0, -100.0], [0.0, 100.0, 100.0]],
        };
        engine.set_test_trace_collision_polys(vec![wall.clone()]);
        engine
            .world
            .registry
            .register(
                0x0115,
                2,
                lookup("Engine.Actor.Trace").expect("Trace native"),
                "Engine.Actor.Trace",
            )
            .expect("register Trace native slot");

        let root = engine.level.root;
        let actor_base = engine
            .world
            .arena
            .find_by_path("Engine.Actor")
            .expect("synthetic Actor class");
        let level_info_class_name = engine.world.arena.names.intern("LevelInfo");
        let level_info_class = engine.world.arena.alloc(UObject {
            name_index: level_info_class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData {
                super_class: Some(actor_base),
                ..Default::default()
            }),
            ..Default::default()
        });
        let level_info_name = engine.world.arena.names.intern("TraceLevelInfo");
        let level_info = engine.world.arena.alloc(UObject {
            class_id: Some(level_info_class),
            name_index: level_info_name,
            outer: Some(root),
            data: ObjectData::Properties(PropStore::new()),
            ..Default::default()
        });
        engine.level.actors.push(level_info);
        let core = engine.world.arena.root_for("Core");
        let vector_name = engine.world.arena.names.intern("Vector");
        let x_name = engine.world.arena.names.intern("X");
        let y_name = engine.world.arena.names.intern("Y");
        let z_name = engine.world.arena.names.intern("Z");
        let vector_fields = [x_name, y_name, z_name]
            .into_iter()
            .map(|name_index| {
                engine.world.arena.alloc(UObject {
                    name_index,
                    outer: Some(core),
                    data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                        links: Default::default(),
                        kind: hp_uobject::value::PropertyKind::Float,
                        array_dim: 1,
                        property_flags: 0,
                        category: 0,
                    })),
                    ..Default::default()
                })
            })
            .collect();
        engine.world.arena.alloc(UObject {
            name_index: vector_name,
            outer: Some(core),
            data: ObjectData::ScriptStruct(hp_uobject::arena::StructData {
                children: vector_fields,
                ..Default::default()
            }),
            ..Default::default()
        });

        let property = |engine: &mut Engine, name: &str, kind| {
            let name_index = engine.world.arena.names.intern(name);
            let id = engine.world.arena.alloc(UObject {
                name_index,
                outer: Some(class),
                data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                    links: Default::default(),
                    kind,
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            });
            (name_index, id)
        };
        let (hit_actor_name, hit_actor) = property(
            &mut engine,
            "TraceHitActor",
            hp_uobject::value::PropertyKind::Object {
                class_ref: Some(actor_base),
            },
        );
        let (hit_location_name, hit_location) = property(
            &mut engine,
            "TraceHitLocation",
            hp_uobject::value::PropertyKind::Struct { struct_ref: None },
        );
        let (hit_normal_name, hit_normal) = property(
            &mut engine,
            "TraceHitNormal",
            hp_uobject::value::PropertyKind::Struct { struct_ref: None },
        );
        let resolver =
            BytecodeResolver::from_package_refs(Vec::new(), vec![hit_actor, hit_location, hit_normal]);
        let trace_code = |start: [f32; 3], end: [f32; 3]| {
            let vector = |value: [f32; 3]| {
                let mut bytes = vec![USToken::VectorConst as u8];
                for component in value {
                    bytes.extend_from_slice(&component.to_le_bytes());
                }
                bytes
            };
            let mut code = vec![
                USToken::Let as u8,
                USToken::InstanceVariable as u8,
                1,
                0x61,
                0x15,
                USToken::InstanceVariable as u8,
                2,
                USToken::InstanceVariable as u8,
                3,
            ];
            code.extend(vector(end));
            code.extend(vector(start));
            code.push(USToken::False as u8);
            code.extend(vector([0.0; 3]));
            code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
            code
        };
        let start = [64.0, 0.0, 0.0];
        let end = [-64.0, 0.0, 0.0];
        let (_, expected_location, expected_normal) =
            trace_level_polys(std::slice::from_ref(&wall), start, end, [0.0; 3])
                .expect("fixed wall blocks the ray");

        engine
            .execute_frame(
                source,
                trace_code(start, end),
                resolver.clone(),
                0,
                PropStore::new(),
                None,
                None,
                None,
                Vec::new(),
                None,
                Some("TraceRequestBlocked".to_string()),
                ScriptKey::Event(class, "TraceRequestBlocked".to_string()),
                "TraceRequestBlocked",
            )
            .expect("TraceRequest dispatch resumes blocked ray");
        let vector_value = |value: [f32; 3]| PropValue::Struct {
            struct_name: vector_name,
            fields: vec![
                (x_name, PropValue::Float(value[0])),
                (y_name, PropValue::Float(value[1])),
                (z_name, PropValue::Float(value[2])),
            ],
        };
        let props = engine
            .world
            .arena
            .get(source)
            .expect("trace source")
            .properties()
            .expect("trace source properties");
        assert_eq!(
            props.get(hit_actor_name),
            Some(&PropValue::Object(Some(level_info.0 as i32)))
        );
        assert_eq!(
            props.get(hit_location_name),
            Some(&vector_value(expected_location))
        );
        assert_eq!(
            props.get(hit_normal_name),
            Some(&vector_value(expected_normal))
        );

        let clear_start = [64.0, 0.0, 0.0];
        let clear_end = [128.0, 0.0, 0.0];
        assert!(
            trace_level_polys(std::slice::from_ref(&wall), clear_start, clear_end, [0.0; 3])
                .is_none(),
            "front-side ray is clear"
        );
        engine
            .execute_frame(
                source,
                trace_code(clear_start, clear_end),
                resolver,
                0,
                PropStore::new(),
                None,
                None,
                None,
                Vec::new(),
                None,
                Some("TraceRequestClear".to_string()),
                ScriptKey::Event(class, "TraceRequestClear".to_string()),
                "TraceRequestClear",
            )
            .expect("TraceRequest dispatch resumes clear ray");
        let props = engine
            .world
            .arena
            .get(source)
            .expect("trace source")
            .properties()
            .expect("trace source properties");
        let zero = vector_value([0.0; 3]);
        assert_eq!(props.get(hit_actor_name), Some(&PropValue::Object(None)));
        assert_eq!(props.get(hit_location_name), Some(&zero));
        assert_eq!(props.get(hit_normal_name), Some(&zero));
    }

    #[test]
    fn cutscript_disk_localized_line_endpoint_is_exclusive() {
        assert!(cutscript_disk_line_in_bounds(
            "..\\Cutscenes\\Privet.txt",
            "line_4095"
        ));
        assert!(!cutscript_disk_line_in_bounds(
            "..\\Cutscenes\\Privet.txt",
            "line_4096"
        ));
        assert!(!cutscript_disk_line_in_bounds(
            "Cutscenes/Privet.txt",
            "LINE_9998"
        ));
        assert!(
            cutscript_disk_line_in_bounds("HPdialog", "line_4096"),
            "the cap belongs only to CutScriptDisk localization files"
        );
        assert!(cutscript_disk_line_in_bounds(
            "..\\Cutscenes\\Privet.txt",
            "caption_4096"
        ));
    }
    #[test]
    fn cutscript_crlf_separators_match_source_parser_contract() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine = Engine::bootstrap(&root, ini, EngineOptions::default()).expect("bootstrap");
        engine.load_map("..\\Maps\\PrivetDr.unr").expect("map");
        let class = engine.world.arena.find_by_path("HGame.CutScript").expect("CutScript");
        let script_name = engine.world.arena.names.find_index("Script").expect("Script");
        let cursor_name = engine.world.arena.names.find_index("curScriptPosition").expect("cursor");
        let function = engine.world.arena.find_function(class, "GetNextCommand")
            .expect("GetNextCommand lookup").expect("GetNextCommand");
        let (code, resolver, parameter) = match &engine.world.arena.get(function).expect("function").data {
            ObjectData::Function(function) => (function.code.clone(), function.resolver.clone(), *function.params.first().expect("command out")),
            _ => panic!("GetNextCommand must be bytecode"),
        };
        let parameter_name = engine.world.arena.get(parameter).expect("command property").name_index;
        let parse_function = engine.world.arena.find_function(class, "ParseCommand")
            .expect("ParseCommand lookup").expect("ParseCommand");
        let (parse_code, parse_resolver, parse_parameter) = match &engine.world.arena.get(parse_function).expect("ParseCommand").data {
            ObjectData::Function(function) => (function.code.clone(), function.resolver.clone(), *function.params.first().expect("command parameter")),
            _ => panic!("ParseCommand must be bytecode"),
        };
        let parse_parameter_name = engine.world.arena.get(parse_parameter).expect("command property").name_index;
        let run_parse = |engine: &Engine, actor: ObjectId, command: String| {
            let mut frame = Frame::with_resolver(
                &engine.world.arena, &engine.world.registry, &parse_code, Some(actor), parse_resolver.clone(),
            );
            frame.set_actor_scope(&engine.level.actors);
            frame.set_next_effect_request_id(engine.next_script_request_id);
            frame.set_active_state_context(
                engine.active_state_name(actor),
                engine.actor_states.get(&actor).copied(),
            );
            frame.locals.set(parse_parameter_name, PropValue::Str(command));
            let result = frame.run().expect("ParseCommand");
            assert_eq!(frame.suspended(), None);
            (result, frame.take_effects())
        };
        for (source, expected, expected_result, expect_error) in [
            ("/*\r\n", "", PropValue::Bool(true), false),
            ("*/\r\n", "*", PropValue::Bool(false), true),
            ("---------------------------------------------------------------------------------------\r\n", "---------------------------------------------------------------------------------------", PropValue::Bool(false), true),
        ] {
            let actor = engine.spawn_script_actor(class, None, [0.0; 3], [0; 3]).expect("spawn");
            engine.actor_store_mut(actor, "test").expect("store").set(script_name, PropValue::Str(source.to_string()));
            engine.actor_store_mut(actor, "test").expect("store").set(cursor_name, PropValue::Int(0));
            let mut frame = Frame::with_resolver(&engine.world.arena, &engine.world.registry, &code, Some(actor), resolver.clone());
            frame.locals.set(parameter_name, PropValue::Str(String::new()));
            assert_eq!(frame.run().expect("GetNextCommand"), PropValue::Bool(true));
            let command = match frame.locals.get(parameter_name).cloned().expect("command local") {
                PropValue::Str(command) => command,
                other => panic!("GetNextCommand returned non-string {other:?}"),
            };
            assert_eq!(command, expected);
            assert!(frame.take_instance_writes().iter().any(|write|
                write.object == actor && write.name == cursor_name && write.value == PropValue::Int(source.len() as i32)
            ));
            drop(frame);
            let state_before = engine.active_state_name(actor);
            let cue_start = engine.cues.len();
            let log_start = engine.script_logs.len();
            let (result, effects) = run_parse(&engine, actor, command);
            assert_eq!(result, expected_result);
            engine.apply_script_effects(effects).expect("apply ParseCommand effects");
            assert_eq!(engine.active_state_name(actor), state_before);
            assert!(engine.cues[cue_start..].is_empty());
            let logs = &engine.script_logs[log_start..];
            let expected_logs = [
                format!("**** CutError:Failed to find {expected}. Actor not captured."),
                format!(
                    "**** CutError:Failed to Parse Command:{expected}. Can't find subject"
                ),
            ];
            if expect_error {
                assert_eq!(logs, expected_logs, "exact CutError payload");
            } else {
                assert!(logs.is_empty(), "successful separator commands do not log");
            }
        }
        let disk_class = engine
            .world
            .arena
            .find_by_path("HGame.CutScriptDisk")
            .expect("CutScriptDisk");
        let disk_cursor_name = engine
            .world
            .arena
            .names
            .find_index("curScriptLine")
            .expect("disk cursor");
        let disk_lines_name = engine
            .world
            .arena
            .names
            .find_index("lineArray")
            .expect("disk line array");
        let disk_function = engine
            .world
            .arena
            .find_function(disk_class, "GetNextCommand")
            .expect("CutScriptDisk GetNextCommand lookup")
            .expect("CutScriptDisk GetNextCommand");
        let (disk_code, disk_resolver, disk_parameter) =
            match &engine.world.arena.get(disk_function).expect("CutScriptDisk GetNextCommand").data {
                ObjectData::Function(function) => (
                    function.code.clone(),
                    function.resolver.clone(),
                    *function.params.first().expect("disk command out"),
                ),
                _ => panic!("CutScriptDisk GetNextCommand must be bytecode"),
            };
        let disk_parameter_name = engine
            .world
            .arena
            .get(disk_parameter)
            .expect("disk command property")
            .name_index;
        for (source, expected, expected_result, expect_error) in [
            ("/*", "", PropValue::Bool(true), false),
            ("*/", "*", PropValue::Bool(false), true),
            (
                "---------------------------------------------------------------------------------------",
                "---------------------------------------------------------------------------------------",
                PropValue::Bool(false),
                true,
            ),
        ] {
            let actor = engine
                .spawn_script_actor(disk_class, None, [0.0; 3], [0; 3])
                .expect("spawn CutScriptDisk");
            let mut rows = vec![PropValue::Str(String::new()); 4096];
            rows[0] = PropValue::Str(source.to_string());
            let store = engine.actor_store_mut(actor, "test").expect("disk store");
            store.set(disk_cursor_name, PropValue::Int(0));
            store.set(disk_lines_name, PropValue::FixedArray(rows));
            let mut frame = Frame::with_resolver(
                &engine.world.arena,
                &engine.world.registry,
                &disk_code,
                Some(actor),
                disk_resolver.clone(),
            );
            frame.locals.set(disk_parameter_name, PropValue::Str(String::new()));
            assert_eq!(
                frame.run().expect("CutScriptDisk GetNextCommand"),
                PropValue::Bool(true)
            );
            let command = match frame
                .locals
                .get(disk_parameter_name)
                .cloned()
                .expect("disk command local")
            {
                PropValue::Str(command) => command,
                other => panic!("CutScriptDisk GetNextCommand returned non-string {other:?}"),
            };
            assert_eq!(command, expected);
            assert!(frame.take_instance_writes().iter().any(|write|
                write.object == actor
                    && write.name == disk_cursor_name
                    && write.value == PropValue::Int(1)
            ));
            drop(frame);
            let state_before = engine.active_state_name(actor);
            let cue_start = engine.cues.len();
            let log_start = engine.script_logs.len();
            let (result, effects) = run_parse(&engine, actor, command);
            assert_eq!(result, expected_result);
            engine
                .apply_script_effects(effects)
                .expect("apply CutScriptDisk ParseCommand effects");
            assert_eq!(engine.active_state_name(actor), state_before);
            assert!(engine.cues[cue_start..].is_empty());
            let logs = &engine.script_logs[log_start..];
            let expected_logs = [
                format!("**** CutError:Failed to find {expected}. Actor not captured."),
                format!(
                    "**** CutError:Failed to Parse Command:{expected}. Can't find subject"
                ),
            ];
            if expect_error {
                assert_eq!(logs, expected_logs, "exact CutScriptDisk CutError payload");
            } else {
                assert!(logs.is_empty(), "successful disk separator commands do not log");
            }
        }
        engine
            .apply_script_effects(vec![ScriptEffect::Log {
                message: "prior-map".to_string(),
            }])
            .expect("record diagnostic");
        engine.load_map("..\\Maps\\PrivetDr.unr").expect("reload map");
        assert!(
            !engine.script_logs.iter().any(|message| message == "prior-map"),
            "map reload clears prior diagnostics"
        );
    }
    #[test]
    fn skeletal_native_metadata_semantics_are_typed_and_source_correct() {
        let skeletal = SkeletalAssetMetadata {
            skeletal: true,
            bone_names: vec!["root".into(), "Bip01 Pelvis".into()],
            bone_children: vec![1, 0],
            animation_names: ["idle", "run"]
                .into_iter()
                .map(hp_uobject::vm::fold_key)
                .collect(),
            animation_groups: HashMap::new(),
        };
        let override_sequences = [hp_uobject::vm::fold_key("blink")]
            .into_iter()
            .collect::<HashSet<_>>();
        assert!(
            skeletal_has_anim_selected(&skeletal, Some(&override_sequences), "blink"),
            "non-null SkelAnim must supply its own sequences"
        );
        assert!(
            !skeletal_has_anim_selected(&skeletal, Some(&override_sequences), "idle"),
            "non-null SkelAnim must override, not merge with, DefaultAnimation"
        );
        assert_eq!(skeletal_bone_number(&skeletal, "ROOT"), 0);
        assert_eq!(skeletal_bone_number(&skeletal, "bip01 pelvis"), 1);
        assert_eq!(skeletal_bone_number(&skeletal, "missing"), -1);
        assert!(skeletal_has_anim(&skeletal, "Idle"));
        assert!(!skeletal_has_anim(&skeletal, "Absent"));

        let nonskeletal = SkeletalAssetMetadata {
            animation_names: [hp_uobject::vm::fold_key("common_only")]
                .into_iter()
                .collect(),
            ..SkeletalAssetMetadata::default()
        };
        assert_eq!(skeletal_bone_number(&nonskeletal, "root"), 0);
        assert!(skeletal_has_anim_selected(
            &nonskeletal,
            Some(&override_sequences),
            "common_only"
        ));
        assert!(!skeletal_has_anim_selected(
            &nonskeletal,
            Some(&override_sequences),
            "blink"
        ));
    }

    #[test]
    fn anim_channel_subset_matches_native_strict_upper_bound() {
        assert!(anim_bone_is_subset(4, 4, 3));
        assert!(anim_bone_is_subset(6, 4, 3));
        assert!(
            !anim_bone_is_subset(7, 4, 3),
            "UnScript.cpp IsSubset uses a strict upper bound"
        );
        assert!(!anim_bone_is_subset(3, 4, 3));
    }

    #[test]
    fn skeletal_queries_resume_serialized_bone_names_psa_groups_and_aux_channels() {
        use hp_format::package79::write_compact_index;
        use hp_uobject::natives::natives_impl::lookup;

        const MESH_NAME: &str = "skharryMesh";
        const ANIMATION_NAME: &str = "skHarryAnims";
        const ROOT_BONE_NAME: &str = "Bip01";
        const SEQUENCE_NAME: &str = "GroupProbe";
        const GROUP_NAME: &str = "Action";

        fn append_chunk(bytes: &mut Vec<u8>, name: &[u8], record_size: i32, data: &[u8]) {
            let mut header = [0; 32];
            header[..name.len()].copy_from_slice(name);
            header[20..24].copy_from_slice(&1_i32.to_le_bytes());
            header[24..28].copy_from_slice(&record_size.to_le_bytes());
            let record_count = if record_size == 0 {
                0
            } else {
                i32::try_from(data.len() / usize::try_from(record_size).expect("record size"))
                    .expect("fixture record count")
            };
            header[28..32].copy_from_slice(&record_count.to_le_bytes());
            bytes.extend_from_slice(&header);
            bytes.extend_from_slice(data);
        }

        fn psa_with_group(sequence: &str, group: &str) -> Vec<u8> {
            let mut bytes = Vec::new();
            append_chunk(&mut bytes, b"ANIMHEAD\0", 0, &[]);

            let mut bone = [0; 120];
            bone[..4].copy_from_slice(b"Root");
            append_chunk(&mut bytes, b"BONENAMES\0", 120, &bone);

            let mut info = [0; 168];
            info[..sequence.len()].copy_from_slice(sequence.as_bytes());
            info[64..64 + group.len()].copy_from_slice(group.as_bytes());
            info[128..132].copy_from_slice(&1_i32.to_le_bytes());
            info[148..152].copy_from_slice(&1.0_f32.to_le_bytes());
            info[152..156].copy_from_slice(&1.0_f32.to_le_bytes());
            info[164..168].copy_from_slice(&1_i32.to_le_bytes());
            append_chunk(&mut bytes, b"ANIMINFO\0", 168, &info);

            let mut key = [0; 32];
            key[24..28].copy_from_slice(&1.0_f32.to_le_bytes());
            append_chunk(&mut bytes, b"ANIMKEYS\0", 32, &key);
            bytes
        }

        let unreal_root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let package = unreal_root.join("System/HPModels.u");
        assert!(
            package.is_file(),
            "HPModels fixture is required for skeletal query acceptance"
        );
        let archive =
            hp_format::package79::read_package(&std::fs::read(package).expect("HPModels.u"))
                .expect("HPModels archive");
        let mesh_export_index = archive
            .exports
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                let named_mesh = archive
                    .names
                    .get(entry.object_name_index.max(0) as usize)
                    .is_some_and(|name| name.text.eq_ignore_ascii_case(MESH_NAME));
                (named_mesh
                    && mesh_export_class(&archive, index)
                        .is_some_and(|class| class.eq_ignore_ascii_case("SkeletalMesh")))
                .then_some(index)
            })
            .expect("skharryMesh SkeletalMesh export");
        let animation_export_index = archive
            .exports
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                archive
                    .names
                    .get(entry.object_name_index.max(0) as usize)
                    .is_some_and(|name| name.text.eq_ignore_ascii_case(ANIMATION_NAME))
                    .then_some(index)
            })
            .expect("skHarryAnims export");
        let fixture_root = std::env::temp_dir().join(format!(
            "hp-engine-psa-group-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let psa_path = fixture_root.join("HPModels/Models/skHarryAnim.PSA");
        std::fs::create_dir_all(psa_path.parent().expect("PSA fixture parent"))
            .expect("PSA fixture directory");
        std::fs::write(&psa_path, psa_with_group(SEQUENCE_NAME, GROUP_NAME))
            .expect("serialized PSA fixture");
        let sequence_text = hp_uobject::vm::fold_key(SEQUENCE_NAME);
        let authored_group_text = GROUP_NAME.to_string();

        let Synth {
            mut engine,
            class,
            actor_a: actor,
            ..
        } = synth_world();
        let hpmodels_name = engine.world.arena.names.intern("HPModels");
        engine
            .world
            .arena
            .get_mut(engine.level.root)
            .expect("synthetic archive root")
            .name_index = hpmodels_name;
        engine.data_root = fixture_root.clone();
        engine.level.archive = Some(archive);
        for (slot, native) in [
            (0x010d, "Engine.Actor.BoneName"),
            (0x011a, "Engine.Actor.IsAnimating"),
            (0x0125, "Engine.Actor.GetAnimGroup"),
        ] {
            engine
                .world
                .registry
                .register(slot, 2, lookup(native).expect("skeletal native body"), native)
                .expect("register skeletal native slot");
            assert!(
                engine.world.registry.is_registered(slot),
                "{native} must use its HP2-registered native slot"
            );
        }

        let mesh_name = engine.world.arena.names.intern("Mesh");
        let skel_anim_name = engine.world.arena.names.intern("SkelAnim");
        let actor_properties = engine
            .world
            .arena
            .get_mut(actor)
            .expect("synthetic skeletal actor")
            .properties_mut()
            .expect("synthetic skeletal actor properties");
        actor_properties.set(
            mesh_name,
            PropValue::Object(Some(
                i32::try_from(mesh_export_index + 1).expect("mesh export reference"),
            )),
        );
        actor_properties.set(
            skel_anim_name,
            PropValue::Object(Some(
                i32::try_from(animation_export_index + 1).expect("animation export reference"),
            )),
        );
        let mesh_key = engine
            .mesh_asset(actor)
            .expect("minimal HPModels archive mesh reference")
            .expect("synthetic actor mesh reference")
            .0;
        let animation_key =
            hp_uobject::vm::fold_key(&format!("HPModels.{ANIMATION_NAME}"));
        let sequence = Name {
            index: engine.world.arena.names.intern(&sequence_text),
            number: NO_NUMBER,
        };
        let root_bone = Name {
            index: engine.world.arena.names.intern(ROOT_BONE_NAME),
            number: NO_NUMBER,
        };
        let authored_group = engine.world.arena.names.intern(&authored_group_text);
        let bone_name_result = engine.world.arena.names.intern("RealQueryBoneName");
        let negative_bone_result = engine.world.arena.names.intern("RealQueryNegativeBoneName");
        let out_of_range_bone_result = engine
            .world
            .arena
            .names
            .intern("RealQueryOutOfRangeBoneName");
        let group_result = engine.world.arena.names.intern("RealQueryGroup");
        let active_result = engine.world.arena.names.intern("RealQueryActive");
        let root_result = engine.world.arena.names.intern("RealQueryRoot");
        let mut exports = Vec::with_capacity(6);
        for (position, name_index) in [
            bone_name_result,
            negative_bone_result,
            out_of_range_bone_result,
            group_result,
            active_result,
            root_result,
        ]
        .into_iter()
        .enumerate()
        {
            exports.push(engine.world.arena.alloc(UObject {
                name_index,
                outer: Some(class),
                data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                    links: Default::default(),
                    kind: if position <= 3 {
                        hp_uobject::value::PropertyKind::Name
                    } else {
                        hp_uobject::value::PropertyKind::Bool
                    },
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            }));
        }
        let query_event = engine
            .world
            .arena
            .names
            .intern("RealSkeletalQueryEvent");
        let mut code = Vec::new();
        let mut assign_query = |property_ref, native, argument| {
            code.push(USToken::Let as u8);
            code.push(USToken::InstanceVariable as u8);
            write_compact_index(&mut code, property_ref);
            code.push(0x61);
            code.push(native);
            code.push(USToken::NameConst as u8);
            write_compact_index(&mut code, argument);
            code.push(USToken::EndFunctionParms as u8);
        };
        assign_query(4, 0x25, sequence.index as i32);
        assign_query(5, 0x1a, Name::none().index as i32);
        assign_query(6, 0x1a, root_bone.index as i32);
        for (property_ref, index) in [(1, 0_i32), (2, -1_i32), (3, i32::MAX)] {
            code.push(USToken::Let as u8);
            code.push(USToken::InstanceVariable as u8);
            write_compact_index(&mut code, property_ref);
            code.extend_from_slice(&[0x61, 0x0d, USToken::IntConst as u8]);
            code.extend_from_slice(&index.to_le_bytes());
            code.push(USToken::EndFunctionParms as u8);
        }
        code.push(USToken::Stop as u8);
        engine.world.arena.alloc(UObject {
            name_index: query_event,
            outer: Some(class),
            data: ObjectData::Function(Box::new(hp_uobject::arena::FunctionData {
                code,
                resolver: BytecodeResolver::from_package_refs(Vec::new(), exports),
                ..Default::default()
            })),
            ..Default::default()
        });

        let aux_name = engine.world.arena.names.intern("AuxAnims");
        let anim_bone_name = engine.world.arena.names.intern("AnimBone");
        let channel_name = engine.world.arena.names.intern("RealQueryAuxChannel");
        let mut channel_properties = PropStore::new();
        channel_properties.set(anim_bone_name, PropValue::Int(0));
        let channel = engine.world.arena.alloc(UObject {
            name_index: channel_name,
            data: ObjectData::Properties(channel_properties),
            ..Default::default()
        });
        engine
            .world
            .arena
            .get_mut(actor)
            .expect("skeletal actor")
            .properties_mut()
            .expect("skeletal actor properties")
            .set(
                aux_name,
                PropValue::Array(vec![PropValue::Object(Some(channel.0 as i32))]),
            );
        engine.actor_animations.insert(
            channel,
            ActorAnimation {
                sequence,
                started_at: 0.0,
                elapsed_seconds: 0.0,
                duration_seconds: Some(1.0),
                finish_after_seconds: Some(1.0),
                mesh_path: None,
                animation_path: None,
                rate: 1.0,
                looped: true,
                tweening: false,
            },
        );

        engine.actor_animations.insert(
            actor,
            ActorAnimation {
                sequence,
                started_at: 0.0,
                elapsed_seconds: 1.0,
                duration_seconds: Some(1.0),
                finish_after_seconds: Some(1.0),
                mesh_path: None,
                animation_path: None,
                rate: 1.0,
                looped: false,
                tweening: false,
            },
        );
        engine
            .run_named_event_on(actor, "RealSkeletalQueryEvent", None)
            .expect("completed query effect resumes");
        assert!(
            engine.skeletal_assets.contains_key(&mesh_key),
            "GetAnimGroup must load the actor's real HPModels SkeletalMesh archive"
        );
        let animation = engine
            .animation_assets
            .get(&animation_key)
            .expect("GetAnimGroup must load skHarryAnim.PSA");
        assert!(
            animation.sequence_names.contains(&sequence_text),
            "selected sequence must originate in skHarryAnim.PSA"
        );
        assert_eq!(
            animation.sequence_groups.get(&sequence_text),
            Some(&authored_group_text),
            "selected sequence must retain its serialized PSA group"
        );
        let properties = engine
            .world
            .arena
            .get(actor)
            .expect("skeletal actor")
            .properties()
            .expect("skeletal actor properties");
        assert_eq!(
            properties.get(bone_name_result),
            Some(&PropValue::Name(root_bone)),
            "BoneName index zero must return RefSkeleton's first serialized bone, not SkelAnim's Root override"
        );
        assert_eq!(
            properties.get(negative_bone_result),
            Some(&PropValue::Name(Name::none())),
            "BoneName must return Name::none() for a negative index"
        );
        assert_eq!(
            properties.get(out_of_range_bone_result),
            Some(&PropValue::Name(Name::none())),
            "BoneName must return Name::none() for an out-of-range index"
        );
        assert_eq!(
            properties.get(group_result),
            Some(&PropValue::Name(Name {
                index: authored_group,
                number: NO_NUMBER,
            })),
            "GetAnimGroup's effect result must resume into the VM assignment"
        );
        let none_group = PropValue::Name(Name::none());
        assert_ne!(
            properties.get(group_result),
            Some(&none_group),
            "GetAnimGroup must preserve the serialized Action group, not return Name::none()"
        );
        assert_eq!(
            properties.get(active_result),
            Some(&PropValue::Bool(false)),
            "a completed non-looping parent animation is inactive"
        );
        assert_eq!(
            properties.get(root_result),
            Some(&PropValue::Bool(true)),
            "non-None root bone must query its active auxiliary channel"
        );
        assert!(
            !engine.actor_scripts.contains_key(&actor),
            "all skeletal-query effect continuations must complete"
        );
        assert!(
            !engine.deferred_scripts.iter().any(|(deferred, _)| *deferred == actor),
            "resolved skeletal native slots must not defer the actor"
        );

        engine.actor_animations.insert(
            actor,
            ActorAnimation {
                sequence,
                started_at: 0.0,
                elapsed_seconds: 1.0,
                duration_seconds: Some(1.0),
                finish_after_seconds: Some(1.0),
                mesh_path: None,
                animation_path: None,
                rate: 1.0,
                looped: true,
                tweening: false,
            },
        );
        engine
            .run_named_event_on(actor, "RealSkeletalQueryEvent", None)
            .expect("looping query effect resumes");
        assert_eq!(
            engine
                .world
                .arena
                .get(actor)
                .expect("skeletal actor")
                .properties()
                .expect("skeletal actor properties")
                .get(active_result),
            Some(&PropValue::Bool(true)),
            "a looping parent animation remains active"
        );
        std::fs::remove_dir_all(fixture_root).expect("remove serialized PSA fixture");
    }

    #[test]
    fn anim_channel_copies_mesh_and_skel_anim_from_parent_cdo() {
        let Synth {
            mut engine,
            class,
            actor_a,
            ..
        } = synth_world();
        let root = engine.level.root;
        let mesh_name = engine.world.arena.names.intern("Mesh");
        let skel_anim_name = engine.world.arena.names.intern("SkelAnim");
        let synthetic_mesh_name = engine.world.arena.names.intern("SyntheticMesh");
        let synthetic_anim_name = engine.world.arena.names.intern("SyntheticAnimation");
        let cdo_name = engine.world.arena.names.intern("Default__GateController");
        let actor_base = match &engine.world.arena.get(class).unwrap().data {
            ObjectData::Class(data) => data.super_class.expect("Engine.Actor"),
            _ => panic!("synthetic class"),
        };
        let channel_class_name = engine.world.arena.names.intern("SyntheticAnimChannel");
        let channel_class = engine.world.arena.alloc(UObject {
            name_index: channel_class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData {
                super_class: Some(actor_base),
                ..Default::default()
            }),
            ..Default::default()
        });
        let root_bone = hp_uobject::name::Name {
            index: engine.world.arena.names.intern("SyntheticRoot"),
            number: 0,
        };
        let reserved = ObjectId(engine.world.arena.len() as u32);
        assert_eq!(
            engine
                .create_anim_channel(
                    reserved,
                    actor_a,
                    Some(channel_class),
                    0,
                    root_bone,
                    false,
                    false,
                )
                .expect("no-mesh channel fallback"),
            None
        );
        let mesh = engine.world.arena.alloc(UObject {
            name_index: synthetic_mesh_name,
            outer: Some(root),
            data: ObjectData::Properties(PropStore::new()),
            ..Default::default()
        });
        let skel_anim = engine.world.arena.alloc(UObject {
            name_index: synthetic_anim_name,
            outer: Some(root),
            data: ObjectData::Properties(PropStore::new()),
            ..Default::default()
        });
        let mut defaults = PropStore::new();
        defaults.set(mesh_name, PropValue::Object(Some(mesh.0 as i32)));
        defaults.set(skel_anim_name, PropValue::Object(Some(skel_anim.0 as i32)));
        let cdo = engine.world.arena.alloc(UObject {
            class_id: Some(class),
            name_index: cdo_name,
            outer: Some(root),
            data: ObjectData::Properties(defaults),
            ..Default::default()
        });
        let ObjectData::Class(class_data) = &mut engine.world.arena.get_mut(class).unwrap().data
        else {
            panic!("synthetic class");
        };
        class_data.default_object = Some(cdo);

        let channel = engine
            .spawn_script_actor(channel_class, Some(actor_a), [0.0; 3], [0; 3])
            .expect("synthetic channel");
        engine
            .copy_anim_channel_properties(actor_a, channel)
            .expect("copy effective parent properties");
        let props = engine
            .world
            .arena
            .get(channel)
            .unwrap()
            .properties()
            .unwrap();
        assert_eq!(
            props.get(mesh_name),
            Some(&PropValue::Object(Some(mesh.0 as i32)))
        );
        assert_eq!(
            props.get(skel_anim_name),
            Some(&PropValue::Object(Some(skel_anim.0 as i32)))
        );
    }

    #[test]
    fn real_skharry_mesh_joins_ref_skeleton_to_companion_psa() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let package = root.join("System/HPModels.u");
        if !package.is_file() {
            return;
        }
        let bytes = std::fs::read(package).expect("HPModels.u");
        let archive = hp_format::package79::read_package(&bytes).expect("HPModels package");
        let export_index = archive
            .exports
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                let named_skharry = archive
                    .names
                    .get(entry.object_name_index.max(0) as usize)
                    .is_some_and(|name| name.text.eq_ignore_ascii_case("skharryMesh"));
                (named_skharry
                    && mesh_export_class(&archive, index)
                        .is_some_and(|class| class.eq_ignore_ascii_case("SkeletalMesh")))
                .then_some(index)
            })
            .expect("skharry SkeletalMesh export");
        let metadata =
            load_skeletal_asset_metadata(&root, "HPModels", &archive, export_index, "SkeletalMesh")
                .expect("mesh RefSkeleton + skharryanim.PSA join");
        assert!(metadata.skeletal);
        assert_eq!(metadata.bone_names.len(), 143);
        assert_eq!(skeletal_bone_number(&metadata, "Bip01"), 0);
        assert!(metadata.animation_names.len() >= 108);
        let psa = read_psa(
            &std::fs::read(root.join("HPModels/Models/skharryanim.PSA")).expect("skharry PSA"),
        )
        .expect("PSA metadata");
        assert_eq!(psa.bones.len(), 140);
        assert_eq!(psa.sequences.len(), 108);
        assert!(skeletal_has_anim(&metadata, "LeanOnWindow"));
        assert!(skeletal_has_anim(&metadata, "look_mirror"));
        assert!(!skeletal_has_anim(&metadata, "AuthoredSequenceAbsent"));
    }
    #[test]
    fn real_genmale_and_dobby_meshes_join_their_authored_companion_psas() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let package = root.join("System/HPModels.u");
        if !package.is_file() {
            return;
        }
        let bytes = std::fs::read(package).expect("HPModels.u");
        let archive = hp_format::package79::read_package(&bytes).expect("HPModels package");
        let load = |mesh_name: &str| {
            let export_index = archive
                .exports
                .iter()
                .enumerate()
                .find_map(|(index, entry)| {
                    let named_mesh = archive
                        .names
                        .get(entry.object_name_index.max(0) as usize)
                        .is_some_and(|name| name.text.eq_ignore_ascii_case(mesh_name));
                    (named_mesh
                        && mesh_export_class(&archive, index)
                            .is_some_and(|class| class.eq_ignore_ascii_case("SkeletalMesh")))
                    .then_some(index)
                })
                .unwrap_or_else(|| panic!("{mesh_name} SkeletalMesh export"));
            load_skeletal_asset_metadata(&root, "HPModels", &archive, export_index, "SkeletalMesh")
                .unwrap_or_else(|error| panic!("{mesh_name} metadata: {error}"))
        };
        let genmale = load("skhp2_genmale1Mesh");
        let genmale_override = load_psa_sequence_names(&root, "HPModels", "skGenMaleAnims")
            .expect("skGenMale PSA lookup")
            .expect("skGenMale.PSA companion");
        assert!(skeletal_has_anim_selected(
            &genmale,
            Some(&genmale_override),
            "idle"
        ));
        assert!(skeletal_has_anim_selected(
            &genmale,
            Some(&genmale_override),
            "walk"
        ));
        assert!(!skeletal_has_anim_selected(
            &genmale,
            Some(&genmale_override),
            "LeanOnWindow"
        ));

        let dobby = load("skdobbyMesh");
        for sequence in [
            "appear",
            "Introduce",
            "shakeheadnostart",
            "shakeheadnoloop",
            "shakeheadnoend",
            "dissapear",
        ] {
            assert!(
                skeletal_has_anim(&dobby, sequence),
                "skdobby.PSA must contain authored sequence {sequence}"
            );
        }
        assert!(!skeletal_has_anim(&dobby, "AuthoredSequenceAbsent"));
    }
    #[test]
    fn real_hprops_mesh_joins_package_local_companion_psa() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let package = root.join("System/HProps.u");
        if !package.is_file() {
            return;
        }
        let bytes = std::fs::read(package).expect("HProps.u");
        let archive = hp_format::package79::read_package(&bytes).expect("HProps package");
        let export_index = archive
            .exports
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                let named_torch = archive
                    .names
                    .get(entry.object_name_index.max(0) as usize)
                    .is_some_and(|name| name.text.eq_ignore_ascii_case("skArmTorchMesh"));
                (named_torch
                    && mesh_export_class(&archive, index)
                        .is_some_and(|class| class.eq_ignore_ascii_case("SkeletalMesh")))
                .then_some(index)
            })
            .expect("skArmTorch SkeletalMesh export");
        let metadata =
            load_skeletal_asset_metadata(&root, "HProps", &archive, export_index, "SkeletalMesh")
                .expect("HProps mesh + package-local skArmTorch.PSA join");
        let psa = read_psa(
            &std::fs::read(root.join("HProps/Models/skArmTorch.PSA")).expect("skArmTorch PSA"),
        )
        .expect("HProps PSA metadata");
        assert!(!psa.sequences.is_empty());
        assert!(skeletal_has_anim(&metadata, &psa.sequences[0].name));
    }

    // ---- Synthetic GateController world ---------------------------------

    /// Registry carrying the retail slot bodies the synthetic bytecode
    /// calls (same numbered slots the census sees in shipped streams).
    fn synth_registry() -> NativeRegistry {
        use hp_uobject::natives::natives_impl::lookup;
        let mut registry = NativeRegistry::new();
        for (slot, subject) in [
            (0x0071u16, "Core.Object.GotoState"),
            (0x0075, "Core.Object.Enable"),
            (0x0076, "Core.Object.Disable"),
            (0x0100, "Engine.Actor.Sleep"),
            (0x012D, "Engine.Actor.FinishInterpolation"),
            (0x0118, "Engine.Actor.SetTimer"),
            (0x0116, "Engine.Actor.Spawn"),
            (0x0117, "Engine.Actor.SetLocation"),
            (0x0119, "Core.Object.NotEqual_ObjectObject"),
            (0x011C, "Engine.Actor.GetStateName"),
        ] {
            let body = lookup(subject).expect("S2 native bound");
            registry.register(slot, 2, body, subject).expect("register");
        }
        registry
    }

    struct Synth {
        engine: Engine,
        class: ObjectId,
        actor_a: ObjectId,
        actor_b: ObjectId,
        running_id: ObjectId,
        done_id: ObjectId,
        done_name: u32,
        seen_name: u32,
        count_name: u32,
        ticked_name: u32,
        callee_resumed_name: u32,
    }

    /// Two `GateController` actors whose scripts exercise the whole loop:
    /// Trigger → GotoState('Running'); Running calls `WaitThen`, whose
    /// `Sleep(0.5)` yields and later writes `CalleeResumed`; the state then
    /// arms SetTimer(0.2, one-shot), enters Done, and Timer records
    /// GetStateName/Count so firing order is observable per actor.
    fn synth_world() -> Synth {
        let mut arena = ObjectArena::new();
        let root = arena.root_for("Engine");
        let actor_base = {
            let name_index = arena.names.intern("Actor");
            arena.alloc(hp_uobject::arena::UObject {
                class_id: None,
                name_index,
                outer: Some(root),
                flags: 0,
                data: ObjectData::Class(ClassData::default()),
            })
        };
        let class = {
            let name_index = arena.names.intern("GateController");
            arena.alloc(hp_uobject::arena::UObject {
                class_id: None,
                name_index,
                outer: Some(root),
                flags: 0,
                data: ObjectData::Class(ClassData {
                    super_class: Some(actor_base),
                    ..Default::default()
                }),
            })
        };
        use hp_uobject::vm::USToken;

        // Trigger(): GotoState('Running'); Stop.
        let running = arena.names.intern("Running");
        let done = arena.names.intern("Done");
        let mut trigger_code = vec![0x71, USToken::NameConst as u8];
        trigger_code.push(running as u8);
        trigger_code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        // Running(): WaitThen() where the callee sleeps and writes after
        // waking; only then does the state arm its timer and GotoState.
        let wait_then = arena.names.intern("WaitThen");
        let callee_resumed = arena.names.intern("CalleeResumed");
        let mut wait_then_code = vec![0x61, 0x00, USToken::FloatConst as u8];
        wait_then_code.extend_from_slice(&0.5f32.to_le_bytes());
        wait_then_code.push(USToken::EndFunctionParms as u8);
        wait_then_code.extend_from_slice(&[
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            callee_resumed as u8,
            USToken::IntOne as u8,
            USToken::Stop as u8,
        ]);
        let mut running_code = vec![
            USToken::VirtualFunction as u8,
            wait_then as u8,
            USToken::EndFunctionParms as u8,
        ];
        running_code.extend_from_slice(&[0x61, 0x18, USToken::FloatConst as u8]); // slot 0x118
        running_code.extend_from_slice(&0.2f32.to_le_bytes());
        running_code.extend_from_slice(&[
            USToken::False as u8,
            USToken::EndFunctionParms as u8,
            0x71,
            USToken::NameConst as u8,
        ]);
        running_code.push(done as u8);
        running_code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        running_code.push(USToken::LabelTable as u8);
        // WaitThen selector + Done NameConst each occupy four runtime bytes
        // but one serialized byte in this synthetic stream.
        let label_table_at = running_code.len() + 6;
        let begin = arena.names.intern("Begin");
        running_code.push(begin as u8);
        running_code.extend_from_slice(&0i32.to_le_bytes()); // Begin -> 0
        running_code.push(0); // NAME_None terminator
        // Done(): a valid `Begin` table points at the immediate Stop.
        let mut done_code = vec![USToken::Stop as u8, USToken::LabelTable as u8];
        let done_label_table_at = done_code.len();
        done_code.push(begin as u8);
        done_code.extend_from_slice(&0i32.to_le_bytes());
        done_code.push(0);
        // Timer(): Seen = GetStateName(); Count = 42; Stop.
        let seen = arena.names.intern("Seen");
        let count = arena.names.intern("Count");
        let mut timer_code = vec![
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            seen as u8,
            0x61,
            0x1C, // slot 0x11C GetStateName
            USToken::EndFunctionParms as u8,
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            count as u8,
            USToken::IntConst as u8,
        ];
        timer_code.extend_from_slice(&42i32.to_le_bytes());
        let timer_event = arena.names.intern("Timer");
        timer_code.push(USToken::Stop as u8);
        // Tick(): Ticked = 1; Stop. Class Tick remains eligible while state
        // code is active unless the state overrides it.
        let ticked = arena.names.intern("Ticked");
        let tick_code = vec![
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            ticked as u8,
            USToken::IntOne as u8,
            USToken::Stop as u8,
        ];
        let property_names = [callee_resumed, seen, count, ticked];
        let mut exports = vec![actor_base; property_names.iter().copied().max().unwrap() as usize];
        for name_index in property_names {
            let property = arena.alloc(hp_uobject::arena::UObject {
                name_index,
                outer: Some(class),
                data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                    links: Default::default(),
                    kind: hp_uobject::value::PropertyKind::Int,
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            });
            exports[name_index as usize - 1] = property;
        }
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), exports);

        let mut function = |name: &str, code: Vec<u8>| {
            let name_index = arena.names.intern(name);
            arena.alloc(hp_uobject::arena::UObject {
                class_id: None,
                name_index,
                outer: Some(class),
                flags: 0,
                data: ObjectData::Function(Box::new(hp_uobject::arena::FunctionData {
                    code,
                    resolver: resolver.clone(),
                    ..Default::default()
                })),
            })
        };
        function("Trigger", trigger_code);
        function("WaitThen", wait_then_code);
        function("Timer", timer_code);
        function("Tick", tick_code);
        function(
            "OnResolveGameState",
            vec![
                USToken::Let as u8,
                USToken::InstanceVariable as u8,
                seen as u8,
                USToken::IntOne as u8,
                USToken::Stop as u8,
            ],
        );
        let disable_timer_code = [
            0x76,
            USToken::NameConst as u8,
            timer_event as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let enable_timer_code = [
            0x75,
            USToken::NameConst as u8,
            timer_event as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        function("DisableTimer", disable_timer_code.to_vec());
        function("EnableTimer", enable_timer_code.to_vec());
        let running_id = {
            let name_index = arena.names.find_index("Running").expect("Running name");
            arena.alloc(hp_uobject::arena::UObject {
                class_id: None,
                name_index,
                outer: Some(class),
                flags: 0,
                data: ObjectData::State(Box::new(hp_uobject::arena::StateData {
                    code: running_code,
                    label_table_offset: label_table_at as u16,
                    resolver: resolver.clone(),
                    ..Default::default()
                })),
            })
        };
        let done_id = {
            let idx = arena.names.find_index("Done").expect("Done name");
            arena.alloc(hp_uobject::arena::UObject {
                class_id: None,
                name_index: idx,
                outer: Some(class),
                flags: 0,
                data: ObjectData::State(Box::new(hp_uobject::arena::StateData {
                    code: done_code,
                    label_table_offset: done_label_table_at as u16,
                    resolver: resolver.clone(),
                    ..Default::default()
                })),
            })
        };
        let mut spawn_actor = |name: &str| {
            let name_index = arena.names.intern(name);
            arena.alloc(hp_uobject::arena::UObject {
                class_id: Some(class),
                name_index,
                outer: Some(root),
                flags: 0,
                data: ObjectData::Properties(PropStore::new()),
            })
        };
        let actor_a = spawn_actor("Alpha");
        let actor_b = spawn_actor("Beta");
        let level = Level {
            root,
            actors: vec![actor_a, actor_b],
            model_count: 0,
            export_ids: Vec::new(),
            archive: None,
        };
        let world = World {
            arena,
            registry: synth_registry(),
            roots: HashMap::new(),
            exports: HashMap::new(),
            archives: HashMap::new(),
            class_package: HashMap::new(),
        };
        let engine = Engine {
            world,
            data_root: PathBuf::new(),
            level,
            rng: SimRng::seeded(DEFAULT_SEED),
            fixed_dt: Some(DT),
            editor_mode: false,
            frame_rate_limit: 0.0,
            bindings: Bindings::parse(&[]),
            ini: IniSet::default(),
            cues: Vec::new(),
            counters: RunCounters::default(),
            sound_cues: Vec::new(),
            sounds_enabled: false,
            audio: None,
            pawn: None,
            collision: None,
            pawn_first_person: false,
            pending_level_exit: false,
            level_exit_destination: None,
            fired_triggers: HashSet::new(),
            deferred_sounds: HashSet::new(),
            quit_requested: false,
            script_lookup: HashMap::new(),
            deferred_scripts: HashSet::new(),
            deferral_reported: HashSet::new(),
            deferral_reason_codes: HashSet::new(),
            native_deferral_census: BTreeMap::new(),
            sim_time: 0.0,
            next_script_request_id: 0,
            actor_scripts: HashMap::new(),
            actor_states: HashMap::new(),
            timers: Vec::new(),
            spline_motions: HashMap::new(),
            base_cam_shake: None,
            deferred_states: HashSet::new(),
            destroyed_actors: HashSet::new(),
            actor_animations: HashMap::new(),
            disabled_events: HashMap::new(),
            skeletal_assets: HashMap::new(),
            script_logs: Vec::new(),
            animation_assets: HashMap::new(),
            animation_metadata_reported: HashSet::new(),
            finish_animation_fallback_reported: HashSet::new(),
            test_trace_collision_polys: None,
        };
        Synth {
            engine,
            class,
            actor_a,
            actor_b,
            running_id,
            done_id,
            done_name: done,
            seen_name: seen,
            count_name: count,
            ticked_name: ticked,
            callee_resumed_name: callee_resumed,
        }
    }

    #[test]
    fn make_noise_dispatches_loaded_pawns_in_level_order_and_allows_none() {
        use hp_uobject::natives::natives_impl::lookup;

        let Synth {
            mut engine,
            class,
            actor_a: source,
            actor_b,
            ..
        } = synth_world();
        let root = engine.level.root;
        let actor_class = match &engine.world.arena.get(class).expect("controller class").data {
            ObjectData::Class(data) => data.super_class.expect("Actor base"),
            other => panic!("controller class data: {other:?}"),
        };
        let pawn_class = engine.world.arena.alloc(UObject {
            name_index: engine.world.arena.names.intern("Pawn"),
            outer: Some(root),
            data: ObjectData::Class(ClassData {
                super_class: Some(actor_class),
                ..Default::default()
            }),
            ..Default::default()
        });
        let loudness = engine.world.arena.names.intern("Loudness");
        let noise_maker = engine.world.arena.names.intern("NoiseMaker");
        let float_parameter = engine.world.arena.alloc(UObject {
            name_index: loudness,
            outer: Some(pawn_class),
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Float,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let actor_parameter = engine.world.arena.alloc(UObject {
            name_index: noise_maker,
            outer: Some(pawn_class),
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Object { class_ref: None },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let resolver = BytecodeResolver::from_package_refs(
            Vec::new(),
            vec![float_parameter, actor_parameter],
        );
        let hear_noise = engine.world.arena.names.intern("HearNoise");
        let listener = |arena: &mut ObjectArena, class_name: &str, marker: &str| {
            let class = arena.alloc(UObject {
                name_index: arena.names.intern(class_name),
                outer: Some(root),
                data: ObjectData::Class(ClassData {
                    super_class: Some(pawn_class),
                    ..Default::default()
                }),
                ..Default::default()
            });
            let mut code = vec![0x70, USToken::StringConst as u8];
            code.extend(marker.bytes());
            code.extend([0, USToken::EndFunctionParms as u8]);
            for parameter in [1u8, 2] {
                code.extend([
                    0x70,
                    USToken::LocalVariable as u8,
                    parameter,
                    USToken::EndFunctionParms as u8,
                ]);
            }
            code.push(USToken::Stop as u8);
            arena.alloc(UObject {
                name_index: hear_noise,
                outer: Some(class),
                data: ObjectData::Function(Box::new(hp_uobject::arena::FunctionData {
                    code,
                    params: vec![float_parameter, actor_parameter],
                    resolver: resolver.clone(),
                    ..Default::default()
                })),
                ..Default::default()
            });
            arena.alloc(UObject {
                class_id: Some(class),
                name_index: arena.names.intern(marker),
                outer: Some(root),
                data: ObjectData::Properties(PropStore::new()),
                ..Default::default()
            })
        };
        let first = listener(&mut engine.world.arena, "FirstNoisePawn", "first");
        let second = listener(&mut engine.world.arena, "SecondNoisePawn", "second");
        engine
            .world
            .registry
            .register(
                0x70,
                2,
                lookup("Core.Object.Log").expect("Log native"),
                "Core.Object.Log",
            )
            .expect("register Log");
        engine.level.actors = vec![source, second, actor_b, first];

        engine
            .apply_script_effects(vec![ScriptEffect::MakeNoise {
                actor: source,
                loudness: 3.25,
            }])
            .expect("MakeNoise dispatch");
        let noise_maker = format!("Object(Some({}))", source.0);
        assert_eq!(
            engine.script_logs,
            vec![
                "second".to_string(),
                "3.25".to_string(),
                noise_maker.clone(),
                "first".to_string(),
                "3.25".to_string(),
                noise_maker,
            ],
            "each loaded Pawn receives HearNoise exactly once in level order"
        );

        engine.level.actors = vec![source, actor_b];
        engine.script_logs.clear();
        engine
            .apply_script_effects(vec![ScriptEffect::MakeNoise {
                actor: source,
                loudness: 1.0,
            }])
            .expect("MakeNoise with no Pawn listeners is a successful no-op");
        assert!(engine.script_logs.is_empty());
    }

    #[test]
    fn actor_error_destroys_only_its_receiver_without_a_deferral() {
        let Synth {
            mut engine,
            class,
            actor_a,
            actor_b,
            seen_name,
            ..
        } = synth_world();
        engine
            .world
            .registry
            .register(
                233,
                2,
                hp_uobject::natives::natives_impl::lookup("Engine.Actor.Error")
                    .expect("Actor.Error native"),
                "Engine.Actor.Error",
            )
            .expect("register Actor.Error");
        let error_name = engine.world.arena.names.intern("ActorError");
        engine.world.arena.alloc(hp_uobject::arena::UObject {
            class_id: None,
            name_index: error_name,
            outer: Some(class),
            flags: 0,
            data: ObjectData::Function(Box::new(hp_uobject::arena::FunctionData {
                code: vec![
                    233,
                    USToken::StringConst as u8,
                    b's',
                    b'c',
                    b'r',
                    b'i',
                    b'p',
                    b't',
                    b' ',
                    b'f',
                    b'a',
                    b'i',
                    b'l',
                    b'e',
                    b'd',
                    0,
                    USToken::EndFunctionParms as u8,
                    USToken::Stop as u8,
                ],
                ..Default::default()
            })),
        });

        engine
            .run_named_event_on(actor_a, "ActorError", None)
            .expect("Actor.Error must terminate only its receiver event");
        assert!(engine.destroyed_actors.contains(&actor_a));
        assert!(!engine.level.actors.contains(&actor_a));
        assert!(engine.level.actors.contains(&actor_b));
        assert_eq!(engine.counters.script_deferrals, 0);
        assert!(engine.deferred_scripts.is_empty());
        assert!(engine.native_deferral_census.is_empty());
        assert!(engine.actor_scripts.is_empty());
        engine
            .run_named_event_on(actor_b, "OnResolveGameState", None)
            .expect("a distinct actor remains runnable after Actor.Error");
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_b)
                .expect("actor B")
                .properties()
                .expect("actor B properties")
                .get(seen_name),
            Some(&PropValue::Int(1))
        );
    }

    #[test]
    fn game_state_exclusion_without_group_resolves_only_matching_actor() {
        let Synth {
            mut engine,
            actor_a,
            actor_b,
            seen_name,
            ..
        } = synth_world();
        let exclude = engine.world.arena.names.intern("ExcludeGameStates");
        engine
            .world
            .arena
            .get_mut(actor_a)
            .unwrap()
            .properties_mut()
            .unwrap()
            .set(exclude, PropValue::Str("GSTATE000".to_string()));

        engine
            .screen_actors_by_game_state(Some("GSTATE000"))
            .expect("screen");

        let in_state = engine
            .world
            .arena
            .names
            .find_index("bInCurrentGameState")
            .unwrap();
        let props_a = engine
            .world
            .arena
            .get(actor_a)
            .unwrap()
            .properties()
            .unwrap();
        assert_eq!(props_a.get(in_state), Some(&PropValue::Bool(false)));
        assert_eq!(props_a.get(seen_name), Some(&PropValue::Int(1)));
        let props_b = engine
            .world
            .arena
            .get(actor_b)
            .unwrap()
            .properties()
            .unwrap();
        assert_eq!(
            props_b.get(in_state),
            None,
            "unrelated actor stays untouched"
        );
        assert_eq!(
            props_b.get(seen_name),
            None,
            "unrelated actor gets no callback"
        );
    }

    #[test]
    fn spawn_result_resumes_same_frame_and_writes_spawned_instance() {
        let Synth {
            mut engine,
            class,
            actor_a,
            seen_name,
            ..
        } = synth_world();
        let holder_name = engine.world.arena.names.intern("SpawnedActor");
        let holder = engine.world.arena.alloc(UObject {
            name_index: holder_name,
            outer: Some(class),
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Object {
                    class_ref: Some(class),
                },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let seen_property = engine
            .world
            .arena
            .children_of(class)
            .find(|(_, object)| object.name_index == seen_name)
            .map(|(id, _)| id)
            .expect("Seen property");
        let resolver =
            BytecodeResolver::from_package_refs(Vec::new(), vec![holder, class, seen_property]);
        let mut code = vec![
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            1,
            0x61,
            0x16,
            USToken::ObjectConst as u8,
            2,
            USToken::EndFunctionParms as u8,
            USToken::Context as u8,
            USToken::InstanceVariable as u8,
            1,
            0,
            0,
            0,
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            3,
            USToken::IntConst as u8,
        ];
        code.extend_from_slice(&77i32.to_le_bytes());
        code.push(USToken::Stop as u8);
        let before = engine.level.actors.len();

        engine
            .execute_frame(
                actor_a,
                code,
                resolver,
                0,
                PropStore::new(),
                None,
                None,
                None,
                Vec::new(),
                None,
                Some("SpawnTest".to_string()),
                ScriptKey::Event(class, "SpawnTest".to_string()),
                "SpawnTest",
            )
            .expect("Spawn result resumes");

        assert_eq!(engine.level.actors.len(), before + 1);
        let spawned = *engine.level.actors.last().expect("spawn append");
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_a)
                .unwrap()
                .properties()
                .unwrap()
                .get(holder_name),
            Some(&PropValue::Object(Some(spawned.0 as i32)))
        );
        assert_eq!(
            engine
                .world
                .arena
                .get(spawned)
                .unwrap()
                .properties()
                .unwrap()
                .get(seen_name),
            Some(&PropValue::Int(77))
        );
    }
    #[test]
    fn spawn_inside_native_argument_resumes_call_with_real_object() {
        let Synth {
            mut engine,
            class,
            actor_a,
            seen_name,
            ..
        } = synth_world();
        let seen_property = engine
            .world
            .arena
            .children_of(class)
            .find(|(_, object)| object.name_index == seen_name)
            .map(|(id, _)| id)
            .expect("Seen property");
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), vec![seen_property, class]);
        let code = vec![
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            1,
            0x61,
            0x19,
            0x61,
            0x16,
            USToken::ObjectConst as u8,
            2,
            USToken::EndFunctionParms as u8,
            USToken::NoObject as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];

        engine
            .execute_frame(
                actor_a,
                code,
                resolver,
                0,
                PropStore::new(),
                None,
                None,
                None,
                Vec::new(),
                None,
                Some("SpawnArgument".to_string()),
                ScriptKey::Event(class, "SpawnArgument".to_string()),
                "SpawnArgument",
            )
            .expect("native call resumes after Spawn argument");

        assert!(matches!(
            engine
                .world
                .arena
                .get(actor_a)
                .unwrap()
                .properties()
                .unwrap()
                .get(seen_name),
            Some(PropValue::Bool(true))
        ));
    }

    #[test]
    fn nested_spawn_result_resumes_inner_then_outer_assignment() {
        let Synth {
            mut engine,
            class,
            actor_a,
            ..
        } = synth_world();
        let holder_name = engine.world.arena.names.intern("NestedSpawnedActor");
        let holder = engine.world.arena.alloc(UObject {
            name_index: holder_name,
            outer: Some(class),
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Object {
                    class_ref: Some(class),
                },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let inner_resolver = BytecodeResolver::from_package_refs(Vec::new(), vec![class]);
        let inner_name = engine.world.arena.names.intern("SpawnInner");
        let inner = engine.world.arena.alloc(UObject {
            name_index: inner_name,
            outer: Some(class),
            data: ObjectData::Function(Box::new(hp_uobject::arena::FunctionData {
                code: vec![
                    USToken::Return as u8,
                    0x61,
                    0x16,
                    USToken::ObjectConst as u8,
                    1,
                    USToken::EndFunctionParms as u8,
                ],
                resolver: inner_resolver,
                ..Default::default()
            })),
            ..Default::default()
        });
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), vec![holder, inner]);
        let code = vec![
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            1,
            USToken::FinalFunction as u8,
            2,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];

        engine
            .execute_frame(
                actor_a,
                code,
                resolver,
                0,
                PropStore::new(),
                None,
                None,
                None,
                Vec::new(),
                None,
                Some("NestedSpawn".to_string()),
                ScriptKey::Event(class, "NestedSpawn".to_string()),
                "NestedSpawn",
            )
            .expect("nested Spawn resumes");

        let spawned = *engine.level.actors.last().expect("spawn append");

        assert_eq!(
            engine
                .world
                .arena
                .get(actor_a)
                .unwrap()
                .properties()
                .unwrap()
                .get(holder_name),
            Some(&PropValue::Object(Some(spawned.0 as i32)))
        );
    }
    #[test]
    fn two_point_follow_spline_moves_and_completes_only_at_deadline() {
        let Synth {
            mut engine,
            class,
            actor_a: mover,
            actor_b: start,
            ..
        } = synth_world();
        let end = engine
            .spawn_script_actor(class, None, [100.0, 0.0, 0.0], [0; 3])
            .expect("end interpolation point");
        let cut_name = engine.world.arena.names.intern("CutName");
        let next = engine.world.arena.names.intern("Next");
        engine
            .actor_store_mut(start, "test")
            .unwrap()
            .set(cut_name, PropValue::Str("SplineStart".into()));
        engine
            .actor_store_mut(end, "test")
            .unwrap()
            .set(cut_name, PropValue::Str("SplineEnd".into()));
        engine
            .actor_store_mut(start, "test")
            .unwrap()
            .set(next, PropValue::Object(Some(end.0 as i32)));
        engine
            .set_actor_vector(start, "Location", [0.0, 0.0, 0.0])
            .unwrap();
        engine
            .set_actor_vector(end, "Location", [100.0, 0.0, 0.0])
            .unwrap();
        engine
            .apply_script_effects(vec![ScriptEffect::FollowSpline {
                actor: mover,
                command:
                    "FollowSpline TestPath start=SplineStart dest=SplineEnd Time=2 accel=40 Align"
                        .into(),
                cue: "SplineDone".into(),
            }])
            .expect("start spline");

        engine.advance_splines(0.5).expect("quarter duration");
        let quarter = engine.actor_location(mover).unwrap();
        assert!(quarter[0] > 0.0 && quarter[0] < 100.0);
        assert!(!engine.cues.iter().any(|cue| cue == "SplineDone"));

        engine.advance_splines(1.49).expect("before deadline");
        assert!(engine.actor_location(mover).unwrap()[0] < 100.0);
        assert!(!engine.cues.iter().any(|cue| cue == "SplineDone"));

        engine.advance_splines(0.01).expect("at deadline");
        assert_eq!(engine.actor_location(mover), Some([100.0, 0.0, 0.0]));
        assert_eq!(
            engine
                .cues
                .iter()
                .filter(|cue| *cue == "SplineDone")
                .count(),
            1
        );
        assert!(!engine.spline_motions.contains_key(&mover));
    }

    #[test]
    fn spawn_non_actor_class_fails_loudly() {
        let Synth {
            mut engine,
            actor_a,
            ..
        } = synth_world();
        let root = engine.level.root;
        let class_name = engine.world.arena.names.intern("NotAnActor");
        let bad_class = engine.world.arena.alloc(UObject {
            name_index: class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let reserved = ObjectId(engine.world.arena.len() as u32);
        let fail = engine
            .apply_script_effects(vec![ScriptEffect::SpawnRequest {
                request_id: 0,
                reserved,
                class: bad_class,
                owner: Some(actor_a),
                location: [0.0; 3],
                rotation: [0; 3],
            }])
            .expect_err("non-Actor spawn class must fail");
        assert_eq!(fail.reason_code, "native.spawn_class_invalid");
        assert!(!engine.level.actors.contains(&reserved));
    }

    #[test]
    fn script_effects_mutate_active_actors_and_record_audio() {
        let Synth {
            mut engine,
            actor_a,
            actor_b,
            ..
        } = synth_world();
        let cue_name = engine.world.arena.names.intern("TestCue");
        let cue = engine.world.arena.alloc(UObject {
            name_index: cue_name,
            outer: Some(engine.level.root),
            ..Default::default()
        });
        let sequence = hp_uobject::name::Name {
            index: engine.world.arena.names.intern("Idle"),
            number: hp_uobject::name::NO_NUMBER,
        };
        engine
            .apply_script_effects(vec![
                ScriptEffect::Log {
                    message: "script diagnostic".to_string(),
                },
                ScriptEffect::SetLocation {
                    actor: actor_a,
                    location: [10.0, 20.0, 30.0],
                },
                ScriptEffect::SetRotation {
                    actor: actor_a,
                    rotation: [100, 200, 300],
                },
                ScriptEffect::SetOwner {
                    actor: actor_a,
                    owner: Some(actor_b),
                },
                ScriptEffect::PlaySound {
                    actor: actor_a,
                    cue,
                },
                ScriptEffect::LoopAnim {
                    actor: actor_a,
                    sequence,
                    rate: 1.25,
                    looped: true,
                },
                ScriptEffect::Destroy { actor: actor_b },
            ])
            .expect("apply effects");
        assert_eq!(engine.actor_location(actor_a), Some([10.0, 20.0, 30.0]));
        assert_eq!(
            engine
                .world
                .arena
                .names
                .find_index("Owner")
                .and_then(|name| engine
                    .world
                    .arena
                    .get(actor_a)
                    .ok()?
                    .properties()?
                    .get(name)),
            Some(&PropValue::Object(Some(actor_b.0 as i32)))
        );
        assert!(
            engine
                .sound_cues
                .last()
                .is_some_and(|cue| cue.contains("TestCue"))
        );
        assert_eq!(
            engine.actor_animations.get(&actor_a),
            Some(&ActorAnimation {
                sequence,
                started_at: 0.0,
                elapsed_seconds: 0.0,
                duration_seconds: None,
                finish_after_seconds: None,
                mesh_path: None,
                animation_path: None,
                rate: 1.25,
                looped: true,
                tweening: false,
            })
        );
        assert!(!engine.level.actors.contains(&actor_b));
        assert_eq!(engine.script_logs, vec!["script diagnostic"]);
        engine
            .apply_script_effects(
                (0..=SCRIPT_LOG_BUDGET)
                    .map(|index| ScriptEffect::Log {
                        message: format!("bounded-{index}"),
                    })
                    .collect(),
            )
            .expect("bounded script logs");
        assert_eq!(engine.script_logs.len(), SCRIPT_LOG_BUDGET);
        assert_eq!(engine.script_logs.first(), Some(&"script diagnostic".to_string()));
    }

    #[test]
    fn animation_elapsed_uses_fixed_ticks_and_new_requests_reset_it() {
        let Synth {
            mut engine,
            actor_a,
            ..
        } = synth_world();
        engine.fixed_dt = Some(0.125);
        let idle = hp_uobject::name::Name {
            index: engine.world.arena.names.intern("Idle"),
            number: hp_uobject::name::NO_NUMBER,
        };
        let wave = hp_uobject::name::Name {
            index: engine.world.arena.names.intern("Wave"),
            number: hp_uobject::name::NO_NUMBER,
        };
        engine
            .apply_script_effects(vec![ScriptEffect::LoopAnim {
                actor: actor_a,
                sequence: idle,
                rate: 1.5,
                looped: true,
            }])
            .expect("loop request");
        engine.tick(&[]).expect("tick one");
        engine.tick(&[]).expect("tick two");
        let first = engine.actor_animation_snapshots().expect("snapshots");
        let snapshot = first.get(&actor_a).expect("actor snapshot");
        assert_eq!(snapshot.sequence, "Idle");
        assert_eq!(snapshot.elapsed_seconds, 0.25);
        assert!(snapshot.looped);
        assert_eq!(snapshot.started_at, 0.0);

        engine
            .apply_script_effects(vec![ScriptEffect::TweenAnim {
                actor: actor_a,
                sequence: wave,
                rate: 0.75,
                looped: false,
            }])
            .expect("replacement request");
        let reset = engine.actor_animation_snapshots().expect("reset snapshot");
        let snapshot = reset.get(&actor_a).expect("actor snapshot");
        assert_eq!(snapshot.sequence, "Wave");
        assert_eq!(snapshot.elapsed_seconds, 0.0);
        assert_eq!(snapshot.started_at, 0.25);
        assert!(snapshot.tweening);
        assert!(!snapshot.looped);
    }

    #[test]
    fn finish_anim_uses_authored_last_frame_crossing() {
        let animation = ActorAnimation {
            sequence: hp_uobject::name::Name::none(),
            started_at: 2.0,
            mesh_path: None,
            animation_path: None,
            elapsed_seconds: 0.25,
            duration_seconds: Some(1.0),
            finish_after_seconds: Some(0.9),
            rate: 1.0,
            looped: true,
            tweening: false,
        };
        let delay = animation_finish_delay(&animation).expect("authored timing");
        assert!((delay - 0.65).abs() < 1.0e-6);
    }

    #[test]
    fn finish_interpolation_polls_until_each_actor_flag_clears() {
        let Synth {
            mut engine,
            class,
            actor_a,
            actor_b,
            ..
        } = synth_world();
        let resumed = engine.world.arena.names.intern("InterpolationResumed");
        let property = engine.world.arena.alloc(UObject {
            class_id: None,
            name_index: resumed,
            outer: Some(class),
            flags: 0,
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Int,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
        });
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), vec![property]);
        let code = vec![
            0x61,
            0x2D,
            USToken::EndFunctionParms as u8,
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            1,
            USToken::IntOne as u8,
            USToken::Stop as u8,
        ];
        let frame = |actor| ScriptFrame {
            code: code.clone(),
            resolver: resolver.clone(),
            pc: 0,
            self_id: actor,
            state: None,
            locals: PropStore::new(),
            nested_call: None,
            iterators: Vec::new(),
            wake: Wake::Ready,
            origin: Some("InterpolationProbe".to_string()),
        };
        engine
            .set_actor_bool(actor_a, "bInterpolating", true)
            .expect("mark A interpolating");
        engine.actor_scripts.insert(actor_a, frame(actor_a));
        engine.actor_scripts.insert(actor_b, frame(actor_b));

        // Both calls suspend at the native boundary. B starts non-interpolating,
        // but is still deferred until the next scheduler pass.
        engine.tick(&[]).expect("initial latent calls");
        for actor in [actor_a, actor_b] {
            let script = engine.actor_scripts.get(&actor).expect("parked");
            assert_eq!(script.pc, 3);
            assert_eq!(script.wake, Wake::Interpolation);
        }

        // B wakes exactly once on the following poll, while A remains parked.
        engine.tick(&[]).expect("first poll");
        assert!(!engine.actor_scripts.contains_key(&actor_b));
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_b)
                .expect("B")
                .properties()
                .and_then(|store| store.get(resumed)),
            Some(&PropValue::Int(1))
        );
        assert_eq!(
            engine.actor_scripts.get(&actor_a).map(|script| script.pc),
            Some(3)
        );

        // A remains paused through more polls until its issuing actor clears
        // bInterpolating, then resumes from the opcode after the native.
        engine.tick(&[]).expect("blocked poll");
        assert!(engine.actor_scripts.contains_key(&actor_a));
        engine
            .set_actor_bool(actor_a, "bInterpolating", false)
            .expect("clear interpolation");
        engine.tick(&[]).expect("cleared poll");
        assert!(!engine.actor_scripts.contains_key(&actor_a));
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_a)
                .expect("A")
                .properties()
                .and_then(|store| store.get(resumed)),
            Some(&PropValue::Int(1))
        );
        engine.tick(&[]).expect("post-resume poll");
        assert!(!engine.actor_scripts.contains_key(&actor_a));
        assert!(!engine.actor_scripts.contains_key(&actor_b));
    }

    #[test]
    fn synthetic_state_machine_parks_resumes_and_switches_states() {
        let Synth {
            mut engine,
            actor_a,
            running_id,
            done_id,
            done_name,
            seen_name,
            count_name,
            ticked_name,
            callee_resumed_name,
            ..
        } = synth_world();
        // Broadcast dispatch reaches every actor; keep only A so the
        // counters below describe exactly one machine.
        engine.level.actors = vec![actor_a];
        // Trigger dispatch enters Running with a Ready frame.
        engine.run_actor_event_on("Trigger", 0.0).expect("trigger");
        let script = engine.actor_scripts.get(&actor_a).expect("parked");
        assert_eq!(script.state, Some(running_id));
        assert_eq!(script.wake, Wake::Ready);
        assert_eq!(script.pc, 0);
        assert_eq!(engine.counters.states_entered, 1);

        // Tick 1 (t=0.1): WaitThen's Sleep(0.5) parks the state frame
        // PAST its call and retains the callee's own pc/locals.
        engine.tick(&[]).expect("tick");
        let t = engine.sim_time;
        let script = engine.actor_scripts.get(&actor_a).expect("still parked");
        assert_eq!(script.pc, 3, "outer cursor is past WaitThen");
        assert!(
            script.nested_call.is_some(),
            "the callee continuation is persisted with the outer frame"
        );
        assert_eq!(script.wake, Wake::At(t + 0.5));
        assert_eq!(engine.counters.latent_sleeps, 1);

        // Ticks before the wake instant (t=0.2..0.5 < 0.6) leave the
        // continuation untouched (resume continues, never restarts).
        for _ in 0..4 {
            engine.tick(&[]).expect("idle ticks");
        }
        assert_eq!(
            engine.actor_scripts.get(&actor_a).expect("parked").pc,
            3,
            "the outer call site is not restarted before the child wakes"
        );

        // Wake tick (t=0.6): SetTimer arms +0.2s; GotoState('Done') swaps
        // the active body and abandons the old continuation.
        engine.tick(&[]).expect("wake tick");
        assert_eq!(engine.actor_states.get(&actor_a), Some(&done_id));
        assert_eq!(engine.counters.set_timers, 1);
        assert_eq!(engine.timers.len(), 1);
        let timer_at = engine.timers[0].at;
        let interval = f64::from(engine.timers[0].interval);
        assert!((timer_at - (engine.sim_time + interval)).abs() < 1e-9);
        let new_frame = engine.actor_scripts.get(&actor_a).expect("Done frame");
        assert_eq!(new_frame.state, Some(done_id));
        assert_eq!(new_frame.origin, None, "state frame is independent of Tick");
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_a)
                .expect("actor")
                .properties()
                .and_then(|store| store.get(callee_resumed_name)),
            Some(&PropValue::Int(1)),
            "the suspended callee completed before Running resumed"
        );

        // Tick (t=0.7): Done stops immediately while Tick remains a
        // separately dispatched probe and state membership persists for
        // GetStateName.
        engine.tick(&[]).expect("done tick");
        assert!(!engine.actor_scripts.contains_key(&actor_a));
        assert_eq!(engine.actor_states.get(&actor_a), Some(&done_id));
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_a)
                .expect("actor")
                .properties()
                .and_then(|store| store.get(ticked_name)),
            Some(&PropValue::Int(1)),
            "active state code must not suppress the actor Tick probe"
        );

        // Timer tick (t=0.8): the fired event records GetStateName() and
        // a constant into instance storage — writes applied, not lost.
        engine.tick(&[]).expect("timer tick");
        assert_eq!(engine.counters.timers_fired, 1);
        let store = engine
            .world
            .arena
            .get(actor_a)
            .expect("actor")
            .properties()
            .cloned()
            .expect("instance store");

        match store
            .get(seen_name)
            .cloned()
            .expect("GetStateName wrote Seen")
        {
            PropValue::Name(name) => assert_eq!(name.index, done_name),
            other => panic!("expected Name in Seen, got {other:?}"),
        }
        assert_eq!(
            store.get(count_name),
            Some(&PropValue::Int(42)),
            "timer-fired event ran to completion"
        );
    }
    #[test]
    fn enable_and_disable_control_probe_event_dispatch() {
        let Synth {
            mut engine,
            actor_a,
            seen_name,
            ..
        } = synth_world();
        engine
            .run_named_event_on(actor_a, "DisableTimer", None)
            .expect("Disable executes");
        assert!(engine.event_is_disabled(actor_a, "Timer"));
        engine
            .run_named_event_on(actor_a, "Timer", None)
            .expect("disabled Timer is skipped");
        assert_eq!(
            engine
                .world
                .arena
                .get(actor_a)
                .expect("actor")
                .properties()
                .and_then(|store| store.get(seen_name)),
            None
        );
        engine
            .run_named_event_on(actor_a, "EnableTimer", None)
            .expect("Enable executes");
        assert!(!engine.event_is_disabled(actor_a, "Timer"));
        engine
            .run_named_event_on(actor_a, "Timer", None)
            .expect("enabled Timer runs");
        assert!(
            engine
                .world
                .arena
                .get(actor_a)
                .expect("actor")
                .properties()
                .and_then(|store| store.get(seen_name))
                .is_some()
        );
    }

    #[test]
    fn startup_lifecycle_runs_all_phases_then_enters_auto_state_once() {
        use hp_format::package79::write_compact_index;
        use hp_uobject::arena::{FunctionData, PropertyData};
        use hp_uobject::value::PropertyKind;
        use hp_uobject::vm::USToken;

        let Synth {
            mut engine,
            class,
            actor_a,
            actor_b,
            running_id,
            ..
        } = synth_world();
        // A preset initialized actor remains in the level but must receive
        // none of the four startup phases.
        let initialized = engine.world.arena.names.intern("bScriptInitialized");
        engine
            .world
            .arena
            .get_mut(actor_b)
            .expect("actor b")
            .properties_mut()
            .expect("properties")
            .set(initialized, PropValue::Bool(true));
        let phase_names: Vec<u32> = ["SawPre", "SawBegin", "SawPost", "SawInitial"]
            .into_iter()
            .map(|name| engine.world.arena.names.intern(name))
            .collect();
        let properties: Vec<ObjectId> = phase_names
            .iter()
            .copied()
            .map(|name_index| {
                engine.world.arena.alloc(UObject {
                    name_index,
                    outer: Some(class),
                    data: ObjectData::Property(Box::new(PropertyData {
                        links: Default::default(),
                        kind: PropertyKind::Int,
                        array_dim: 1,
                        property_flags: 0,
                        category: 0,
                    })),
                    ..Default::default()
                })
            })
            .collect();
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), properties);
        for (index, event) in [
            "PreBeginPlay",
            "BeginPlay",
            "PostBeginPlay",
            "SetInitialState",
        ]
        .into_iter()
        .enumerate()
        {
            let mut code = vec![USToken::Let as u8, USToken::InstanceVariable as u8];
            write_compact_index(&mut code, index as i32 + 1);
            // Each phase copies the prior phase's marker; only Pre seeds 1.
            // Any permutation leaves a downstream marker at its default 0.
            if index == 0 {
                code.push(USToken::IntOne as u8);
            } else {
                code.push(USToken::InstanceVariable as u8);
                write_compact_index(&mut code, index as i32);
            }
            if event == "SetInitialState" {
                code.push(0x71); // Core.Object.GotoState
                code.push(USToken::NameConst as u8);
                let auto = engine.world.arena.names.intern("Auto");
                write_compact_index(&mut code, auto as i32);
                code.push(USToken::EndFunctionParms as u8);
            }
            code.push(USToken::Stop as u8);
            let name_index = engine.world.arena.names.intern(event);
            engine.world.arena.alloc(UObject {
                name_index,
                outer: Some(class),
                data: ObjectData::Function(Box::new(FunctionData {
                    code,
                    resolver: resolver.clone(),
                    ..Default::default()
                })),
                ..Default::default()
            });
        }
        let ObjectData::State(running) = &mut engine
            .world
            .arena
            .get_mut(running_id)
            .expect("Running")
            .data
        else {
            panic!("Running must be a state");
        };
        running.state_flags |= 0x0000_0002; // STATE_Auto, UnStack.h

        engine.run_startup_lifecycle().expect("startup lifecycle");
        let store = engine
            .world
            .arena
            .get(actor_a)
            .expect("actor")
            .properties()
            .expect("properties");
        for &name in &phase_names {
            assert_eq!(
                store.get(name),
                Some(&PropValue::Int(1)),
                "each lifecycle phase must execute"
            );
        }
        let preset = engine
            .world
            .arena
            .get(actor_b)
            .expect("preset actor")
            .properties()
            .expect("properties");
        for &name in &phase_names {
            assert_eq!(
                preset.get(name),
                None,
                "bScriptInitialized actor must skip every startup phase"
            );
        }
        assert!(!engine.actor_states.contains_key(&actor_b));
        assert_eq!(engine.actor_states.get(&actor_a), Some(&running_id));
        assert_eq!(engine.counters.states_entered, 1);
        assert_eq!(
            engine.actor_scripts.get(&actor_a).map(|frame| frame.wake),
            Some(Wake::Ready),
            "auto-state Begin frame is installed before the first Tick"
        );
        engine.tick(&[]).expect("first state slice");
        assert_eq!(
            engine.counters.states_entered, 1,
            "the auto state is not restarted by Tick"
        );
    }

    #[test]
    fn startup_phase_rechecks_initialized_after_each_pass() {
        use hp_format::package79::write_compact_index;
        use hp_uobject::arena::{FunctionData, PropertyData};
        use hp_uobject::value::PropertyKind;
        use hp_uobject::vm::USToken;

        let Synth {
            mut engine, class, ..
        } = synth_world();
        let derived_name = engine.world.arena.names.intern("InitializesInPre");
        let derived_outer = engine.world.arena.get(class).expect("class").outer;
        let derived = engine.world.arena.alloc(UObject {
            name_index: derived_name,
            outer: derived_outer,
            data: ObjectData::Class(ClassData {
                super_class: Some(class),
                ..Default::default()
            }),
            ..Default::default()
        });
        let initialized = engine.world.arena.names.intern("bScriptInitialized");
        let later = engine.world.arena.names.intern("SawLaterPhase");
        let mut property = |name_index, kind| {
            engine.world.arena.alloc(UObject {
                name_index,
                outer: Some(derived),
                data: ObjectData::Property(Box::new(PropertyData {
                    links: Default::default(),
                    kind,
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            })
        };
        let initialized_property = property(initialized, PropertyKind::Bool);
        let later_property = property(later, PropertyKind::Int);
        let resolver = BytecodeResolver::from_package_refs(
            Vec::new(),
            vec![initialized_property, later_property],
        );
        let mut function = |event: &str, property_ref: i32, value: u8| {
            let mut code = vec![USToken::Let as u8, USToken::InstanceVariable as u8];
            write_compact_index(&mut code, property_ref);
            code.extend_from_slice(&[value, USToken::Stop as u8]);
            let event_name = engine.world.arena.names.intern(event);
            engine.world.arena.alloc(UObject {
                name_index: event_name,
                outer: Some(derived),
                data: ObjectData::Function(Box::new(FunctionData {
                    code,
                    resolver: resolver.clone(),
                    ..Default::default()
                })),
                ..Default::default()
            });
        };
        function("PreBeginPlay", 1, USToken::True as u8);
        function("BeginPlay", 2, USToken::IntOne as u8);
        let actor_name = engine.world.arena.names.intern("MidPhaseActor");
        let actor = engine.world.arena.alloc(UObject {
            class_id: Some(derived),
            name_index: actor_name,
            outer: Some(engine.level.root),
            data: ObjectData::Properties(PropStore::new()),
            ..Default::default()
        });
        engine.level.actors = vec![actor];

        engine.run_startup_lifecycle().expect("startup");
        let store = engine
            .world
            .arena
            .get(actor)
            .expect("actor")
            .properties()
            .expect("properties");
        assert_eq!(store.get(initialized), Some(&PropValue::Bool(true)));
        assert_eq!(
            store.get(later),
            None,
            "setting bScriptInitialized in PreBeginPlay must suppress every later phase"
        );
    }
    #[test]
    fn missing_state_is_no_state_but_malformed_code_is_loud() {
        let Synth {
            mut engine,
            actor_a,
            done_id,
            ..
        } = synth_world();
        engine
            .enter_state(actor_a, StateChange::Named("Missing".into()))
            .expect("retail unresolved GotoState enters no state");
        assert!(!engine.actor_states.contains_key(&actor_a));
        if let ObjectData::State(state) = &mut engine
            .world
            .arena
            .get_mut(done_id)
            .expect("Done state")
            .data
        {
            state.code.clear();
        } else {
            panic!("Done must be a state");
        }
        assert_eq!(
            engine
                .enter_state(actor_a, StateChange::Named("Done".into()))
                .expect_err("empty state code must fail")
                .reason_code,
            "vm.state_code_empty"
        );
    }

    #[test]
    fn execution_guards_are_never_script_deferrals() {
        for reason in [
            "vm.call_depth_exceeded",
            "vm.step_budget_exceeded",
            "vm.write_budget_exceeded",
            "vm.latent_budget_exceeded",
            "vm.switch_chain_unterminated",
            "vm.label_table_unterminated",
        ] {
            assert!(
                !Engine::is_deferred_reason(reason),
                "{reason} must abort instead of blacklisting a script"
            );
        }
    }

    #[test]
    fn timers_fire_in_wake_time_order_across_actors() {
        let Synth {
            mut engine,
            actor_a,
            actor_b,
            seen_name,
            ..
        } = synth_world();
        // A triggers at t=0; B one tick later — B's whole schedule trails
        // A's by exactly one tick.

        engine
            .run_named_event_on(actor_a, "Trigger", None)
            .expect("trigger a");
        engine.tick(&[]).expect("tick"); // t=0.1: A sleeps until 0.6
        engine
            .run_named_event_on(actor_b, "Trigger", None)
            .expect("trigger b");
        engine.tick(&[]).expect("tick"); // t=0.2: B sleeps until 0.7
        // Drive through A's timer firing at t=0.8 (six more ticks).
        for _ in 0..6 {
            engine.tick(&[]).expect("ticks");
        }
        assert_eq!(
            engine.counters.timers_fired, 1,
            "only A's timer is due by t=0.8"
        );
        let store_b = engine
            .world
            .arena
            .get(actor_b)
            .expect("actor b")
            .properties()
            .and_then(|s| s.get(seen_name))
            .cloned();
        assert_eq!(
            store_b, None,
            "the later-wake actor's timer must not fire early"
        );
        let store_a = engine
            .world
            .arena
            .get(actor_a)
            .expect("actor a")
            .properties()
            .and_then(|s| s.get(seen_name))
            .cloned();
        assert!(store_a.is_some(), "the earlier-wake actor fired first");
    }

    #[test]
    fn privetdr_prebegin_rejects_authored_odds_exclusion() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine =
            Engine::bootstrap(&root, ini, EngineOptions::default()).expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("map lifecycle");
        let template = engine
            .world
            .arena
            .find_by_path("PrivetDr.TriggerChangeLevel0")
            .expect("retained trigger");
        let class = engine.world.arena.get(template).unwrap().class_id.unwrap();
        let default_object = match &engine.world.arena.get(class).unwrap().data {
            ObjectData::Class(class) => class.default_object.expect("class default object"),
            _ => panic!("actor class"),
        };
        let odds = engine
            .world
            .arena
            .names
            .find_index("OddsOfAppearing")
            .unwrap();
        let game_relevant = engine
            .world
            .arena
            .names
            .find_index("bGameRelevant")
            .unwrap();
        let defaults = engine
            .world
            .arena
            .get_mut(default_object)
            .unwrap()
            .properties_mut()
            .unwrap();
        defaults.set(odds, PropValue::Float(-1.0));
        defaults.set(game_relevant, PropValue::Bool(false));
        let actor = engine
            .spawn_script_actor(class, None, [0.0; 3], [0; 3])
            .expect("synthetic authored actor");
        assert!(
            !engine.level.actors.contains(&actor),
            "Spawn must synchronously run PreBeginPlay and reject excluded actors"
        );
    }

    #[test]
    fn privetdr_spawnthingy_class_import_resolves_before_spawn() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine =
            Engine::bootstrap(&root, ini, EngineOptions::default()).expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("map lifecycle");
        let spawnthingy = engine
            .world
            .arena
            .find_by_path("PrivetDr.SpawnThingy0")
            .expect("authored OwlHootSound SpawnThingy");
        let spawn_class_name = engine
            .world
            .arena
            .names
            .find_index("SpawnClass")
            .expect("SpawnClass");
        let class = match engine
            .world
            .arena
            .get(spawnthingy)
            .expect("SpawnThingy0")
            .properties()
            .and_then(|properties| properties.get(spawn_class_name))
        {
            Some(PropValue::Object(Some(raw))) if *raw >= 0 => ObjectId(*raw as u32),
            other => panic!("SpawnClass map import was not resolved: {other:?}"),
        };
        assert!(matches!(
            engine
                .world
                .arena
                .get(class)
                .expect("resolved SpawnClass")
                .data,
            ObjectData::Class(_)
        ));
        let actor_class = engine
            .world
            .arena
            .find_by_path("Engine.Actor")
            .expect("Actor class");
        assert!(
            engine
                .world
                .arena
                .class_is_a(class, actor_class)
                .expect("SpawnClass ancestry"),
            "resolved SpawnClass must be spawnable"
        );
        assert_eq!(
            engine.world.arena.path_of(class).expect("SpawnClass path"),
            "HPParticle.DizzyStars"
        );
    }

    #[test]
    fn privetdr_cutscene3_lifecycle_reaches_idle_through_begin_state() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine = Engine::bootstrap(
            &root,
            ini,
            EngineOptions {
                fixed_dt: Some(0.1),
                ..Default::default()
            },
        )
        .expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("map lifecycle");
        assert!(
            engine
                .player_game_state()
                .is_some_and(|state| state.eq_ignore_ascii_case("GSTATE000")),
            "PrivetDr startup must screen actors using PlayerPawn.CurrentGameState"
        );
        // HGame import -129 must bind ColorParams.Base before the delayed
        // CutScene thread startup reaches spawned CutScript.BeginState.
        let cutscene = engine
            .world
            .arena
            .find_by_path("PrivetDr.CutScene3")
            .expect("serialized PrivetDr.CutScene3");
        let bool_prop = |engine: &Engine, actor: ObjectId, property: &str| {
            engine
                .world
                .arena
                .names
                .find_index(property)
                .and_then(|name| engine.world.arena.get(actor).ok()?.properties()?.get(name))
                .is_some_and(|value| matches!(value, PropValue::Bool(true)))
        };
        assert!(
            engine.level.actors.contains(&cutscene),
            "CutScene3 must remain active after source relevance screening"
        );
        assert!(
            bool_prop(&engine, cutscene, "bDoImmediateStart"),
            "CutScene3 PostBeginPlay must observe bLevelLoadStarts"
        );
        assert!(
            bool_prop(&engine, cutscene, "bScriptInitialized"),
            "Actor.SetInitialState must run after PostBeginPlay"
        );
        let state = engine
            .active_state_name(cutscene)
            .and_then(|name| engine.world.arena.names.text(name))
            .unwrap_or("None")
            .to_string();
        assert_eq!(
            state.to_ascii_lowercase(),
            "idle",
            "auto disabled.BeginState must transition CutScene3 to idle"
        );
        assert_eq!(
            engine.actor_scripts.get(&cutscene).map(|frame| frame.wake),
            Some(Wake::Ready),
            "idle Begin code must be active before the first Tick"
        );
        let cutscript_class = engine
            .world
            .arena
            .find_by_path("HGame.CutScript")
            .expect("CutScript class");
        let line_name = engine
            .world
            .arena
            .names
            .find_index("curScriptLine")
            .unwrap();
        let cue_name = engine.world.arena.names.find_index("nCues").unwrap();
        let playing_name = engine.world.arena.names.find_index("bPlaying").unwrap();
        let done_name = engine.world.arena.names.find_index("bDone");
        let captured_name = engine
            .world
            .arena
            .names
            .find_index("aCapturedActors")
            .unwrap();
        let cut_name = engine.world.arena.names.find_index("CutName").unwrap();
        let mut scheduler_checkpoints = Vec::new();
        let mut threads = Vec::new();
        let mut line_checkpoints = Vec::new();
        let mut spline_pose_checkpoints = Vec::new();
        let mut music_play_tick = None;
        for tick in 1..=600 {
            engine.tick(&[]).expect("PrivetDr CutScript progression");
            if music_play_tick.is_none()
                && engine
                    .sound_cues
                    .iter()
                    .any(|cue| cue.starts_with("PlayMusic Music_temp_E3_cut_scene handle="))
            {
                music_play_tick = Some(tick);
            }
            if tick == 1 {
                assert!(
                    !bool_prop(&engine, cutscene, "bDoImmediateStart"),
                    "first slice consumes the level-load start latch"
                );
                assert!(engine.counters.latent_sleeps > 0);
                let frame = engine
                    .actor_scripts
                    .get(&cutscene)
                    .expect("CutScene sleep continuation");
                assert!(frame.pc > 0);
                assert!(matches!(frame.wake, Wake::At(_)));
            }
            if tick == 10 {
                threads = engine
                    .level
                    .actors
                    .iter()
                    .copied()
                    .filter(|actor| {
                        engine
                            .world
                            .arena
                            .get(*actor)
                            .ok()
                            .and_then(|object| object.class_id)
                            .is_some_and(|class| {
                                engine
                                    .world
                                    .arena
                                    .class_is_a(class, cutscript_class)
                                    .unwrap_or(false)
                            })
                    })
                    .collect();
                assert!(
                    !threads.is_empty(),
                    "CreateThreads must spawn CutScriptDisk actors"
                );
                assert!(threads.iter().all(|actor| {
                    engine
                        .active_state_name(*actor)
                        .and_then(|name| engine.world.arena.names.text(name))
                        .is_some_and(|name| name.eq_ignore_ascii_case("Running"))
                }));
                assert!(
                    threads.iter().any(|actor| {
                        let props = engine
                            .world
                            .arena
                            .get(*actor)
                            .unwrap()
                            .properties()
                            .unwrap();
                        matches!(props.get(cue_name), Some(PropValue::Int(cues)) if *cues > 0)
                    }),
                    "an authored CUE command must update the CutScript cue state"
                );
                let captured: Vec<_> = threads
                    .iter()
                    .flat_map(|thread| {
                        let values = engine
                            .world
                            .arena
                            .get(*thread)
                            .ok()
                            .and_then(|object| object.properties())
                            .and_then(|props| props.get(captured_name));
                        match values {
                            Some(PropValue::FixedArray(values) | PropValue::Array(values)) => {
                                values
                                    .iter()
                                    .filter_map(|value| match value {
                                        PropValue::Object(Some(raw)) if *raw >= 0 => {
                                            Some(ObjectId(*raw as u32))
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                            }
                            _ => Vec::new(),
                        }
                    })
                    .collect();
                for alias in ["BASECAM", "harry", "oWl"] {
                    assert!(
                        captured.iter().any(|actor| {
                            matches!(
                                engine.actor_effective_value(*actor, cut_name),
                                Ok(Some(PropValue::Str(name)))
                                    if name.eq_ignore_ascii_case(alias)
                            )
                        }),
                        "CAPTURE must register {alias} through the shared case-insensitive CutName table"
                    );
                }
            }
            if matches!(tick, 10 | 120 | 600) {
                let max_line = threads
                    .iter()
                    .filter_map(|actor| {
                        engine
                            .world
                            .arena
                            .get(*actor)
                            .ok()?
                            .properties()?
                            .get(line_name)
                            .and_then(|value| match value {
                                PropValue::Int(line) => Some(*line),
                                _ => None,
                            })
                    })
                    .max()
                    .unwrap_or(0);
                line_checkpoints.push((tick, max_line));
                let actor = threads[0];
                let props = engine.world.arena.get(actor).unwrap().properties().unwrap();
                let state = engine
                    .active_state_name(actor)
                    .and_then(|name| engine.world.arena.names.text(name))
                    .unwrap_or("None")
                    .to_string();
                let frame = engine
                    .actor_scripts
                    .get(&actor)
                    .map(|frame| (frame.pc, frame.wake));
                let playing = props.get(playing_name).cloned();
                let done = done_name.and_then(|name| props.get(name)).cloned();
                scheduler_checkpoints.push((tick, state, frame, playing, done, max_line));
                spline_pose_checkpoints.push(
                    ["BaseCam", "Owl"]
                        .into_iter()
                        .filter_map(|name| {
                            engine
                                .find_actor_by_cut_name(name)
                                .and_then(|actor| engine.actor_location(actor))
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
        // Source line 2 follows `sleep 1.5`; allow the dependent thread to
        // reach it, but do not wait for a StopMusic that DestroyTrigger.uc
        // never authors.
        for tick in 601..=1200 {
            if music_play_tick.is_some() {
                break;
            }
            engine
                .tick(&[])
                .expect("post-checkpoint CutScript animation timing");
            if engine
                .sound_cues
                .iter()
                .any(|cue| cue.starts_with("PlayMusic Music_temp_E3_cut_scene handle="))
            {
                music_play_tick = Some(tick);
            }
        }
        assert_eq!(line_checkpoints.len(), 3);
        assert_eq!(scheduler_checkpoints.len(), 3);
        assert_eq!(spline_pose_checkpoints.len(), 3);
        let basecam_positions: Vec<_> = spline_pose_checkpoints
            .iter()
            .filter_map(|poses| poses.first().copied())
            .collect();
        assert_eq!(
            basecam_positions.len(),
            spline_pose_checkpoints.len(),
            "every checkpoint must resolve the BaseCam through LevelInfo -> Harry -> cam"
        );
        assert!(
            basecam_positions
                .windows(2)
                .any(|pair| vector_distance(pair[0], pair[1]) > 1.0),
            "the authored BaseCam FollowSpline command must change the camera pose across checkpoints"
        );
        for (tick, state, frame, playing, done, line) in &scheduler_checkpoints {
            match state.to_ascii_lowercase().as_str() {
                "running" => {
                    assert_eq!(
                        *frame, None,
                        "Running has event Tick, not latent state code"
                    );
                    assert_eq!(
                        *playing,
                        Some(PropValue::Bool(true)),
                        "bPlaying at tick {tick}"
                    );
                    assert!(
                        done.is_none() || *done == Some(PropValue::Bool(false)),
                        "bDone at tick {tick}: {done:?}"
                    );
                }
                "finished" => {
                    assert_eq!(
                        *playing,
                        Some(PropValue::Bool(false)),
                        "Finished clears bPlaying at tick {tick}"
                    );
                    assert!(
                        done.is_none() || *done == Some(PropValue::Bool(true)),
                        "Finished bDone at tick {tick}: {done:?}"
                    );
                }
                other => panic!("unexpected CutScript state {other} at tick {tick}"),
            }
            assert!(*line > 0, "curScriptLine at tick {tick}");
        }
        assert!(
            matches!(
                scheduler_checkpoints
                    .last()
                    .unwrap()
                    .1
                    .to_ascii_lowercase()
                    .as_str(),
                "running" | "finished"
            ),
            "successful authored subject commands keep the CutScript thread live or finish it normally"
        );
        assert!(
            line_checkpoints[0].1 > 0,
            "Running.Tick consumes authored commands"
        );
        assert!(
            line_checkpoints
                .windows(2)
                .all(|pair| pair[1].1 >= pair[0].1),
            "CutScript line cursors must not regress at 10/120/600 ticks"
        );
        let music_trigger = engine
            .world
            .arena
            .find_by_path("PrivetDr.MusicTrigger0")
            .expect("authored MusicTrigger actor");
        let music_tag = engine
            .world
            .arena
            .names
            .find_index("MusicTrigger")
            .expect("authored MusicTrigger tag");
        let music_name = hp_uobject::name::Name {
            index: music_tag,
            number: hp_uobject::name::NO_NUMBER,
        };
        assert!(
            engine.actor_has_tag(music_trigger, music_name),
            "Actor.TriggerEvent must select PrivetDr.MusicTrigger0 by authored Tag"
        );
        let music_class = engine
            .world
            .arena
            .get(music_trigger)
            .expect("MusicTrigger actor")
            .class_id
            .expect("MusicTrigger class");
        let trigger_function = engine
            .resolve_event_function(music_class, "Trigger")
            .expect("resolve MusicTrigger.Trigger")
            .expect("MusicTrigger.Trigger exists");
        assert_eq!(
            engine.world.arena.path_of(trigger_function).expect("function path"),
            "Engine.MusicTrigger.Trigger"
        );
        let triggered_name = engine
            .world
            .arena
            .names
            .find_index("Triggered")
            .expect("MusicTrigger.Triggered");
        assert!(
            matches!(
                engine.actor_effective_value(music_trigger, triggered_name),
                Ok(Some(PropValue::Bool(true)))
            ),
            "authored MusicTrigger branch must mark itself Triggered before PlayMusic"
        );
        let harry_class = engine
            .world
            .arena
            .find_by_path("HGame.Harry")
            .expect("Harry class");
        let harry = engine
            .level
            .actors
            .iter()
            .copied()
            .find(|actor| {
                engine
                    .world
                    .arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .is_some_and(|class| engine.world.arena.class_is_a(class, harry_class).unwrap_or(false))
            })
            .expect("PlayerHarryActor");
        let (trigger_code, trigger_resolver) = match &engine
            .world
            .arena
            .get(trigger_function)
            .expect("MusicTrigger.Trigger")
            .data
        {
            ObjectData::Function(function) => (function.code.clone(), function.resolver.clone()),
            other => panic!("MusicTrigger.Trigger is not a function: {other:?}"),
        };
        engine
            .set_actor_bool(music_trigger, "Triggered", false)
            .expect("reset MusicTrigger branch for direct frame probe");
        let mut trigger_frame = hp_uobject::vm::Frame::with_resolver(
            &engine.world.arena,
            &engine.world.registry,
            &trigger_code,
            Some(music_trigger),
            trigger_resolver,
        );
        trigger_frame.locals.set(
            engine.world.arena.names.find_index("Other").expect("Other local"),
            PropValue::Object(None),
        );
        trigger_frame.locals.set(
            engine
                .world
                .arena
                .names
                .find_index("EventInstigator")
                .expect("EventInstigator local"),
            PropValue::Object(Some(harry.0 as i32)),
        );
        trigger_frame.run().expect("MusicTrigger.Trigger bytecode");
        assert!(
            trigger_frame.take_effects().iter().any(|effect| matches!(
                effect,
                ScriptEffect::PlayMusic { actor, song, .. }
                    if *actor == harry && song == "Music_temp_E3_cut_scene"
            )),
            "direct Engine.MusicTrigger.Trigger must emit C++-observed PlayMusic"
        );
        let actor_class = engine.world.arena.find_by_path("Engine.Actor").expect("Actor class");
        let cutscript_class = engine.world.arena.find_by_path("HGame.CutScript").expect("CutScript class");
        let cut_thread = engine
            .level
            .actors
            .iter()
            .copied()
            .find(|actor| {
                engine.world.arena.get(*actor).ok().and_then(|object| object.class_id)
                    .is_some_and(|class| engine.world.arena.class_is_a(class, cutscript_class).unwrap_or(false))
            })
            .expect("active CutScript thread");
        assert!(
            engine.level.actors.contains(&music_trigger),
            "MusicTrigger0 must remain in the active actor scope for direct fan-out"
        );
        engine
            .set_actor_bool(music_trigger, "Triggered", false)
            .expect("reset MusicTrigger before Actor.TriggerEvent");
        let trigger_event = engine
            .world
            .arena
            .find_function(actor_class, "TriggerEvent")
            .expect("Actor.TriggerEvent lookup")
            .expect("Actor.TriggerEvent");
        let prior_cues = engine.sound_cues.len();
        engine
            .run_function_frame(
                cut_thread,
                trigger_event,
                "TriggerEvent",
                None,
                &[
                    PropValue::Name(music_name),
                    PropValue::Object(Some(cut_thread.0 as i32)),
                    PropValue::Object(Some(harry.0 as i32)),
                ],
            )
            .expect("run authored Actor.TriggerEvent");
        assert!(
            engine.sound_cues[prior_cues..]
                .iter()
                .any(|cue| cue.starts_with("PlayMusic Music_temp_E3_cut_scene handle=")),
            "Actor.TriggerEvent fan-out must invoke MusicTrigger.Trigger"
        );
        let music_cue = engine
            .sound_cues
            .iter()
            .find(|cue| cue.starts_with("PlayMusic Music_temp_E3_cut_scene handle="))
            .expect("authored MusicTrigger must retain its PlayMusic effect");
        let handle = music_cue
            .rsplit_once("handle=")
            .and_then(|(_, handle)| handle.parse::<i32>().ok())
            .expect("PlayMusic cue carries an integer handle");
        assert!(handle > 0, "nonempty authored song receives a valid handle");
        assert!(
            music_play_tick.is_some_and(|tick| tick > 15),
            "line_2 PlayMusic must not run before authored sleep 1.5"
        );
        assert!(
            !engine
                .sound_cues
                .iter()
                .any(|cue| cue.starts_with("StopMusic handle=0")),
            "MusicTrigger.uc explicitly logs an invalid zero handle instead of stopping it"
        );
        assert!(
            !engine
                .sound_cues
                .iter()
                .any(|cue| cue.starts_with("StopMusic handle=")),
            "DestroyTrigger.uc destroys tagged actors; it does not synthesize StopMusic"
        );
        let snapshots = engine
            .actor_animation_snapshots()
            .expect("render animation snapshots");
        assert!(
            snapshots.values().any(|snapshot| {
                snapshot.elapsed_seconds > 0.0
                    || (snapshot.duration_seconds.is_none() && snapshot.animation_path.is_none())
            }),
            "real CutScript PlayAnim must expose progressed source time or an explicit no-data bind pose"
        );
    }

    #[test]
    fn privetdr_authored_sleep_18_5_waits_for_30hz_scheduler_deadline() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine = Engine::bootstrap(
            &root,
            ini,
            EngineOptions {
                fixed_dt: Some(1.0 / 30.0),
                ..Default::default()
            },
        )
        .expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("map lifecycle");
        let level_info_class = engine
            .world
            .arena
            .find_by_path("Engine.LevelInfo")
            .expect("LevelInfo class");
        let level_info = engine
            .level
            .actors
            .iter()
            .copied()
            .find(|actor| {
                engine
                    .world
                    .arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .is_some_and(|class| {
                        engine
                            .world
                            .arena
                            .class_is_a(class, level_info_class)
                            .unwrap_or(false)
                    })
            })
            .expect("active LevelInfo");
        let object_property = |engine: &Engine, actor: ObjectId, property: &str| {
            let name = engine.world.arena.names.find_index(property)?;
            match engine.actor_effective_value(actor, name).ok()?? {
                PropValue::Object(Some(raw)) if raw >= 0 => Some(ObjectId(raw as u32)),
                _ => None,
            }
        };
        let harry = object_property(&engine, level_info, "PlayerHarryActor")
            .expect("LevelInfo publishes authored player actor");
        let base_cam =
            object_property(&engine, harry, "cam").expect("Harry publishes spawned BaseCam");
        let cut_name = engine.world.arena.names.find_index("CutName").unwrap();
        assert!(
            matches!(
                engine.actor_effective_value(base_cam, cut_name).unwrap(),
                Some(PropValue::Str(name)) if name.eq_ignore_ascii_case("BaseCam")
            ),
            "CAPTURE/FindCutSubject must expose the actual BaseCam under its authored CutName"
        );

        let cutscript_class = engine
            .world
            .arena
            .find_by_path("HGame.CutScript")
            .expect("CutScript class");
        let cursor_name = engine
            .world
            .arena
            .names
            .find_index("curScriptLine")
            .unwrap();
        let lines_name = engine.world.arena.names.find_index("lineArray").unwrap();
        let pending_name = engine.world.arena.names.find_index("nPendingCues").unwrap();
        let cursor = |engine: &Engine, actor: ObjectId| {
            engine
                .world
                .arena
                .get(actor)
                .ok()?
                .properties()?
                .get(cursor_name)
                .and_then(|value| match value {
                    PropValue::Int(value) => Some(*value),
                    _ => None,
                })
        };

        let mut sleeper = None;
        for tick in 1..=30 {
            engine.tick(&[]).expect("reach authored Sleep");
            for actor in engine.level.actors.iter().copied() {
                let Some(class) = engine.world.arena.get(actor).ok().and_then(|o| o.class_id)
                else {
                    continue;
                };
                if !engine
                    .world
                    .arena
                    .class_is_a(class, cutscript_class)
                    .unwrap_or(false)
                {
                    continue;
                }
                let Some(line) = cursor(&engine, actor) else {
                    continue;
                };
                let Some(PropValue::FixedArray(lines) | PropValue::Array(lines)) = engine
                    .world
                    .arena
                    .get(actor)
                    .ok()
                    .and_then(|object| object.properties())
                    .and_then(|props| props.get(lines_name))
                else {
                    continue;
                };
                if line > 0
                    && lines
                        .get(line as usize - 1)
                        .is_some_and(|value| matches!(value, PropValue::Str(text) if text.trim().eq_ignore_ascii_case("SLEEP 18.5")))
                {
                    assert!(
                        engine.level.actors.iter().copied().any(|candidate| {
                            engine
                                .world
                                .arena
                                .get(candidate)
                                .ok()
                                .and_then(|object| object.properties())
                                .and_then(|props| props.get(lines_name))
                                .and_then(|value| match value {
                                    PropValue::FixedArray(lines) | PropValue::Array(lines) => {
                                        Some(lines)
                                    }
                                    _ => None,
                                })
                                .is_some_and(|lines| {
                                    lines.iter().any(|value| matches!(
                                        value,
                                        PropValue::Str(text)
                                            if text.to_ascii_lowercase().contains("time=12")
                                    ))
                                })
                        }),
                        "the shipped CutScene must carry its authored 12s camera command"
                    );
                    let pending = engine
                        .world
                        .arena
                        .get(actor)
                        .unwrap()
                        .properties()
                        .unwrap()
                        .get(pending_name);
                    assert!(
                        matches!(pending, Some(PropValue::Int(value)) if *value > 0),
                        "Sleep must own a pending auto cue"
                    );
                    let Some(deadline) = engine
                        .timers
                        .iter()
                        .find(|timer| {
                            timer.actor == actor && timer.at - engine.sim_time > 18.4
                        })
                        .map(|timer| timer.at)
                    else {
                        // Another CutScript can transiently have the same
                        // cursor value; ownership is proven by the 18.5s
                        // timer installed for this exact actor.
                        continue;
                    };
                    sleeper = Some((actor, line, tick, deadline));
                    break;
                }
            }
            if sleeper.is_some() {
                break;
            }
        }
        let (actor, parked_line, start_tick, deadline) = sleeper.unwrap_or_else(|| {
            let timers: Vec<_> = engine
                .timers
                .iter()
                .map(|timer| {
                    (
                        engine.world.arena.path_of(timer.actor).ok(),
                        timer.at - engine.sim_time,
                        cursor(&engine, timer.actor),
                    )
                })
                .collect();
            panic!("PrivetDr authored SLEEP 18.5 thread; timers={timers:?}")
        });
        let scheduled_at = deadline - 18.5;
        let frame = f64::from(engine.next_delta());
        assert!(
            scheduled_at >= 0.0
                && scheduled_at <= engine.sim_time
                && ((scheduled_at / frame) - (scheduled_at / frame).round()).abs() < 1.0e-4,
            "authored Sleep deadline must be exactly 18.5s after a 30Hz scheduler boundary: scheduled={scheduled_at} now={} deadline={deadline}",
            engine.sim_time
        );
        let mut last_line = parked_line;
        let mut tick = start_tick;
        while engine.sim_time + f64::from(engine.next_delta()) < deadline {
            engine.tick(&[]).expect("pre-deadline tick");
            tick += 1;
            let current = cursor(&engine, actor).expect("sleeping cursor");
            assert_eq!(
                current, parked_line,
                "Running must remain parked before the Sleep auto cue deadline"
            );
            assert!(current >= last_line, "line cursor must stay monotonic");
            last_line = current;
        }
        assert!(engine.sim_time < deadline);
        engine.tick(&[]).expect("deadline tick");
        tick += 1;
        assert!(engine.sim_time >= deadline);
        let after = cursor(&engine, actor).expect("resumed cursor");
        assert!(
            after > parked_line,
            "Running advances only after the scheduler delivers Sleep's cue"
        );
        assert!(after >= last_line);

        while tick < 600 {
            engine.tick(&[]).expect("600-tick release trace");
            tick += 1;
            let current = cursor(&engine, actor).expect("trace cursor");
            assert!(current >= last_line, "line cursor regressed at tick {tick}");
            last_line = current;
        }
    }

    #[test]
    fn privetdr_three_ticks_scheduler_stays_bounded_without_oom() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine = Engine::bootstrap(
            &root,
            ini,
            EngineOptions {
                rng_seed: DEFAULT_SEED,
                fixed_dt: None,
                editor_mode: false,
            },
        )
        .expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("map loads");
        let actors = engine.level.actors.len();
        for _ in 0..3 {
            engine.tick(&[]).expect("three-tick scheduler run");
        }
        let counters = engine.counters();
        // The historical Context-relative-skip failure repeatedly rebuilt
        // frames during the opening ticks until allocation exhausted. The
        // scheduler has one persisted continuation and one timer per actor.
        assert_eq!(counters.ticks, 3);
        assert!(engine.actor_scripts.len() <= actors);
        assert!(engine.timers.len() <= actors);
        let parked_bytes: usize = engine.actor_scripts.values().map(|s| s.code.len()).sum();
        assert!(
            parked_bytes < 8 * 1024 * 1024,
            "parked code ballooned to {parked_bytes}B"
        );
        println!(
            "scheduler: ticks={} states_entered={} sleeps={} finish_anims={} \
             set_timers={} timers_fired={} parked={} bytes={parked_bytes}",
            counters.ticks,
            counters.states_entered,
            counters.latent_sleeps,
            counters.latent_finish_anims,
            counters.set_timers,
            counters.timers_fired,
            engine.actor_scripts.len()
        );
    }
    #[test]
    fn add_pawn_effect_prepends_in_emission_order() {
        let Synth {
            mut engine,
            actor_a: pawn1,
            actor_b: pawn2,
            ..
        } = synth_world();
        let level_name = engine.world.arena.names.intern("Level");
        let pawn_list_name = engine.world.arena.names.intern("PawnList");
        let next_pawn_name = engine.world.arena.names.intern("nextPawn");
        let pawn_class = engine.world.arena.get(pawn1).unwrap().class_id.unwrap();
        let next_pawn_decl = engine.world.arena.alloc(hp_uobject::arena::UObject {
            name_index: next_pawn_name,
            outer: Some(pawn_class),
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Object { class_ref: None },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        if let ObjectData::Class(class) = &mut engine.world.arena.get_mut(pawn_class).unwrap().data
        {
            class.children.push(next_pawn_decl);
        }
        let level_class_name = engine.world.arena.names.intern("SyntheticLevelInfoClass");
        let level_class = engine.world.arena.alloc(hp_uobject::arena::UObject {
            name_index: level_class_name,
            outer: Some(engine.level.root),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let pawn_list_decl = engine.world.arena.alloc(hp_uobject::arena::UObject {
            name_index: pawn_list_name,
            outer: Some(level_class),
            data: ObjectData::Property(Box::new(hp_uobject::arena::PropertyData {
                links: Default::default(),
                kind: hp_uobject::value::PropertyKind::Object { class_ref: None },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        if let ObjectData::Class(class) = &mut engine.world.arena.get_mut(level_class).unwrap().data
        {
            class.children.push(pawn_list_decl);
        }
        let level_info_name = engine.world.arena.names.intern("SyntheticLevelInfo");
        let level_info = engine.world.arena.alloc(hp_uobject::arena::UObject {
            class_id: Some(level_class),
            name_index: level_info_name,
            outer: Some(engine.level.root),
            data: ObjectData::Properties(PropStore::new()),
            ..Default::default()
        });
        engine
            .world
            .arena
            .get_mut(level_info)
            .unwrap()
            .properties_mut()
            .unwrap()
            .set(pawn_list_name, PropValue::Object(None));
        for pawn in [pawn1, pawn2] {
            let properties = engine
                .world
                .arena
                .get_mut(pawn)
                .unwrap()
                .properties_mut()
                .unwrap();
            properties.set(level_name, PropValue::Object(Some(level_info.0 as i32)));
            properties.set(next_pawn_name, PropValue::Object(None));
        }

        engine
            .apply_script_effects(vec![
                ScriptEffect::AddPawn { pawn: pawn1 },
                ScriptEffect::AddPawn { pawn: pawn2 },
            ])
            .expect("ordered AddPawn effects");

        let level_store = engine
            .world
            .arena
            .get(level_info)
            .unwrap()
            .properties()
            .unwrap();
        assert_eq!(
            level_store.get(pawn_list_name),
            Some(&PropValue::Object(Some(pawn2.0 as i32)))
        );
        let pawn2_store = engine.world.arena.get(pawn2).unwrap().properties().unwrap();
        assert_eq!(
            pawn2_store.get(next_pawn_name),
            Some(&PropValue::Object(Some(pawn1.0 as i32)))
        );
        let pawn1_store = engine.world.arena.get(pawn1).unwrap().properties().unwrap();
        assert_eq!(
            pawn1_store.get(next_pawn_name),
            Some(&PropValue::Object(None))
        );
    }
    #[test]
    fn privetdr_basecam_shake_tracks_rotation_expiry_and_timed_cue_deadline() {
        fn expected_shake_rotation(
            rng: &mut SimRng,
            max_shake: f32,
            roll_magnitude: f32,
            envelope: f32,
        ) -> [i32; 3] {
            let yaw = (-max_shake + 2.0 * max_shake * rng.next_f32()) * envelope;
            let pitch = (-max_shake + 2.0 * max_shake * rng.next_f32()) * envelope;
            let roll =
                (-roll_magnitude + 2.0 * roll_magnitude * rng.next_f32()) * envelope;
            [pitch as i32, yaw as i32, roll as i32]
        }

        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine = Engine::bootstrap(
            &root,
            ini,
            EngineOptions {
                fixed_dt: Some(0.1),
                ..Default::default()
            },
        )
        .expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("PrivetDr map lifecycle");

        let mut captured = None;
        for _ in 0..30 {
            if let Some(snapshot) = engine.base_cam_snapshot()
                && let Some(thread) = snapshot.cut_notify_actor
            {
                captured = Some((snapshot.actor, thread));
                break;
            }
            engine.tick(&[]).expect("initialize and capture authored BaseCam");
        }
        let (base_cam, cut_thread) = captured.expect("loaded CutScript captures BaseCam");
        let class = engine.world.arena.get(cut_thread).expect("CutScript actor").class_id.expect("CutScript class");
        let function = engine.world.arena.find_function(class, "ParseCommand").expect("ParseCommand lookup").expect("loaded CutScript.ParseCommand");
        let (code, resolver, parameter) = match &engine.world.arena.get(function).expect("ParseCommand function").data {
            ObjectData::Function(function) => (function.code.clone(), function.resolver.clone(), *function.params.first().expect("command parameter")),
            _ => panic!("ParseCommand must be bytecode"),
        };
        let mut locals = PropStore::new();
        locals.set(
            engine.world.arena.get(parameter).expect("command parameter").name_index,
            PropValue::Str("BaseCam Shake Magnitude=100 Time=2".to_string()),
        );

        let shake_started_at = engine.sim_time;
        const SHAKE_RNG_SEED: u64 = 0x5348_414b_4552_4e47;
        engine.rng = SimRng::seeded(SHAKE_RNG_SEED);
        let mut expected_rng = SimRng::seeded(SHAKE_RNG_SEED);
        let timer_count = engine.timers.len();
        let authored_rotation = engine.actor_rotation(base_cam).expect("BaseCam rotation");
        engine.execute_frame(
            cut_thread, code, resolver, 0, locals, None, None, None, Vec::new(), None,
            Some("ParseCommand".to_string()),
            ScriptKey::Event(class, "ParseCommand".to_string()),
            "loaded CutScript.ParseCommand",
        ).expect("execute source-style BaseCam Shake");

        let active = engine.base_cam_snapshot().expect("BaseCam during Shake")
            .shake_intervals.into_iter().next().expect("ShakeView native state");
        assert_eq!(active.magnitude, 100.0);
        assert_eq!(active.max_shake, 100.0);
        assert_eq!(active.duration_seconds, 2.0);
        assert_eq!(active.started_at_seconds, shake_started_at);
        assert_eq!(active.ends_at_seconds, shake_started_at + 2.0);
        assert_eq!(active.elapsed_seconds, 0.0);
        assert_eq!(active.progress, 0.0);
        assert_eq!(active.rotation_delta, [0; 3]);
        assert_eq!(
            engine.cutscene_camera().expect("captured BaseCam before first ViewShake").rotation,
            authored_rotation,
            "ShakeView does not sample until its first elapsed-time advance"
        );
        assert_eq!(
            engine.rng.next_u32(),
            expected_rng.next_u32(),
            "starting ShakeView must not consume the shared simulation RNG"
        );
        assert!(
            engine.timers[timer_count..].iter()
                .any(|timer| (timer.at - (shake_started_at + 2.5)).abs() < 1.0e-6),
            "non-fast BaseCam Shake creates its TimedCue at Time + 0.5 seconds"
        );

        engine.begin_base_cam_shake(20.0, 100.0, 2.0);
        let mut expected_midpoint = [0; 3];
        for step in 1..=10 {
            let _ = expected_rng.next_f32();
            expected_midpoint = expected_shake_rotation(
                &mut expected_rng,
                100.0,
                20.0,
                1.0 - step as f32 * 0.05,
            );
            engine.tick(&[]).expect("advance ShakeView to midpoint");
            if step == 1 {
                let first = engine.base_cam_snapshot().expect("BaseCam after first shake sample")
                    .shake_intervals.into_iter().next().expect("active first shake sample");
                assert_eq!(first.elapsed_seconds, 0.1);
                assert_eq!(first.rotation_delta, expected_midpoint);
            }
        }
        let midpoint = engine.base_cam_snapshot().expect("BaseCam at shake midpoint")
            .shake_intervals.into_iter().next().expect("shake remains active at midpoint");
        assert!((midpoint.elapsed_seconds - 1.0).abs() < 1.0e-5);
        assert!((midpoint.progress - 0.5).abs() < 1.0e-5);
        assert_eq!(midpoint.rotation_delta, expected_midpoint);
        assert!(midpoint.rotation_delta[0].abs() <= 50);
        assert!(midpoint.rotation_delta[1].abs() <= 50);
        assert!(midpoint.rotation_delta[2].abs() <= 10);
        let midpoint_authored = engine.actor_rotation(base_cam).expect("BaseCam midpoint rotation");
        assert_eq!(
            engine.cutscene_camera().expect("midpoint shaken camera").rotation,
            std::array::from_fn(|component| midpoint_authored[component].wrapping_add(midpoint.rotation_delta[component]))
        );

        for step in 11..20 {
            let _ = expected_rng.next_f32();
            let _ = expected_shake_rotation(
                &mut expected_rng,
                100.0,
                20.0,
                1.0 - step as f32 * 0.05,
            );
            engine.tick(&[]).expect("advance active ShakeView toward expiry");
        }
        let _ = expected_rng.next_f32();
        engine.tick(&[]).expect("advance ShakeView to expiry");
        assert_eq!(
            engine.rng.next_u32(),
            expected_rng.next_u32(),
            "expiry consumes the ordinary tick draw but no ViewShake RandRange draws"
        );
        assert!(engine.base_cam_snapshot().expect("BaseCam after shake").shake_intervals.is_empty());
        assert_eq!(
            engine.cutscene_camera().expect("camera after shake expiry").rotation,
            engine.actor_rotation(base_cam).expect("authored BaseCam rotation"),
            "expired ShakeView leaves no residual rExtraRotation"
        );
    }

    #[test]
    fn privetdr_basecam_authored_splines_render_at_deadlines_and_release_to_pawn() {
        use crate::render_bridge::RendererSession;

        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let ini = hp_ini::load_pair(&root.join("System/Default.ini"), None).expect("ini");
        let mut engine = Engine::bootstrap(
            &root,
            ini,
            EngineOptions {
                fixed_dt: Some(0.1),
                ..Default::default()
            },
        )
        .expect("bootstrap");
        engine
            .load_map("..\\Maps\\PrivetDr.unr")
            .expect("PrivetDr map lifecycle");

        // BaseCam identity is source-owned, not inferred from actor counts:
        // LevelInfo.PlayerHarryActor -> Harry.cam.
        let level_info_class = engine
            .world
            .arena
            .find_by_path("Engine.LevelInfo")
            .expect("LevelInfo class");
        let level_info = engine
            .level
            .actors
            .iter()
            .copied()
            .find(|actor| {
                engine
                    .world
                    .arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .is_some_and(|class| {
                        engine
                            .world
                            .arena
                            .class_is_a(class, level_info_class)
                            .unwrap_or(false)
                    })
            })
            .expect("active LevelInfo");
        let harry = engine
            .actor_object_property(level_info, "PlayerHarryActor")
            .expect("LevelInfo.PlayerHarryActor");
        let base_cam = engine
            .actor_object_property(harry, "cam")
            .expect("Harry.cam");

        let scene = crate::scene::build_render_scene_with_world(&engine.world, &engine.level)
            .expect("PrivetDr render scene");
        let mut renderer = RendererSession::new(&root).expect("headless renderer");
        renderer
            .load_scene(&scene, &engine.world.archives)
            .expect("load PrivetDr render scene");
        let render_selected = |engine: &Engine, renderer: &mut RendererSession| {
            let pose = engine
                .cutscene_camera()
                .or_else(|| engine.pawn_camera())
                .expect("selected cutscene or pawn camera");
            renderer.apply_actor_transform_snapshots(&engine.actor_transform_snapshots());
            renderer.update_camera(pose.location, pose.rotation);
            renderer.render_frame();
            (pose, renderer.read_pixels_bgra())
        };

        let mut target = engine.actor_object_property(base_cam, "CamTarget");
        let mut cut_thread = engine.actor_object_property(base_cam, "CutNotifyActor");
        for _ in 0..30 {
            if target.is_some() && cut_thread.is_some() {
                break;
            }
            engine.tick(&[]).expect("initialize and capture authored BaseCam");
            target = engine.actor_object_property(base_cam, "CamTarget");
            cut_thread = engine.actor_object_property(base_cam, "CutNotifyActor");
        }
        let target = target.expect("BaseCam.CamTarget");
        let cut_thread = cut_thread.expect("loaded CutScript owns the captured BaseCam");
        let snapshot = engine.base_cam_snapshot().expect("live BaseCam snapshot");
        assert_eq!(snapshot.actor, base_cam);
        assert_eq!(snapshot.target, Some(target));
        assert_eq!(snapshot.cut_notify_actor, Some(cut_thread));
        assert_eq!(snapshot.sync_position_with_target, Some(false));
        assert_eq!(snapshot.sync_rotation_with_target, Some(true));
        assert!(
            snapshot.current_position.is_some()
                && snapshot.destination_position.is_some()
                && snapshot.current_rotation.is_some()
                && snapshot.destination_rotation.is_some(),
            "StateCutSceneCam exposes current and destination camera pose"
        );
        let target_state = snapshot
            .target_state
            .as_ref()
            .expect("BaseCamTarget state is represented");
        assert_eq!(target_state.actor, target);
        assert!(target_state.offset.is_some());
        assert!(target_state.relative.is_some());
        assert_eq!(snapshot.locked_to_target, Some(false));
        assert!(snapshot.settings.is_some(), "live CurrentSet is represented");
        assert_eq!(snapshot.camera_fly_to.actor, base_cam);
        assert_eq!(
            snapshot.target_fly_to.as_ref().map(|motion| motion.actor),
            Some(target)
        );
        assert!(snapshot.follow_splines.is_empty());
        assert!(matches!(
            snapshot.fov_intervals,
            BaseCamEffectIntervals::Available(_)
        ));
        assert!(matches!(
            snapshot.flash_intervals,
            BaseCamEffectIntervals::Available(_)
        ));
        assert!(matches!(
            snapshot.fade_intervals,
            BaseCamEffectIntervals::Available(_)
        ));
        assert!(snapshot.shake_intervals.is_empty());
        fn parse_loaded_cutscript(engine: &mut Engine, thread: ObjectId, command: &str) {
            let class = engine
                .world
                .arena
                .get(thread)
                .expect("CutScript actor")
                .class_id
                .expect("CutScript class");
            let function = engine
                .world
                .arena
                .find_function(class, "ParseCommand")
                .expect("ParseCommand lookup")
                .expect("loaded CutScript.ParseCommand");
            let (code, resolver, parameter) = match &engine
                .world
                .arena
                .get(function)
                .expect("ParseCommand function")
                .data
            {
                ObjectData::Function(function) => (
                    function.code.clone(),
                    function.resolver.clone(),
                    *function.params.first().expect("command parameter"),
                ),
                _ => panic!("ParseCommand must be bytecode"),
            };
            let parameter_name = engine
                .world
                .arena
                .get(parameter)
                .expect("command parameter")
                .name_index;
            let mut locals = PropStore::new();
            locals.set(parameter_name, PropValue::Str(command.to_string()));
            engine
                .execute_frame(
                    thread,
                    code,
                    resolver,
                    0,
                    locals,
                    None,
                    None,
                    None,
                    Vec::new(),
                    None,
                    Some("ParseCommand".to_string()),
                    ScriptKey::Event(class, "ParseCommand".to_string()),
                    "loaded CutScript.ParseCommand",
                )
                .expect("execute loaded CutScript command");
        }
        fn parse_command_effects(
            engine: &Engine,
            thread: ObjectId,
            command: &str,
        ) -> (PropValue, Vec<ScriptEffect>) {
            let class = engine.world.arena.get(thread).expect("CutScript actor").class_id.expect("CutScript class");
            let function = engine.world.arena.find_function(class, "ParseCommand").expect("ParseCommand lookup").expect("loaded CutScript.ParseCommand");
            let (code, resolver, parameter) = match &engine.world.arena.get(function).expect("ParseCommand function").data {
                ObjectData::Function(function) => (function.code.clone(), function.resolver.clone(), *function.params.first().expect("command parameter")),
                _ => panic!("ParseCommand must be bytecode"),
            };
            let mut frame = Frame::with_resolver(
                &engine.world.arena, &engine.world.registry, &code, Some(thread), resolver,
            );
            frame.set_actor_scope(&engine.level.actors);
            frame.set_next_effect_request_id(engine.next_script_request_id);
            frame.set_active_state_context(
                engine.active_state_name(thread),
                engine.actor_states.get(&thread).copied(),
            );
            frame.locals.set(engine.world.arena.get(parameter).expect("command parameter").name_index, PropValue::Str(command.to_string()));
            let result = frame.run().expect("run loaded CutScript.ParseCommand");
            assert_eq!(frame.suspended(), None, "ParseCommand must complete before effect inspection");
            (result, frame.take_effects())
        }
        let actor_class = engine.world.arena.find_by_path("Engine.Actor").expect("Actor class");
        let delimiter = engine
            .world
            .arena
            .find_function(actor_class, "ParseDelimitedString")
            .expect("ParseDelimitedString lookup")
            .expect("Actor.ParseDelimitedString");
        assert!(
            matches!(
                &engine.world.arena.get(delimiter).expect("delimiter function").data,
                ObjectData::Function(function) if function.params.len() >= 3
            ),
            "loaded Actor.ParseDelimitedString must retain Text, Delimiter, and Count parameters"
        );
        let (trigger_result, trigger_effects) =
            parse_command_effects(&engine, cut_thread, "TRIGGER MusicTrigger");
        assert_eq!(trigger_result, PropValue::Bool(true));
        let _trigger_effect = trigger_effects.iter().find(|effect| matches!(
            effect,
            ScriptEffect::PlayMusic { actor, song, .. }
                if *actor == harry && song == "Music_temp_E3_cut_scene"
        )).unwrap_or_else(|| panic!("CutScript.ParseCommand emitted no MusicTrigger PlayMusic effect: {trigger_effects:?}"));
        parse_loaded_cutscript(&mut engine, cut_thread, "BaseCam IgnoreTargetOn");
        let ignored = engine.base_cam_snapshot().expect("IgnoreTargetOn snapshot");
        assert_eq!(ignored.ignore_target, Some(true));
        assert_eq!(ignored.sync_position_with_target, Some(false));
        assert_eq!(ignored.sync_rotation_with_target, Some(false));
        parse_loaded_cutscript(&mut engine, cut_thread, "BaseCam IgnoreTargetOff");
        assert_eq!(
            engine
                .base_cam_snapshot()
                .expect("IgnoreTargetOff snapshot")
                .ignore_target,
            Some(false)
        );

        parse_loaded_cutscript(
            &mut engine,
            cut_thread,
            "BaseCam Locked Distance=321 RotTightness=4 MoveTightness=5",
        );
        for _ in 0..100 {
            if engine
                .base_cam_snapshot()
                .is_some_and(|snapshot| snapshot.locked_to_target == Some(true))
            {
                break;
            }
            engine.tick(&[]).expect("resume loaded Locked dispatch");
        }
        let locked = engine.base_cam_snapshot().expect("Locked snapshot");
        assert_eq!(locked.locked_to_target, Some(true));
        let locked_settings = locked.settings.expect("Locked CurrentSet");
        assert_eq!(locked_settings.look_at_distance, Some(321.0));
        assert_eq!(locked_settings.rotation_tightness, Some(4.0));
        assert_eq!(locked_settings.movement_tightness, Some(5.0));
        assert_eq!(locked_settings.unavailable_reason_code, None);
        parse_loaded_cutscript(&mut engine, cut_thread, "BaseCam UnLock");
        let unlocked = engine.base_cam_snapshot().expect("UnLock snapshot");
        assert_eq!(unlocked.locked_to_target, Some(false));
        assert_eq!(unlocked.sync_rotation_with_target, Some(true));

        let original_target_snapshot = engine
            .base_cam_snapshot()
            .expect("BaseCam before live target transition");
        let original_target_state = original_target_snapshot
            .target_state
            .clone()
            .expect("live BaseCamTarget before property transition");
        let original_target_fly_to = original_target_snapshot
            .target_fly_to
            .expect("live target FlyTo before property transition");
        engine
            .set_actor_object(target, "aAttachedTo", Some(harry))
            .expect("attach live BaseCamTarget");
        engine
            .set_actor_vector(target, "vOffset", [12.0, 34.0, 56.0])
            .expect("offset live BaseCamTarget");
        engine
            .set_actor_bool(target, "bRelative", true)
            .expect("make live BaseCamTarget relative");
        engine
            .set_actor_float(target, "fFlyToTime", 1.5)
            .expect("advance target FlyTo clock");
        engine
            .set_actor_float(target, "fFlyToTimeSpan", 4.0)
            .expect("set target FlyTo duration");
        engine
            .set_actor_vector(target, "vFlyToDest", [90.0, 80.0, 70.0])
            .expect("set target FlyTo destination");
        let transitioned_target = engine
            .base_cam_snapshot()
            .expect("BaseCam after live target transition");
        let transitioned_state = transitioned_target
            .target_state
            .expect("transitioned BaseCamTarget");
        assert_eq!(transitioned_state.attached_to, Some(harry));
        assert_eq!(transitioned_state.offset, Some([12.0, 34.0, 56.0]));
        assert_eq!(transitioned_state.relative, Some(true));
        let transitioned_fly_to = transitioned_target
            .target_fly_to
            .expect("transitioned target FlyTo");
        assert_eq!(transitioned_fly_to.elapsed_seconds, Some(1.5));
        assert_eq!(transitioned_fly_to.duration_seconds, Some(4.0));
        assert_eq!(transitioned_fly_to.destination, Some([90.0, 80.0, 70.0]));
        engine
            .set_actor_object(target, "aAttachedTo", original_target_state.attached_to)
            .expect("restore target attachment");
        engine
            .set_actor_vector(
                target,
                "vOffset",
                original_target_state.offset.unwrap_or([0.0; 3]),
            )
            .expect("restore target offset");
        engine
            .set_actor_bool(
                target,
                "bRelative",
                original_target_state.relative.unwrap_or(false),
            )
            .expect("restore target relative mode");
        if let Some(elapsed) = original_target_fly_to.elapsed_seconds {
            engine
                .set_actor_float(target, "fFlyToTime", elapsed)
                .expect("restore target FlyTo clock");
        }
        if let Some(duration) = original_target_fly_to.duration_seconds {
            engine
                .set_actor_float(target, "fFlyToTimeSpan", duration)
                .expect("restore target FlyTo duration");
        }
        if let Some(destination) = original_target_fly_to.destination {
            engine
                .set_actor_vector(target, "vFlyToDest", destination)
                .expect("restore target FlyTo destination");
        }
        let base_parked_at_start = engine.actor_location(base_cam).expect("BaseCam location");
        let target_parked_at_start = engine.actor_location(target).expect("target location");

        let crane_start = engine
            .find_actor_by_cut_name("StartCranePosition")
            .expect("authored crane start point");
        let crane_end = engine
            .find_actor_by_cut_name("EndCranePosition")
            .expect("authored crane endpoint");
        let target_start = engine
            .find_actor_by_cut_name("StartTargetPosition")
            .expect("authored target start point");
        let target_end = engine
            .find_actor_by_cut_name("EndTargetPosition")
            .expect("authored target endpoint");
        // The prototype serializes these two-point InterpolationPoint links
        // as package-local references. Resolve those authored links inside
        // the isolated test world; production spline resolution is untouched.
        engine
            .set_actor_object(crane_start, "Next", Some(crane_end))
            .expect("resolve CameraCrane endpoint link");
        engine
            .set_actor_object(target_start, "Next", Some(target_end))
            .expect("resolve TargetPath endpoint link");
        parse_loaded_cutscript(
            &mut engine,
            cut_thread,
            "BaseCam FollowSpline CameraCrane start=MissingPrototypePoint dest=EndCranePosition Time=18 *",
        );
        assert!(
            !engine.spline_motions.contains_key(&base_cam),
            "a source-rejected FollowSpline must not queue an engine motion"
        );
        // These are the shipped 00001PrivetIntro line_13/line_15 commands,
        // executed through the loaded CutScript parser and subject dispatch.
        parse_loaded_cutscript(
            &mut engine,
            cut_thread,
            "BaseCam FollowSpline CameraCrane EaseBetween start=StartCranePosition dest=EndCranePosition Time=18 accel=40 *",
        );
        for _ in 0..100 {
            if engine.spline_motions.contains_key(&base_cam) {
                break;
            }
            engine
                .tick(&[])
                .expect("resume loaded BaseCam FollowSpline dispatch");
        }
        let base_started_at = engine.sim_time;
        assert_eq!(
            engine.actor_location(base_cam),
            Some(base_parked_at_start),
            "BaseCam remains parked until its loaded 18s command reaches the scheduler"
        );
        parse_loaded_cutscript(
            &mut engine,
            cut_thread,
            "BaseCam Target FollowSpline TargetPath EaseBetween start=StartTargetPosition dest=EndTargetPosition Time=12 accel=60 *",
        );
        for _ in 0..100 {
            if engine.spline_motions.contains_key(&target) {
                break;
            }
            engine
                .tick(&[])
                .expect("resume loaded target FollowSpline dispatch");
        }
        let target_started_at = engine.sim_time;
        assert_eq!(
            engine.actor_location(target),
            Some(target_parked_at_start),
            "target remains parked until its loaded 12s command reaches the scheduler"
        );
        let active_snapshot = engine
            .base_cam_snapshot()
            .expect("BaseCam snapshot while both splines are active");
        assert_eq!(active_snapshot.follow_splines.len(), 2);
        assert!(active_snapshot.follow_splines.iter().any(|motion| {
            motion.actor == base_cam
                && motion.duration_seconds == 18.0
                && !motion.aligns_to_spline
        }));
        assert!(active_snapshot.follow_splines.iter().any(|motion| {
            motion.actor == target && motion.duration_seconds == 12.0
        }));
        let cutscript_class = engine
            .world
            .arena
            .find_by_path("HGame.CutScript")
            .expect("CutScript class");
        let cut_threads: Vec<_> = engine
            .level
            .actors
            .iter()
            .copied()
            .filter(|actor| {
                engine
                    .world
                    .arena
                    .get(*actor)
                    .ok()
                    .and_then(|object| object.class_id)
                    .is_some_and(|class| {
                        engine
                            .world
                            .arena
                            .class_is_a(class, cutscript_class)
                            .unwrap_or(false)
                    })
            })
            .collect();
        for thread in &cut_threads {
            engine.actor_scripts.remove(thread);
            engine.actor_states.remove(thread);
        }
        engine
            .timers
            .retain(|timer| !cut_threads.contains(&timer.actor));
        let (base_endpoint_actor, base_endpoint) = {
            let motion = engine.spline_motions.get(&base_cam).unwrap();
            let endpoint = *motion.points.last().expect("BaseCam final interpolation point");
            (endpoint, engine.actor_location(endpoint).unwrap())
        };
        let (target_endpoint_actor, target_endpoint) = {
            let motion = engine.spline_motions.get(&target).unwrap();
            let endpoint = *motion.points.last().expect("target final interpolation point");
            (endpoint, engine.actor_location(endpoint).unwrap())
        };
        assert_ne!(base_endpoint_actor, base_cam);
        assert_ne!(target_endpoint_actor, target);

        let (base_start_camera, base_start_pixels) = render_selected(&engine, &mut renderer);
        let mut base_mid = None;
        let mut target_mid = None;
        let mut target_pre_deadline = None;
        let mut base_pre_deadline = None;
        let mut base_completed_at = None;
        let mut target_completed_at = None;
        while engine.spline_motions.contains_key(&base_cam)
            || engine.spline_motions.contains_key(&target)
        {
            if let Some(motion) = engine.spline_motions.get(&base_cam) {
                if motion.elapsed < motion.duration {
                    base_pre_deadline =
                        Some(engine.actor_location(base_cam).expect("BaseCam before deadline"));
                }
            }
            if let Some(motion) = engine.spline_motions.get(&target) {
                if motion.elapsed < motion.duration {
                    target_pre_deadline =
                        Some(engine.actor_location(target).expect("target before deadline"));
                }
            }
            let base_was_active = engine.spline_motions.contains_key(&base_cam);
            let target_was_active = engine.spline_motions.contains_key(&target);
            engine.tick(&[]).expect("advance authored spline clocks");
            if base_was_active && !engine.spline_motions.contains_key(&base_cam) {
                base_completed_at = Some(engine.sim_time);
            }
            if target_was_active && !engine.spline_motions.contains_key(&target) {
                target_completed_at = Some(engine.sim_time);
            }
            if base_mid.is_none()
                && engine
                    .spline_motions
                    .get(&base_cam)
                    .is_some_and(|motion| motion.elapsed >= 9.0)
            {
                base_mid = Some(render_selected(&engine, &mut renderer));
            }
            if target_mid.is_none()
                && engine
                    .spline_motions
                    .get(&target)
                    .is_some_and(|motion| motion.elapsed >= 6.0)
            {
                target_mid = Some((
                    engine.actor_location(target).expect("moving target"),
                    render_selected(&engine, &mut renderer),
                ));
            }
        }

        let base_final = engine.actor_location(base_cam).expect("BaseCam endpoint");
        let target_final = engine.actor_location(target).expect("target endpoint");
        assert_ne!(base_parked_at_start, base_final, "18s action must move BaseCam");
        assert_ne!(
            target_parked_at_start, target_final,
            "12s action must move the target"
        );
        assert_ne!(
            base_pre_deadline.expect("BaseCam pre-deadline pose"),
            base_endpoint,
            "BaseCam must not snap to its final point before the 18s deadline"
        );
        assert_ne!(
            target_pre_deadline.expect("target pre-deadline pose"),
            target_endpoint,
            "target must not snap to its final point before the 12s deadline"
        );
        assert_eq!(
            base_final, base_endpoint,
            "18s action reaches its actual final interpolation point"
        );
        assert_eq!(
            target_final, target_endpoint,
            "12s action reaches its actual final interpolation point"
        );
        let base_completed_at = base_completed_at.expect("18s completion tick");
        let target_completed_at = target_completed_at.expect("12s completion tick");
        assert!(
            (base_completed_at - (base_started_at + 18.0)).abs() < 1.0e-4,
            "BaseCam completes on its exact fixed-clock 18s scheduler deadline: start={base_started_at} completed={base_completed_at}"
        );
        assert!(
            (target_completed_at - (target_started_at + 12.0)).abs() < 1.0e-4,
            "target completes on its exact fixed-clock 12s scheduler deadline: start={target_started_at} completed={target_completed_at}"
        );

        let (base_mid_camera, base_mid_pixels) = base_mid.expect("9s rendered checkpoint");
        assert!(
            vector_distance(base_start_camera.location, base_mid_camera.location) > 1.0,
            "selected authored camera changes during the 18s action"
        );
        assert_ne!(
            base_start_pixels, base_mid_pixels,
            "rendered pixels must change when the selected camera moves"
        );
        let (target_mid_location, (target_mid_camera, target_mid_pixels)) =
            target_mid.expect("6s target checkpoint");
        assert_ne!(target_parked_at_start, target_mid_location);
        if target_mid_camera.location != base_mid_camera.location
            || target_mid_camera.rotation != base_mid_camera.rotation
        {
            assert_ne!(
                base_mid_pixels, target_mid_pixels,
                "a changed selected camera at the target checkpoint must change rendered pixels"
            );
        }

        let captured = engine.cutscene_camera().expect("captured BaseCam selected");
        let (_, deadline_pixels) = render_selected(&engine, &mut renderer);
        parse_loaded_cutscript(&mut engine, cut_thread, "Release BaseCam");
        assert!(
            engine.cutscene_camera().is_none(),
            "Release removes the cutscene override"
        );
        let released_snapshot = engine
            .base_cam_snapshot()
            .expect("live BaseCam remains observable after Release");
        assert_eq!(released_snapshot.cut_notify_actor, None);
        assert_eq!(released_snapshot.follow_splines, Vec::new());
        let pawn = engine.pawn_camera().expect("pawn camera after Release");
        let selected = engine
            .cutscene_camera()
            .or_else(|| engine.pawn_camera())
            .expect("post-Release camera selection");
        assert_eq!(selected.location, pawn.location);
        assert_eq!(selected.rotation, pawn.rotation);
        assert!(
            captured.location != selected.location || captured.rotation != selected.rotation,
            "Release visibly returns selection from BaseCam to the pawn camera"
        );
        let (_, released_pixels) = render_selected(&engine, &mut renderer);
        assert_ne!(
            deadline_pixels, released_pixels,
            "Release camera selection must be observable in rendered output"
        );
        renderer.shutdown();
    }
}
